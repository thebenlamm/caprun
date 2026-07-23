---
phase: 34-regression-live-proof-v1-7-done
plan: 04
subsystem: testing
tags: [rust, tokio, live-proof, process-exec, file-write, confirm-release, audit-dag, landlock, seccomp, mailpit]

requires:
  - phase: 34-01
    provides: invoke_process_exec_from_resolved
  - phase: 34-02
    provides: async confirm() + process.exec Step-4.75 guard + Step-7 dispatch (EXEC-05)
  - phase: 34-03
    provides: "Gate A (Linux compile-check) + Gate B (fresh Fable-5 adversarial trace) both green — LIVE-01 authorized"

provides:
  - "cli/caprun/tests/live_acceptance_v1_7_composed.rs — a shared-audit.db, four-leg composed live-proof test (cfg linux)"
  - "LIVE-01: a recorded real-Linux run of the composed proof (true-exit-0, named tests + counts)"
  - "LIVE-02: a recorded real-Linux full-workspace regression result (true-exit-0, named tests + counts) with no regression to v1.0-v1.6"

affects: []

tech-stack:
  added: []
  patterns:
    - "shared-audit.db composed multi-leg live proof (mirrors live_acceptance_v1_3.rs/live_acceptance_v1_4_composed.rs): one persisted audit.db across N legs, each leg its own session_id, per-session verify_chain independently true"
    - "persist_known_session — a thin session::persist_session wrapper accepting a caller-chosen session_id, needed because in-process brokerd calls (unlike a full caprun CLI run) never insert a sessions-table row on their own"

key-files:
  created:
    - cli/caprun/tests/live_acceptance_v1_7_composed.rs
  modified: []

key-decisions:
  - "Legs (a)/(b)/(c) drive brokerd's production functions directly, in-process, against the shared persisted audit.db (executor::submit_plan_node, brokerd::quarantine::mint_from_exec, brokerd::sinks::process_exec::invoke_process_exec, brokerd::sinks::file_write::invoke_file_write) — mirrors s9_process_exec_block.rs's own scoping note that there is no process.exec/file.write intent kind on the intent-first caprun CLI yet. Leg (d) is the one leg that must be a REAL caprun confirm subprocess (the confirm-release path is inherently cross-process — a human runs it later against a persisted DB)."
  - "A single MAC key is minted once via seed_test_key (idempotent read-existing-first, mirrors cli/caprun/src/key.rs's custody discipline) BEFORE any leg runs, so every in-process append_event/verify_chain call and the leg (d) caprun confirm subprocess (which independently reads the same <audit_db>.key file) MAC against the identical key."
  - "Rule 1 fix (see Deviations): added persist_known_session — none of the in-process brokerd calls this file drives (session::create_session, append_event, mint_from_read/exec) insert a sessions-table row for a caller-chosen id; only the full caprun CLI flow does. Without it the end-of-run all_session_ids sweep found 0 rows instead of 4."

requirements-completed: [LIVE-01, LIVE-02]

coverage:
  - id: D1
    description: "live_acceptance_v1_7_composed.rs exists, is #[cfg(target_os = \"linux\")]-gated, opens ONE shared persisted audit.db, and drives four legs: (a) genuine non-stapled exec-output taint -> I2 Block, (b) clean exec Allow, (c) file.write within WorkspaceRoot Allow (durably audited), (d) EXEC-05 confirm-release via a real caprun confirm subprocess"
    requirement: "LIVE-01"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/live_acceptance_v1_7_composed.rs#linux::live_acceptance_v1_7_composed_four_legs"
        status: pass
    human_judgment: false
  - id: D2
    description: "LIVE-01 scoped run on real Linux via mailpit-verify.sh: true_exit=0 captured before any pipe, composed test named and passing, each leg's session verify_chain independently true"
    requirement: "LIVE-01"
    verification:
      - kind: other
        ref: "bash scripts/mailpit-verify.sh (MAILPIT_VERIFY_CMD scoped to --test live_acceptance_v1_7_composed) — /tmp/34-04-live01.log, true_exit=0, 2 passed / 0 failed"
        status: pass
    human_judgment: false
  - id: D3
    description: "LIVE-02: full-workspace regression on real Linux, unscoped mailpit-verify.sh, true_exit=0 captured before any pipe, counts + named tests asserted, per-new-sink negative tests (s9_process_exec_block, s9_file_write_block) present and passing, no v1.0-v1.6 regression"
    requirement: "LIVE-02"
    verification:
      - kind: other
        ref: "bash scripts/mailpit-verify.sh (unscoped) — /tmp/34-04-live02.log, true_exit=0, 390 passed / 0 failed across 55 test binaries"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-07-18
status: complete
---

# Phase 34 Plan 04: LIVE-01/LIVE-02 — v1.7 composed live proof + full-workspace regression Summary

**Authored a shared-audit.db, four-leg composed live-proof test (`live_acceptance_v1_7_composed.rs`) proving the tainted-exec I2 Block, a clean exec Allow, an in-WorkspaceRoot `file.write`, and the EXEC-05 confirm-release path all in one run — LIVE-01 passed true-exit-0 on real Linux, and LIVE-02's full-workspace regression is green (390/0) with no regression to v1.0-v1.6.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-18T03:19:00Z
- **Tasks:** 3/3 completed
- **Files modified:** 1 (`cli/caprun/tests/live_acceptance_v1_7_composed.rs`)

## Accomplishments

- `cli/caprun/tests/live_acceptance_v1_7_composed.rs` exists, `#[cfg(target_os = "linux")]`-gated, opens ONE shared persisted `audit.db` (never `:memory:`, never a fresh per-leg path), and drives four legs against it:
  - **Leg (a) BLOCK** — a trusted `process.exec` Allows; `invoke_process_exec` spawns the real, kernel-confined `caprun-exec-launcher` and appends `process_exited`; `mint_from_exec` mints the captured output rooted on THAT SAME event id (`provenance_chain == [exec_event_id]`, DB-verified before minting — anti-stapling). Routing that handle into a SECOND `process.exec`'s `command` arg deterministically Blocks (`BlockedPendingConfirmation`, I2); the block anchor's `provenance_chain[0]`/`read_event_id` equal the real `process_exited` event id (genuine, non-stapled). `verify_chain` true.
  - **Leg (b) ALLOW** — a clean `process.exec` Allows; its unconditionally-tainted output, never routed anywhere, causes no Block. `verify_chain` true.
  - **Leg (c) FS WRITE** — a trusted `path`/`contents` pair Allows a `file.write` that overwrites a pre-existing file within `WorkspaceRoot` (real `write_within` `openat2` syscall) and is durably audited (`sink_executed`). `verify_chain` true.
  - **Leg (d) EXEC-05 RELEASE** — a Blocked `process.exec` (seeded directly, mirrors `s9_process_exec_block.rs`'s own confirm-release leg) is released via a REAL `caprun confirm` subprocess: runs exactly once (marker file created, exactly one `process_exited` row), chained onto the real `confirm_granted` head (never a fabricated root), `verify_chain` true; a second confirm on the same `effect_id` returns exit 5 (`AlreadyTerminal`), no double-spawn.
  - End-of-run sweep: opens the shared `audit.db` once, enumerates all four sessions (`all_session_ids`, `ORDER BY rowid`, never `LIMIT 1`), asserts `verify_chain` true for every one independently.
- **LIVE-01** (D-12/D-13): scoped run via `scripts/mailpit-verify.sh` (`MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_7_composed'`), true exit code captured BEFORE any pipe. **`true_exit=0`.** Log (`/tmp/34-04-live01.log`): `test live_acceptance_v1_7_composed_guard_binary_present ... ok`, `test linux::live_acceptance_v1_7_composed_four_legs ... ok`, `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
- **LIVE-02** (D-14): unscoped run via `scripts/mailpit-verify.sh` (default, Mailpit sidecar up, `cargo test --workspace --no-fail-fast`), true exit code captured BEFORE any pipe. **`true_exit=0`.** Log (`/tmp/34-04-live02.log`): **390 passed; 0 failed** summed across all 55 reported `test result: ok` lines from every test binary in the workspace. Named per-new-sink negative tests present and passing: `s9_process_exec_block` — `test result: ok. 6 passed; 0 failed` (includes the EXEC-05 confirm-release + entry-guard-fail-closed legs from 34-02); `s9_file_write_block` — `test result: ok. 3 passed; 0 failed`. Every prior v1.0-v1.6 composed/live suite also passed in the same run: `live_acceptance_v1_3` (2/2), `live_acceptance_v1_4_composed` (2/2), `live_acceptance_tainted_session` (3/3), `s9_live_block` (7/7), `s9_acceptance` (5/5), `confirm` (4/4), `e2e` (2/2), `harden04_featureless_create_session` (1/1) — no v1.0-v1.6 regression. `live_acceptance_v1_7_composed` itself: 2/2. Only pre-existing, unrelated warnings appear in the log (`sandbox`'s unused `RulesetCreatedAttr` import, dead-code warnings in `planner.rs`'s never-constructed `LlmPlanner` type) — no new errors.

## Task Commits

1. **Task 1: Author live_acceptance_v1_7_composed.rs** - `61b9ead` (test)
2. **Task 1 fix (Rule 1) + Task 2: LIVE-01 run** - `1e489d6` (fix)
3. **Task 3: LIVE-02 run** - no code change; verification-only (see below)

_No separate plan-metadata commit — worktree mode; the orchestrator handles STATE.md/ROADMAP.md centrally after merge._

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_v1_7_composed.rs` — the composed four-leg live-proof test (created in `61b9ead`; the `persist_known_session` fix landed in `1e489d6`).

## Decisions Made

See `key-decisions` in frontmatter — summarized: (1) legs (a)/(b)/(c) drive brokerd production functions directly, in-process, against the shared persisted `audit.db` since there is no `process.exec`/`file.write` intent kind on the CLI yet; leg (d) is the one leg that must be a real `caprun confirm` subprocess since confirm-release is inherently cross-process; (2) a single MAC key is minted once via `seed_test_key` before any leg runs, threaded through every in-process call AND the leg (d) subprocess (which reads the same `.key` file); (3) `persist_known_session` was added as a Rule-1 auto-fix (see Deviations) since none of the driven functions insert a `sessions` row for a caller-chosen id.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `all_session_ids` end-of-run sweep found 0 sessions instead of 4**
- **Found during:** Task 2 (first LIVE-01 run)
- **Issue:** Legs (a)-(d) drive `brokerd` functions directly (`session::create_session`, `append_event`, `mint_from_read`/`mint_from_exec`, `seed_pending_process_exec_block`) against a shared persisted `audit.db`, but none of these insert a `sessions` table row for a caller-chosen `session_id` — only the full `caprun` CLI flow (via `session::create_session` + `session::persist_session`) does that end-to-end. The plan's own end-of-run sweep pattern (mirrored from `live_acceptance_v1_4_composed.rs`, whose legs ARE real `caprun` CLI subprocess invocations) asserts `all_session_ids(&conn).len() == 4`; the first run failed with `left: 0, right: 4`.
- **Fix:** Added `persist_known_session` — a thin wrapper around `session::persist_session` that builds a `runtime_core::Session` with the test's own pre-chosen `session_id` (rather than `session::create_session`'s self-minted fresh id) — and called it at the start of every leg (inside the locked/unlocked connection scope, before the leg's first `append_event`), including inside `seed_pending_process_exec_block` for leg (d).
- **Files modified:** `cli/caprun/tests/live_acceptance_v1_7_composed.rs`.
- **Verification:** Re-ran LIVE-01: `true_exit=0`, `linux::live_acceptance_v1_7_composed_four_legs ... ok` (2/2 total), end-of-run sweep found exactly 4 sessions, all four `verify_chain` true.
- **Committed in:** `1e489d6`.

---

**Total deviations:** 1 auto-fixed (Rule 1, structural bug in the test harness's own session bookkeeping — no TCB source touched, matches the plan's explicit prohibition on adding TCB source).
**Impact on plan:** None on the plan's deliverable shape; the fix was required for the composed test's own end-of-run sweep assertion to be meaningful.

## Issues Encountered

None beyond the deviation above.

## User Setup Required

None — no external service configuration required. Colima was already running; `scripts/mailpit-verify.sh` handled the Mailpit sidecar lifecycle automatically for both LIVE-01 and LIVE-02.

## Closeout (D-17, flagged for the operator — NOT an executor action)

LIVE-01 and LIVE-02 are both green. Per the plan's explicit scope: **a human DONE sign-off precedes marking the v1.7 milestone complete, and the milestone is NOT pushed to `origin` unless the operator explicitly requests it.** This executor took no closeout action beyond recording the LIVE-01/LIVE-02 results above — milestone-closing (archiving, tagging, pushing) is an orchestrator/operator step outside this plan's scope.

## Next Phase Readiness

Phase 34 (34-01 through 34-04) is now fully executed: EXEC-05 confirm-release TCB slice (34-01/34-02), release gates (34-03, both green), and the composed live proof + full-workspace regression (34-04, this plan, both LIVE-01 and LIVE-02 green). No blockers. Ready for the operator's D-17 human sign-off and v1.7 milestone closeout (`/gsd-complete-milestone` or equivalent), per the standing project convention that the orchestrator/operator — never an executor — makes the final closeout/push decision.

---
*Phase: 34-regression-live-proof-v1-7-done*
*Completed: 2026-07-18*

## Self-Check: PASSED

- `cli/caprun/tests/live_acceptance_v1_7_composed.rs` confirmed present on disk. ✓
- `.planning/phases/34-regression-live-proof-v1-7-done/34-04-SUMMARY.md` confirmed present on disk. ✓
- Both task commit hashes (`61b9ead`, `1e489d6`) confirmed present in `git log --oneline --all`. ✓
