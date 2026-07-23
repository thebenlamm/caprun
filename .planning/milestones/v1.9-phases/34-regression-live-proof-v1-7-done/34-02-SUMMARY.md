---
phase: 34-regression-live-proof-v1-7-done
plan: 02
subsystem: brokerd
tags: [rust, tokio, confirm-release, process-exec, i2, audit-dag, landlock, seccomp]

requires:
  - phase: 34-01
    provides: invoke_process_exec_from_resolved (async, plain &rusqlite::Connection) — the confirm-time release twin this plan's Step-7 arm dispatches to

provides:
  - "confirmation::confirm() is async; a \"process.exec\" Step-4.75 entry-guard arm + Step-7 dispatch arm (both added in the same diff) release a Blocked process.exec via caprun confirm"
  - "The released output is taint-minted via the sanctioned quarantine::mint_from_exec at an inline-annotated confirmation.rs call site (Gate 3's two-file allow-list array stays byte-identical)"
  - "run_confirm_or_deny is async, threaded to its #[tokio::main] call site in main() with no runtime nesting"
  - "cfg(linux) EXEC-05 acceptance: confirm-release (exactly-once, verify_chain true) + entry-guard fail-closed (D-07) legs, both cross-process via a real caprun confirm subprocess"

affects: [34-03, 34-04]

tech-stack:
  added: []
  patterns:
    - "async confirm() with per-arm async: only the process.exec Step-7 arm awaits; file.create/email.send/file.write stay sync internally inside the now-async fn"
    - "throwaway executor::value_store::ValueStore at a confirm-time mint call site — mint_from_exec RUNS (producing a genuinely-rooted ValueId) without needing to persist output_value_id beyond the audit Event chain, since no live worker exists at confirm time"

key-files:
  created: []
  modified:
    - crates/brokerd/src/confirmation.rs
    - cli/caprun/src/main.rs
    - cli/caprun/tests/s9_process_exec_block.rs
    - crates/brokerd/tests/email_smtp_acceptance.rs

key-decisions:
  - "Rule 3 fix: making confirm() async broke every existing synchronous caller. Converted all 14 confirmation.rs unit tests that call confirm() to #[tokio::test] async fn (mechanical .await insertion, verified line-by-line), plus email_smtp_acceptance.rs's seed_and_confirm_email_send + its two #[cfg(target_os = \"linux\")] callers — none of these were in the plan's explicit file list, but all were unavoidable compile-blockers of Task 1's own required change."
  - "Rule 1 fix: confirm_on_undispatchable_sink_does_not_burn_confirmation used \"process.exec\" as its un-dispatchable-sink example (correct through Phase 33). Since this plan wires process.exec's dispatch, swapped the example sink to a fictitious \"sink.never-wired\" name so the test keeps exercising the guard mechanism itself rather than silently becoming a false test of a now-dispatchable sink."
  - "Task 3's confirm-release test uses \"/usr/bin/touch <marker>\" (never sh -c — matches caprun-exec-launcher's argv discipline) as the released command: a regular-file create is exactly what the exec-child Landlock ruleset grants inside the workspace (ReadFile+WriteFile+MakeReg — no MakeDir, so mkdir would fail closed)."
  - "\"Exactly once\" for the release is proven two ways: the marker file exists (side effect), and the audit DB shows precisely one process_exited event for the session before AND after a second confirm — the DB count is the authoritative proof since a second physical invocation is impossible once the guard/CAS refuses at AlreadyTerminal."

requirements-completed: [EXEC-05]

coverage:
  - id: D1
    description: "confirmation::confirm() is async; \"process.exec\" is added to both the Step-4.75 entry-guard allow-list and the Step-7 dispatch match in the same diff, dispatching to invoke_process_exec_from_resolved and minting the released output via mint_from_exec at an inline-annotated site"
    requirement: "EXEC-05"
    verification:
      - kind: unit
        ref: "cargo build -p brokerd (source-level greps: pub async fn confirm, process.exec guard+dispatch arms, single mint_from_exec call with planner-discipline-allow marker)"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh — Gate 1 + Gate 3 both PASS; git diff scripts/check-invariants.sh empty"
        status: pass
    human_judgment: false
  - id: D2
    description: "run_confirm_or_deny is async and awaited at its call site in main() (#[tokio::main]); no runtime nesting introduced"
    requirement: "EXEC-05"
    verification:
      - kind: unit
        ref: "cargo build --workspace (source-level greps: async fn run_confirm_or_deny, .await call sites, zero new Runtime::new/block_on)"
        status: pass
    human_judgment: false
  - id: D3
    description: "cfg(linux) confirm-release acceptance: a Blocked process.exec is released by a real caprun confirm subprocess, runs exactly once, chains process_exited onto confirm_granted, verify_chain true; a second confirm returns AlreadyTerminal with no double-spawn"
    requirement: "EXEC-05"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/s9_process_exec_block.rs#linux::s9_process_exec_confirm_release_runs_once_and_second_confirm_is_terminal"
        status: pass
    human_judgment: false
  - id: D4
    description: "cfg(linux) entry-guard fail-closed leg: a still-un-dispatchable sink is refused before any state transition, row remains Pending (D-07, guard mechanism proven not to have regressed OPEN)"
    requirement: "EXEC-05"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/s9_process_exec_block.rs#linux::s9_process_exec_confirm_on_still_undispatchable_sink_refuses_and_stays_pending"
        status: pass
    human_judgment: false

duration: ~35min
completed: 2026-07-18
status: complete
---

# Phase 34 Plan 02: process.exec confirm-release dispatch wiring (EXEC-05) Summary

**`caprun confirm` now releases a Blocked `process.exec` at parity with file.create/file.write/email.send — async `confirm()`, a synchronized Step-4.75 guard + Step-7 dispatch arm, a sanctioned inline-annotated `mint_from_exec` call site, and a real cross-process Linux acceptance test proving exactly-once release with an unbroken `verify_chain`-true audit chain.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-18T02:37:44Z
- **Tasks:** 3/3 completed
- **Files modified:** 4 (`crates/brokerd/src/confirmation.rs`, `cli/caprun/src/main.rs`, `cli/caprun/tests/s9_process_exec_block.rs`, `crates/brokerd/tests/email_smtp_acceptance.rs`)

## Accomplishments

- `confirmation::confirm()` is `pub async fn`; `"process.exec"` was added to BOTH the Step-4.75 entry-guard allow-list and the Step-7 dispatch match in the same diff (the exact P33 MAJOR-1 drift class this plan closes for EXEC-05). Only the new `"process.exec"` arm awaits — `file.create`/`email.send`/`file.write` stay sync internally.
- The Step-7 `"process.exec"` arm awaits `invoke_process_exec_from_resolved` (34-01's callee) then mints the released output via `quarantine::mint_from_exec`, rooted on the REAL `process_exited` Event id the sink just appended (non-stapled, D-03) — authorized by an inline `planner-discipline-allow: mint_from_exec` marker on the same source line as the call, since `confirmation.rs` is not in Gate 3's two-file allow-list array (that array stays byte-identical: `git diff scripts/check-invariants.sh` is empty).
- `run_confirm_or_deny` is `async fn`, `.await`s the now-async `confirm()`, and its single call site in `main()` (already `#[tokio::main]`) is `.await`ed — no new runtime, no `block_on`, no runtime nesting.
- Two new `#[cfg(target_os = "linux")]` `#[tokio::test]`s in `s9_process_exec_block.rs` prove EXEC-05 end-to-end via a REAL `caprun confirm` subprocess: the confirm-release leg (exactly-once execution, `process_exited` chained onto `confirm_granted`, `verify_chain` true, second confirm exits 5/AlreadyTerminal with no double-spawn) and the entry-guard fail-closed leg (a still-un-dispatchable sink refuses at exit 1, row stays Pending, D-07).
- Rule-1/Rule-3 fixes: converted 14 pre-existing `confirmation.rs` unit tests and 2 `email_smtp_acceptance.rs` tests to `#[tokio::test]` (mechanical fallout of making `confirm()` async), and swapped the entry-guard unit test's fictitious sink from `"process.exec"` (no longer un-dispatchable) to `"sink.never-wired"`.
- Verified on real Linux via `bash scripts/mailpit-verify.sh`: scoped run (`s9_process_exec_block`) 6/6 passed, true_exit=0; full unscoped workspace run 386/386 passed, 0 failed, true_exit=0 — confirms none of the async-plumbing fallout regressed any other suite.

## Task Commits

1. **Task 1: Async confirm() + process.exec guard arm + Step-7 dispatch + mint** - `38c5c5f` (feat)
2. **Task 2: Thread async through run_confirm_or_deny to its main() call site** - `7e5639c` (feat)
3. **Task 3: cfg(linux) EXEC-05 acceptance test — confirm-release + entry-guard fail-closed legs (D-11)** - `842ee12` (test)

_No separate plan-metadata commit — worktree mode; the orchestrator handles STATE.md/ROADMAP.md centrally after merge._

## Files Created/Modified

- `crates/brokerd/src/confirmation.rs` — `confirm()` is now `pub async fn`; `"process.exec"` guard+dispatch arms added; sanctioned inline-annotated `mint_from_exec` call site; 14 unit tests converted to `#[tokio::test]`; `confirm_on_undispatchable_sink_does_not_burn_confirmation` re-targeted to a fictitious never-wired sink.
- `cli/caprun/src/main.rs` — `run_confirm_or_deny` is `async fn`; its `confirm(...)` call and its own call site in `main()` are both `.await`ed.
- `cli/caprun/tests/s9_process_exec_block.rs` — two new Linux-gated acceptance tests (confirm-release + entry-guard fail-closed) plus their local seeding/subprocess helpers (mirroring `tests/confirm.rs`'s established pattern, duplicated here since integration-test binaries cannot import across `tests/*.rs` files).
- `crates/brokerd/tests/email_smtp_acceptance.rs` — `seed_and_confirm_email_send` and its two `#[cfg(target_os = "linux")]` callers converted to async/`#[tokio::test]` (Rule 3 fallout of confirm() becoming async).

## Decisions Made

See `key-decisions` in frontmatter — summarized: (1) the async-conversion cascade across 16 pre-existing tests was an unavoidable Rule-3 fix, not scope creep; (2) the entry-guard unit test's example sink was swapped to a fictitious name (Rule 1) since `process.exec` stopped being a valid "un-dispatchable" example; (3) `/usr/bin/touch` was chosen as the release-leg's target command because it's the one Landlock-permitted regular-file-create primitive with no shell involved; (4) "exactly once" is proven via the audit-DB `process_exited` row count, which is authoritative regardless of command idempotency.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] `confirm()` becoming async broke 16 pre-existing synchronous test callers**
- **Found during:** Task 1 build verification (`cargo build -p brokerd --tests` / Linux `cargo build --workspace --tests --keep-going`)
- **Issue:** Task 1's required change (`pub fn confirm` → `pub async fn confirm`) is a breaking signature change for every existing caller. 14 unit tests inside `confirmation.rs` and `email_smtp_acceptance.rs`'s `seed_and_confirm_email_send` (plus its two Linux-gated callers) all called `confirm(...)` synchronously.
- **Fix:** Converted all 16 to `#[tokio::test]` `async fn` and added `.await` at each `confirm(...)` call site. Used a scripted line-based transform for the 14 confirmation.rs tests (verified unique-pattern match before each replacement) to avoid partial/incorrect edits at scale.
- **Files modified:** `crates/brokerd/src/confirmation.rs`, `crates/brokerd/tests/email_smtp_acceptance.rs`.
- **Verification:** `cargo build -p brokerd --tests` clean on macOS; Linux `cargo build --workspace --tests --keep-going` showed zero errors in either file (only main.rs's pre-Task-2 error remained, as expected); full `mailpit-verify.sh` run confirmed all 16 converted tests pass (386/386 total, 0 failed).
- **Committed in:** `38c5c5f` (Task 1 commit).

**2. [Rule 1 - Bug] `confirm_on_undispatchable_sink_does_not_burn_confirmation` used `"process.exec"` as its un-dispatchable-sink fixture**
- **Found during:** Task 1, reviewing existing guard-related tests after adding `"process.exec"` to the Step-4.75 allow-list.
- **Issue:** This pre-existing unit test asserted that `"process.exec"` was refused by the guard — true through Phase 33, but now FALSE (this plan wires it). Left unchanged, the test would either fail to compile after the async conversion or, once fixed to compile, silently assert the wrong thing (or attempt a real spawn on macOS/CI, since the guard no longer refuses it).
- **Fix:** Swapped both `SinkId("process.exec".into())` occurrences in the test to `SinkId("sink.never-wired".into())` (a fictitious, deliberately-never-wired sink name) and updated the doc comment to explain the swap. The test still exercises the exact same guard mechanism (D-07) — it just no longer picks an example that this plan just made dispatchable.
- **Files modified:** `crates/brokerd/src/confirmation.rs`.
- **Verification:** `confirm_on_undispatchable_sink_does_not_burn_confirmation` passes (confirmed in the full 386/386 Linux run); row stays Pending, no `confirm_granted` appended.
- **Committed in:** `38c5c5f` (Task 1 commit).

---

**Total deviations:** 2 auto-fixed (1 Rule 3 blocking-compile cascade across 16 tests, 1 Rule 1 stale-fixture bug). Both were direct, unavoidable consequences of Task 1's own required change — no scope creep beyond what the plan's own async-confirm requirement necessitated.
**Impact on plan:** None on the plan's actual deliverable; both fixes were required for the codebase to compile and for its existing test suite to keep testing what it claims to test.

## Issues Encountered

None beyond the deviations above. The mandatory `cfg-linux-test-blindness` guardrail (CLAUDE.md / project memory) was followed proactively: ran `cargo build --workspace --tests --keep-going` in a Linux container after Task 1 (before Task 2 existed) to confirm the ONLY remaining Linux compile errors were `main.rs`'s pre-Task-2 scope, rather than discovering additional Linux-only breakage later.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

EXEC-05 is closed: `process.exec` now has full confirm-release parity with the other three sinks, proven live on Linux with a genuine, non-stapled, `verify_chain`-true audit chain. Remaining Phase 34 scope (LIVE-01/LIVE-02, plans 34-03/34-04) is the composed live-proof close and is unblocked by this plan — no follow-up items opened.

---
*Phase: 34-regression-live-proof-v1-7-done*
*Completed: 2026-07-18*

## Self-Check: PASSED

All 4 modified files confirmed present on disk; all 4 task/summary commit hashes (`38c5c5f`, `7e5639c`, `842ee12`, `0bbc0a1`) confirmed present in `git log --oneline --all`.
