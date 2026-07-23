---
phase: 17-live-acceptance-framing-honesty
plan: 04
subsystem: docs
tags: [project-md, doc-01, framing-honesty, milestone-docs]

requires:
  - phase: 16-confirm-ux-literal-binding-negative-controls
    provides: "verbatim source language for points 2, 3, 5, 6 (16-02-SUMMARY.md, 16-04-SUMMARY.md, v2-obligations todo)"
  - phase: 17-live-acceptance-framing-honesty (Plans 02/03)
    provides: "the CONTROL-01 honest-framing doc comments in cli/caprun/tests/s9_live_block.rs (point 1); the green ACCEPT-01 live proof that will authorize point 7 in Task 2"
provides:
  - "PROJECT.md v1.3 section carries DOC-01 points 1, 2, 6, 8 as standalone sentences, plus the revised controlled-experiment framing, the nonce honesty line, the one-unbroken-DAG scope sentence, and the self-consistency honesty clause"
  - "PROJECT.md's v0-era Residual risks bullet extended with a new v1.3 clause carrying DOC-01 points 3, 4, 5"
affects: ["Phase 17 DONE gate (coordinator legibility read, COORD MED-5); Task 2 of this same plan (point 7, still pending)"]

tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - .planning/PROJECT.md

key-decisions:
  - "Task 2 (DOC-01 point 7) deliberately NOT executed in this run. Per the plan's coordinator_gate, point 7 is gated on caprun-opus-77's OWN independent re-run of the live proof (scripts/mailpit-verify.sh, true exit code captured, live_acceptance_v1_3_composed passing under opus's own execution) — this executor's own SUMMARY files (17-02, 17-03) are informative context only and are explicitly disqualified as authorizing evidence, per the project's documented false-PASS-through-a-pipe and executor-self-marks-complete recurrence."
  - "No new top-level section was created in PROJECT.md — points 1/2/6/8 + framing/honesty lines were anchored as a new paragraph in the existing 'Current Milestone: v1.3' section immediately after the LLM-planner non-reopening paragraph; points 3/4/5 were appended as a new 'v1.3 residual risks' clause onto the existing v0-era Residual risks bullet, per 17-RESEARCH.md's explicit anchor-point guidance."
  - "REQUIREMENTS.md's DOC-01 checkbox was NOT touched — DOC-01 is only fully satisfied once Task 2 lands (per this plan's explicit instruction that the verification flow, not this task, owns the status flip)."

requirements-completed: []  # DOC-01 intentionally NOT marked complete — Task 2 (point 7) remains pending the coordinator's gate

coverage:
  - id: D1
    description: "PROJECT.md's v1.3 section states DOC-01 points 1, 2, 6, 8 as standalone, unsoftened sentences, plus the revised controlled-experiment framing, nonce honesty line, one-unbroken-DAG scope sentence, and self-consistency honesty clause"
    requirement: "DOC-01"
    verification:
      - kind: other
        ref: "grep token-presence check over .planning/PROJECT.md (decorative, mint_from_read, ProvideIntent, native-tls, test-isolation, per-session, residual-risks clause below) — all present"
        status: pass
      - kind: other
        ref: "grep -niE 'byte[- ]?identical' .planning/PROJECT.md — zero matches (pass condition)"
        status: pass
    human_judgment: true
    rationale: "DOC-01 is fundamentally a legibility requirement — per the plan's own verification section and coordinator_gate (COORD MED-5), a human/coordinator read confirming points 2 and 3 are standalone, unsoftened sentences is required IN ADDITION to the automated greps, which are presence scaffolding only."
  - id: D2
    description: "PROJECT.md's v0-era Residual risks bullet extended with a new v1.3 clause carrying points 3 (verify_chain forgeable head), 4 (guard-(c) runtime-vs-compile-time gap), 5 (Allowed-path replay/no-CAS)"
    requirement: "DOC-01"
    verification:
      - kind: other
        ref: "grep token-presence check (externally anchored, CAPRUN_ENABLE_IPC_CREATE_SESSION, replayed) — all present"
        status: pass
    human_judgment: true
    rationale: "Same legibility requirement as D1 — points 3 and 4 are [HOLD THE LINE] items the coordinator must personally confirm are not softened."
  - id: D3
    description: "Task 2 (DOC-01 point 7 — CONFIRM-01 proven live for the first time) intentionally NOT executed, per the plan's coordinator_gate"
    verification: []
    human_judgment: true
    rationale: "Task 2 is explicitly GATED on caprun-opus-77's own independent live-proof re-run (COORD MED-4), which happens outside this session via FAMP. This executor cannot obtain or substitute for that authorization; task deliberately left undone and uncommitted."

duration: ~25min
completed: 2026-07-09
status: blocked
---

# Phase 17 Plan 04: DOC-01 Framing Consolidation (Task 1 only) Summary

**PROJECT.md now carries 7 of 8 DOC-01 honesty points (1,2,3,4,5,6,8) plus the revised nonce framing, anchored in the existing v1.3 and Residual-risks sections — point 7 is deliberately withheld pending caprun-opus-77's independent live-proof re-run.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-09T22:00:00Z (approx)
- **Completed:** 2026-07-09T22:14:40Z
- **Tasks:** 1 of 2 (Task 2 intentionally not executed — see below)
- **Files modified:** 1

## Accomplishments

- Added a new "What v1.3's live proof does and does not claim (DOC-01):" paragraph to PROJECT.md's "Current Milestone: v1.3" section, immediately after the existing LLM-planner non-reopening sentence, carrying DOC-01 points 1, 2, 6, 8 as standalone, unsoftened sentences (point 2 explicitly marked [HOLD THE LINE] in the plan is stated in full, not as a subordinate clause), plus the residual-risks headline pointer (literal phrase "residual-risks clause below" present, grep-checked).
- Added the revised controlled-experiment framing (two hostile docs with identical injection text and derivation structure, differing only in a per-run test-isolation recipient token; operator's decision is the controlled variable), the nonce honesty line, the one-unbroken-DAG scope sentence (per-session verify_chain across a shared audit.db, not a cross-session parent_id chain), and the self-consistency honesty clause (COORD MED-1 — the live anchor pin is a self-consistency reconstruction, not independently-sourced ground truth).
- Extended the existing v0-era "Residual risks (acknowledged, not solved in v0)" bullet with a new "v1.3 residual risks (Phase 16/17, DOC-01)" clause carrying points 3 [HOLD THE LINE] (verify_chain's forgeable chain head), 5 (Allowed-path replay/no-CAS), and 4 (guard-(c)'s runtime-vs-compile-time gap) — all sourced verbatim/near-verbatim from 16-02-SUMMARY.md, 16-04-SUMMARY.md, and the v2-obligations todo, per 17-RESEARCH.md's citations.
- Confirmed via automated grep that none of the 10 required tokens are missing and that no "byte identical"/"byte-identical" phrasing appears anywhere in PROJECT.md.
- **Task 2 (point 7) deliberately NOT executed.** Per the plan's `<coordinator_gate>`, point 7 (CONFIRM-01 proven live for the first time) may only be added after caprun-opus-77 independently re-runs `scripts/mailpit-verify.sh` with `MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3 live_acceptance_v1_3_composed'`, captures the true exit code, and confirms `live_acceptance_v1_3_composed` green under their OWN execution — separately from and not substitutable by this executor's own 17-02/17-03 SUMMARY.md files, which are explicitly informative context only, not authorizing evidence (COORD MED-4). That confirmation happens outside this session via FAMP and was not obtained here.

## Task Commits

Each task was committed atomically:

1. **Task 1: Anchor DOC-01 points 1, 2, 6, 8 + revised framing + honesty lines in the v1.3 section; points 3, 5, 4 in a new v1.3 residual-risks clause** - `5bf32dd` (docs)

**Task 2 was NOT executed and has no commit** — see "Deviations from Plan" / Task-2 gate note below.

_Note: this is a per-worktree/plan-branch commit hash; the orchestrator's merge into the mainline may produce a different hash for the same content._

## Files Created/Modified

- `.planning/PROJECT.md` - Added the DOC-01 framing paragraph (points 1, 2, 6, 8 + revised framing + honesty lines) to the v1.3 section, and a new v1.3 residual-risks clause (points 3, 4, 5) appended to the existing v0-era Residual risks bullet. No new top-level section created; existing content untouched (scoped Edit calls only, no whole-file Write).

## Decisions Made

- Followed 17-RESEARCH.md's recommended anchor points exactly: points 1/2/6/8 + framing/honesty lines went into the "Current Milestone: v1.3" section (new paragraph after the existing LLM-planner non-reopening sentence); points 3/4/5 went into an extension of the existing v0-era "Residual risks" bullet. No new top-level section.
- Preserved the plan's mandated point ORDER within the residual-risks clause (3, 5, 4) as specified in the plan's `<action>` block, even though 17-RESEARCH.md's own recommended-edit narrative lists them as 3, 5, 4 as well — consistent.
- Did not touch REQUIREMENTS.md's DOC-01 checkbox, per the plan's explicit instruction ("verification flow owns status flips") and per this dispatch's explicit "CRITICAL FOR THIS PLAN" instruction not to mark DOC-01 complete until Task 2 lands.
- Did not touch STATE.md or ROADMAP.md, per worktree-mode `<parallel_execution>` instructions (orchestrator updates these centrally after merge).

## Deviations from Plan

None — Task 1 executed exactly as written. No auto-fixes were needed; the automated verify (token-presence check + byte-identical grep) passed on the first run.

### Task 2 — deliberate non-execution (not a deviation, a plan-mandated gate)

Task 2 (DOC-01 point 7) is `autonomous: false` and carries its own `<coordinator_gate>` in the plan, explicitly requiring caprun-opus-77's OWN independent re-run of the live proof as the authorizing evidence — not this executor's SUMMARY files. This executor has no mechanism to obtain that FAMP-mediated authorization within this session, so Task 2 was correctly left unexecuted and uncommitted, per the objective given for this dispatch. This is the plan working as designed, not a blocker to be worked around.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Task 1's content is committed (`5bf32dd`) and ready for the coordinator's legibility read (COORD MED-5) alongside the automated checks.
- **Blocker for Phase 17 DONE:** Task 2 (DOC-01 point 7) remains pending caprun-opus-77's independent re-run of `scripts/mailpit-verify.sh` (live_acceptance_v1_3_composed, true exit code captured). Once that re-run is green and the coordinator authorizes it, a follow-up executor/session should add point 7 to the same DOC-01 paragraph and only THEN allow REQUIREMENTS.md's DOC-01 checkbox to flip.
- Per the plan's own closing instruction: when Phase 17 is finally marked DONE, diff-check ROADMAP.md to ensure the doc-completion commit does not flip the PHASE-level checkbox prematurely (documented recurring GSD bug, 2-for-2 at Phase 15/16) — the coordinator marks the phase DONE only after the independent live-proof re-run.

---
*Phase: 17-live-acceptance-framing-honesty*
*Completed: 2026-07-09 (Task 1 only; Task 2 pending)*
