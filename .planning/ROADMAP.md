# Roadmap: AgentOS

## Overview

AgentOS v0 proves one thing end-to-end: a kernel-confined worker whose only
egress is broker-mediated plan nodes cannot push an attacker-tainted value into
a sensitive sink argument without deterministic blocking and literal-value human
confirmation. The journey runs two tracks that start in parallel — a
**substrate** track (workspace, domain core, kernel confinement, broker
reference monitor, fd-pass fs adapter) and a **design-gate** track (the
taint-model and plan-executor DESIGN docs). The substrate proves complete
mediation; the design gate must be reviewed before any executor code is written.
Both tracks converge in the final phase, the §9 value-injection security demo —
**which is the only thing that counts as v0 DONE. Substrate done ≠ v0 done.**

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

**Execution note:** Phase 1 (substrate foundation) and Phase 2 (design gate)
both depend on nothing — they start in parallel on day one (the M0 + M0-design
parallel start). Phase 3 continues the substrate from Phase 1. Phase 4 requires
**both** Phase 2 (the design gate) and Phase 3 (the substrate) complete.

- [x] **Phase 1: Substrate Foundation** - Cargo workspace, domain core, and the locked plan-node broker API (completed 2026-06-29)
- [ ] **Phase 2: Security Design Gate** - The taint-model and plan-executor DESIGN docs that hard-gate all executor code (runs parallel to substrate)
- [ ] **Phase 3: Confinement & Mediation Substrate** - Kernel confinement, broker reference monitor, fd-pass fs adapter, proven by the no-LLM substrate demo
- [ ] **Phase 4: Value-Injection Security Demo (v0 DONE)** - The §9 acceptance test passes with a genuine taint chain — the only gate for v0 done

## Phase Details

### Phase 1: Substrate Foundation

**Goal**: Stand up the single Cargo workspace, the core domain types with no
I/O, and the broker's plan-node effect API surface with its shape locked from
day one — so every later effect path is forced through PlanNode/ValueNode.
**Depends on**: Nothing (first phase; starts in parallel with Phase 2)
**Requirements**: REQ-runtime-core, REQ-api-stub-plan-node
**Success Criteria** (what must be TRUE):

  1. `cargo build` succeeds across the workspace, and Intent, Session, Event,
     Artifact, and the 3-class Effect enums compile in `runtime-core` with no
     I/O.

  2. `submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> })`
     exists and returns `NotImplemented`; there is no raw
     `EffectRequest`-to-sink path anywhere in the crate tree.

  3. `ValueNode` carries literal + provenance + taint fields in its type
     definition, so plan nodes can express genuine taint later.
**Plans**: 2/2 plans complete
**Wave 1**

- [x] 01-01-PLAN.md — Virtual Cargo workspace + runtime-core domain types (no I/O), incl. ValueNode literal+provenance+taint lock

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 01-02-PLAN.md — brokerd submit_plan_node stub (returns NotImplemented) + architectural no-bypass invariant gate

### Phase 2: Security Design Gate

**Goal**: Author and review the two DESIGN docs that gate all executor code:
`DESIGN-taint-model.md` and `DESIGN-plan-executor.md`. This phase runs in
**parallel** with the substrate track and is a **hard gate** — no code in
`crates/executor` may be written until both docs are reviewed.
**Depends on**: Nothing (runs in parallel with Phases 1 and 3; gates Phase 4)
**Requirements**: REQ-design-taint-model, REQ-design-plan-executor
**Success Criteria** (what must be TRUE):

  1. `DESIGN-taint-model.md` exists and explicitly states the dynamic-taint
     default, the hard planner/worker split for Tier 3+, and the I0 draft-only
     rule for Sessions seeded from untrusted content.

  2. `DESIGN-plan-executor.md` exists and specifies ValueNode, PlanNode, sink
     sensitivity, taint propagation, and the literal-value confirmation UX.

  3. Both docs are reviewed and approved — the recorded gate that unblocks
     `crates/executor` in Phase 4.
**Plans**: 3 plans
**Wave 1**

- [ ] 02-01-PLAN.md — Author DESIGN-taint-model.md (dynamic-taint default, hard Tier 3+ split, I0 draft-only, genuine-taint requirement)
- [ ] 02-02-PLAN.md — Author DESIGN-plan-executor.md (ValueNode/PlanNode, hardcoded sink sensitivity, monotonic propagation, literal-value confirmation UX)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 02-03-PLAN.md — DESIGN-GATE-RECORD.md: checklist + sha256 pin + blocking human review that unblocks crates/executor

### Phase 3: Confinement & Mediation Substrate

**Goal**: Deliver kernel-enforced confinement (sandbox), the broker reference
monitor with a SQLite audit DAG, and the fd-pass filesystem adapter — then prove
complete mediation with an end-to-end substrate demo that requires no LLM.
**Note: completing this phase is NOT v0 done** — it proves mediation, not
value-injection defense.
**Depends on**: Phase 1 (continues the substrate; parallel with Phase 2)
**Requirements**: REQ-sandbox, REQ-brokerd-core, REQ-adapters-fs, REQ-substrate-demo
**Success Criteria** (what must be TRUE):

  1. A confined worker starts with CPU, memory, and a broker UDS but zero ambient
     fs/net/shell; negative assertions hold — it cannot read `~/.ssh`, cannot
     reach the network, and cannot exec un-allowlisted binaries.

  2. The broker creates a Session, appends to the SQLite audit DAG, and serves
     UDS IPC.

  3. The broker opens a workspace file and passes its fd to the worker via
     SCM_RIGHTS.

  4. `caprun` runs a confined worker that reads a file via the passed fd, and the
     read Event appears in the audit DAG (complete mediation, no LLM).
**Plans**: TBD

### Phase 4: Value-Injection Security Demo (v0 DONE)

**Goal**: Prove the core value. A quarantined reader emits genuinely-tainted
typed extracts; a deterministic non-LLM executor walks the PlanNode DAG with I2
hardcoded; a scripted plan flows a tainted value into a mediated sink's sensitive
argument and is blocked with literal-value confirmation. The §9 integration test
passing — with a genuine, audited taint chain — **is v0 DONE.**
**Depends on**: Phase 2 (design gate, hard) AND Phase 3 (substrate)
**Requirements**: REQ-quarantined-reader, REQ-executor-stub, REQ-mediated-sink-stub, REQ-approval-hook, REQ-s9-acceptance-test
**Success Criteria** (what must be TRUE):

  1. A quarantined reader reads hostile input and emits a schema-valid typed
     ValueNode whose taint originates from the read Event (never hand-set); the
     planner never sees the raw sentence.

  2. The deterministic non-LLM executor stub walks the PlanNode DAG with I2
     hardcoded, propagating taint monotonically through edges.

  3. A scripted plan (no LLM) flows the tainted ValueNode into a mediated sink
     stub's sensitive `to` argument; the executor sees it tainted
     (`external.untrusted`) → blocks, and surfaces a literal-value confirmation
     prompt (via FAMP) for the exact address.

  4. The §9 integration test passes end-to-end, and the audit DAG shows the
     reader had no send cap, the sender never saw raw text, and an unbroken taint
     edge from the raw-read Event to the blocked sink argument. If taint is
     stapled at the sink instead of propagated, the test fails.
**Plans**: TBD

## Progress

**Execution Order:**
Phases 1 and 2 start in parallel (day one). Phase 3 follows Phase 1. Phase 4
requires both Phase 2 and Phase 3.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Substrate Foundation | 2/2 | Complete    | 2026-06-29 |
| 2. Security Design Gate | 0/3 | Not started | - |
| 3. Confinement & Mediation Substrate | 0/TBD | Not started | - |
| 4. Value-Injection Security Demo (v0 DONE) | 0/TBD | Not started | - |
