---
phase: 51-non-hybrid-live-proof-v1-10-done
plan: 03
subsystem: cli-live-proof
tags: [rust, cargo-features, hermetic-tests, audit-provenance]
requires: [51-02]
provides: [release-safe-proof-selector, hermetic-live-harness, exact-github-pr-provenance]
affects: [51-04]
tech-stack:
  added: []
  patterns: [non-default fixture feature, env-clear command factory, durable event anchor attribution]
key-files:
  created: [cli/caprun/tests/proof_selector_rejection.rs]
  modified: [cli/caprun/Cargo.toml, cli/caprun/src/main.rs, cli/caprun/src/planner.rs, cli/caprun/src/worker.rs, cli/caprun/tests/planner.rs, cli/caprun/tests/live_acceptance_v1_10_cli.rs]
key-decisions:
  - "Compile the destructive LIVE-08 planner and selector forwarding only with live-proof-fixtures."
  - "Treat the hashed github.pr/body SinkBlockedAnchor as the provenance oracle, not event row order."
metrics:
  duration: 8min
  completed: 2026-08-04
status: complete
---

# Phase 51 Plan 03: Live-Proof Review Gap Closure Summary

Ordinary caprun builds now reject the ambient LIVE-08 selector before audit creation, while the fixture build uses hermetic subprocess environments and proves the exact github.pr body anchor descends from the real process event.

## Accomplishments

- Added the non-default `live-proof-fixtures` feature and compiled the proof planner, worker selection arm, and parent forwarding path only when it is enabled.
- Added an ordinary-build regression proving selector rejection occurs before audit/key creation or workspace mutation.
- Centralized all LIVE caprun process construction behind `env_clear()` with a narrow fixture allowlist and case-specific proof selection.
- Replaced row-order inference with deserialization of hashed Event payloads and exact `github.pr`/`body` anchor assertions tying `read_event_id` and the provenance root to the unique `process_exited` event.

## Task Commits

- `f86c210` — `fix(51-03): contain live proof planner selector`
- `3934e1b` — `test(51-03): make live proof hermetic and attributable`

## Verification

- `./scripts/check-invariants.sh` — passed all Gates 1–6 after both tasks.
- `git diff --check` — passed.
- Cargo verification could not run on this host because `cargo-fmt` is absent and `openssl-sys` cannot find either `pkg-config` or an OpenSSL development installation. Plan 51-04's authoritative Docker environment installs those prerequisites and owns the real-Linux run.

## Deviations from Plan

None in implementation scope. Host dependency absence prevented the planned Cargo commands; no packages were installed or substituted.

## Known Stubs

None.

## Deferred Issues

- Run the full no-feature, planner, feature-check, and LIVE acceptance commands in Plan 51-04's Docker-capable environment.

## Self-Check: PASSED

- All seven plan-owned source/test files exist.
- Task commits `f86c210` and `3934e1b` exist in git history.
- The invariant suite passed after final implementation.
