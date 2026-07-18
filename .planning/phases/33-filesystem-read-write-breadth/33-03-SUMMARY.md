---
phase: 33-filesystem-read-write-breadth
plan: 03
subsystem: brokerd
tags: [rust, tokio, resource-exhaustion-guard, dos-mitigation, requestfd]

# Dependency graph
requires:
  - phase: 33 (33-01/33-02, same wave)
    provides: the existing RequestFd -> read_within -> mint_from_read single-file read path this plan bounds
provides:
  - "MAX_REQUEST_FD_PER_SESSION const (256) — hardcoded resource-exhaustion guard"
  - "fd_request_count per-connection counter threaded through dispatch_request"
  - "fail-closed deny-this-request-keep-connection-alive semantics on the (N+1)th RequestFd"
affects: [33-04, 33-05, any future plan touching crates/brokerd/src/server.rs's dispatch_request signature]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "per-connection counters threaded as &mut u32 through dispatch_request, mirroring the existing intent_provided/fd_requested &mut bool guards"
    - "over-limit resource guards fail closed with BrokerResponse::Error + return Ok(()) — never break/terminate the connection"

key-files:
  created: []
  modified:
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/harden01_session_integrity.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/extract_provenance_threading.rs
    - crates/brokerd/tests/phase5_dispatch.rs

key-decisions:
  - "MAX_REQUEST_FD_PER_SESSION = 256, a hardcoded const (RESEARCH-recommended bound), not a runtime knob"
  - "Counter increments at the TOP of the RequestFd arm (immediately after *fd_requested = true;), before any read/fstat/pass_fd work, so a worker cannot dodge it via read failures"
  - "Over-limit path returns Ok(()) (connection stays alive), never break — mirrors the ProvideIntent-reject error-then-continue shape"
  - "Test for the limit path presets fd_request_count to the bound rather than driving 256 real reads — the bound check runs before any read work, so this exercises the identical branch a genuine 257th call would hit, while keeping the test fast and macOS-runnable"

patterns-established:
  - "Fail-closed per-session resource counters as a first-class dispatch_request parameter, alongside the existing ordering-guard booleans"

requirements-completed: [FS-01]

coverage:
  - id: D1
    description: "Per-session RequestFd count limiter (MAX_REQUEST_FD_PER_SESSION=256) threaded through dispatch_request, bound-checked at arm entry"
    requirement: "FS-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/server.rs#server::tests::request_fd_count_limit"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/server.rs#server::tests::request_fd_repeated_reads_under_bound_succeed"
        status: pass
    human_judgment: false

# Metrics
duration: 18min
completed: 2026-07-17
status: complete
---

# Phase 33 Plan 03: RequestFd Per-Session Counter Summary

**Per-session `RequestFd` call limiter (`MAX_REQUEST_FD_PER_SESSION = 256`) added to `crates/brokerd/src/server.rs`, bound-checked at the top of the existing single-file RequestFd arm, failing closed with an `Error` while keeping the connection alive past the bound.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-07-17T20:14:00Z (approx, first commit 20:16:01 -0400)
- **Completed:** 2026-07-17T20:17:43Z
- **Tasks:** 2 completed
- **Files modified:** 6

## Accomplishments
- Added module-level `const MAX_REQUEST_FD_PER_SESSION: u32 = 256;` and a per-connection `fd_request_count: u32` counter threaded `&mut` through `dispatch_request`, exactly like the existing `intent_provided`/`fd_requested` guards.
- Incremented and bound-checked at the TOP of the `RequestFd` arm (immediately after `*fd_requested = true;`), before any read/fstat/pass_fd work — a worker cannot dodge the counter by triggering read failures.
- Over-limit path sends `BrokerResponse::Error` and `return Ok(())` — connection stays open (fail-closed resource guard, not a connection kill), mirroring the existing ProvideIntent-reject error-then-continue shape.
- Added two inline unit tests: `request_fd_count_limit` (denies the (MAX+1)th call, connection alive) and `request_fd_repeated_reads_under_bound_succeed` (repeated calls strictly under the bound all succeed normally).
- FS-01's multi-file read remains the existing single-file `RequestFd -> read_within -> mint_from_read` path invoked N times — no new read mechanism was introduced (confirmed via `git diff`: exactly one RequestFd arm).

## Task Commits

1. **Task 1: Thread a per-session RequestFd counter through dispatch_request and bound-check at the arm entry** - `fbf799e` (feat)
2. **Task 2: Unit tests for the RequestFd per-session limiter** - `e55a6a9` (test)

**Plan metadata:** (this commit, docs: complete plan)

## Files Created/Modified
- `crates/brokerd/src/server.rs` - `MAX_REQUEST_FD_PER_SESSION` const, `fd_request_count` state, bound-check at RequestFd arm entry, 2 new inline unit tests
- `crates/brokerd/tests/harden01_session_integrity.rs` - updated 2 `dispatch_request` call sites for the new param
- `crates/brokerd/tests/proto_claims.rs` - updated `DispatchHarness` struct + 1 standalone call site for the new param
- `crates/brokerd/tests/durable_anchor.rs` - updated 1 `dispatch_request` call site for the new param
- `crates/brokerd/tests/extract_provenance_threading.rs` - updated 1 `dispatch_request` call site for the new param
- `crates/brokerd/tests/phase5_dispatch.rs` - updated 2 `dispatch_request` call sites for the new param

## Decisions Made
- `MAX_REQUEST_FD_PER_SESSION` is a hardcoded const (256), not a config-loaded value — matches this codebase's discipline that security parameters live in code, not swappable config.
- The `request_fd_count_limit` test presets the counter to the bound rather than driving 256 real RequestFd round-trips, since the increment-then-bound-check runs before any read work — this proves the identical code path a genuine 257th call would hit while keeping the test fast and macOS-runnable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated 5 integration test files outside the plan's declared `files_modified` scope**
- **Found during:** Task 1 (adding the `fd_request_count: &mut u32` parameter to `dispatch_request`)
- **Issue:** `dispatch_request` is `pub` and called directly by 5 crate-level integration test files (`harden01_session_integrity.rs`, `proto_claims.rs`, `durable_anchor.rs`, `extract_provenance_threading.rs`, `phase5_dispatch.rs`), not just the in-crate `#[cfg(test)]` unit tests the plan's `<read_first>` scoped to. Adding the new parameter is a compile-breaking signature change for all 5 files.
- **Fix:** Added a `fd_request_count: u32 = 0` local (or a `fd_request_count` struct field for `proto_claims.rs`'s `DispatchHarness`) and threaded `&mut` through each of the 7 total call sites across the 5 files. No test behavior changed — these harnesses never previously exercised the RequestFd bound, so a fresh `0` local is the correct default.
- **Files modified:** `crates/brokerd/tests/harden01_session_integrity.rs`, `crates/brokerd/tests/proto_claims.rs`, `crates/brokerd/tests/durable_anchor.rs`, `crates/brokerd/tests/extract_provenance_threading.rs`, `crates/brokerd/tests/phase5_dispatch.rs`
- **Verification:** `cargo build --workspace --tests` and `cargo test -p brokerd --no-fail-fast` (110+ tests across all suites) both pass clean.
- **Committed in:** `fbf799e` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking — required for compile)
**Impact on plan:** Necessary to keep the whole workspace compiling; no behavior change to any existing test, no scope creep beyond the mechanical signature-update ripple.

## Issues Encountered
None beyond the deviation above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `crates/brokerd/src/server.rs`'s `dispatch_request` signature now has 3 ordering/counter guard params (`intent_provided`, `fd_requested`, `fd_request_count`) at the tail. Plan 33-04 (later wave) also touches `server.rs` and must account for this signature when it lands.
- No architectural changes; no auth gates encountered.

---
*Phase: 33-filesystem-read-write-breadth*
*Completed: 2026-07-17*

## Self-Check: PASSED
- FOUND: crates/brokerd/src/server.rs
- FOUND: commit fbf799e (Task 1)
- FOUND: commit e55a6a9 (Task 2)
