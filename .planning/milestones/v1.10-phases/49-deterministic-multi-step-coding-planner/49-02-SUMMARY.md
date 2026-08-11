---
phase: 49-deterministic-multi-step-coding-planner
plan: 02
subsystem: planner
tags: [coding-planner, LIVE-08-expressibility, anti-launder, CODE-01, CODE-02, COVERAGE, nyquist]

requires:
  - phase: 49-deterministic-multi-step-coding-planner
    provides: "SafeCodingWorkflow + named_handles multi-mint + plan_next five-node recipe + CODE-01/02 unit proofs"
provides:
  - "LIVE-08 expressibility via test-only CodingI2ProofPlanner (out_1 → github.pr body)"
  - "Strengthened success-path anti-launder (every arg is intent-minted key)"
  - "Coding RequestFd residual hygiene documented/confirmed"
  - "COVERAGE.md no-external-API + CaprunIntent add-alongside assumption-delta"
  - "49-VALIDATION Wave 0 complete / nyquist_compliant true"
affects:
  - 50-cli-multi-node-confirm-hold
  - 51-live-coding-proof

tech-stack:
  added: []
  patterns:
    - "Test-only proof planner for LIVE-08 bag routing; production DeterministicPlanner never places out_*"
    - "Strengthened anti-launder: intent-minted key-set membership, not merely out_* exclusion"
    - "Phase COVERAGE.md assumption-delta for closed CaprunIntent add-alongside"

key-files:
  created:
    - .planning/phases/49-deterministic-multi-step-coding-planner/COVERAGE.md
  modified:
    - cli/caprun/tests/planner.rs
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs
    - .planning/phases/49-deterministic-multi-step-coding-planner/49-VALIDATION.md

key-decisions:
  - "Prefer test-only CodingI2ProofPlanner over production default-off flag (never product-selected)"
  - "Proof path places out_1 into github.pr body only; other PR args stay intent-minted"
  - "Worker RequestFd skip already correct from 49-01 — residual hygiene is documentation confirmation"
  - "No CLI multi-node product path; no LIVE SUCCESS claim; zero new crates"

patterns-established:
  - "CodingI2ProofPlanner reuses DeterministicPlanner for steps 0..3, diverges at step 4 for out_* placement"
  - "Expressibility tests assert success-path DeterministicPlanner independently still refuses out_*"

requirements-completed: [CODE-01, CODE-02]

coverage:
  - id: D1
    description: "LIVE-08 expressibility: test-only proof planner places out_1 into github.pr body"
    requirement: CODE-02
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#coding_i2_proof_places_out_handle"
        status: pass
    human_judgment: false
  - id: D2
    description: "Success-path anti-launder: every arg value_id is intent-minted (no out_*)"
    requirement: CODE-02
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#coding_success_path_does_not_place_out_handles"
        status: pass
    human_judgment: false
  - id: D3
    description: "Email/file plan_next + stream_substrate + proto_claims regression green"
    requirement: CODE-01
    verification:
      - kind: unit
        ref: "cargo test -p caprun --test planner; cargo test -p caprun --test stream_substrate; cargo test -p brokerd --test proto_claims"
        status: pass
    human_judgment: false
  - id: D4
    description: "COVERAGE.md no-external-API + CaprunIntent add-alongside assumption-delta; Wave 0 complete"
    requirement: CODE-01
    verification:
      - kind: other
        ref: ".planning/phases/49-deterministic-multi-step-coding-planner/COVERAGE.md"
        status: pass
    human_judgment: false

duration: 4min
completed: 2026-07-29
status: complete
---

# Phase 49 Plan 02: LIVE-08 Expressibility + Coverage Summary

**Test-only CodingI2ProofPlanner places bag `out_1` into `github.pr` body for LIVE-08 expressibility while success-path DeterministicPlanner remains intent-key-only; COVERAGE/Wave-0 docs closed — no CLI multi-node (Phase 50) and no LIVE SUCCESS claim (Phase 51).**

## Performance

- **Duration:** 4 min
- **Started:** 2026-07-29T19:57:40Z
- **Completed:** 2026-07-29T20:01:27Z
- **Tasks:** 2
- **Files modified:** 5 (3 code/docs-in-code + COVERAGE.md + 49-VALIDATION.md)

## Accomplishments

- Added test-only `CodingI2ProofPlanner` + `coding_i2_proof_places_out_handle` proving bag `out_1` → `github.pr`/`body` routing expressibility (Phase 49 unit level only).
- Strengthened `coding_success_path_does_not_place_out_handles`: every success-path arg is in the intent-minted key set, not merely non-`out_*`.
- Confirmed coding worker path skips RequestFd/claim demotion (CODE-02 residual hygiene); documented bag key contract (proof vs success) in planner module docs.
- Wrote `COVERAGE.md` (no external API; CaprunIntent add-alongside assumption-delta) and marked `49-VALIDATION.md` Wave 0 complete / `nyquist_compliant: true`.
- Host regressions green: planner 26, stream_substrate 9, proto_claims 16; `check-invariants.sh` green; zero new crates.

## Task Commits

Each task was committed atomically:

1. **Task 1: LIVE-08 expressibility + anti-launder + coding RequestFd hygiene** - `6681aed` (feat)
2. **Task 2: COVERAGE + validation Wave 0 + stream regression + hygiene sweep** - `a812ebb` (docs)

**Plan metadata:** (pending final docs commit)

## Files Created/Modified

- `cli/caprun/tests/planner.rs` — CodingI2ProofPlanner + coding_i2_proof_places_out_handle; strengthened anti-launder
- `cli/caprun/src/planner.rs` — bag key / LIVE-08 expressibility framing docs (success path unchanged)
- `cli/caprun/src/worker.rs` — residual CODE-02 RequestFd-skip hygiene comment
- `.planning/phases/49-deterministic-multi-step-coding-planner/COVERAGE.md` — no-external-API + assumption-delta
- `.planning/phases/49-deterministic-multi-step-coding-planner/49-VALIDATION.md` — Wave 0 complete

## Decisions Made

- **Test-only proof planner preferred** over a production default-off flag — never product-selected; worker always uses DeterministicPlanner.
- **Proof placement target = github.pr body** with `out_1` (simulating mint_from_exec); other PR args remain intent-minted.
- **Worker residual hygiene was already correct** from 49-01 (skip RequestFd entirely for SafeCodingWorkflow); Task 1 confirmed and documented rather than restructured.
- **Framing honesty:** expressibility ≠ LIVE multi-step DONE; Phase 51 owns non-hybrid LIVE-07/08.

## Deviations from Plan

None - plan executed exactly as written.

Minor notes (not deviations):
- TDD RED/GREEN collapsed into a single feat commit because the proof planner is test-only (lives entirely in `tests/planner.rs`); production success-path recipe was not changed.
- No IntentAccepted fixture compile breaks remained after 49-01 exhaustive sweep — Task 2 hygiene found zero residual omissions.

## Issues Encountered

- Host needs user-local `~/.local/openssl-dev` (pkg-config + libssl-dev extracted debs) for lettre native-tls builds — same as 49-01; no repo change. Docker/mailpit-verify not required for these host-safe unit tests.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 49 CODE-01/02 complete: coding recipe + trusted mint/bag + LIVE-08 expressibility + HYG green.
- Phase 50 can productize CLI multi-node coding verb + Block-and-Hold on this spine.
- Phase 51 LIVE proof can wire genuine taint-via-bag using the proven `out_*` placement pattern (still blocked until CLI + confirm-hold product surface exists).

## Self-Check: PASSED

- All key files present
- Commits `6681aed`, `a812ebb` present in git log
- `check-invariants.sh` green
- `cargo test -p caprun --test planner` 26 passed
- `cargo test -p caprun --test stream_substrate` 9 passed
- `cargo test -p brokerd --test proto_claims` 16 passed
- No product stubs; intentional fail-closed `coding.use_plan_next` placeholder remains non-shipped sink for mis-routed plan()

---
*Phase: 49-deterministic-multi-step-coding-planner*
*Completed: 2026-07-29*
