---
phase: 11-live-acceptance-tainted-session-human-gate
plan: 01
subsystem: testing
tags: [rust, cargo-test, integration-test, colima, docker, audit-dag, sqlite, landlock, seccomp]

# Dependency graph
requires:
  - phase: 09-tainted-session-demotion
    provides: mint_from_read (I1 demotion), session_demoted event, quarantine.rs chain-head semantics
  - phase: 10-confirm-deny-release
    provides: caprun confirm/deny CLI verbs, confirmation.rs release logic, PendingConfirmation persistence
  - phase: 07-file-create-sink
    provides: file.create sink, executor I2 block on ExternalUntrusted path args
provides:
  - Live, Linux-verified end-to-end proof that the v1.2 chain composes (I1 demotion -> I2 block -> human confirm/deny) for BOTH outcomes
  - Corrected s9_live_block.rs assertion (was stale relative to Phase 9's chain-head fix, never previously run on Linux)
  - A written D-06 acceptance record (this file) with actual Colima+Docker output and audit-DAG parent_id rows
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cross-process live-acceptance test: block-producing subprocess writes to a persistent SQLite audit DB, effect_id is discovered by reopening the DB (never stdout scraping), a second subprocess (confirm/deny) resumes against the same DB"
    - "openat2 RESOLVE_BENEATH create_exclusive_within does not create intermediate directories — test workspace roots exercising nested sink paths must pre-create parent dirs"

key-files:
  created:
    - cli/caprun/tests/live_acceptance_tainted_session.rs
  modified:
    - cli/caprun/tests/s9_live_block.rs

key-decisions:
  - "Reused HOSTILE_FC_CONTENT/HOSTILE_FC_PATH verbatim from s9_live_block.rs (D-01 double-duty read) rather than inventing new hostile content."
  - "New sibling test file rather than extending s9_live_block.rs in place (per D-03's explicit suggestion), keeping s9_live_block.rs scoped to its original ACC-04/05 purpose."
  - "Pre-create the reports/ subdirectory in the test's workspace root before the block run — create_exclusive_within's single-syscall openat2(RESOLVE_BENEATH) intentionally does not create intermediate directories (TOCTOU-safety by design), so a test that actually invokes the live sink on a nested path must supply that directory itself."

requirements-completed: [ACC-01, ACC-02, ACC-03]

coverage:
  - id: D1
    description: "Deny path: hostile file read -> I1 demotion -> I2 block -> caprun deny (exit 2) -> no effect ever proceeds, live on real Linux"
    requirement: "ACC-01"
    verification:
      - kind: integration
        ref: "docker run --rm --security-opt seccomp=unconfined -v \"$PWD\":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test live_acceptance_tainted_session live_acceptance_deny_path"
        status: pass
    human_judgment: false
  - id: D2
    description: "Confirm path: same hostile scenario -> caprun confirm (exit 0) -> the effect proceeds exactly once (reports/pwned.txt created), live on real Linux"
    requirement: "ACC-02"
    verification:
      - kind: integration
        ref: "docker run --rm --security-opt seccomp=unconfined -v \"$PWD\":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test live_acceptance_tainted_session live_acceptance_confirm_path"
        status: pass
    human_judgment: false
  - id: D3
    description: "Both deny and confirm runs prove one unbroken causal chain via verify_chain() plus the corrected parent_id walk (fd_granted -> file_read -> session_demoted -> sink_blocked -> confirm_denied/confirm_granted)"
    requirement: "ACC-03"
    verification:
      - kind: integration
        ref: "docker run --rm --security-opt seccomp=unconfined -v \"$PWD\":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test live_acceptance_tainted_session"
        status: pass
      - kind: integration
        ref: "docker run --rm --security-opt seccomp=unconfined -v \"$PWD\":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test s9_live_block (regression guard for the Task 2 fix)"
        status: pass
    human_judgment: false

# Metrics
duration: 25min
completed: 2026-07-07
status: complete
---

# Phase 11 Plan 01: Live Acceptance — Tainted Session, Human Gate Summary

**Live, Colima+Docker-verified proof that a real confined caprun worker's hostile file read demotes the session (I1), the same tainted value blocks file.create (I2), and a separate `caprun confirm`/`caprun deny` process either releases the effect exactly once or blocks it forever — with one unbroken audit-DAG causal chain for both outcomes.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-07T15:05:00Z (approx.)
- **Completed:** 2026-07-07T15:30:00Z
- **Tasks:** 3
- **Files modified:** 2 (1 new, 1 fixed)

## Accomplishments

- New Linux-gated integration test `cli/caprun/tests/live_acceptance_tainted_session.rs` with `live_acceptance_deny_path` (ACC-01), `live_acceptance_confirm_path` (ACC-02), and an always-compiled `live_acceptance_guard_binary_present` guard test.
- Fixed a real, previously-unexercised bug: `s9_live_block.rs`'s `blocked.parent_id == Some(file_read.id)` assertion was stale relative to Phase 9's `mint_from_read` chain-head fix (never run on Linux since Phase 9 landed because the body is `#[cfg(target_os = "linux")]`-gated). Corrected to the two-edge assertion `session_demoted.parent_id == file_read.id` (TAINT-04) and `sink_blocked.parent_id == session_demoted.id`.
- Ran the real Colima+Docker recipe (not a macOS compile check) and captured a full D-06 acceptance record below, including literal SQL `parent_id` rows for both the deny and confirm scenarios.

## Task Commits

Each task was committed atomically:

1. **Task 1: Write live_acceptance_tainted_session.rs (deny + confirm + guard, ACC-01/02/03)** - `02e9948` (feat)
2. **Task 2: Fix stale sink_blocked parent_id assertion in s9_live_block.rs (Pitfall 1 / ACC-03 regression guard)** - `8258e9c` (fix)
3. **Task 3: Run the live Colima+Docker acceptance recipe and capture the D-06 acceptance record** - `f6876ba` (fix — reports/ dir pre-create, discovered during the live run; this SUMMARY.md is the record itself)

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_tainted_session.rs` - New Linux-gated integration test: two `#[test]` functions (`live_acceptance_deny_path`, `live_acceptance_confirm_path`) driving the real `caprun` binary as two separate subprocesses against a shared persistent SQLite audit DB, plus a cross-platform guard test.
- `cli/caprun/tests/s9_live_block.rs` - Corrected the stale `sink_blocked.parent_id` assertion in `s9_live_file_create_hostile_block` to point at `session_demoted`, not `file_read`, matching current production wiring (`mint_from_read`'s chain-head advance, Phase 9).

## Decisions Made

- Reused `HOSTILE_FC_CONTENT`/`HOSTILE_FC_PATH` verbatim from `s9_live_block.rs` (D-01: the same hostile read triggers both I1 demotion and the I2-blocked value — no `--seed-from-file` needed).
- Wrote a new sibling test file rather than extending `s9_live_block.rs` in place (per D-03's explicit suggestion) — keeps `s9_live_block.rs` scoped to its original ACC-04/05 purpose.
- `effect_id` is discovered by reopening the persisted audit DB and reading `sink_blocked`'s `anchor.effect_id` — never by scraping stdout — mirroring `s9_live_block.rs`'s anchor-reading pattern and RESEARCH.md's explicit "Don't Hand-Roll" guidance.
- Pre-created the `reports/` subdirectory in the test's workspace root before the block run (see Deviations below) — this is a test-setup fix, not a production-code change; `create_exclusive_within`'s intentional TOCTOU-safe design does not create intermediate directories.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Pre-create `reports/` directory under the test's workspace root**
- **Found during:** Task 3 (running the live Colima+Docker recipe)
- **Issue:** The first live run of `live_acceptance_confirm_path` on real Linux failed: `caprun confirm` exited `3` (`ConfirmedButSinkFailed`) instead of the expected `0` (`Released`). Root cause: `adapter_fs::workspace::WorkspaceRoot::create_exclusive_within` resolves and creates the target file in a single `openat2` syscall with `RESOLVE_BENEATH` — by design (TOCTOU-safety, CWE-367) this does **not** create intermediate directories. `HOSTILE_FC_PATH = "reports/pwned.txt"` requires a `reports/` subdirectory that the test's temp workspace root never created. This was never caught before because every prior test exercising this hostile-path scenario (`s9_live_block.rs`'s hostile-block test, and Phase 11's own deny path) stops at the I2 block and never actually invokes the live sink — Phase 11's confirm path is the FIRST test to actually invoke `create_exclusive_within` on this nested path.
- **Fix:** Added `std::fs::create_dir_all(tmp.join("reports"))` to the shared `run_caprun_block` helper (used by both the deny and confirm tests), mirroring a realistic workspace that already has a `reports/` folder. This is a test-setup addition only — no production code was touched, consistent with the plan's threat model and D-02 ("reuse the existing hardened sink").
- **Files modified:** `cli/caprun/tests/live_acceptance_tainted_session.rs`
- **Verification:** Re-ran the Colima+Docker recipe; `live_acceptance_confirm_path` now exits `0` and `reports/pwned.txt` exists under the workspace root with `sink_executed` recorded in the audit DAG.
- **Committed in:** `f6876ba`

---

**Total deviations:** 1 auto-fixed (Rule 1 — test-setup bug, no production code changed)
**Impact on plan:** Necessary to make the confirm path's live sink invocation reachable at all; no scope creep — the fix is scoped entirely to test setup and does not touch `crates/adapter-fs` or any TCB code.

## Issues Encountered

None beyond the deviation above — the deny path, the chain assertions, and the s9_live_block.rs fix all passed on the first live Colima+Docker run.

## D-06 Acceptance Record — Colima+Docker Live Run

**Environment:** Colima 0.10.3 (VM driver `vz`), Docker 29.6.1 client against the Colima daemon, `rust:1` image, `--security-opt seccomp=unconfined` (required — the default seccomp profile blocks `landlock()`/`seccomp()` syscalls), no `--privileged`.

### Run 1 — `live_acceptance_tainted_session` (both new tests)

**Command:**
```
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 cargo test -p caprun --test live_acceptance_tainted_session -- --nocapture
```

**Result:**
```
running 3 tests
test live_acceptance_deny_path ... ok
test live_acceptance_guard_binary_present ... ok
test live_acceptance_confirm_path ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```
(First attempt, before the Rule-1 fix above, showed `live_acceptance_confirm_path` failing: `assertion left == right failed: confirm on a Pending block must exit 0 (Released), left: Some(3), right: Some(0)`. The rerun above is post-fix, all green.)

### Run 2 — `s9_live_block` (Task 2 regression guard, first-ever Linux run since Phase 9)

**Command:**
```
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 cargo test -p caprun --test s9_live_block -- --nocapture
```

**Result:**
```
running 4 tests
test s9_live_block_guard_binary_present ... ok
test s9_live_file_create_clean_allow ... ok
test s9_live_clean_allow_path ... ok
test s9_live_file_create_hostile_block ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```
This is the first real-Linux run of `s9_live_file_create_hostile_block` since the Task 2 fix — it confirms the corrected `blocked.parent_id == Some(session_demoted.id)` assertion holds against current production code (the pre-fix assertion `blocked.parent_id == Some(file_read.id)` would have panicked here).

### Audit-DAG `parent_id` rows — deny scenario

Queried directly from the persisted SQLite audit DB after both subprocesses completed (`SELECT id, event_type, parent_id FROM events ORDER BY rowid`; ids truncated to 8 hex chars for readability):

| id | event_type | parent_id |
|---|---|---|
| `487c6170` | session_created | (none) |
| `a2913e2d` | intent_received | `487c6170` |
| `5b590508` | fd_granted | `a2913e2d` |
| `b81451cd` | file_read | `5b590508` |
| `3376582d` | session_demoted | `b81451cd` |
| `81cf5232` | sink_blocked | `3376582d` |
| `2ab0b045` | confirm_denied | `81cf5232` |

Process 1 (`create-file-from-report`) exited non-zero (I2 block); process 2 (`caprun deny <effect_id> <db>`) exited `2` (Denied). Neither `reports/pwned.txt` nor `intended_output.txt` exists on disk after both processes complete. `verify_chain()` returned `true`.

### Audit-DAG `parent_id` rows — confirm scenario

| id | event_type | parent_id |
|---|---|---|
| `a620116d` | session_created | (none) |
| `047b9312` | intent_received | `a620116d` |
| `aeb8515c` | fd_granted | `047b9312` |
| `99a50e3b` | file_read | `aeb8515c` |
| `28c347b8` | session_demoted | `99a50e3b` |
| `c706d890` | sink_blocked | `28c347b8` |
| `83023463` | confirm_granted | `c706d890` |
| `65302770` | sink_executed | `83023463` |

Process 1 (`create-file-from-report`) exited non-zero (I2 block); process 2 (`caprun confirm <effect_id> <db>`) exited `0` (Released). `reports/pwned.txt` exists on disk under the workspace root after process 2 completes (created exactly once — the `sink_executed` row is the only file.create effect in the chain). `verify_chain()` returned `true`.

**Unbroken causal chain, both scenarios (ACC-03):** `fd_granted -> file_read -> session_demoted -> sink_blocked -> confirm_denied` (deny) / `confirm_granted` (confirm), each edge a direct `parent_id` link, `verify_chain()` true in both cases — matching RESEARCH.md's predicted single linear chain exactly, with no fork.

## User Setup Required

None — no external service configuration required. Colima/Docker are already-provisioned local dev tooling per this project's `CLAUDE.md`.

## Next Phase Readiness

- ACC-01/02/03 are proven live on real Linux — the v1.2 DONE gate criteria (per the phase objective) are satisfied.
- `s9_live_block.rs`'s previously-stale, never-Linux-run assertion is now correct and verified; future Linux CI runs of this test will not silently mask regressions.
- No blockers for milestone close-out. The only residual note: this was the first time `create_exclusive_within` was exercised on a nested (`dir/file`) path in a live test — future tests using multi-segment sink paths should pre-create parent directories in their workspace-root setup, as this plan's `run_caprun_block` now does.

---
*Phase: 11-live-acceptance-tainted-session-human-gate*
*Completed: 2026-07-07*
