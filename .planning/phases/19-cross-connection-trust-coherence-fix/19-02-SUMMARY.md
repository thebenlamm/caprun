---
phase: 19-cross-connection-trust-coherence-fix
plan: 02
subsystem: security
tags: [rust, tokio, brokerd, docker, colima, mailpit, live-verification, docs]

# Dependency graph
requires:
  - phase: 19-cross-connection-trust-coherence-fix (Plan 01)
    provides: "One-way occupancy latch in run_broker_server's accept loop + 3 fresh-broker regression test variants (guard-a control, overlapping, sequential-reconnect)"
provides:
  - "Independent live-Linux confirmation (Colima+Docker, real exit codes) that all 3 two_connection_intent_bypass variants pass and the full workspace suite has no regression"
  - "PROJECT.md DOC-02 disclosure finalized against the shipped fix"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Real exit code captured via $? immediately after mailpit-verify.sh, before any pipe/tail — never inferred from a piped exit 0"

key-files:
  created: []
  modified:
    - .planning/PROJECT.md

key-decisions:
  - "No source files touched — Task 1 is verification-only per the plan's own <files> spec; its evidence lives entirely in this SUMMARY, not in a code commit."
  - "PROJECT.md edits scoped strictly to the '⚠️ Superseded finding (v1.4 Phase 0)' block and the Active-section Phase 19 bullet, per the plan's surgical-edit instruction — no other PROJECT.md section touched."

requirements-completed: [TRUST-03, DOC-02]

coverage:
  - id: D1
    description: "Scoped live-Linux run: all 3 two_connection_intent_bypass.rs variants (guard-a control, overlapping, sequential-reconnect) pass green via scripts/mailpit-verify.sh on real Linux, real (pre-pipe) exit code 0."
    requirement: "TRUST-03"
    verification:
      - kind: integration
        ref: "MAILPIT_VERIFY_CMD='cargo test -p brokerd --test two_connection_intent_bypass -- --nocapture' bash scripts/mailpit-verify.sh -> test result: ok. 3 passed; 0 failed; 0 ignored; SCOPED_EXIT=0"
        status: pass
    human_judgment: false
  - id: D2
    description: "Full no-regression run: bash scripts/mailpit-verify.sh (cargo test --workspace --no-fail-fast) green on real Linux, real exit code 0, no regression from the v1.3 250/0/36 baseline."
    requirement: "TRUST-03"
    verification:
      - kind: integration
        ref: "bash scripts/mailpit-verify.sh -> 253 passed; 0 failed across 37 test binaries (0 errors/panics grepped); FULL_EXIT=0"
        status: pass
    human_judgment: false
  - id: D3
    description: "PROJECT.md's DOC-02 disclosure finalized: states the fix SHIPPED, names the one-way accept-time occupancy-latch mechanism, cites both regression variants + the live mailpit-verify.sh rerun as evidence, only PROJECT.md changed."
    requirement: "DOC-02"
    verification:
      - kind: other
        ref: "grep -niE 'shipped|one-way|occupancy|latch|accept-time' .planning/PROJECT.md; grep -n 'two_connection_intent_bypass|mailpit-verify' .planning/PROJECT.md; git diff --stat .planning/PROJECT.md (only file changed)"
        status: pass
    human_judgment: false

# Metrics
duration: 25min
completed: 2026-07-11
status: complete
---

# Phase 19 Plan 02: Live Linux Verification & DOC-02 Finalization Summary

**Independently re-ran `scripts/mailpit-verify.sh` on real Linux (Colima+Docker) — both the scoped `two_connection_intent_bypass` run (3/3 green) and the full workspace suite (253 passed / 0 failed / 37 binaries, no regression) — then finalized PROJECT.md's DOC-02 disclosure to state the cross-connection fix has SHIPPED, naming the one-way accept-time occupancy latch and citing this live evidence.**

## Performance

- **Duration:** ~25 min (dominated by two full Docker/cargo compile+test cycles)
- **Started:** 2026-07-11T05:58Z
- **Completed:** 2026-07-11T06:23Z
- **Tasks:** 2 completed
- **Files modified:** 1 (`.planning/PROJECT.md`)

## Accomplishments

- Confirmed Colima running, no stray containers, before starting (`colima status`, `docker ps -a`).
- **Step 1 (scoped run):** `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test two_connection_intent_bypass -- --nocapture' bash scripts/mailpit-verify.sh` run as a blocking foreground call. Captured real `$?` before any pipe: `SCOPED_EXIT=0`. Verbatim result line:
  ```
  running 3 tests
  test linux_tests::overlapping_connection_bypass_repro ... ok
  test linux_tests::guard_a_intra_connection_control ... ok
  test linux_tests::sequential_reconnect_bypass_repro ... ok

  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  ```
  All three rejected-2nd-connection error messages observed live: `"a connection is already established for this session; only one connection per session is permitted"` (overlapping and sequential variants), and the intra-connection guard-a control's own distinct message `"ProvideIntent rejected: must arrive exactly once, before any RequestFd (fail-closed)"`.
- **Step 2 (full workspace run):** `bash scripts/mailpit-verify.sh` (default `MAILPIT_VERIFY_CMD = cargo test --workspace --no-fail-fast`) run as a blocking foreground call. Captured real `$?` before any pipe: `FULL_EXIT=0`. Aggregated across all 37 `test result: ok` lines in the run: **253 passed, 0 failed** (0 `error[E...]`, 0 `FAILED`, 0 `panicked at` greps in the full log). This is `+3 passed / +1 binary` vs. v1.3's previously recorded 250 passed / 0 failed / 36 binaries — exactly the delta expected from un-ignoring the 3 `two_connection_intent_bypass` tests (1 new test binary, 3 new passing tests) with no other regression.
- Finalized `.planning/PROJECT.md`'s "⚠️ Superseded finding (v1.4 Phase 0)" block: now states the fix has SHIPPED (2026-07-11), names the mechanism ("a one-way, session-lifetime occupancy latch... added to `run_broker_server`'s accept loop"), states it "restores the `UserTrusted == human-typed` invariant across connections," and cites the concrete evidence (both regression variants green + the full live rerun counts) with pointers to both 19-01-SUMMARY.md and this SUMMARY.
- Updated the Active-section Phase 19 bullet from "(next)" (aspirational/pending framing) to "✓ ... SHIPPED (2026-07-11)", closing TRUST-01, TRUST-02, TRUST-03, and DOC-02.

## Task Commits

1. **Task 1: Run the live Linux acceptance and capture real pass/fail counts (TRUST-03)** - no commit (verification-only task per plan's own `<files>` spec: "no source files modified"). Evidence recorded in this SUMMARY.
2. **Task 2: Finalize PROJECT.md's DOC-02 disclosure against the shipped fix (DOC-02)** - `600e0aa` (docs)

**Plan metadata:** (this commit, appended after self-check)

## Files Created/Modified

- `.planning/PROJECT.md` - "⚠️ Superseded finding (v1.4 Phase 0)" block finalized to state the fix SHIPPED with the one-way accept-time occupancy-latch mechanism named and live-Linux test evidence cited; Active-section Phase 19 bullet flipped from "(next)" to "✓ ... SHIPPED"

## Decisions Made

- Task 1 intentionally produced no git commit — its `<files>` spec explicitly states "no source files modified — this task runs the verification harness and reports," so its evidence is recorded here in the SUMMARY rather than staged/committed.
- PROJECT.md edits were kept strictly surgical: only the superseded-finding `<details>` block and the single Active-section "Phase 19 (next)" bullet were touched, per the plan's explicit "do not rewrite unrelated PROJECT.md sections" instruction. The top-of-milestone "Goal"/"Why now" framing (lines 19-39) was deliberately left as-is (historical framing of why the fix was needed, not a currently-false "unshipped" claim).
- Did not touch ROADMAP.md or STATE.md, per the plan's own output instruction and the standing v1.4 mitigation (orchestrator owns phase-completion state).

## Deviations from Plan

None - plan executed exactly as written. Both mailpit-verify.sh invocations were run as blocking foreground Bash calls per the retry_note's explicit instruction (the harness auto-classified them as backgrounded internally, but this agent waited synchronously for each completion notification before proceeding — no work was left orphaned, and the real `$?` was captured in-script before any pipe in both cases).

## Issues Encountered

None. Both Linux runs (Colima+Docker) completed cleanly on the first attempt; no compile errors, no flaky tests, no regression from the v1.3 baseline.

## User Setup Required

None - no external service configuration required. Colima was already running at task start; Mailpit sidecar setup/teardown is fully automated by `scripts/mailpit-verify.sh` itself.

## Next Phase Readiness

- Phase 19 (both plans) is now fully proven: the fix is implemented (Plan 01), independently re-verified live on real Linux with real exit codes (this plan), and the documentation record (DOC-02) is finalized against the shipped reality.
- TRUST-01, TRUST-02, TRUST-03, and DOC-02 are all complete. Phase 19's own DONE gate (per `19-02-PLAN.md`'s `<verification>` section) is satisfied.
- Orchestrator owns updating STATE.md/ROADMAP.md/REQUIREMENTS.md checkboxes across the wave; this plan deliberately did not touch them.
- No blockers identified for Phase 20 (adversarial LLM planner, unblocked by this fix).

---
*Phase: 19-cross-connection-trust-coherence-fix*
*Completed: 2026-07-11*
