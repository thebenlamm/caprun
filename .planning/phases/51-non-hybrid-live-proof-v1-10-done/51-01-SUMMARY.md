---
phase: 51-non-hybrid-live-proof-v1-10-done
plan: 01
subsystem: testing
tags: [rust, cli, live-acceptance, audit-chain, mock-egress]
requires:
  - phase: 50-cli-multi-node-driver-mid-loop-confirm-continuity
    provides: safe-coding-workflow CLI driver and same-Session external confirmation hold
provides:
  - Real-CLI LIVE-07 one-Session integration harness
  - Concurrent external github.pr grant and git.push confirmation sidecar
  - Machine-checked non-hybrid framing and mock-egress feature gate
affects: [51-02, LIVE-07, LIVE-08, compose-verify]
tech-stack:
  added: []
  patterns: [concurrent CLI sidecar, one-session durable terminal assertions, forwarded test feature]
key-files:
  created: [cli/caprun/tests/live_acceptance_v1_10_cli.rs]
  modified: [cli/caprun/Cargo.toml]
key-decisions:
  - "Expose brokerd/mock-egress-ca through a local caprun feature so the crate-level cfg cannot silently compile out the LIVE test."
  - "Keep LIVE-07 requirement verification pending until compose-verify runs on a Docker-capable Linux host."
patterns-established:
  - "Live CLI sidecars react to surfaced session/effect identifiers without opening a second Session."
requirements-completed: []
coverage:
  - id: D1
    description: "CLI-driven edit-test-commit-push-PR chain remains in one Session and passes real audit verification"
    requirement: LIVE-07
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/live_acceptance_v1_10_cli.rs#live_07_cli_multi_node_one_session_verify_chain"
        status: unknown
    human_judgment: true
    rationale: "The executor host lacks Docker and cannot run the authoritative compose-verify proof."
duration: 18min
completed: 2026-08-04
status: complete
---

# Phase 51 Plan 01: Non-hybrid LIVE-07 CLI Proof Summary

**Real `caprun run safe-coding-workflow` harness drives edit through PR in one Session, with concurrent external grant/confirm actions and durable audit assertions.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-08-04T03:12:00Z
- **Completed:** 2026-08-04T03:30:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Added an always-on binary guard plus a Linux/mock-CA gated LIVE-07 test using the real CLI argv path.
- Added a concurrent sidecar that grants `github.pr` once and confirms only the surfaced `git.push` effect while the worker remains connected.
- Asserted exit 0, exactly one surfaced Session, real `caprun audit` PASSED output, and exactly one durable push and PR success terminal.

## Task Commits

1. **Task 1 RED: establish failing LIVE-07 tracer** - `3b0ead5` (test)
2. **Task 1 GREEN: implement one-Session CLI proof** - `ff71f61` (feat)
3. **Task 1 REFACTOR: clarify opaque credentials** - `e86ead6` (refactor)

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_v1_10_cli.rs` - F1-safe git fixture, real CLI driver, sidecar, audit/event assertions, and framing pins.
- `cli/caprun/Cargo.toml` - Non-default `mock-egress-ca` forwarding feature for an effective crate-level cfg gate.

## Decisions Made

- Forwarded the dependency feature through `caprun`; dependency feature syntax alone does not activate a local `cfg(feature = ...)` predicate.
- Did not mark LIVE-07 complete because its authoritative Docker/compose execution was unavailable on this host.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Made the mock-egress cfg gate executable**
- **Found during:** Task 1
- **Issue:** `--features brokerd/mock-egress-ca` does not set `cfg(feature = "mock-egress-ca")` in the `caprun` crate, silently compiling out the planned test.
- **Fix:** Added a non-default forwarding feature and changed the authoritative recipe to `--features mock-egress-ca`.
- **Files modified:** `cli/caprun/Cargo.toml`, `cli/caprun/tests/live_acceptance_v1_10_cli.rs`
- **Verification:** `cargo metadata --no-deps --format-version 1` and invariant gates passed.
- **Committed in:** `ff71f61`

---

**Total deviations:** 1 auto-fixed (1 Rule 2)
**Impact on plan:** The small build-configuration addition prevents a false-green zero-test LIVE run; no production source or dependency was added.

## Issues Encountered

- Host test linking could not run because this executor image lacks `pkg-config`/OpenSSL discovery.
- Authoritative compose verification could not start because `docker` is not installed. LIVE-07 therefore remains pending for requirement completion.
- `cargo fmt` was unavailable in the executor toolchain; repository commit hooks formatted and accepted the Rust changes.
- `./scripts/check-invariants.sh` passed all gates; `cargo metadata` accepted the feature graph.

## Known Stubs

None.

## User Setup Required

Run the documented compose command on a Docker-capable Linux host; no application configuration changes are required.

## Next Phase Readiness

The Plan 02 proof family can build on the same feature gate and one-Session framing. Phase verification must run the authoritative compose command before marking LIVE-07 complete.

## Self-Check: PASSED

- Created test and Cargo feature files exist.
- Task commits `3b0ead5`, `ff71f61`, and `e86ead6` exist.
- No unexpected tracked-file deletions occurred.

---
*Phase: 51-non-hybrid-live-proof-v1-10-done*
*Completed: 2026-08-04*
