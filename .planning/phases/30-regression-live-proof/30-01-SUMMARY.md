---
phase: 30-regression-live-proof
plan: 01
subsystem: testing
tags: [bash, cargo, mailpit, false-assurance-guard, regression-audit, harden04]

# Dependency graph
requires:
  - phase: 29-sink-path-hardening-replay-cas-contents-slot
    provides: HARDEN-03 replay CAS + HARDEN-05 contents slot; the harden04_featureless_create_session.rs test (authored in Phase 27) whose invocation gap this plan closes
provides:
  - "scripts/verify-harden04-featureless.sh — standalone false-assurance-guarded proof script for HARDEN-06 criterion 4"
  - "30-REGRESSION-AUDIT.md — independent sweep confirming no weakened/ignored hardening test exists across Phase 27/28/29"
affects: [30-02 (live-proof plan that runs this script on real Linux)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Standalone verification-tooling scripts delegate to a shared recipe (scripts/mailpit-verify.sh) via a scoped MAILPIT_VERIFY_CMD override rather than editing the shared script"
    - "False-assurance guard: detect a test's own self-skip sentinel in captured output and fail the wrapper non-zero, so a green wrapper run can never be produced by the assertion being skipped"
    - "set +e / capture rc=$? / set -e bracketing around a single delegated command under an otherwise set -e script, so a non-zero exit doesn't terminate before the true exit code is captured"

key-files:
  created:
    - scripts/verify-harden04-featureless.sh
    - .planning/phases/30-regression-live-proof/30-REGRESSION-AUDIT.md
  modified: []

key-decisions:
  - "Standalone new script, not an edit to scripts/mailpit-verify.sh — keeps every other phase's use of the bare --workspace recipe unchanged (RESEARCH Open Question 1 recommendation, confirmed)"
  - "Positive assertion requires both exit 0 AND the named test's libtest '... ok' line in the log — a bare exit code is never trusted as proof the assertion executed"
  - "strings/nm symbol check is env-gated (HARDEN04_STRINGS_CHECK=1) and non-fatal, per DESIGN §d's explicit prohibition on making binary inspection the primary gate"

patterns-established:
  - "verification-exit-code-through-pipe discipline applied inside a set -euo pipefail script: bracket the delegated command with set +e / rc=$? / set -e, since set -e alone would otherwise abort the script before the exit code could be captured on a failure"

requirements-completed: [HARDEN-06]

coverage:
  - id: D1
    description: "scripts/verify-harden04-featureless.sh authored: delegates a scoped, genuinely featureless invocation to scripts/mailpit-verify.sh, captures true exit before any pipe, and fails non-zero if the D-10 assertion's self-skip sentinel is found in the log"
    requirement: "HARDEN-06"
    verification:
      - kind: other
        ref: "bash -n scripts/verify-harden04-featureless.sh"
        status: pass
      - kind: other
        ref: "shellcheck scripts/verify-harden04-featureless.sh (0 findings, exit 0)"
        status: pass
      - kind: other
        ref: "grep -q 'cargo test -p caprun --test harden04_featureless_create_session' scripts/verify-harden04-featureless.sh"
        status: pass
      - kind: other
        ref: "grep -q -- '--workspace --release' scripts/verify-harden04-featureless.sh"
        status: pass
    human_judgment: true
    rationale: "This task only authors and syntax-proves the script on macOS (host-side checks only). The authoritative real-Linux green run that actually exercises brokerd::TEST_FIXTURES_ACTIVE==false and the D-10 assertion is executed by the orchestrator in Plan 30-02 — this plan cannot self-certify that outcome."
  - id: D2
    description: ".planning/phases/30-regression-live-proof/30-REGRESSION-AUDIT.md authored: independently re-runs #[ignore]/weakened-assertion/stale-skip sweeps across Phase 27/28/29 test files, maps all 5 HARDEN-06 criteria to live-confirmed file:test-name anchors, and adjudicates the one deliberate skip branch as a correct feature-keyed guard"
    requirement: "HARDEN-06"
    verification:
      - kind: other
        ref: "grep -rn '#\\[ignore\\]' crates/ cli/ --include='*.rs' | grep -v '/target/' -> 0 matches (live re-run, recorded in doc)"
        status: pass
      - kind: other
        ref: "grep -c 'NEEDS-FIX\\|COVERED\\|CLEARED' 30-REGRESSION-AUDIT.md -> 10 (verdicts present)"
        status: pass
    human_judgment: false

# Metrics
duration: 26min
completed: 2026-07-17
status: complete
---

# Phase 30 Plan 01: HARDEN-04 Featureless Proof Script + Regression Audit Summary

**Standalone false-assurance-guarded bash wrapper that forces the self-skipping harden04 D-10 negative test to actually execute, plus an independent audit confirming zero weakened/ignored hardening tests across Phases 27-29.**

## Performance

- **Duration:** 26 min
- **Started:** 2026-07-17T17:37:00Z
- **Completed:** 2026-07-17T18:03:00Z
- **Tasks:** 2
- **Files modified:** 2 (both new)

## Accomplishments
- Authored `scripts/verify-harden04-featureless.sh`, a standalone wrapper that delegates to the shared `scripts/mailpit-verify.sh` recipe via a scoped `MAILPIT_VERIFY_CMD` override (`cargo build --workspace --release && cargo test -p caprun --test harden04_featureless_create_session`), bypassing Cargo's `--workspace` feature unification so `brokerd::TEST_FIXTURES_ACTIVE` resolves false and the D-10 assertion runs for real.
- Built a false-assurance guard into the wrapper: it greps the captured log for the test's own self-skip sentinel and exits non-zero if found, and separately requires the named test's libtest `... ok` line (not a bare exit code) before declaring success.
- Independently re-ran the regression-fixture audit sweep across all Phase 27/28/29 hardening test files (`#[ignore]`, `assert!(true)`, TODO/FIXME, non-Linux stub bodies) — 0 findings in every sweep — and adjudicated `harden04_featureless_create_session`'s self-skip as a correct, feature-keyed guard rather than a weakened test.
- Confirmed all 5 HARDEN-06 success criteria still map to their exact `file:test-name` anchors with zero line-number drift from RESEARCH.md.

## Task Commits

Each task was committed atomically:

1. **Task 1: Author scripts/verify-harden04-featureless.sh** - `ce0f880` (feat)
2. **Task 2: Regression-fixture audit sweep of Phase 27/28/29 test files** - `bdcb78b` (docs)

**Plan metadata:** committed by this SUMMARY commit (docs)

## Files Created/Modified
- `scripts/verify-harden04-featureless.sh` - new standalone, executable bash script; featureless `--release` build + scoped `-p caprun` test invocation + false-assurance skip-sentinel guard + positive libtest-line assertion + optional non-fatal `strings`/`nm` defense-in-depth
- `.planning/phases/30-regression-live-proof/30-REGRESSION-AUDIT.md` - new audit doc; independent re-run of the `#[ignore]`/weakened-assertion sweeps, per-criterion coverage table, and self-skip adjudication

## Decisions Made
- Standalone script rather than editing `scripts/mailpit-verify.sh` — per RESEARCH's Open Question 1 recommendation, this avoids adding an unconditional new step to a script every other phase reuses.
- Required both exit 0 AND the named test's passing libtest line as the positive proof condition, so a bare green exit code can never stand in for the assertion having actually run.
- Kept the optional `strings`/`nm` symbol scan strictly informational (env-gated, non-fatal) per DESIGN §d's explicit ruling against making binary inspection the primary HARDEN-04 gate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed exit-code capture defeated by `set -e`**
- **Found during:** Task 1 (self-review before committing, per the plan's own "capture true exit before any pipe" acceptance criterion)
- **Issue:** The script runs under `set -euo pipefail`. As first drafted, `rc=$?` appeared on the line immediately after the delegated `bash scripts/mailpit-verify.sh` call — but with `set -e` active, a non-zero exit from that command would terminate the script immediately, before the `rc=$?` capture line was ever reached. This would have silently defeated the whole point of capturing the true exit code (the project's own standing `verification-exit-code-through-pipe` discipline), and would have made a genuine failure look like an unhandled script crash rather than a clean, diagnosable non-zero exit.
- **Fix:** Bracketed the delegated command with `set +e` immediately before it and `set -e` immediately after the `rc=$?` capture, so a non-zero exit from `mailpit-verify.sh` is caught and recorded rather than aborting the script.
- **Files modified:** `scripts/verify-harden04-featureless.sh`
- **Verification:** `bash -n` and `shellcheck` both pass; manual read-through confirms `rc=$?` now always executes regardless of the delegated command's exit status.
- **Committed in:** `ce0f880` (part of Task 1's commit — caught and fixed before the commit, not a separate follow-up)

---

**Total deviations:** 1 auto-fixed (1 bug fix, Rule 1)
**Impact on plan:** Necessary correctness fix for the script's own load-bearing exit-code discipline; no scope creep, no plan-text change required.

## Issues Encountered
None beyond the deviation above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `scripts/verify-harden04-featureless.sh` is authored, syntactically valid (`bash -n`, `shellcheck` clean), and contains all the acceptance-criteria-required elements (scoped invocation, featureless release leg, true-exit capture before any pipe, skip-sentinel guard, env-gated non-fatal symbol check). It has NOT yet been run on real Linux — that authoritative green run is Plan 30-02's job (orchestrator-driven).
- `30-REGRESSION-AUDIT.md` found zero gaps; no fix-work was generated for Plan 30-02 to inherit.
- `./scripts/check-invariants.sh` passes (all 4 gates green) — no crate code was touched by this plan, confirming the "surgical, script+doc only" scope was honored.
- Plan 30-02 can proceed directly to running the bare `scripts/mailpit-verify.sh` (criterion 1) and this plan's new `scripts/verify-harden04-featureless.sh` (criterion 4) on real Linux, then compiling the final per-criterion evidence table and human sign-off.

---
*Phase: 30-regression-live-proof*
*Completed: 2026-07-17*
