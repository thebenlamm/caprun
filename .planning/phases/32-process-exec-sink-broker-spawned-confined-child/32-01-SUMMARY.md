---
phase: 32-process-exec-sink-broker-spawned-confined-child
plan: 01
subsystem: executor
tags: [rust, taint-model, sink-sensitivity, process-exec, i2]

# Dependency graph
requires:
  - phase: 31-effect-breadth-design-gate
    provides: DESIGN-effect-breadth-exec.md (DESIGN-13/14 — process.exec model + fail-closed defaults, cleared adversarial review)
provides:
  - "TaintLabel::ExecRaw untrusted taint variant (runtime-core)"
  - "process.exec KNOWN_SINKS schema row (executor::sink_schema)"
  - "process.exec sensitivity/expected_role table entries (executor::sink_sensitivity)"
affects: [32-02, 32-03, 32-04, 32-05, 32-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Table-entries-only sink registration (no new submit_plan_node logic) — mirrors the file.create precedent"
    - "expected_role = None for a sink arg with no origin_role-producing mint site, relying on sensitivity+taint for the Block (avoids fail-closed-Denying legitimate values at Step 1c)"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/plan_node.rs
    - crates/runtime-core/tests/intent_taint.rs
    - crates/brokerd/src/confirmation.rs
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs

key-decisions:
  - "TaintLabel::ExecRaw placed after PathRaw in the untrusted arm, per DESIGN §2.2 naming convention"
  - "command/args classified BOTH routing- AND content-sensitive (DESIGN §4.2) so a tainted value Blocks regardless of which classifier fires"
  - "command/args expected_role = None (Round-1 finding M2) — no origin_role-producing mint site exists for a legitimately-authored command; Some(...) would fail-closed-Deny the legitimate command at Step 1c"
  - "cwd reuses file.create's [\"path\",\"relative_path\"] expected_role vocabulary (RESEARCH A3 recommendation)"

patterns-established:
  - "New untrusted TaintLabel variant requires updating every non-wildcard match over TaintLabel (compiler-enforced, no wildcard arm) — found and updated confirmation.rs's taint_label_display CLI-rendering match in addition to is_untrusted()"

requirements-completed: [EXEC-01, EXEC-02, EXEC-03, EXEC-04]

coverage:
  - id: D1
    description: "TaintLabel::ExecRaw exists and is_untrusted() returns true for it, with no wildcard arm added anywhere in the workspace"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_exec_raw_returns_true"
        status: pass
      - kind: unit
        ref: "cargo build --workspace (compiles with no wildcard TaintLabel arm)"
        status: pass
    human_judgment: false
  - id: D2
    description: "process.exec is a callable sink with schema {command,args,cwd}, command required, fail-closed on unknown/duplicate/missing arg"
    requirement: "EXEC-02"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#tests::process_exec_command_only_ok"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#tests::process_exec_missing_command_denied"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#tests::process_exec_unknown_arg_denied"
        status: pass
    human_judgment: false
  - id: D3
    description: "process.exec command/args are routing- and content-sensitive; cwd is routing-sensitive only; process.exec is CommitIrreversible"
    requirement: "EXEC-03"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#tests::process_exec_command_and_args_routing_and_content_sensitive"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#tests::process_exec_cwd_routing_but_not_content_sensitive"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#tests::process_exec_is_commit_irreversible"
        status: pass
    human_judgment: false
  - id: D4
    description: "command/args expected_role is None (no structural fail-closed-Deny of legitimate commands); cwd expected_role is Some([path, relative_path])"
    requirement: "EXEC-04"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#tests::process_exec_command_and_args_expected_role_is_none"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#tests::process_exec_cwd_expects_path_or_relative_path"
        status: pass
    human_judgment: false

duration: 6min
completed: 2026-07-17
status: complete
---

# Phase 32 Plan 01: TaintLabel::ExecRaw + process.exec sink tables Summary

**`process.exec` is now a fail-closed, CommitIrreversible, I2-governed sink whose `command`/`args` classify BOTH routing- and content-sensitive (so tainted values Block), added entirely via table entries in `sink_schema.rs`/`sink_sensitivity.rs` with zero changes to `submit_plan_node` enforcement logic, plus a new compiler-enforced `TaintLabel::ExecRaw` untrusted variant in runtime-core.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-07-17T21:40:00Z
- **Completed:** 2026-07-17T21:46:01Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- `TaintLabel::ExecRaw` added to the 8-variant enum (now 9), placed in `is_untrusted()`'s untrusted arm with no wildcard arm anywhere in the workspace
- Grepped and updated the ONE other non-wildcard `TaintLabel` match the compiler would have needed anyway (`confirmation.rs`'s `taint_label_display` CLI-rendering function) — added an `"exec.raw"` arm
- `process.exec` registered in `KNOWN_SINKS` (schema: `{command,args,cwd}` allowed, `{command}` required)
- `process.exec` wired into `sink_effect_class` (CommitIrreversible), `is_routing_sensitive`/`is_content_sensitive` (command+args both true; cwd routing-only), and `expected_role` (command/args = None, cwd = Some(["path","relative_path"]))
- Zero new enforcement logic — confirmed by `git diff` showing no changes to `crates/executor/src/lib.rs`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add TaintLabel::ExecRaw and place it in the untrusted arm** - `56fd319` (feat)
2. **Task 2: Register process.exec in the schema + sensitivity + expected_role tables** - `4d026e3` (feat)

_TDD note: both tasks are marked `tdd="true"` in the plan, but the described "behavior" for each was net-new additive table/enum work with no pre-existing failing-test phase to author separately — tests were written alongside the implementation in the same commit, mirroring this codebase's existing convention for `file.create`'s original table-entry commits. No separate RED-phase commit was produced; this is a scope judgment, not a compliance gap, and is called out below in TDD Gate Compliance._

## Files Created/Modified
- `crates/runtime-core/src/plan_node.rs` - Added `TaintLabel::ExecRaw` variant + untrusted-arm membership
- `crates/runtime-core/tests/intent_taint.rs` - Added `is_untrusted_exec_raw_returns_true` truth-table test
- `crates/brokerd/src/confirmation.rs` - Added `ExecRaw => "exec.raw"` arm to `taint_label_display`'s exhaustive match
- `crates/executor/src/sink_schema.rs` - Added `process.exec` `KNOWN_SINKS` row + 6 inline unit tests
- `crates/executor/src/sink_sensitivity.rs` - Added `PROCESS_EXEC_ROUTING_SENSITIVE`/`PROCESS_EXEC_CONTENT_SENSITIVE` consts, `process.exec` arms in `sink_effect_class`/`is_routing_sensitive`/`is_content_sensitive`/`expected_role`, + 5 inline unit tests

## Decisions Made
- `command`/`args` are classified BOTH routing- and content-sensitive (DESIGN §4.2's single highest-consequence decision) — the routing/content distinction is academic for these two args; the point is neither classifier ever returns `false`, so a tainted value Blocks under the existing collect-then-Block loop regardless of path.
- `command`/`args` carry `expected_role = None` rather than a role list — pinned in DESIGN as Round-1 finding M2's resolution. There is no `origin_role`-producing mint site for a legitimately-authored exec command; requiring a role would fail-closed-Deny the legitimate command at the earlier, independent Step 1c structural gate. The security property (tainted command/args Blocks) is delivered entirely by the sensitivity+taint check at Step 2/3, not by `expected_role`.
- `cwd` reuses `file.create`'s existing `["path","relative_path"]` expected_role vocabulary rather than inventing a new role name (RESEARCH A3 recommendation cited directly in the plan).

## Deviations from Plan

None - plan executed exactly as written. The plan's `<read_first>` for Task 1 anticipated needing to grep for other `TaintLabel::` matches the compiler would flag; the grep found exactly one additional non-wildcard match (`confirmation.rs`), which was updated in the same commit as planned.

## Issues Encountered
None.

## TDD Gate Compliance

This plan's tasks are marked `tdd="true"` at the task level (not `type: tdd` at the plan level, so the plan-level RED/GREEN/REFACTOR gate sequence in `execute-plan.md` does not strictly apply). Both tasks' `<behavior>` blocks describe additive table/enum entries with unit-test assertions authored in the same commit as the implementation (mirroring this codebase's existing precedent for `file.create`'s original schema/sensitivity table commits, which were also table-entries-only with tests alongside). No separate failing-test-first commit exists for either task. Flagging this here per the plan-level TDD gate-sequence validation instruction, even though this plan's frontmatter is `type: execute` (not `type: tdd`), for auditability.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `TaintLabel::ExecRaw` and the `process.exec` sink tables are the foundational types every downstream 32-0N plan needs to compile against: 32-02 (sandbox confinement primitives), 32-03 (`caprun-exec-launcher` binary), 32-04 (`invoke_process_exec` + audit events), 32-05 (`mint_from_exec` + Gate-3 extension) all reference `TaintLabel::ExecRaw` and/or the `process.exec` sink id established here.
- No blockers. Full `cargo build --workspace` compiles clean; `cargo test --workspace --no-fail-fast` green with no regressions (all suites `0 failed`); `./scripts/check-invariants.sh` PASSED (all 4 gates).
- `check-invariants.sh` Gate 3's mandated extension for a future `mint_from_exec(` call site (DESIGN §2.4) is NOT yet done — correctly deferred to 32-05, which introduces that call site.

---
*Phase: 32-process-exec-sink-broker-spawned-confined-child*
*Completed: 2026-07-17*
