---
phase: 09-session-trust-state-i1-i0
plan: 01
subsystem: runtime-core
tags: [rust, domain-types, taint-model, session-lifecycle]

# Dependency graph
requires:
  - phase: 08-design-gate
    provides: DESIGN-session-trust-state.md (APPROVED, DEC-ai-review-satisfies-human-gate)
provides:
  - "SessionStatus::Draft variant (runtime_core::SessionStatus)"
  - "SeedProvenance { TrustedArg, FileDerived } typed enum"
  - "DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink: SinkId } variant"
affects: [09-02-executor-step0.5, 09-03-broker-demotion, 09-04-cli-onramp]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Insert-after-Active variant addition to an existing serde-by-name enum (no wrapper type)"
    - "Exhaustive match, explicit arm per new enum variant, no wildcard (security enum discipline)"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/session.rs
    - crates/runtime-core/src/executor_decision.rs
    - crates/runtime-core/src/lib.rs

key-decisions:
  - "SeedProvenance added as a standalone enum in session.rs with no struct field yet — Plans 03/04 decide where it is threaded/persisted, per plan scope"
  - "DraftOnlySessionDeniesCommitIrreversible added as a struct-variant (named `sink` field) matching DESIGN §7's exact shape, not a tuple variant"

patterns-established:
  - "Security enums (SessionStatus, DenyReason) are matched exhaustively with no `_` wildcard at every in-crate call site; this plan preserved that discipline in code()/Display"

requirements-completed: [TAINT-01, TAINT-02, ORIGIN-01, ORIGIN-02]

coverage:
  - id: D1
    description: "SessionStatus::Draft variant inserted between Active and WaitingApproval"
    requirement: "TAINT-01"
    verification:
      - kind: unit
        ref: "cargo test -p runtime-core --no-fail-fast (session_constructs_and_round_trips, all_domain_types_compose_in_a_plan_node)"
        status: pass
    human_judgment: false
  - id: D2
    description: "SeedProvenance { TrustedArg, FileDerived } typed enum added, no third variant"
    requirement: "ORIGIN-01"
    verification:
      - kind: unit
        ref: "cargo test -p runtime-core --no-fail-fast (workspace build + full test suite green)"
        status: pass
    human_judgment: false
  - id: D3
    description: "DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink: SinkId } appended to the single DenyReason taxonomy"
    requirement: "TAINT-02"
    verification:
      - kind: unit
        ref: "cargo test -p runtime-core --no-fail-fast (executor_decision_has_all_variants, executor_decision_has_not_implemented_variant)"
        status: pass
    human_judgment: false

duration: 8min
completed: 2026-07-07
status: complete
---

# Phase 9 Plan 1: Session Trust State Domain Types Summary

**Added `SessionStatus::Draft`, a new `SeedProvenance` typed enum, and `DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink }` to `runtime-core` — the pure vocabulary Plans 02-04 build the I1/I0 mechanism on.**

## Performance

- **Duration:** ~8 min
- **Completed:** 2026-07-07T02:05:49Z
- **Tasks:** 2 completed
- **Files modified:** 3 (2 planned + 1 re-export)

## Accomplishments
- `SessionStatus::Draft` inserted between `Active` and `WaitingApproval` per DESIGN-session-trust-state.md §1 — no wrapper type, existing serde-by-name derives unchanged
- New `SeedProvenance { TrustedArg, FileDerived }` enum added in `session.rs`, re-exported from `runtime-core::lib`
- `DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink: SinkId }` appended to the ONE existing `DenyReason` taxonomy, with explicit (non-wildcard) arms added to both `code()` and the `Display` impl
- `cargo test -p runtime-core --no-fail-fast` green (28 tests); `cargo build --workspace` confirmed the whole workspace still compiles with the new variants

## Task Commits

Each task was committed atomically:

1. **Task 1: Add SessionStatus::Draft and the SeedProvenance enum in runtime-core session.rs** - `ad8c4fb` (feat)
2. **Task 2: Append the DraftOnlySessionDeniesCommitIrreversible variant to DenyReason** - `20377cf` (feat)

## Files Created/Modified
- `crates/runtime-core/src/session.rs` - added `Draft` variant to `SessionStatus`; added `SeedProvenance` enum
- `crates/runtime-core/src/executor_decision.rs` - appended `DraftOnlySessionDeniesCommitIrreversible { sink }` to `DenyReason`; updated `code()`/`Display` exhaustive matches
- `crates/runtime-core/src/lib.rs` - re-exported `SeedProvenance` alongside `Session`/`SessionStatus`

## Decisions Made
- `SeedProvenance` is a standalone enum with no struct field wired yet (plan explicitly scopes threading/persistence to Plans 03/04)
- `DraftOnlySessionDeniesCommitIrreversible` uses the struct-variant `{ sink: SinkId }` shape (matches DESIGN §7 verbatim), referenced via `crate::plan_node::SinkId` — same import path `plan_node.rs` exposes elsewhere in the file

## Deviations from Plan

None - plan executed exactly as written. No exhaustive `match SessionStatus` existed anywhere in `runtime-core` prior to this plan, so no additional match-site fix was needed for that enum; `DenyReason`'s two existing exhaustive matches (`code()`, `Display`) were updated with explicit arms per the plan's own instruction.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `SessionStatus::Draft`, `SeedProvenance`, and `DenyReason::DraftOnlySessionDeniesCommitIrreversible` all exist and compile — Plan 02 (executor Step 0.5) can now add the `session_status: &SessionStatus` parameter and the post-loop deny match; Plan 03 (broker demotion) and Plan 04 (CLI on-ramp) can reference `SeedProvenance` and the new `DenyReason` variant by name.
- No blockers.

## Self-Check: PASSED
- FOUND: crates/runtime-core/src/session.rs
- FOUND: crates/runtime-core/src/executor_decision.rs
- FOUND: crates/runtime-core/src/lib.rs
- FOUND commit ad8c4fb
- FOUND commit 20377cf

---
*Phase: 09-session-trust-state-i1-i0*
*Completed: 2026-07-07*
