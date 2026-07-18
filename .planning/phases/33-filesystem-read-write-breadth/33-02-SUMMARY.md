---
phase: 33-filesystem-read-write-breadth
plan: 02
subsystem: executor
tags: [rust, i2, taint, sink-schema, sink-sensitivity]

requires:
  - phase: 32-process-exec-sink
    provides: process.exec sink with exec_output origin_role minted on captured stdout/stderr
provides:
  - "file.write registered in executor's KNOWN_SINKS schema table (path/contents, both required, exact-match)"
  - "file.write routing/content sensitivity classification (path routes, contents is content-sensitive)"
  - "file.write CommitIrreversible effect class"
  - "file.write expected_role table: path mirrors file.create; contents admits path/exec_output/doc_fragment (WIDER than file.create) so a chained exec->write flow reaches I2's Block instead of a structural Deny"
affects: [33-03, 33-file-write-sink-invocation, 33-brokerd-dispatch]

tech-stack:
  added: []
  patterns: ["table-entries-only sink extension — no new ExecutorDecision variant, no submit_plan_node change"]

key-files:
  created: []
  modified:
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs

key-decisions:
  - "file.write's contents expected_role is Some(&[\"path\",\"exec_output\",\"doc_fragment\"]) — deliberately wider than file.create's Some(&[\"path\"]) — to admit the Phase 32 chained process.exec -> file.write flow at the structural role gate, while contents remaining content-sensitive still Blocks a genuinely tainted value"

patterns-established:
  - "New sink registration = KNOWN_SINKS row + 4 sink_sensitivity match arms + consts, zero changes to the enforcement loop itself (crates/executor/src/lib.rs)"

requirements-completed: [FS-03]

coverage:
  - id: D1
    description: "file.write is a registered KNOWN_SINKS entry with allowed/required = [path, contents], exact-match"
    requirement: "FS-03"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#file_write_exact_args_ok"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#file_write_unknown_arg_denied"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#file_write_missing_required_arg_denied"
        status: pass
    human_judgment: false
  - id: D2
    description: "file.write path is routing-sensitive, contents is content-sensitive"
    requirement: "FS-03"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_path_is_routing_sensitive"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_contents_not_routing_sensitive"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_contents_is_content_sensitive"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_path_not_content_sensitive"
        status: pass
    human_judgment: false
  - id: D3
    description: "file.write contents expected_role admits path/exec_output/doc_fragment (wider than file.create); path mirrors file.create verbatim"
    requirement: "FS-03"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_contents_expects_path_exec_output_or_doc_fragment"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_path_expects_path_or_relative_path"
        status: pass
    human_judgment: false
  - id: D4
    description: "file.write is CommitIrreversible (Draft-status session cannot invoke it)"
    requirement: "FS-03"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_write_is_commit_irreversible"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-18
status: complete
---

# Phase 33 Plan 02: file.write Executor Table Registration Summary

**Registered `file.write` in the executor's I2 schema/sensitivity/role tables as a table-entries-only extension, with `contents` deliberately admitting a wider role set than `file.create` to support the Phase 32 chained exec-output flow.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-18T00:12:59Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- `file.write` added to `KNOWN_SINKS` (`path`/`contents`, both required, exact-match — mirrors `file.create`)
- `sink_sensitivity.rs`: `FILE_WRITE_ROUTING_SENSITIVE`/`FILE_WRITE_CONTENT_SENSITIVE` consts, plus `file.write` arms in `sink_effect_class` (CommitIrreversible), `is_routing_sensitive`, `is_content_sensitive`, and `expected_role`
- `expected_role(file.write, "contents")` deliberately widened to `Some(&["path","exec_output","doc_fragment"])` (vs. `file.create`'s `Some(&["path"])`) so a tainted value from a chained `process.exec` -> `file.write` flow reaches I2's per-arg Block instead of a structural Step-1c Deny
- 11 new unit tests covering schema exact-match/unknown-arg/missing-arg, routing vs content sensitivity split, `CommitIrreversible` class, and both `expected_role` lists

## Task Commits

Each task was committed atomically:

1. **Task 1: Add the file.write KNOWN_SINKS entry and the four sink_sensitivity arms** - `983d9b5` (feat)
2. **Task 2: Unit tests for file.write schema, sensitivity, and slot-type-binding roles** - `fd6a359` (test)

**Plan metadata:** committed separately by orchestrator/final-commit step (worktree agent does not touch STATE.md/ROADMAP.md)

## Files Created/Modified
- `crates/executor/src/sink_schema.rs` - new `file.write` `KNOWN_SINKS` row + 3 new unit tests
- `crates/executor/src/sink_sensitivity.rs` - new consts, 4 new match arms, 8 new unit tests

## Decisions Made
- `file.write`'s `contents` `expected_role` is wider than `file.create`'s by design (admits `exec_output` and `doc_fragment`), per DESIGN §4.3 and the plan's explicit prohibition against a blind copy of `file.create`'s narrower list.
- `path`'s `expected_role` mirrors `file.create`'s verbatim (`Some(&["path","relative_path"])`) — no widening needed since there's no new legitimate origin_role for a write path beyond what file.create already admits.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `file.write` is now a fully governed I2 sink at the schema/sensitivity/role level, ready for the sink-invocation plan (`crates/brokerd/src/sinks/file_write.rs`, `write_within`, and the `server.rs` dispatch arm) to wire the live effect on top of these tables.
- No blockers. `submit_plan_node` (`crates/executor/src/lib.rs`) remains untouched, confirmed via `git diff --name-only` against this plan's base commit.

---
*Phase: 33-filesystem-read-write-breadth*
*Completed: 2026-07-18*
