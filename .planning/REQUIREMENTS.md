# Requirements: AgentOS (caprun) — v1.3 "Doc → Action Assistant"

**Defined:** 2026-07-07
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink.

Scoped with caprun-opus-77 (coordinator, delegated by Ben Lamm) plus an advisor panel pass. Reopens two v1.2-locked decisions (`CONTENT-01`, real SMTP adapter); does not reopen the LLM-planner lock. See `.planning/PROJECT.md` Key Decisions for full rationale.

## v1 Requirements

### SMTP (broker-mediated adapter)

- [ ] **SMTP-01**: Broker-mediated adapter sends email only after executor-authorize + human-confirm; the confined worker never performs the SMTP call. Proven with a Linux NEGATIVE assertion: the confined worker attempting a direct SMTP connection FAILS (default-deny net) — a kernel-enforced claim, not just code structure.
- [ ] **SMTP-02**: SMTP secrets/credentials live only in the broker process. Asserted absent from worker env/args AND from any plan-node payload.
- [x] **SMTP-03**: Acceptance-gate test targets a local capture SMTP (MailHog/Mailpit) — Linux-verifiable, repeatable, no live infra dependency.
- [x] **SMTP-05**: Adapter constructs the wire message so tainted literals CANNOT alter envelope/headers (CRLF/header injection) — a tainted body containing `\r\nBcc: attacker@...` must not smuggle recipients past the human's body confirm. Tested with a CRLF injection fixture.

### CONTENT (reopens v1.2's deferred CONTENT-01)

- [ ] **CONTENT-01**: Executor blocks a tainted value occupying a content-sensitive sink arg (email body), not just routing/recipient — same literal-value confirm UX as existing I2 routing blocks.
- [ ] **CONTENT-02**: Content-sensitivity classification is hardcoded in the executor TCB for the email sink's args ONLY (one match arm) — not a general content-classification taxonomy/abstraction.

### EXTRACT (deterministic doc→action)

- [ ] **EXTRACT-01**: The deterministic (non-LLM) extractor runs CONFINED (worker-side, over hostile bytes), emitting only plan nodes — not in the broker control plane (a parser over hostile bytes is attack surface).
- [ ] **EXTRACT-02**: A programmatic audit-DAG query proves an unbroken edge path: raw-read Event → extractor-derived ValueNodes → blocked sink args. Test FAILS if any edge is missing; an anti-staple check distinguishes/rejects a value minted fresh at the sink. Per Phase 12's collect-then-Block mandate, this must hold for EVERY blocked arg in a multi-arg Block (e.g. tainted recipient + tainted body on the same plan node), not just one.
- [ ] **EXTRACT-03**: ≥1 fixture where the extractor TRANSFORMS the tainted value before the sink (e.g. concatenates two doc fields into the recipient, or base64-decodes a body) and taint STILL propagates + STILL blocks — proves taint survives a manipulation, not just a copy. Scope is honest: no universality claim.

### CONFIRM (UX + fixture)

- [ ] **CONFIRM-01**: `caprun confirm`/`deny` displays the verbatim recipient AND body (not just recipient) plus provenance, for a doc-derived send blocked at I2+CONTENT-01.
- [ ] **CONFIRM-02**: A realistic doc fixture exists (embedded injection attempting to redirect/alter the send) for gate-test and live-demo use.
- [ ] **CONFIRM-03**: `caprun confirm` binds to ONE combined hash covering the FULL SET of blocked args' exact resolved literals (recipient AND body together) — so the bytes the human read are provably the bytes the adapter sends; the plan node cannot drift between confirm and send; no truncated display of long bodies; no partial-set confirm. Builds on the existing `PendingConfirmation` resolved_args store.
- [ ] **CONFIRM-04**: The BLOCK moment narrates provenance for EVERY blocked arg in the set, not "Error: blocked" and not just the first-matched arg — renders recipient/body → untrusted doc → these bytes → this sink arg, for each. The block is the demo's climax.

### CONTROL (negative controls)

- [ ] **CONTROL-01**: A fully-TRUSTED send (recipient+body from a trusted/first-party source, not the doc) proceeds with NO block and NO confirm gate — proves the gate is TAINT-driven, not "blocks all email." Runs in the SAME acceptance run as the hostile block (the A/B that turns the demo into a controlled experiment, not an anecdote).
- [ ] **CONTROL-02**: Body TAINTED + recipient TRUSTED → STILL blocks. Proves the body dimension isn't dead code / redundant with the routing block — without it, CONTENT-01 is vacuously satisfied by the recipient block alone.

### SEND (idempotency — bounded)

- [x] **SEND-01**: The confirm-triggered send is idempotent — a re-issued confirm, a broker restart mid-send, or a duplicate plan-node submission CANNOT double-fire; the audit DAG records exactly ONE send. Bound to single-confirm idempotency — NOT distributed exactly-once/delivery semantics.
- [x] **SEND-02**: Adapter send failing AFTER confirm (connection refused / 5xx) surfaces the error (never swallowed), records it in the DAG, and no silent retry can double-send; confirm-token consumption is defined for the failure path.

### DESIGN (process gate)

- [ ] **DESIGN-01**: A reviewed DESIGN doc (content-sensitivity semantics + real-adapter mediation + confirm-binding) exists and is ADVERSARIALLY reviewed BEFORE any executor/TCB code — same discipline as v1.0/v1.2. This is the roadmap's Phase 1 (design gate); CONTENT-01/SMTP-05/CONFIRM-03 executor code may not precede it.

### DOC (framing honesty)

- [ ] **DOC-01**: PROJECT.md explicitly scopes what v1.3 DOES and does NOT prove. It proves taint ENFORCEMENT (a live tag blocks + gates on the literal value) with genuine propagation through a deterministic extractor. It does NOT prove taint survives a real LLM planner — a real model can regenerate a tainted value as fresh model-authored tokens with no provenance ("laundering"). The deterministic planner is compromised BY DESIGN (correct threat model); taint-survives-a-real-agent is explicitly v1.4+. No external claim may say otherwise.

### ACCEPT (hero demo — composes the above)

- [ ] **ACCEPT-01**: Full live acceptance, Linux-verified via Colima+Docker, ONE unbroken audit DAG: hostile doc read → I1 demotion → deterministic extraction → tainted recipient+body block (I2+CONTENT-01) → confirm sends exactly once (real adapter → local capture SMTP) → deny sends nothing. Composes CONTROL-01 (clean send, ungated) alongside the hostile block in the SAME run.

## v2 Requirements

### Live send target

- **SMTP-04** (downgraded from a v1.3 requirement): Live demo path can optionally target real SES/live inbox via config-swap, post-milestone. NOT gated, NOT required for ACCEPT-01.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Live SES / real inbox send as a gated requirement | MailHog/Mailpit already constitutes a real SMTP send with observable arrival; live SES adds credential/DNS/deliverability fragility and a live default-deny-net exception for ~zero legibility gain |
| General content-classification taxonomy/abstraction | CONTENT-02 hardcodes sensitivity for the email sink's args only; a reusable framework is unvalidated speculation |
| LLM planner / taint-survives-a-real-agent claim | Deterministic planner stays locked; DOC-01 requires this milestone to state honestly that it does not prove taint survives LLM regeneration — that's v1.4+ |
| Distributed exactly-once send semantics | SEND-01 is bounded to single-confirm idempotency only |
| Git/GitHub adapters, Cedar, cross-host delegation, gVisor/Firecracker | Unchanged from v1.2 — see PROJECT.md |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-01 | Phase 12 | Pending |
| SMTP-01 | Phase 13 | Pending |
| SMTP-02 | Phase 13 | Pending |
| SMTP-03 | Phase 13 | Complete |
| SMTP-05 | Phase 13 | Complete |
| CONTENT-01 | Phase 14 | Pending |
| CONTENT-02 | Phase 14 | Pending |
| EXTRACT-01 | Phase 15 | Pending |
| EXTRACT-02 | Phase 15 | Pending |
| EXTRACT-03 | Phase 15 | Pending |
| CONFIRM-01 | Phase 16 | Pending |
| CONFIRM-02 | Phase 15 | Pending |
| CONFIRM-03 | Phase 16 | Pending |
| CONFIRM-04 | Phase 16 | Pending |
| CONTROL-01 | Phase 16 | Pending |
| CONTROL-02 | Phase 16 | Pending |
| SEND-01 | Phase 13 | Complete |
| SEND-02 | Phase 13 | Complete |
| DOC-01 | Phase 17 | Pending |
| ACCEPT-01 | Phase 17 | Pending |

**Coverage:**

- v1 requirements: 20 total
- Mapped to phases: 20/20 ✓
- Unmapped: 0

---
*Requirements defined: 2026-07-07*
*Last updated: 2026-07-07 after ROADMAP.md creation — all 20 v1 requirements mapped to Phases 12-17, Phase 12 (DESIGN-01) is the mandatory design gate preceding CONTENT-01/SMTP-05/CONFIRM-03 executor code.*
