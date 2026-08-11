---
phase: 51-non-hybrid-live-proof-v1-10-done
plan: 09
subsystem: testing
tags: [rust, live-proof, audit-provenance, order-independence]
requires:
  - phase: 51-08
    provides: cleared audit append design gate after independent adversarial trace
provides:
  - Anchor-first LIVE-08 process-exit attribution independent of event insertion order
  - Explicit two-exit and one-per-actor duplicate-dispatch protection
affects: [51-04, LIVE-08]
tech-stack:
  added: []
  patterns: [durable-id attribution, order-independent proof oracle]
key-files:
  created:
    - .planning/phases/51-non-hybrid-live-proof-v1-10-done/51-09-SUMMARY.md
  modified:
    - cli/caprun/tests/live_acceptance_v1_10_cli.rs
key-decisions:
  - "Select the genuine process.exec exit exclusively by the unique github.pr/body anchor's read_event_id; vector order is never evidence."
  - "Keep dispatch cardinality separate from attribution: exactly two exits, one process.exec and one git.commit."
patterns-established:
  - "Durable anchor first: resolve an event by persisted identity, then verify actor and provenance root."
requirements-completed: []
coverage:
  - id: D1
    description: "Order-independent LIVE-08 durable-anchor attribution with duplicate-dispatch protection"
    requirement: LIVE-08
    verification:
      - kind: integration
        ref: "cli/caprun/tests/live_acceptance_v1_10_cli.rs#live_08_attribution_is_independent_of_exit_event_order"
        status: pass
    human_judgment: true
    rationale: "The helper is host-proven, but LIVE-08 completion requires the unchanged Plan 51-04 authoritative real-Linux compose run and evidence checkpoint."
duration: 10min
completed: 2026-08-08
status: complete
---

# Phase 51 Plan 09: LIVE-08 Proof Oracle Repair Summary

**LIVE-08 now attributes the blocked PR body through its durable read-event anchor while independently enforcing the intentional process.exec plus git.commit exit pair.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-08-08T16:33:14Z
- **Completed:** 2026-08-08T16:43:14Z
- **Tasks:** 2
- **Files modified:** 1 implementation/test file, plus this summary

## Accomplishments

- Added a test-local oracle that finds exactly one event matching `anchor.read_event_id`, verifies its `sink:process.exec:*` actor, and binds its id to both the anchor read id and provenance-chain root.
- Preserved duplicate-dispatch protection independently: exactly two `process_exited` events total, exactly one `process.exec`, and exactly one `git.commit`.
- Added an ordinary host regression proving forward and reversed exit-event order produce the same process attribution.
- Replaced LIVE-08's obsolete global-one-exit assertion and `process_events[0]` selection without changing production code or completion evidence.

## Task Commits

1. **Task 1: Lock the anchor-first oracle with an order-independent regression** — `171bdc0`
2. **Task 2: Replace LIVE-08 global uniqueness and positional binding with the durable oracle** — `54bca8e`

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_v1_10_cli.rs` — shared anchor-first assertion helper, reversed-order regression, and corrected LIVE-08 assertion tail.
- `.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-09-SUMMARY.md` — bounded verification and unchanged Plan 51-04 handoff.

## Verification

- `cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca live_08_attribution_is_independent_of_exit_event_order -- --exact --test-threads=1` — exit 0; 1 passed.
- `cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca --no-run` — exit 0.
- `./scripts/check-invariants.sh` — exit 0; Gates 1–6 passed.
- `rustfmt +1.89.0 --edition 2021 --check cli/caprun/tests/live_acceptance_v1_10_cli.rs` — exit 0.
- `git diff --check` — exit 0.
- The plan's literal `cargo fmt --all -- --check` command could not run because the active stable toolchain lacks the rustfmt component (exit 1). No component/package installation was attempted. The modified file passed the available pinned-toolchain rustfmt check.
- An initial unfiltered LIVE target attempt reached the environment-bound tests and failed because this host lacked the compose mock-GitHub/DNS sidecars. That attempt is not green proof and created no evidence artifact.

## Decisions Made

- Used full borrowed `Event` and `SinkBlockedAnchor` values so the focused regression exercises exactly the same helper as the LIVE assertion.
- Kept the unique durable anchor selection before process-event attribution; no temporal or vector-position inference remains.

## Deviations from Plan

None in implementation scope. Verification used an exact host-only test filter because the unfiltered target executes real Linux LIVE paths and requires the Plan 51-04 compose environment.

## Issues Encountered

- The active stable Rust toolchain has no rustfmt component. A separately installed 1.89.0 rustfmt checked the only modified Rust file successfully.
- The retained `.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-LIVE-SCOPED.log` remains untouched RED diagnostic evidence; it is not proof of completion.

## Authentication Gates

None.

## Known Stubs

None.

## Threat Flags

None. This plan changed test proof code only and introduced no network, auth, file-access, schema, or TCB surface.

## Next Phase Readiness

LIVE-08 remains Pending. Plan 51-04 must now be re-run unchanged for the authoritative scoped and full real-Linux compose gates, raw-log retention, evidence/status reconciliation, and human evidence checkpoint. This plan did not modify Plan 51-04, LIVE evidence, requirements, windows, validation, ROADMAP, STATE, or the retained failed log.

## Self-Check: PASSED

- The modified test file and this summary exist.
- Task commits `171bdc0` and `54bca8e` are reachable.
- Both task commits contain only `cli/caprun/tests/live_acceptance_v1_10_cli.rs`.
- Focused regression, compile gate, invariant Gates 1–6, targeted rustfmt, and diff check passed.
- No production source or protected proof/status artifact was edited by this executor.

---
*Phase: 51-non-hybrid-live-proof-v1-10-done*
*Completed: 2026-08-08*
