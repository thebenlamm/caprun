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
content-sensitivity classification for `email.send`'s content args (`subject`/`body` for v1.3 —
`attachment` is **descoped**, see "Attachment Is Descoped for v1.3 (D-23)" below) already
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
verified. (The current code's content-sensitive set is `["subject", "body", "attachment"]`; v1.3
narrows the *live-scoped* set to `["subject", "body"]` — see D-23 immediately below. The reviewer
verifies the current-source claim as written; the narrowing is a mandated Phase-13/14 code change,
not a contradiction of it.)

---

## Attachment Is Descoped for v1.3 (D-23, MUST)

**MUST:** `attachment` is **OUT OF SCOPE for v1.3** and MUST be removed from `email.send`'s live
surface in the two hardcoded sets that currently list it:

1. `EMAIL_SEND_CONTENT_SENSITIVE` (`crates/executor/src/sink_sensitivity.rs:71`) MUST become
   `&["subject", "body"]` — `attachment` removed.
2. `email.send`'s schema `allowed` set (`crates/executor/src/sink_schema.rs`,
   `KNOWN_SINKS` `email.send` entry, currently `["to", "cc", "bcc", "subject", "body", "attachment"]`)
   MUST become `["to", "cc", "bcc", "subject", "body"]` — `attachment` removed, so a plan node
   carrying an `attachment` arg is `Denied` with `UnknownArg` at the Step 0 schema gate, before any
   sensitivity evaluation.

**Why (start-simplest, D-01 scope discipline):** `attachment` is both schema-accepted AND
content-sensitive today, so a tainted attachment would be Blocked → confirmed → and then MUST be
sent — but SMTP-05's typed-builder allow-list (`.to`/`.cc`/`.bcc`/`.subject`/`.body`) has NO
attachment path, and the `Content-Disposition` filename→header CRLF surface an attachment introduces
is not analyzed by the D-07/D-22 defense (which reasons only about address/subject headers and the
after-separator body). Rather than design an unanalyzed attachment-header injection surface into
v1.3, `attachment` is dropped entirely. `subject` stays (RFC 2047 encoded-word path — analyzed and
safe, D-07); `body` stays (after the header/body separator — analyzed and safe, D-07). Only
`attachment` goes.

**MUST NOT (scope creep guard):** This descope MUST NOT be read as a temporary stub to be re-enabled
mid-milestone. Re-adding `attachment` requires its own DESIGN analysis of the Content-Disposition
CRLF surface (a future milestone's work, not v1.3's).

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
- **(D-16) The unbroken-edge anti-staple gate applies to EVERY blocked arg in the set, not just one,
  AND the edge MUST descend from the originating untrusted-doc read — not a re-anchored fresh root.**
  The existing genuine-taint requirement (`DESIGN-taint-model.md` §Genuine-Taint Requirement — raw-read
  Event → `ValueRecord` → sensitive sink arg, provable as an unbroken edge in the audit DAG via
  `provenance_chain[0]`) MUST hold independently for EVERY element of the blocked-arg collection. A
  two-tainted-arg Block (e.g., tainted `to` AND tainted `body`) with only ONE of the two edges proven
  in the audit DAG is a PARTIAL pass, not a pass — Phase 15's verification MUST assert the unbroken
  edge for each blocked arg individually, not merely for the decision as a whole.
  **Provenance-threading MUST (the anti-laundering half of D-16, specified in full in
  `DESIGN-confirm-binding.md`'s "Post-Transformation Bytes" section):** for a *transform-derived*
  blocked arg (an EXTRACT-03 concat/base64-decode value), the unbroken edge D-16 asserts MUST be the
  edge from the transform to EVERY input's originating untrusted-doc read Event — i.e., the derived
  value's `provenance_chain` root MUST remain the originating read, threaded from its inputs, NOT a
  fresh event minted in the transform call. A derived value whose `provenance_chain` is a fresh
  single-element chain rooted at a NEW event unrelated to the original read satisfies the letter of
  "an unbroken edge exists via `provenance_chain[0]`" while laundering lineage — copied taint labels
  with a re-anchored root are exactly the "taint stapled on at the sink" failure `CLAUDE.md` hard
  constraint #1 forbids. D-16's per-arg edge assertion MUST therefore check that the root is the
  originating read (or the explicit DAG derivation edge back to it), not merely that *some* root
  exists. See `DESIGN-confirm-binding.md` for the `mint_from_read`-successor provenance-threading
  contract this depends on.
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

---

## Adapter Mediation Boundary (SMTP-01/02, D-03/D-04)

**Worker-never-sends (MUST, D-03):** The confined worker MUST NEVER perform the SMTP call. The
broker/adapter MUST perform the SMTP call ONLY after (a) the executor has authorized the plan node
(no outstanding Block/Deny) AND (b) the human has confirmed any Blocked args via `caprun confirm
<effect_id>`. This mirrors `DEC-layer-roles`: sandbox is the boundary, broker is the reference monitor,
adapters are the only paths to effects — the confined worker holds no send capability by construction
(reinforced kernel-side, see Kernel-Enforced Negative Net Assertion below).

**Secrets never reach the confined worker or any tainted context (MUST, D-04 — restated to its real
threat intent):** The real invariant D-04 protects is NOT "secrets live in one specific process" — it
is that **SMTP secrets/credentials (host, port, auth) MUST NEVER reach the confined worker or any
tainted/plan-node/confined context.** They MUST NOT appear in the worker's environment, in the worker's
process arguments, or in any plan-node payload. A `PlanNode` carries only `{ sink, args: Vec<ValueNode> }`
(locked, see above) — SMTP credentials are never a `ValueNode` and never travel through the plan-node
path. The confirmed send runs in the trusted, human-invoked confirm process (see the D-03 refinement
below) — which is NOT the worker and NOT a tainted context — so holding no-secret (Mailpit) there, or a
properly custodied secret there in a future SES path, does not violate D-04.

**For the v1.3 Mailpit gate there is NO secret to custody (custody trivially satisfied):** Mailpit
(`localhost:1025`) is UNAUTHENTICATED — no host credential, no username, no password — so D-04's custody
obligation is trivially satisfied because the secret set is empty. D-04's non-trivial custody problem (a
real credential held in a long-lived process) exists ONLY for the live-SES path, which is
out-of-scope/post-milestone (SMTP-04) and is explicitly DEFERRED with its own future-work design
obligation (see the D-03 refinement's future-work note below).

**New adapter location (MUST):** The real adapter lives at `crates/brokerd/src/sinks/email_smtp.rs`
(broker-resident, NOT confined-worker-resident), replacing the existing `invoke_email_send_stub`
(per the RESEARCH Architectural Responsibility Map). This module is the ONLY code path that performs
the SMTP call.

**The confirmed send runs in the confirm-path process — the SAME locus as `file.create` today (MUST,
D-03 refinement; this REVERSES round-1's broker-daemon mandate):** Round 1 mandated that the confirmed
SMTP send run in a long-lived "broker daemon" reached over UDS, on the theory that keeping the SMTP call
out of the short-lived confirm process was required by D-04. **Round 2 traced that mandate against the
actual codebase and found it UNBUILDABLE; it is reversed here.** The caprun broker is EPHEMERAL and
session-scoped, NOT a daemon (independently re-verified against source at round-2 authoring time):

- `crates/brokerd/src/server.rs:95-96` binds a per-session abstract socket
  `\0/agentos/{session_id}` — one socket per session, not a stable control endpoint.
- `cli/caprun/src/main.rs:270` calls `broker_task.abort()` the instant the worker exits — the broker
  does not outlive the run.
- There is NO daemon binary: `crates/brokerd/src/bin/` does not exist.
- `BrokerRequest` (`crates/brokerd/src/proto.rs`) is a worker→broker SUBMISSION channel
  (`CreateSession`/`ProvideIntent`/`RequestFd`/`ReportRead`/`ReportClaims`/`SubmitPlanNode`) with NO
  confirm-or-perform-send control variant — it is not a control channel a later confirm process could
  reconnect to.

A "hand the confirmed `effect_id` to the already-running broker daemon over the existing UDS channel"
handoff therefore has nothing to hand to. An implementer told to build it would either invent a whole
persistent-daemon + control-socket architecture (a scope explosion this demo MUST NOT take on) or
silently fall back to the local-invoke pattern round 1 was trying to forbid.

**For v1.3, the confirmed send runs in the confirm-path process, invoking
`crates/brokerd/src/sinks/email_smtp.rs` from the frozen snapshot — the SAME locus where
`crates/brokerd/src/confirmation.rs::confirm()` invokes `file.create`'s
`invoke_file_create_from_resolved` today** (`confirmation.rs:400-418`, after the state-transition CAS).
`caprun confirm <effect_id>` records the human's confirmation and performs the send in-process. There is
no second process and no daemon handoff.

**Why this is secure for v1.3 — D-03 (worker-never-sends) STILL HOLDS, unchanged:** the confined worker
(default-deny-net, cannot `socket()` — see Kernel-Enforced Negative Net Assertion below) NEVER performs
the send. The send runs in the SEPARATE, trusted, human-invoked confirm process — never in the worker,
never in any tainted/plan-node context. Worker-never-sends is a statement about the CONFINED WORKER, not
about which trusted process holds the SMTP call; moving the send from a hypothetical daemon to the
confirm process puts no capability back in the worker and does not weaken D-03.

**Why D-04 is satisfied for the v1.3 gate:** per the restated D-04 above, the Mailpit gate target is
unauthenticated (`localhost:1025`, no credentials), so there is NO SMTP secret to reach any context —
D-04's custody concern is trivially satisfied because the secret set is empty. Real secret-custody in a
persistent process is a genuine problem ONLY for the live-SES path, which is deferred.

**Future-work note (MUST NOT build now — SES carries its own daemon+custody design):** if/when the
live-SES path (SMTP-04) is taken on in a later milestone, it MUST carry its OWN DESIGN for (a) a
persistent send process (daemon + control-socket) and (b) a secret-custody model that keeps real SMTP
credentials out of every short-lived process AND every tainted/confined context. That
daemon + control-socket + secret-custody design is FUTURE WORK; it is NOT designed here and MUST NOT be
built as part of v1.3. v1.3's confirm-path-process send is correct precisely because the Mailpit target
has no secret to protect.

---

## At-Most-Once Send + Durable Attempt Ledger — No Swallowed Errors (SEND-01/SEND-02, D-24)

Because the confirmed send now runs in the SINGLE confirm-path process (per the D-03 refinement above —
round-1's two-process daemon split is reversed), the double-fire / silent-loss risk is confined to one
process rather than split across two; but two hazards remain and MUST be closed explicitly: a
redelivered or double `caprun confirm <effect_id>` could re-drive an irreversible send, and the
inherited v1.2 confirm path's `Err(_) => Ok(ConfirmedButSinkFailed)` shape SWALLOWS the send error.

**At-most-once via the EXISTING CAS as the sole gate (MUST, SEND-01 — reuses the current mechanism, does
NOT invent a cross-process token):** Because the send now runs in the single confirm-path process,
at-most-once reuses the atomic guard already in the codebase:
`crates/brokerd/src/confirmation.rs::transition_state`'s
`UPDATE pending_confirmations SET state=? WHERE effect_id=? AND state='pending'` CAS. That CAS — a row
already `confirmed`/`denied` matches ZERO rows — is the SOLE gate that authorizes the irreversible wire
action: only the single caller that observes the CAS flip `pending → confirmed` (affected-rows = 1) may
perform the send; any redelivered or double `caprun confirm <effect_id>` observes affected-rows = 0 and
MUST NOT send. **Round 1's cross-process / UDS-handoff durable-idempotency-token language is DROPPED** —
it was solving the two-process daemon split that finding #1 removed; there is no process boundary to make
idempotent across, and the in-DB CAS already provides exactly-one-winner semantics.

**The CAS and the durable `email_send_attempted` append MUST be ONE atomic DB transaction (MUST,
SEND-01):** the state-transition CAS (`pending → confirmed`) and the durable `email_send_attempted` Event
append (anchored to the same `effect_id`, hash-chained into the SHA-256 audit DAG) MUST commit in a
SINGLE atomic SQLite transaction, BEFORE any SMTP connection is opened. This closes the double-fire
window: a redelivered/double `caprun confirm` cannot BOTH pass the "already attempted?" check AND send,
because the winning transaction atomically flips the state and records the attempt together — a second
invocation sees `state != 'pending'` (affected-rows = 0), reads the durable `email_send_attempted`, and
refuses. There is no interleaving in which two callers both flip the CAS or both append the attempt.

**Order of operations (MUST):**
1. In ONE atomic transaction: CAS `pending → confirmed` AND append the durable `email_send_attempted`
   Event (before opening the SMTP connection). If the CAS affects ZERO rows, abort — do NOT send.
2. perform the `email_smtp.rs` send from the frozen snapshot;
3. on success, append `email_send_succeeded`; on error, append `email_send_failed` (OPAQUE payload —
   see the literal-leak rule immediately below).

**`email_send_failed`'s hashed payload carries ONLY an opaque error code/digest — NEVER a confirmed
literal or raw SMTP response (MUST NOT — closes the literal-leak into the immutable chain):** SMTP
rejections routinely echo the recipient or body (e.g. `550 <attacker@evil.com> rejected`). Appending
that raw error text into the `email_send_failed` Event would STAPLE the confirmed literal into the
immutable, hash-chained audit DAG — violating this codebase's standing invariant that raw literals NEVER
enter a hashed Event payload (they go ONLY to the redactable `blocked_literals` side table
(`crates/brokerd/src/server.rs`), so `confirm()`'s redaction gate (`crates/brokerd/src/confirmation.rs`)
can purge them). Therefore the hashed `email_send_failed` payload MUST carry ONLY an OPAQUE error code
and/or digest — never the recipient/body literal, never the raw SMTP response bytes. Raw error detail
(the SMTP response text, for operator diagnosis) MUST be routed to `logger.error()` and/or the redactable
side table ONLY, never the hash chain. This is "never swallow" done RIGHT: the failure is fully audited
(opaque code in the chain + raw detail in the redactable/log channel) WITHOUT laundering a confirmed
literal into an immutable, unredactable record.

**No auto-retry of an irreversible send (MUST, SEND-02):** A confirmed-but-unsent state (a crash or
error between the step-1 transaction and a terminal step) MUST NOT be auto-retried. Recovery is an
explicit, human-visible operation — the DAG shows `email_send_attempted` with no `email_send_succeeded`,
and the operator decides. Silent re-drive of a possibly-already-delivered message is forbidden
(at-most-once beats at-least-once for an irreversible send).

**Never swallow the send error (MUST NOT):** The error path MUST NOT return a bare `Ok(...)` that hides
the failure (the inherited v1.2 `Err(_) => Ok(ConfirmedButSinkFailed)` shape is explicitly rejected),
MUST NOT `.unwrap()`/panic on a send error, and MUST NOT drop the error silently. It MUST
`logger.error()` with the raw context AND append the durable, OPAQUE-payload `email_send_failed` Event
(per the literal-leak rule above). The caller receives a distinct non-zero result that a scripted
operator can tell apart from "denied" or "unknown effect_id" (mirrors v1.2's M3 `sink_invocation_failed`
exit-code discipline, `DESIGN-confirmation-release.md`).

---

## Kernel-Enforced Negative Net Assertion (SMTP-01, D-05)

**MUST:** A confined worker's direct attempt to open an SMTP connection MUST FAIL under default-deny
net. This is a claim about the sandbox BOUNDARY, not merely adapter code structure, and MUST be
testable on real Linux, mirroring the project's existing default-deny-net posture
(`DEC-layer-roles`).

**Point at the EXISTING mechanism — do not design a new confinement primitive (MUST NOT):** This
assertion MUST be pointed at the mechanism that already exists:
`crates/sandbox/src/seccomp.rs::apply_worker_filter()`, which installs a seccomp-bpf filter denying
`socket(AF_INET, ...)`/`socket(AF_INET6, ...)` with `SeccompAction::Errno(EPERM)`. The existing test
pattern — `crates/sandbox/tests/confinement_integration.rs::negative_net` and
`crates/sandbox/src/bin/confine-probe.rs::probe_net` — already proves `socket(AF_INET, SOCK_STREAM, 0)`
returns `EPERM` under confinement. Landlock does NOT restrict socket creation (confirmed in
`probe_net`'s own doc comment); only seccomp produces this `EPERM`. This document forbids inventing a
second, parallel network-denial mechanism for SMTP-01 — the SAME seccomp filter already covers it,
since it denies the underlying `socket()` syscall regardless of destination port.

**Phase 13's negative test (MUST reuse this pattern):** Phase 13 MUST add an integration test that
reuses `confine-probe`'s pattern to attempt an actual `connect()` to the Mailpit host:port under
confinement, asserting the connection attempt fails (`EPERM` at `socket()`, or an equivalent
kernel-enforced denial) — not merely that the adapter code "doesn't call SMTP from the worker" by
inspection.

---

## Local Capture SMTP Target (SMTP-03, D-06)

**MUST:** The acceptance-gate test targets a LOCAL capture SMTP server — Mailpit (`axllent/mailpit`
Docker image), the maintained, API-compatible successor to abandoned MailHog (unmaintained since
~2020). This is Linux-verifiable via Colima+Docker (this project's existing verification recipe), and
has no live infra dependency.

**Live SES is OUT of gate scope (MUST NOT design for it as a requirement):** Live SES / real inbox
send is explicitly downgraded to an optional, non-gated, post-milestone config-swap (see `PROJECT.md`
Out of Scope, `SMTP-04`). This document MUST NOT design the adapter, its secrets model, or its
acceptance test as if live SES were a milestone requirement. Mailpit IS a real SMTP send with a web
UI showing arrival — this satisfies "real send" for the gate.

---

## Wire-Message Construction — CRLF/Header-Injection Defense (SMTP-05, D-07/D-22)

**Typed builder only (MUST):** The adapter MUST construct the outgoing message EXCLUSIVELY through
`lettre`'s typed `Message::builder()` setters — `.to()`, `.cc()`, `.bcc()`, `.subject()`, `.body()` —
pinned to `lettre >= 0.11.22` (fixes RUSTSEC-2021-0069's dot-stuffing SMTP-command-injection and
picks up the RUSTSEC-2026-0141 TLS fix).

**Forbidden constructs (MUST NOT):** The adapter MUST NOT use `HeaderValue::dangerous_new_pre_encoded`
(its own doc comment states it "exposes the encoder to header injection attacks") and MUST NOT build
any header line via `format!()` or string concatenation.

**Concrete defense mechanics (D-07 — answering exactly how injection is prevented, verified by
direct source read, not assumed):**

- `Address::new` (`lettre` `src/address/types.rs`) validates the local part via an ALLOW-LIST grammar
  (`is_atext`/`is_qtext_char`/`is_wsp`) that does not include byte 10 (LF) or byte 13 (CR) in any
  branch — a recipient/`Mailbox` value containing raw CR/LF is REJECTED at parse time, not
  sanitized after the fact.
- `HeaderValueEncoder::allowed_char` (`lettre` `src/message/header/mod.rs`) excludes bytes 10 and 13
  from its allowed range; any header value (e.g., `Subject`) containing CR/LF is routed through RFC
  2047 encoded-word encoding (`email_encoding::headers::rfc2047::encode`) — raw CR/LF cannot appear
  on the wire in a header.
- **Why the body is safe even though it is not run through the header encoder:** the body is written
  after the blank-line header/body separator (RFC 5322 structure). A receiving MTA (Mailpit) parses
  headers only up to the first blank line; everything after it is opaque body content. A body literal
  containing `\r\nBcc: attacker@evil.com` is inert — it stays body text and is never re-parsed as a
  header — **PROVIDED the adapter never concatenates the body literal into the header-construction
  call chain.** This is a structural (call-boundary) guarantee, not a string-scrubbing one.

**lettre rejection semantics on a confirmed literal (MUST, D-07 refinement):** The by-construction
defense above relies on `lettre` REJECTING (returning `Result::Err`) a CR/LF-bearing address or
header at construction time. The adapter MUST define what it does with that `Err` — and it is NOT
"proceed anyway" and NOT "panic." Any `lettre` construction `Err` (`Address::new`, `Message::builder()`
setters, `.to()`/`.cc()`/`.bcc()`/`.subject()`/`.body()`) on a literal the human already confirmed
MUST become a **fail-closed, AUDITED abort**: append a durable `email_send_failed` Event (same
`effect_id`, per SEND-01/SEND-02 above) whose HASHED payload carries ONLY an opaque error code/digest —
NEVER the confirmed literal (e.g., the CRLF-bearing address) and NEVER the raw `lettre` error text (per
the literal-leak rule in the At-Most-Once section above) — route the raw construction-error detail to
`logger.error()` and/or the redactable side table, and return the distinct non-zero send-failed result —
NEVER `.unwrap()`/panic, NEVER a silent drop, NEVER a fallback to a raw/`format!`-built message. A
confirmed literal that `lettre` refuses to encode safely is a blocked-and-audited failure, not a
best-effort send.

**Forbid `boring-tls` (MUST NOT):** The adapter MUST NOT enable `lettre`'s `boring-tls` Cargo feature
(RUSTSEC-2026-0141: silently disables TLS hostname verification for `0.10.1..=0.11.21`). The local
Mailpit target needs no TLS at all; enabling this feature has no upside and a known CVSS-9.1 downside.

**Phase 13 CRLF fixture is a HARD requirement regardless of lettre's by-construction defense (MUST,
D-22):** "Defends by construction" MUST be VERIFIED by a passing adversarial fixture test in Phase 13,
not assumed from the library's reputation alone. The fixture MUST assert: a tainted body carrying a
CRLF-then-`Bcc:`/extra-recipient sequence (e.g., `"...\r\nBcc: attacker@evil.com"`) produces EXACTLY
the intended envelope recipients at Mailpit — no smuggled recipient — verified via Mailpit's HTTP API
(query the captured message's actual `To`/`Cc`/`Bcc` envelope, not just that the send succeeded).

**Recommended negative-assertion test (grep-based, mirroring `check-invariants.sh`'s style):** a test
or CI gate asserting no `format!` call in `crates/brokerd/src/sinks/email_smtp.rs` builds a header
line, and that the token `dangerous_new_pre_encoded` never appears in that file.

---

## Done-When (Acceptance Predicate)

This document's design is satisfied when the following conditions ALL hold simultaneously:

1. **Content-sensitivity scope is a single hardcoded match arm, already implemented.** CONTENT-01/02
   classification for `email.send`'s content args is confirmed to already exist at
   `crates/executor/src/sink_sensitivity.rs:93-98`; no new classification code is proposed anywhere
   downstream of this document (D-01, D-21). `attachment` is descoped for v1.3 — removed from both
   `EMAIL_SEND_CONTENT_SENSITIVE` and the schema `allowed` set, leaving the live content set as
   `subject`/`body` (D-23).
2. **Precedence between routing-block and body-block is explicit, not implicit.** A tainted recipient
   AND a tainted body on the same plan node both surface as Blocked; neither is dropped, masked, or
   silently pre-empted (D-02).
3. **Collect-then-Block is specified as a MUST, with a plural decision/anchor shape.** The per-arg
   loop collects ALL sensitive+tainted args before any Block is returned;
   `ExecutorDecision::BlockedPendingConfirmation`/`SinkBlockedAnchor` become `Vec`-shaped (D-14),
   I2-over-I1 precedence via Step 0.5 is preserved unchanged (D-15), the unbroken-edge gate is required
   per blocked arg (D-16), and single-shot confirm/deny covers the whole set (D-17).
4. **The plan-node API is confirmed untouched.** `PlanNode { sink, args: Vec<ValueNode> }` is
   unchanged; only internal decision/anchor types become plural (D-18).
5. **Worker-never-sends and secrets-never-reach-worker/tainted-context are both stated as MUSTs**, with
   the adapter located at `crates/brokerd/src/sinks/email_smtp.rs` (D-03, D-04), AND the confirmed send
   runs in the confirm-path process from the frozen snapshot — the SAME locus as `file.create` today
   (`confirmation.rs::confirm()`), NOT a broker daemon (round-1's daemon mandate is reversed: the broker
   is ephemeral/session-scoped, with no daemon binary and no confirm/send control channel). D-04 is
   satisfied because the Mailpit gate is unauthenticated (no secret to custody); the live-SES path is
   deferred with its own daemon+control-socket+secret-custody future-work obligation (D-03 refinement).
6. **At-most-once send + durable attempt ledger + no swallowed errors + no literal-leak are stated as
   MUSTs.** The EXISTING `transition_state` CAS (`pending → confirmed`) is the SOLE at-most-once gate,
   committed in ONE atomic DB transaction with the durable `email_send_attempted` append before any wire
   action (round-1's cross-process idempotency-token language dropped, per finding #1's single-process
   reversal); `email_send_succeeded`/`email_send_failed` terminal Events, with `email_send_failed`'s
   HASHED payload carrying ONLY an opaque error code/digest — never the confirmed literal or raw SMTP
   response (raw detail to `logger.error()`/the redactable side table only); no auto-retry of a
   confirmed-but-unsent irreversible send; and a never-swallow/never-`unwrap` error path (SEND-01/SEND-02,
   D-24).
7. **The negative net assertion points at the existing seccomp mechanism**, not a new confinement
   primitive, and specifies Phase 13's reuse of the `confine-probe`/`negative_net` test pattern (D-05).
8. **The gate test target is Mailpit, local-only, with live SES explicitly out of scope** (D-06).
9. **CRLF/header-injection defense is specified with concrete, source-verified mechanics** (typed
   builder only, `dangerous_new_pre_encoded` and `format!`-built headers forbidden, `boring-tls`
   forbidden), with `lettre` construction `Err` on a confirmed literal defined as a fail-closed
   audited abort (D-07), AND a Phase-13 adversarial CRLF fixture test is mandated regardless of the
   library's by-construction defense (D-07, D-22). The `attachment` header/CRLF surface is out of
   scope for v1.3 by construction (D-23).

If any condition fails, this document is NOT ready for `DESIGN-GATE-RECORD-v1.3.md` to record
APPROVED/UNBLOCKED.
