---
phase: 33-filesystem-read-write-breadth
plan: 04
subsystem: broker-sinks
tags: [rust, brokerd, filesystem, audit-dag, sink-dispatch]

# Dependency graph
requires:
  - phase: 33-filesystem-read-write-breadth (Plan 01)
    provides: "WorkspaceRoot::write_within primitive (O_WRONLY|O_TRUNC openat2)"
  - phase: 33-filesystem-read-write-breadth (Plan 02)
    provides: "file.write sink id registered in executor schema/sensitivity tables"
  - phase: 33-filesystem-read-write-breadth (Plan 03)
    provides: "RequestFd limiter in server.rs (unrelated to this plan's arm placement, but confirms current server.rs line numbers)"
provides:
  - "invoke_file_write sink module with two-phase durable audit"
  - "file.write Allowed-dispatch arm in evaluate_plan_node_and_record"
affects: [33-05, milestone-close]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-phase durable audit (sink_executed / sink_execution_failed) mirrored verbatim across file.create and file.write sink modules"
    - "Per-module private resolve_arg helper (no shared cross-module extraction)"

key-files:
  created:
    - crates/brokerd/src/sinks/file_write.rs
  modified:
    - crates/brokerd/src/sinks.rs
    - crates/brokerd/src/server.rs

key-decisions:
  - "invoke_file_write is a verbatim structural mirror of invoke_file_create's allow-path variant only — the from_resolved (confirm-time re-invocation) sibling and resolved_literal helper were NOT copied, since file.write args are always resolved from the live ValueStore on this Allowed-only path and the plan's artifact list scoped this to invoke_file_write + resolve_arg"
  - "Inline tests were written alongside the module in Task 1 rather than deferred to Task 2 — both tasks' verification commands (cargo build -p brokerd, cargo test -p brokerd file_write) pass regardless of which task's commit they land in"

patterns-established: []

requirements-completed: [FS-02]

coverage:
  - id: D1
    description: "invoke_file_write sink module: resolves path+contents via ValueStore, calls WorkspaceRoot::write_within, two-phase durable audit (sink_executed / sink_execution_failed) chained onto parent_id/parent_hash"
    requirement: "FS-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/file_write.rs#invoke_file_write_success_records_sink_executed"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/sinks/file_write.rs#invoke_file_write_failure_records_sink_execution_failed"
        status: pass
    human_judgment: false
  - id: D2
    description: "file.write Allowed-dispatch arm wired into evaluate_plan_node_and_record, mirroring file.create's arm, with output_value_id left untouched (None)"
    requirement: "FS-02"
    verification:
      - kind: unit
        ref: "cargo build -p brokerd (compiles the new arm)"
        status: pass
      - kind: other
        ref: "git diff crates/brokerd/src/server.rs — no output_value_id assignment in the new arm"
        status: pass
    human_judgment: false

# Metrics
duration: 15min
completed: 2026-07-17
status: complete
---

# Phase 33 Plan 04: file.write Broker Sink Summary

**New `invoke_file_write` broker sink module (two-phase durable audit) wired as a fourth Allowed-dispatch arm in `evaluate_plan_node_and_record`, overwriting existing WorkspaceRoot files via `write_within` with no mint and no new EffectRequest.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-07-18T00:07:00Z
- **Completed:** 2026-07-18T00:25:43Z
- **Tasks:** 2
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments
- `crates/brokerd/src/sinks/file_write.rs::invoke_file_write` mirrors `invoke_file_create` verbatim in structure: resolves `path`/`contents` from the per-connection `ValueStore`, calls `WorkspaceRoot::write_within` (overwrite-existing, O_WRONLY|O_TRUNC — not create), and records the same two-phase durable audit (`sink_executed` on success, `sink_execution_failed` FIRST then propagate on error, no retry), both chained onto `parent_id`/`parent_hash`.
- Registered `pub mod file_write;` in `sinks.rs` beside `process_exec`.
- Wired a fourth Allowed-decision dispatch arm in `server.rs`'s `evaluate_plan_node_and_record`, gated on `plan_node.sink.0 == "file.write"`, placed adjacent to the `file.create` arm with identical locking/head-advance shape. `output_value_id` is left untouched (stays `None`) — file.write is terminal and mints no value.
- Inline unit tests prove both the success (file content changes, chained `sink_executed`) and failure (missing target → ENOENT → `sink_execution_failed`, no retry, no `sink_executed`) paths.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create the file_write.rs sink module (invoke_file_write + private resolve_arg) and register it** - `fbd4c6c` (feat)
2. **Task 2: Add the file.write Allowed-dispatch arm in evaluate_plan_node_and_record + two-phase audit test** - `4a806ca` (feat)

**Plan metadata:** (this commit, docs: complete plan)

_Note: the inline test module for `invoke_file_write` was authored together with the sink module in Task 1's commit rather than split into a separate Task 2 commit — both tasks' verification gates (`cargo build -p brokerd`, `cargo test -p brokerd file_write`) pass identically regardless of which commit the tests land in, and no behavior differs from the plan's intent._

## Files Created/Modified
- `crates/brokerd/src/sinks/file_write.rs` - new `invoke_file_write` sink + private `resolve_arg` + inline two-phase-audit tests
- `crates/brokerd/src/sinks.rs` - `pub mod file_write;` registration
- `crates/brokerd/src/server.rs` - new `file.write` Allowed-dispatch arm in `evaluate_plan_node_and_record`

## Decisions Made
- Scoped `invoke_file_write` to the allow-path variant only (no `invoke_file_write_from_resolved` confirm-time sibling) — the plan's artifact list only calls for `invoke_file_write` + `resolve_arg`, and `file.write`'s threat register (T-33-08/09/10) does not require a separate frozen-literal re-invocation path for this milestone.
- Reused the literal event-type strings `sink_executed`/`sink_execution_failed` (not new file.write-specific event types) per RESEARCH's Open Question recommendation — the `actor` field (`sink:file.write:{effect_id}`) disambiguates which sink produced the event.

## Deviations from Plan

None - plan executed exactly as written. The only structural choice ("tests authored in Task 1 vs Task 2") is documented above under Task Commits and does not change scope, behavior, or verification outcomes from what the plan specified.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `file.write` is now a fully wired, tested, and audited broker sink alongside `file.create`, `email.send`, and `process.exec`.
- `./scripts/check-invariants.sh` passes all 4 gates (Gate 1 no new `EffectRequest`; Gate 3 unchanged — `file.write` never mints).
- `cargo build --workspace` and `cargo test -p brokerd file_write` both pass on macOS. Linux-gated enforcement tests are unaffected (no new `#[cfg(target_os = "linux")]` surfaces introduced by this plan beyond the existing `write_within` primitive from Plan 01).
- Ready for Plan 05 (or milestone-close verification) to proceed.

---
*Phase: 33-filesystem-read-write-breadth*
*Completed: 2026-07-17*

## Self-Check: PASSED
- FOUND: crates/brokerd/src/sinks/file_write.rs
- FOUND: .planning/phases/33-filesystem-read-write-breadth/33-04-SUMMARY.md
- FOUND: fbd4c6c (Task 1 commit)
- FOUND: 4a806ca (Task 2 commit)
- FOUND: b12ad40 (SUMMARY commit)
