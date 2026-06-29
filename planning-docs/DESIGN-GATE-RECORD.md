# DESIGN Gate Record

**Date:** 2026-06-29
**Reviewer:** Ben Lamm
**Phase:** 02-security-design-gate — Plan 03
**Review round:** 2 (re-review of revised docs)

## Revision History

- **Round 1 — NEEDS REVISION** (commit `eee5278`): two independent adversarial reviews converged
  on a disqualifying defect — an injected planner could *strip* taint (`taint: []`) because the
  planner authored `ValueNode { literal, taint }` directly. Six required fixes recorded.
- **Round 1 fixes applied:** DESIGN-taint-model.md revised in `7dc2a46`; DESIGN-plan-executor.md
  revised in `539def7`. Both docs re-hashed below. This round-2 record re-reviews the revised docs.

## Documents Under Review (round 2 — revised)

| Document | sha256 |
|----------|--------|
| `planning-docs/DESIGN-taint-model.md` | `9606b1a6a5106644f4a59e300c4a0cdd2bb552c27025de568edd77a5d56a194e` |
| `planning-docs/DESIGN-plan-executor.md` | `7f88782b6217f52630b9c0c8e0d40949e3434c9d50a250c18dde0ff256c7bca2` |

Hashes were computed with `shasum -a 256` at gate-record authoring time. The reviewer MUST
re-run `shasum -a 256 planning-docs/DESIGN-taint-model.md planning-docs/DESIGN-plan-executor.md`
and confirm the values match before setting Decision: APPROVED.

---

## Checklist

Each item maps one-to-one to a REQ-design-* done-when criterion (see
`.planning/phases/02-security-design-gate/02-RESEARCH.md`, Phase Requirements → Review Map).
Boxes are pre-filled by grep: a box is checked only if the corresponding grep matched the
target document. Unchecked items indicate missing required content — the doc must be revised
before approval.

### DESIGN-taint-model.md

- [x] **Item 1 — Dynamic-taint default explicitly stated** (REQ-design-taint-model)
  - Grep matched: `grep -i "dynamic.taint" planning-docs/DESIGN-taint-model.md` — confirmed by
    section heading "Default Taint Model — Dynamic Taint" and invariant statement: "Default
    enforcement: **dynamic taint** — any context that reads raw untrusted bytes becomes tainted
    and MUST operate in draft-only mode."

- [x] **Item 2 — Hard planner/worker split for Tier 3+ explicitly stated** (REQ-design-taint-model)
  - Grep matched: `grep -iE "hard.*planner.*worker.*split|planner.*worker.*split.*Tier 3"` —
    confirmed by "High-Risk Mode — Hard Planner/Worker Split (Tier 3+)" section and: "The hard
    planner/worker split is RESERVED for Tier 3+ tasks only."

- [x] **Item 3 — I0 draft-only rule for tainted-seed Sessions explicitly stated** (REQ-design-taint-model)
  - Grep matched: `grep -i "draft-only" planning-docs/DESIGN-taint-model.md` — confirmed by I0
    invariant statement: "A Session whose intent text or seed derives from external or untrusted
    content MUST start in draft-only status AND MUST NOT be permitted to auto-authorize Tier 3+
    effects."

### DESIGN-plan-executor.md

- [x] **Item 4 — ValueNode (literal + provenance + taint) specified** (REQ-design-plan-executor)
  - Grep matched: `grep -i "ValueNode" planning-docs/DESIGN-plan-executor.md` — confirmed by
    "ValueNode Schema" section specifying all three fields: `literal` (actual runtime value),
    `provenance` (EventId of read Event), and `taint` (list of taint labels).

- [x] **Item 5 — PlanNode (sink + args) specified** (REQ-design-plan-executor)
  - Grep matched: `grep -i "PlanNode" planning-docs/DESIGN-plan-executor.md` — confirmed by
    "PlanNode Schema" section specifying: `sink` (SinkId) and `args` (list of named ValueNodes),
    plus the locked broker API shape `submit_plan_node(session_id, plan_node) -> ExecutorDecision`.

- [x] **Item 6 — v0 hardcoded sink sensitivity map specified** (REQ-design-plan-executor)
  - Grep matched: `grep -iE "hardcoded|v0.*sink.*sensitivity"` — confirmed by "Sink Sensitivity
    Map (v0: hardcoded)" section: `email.send` with `to`, `cc`, `bcc` as sensitive args;
    `subject`, `body` as non-sensitive; explicitly hardcoded in Rust with no Cedar/schema system.

- [x] **Item 7 — Monotonic taint propagation through plan DAG specified** (REQ-design-plan-executor)
  - Grep matched: `grep -iE "monotonic|taint.*propagat"` — confirmed by "Taint Propagation
    Rules" section with five explicit rules: (1) Monotonic — labels never removed; (2)
    Per-dataflow-edge; (3) Originates at read Events; (4) No provenance-less taint; (5) No
    executor-side taint injection.

- [x] **Item 8 — Literal-value confirmation UX specified** (REQ-design-plan-executor)
  - Grep matched: `grep -iE "Literal-Value Confirmation"` — confirmed by "Literal-Value
    Confirmation UX" section specifying the exact v0 prompt format: "Proposed recipient
    `<literal>` came from untrusted content. Confirm this exact address to proceed." with
    `[ Confirm: <literal> ]  [ Deny ]` buttons delivered via FAMP.

### Both Documents

- [x] **Item 9 — Genuine-taint requirement acknowledged: taint originates from a read Event,
  never hand-set** (REQ-design-plan-executor / REQ-design-taint-model)
  - Grep matched in DESIGN-taint-model.md: `grep -i "read Event"` and `grep -iE "hand-set"` —
    confirmed by "Genuine-Taint Requirement" section: "Taint MUST originate from a read Event
    recorded in the audit DAG. Taint MUST NOT be hand-set at the sink."
  - Grep matched in DESIGN-plan-executor.md: `grep -i "read Event"` and `grep -iE "hand-set"` —
    confirmed by "Taint Provenance Requirement" section: "Taint MUST originate from a read Event
    recorded in the audit DAG" and "A ValueNode whose taint labels were hand-set at the sink call
    site is invalid."

### Soundness (round 2 — added after round-1 review)

Completeness greps pass fully-written-but-wrong specs; this item gates *soundness*, not presence.

- [x] **Item 10 — An injected/compromised planner cannot drive a tainted value into a sensitive
  sink arg as Proceed** (soundness criterion; satisfies the round-1 central finding)
  - The planner no longer authors taint. Verified by content of the revised docs:
    - `grep -cE "ValueRecord|ValueId|PlanArg" DESIGN-plan-executor.md` → 45 matches: planner emits
      `PlanArg { name, value_id }`; broker-owned `ValueRecord { id, literal, taint, provenance_node,
      provenance_chain }` resolved from a trusted store; executor dereferences by `ValueId`
      (dangling handle → Block). Closes taint-**stripping** (false negatives), the dual of the
      taint-**stapling** the docs already closed.
    - `grep -c "provenance_chain"` → 17 (executor) / 5 (taint-model): `Vec<EventId>` derivation
      edges make literal→read-Event ancestry locally verifiable.
    - Sink args split `routing_sensitive` (to/cc/bcc → Block) vs `content_sensitive`
      (subject/body/attachment → Tier-4 verbatim); per-sink rule replaces the global
      "content non-sensitive" principle.
    - Taint propagates to child sessions/intents; release only via broker-owned
      **declassification/endorsement logged as an audit Event** (reconciles UX-rule 4 vs handover §4.6).
    - Literal confirmation shows raw **and** canonical forms (punycode/homoglyph/RTL/display-name).
    - Standing-policy **patterns removed for v0** — exact-literal allowlist only.
    - I0 tainted-seed tag set by the **trusted brokerd creation path from provenance**, never
      self-declared by the creating agent.

---

## How to Verify (Human Review Steps)

Before setting Decision and Gate status, the reviewer MUST:

1. **Confirm all nine checklist boxes are checked.** If any box is unchecked, the corresponding
   doc is incomplete — send it back to Plan 02-01 or 02-02 for revision; do NOT approve.

2. **Read `planning-docs/DESIGN-taint-model.md` end-to-end as an attacker** (adversarial review):
   can you find a loophole in the stated invariants? Confirm the dynamic-taint default, the hard
   Tier 3+ split, the I0 draft-only rule, and the genuine-taint requirement are stated as hard
   MUST/MUST NOT predicates — not "should".

3. **Read `planning-docs/DESIGN-plan-executor.md`**: confirm ValueNode (literal+provenance+taint),
   PlanNode (sink+args), the hardcoded sink sensitivity map, monotonic propagation, the
   literal-value confirmation prompt, and the taint-provenance requirement are all unambiguous
   enough to implement `crates/executor` without interpretation.

4. **Confirm the two sha256 hashes match the current files:** run
   `shasum -a 256 planning-docs/DESIGN-taint-model.md planning-docs/DESIGN-plan-executor.md`
   and compare the output to the values in the "Documents Under Review" table above.

5. **If satisfied:** set Decision to APPROVED and Gate status to UNBLOCKED (below), dated.
   **If not satisfied:** set Decision to NEEDS REVISION, list the gaps, and the phase loops.

---

## Decision

> **Round 2 — pending human re-review.** Round 1 was NEEDS REVISION; all six required fixes were
> applied (commits `7dc2a46`, `539def7`) and the revised docs are re-hashed above. The reviewer
> sets one of the two values below after re-reading the revised docs.

**Round-1 fixes — verification status (all applied):**

| # | Required fix (round 1) | Status |
|---|------------------------|--------|
| 1 | Broker-owned `ValueRecord`/`ValueId`; planner references handles, never authors taint | ✓ applied |
| 2 | `provenance_chain: Vec<EventId>` proving literal→read-Event ancestry | ✓ applied |
| 3 | Per-sink split: routing-sensitive (block) vs content-sensitive (Tier-4 verbatim) | ✓ applied |
| 4 | Taint propagates to child sessions/intents; release via logged declassification Event | ✓ applied |
| 5 | Literal confirmation shows raw + canonical (punycode/homoglyph/RTL) | ✓ applied |
| 6 | Standing-policy patterns removed for v0 — exact-literal allowlist only | ✓ applied |

**Decision:** APPROVED / NEEDS REVISION

*(Replace with the selected decision and today's date once the round-2 review is complete.)*

---

## Gate status

> **Phase 4 MUST NOT author any `crates/executor` file until this record shows
> Decision: APPROVED and Gate status: UNBLOCKED.**

**`crates/executor` is: BLOCKED**

Available resolutions: [ UNBLOCKED / BLOCKED ]

*(Set to UNBLOCKED only after Decision: APPROVED is recorded above.)*
