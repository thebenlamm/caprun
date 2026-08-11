---
phase: 51
slug: non-hybrid-live-proof-v1-10-done
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
block_on: high
created: 2026-08-09
register_authored_at_plan_time: true
threats_total: 60
threats_closed: 60
---

# Phase 51 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

**Register origin:** authored at plan time. All nine plans (`51-01` … `51-09`) shipped a
`<threat_model>` block. This audit **verifies the registered mitigations exist** in the
implementation; it is not a fresh threat scan.

**Blocking threshold:** `high` (OWASP ASVS L1). Only `open` threats at or above `high`
count toward `threats_open`.

---

## Trust Boundaries

Union of the boundaries declared across the nine plan threat models.

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Test harness → real `caprun` binary | Ambient env vars and argv are untrusted test input; the product path must not auto-grant, auto-confirm, or select a planner from ambient state | Env vars, argv, policy path |
| Parent `caprun` → confined worker | `env_clear()` plus an explicit allowlist; only the non-secret `CAPRUN_CODING_I2_PROOF` may cross | Non-secret feature flag only (tokens stay parent/broker-side) |
| Worker → broker UDS | Plan nodes only; no free-form effect path exists | `PlanNode { sink, args: Vec<ValueNode> }` |
| Broker → mock GitHub / push TLS | `mock-egress-ca` admits mock hosts only; SSRF guard restricts to public ranges | HTTPS requests, opaque tokens |
| Proof planner → sensitive sink args | Deliberate `out_*` placement must trip I2, never bypass it | Untrusted `process.exec` output → `github.pr`/`body` |
| Policy layer → I2 | Policy may omit sinks but must never be used to fake an I2 Block | Sink permission set |
| Out-of-band CLI process → shared audit SQLite | `caprun grant` / `confirm` / `deny` append to the DB the broker is writing — the D1 boundary | Audit events, capability rows |
| Causal `parent_id` chain ↔ provenance `read_event_id` graph | Two graphs that must never be equated; I2 and M7 ride on the second | Event ids, provenance chains |
| Audit viewer → integrity claim | `verify_chain` / `caprun audit` is the trust surface backing any DONE claim | Hash chain, MACs |
| Host → Docker compose environment | Proof depends on actual Linux/Docker execution, not on test source presence | Compose exit status, raw logs |
| Command output → evidence markdown | Results must be transcribed without converting failures into successes | stdout/stderr, SHA-256 |
| Evidence → requirements / ledger | Completion is authorized only by green scoped **and** full gates | Requirement + window status |
| Authoring context → reviewing context | A self-review looks identical in the record but provides no assurance | Design-gate clearance |
| Landed diff → phase completion claim | A fix that loosens the verifier passes tests while deleting HARDEN-02 detection | Verifier / MAC functions |

---

## Threat Register

### Plan 51-01 — CLI-driven LIVE-07 success path

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-01 | Spoofing | LIVE-07 framing / DONE claim | high | mitigate | `LIVE_07_DRIVER` / `LIVE_07_NOT` framing pins at `live_acceptance_v1_10_cli.rs:21-22`; real binary spawned via `CARGO_BIN_EXE_caprun` (`:112`, `:214`); hybrid composition excluded from the DONE claim | closed |
| T-51-02 | Elevation | dual-Session stitch after push | high | mitigate | `session_ids.len()` asserted `== 1` (`:411-420`); a second `session_id=` line errors out at `:288` | closed |
| T-51-03 | Spoofing | auto-grant / auto-confirm | medium | mitigate | `CAPRUN_CONFIRM=external` (`:373`, `:451`); grant/confirm driven only by out-of-band sidecar (`:254-290`); no auto-mint in worker | closed |
| T-51-04 | Info disclosure | `CAPRUN_*` tokens in audit DAG | medium | mitigate | Tokens set on the parent only (`:375-376`, `:453-454`) and excluded by the worker `env_clear` allowlist (`main.rs:558-627`); production `scrub_secrets` at `git_push.rs:729` | closed |
| T-51-05 | Tampering | free-form effect path | high | mitigate | `check-invariants.sh` Gate 1 green (re-run 2026-08-09, Gates 1–6 pass); plan nodes only | closed |
| T-51-SC (01) | Tampering | package installs | low | accept | Zero new packages this phase — see Accepted Risks `AR-01` | closed |

### Plan 51-02 — Genuine mid-loop I2 Block (LIVE-08)

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-10 | Tampering / Elevation | `out_*` success-path laundering | critical | mitigate | `DeterministicPlanner` never places `out_*` (CODE-02, `planner.rs:88-102`, `:288`); proof planner is `#[cfg(feature = "live-proof-fixtures")]` (`planner.rs:247-250`) and default-off (`Cargo.toml:8`) | closed |
| T-51-11 | Spoofing | `policy_deny` vacuity as I2 | high | mitigate | Policy explicitly permits `github.pr` (`:433-434`); `assert!(!stdout.contains("DENIED code=policy_deny"))` at `:489` keeps `sink_blocked` distinct | closed |
| T-51-12 | Tampering | stapled taint at sink | high | mitigate | Real `process.exec` exit event selected via durable `anchor.read_event_id` (`:519`) and `provenance_chain[0]` (`:520`); production `mint_from_exec` path, no test-local Untrusted root | closed |
| T-51-13 | Spoofing | hybrid LIVE-08 rebrand | high | mitigate | Real CLI spawn under `CAPRUN_CODING_I2_PROOF` product path (`:228`); unit expressibility explicitly not the DONE claim | closed |
| T-51-14 | Tampering | confirm I2 Block then claim no effect | medium | mitigate | `github_pr_succeeded` count asserted `== 0` (`:501`) | closed |
| T-51-15 | Info disclosure | secret env forwarded to worker | high | mitigate | `env_clear()` + `PATH`-only allowlist (`:215`); only non-secret `CAPRUN_CODING_I2_PROOF` crosses | closed |
| T-51-16 | Elevation | new mint site / raw effect path | high | mitigate | `check-invariants.sh` Gates 1 and 3 green; bag `out_1` only | closed |
| T-51-SC (02) | Tampering | package installs | low | accept | Zero new packages — `AR-01` | closed |

### Plan 51-03 — Fixture containment and exact attribution

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-03-01 | Elevation | ambient proof selector | high | mitigate | Selector compiled only under non-default `live-proof-fixtures` (`worker.rs:382-397`, `main.rs:357-360`, `:563`); normal builds reject `CAPRUN_CODING_I2_PROOF` before any effect; `proof_selector_rejection.rs` covers the rejection | closed |
| T-51-03-02 | Spoofing | LIVE subprocess configuration | high | mitigate | Every `caprun` subprocess is `env_clear()`ed then given an explicit per-case fixture allowlist (`:214-228`) | closed |
| T-51-03-03 | Tampering | LIVE-08 provenance claim | high | mitigate | Hashed `sink_blocked` anchors deserialized; `github.pr`/`body` `read_event_id` and provenance root equated to the `process.exec` exit id (`:504-520`) | closed |
| T-51-03-04 | Repudiation | generic blocked-event counting | medium | mitigate | Exact sink + argument identified, terminal no-effect assertion, and `assert_audit_passed` requires `Chain verification: PASSED` (`:343-356`) | closed |
| T-51-03-SC | Tampering | package installation | low | accept | No dependency change — `AR-01` | closed |

### Plan 51-04 — Authoritative real-Linux compose proof

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-04-01 | Spoofing | real-Linux execution claim | high | mitigate | `51-LIVE-EVIDENCE.md` records host/OS/Docker/commit `cb34b91`/timestamps; both compose commands exited `0` through a `pipefail` tee | closed |
| T-51-04-02 | Repudiation | LIVE result retention | high | mitigate | Complete stdout/stderr retained as `51-LIVE-SCOPED.log` (SHA-256 `dc57e49d…b98d`) and `51-LIVE-FULL.log` (`4bcb275b…ee3e`), both bound into the evidence record and independently recomputed; human checkpoint returned explicit `approved` | closed |
| T-51-04-03 | Tampering | requirement / window status | high | mitigate | Statuses reconciled only after both gates passed; commits `43eb822`, `43fdb67`, `3b4ef4f` preserve the ordering | closed |
| T-51-04-04 | Denial of service | incapable executor host | medium | accept | Plan halted at the precondition checkpoint and resumed on a provisioned EC2 Linux host; completion was never manufactured — `AR-02` | closed |
| T-51-04-SC | Tampering | package installation | low | accept | Existing Docker/Rust workspace only — `AR-01` | closed |

### Plan 51-05 — Blocking-defect reproducers

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-05-01 | Tampering | cross-process `parent_id` chain | high | mitigate | `audit_chain_fork_regression.rs` is a genuine assertion, not a `should_panic` inversion — zero `should_panic` in `cli/caprun/tests/` and `crates/brokerd/tests/` (the single workspace occurrence is an unrelated `confirmation.rs` unit test) | closed |
| T-51-05-02 | Spoofing | "the defect is fixed" claim | high | mitigate | Both regressions were shown to fail on the pre-fix tree before the fix landed; they pass at `442a056` and were re-run green during review | closed |
| T-51-05-03 | Repudiation | LIVE failure diagnostics | high | mitigate | Broker/worker stderr captured and interpolated into the sidecar-failure panic message in both LIVE bodies | closed |
| T-51-05-04 | Tampering | LIVE-08 attribution oracle | high | mitigate | Weak stdout grep deleted (`:492` records the prohibition); the six durable-anchor/provenance assertions are intact at `:504-520`; `cli/caprun/src/main.rs` absent from the diff | closed |
| T-51-05-05 | Info disclosure | temp audit DB in `temp_dir()` | low | accept | Synthetic session ids only, no tokens or literals — `AR-03` | closed |
| T-51-05-06 | Denial of service | contention test flakiness | low | accept | Deterministic pre-fix failure; connections opened before threads — `AR-04` | closed |
| T-51-05-SC | Tampering | package / dependency installs | high | mitigate | Zero new crates and zero new dev-dependencies; `crates/brokerd/Cargo.toml` unchanged; the OS `pkg-config`/`libssl-dev` pair was a human `user_setup` step that enters no build graph | closed |

### Plan 51-06 — Design note for the audit-append fix

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-06-01 | Elevation | design-gate clearance | high | mitigate | Gate shipped PENDING with no pre-filled reviewer identity; flipped only by Plan 51-08's orchestrator-owned trace — `DESIGN-GATE-RECORD-v1.10.md:140`, `:171` | closed |
| T-51-06-02 | Tampering | HARDEN-02 tail-truncation detection | high | mitigate | Loosening `verify_chain` recorded as FORBIDDEN in `DESIGN-audit-append-concurrency.md` §1; the six verifier/MAC functions byte-compare unchanged (Plan 51-07 Gate A) | closed |
| T-51-06-03 | Tampering | I2 / M7 provenance graph | high | mitigate | Non-unification stated as a hard rule naming `read_event_id` and `provenance_chain`; 219 live references remain across `brokerd`/`runtime-core` | closed |
| T-51-06-04 | Spoofing | the D2 premise | high | mitigate | Measured rusqlite 0.32.1 implicit 5000 ms timeout recorded; `AUDIT_BUSY_TIMEOUT_MS = 5000` at `audit.rs:81` | closed |
| T-51-06-05 | Repudiation | latent-instance handling | medium | mitigate | Written disposition present for both latent instances (§8) | closed |
| T-51-06-06 | Info disclosure | design-note contents | low | accept | Repository-internal architecture docs; no credentials — `AR-03` | closed |
| T-51-06-SC | Tampering | package installs | low | accept | Documentation-only plan — `AR-01` | closed |

### Plan 51-07 — TCB change: serialized append at durable head

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-07-01 | Tampering | HARDEN-02 tail-truncation detection | high | mitigate | Gate A byte-compared the six verifier and MAC functions against the pre-fix reference; independent trace obligation confirmed unchanged | closed |
| T-51-07-02 | Tampering | I2 enforcement via the provenance graph | high | mitigate | Gate B found no added/removed `read_event_id` or `provenance_chain` line under `crates/brokerd/src/`; `extract_provenance_threading` re-run green | closed |
| T-51-07-03 | Tampering | audit chain integrity across processes | high | mitigate | Head read and row inserted inside one `TransactionBehavior::Immediate` transaction (`audit.rs:1068`; also `confirmation.rs:1163`, `server.rs:1184`, `:1557`); proven by the two committed fork/contention regressions | closed |
| T-51-07-04 | Denial of service | broker killed by unhandled `SQLITE_BUSY` | high | mitigate | `conn.busy_timeout(AUDIT_BUSY_TIMEOUT_MS)` in `open_audit_db` (`audit.rs:729`) so every connection inherits it; enclosing transactions are immediate | closed |
| T-51-07-05 | Elevation | timeout committed without the atomic append | high | mitigate | Single commit `442a056` contains exactly the three declared broker source files | closed |
| T-51-07-06 | Repudiation | atomicity of the `sink_blocked` group | medium | mitigate | Design §7 pins non-regression; trace obligation 6 PASS — fail-closed under the broker mutex at `server.rs:1020-1054`. Residual comment-accuracy defect recorded as `AR-06` | closed |
| T-51-07-07 | Spoofing | real-Linux proof from a scoped host run | high | mitigate | Plan 51-07 forbade compose/docker/workspace runs and its summary states the full regression was unrun and owned by Plan 51-04 | closed |
| T-51-07-SC | Tampering | dependency / package surface | high | mitigate | Gate C asserted no `Cargo.toml`/`Cargo.lock` change and no new `mint_from_` or forbidden effect-request token; `check-invariants.sh` Gates 1, 3, 4, 5 green | closed |

### Plan 51-08 — Independent adversarial trace and gate clearance

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-08-01 | Spoofing | reviewer independence | high | mitigate | `51-ADVERSARIAL-TRACE.md:3-11` records a fresh-context read-only reviewer `/root/review_51_07_fix`, explicit author-≠-reviewer, and orchestrator re-verification | closed |
| T-51-08-02 | Tampering | I2 value-injection enforcement | high | mitigate | Direct named statement covering `server.rs:941`, `:957`, `:973` recorded in the gate record (`DESIGN-GATE-RECORD-v1.10.md:188`) | closed |
| T-51-08-03 | Elevation | gate clearance with open BLOCKERs | high | mitigate | Trace verdict: BLOCKER 0, unresolved MAJOR 0 (`51-ADVERSARIAL-TRACE.md:52-58`); precondition satisfied before clearance | closed |
| T-51-08-04 | Tampering | requirement / roadmap / ledger status | high | mitigate | Plan 51-08 left `51-LIVE-EVIDENCE.md` absent, LIVE-07/08 `Pending`, `open_count: 3`, and touched none of the four orchestrator-owned files | closed |
| T-51-08-05 | Spoofing | "v1.10 is DONE" from a Docker-less host | high | mitigate | Handoff paragraph states Plan 51-04 must re-run UNCHANGED on a Docker-capable host; the trace verdict explicitly disclaims constituting real-Linux proof | closed |
| T-51-08-06 | Repudiation | incomplete call-site inventory | medium | mitigate | Appendix A completeness was an explicit reviewer question; reviewer verified all 45 production sites (2 roots, 40 continuations, 3 in-transaction) and raised stale-comment `NIT-01` — recorded as `AR-05` | closed |
| T-51-08-07 | Info disclosure | trace artifacts | low | accept | Repository-internal review records — `AR-03` | closed |
| T-51-08-SC | Tampering | package installs | low | accept | No package-manager invocation — `AR-01` | closed |

### Plan 51-09 — Order-independent attribution oracle

| Threat ID | Category | Component | Severity | Disposition | Mitigation (verified) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-51-09-01 | Spoofing | LIVE-08 provenance attribution | high | mitigate | Process event selected exclusively through the unique `github.pr`/`body` anchor's `read_event_id` (`:54`), with actor and provenance-root checks (`:62-63`) | closed |
| T-51-09-02 | Tampering | duplicate-dispatch detection | high | mitigate | Exactly two exit events with one-per-actor cardinality asserted independently of attribution selection | closed |
| T-51-09-03 | Repudiation | order-independence claim | medium | mitigate | `live_08_attribution_is_independent_of_exit_event_order` runs the same oracle with reversed event order — 1 passed | closed |
| T-51-09-04 | Spoofing | LIVE completion from failed/host-only evidence | high | mitigate | Retained failure log labelled RED-only with its own SHA-256 (`9b78bd4f…febbc`, exit 101, commit `3e6e389`) and excluded from the green claim | closed |
| T-51-09-05 | Elevation | product / TCB scope expansion | high | mitigate | File scope limited to one integration-test source; diff checks rejected planner/worker/broker/executor/runtime changes | closed |
| T-51-09-SC | Tampering | package installation | low | accept | No dependency change — `AR-01` | closed |

*Status: open · closed · open — below `high` threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above `workflow.security_block_on` count toward `threats_open`*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-01 | `T-51-SC` (all nine plans) | Supply-chain disposition is `accept` because Phase 51 introduced **zero** new crates, dev-dependencies, or package-manager invocations. `check-invariants.sh` Gate C / Gates 1–6 assert the build graph is unchanged. The only host install was an OS `pkg-config` / `libssl-dev` pair performed by the human outside the build graph. | Plan authors (plan-time disposition) | 2026-08-09 |
| AR-02 | `T-51-04-04` | An executor host without Docker halts at the precondition checkpoint rather than manufacturing completion. Realised as designed: the plan halted, a Docker-capable EC2 Linux host was provisioned, and the plan re-ran unchanged. | Plan 51-04 disposition; human checkpoint `approved` | 2026-08-09 |
| AR-03 | `T-51-05-05`, `T-51-06-06`, `T-51-08-07` | Test/design/review artifacts contain only synthetic session ids and repository-internal architecture prose — no credentials, tokens, literals, or customer data. Temp audit DBs are removed on the happy path and deliberately retained on failure for inspection. | Plan authors (plan-time disposition) | 2026-08-09 |
| AR-04 | `T-51-05-06` | Contention-test flakiness accepted as low: the pre-fix failure is deterministic (identical stale parent), worker connections open before threads spawn, and counts are small named constants. | Plan 51-05 disposition | 2026-08-09 |
| AR-05 | `T-51-08-06` / NIT-01 | `audit.rs:1033-1037` still states 19 production `append_event` sites where the verified inventory is 45. Documentation drift only; Appendix A of `51-ADVERSARIAL-TRACE.md` is the authoritative inventory. Non-blocking. | Independent reviewer `/root/review_51_07_fix`; orchestrator | 2026-08-05 |
| AR-06 | `T-51-07-06` / MINOR-01 | `server.rs:1044-1049` comments overstate multi-write database atomicity: `append_event` commits before the later literal/checkpoint statements. Ordering is mutex-protected and every failure path remains fail-closed, so no effect is authorized from an uncommitted head. Pre-existing limitation, comment-accuracy defect only. | Independent reviewer `/root/review_51_07_fix`; orchestrator | 2026-08-05 |
| AR-07 | **CR-01** — not in the plan-time register | `record_github_grant` (`crates/brokerd/src/audit.rs:542-564`) inserts the `session_grants` row in autocommit, then appends `github_grant_authorized` in a separate transaction. A recoverable append failure leaves an active grant whose retry (`INSERT OR IGNORE`, 0 rows) skips the event, so `has_github_grant` can authorize `github.pr` with no authorization event in the tamper-evident chain. **Accepted for Phase 51 only**, on the recorded ground that Phase 51's contract is proof of an actual CLI-driven real-Linux run, not universal grant atomicity: the one-shot sidecar propagates a non-zero grant result, so the LIVE test would have failed on that path, and the retained scoped run passed with its chain and sink assertions green. CR-01 cannot retroactively falsify that execution. **Carries a mandatory follow-up** (below). | Ben Lamm, via `51-VERIFICATION.md` (`status: passed`, warning CR-01 with explicit disposition and `next_action`) | 2026-08-09 |

### Mandatory follow-up carried out of Phase 51

**CR-01 — high, scheduled, not waived.** Accepting AR-07 closes Phase 51's gate; it does
not close the defect. The required remediation, per `51-REVIEW.md` and
`51-VERIFICATION.md`:

1. Wrap the `session_grants` insert and its conditional `append_event` in one
   `TransactionBehavior::Immediate` transaction, committing only after the append
   succeeds. `append_event` already detects an enclosing transaction and appends at the
   locked durable head without nesting.
2. Add a fault/lock regression proving an append failure rolls back `session_grants`, and
   that a retry yields exactly one grant row and exactly one `github_grant_authorized`
   event. No such fault-injection test exists today — `51-VERIFICATION.md` records this as
   the phase's uncovered error path.

Secondary, non-blocking: correct the `AR-05` and `AR-06` comment defects.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-08-09 | 60 | 60 | 0 | the agent (`/gsd-secure-phase 51`, orchestrator-verified) |

**Method.** Register origin was plan-time (all nine PLAN files carry a `<threat_model>`),
so this run verified that each registered mitigation exists rather than scanning for new
threats. Verification was grep/read-depth (ASVS L1) against the implementation at `HEAD`,
cross-checked against `51-VERIFICATION.md` (4/4 truths, all prohibition families
VERIFIED), `51-REVIEW.md`, `51-ADVERSARIAL-TRACE.md` (BLOCKER 0 / MAJOR 0),
`DESIGN-GATE-RECORD-v1.10.md` (CLEARED), and the two hash-bound real-Linux compose logs.
`threats_open: 0` therefore reflects the plan-time register only; AR-07 records the one
high-severity finding that arose outside it.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-08-09
