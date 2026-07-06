# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- 🚧 **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (in progress)

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

### 🚧 v1.2 — Tainted Session, Human Gate (Phases 8-11) — IN PROGRESS

**Milestone goal:** A session that touches untrusted content is mechanically demoted to draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked sink arg can be released only by literal-value human confirmation — all deterministic, all in the audit DAG.

- [ ] **Phase 8: Session-Trust & Confirmation Design Gate** - DESIGN doc for session-trust-state (I1 demotion + I0 creation rule) and confirmation-release semantics exists and is reviewed before any executor code for this milestone is written — docs drafted and round-1-revised, but genuine human re-review of round 2 is still pending (see `planning-docs/DESIGN-GATE-RECORD-v1.2.md`)
- [ ] **Phase 9: Session Trust State (I1 + I0)** - reading untrusted content or being seeded from external content demotes/starts a session as draft-only; draft-only sessions deny CommitIrreversible plan nodes via one executor TCB function
- [ ] **Phase 10: Single-Shot Confirmation Loop** - `caprun confirm <effect_id>` shows the human the blocked literal + provenance and releases exactly one (sink, arg, literal-digest) triple; deny is durable
- [ ] **Phase 11: Live Acceptance — Tainted Session, Human Gate** - live §9-style run on real Linux: hostile read → session demotion → sink block → human deny (nothing sent) / human confirm (exactly once), one unbroken audit chain

## Phase Details

### Phase 8: Session-Trust & Confirmation Design Gate

**Goal**: A reviewed DESIGN doc for session-trust-state (I1 dynamic demotion + I0 creation rule) and confirmation-release semantics exists, gating all executor code written for this milestone — mirroring the v1.0 Phase 2 design-gate discipline.
**Depends on**: Phase 7 (v1.1 shipped; this is the first v1.2 phase)
**Requirements**: PROC-01
**Success Criteria** (what must be TRUE):

  1. A DESIGN doc exists under `planning-docs/` defining the draft-only demotion rule (I1 trigger = `mint_from_read`), the I0 session-creation rule, and the new `DenyReason` variant/taxonomy for draft-only denial.
  2. The same (or a paired) DESIGN doc defines confirmation-release semantics: single-shot `(sink, arg, literal-digest)` triple release, durable deny, and TCB-resident (not policy-file) release path.
  3. The doc explicitly assigns the draft-only deny decision to one executor TCB function — not a broker pre-check — before Phase 9 or Phase 10 executor code is written.

**Plans**: 3/3 plans complete
Plans:
**Wave 1**

- [x] 08-01-PLAN.md — Author DESIGN-session-trust-state.md (I1 demotion + I0 creation rule + SessionStatus::Draft + executor deny mechanism)
- [x] 08-02-PLAN.md — Author DESIGN-confirmation-release.md (PendingConfirmation checkpoint + confirm/deny semantics + CLI contract)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 08-03-PLAN.md — Author DESIGN-GATE-RECORD-v1.2.md + blocking human-review checkpoint (depends on 08-01, 08-02)

### Phase 9: Session Trust State (I1 + I0)

**Goal**: A session's trust state is mechanically tracked: reading untrusted content or being seeded from externally-derived content demotes/starts a session as draft-only, and draft-only sessions deterministically deny irreversible effects while still permitting reversible ones.
**Depends on**: Phase 8
**Requirements**: TAINT-01, TAINT-02, TAINT-03, TAINT-04, ORIGIN-01, ORIGIN-02
**Success Criteria** (what must be TRUE):

  1. A session that triggers `mint_from_read` (a raw untrusted read) is demoted to draft-only, with the demotion recorded as an audit event carrying a causal edge to the triggering read event.
  2. A session created with an externally-derived seed starts draft-only from creation, with the seed-provenance (trusted-arg vs file-derived) recorded and decided by the `caprun` CLI.
  3. Submitting a `CommitIrreversible`-class plan node against a draft-only session is Denied with a new `DenyReason` variant, decided in the executor (one TCB function, one taxonomy).
  4. Submitting a `MutateReversible` or `Observe`-class plan node against a draft-only session still succeeds.

**Plans**: TBD

### Phase 10: Single-Shot Confirmation Loop

**Goal**: A human can inspect a blocked effect's verbatim literal and provenance, then release it exactly once or durably deny it, via a second CLI command — never a session-wide waiver or standing policy.
**Depends on**: Phase 8
**Requirements**: CONFIRM-01, CONFIRM-02, CONFIRM-03, CONFIRM-04
**Success Criteria** (what must be TRUE):

  1. Running `caprun confirm <effect_id>` against a `BlockedPendingConfirmation` effect displays the verbatim literal value and its provenance to the human.
  2. Confirming releases exactly that one `(sink, arg, literal-digest)` triple — the effect proceeds once, and no standing policy or session-wide waiver is created.
  3. Denying is durable: the effect never proceeds, and the same `effect_id` cannot later be confirmed.
  4. Every confirm/deny decision is recorded as an audit event anchored to `SinkBlockedAnchor.effect_id`, and the release path executes in the TCB (not a policy file).

**Plans**: TBD

### Phase 11: Live Acceptance — Tainted Session, Human Gate

**Goal**: The full chain — hostile read, session demotion, sink block, and human decision — runs live on real Linux `caprun` with one unbroken, auditable causal chain, for both the deny and confirm outcomes.
**Depends on**: Phase 9, Phase 10
**Requirements**: ACC-01, ACC-02, ACC-03
**Success Criteria** (what must be TRUE):

  1. Deny path: a hostile workspace file is read by the worker, the session is demoted (I1), a tainted routing arg is Blocked (I2), and a human deny via `caprun confirm` results in no effect ever proceeding.
  2. Confirm path: the same scenario, but a human confirm via `caprun confirm` results in the effect proceeding exactly once.
  3. For both runs, the audit DAG shows one unbroken causal chain: read → demotion → block → human decision.

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
| 8. Session-Trust & Confirmation Design Gate | v1.2 | 3/3 | Complete   | 2026-07-06 |
| 9. Session Trust State (I1 + I0) | v1.2 | 0/? | Not started | - |
| 10. Single-Shot Confirmation Loop | v1.2 | 0/? | Not started | - |
| 11. Live Acceptance — Tainted Session, Human Gate | v1.2 | 0/? | Not started | - |
