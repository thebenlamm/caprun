# DESIGN-content-adapter-mediation.md — caprun Content-Sensitivity + Real SMTP Adapter Mediation (v1.3)

**Requirement:** DESIGN-01 (forward-references CONTENT-01, CONTENT-02, SMTP-01, SMTP-02, SMTP-03, SMTP-05)
**Status:** Draft — pending `DESIGN-GATE-RECORD-v1.3.md` approval
**Canonical source:** `planning-docs/PLAN.md` (wins on any conflict)
**Gate:** `crates/executor` MUST NOT gain CONTENT-01 code, and `crates/brokerd` MUST NOT gain the
SMTP-05 adapter module, until this document AND its companion `planning-docs/DESIGN-confirm-binding.md`
are both reviewed and `planning-docs/DESIGN-GATE-RECORD-v1.3.md` records decision = APPROVED.

**Prior art / relationship to the approved v1.0/v1.2 docs:** This document extends
`planning-docs/DESIGN-taint-model.md` (I0/I1/I2 invariant text, genuine-taint requirement) and
`planning-docs/DESIGN-plan-executor.md` (`ValueRecord`/`ValueId` handle model, `PlanNode` schema,
sink sensitivity map) — both APPROVED per `planning-docs/DESIGN-GATE-RECORD.md`. It also extends
`planning-docs/DESIGN-session-trust-state.md` (the Step 0.5 draft-only class deny, I2-over-I1
precedence, the round-1 B1 fix) — APPROVED per `planning-docs/DESIGN-GATE-RECORD-v1.2.md`. This
document is **additive and hardening**: it does not reopen the locked plan-node API, the I0/I1/I2
invariant text, or Step 0.5's placement — it resolves what the executor's per-arg loop does when
MORE THAN ONE sensitive arg on the same plan node is tainted, a case CONTENT-01 makes reachable for
the first time (`email.send` has both routing args and content args live simultaneously).

---

## The Problem Being Solved

v1.2 shipped I2 routing-sensitivity blocking (`to`/`cc`/`bcc` for `email.send`) and I1/I0 session
draft-only demotion, gated by the Phase 8 DESIGN pair. Its round-1 review found a real blocker (B1,
`planning-docs/DESIGN-REVIEW-v1.2-round1.md`): an unstated precedence between two deny/block
mechanisms made the confirm path unreachable in every live run. That was fixed by moving the
draft-only class deny to a post-loop "Step 0.5."

v1.3's hero demo requires blocking a tainted email **body**, not just the recipient — reopening
`CONTENT-01` (previously deferred to v2 at v1.2 scoping; see `PROJECT.md` Key Decisions). The
content-sensitivity classification for `email.send`'s `subject`/`body`/`attachment` args already
exists in the executor (`crates/executor/src/sink_sensitivity.rs:93-98`); today it is a documented
no-op ("Content-sensitive tainted args do NOT Block in v0" — `crates/executor/src/lib.rs:141-142`,
Step 3). Making CONTENT-01 real — Blocking on a tainted body — exposes a second, structurally
identical instance of the B1 failure mode, this time WITHIN a single mechanism instead of between
two: the current per-arg loop (`crates/executor/src/lib.rs:62-143`) returns on the FIRST tainted
routing-sensitive arg it finds (Step 2, lines 99-139), and `ExecutorDecision::BlockedPendingConfirmation`
/ `SinkBlockedAnchor` (`crates/runtime-core/src/executor_decision.rs:108-153`) are built around
exactly ONE blocked arg (`SinkBlockedAnchor.arg: String`, line 115; `BlockedPendingConfirmation.literal:
String`, line 152). A plan node carrying BOTH a tainted `to` and a tainted `body` would, if CONTENT-01
were bolted onto Step 3 unchanged, Block on `to` only. The human confirms that shown Block.
`crates/brokerd/src/confirmation.rs`'s `confirm()` re-invokes the sink using the FULL frozen
`resolved_args` snapshot — which includes the body's literal, never individually shown or confirmed.
**The tainted body ships, unconfirmed, riding the recipient's confirmation.** This is the
B1-reincarnation risk (D-02, D-12a) this document exists to close, and closing it — not adding a new
classification — is CONTENT-01's actual engineering content.

This document, together with `DESIGN-confirm-binding.md` (CONFIRM-03) and
`DESIGN-GATE-RECORD-v1.3.md` (the adversarial review record), is the DESIGN-01 gate. No executor/TCB
code for CONTENT-01 or the SMTP-05 adapter may be written until the gate record shows
APPROVED/UNBLOCKED.

---

## Content-Sensitivity Classification (CONTENT-01/CONTENT-02)

**Scope (MUST, D-01):** Content-sensitivity classification for `email.send`'s body-bearing args is a
**single hardcoded match arm in the executor TCB**, scoped to `email.send`'s args only. It MUST NOT
be generalized into a content-classification taxonomy, a reusable framework, or any policy-file-driven
mechanism. This mirrors `CON-i2-non-bypassable`: sensitivity is a security property hardcoded in Rust,
never a configuration knob. `CONTENT-02` is this one-match-arm scope guard, explicit and intentional
(per the v1.3 scoping advisor panel) — it MUST NOT grow to cover other sinks or a general "content
policy" concept in this milestone.

**Already exists — do not re-implement (MUST NOT duplicate, D-21):** This classification IS ALREADY
IMPLEMENTED. `crates/executor/src/sink_sensitivity.rs:93-98`'s `is_content_sensitive` function:

```rust
// Illustrative shape, not literal code to paste — cite the existing source directly.
pub fn is_content_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_CONTENT_SENSITIVE.contains(&arg_name), // ["subject", "body", "attachment"]
        _ => false,
    }
}
```

already returns `true` for `subject`/`body`/`attachment` on `email.send` (`EMAIL_SEND_CONTENT_SENSITIVE`,
`sink_sensitivity.rs:71`), and has since v0. **Phase 14's real work is changing Step 3's CONSEQUENCE
in `submit_plan_node`** (`crates/executor/src/lib.rs:141-142`, currently a documented no-op/fall-through
comment: "Content-sensitive tainted args do NOT Block in v0 — Tier-4 verbatim review is deferred") —
from "mark and fall through" to "Block, same as routing-sensitive" — **NOT adding a new match arm.**
A Phase 14 plan that proposes writing a new `is_content_sensitive` classification duplicates existing
code and MUST be corrected against this section.

**Independent re-verification required (MUST, D-21):** The adversarial reviewer arranged per D-11
MUST NOT accept this claim on this document's or the research doc's word — the reviewer MUST
independently re-read `crates/executor/src/sink_sensitivity.rs:93-98` and confirm `is_content_sensitive`
already returns `true` for `email.send`'s `subject`/`body`/`attachment` before treating this section as
verified.

---

## Precedence — Routing-Block vs Body-Block (CONTENT-01, D-02)

**MUST:** A plan node carrying BOTH a tainted routing-sensitive arg (e.g., `to`) AND a tainted
content-sensitive arg (e.g., `body`) MUST surface BOTH as Blocked in the same decision. Neither
silently pre-empts, masks, or is dropped in favor of the other. This is not a hypothetical concern —
it is the same shape of bug as B1 (`planning-docs/DESIGN-REVIEW-v1.2-round1.md`), where an unstated
precedence between two deny/block mechanisms made the confirm path unreachable in every live run.
Here the risk reincarnates for the body arg: instead of two mechanisms (I2 Block vs I1/I0 class deny)
silently favoring one outcome, it is ONE mechanism (the per-arg loop) silently favoring one blocked
arg over another because of first-match-wins early return.

**Resolution (MUST):** Both the routing-sensitive check (Step 2) and the content-sensitive check
(the former Step 3, now made a real Block) MUST be evaluated for EVERY arg on the plan node before
any Block decision is returned — see Collect-then-Block below, the mechanism that makes this
precedence statement true. **Both tainted args MUST be confirmable or deniable through the existing
single-shot confirm/deny mechanism** (`caprun confirm`/`caprun deny <effect_id>`) as one combined set
— see D-17 and `DESIGN-confirm-binding.md`.

---

## Collect-then-Block (D-14, MUST)

**Current shape (cited, not modified by this document — this document specifies the target, Phase 14
implements it):**

- The per-arg loop in `submit_plan_node` (`crates/executor/src/lib.rs:62`, `for arg in &plan_node.args`)
  returns immediately, INSIDE the loop body, on the first arg that is routing-sensitive AND tainted
  (Step 2, lines 99-139: `if sink_sensitivity::is_routing_sensitive(...) && record.taint.iter().any(...)
  { ... return ExecutorDecision::BlockedPendingConfirmation { anchor, literal }; }`). It never
  continues scanning subsequent args in the same plan node once one Block fires.
- `SinkBlockedAnchor` (`crates/runtime-core/src/executor_decision.rs:108-131`) is singular by
  construction: `pub arg: String` (line 115), one `value_id`, one `literal_sha256`, one `taint`
  vec, one `provenance_chain` — ONE blocked arg per anchor.
- `ExecutorDecision::BlockedPendingConfirmation { anchor: SinkBlockedAnchor, literal: String }`
  (`executor_decision.rs:150-153`) is likewise singular — one blocked value per decision.

**MUST (D-14, not discretionary):** The executor's per-arg loop MUST change from first-match-wins
early-return to **collect-ALL-sensitive-and-tainted-args-in-one-pass, then Block as a set.** For every
arg in `plan_node.args`, the loop MUST check BOTH `is_routing_sensitive` and `is_content_sensitive`
(CONTENT-01/02) against the resolved record's taint, and MUST accumulate every arg that is
(routing-sensitive OR content-sensitive) AND tainted into one collection, before returning any Block
decision. Only after every arg has been checked does the function decide: if the collected set is
non-empty, return one `BlockedPendingConfirmation`; if empty, proceed to Step 0.5 (below) then
`Allowed`.

**Illustrative shape, not literal code to paste** (adapted from `12-RESEARCH.md` Pattern 1, matching
this document's citations of the current file/line shape above):

```rust
// Illustrative target shape for Phase 14 — not existing code, not literal code to paste.
let mut blocked: Vec<BlockedArg> = Vec::new();
for arg in &plan_node.args {
    let record = /* resolve as today: Steps 1/1a/1b unchanged */;
    let sensitive = sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
        || sink_sensitivity::is_content_sensitive(&plan_node.sink, &arg.name); // CONTENT-01/02
    if sensitive && record.taint.iter().any(|t| t.is_untrusted()) {
        blocked.push(BlockedArg::from_record(&arg.name, &arg.value_id, &record));
    }
}
if !blocked.is_empty() {
    return ExecutorDecision::BlockedPendingConfirmation { anchors: blocked };
}
```

**Plural decision/anchor shape (MUST, D-14):** `ExecutorDecision::BlockedPendingConfirmation` and
`SinkBlockedAnchor` MUST become PLURAL — a `Vec<BlockedArg>` (or equivalently named collection type),
each element carrying its own `literal` (or `literal_sha256` digest, matching the existing
redactable-digest pattern) + `taint` + `provenance_chain`, mirroring today's singular
`SinkBlockedAnchor` fields but one-per-blocked-arg instead of one-per-decision. The existing
anti-stapling discipline (T-04-03: every field is a verbatim clone of the resolved `ValueRecord`, the
executor mints nothing) MUST be preserved per-element in the new plural shape.

**Three MUST sub-statements binding the collection mechanism to the rest of the security model:**

- **(D-15) I2-over-I1 precedence is PRESERVED unchanged.** The collect-all loop MUST complete with NO
  Block before Step 0.5's draft-only check (`crates/executor/src/lib.rs:145-178`,
  `DESIGN-session-trust-state.md` §8/§11) runs. This document MUST NOT reorder Step 0.5 relative to
  the per-arg loop — Step 0.5 remains reachable ONLY when the (now-collecting) loop finds nothing to
  Block, exactly as today. Reordering this — running Step 0.5 before or interleaved with the loop —
  would reintroduce a variant of B1: a `Draft` session with a tainted body would be Denied before the
  body's Block is ever collected, making the confirm path unreachable again.
- **(D-16) The unbroken-edge anti-staple gate applies to EVERY blocked arg in the set, not just one.**
  The existing genuine-taint requirement (`DESIGN-taint-model.md` §Genuine-Taint Requirement — raw-read
  Event → `ValueRecord` → sensitive sink arg, provable as an unbroken edge in the audit DAG via
  `provenance_chain[0]`) MUST hold independently for EVERY element of the blocked-arg collection. A
  two-tainted-arg Block (e.g., tainted `to` AND tainted `body`) with only ONE of the two edges proven
  in the audit DAG is a PARTIAL pass, not a pass — Phase 15's verification MUST assert the unbroken
  edge for each blocked arg individually, not merely for the decision as a whole.
- **(D-17) Single-shot semantics extend to the whole set.** `caprun confirm <effect_id>` MUST authorize
  the WHOLE `(sink, all-blocked-args, combined-digest)` set, or `caprun deny <effect_id>` MUST deny
  ALL of it. There MUST NOT be a partial-confirm path that releases a subset of the blocked args while
  leaving others pending or silently including others unconfirmed (the exact shape of the risk this
  document exists to close — see Precedence above). `DESIGN-confirm-binding.md` (D-19) specifies the
  combined-digest binding hash that makes this concrete.

---

## Plan-Node API Is Untouched (D-18)

**MUST:** This document specifies internal decision/anchor hardening ONLY. The locked plan-node API —
`PlanNode { sink, args: Vec<ValueNode> }` (`DEC-architectural-lock-plan-nodes`, `CLAUDE.md`) — is
UNCHANGED. Only the internal `ExecutorDecision`/`SinkBlockedAnchor` types become plural, as specified
above. This is NOT a reopening of the locked plan-node API shape; it MUST NOT be read or implemented as
one. No new field is added to `PlanNode`, and no raw `EffectRequest { effect, args: Map }` path is
introduced — `check-invariants.sh` Gate 1's `EffectRequest` token gate continues to apply unchanged.
