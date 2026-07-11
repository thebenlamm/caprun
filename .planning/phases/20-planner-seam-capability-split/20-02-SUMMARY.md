---
phase: 20-planner-seam-capability-split
plan: 02
subsystem: security
tags: [brokerd, tokio, uds-ipc, capability-model, trust-boundary, accept-loop]

# Dependency graph
requires:
  - phase: 19-session-trust-coherence-fix
    provides: "the one-way, session-lifetime worker-slot occupancy latch in run_broker_server's accept loop, and the three regression tests that anchor it (two_connection_intent_bypass.rs)"
provides:
  - "BrokerRequest::DeclarePlannerRole establishment handshake (proto.rs)"
  - "ConnectionRole::{Worker,Planner} pure, default-deny capability model with ConnectionRole::permits (server.rs)"
  - "pre-dispatch capability gate in handle_connection, checked on every message before dispatch_request"
  - "accept-loop extension: a second, orthogonal one-way planner-slot latch admitting exactly one capability-restricted planner connection per session"
affects: [21-adversarial-planner, planner-seam-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "capability set decided once at connection establishment, threaded immutably, checked per-message (never re-derived) — extends DESIGN-session-trust-coherence.md §3's shape into concrete Rust"
    - "accept-loop classification of a subsequent connection runs in its own spawned task with a bounded first-frame read timeout, so a stalled connection cannot block the accept loop (T-20-08)"

key-files:
  created:
    - crates/brokerd/tests/planner_capability_split.rs
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/Cargo.toml

key-decisions:
  - "Planner capability gate lives in handle_connection's request loop (pre-dispatch), not inside dispatch_request — keeps dispatch_request's signature and every existing call site untouched"
  - "The planner slot is a SEPARATE one-way latch (Arc<AtomicBool>, compare_exchange-claimed) from the worker slot's loop-local bool — required because subsequent-connection classification runs in its own spawned task (T-20-08), unlike the worker slot's synchronous accept-loop check"
  - "DeclarePlannerRole arriving as a dispatch_request arm (mid-stream, on an already-classified connection) is rejected fail-closed rather than silently no-op'd — required by the exhaustive match once the variant exists"
  - "brokerd's own Cargo.toml now declares tokio's time feature explicitly rather than relying on incidental cross-crate feature unification with cli/caprun"

requirements-completed: [PLANNER-02, PLANNER-04]

coverage:
  - id: D1
    description: "A planner-role connection can never invoke ProvideIntent, ReportClaims, ReportDerivedClaim, or CreateSession — all four fail closed with BrokerResponse::Error, minting nothing"
    requirement: PLANNER-02
    verification:
      - kind: unit
        ref: "crates/brokerd/tests/planner_capability_split.rs#planner_role_permits_only_submit"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/planner_capability_split.rs#linux_tests::planner_second_connection_accepted_and_capability_restricted (Linux, via mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D2
    description: "A planner-role connection is also denied RequestFd and ReportRead, so it never receives a raw-bytes fd"
    requirement: PLANNER-04
    verification:
      - kind: unit
        ref: "crates/brokerd/tests/planner_capability_split.rs#planner_role_gate_denies_every_mint_and_fd_verb"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/planner_capability_split.rs#linux_tests::planner_second_connection_accepted_and_capability_restricted (Linux, via mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Phase 19's one-way occupancy latch is extended, not weakened: exactly one additional planner-role connection is admitted per session after DeclarePlannerRole; every other 2nd/3rd connection is rejected exactly as Phase 19 rejects it; the worker path is byte-identical"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/two_connection_intent_bypass.rs (Phase 19's 3 regression tests, UNMODIFIED, git diff empty) — Linux via mailpit-verify.sh"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/planner_capability_split.rs#linux_tests::planner_second_connection_accepted_and_capability_restricted — asserts conn#3 (2nd planner declare) and conn#4 (undeclared 2nd connection) are both rejected"
        status: pass
    human_judgment: false

# Metrics
duration: 45min
completed: 2026-07-11
status: complete
---

# Phase 20 Plan 02: Planner Capability Split Summary

**Per-connection capability model (`ConnectionRole::{Worker,Planner}`) plus an accept-loop extension admitting exactly one capability-restricted planner connection per session, fail-closed default-deny on every mint verb and every raw-bytes verb.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-11T11:51:09Z
- **Tasks:** 3
- **Files modified:** 3 (proto.rs, server.rs, Cargo.toml) + 1 created (planner_capability_split.rs)

## Accomplishments
- `BrokerRequest::DeclarePlannerRole` — additive establishment handshake a connection sends as its FIRST framed message to request planner-role capabilities.
- `ConnectionRole::{Worker, Planner}` + pure `permits(&BrokerRequest) -> bool` — `Worker` permits everything (no behavior change); `Planner` permits ONLY `SubmitPlanNode`, with every other verb (including a mid-stream `DeclarePlannerRole` re-handshake) denied by an explicit named arm.
- Pre-dispatch capability gate in `handle_connection`'s request loop: a non-permitted verb is rejected `Error` BEFORE `dispatch_request` is ever called — mints nothing, advances no chain head.
- Accept-loop extension in `run_broker_server`: the first connection still takes the worker slot exactly as Phase 19 did (byte-identical, no pre-read); every subsequent connection is classified in its own spawned task (`classify_second_connection`) with a bounded 5s first-frame-read timeout — `DeclarePlannerRole` + a free planner slot (claimed via atomic `compare_exchange`) is Ack'd and serviced as `ConnectionRole::Planner`; everything else gets the same Phase-19 "already established" rejection.
- New `crates/brokerd/tests/planner_capability_split.rs`: 2 macOS-runnable pure-gate unit tests plus a Linux-gated end-to-end integration test proving conn#2's planner declaration is accepted and every mint/fd verb it sends is rejected, while a 3rd connection and an undeclared 2nd connection are both rejected.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add the `DeclarePlannerRole` handshake and the `ConnectionRole` capability model** - `12ee701` (feat)
2. **Task 2: Gate dispatch by role in `handle_connection` and prove mint verbs fail closed** - `2677c0e` (feat)
3. **Task 3: Extend the accept-loop latch to admit exactly one planner-role connection** - `de372f0` (feat)

**Plan metadata:** (this commit, docs)

## Files Created/Modified
- `crates/brokerd/src/proto.rs` - Added `BrokerRequest::DeclarePlannerRole` (additive unit variant + doc)
- `crates/brokerd/src/server.rs` - `ConnectionRole` enum + `permits`; `DeclarePlannerRole` dispatch arm (fail-closed); `handle_connection` gained a `role` param + pre-dispatch gate; accept loop split into worker-slot (unchanged) + planner-slot (new, `classify_second_connection` + `read_one_frame` helpers)
- `crates/brokerd/Cargo.toml` - `tokio` now declares `features = ["time"]` explicitly for this crate
- `crates/brokerd/tests/planner_capability_split.rs` - new: pure-gate unit tests (macOS) + Linux-gated accept-loop integration test

## Decisions Made
- The planner capability gate lives in `handle_connection`'s request loop (pre-dispatch), not inside `dispatch_request` — keeps `dispatch_request`'s signature and all four existing call-site tests (`phase5_dispatch`, `proto_claims`, `durable_anchor`, `extract_provenance_threading`) untouched.
- The planner slot is a separate `Arc<AtomicBool>` one-way latch (atomically claimed via `compare_exchange`), distinct from the worker slot's loop-local `bool` — required because subsequent-connection classification now runs in its own spawned task (T-20-08 DoS mitigation: a stalled connection cannot block the accept loop), so two such tasks could otherwise race on a shared decision.
- `DeclarePlannerRole` reaching `dispatch_request` (i.e., arriving mid-stream on an already-classified connection, since the accept-loop classification consumes it before `handle_connection`'s loop starts for a newly-admitted planner connection) is rejected fail-closed rather than silently no-op'd.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `DeclarePlannerRole` arm to `dispatch_request`'s exhaustive match**
- **Found during:** Task 1 (build failure immediately after adding the variant)
- **Issue:** `dispatch_request`'s `match request { ... }` is exhaustive (no wildcard arm, by design — BrokerRequest's own fail-closed discipline). Adding the additive `DeclarePlannerRole` variant to `BrokerRequest` therefore fails the build with `E0004: non-exhaustive patterns` until every match site handles it.
- **Fix:** Added an explicit arm that rejects `DeclarePlannerRole` fail-closed (`BrokerResponse::Error`, mints nothing) when it reaches this dispatch loop — documented as "only meaningful as a connection's first framed message, consumed by accept-loop classification; if it arrives here, role is already fixed."
- **Files modified:** crates/brokerd/src/server.rs
- **Verification:** `cargo build --workspace` succeeds; `dispatch_request`'s parameter signature is unchanged (only a match arm was added).
- **Committed in:** `12ee701` (Task 1 commit)

**2. [Rule 3 - Blocking] Declared brokerd's own `tokio` "time" feature**
- **Found during:** Task 3 (`cargo test -p brokerd --test two_connection_intent_bypass --no-run` failed: `error[E0433]: could not find 'time' in 'tokio'`)
- **Issue:** `classify_second_connection`'s `tokio::time::timeout` (needed for T-20-08's bounded first-frame read) compiled successfully under `cargo build --workspace` only because `cli/caprun`'s own `tokio = { features = ["time"] }` incidentally unified features across the whole workspace build. A scoped `cargo test -p brokerd` — the exact command this plan's own acceptance criteria and verification steps specify — does not get that unification and failed to compile.
- **Fix:** Added `features = ["time"]` to `brokerd`'s own `[dependencies]` `tokio` line in `crates/brokerd/Cargo.toml`, with a comment explaining why relying on cross-crate unification was wrong.
- **Files modified:** crates/brokerd/Cargo.toml
- **Verification:** `cargo test -p brokerd --test two_connection_intent_bypass --no-run` succeeds; `cargo test -p brokerd --no-fail-fast` passes; Linux verification via `mailpit-verify.sh` passes.
- **Committed in:** `de372f0` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 - blocking issues, both required by the compiler)
**Impact on plan:** Both fixes are mechanical compilation requirements introduced by the plan's own additive changes (a new exhaustive-match variant; a new `tokio::time` call site). No scope creep — no new functionality beyond what the plan specified.

## Issues Encountered
None beyond the two blocking-issue auto-fixes documented above.

## Threat Model Verification

All 5 threats from the plan's STRIDE register were verified mitigated:
- **T-20-01** (mint verbs on a planner connection): `ConnectionRole::permits` denies all four; proven by `planner_role_permits_only_submit` (unit) and the Linux integration test.
- **T-20-02** (role re-declaration mid-stream): role fixed once at establishment; `DeclarePlannerRole` itself is a denied verb for `Planner`, and is rejected fail-closed if it somehow reaches `dispatch_request` on any connection.
- **T-20-03** (bypass of Phase 19's one-way latch via a 2nd connection): the worker slot latch is untouched; the planner slot is a SEPARATE one-way latch; every other 2nd/3rd connection is rejected exactly as Phase 19 — proven by the Linux integration test's conn#3/conn#4 assertions AND by Phase 19's 3 regression tests passing unmodified.
- **T-20-05** (planner obtains a raw-bytes fd): `RequestFd` denied for `Planner`; proven by both pure-gate tests and the Linux integration test.
- **T-20-08** (a stalled 2nd connection blocking the accept loop): subsequent-connection classification runs in its own spawned task with a bounded 5s timeout, never inline in the accept loop.

## Next Phase Readiness
- The capability-restricted planner connection seam (`DeclarePlannerRole` → `ConnectionRole::Planner` → `SubmitPlanNode`-only) is ready for Phase 21 to connect a real adversarial LLM planner to.
- Phase 19's regression suite remains green and unmodified — no regression risk carried forward.
- `crates/brokerd/src/quarantine.rs` (the three mint functions) was untouched throughout this plan — only WHO may reach the dispatch arms that call them changed, per the plan's own constraint.

---
*Phase: 20-planner-seam-capability-split*
*Completed: 2026-07-11*

## Self-Check: PASSED

- FOUND: crates/brokerd/tests/planner_capability_split.rs
- FOUND: crates/brokerd/src/proto.rs (modified)
- FOUND: crates/brokerd/src/server.rs (modified)
- FOUND: crates/brokerd/Cargo.toml (modified)
- FOUND commit 12ee701 (Task 1)
- FOUND commit 2677c0e (Task 2)
- FOUND commit de372f0 (Task 3)
