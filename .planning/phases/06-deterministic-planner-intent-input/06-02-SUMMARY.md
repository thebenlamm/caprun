---
phase: 06-deterministic-planner-intent-input
plan: 02
subsystem: executor
tags: [rust, executor, taint, i2, hard02, security, predicate]

requires:
  - phase: 06-01
    provides: TaintLabel::is_untrusted() exhaustive match on runtime-core TaintLabel

provides:
  - Executor blocking predicate refined to record.taint.iter().any(|t| t.is_untrusted())
  - HARD-02 allow test: UserTrusted-only taint in routing-sensitive arg → Allowed
  - HARD-02 regression guard: ExternalUntrusted/EmailRaw still → BlockedPendingConfirmation

affects:
  - 06-03 (brokerd quarantine — mint_from_intent returns UserTrusted record that now passes)
  - 06-04 (IPC/worker clean-allow-path demo depends on executor allowing UserTrusted handle)
  - 06-05 (e2e live clean-path test relies on this predicate for Allowed exit)

tech-stack:
  added: []
  patterns:
    - "Executor I2 predicate is over TaintLabel::is_untrusted() — never a raw is_empty() check"
    - "Allow test uses [UserTrusted] not [] so it fails under the old predicate (Pitfall 2 guard)"

key-files:
  created: []
  modified:
    - crates/executor/src/lib.rs
    - crates/executor/tests/executor_decision.rs

key-decisions:
  - "Predicate change is one surgical line: !record.taint.is_empty() → record.taint.iter().any(|t| t.is_untrusted())"
  - "Allow test mints [UserTrusted] not [] to make HARD-02 fix load-bearing (Pitfall 2)"
  - "Adjacent doc-comment updated to say 'any explicitly-untrusted label' not 'non-empty'"

patterns-established:
  - "TDD order: write failing test (RED) first, then predicate fix (GREEN) — commit each separately"

requirements-completed: [HARD-02]

coverage:
  - id: D1
    description: "Executor predicate changed to record.taint.iter().any(|t| t.is_untrusted())"
    requirement: HARD-02
    verification:
      - kind: unit
        ref: "crates/executor/tests/executor_decision.rs#hard02_usertrusted_only_allows"
        status: pass
      - kind: unit
        ref: "crates/executor/tests/executor_decision.rs#hard02_externaltainted_still_blocks"
        status: pass
    human_judgment: false
  - id: D2
    description: "Existing executor tests still pass after predicate change (no regression)"
    requirement: HARD-02
    verification:
      - kind: unit
        ref: "crates/executor/tests/executor_decision.rs (6 pre-existing tests)"
        status: pass
    human_judgment: false

duration: 2min
completed: 2026-07-01
status: complete
---

# Phase 06 Plan 02: HARD-02 Executor Predicate Fix Summary

**Executor blocking predicate refined from `!is_empty()` to `any(is_untrusted())`, making UserTrusted-only provenance reachable at the allow-path (HARD-02)**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-07-01T00:31:09Z
- **Completed:** 2026-07-01T00:32:41Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added HARD-02 TDD tests (RED first): `hard02_usertrusted_only_allows` with `[UserTrusted]` taint (not `[]`) so it fails under old predicate, plus `hard02_externaltainted_still_blocks` regression guard
- Changed executor predicate from `!record.taint.is_empty()` to `record.taint.iter().any(|t| t.is_untrusted())` — one surgical line in `lib.rs`
- Updated adjacent doc-comment to say "explicitly-untrusted label" instead of "non-empty"
- All 8 executor tests pass; `cargo test --workspace --no-fail-fast` fully green; `check-invariants.sh` all PASS

## Task Commits

1. **Task 1: Add HARD-02 tests (allow RED + regression guard)** — `eba40a8` (test)
2. **Task 2: Refine blocking predicate over is_untrusted()** — `8fe5e7a` (feat)

## Files Created/Modified

- `crates/executor/src/lib.rs` — predicate changed + doc-comment updated
- `crates/executor/tests/executor_decision.rs` — two new HARD-02 tests added

## Decisions Made

- Used `[UserTrusted]` (not `[]`) in the allow test so the fix is provably load-bearing: the test fails under the old `!is_empty()` predicate (Pitfall 2 from 06-RESEARCH.md)
- Exhaustive `match self` for `is_untrusted()` was already in place from Plan 01; this plan only consumes it in the executor

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- Plan 03 (mint_from_intent) can now mint `[UserTrusted]` records; the executor predicate will allow them through routing-sensitive arg checks
- Plan 04 (IPC + worker clean-path) depends on this predicate being correct
- Plan 05 (e2e live test) requires the full chain (Plans 01-04) to complete before the Linux clean-path demo

---
*Phase: 06-deterministic-planner-intent-input*
*Completed: 2026-07-01*
