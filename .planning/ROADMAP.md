# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- ✅ **v1.3 — Doc → Action Assistant** — Phases 12-17 (shipped 2026-07-09)

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
