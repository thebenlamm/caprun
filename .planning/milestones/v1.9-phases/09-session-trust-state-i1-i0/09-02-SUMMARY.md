---
phase: 09-session-trust-state-i1-i0
plan: 02
subsystem: executor
tags: [rust, tcb, i1, i0, session-trust-state, executor-decision]

# Dependency graph
requires:
  - phase: 09-session-trust-state-i1-i0
    plan: "09-01"
    provides: "SessionStatus::Draft, DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink }"
provides:
  - "executor::sink_sensitivity::EffectClass { Observe, MutateReversible, CommitIrreversible }"
  - "executor::sink_sensitivity::sink_effect_class(&SinkId) -> EffectClass (hardcoded, fail-closed)"
  - "5th parameter session_status: &SessionStatus on executor::submit_plan_node"
  - "Step 0.5 post-loop draft-only CommitIrreversible deny inside submit_plan_node"
  - "#[cfg(any(test, feature = \"test-fixtures\"))] test.observe fixture sink"
affects: [09-03-broker-demotion, 09-04-cli-onramp, 11-live-acceptance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cargo self dev-dependency + feature flag (test-fixtures) to make a fixture visible to both unit AND integration tests, since bare #[cfg(test)] is invisible across the tests/ crate boundary"
    - "Post-loop class-level deny appended after an existing per-arg security loop, preserving per-arg-Block precedence (I2 over I1/I0)"
    - "Exhaustive match over all 6 SessionStatus variants, no wildcard arm"

key-files:
  created: []
  modified:
    - crates/executor/src/sink_sensitivity.rs
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/lib.rs
    - crates/executor/tests/executor_decision.rs
    - crates/executor/Cargo.toml

key-decisions:
  - "test.observe fixture gated on #[cfg(any(test, feature = \"test-fixtures\"))] instead of bare #[cfg(test)], with a self dev-dependency (executor = { path = \".\", features = [\"test-fixtures\"] }) added to Cargo.toml — discovered mid-Task-3 that plain #[cfg(test)] items are invisible to integration tests in tests/, which link the crate as a normal (non `--cfg test`) dependency"
  - "EffectClass placed in executor::sink_sensitivity (RESEARCH Assumption A3), not runtime-core — nothing outside executor needs it yet"
  - "Added a fourth new test (draft_session_tainted_routing_arg_still_blocks_not_denied) beyond the plan's two named tests, to make the I2-Block-precedence-over-Step-0.5 requirement (DESIGN §8/§11 condition 4, round-1 blocker B1) an explicit regression test rather than only an inline code comment"

requirements-completed: [TAINT-02, TAINT-03]

coverage:
  - id: D1
    description: "EffectClass enum (3 variants) + sink_effect_class hardcoded classifier, fail-closed unknown-sink"
    requirement: "TAINT-02"
    verification:
      - kind: unit
        ref: "cargo test -p executor --no-fail-fast (email_send_is_commit_irreversible, file_create_is_commit_irreversible, unregistered_sink_is_fail_closed_commit_irreversible, test_observe_fixture_is_observe)"
        status: pass
    human_judgment: false
  - id: D2
    description: "submit_plan_node gains session_status: &SessionStatus 5th param; Step 0.5 runs strictly after the per-arg loop, exhaustive match, denies only Draft+CommitIrreversible with no prior Block"
    requirement: "TAINT-02"
    verification:
      - kind: unit
        ref: "cargo test -p executor --test executor_decision draft_session_denies_commit_irreversible"
        status: pass
    human_judgment: false
  - id: D3
    description: "Draft + Observe-class sink (test.observe fixture) still Allowed, proven end-to-end through the full submit_plan_node path"
    requirement: "TAINT-03"
    verification:
      - kind: unit
        ref: "cargo test -p executor --test executor_decision draft_session_allows_observe"
        status: pass
    human_judgment: false
  - id: D4
    description: "I2 per-arg Block takes precedence over the Step 0.5 class deny (Draft + CommitIrreversible + tainted routing arg still Blocks, not Denied)"
    requirement: "TAINT-02 (non-regression, DESIGN §11 condition 4)"
    verification:
      - kind: unit
        ref: "cargo test -p executor --test executor_decision draft_session_tainted_routing_arg_still_blocks_not_denied"
        status: pass
    human_judgment: false

duration: ~35min
completed: 2026-07-07
status: complete
---

# Phase 9 Plan 2: Executor Draft-Only Deny Mechanism (Step 0.5) Summary

**Added the single TCB deny function for I1/I0: a hardcoded `EffectClass`/`sink_effect_class` classifier, a `session_status: &SessionStatus` parameter on `executor::submit_plan_node`, and a post-loop Step 0.5 that denies `Draft`+`CommitIrreversible` plan nodes while never pre-empting the existing per-arg I2 Block.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-07
- **Tasks:** 3 completed
- **Files modified:** 5 (4 planned + `Cargo.toml`, discovered necessary mid-Task-3)

## Accomplishments

- New `EffectClass { Observe, MutateReversible, CommitIrreversible }` enum and `sink_effect_class(sink: &SinkId) -> EffectClass` hardcoded classifier in `crates/executor/src/sink_sensitivity.rs`, mirroring the existing `is_routing_sensitive` match shape exactly. Both live sinks (`email.send`, `file.create`) map to `CommitIrreversible`; unknown sinks fail-closed to `CommitIrreversible`.
- `submit_plan_node` gains a 5th parameter `session_status: &SessionStatus`, sourced only as an explicit caller-supplied parameter (never from `PlanNode` or IPC).
- New **Step 0.5** inserted strictly *after* the existing per-arg I2 loop completes with no Block and *before* the final `Allowed` return — an exhaustive match over all six `SessionStatus` variants (no wildcard arm) that denies only `Draft` + `CommitIrreversible` with `DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink }`.
- All 8 pre-existing `submit_plan_node` calls in `crates/executor/tests/executor_decision.rs` updated to pass `&SessionStatus::Active`, preserving existing semantics.
- 4 new tests: `draft_session_denies_commit_irreversible` (TAINT-02), `draft_session_allows_observe` (TAINT-03, driven end-to-end through the real `submit_plan_node` path via the `test.observe` fixture), `draft_session_tainted_routing_arg_still_blocks_not_denied` (I2-Block-precedence regression, DESIGN §11 condition 4), plus 4 new unit tests on `sink_effect_class` itself.
- `cargo test -p executor --no-fail-fast`: 33 passed, 0 failed.
- `./scripts/check-invariants.sh`: both gates still PASS (no raw `EffectRequest` token; `runtime-core` still pure).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add EffectClass, sink_effect_class, and the test.observe fixture sink** - `0f657ac` (feat)
2. **Task 2: Add the session_status parameter and the post-loop Step 0.5 deny to submit_plan_node** - `48bedde` (feat)
3. **Task 3: Update the 8 executor test call sites and add TAINT-02/TAINT-03 tests** - `d5224dc` (test)

## Files Created/Modified

- `crates/executor/src/sink_sensitivity.rs` - added `EffectClass` enum, `sink_effect_class` classifier, 4 new unit tests
- `crates/executor/src/sink_schema.rs` - added `TEST_KNOWN_SINKS` registry + `test_schema_for` lookup for the `test.observe` fixture
- `crates/executor/src/lib.rs` - `submit_plan_node` gains `session_status: &SessionStatus`; new Step 0.5 block
- `crates/executor/tests/executor_decision.rs` - all 8 call sites updated to 5 args; 3 new integration tests
- `crates/executor/Cargo.toml` - new `test-fixtures` feature + self dev-dependency (deviation, see below)
- `Cargo.lock` - updated for the new self dev-dependency edge

## Decisions Made

- `EffectClass` lives in `executor::sink_sensitivity`, not `runtime-core` (RESEARCH Assumption A3) — nothing outside `executor` currently constructs or matches on it.
- Added a fourth test (`draft_session_tainted_routing_arg_still_blocks_not_denied`) beyond the plan's two explicitly-named tests, making the I2-precedence requirement (DESIGN §8/§11 condition 4, round-1 blocker B1) an explicit regression test rather than relying on inline comments alone.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] `#[cfg(test)]` fixture invisible to integration tests**
- **Found during:** Task 3, first `cargo test -p executor` run after adding the `draft_session_allows_observe` test
- **Issue:** The plan (and RESEARCH.md's own code example) specified gating the `test.observe` fixture sink and its `EffectClass::Observe` classification with plain `#[cfg(test)]`. This compiles, but `cargo test -p executor` failed the new test with `Denied { reason: UnknownSink("test.observe") }` — bare `#[cfg(test)]` items in a library crate are only visible when that crate is compiled with `--cfg test` for its own unit tests (`cargo test --lib`); integration tests under `tests/` link the crate as a normal external dependency, built *without* `--cfg test`, so the fixture sink was never registered for `executor_decision.rs`.
- **Fix:** Added a `test-fixtures` Cargo feature (`[features] test-fixtures = []`) plus a self dev-dependency (`executor = { path = ".", features = ["test-fixtures"] }`) in `crates/executor/Cargo.toml`. Cargo unifies this dev-dependency's feature with the lib target when building the package's test targets, making the fixture visible to both unit and integration tests when gated on `#[cfg(any(test, feature = "test-fixtures"))]`. Verified the fixture is absent from a plain `cargo build -p executor` (no `test.observe` reference in the compiled output) — the fail-closed/never-in-production property the plan required is preserved.
- **Files modified:** `crates/executor/Cargo.toml`, `crates/executor/src/sink_sensitivity.rs`, `crates/executor/src/sink_schema.rs`, `Cargo.lock`
- **Commit:** `d5224dc`

Or: this is the only deviation — all other plan instructions (Step 0.5 placement, exhaustive match, `EffectClass` shape, call-site updates) were followed exactly as written.

## Issues Encountered

None beyond the `#[cfg(test)]`-visibility issue documented above.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. No hardcoded empty/placeholder values were introduced.

## Threat Flags

None. This plan's changes are exactly the mitigations named in its own `<threat_model>` (T-09-02 through T-09-05) — no new, undocumented security-relevant surface was introduced.

## Next Phase Readiness

- `executor::submit_plan_node` now requires a 5th `&SessionStatus` argument everywhere. `cargo build --workspace` fails to compile in `crates/brokerd` (2 call sites: `crates/brokerd/src/server.rs:353`, `crates/brokerd/src/lib.rs:41`) — this is the exact, expected breaking-signature ripple this plan's own `<verification>` section calls out; Plan 09-03 (broker demotion) updates both call sites and threads the real `session_status` value through `server.rs`'s per-connection state.
- `EffectClass`, `sink_effect_class`, and the `test.observe` fixture (now properly visible to both unit and integration test builds) are ready for Plan 09-03/09-04 to reference if needed.
- No blockers.

## Self-Check: PASSED
- FOUND: crates/executor/src/sink_sensitivity.rs
- FOUND: crates/executor/src/sink_schema.rs
- FOUND: crates/executor/src/lib.rs
- FOUND: crates/executor/tests/executor_decision.rs
- FOUND: crates/executor/Cargo.toml
- FOUND commit 0f657ac
- FOUND commit 48bedde
- FOUND commit d5224dc

---
*Phase: 09-session-trust-state-i1-i0*
*Completed: 2026-07-07*
