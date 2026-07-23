---
phase: 20-planner-seam-capability-split
plan: 01
subsystem: planner
tags: [rust, trait-abstraction, planner-seam, caprun]

# Dependency graph
requires:
  - phase: 19-cross-connection-trust-coherence-fix
    provides: fixed broker trust boundary (one-way occupancy latch) that Phase 20/21's planner connection will sit behind
provides:
  - "A real `Planner` trait in cli/caprun/src/planner.rs — the compile-time seam Phase 21's adversarial LlmPlanner will implement"
  - "`DeterministicPlanner` implementing `Planner`, delegating unchanged to `plan_from_intent`"
  - "worker.rs routes its PlanNode construction through the trait method, not the bare free fn"
affects: [21-adversarial-llm-planner, 20-02-per-verb-capability-split, 20-03-planner-connection-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Swappable seam via a trait with a single method mirroring an existing free fn's exact signature, with the free fn retained unchanged so existing direct-call test suites (via #[path] inclusion) pass unmodified"

key-files:
  created: []
  modified:
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs

key-decisions:
  - "Trait method signature is byte-identical to plan_from_intent's parameter list (typed &CaprunIntent + five ValueId/Option<ValueId> handles) — no ValueRecord, raw bytes, or taint label introduced, per PLAN-03/PLANNER-04's compile-time boundary"
  - "plan_from_intent kept as a pub fn with its exact original signature so cli/caprun/tests/planner.rs (which includes src/planner.rs via #[path] and calls the free fn directly) required zero test-body edits"

patterns-established:
  - "DeterministicPlanner is a unit struct whose impl Planner::plan delegates its full argument list unchanged to the pre-existing free fn — the seam adds zero new logic, only a swappable entry point"

requirements-completed: [PLANNER-01, PLANNER-04]

coverage:
  - id: D1
    description: "A `Planner` trait exists in planner.rs with a single method mapping &CaprunIntent + opaque ValueId handles to a PlanNode"
    requirement: "PLANNER-01"
    verification:
      - kind: unit
        ref: "grep -n 'pub trait Planner' cli/caprun/src/planner.rs"
        status: pass
      - kind: unit
        ref: "cargo build --workspace"
        status: pass
    human_judgment: false
  - id: D2
    description: "DeterministicPlanner implements Planner by delegating to the unchanged plan_from_intent deterministic logic"
    requirement: "PLANNER-01"
    verification:
      - kind: unit
        ref: "grep -n 'impl Planner for DeterministicPlanner' cli/caprun/src/planner.rs"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs (5 tests, unmodified bodies)"
        status: pass
    human_judgment: false
  - id: D3
    description: "The worker constructs a DeterministicPlanner and calls its trait method instead of the bare free fn"
    requirement: "PLANNER-01"
    verification:
      - kind: unit
        ref: "grep -n 'DeterministicPlanner' cli/caprun/src/worker.rs && grep -n '\\.plan(' cli/caprun/src/worker.rs"
        status: pass
      - kind: unit
        ref: "cargo build --workspace"
        status: pass
    human_judgment: false
  - id: D4
    description: "Trait method signature exposes only &CaprunIntent and ValueId handles — never ValueRecord/raw bytes/taint label (PLANNER-04)"
    requirement: "PLANNER-04"
    verification:
      - kind: unit
        ref: "cli/caprun/src/planner.rs:53-62 (trait Planner::plan signature) — manual code inspection"
        status: pass
    human_judgment: false
  - id: D5
    description: "All pre-existing planner unit tests and workspace build pass unchanged through the new seam"
    verification:
      - kind: unit
        ref: "cargo test -p caprun --test planner (5 passed, 0 failed)"
        status: pass
      - kind: unit
        ref: "cargo test -p caprun --no-fail-fast (15 passed across all non-Linux-gated binaries, 0 failed)"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (all 3 gates PASSED)"
        status: pass
    human_judgment: false

duration: 25min
completed: 2026-07-11
status: complete
---

# Phase 20 Plan 01: Planner Seam Summary

**Introduced a real `Planner` trait in `cli/caprun/src/planner.rs` — `DeterministicPlanner` implements it by delegating unchanged to `plan_from_intent`, and `worker.rs` now constructs the planner and calls it through the trait method instead of a bare free-fn call.**

## Performance

- **Duration:** 25 min
- **Started:** 2026-07-11T11:20:00Z
- **Completed:** 2026-07-11T11:45:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added `pub trait Planner` (single `plan()` method, `&self` receiver) whose signature mirrors `plan_from_intent`'s exact parameter list — typed `&CaprunIntent` plus five `ValueId`/`Option<ValueId>` handle parameters, returning `PlanNode`. No `ValueRecord`, raw byte/string, or taint-label parameter was introduced, preserving PLANNER-04's compile-time boundary.
- Added `pub struct DeterministicPlanner` + `impl Planner for DeterministicPlanner`, whose `plan()` body forwards every argument unchanged to the retained `plan_from_intent` free fn.
- Retained `plan_from_intent` with its exact original signature — `cli/caprun/tests/planner.rs` (which includes `src/planner.rs` via `#[path]` and calls the free fn directly in all 5 tests) required zero test-body edits and all 5 tests pass unmodified.
- Extended the `planner.rs` module doc with a new "The `Planner` seam" section referencing PLANNER-01 (seam) and PLANNER-04 (structural handle-only boundary), naming Phase 21's `LlmPlanner` as the future second implementor.
- Routed `worker.rs`'s plan-node construction through the trait: `let planner = crate::planner::DeterministicPlanner; let plan_node = planner.plan(...)` replaces the direct `crate::planner::plan_from_intent(...)` call. Brought `Planner` into scope via `use crate::planner::Planner;`. Updated the worker's ordering-doc comment (step 12) and the adjacent PLAN-03 commentary to describe invocation through the seam, without weakening the existing opaque-handles-only language.

## Task Commits

Each task was committed atomically:

1. **Task 1: Introduce the `Planner` trait and `DeterministicPlanner` impl** - `e4394be` (feat)
2. **Task 2: Route the worker's planner call through the `Planner` trait** - `44a189a` (feat)

**Plan metadata:** (this commit, docs: complete plan)

## Files Created/Modified
- `cli/caprun/src/planner.rs` - Added `pub trait Planner` + `pub struct DeterministicPlanner` + `impl Planner for DeterministicPlanner`; extended module doc; `plan_from_intent` unchanged.
- `cli/caprun/src/worker.rs` - Planner invocation moved from a direct `plan_from_intent(...)` call to `DeterministicPlanner::plan(...)` via the trait; added `use crate::planner::Planner;`; updated adjacent doc comments.

## Decisions Made
- Trait method signature is byte-identical to `plan_from_intent`'s parameter list, so the seam adds zero new logic and cannot accidentally widen the PLANNER-04 boundary.
- `plan_from_intent` retained unchanged (not folded into the trait impl body only) specifically to keep `cli/caprun/tests/planner.rs`'s direct free-fn calls compiling and passing without any test-body edits, per the plan's explicit `<read_first>` guidance.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. `cargo test -p caprun --test planner` and `cargo build --workspace` both surfaced expected `dead_code` warnings for `Planner`/`DeterministicPlanner` after Task 1 alone (since nothing used them yet) — these warnings disappeared after Task 2 wired the worker to the trait, confirming the seam is genuinely exercised, not vestigial.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The `Planner` trait is a genuine, generic abstraction (not a rename): its signature carries only the typed intent + opaque handles, so Phase 21's `LlmPlanner` can implement it with zero changes to `worker.rs`'s call site beyond swapping which concrete type is constructed.
- Phase 20-02 (per-verb capability split) and 20-03 (planner-connection wiring) are unaffected by this plan — this plan touched only `cli/caprun/src/planner.rs` and `cli/caprun/src/worker.rs`, with no overlap with `crates/brokerd` (20-02's territory).
- No blockers. `cargo test -p caprun --no-fail-fast` shows no regression from baseline; `./scripts/check-invariants.sh` passes all 3 gates (no `EffectRequest` token introduced; planner still holds only opaque handles).

---
*Phase: 20-planner-seam-capability-split*
*Completed: 2026-07-11*

## Self-Check: PASSED

All claimed files and commits verified present:
- FOUND: cli/caprun/src/planner.rs
- FOUND: cli/caprun/src/worker.rs
- FOUND: .planning/phases/20-planner-seam-capability-split/20-01-SUMMARY.md
- FOUND: e4394be (Task 1 commit)
- FOUND: 44a189a (Task 2 commit)
