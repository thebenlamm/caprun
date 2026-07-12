# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- ✅ **v1.3 — Doc → Action Assistant** — Phases 12-17 (shipped 2026-07-09)
- ✅ **v1.4 — Trust-Boundary Integrity & the Adversarial Planner** — Phases 18-22 (shipped 2026-07-11)
- ✅ **v1.5 — Slot-Type Binding Enforcement (T2)** — Phases 23-25 (shipped 2026-07-12)
- 🚧 **v1.6 — Security Hardening (close the residuals)** — Phases 26-30 (in progress)

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

<details>
<summary>✅ v1.5 — Slot-Type Binding Enforcement (T2) (Phases 23-25) — SHIPPED 2026-07-12</summary>

Full detail archived in [`milestones/v1.5-ROADMAP.md`](milestones/v1.5-ROADMAP.md).

**Milestone goal:** Close v1.4's accepted residual #5 (T2) — the executor gains a structural check that a resolved value's semantic origin matches the semantic role of the plan-node slot it's routed into, so a misrouted `UserTrusted` handle (e.g. a subject-typed string landed in `to`) is caught even though it is neither untrusted (I2 doesn't fire) nor a class-level deny (I0/I1 don't apply).

- [x] **Phase 23: Slot-Type Binding Design Gate** (2/2 plans) — `DESIGN-slot-type-binding.md` cleared a fresh non-self adversarial review before any TCB code — completed 2026-07-12
- [x] **Phase 24: Slot-Type Binding Enforcement** (3/3 plans) — origin_role mint-time tag threaded through all mint sites, hardcoded `expected_role()` table, exhaustive `DenyReason::SlotTypeMismatch`, fail-closed Step 1c in `submit_plan_node` — completed 2026-07-12
- [x] **Phase 25: Regression & Live Proof** (3/3 plans) — held-out swapped subject↔recipient deny test (genuine audit chain), 0-NEEDS-FIX regression audit, independent bare `mailpit-verify.sh` green on real Linux (309 passed/0 failed) + human DONE sign-off — completed 2026-07-12

**v1.5 DONE gate cleared:** a deliberately swapped subject↔recipient handle pair (both `UserTrusted`) hard-Denies with `SlotTypeMismatch` via Step 1c through the real broker path, with a durable `plan_node_evaluated` audit event and `verify_chain` true — proven live on real Linux. Regression audit found 0 fixture bypasses; full-workspace regression independently re-run green. All 11 requirements (DESIGN-07..10, T2-02..08) Complete; milestone audit PASSED (11/11 reqs, 5/5 integration hops wired). No git push yet (Ben's call).

</details>

### 🚧 v1.6 — Security Hardening (close the residuals) (In Progress)

**Milestone goal:** Close the standing TCB-local security residuals that v1.1–v1.5 accumulated and documented as accepted caveats — turning each DOC-01 honesty qualifier into an enforced guarantee, without adding any new external-effect surface. Per this project's standing design-gate precedent (v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23), v1.6 opens with a design-gate phase that hard-blocks all five hardening items until it clears a fresh (non-self) adversarial review, followed by implementation grouped by blast radius, closing with a dedicated regression & live-proof phase.

- [ ] **Phase 26: Security Hardening Design Gate** - A DESIGN doc specifies the mechanism + fail-closed default for all five residuals and clears a fresh adversarial review before any hardening code is written
- [ ] **Phase 27: Session & Connection Integrity Hardening** - fd release itself carries the I1 draft-only consequence, and the CreateSession forced-Active mint arm is compiled out of the production binary
- [ ] **Phase 28: Authenticated Audit Chain** - `verify_chain` becomes forge-resistant (keyed MAC and/or externally-anchored head), not just a corruption detector
- [ ] **Phase 29: Sink-Path Hardening — Replay CAS & contents Slot** - the Allowed email.send path is replay-safe (at-most-once), and `file.create`'s `contents` arg is no longer an unconstrained slot
- [ ] **Phase 30: Regression & Live Proof** - the full workspace regression is independently re-run green on real Linux with a dedicated negative test proving each closed residual, no regression to v1.1–v1.5 behavior

## Phase Details

_All shipped milestone phases (1-25) are archived in `milestones/`. v1.6 phases (26-30) below are in progress._

### Phase 26: Security Hardening Design Gate

**Goal**: A DESIGN doc (`planning-docs/DESIGN-security-hardening.md`) specifies the approach and fail-closed default for all five hardening residuals, and clears a fresh (non-self) adversarial review before any `crates/executor`, `crates/brokerd`, or `crates/runtime-core` hardening code is written.
**Depends on**: Nothing (first phase of v1.6; builds on v1.5's shipped Phase 25 baseline)
**Requirements**: DESIGN-11, DESIGN-12
**Success Criteria** (what must be TRUE):

  1. `planning-docs/DESIGN-security-hardening.md` exists and specifies, for each of the five residuals, the mechanism and fail-closed default: (a) demote-at-RequestFd reconciled with the CONTROL-01 clean path; (b) `verify_chain` keyed-MAC/externally-anchored-head mechanism including key/anchor custody and threat model; (c) the Allowed-path idempotency/CAS shape; (d) the `CreateSession` forced-Active compile-exclusion mechanism; (e) the `file.create` `contents` expected-role/sensitivity treatment.
  2. The DESIGN doc clears a fresh (non-self) adversarial review with every finding resolved, recorded in a GATE-RECORD — mirroring v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23.
  3. No hardening code exists yet in `crates/executor`, `crates/brokerd`, or `crates/runtime-core` — the gate hard-blocks Phases 27-29 until it clears.

**Plans**: 2 plans
**Wave 1**

- [ ] 26-01-PLAN.md — Author `DESIGN-security-hardening.md`: §a-§e (five residual mechanisms + fail-closed defaults), §f cross-cutting (X-01/X-02/X-03 + X-04 ruling), Adversarial-Review-Preemption, Accepted Residuals, Phase 27-30 impl map (DESIGN-11)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 26-02-PLAN.md — Fresh non-self Fable code-tracing review, fold amendments, amend DESIGN-session-trust-state.md (D-02), write DESIGN-GATE-RECORD-v1.6.md, hard-gate re-confirmation (DESIGN-12)

### Phase 27: Session & Connection Integrity Hardening

**Goal**: fd release to the confined worker (`RequestFd`) itself demotes the session to draft-only for the I1 reason, and the `CreateSession`-IPC forced-`Active` mint arm is physically excluded from the production binary at compile time — both changes land in the same session/connection-lifecycle surface of `server.rs`.
**Depends on**: Phase 26
**Requirements**: HARDEN-01, HARDEN-04
**Success Criteria** (what must be TRUE):

  1. Requesting an fd (`RequestFd`) for a workspace file demotes the session to `Draft` for the I1 reason, even if the worker never reports reading it back.
  2. A benign, fragment-free document read still leaves the session `Active`, and the CONTROL-01 clean-send path still completes ungated — no regression to the existing clean path.
  3. The `CreateSession`-IPC forced-`Active` mint arm is excluded from a default production build via a compile-time feature/cfg — grep/build evidence shows it absent from a default release build, not merely gated behind `CAPRUN_ENABLE_IPC_CREATE_SESSION` at runtime.
  4. Existing test fixtures that previously relied on the runtime env-flag opt-in still exercise the forced-Active behavior, now under an explicit test-only compile feature, so coverage isn't silently lost.

**Plans**: TBD

### Phase 28: Authenticated Audit Chain

**Goal**: `verify_chain` becomes an authenticated-integrity check rather than a corruption detector — an actor with `events`-table write access can no longer produce a chain that `verify_chain` accepts.
**Depends on**: Phase 26
**Requirements**: HARDEN-02
**Success Criteria** (what must be TRUE):

  1. `verify_chain` rejects a chain where an event row has been rewritten and every descendant hash/parent_hash recomputed to be internally self-consistent — the exact forgery that previously passed.
  2. The chain's authenticity depends on a secret key or an out-of-store anchor that a bare `events`-table writer cannot derive or reproduce.
  3. An untampered chain continues to verify true — no false positives; existing confirm-path and live-acceptance callers of `verify_chain` are unaffected.

**Plans**: TBD

### Phase 29: Sink-Path Hardening — Replay CAS & contents Slot

**Goal**: the trusted `email.send` path is replay-safe (at-most-once, matching the confirm path's transaction discipline), and `file.create`'s `contents` arg is no longer an unconstrained slot.
**Depends on**: Phase 26
**Requirements**: HARDEN-03, HARDEN-05
**Success Criteria** (what must be TRUE):

  1. A replayed `SubmitPlanNode` on the Allowed (trusted) `email.send` path sends at most once, enforced via an idempotency key/CAS in the same atomic-transaction discipline as the existing confirm path.
  2. A tainted value routed into `file.create`'s `contents` arg is now handled under the same I2/slot-type discipline as other sensitive args (blocked or slot-type-mismatched as appropriate), closing the previously-unconstrained gap.
  3. Existing trusted-content `file.create` flows continue to succeed unchanged — no false-positive block on legitimate `contents` values.

**Plans**: TBD

### Phase 30: Regression & Live Proof

**Goal**: All v1.6 hardening is proven live on real Linux with no regression, and each closed residual has a dedicated negative test.
**Depends on**: Phase 27, Phase 28, Phase 29
**Requirements**: HARDEN-06
**Success Criteria** (what must be TRUE):

  1. The full workspace regression is independently re-run green on real Linux via the bare `scripts/mailpit-verify.sh` recipe, with no regression to v1.1–v1.5 behavior.
  2. A negative test proves a forged/tampered audit chain is rejected by `verify_chain`.
  3. A negative test proves a replayed Allowed `email.send` delivers exactly once (not N times).
  4. A test/build check proves the forced-Active `CreateSession` path is absent from the built production binary.
  5. A test proves fd release (`RequestFd`) demotes the session, while the CONTROL-01 clean path still succeeds.

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
| 24. Slot-Type Binding Enforcement | v1.5 | 3/3 | Complete    | 2026-07-12 |
| 25. Regression & Live Proof | v1.5 | 3/3 | Complete    | 2026-07-12 |
| 26. Security Hardening Design Gate | v1.6 | 0/2 | Planned | - |
| 27. Session & Connection Integrity Hardening | v1.6 | 0/? | Not started | - |
| 28. Authenticated Audit Chain | v1.6 | 0/? | Not started | - |
| 29. Sink-Path Hardening — Replay CAS & contents Slot | v1.6 | 0/? | Not started | - |
| 30. Regression & Live Proof | v1.6 | 0/? | Not started | - |
