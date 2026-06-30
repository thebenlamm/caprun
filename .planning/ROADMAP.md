# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- 🚧 **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (in progress)

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

### 🚧 v1.1 — Usable Runtime (Live §9 from the CLI)

**Milestone goal:** Turn the proven-in-tests value-injection defense into a real `caprun` run — a deterministic scripted planner turns an intent into PlanNodes, a confined worker drives toward a real `file.create` sink, and the deterministic I2 block fires on a genuine taint chain (with a clean, broker-minted allow-path too). Runtime assembly only — no new capability surface.

- [ ] **Phase 5: Runtime Spine & Live §9 Email Block** - Collapse dual dispatch, land session-scoped handle model, and prove live §9 block with durable blocked-path audit through the email.send stub
- [ ] **Phase 6: Deterministic Planner & Intent Input** - Typed intent → PlanNode planner with broker-minted trusted values for the clean allow-path
- [ ] **Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance** - Real hardened sink, closed enforcement edge cases, full live §9 acceptance contract green

## Phase Details

### Phase 5: Runtime Spine & Live §9 Email Block

**Goal**: `caprun` operates through a single unified `brokerd::server` dispatch path — no second executor loop — with a session-scoped broker state model, and a real `caprun` invocation on hostile input fires a live §9 block with a durable causal `sink_blocked` audit event through the existing `email.send` stub
**Depends on**: Phase 4
**Requirements**: ASM-01, ASM-02, ASM-03, ASM-04, HARD-03, ACC-02
**Success Criteria** (what must be TRUE):

  1. `caprun` has no second executor-dispatch loop; RequestFd, read reporting, mint, evaluate, audit, and sink invocation all route exclusively through `brokerd::server` dispatch
  2. `executor::submit_plan_node` runs through the live broker path; the `"SubmitPlanNode not wired"` placeholder is gone from the codebase
  3. A confined worker emits typed `ReportClaims` IPC with the `EmailAddress` claim variant; raw source bytes never cross the planner boundary; unknown claim kinds fail closed
  4. `mint_from_read` produces authoritative `ValueId`s anchored to the real `file_read` event in the SQLite audit DAG
  5. `ValueRecord`s are session-scoped: the broker connection is bound to its session, a handle minted in one session is denied in another, and a request-supplied `session_id` is never trusted for resolution
  6. A real `caprun` invocation on hostile input produces a durable causal `sink_blocked` event — causal parent preserved, append-failure fails closed — and the CLI exits non-success before any effect executes; the block is durable before the CLI returns

**Plans**: 1/4 plans executed
**Wave 1**

- [x] 05-01-PLAN.md — Additive IPC protocol types (WorkerClaim/ReportClaims/ClaimsReceived) + fail-closed serde tests [wave 1]

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 05-02-PLAN.md — Unified session-scoped broker dispatch: mint_from_read wiring, durable fail-closed sink_blocked, session_id removal [wave 2]

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 05-03-PLAN.md — caprun binary: delete second dispatch loop, worker emits typed claims + exits on block, e2e update [wave 3]

**Wave 4** *(blocked on Wave 3 completion)*

- [ ] 05-04-PLAN.md — Live §9 block e2e (hostile → durable causal sink_blocked, non-success exit) + phase gate [wave 4]

### Phase 6: Deterministic Planner & Intent Input

**Goal**: `caprun` accepts typed intents and a deterministic non-LLM planner translates them into plan nodes, with `mint_from_intent` enabling a clean allow-path that does not block at the executor
**Depends on**: Phase 5
**Requirements**: PLAN-01, PLAN-02, PLAN-03, PLAN-04, HARD-02
**Success Criteria** (what must be TRUE):

  1. `caprun` CLI accepts an intent input alongside a workspace path (not just a bare file path)
  2. A typed intent enum maps deterministically to `PlanNode{sink, args}` — the planner emits only `SinkId` + existing `ValueId` handles and never receives raw bytes or taint labels
  3. `mint_from_intent` mints a `UserTrusted` `ValueId` anchored to an `intent_received` audit event, distinct from `mint_from_read`
  4. A plan node carrying `UserTrusted`/`LocalWorkspace`-only provenance passes the executor without blocking — the clean allow-path is reachable end to end

**Plans**: TBD

### Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance

**Goal**: `file.create` is a real, hardened sink; all enforcement edge cases from channel review are closed; the `RelativePath` claim variant completes the `ReportClaims` enum; and the full live §9 acceptance contract — hostile block with genuine-taint proof, clean allow, and a causal audit chain durable across process exit — is green on a real Linux `caprun` run
**Depends on**: Phase 6
**Requirements**: SINK-01, SINK-02, SINK-03, SINK-04, HARD-01, HARD-04, HARD-05, HARD-06, ACC-01, ACC-03, ACC-04, ACC-05, ACC-06, ACC-07
**Success Criteria** (what must be TRUE):

  1. `file.create` validates its arg schema (`path`, `contents`) — missing, duplicate, or unknown args are rejected before any sensitivity or executor step; unknown sinks also fail closed
  2. `RequestFd` reads and `file.create` path resolution share ONE workspace-root capability model: `HARD-04` (capability-restricted reads) is the read-side prerequisite for `SINK-04` (dirfd + `openat2`); absolute paths, traversal, and symlink escapes are rejected at both sites; no validate-then-write race (TOCTOU-safe)
  3. `file.create` uses `O_EXCL` exclusive creation — it never overwrites an existing file
  4. The `ReportClaims` enum adds the `RelativePath` variant; the broker validates the claim, resolves the path under the workspace-root capability, and assigns taint/provenance; unknown variants continue to fail closed
  5. A `caprun` run with hostile input → typed path claim → `mint_from_read` → `file.create` is blocked: no file is written, the CLI exits non-success, and a `sink_blocked` event is in the audit DB
  6. A `caprun` run with a broker-minted trusted intent path (`mint_from_intent`) creates exactly the expected file under the workspace root
  7. The audit DB shows one unbroken causal chain per run — `fd_granted → file_read → plan_node_evaluated → sink_blocked` (hostile) or `sink_executed` (clean); the blocked `PlanArg`'s `ValueId` resolves to a `ValueRecord` whose `provenance_chain[0]` equals the actual `file_read` event id; the durable audit evidence links `effect_id + sink + arg + ValueId + provenance anchor` so the proof survives process exit (anti-stapling sentinel — an event-order-only assertion is insufficient)
  8. Forged `ValueId` handles and unknown sink/arg combinations are denied; an effect-path crash leaves an explicit indeterminate audit record with no automatic retry; cross-session handle access is adversarially denied

**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Substrate Foundation | v1.0 | 2/2 | Complete | 2026-06-29 |
| 2. Security Design Gate | v1.0 | 3/3 | Complete | 2026-06-29 |
| 3. Confinement & Mediation Substrate | v1.0 | 5/5 | Complete | 2026-06-29 |
| 4. Value-Injection Security Demo (v0 DONE) | v1.0 | 5/5 | Complete | 2026-06-30 |
| 5. Runtime Spine & Live §9 Email Block | v1.1 | 1/4 | In Progress|  |
| 6. Deterministic Planner & Intent Input | v1.1 | 0/TBD | Not started | - |
| 7. file.create Sink, Enforcement Hardening & Full Acceptance | v1.1 | 0/TBD | Not started | - |
