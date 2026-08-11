---
phase: 51-non-hybrid-live-proof-v1-10-done
plan: 05
subsystem: testing
tags: [rust, sqlite, audit-chain, concurrency, live-proof]
requires:
  - phase: 51-non-hybrid-live-proof-v1-10-done
    provides: D1-D4 blocking-defect analysis from the real-Linux proof attempt
provides:
  - Host-runnable RED oracle for the external-append audit-chain fork
  - Deterministic contention RED oracle requiring successful linear appends
  - LIVE harness diagnostics that preserve child stderr and use durable provenance attribution
affects: [51-07, 51-04-live-proof-rerun]
tech-stack:
  added: []
  patterns: [file-backed shared SQLite regression tests, durable-anchor attribution oracle]
key-files:
  created: [crates/brokerd/tests/audit_chain_fork_regression.rs]
  modified: [cli/caprun/tests/live_acceptance_v1_10_cli.rs]
key-decisions:
  - "Keep both audit regressions as genuine RED requirements; Plan 51-07 must fix the append path without weakening verification."
  - "Treat persisted anchor provenance, not driver stdout text, as LIVE-08's attribution oracle."
patterns-established:
  - "Cross-process audit-chain tests use a shared file-backed SQLite database and public brokerd audit APIs."
requirements-completed: [LIVE-07, LIVE-08]
coverage:
  - id: D1
    description: Host-runnable regression reproduces the external-append audit-chain fork.
    requirement: LIVE-07
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/audit_chain_fork_regression.rs#broker_append_after_external_grant_must_not_fork_chain"
        status: fail
    human_judgment: true
    rationale: "This plan intentionally commits a RED oracle; Plan 51-07 must make it pass unchanged."
  - id: D2
    description: Independent contended appends must succeed while preserving one linear chain leaf.
    requirement: LIVE-07
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/audit_chain_fork_regression.rs#contended_appends_from_independent_connections_stay_linear"
        status: fail
    human_judgment: true
    rationale: "This plan intentionally commits a RED oracle; Plan 51-07 must make it pass unchanged."
  - id: D3-D4
    description: LIVE-08 reaches durable attribution checks and both LIVE bodies retain stderr on sidecar failure.
    requirement: LIVE-08
    verification:
      - kind: integration
        ref: "cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca --no-run"
        status: pass
    human_judgment: false
duration: 57min
completed: 2026-08-05
status: complete
---

# Phase 51 Plan 05: Host-Runnable Audit RED Oracles and LIVE Harness Repair Summary

**Deterministic audit-chain fork regressions now expose D1/D2 without Docker, while LIVE failures retain stderr and LIVE-08 relies on persisted provenance.**

## Performance

- **Duration:** 57 min
- **Started:** 2026-08-05T20:42:54Z
- **Completed:** 2026-08-05T21:39:53Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added a public-API regression that deterministically reports the two-leaf fork caused by an external grant append followed by a stale broker append.
- Added a six-worker, eight-append contention regression that requires every write to succeed and the resulting chain to remain linear.
- Reordered LIVE sidecar/stderr joins so failures include broker and worker diagnostics, and removed the unreachable LIVE-08 stdout gate in favor of durable anchor provenance.

## Task Commits

1. **Task 1: Commit a host-runnable RED regression that reproduces the D1 chain fork** - `f84daa7`
2. **Task 2: Add the contention RED regression that D2 alone cannot satisfy** - `b9252ec`
3. **Task 3: Repair the LIVE harness so failures are diagnostic and LIVE-08's attribution actually runs** - `ada45b6`

## Files Created/Modified

- `crates/brokerd/tests/audit_chain_fork_regression.rs` - Two ungated, file-backed audit-chain RED regressions.
- `cli/caprun/tests/live_acceptance_v1_10_cli.rs` - Diagnostic sidecar failure handling and durable LIVE-08 attribution oracle.

## Verification

- `cargo test -p brokerd --test audit_chain_fork_regression --no-run` passed.
- Bounded full RED run exited 101 as expected: sequential append reported leaf count 2 and contention reported leaf count 48.
- `cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca --no-run` passed.
- `./scripts/check-invariants.sh` passed Gates 1-6.

## Decisions Made

- Preserved `verify_chain` and direct requirement assertions; the production append path is the only permitted future fix locus.
- Used durable anchor/read-event/provenance identity as LIVE-08's attribution oracle rather than process-local terminal wording.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The resumed worktree already contained Task 2 commit `b9252ec`; it was inspected and its required bounded pre-fix failure was re-verified before continuing.
- Existing compiler warnings in sandbox and stream-hold code were unrelated and left unchanged.

## Known Stubs

None.

## User Setup Required

The required host OpenSSL prerequisites were already present (`pkg-config`, Cargo, and `/usr/include/openssl/ssl.h` all resolved).

## Next Phase Readiness

Plan 51-07 can now make both broker regressions green without changing their assertions. LIVE-07 and LIVE-08 remain pending until the unchanged Plan 51-04 real-Linux proof rerun succeeds.

## Self-Check: PASSED

- Both modified test files exist.
- Task commits `f84daa7`, `b9252ec`, and `ada45b6` exist in git history.
- No live evidence, completion ledger, STATE, ROADMAP, REQUIREMENTS, or WINDOWS file was edited by this plan completion.

---
*Phase: 51-non-hybrid-live-proof-v1-10-done*
*Completed: 2026-08-05*
