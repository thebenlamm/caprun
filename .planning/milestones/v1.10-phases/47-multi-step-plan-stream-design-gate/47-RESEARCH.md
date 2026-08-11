# Phase 47: Multi-step Plan Stream Design Gate - Research

**Researched:** 2026-07-23
**Domain:** Design-gate process + multi-step plan-stream security architecture on shipped caprun Intent Runtime (Rust TCB; no new crates)
**Confidence:** HIGH

## Summary

Phase 47 is a **doc-only design gate**. It hard-blocks all multi-step TCB / worker submit / confirm-hold code for v1.10 (Phases 48–52). Deliverables are (1) `planning-docs/DESIGN-multi-step-plan-stream.md` pinning stream shape, handle bag, mid-loop Block-and-Hold, I1×coding-loop bounds, instruction vs value channel disjointness, mid-stream deny/abort, and HYG-02 hygiene; and (2) a **fresh, non-self, orchestrator-owned** adversarial code-trace that records **APPROVE / CLEARED** in `planning-docs/DESIGN-GATE-RECORD-v1.10.md` (or equivalent) before any change lands under `crates/{executor,brokerd,sandbox,runtime-core}` or the worker submit/confirm-hold path in `cli/caprun`.

This is **not** an implementation phase. The multi-step product gap is already well-characterized: through v1.9 the broker already accepts N× `SubmitPlanNode` on one worker connection, `PlanNodeDecision.output_value_id` is already wired (and discarded by the worker), and confirm is already a durable single-shot snapshot path — but the **worker is one-shot** (`plan()` → one `SubmitPlanNode` → exit on Block), `caprun run` only drives single-node email/file intents, and LIVE-05 was honest hybrid in-crate composition. v1.10 closes that gap with a sequential plan stream on the existing `Planner` seam; this phase only **pins** the mechanisms so Phases 48–52 cannot invent laundering valves mid-code.

**Primary recommendation:** Author one §-numbered DESIGN doc modeled on `DESIGN-v1.9-egress-policy.md` / `DESIGN-git-github-http-sinks.md` (decisions not options; every load-bearing claim cited to live `file:line`; §-per-pitfall threat model; invariant-preservation checklist; orchestrator-owned adversarial-trace gate that re-runs on stream-shape / confirm-hold / trusted-arg-mint pivots). Split the phase into **Plan 01 = author doc** (gsd-executor, docs-only) + **Plan 02 / orchestrator gate = fresh Fable-5 (or project-standard) code-trace + fold + gate record** — never let a gsd-executor self-review. Pin sequential multi-node on the existing seam (**not** batch DAG authorize, **not** `EffectRequest`, **not** a new crate). Success path = trusted-intent-only args; mid-loop I2 proof uses deliberate tainted-handle routing; Block-and-Hold same Session.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DESIGN-19 | Single DESIGN doc pins plan-stream shape (additive multi-node on existing Planner seam — not batch DAG authorize, not EffectRequest); worker sequential submit + handle bag (opaque ValueIds only; planner never mints); mid-loop Block-and-Hold (same Session, same policy bind, same audit chain; no reconnect-remint; no session-wide confirm waiver); I1×coding-loop bounds (trusted-intent success path; no weakening CommitIrreversible Draft denies); instruction vs value channels disjoint under multi-node (PLAN-03); mid-stream deny/abort semantics; carry-forward ProvideIntent-once, Gate 3 mint-site discipline, P33/P34 precheck-before-burn, POLICY-02 non-bypass of I2 | §§ Architecture Patterns, Standard Stack (reuse), Don't Hand-Roll, Common Pitfalls, Code Examples, Recommended DESIGN outline |
| DESIGN-20 | DESIGN clears a fresh, non-self, orchestrator-owned adversarial code-trace (NOT a gsd-executor) before any multi-step TCB change in `crates/{executor,brokerd,sandbox,runtime-core}` or worker submit/confirm-hold in `cli/caprun`; trace re-runs if stream shape, confirm-hold, or trusted-arg mint path changes mid-implementation | §§ Prior Design-Gate Pattern, Validation Architecture, Process checks |
| HYG-02 | Zero new crates unless design-gate-justified (default **zero**); no `EffectRequest` under `crates/`; Gate 3 mint-site list unchanged or explicitly amended; `check-invariants.sh` green; compose-verify remains authoritative Linux gate | §§ Project Constraints, Standard Stack, Package Legitimacy (N/A), Validation Architecture |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| DESIGN doc authoring (pin decisions) | Docs / planning-docs | — | Phase deliverable is markdown under `planning-docs/`; no runtime tier owns product behavior yet |
| Adversarial code-trace spawn + fold + gate record | Orchestrator process (GSD) | Fresh non-self reviewer agent | Unbroken precedent: gsd-executors have no Agent tool; self-read fails fresh-context discipline |
| Plan-stream shape (future Phase 48) | Worker (confined CLI process) | Planner seam (in-process trait) | Multi-submit must stay inside confined worker + handle-only planner — not unconfined orchestrator ambient path |
| Per-node I2 / policy evaluation (unchanged) | Executor (Rust TCB) | Broker reference monitor | Each `SubmitPlanNode` already runs policy pre-I2 then I2; multi-step must not add batch authorize |
| Handle bag / `output_value_id` threading | Worker | Broker ValueStore (mint sites) | Opaque ValueIds only; planner never mints; taint lives in broker-owned records |
| Mid-loop confirm continuity | CLI main (orchestrator process) + broker confirmation substrate | Worker hold/wait | Confirm is durable snapshot + single-shot; worker must not re-submit blocked node; ValueStore dies with process → Block-and-Hold preferred |
| Policy bind (POLICY-03) | Broker at session create | CLI trusted path outside workspace | Immutable for Session; multi-step must not rebind mid-stream |
| Kernel confinement | Sandbox | Worker self-confine after connect | Multi-step must not re-open ambient net/exec |
| Architectural invariant gate | `scripts/check-invariants.sh` | compose-verify (Linux) | Gate 1 EffectRequest absence; Gate 3 mint sites; compose-verify remains LIVE authority |

## Project Constraints (from CLAUDE.md)

Treat as locked (same authority as discuss-phase decisions for this repo):

1. **Source of truth:** `planning-docs/PLAN.md` wins on conflicts between docs/code comments/this file.
2. **v0 DONE / product bar lineage:** genuine taint chain through plan nodes + audit DAG; substrate working ≠ done. v1.10 DONE = non-hybrid CLI multi-node LIVE (LIVE-07/08), not hybrid rebrand.
3. **Effect path locked:** `submit_plan_node(session_id, PlanNode { sink, args: ValueIds })` — **never** introduce raw `EffectRequest { effect, args: Map }`. `check-invariants.sh` Gate 1 fails the build if `EffectRequest` appears under `crates/` (annotate intentional mentions with `planner-discipline-allow`).
4. **Design-gate docs block TCB code:** historically `DESIGN-taint-model.md` + `DESIGN-plan-executor.md` blocked executor; same discipline for multi-step — **no** multi-step TCB code until DESIGN + adversarial APPROVE.
5. **I0 / I1 / I2:**
   - I1: no LLM context holds untrusted content *and* authority for irreversible effects; default dynamic taint; reading raw untrusted → draft-only.
   - I2: no attacker-tainted value in sensitive sink arg without literal-value human confirmation; **hardcoded in Rust TCB executor — never swappable policy**.
   - I0: Session seeded from external/untrusted starts draft-only; cannot auto-authorize Tier 3+.
6. **Terminology locked:** Intent, Session, Planner, Worker, Broker, Adapter, Effect, Artifact, Event. `ExecutionContext` internal only. Project/binary = `caprun`.
7. **TCB is Rust.** Python non-TCB experiments only.
8. **Linux-only security claims.** macOS stubs expected; compose-verify / mailpit-verify are authoritative Linux gates. From Phase 16+, SMTP-touching verification uses `scripts/mailpit-verify.sh` (not bare `docker run rust:1` alone when SMTP may fire).
9. **Out of scope until relevant gates hold:** agent frameworks, memory, marketplace, Cedar, web UI, cross-host/Biscuit, gVisor/Firecracker, LLM multi-step (explicitly deferred past v1.10).
10. **Never bind named Docker volumes for `CARGO_TARGET_DIR` as a manual speed hack** (repo policy).

## Prior Design-Gate Pattern (repo precedent)

**Standing chain (unbroken):** v1.0 P2 → v1.2 P8 → v1.3 P12 → v1.4 P18 → v1.5 P23 → v1.6 P26 → v1.7 P31 → v1.8 P35 → v1.9 P41 → **v1.10 P47**.

### Two-plan structure (recommended)

| Plan | Owner | Produces | Forbidden |
|------|-------|----------|-----------|
| **47-01** Author DESIGN | gsd-executor | `planning-docs/DESIGN-multi-step-plan-stream.md` only | Any `crates/` / `cli/` edit; self-running the adversarial trace |
| **47-02** (or orchestrator-owned post-01 gate) Adversarial clear | **Orchestrator** spawns fresh non-self reviewer; fold may be executor under orchestrator control | Amendments to DESIGN + `planning-docs/DESIGN-GATE-RECORD-v1.10.md` with CLEARED | Reviewer = doc author; gsd-executor performing the review itself |

**v1.9 Phase 41 model** (closest recent twin): Plan 01 authors DESIGN; DESIGN-18 adversarial clear is explicitly **orchestrator-owned** because gsd-executors lack Agent tool. Gate record: `planning-docs/DESIGN-GATE-RECORD-v1.9.md`.

**v1.8 Phase 35 model** (two formal plans): 35-01 author DESIGN; 35-02 spawn Fable-5 reviewer → re-verify findings → fold → write `DESIGN-GATE-RECORD-v1.8.md`. Reviewer brief supplies DESIGN text + ordered list of code files to open (code-trace, not prose skim).

### DESIGN doc shape that works here

Mirror `DESIGN-v1.9-egress-policy.md` / `DESIGN-git-github-http-sinks.md` / `DESIGN-effect-breadth-exec.md`:

1. Header: milestone, phase, status Draft→pending trace, requirements gated, grounding sources, "decisions not options"
2. **§0 Purpose & Scope** — what is pinned; what is deferred (LLM multi-step, new sinks, session-wide waiver); hard-blocks Phases 48–52; no TCB code this phase
3. **§1 Plan-stream shape** — additive multi-node API on existing `Planner` seam
4. **§2 Worker sequential submit + handle bag**
5. **§3 Mid-loop Block-and-Hold confirm continuity**
6. **§4 I1×coding-loop bounds** (trusted-intent success path; Draft×CommitIrreversible)
7. **§5 Instruction vs value channel disjointness** (PLAN-03 / GATE-01)
8. **§6 Deny/abort semantics mid-stream**
9. **§7 Carry-forward invariants** (ProvideIntent-once, P33/P34, POLICY-02, Gate 3)
10. **§8 HYG-02 / Gate discipline**
11. **§9 Threat model** — one §/row per pitfall → named mechanism
12. **§10 Invariant preservation checklist** (I0/I1/I2, no EffectRequest, no batch authorize)
13. **§11 Fail-closed defaults table**
14. **§12 New-symbol summary** (or "none — no new mint sites / TaintLabels expected")
15. **§13 Adversarial-trace gate (DESIGN-20)** — orchestrator-owned; re-run triggers
16. **§14 Acceptance predicate**
17. **Amendments (post-review)** section placeholder for Round-1 fold

### Gate record shape that works here

Model on `DESIGN-GATE-RECORD-v1.9.md` / `-v1.8.md`:

- Phase, DESIGN under review, requirements gated, status CLEARED/NOT CLEARED
- Gate discipline restated (orchestrator-owned, non-self)
- Reviewer identity & independence (author ≠ reviewer; files opened listed)
- Findings table: severity / claim / re-verified code fact / resolution → DESIGN §
- Round-N amendments with orchestrator re-verification
- No-TCB-code reconfirmation (`git status --porcelain -- crates cli` empty; `check-invariants.sh` exit 0)
- Verdict authorizing Phases 48–52

### What the adversarial reviewer must pressure-test (Phase 47 specific)

| # | Attack surface | Why highest consequence |
|---|----------------|-------------------------|
| 1 | Cross-node taint laundering via `output_value_id` | Wire exists; worker currently discards handle — multi-step makes it load-bearing |
| 2 | ProvideIntent reopened mid-stream / second "declare trusted" verb | Only mint that yields UserTrusted from supplied string |
| 3 | Draft demotion "fixed" by weakening CommitIrreversible Step 0.5 | Coding loop pressure after exec/read |
| 4 | Batch authorize / EffectRequest / new effect path | Architectural lock + Gate 1 |
| 5 | Mid-loop confirm splits Session / reconnect-remint / session-wide waiver | Confirm designed for one effect, not a stream scheduler |
| 6 | Instruction channel collapsed into bindable ValueId | PLAN-03 / GATE-01 regression |
| 7 | Policy mid-stream rebind or I2 override narrative | POLICY-02/03 |
| 8 | Hybrid composition framed as CLI multi-step in acceptance text | LIVE-05 honesty class |
| 9 | New mint site outside Gate 3 loci | Gate 3 discipline |
| 10 | P33/P34 confirm-release order under multi-confirm Session | Recurring audit-gap class |

**Proven value of the discipline:** v1.9 gate caught BLOCKER-level I0 escape (`http.request` WRITE would inherit Observe) that plan-checker + green docs-only invariants both missed. [VERIFIED: `planning-docs/DESIGN-GATE-RECORD-v1.9.md`]

## Standard Stack

### Core (reuse only — zero new crates)

| Library / artifact | Version / locus | Purpose | Why Standard |
|--------------------|-----------------|---------|--------------|
| Rust edition 2021, workspace resolver 3 | root `Cargo.toml` | TCB language | Locked project stack [VERIFIED: Cargo.toml] |
| Existing workspace crates | `runtime-core`, `brokerd`, `executor`, `sandbox`, `adapter-fs`, `llm-planner` | Shipped substrate | Multi-step extends; does not replace [VERIFIED: workspace members] |
| `cli/caprun` + `caprun-worker` + `caprun-exec-launcher` | `cli/*` | Orchestrator / worker / exec child | Sibling `current_exe()` layout; packaging later [VERIFIED: Cargo.toml members] |
| tokio + serde_json + framed UDS | workspace deps | Broker accept + multi-call `SubmitPlanNode` | Already multi-submit safe [VERIFIED: brokerd server] |
| rusqlite + sha2/hmac | workspace deps | Session + authenticated audit DAG | `verify_chain` continuity [VERIFIED: workspace deps] |
| landlock + seccompiler + nix | workspace deps | Kernel boundary | Must not re-open ambient authority [VERIFIED: workspace deps] |
| reqwest (rustls) + ring + webpki-roots | shipped | Broker egress for push/PR/http | Gate 5 ring-only; no new crypto [VERIFIED: check-invariants Gate 5 PASS 2026-07-23] |
| `scripts/check-invariants.sh` | repo scripts | Architectural gate (Gates 1–6) | HYG-02 enforcement locus [VERIFIED: script run PASS] |
| `scripts/compose-verify.sh` / `mailpit-verify.sh` | repo scripts | Authoritative Linux verification | CLAUDE.md Phase 16+ policy [CITED: CLAUDE.md] |

### Supporting (process, not packages)

| Artifact | Purpose | When to Use |
|----------|---------|-------------|
| `planning-docs/DESIGN-v1.9-egress-policy.md` | Section shape + carry-forward policy↔I2 | Model § structure |
| `planning-docs/DESIGN-plan-executor.md` | ValueId handle model / PLAN-03 spine | Cite handle-only planner |
| `planning-docs/DESIGN-confirmation-release.md` | Single-shot confirm + P33/P34 | Block-and-Hold section |
| `planning-docs/DESIGN-session-trust-coherence.md` | Session trust / planner reduced decision | Instruction channel + demotion |
| `.planning/research/{SUMMARY,ARCHITECTURE,PITFALLS}.md` | v1.10 milestone research (2026-07-23) | Authoritative multi-step mechanism input |
| Prior `DESIGN-GATE-RECORD-v1.{8,9}.md` | Gate record template | Plan 02 / orchestrator |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Sequential plan stream on existing Planner | Batch `SubmitPlanDAG` authorize-all | **Rejected:** new I2 bypass surface; violates architectural lock |
| Block-and-Hold same Session | Exit worker + reconnect/`caprun continue` remint | **Rejected:** occupancy latch + empty ValueStore + ProvideIntent laundering tripwire |
| Deterministic multi-step first (later phases) | LLM multi-step tool-use now | **Deferred** past v1.10 (LLM-MS-01); design gate must not invent ReAct loop in TCB |
| Zero new crates | New orchestration / workflow crate | **Rejected:** HYG-02 default zero; product boundary Intent Runtime not agent framework |
| Orchestrator-submitted PlanNodes (no worker) | Unconfined ambient multi-step | **Rejected:** weakens kernel-confined worker as only egress story |

**Installation:** none — **zero** new packages this phase.

**Version verification:** N/A (no package installs). Workspace gates re-verified 2026-07-23: `check-invariants.sh` all PASS (Gates 1–6).

## Package Legitimacy Audit

> No external packages are installed in Phase 47 (docs-only design gate). HYG-02 pins **default zero new crates** for the entire multi-step milestone unless a later phase design-justifies an exception (none expected for plan-stream substrate).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| — | — | — | — | — | N/A | No installs |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram (target multi-step — DESIGN must pin)

```
Operator
  │  caprun run --policy P <coding-intent> <workspace> …
  ▼
CLI main (unconfined orchestrator)
  · create Session · bind_policy (POLICY-03, outside workspace)
  · start broker · spawn worker
  · mid-loop: surface effect_id + confirm/deny; hold stream
        │ UDS
        ▼
┌──────────────────────────┐     ┌─────────────────────────────┐
│ Broker (reference mon.)  │◄────│ Worker (self-confined)      │
│ · Session + audit DAG    │     │ · ProvideIntent ONCE        │
│ · ValueStore (mint only) │     │ · [optional] RequestFd      │
│ · N× SubmitPlanNode      │     │ · handle bag (ValueIds)     │
│ · PendingConfirmation    │     │ · loop: plan_next → submit  │
│ · policy immutable       │     │ · on Block: HOLD (no exit)  │
└───────────┬──────────────┘     │ · on Deny: ABORT remaining  │
            │                    └──────────────┬──────────────┘
            ▼                                   │ handles only
┌──────────────────────────┐                    ▼
│ Executor I2 (per node)   │         Planner trait (PLAN-03)
│ policy pre-narrow → I2   │         · plan / plan_next
│ no stream-wide waiver    │         · opaque ValueIds only
└──────────────────────────┘         · never mint / never taint
```

**Today (v1.9 gap the DESIGN pins closing):**

```
ProvideIntent → RequestFd → plan() → SubmitPlanNode ONCE → exit(on Block=1)
output_value_id bound and discarded (let _ = &output_value_id)
```

### Recommended Project Structure (phase touch points)

```
planning-docs/
├── DESIGN-multi-step-plan-stream.md   # NEW — Plan 01 deliverable
└── DESIGN-GATE-RECORD-v1.10.md        # NEW — after adversarial clear

# Explicitly NOT touched this phase:
crates/{executor,brokerd,sandbox,runtime-core}/
cli/caprun/src/{worker,planner,main}.rs   # multi-step code starts Phase 48+
Cargo.toml                                 # no new crates / deps
```

### Pattern 1: Sequential Plan Stream (pin as THE stream shape)

**What:** One Session, one worker connection, N sequential `SubmitPlanNode` calls. Each node independently policy+I2 evaluated. No batch authorize. Additive `Planner` API (`plan_next` and/or static sequence index) — keep existing one-shot `plan()` for email/file + LlmPlanner.

**When to use:** Entire v1.10 multi-step coding path.

**Ground truth already legal:** broker multi-submit on one connection [VERIFIED: `crates/brokerd/src/server.rs` `SubmitPlanNode` arm + connection loop]. Gap is worker loop + planner multi-node surface + CLI driver.

### Pattern 2: Handle Bag + Output Threading

**What:** Worker-side map of opaque `ValueId`s. On Allowed sinks that mint outputs (`process.exec` → `mint_from_exec`, etc.), broker returns `PlanNodeDecision { output_value_id: Some(id) }`. Worker stores handle for later `PlanArg`s. Planner only places handles offered by call-site convention — never upgrades trust.

**When to use:** LIVE-08 mid-loop I2 Block (tainted exec/http handle → sensitive arg). Success path preferably does **not** require intermediate untrusted outputs for irreversible sinks.

**Ground truth:** `output_value_id` already on `PlanNodeDecision` [VERIFIED: `crates/brokerd/src/proto.rs`]; worker discards today [VERIFIED: `cli/caprun/src/worker.rs` `let _ = &output_value_id`].

### Pattern 3: Block-and-Hold Confirm Continuity

**What:** On `BlockedPendingConfirmation` (I2 Block or always-confirm `git.push`), worker **stays connected** (or uses a design-locked same-Session hold). Human confirms durable `PendingConfirmation` snapshot (existing confirm path). Sink executes from snapshot — worker **does not re-submit** the blocked node. Remaining nodes continue under same Session id, same policy bind, same audit chain.

**When to use:** Any multi-node stream including `git.push` / confirm-releasable irreversible sinks.

**Rejected:** reconnect + ProvideIntent remint; dual-Session stitch; session-wide confirm waiver; auto-confirm mid-loop.

### Pattern 4: Trusted-Intent Success Path (I1×coding-loop)

**What:** Coding success path args (paths, commands, messages, remotes/refspecs, PR title/body) minted **once** at session start via ProvideIntent from operator-typed CLI/intent. Do **not** require multi-file untrusted RequestFd before CommitIrreversible nodes (HARDEN-01 demotes non-seed files → Draft → Step 0.5 denies CommitIrreversible).

**Effect classes (code-verified, DESIGN must not re-litigate without explicit fork):**

| Sink | Class | Draft session |
|------|-------|---------------|
| `git.commit` | MutateReversible | Allowed (class gate) |
| `http.request` GET | Observe | Allowed |
| `file.write` | CommitIrreversible | **Denied** Step 0.5 |
| `process.exec` | CommitIrreversible | **Denied** |
| `git.push` | CommitIrreversible | **Denied** |
| `github.pr` | CommitIrreversible | **Denied** |
| `http.request.write` | CommitIrreversible | **Denied** |

[VERIFIED: `crates/executor/src/sink_sensitivity.rs` `sink_effect_class`]

### Pattern 5: Instruction ≠ Value Channel

**What:** `task_instruction: Option<String>` may influence planner choice of which offered handle to place; it is **never** a `ValueId` and cannot bind as a sink arg. Values only as pre-minted handles. [VERIFIED: `cli/caprun/src/planner.rs` `Planner::plan` signature + module docs]

### Anti-Patterns to Avoid

- **Batch DAG authorize / EffectRequest:** Gate 1 + architectural lock
- **Planner-authored literals / taint strip:** DESIGN-plan-executor Phase 2 hole class
- **Mid-stream ProvideIntent / re-intent IPC:** UserTrusted laundering valve
- **Weaken Draft CommitIrreversible for "green tests":** I0/I1 breach
- **Hybrid LIVE as DONE claim:** v1.9 honesty gap this milestone exists to close
- **New crate for orchestration:** HYG-02
- **Self-review of DESIGN by authoring executor:** fails DESIGN-20

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-step orchestration | New workflow engine / agent framework crate | Sequential `SubmitPlanNode` loop on existing UDS + `Planner` trait | Product boundary; zero-crate HYG-02; broker already multi-call safe |
| Cross-node values | Planner retypes strings; second mint verb | Opaque `ValueId` handle bag + existing mint sites | I2 soundness spine (handle model) |
| Mid-loop human gate | Session-wide YOLO / auto-confirm | Existing `PendingConfirmation` single-shot + Block-and-Hold | Confirm substrate already durable |
| Policy for the loop | Cedar / "workflow allow" override I2 | Bound `SessionPolicy` pre-I2 only (POLICY-02) | I2 hardcoded in executor |
| Design-gate validation | Unit tests of unwritten multi-step code | Grep/section assertions + adversarial APPROVE + check-invariants | Doc-only phase |
| Effect path shortcut | Raw tool maps / EffectRequest | PlanNode only | Gate 1 |

**Key insight:** Multi-step security failures here are almost all **composition** of already-correct single-node controls (ProvideIntent-once × demotion × confirm × handle-only planner). The design gate exists to pin composition rules before code invents convenience bypasses.

## Common Pitfalls

### Pitfall 1: Cross-node taint laundering via `output_value_id`
**What goes wrong:** Intermediate exec/http/read output treated as trusted for PR body / push refspec.
**Why:** Multi-step first needs handle reuse; "tests passed ⇒ clean" intuition.
**How to avoid:** Opaque ValueIds only; no mid-stream ProvideIntent; no new mint without Gate 3; LIVE-08 genuine provenance root.
**Warning signs:** New `mint_from_*`; Allowed on exec-stdout→sensitive without confirm; stapled taint.

### Pitfall 2: ProvideIntent as multi-step trust valve
**What goes wrong:** Second trusted-declare after observations.
**How to avoid:** ProvideIntent exactly once, only before RequestFd [VERIFIED: `server.rs` intent_provided / fd_requested guards]. All coding trusted args at session start.

### Pitfall 3: Draft demotion "fixed" by weakening I1
**What goes wrong:** After exec/read demotion, irreversible sinks auto-Allow — or class rewritten.
**How to avoid:** Keep Step 0.5; success path avoids demotion before irreversible nodes; confirm ≠ class waiver.

### Pitfall 4: Hybrid sold as CLI multi-step
**What goes wrong:** In-crate `evaluate_plan_node_and_record_for_test` chain marketed as `caprun run`.
**How to avoid:** DESIGN pins acceptance: DONE requires CLI-driven multi-node one Session; hybrid only unit harness.

### Pitfall 5: Careless Planner multi-node extension
**What goes wrong:** Trait accepts tool output strings / ValueRecord / mint verbs.
**How to avoid:** Additive API still handles-only; deterministic planner first; LLM multi-step out of milestone.

### Pitfall 6: P33/P34 audit gap amplified by multi-confirm
**What goes wrong:** Terminal state before terminal event on push/PR under multi-node Session.
**How to avoid:** Restate precheck-before-burn + terminal-event-before-state; multi-node LIVE asserts events for every released effect.

### Pitfall 7: Mid-loop Block splits Session
**What goes wrong:** Worker exits; new Session for PR tail; policy rebind; I2 skipped after confirm.
**How to avoid:** Block-and-Hold same Session; subsequent nodes still full submit_plan_node.

### Pitfall 8: Policy↔I2 erosion under "workflow allow"
**What goes wrong:** Policy overrides Block; mid-stream rebind from workspace.
**How to avoid:** POLICY-02 unconditional I2; POLICY-03 bind once outside worker.

### Pitfall 9: Design-gate process failure (self-review / early TCB code)
**What goes wrong:** Author reviews own DESIGN; or Phase 48 code starts before CLEARED.
**How to avoid:** Orchestrator-owned spawn; plan verify: empty `crates/`/`cli/` porcelain; gate record independence section.

### Pitfall 10: Scope creep into implementation
**What goes wrong:** DESIGN phase "helps" by landing worker loop scaffolding.
**How to avoid:** Success criteria are docs + APPROVE only; plan must_haves forbid TCB paths.

## Code Examples

Verified patterns from **this codebase** (cite in DESIGN; do not invent parallel types).

### Planner seam (single-node today — extend additively)

```70:80:cli/caprun/src/planner.rs
    fn plan(
        &self,
        intent: &CaprunIntent,
        intent_value_id: ValueId,
        derived_recipient: Option<ValueId>,
        body: Option<ValueId>,
        trusted_subject_handle: ValueId,
        trusted_body_handle: ValueId,
        task_instruction: Option<String>,
    ) -> PlanNode;
```

DESIGN should pin an **additive** multi-node surface (e.g. `plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode>`) that preserves PLAN-03: only opaque `ValueId`s + typed intent; never `ValueRecord` / raw untrusted bytes / taint.

### Worker one-shot submit (gap)

Worker constructs `Box<dyn Planner>`, calls `plan(...)` once, sends `BrokerRequest::SubmitPlanNode { plan_node }`, receives `PlanNodeDecision { decision, output_value_id }`, **discards** `output_value_id`, and exits non-zero on `BlockedPendingConfirmation`. DESIGN pins replacing this with a sequential loop + hold-for-confirm without re-opening mint verbs. [VERIFIED: `cli/caprun/src/worker.rs`]

### CaprunIntent closed enum (coding variant later)

```22:27:crates/runtime-core/src/intent.rs
pub enum CaprunIntent {
```

Today: `SendEmailSummary`, `CreateFileFromReport` (and related). Phase 49 adds coding variant — DESIGN may name the variant and field set as decisions or mark naming as implementation detail with **closed enum** non-negotiable.

### ExecutorDecision outcomes multi-step must distinguish

- `Allowed` → dispatch; maybe `output_value_id`
- `BlockedPendingConfirmation` → hold + human gate (not silent continue)
- `Denied` (schema / slot / Draft class / method) → abort remaining (recommended)
- Policy deny as distinct machine-checkable reason (`policy_deny`) ≠ I2 Block

[VERIFIED: `crates/runtime-core/src/executor_decision.rs`; policy_gate tests]

### Gate 1 / Gate 3 (HYG-02)

```bash
# Must remain green after DESIGN authoring (docs must not introduce EffectRequest under crates/)
./scripts/check-invariants.sh
# Gate 1: no EffectRequest under crates/
# Gate 3: mint_from_read|derivation|exec|http|.mint( only at sanctioned loci
# Gate 5: ring-only (aws-lc-rs absent)
```

If multi-step needs a new mint site (default: **none**), DESIGN must explicitly amend Gate 3 allowlist — else leave list unchanged.

### Always-confirm git.push rewrite (broker)

Broker rewrites clean `git.push` Allowed → synthetic `BlockedPendingConfirmation` for always-confirm product model. Multi-step success path **will** hit mid-loop Block even without I2 taint. DESIGN must account for this — not only I2 Block. [VERIFIED: `crates/brokerd/src/server.rs` git.push rewrite path]

## State of the Art (in this project)

| Old Approach (≤v1.9) | Current Target (v1.10) | When Changed | Impact |
|----------------------|------------------------|--------------|--------|
| One-shot worker `plan` → single SubmitPlanNode | Sequential multi-node stream | v1.10 Phases 48–50 | Product path for coding loop |
| LIVE-05 hybrid in-crate multi-node | CLI-driven one Session multi-node | v1.10 Phase 51 | Honesty / DONE bar |
| `output_value_id` unused | Handle bag load-bearing | Phase 48 | Enables genuine mid-loop I2 proof |
| Confirm as end-of-session human gate | Mid-loop Block-and-Hold | Phase 50 | Same Session continuity |
| Design gates for sinks/policy | Design gate for **orchestration composition** | Phase 47 | Composition risks (not new sink physics) |

**Deprecated/outdated for this phase:**
- Treating multi-step as needing Temporal/LangGraph/agent framework — product boundary forbids
- Net-allowed git.push child (v1.8 FORK later overturned) — irrelevant to stream shape; do not re-open
- LLM multi-step tool-use as v1.10 TCB — deferred LLM-MS-01

## Downstream What DESIGN Must Pin (for Phases 48–52)

| Phase | Req | DESIGN pins that unlock it |
|-------|-----|----------------------------|
| 48 Plan-Stream Substrate | STREAM-01/02 | Exact stream API shape; handle bag rules; chain-head continuity; fail-closed on Deny; no new mint; ProvideIntent still once |
| 49 Deterministic Coding Planner | CODE-01/02 | Sink sequence over shipped sinks; trusted-intent-only success args; planner handles-only; email/file single-node remain green |
| 50 CLI + Confirm Continuity | CLI-01/02, CONFIRM-01 | Block-and-Hold mechanism (signal path: stdout vs pipe vs broker verb); no dual-Session; exit codes; no re-submit blocked node |
| 51 Non-hybrid LIVE | LIVE-07/08 | Acceptance framing: CLI-driven one Session; mid-loop I2 Block with genuine taint; verify_chain; hybrid forbidden as DONE |
| 52 Packaging | PKG-01 | No TCB pins required; may note three sibling bins layout unchanged by multi-step |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Recommended Block-and-Hold IPC detail (stdout convention vs side channel vs broker verb) is product discretion within same-Session constraint | Pattern 3 / Open Questions | Wrong UX choice is recoverable; wrong Session-split choice is a security failure — Session sameness is NOT assumed free |
| A2 | Gate record filename `DESIGN-GATE-RECORD-v1.10.md` follows v1.8/v1.9 naming | Prior Design-Gate Pattern | Cosmetic; orchestrator may pick equivalent path |
| A3 | Phase 47 needs **two** formal plans (author + clear) like P35 rather than one plan + orchestrator-only clear like P41 | Prior Design-Gate Pattern | Either works if DESIGN-20 independence holds; planner should pick one and state owner of spawn |
| A4 | No new `TaintLabel` / mint site required for multi-step substrate | HYG-02 / Don't Hand-Roll | If LIVE-08 needs a new mint, Gate 3 amendment must be explicit in DESIGN |

**If planner needs user confirmation:** only A1 UX signal path and A3 plan-split ownership are soft; security pins in DESIGN-19 are locked by requirements.

## Open Questions

1. **Exact worker↔main "blocked, waiting" signal**
   - What we know: confirm is durable + often separate process; ValueStore is process-lifetime; occupancy latch one-way.
   - What's unclear: stdout convention vs side UDS vs broker poll verb vs interactive in-process confirm in `caprun run`.
   - Recommendation: DESIGN must pick **one** primary path (recommend: main holds broker lifetime + interactive or documented dual-terminal confirm without worker exit/remint). Mark alternatives rejected with reason.

2. **Interactive confirm in `caprun run` vs dual-terminal `caprun confirm` only**
   - What we know: single-shot semantics non-negotiable; product already has `confirm`/`deny`/`review`.
   - What's unclear: UX product call.
   - Recommendation: either OK if same Session + no re-submit; pin machine-checkable stop semantics either way.

3. **Residual plan after mid-loop Deny / policy_deny**
   - What we know: research recommends abort remaining nodes + durable terminal events.
   - What's unclear: whether partial success exit code taxonomy is fixed.
   - Recommendation: pin **abort remaining** fail-closed; distinct exit codes (CLI-02) deferred detail to Phase 50 but semantics locked here.

4. **How many workspace files success path may RequestFd without demotion**
   - What we know: HARDEN-01 single trusted seed inode.
   - Recommendation: default **seed only / none** for irreversible success path; multi-file untrusted read then still push = **separate future design-gate**.

5. **`plan_next` vs static `Vec<PlanNode>` only**
   - What we know: static sequence is enough for deterministic coding planner.
   - Recommendation: pin additive API that supports static index now; leave reactive LLM out of trait requirements for v1.10.

6. **CaprunIntent coding variant naming / fields**
   - Product naming only; closed enum structure locked. May defer exact field names to Phase 49 if DESIGN pins "all success-path literals from operator intent at ProvideIntent".

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | Invariant gate / future phases | ✓ | 1.89.0 | — |
| `scripts/check-invariants.sh` | HYG-02 / phase verify | ✓ | Gates 1–6 PASS (2026-07-23) | — |
| Git working tree | No-TCB-code porcelain checks | ✓ | — | — |
| Docker / compose-verify | **Not required this phase** | optional | — | Design-gate is docs-only |
| Mailpit | **Not required this phase** | optional | — | No multi-step code; no SMTP LIVE |
| Fresh non-self reviewer agent (Fable-5 or project standard) | DESIGN-20 | process | — | **Blocking for gate clear** if Agent tool unavailable — orchestrator must resolve (manual human review or alternate model) |

**Missing dependencies with no fallback:**
- Ability to spawn a **non-self** adversarial reviewer (orchestrator Agent/Task tool or human). Self-read does not satisfy DESIGN-20.

**Missing dependencies with fallback:**
- Linux compose-verify — not needed until Phase 51; do not block Phase 47.

**Step 2.6 note:** No multi-step runtime external services required for this phase.

## Validation Architecture

> Design-gate Nyquist: verify **document completeness + process clearance**, not multi-step unit tests (code does not exist yet).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | None for multi-step code — **doc-assertion + process checks** |
| Config file | none — doc-only phase |
| Quick run command | `test -f planning-docs/DESIGN-multi-step-plan-stream.md && ./scripts/check-invariants.sh` |
| Full suite command | Same + gate-record CLEARED checks + `git status --porcelain -- crates cli` empty |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DESIGN-19 | DESIGN file exists | doc | `test -f planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Pins plan-stream shape (multi-node / sequential / not batch DAG) | doc-assertion | `grep -qiE 'plan.stream\|plan_next\|sequential' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'not batch\|batch.*reject\|no batch' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Pins handle bag / opaque ValueId / planner never mints | doc-assertion | `grep -qiE 'handle bag\|output_value_id' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'opaque ValueId\|planner never mints\|PLAN-03' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Pins Block-and-Hold same Session / no reconnect-remint | doc-assertion | `grep -qiE 'Block-and-Hold\|block and hold' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'same Session\|no reconnect\|remint' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Pins trusted-intent success path + no Draft CommitIrreversible weaken | doc-assertion | `grep -qiE 'trusted-intent\|ProvideIntent' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'CommitIrreversible\|Draft' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Pins instruction vs value disjointness | doc-assertion | `grep -qiE 'task_instruction\|instruction.*value\|value channel' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Pins deny/abort mid-stream semantics | doc-assertion | `grep -qiE 'abort\|deny' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Carry-forward ProvideIntent-once, P33/P34, POLICY-02 | doc-assertion | `grep -qiE 'ProvideIntent.*once\|exactly once' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'P33\|P34\|precheck\|terminal.event' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'POLICY-02\|never.*override I2\|I2.*unconditional' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-19 | Explicitly rejects EffectRequest / batch authorize | doc-assertion | `grep -qiE 'EffectRequest\|batch' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-20 | Declares orchestrator-owned non-self trace + re-run triggers | doc-assertion | `grep -qiE 'orchestrator-owned\|non-self' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 're-run\|re-runs' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| DESIGN-20 | Gate record exists with CLEARED/APPROVE | process | `test -f planning-docs/DESIGN-GATE-RECORD-v1.10.md && grep -qiE 'CLEARED\|APPROVE' planning-docs/DESIGN-GATE-RECORD-v1.10.md` | ❌ Wave 0 |
| DESIGN-20 | Reviewer independence recorded | process | `grep -qiE 'reviewer\|independence\|non-self\|Fable' planning-docs/DESIGN-GATE-RECORD-v1.10.md` | ❌ Wave 0 |
| HYG-02 | Zero new crates / Gate discipline re-asserted in DESIGN | doc-assertion | `grep -qiE 'zero new crate\|HYG-02\|Gate 3\|check-invariants' planning-docs/DESIGN-multi-step-plan-stream.md` | ❌ Wave 0 |
| HYG-02 | check-invariants still green; no TCB code | automated | `./scripts/check-invariants.sh && test -z "$(git status --porcelain -- crates cli)"` | ✅ scripts exist |

### Sampling Rate

- **Per task commit:** section-presence greps for sections authored that task + empty TCB porcelain
- **Per wave merge:** full DESIGN-19 grep bundle + `check-invariants.sh`
- **Phase gate:** DESIGN-20 gate record CLEARED + all DESIGN-19 pins greppable + no TCB code + invariants green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `planning-docs/DESIGN-multi-step-plan-stream.md` — does not exist yet (primary deliverable)
- [ ] `planning-docs/DESIGN-GATE-RECORD-v1.10.md` — does not exist yet (post-trace)
- [ ] Optional: `.planning/phases/47-.../47-VALIDATION.md` — planner may emit Nyquist contract (model on Phase 31 VALIDATION)
- [ ] Framework install: **none** — no multi-step test code this phase

*(Existing `check-invariants.sh` covers architectural non-regression. No unit-test framework gap for multi-step implementation — out of phase scope.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (design gate; no new auth surface) | Existing session/auth-grant unchanged |
| V3 Session Management | **yes** (composition) | One Session continuity; occupancy latch; immutable policy bind; no dual-Session stitch |
| V4 Access Control | **yes** | ConnectionRole (Worker vs Planner); policy pre-I2; I2 hardcoded |
| V5 Input Validation | **yes** | Closed `CaprunIntent`; PlanNode schema; handles-only planner args |
| V6 Cryptography | no new crypto | Gate 5 ring-only; no new crates |
| V1 Architecture | **yes** | Design gate + adversarial code-trace; Gate 1 EffectRequest absence |

### Known Threat Patterns for multi-step plan streams

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-node taint laundering | Tampering / Elevation | Opaque ValueIds; no mid-stream ProvideIntent; genuine mint provenance; I2 per node |
| ProvideIntent remint after observation | Elevation | ProvideIntent once before RequestFd; occupancy latch |
| Draft auto-authorize irreversible | Elevation / I0-I1 bypass | Keep Step 0.5 CommitIrreversible deny; trusted-intent success path |
| Batch authorize / EffectRequest | Tampering / Spoofing | Gate 1; sequential SubmitPlanNode only |
| Session split after confirm | Tampering / repudiation risk | Block-and-Hold same Session; same audit chain |
| Session-wide confirm waiver | Elevation | Single-shot per effect_id; subsequent nodes full I2 |
| Policy override I2 | Elevation | POLICY-02; distinct policy_deny vs Block |
| Instruction→value channel collapse | Tampering | task_instruction non-handle; PLAN-03 |
| Hybrid framing as CLI multi-step | Information disclosure / integrity of claims | LIVE DONE requires real multi-node `caprun run` |
| Confirm terminal-state before event | Repudiation | P33/P34 precheck-before-burn |
| New mint outside quarantine | Tampering | Gate 3; default zero new mint sites |
| Self-reviewed design greenlights unsound composition | Elevation (latent) | Non-self orchestrator-owned code-trace |

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` — DESIGN-19/20, HYG-02, STREAM/CODE/CLI/CONFIRM/LIVE mapping [VERIFIED: file read]
- `.planning/ROADMAP.md` / `.planning/STATE.md` — Phase 47 success criteria + phase structure [VERIFIED]
- `.planning/research/SUMMARY.md`, `ARCHITECTURE.md`, `PITFALLS.md` — v1.10 milestone research 2026-07-23 [VERIFIED]
- `planning-docs/DESIGN-v1.9-egress-policy.md`, `DESIGN-GATE-RECORD-v1.9.md` — design-gate + adversarial clear precedent [VERIFIED]
- `planning-docs/DESIGN-plan-executor.md`, `DESIGN-taint-model.md` — handle model / I2 spine [VERIFIED]
- `.planning/milestones/v1.9-phases/41-.../41-01-PLAN.md`, `35-.../35-01-PLAN.md`, `35-02-PLAN.md` — plan structure for design gates [VERIFIED]
- `.planning/milestones/v1.9-phases/31-effect-breadth-design-gate/31-VALIDATION.md` — design-gate Nyquist pattern [VERIFIED]
- Live code: `cli/caprun/src/planner.rs`, `worker.rs`; `crates/brokerd/src/{server,proto,confirmation}.rs`; `crates/executor/src/sink_sensitivity.rs`; `crates/runtime-core/src/{plan_node,intent,executor_decision}.rs`; `scripts/check-invariants.sh` [VERIFIED: read/grep 2026-07-23]
- `CLAUDE.md` / `planning-docs/PLAN.md` — product + architectural locks [VERIFIED]

### Secondary (MEDIUM confidence)

- CaMeL (arXiv:2503.18813) — control/data separation analogy already adopted in DESIGN-plan-executor (background only)
- Industry coding agents (Aider/SWE-agent) — table-stakes loop shape; **not** authority model for caprun

### Tertiary (LOW confidence)

- Exact interactive-confirm UX preferences — product discretion [ASSUMED open until DESIGN pins]

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — zero new crates; workspace + gates verified this session
- Architecture: **HIGH** — live single-node path + milestone research + prior DESIGN gates aligned
- Pitfalls: **HIGH** — project retros + research PITFALLS + gate records of real catches
- Process (adversarial clear): **HIGH** — unbroken v1.0–v1.9 precedent documented
- Open UX signal path: **MEDIUM** — must be decided in DESIGN, not assumed

**Research date:** 2026-07-23
**Valid until:** 2026-08-22 (30 days; substrate stable; re-verify file:line citations if large TCB churn before authoring)

**Graph context:** `.planning/graphs/graph.json` absent — no graphify inject.

**CONTEXT.md:** none (discuss-phase skipped under --auto). Locked scope taken from REQUIREMENTS + ROADMAP + CLAUDE.md + milestone research.
