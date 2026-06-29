# Requirements

Build-order deliverables extracted from `planning-docs/PLAN.md` (§ Build Order +
§ v0 Acceptance Test). Each "Done when" clause is captured as acceptance
criteria. Milestone scope: M0 (Substrate, Week 1), M0-design (parallel, blocks
M1 executor code), M1 (Security Demo = v0 DONE, Week 2).

**Gate:** Substrate alone ≠ v0 done. Only §9 (REQ-s9-acceptance-test) passing =
v0 done.

---

## REQ-runtime-core

- **source:** planning-docs/PLAN.md (M0 #1)
- **scope:** M0 — Substrate
- **description:** `runtime-core` crate — core domain types with no I/O.
- **acceptance:** Intent, Session, Event, Artifact, and the 3-class Effect enums
  compile.

## REQ-sandbox

- **source:** planning-docs/PLAN.md (M0 #2)
- **scope:** M0 — Substrate
- **description:** `sandbox` crate — kernel-enforced confinement boundary.
- **acceptance:** Worker starts with CPU, memory, broker UDS, and zero ambient
  fs/net/shell. Negative assertions hold: agent cannot read `~/.ssh`, cannot
  reach network, cannot exec un-allowlisted binaries.

## REQ-brokerd-core

- **source:** planning-docs/PLAN.md (M0 #3)
- **scope:** M0 — Substrate
- **description:** `brokerd` core — control plane / reference monitor.
- **acceptance:** Session create, SQLite audit DAG append, UDS IPC all working.

## REQ-adapters-fs

- **source:** planning-docs/PLAN.md (M0 #4)
- **scope:** M0 — Substrate
- **description:** `adapters/fs` — filesystem adapter via fd-pass.
- **acceptance:** Broker opens a workspace file and passes the fd via SCM_RIGHTS.

## REQ-substrate-demo

- **source:** planning-docs/PLAN.md (M0 #5)
- **scope:** M0 — Substrate
- **description:** End-to-end substrate demonstration proving complete mediation
  (no LLM required).
- **acceptance:** `caprun` confined worker reads a file via passed fd; the event
  appears in the audit DAG.

## REQ-api-stub-plan-node

- **source:** planning-docs/PLAN.md (M0 #6)
- **scope:** M0 — Substrate
- **description:** Broker `submit_plan_node()` API surface — shape locked from
  day one.
- **acceptance:** `submit_plan_node()` exists and returns `NotImplemented`; the
  PlanNode/ValueNode shape is locked.

## REQ-design-taint-model

- **source:** planning-docs/PLAN.md (M0-design #7)
- **scope:** M0-design (blocks M1 executor code)
- **description:** `DESIGN-taint-model.md` design doc.
- **acceptance:** States dynamic taint default, hard split for Tier 3+, and the
  I0 intent/session-creation injection rule (draft-only when seeded from
  untrusted content) explicitly.

## REQ-design-plan-executor

- **source:** planning-docs/PLAN.md (M0-design #8)
- **scope:** M0-design (blocks M1 executor code)
- **description:** `DESIGN-plan-executor.md` design doc. `crates/executor` must
  NOT be written until this is reviewed.
- **acceptance:** Specifies ValueNode, PlanNode, sink sensitivity, propagation,
  and confirmation UX.

## REQ-quarantined-reader

- **source:** planning-docs/PLAN.md (M1 #9)
- **scope:** M1 — Security Demo
- **description:** Quarantined reader worker producing genuinely-tainted typed
  extracts.
- **acceptance:** Worker reads hostile input → emits a ValueNode; taint
  originates from the read Event (never hand-set).

## REQ-executor-stub

- **source:** planning-docs/PLAN.md (M1 #10)
- **scope:** M1 — Security Demo
- **description:** `executor` stub — deterministic I2 interpreter.
- **acceptance:** Walks the PlanNode DAG; I2 hardcoded; monotonic taint
  propagation through edges.

## REQ-mediated-sink-stub

- **source:** planning-docs/PLAN.md (M1 #11)
- **scope:** M1 — Security Demo
- **description:** Mediated sink stub (e.g. `email.send`) with a sensitive `to`
  argument.
- **acceptance:** Sink sensitivity map is hardcoded in v0 — no policy/schema
  system yet.

## REQ-approval-hook

- **source:** planning-docs/PLAN.md (M1 #12)
- **scope:** M1 — Security Demo
- **description:** Approval hook for human confirmation of sensitive sink args.
- **acceptance:** FAMP delivery; prompt shows the literal value to confirm.

## REQ-s9-acceptance-test

- **source:** planning-docs/PLAN.md (M1 #13 + § v0 Acceptance Test §9)
- **scope:** M1 — Security Demo = v0 DONE
- **description:** Automated integration test running the §9 scenario end-to-end
  with a genuine taint chain. This is the single gate for v0 DONE.
- **acceptance:**
  1. Reader emits a schema-valid typed extract as a ValueNode; planner never
     sees the raw sentence.
  2. Taint is genuine — originates from the reader's read Event, never hand-set
     at the sink.
  3. A scripted plan (no LLM) flows that ValueNode into the sink's sensitive
     `to` argument.
  4. Executor sees recipient is tainted (`external.untrusted`) in a sensitive
     sink arg → blocks.
  5. Surfaces a literal-value confirmation prompt for the exact address.
  6. Audit DAG shows: reader had no send cap; sender never saw raw text; an
     unbroken taint edge from the raw-read Event to the blocked sink argument.
  - **Non-negotiable sub-criterion:** If taint is stapled on at the sink instead
    of propagated through the DAG, the demo fails — it proves nothing.
