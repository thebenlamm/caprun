---
phase: 25-regression-live-proof
plan: 01
subsystem: testing
tags: [rust, cargo-test, executor, audit-dag, slot-type-binding, t2-06]

# Dependency graph
requires:
  - phase: 24-slot-type-binding-enforcement
    provides: "Step 1c role/slot mismatch enforcement (DenyReason::SlotTypeMismatch) and unit tests calling store.mint() directly"
provides:
  - "Held-out full-broker-path acceptance test proving T2-06 through a genuine mint_from_intent -> submit_plan_node -> Denied{SlotTypeMismatch} chain with a durable plan_node_evaluated audit-DAG event and verify_chain true"
  - "Correctly-routed Allowed control mechanically proving the swapped-deny is Step-1c-attributable (not I0/I2)"
affects: ["25-02-regression-live-proof", "25-03-regression-live-proof"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Genuine-chain discipline for acceptance tests: mint via production mint_from_intent (never store.mint()), submit via production submit_plan_node (never hand-construct DenyReason), hand-mirror the private evaluate_plan_node_and_record's plan_node_evaluated append exactly, then assert verify_chain true."

key-files:
  created: []
  modified:
    - crates/brokerd/tests/s9_acceptance.rs

key-decisions:
  - "s9_acceptance.rs is intentionally NOT cfg-gated to Linux for these two tests (deliberate deviation from DESIGN §9's informative note, documented in the plan objective): the test shape is in-process and identical on Mac/Linux, and DESIGN §9 is explicitly labeled informative, not gate. Real-Linux confirmation happens via Plan 03's mailpit-verify.sh run of this same file."
  - "Added DenyReason to the file-level runtime_core import (was previously only ExecutorDecision, SessionStatus) since both new tests match on DenyReason::SlotTypeMismatch."

requirements-completed: [T2-06]

coverage:
  - id: D1
    description: "Swapped subject<->recipient plan node denies with SlotTypeMismatch through the real broker path (genuine mint_from_intent chain), records a plan_node_evaluated audit-DAG event parented on the second mint's event id, and verify_chain returns true"
    requirement: "T2-06"
    verification:
      - kind: unit
        ref: "crates/brokerd/tests/s9_acceptance.rs#slot_type_binding_swapped_subject_recipient_denies"
        status: pass
    human_judgment: false
  - id: D2
    description: "Correctly-routed control using the same two UserTrusted values evaluates to Allowed, mechanically proving the D1 deny is attributable to Step 1c alone (not I0 class-deny, not I2 taint-block)"
    requirement: "T2-06"
    verification:
      - kind: unit
        ref: "crates/brokerd/tests/s9_acceptance.rs#slot_type_binding_correctly_routed_allows"
        status: pass
    human_judgment: false

duration: 20min
completed: 2026-07-12
status: complete
---

# Phase 25 Plan 01: Held-out T2-06 genuine-chain proof Summary

**Added two `#[test] fn`s to `s9_acceptance.rs` that drive the real broker path (`mint_from_intent` -> `submit_plan_node`) to prove Phase 24's Step 1c slot-type binding catches a swapped subject/recipient handle pair, with a durable `plan_node_evaluated` audit-DAG event and `verify_chain` true, plus a correctly-routed Allowed control proving the deny is Step-1c-attributable.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-07-12T04:14:00Z (approx)
- **Completed:** 2026-07-12T04:34:43Z
- **Tasks:** 2 completed
- **Files modified:** 1

## Accomplishments
- `slot_type_binding_swapped_subject_recipient_denies`: mints a subject-role and a recipient-role `UserTrusted` value via `mint_from_intent` with a genuine causal chain, swaps their handles into each other's `PlanArg` slot at `email.send`, asserts `ExecutorDecision::Denied{SlotTypeMismatch{sink, arg, expected, found}}` with the exact expected field values, hand-mirrors the broker's `plan_node_evaluated` audit-DAG append, and asserts `verify_chain` stays true.
- `slot_type_binding_correctly_routed_allows`: reuses the identical two minted values, routes them into their correct slots, and asserts `Allowed` — the mechanical proof that neither I0 (session Active) nor I2 (no taint present) could have produced the swapped-routing deny, isolating the cause to Step 1c.
- Full `s9_acceptance.rs` suite (5 tests) and `check-invariants.sh` (Gates 1-3) both pass with no regressions to existing tests.

## Task Commits

Each task was committed atomically:

1. **Task 1: Held-out swapped subject↔recipient deny test** - `c97bd4b` (test)
2. **Task 2: Correctly-routed Allowed control** - `3b6c7b9` (test)

**Plan metadata:** (this SUMMARY.md commit, in worktree mode)

_Note: both tasks are pure test additions — no RED/GREEN/REFACTOR TDD cycle applies (plan `tdd` not set); each commit type is `test`._

## Files Created/Modified
- `crates/brokerd/tests/s9_acceptance.rs` - Added `DenyReason` to the runtime_core import, and two new `#[test] fn`s (swapped-deny proof + correctly-routed Allowed control) per the plan's exact steps.

## Decisions Made
- Followed the plan's exact literal values, causal-chaining approach, and hand-mirror-of-`evaluate_plan_node_and_record` discipline (that fn is private/async in `brokerd::server`, unreachable from `crates/brokerd/tests/`). No deviation from the plan's example code shape (which the plan itself sourced from `25-RESEARCH.md` Pattern 1).
- Kept the `DenyReason::SlotTypeMismatch` field types (`sink: String`, not `SinkId`) as defined in `runtime_core::executor_decision` — confirmed via source read before writing assertions.

## Deviations from Plan
None - plan executed exactly as written. Both tasks' acceptance criteria and verify commands passed on the first attempt with no auto-fixes required.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- T2-06 tripwire (genuine-chain proof) is now closed on Mac via `cargo test -p brokerd --test s9_acceptance`.
- Real-Linux confirmation (T2-08, DESIGN §9's informative note) is deferred to Plan 03, which runs `bash scripts/mailpit-verify.sh` and asserts these two test names appear in the Linux log with 0 failures — this plan makes no claim about Linux execution.
- No production code changed; no blockers for Plan 02/03.

---
*Phase: 25-regression-live-proof*
*Completed: 2026-07-12*

## Self-Check: PASSED
- FOUND: crates/brokerd/tests/s9_acceptance.rs
- FOUND: .planning/phases/25-regression-live-proof/25-01-SUMMARY.md
- FOUND: commit c97bd4b (Task 1)
- FOUND: commit 3b6c7b9 (Task 2)
