# DESIGN Gate Record

**Date:** 2026-06-29
**Reviewer:** Ben Lamm
**Phase:** 02-security-design-gate — Plan 03

## Documents Under Review

| Document | sha256 |
|----------|--------|
| `planning-docs/DESIGN-taint-model.md` | `b7a53fe2ced0cec9fe44d23bacea3ee58616963c523305211411bcdcd2080244` |
| `planning-docs/DESIGN-plan-executor.md` | `afa6b595c176bc0e95412143c1cf31f29e6586e7f6301ea9bb1b10d309732e0c` |

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

**Decision:** NEEDS REVISION
**Decided:** 2026-06-29 by Ben Lamm

The nine completeness checkboxes all pass — the docs *state* every required item. But the
gate also requires **soundness**, and two independent adversarial reviews converged on the same
disqualifying defect: the value-injection defense is forgeable by an injected planner. A
completeness gate passes a fully-written-but-wrong spec; none of the gaps below trip a checkbox.
Approving here would let Phase 4 build `crates/executor` against a spec that can pass the §9
demo with zero real security (a planner that simply emits `taint: []`).

### Soundness criterion added to this gate (must hold before APPROVED)

> **An injected/compromised planner MUST NOT be able to cause a tainted value to reach a
> sensitive sink arg with an executor decision of Proceed.** Satisfied by Required Fix 1 below
> (planner references opaque value handles; it never authors literal/taint metadata).

### Required fixes before re-review (union of two independent reviews)

1. **Planner cannot author taint.** Replace the planner-writable `ValueNode { literal, taint }`
   with a broker-owned `ValueRecord` resolved by an opaque `ValueId`. The planner emits
   `PlanArg { name, value_id }` and references handles only; the executor dereferences
   `ValueRecord { id, literal, taint, provenance_node }` from a trusted store. This closes
   taint-*stripping* (false negatives) — the dual of the taint-*stapling* the docs already
   close. (Both reviews — the core fix; mirrors CaMeL's variable model.)

2. **Provenance must prove ancestry, not just point.** `provenance: EventId` only references a
   read Event; it cannot prove the literal *descends* from it. Use `provenance_chain: Vec<EventId>`
   (or `value_id` derivation edges recorded in the audit DAG) so literal → read-Event ancestry
   is locally verifiable, matching the "unbroken taint edge" the docs already require.

3. **Content args are not "non-sensitive."** Split sink args into `routing-sensitive`
   (`to`/`cc`/`bcc` → I2 blocks tainted) and `content-sensitive` (`subject`/`body`/`attachment`
   → Tier-4 approval displays verbatim with tainted spans). State sensitivity per-sink, not as a
   global principle — for `http.post`/`file.write`/`exec` the body/command *is* the dangerous arg.

4. **Taint propagates to child sessions/intents,** released only via a broker-owned
   **declassification/endorsement step logged as an audit Event** (not a silent allowlist). This
   also reconciles UX-rule 4 (no learning/auto-confirm) against handover §4.6 (broker proposes
   standing policy from repeated approvals): endorsement-as-Event is the release on the ratchet.

5. **Literal confirmation needs canonicalization.** The prompt MUST show raw *and* canonical
   forms (display-name, Unicode homoglyph, punycode domain, plus-addressing, RTL markers) plus
   known-contact and source, so `accounts@xn--ev1l...` cannot be confirmed as legitimate.

6. **Drop standing-policy patterns for v0.** Exact-literal allowlist only; pattern allowlists
   (e.g. `@company.com`) are a post-v0 policy-language problem.

*(Both reviewers explicitly endorse the direction — threat decomposition and the deterministic
Rust executor are correct — and would approve after fixes 1–6. The design is no longer drifting
back into authz.)*

---

## Gate status

> **Phase 4 MUST NOT author any `crates/executor` file until this record shows
> Decision: APPROVED and Gate status: UNBLOCKED.**

**`crates/executor` is: BLOCKED**

Available resolutions: [ UNBLOCKED / BLOCKED ]

Gate remains BLOCKED pending revision of both DESIGN docs per Required Fixes 1–6 and a fresh
gate record pinned to the revised docs' sha256 hashes.
