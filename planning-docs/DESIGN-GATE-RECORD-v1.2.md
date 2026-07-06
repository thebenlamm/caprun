# DESIGN Gate Record — v1.2

**Date:** 2026-07-02
**Reviewer:** Ben Lamm
**Phase:** 08-session-trust-confirmation-design-gate — Plan 03
**Review round:** 2 (re-review of revised docs)

## Revision History

- **Round 1 — NEEDS REVISION** (`planning-docs/DESIGN-REVIEW-v1.2-round1.md`, commit `43055b9`):
  Ben Lamm's adversarial review, checked against code on `main` (session.rs, plan_node.rs,
  quarantine.rs, server.rs, sink_schema.rs, executor_decision.rs, s9_live_block.rs,
  REQUIREMENTS.md), found **1 blocker, 3 major, 3 minor** findings:
  - **B1 (blocker):** Step 0.5 (draft-only class deny) ran *before* the per-arg I2 taint loop,
    so every hostile live run on a `Draft` session returned `Denied` at Step 0.5 and never
    reached the I2 `Block`. ACC-01/ACC-02 were unsatisfiable, `caprun confirm` was unreachable
    in any live flow, and the v1.1 §9 live test (`cli/caprun/tests/s9_live_block.rs:243-251`)
    would have broken post-Phase-9.
  - **M1 (major):** `PendingConfirmation.plan_node: PlanNode` cannot carry literal/taint/provenance
    data — `PlanNode` is opaque `ValueId` handles only.
  - **M2 (major):** the deny input mechanism was unspecified (no `caprun deny` command), and the
    mock CLI output showed an interactive `[ Confirm ] [ Deny ]` chooser contradicting the
    locked non-interactive-TTY UX decision.
  - **M3 (major):** `Confirmed`-before-invoke left an undefined "confirmed but sink invocation
    failed" terminal state with no exit-code row or audit Event for that leg.
  - **m1–m3 (minor):** Step 0.5's `SessionStatus` check was a de-facto wildcard (not exhaustive);
    TAINT-03 has no live sink to exercise (both registered sinks are `CommitIrreversible`);
    redaction of a blocked literal did not cover the new `PendingConfirmation` side table.
- **Root cause:** `planning-docs/MILESTONE-v1.2-SEED.md` (the seed doc that originated this
  milestone) proposed both "draft-only sessions deny `CommitIrreversible`" and an acceptance flow
  "demoted → Blocked → confirm" without specifying precedence between them — the round-1 DESIGN
  doc resolved the ambiguity in the direction that made the confirm path unreachable.
- **Round 1 fixes applied** (this round-2 record re-reviews the revised docs):
  - `.planning/REQUIREMENTS.md` TAINT-02 amended (one-line, flagged, commit `924aca5`): "denies
    `CommitIrreversible`-class plan nodes **that do not already Block on I2**."
  - `DESIGN-session-trust-state.md` §8/§9/§11 revised: the draft-only deny (Step 0.5) now runs
    **after** the per-arg I2 loop completes with no Block, never before; Step 0.5's predicate is
    now an exhaustive `match` over `SessionStatus` (fixes m1); TAINT-03's untestable-sink gap is
    now named explicitly as a Phase 9 fixture requirement (fixes m2); the Acceptance Predicate
    gained an explicit "I2 Block takes precedence" condition (now 6 conditions, was 5).
  - `DESIGN-confirmation-release.md` revised: `PendingConfirmation.plan_node: PlanNode` replaced
    with `sink: SinkId` + `resolved_args: Vec<ResolvedArg>` (fixes M1); CLI contract gained a
    `caprun deny <effect_id>` command and the interactive chooser was removed from the mock output
    (fixes M2); Step 4a gained an explicit at-most-once statement, a `sink_invocation_failed`
    audit Event, and a dedicated exit-code row (fixes M3); the redaction-interplay gap between
    `blocked_literals` and `PendingConfirmation.resolved_args` is now stated as a MUST (fixes m3);
    "Two Independent Mechanisms" gained an explicit statement that a confirm-on-Draft-Block IS the
    I0/I1 human gate, not an incidental interaction.

## Documents Under Review (round 2 — revised)

| Document | sha256 |
|----------|--------|
| `planning-docs/DESIGN-session-trust-state.md` | `9b87bfc572eb5039787ba2c27e23dae8a2a9256527123931e371901c09cb6e0a` |
| `planning-docs/DESIGN-confirmation-release.md` | `3b8cc549d7e278fb8f64c0afd2e1e7b9f25d46d9e902be19319340ab71af4450` |

Hashes were computed with `shasum -a 256` at gate-record authoring time. The reviewer MUST
re-run `shasum -a 256 planning-docs/DESIGN-session-trust-state.md planning-docs/DESIGN-confirmation-release.md`
and confirm the values match before setting Decision: APPROVED. Per the review's own note, round 2
re-review may scope to the amended sections (§8/§9/§11 of doc 1; the Schema/CLI-contract/Step
4a/"Two Independent Mechanisms" sections of doc 2) rather than the full docs.

---

## Checklist

Each item maps one-to-one to a TAINT/ORIGIN/CONFIRM requirement in `.planning/REQUIREMENTS.md`.
Boxes are pre-filled by grep: a box is checked only if the corresponding grep matched the target
document. Unchecked items indicate missing required content — the doc must be revised before
approval.

### DESIGN-session-trust-state.md

- [x] **Item 1 — `SessionStatus::Draft`, `mint_from_read` I1 trigger, and I0 seed-provenance rule
  explicitly stated** (TAINT-01, ORIGIN-01, ORIGIN-02)
  - Grep matched: `grep -c 'SessionStatus::Draft'` → 7; `grep -c 'mint_from_read'` → 14;
    `grep -ci 'seed-provenance\|seed provenance'` → 5.

- [x] **Item 2 — draft-only `CommitIrreversible` denial via `sink_effect_class`, Step 0.5 placed
  AFTER the per-arg I2 loop, new `DenyReason` variant** (TAINT-02, amended)
  - Grep matched: `grep -c 'sink_effect_class'` → 13; `grep -c 'DraftOnlySessionDeniesCommitIrreversible'`
    → 3; `grep -c "Step 0.5"` → 14; `grep -ci 'precedence'` → 6 — confirms §8's "Precedence rule
    (MUST, amended per DESIGN-REVIEW-v1.2-round1.md B1)" is present, not just the old ordering.

- [x] **Item 3 — `MutateReversible`/`Observe` still allowed on `Draft` sessions (TAINT-03), with a
  named test vehicle for the untestable-sink gap** (TAINT-03, m2 fix)
  - Grep matched: `grep -c 'TAINT-03'` → 4 — includes §9's "Phase 9's verifier MUST exercise it via
    a test-only sink" fixture requirement.

- [x] **Item 4 — session demotion recorded as an audit event with a causal edge to the triggering
  read event** (TAINT-04)
  - Grep matched: `grep -c 'session_demoted'` → 6; `grep -c 'parent_id'` → 6.

### DESIGN-confirmation-release.md

- [x] **Item 5 — `caprun confirm` CLI contract with exact output format (verbatim literal +
  provenance)** (CONFIRM-01)
  - Grep matched: `grep -c 'Literal value:'` → 1 — confirms the exact terminal output block is
    present.

- [x] **Item 6 — single-shot `(sink, arg, literal-digest)` release, explicit no-standing-policy
  statement** (CONFIRM-02)
  - Grep matched: `grep -c 'Single-Shot Release Semantics'` → 1;
    `grep -ci 'no standing policy\|standing policy'` → 3.

- [x] **Item 7 — durable deny, persisted terminal-state check, no re-confirmation** (CONFIRM-03)
  - Grep matched: `grep -c 'Durable-Deny Semantics'` → 1.

- [x] **Item 8 — confirm/deny audited and anchored to `effect_id` (same key as `SinkBlockedAnchor`),
  TCB-resident release path** (CONFIRM-04)
  - Grep matched: `grep -c 'TCB-Residency'` → 1; `grep -c 'SinkBlockedAnchor.effect_id'` → 3.

### Both Documents — Soundness (round 1 + round 2)

Completeness greps pass fully-written-but-wrong specs; these two items gate *soundness*, not
presence — this is the section that caught B1 in round 1 and must be re-checked, not skipped, now
that the fix is applied.

- [x] **Item 9 — trust state cannot be self-declared by the worker or IPC**
  - Grep matched: `grep -ci 'never self-declared\|self-declaration\|never worker-declared'` → 7 in
    `DESIGN-session-trust-state.md` — confirms §2/§4/§11 condition 0's anti-self-declaration
    language.

- [x] **Item 10 — confirm cannot re-invoke `submit_plan_node`, and the frozen checkpoint carries a
  full resolved arg set (not just the blocked arg)** (fixes M1, verified soundness of the B1 fix)
  - Grep matched: `grep -c "MUST NOT.*submit_plan_node"` → 2 in `DESIGN-confirmation-release.md`;
    `grep -c 'resolved_args'` → 5 — confirms the `PlanNode`-cannot-carry-literals defect (M1) is
    actually fixed with a resolved-snapshot type, not merely renamed.
  - **B1 soundness re-check (the central round-1 finding):** §8's predicate now runs the class
    check only "after the per-arg loop completes with NO Block" (grep: `grep -c "Step 0.5"` → 14
    hits spanning both the corrected placement description and the code comment); §11 condition 4
    states explicitly that a tainted routing-sensitive arg "MUST return `BlockedPendingConfirmation`
    ... regardless of `session_status` — including on a `Draft` session." This is the fix ACC-01/
    ACC-02 require — verified by content, not merely by the presence of the word "precedence."

MUST/MUST NOT density: `grep -c 'MUST'` → 74 (`DESIGN-session-trust-state.md`), 46
(`DESIGN-confirmation-release.md`) — comparable to the v1.0 analog docs' density.

---

## How to Verify (Human Review Steps)

Before setting Decision and Gate status, the reviewer MUST:

1. **Confirm all ten checklist boxes are checked.** If any box is unchecked, the corresponding doc
   is incomplete — it must be revised before approval.

2. **Re-read the amended sections of `planning-docs/DESIGN-session-trust-state.md` as an attacker**
   (§8, §9, §11): does the corrected ordering (I2 Block takes precedence over the class-level
   draft-only deny) actually close B1, or does it open a new loophole (e.g., a way to make the
   per-arg loop falsely report "no Block" for a genuinely tainted routing-sensitive arg)? Confirm
   every rule reads as a hard MUST/MUST NOT, not "should".

3. **Re-read the amended sections of `planning-docs/DESIGN-confirmation-release.md`** (PendingConfirmation
   Schema, Confirmation Decision Logic Step 4a, caprun confirm CLI Contract, Two Independent
   Mechanisms): confirm the `resolved_args` snapshot, the `caprun deny` verb, the at-most-once
   statement + `sink_invocation_failed` Event, and the Draft-Block-is-the-human-gate statement are
   all unambiguous enough to implement `crates/executor`/`crates/brokerd` changes without
   interpretation.

4. **Confirm the two sha256 hashes match the current files:** run
   `shasum -a 256 planning-docs/DESIGN-session-trust-state.md planning-docs/DESIGN-confirmation-release.md`
   and compare the output to the values in the "Documents Under Review" table above.

5. **If satisfied:** set Decision to APPROVED and Gate status to UNBLOCKED (below), dated.
   **If not satisfied:** set Decision to NEEDS REVISION, list the gaps, and the phase loops again.

---

## Decision

> **Round 2 — pending human re-review.** Round 1 was NEEDS REVISION (1 blocker, 3 major, 3 minor —
> see Revision History above); all required fixes were applied (commits recorded above) and the
> revised docs are re-hashed above. The reviewer sets one of the two values below after re-reading
> the amended sections.

**Round-1 fixes — verification status (all applied, pending reviewer re-confirmation):**

| # | Required fix (round 1) | Status |
|---|------------------------|--------|
| B1 | I2 per-arg Block takes precedence over the class-level draft-only deny; Step 0.5 moved to run after the loop; TAINT-02 amended | applied |
| M1 | `PendingConfirmation` carries a resolved-snapshot (`sink` + `resolved_args: Vec<ResolvedArg>`), not a bare `PlanNode` | applied |
| M2 | `caprun deny <effect_id>` added as a distinct command; interactive `[ Confirm ] [ Deny ]` chooser removed from mock output | applied |
| M3 | At-most-once semantics stated explicitly; `sink_invocation_failed` Event + dedicated exit-code row added | applied |
| m1 | Step 0.5 predicate is now an exhaustive `match` over `SessionStatus` | applied |
| m2 | TAINT-03's test vehicle (test-only sink) named explicitly for Phase 9 | applied |
| m3 | Redaction of `blocked_literals` MUST also cover `PendingConfirmation.resolved_args` | applied |

**Decision:** APPROVED
**Decided:** 2026-07-06 by Ben Lamm (round 2)

Round-1 NEEDS REVISION findings (B1 blocker + M1/M2/M3 major + m1/m2/m3 minor) are all resolved in
the revised docs re-hashed above. The reviewer re-read the amended sections (§8/§9/§11 of
`DESIGN-session-trust-state.md`; PendingConfirmation Schema, Confirmation Decision Logic, CLI
Contract, and Two Independent Mechanisms of `DESIGN-confirmation-release.md`) and confirmed the B1
fix (I2 Block precedence) closes the round-1 defect without introducing a new loophole. Approval is
pinned to the round-2 sha256 hashes above; a later edit to either DESIGN doc invalidates this
approval and requires a fresh gate round.

> Prior round-1 decision: NEEDS REVISION (superseded by this round-2 APPROVED).

---

## Gate status

> **Phase 9 and Phase 10 MUST NOT author any `crates/executor` or `crates/brokerd` file
> implementing session-trust or confirmation-release logic until this record shows
> Decision: APPROVED and Gate status: UNBLOCKED.**

**`crates/executor` / `crates/brokerd` (session-trust + confirmation-release additions) is: UNBLOCKED**

Available resolutions: [ UNBLOCKED / BLOCKED ]

Decision: APPROVED is recorded above (round 2, 2026-07-06). Phase 9 and Phase 10 are authorized to
author `crates/executor`/`crates/brokerd` code against the revised DESIGN docs pinned by the
round-2 hashes. The prior round-1 gate state was BLOCKED; it is now UNBLOCKED.
