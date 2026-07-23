---
phase: 25-regression-live-proof
plan: 02
subsystem: testing
tags: [regression-audit, taint-model, slot-type-binding, rust, cargo]

requires:
  - phase: 24-slot-type-binding-enforcement
    provides: Step 1c role check (SlotTypeMismatch DenyReason) wired into submit_plan_node, expected_role table in sink_sensitivity.rs
provides:
  - Independent per-site catalog of every Mac-buildable direct-mint fixture, cross-referenced against expected_role, with a CORRECT/NEEDS-FIX verdict
  - Confirmation the full Mac workspace (`cargo test --workspace --no-fail-fast`) is green after Phase 24's change
affects: [25-03-live-linux-reverification]

tech-stack:
  added: []
  patterns:
    - "Independent re-grep discipline: never cite a prior session's file-inventory or counts as final — re-run the search and record actual counts, even when they match."

key-files:
  created:
    - .planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md
  modified: []

key-decisions:
  - "0 fixtures needed fixing — Phase 24's role-assignment sweep is independently confirmed correct across all 31 role-checked-slot sites plus 11 sites confirmed out of scope (unconstrained slots, non-sink-routed mechanism tests, production mint implementations)."
  - "New finding: crates/brokerd/src/sinks/file_create.rs's own #[cfg(test)] setup() helper is a 6th Mac-buildable direct-mint file not named in the prior session's 5-file target list — found only because the re-grep was run independently rather than trusting the prior inventory."

patterns-established:
  - "A fixture that deliberately mints a mismatched/None origin_role at a role-checked slot is NOT a bypass if it asserts the Denied(SlotTypeMismatch) outcome — only a fixture asserting the old permissive Allowed while carrying a mismatched role would be a bypass."

requirements-completed: [T2-07]

coverage:
  - id: D1
    description: "Independent re-grep of every Linux-gated file (22 found) confirms 0 construct a direct-mint fixture — the T2-07 blind spot is entirely in the 9 Mac-buildable files identified by the second grep."
    requirement: T2-07
    verification:
      - kind: other
        ref: "find crates cli -name '*.rs' ... | xargs grep -l 'cfg(target_os = \"linux\")' (22 files); per-file grep -q '\\.mint(\\|ValueRecord {' — 0 matches"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every Mac-buildable direct-mint site routed into a role-checked slot (31 sites across executor_decision.rs, durable_anchor.rs, s9_acceptance.rs, file_create.rs) cross-referenced against sink_sensitivity.rs's expected_role table — 31/31 CORRECT, 0 NEEDS-FIX, catalogued in 25-REGRESSION-AUDIT.md."
    requirement: T2-07
    verification:
      - kind: other
        ref: ".planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md per-site catalog"
        status: pass
    human_judgment: false
  - id: D3
    description: "Full Mac workspace regression (cargo build --workspace && cargo test --workspace --no-fail-fast) is green: exit 0, 46 test-result blocks all ok, 269 passed / 0 failed / 0 ignored."
    requirement: T2-07
    verification:
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast (captured exit code before pipe, per learned-rule on exit-code-through-pipe)"
        status: pass
    human_judgment: false

duration: 22min
completed: 2026-07-12
status: complete
---

# Phase 25 Plan 02: Independent T2-07 Regression Audit Summary

**Independently re-ran both T2-07 search commands from scratch (not citing prior counts), cross-referenced all 31 role-checked-slot direct-mint sites against `sink_sensitivity.rs`'s `expected_role` table, found 0 bypasses, discovered one new Mac-buildable direct-mint file not in the prior session's target list, and confirmed the full Mac workspace green (46 binaries, 269 passed, 0 failed).**

## Performance

- **Duration:** 22 min
- **Tasks:** 2
- **Files modified:** 1 (`.planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md`, created then extended)

## Accomplishments

- Re-ran the Linux-gated-file blind-spot check independently: found 22 files (vs. RESEARCH's cited 21 — the delta is `s9_live_block.rs`, extended by this same phase's Plan 01), confirmed 0 of them construct a direct-mint fixture.
- Re-ran the Mac-buildable direct-mint grep independently: found 42 raw hits across 9 files (not the 5 named in PATTERNS.md) — read every hit's surrounding code to classify it as (a) a role-checked-slot fixture with a CORRECT/NEEDS-FIX verdict, (b) a fixture routed to an unconstrained slot (no-op by design), (c) a fixture never routed to any sink at all (round-trip/serde/mechanism unit tests), or (d) production mint-implementation code / a struct definition / a doc comment (not a fixture at all).
- Found 31 sites actually routed into a role-checked slot; all 31 carry an `origin_role` consistent with `sink_sensitivity.rs`'s `expected_role` table. 0 NEEDS-FIX.
- Confirmed the two deliberately-mismatched fixtures (`role_mismatch_denies`, `role_none_at_role_checked_slot_denies`) are the intentional adversarial tests proving Step 1c fires — both assert `Denied`, never `Allowed`, so neither is a bypass.
- New finding beyond the prior session's inventory: `crates/brokerd/src/sinks/file_create.rs`'s own `#[cfg(test)] setup()` helper is a 6th Mac-buildable direct-mint file (correctly role-tagged) — demonstrates why the independent re-grep mattered rather than trusting the named 5-file list.
- Ran `cargo build --workspace && cargo test --workspace --no-fail-fast`: exit 0, 46 test-result blocks all `ok`, 269 passed / 0 failed / 0 ignored. No production code was touched (0 fixes needed).

## Task Commits

1. **Task 1: Independent re-grep + per-site cross-reference audit** - `be9e91d` (docs)
2. **Task 2: Reconcile any findings + prove the full Mac workspace green** - `a481069` (test)

**Plan metadata:** (this commit, docs — see final_commit step)

## Files Created/Modified

- `.planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md` - Per-site catalog of all 42 direct-mint grep hits with verdicts, plus a Reconciliation section recording the green cargo-test run and final T2-07 PASS verdict.

## Decisions Made

- No fixture fixes were needed — Phase 24's role-assignment sweep held up under independent re-audit. No production/TCB code was touched this plan (proof-only phase, as required).
- Classified sites that never route a value to any sink (round-trip/serde tests in `value_store.rs`, `types_compile.rs`, and `quarantine.rs`'s dedup-ordering test) as "not applicable" to the T2-07 bypass question, since Step 1c's role check only ever fires for values actually placed in a `PlanArg` and resolved through `submit_plan_node`.

## Deviations from Plan

None — plan executed exactly as written. The audit surfaced one file (`file_create.rs`'s test module) not named in the prior session's target list, but this is exactly the kind of independent finding the plan's `read_first` section anticipated and required, not a deviation from the task's instructions.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- T2-07 is independently confirmed PASS with a durable, per-site audit record. The full Mac workspace is green (46 binaries, 269 passed, 0 failed), satisfying this plan's per-wave-merge gate before Plan 03's expensive Linux `scripts/mailpit-verify.sh` re-run (T2-08).
- No blockers or concerns for Plan 03.

---
*Phase: 25-regression-live-proof*
*Completed: 2026-07-12*

## Self-Check: PASSED

- FOUND: .planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md
- FOUND: .planning/phases/25-regression-live-proof/25-02-SUMMARY.md
- FOUND commit: be9e91d (Task 1)
- FOUND commit: a481069 (Task 2)
- FOUND commit: ac831a5 (SUMMARY.md)
