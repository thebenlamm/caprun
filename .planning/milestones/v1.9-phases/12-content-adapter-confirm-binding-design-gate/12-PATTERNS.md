# Phase 12: Content, Adapter & Confirm-Binding Design Gate - Pattern Map

**Mapped:** 2026-07-07
**Files analyzed:** 2 (DESIGN doc(s) + gate record) — doc-only phase, no source files created/modified
**Analogs found:** 2 / 2

## File Classification

This phase produces documentation only (`planning-docs/*.md`), not source code — role/data-flow classification is repurposed as "doc type."

| New Doc | Doc Type | Closest Analog(s) | Match Quality |
|---------|----------|--------------------|----------------|
| `planning-docs/DESIGN-content-adapter-mediation.md` (or split, per Claude's Discretion) | design-doc (security-invariant spec) | `planning-docs/DESIGN-taint-model.md` (v1.0), `planning-docs/DESIGN-confirmation-release.md` (v1.2) | exact — same genre: MUST/MUST NOT invariant doc gating TCB code |
| `planning-docs/DESIGN-confirm-binding.md` (if split out) | design-doc (mechanism-extension spec) | `planning-docs/DESIGN-confirmation-release.md` (v1.2) | exact — CONFIRM-03 literally extends this doc's `PendingConfirmation` mechanism |
| `planning-docs/DESIGN-GATE-RECORD-v1.3.md` | gate-record (adversarial review + approval ledger) | `planning-docs/DESIGN-GATE-RECORD-v1.2.md`, `planning-docs/DESIGN-REVIEW-v1.2-round1.md` | exact — same convention explicitly named in D-13 |

## Pattern Assignments

### `planning-docs/DESIGN-content-adapter-mediation.md` / `DESIGN-confirm-binding.md`

**Analogs:** `planning-docs/DESIGN-taint-model.md`, `planning-docs/DESIGN-confirmation-release.md`, `planning-docs/DESIGN-session-trust-state.md`

**Heading hierarchy to copy** (from `DESIGN-confirmation-release.md`, lines 1-17 title/intro, then `## `-level sections at lines 17, 47, 122, 190, 222, 290, 317, 334, 352/354, 378, 399):
```
# DESIGN-<topic>.md — <Project> <Topic> (v<milestone>)
## The Problem Being Solved
## <Schema/Data-shape section, e.g. "PendingConfirmation Schema">
## <Core Decision Logic section>
## <Explicit "MUST NOT" reinvocation/composition section>   <- see below, load-bearing pattern
## <CLI/external-contract section, if applicable>
## Single-Shot Release Semantics (CONFIRM-02)
## Durable-Deny Semantics (CONFIRM-03)
## TCB-Residency (CONFIRM-04)
## Relationship to <other DESIGN doc> & Done-When
### Two Independent Mechanisms
### Done-When Predicate
## Accepted Residual Risks
```
`DESIGN-taint-model.md` uses the same discipline with different section names (`## Invariant Statements`, `## Threat Model — I1 Attack Surface`, `## Genuine-Taint Requirement & I0 Acceptance Predicate`, `### I0 Acceptance Predicate (Done When)`) — the pattern to copy is: **every doc ends in a named, falsifiable "Done When"/"Acceptance Predicate" section**, not prose that trails off.

**MUST/MUST NOT density pattern** (this is checked by the gate record, not stylistic): `DESIGN-session-trust-state.md` and `DESIGN-confirmation-release.md` each have `grep -c 'MUST'` counts in the 40s-70s range (see gate record line 136-137: 74 and 46 respectively). New doc(s) for Phase 12 should hit comparable density — every invariant (D-01 through D-22) stated as a hard `MUST`/`MUST NOT`, never "should"/"generally".

**Explicit "MUST NOT re-invoke" pattern to copy verbatim-in-spirit** (`DESIGN-confirmation-release.md:190`, section `## Confirm MUST NOT Re-Invoke submit_plan_node`) — this is EXACTLY the shape D-02/D-17 need for the collect-then-Block set: a named section stating precisely what the confirm/release path is forbidden from doing, so a reviewer can grep for it. Model the new doc's "single-shot confirms/denies the WHOLE blocked-arg set" rule (D-17) on this section's structure.

**Precedence-explicit pattern** (the fix for the historical B1 bug, `DESIGN-session-trust-state.md` §8 per gate record line 79-83): state precedence as an explicit named subsection — `grep -c 'precedence'` was checked as a completeness gate (6 hits). Phase 12's new doc MUST have an equally explicit "Precedence" subsection resolving D-02 (routing-block vs. body-block never silently pre-empting).

**Schema-extension pattern to copy** (`crates/brokerd/src/confirmation.rs:38-48`, `ResolvedArg` struct + its "frozen at Block time" doc-comments):
```rust
pub struct ResolvedArg {
    pub name: String,
    pub value_id: runtime_core::plan_node::ValueId,
    pub literal: String,               // frozen at Block time
    pub taint: Vec<runtime_core::plan_node::TaintLabel>,
    pub provenance_chain: Vec<uuid::Uuid>,
}
```
CONFIRM-03's design doc should describe the analogous extension (a combined-digest field alongside `resolved_args: Vec<ResolvedArg>` in `PendingConfirmation`, `confirmation.rs:101-124`) using this exact "frozen at Block time, never re-derived" doc-comment convention.

**Illustrative-code-not-literal-code convention** (`DESIGN-confirmation-release.md`'s own note, echoed in `confirmation.rs:34-36`: "This whole record serializes... it is safe to extend beyond the design doc's illustrative struct") — new DESIGN doc(s) should mark example Rust snippets as "illustrative shape, not literal code to paste," exactly as RESEARCH.md's Pattern 1 snippet already does (12-RESEARCH.md lines 146-149).

---

### `planning-docs/DESIGN-GATE-RECORD-v1.3.md`

**Analog:** `planning-docs/DESIGN-GATE-RECORD-v1.2.md` (primary), `planning-docs/DESIGN-REVIEW-v1.2-round1.md` (round-1 finding format)

**Top-level structure to copy exactly** (from `DESIGN-GATE-RECORD-v1.2.md`):
```
# DESIGN Gate Record — v<milestone>
**Date / Reviewer / Phase / Review round**
## Revision History              <- only after round 1; omit for a clean round-1 record
## Documents Under Review          <- sha256 table, see below
## Checklist                       <- one item per REQ-ID, grep-verified
## How to Verify (Human Review Steps)
## Decision
## Gate status
```

**sha256 hash-verification table format** (`DESIGN-GATE-RECORD-v1.2.md:50-61`):
```markdown
| Document | sha256 |
|----------|--------|
| `planning-docs/DESIGN-session-trust-state.md` | `9b87bfc572eb5039787ba2c27e23dae8a2a9256527123931e371901c09cb6e0a` |
| `planning-docs/DESIGN-confirmation-release.md` | `3b8cc549d7e278fb8f64c0afd2e1e7b9f25d46d9e902be19319340ab71af4450` |

Hashes were computed with `shasum -a 256` at gate-record authoring time. The reviewer MUST
re-run `shasum -a 256 <files>` and confirm the values match before setting Decision: APPROVED.
```
Reuse this exact "computed at authoring time, reviewer MUST re-run before approving" phrasing.

**Checklist item format** (`DESIGN-GATE-RECORD-v1.2.md:74-88`, one item per REQ-ID group, grep-verified, box pre-filled only if grep matched):
```markdown
- [x] **Item N — <what must be present>** (<REQ-ID(s)>)
  - Grep matched: `grep -c '<token>'` → <count>; `grep -c '<token2>'` → <count2>.
```
Phase 12's checklist must have one item per CONTENT-01/02, SMTP-01/02/03/05, CONFIRM-03 (D-13), PLUS the "Both Documents — Soundness" section pattern below.

**Soundness-vs-completeness split** (`DESIGN-GATE-RECORD-v1.2.md:113-137`, "Both Documents — Soundness" section) — this is the section that actually caught B1; it is NOT a grep-presence check but a directed adversarial re-read with a named finding trace:
```markdown
- [x] **Item 9 — <soundness property>**
  - Grep matched: `grep -ci '<phrase>'` → N in `<file>` — confirms §X/§Y's <language> is present.
- [x] **Item 10 — <the specific historical failure mode, re-verified>**
  - **<Bug-name> soundness re-check (the central round-1 finding):** §N's predicate now runs
    <exact corrected behavior> (grep: `grep -c "<token>"` → N hits); §M states explicitly that
    <exact MUST statement>. This is the fix <ACC-ID> requires — verified by content, not merely
    by the presence of the word "<keyword>".
```
Phase 12's gate record MUST include an analogous soundness item for EACH of D-12(a)/(b)/(c), each tracing to actual file/line in the DESIGN doc (not just a keyword count) — mirroring exactly how B1's fix was traced through `DESIGN-session-trust-state.md` §8/§9/§11 with quoted corrected language, not asserted abstractly.

**Round-1 finding format** (`DESIGN-REVIEW-v1.2-round1.md` headings: `## B1 (BLOCKER) — <one-line root cause>`, `## M1 (MAJOR) — ...`, `## m1 (minor) — ...`, `## What's right (keep verbatim)`, `## Suggested resolution order (blast radius)`) — if round 1 of Phase 12's review finds a blocker (expected per D-13), use this exact severity-prefixed heading convention and include a "What's right" section (don't just list problems) plus a "Suggested resolution order."

**"How to Verify" imperative-numbered-steps pattern** (`DESIGN-GATE-RECORD-v1.2.md:141-166`): 5 numbered MUST-imperative steps ending in "If satisfied: set Decision to APPROVED... If not satisfied: set Decision to NEEDS REVISION, list the gaps." Copy this structure so the checkpoint task in Phase 12's plan (D-11's mandatory stop-and-report) has an unambiguous exit test.

**Decision/Gate-status block format** (`DESIGN-GATE-RECORD-v1.2.md:170-244`) — `**Decision:** APPROVED | NEEDS REVISION` plus `**Decided:** <date> — <who, under what authorization>`, and a separate `## Gate status` section with `**`crates/X`/`crates/Y` ... is: UNBLOCKED|BLOCKED**` and an explicit `Available resolutions: [ UNBLOCKED / BLOCKED ]` line. Copy this literally — it is what phases 13-16's plans will grep for before writing code.

---

## Shared Patterns

### Adversarial-review-not-self-review discipline (D-11)
**Source:** `DESIGN-GATE-RECORD-v1.2.md` Decision-field "History of this Decision field" (lines 194-211) — records verbatim that a self-satisfied "APPROVED" was REVERTED when it turned out the executing agent, not an independent reviewer, had done the read.
**Apply to:** The Phase 12 plan's checkpoint task — the plan must not let the same session/agent that authored the DESIGN doc also set Decision: APPROVED. Model the explicit revert-and-relog behavior from this precedent if the same failure mode is caught mid-phase.

### Precedence-must-be-explicit (D-02/D-12a)
**Source:** `DESIGN-session-trust-state.md` §8 (per gate record's Item 2, `grep -ci 'precedence'` → 6)
**Apply to:** The new content-adapter DESIGN doc's section resolving routing-block vs. body-block ordering — must be a named, greppable subsection, not embedded prose.

### Illustrative-code-only-not-literal (all DESIGN docs)
**Source:** `confirmation.rs:34-36` doc comment + `DESIGN-confirmation-release.md`'s own framing
**Apply to:** Any Rust code block in the new DESIGN doc(s) — label explicitly as "shape, not literal code," matching RESEARCH.md's own Pattern 1 example convention.

## Independent Cross-Check of RESEARCH.md's Central Claim

**Confirmed, no correction.** Read directly:
- `crates/executor/src/lib.rs:62` — `for arg in &plan_node.args {` (per-arg loop, no pre-collection).
- `crates/executor/src/lib.rs:135-138` — `return ExecutorDecision::BlockedPendingConfirmation { anchor, literal: record.literal.clone() };` sits **inside** the loop body (inside the `if sink_sensitivity::is_routing_sensitive(...) && record.taint.iter().any(...)` branch, lines 99-101), so the function returns immediately on the FIRST arg satisfying routing-sensitive+tainted — it never continues scanning subsequent args in the same plan node.
- `crates/runtime-core/src/executor_decision.rs:108-129` — `SinkBlockedAnchor` fields are singular: `pub arg: String` (line 115), `pub value_id: ...` (line 117), `pub literal_sha256: String` (implied by the digest comment at 118-123) — one arg per anchor by construction, not a `Vec`.
- `ExecutorDecision::BlockedPendingConfirmation { anchor: SinkBlockedAnchor, literal: String }` (executor_decision.rs:150-153) — also singular `literal: String`, confirming one blocked value per decision, matching the anchor's singularity.

This independently confirms RESEARCH.md's finding verbatim: the current code's shape is first-match-wins with a single-arg anchor, exactly the precondition for the B1-reincarnation risk (D-02/D-12a) the DESIGN doc must resolve via Pattern 1 (collect-then-Block, D-14).

## No Analog Found

None — this phase's doc genre (design-doc + gate-record) has strong, recent, directly-applicable precedent from both v1.0 and v1.2. No file/doc lacks a close match.

## Metadata

**Analog search scope:** `planning-docs/` (all DESIGN-*.md and DESIGN-GATE-RECORD*.md files), `crates/executor/src/lib.rs`, `crates/runtime-core/src/executor_decision.rs`, `crates/brokerd/src/confirmation.rs`
**Files scanned:** 6 planning docs + 3 source files (read-only, independent verification)
**Pattern extraction date:** 2026-07-07
