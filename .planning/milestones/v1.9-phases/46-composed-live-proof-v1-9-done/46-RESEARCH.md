# Phase 46: Composed Live Proof (v1.9 DONE) — Research

**Researched:** 2026-07-18
**Domain:** Composition + acceptance test (little/no new TCB). Wiring the shipped
v1.9 write sinks + policy + CLI/viewer into ONE genuine composed live run on real
Linux, driven via `caprun run` + inspected via `caprun audit`, with 5
independently-attributable negative legs. This is the v1.9 DONE gate.
**Confidence:** HIGH (all claims traced to code at current HEAD; no external deps)

## Summary

Phase 46 is a composition phase: every sink, mint path, confirm-release, audit
event, and mock endpoint it needs already ships and is proven in isolation
(s43/s44 differentials, s45 CLI-viewer loop, the v1.8 composed test). The work is
(a) chaining the success legs over ONE shared persisted `audit.db`, (b) reusing
the shipped negative-leg assertions, (c) inspecting the whole run through the real
`caprun audit` subprocess, and (d) closing three concrete wiring gaps. The v1.8
composed test (`cli/caprun/tests/live_acceptance_v1_8_composed.rs`) is the exact
structural template — one session per leg, per-session `verify_chain` true, a
final sweep asserting the exact session set.

Three wiring gaps dominate and must be resolved before this is GENUINE, not
stubbed: (1) `caprun run` cannot drive the v1.9 write sinks or a multi-sink chain
— only two single-node intents exist (email/file), so "driven via `caprun run`"
for the whole chain needs a decision; (2) a `PolicyDeny` records as the *generic*
`plan_node_evaluated` audit event (same as an Allow), so the audit DAG cannot by
itself distinguish policy-deny from allow — the "distinct machine-checkable tag"
for LIVE-06 leg 3 needs a home; (3) the mock server serves NO 2xx `http.request.write`
POST endpoint on the write-allowlisted host, so a clean POST currently 404s →
`http_write_failed`, never `http_write_succeeded`.

**Primary recommendation:** Build a faithful in-crate composed test (v1.8 pattern)
that drives every leg through the REAL broker path over one shared persisted
`audit.db`, THEN inspect the whole run via a genuine `caprun audit <session> <db>`
subprocess; additionally drive the acceptance-critical I2-Block leg via a genuine
`caprun run` subprocess (s45 pattern) to literally satisfy "driven via CLI." Add
one mock POST endpoint (mirror the git-receive-pack addition) and assert the
policy-deny distinct tag at the decision level (no new TCB). Run under
`scripts/compose-verify.sh` with `--features brokerd/mock-egress-ca`.

## User Constraints (from ROADMAP / REQUIREMENTS — no CONTEXT.md yet)

### Locked (LIVE-05 / LIVE-06, `.planning/REQUIREMENTS.md:43-44`)
- Composed workflow: `process.exec` (test) → filesystem edit → `git.commit` →
  `git.push` → `github.pr` PLUS an `http.request` POST leg, on real Linux (mock git
  remote + mock endpoint), **DRIVEN via `caprun run` AND INSPECTED via `caprun
  audit`** (SDK-01/U1 on the acceptance critical path), every step
  gated/tainted/audit-DAG-chained, `verify_chain` true across the run.
- Five **independently attributable** negative legs, distinct machine-checkable
  tags asserted separately: (1) tainted push remote/refspec I2-Blocks; (2) tainted
  POST body I2-Blocks; (3) policy-deny leg (off-allowlist sink refused via the
  distinct `policy_deny` outcome) where the I2-Block legs run a sink+arg the policy
  explicitly PERMITS; (4) destination-pin negative (push/POST redirected/off-pin
  refused at broker/app layer); (5) credential-absence after a real push (no
  credential/remote-URL in value store OR audit chain — DESIGN §1.4 extends to
  broker-log).
- Full-workspace regression green on real Linux, no v1.0–v1.8 regression.
- git.push safety-valve: GIT-02 SHIPPED (not deferred) — the auto-descope branch
  does NOT apply. Phase 44 is complete (`.planning/ROADMAP.md:88`); the composed
  proof INCLUDES the git.push leg.

### Claude's Discretion
- Exactly how "driven via `caprun run`" is satisfied for a multi-sink chain the
  single-node planner cannot express (see Gap G1 for the recommended split).
- Where the policy-deny distinct tag is asserted (decision-level vs a new audit
  event — see Gap G2; decision-level recommended to avoid new TCB).
- Mock write-endpoint path/name (see Gap G3).

### Out of scope
- 10MB pack-cap enforcement (deferred; Phase 46 pushes a SMALL mock repo →
  non-blocking, note it — REQUIREMENTS Out of Scope).
- Any new sink, new planner LLM behavior, Cedar policy, or seccomp-based pinning.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LIVE-05 | Composed workflow driven via CLI + inspected via viewer, verify_chain true | v1.8 composed template + s45 CLI loop + all sink invokers exist (anchor table) |
| LIVE-06 | 5 independently-attributable negative legs + regression green | s43 leg_b, s44 leg_b/leg_c/leg_e reusable; policy-deny + mock-write gaps identified |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Drive the composed run | CLI (`caprun run`) / in-crate test harness | — | LIVE-05 wants CLI-driven; planner limits force a hybrid (G1) |
| Sink dispatch + taint gate | Broker (`brokerd`) + executor TCB | — | All effect authorization is broker-mediated; unchanged |
| Confirm-release (git.push) | Broker `confirmation::confirm` | CLI `caprun confirm` | git.push always confirm-gated (server.rs:791) |
| Policy narrowing (deny) | Executor `policy_gate` (pre-I2) | — | `Denied{PolicyDeny}`, never a Block (policy_gate.rs) |
| Inspect / verify_chain | CLI viewer (`caprun audit`) | — | Read-only trust surface (s45; main.rs run_audit_viewer) |
| Mock endpoints (PR/push/POST) | `scripts/mock-github/server.py` | compose-verify sidecar | Real TLS under `mock-egress-ca` |

## Verified Anchor Table — EXISTS-to-reuse vs NEW

Success-chain legs (all sink invokers ship today):

| Leg | Shipped entry point (file:line) | Status |
|-----|-------------------------------|--------|
| process.exec | `brokerd::sinks::process_exec::invoke_process_exec`; used `live_acceptance_v1_8_composed.rs:328` | REUSE |
| filesystem edit | `brokerd::sinks::file_write::invoke_file_write`; used v1.8 composed:416 | REUSE |
| git.commit | `brokerd::sinks::git_commit::invoke_git_commit`; v1.8 composed:487 | REUSE |
| github.pr | `brokerd::sinks::github_pr::invoke_github_pr_from_resolved`; v1.8 composed:593 (mock 201) | REUSE |
| http GET (mint) | `brokerd::sinks::http_request::invoke_http_get` + `mint_from_http`; v1.8 composed:655 | REUSE (optional) |
| **git.push** | `server::evaluate_plan_node_and_record_for_test` → always-confirm-gate → `confirmation::confirm`; `s44_git_push_differential.rs:553` `evaluate_and_confirm` | REUSE (Linux+mock-egress-ca) |
| **http.request.write POST** | Allowed dispatch arm → `sinks::http_write::invoke_http_write_sink` (server.rs:1641-1657) | REUSE dispatch; **NEW mock endpoint (G3)** |

Negative legs:

| # | LIVE-06 leg | Shipped proof (file:line) | Status |
|---|-------------|---------------------------|--------|
| 1 | tainted push remote/refspec I2-Blocks | `s44_git_push_differential.rs:248` `legs_b_and_c` (B1/B2 anchor on `remote`/`refspec`) | REUSE |
| 2 | tainted POST body I2-Blocks | `s43_http_write_differential.rs:242` `legs_b_and_c` + dispatch `dispatch_tainted_body_blocks_and_never_writes:588` | REUSE |
| 3 | policy-deny distinct tag | `executor::policy_gate` → `Denied{PolicyDeny code="policy_deny"}` (executor_decision.rs:133); I2 legs use `broker_default()` which PERMITS the sinks (policy.rs:232) | **NEW pairing + assertion (G2)** |
| 4 | destination-pin negative | `s44 leg_e_receive_pack_redirect_is_refused:707` (302 refused → `git_push_failed`, `ConfirmedButSinkFailed`) + http `ssrf_check` (v1.8 composed adv-b:875) | REUSE |
| 5 | credential-absence after real push | `s44 leg_c_clean_confirmed_push_reaches_mock_receive_pack:648` asserts `TOKEN_SENTINEL`+`REMOTE` absent from all payloads+actor | REUSE; **extend to broker-log per DESIGN §1.4 (G4)** |

Inspection / infra:

| Capability | Shipped (file:line) | Status |
|-----------|--------------------|--------|
| `caprun audit` renders DAG + `Chain verification: PASSED` + `sink_blocked` + pending decisions | `main.rs:908` `run_audit_viewer`; proven `s45_cli_viewer_acceptance.rs:380-401` | REUSE |
| `caprun run` → I2 Block → surfaced effect_id → review/confirm/audit loop | `s45_cli_viewer_acceptance.rs:322` `end_to_end_run_block_surface_review_audit` | REUSE (Block leg only — G1) |
| Shared-DB composed pattern (one session/leg, per-session verify_chain, final sweep) | `live_acceptance_v1_8_composed.rs:267-951` | TEMPLATE |
| compose-verify harness (Mailpit + mock-GitHub sidecars, mock-egress-ca, PUBLIC subnet, feature-OFF guard) | `scripts/compose-verify.sh` | REUSE |

## How to Drive Composed + Inspect (the LIVE-05 core)

**Constraint (verified):** `caprun run <intent> ...` drives ONE `CaprunIntent` →
ONE `PlanNode` → ONE sink. Only two intents exist — `SendEmailSummary`→`email.send`
and `CreateFileFromReport`→`file.create` (`crates/runtime-core/src/intent.rs:22-46`;
`cli/caprun/src/planner.rs:154-210` maps each to exactly one node). **None of the
v1.9 write sinks (process.exec, git.commit, git.push, github.pr,
http.request.write) is reachable via `caprun run`.** A single `caprun run` cannot
express the multi-sink chain.

**Recommended approach (faithful composition, no new TCB, matches v1.8 precedent):**
1. Drive the full success chain IN-CRATE over ONE shared persisted `audit.db`
   (never `:memory:`), each leg its own session, exactly as
   `live_acceptance_v1_8_composed.rs` does — but through the REAL production arms:
   `evaluate_plan_node_and_record_for_test` (server.rs:1679, `test-fixtures`-gated
   verbatim delegate to the live arm) for the decision+dispatch, and
   `confirmation::confirm` for git.push confirm-release. This exercises the same
   broker path the live daemon runs (closes the Phase-38 mirror-drift finding).
2. INSPECT the whole run by spawning the REAL compiled `caprun audit <session_id>
   <db>` subprocess (env `CARGO_BIN_EXE_caprun`) per composed session and asserting
   `Chain verification: PASSED`, the rendered sink/terminal events, and the pending
   decisions — the same viewer proven in s45.
3. Additionally drive the acceptance-critical I2-Block leg via a genuine `caprun
   run --policy <trusted> create-file-from-report ...` subprocess (verbatim s45
   `run_genuine_hostile_file_create`) so the run is LITERALLY driven via the CLI on
   at least one confined leg, and its block is surfaced + reviewed + audited.

This satisfies "driven AND inspected via the CLI + viewer" pragmatically: the
inspection is 100% real CLI, and the CLI genuinely drives a confined blocking run;
the multi-sink success chain is composed through the identical broker arms the CLI
would call. **The alternative — a strict reading requiring the WHOLE chain driven
by one `caprun run` — requires a NEW composed intent + a multi-node planner recipe
(new non-TCB code), materially enlarging scope. Flag for the planner/user (G1).**

## The git.push confirm-release step (composed happy path)

git.push is ALWAYS confirm-gated — there is NO Allowed→auto-dispatch arm
(server.rs:791-808 always-confirm-gate rewrites even a clean Allowed git.push into
`BlockedPendingConfirmation` and freezes the new-oid). The exact composed pattern
ships in `s44_git_push_differential.rs:553` `evaluate_and_confirm`:

1. Mint clean `remote`/`refspec` via `mint_from_intent` (session stays Active →
   executor Allows).
2. `evaluate_plan_node_and_record_for_test(...)` → asserts
   `BlockedPendingConfirmation` (always-confirm-gate), no socket yet, one
   `pending_confirmations` row.
3. Read `effect_id` from `pending_confirmations`; take sole ownership of the conn;
   `confirmation::confirm(&mut conn, key, &effect_id, &ws)` → `ConfirmOutcome::Released`
   → one opaque `git_push_succeeded` terminal event; `verify_chain` holds.

Reuse verbatim in the composed run. The mock repo is a SMALL one-commit repo
(`setup_git_push_repo:521`), so the 10MB pack-cap is non-blocking (note it). This
leg is `#[cfg(all(target_os="linux", feature="mock-egress-ca"))]`.

## The 5 negative legs — mapping detail

- **Leg 1 (tainted remote/refspec):** REUSE `s44 legs_b_and_c`. Anchor names the
  tainted arg; `sink_blocked` event; genuine `mint_from_http` provenance.
- **Leg 2 (tainted POST body):** REUSE `s43 legs_b_and_c` (decision) +
  `dispatch_tainted_body_blocks_and_never_writes` (dispatch: `sink_blocked`, no
  `http_write_*`). Anchor names `body`.
- **Leg 3 (policy-deny distinct tag) — NEW:** `executor::policy_gate` runs BEFORE
  the I2 loop and returns `Denied{PolicyDeny{ constraint:"sink-not-allowed" }}`,
  `code()=="policy_deny"` (policy_gate.rs:53-62; executor_decision.rs:94,133), a
  `Denied` outcome NEVER a `BlockedPendingConfirmation`. **Distinctness setup:**
  build a session policy that PERMITS the I2-legs' sink (e.g. allows
  `http.request.write` / `git.push`) but OMITS one other sink; submit that omitted
  sink → `PolicyDeny`. The I2-Block legs run `SessionPolicy::broker_default()`
  which permits their sinks (policy.rs:232 `PRODUCTION_SINKS`) — so policy is
  provably NOT what blocks them.
  **GAP (G2):** a `Denied` decision (incl. PolicyDeny) is recorded as the GENERIC
  `plan_node_evaluated` audit event — the SAME event an Allow produces; only a
  Block gets its own `sink_blocked` type (server.rs:949-982; confirmed
  `cli/caprun/src/planner.rs:556-557`). So `caprun audit` CANNOT by itself
  distinguish policy-deny from allow in the DAG. Two distinct machine-checkable
  tags are available WITHOUT new TCB: the decision-level `code()=="policy_deny"`
  (returned by `submit_plan_node`) vs the `sink_blocked` event type. Recommend
  asserting leg 3 at the decision level (policy-deny → `Denied{PolicyDeny}.code()`;
  I2 legs → `BlockedPendingConfirmation` + `sink_blocked` event). If a
  DAG-visible distinct tag is mandated, that is a small broker change (append a
  distinct `plan_node_denied` event carrying `reason.code()`) — TCB, larger scope;
  flag for decision.
- **Leg 4 (destination-pin negative):** REUSE `s44 leg_e` (receive-pack 302
  refused → `git_push_failed`, `ConfirmedButSinkFailed`; frozen redirect-none
  client) — proves the pin holds (redirect refused at the broker app layer), not
  merely that a happy push reaches a listener. Optionally also the http SSRF
  pin (`ssrf_check` on metadata/RFC1918, v1.8 composed adv-b:875).
- **Leg 5 (credential-absence after real push):** REUSE `s44 leg_c` assertions
  (`assert_absent_from_all_payloads` for `TOKEN_SENTINEL` + `REMOTE` across all
  event payloads AND actor columns, s44:685-696). **GAP (G4):** DESIGN §1.4
  (`DESIGN-v1.9-egress-policy.md:212-220`) requires the credential-absence
  assertion cover the broker LOG sink too, not only value store + audit chain. The
  existing s44 leg asserts the audit DB only. Confirm whether the composed leg must
  additionally assert no credential/remote-URL substring in captured broker log
  output (the `do_pinned_post` error path at `http_request.rs:542` can echo URL
  material on 401/407).

## Mock http-write endpoint status — GAP (G3)

- Under `mock-egress-ca`, `WRITE_HOST_ALLOWLIST` admits EXACTLY ONE host:
  `MOCK_EGRESS_HOST = "github-mock.caprun.test"` (`http_request.rs:123,130,164-173`;
  test `write_allowlist_feature_on_admits_only_the_mock_host:1234`). The GET-only
  `api.github.com` is NOT write-allowlisted.
- `invoke_http_write` appends `http_write_succeeded` ONLY on a 2xx status
  (`sinks/http_write.rs:174` `(200..300).contains(&status)`); anything else →
  `http_write_failed`.
- The mock (`scripts/mock-github/server.py:175-196` `do_POST`) returns 201 ONLY for
  `/repos/<owner>/<repo>/pulls` (github.pr) and handles `/accept/*/git-receive-pack`
  (git.push); **every other POST path → 404.** There is NO generic write endpoint.
- Consequence: a clean `http.request.write` POST to
  `https://github-mock.caprun.test/<anything>` currently 404s → `http_write_failed`,
  never `http_write_succeeded`. The s43 test uses host `mock-write.caprun.test`
  (`s43:63`) which is not even allowlisted — it bails pre-socket by design.
- **Fix:** add a dedicated 2xx write endpoint to `server.py` `do_POST` (e.g.
  `POST /ingest` → 201, records a receipt to the in-memory ledger + stderr, mirror
  `_handle_receive_pack:198`), and target the composed write leg at
  `https://github-mock.caprun.test/ingest`. This mirrors the Phase-44 WG-9
  git-receive-pack mock addition — a pure test-double change, no TCB. The
  feature-OFF guard (compose-verify.sh:104) already proves the mock host is absent
  from release builds; no new guard needed.

## Wiring Gaps (highest-value output)

| ID | Gap | Impact | Recommended resolution |
|----|-----|--------|------------------------|
| **G1** | `caprun run` drives only single-node email/file intents — cannot drive the v1.9 write sinks or the multi-sink chain | LIVE-05 "driven via `caprun run`" not literally satisfiable for the write chain | Faithful in-crate composition through the REAL broker arms + real `caprun audit` inspection + one genuine `caprun run` Block leg (s45). Strict alternative (new composed intent + multi-node planner) = new code; flag for user sign-off. |
| **G2** | `PolicyDeny` records as generic `plan_node_evaluated` (same as Allow); no distinct audit-DAG event type | LIVE-06 leg 3 "distinct machine-checkable terminal-event tag" not visible in `caprun audit` | Assert leg 3 at decision level (`code()=="policy_deny"` vs `sink_blocked` event). If DAG-visible tag required → small broker change (distinct denied event) = TCB, larger scope. |
| **G3** | Mock has no 2xx `http.request.write` POST endpoint on the write-allowlisted host | Clean POST leg 404s → `http_write_failed`, so no genuine delivery / `http_write_succeeded` | Add `POST /ingest`→201 (+ receipt ledger) to `server.py`; target the write leg at `github-mock.caprun.test/ingest`. Mirror git-receive-pack addition. |
| **G4** | Credential-absence (leg 5) asserts audit DB only; DESIGN §1.4 extends to broker-log sink | Potential under-proof of the §1.4 broker-log leak vector | Capture broker log output on the push leg and assert no credential/remote-URL substring, OR confirm §1.4 is satisfied by the do-not-log discipline already in place. |
| G5 | Feature/cfg gating: dispatch + confirm helpers are `#[cfg(feature="test-fixtures")]`; live legs `#[cfg(all(target_os="linux", feature="mock-egress-ca"))]` | A macOS `cargo test` reports 0 composed Linux tests (expected, not a gap — CLAUDE.md) | Gate the composed body `#[cfg(target_os="linux")]`, keep a host-portable guard test (v1.8 composed:961), run ONLY via `scripts/compose-verify.sh`. Ensure `caprun`/`caprun-worker`/`caprun-exec-launcher` built first (compose-verify does `cargo build --workspace`). |
| G6 | Env-mutation races (`CAPRUN_GITHUB_TOKEN`, `CAPRUN_GIT_PUSH_TOKEN`, `CAPRUN_GITHUB_API_BASE`) | Cross-leg env bleed if parallelized | Single test fn, sequential legs, `set_var`/`remove_var` around each leg (v1.8 composed:529, s44:655). |

## Common Pitfalls

### Pitfall 1: Stapled taint (§9 anti-stapling)
**What goes wrong:** minting a tainted value with a hand-set taint field or empty
provenance, so the Block rides no real DAG edge.
**How to avoid:** mint via the REAL broker paths (`mint_from_http`/`mint_from_exec`/
`mint_from_intent`) and assert `anchor.provenance_chain[0] == read_event_id` equals
a real `http_response_received`/`process_exited` event id (v1.8 composed:754-759;
s44:398-401). The DONE gate requires a GENUINE unbroken taint edge.

### Pitfall 2: `:memory:` or per-leg fresh DB
**What goes wrong:** breaks the "one shared persisted audit.db, per-session
verify_chain" contract; `caprun audit` refuses `:memory:` (s45:172).
**How to avoid:** one shared file `audit.db` + sibling `.key`, F1-safe layout
(sibling of workspace roots, never nested) — v1.8 composed:277; s45:116.

### Pitfall 3: verify_chain fork on tainted mints
**What goes wrong:** threading a `sink_blocked` onto the `http_response_received`
id instead of the LAST-appended `session_demoted` head forks the DAG, breaking
`verify_chain`.
**How to avoid:** thread onto the chain head the mint returns
(`demoted_id`/`chain_head_id`) — v1.8 composed:717-737; s43:156-166.

### Pitfall 4: exit code through a pipe / false PASS
**How to avoid:** compose-verify captures `rc` BEFORE any pipe and asserts on named
tests + counts (compose-verify.sh:195-205; MEMORY `verification-exit-code-through-pipe`).

## Runtime State Inventory

Not a rename/refactor phase — greenfield composed test + one mock endpoint + docs.
No stored data, live-service config, OS-registered state, secrets, or build
artifacts carry a renamed string. **None — verified: this phase only adds a test
file, a mock endpoint, and milestone records.**

## Environment Availability

| Dependency | Required by | Available | Notes |
|-----------|-------------|-----------|-------|
| Docker / Colima | compose-verify sidecars (Mailpit + mock-GitHub) | ✓ (dev Mac) | CLAUDE.md Linux verification recipe |
| `rust:1` container | Linux enforcement + socket legs | via compose-verify | seccomp=unconfined, no `--privileged` |
| Python 3 (python:3-slim) | mock-github server.py | via sidecar | stdlib only, no pip |
| `git` binary in rust:1 | git.commit/git.push confined children | ✓ | compose-verify installs libssl-dev/pkg-config |
| `mock-egress-ca` cargo feature | trust the mock TLS cert + admit write/push hosts | non-default | enabled by compose-verify test step |

**Blocking:** none. All infra ships and is exercised by the v1.8 composed run today.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio; `cargo test --workspace` |
| Quick run | `cargo test -p caprun --test <name>` (host; Linux legs cfg-excluded on macOS) |
| Full / authoritative | `bash scripts/compose-verify.sh` (Linux, mock-egress-ca, both sidecars) |

### Phase Requirements → Test Map
| Req | Behavior | Type | Command | Exists? |
|-----|----------|------|---------|---------|
| LIVE-05 | composed success chain + caprun audit inspection + verify_chain | live/integration | `COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test <new-composed> --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh` | ❌ Wave 0 (new test) |
| LIVE-06 | 5 negative legs, distinct tags, regression green | live/integration | same harness, full suite | ⚠️ legs 1/2/4/5 reuse; leg 3 + mock-write new |

### Sampling
- Per task: `cargo test -p caprun --test <new-composed>` (macOS compiles, Linux body cfg-excluded).
- Phase gate: full `bash scripts/compose-verify.sh` green (no v1.0–v1.8 regression) before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] New composed test file (e.g. `cli/caprun/tests/live_acceptance_v1_9_composed.rs`) — covers LIVE-05/06.
- [ ] `POST /ingest`→201 endpoint in `scripts/mock-github/server.py` — covers the http-write delivery leg.
- [ ] Host-portable guard test (mirrors v1.8 composed:961) so macOS `cargo test -p caprun` stays meaningful.

## Security Domain

`security_enforcement` enabled. This phase adds no new authorization code; it
re-demonstrates the shipped I0/I1/I2 + policy boundary live.

| ASVS | Applies | Control (unchanged, re-proven) |
|------|---------|-------------------------------|
| V5 Input Validation | yes | I2 taint gate on tainted remote/refspec/body (legs 1/2) |
| V4 Access Control | yes | policy_gate deny-by-default (leg 3); git.push always-confirm-gate |
| V6 Cryptography | yes | keyed SHA-256 audit hash chain, `verify_chain` across the run |
| V9 Communications | yes | SSRF resolve-and-pin + redirect-none destination pin (leg 4) |
| V2/V3 secrets | yes | credential-absence, broker-env-only custody (leg 5, DESIGN §1.4) |

| Threat | STRIDE | Mitigation re-proven |
|--------|--------|----------------------|
| Exfil via tainted push/POST payload | Tampering/Info-disclosure | I2 Block on tainted arg (legs 1/2) |
| Destination-pin bypass via redirect | Tampering | frozen redirect-none client refuses 3xx (leg 4) |
| Credential leak into audit/log | Info-disclosure | opaque audit + broker-env-only + log-scrub (leg 5) |
| Policy bypass of I2 | Elevation | policy narrows, NEVER disables I2 (leg 3 distinctness) |

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | Decision-level `code()=="policy_deny"` satisfies LIVE-06 leg 3 "distinct machine-checkable terminal-event tag" | Leg 3 / G2 | If a DAG-VISIBLE tag is mandated, needs a broker change (new denied event) — scope grows into TCB |
| A2 | "Driven via `caprun run`" is satisfied by CLI-driving the Block leg + CLI-inspecting the composed run (not the whole chain via one run) | LIVE-05 / G1 | Strict reading needs a new composed intent + multi-node planner (new code) |
| A3 | Adding `POST /ingest`→201 to the mock is acceptable (test-double only, no TCB, feature-gated host) | G3 | Low — mirrors the shipped git-receive-pack mock precedent |
| A4 | Existing s44 credential-absence (audit DB) plus a broker-log assertion fully covers DESIGN §1.4 | Leg 5 / G4 | If §1.4 demands more (e.g. proxy-auth echo paths), leg 5 needs extra capture |

## Open Questions

1. **Whole-chain `caprun run` vs hybrid (G1/A2)** *(RESOLVED — hybrid locked: in-crate composition through the real arms + genuine `caprun audit` + one genuine `caprun run` Block leg; no multi-node planner)* — does the user require the entire
   multi-sink chain literally driven by one `caprun run` invocation? Recommendation:
   accept the hybrid (in-crate composition through real arms + real `caprun audit`
   inspection + one genuine `caprun run` Block leg); if not, add a scoped
   composed-intent + planner recipe as a separate task.
2. **Policy-deny tag home (G2/A1)** *(RESOLVED — decision-level assertion locked: `code()=="policy_deny"` vs `sink_blocked`, asserted separately; no new TCB / no DAG-visible denied event)* — decision-level assertion (no TCB) vs a new
   DAG-visible denied event (TCB). Recommendation: decision-level.
3. **Broker-log credential-absence (G4/A4)** *(RESOLVED — in-scope: leg 5b asserts broker-log absence on the ERROR-PATH push where `scrub_secrets`→`eprintln!` fires, via FD-2 capture; the clean-push log check would be vacuous)* — is capturing+asserting broker log
   output in-scope for leg 5, or is the do-not-log discipline sufficient?

## Suggested Plan Breakdown (3 plans, ~2 waves)

- **Plan 46-01 (infra; Wave 1, no deps):** add `POST /ingest`→201 (+ receipt) to
  `scripts/mock-github/server.py` (G3); confirm compose-verify picks it up; if the
  policy-deny is asserted at decision-level (recommended) no broker change — else
  scope the distinct-denied-event decision here. Small, self-contained.
- **Plan 46-02 (composed success proof; Wave 2, depends 46-01):** new Linux-gated
  `live_acceptance_v1_9_composed.rs` — success chain process.exec → file edit →
  git.commit → git.push (confirm-release) → github.pr → http.request.write POST over
  ONE shared persisted `audit.db`, each leg its own session, driven through the REAL
  broker arms; then inspect via genuine `caprun audit` subprocess per session
  (verify_chain PASSED); + the s45-style genuine `caprun run` Block leg; final
  session sweep. Host-portable guard test.
- **Plan 46-03 (negative legs + regression + record; Wave 2, depends 46-01):** the 5
  independently-attributable negative legs (reuse s43/s44 patterns; new policy-deny
  distinct-tag pairing; leg-5 credential-absence incl. broker-log per §1.4); full
  `compose-verify` green with no v1.0–v1.8 regression; write the v1.9 milestone
  DONE record + human sign-off (git.push SHIPPED, safety-valve not triggered);
  reconcile LIVE-05/06 in REQUIREMENTS.

Wave 1: 46-01. Wave 2: 46-02 ∥ 46-03. (A 4th plan is only needed if the user
mandates whole-chain `caprun run` driving — then split the new composed
intent+planner recipe out as 46-04, Wave 1.)

## Sources

### Primary (HIGH — traced at current HEAD)
- `cli/caprun/tests/live_acceptance_v1_8_composed.rs` — composed template, shared-DB pattern, anti-stapling, sweep.
- `cli/caprun/tests/s45_cli_viewer_acceptance.rs` — `caprun run`→Block→review→`caprun audit` loop; viewer guarantees.
- `crates/brokerd/tests/s44_git_push_differential.rs` — git.push legs (evaluate_and_confirm, leg_c delivery+cred-absence, leg_e redirect).
- `crates/brokerd/tests/s43_http_write_differential.rs` — http-write differential + dispatch legs.
- `scripts/compose-verify.sh` + `scripts/mock-github/server.py` — mock harness (PR/receive-pack; no write endpoint).
- `crates/brokerd/src/sinks/http_request.rs` (WRITE_HOST_ALLOWLIST/MOCK_EGRESS_HOST) + `sinks/http_write.rs` (2xx success).
- `crates/brokerd/src/server.rs:791-808,949-982,1641-1657` (always-confirm-gate; generic Denied event; http-write dispatch arm).
- `crates/runtime-core/src/{policy.rs,executor_decision.rs}` + `crates/executor/src/policy_gate.rs` — PolicyDeny outcome, `code()`.
- `cli/caprun/src/{planner.rs,main.rs}` + `crates/runtime-core/src/intent.rs` — single-node planner, 2 intents, run/audit surface.
- `planning-docs/DESIGN-v1.9-egress-policy.md` §1.4/§1.5/§2 — credential custody, destination pin, policy↔I2 boundary.
- `.planning/REQUIREMENTS.md:43-44` / `.planning/ROADMAP.md:268-280` — LIVE-05/06 locked text.

## Metadata

**Confidence breakdown:**
- Anchor table (exists-vs-new): HIGH — every entry cited to a shipped test/function.
- Wiring gaps G1/G2/G3: HIGH — traced to intent enum, event-recording arm, and mock do_POST.
- Plan breakdown: MEDIUM — depends on the G1/G2 decisions above.

**Research date:** 2026-07-18
**Valid until:** ~2026-08-17 (stable internal codebase; re-verify if sinks/planner change).
