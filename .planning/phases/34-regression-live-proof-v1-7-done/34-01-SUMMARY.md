---
phase: 34-regression-live-proof-v1-7-done
plan: 01
subsystem: brokerd
tags: [rust, tokio, confirm-release, process-exec, audit-dag, seccomp, landlock]

requires:
  - phase: 32-process-exec-sink
    provides: invoke_process_exec (Allowed-path confined-child spawn+capture), run_launcher, resolve_launcher_path
  - phase: 33-filesystem-read-write-breadth
    provides: invoke_file_write_from_resolved / invoke_file_create_from_resolved (the _from_resolved sibling pattern this plan mirrors), the confirmation.rs Step-7 dispatch shape

provides:
  - invoke_process_exec_from_resolved (async, plain &rusqlite::Connection) — confirm-time release twin of invoke_process_exec
  - Two #[cfg(test)] #[cfg(target_os = "linux")] unit tests proving the function's success and spawn-failure audit paths on real Linux

affects: [34-02, 34-03, 34-04, confirmation.rs Step-7 dispatch]

tech-stack:
  added: []
  patterns:
    - "_from_resolved confirm-time release twin (ValueStore-free, frozen ResolvedArg lookup instead of live resolution)"
    - "plain &rusqlite::Connection for confirm-time (single-shot, no concurrent broker tasks) vs Arc<Mutex<>> for the Allowed path"

key-files:
  created: []
  modified:
    - crates/brokerd/src/sinks/process_exec.rs

key-decisions:
  - "Used /bin/yes byte-cap trip (not a nonexistent target command) to force the genuine run_launcher Err path — a nonexistent target's execve failure inside caprun-exec-launcher surfaces as a normal nonzero-exit process_exited, not a run_launcher Err, so it cannot exercise the process_spawn_failed leg."
  - "resolved_literal/resolved_literal_optional are a private per-file copy (mirrors file_write.rs's own private resolved_literal) — not extracted into a shared helper, matching the project's established _from_resolved sibling-shape discipline."

requirements-completed: [EXEC-05]

coverage:
  - id: D1
    description: "invoke_process_exec_from_resolved re-invokes a released process.exec from frozen ResolvedArg literals via the confined caprun-exec-launcher, chaining process_exited onto the confirm_granted parent head"
    requirement: "EXEC-05"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/process_exec.rs#invoke_process_exec_from_resolved_success_appends_process_exited_chained_on_parent"
        status: pass
    human_judgment: false
  - id: D2
    description: "On spawn failure (byte-cap fail-closed), process_spawn_failed is appended chained on the passed parent and the error propagates (no retry)"
    requirement: "EXEC-05"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/process_exec.rs#invoke_process_exec_from_resolved_spawn_failure_appends_process_spawn_failed"
        status: pass
    human_judgment: false

duration: 20min
completed: 2026-07-18
status: complete
---

# Phase 34 Plan 01: process.exec confirm-release sink twin Summary

**Added `invoke_process_exec_from_resolved` — the confirm-time release twin of `invoke_process_exec` — reusing the same confined-launcher spawn discipline and chaining its two-phase audit onto the passed `confirm_granted` head, with Linux unit coverage proving both the success and byte-cap spawn-failure paths.**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-18T02:17:05Z
- **Tasks:** 2/2 completed
- **Files modified:** 1 (`crates/brokerd/src/sinks/process_exec.rs`)

## Accomplishments

- `invoke_process_exec_from_resolved` exists: async, plain `&rusqlite::Connection` (no `Arc<Mutex<>>`), resolves `command`/`args`/`cwd` from a frozen `ResolvedArg` slice, reuses `run_launcher` and `resolve_launcher_path` unmodified, and appends `process_exited`/`process_spawn_failed` chained onto the caller-supplied `parent_id`/`parent_hash` (the real `confirm_granted` head — never a fresh root, D-04).
- The module still never mints (`mint_from_exec` call count: 0) and adds no `submit_plan_node`/`ExecutorDecision`/raw-effect-request path — `./scripts/check-invariants.sh` Gate 1 and Gate 3 both stay green.
- Two `#[cfg(test)] #[cfg(target_os = "linux")]` unit tests added, proven on real Linux via `scripts/mailpit-verify.sh` (`cargo build --workspace && cargo test -p brokerd invoke_process_exec_from_resolved`): 2 passed, 0 failed, true-exit-0 captured before any pipe.

## Task Commits

1. **Task 1: Add invoke_process_exec_from_resolved (async release twin)** - `99775c7` (feat)
2. **Task 2: Linux unit tests for invoke_process_exec_from_resolved** - `548b3cb` (test)

_No separate plan-metadata commit — worktree mode; the orchestrator handles STATE.md/ROADMAP.md centrally after merge._

## Files Created/Modified

- `crates/brokerd/src/sinks/process_exec.rs` — added `invoke_process_exec_from_resolved`, its private `resolved_literal`/`resolved_literal_optional` helpers, and a Linux-gated `from_resolved_tests` module (2 `#[tokio::test]` cases).

## Decisions Made

- **Byte-cap trip instead of a nonexistent command for the failure test.** The plan's suggested example ("force a spawn failure, e.g. a non-existent command") does not actually exercise `run_launcher`'s `Err` path: `caprun-exec-launcher` self-confines and then calls `Command::exec()` on the target; if that `execve` fails, the launcher prints an error and calls `std::process::exit(2)` — a normal process exit. From the broker's perspective `child.wait()` still returns `Ok(exit_status)` (run_launcher does not inspect the exit code), so this actually appends `process_exited`, not `process_spawn_failed`. Used the same genuine-failure technique the Allowed-path's own `process_exec_spawn.rs` integration tests already use (`/bin/yes` tripping the 10 MiB combined-output byte cap, fail-closed) to reliably force a real `run_launcher` `Err`. Documented inline in the test's doc comment.
- **`cfg(test)]`/`#[cfg(target_os = "linux")]` as two stacked attributes, not `cfg(all(...))`.** The plan's acceptance criteria greps for the literal substring `cfg(target_os = "linux")`; `cfg(all(test, target_os = "linux"))` doesn't contain that substring verbatim (comma-separated form), so the attributes are stacked (equivalent AND semantics, matches the literal grep, and mirrors the project's other Linux-gated-plus-cfg(test) module shapes).

## Deviations from Plan

None (Rule 1-3 auto-fixes only, no scope changes):

**1. [Rule 3 - Blocking issue] `mailpit-verify.sh` MAILPIT_VERIFY_CMD needed the leading `cargo build --workspace &&`**
- **Found during:** Task 2 verification
- **Issue:** Running the plan's literal verify command (`MAILPIT_VERIFY_CMD='cargo test -p brokerd invoke_process_exec_from_resolved' bash scripts/mailpit-verify.sh`) failed both new tests: `caprun-exec-launcher` sibling binary was not yet built in the container's `/tmp/lt` target dir (cargo-test-workspace-missing-sibling-binary, a documented project gotcha — CLAUDE.md explicitly calls out keeping `cargo build --workspace &&` in any custom `MAILPIT_VERIFY_CMD`).
- **Fix:** Re-ran with `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p brokerd invoke_process_exec_from_resolved'`.
- **Files modified:** none (verification-only).
- **Verification:** true_exit=0; both named tests passed (2 passed, 0 failed).
- **Committed in:** N/A (no code change).

---

**Total deviations:** 1 auto-fixed (Rule 3, verification-command correction only — no code/scope impact).
**Impact on plan:** None — verification-only correction, matches documented project convention.

## Issues Encountered

None beyond the deviation above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

`invoke_process_exec_from_resolved` is ready for 34-02 to wire the `"process.exec"` arm into `confirmation.rs`'s Step-7 dispatch (D-05), giving `caprun confirm` a callee for a blocked `process.exec`. The function's signature (`&rusqlite::Connection`, `resolved_args: &[ResolvedArg]`, `parent_id`/`parent_hash`) matches the shape `confirmation.rs` already uses for `invoke_file_write_from_resolved`/`invoke_file_create_from_resolved`, so the dispatch wiring should be a direct structural mirror. No blockers.

---
*Phase: 34-regression-live-proof-v1-7-done*
*Completed: 2026-07-18*
