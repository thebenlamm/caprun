# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- ✅ **v1.3 — Doc → Action Assistant** — Phases 12-17 (shipped 2026-07-09)
- ✅ **v1.4 — Trust-Boundary Integrity & the Adversarial Planner** — Phases 18-22 (shipped 2026-07-11)
- 🚧 **v1.5 — Slot-Type Binding Enforcement (T2)** — Phases 23-25 (in progress)

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

<details>
<summary>✅ v1.3 — Doc → Action Assistant (Phases 12-17) — SHIPPED 2026-07-09</summary>

Full detail archived in [`milestones/v1.3-ROADMAP.md`](milestones/v1.3-ROADMAP.md).

**Milestone goal:** caprun ingests an untrusted document containing an embedded injection, deterministically extracts a "send to X" action (recipient + body derived from the doc's content, no LLM planner), and attempts a real email send. The read demotes the session (I1, existing); the tainted recipient AND body both block at the sink (I2 + new CONTENT-01); `caprun confirm`/`deny` shows verbatim recipient+body+provenance; confirm sends exactly once via a real broker-mediated SMTP adapter, deny sends nothing — one unbroken audit DAG for both outcomes, plus a clean-send negative control in the same run, proven live on real Linux via Colima+Docker.

- [x] **Phase 12: Content, Adapter & Confirm-Binding Design Gate** (3/3 plans) - A reviewed, adversarially-reviewed DESIGN doc covering content-sensitivity semantics, real-adapter mediation, and confirm-binding exists before any executor/TCB code for this milestone is written (completed 2026-07-07)
- [x] **Phase 13: Real Broker-Mediated SMTP Adapter** (4/4 plans) - caprun can send a real email through a broker-mediated adapter — worker never touches the network, secrets never leave the broker, and the send is idempotent/failure-safe (completed 2026-07-08)
- [x] **Phase 14: Content-Sensitive Sink-Arg Blocking** (2/2 plans) - The executor blocks a tainted email body the same way it already blocks a tainted recipient (completed 2026-07-08)
- [x] **Phase 15: Deterministic Doc→Action Extraction** (4/4 plans) - A confined, deterministic extractor turns a hostile document's bytes into a plan node, with a proven unbroken audit-DAG edge from read to block, including through a transformation (completed 2026-07-08, independently verified live on Linux — 8/8 must-haves)
- [x] **Phase 16: Confirm UX, Literal Binding & Negative Controls** (4/4 plans) - A human sees the verbatim, provenance-narrated recipient and body before deciding; confirm is bound to the exact resolved literals; the gate is proven taint-driven, not a blanket email block (completed 2026-07-09, independently verified — 10/10 checks, real exfiltration hole confirmed closed in source)
- [x] **Phase 17: Live Acceptance & Framing Honesty** (4/4 plans) - The full doc→action chain runs live on real Linux as one unbroken audit DAG, composing the hostile block and the clean control in the same run, with honest public framing of what was and wasn't proven (completed 2026-07-09, independently re-verified by both caprun-sonnet-77 and caprun-opus-77 — 250/250 tests passed on real Linux, HARD GATE tooth #2 confirmed genuine not stapled, DOC-01 prose read confirmed honest)

**v1.3 DONE gate cleared:** live on real Linux via Colima+Docker, ONE shared audit.db across 3 sessions (`live_acceptance_v1_3_composed`) — a hostile doc read demotes the session (I1), deterministic extraction derives a tainted recipient+body pair, the executor Blocks both (I2+CONTENT-01) with genuinely-propagated (not stapled) taint re-proven against these exact live anchors, `caprun confirm` sends exactly once via the real SMTP adapter, a SEPARATE hostile block is denied sending nothing (Mailpit count==0 AND no send-attempt ledger entry), and a clean trusted-intent send is Allowed and delivers ungated in the SAME run — all 3 sessions independently `verify_chain`-true. All 20 v1.3 requirement IDs Complete. No git tag (Ben's call).

</details>

<details>
<summary>✅ v1.4 — Trust-Boundary Integrity & the Adversarial Planner (Phases 18-22) — SHIPPED 2026-07-11</summary>

**Milestone goal:** Fix a confirmed live cross-connection trust bypass in the broker (Phase 0 — a security fix, gated by an already-red regression test), then prove the trust boundary is indifferent to planner intelligence by putting an adversarial LLM planner behind it (Phase 1+) — a hostile injected document makes the planner *comply* and try to route a tainted value to `email.send`, and the executor **Blocks deterministically** anyway, with genuine taint propagation re-verified live (the §9 standard: `verify_chain` true, Mailpit == 0), because the value flows around the planner through the worker's own mint sites, never through the planner's tokens.

- [x] **Phase 18: Trust-Boundary Coherence Design Gate** - A DESIGN doc resolving the cross-connection fix shape, the replay-risk re-earning, the three-mint-site audit, the decision-oracle question, the forward-looking per-verb capability split, and guard-(c)'s status exists and clears a fresh adversarial review, before any `server.rs` change (completed 2026-07-11)
- [x] **Phase 19: Cross-Connection Trust Coherence Fix** - The broker rejects a second connection to an already-active session, closing the cross-connection `ProvideIntent` bypass; the regression test goes green by fixing the broker, never by weakening its assertions (completed 2026-07-11)
- [x] **Phase 20: Planner Seam & Capability Split** - A designed `Planner` trait/seam exists, a planner-role connection can never hold a mint verb, and the planner is structurally kept out of the worker's raw-bytes path (completed 2026-07-11)
- [x] **Phase 21: Adversarial LLM Planner** - A minimal LLM-backed planner, running behind the new seam, emits only `PlanNode{sink, args}` — no literal field to carry (completed 2026-07-11)
- [x] **Phase 22: Adversarial Gate Proof & Residual Disclosure** - A hostile-doc-primed planner complies and is Blocked deterministically with genuine, live-verified taint propagation; T2 is documented as the accepted v1.4 residual (completed 2026-07-11)

**v1.4 DONE gate cleared:** live on real Linux, a hostile document's injection reaches a genuine OpenAI-backed `LlmPlanner` via a taint-tracked `task_instruction` channel (never itself a sink-arg value); the model complies and routes the tainted handle to `to`; the executor Blocks it deterministically (`verify_chain` true, Mailpit==0 for the attacker); a trusted-intent control in the SAME composed run Allows and delivers exactly once. Full default `scripts/mailpit-verify.sh` recipe: 46 test groups, 0 failed, real exit 0. T2 (slot-type binding) documented as the accepted residual, deferred to v1.5. All v1.4 requirement IDs Complete. No git tag, not pushed (Ben's call).

</details>

### 🚧 v1.5 — Slot-Type Binding Enforcement (T2) (Phases 23-25) — IN PROGRESS

**Milestone goal:** Close v1.4's accepted residual #5 (T2) — the executor gains a structural check that a resolved value's semantic origin matches the semantic role of the plan-node slot it's routed into, so a misrouted `UserTrusted` handle (e.g. a subject-typed string landed in `to`) is caught even though it is neither untrusted (I2 doesn't fire) nor a class-level deny (I0/I1 don't apply).

- [x] **Phase 23: Slot-Type Binding Design Gate** - A DESIGN doc for slot-type binding enforcement exists, unifies with the existing `claim_type` taxonomy, resolves derivation role propagation, and pins the fail-closed default — clearing a fresh (non-self) adversarial review before any TCB code (completed 2026-07-12)
- [ ] **Phase 24: Slot-Type Binding Enforcement** - The executor structurally enforces that a resolved value's origin role matches its plan-node slot's expected role, via a new mint-time tag, a hardcoded expected-role table, and a new exhaustive `DenyReason` variant
- [ ] **Phase 25: Regression & Live Proof** - A deliberately swapped subject/recipient handle pair is proven to produce the new deny with an unbroken audit chain, existing tests are updated for the new check, and the full workspace regression passes green on real Linux

## Phase Details

### Phase 23: Slot-Type Binding Design Gate

**Goal**: A DESIGN doc for slot-type binding enforcement exists — specifying the origin-role tagging mechanism, unifying with the existing `claim_type` taxonomy, resolving role propagation through derivation, and pinning the fail-closed default — and clears a fresh (non-self) adversarial review before any `crates/executor` or `crates/brokerd` mint-site code is written (mirrors the v1.0 Phase 2 / v1.2 Phase 8 / v1.3 Phase 12 / v1.4 Phase 18 design-gate discipline).
**Depends on**: Phase 22 (v1.4 shipped; this is the first v1.5 phase)
**Requirements**: DESIGN-07, DESIGN-08, DESIGN-09, DESIGN-10
**Success Criteria** (what must be TRUE):

  1. `planning-docs/DESIGN-slot-type-binding.md` exists and specifies the origin-role tagging mechanism, the new `DenyReason` variant's shape and its full exhaustive-match blast radius, and the collect-vs-deny-immediately ordering ruling (whether a slot-type mismatch joins the collect-then-Block `BlockedPendingConfirmation` set or returns a hard `Denied`).
  2. The doc unifies with the existing `claim_type` taxonomy in `crates/brokerd/src/quarantine.rs` for untrusted-origin values, and defines analogous role tags from scratch for `ProvideIntent`-minted `UserTrusted` values (recipient/subject/body).
  3. The doc explicitly resolves role propagation through `mint_from_derivation` (e.g. `ReportDerivedClaim`'s `Concat` transform over a Reply-To/Domain pair) — what role, if any, a derived/composite value carries — not left implicit.
  4. The doc pins the fail-closed default: a value with no assigned role, or a role that isn't in the expected-role table for the target slot, hitting a role-checked slot is a `Deny` — never a silent pass-through to `Allowed`.
  5. The doc has cleared a fresh (non-self) adversarial review with every raised finding resolved, and no `crates/executor` or `crates/brokerd` mint-site code exists yet.

**Plans**: 2/2 plans complete
**Wave 1**

- [x] 23-01-PLAN.md — Author `planning-docs/DESIGN-slot-type-binding.md` (§0-§10): origin-role tag, claim_type unification, expected-role table, derivation propagation, ordering ruling, fail-closed default (DESIGN-07..10)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 23-02-PLAN.md — Fresh non-self adversarial review (traces code, re-runs the DenyReason blast-radius grep), resolve every finding, gate record `DESIGN-GATE-RECORD-v1.5.md` reads CLEARED (DESIGN-07..10)

### Phase 24: Slot-Type Binding Enforcement

**Goal**: The executor structurally enforces that a resolved value's semantic origin role matches its plan-node slot's expected role, per Phase 23's design ruling — closing the v1.4 T2 residual (a misrouted `UserTrusted` handle is now caught even though it is neither untrusted nor a class-level deny).
**Depends on**: Phase 23 (design gate must clear first — no TCB code before it)
**Requirements**: T2-02, T2-03, T2-04, T2-05
**Success Criteria** (what must be TRUE):

  1. Every minted value (via `mint_from_intent`, `mint_from_read`, `mint_from_derivation`) carries a semantic origin-role tag, added as an additive, mechanical change to the three mint call sites' signatures; I0/I1 trust classification (which values become `UserTrusted` vs untrusted) is unaffected.
  2. A hardcoded per-sink-arg "expected role" table exists in `crates/executor`, scoped to the two live sinks (`email.send`, `file.create`), mirroring the `sink_sensitivity.rs` CONTENT-01/02 precedent — not a general framework.
  3. A new exhaustive `DenyReason` variant exists for a slot-type mismatch (no wildcard arm), and every existing exhaustive match over `DenyReason` across the workspace (CLI rendering, audit serialization/`code()`/`Display`, existing tests) is updated for the new arm — not just the match inside `submit_plan_node`.
  4. `submit_plan_node` denies (or blocks, per Phase 23's ordering ruling) a plan node when a resolved value's origin role doesn't match its slot's expected role, evaluated per-arg in the same pass as the existing routing/content-sensitivity check, without weakening or reordering the existing I0 (Step 0.5 class-deny) / I2 (per-arg Block) precedence.

**Plans**: 3 plans (2 waves)
**Wave 1**

- [ ] 24-01-PLAN.md — T2-02: origin-role tag on ValueRecord + threading through ValueStore::mint, the 3 mint_from_* wrappers, and 5 server.rs dispatch sites, plus the ~63 compilation-forced test-fixture updates (Wave 1)
- [ ] 24-02-PLAN.md — T2-04: new exhaustive DenyReason::SlotTypeMismatch variant + both code()/Display matches (Wave 1, parallel)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 24-03-PLAN.md — T2-03 + T2-05: hardcoded expected-role table + Step 1c fail-closed enforcement in submit_plan_node (Wave 2)

### Phase 25: Regression & Live Proof

**Goal**: The T2 gap is demonstrably closed on real Linux — a deliberately swapped subject/recipient handle pair produces the new deny with an unbroken audited chain, existing tests no longer rely on permissive `UserTrusted`-in-any-slot behavior, and the full workspace regression is independently re-verified green (the v1.5 DONE gate).
**Depends on**: Phase 24
**Requirements**: T2-06, T2-07, T2-08
**Success Criteria** (what must be TRUE):

  1. A held-out acceptance test proves a plan node with a deliberately swapped subject↔recipient handle pair (both `UserTrusted`, both otherwise valid) produces the new deny, with a corresponding audit-DAG event recorded and `verify_chain` still true.
  2. Existing tests that currently rely on permissive `UserTrusted`-in-any-slot behavior are identified via a regression audit and updated so the new check isn't silently bypassed or broken by a fixture that never assigns a role.
  3. `scripts/mailpit-verify.sh` is independently re-run green (0 failures) after the change lands — not assumed from a prior pass, per this project's standing milestone-close discipline.

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
| 12. Content, Adapter & Confirm-Binding Design Gate | v1.3 | 3/3 | Complete   | 2026-07-07 |
| 13. Real Broker-Mediated SMTP Adapter | v1.3 | 4/4 | Complete    | 2026-07-08 |
| 14. Content-Sensitive Sink-Arg Blocking | v1.3 | 2/2 | Complete    | 2026-07-08 |
| 15. Deterministic Doc→Action Extraction | v1.3 | 4/4 | Complete    | 2026-07-08 |
| 16. Confirm UX, Literal Binding & Negative Controls | v1.3 | 4/4 | Complete    | 2026-07-09 |
| 17. Live Acceptance & Framing Honesty | v1.3 | 4/4 | Complete | 2026-07-09 |
| 18. Trust-Boundary Coherence Design Gate | v1.4 | 2/2 | Complete    | 2026-07-11 |
| 19. Cross-Connection Trust Coherence Fix | v1.4 | 2/2 | Complete    | 2026-07-11 |
| 20. Planner Seam & Capability Split | v1.4 | 3/3 | Complete    | 2026-07-11 |
| 21. Adversarial LLM Planner | v1.4 | 4/4 | Complete    | 2026-07-11 |
| 22. Adversarial Gate Proof & Residual Disclosure | v1.4 | 3/3 | Complete    | 2026-07-11 |
| 23. Slot-Type Binding Design Gate | v1.5 | 2/2 | Complete    | 2026-07-12 |
| 24. Slot-Type Binding Enforcement | v1.5 | 0/TBD | Not started | - |
| 25. Regression & Live Proof | v1.5 | 0/TBD | Not started | - |
