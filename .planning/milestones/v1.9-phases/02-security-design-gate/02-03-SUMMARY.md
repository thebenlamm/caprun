---
plan: 02-03
phase: 02-security-design-gate
status: complete
requirements:
  - REQ-design-taint-model
  - REQ-design-plan-executor
completed: 2026-06-29
---

# Plan 02-03 Summary — DESIGN Gate Record

## What was built

`planning-docs/DESIGN-GATE-RECORD.md` — the recorded, human-approved gate artifact that satisfies
ROADMAP Phase 2 Success Criterion 3 ("both DESIGN docs reviewed and approved"). It is the
documented UNBLOCK signal Phase 4 checks before authoring any `crates/executor` file.

## Outcome: APPROVED / UNBLOCKED (after one revision round)

The gate ran **two rounds**, which is the gate working as designed — it caught a real soundness
defect that the completeness checklist alone would have passed.

- **Round 1 — NEEDS REVISION** (`eee5278`): Task 1 authored the record with all 9 completeness
  checkboxes grep-pre-filled and both doc hashes pinned. At the blocking human checkpoint, two
  independent adversarial reviews converged on a disqualifying defect: an injected planner could
  **strip** taint (`taint: []`) because it authored `ValueNode { literal, taint }` directly — the
  dual of the taint-*stapling* the docs already defended against. Six required fixes were recorded.
- **Round-1 fixes applied**: `7dc2a46` (DESIGN-taint-model.md), `539def7` (DESIGN-plan-executor.md).
- **Round 2 — APPROVED** (`3334448` regenerate, then approval edit): record re-hashed to the revised
  docs, a soundness checklist item (Item 10) added, and the human reviewer (Ben Lamm) set
  Decision: APPROVED / Gate status: UNBLOCKED.

## The six fixes (round 1 → resolved round 2)

1. Broker-owned `ValueRecord { id, literal, taint, provenance_node, provenance_chain }` resolved by
   opaque `ValueId`; planner emits `PlanArg { name, value_id }` and never authors taint (closes
   taint-stripping; mirrors CaMeL's variable model).
2. `provenance_chain: Vec<EventId>` — literal→read-Event ancestry locally verifiable (not just a
   pointer).
3. Per-sink sensitivity: `routing_sensitive` (to/cc/bcc → Block) vs `content_sensitive`
   (subject/body/attachment → Tier-4 verbatim); replaces the unsafe global "content non-sensitive".
4. Taint propagates to child sessions/intents; release only via broker-owned declassification/
   endorsement logged as an audit Event (reconciles executor UX-rule 4 vs handover §4.6).
5. Literal confirmation shows raw + canonical forms (punycode/homoglyph/RTL/display-name).
6. Standing-policy patterns removed for v0 — exact-literal allowlist only.

Plus: I0 tainted-seed tag set by the trusted brokerd creation path from provenance, never
self-declared by the (possibly injected) creating agent.

## Key files

- Created: `planning-docs/DESIGN-GATE-RECORD.md` (gate artifact; Decision: APPROVED, UNBLOCKED)
- Reviewed & revised under this gate: `planning-docs/DESIGN-taint-model.md` (sha256
  `9606b1a6…a194e`), `planning-docs/DESIGN-plan-executor.md` (sha256 `7f88782b…bca2`)

## Verification

- All Task 1 acceptance-criteria greps pass; gate record carries 10 checkboxes, 2 pinned hashes
  matching the live files, Decision + Gate-status sections.
- Human checkpoint (Task 2) completed: Decision: APPROVED, Gate status: UNBLOCKED recorded.
- Prohibition held: no file exists under `crates/executor/`.

## Self-Check: PASSED

## Notes for Phase 4

`crates/executor` is authorized **only** against the revised docs pinned by the round-2 hashes. Any
later edit to either DESIGN doc invalidates this approval and requires a fresh gate round. The
implementation must satisfy the soundness criterion (Item 10): an injected planner cannot drive a
tainted value into a sensitive sink arg as Proceed.
