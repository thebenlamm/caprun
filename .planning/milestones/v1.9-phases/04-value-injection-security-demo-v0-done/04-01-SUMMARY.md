---
phase: 04-value-injection-security-demo-v0-done
plan: 01
subsystem: api
tags: [rust, runtime-core, handle-model, taint, value-injection, serde, uuid]

# Dependency graph
requires:
  - phase: 01-substrate
    provides: runtime-core domain types (PlanNode, ValueNode, Provenance, SinkId, TaintLabel, ExecutorDecision)
provides:
  - "ValueId opaque handle newtype (Hash+Eq, ::new() mints v4 UUID)"
  - "PlanArg { name, value_id } — planner-facing arg carrying NO literal/taint (T-04-02 mitigation)"
  - "PlanNode.args migrated from Vec<ValueNode> to Vec<PlanArg>"
  - "Provenance.provenance_chain: Vec<Uuid> — ordered derivation edges (chain[0] = file_read Event id)"
  - "ValueRecord broker-owned type { id, literal, taint, provenance_chain }"
  - "ExecutorDecision::BlockedPendingConfirmation extended with taint + provenance_chain"
affects: [04-02, 04-03, 04-04, 04-05, executor, brokerd, reader, s9-acceptance-test]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Handle model: planner holds only opaque ValueId; broker owns ValueRecord (literal+taint+chain)"
    - "Block payload self-contained: held-out §9 test asserts unbroken chain from decision alone (no second query)"

key-files:
  created:
    - crates/runtime-core/src/value_record.rs
  modified:
    - crates/runtime-core/src/plan_node.rs
    - crates/runtime-core/src/executor_decision.rs
    - crates/runtime-core/src/lib.rs
    - crates/runtime-core/tests/types_compile.rs
    - crates/runtime-core/tests/task2_types.rs

key-decisions:
  - "ValueId derives Hash+Eq so it can key a HashMap in the broker value store"
  - "ValueNode retained unchanged in shape (legacy serde contract) but removed from the PlanNode arg path"
  - "provenance_chain duplicated on both Provenance and ValueRecord — ValueRecord is the broker-owned authority the §9 test asserts against"
  - "Added Default for ValueId (clippy new_without_default) — non-breaking ergonomics"

patterns-established:
  - "Opaque-handle boundary: PlanArg carries no literal/taint, closing taint-stripping at the planner→broker boundary"
  - "Broker-owned value records: literal authority lives in ValueRecord, never in planner-authored types"

requirements-completed: [REQ-executor-stub]

coverage:
  - id: D1
    description: "ValueId opaque handle + PlanArg (no literal/taint) + ValueRecord with provenance_chain; PlanNode.args is Vec<PlanArg>; Provenance has provenance_chain"
    requirement: "REQ-executor-stub"
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/types_compile.rs#value_id_is_opaque_handle_with_no_literal_or_taint"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/types_compile.rs#value_record_carries_literal_taint_provenance_chain"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/types_compile.rs#provenance_has_provenance_chain_field"
        status: pass
    human_judgment: false
  - id: D2
    description: "ExecutorDecision::BlockedPendingConfirmation carries taint + provenance_chain; whole workspace builds and tests green"
    requirement: "REQ-executor-stub"
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/task2_types.rs#blocked_decision_carries_taint_and_provenance_chain"
        status: pass
      - kind: integration
        ref: "cargo test --workspace"
        status: pass
    human_judgment: false

# Metrics
duration: ~15min
completed: 2026-06-30
status: complete
---

# Phase 4 Plan 01: Handle-Model Type Contract Summary

**runtime-core handle model: opaque ValueId + PlanArg (no literal/taint) + broker-owned ValueRecord, with provenance_chain threaded through Provenance and the extended BlockedPendingConfirmation Block payload — whole workspace green.**

## Performance

- **Duration:** ~15 min active (session spanned a watchdog stall; per-task work was fast)
- **Started:** 2026-06-30T02:31:21Z
- **Completed:** 2026-06-30
- **Tasks:** 2 (both TDD)
- **Files modified:** 5 (1 created, 4 modified)

## Accomplishments
- Established the opaque-handle boundary: `PlanArg` carries only a `ValueId`, never a literal or taint field — taint-stripping (T-04-02) is now structurally impossible at the planner→broker boundary.
- Added broker-owned `ValueRecord { id, literal, taint, provenance_chain }` — the authority the §9 held-out test asserts `provenance_chain[0]` against the file_read Event id.
- Migrated `PlanNode.args` from `Vec<ValueNode>` to `Vec<PlanArg>` and threaded `provenance_chain` through `Provenance` and `ExecutorDecision::BlockedPendingConfirmation`.
- Repaired every breaking caller; `cargo build --workspace` and `cargo test --workspace` are green.

## Task Commits

Each task was committed atomically (TDD RED → GREEN):

1. **Task 1 RED: failing tests for ValueId/PlanArg/ValueRecord** - `babf6f4` (test)
2. **Task 1 GREEN: handle-model types in runtime-core** - `ce3e1f2` (feat)
3. **Task 2 RED: failing test for extended Block payload** - `1c7136d` (test)
4. **Task 2 GREEN: extend BlockedPendingConfirmation** - `4966b7f` (feat)

_No REFACTOR commits — implementations were minimal and clean as written._

## Files Created/Modified
- `crates/runtime-core/src/plan_node.rs` - Added ValueId newtype (+Default), PlanArg; changed PlanNode.args to Vec<PlanArg>; added Provenance.provenance_chain; ValueNode re-documented as legacy.
- `crates/runtime-core/src/value_record.rs` - New module: broker-owned ValueRecord type.
- `crates/runtime-core/src/executor_decision.rs` - BlockedPendingConfirmation extended with taint + provenance_chain.
- `crates/runtime-core/src/lib.rs` - Added `pub mod value_record`; re-exported ValueId, PlanArg, ValueRecord.
- `crates/runtime-core/tests/types_compile.rs` - New tests for handle model; migrated PlanNode/Provenance literals.
- `crates/runtime-core/tests/task2_types.rs` - Migrated to PlanArg; added Block-payload chain assertion.

## Decisions Made
- ValueId derives Hash+Eq to key a broker HashMap; added `Default` impl to satisfy clippy `new_without_default` (non-breaking).
- ValueNode kept structurally unchanged (preserves its existing serde tests) but removed from the sink-argument path.
- `provenance_chain` lives on both Provenance and ValueRecord; ValueRecord is the broker-owned authority the §9 test reads.

## Deviations from Plan

None - plan executed exactly as written. (The only addition beyond the literal action text was a `Default for ValueId` impl to satisfy the clippy lint that the workspace build runs; this is a non-breaking ergonomics addition consistent with `ValueId::new()`.)

## Issues Encountered
None. brokerd's in-crate test (`args: vec![]`) type-checked unchanged under `Vec<PlanArg>` as the plan anticipated; no other workspace callers of BlockedPendingConfirmation existed.

## User Setup Required
None - no external service configuration required. No new dependencies added (T-04-SC: uuid/serde/serde_json only).

## Next Phase Readiness
- The handle-model type contract is locked: downstream plans (04-02 executor, brokerd mint path, reader, 04-05 §9 test) can import `ValueId`, `PlanArg`, `ValueRecord`, and the extended Block payload without further type changes.
- No blockers.

## Self-Check: PASSED

All created/modified files exist on disk; all four task commits (babf6f4, ce3e1f2, 1c7136d, 4966b7f) are present in git history.

---
*Phase: 04-value-injection-security-demo-v0-done*
*Completed: 2026-06-30*
