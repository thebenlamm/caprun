# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- 🚧 **v1.3 — Doc → Action Assistant** — Phases 12-17 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP — AgentOS v0 (Phases 1-4) — SHIPPED 2026-06-30</summary>

Full detail archived in [`milestones/v1.0-ROADMAP.md`](milestones/v1.0-ROADMAP.md).

- [x] **Phase 1: Substrate Foundation** (2/2 plans) — Cargo workspace, domain core, locked plan-node broker API — completed 2026-06-29
- [x] **Phase 2: Security Design Gate** (3/3 plans) — taint-model + plan-executor DESIGN docs that hard-gate all executor code — completed 2026-06-29
- [x] **Phase 3: Confinement & Mediation Substrate** (5/5 plans) — kernel confinement, broker reference monitor, fd-pass fs adapter, no-LLM substrate demo (Linux-verified 29/29) — completed 2026-06-29
- [x] **Phase 4: Value-Injection Security Demo (v0 DONE)** (5/5 plans) — §9 acceptance test passes with a genuine, audited taint chain — completed 2026-06-30

**v0 DONE gate cleared:** the §9 value-injection test blocks a tainted address at a mediated sink with literal-value confirmation; `mint_from_read` is the sole broker taint-mint site; stapled taint fails the test. `cargo test --workspace` = 51 green.

</details>

<details>
<summary>✅ v1.1 — Usable Runtime (Live §9 from the CLI) (Phases 5-7) — SHIPPED 2026-07-01</summary>

Full detail archived in [`milestones/v1.1-ROADMAP.md`](milestones/v1.1-ROADMAP.md).

**Milestone goal:** Turn the proven-in-tests value-injection defense into a real `caprun` run — a deterministic scripted planner turns an intent into PlanNodes, a confined worker drives toward a real `file.create` sink, and the deterministic I2 block fires on a genuine taint chain (with a clean, broker-minted allow-path too).

- [x] **Phase 5: Runtime Spine & Live §9 Email Block** (4/4 plans) — collapsed dual dispatch, session-scoped handle model (HARD-03), live §9 block with durable blocked-path audit (ACC-02) through the email.send stub — completed 2026-06-30
- [x] **Phase 6: Deterministic Planner & Intent Input** (5/5 plans) — typed intent → PlanNode planner, `mint_from_intent` `[UserTrusted]` values, executor predicate over `is_untrusted()` (HARD-02), clean allow-path reachable — completed 2026-07-01
- [x] **Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance** (6/6 plans) — real hardened `file.create` sink (schema gate, `O_EXCL`, dirfd + `openat2 RESOLVE_BENEATH`), mint invariant + typed `DenyReason`, durable genuine-taint anchor (ACC-07), full live §9 acceptance green on real Linux — completed 2026-07-01

**v1.1 DONE gate cleared:** a real kernel-confined `caprun` `file.create` run blocks a genuine-tainted path (no file, non-zero exit, durable `sink_blocked` anchor, no effect) and allows a trusted-intent path (`sink_executed`); each run is ONE unbroken causal chain (ACC-05); the canonical ACC-07 proof is a dispatch-level, after-exit, DB-alone anti-stapling sentinel + tamper-evidence. Verified on real Linux via Colima/Docker. All 14 Phase-7 requirement IDs Complete; verifier scored 14/14.

</details>

<details>
<summary>✅ v1.2 — Tainted Session, Human Gate (Phases 8-11) — SHIPPED 2026-07-07</summary>

Full detail archived in [`milestones/v1.2-ROADMAP.md`](milestones/v1.2-ROADMAP.md).

**Milestone goal:** A session that touches untrusted content is mechanically demoted to draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked sink arg can be released only by literal-value human confirmation — all deterministic, all in the audit DAG.

- [x] **Phase 8: Session-Trust & Confirmation Design Gate** (3/3 plans) — DESIGN doc for session-trust-state (I1 demotion + I0 creation rule) and confirmation-release semantics, reviewed before any executor code — completed 2026-07-06
- [x] **Phase 9: Session Trust State (I1 + I0)** (4/4 plans) — reading untrusted content or being seeded from external content demotes/starts a session as draft-only; draft-only sessions deny CommitIrreversible plan nodes via one executor TCB function — completed 2026-07-07
- [x] **Phase 10: Single-Shot Confirmation Loop** (3/3 plans) — `caprun confirm <effect_id>` shows the human the blocked literal + provenance and releases exactly one (sink, arg, literal-digest) triple; deny is durable — completed 2026-07-07
- [x] **Phase 11: Live Acceptance — Tainted Session, Human Gate** (1/1 plans) — live run on real Linux: hostile read → session demotion → sink block → human deny (nothing sent) / human confirm (exactly once), one unbroken audit chain — completed 2026-07-07

**v1.2 DONE gate cleared:** live on real Linux via Colima+Docker, a hostile workspace-file read demotes the session (I1), the same tainted value Blocks `file.create` (I2), and a human `caprun deny`/`caprun confirm` either durably blocks the effect or releases it exactly once — one unbroken audit-DAG causal chain (`fd_granted→file_read→session_demoted→sink_blocked→confirm_{denied,granted}`) for both outcomes (ACC-01/02/03). A pre-existing stale test assertion in `s9_live_block.rs` (dating to Phase 9, never previously run on Linux) was caught and fixed in the process. All 14 v1.2 requirement IDs Complete.

</details>

### 🚧 v1.3 — Doc → Action Assistant (Phases 12-17) — IN PROGRESS

**Milestone goal:** caprun ingests an untrusted document containing an embedded injection, deterministically extracts a "send to X" action (recipient + body derived from the doc's content, no LLM planner), and attempts a real email send. The read demotes the session (I1, existing); the tainted recipient AND body both block at the sink (I2 + new CONTENT-01); `caprun confirm`/`deny` shows verbatim recipient+body+provenance; confirm sends exactly once via a real broker-mediated SMTP adapter, deny sends nothing — one unbroken audit DAG for both outcomes, plus a clean-send negative control in the same run, proven live on real Linux via Colima+Docker.

- [ ] **Phase 12: Content, Adapter & Confirm-Binding Design Gate** - A reviewed, adversarially-reviewed DESIGN doc covering content-sensitivity semantics, real-adapter mediation, and confirm-binding exists before any executor/TCB code for this milestone is written
- [ ] **Phase 13: Real Broker-Mediated SMTP Adapter** - caprun can send a real email through a broker-mediated adapter — worker never touches the network, secrets never leave the broker, and the send is idempotent/failure-safe
- [ ] **Phase 14: Content-Sensitive Sink-Arg Blocking** - The executor blocks a tainted email body the same way it already blocks a tainted recipient
- [ ] **Phase 15: Deterministic Doc→Action Extraction** - A confined, deterministic extractor turns a hostile document's bytes into a plan node, with a proven unbroken audit-DAG edge from read to block, including through a transformation
- [ ] **Phase 16: Confirm UX, Literal Binding & Negative Controls** - A human sees the verbatim, provenance-narrated recipient and body before deciding; confirm is bound to the exact resolved literals; the gate is proven taint-driven, not a blanket email block
- [ ] **Phase 17: Live Acceptance & Framing Honesty** - The full doc→action chain runs live on real Linux as one unbroken audit DAG, composing the hostile block and the clean control in the same run, with honest public framing of what was and wasn't proven

## Phase Details

### Phase 12: Content, Adapter & Confirm-Binding Design Gate

**Goal**: A reviewed, adversarially-reviewed DESIGN doc exists — covering content-sensitivity semantics (CONTENT-01/02), real-adapter mediation (SMTP-01/02/03/05), and confirm-binding to resolved literals (CONFIRM-03) — gating all executor/TCB code written for this milestone, mirroring the v1.0 Phase 2 and v1.2 Phase 8 design-gate discipline.
**Depends on**: Phase 11 (v1.2 shipped; this is the first v1.3 phase)
**Requirements**: DESIGN-01
**Success Criteria** (what must be TRUE):

  1. A DESIGN doc exists under `planning-docs/` defining content-sensitivity classification for the email sink's body arg (CONTENT-01/02) as a single hardcoded match arm, not a general taxonomy.
  2. The same (or a paired) DESIGN doc defines the real SMTP adapter's mediation boundary — confined worker never performs the SMTP call, secrets live only in the broker, and the wire message is constructed so tainted literals cannot smuggle envelope/header changes (SMTP-01/02/03/05).
  3. The doc defines confirm-binding: `caprun confirm` binds to a hash of the exact resolved recipient+body literals, with no drift between confirm and send and no truncated display (CONFIRM-03).
  4. The doc has been adversarially reviewed and issues raised were resolved before sign-off.
  5. No executor/TCB code implementing CONTENT-01, SMTP-05, or CONFIRM-03 exists in the repo until this phase is marked complete.

**Plans**: TBD

### Phase 13: Real Broker-Mediated SMTP Adapter

**Goal**: caprun can actually send an email through a broker-mediated adapter — the confined worker never touches the network, SMTP secrets never leave the broker process, and the confirm-triggered send is idempotent and failure-safe.
**Depends on**: Phase 12
**Requirements**: SMTP-01, SMTP-02, SMTP-03, SMTP-05, SEND-01, SEND-02
**Success Criteria** (what must be TRUE):

  1. A confirmed effect results in a real email arriving at a local capture SMTP (MailHog/Mailpit), sent by the broker process — not the worker — and this is the acceptance-gate test target (Linux-verifiable, repeatable, no live infra dependency).
  2. A confined worker's direct attempt to open an SMTP connection FAILS under kernel-enforced default-deny net — a Linux negative assertion, not just code structure — and SMTP credentials are asserted absent from worker env/args and from any plan-node payload.
  3. A CRLF/header-injection fixture (a tainted body containing `\r\nBcc: attacker@...`) cannot alter envelope/recipients — the adapter constructs the wire message so this cannot smuggle recipients past the human's body confirm.
  4. A re-issued confirm, a broker restart mid-send, or a duplicate plan-node submission cannot double-fire — the audit DAG records exactly ONE send.
  5. An adapter failure after confirm (connection refused / 5xx) surfaces the error (never swallowed), is recorded in the DAG, and cannot silently retry into a double-send.

**Plans**: TBD

### Phase 14: Content-Sensitive Sink-Arg Blocking

**Goal**: The executor blocks a tainted email body the same way it already blocks a tainted recipient — I2 coverage extends from routing/recipient args to content args, reopening and closing the `CONTENT-01` gap deferred at v1.2.
**Depends on**: Phase 12
**Requirements**: CONTENT-01, CONTENT-02
**Success Criteria** (what must be TRUE):

  1. Submitting a plan node with a tainted value occupying the email sink's body arg is Blocked by the executor, with the same literal-value human-confirm UX as the existing routing/recipient block.
  2. The content-sensitivity classification is implemented as one hardcoded match arm in the executor TCB, scoped to the email sink's args only — not a reusable content-classification framework.

**Plans**: TBD

### Phase 15: Deterministic Doc→Action Extraction

**Goal**: A confined, deterministic (non-LLM) extractor turns a hostile document's bytes into a "send to X" plan node inside the sandbox, and a programmatic audit-DAG query proves the taint genuinely propagated from that read — including through a transformation, not just a copy — to the blocked sink args.
**Depends on**: Phase 12, Phase 14
**Requirements**: EXTRACT-01, EXTRACT-02, EXTRACT-03, CONFIRM-02
**Success Criteria** (what must be TRUE):

  1. A realistic hostile-doc fixture exists (an embedded injection attempting to redirect/alter the send) for reuse across extraction tests, confirm tests, and the live demo.
  2. The extractor that derives the recipient+body plan node from doc bytes runs entirely inside the confined worker, over hostile bytes, and only emits plan nodes — never in the broker control plane.
  3. A programmatic audit-DAG query proves an unbroken edge path (raw-read Event → extractor-derived ValueNodes → blocked sink args) and FAILS if any edge is missing, distinguishing a genuine chain from a value stapled fresh at the sink.
  4. At least one fixture shows the extractor transforming the tainted value before the sink (concatenating two doc fields, or base64-decoding a body) with taint still propagating and the block still firing — proving survival of manipulation, not just copying.

**Plans**: TBD

### Phase 16: Confirm UX, Literal Binding & Negative Controls

**Goal**: A human sees the verbatim, provenance-narrated recipient and body before deciding, the confirm is cryptographically bound to the exact resolved literals so send cannot drift from what was shown, and two negative controls prove the gate is taint-driven rather than a blanket email block.
**Depends on**: Phase 13, Phase 14, Phase 15
**Requirements**: CONFIRM-01, CONFIRM-03, CONFIRM-04, CONTROL-01, CONTROL-02
**Success Criteria** (what must be TRUE):

  1. `caprun confirm`/`deny` on a doc-derived send displays the verbatim recipient AND body (never truncated, even for long bodies), plus provenance, for an effect blocked at I2+CONTENT-01.
  2. The block moment narrates provenance — recipient/body → untrusted doc → these bytes → this sink arg — rather than a bare "Error: blocked."
  3. Confirm binds to a hash of the exact resolved recipient+body literals; the plan node cannot drift between confirm time and send time.
  4. A fully-trusted send (recipient+body from a trusted, non-doc source) proceeds with NO block and NO confirm gate, in the same acceptance run as the hostile block — proving the gate is taint-driven.
  5. A send with a tainted body but a trusted recipient still blocks — proving the body dimension isn't dead code redundant with the routing block.

**Plans**: TBD

### Phase 17: Live Acceptance & Framing Honesty

**Goal**: The full doc→action chain — hostile read, I1 demotion, deterministic extraction, tainted recipient+body block, confirm-send/deny — runs live on real Linux as one unbroken audit DAG, composed in the same run as the clean-send negative control, and the project's public claims honestly scope what v1.3 does and does not prove.
**Depends on**: Phase 13, Phase 15, Phase 16
**Requirements**: ACCEPT-01, DOC-01
**Success Criteria** (what must be TRUE):

  1. A live Colima+Docker run shows: hostile doc read → I1 demotion → deterministic extraction → tainted recipient+body block (I2+CONTENT-01) → confirm sends exactly once via the real adapter to a local capture SMTP → deny sends nothing.
  2. The same run composes the clean-send negative control (CONTROL-01) alongside the hostile block, and the whole scenario is one unbroken audit-DAG causal chain.
  3. PROJECT.md explicitly states that v1.3 proves taint ENFORCEMENT through a deterministic extractor with genuine propagation, and does NOT claim taint survives a real LLM planner's regeneration ("laundering") — no external claim contradicts this.

**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Substrate Foundation | v1.0 | 2/2 | Complete | 2026-06-29 |
| 2. Security Design Gate | v1.0 | 3/3 | Complete | 2026-06-29 |
| 3. Confinement & Mediation Substrate | v1.0 | 5/5 | Complete | 2026-06-29 |
| 4. Value-Injection Security Demo (v0 DONE) | v1.0 | 5/5 | Complete | 2026-06-30 |
| 5. Runtime Spine & Live §9 Email Block | v1.1 | 4/4 | Complete | 2026-06-30 |
| 6. Deterministic Planner & Intent Input | v1.1 | 5/5 | Complete | 2026-07-01 |
| 7. file.create Sink, Enforcement Hardening & Full Acceptance | v1.1 | 6/6 | Complete | 2026-07-01 |
| 8. Session-Trust & Confirmation Design Gate | v1.2 | 3/3 | Complete | 2026-07-06 |
| 9. Session Trust State (I1 + I0) | v1.2 | 4/4 | Complete | 2026-07-07 |
| 10. Single-Shot Confirmation Loop | v1.2 | 3/3 | Complete | 2026-07-07 |
| 11. Live Acceptance — Tainted Session, Human Gate | v1.2 | 1/1 | Complete | 2026-07-07 |
| 12. Content, Adapter & Confirm-Binding Design Gate | v1.3 | 0/0 | Not started | - |
| 13. Real Broker-Mediated SMTP Adapter | v1.3 | 0/0 | Not started | - |
| 14. Content-Sensitive Sink-Arg Blocking | v1.3 | 0/0 | Not started | - |
| 15. Deterministic Doc→Action Extraction | v1.3 | 0/0 | Not started | - |
| 16. Confirm UX, Literal Binding & Negative Controls | v1.3 | 0/0 | Not started | - |
| 17. Live Acceptance & Framing Honesty | v1.3 | 0/0 | Not started | - |
