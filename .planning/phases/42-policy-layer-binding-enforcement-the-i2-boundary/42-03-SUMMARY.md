---
phase: 42-policy-layer-binding-enforcement-the-i2-boundary
plan: 03
subsystem: security
tags: [policy, i2, executor, session-policy, deny-only-gate, taint, brokerd]

# Dependency graph
requires:
  - phase: 42-01
    provides: SessionPolicy + DenyReason::PolicyDeny in runtime-core; allow_all()/broker_default()/default_fail_closed() constructors; policy.evaluate()/permits_sink()/PolicyDenyKind::constraint_tag()
  - phase: 42-02
    provides: adapter-fs containment helper (unused by this plan; parallel wave)
provides:
  - Deny-only pre-I2 policy gate in the executor TCB (policy_gate.rs) wired into submit_plan_node between validate_schema and the collect-then-Block I2 loop
  - executor::submit_plan_node gains a policy: &SessionPolicy parameter threaded through every workspace caller (brokerd wrapper, live server dispatch path, all test callers incl. ~18 cfg(linux)-gated cli tests)
  - POLICY-01 distinctness + POLICY-02 enforcement-order proof suite (tests/policy_gate.rs)
  - run_broker_server constructs the session policy (broker_default) per session as an Arc<SessionPolicy>; Plan 04 replaces this with the trusted-source-bound policy (POLICY-03)
affects: [42-04-policy-binding, 43-http-write, 46-live-acceptance, LIVE-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Deny-only gate returns Result<(), DenyReason> with NO permit-carrying return — POLICY-02 holds by construction (a permit can only fall through to the unmodified I2 loop, never skip it)"
    - "Immutable session-scoped state (SessionPolicy) threaded as Arc<T> cloned per connection, mirroring the key: Arc<[u8;32]> pattern (no Arc<Mutex<..>> since nothing mutates it)"
    - "test.observe fixture sink gated into SessionPolicy::allow_all() behind #[cfg(any(test, feature = \"test-fixtures\"))], mirroring sink_sensitivity.rs/sink_schema.rs"

key-files:
  created:
    - crates/executor/src/policy_gate.rs
    - crates/executor/tests/policy_gate.rs
  modified:
    - crates/executor/src/lib.rs
    - crates/runtime-core/src/policy.rs
    - crates/runtime-core/Cargo.toml
    - crates/executor/Cargo.toml
    - crates/brokerd/src/lib.rs
    - crates/brokerd/src/server.rs

key-decisions:
  - "Policy gate placed at lib.rs Step 0.25 (line 84) — AFTER validate_schema (69), BEFORE the collect-then-Block loop (94). Deny-only: a PERMIT is Ok(()) and falls through to the UNMODIFIED I2 loop; there is no Allow-and-skip-I2 branch (POLICY-02 by construction)."
  - "run_broker_server uses broker_default() (not the plan's e.g. allow_all()) — justified by the constructors' own doc-strings: broker_default is the production deny-by-default allowlist Plan 04's bind_policy(None,..) returns; allow_all is documented as the POLICY-02-proof / policy-agnostic-test constructor. Functionally identical today (same 7 production sinks)."
  - "test.observe gated into allow_all() under test-fixtures so the two policy-agnostic test.observe executor tests do not PolicyDeny before I2 (landed 42-01 allow_all lacked the fixture sink)."

patterns-established:
  - "POLICY-02-by-construction: the gate function's signature (Result<(), DenyReason>, no Ok-carried decision) structurally forbids a policy-permit from short-circuiting I2."

requirements-completed: [POLICY-01, POLICY-02]

coverage:
  - id: D1
    description: "Deny-only pre-I2 policy gate: a not-allowlisted sink/arg yields Denied{PolicyDeny} (code==policy_deny), distinct from an I2 Block (POLICY-01)."
    requirement: "POLICY-01"
    verification:
      - kind: unit
        ref: "crates/executor/tests/policy_gate.rs#policy_denied_sink_is_a_distinct_policy_deny_not_a_block"
        status: pass
      - kind: unit
        ref: "crates/executor/src/policy_gate.rs#sink_not_in_allowlist_denies_sink_scoped"
        status: pass
    human_judgment: false
  - id: D2
    description: "POLICY-02 enforcement-order: a permissive policy does NOT weaken the hardcoded I2 taint Block — a tainted routing-sensitive arg on a policy-PERMITTED sink+arg still yields BlockedPendingConfirmation, byte-identical across two permissive policies."
    requirement: "POLICY-02"
    verification:
      - kind: unit
        ref: "crates/executor/tests/policy_gate.rs#permissive_policy_does_not_weaken_the_i2_taint_block"
        status: pass
      - kind: unit
        ref: "crates/executor/tests/policy_gate.rs#policy_deny_fires_before_i2_on_a_doubly_offending_node"
        status: pass
    human_judgment: false
  - id: D3
    description: "policy: &SessionPolicy threaded through brokerd::submit_plan_node, the full server.rs dispatch path (evaluate_plan_node_and_record + _for_test wrapper, dispatch_request, handle_connection, classify_second_connection, run_broker_server), and EVERY macOS-forced + cfg(linux)-gated test caller — verified by a green cargo build --workspace --tests IN THE LINUX CONTAINER."
    verification:
      - kind: integration
        ref: "MAILPIT_VERIFY_CMD='bash -c \"cargo build --workspace && cargo build --workspace --tests\"' bash scripts/mailpit-verify.sh (GATE_EXIT=0)"
        status: pass
      - kind: integration
        ref: "cargo test -p brokerd (227 passed, 0 failed)"
        status: pass
    human_judgment: false
  - id: D4
    description: "No raw effect-to-sink path introduced; runtime-core purity intact; policy narrows the plan-node path, never bypasses it."
    verification:
      - kind: automated
        ref: "bash scripts/check-invariants.sh (all gates PASS: Gate 1 no EffectRequest, Gate 2 runtime-core purity)"
        status: pass
    human_judgment: false

# Metrics
duration: 22min
completed: 2026-07-18
status: complete
---

# Phase 42 Plan 03: Policy Layer Binding — the I2 Boundary Summary

**Deny-only pre-I2 policy gate wired into the executor TCB (policy_gate.rs, Step 0.25) with the breaking `policy: &SessionPolicy` signature threaded through every workspace caller; POLICY-02 proven by construction and by an enforcement-order test — a permissive policy provably cannot weaken an I2 taint Block.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-07-18T15:02Z (approx)
- **Completed:** 2026-07-18T15:24Z
- **Tasks:** 3
- **Files modified:** 21 (2 created, 19 modified)

## Accomplishments
- **The policy↔I2 boundary (milestone #1 adversarial-trace risk) landed correctly.** `crates/executor/src/policy_gate.rs::policy_gate()` returns `Result<(), DenyReason>` — `Ok(())` = PERMIT (falls through to the UNMODIFIED I2 collect-then-Block loop), `Err(PolicyDeny)` = DENY. There is **no permit-carrying return**, so POLICY-02 (I2 unconditional on every policy-permitted call) holds **by construction**.
- **Exact placement pinned.** The gate is a deny-only early return at `crates/executor/src/lib.rs:84-86` (Step 0.25), sitting **AFTER** `sink_schema::validate_schema` (line 69 — so an unknown sink still Denies with `UnknownSink`, never `PolicyDeny`) and **BEFORE** the `let mut blocked: Vec<BlockedArg>` collect-then-Block loop (line 94) and the Step-0.5 `CommitIrreversible` class gate (line 223). The I2 sensitivity map + loop are **untouched**.
- **Full breaking-signature blast radius threaded.** `executor::submit_plan_node` gained `policy: &SessionPolicy` (appended last); the param is threaded through `brokerd::submit_plan_node`, the entire live `server.rs` dispatch path, the `_for_test` wrapper + its s38 caller, and **every** macOS-forced + `#[cfg(target_os="linux")]`-gated test caller. Confirmed green by `cargo build --workspace --tests` **in the Linux container** (GATE_EXIT=0) — the authoritative [[cfg-linux-test-blindness]] done-gate, not just a macOS build.
- **POLICY-01/POLICY-02 proof suite** (`crates/executor/tests/policy_gate.rs`, 4 proofs, all pass): distinct policy_deny outcome; permissive policy does NOT weaken the I2 Block (byte-identical across `allow_all()` and `broker_default()`); policy-deny fires before I2 on a doubly-offending node; permitted clean node is Allowed.

## Task Commits

Each task committed atomically:

1. **Task 1: Add the deny-only pre-I2 policy gate to the executor** — `bd6cb39` (feat)
2. **Task 2: Thread the policy param through brokerd, the live server path, AND every workspace caller** — `bbdad02` (feat)
3. **Task 3: POLICY-01 distinctness + POLICY-02 enforcement-order proofs** — `3c3037c` (test)

_Task 1 is `tdd="true"`: the signature change breaks the whole executor test target's compilation, so RED (a standalone failing test) is not cleanly separable from the wiring; `policy_gate.rs` ships with its own `#[cfg(test)]` unit proofs alongside the implementation, and the marquee POLICY-01/POLICY-02 proofs land in Task 3's dedicated `test(...)` commit (the load-bearing GREEN evidence)._

## Files Created/Modified
- `crates/executor/src/policy_gate.rs` (created) — the deny-only `policy_gate()` fn + 3 unit proofs (sink-deny, permit fall-through, dangling-handle-skipped).
- `crates/executor/tests/policy_gate.rs` (created) — the 4 POLICY-01/POLICY-02 integration proofs.
- `crates/executor/src/lib.rs` — `pub mod policy_gate;`, `SessionPolicy` import, `policy` param on `submit_plan_node`, the Step-0.25 deny-only gate insertion.
- `crates/runtime-core/src/policy.rs` — `test.observe` gated into `allow_all()` under `test-fixtures`.
- `crates/runtime-core/Cargo.toml` — `test-fixtures` feature declared.
- `crates/executor/Cargo.toml` — dev-dep enables `runtime-core/test-fixtures`.
- `crates/brokerd/src/lib.rs` — wrapper `submit_plan_node` gains + forwards `policy`; 2 unit tests updated.
- `crates/brokerd/src/server.rs` — `evaluate_plan_node_and_record` (+ `_for_test`), `dispatch_request`, `handle_connection`, `classify_second_connection` gain/thread `policy`; `run_broker_server` constructs `Arc<SessionPolicy>=broker_default()` per session, cloned per connection; 7 test-module dispatch sites updated.
- Test callers updated to pass `&SessionPolicy::allow_all()`: `crates/executor/tests/executor_decision.rs` (20), `crates/brokerd/tests/{s9_acceptance,s37_http_request,phase5_dispatch,s38_github_pr,proto_claims,harden01_session_integrity,extract_provenance_threading,durable_anchor}.rs`, `cli/caprun/tests/{s9_file_write_block,s9_process_exec_block,live_acceptance_v1_8_composed,live_acceptance_v1_7_composed}.rs`.

## Decisions Made
- **run_broker_server uses `broker_default()`, not the plan's `e.g. allow_all()`.** The constructors' own doc-strings assign the roles: `broker_default()` is the production deny-by-default allowlist Plan 04's `bind_policy(None,..)` returns; `allow_all()` is documented as the POLICY-02-proof / policy-agnostic-test constructor. Both are functionally identical today (same 7 production sinks, no arg constraints), so no live/e2e behavior changes; the choice makes the production default semantically correct.
- **`policy` appended as the LAST param of `executor::submit_plan_node`** (mirroring how `session_status` was appended in a prior phase — "extend, don't reshuffle"); inserted **after `session_status`** in the brokerd `evaluate_plan_node_and_record`/`dispatch_request` signatures so the forwarded args stay grouped.
- **`policy` threaded as `Arc<SessionPolicy>`** (cloned per connection like `key`), never `Arc<Mutex<..>>` — the session policy is immutable for the session's life (DESIGN §5.3).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `allow_all()` did not permit the `test.observe` fixture sink**
- **Found during:** Task 1
- **Issue:** The plan instructs "pass `allow_all()` at every existing site", but the landed 42-01 `allow_all()` lists only the 7 production sinks — NOT the `#[cfg(...)]`-gated `test.observe` fixture. The two executor tests that submit a `test.observe` node (`draft_session_allows_observe`, `non_live_session_allows_observe`) would have flipped from `Allowed` to `Denied{PolicyDeny}`.
- **Fix:** Gated `test.observe` into `SessionPolicy::allow_all()` behind `#[cfg(any(test, feature = "test-fixtures"))]` (byte-for-byte the discipline already used for `test.observe` in `sink_sensitivity.rs`/`sink_schema.rs`); declared a `test-fixtures` feature on runtime-core; enabled `runtime-core/test-fixtures` in executor's dev-dep. Production `allow_all()` still lists ONLY the 7 real sinks; `broker_default()` deliberately does NOT get the gate.
- **Files modified:** crates/runtime-core/src/policy.rs, crates/runtime-core/Cargo.toml, crates/executor/Cargo.toml
- **Verification:** `cargo test -p executor` green incl. both test.observe tests; check-invariants Gate 2 (runtime-core purity) still PASS; Gate 4 (test-fixtures never a brokerd default) unaffected.
- **Committed in:** bd6cb39 (Task 1 commit)

**2. [Rule 3 - Blocking] Plan blast-radius list omitted several `dispatch_request` callers**
- **Found during:** Task 2
- **Issue:** The breaking `dispatch_request` signature (new `policy` param) has macOS-visible callers NOT enumerated in the plan's `files_modified`: `crates/brokerd/tests/{proto_claims,harden01_session_integrity,extract_provenance_threading,durable_anchor}.rs` and the `server.rs` `#[cfg(test)]` module's own 7 dispatch sites. `cargo build --workspace --tests` failed on them.
- **Fix:** Threaded `&SessionPolicy::allow_all()` into each (a `request_fd_via_dispatch` helper in harden01 also forwards `session_status, trusted_inode` — insertions were scoped to actual `dispatch_request(` call spans via paren-matching so the helper's own call-forwarding was not mis-edited).
- **Files modified:** crates/brokerd/tests/{proto_claims,harden01_session_integrity,extract_provenance_threading,durable_anchor}.rs, crates/brokerd/src/server.rs (test module)
- **Verification:** `cargo build --workspace --tests` (macOS) clean; `cargo test -p brokerd` 227 passed; Linux-container build --tests GATE_EXIT=0.
- **Committed in:** bbdad02 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 - blocking compilation).
**Impact on plan:** Both were required for the workspace to compile under the breaking signature change; neither altered the gate's security semantics or added scope. Deviation 1 is confined to test-only feature-gating; production `allow_all()`/`broker_default()` behavior is unchanged.

## Issues Encountered
- The `test.observe`/dispatch-caller gaps above surfaced only when compiling the full test graph (macOS `cargo build --workspace --tests` for the brokerd callers; the Linux container for the cfg(linux) cli tests). Both were mechanical signature-propagation fixes.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- **Plan 04 (POLICY-03) is unblocked.** `run_broker_server` constructs the policy internally today; Plan 04 replaces that with a trusted-source-bound `SessionPolicy` passed in as a parameter (via the extracted `refuse_if_beneath_workspace` containment helper) and hash-records the policy identity into the audit DAG at session creation. No further executor/wiring changes are needed there — the `policy: &SessionPolicy` seam is now complete end-to-end.
- **LIVE-06 (Phase 46) leg 3** will re-demonstrate the POLICY-02 guarantee live (a permissive policy on a sink the I2-Block legs use, proving policy is provably not what's blocking) — the enforcement-order test here is its unit-level precursor.

## Self-Check: PASSED

- FOUND: crates/executor/src/policy_gate.rs
- FOUND: crates/executor/tests/policy_gate.rs
- FOUND: .planning/phases/42-policy-layer-binding-enforcement-the-i2-boundary/42-03-SUMMARY.md
- FOUND commits: bd6cb39 (Task 1), bbdad02 (Task 2), 3c3037c (Task 3)

---
*Phase: 42-policy-layer-binding-enforcement-the-i2-boundary*
*Completed: 2026-07-18*
