# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- ✅ **v1.3 — Doc → Action Assistant** — Phases 12-17 (shipped 2026-07-09)
- 🚧 **v1.4 — Trust-Boundary Integrity & the Adversarial Planner** — Phases 18-22 (in progress)

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

### 🚧 v1.4 — Trust-Boundary Integrity & the Adversarial Planner (Phases 18-22) — IN PROGRESS

**Milestone goal:** Fix a confirmed live cross-connection trust bypass in the broker (Phase 0 — a security fix, gated by an already-red regression test), then prove the trust boundary is indifferent to planner intelligence by putting an adversarial LLM planner behind it (Phase 1+) — a hostile injected document makes the planner *comply* and try to route a tainted value to `email.send`, and the executor **Blocks deterministically** anyway, with genuine taint propagation re-verified live (the §9 standard: `verify_chain` true, Mailpit == 0), because the value flows around the planner through the worker's own mint sites, never through the planner's tokens.

- [x] **Phase 18: Trust-Boundary Coherence Design Gate** - A DESIGN doc resolving the cross-connection fix shape, the replay-risk re-earning, the three-mint-site audit, the decision-oracle question, the forward-looking per-verb capability split, and guard-(c)'s status exists and clears a fresh adversarial review, before any `server.rs` change (completed 2026-07-11)
- [x] **Phase 19: Cross-Connection Trust Coherence Fix** - The broker rejects a second connection to an already-active session, closing the cross-connection `ProvideIntent` bypass; the regression test goes green by fixing the broker, never by weakening its assertions (completed 2026-07-11)
- [ ] **Phase 20: Planner Seam & Capability Split** - A designed `Planner` trait/seam exists, a planner-role connection can never hold a mint verb, and the planner is structurally kept out of the worker's raw-bytes path
- [ ] **Phase 21: Adversarial LLM Planner** - A minimal LLM-backed planner, running behind the new seam, emits only `PlanNode{sink, args}` — no literal field to carry
- [ ] **Phase 22: Adversarial Gate Proof & Residual Disclosure** - A hostile-doc-primed planner complies and is Blocked deterministically with genuine, live-verified taint propagation; T2 is documented as the accepted v1.4 residual

## Phase Details

### Phase 18: Trust-Boundary Coherence Design Gate

**Goal**: A DESIGN doc resolving the cross-connection trust-coherence fix shape, the replay-risk framing under an adaptive-planner threat model, a full three-mint-site audit, the decision-oracle question, the forward-looking per-verb capability split, and guard-(c)'s status exists and clears a fresh adversarial review — before any `server.rs` code change (mirrors the v1.0 Phase 2 / v1.2 Phase 8 / v1.3 Phase 12 design-gate discipline).
**Depends on**: Phase 17 (v1.3 shipped; this is the first v1.4 phase)
**Requirements**: DESIGN-01, DESIGN-02, DESIGN-03, DESIGN-04, DESIGN-05, DESIGN-06
**Success Criteria** (what must be TRUE):

  1. `planning-docs/DESIGN-session-trust-coherence.md` exists, specifies the fix shape (reject a 2nd connection to an already-active session) as the chosen approach over shared coherent multi-connection state, and a fresh adversarial panel (not the authoring session — per `DEC-ai-review-satisfies-human-gate`) has reviewed it with every raised issue resolved before any `server.rs` change begins.
  2. The doc rules on MAJOR-2 (replay risk), re-earning "accepted" in writing against the new adaptive-planner threat model — amplification stays bounded to trusted/human-typed recipients (untrusted still Blocks), no new CAS added this milestone.
  3. The doc audits all three mint sites (`mint_from_read`, `mint_from_intent`, `mint_from_derivation`) and states the corrected, narrower claim: only `ProvideIntent` yields a TRUSTED handle from a supplied string.
  4. The doc rules on MEDIUM-1 (the decision oracle) — whether Phase 1's planner connection sees the full `Allowed`/`BlockedPendingConfirmation{anchors, literal_sha256}` decision or a reduced signal.
  5. The doc specifies the per-verb capability split (a connection may hold NO mint verb: `ProvideIntent`/`ReportClaims`/`ReportDerivedClaim`) that Phase 1's planner connection will rely on.
  6. The doc re-confirms guard-(c) (`CAPRUN_ENABLE_IPC_CREATE_SESSION`) is not widened by the Phase-0 fix and re-states whether it should finally be compile-excluded.

**Plans**: 2/2 plans complete
**Wave 1**

- [x] 18-01-PLAN.md — Author `DESIGN-session-trust-coherence.md` resolving DESIGN-01..06 (fix shape, replay re-earning, three-mint-site audit, decision oracle, per-verb capability split, guard-(c) status)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 18-02-PLAN.md — Fresh adversarial gate: independent code-tracing review → `DESIGN-GATE-RECORD-v1.4.md`, resolve every finding, gate CLEARED before Phase 19

### Phase 19: Cross-Connection Trust Coherence Fix

**Goal**: The broker rejects a second connection to an already-active session, closing the cross-connection `ProvideIntent` bypass that let a worker mint an attacker-controlled `UserTrusted` literal and route it to `email.send` as `Allowed`.
**Depends on**: Phase 18
**Requirements**: TRUST-01, TRUST-02, TRUST-03, DOC-02
**Success Criteria** (what must be TRUE):

  1. A second connection attempt to an already-active session is rejected by the broker — the "smaller hammer" fix specified in Phase 18's DESIGN doc, implemented in `server.rs`.
  2. `crates/brokerd/tests/two_connection_intent_bypass.rs`'s `#[ignore]` is removed and the test passes green, with its safe-outcome assertions completely unchanged from what was written pre-fix.
  3. `scripts/mailpit-verify.sh`'s full existing test suite (the v1.3 live acceptance) is independently re-run on real Linux and still passes — no regression, not assumed from a prior pass.
  4. PROJECT.md's DOC-02 correction is finalized against the shipped fix (the scoping-time draft disclosure is confirmed accurate against the actual fix, not left aspirational).

**Plans**: 2/2 plans complete

**Wave 1**

- [x] 19-01-PLAN.md — One-way accept-loop occupancy latch in `server.rs` + restructure `two_connection_intent_bypass.rs` into 3 fresh-broker variants (guard-a control, overlapping, new sequential-reconnect), un-ignore (TRUST-01, TRUST-02)

**Wave 2** *(depends on Wave 1)*

- [x] 19-02-PLAN.md — Live Linux `mailpit-verify.sh` rerun with captured pass/fail counts (RED→GREEN proof + no-regression) + finalize PROJECT.md's DOC-02 disclosure against the shipped fix (TRUST-03, DOC-02)

### Phase 20: Planner Seam & Capability Split

**Goal**: A designed `Planner` seam exists in code, a connection identifying itself in the planner role can never hold a mint verb, and the planner is structurally kept out of the process/context that touches the worker's raw untrusted bytes.
**Depends on**: Phase 19
**Requirements**: PLANNER-01, PLANNER-02, PLANNER-04
**Success Criteria** (what must be TRUE):

  1. A `Planner` trait exists, and the existing deterministic intent→PlanNode logic (today a bare `plan_from_intent` fn) is refactored to implement it — the seam is a real abstraction, not a rename.
  2. A connection operating in the planner role is rejected by the broker if it sends `ProvideIntent`, `ReportClaims`, or `ReportDerivedClaim` — proven by a test that attempts each verb on that connection and observes rejection.
  3. The planner never receives the worker's raw-bytes fd or raw untrusted content — it is given only typed extracts and handle IDs, with no filesystem capability and no network reachability beyond its own inference endpoint.
  4. All pre-existing deterministic-planner-based tests/behavior from v1.0-v1.3 continue to pass unchanged through the new seam — no regression from the refactor.

**Plans**: TBD

### Phase 21: Adversarial LLM Planner

**Goal**: A minimal LLM-backed planner, running behind Phase 20's seam, drives a real intent end-to-end using only `PlanNode{sink, args}` — no literal field to carry.
**Depends on**: Phase 20
**Requirements**: PLANNER-03
**Success Criteria** (what must be TRUE):

  1. An LLM-backed implementation of the `Planner` trait exists and is selectable in place of the deterministic planner.
  2. Given a clean, trusted intent, the LLM planner emits a syntactically valid `PlanNode{sink, args}` referencing handle IDs only (never a literal value), and the executor Allows it, delivering a real send.
  3. The LLM planner's own prompt/tool-call construction is built only from typed extracts and handle IDs — never raw untrusted bytes — consistent with Phase 20's co-location boundary.

**Plans**: TBD

### Phase 22: Adversarial Gate Proof & Residual Disclosure

**Goal**: The trust boundary is proven indifferent to planner intelligence — a hostile-doc-primed LLM planner complies and tries to route a tainted value to `email.send`, and the executor Blocks it deterministically with genuine, live-verified taint propagation; the one remaining unenforced degree of freedom (T2) is honestly documented rather than silently left implicit.
**Depends on**: Phase 21
**Requirements**: GATE-01, GATE-02, GATE-03, GATE-04, T2-01
**Success Criteria** (what must be TRUE):

  1. A hostile document whose embedded injection instructs the LLM planner to email `attacker@evil.com` causes the planner to comply, emitting a syntactically valid `PlanNode` that routes the tainted handle to `to`.
  2. The executor Blocks that PlanNode deterministically; `verify_chain` is true; Mailpit's captured-message count is 0 — proven live on real Linux via `scripts/mailpit-verify.sh`, not asserted from code alone.
  3. In the SAME run, a trusted-intent control on the same sink Allows and delivers exactly once.
  4. A deterministic construction-site sentinel assertion (feed the prompt constructor a sentinel-tagged tainted record — sentinel each fragment — and assert the sentinel bytes never appear in the constructed prompt) replaces the old context-dump grep, and is unit-level/deterministic, not probabilistic.
  5. PROJECT.md (and/or the DESIGN doc) documents T2 (slot-type binding) as the accepted v1.4 residual risk — safe today only by incidental human-typing of every `UserTrusted` handle — with enforcement explicitly deferred to v1.5.

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
| 19. Cross-Connection Trust Coherence Fix | v1.4 | 2/2 | Complete   | 2026-07-11 |
| 20. Planner Seam & Capability Split | v1.4 | 0/TBD | Not started | - |
| 21. Adversarial LLM Planner | v1.4 | 0/TBD | Not started | - |
| 22. Adversarial Gate Proof & Residual Disclosure | v1.4 | 0/TBD | Not started | - |
