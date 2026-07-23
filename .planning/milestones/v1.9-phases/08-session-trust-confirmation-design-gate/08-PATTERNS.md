# Phase 8: Session-Trust & Confirmation Design Gate - Pattern Map

**Mapped:** 2026-07-01
**Files analyzed:** 3 (documentation-only phase — no code)
**Analogs found:** 3 / 3

This phase writes no code. It authors new Markdown DESIGN docs under `planning-docs/` following
the exact structural precedent of the Phase 2 design-gate pair. There is no "role/data-flow"
classification in the code sense — the table below maps each new doc to its structural analog.

## File Classification

| New File | Role | "Data Flow" (doc purpose) | Closest Analog | Match Quality |
|----------|------|---------------------------|-----------------|----------------|
| `planning-docs/DESIGN-session-trust-state.md` | design-doc (invariant spec) | I0/I1-style invariant + acceptance predicate | `planning-docs/DESIGN-taint-model.md` | exact (same doc genre: invariant statements + Acceptance Predicate + Accepted Residual Risks) |
| `planning-docs/DESIGN-confirmation-release.md` | design-doc (mechanism spec) | schema/state-machine + UX contract | `planning-docs/DESIGN-plan-executor.md` | exact (same doc genre: schema definition + decision logic + UX contract + Done-When predicate) |
| Gate-record artifact (new file, e.g. `DESIGN-GATE-RECORD-v1.2.md`, or a new round appended to `DESIGN-GATE-RECORD.md`) | gate-record (review artifact) | checklist → human review → Decision/Gate-status | `planning-docs/DESIGN-GATE-RECORD.md` (round 1 → round 2 → APPROVED) | exact — same artifact type, reuse verbatim structure |

## Pattern Assignments

### `planning-docs/DESIGN-session-trust-state.md` (design-doc, invariant spec)

**Analog:** `planning-docs/DESIGN-taint-model.md`

**Header/front-matter pattern** (lines 1-13):
```
# DESIGN-taint-model.md — AgentOS Dynamic Taint Model

**Requirement:** REQ-design-taint-model
**Status:** Draft — pending DESIGN-GATE-RECORD.md approval
**Canonical source:** `planning-docs/PLAN.md` (wins on any conflict)
**Gate:** `crates/executor` MUST NOT be created until this document and `DESIGN-plan-executor.md`
are reviewed and `planning-docs/DESIGN-GATE-RECORD.md` records decision = APPROVED.

**Prior art:** ...
```
Copy this shape exactly: `Requirement: REQ-...`, `Status: Draft — pending ... approval`,
`Canonical source`, and a `Gate:` line naming which crate/dir MUST NOT be touched until approval
(for Phase 8: `crates/executor` and `crates/brokerd` MUST NOT gain session-trust/confirmation code
until this doc + `DESIGN-confirmation-release.md` are APPROVED in the new gate-record round).

**Invariant Statements pattern** (lines 16-54): each invariant (I0, I1, I2) is stated as a single
bolded label + one-sentence MUST/MUST NOT definition, followed by bullet sub-rules, e.g.:
```
**I0 (intent/session-creation injection):** A Session whose intent text or seed derives from
external or untrusted content MUST start in draft-only status AND MUST NOT be permitted to
auto-authorize Tier 3+ effects.
```
For the new doc, restate I1's dynamic-demotion extension the same way: a bolded label
(`I1 dynamic demotion` or similar), then the MUST rule that `mint_from_read` is the sole
trust-flip site (see Pattern 2 in RESEARCH.md), never worker-declared.

**Acceptance Predicate pattern** (lines 359-376, "I0 Acceptance Predicate (Done When)"):
```
The I0 invariant is satisfied when the following predicate holds for every Session whose intent
text or seed derives from external or untrusted content:

0. **The tainted-seed determination is made by the trusted brokerd session-creation path from the
   seed's provenance** ... NOT self-declared by the agent creating the Session.
1. The Session starts in draft-only status (`Session.status == Draft`) at creation time.
2. The broker MUST reject any `submit_plan_node()` call for a Tier 3+ sink from a draft Session
   without human gate confirmation having been recorded.

All three conditions MUST hold simultaneously.
```
Mirror this numbered-condition format exactly for the new `SessionStatus::Draft` acceptance
predicate — number each condition starting at 0 if there is a "who sets this, and never
self-declared" trust condition (there is one here per RESEARCH.md Pattern 2 / A2), and close with
"All N conditions MUST hold simultaneously."

**Accepted Residual Risks pattern** (lines 276-314): each risk gets a bolded numbered heading,
a description paragraph, and an `*Accepted for v0:*` paragraph naming the mitigation and what is
deferred. Use this exact shape for any I1/I0-extension residual risk the new doc surfaces (e.g.
"session-demotion race between mint_from_read and a concurrent submit_plan_node").

**Genuine-anchor / anti-self-declaration phrasing to reuse verbatim style** (lines 34-40, 364-367):
> "The tainted-seed tag MUST be set by the trusted session-creation path from provenance — never
> self-declared by the creating agent."

Apply the identical phrasing pattern to I1: "the session's Draft transition on `mint_from_read` is
set by the same trusted broker function that mints the tainted `ValueRecord` — never by the
worker, never by a flag the worker's IPC message could carry" (already drafted in RESEARCH.md
Pattern 2 — copy that sentence near-verbatim into the new doc).

---

### `planning-docs/DESIGN-confirmation-release.md` (design-doc, mechanism spec)

**Analog:** `planning-docs/DESIGN-plan-executor.md`

**Section skeleton to mirror** (headings found via grep, lines noted):
```
## The Problem Being Solved                         (line 17)
## Reconciliation with PLAN.md (the locked API shape) (line 82)
## ValueRecord & ValueId Handle Model                (line 111)   -> analog: PendingConfirmation record shape
## PlanNode Schema                                    (line 168)   -> analog: unchanged, referenced not redefined
## Plan DAG Structure                                 (line 208)
## Sink Sensitivity Map (v0: hardcoded)                (line 232)   -> analog: sink_effect_class table
## Taint Propagation Rules                             (line 288)
## Executor Decision Logic                             (line 326)   -> analog: Step 0.5 draft-only deny placement
## Taint Provenance Requirement                        (line 390)
## Literal-Value Confirmation UX                       (line 446)   -> analog: confirm CLI UX contract
## Relationship to Broker's Callability Gate & Done-When (line 536)
### Two Independent Gates                              (line 538)
### Done-When Predicate                                (line 562)
```
For `DESIGN-confirmation-release.md`, use the same skeleton shape but retarget each section to the
confirmation mechanism: "The Problem Being Solved" (pause/resume across process boundary),
a `PendingConfirmation` schema section (mirroring the `ValueNode Schema` / `ValueRecord` section's
field-by-field table format), a "Confirmation Decision Logic" section mirroring "Executor Decision
Logic" (ordered numbered steps: lookup by effect_id → check terminal state → confirm/deny →
re-invoke sink), and a final "Done-When Predicate" section in the same style as line 562.

**Schema-as-table pattern to copy** (used for `ValueNode`/`PlanNode` — apply to `PendingConfirmation`):
grep shows these schemas are defined as a Rust-struct-like block followed by a field table
(`literal`, `provenance`, `taint` each with a Purpose column, matching the Worker Output Contract
table style in `DESIGN-taint-model.md` lines 138-145). Define `PendingConfirmation` the same way:
a field table with columns `Field | Purpose`, listing `effect_id`, `session_id`, `plan_node` (full
resolved arg set), `state` (`Pending | Confirmed | Denied`).

**"Literal-Value Confirmation UX" section is the direct analog for the `caprun confirm` CLI
contract** (line 446) — copy its structure: exact prompt/output format specified verbatim, not
described abstractly, plus explicit denial/duplicate-confirm behavior.

**MUST/MUST NOT phrasing density:** `DESIGN-plan-executor.md` uses "MUST"/"MUST NOT" as the sole
normative verbs throughout every schema and rule section (never "should"/"needs to"). The gate
checklist in `DESIGN-GATE-RECORD.md` literally greps for this density
(`grep -c "MUST\|MUST NOT"`). The new doc must match this density — every rule in the
"Confirmation Decision Logic" and PendingConfirmation sections stated as MUST/MUST NOT.

---

### Gate-record artifact (new file or new round)

**Analog:** `planning-docs/DESIGN-GATE-RECORD.md` (round 1 → round 2 → APPROVED)

**Front-matter fields to copy exactly:**
```
**Date:** 2026-06-29
**Reviewer:** Ben Lamm
**Phase:** 02-security-design-gate — Plan 03
**Review round:** 2 (re-review of revised docs)
```
For Phase 8, use: `**Phase:** 08-session-trust-confirmation-design-gate — Plan NN`,
`**Review round:** 1` (first round for this gate; the pattern supports later re-review rounds if
NEEDS REVISION occurs).

**sha256-pinning table pattern** (lines 16-25):
```
## Documents Under Review (round N — ...)

| Document | sha256 |
|----------|--------|
| `planning-docs/DESIGN-taint-model.md` | `9606b1a6...` |
| `planning-docs/DESIGN-plan-executor.md` | `7f88782b...` |

Hashes were computed with `shasum -a 256` at gate-record authoring time. The reviewer MUST
re-run `shasum -a 256 <files>` and confirm the values match before setting Decision: APPROVED.
```
Reuse verbatim for `DESIGN-session-trust-state.md` + `DESIGN-confirmation-release.md`.

**Checklist pattern** (lines 29-119): one checkbox per requirement-mapped item, each with:
- a bold title naming the doc-section it verifies + the requirement ID it satisfies,
- a `Grep matched:` line showing the exact grep command and what it found (quoting the doc text),
- for soundness-only items (not just presence), an explicit note like line 99: "Completeness
  greps pass fully-written-but-wrong specs; this item gates *soundness*, not presence."

Map Phase 8's checklist items 1:1 to PROC-01's sub-requirements: I1 dynamic-demotion trigger
explicitly stated, I0 draft-creation rule explicitly stated (extension, not restatement),
`sink_effect_class` table specified, `PendingConfirmation` schema specified with full field list,
confirm/deny CLI contract specified, "confirm does not re-run submit_plan_node" soundness item
(mirrors round-2 Item 10's soundness-not-presence framing — see Pitfall 3 in RESEARCH.md).

**"How to Verify" 5-step human-review procedure** (lines 123-145) — copy the exact 5-step
structure: (1) confirm all checkboxes checked, (2) read doc 1 end-to-end as an attacker, (3) read
doc 2 for implementability without interpretation, (4) re-run `shasum -a 256` and compare, (5) set
Decision + Gate status, dated.

**Decision/Gate-status block pattern** (lines 149-191):
```
## Decision
**Decision:** APPROVED
**Decided:** 2026-06-29 by Ben Lamm (round 2)
...
## Gate status
> **Phase 4 MUST NOT author any `crates/executor` file until this record shows
> Decision: APPROVED and Gate status: UNBLOCKED.**
**`crates/executor` is: UNBLOCKED**
Available resolutions: [ UNBLOCKED / BLOCKED ]
```
For Phase 8's gate record, restate the blocked-until clause for the two crates this milestone's
new mechanisms touch: "Phase 9/10 MUST NOT author any `crates/executor` or `crates/brokerd` file
implementing session-trust/confirmation-release logic until this record shows Decision: APPROVED
and Gate status: UNBLOCKED." Use the same `Available resolutions: [ UNBLOCKED / BLOCKED ]` line and
leave Decision as pending (`NEEDS REVIEW` / blank) until human review, matching round-1's initial
pending state in the historical record (lines 149-153 show the pending-then-decided pattern from
the round-1/round-2 history in this same file).

## Shared Patterns

### Normative-verb discipline (MUST / MUST NOT only)
**Source:** Both `DESIGN-taint-model.md` and `DESIGN-plan-executor.md`, throughout.
**Apply to:** Both new DESIGN docs, every rule/schema statement — never "should"/"needs to". The
gate-record checklist literally greps for `MUST\|MUST NOT` counts, so low density fails the
completeness check before human review even begins.

### Trusted-path-only / anti-self-declaration framing
**Source:** `DESIGN-taint-model.md` lines 34-40 (I0) — "MUST be set by the trusted ... path from
provenance — never self-declared by the creating agent."
**Apply to:** `DESIGN-session-trust-state.md`'s I1 demotion rule (session-status flip happens
inside `mint_from_read`, never worker-reported) — same anti-spoofing structure, different trigger.

### Sole-mint-site / single-TCB-function discipline
**Source:** `DESIGN-plan-executor.md` "Executor Decision Logic" (one function returns
`ExecutorDecision::Denied`); `DenyReason` doc comment: "the ONE base denial error enum ... never
introduce a second denial error type."
**Apply to:** `DESIGN-session-trust-state.md`'s new `DenyReason` variant (append, don't duplicate
the enum) and `DESIGN-confirmation-release.md`'s confirm path (must NOT re-invoke
`submit_plan_node` — a distinct, logged endorsement path instead, per RESEARCH.md Pitfall 3).

### Declassification-as-logged-audit-Event
**Source:** `DESIGN-taint-model.md` "Declassification & Endorsement" section (lines 201-228) —
release is never a silent allowlist mutation; it is a broker-owned, TCB-resident audit Event.
**Apply to:** `DESIGN-confirmation-release.md`'s confirm-grant path — model `confirm_granted` /
`confirm_denied` as audit Events anchored to `effect_id`, exactly analogous to the existing
endorsement-Event pattern, not a new subsystem.

### Exhaustive-match discipline for new enums
**Source:** `crates/runtime-core/src/plan_node.rs` `TaintLabel::is_untrusted()` (explicit match,
no wildcard arm) — cited directly in RESEARCH.md Code Examples and Pitfall 4.
**Apply to:** Both new docs must require this discipline explicitly for any new `EffectClass` or
`DenyReason` variant they introduce — state it as a MUST in the doc text itself, not left implicit.

## No Analog Found

None — all three artifacts for this phase (two design docs + one gate record) have exact
structural analogs already approved and checked into `planning-docs/`. There is no code-role
gap because this phase produces no code.

## Metadata

**Analog search scope:** `planning-docs/` (3 files read in full: `DESIGN-taint-model.md`,
`DESIGN-plan-executor.md` headings, `DESIGN-GATE-RECORD.md`)
**Files scanned:** 3 analogs + 1 research doc (`08-RESEARCH.md`)
**Pattern extraction date:** 2026-07-01
