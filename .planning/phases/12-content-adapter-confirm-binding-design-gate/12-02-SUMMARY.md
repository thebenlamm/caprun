---
phase: 12-content-adapter-confirm-binding-design-gate
plan: 02
subsystem: docs
tags: [design-doc, confirm-binding, sha256, pending-confirmation, gate]

# Dependency graph
requires:
  - phase: 12-content-adapter-confirm-binding-design-gate (plan 01)
    provides: DESIGN-content-adapter-mediation.md's collect-then-Block blocked-arg set (D-14)
provides:
  - "planning-docs/DESIGN-confirm-binding.md — CONFIRM-03 combined-digest binding spec"
affects: [phase-13-adapter, phase-14-content-block, phase-15-extraction, phase-16-confirm-binding-impl, plan-03-gate-record]

# Tech tracking
tech-stack:
  added: []
  patterns: [combined-digest-over-full-arg-set, mint-after-transform, single-shot-confirm-extends-to-set]

key-files:
  created: [planning-docs/DESIGN-confirm-binding.md]
  modified: []

key-decisions:
  - "Combined digest is ONE SHA-256 over the full blocked-arg set (recipient+body together), reusing the existing literal_sha256 pattern — not a new hash primitive and not per-arg digests."
  - "combined_digest is an additive field alongside PendingConfirmation.resolved_args, frozen at Block time, never re-derived at confirm time — extends v1.2's mechanism, does not replace it."
  - "Post-transformation-bytes rule closes D-12(b) by construction: mint ValueRecord only after transform, no transform between mint/Block/send — so the digest is guaranteed to equal sent bytes rather than needing a runtime check."

patterns-established:
  - "Combined-digest-over-full-set: single-shot confirm/deny must bind to one digest covering every blocked arg, never a per-arg or subset digest, to prevent a confirmed arg from silently carrying an unconfirmed sibling."

requirements-completed: [DESIGN-01]

coverage:
  - id: D1
    description: "DESIGN-confirm-binding.md exists with Combined-Digest Binding, Post-Transformation Bytes, Verbatim Display, Block Narration, and Single-Shot/No-Re-Invoke sections, all D-08/09/10/19/20 resolved as hard MUSTs, plus a terminal Done-When predicate"
    requirement: "DESIGN-01"
    verification:
      - kind: other
        ref: "grep verify: combined-digest, post-transform, resolved_args, submit_plan_node, Done-When all present; D-08/D-09/D-10/D-19/D-20 all present; MUST count = 40 (>=15 required); no crates/ files touched; ./scripts/check-invariants.sh PASSED"
        status: pass
    human_judgment: true
    rationale: "Per D-11, this session authoring the doc must not also review/approve it — plan 03's fresh-context adversarial review is the actual soundness gate. Grep checks here verify structural completeness only, not soundness."

# Metrics
duration: 25min
completed: 2026-07-07
status: complete
---

# Phase 12 Plan 02: DESIGN-confirm-binding.md Summary

**Authored the CONFIRM-03 combined-digest DESIGN doc extending v1.2's PendingConfirmation mechanism — one SHA-256 digest over the FULL blocked-arg set (recipient+body together), post-transformation-bytes binding, no-truncation display, and every-arg block narration.**

## Performance

- **Duration:** 25 min
- **Tasks:** 2 (Task 2 was a verification-only no-op — no gaps found, no repair needed)
- **Files modified:** 1 created

## Accomplishments
- `planning-docs/DESIGN-confirm-binding.md` created with all required named sections: Combined-Digest Binding (CONFIRM-03, D-08/D-19), Post-Transformation Bytes — No Drift Between Confirm and Send (D-08/Pitfall 2), Verbatim Display — No Truncation (D-09), Block Narration for Every Arg (CONFIRM-04, D-20), Single-Shot Over the Whole Set — Confirm MUST NOT Re-Invoke `submit_plan_node` (D-17/D-19), and a terminal Done-When (Acceptance Predicate).
- Explicitly reuses the existing `literal_sha256` SHA-256 pattern (`crates/executor/src/lib.rs:112-116`) rather than introducing a new hash primitive.
- Specifies `combined_digest` as an additive field on `PendingConfirmation` (`crates/brokerd/src/confirmation.rs`), following the existing "frozen at Block time, never re-derived" doc-comment convention used by `ResolvedArg`.
- Explicitly cross-references `DESIGN-content-adapter-mediation.md`'s collect-then-Block section (D-14) so both docs agree on the blocked-arg set shape (recipient AND body together).
- Task 2's completeness self-check found zero gaps: all 5 D-IDs (D-08, D-09, D-10, D-19, D-20) present verbatim, MUST count = 40 (comparable to the 40s-70s range of the v1.2 analog docs, well above the >=15 floor), no "should"-weakened language, cross-doc set agreement confirmed, and `./scripts/check-invariants.sh` still passes with no `crates/` files touched.

## Task Commits

1. **Task 1: Confirm-binding + combined-digest + block-narration sections** - `11cba6c` (docs)
2. **Task 2: Completeness self-check against D-08/09/10/19/20 and cross-doc set agreement** - no commit (verification-only; zero gaps found, no file changes required)

**Plan metadata:** included in Task 1's commit and this SUMMARY's own commit.

## Files Created/Modified
- `planning-docs/DESIGN-confirm-binding.md` - New DESIGN doc specifying CONFIRM-03's combined-digest confirm-binding, extending v1.2's `PendingConfirmation`/`ResolvedArg` mechanism.

## Decisions Made
- Reused SHA-256 (the existing `literal_sha256` pattern) for the combined digest rather than any new primitive, per the RESEARCH doc's "Don't Hand-Roll" table and D-19's requirement.
- Specified a MUST-fixed canonical ordering for both the digest's concatenation input and the block-narration display order, so an independent verifier can reproduce the digest from the displayed literals without guessing an ordering convention (not explicitly required by the plan's task text, but necessary to make "the digest is reproducible" a falsifiable claim rather than an implicit assumption).
- Task 2 required no repair — the doc as authored in Task 1 already satisfied every completeness criterion, so no second commit was needed for that task.

## Deviations from Plan

None - plan executed exactly as written. Task 2 found no gaps to repair, which is a valid Task 2 outcome per its own acceptance criteria ("repair any gap in-place" — there was none to repair).

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required. Documentation-only phase; no crates/ code was created or modified, preserving the DESIGN-01 gate (executor/brokerd code for CONFIRM-03 remains blocked until `DESIGN-GATE-RECORD-v1.3.md` records APPROVED/UNBLOCKED, per plan 03).

## Next Phase Readiness
- `planning-docs/DESIGN-confirm-binding.md` is ready, alongside plan 01's `DESIGN-content-adapter-mediation.md`, for plan 03's mandatory fresh-context adversarial review and `DESIGN-GATE-RECORD-v1.3.md` authoring (D-11: this authoring session must NOT self-review).
- No blockers. The two DESIGN docs agree on the blocked-arg set shape (recipient+body together); plan 03 can proceed directly to the adversarial review checkpoint.

---
*Phase: 12-content-adapter-confirm-binding-design-gate*
*Completed: 2026-07-07*
