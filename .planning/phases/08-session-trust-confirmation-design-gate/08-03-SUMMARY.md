---
phase: 08-session-trust-confirmation-design-gate
plan: 03
subsystem: security-design
tags: [design-doc, security-gate, session-trust, confirmation-release]

requires:
  - phase: 08-session-trust-confirmation-design-gate
    provides: DESIGN-session-trust-state.md (08-01), DESIGN-confirmation-release.md (08-02)
provides:
  - planning-docs/DESIGN-GATE-RECORD-v1.2.md — round-1 + round-2 gate record, Decision APPROVED, Gate status UNBLOCKED
affects: [phase-09-session-trust-state, phase-10-confirmation-release, phase-11-live-acceptance]

tech-stack:
  added: []
  patterns:
    - "Design-gate record with sha256-pinned hashes, grep-verified checklist, and a blocking human-verify checkpoint — mirrors v1.0 Phase 2's DESIGN-GATE-RECORD.md round-1/round-2/APPROVED precedent"

key-files:
  created:
    - planning-docs/DESIGN-GATE-RECORD-v1.2.md
    - planning-docs/DESIGN-REVIEW-v1.2-round1.md (authored by the human reviewer, not this plan)
  modified:
    - planning-docs/DESIGN-session-trust-state.md (B1/m1/m2 fixes)
    - planning-docs/DESIGN-confirmation-release.md (M1/M2/M3/m3 fixes)
    - .planning/REQUIREMENTS.md (TAINT-02 amended)

key-decisions:
  - "Round 1 (human adversarial review) found a genuine blocker: the draft-only class deny (Step 0.5) ran before the per-arg I2 taint loop, making ACC-01/ACC-02 unsatisfiable and breaking the v1.1 §9 live test. Fix: I2 per-arg Block now takes precedence — Step 0.5 runs only after the loop completes with no Block."
  - "TAINT-02 requirement text was amended (not silently reinterpreted) to state the corrected precedence, per explicit reviewer instruction to surface the ambiguity rather than resolve it invisibly inside the DESIGN doc alone."
  - "PendingConfirmation's schema changed from a bare `plan_node: PlanNode` (which cannot carry literal/taint/provenance — PlanNode is opaque ValueId handles only) to `sink: SinkId` + `resolved_args: Vec<ResolvedArg>`, a resolved snapshot captured at Block time."
  - "The executing agent (and this orchestrator) never sets Decision/Gate-status itself — round 1 was recorded NEEDS REVISION by the human reviewer, and round 2's APPROVED/UNBLOCKED was recorded only after the human reviewer's own re-read and explicit verdict."

patterns-established:
  - "Multi-round design gate: round 1 (NEEDS REVISION with itemized findings) → direct doc revisions addressing each finding → round 2 (re-hash + re-verify checklist + human re-review of amended sections only) → APPROVED/UNBLOCKED. Identical shape to the v1.0 Phase 2 precedent."

requirements-completed: [PROC-01]

coverage:
  - id: D1
    description: "Gate-record artifact exists, pins sha256 hashes of both v1.2 DESIGN docs, and provides a grep-verified 10-item checklist (8 requirement items + 2 soundness items) covering TAINT-01..04/ORIGIN-01..02/CONFIRM-01..04"
    requirement: "PROC-01"
    verification:
      - kind: other
        ref: "grep -c 'sha256' / grep -oE '[a-f0-9]{64}' / grep -c 'Grep matched:' against planning-docs/DESIGN-GATE-RECORD-v1.2.md"
        status: pass
    human_judgment: false
  - id: D2
    description: "A human reviewer (not the executing agent) performed genuine adversarial review, found a real architectural blocker (B1) plus 3 major + 3 minor findings, and all were resolved before Decision was recorded"
    verification: []
    human_judgment: true
    rationale: "Design-review soundness cannot be automated — the entire point of this checkpoint is that a human, not the agent, judges whether the DESIGN docs are actually correct, not merely grep-complete."
  - id: D3
    description: "Gate-record shows Decision: APPROVED and Gate status: UNBLOCKED, authorizing Phase 9/Phase 10 to author crates/executor and crates/brokerd code"
    requirement: "PROC-01"
    verification:
      - kind: other
        ref: "grep -c 'Decision: APPROVED' / grep -c 'UNBLOCKED' planning-docs/DESIGN-GATE-RECORD-v1.2.md"
        status: pass
    human_judgment: true
    rationale: "The Decision/Gate-status values themselves are the human's judgment call, recorded verbatim from their explicit approval — not inferred."

duration: multi-session (spanned an interruption for real adversarial review)
completed: 2026-07-06
status: complete
---

# Phase 8: Session-Trust & Confirmation Design Gate Summary

**A human adversarial review caught a genuine architectural blocker before it reached code — the draft-only session-trust deny and the I2 taint-Block mechanism composed into a dead end, and the gate correctly stopped the phase until it was fixed.**

## Performance

- **Duration:** multi-session (interrupted for real human review between round 1 and round 2)
- **Tasks:** 2/2 (gate-record authoring, human-verify checkpoint)
- **Files modified:** 4 (2 DESIGN docs, 1 requirements doc, 1 new gate-record)

## Accomplishments

- Produced `planning-docs/DESIGN-GATE-RECORD-v1.2.md`, sha256-pinning both v1.2 DESIGN docs and providing a grep-verified 10-item checklist, mirroring the v1.0 Phase 2 `DESIGN-GATE-RECORD.md` round-1/round-2/APPROVED precedent exactly.
- The human-verify checkpoint (Task 2) functioned as designed: the executing agent explicitly refused to auto-approve it (overriding the ambiguous `gate="blocking"` vs `gate="blocking-human"` string match in the executor's own auto-mode heuristics), and the orchestrator disabled the auto-chain flag before dispatch specifically to guarantee this.
- The human reviewer's round-1 adversarial read found a genuine, code-verified blocker (B1): Step 0.5's placement made ACC-01/ACC-02 unsatisfiable and would have broken the v1.1 §9 live acceptance test. This is exactly the failure mode the design-gate discipline exists to catch before Phase 9/10 write code against a defective spec.
- All round-1 findings (B1 blocker; M1/M2/M3 major; m1/m2/m3 minor) were resolved with targeted doc revisions, re-verified by grep, and the gate-record was updated to round 2 with fresh sha256 hashes.
- Round 2 was reviewed and explicitly approved by the human reviewer (Ben Lamm), who recorded `Decision: APPROVED` / `Gate status: UNBLOCKED` themselves — never inferred or set by the executing agent or orchestrator.

## Task Commits

1. **Task 1 (round 1): Author gate-record front matter, sha256-pinning table, grep-prefilled checklist** — `9011a38` (superseded — see below)
2. **Task 2: Human review checkpoint** — reached correctly, returned `## CHECKPOINT REACHED` without auto-approving
3. **Round-1 human review** (authored by Ben Lamm, not this plan) — `43055b9`
4. **Requirements amendment (TAINT-02)** — `924aca5`
5. **B1/m1/m2 fix in DESIGN-session-trust-state.md** — `22c39ac`
6. **M1/M2/M3/m3 fix in DESIGN-confirmation-release.md** — `227ae7d`
7. **Round-2 gate-record authored (re-hashed, Decision pending)** — `bd4aac1`
8. **Decision: APPROVED / Gate status: UNBLOCKED recorded** — `8834db7`

_Note on deviation from the standard single-worktree flow: Task 1's original output (commit `9011a38`, in worktree branch `worktree-agent-acfce88338a165ed5`) pinned sha256 hashes of the round-1 (buggy) DESIGN docs. Once round-1 review required revising those already-merged docs, that worktree's content was superseded — the round-2 gate-record was authored directly against main's corrected docs instead of reconciling two diverged copies. The worktree/branch itself was left in place (not force-deleted) pending the user's own cleanup._

## Files Created/Modified

- `planning-docs/DESIGN-GATE-RECORD-v1.2.md` - v1.2 gate-record artifact; round-1 + round-2 history, checklist, Decision: APPROVED, Gate status: UNBLOCKED
- `planning-docs/DESIGN-session-trust-state.md` - §8/§9/§11 revised: I2-Block-precedence fix (B1), exhaustive-match Step 0.5 (m1), named TAINT-03 test vehicle (m2)
- `planning-docs/DESIGN-confirmation-release.md` - resolved-snapshot PendingConfirmation schema (M1), `caprun deny` verb + non-interactive output (M2), at-most-once + failure event (M3), redaction interplay (m3)
- `.planning/REQUIREMENTS.md` - TAINT-02 amended with explicit precedence wording, flagged with a dated note

## Decisions Made

See `key-decisions` in frontmatter above — the central decision was resolving the I1/I0-vs-I2 precedence ambiguity in favor of I2 Block, per the human reviewer's directed fix, with the requirement text itself amended rather than silently reinterpreted.

## Deviations from Plan

### Auto-fixed Issues

**1. Round-1 NEEDS REVISION — architectural precedence bug (B1) found by human review**
- **Found during:** Task 2 (human-verify checkpoint), by the human reviewer's own adversarial read against live code
- **Issue:** Step 0.5 (draft-only class deny) ran before the per-arg I2 taint loop, so a `Draft` session's tainted routing-sensitive arg was denied at Step 0.5 and never reached the I2 Block — making ACC-01/ACC-02 unsatisfiable and breaking the v1.1 §9 live test.
- **Fix:** Reordered so the I2 per-arg loop's Block always takes precedence; Step 0.5 only fires when the loop completes with no Block. TAINT-02 amended to state this precedence explicitly.
- **Files modified:** `planning-docs/DESIGN-session-trust-state.md`, `.planning/REQUIREMENTS.md`
- **Verification:** Grep-verified against the corrected doc; re-hashed; human reviewer re-read the amended sections and confirmed the fix closes the loophole without introducing a new one.
- **Committed in:** `924aca5`, `22c39ac`

**2. M1/M2/M3/m3 — three major + one minor gap in the confirmation-release schema/CLI/failure-mode spec**
- **Found during:** Same round-1 human review pass
- **Issue:** `PendingConfirmation.plan_node: PlanNode` couldn't carry the literal/taint/provenance data its own field-purpose table promised; no `deny` CLI verb existed despite the mock output showing one; a confirm-recorded-but-invocation-failed state was undefined; redaction of a blocked literal didn't cover the new side table.
- **Fix:** Resolved-snapshot schema, added `caprun deny`, explicit at-most-once semantics + failure event + exit code, explicit redaction-interplay MUST.
- **Files modified:** `planning-docs/DESIGN-confirmation-release.md`
- **Verification:** Grep-verified; re-hashed; human reviewer re-read and approved.
- **Committed in:** `227ae7d`

---

**Total deviations:** 2 auto-fixed groups (1 blocker + 3 major + 3 minor findings, all from one human review pass). **Impact on plan:** all fixes were required corrections to a genuine pre-implementation design defect — no scope creep; this is exactly what the design-gate discipline is for.

## Issues Encountered

The 08-03 executor's checkpoint correctly refused to auto-approve despite the ambient `--auto` chain flag, because the orchestrator explicitly disabled `workflow._auto_chain_active` before dispatching Wave 2 and instructed the executor to treat the `gate="blocking"` checkpoint identically to `gate="blocking-human"` — closing a real gap where the executor's own auto-mode heuristic (which checks for the exact string `gate="blocking-human"`) would otherwise have silently auto-approved a security design gate under `--auto`. This gap in `gsd-executor.md`'s auto-approve heuristic (string-matching `gate="blocking-human"` rather than treating any `checkpoint:human-verify` with a non-empty `gate` attribute as non-auto-approvable) is worth fixing at the tooling level in a future session.

## User Setup Required

None — no external service configuration required.
