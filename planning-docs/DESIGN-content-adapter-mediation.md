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

---

## Adapter Mediation Boundary (SMTP-01/02, D-03/D-04)

**Worker-never-sends (MUST, D-03):** The confined worker MUST NEVER perform the SMTP call. The
broker/adapter MUST perform the SMTP call ONLY after (a) the executor has authorized the plan node
(no outstanding Block/Deny) AND (b) the human has confirmed any Blocked args via `caprun confirm
<effect_id>`. This mirrors `DEC-layer-roles`: sandbox is the boundary, broker is the reference monitor,
adapters are the only paths to effects — the confined worker holds no send capability by construction
(reinforced kernel-side, see Kernel-Enforced Negative Net Assertion below).

**Secrets broker-only (MUST, D-04):** SMTP secrets/credentials (host, port, auth) MUST live ONLY in
the broker process. They MUST NOT appear in the worker's environment, in the worker's process
arguments, or in any plan-node payload that could reach the tainted/confined context. A `PlanNode`
carries only `{ sink, args: Vec<ValueNode> }` (locked, see above) — SMTP credentials are never a
`ValueNode` and never travel through the plan-node path.

**New adapter location (MUST):** The real adapter lives at `crates/brokerd/src/sinks/email_smtp.rs`
(broker-resident, NOT confined-worker-resident), replacing the existing `invoke_email_send_stub`
(per the RESEARCH Architectural Responsibility Map). This module is the ONLY code path that performs
the SMTP call.

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
   classification for `email.send`'s `subject`/`body`/`attachment` is confirmed to already exist at
   `crates/executor/src/sink_sensitivity.rs:93-98`; no new classification code is proposed anywhere
   downstream of this document (D-01, D-21).
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
5. **Worker-never-sends and secrets-broker-only are both stated as MUSTs**, with the adapter located
   at `crates/brokerd/src/sinks/email_smtp.rs` (D-03, D-04).
6. **The negative net assertion points at the existing seccomp mechanism**, not a new confinement
   primitive, and specifies Phase 13's reuse of the `confine-probe`/`negative_net` test pattern (D-05).
7. **The gate test target is Mailpit, local-only, with live SES explicitly out of scope** (D-06).
8. **CRLF/header-injection defense is specified with concrete, source-verified mechanics** (typed
   builder only, `dangerous_new_pre_encoded` and `format!`-built headers forbidden, `boring-tls`
   forbidden), AND a Phase-13 adversarial CRLF fixture test is mandated regardless of the library's
   by-construction defense (D-07, D-22).

If any condition fails, this document is NOT ready for `DESIGN-GATE-RECORD-v1.3.md` to record
APPROVED/UNBLOCKED.
