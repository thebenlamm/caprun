# Phase 48: Plan-Stream Substrate - Research

**Researched:** 2026-07-27
**Domain:** Worker sequential multi-submit + opaque handle bag on shipped caprun Intent Runtime (Rust TCB; zero new crates)
**Confidence:** HIGH

## Summary

Phase 48 is the **first multi-step TCB / worker code phase** of v1.10. Phase 47 CLEARED (`planning-docs/DESIGN-GATE-RECORD-v1.10.md`); multi-step work under `crates/{executor,brokerd,sandbox,runtime-core}` and the worker submit path in `cli/caprun` is authorized **only under** the pins in `planning-docs/DESIGN-multi-step-plan-stream.md`. Discuss was skipped — locked authority is DESIGN + STREAM-01/02 + live code.

The physics already exist: the broker connection loop accepts N independent `SubmitPlanNode` calls on one Session (`server.rs` `handle_connection` loop at `:572+`); each arm runs policy pre-I2 then I2 via `evaluate_plan_node_and_record`; `PlanNodeDecision.output_value_id` is already returned for Allowed `process.exec` / `git.commit` / `http.request` mints. The **gap is composition**: the worker is one-shot (`planner.plan` → one submit → discard `output_value_id` via `let _ = &output_value_id` at `worker.rs:389` → exit), and `Planner` is one-shot (`plan()` only at `planner.rs:61-79`). STREAM-01/02 close that gap with a sequential submit loop + opaque ValueId handle bag + chain-head continuity — **not** a batch DAG, **not** a new IPC effect verb, **not** a coding recipe (Phase 49), **not** full Block-and-Hold product UX (Phase 50).

**Primary recommendation:** Keep existing one-shot `Planner::plan()` green; add an **additive** multi-node surface (static step index / `plan_next` returning `Option<PlanNode>`, PLAN-03 handles-only). Replace the worker one-shot tail with a sequential loop that stores **any** `Some(output_value_id)` in a worker-local `HashMap`/bag of opaque `ValueId`s, offers bag handles by call-site convention to the next plan step, aborts remaining on Deny/`policy_deny`, and on `BlockedPendingConfirmation` **stops without re-submit** (substrate-ready; product hold = Phase 50). Prove with a minimal N-node tracer (broker multi-submit + bag threading + `verify_chain` true). Zero new crates, zero new mint sites, fix stale "process.exec only" comments as docs drift.

## User Constraints (from DESIGN + REQUIREMENTS; discuss skipped)

> No `*-CONTEXT.md` for this phase. Locked authority is `planning-docs/DESIGN-multi-step-plan-stream.md` (CLEARED), STREAM-01/02, and CLAUDE.md / PLAN.md product locks.

### Locked Decisions (must honor — do not re-litigate)

1. **Stream shape:** One Session, one worker connection, **N sequential** `SubmitPlanNode` only — **not** batch DAG authorize, **not** free-form tool-map / `EffectRequest`, **not** orchestrator-submitted PlanNodes as product path (DESIGN §1).
2. **Broker multi-submit already legal** — gap is worker loop + planner multi-node surface + chain/handle discipline (DESIGN §0 table, §1.1).
3. **Handle bag:** Opaque `ValueId`s only — never literals, taint labels, or `ValueRecord`s. Store **any** `Some(output_value_id)` (process.exec, git.commit, http.request). Stale "process.exec only" comments are **drift not authority** (DESIGN §2.2 F-01).
4. **Post-confirm outputs out-of-bag:** Confirm does not mint into live worker ValueStore and must not re-`submit_plan_node` the blocked node (DESIGN §2.2 F-02).
5. **ProvideIntent exactly once** before RequestFd; mid-stream re-ProvideIntent DENIED by broker (DESIGN §2.3, §7.1).
6. **Planner never mints / never strips taint** (PLAN-03); places handles offered by call-site convention only (DESIGN §2.2, §5).
7. **Trusted-intent success path** for coding later; **no** Draft×CommitIrreversible Step 0.5 weaken (DESIGN §4) — Phase 48 must not introduce a demotion-before-irreversible success path.
8. **Deny / policy_deny → abort remaining** fail-closed; Block → hold semantics (product hold Phase 50); sequential order only (DESIGN §6).
9. **HYG-02:** Default **zero new crates**; Gate 1 EffectRequest ban; Gate 3 mint list **unchanged** unless DESIGN amends by name; `check-invariants.sh` architectural gate; compose-verify / mailpit-verify authoritative Linux (DESIGN §8).
10. **Empty multi-node stream** is reject/N/A — not silent success (DESIGN §8.2).
11. **CaprunIntent coding variant** is Phase 49 (closed enum only) — not required to land STREAM-01/02 substrate (DESIGN §8.3, §12).
12. **Re-run adversarial design-trace** if stream shape, confirm-hold, or trusted-arg mint path pivots mid-implementation (DESIGN §13.2 / GATE-RECORD).

### Claude's Discretion (recommend in plan)

- Exact additive Planner API shape: `plan_next` vs static sequence index vs both (DESIGN §1.3 default: static index sufficient for v1.10).
- Handle-bag type name / map keying (`ValueId` already `Hash+Eq`; optional named-slot bag vs pure `Vec`/`HashMap`).
- How much Block-and-Hold **hook** Phase 48 leaves (stop-without-re-submit vs stay-connected wait) — DESIGN product hold is Phase 50; substrate must not invent reconnect-remint.
- Minimal tracer sink pair for STREAM-01/02 (recommend: process.exec → process.exec command-taint Block for genuine bag proof; plus trusted multi-Allowed chain for verify_chain).
- Whether to land docs-only comment fixes for F-01 drift in the same plans as the loop.

### Deferred Ideas (OUT OF SCOPE for Phase 48)

- Deterministic coding recipe edit→test→commit→push→PR (Phase 49 / CODE-01/02)
- `caprun run` multi-node CLI driver + exit-code taxonomy productization (Phase 50 / CLI-01/02)
- Full mid-loop Block-and-Hold product path (worker stay-connected + main confirm UX) (Phase 50 / CONFIRM-01)
- Non-hybrid LIVE multi-node DONE (Phase 51 / LIVE-07/08)
- Packaging (Phase 52 / PKG-01)
- LLM multi-step / ReAct / agent frameworks / batch DAG / session-wide confirm waiver / new sinks / new mint sites / new crates

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STREAM-01 | In one Session, on one worker connection, evaluate and submit **N sequential plan nodes** (`SubmitPlanNode` × N). Each node independently I2-evaluated; policy remains pre-I2 narrowing gate; no batch-authorize. Worker loop + **chain-head continuity** so every decision/event lands on the same audit DAG with `verify_chain` true | §§ Architecture Patterns 1–2, Code Examples (worker loop + broker multi-submit), Validation Architecture, Don't Hand-Roll |
| STREAM-02 | Intermediate sink outputs as `output_value_id` carried only as **opaque ValueIds** in a worker-side handle bag; retain genuine taint/provenance. Planner places handles only — never literals, never mid-stream ProvideIntent. ProvideIntent **exactly once** before RequestFd | §§ Architecture Pattern 2, Common Pitfalls 1–2, Code Examples (bag), Security Domain |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Sequential multi-submit loop | Worker (`cli/caprun` process) | Planner seam (in-process trait) | Multi-step must stay inside confined worker egress story — not unconfined orchestrator ambient submit |
| Per-node policy pre-I2 + I2 | Executor (Rust TCB) | Broker `evaluate_plan_node_and_record` | Already shared path; multi-step must not add batch authorize |
| Handle bag (opaque ValueIds) | Worker (in-process map) | Broker ValueStore (mint + resolve) | Bag is worker-local routing table; taint/literals stay broker-owned |
| Intermediate output mint | Broker (`server.rs` Allowed arms) | quarantine mint helpers | Gate 3 loci only; Phase 48 **consumes** handles, does not add mint sites |
| ProvideIntent-once / occupancy | Broker connection locals + latch | Worker startup order | Already enforced; multi-step must not reopen |
| Audit chain / `verify_chain` | Broker SQLite audit DAG | CLI audit viewer (later) | Each submit advances same session head; continuity is STREAM-01 proof |
| Additive multi-node Planner surface | `cli/caprun/src/planner.rs` | Worker call site | PLAN-03 compile-time boundary; keep one-shot `plan()` for email/file/LLM |
| Block-and-Hold product UX | CLI main + confirm substrate | Worker hold | **Phase 50**; Phase 48 only fail-closed stop without re-submit |
| Kernel confinement | Sandbox | Worker self-confine after connect | Unchanged order; multi-step must not re-open ambient net/exec |
| Architectural invariant gate | `scripts/check-invariants.sh` | compose-verify / mailpit-verify | Gate 1/3; Linux authority |

## Project Constraints (from CLAUDE.md)

Treat with same authority as locked DESIGN decisions:

1. **Source of truth:** `planning-docs/PLAN.md` wins on doc/code conflicts.
2. **Effect path locked:** `submit_plan_node(session_id, PlanNode { sink, args: ValueIds })` only — never raw `EffectRequest`. Gate 1 fails build if `EffectRequest` under `crates/` (annotate intentional mentions with `planner-discipline-allow`).
3. **I0 / I1 / I2:** I2 hardcoded in Rust executor; policy never disables I2; untrusted seed → draft-only; no ambient authority for workers.
4. **Terminology locked:** Intent, Session, Planner, Worker, Broker, Adapter, Effect, Artifact, Event. Project/binary = `caprun`.
5. **TCB is Rust.** Linux-only security claims; macOS stubs expected (`#[cfg(target_os = "linux")]` tests).
6. **From Phase 16+:** Linux verification that may touch SMTP uses `scripts/mailpit-verify.sh`; full composed LIVE uses `scripts/compose-verify.sh`. Never bare `docker run rust:1` alone when SMTP may fire. Never bind named Docker volumes for `CARGO_TARGET_DIR` as a manual speed hack.
7. **Out of scope:** agent frameworks, LLM multi-step, Cedar, web UI, cross-host/Biscuit, gVisor/Firecracker until relevant gates.

## Standard Stack

### Core (reuse only — zero new crates / packages)

| Library / artifact | Version / locus | Purpose | Why Standard |
|--------------------|-----------------|---------|--------------|
| Rust edition 2021, workspace resolver 3 | root `Cargo.toml` | TCB language | Locked project stack [VERIFIED: Cargo.toml + `cargo --version` 1.89.0] |
| `runtime-core` | workspace | `PlanNode`, `PlanArg`, `ValueId` (Hash+Eq), `ExecutorDecision`, `CaprunIntent` | Handle model spine [VERIFIED: `plan_node.rs:72-73`] |
| `brokerd` | workspace | Multi-submit loop, mint, audit DAG, `verify_chain` | Already multi-submit legal [VERIFIED: `server.rs:572+`, `proto.rs:136-138`] |
| `executor` | workspace | Per-node I2 + policy pre-gate | Unchanged per-node path [VERIFIED: DESIGN §1.1 cites evaluate path] |
| `sandbox` | workspace | Landlock + seccomp self-confine | Worker order unchanged [VERIFIED: `worker.rs` module docs] |
| `cli/caprun` worker + planner | `cli/caprun/src/{worker,planner}.rs` | **Primary change surface** | One-shot → sequential [VERIFIED: `worker.rs:361-412`] |
| tokio + serde_json + framed UDS | workspace deps | IPC framing | Existing 4-byte LE + JSON [VERIFIED: `worker.rs:470-489`] |
| rusqlite + sha2/hmac | workspace deps | Authenticated audit DAG | `verify_chain` continuity [VERIFIED: audit usages in tests] |
| `std::collections::HashMap` | std | Handle bag | `ValueId: Hash+Eq` already [VERIFIED: `plan_node.rs:70-73`] |
| `scripts/check-invariants.sh` | repo | Gates 1–6 | HYG-02 enforcement [VERIFIED: PASS 2026-07-27] |
| `scripts/mailpit-verify.sh` / `compose-verify.sh` | repo | Authoritative Linux | CLAUDE.md Phase 16+ [CITED: CLAUDE.md] |

### Supporting

| Artifact | Purpose | When to Use |
|----------|---------|-------------|
| `planning-docs/DESIGN-multi-step-plan-stream.md` | Authoritative pins | Every task decision |
| `planning-docs/DESIGN-GATE-RECORD-v1.10.md` | CLEARED + re-run triggers | Before any pivot on stream/confirm/mint |
| `planning-docs/DESIGN-plan-executor.md` | PLAN-03 / handle spine | Planner API additions |
| `planning-docs/DESIGN-confirmation-release.md` | No re-submit blocked node | Block branch semantics |
| `crates/brokerd/tests/replay_cas.rs` | Same-connection multi-`SubmitPlanNode` precedent | Broker STREAM-01 harness shape |
| `cli/caprun/tests/s9_process_exec_block.rs` + `live_acceptance_v1_7_composed.rs` | Hybrid genuine-taint two-node pattern | STREAM-02 taint proof analog (must not claim CLI DONE) |
| `cli/caprun/tests/planner.rs` | Pure planner unit tests (macOS-safe) | Additive multi-node surface tests |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Sequential N× `SubmitPlanNode` | Batch `SubmitPlanDAG` / multi-node authorize | **Rejected** — I2 bypass surface (DESIGN §1.4) |
| Worker-side handle bag | Broker-side "session bag" IPC | Unnecessary; bag is routing only; taint already in ValueStore |
| Additive `plan_next` | Replace `plan()` with stream-only API | Breaks email/file/LLM single-node paths — keep both |
| Substrate stop-on-Block | Full Block-and-Hold in Phase 48 | Product UX belongs Phase 50; substrate must not invent reconnect-remint |
| New crate `plan-stream` | In-tree worker/planner | HYG-02 zero crates |
| CaprunIntent coding variant in 48 | Test-only multi-node planner | Coding recipe is Phase 49; substrate can use test planner / static sequence over existing sinks |

**Installation:** none — zero external packages.

**Version verification:** No new packages. Workspace builds with `cargo 1.89.0` / `rustc 1.89.0` on the research host [VERIFIED: local toolchain].

## Package Legitimacy Audit

> Phase 48 installs **zero** external packages (HYG-02 / DESIGN §8.4 pattern continues). Package Legitimacy Gate **N/A**.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| *(none)* | — | — | — | — | — | No installs |

**Packages removed due to [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                    ┌─────────────────────────────────────┐
                    │ CLI main (unchanged product path)   │
                    │ session create · policy bind · spawn│
                    └──────────────┬──────────────────────┘
                                   │ INTENT / BROKER_SOCK
                                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ Worker (caprun-worker) — Phase 48 change surface                          │
│  1. connect abstract UDS → self-confine (order UNCHANGED)               │
│  2. ProvideIntent EXACTLY ONCE → IntentAccepted handles                  │
│  3. [optional] RequestFd → claims / derivation (existing)                │
│  4. seed handle bag with ProvideIntent + claim handles                   │
│  5. LOOP (NEW):                                                          │
│       plan_next(ctx, bag) → Option<PlanNode>                             │
│         None → exit success (if ≥1 node submitted) / reject empty        │
│         Some(node) → SubmitPlanNode                                      │
│            ├─ Allowed + Some(output_value_id) → bag.insert(id)           │
│            ├─ Allowed + None → continue                                  │
│            ├─ Denied / policy_deny → ABORT remaining (exit non-zero)     │
│            └─ BlockedPendingConfirmation → STOP (no re-submit;           │
│                 product hold = Phase 50)                                 │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │ framed UDS (existing verbs only)
                                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ Broker (mostly UNCHANGED for multi-submit)                                │
│  handle_connection loop: each SubmitPlanNode independently:              │
│    evaluate_plan_node_and_record → policy pre-I2 → I2 → maybe dispatch  │
│    mint output (exec/commit/http) → PlanNodeDecision{decision, out_id} │
│  ProvideIntent-once guards · occupancy latch · immutable policy          │
│  audit DAG: same session_id chain head advanced per event                │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │
                ┌───────────────┴────────────────┐
                ▼                                ▼
        Executor (I2 TCB)                 Audit SQLite
        per-node, unconditional           verify_chain(session)
```

### Recommended Project Structure (touch points only)

```
cli/caprun/src/
├── worker.rs          # MODIFY: sequential loop + handle bag; decision branch table
├── planner.rs         # MODIFY: additive multi-node surface; keep plan() + DeterministicPlanner/LlmPlanner
└── main.rs            # LIKELY UNCHANGED in 48 (CLI multi-node = Phase 50)

cli/caprun/tests/
├── planner.rs         # EXTEND: multi-node surface unit tests (macOS-safe)
└── stream_substrate.rs  # NEW (recommended): N-node / bag / ProvideIntent-once proofs
                         # Linux-gated legs where process.exec / confinement required

crates/brokerd/tests/
└── stream_multi_submit.rs  # NEW (recommended): same-connection N× SubmitPlanNode + verify_chain
                            # pattern after replay_cas.rs

crates/brokerd/src/
├── server.rs          # OPTIONAL docs-only: fix "process.exec only" comment drift (F-01)
└── proto.rs           # OPTIONAL docs-only: same

# DO NOT touch for STREAM-01/02 unless fixing comments:
crates/executor/       # per-node I2 already correct
crates/runtime-core/   # CaprunIntent coding variant = Phase 49
scripts/check-invariants.sh  # Gate 3 list stays unless DESIGN amends
```

### Pattern 1: Sequential Plan Stream (STREAM-01)

**What:** One Session, one worker connection, N sequential `SubmitPlanNode` calls. Each node independently policy-pre-I2 then I2. No batch authorize.

**When to use:** Always for v1.10 multi-step; this phase makes the worker implement it.

**Live gap today** [VERIFIED: `cli/caprun/src/worker.rs:361-412`]:

```361:412:cli/caprun/src/worker.rs
    let plan_node = planner.plan(
        &intent,
        intent_value_id,
        derived_recipient,
        body,
        trusted_subject_handle,
        trusted_body_handle,
        task_instruction,
    );

    // ── Submit for I2 evaluation (no session_id field — HARD-03) ─────────────
    send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;
    // …
    let _ = &output_value_id;
    if !matches!(decision, ExecutorDecision::Allowed) {
        eprintln!(
            "[worker] NOT ALLOWED ({decision:?}): no effect ran — exiting 1"
        );
        std::process::exit(1);
    }
    Ok(())
```

**Broker already multi-submit** [VERIFIED: `crates/brokerd/src/server.rs:572-610` loop + `:2221-2267` arm]: connection loop reads framed requests until EOF; each `SubmitPlanNode` independently evaluates and responds. `replay_cas.rs` already submits the **same** node twice on one connection [VERIFIED: `crates/brokerd/tests/replay_cas.rs:312-350`].

### Pattern 2: Handle Bag + Opaque Output Threading (STREAM-02)

**What:** Worker-side map of opaque `ValueId`s. On Allowed with `Some(output_value_id)`, insert. Later plan steps receive bag handles via call-site convention; planner only places named handles.

**Mint sources that produce `Some` today** [VERIFIED: `server.rs:1274-1299` process.exec; `:1308-1332` git.commit; `:1343-1401` http.request] — all untrusted provenance via `mint_from_exec` / `mint_from_http` (no trust upgrade).

**Bag rules (DESIGN §2.2):**

| Rule | Behavior |
|------|----------|
| Store | Any `Some(output_value_id)` regardless of sink |
| Type | Opaque `ValueId` only — never literal / taint / ValueRecord |
| Offer | Worker chooses which handles to pass into `plan_next` / stream context |
| Place | Planner puts offered handles into `PlanArg`s only |
| Post-confirm | Out of bag — do not re-submit blocked node to "recover" output |
| ProvideIntent | Never mid-stream to remint UserTrusted |

**ValueId is Hash+Eq** for `HashMap` [VERIFIED: `plan_node.rs:70-73`].

### Pattern 3: Additive Multi-node Planner Surface

**What:** Keep `plan()` for email/file + `LlmPlanner`. Add static index and/or `plan_next` → `Option<PlanNode>` with PLAN-03 signature (typed intent + opaque ValueIds only; `task_instruction` remains `Option<String>`, never ValueId).

**DESIGN default (Open Question 5):** static ordered sequence sufficient for v1.10; reactive LLM `plan_next` not required (DESIGN §1.3).

**Recommended Phase 48 shape (discretion, aligned with DESIGN §12 planning-only names):**

```rust
// Planning target — names flexible; PLAN-03 semantics are not.
// Source: DESIGN §1.2–1.3 + .planning/research/ARCHITECTURE.md Pattern 1

pub trait Planner {
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

    /// Additive multi-node surface. Default impl may return None after
    /// delegating once via plan() for single-node planners (email/file/LLM).
    fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode> {
        let _ = ctx;
        None // or one-shot: if ctx.step == 0 { Some(self.plan(...)) } else { None }
    }
}

/// Worker-owned routing context — never broker authority.
/// Only opaque ValueIds + typed intent + step index.
pub struct PlanStreamContext {
    pub intent: CaprunIntent,
    pub step_index: usize,
    // named or map of opaque handles from ProvideIntent + bag
    pub handles: /* HashMap<String, ValueId> or structured fields */,
}
```

**Phase 48 does not need CaprunIntent coding variant** — use a **test-only** multi-node planner (or a private struct implementing the additive surface) that emits a fixed N-node sequence over existing sinks for STREAM proofs. Production coding planner = Phase 49.

### Pattern 4: Decision Branch Table (mid-stream)

| Outcome | Phase 48 action | Phase 50 product |
|---------|-----------------|------------------|
| `Allowed` | Store optional output handle; continue loop | same |
| `Denied` / `policy_deny` | **Abort remaining**; exit non-zero | distinct exit codes (CLI-02) |
| `BlockedPendingConfirmation` | **Stop**; do **not** re-submit blocked node; exit non-zero (substrate) | Block-and-Hold: stay connected, main confirms durable pending, continue remaining nodes same Session |
| `NotImplemented` | Abort (treat as non-Allowed) | same |

Phase 48 **must leave ready**: no re-submit of blocked node; no ProvideIntent remint; no dual-Session stitch. Phase 48 **must not** invent reconnect-remint as temporary "hold".

### Anti-Patterns to Avoid

- **Batch authorize / multi-node one-shot I2:** violates STREAM-01 and DESIGN §1.4.
- **Bag stores literals or "trusted stdout" strings:** laundering; I2 becomes theater.
- **Mid-stream ProvideIntent** after observing exec/http output: UserTrusted remint valve (broker will reject; do not add worker path that tries).
- **Planner mints or strips taint:** PLAN-03 / Gate 3 violation.
- **Re-submit blocked node after confirm:** forbidden by confirmation DESIGN + F-02.
- **Claim hybrid in-crate multi-node as CLI multi-step DONE:** honesty class LIVE-05; DONE is Phase 51.
- **Weaken Draft×CommitIrreversible to green multi-file loops:** I0/I1 breach (DESIGN §4.3).
- **New mint site "for intermediate convenience":** Gate 3; DESIGN default zero new mints.
- **Empty stream exits 0:** DESIGN §8.2 fail-closed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-submit protocol | New `SubmitPlanStream` IPC verb | Existing `SubmitPlanNode` × N | Broker already multi-call; new verb is batch temptation |
| Taint tracking in worker | Worker-side taint labels | Broker ValueStore + opaque bag | Taint must stay broker-owned for I2 |
| Audit continuity | Custom chain-head in worker | Existing `append_event` / `verify_chain` | Single linear MAC chain already correct |
| Handle bag map type | New crate / custom UUID wrapper | `HashMap` + existing `ValueId` | Already Hash+Eq |
| Multi-node planner framework | Temporal/LangGraph/agent crate | Additive trait + static sequence | Product boundary + HYG-02 |
| Confirm mid-loop | Reconnect + remint + dual Session | Durable `PendingConfirmation` (Phase 50 hold) | Occupancy latch + empty ValueStore |
| Genuine taint proof | Staple taint at sink | Real mint_from_exec rooted on process_exited | §9 / LIVE lineage |

**Key insight:** Phase 48 is **composition discipline**, not new security physics. Hand-rolling a second effect path or trust channel is how multi-step fails.

## Common Pitfalls

### Pitfall 1: Cross-node taint laundering via `output_value_id`
**What goes wrong:** Treat exec/http/commit output as trusted for a later sensitive arg (PR body, push refspec, command).  
**Why it happens:** Intermediate outputs "feel" like clean pipeline data; bag looks like promotion.  
**How to avoid:** Opaque handles only; never strip taint; no mid-stream ProvideIntent; negative test: bagged exec output → sensitive sink arg → I2 Block with genuine provenance root.  
**Warning signs:** Bag type includes `String`; planner method accepts literal; "sanitize then remint" helper.

### Pitfall 2: ProvideIntent remint mid-stream
**What goes wrong:** After observation, worker tries second ProvideIntent to mint UserTrusted from observed content.  
**Why it happens:** Convenience after reading test output.  
**How to avoid:** Rely on broker guards (`intent_provided` / `fd_requested`); never add worker remint path; all success-path trusted args at session start (later coding path).  
**Warning signs:** Loop body contains ProvideIntent; tests expect second IntentAccepted.

### Pitfall 3: Under-bagging non-exec outputs
**What goes wrong:** Bag only process.exec because stale comments say so — drop git.commit/http.request handles.  
**Why it happens:** F-01 comment drift at `worker.rs:376-381`, `proto.rs:242-253`, `server.rs:2257-2259`.  
**How to avoid:** Store any `Some(output_value_id)`; fix comments as docs-only in same phase.  
**Warning signs:** `if sink == process.exec` around bag insert.

### Pitfall 4: Batch or dual-connection "optimization"
**What goes wrong:** SubmitPlanDAG, parallel workers, or planner-role connection used as multi-step product path.  
**Why it happens:** Latency / API aesthetics.  
**How to avoid:** Sequential one worker connection only; planner-role connection remains capability-restricted (no mint) — not the multi-step product driver.  
**Warning signs:** New proto variants; multi-threaded submit.

### Pitfall 5: Silent continue past Deny/Block
**What goes wrong:** Loop continues after Denied or treats Block as soft skip.  
**Why it happens:** Porting agent "retry next tool" habits.  
**How to avoid:** Branch table (Pattern 4); abort remaining on Deny; stop on Block without re-submit.  
**Warning signs:** `continue` after non-Allowed without exit.

### Pitfall 6: Breaking one-shot email/file/LLM paths
**What goes wrong:** Refactor of `plan()` / worker startup regresses CONTROL-01 live email or file.create.  
**Why it happens:** Shared loop without default one-shot behavior.  
**How to avoid:** Default `plan_next` one-shot adapter over existing `plan()`; keep DeterministicPlanner/LlmPlanner behavior byte-stable for single-node intents; regression tests stay green.  
**Warning signs:** Existing planner unit tests or e2e email paths fail.

### Pitfall 7: Hybrid tracer sold as product DONE
**What goes wrong:** Phase 48 broker/worker substrate tests claimed as LIVE multi-step CLI.  
**Why it happens:** LIVE-05 honesty class recurrence.  
**How to avoid:** Frame STREAM proofs as substrate; CLI multi-node + LIVE-07/08 later.  
**Warning signs:** README/acceptance language "safe coding agent works end-to-end" without real `caprun run` multi-node.

### Pitfall 8: Gate 3 mint drift
**What goes wrong:** New mint call site "for bag convenience".  
**Why it happens:** Want trusted intermediate without ProvideIntent.  
**How to avoid:** Consume existing outputs only; Gate 3 allowlist unchanged.  
**Warning signs:** `check-invariants.sh` Gate 3 FAIL; new `mint_from_*` under `cli/`.

## Code Examples

### Minimal sequential loop + bag (target shape)

```rust
// Target shape for cli/caprun/src/worker.rs (illustrative — not shipped).
// Grounded in DESIGN §2 + live send_framed/recv_framed + ExecutorDecision.

use std::collections::HashMap;
use runtime_core::plan_node::ValueId;
use runtime_core::ExecutorDecision;

// Seed bag from ProvideIntent + claim handles (existing worker locals).
let mut bag: HashMap<String, ValueId> = HashMap::new();
bag.insert("intent".into(), intent_value_id.clone());
// … derived_recipient, body, trusted_* …

let mut step = 0usize;
let mut submitted = 0usize;
loop {
    let ctx = PlanStreamContext { /* intent, step, bag refs, task_instruction */ };
    let Some(plan_node) = planner.plan_next(&ctx) else {
        break;
    };
    submitted += 1;

    send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;
    let (decision, output_value_id) = match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::PlanNodeDecision { decision, output_value_id } => {
            (decision, output_value_id)
        }
        other => anyhow::bail!("unexpected response to SubmitPlanNode: {other:?}"),
    };

    match decision {
        ExecutorDecision::Allowed => {
            // DESIGN F-01: store ANY Some(output_value_id), not process.exec-only.
            if let Some(id) = output_value_id {
                bag.insert(format!("out_{step}"), id);
            }
            step += 1;
        }
        ExecutorDecision::BlockedPendingConfirmation { .. } => {
            // Substrate: stop without re-submit. Product hold = Phase 50.
            eprintln!("[worker] BLOCKED: hold required — exiting 1 (no re-submit)");
            std::process::exit(1);
        }
        ExecutorDecision::Denied { .. } | ExecutorDecision::NotImplemented => {
            eprintln!("[worker] DENIED/abort remaining: {decision:?}");
            std::process::exit(1);
        }
    }
}
if submitted == 0 {
    anyhow::bail!("empty multi-node stream rejected (fail-closed)");
}
```

### Broker same-connection multi-submit proof (harness shape)

```rust
// Pattern after crates/brokerd/tests/replay_cas.rs:312-350
// Extend to DIFFERENT nodes + verify_chain (STREAM-01).

// ProvideIntent once → IntentAccepted handles
// SubmitPlanNode(node_1) → PlanNodeDecision { Allowed, output_value_id: Some(h) }
// SubmitPlanNode(node_2 with PlanArg { value_id: h }) → decision_2
// drop stream; open audit.db; assert verify_chain(&conn, &session_id, key)
// assert ≥2 plan_node_evaluated (or sink events) on SAME session_id
```

### Genuine bag taint proof (STREAM-02 negative)

```rust
// Analog: cli/caprun/tests/s9_process_exec_block.rs:236-270
// and live_acceptance_v1_7_composed.rs leg (a) — hybrid in-crate OK for substrate.

// Node1: trusted process.exec → Allowed → output_value_id (ExecRaw+ExternalUntrusted)
// Node2: process.exec command = that handle → BlockedPendingConfirmation
// Assert provenance_chain[0] == process_exited event id (anti-stapling)
// Assert verify_chain true for the session
// Frame as STREAM substrate, NOT LIVE-07 CLI DONE
```

## State of the Art (this project)

| Old Approach (through v1.9) | Current Approach (v1.10 Phase 48) | When Changed | Impact |
|-----------------------------|-----------------------------------|--------------|--------|
| Worker one-shot submit | Sequential N× SubmitPlanNode + bag | Phase 48 | Enables multi-step without new effect path |
| `output_value_id` discarded | Load-bearing bag storage | Phase 48 | STREAM-02 / mid-loop I2 expressible |
| Hybrid in-crate multi-leg LIVE | Substrate tests only; CLI multi-node later | 48 vs 51 | Honesty: substrate ≠ DONE |
| Confirm end-of-session only | Product mid-loop hold Phase 50; substrate stop-no-resubmit 48 | DESIGN pin | Avoid reconnect-remint trap |
| Single-node Planner only | Additive multi-node surface; keep `plan()` | Phase 48–49 | Email/file/LLM stay green |

**Deprecated/outdated:**
- Comment text "output_value_id only on process.exec" — drift; bag any `Some` (F-01).
- Treating LIVE-05 hybrid composition as multi-step product DONE.

## Minimal Tracer (STREAM-01/02 acceptance spine)

Smallest end-to-end proof set that satisfies the phase without coding recipe / CLI productization:

| # | Proof | Layer | Linux? | Asserts |
|---|-------|-------|--------|---------|
| T1 | N≥2 sequential `SubmitPlanNode` on one connection/session | brokerd test | Prefer Linux for real sinks; can use trusted email/file where SMTP/mailpit available | Distinct evaluations; same `session_id`; `verify_chain` true |
| T2 | Worker bag stores `Some(output_value_id)` and offers it to next plan step | worker/planner unit or integration | Unit macOS-safe if mocked; real mint Linux | Second node args carry bag handle (opaque only) |
| T3 | Genuine taint: exec output handle → sensitive arg → I2 Block | brokerd or caprun test (hybrid OK) | **Yes** (process.exec) | provenance root = process_exited; `verify_chain` true; no effect on blocked node |
| T4 | ProvideIntent second call mid-stream rejected | brokerd test | Host OK | Error response; no new UserTrusted mint |
| T5 | Deny/`policy_deny` aborts remaining (no further submits) | worker logic unit + optional integration | Host for unit | After Denied, no subsequent SubmitPlanNode |
| T6 | One-shot email/file regression | existing tests | mailpit for email | CONTROL-01 / planner unit green |
| T7 | `check-invariants.sh` Gates 1+3 | script | Host | PASS; no new mint sites |

**Not in Phase 48 tracer:** full edit→test→commit→push→PR, `caprun run` multi-node, interactive confirm-hold, LIVE-07/08 framing.

## Block-and-Hold: Phase 48 vs Phase 50

| Concern | Phase 48 (substrate) | Phase 50 (product) |
|---------|----------------------|--------------------|
| On Block | Exit/stop fail-closed; **no re-submit** | Stay same Session; signal effect_id; human confirm/deny |
| Worker connection | May exit (current behavior) | Prefer stay-connected hold (DESIGN §3) |
| Confirm path | Unchanged durable single-shot | Wired into multi-node driver |
| Dual-Session stitch | **Forbidden** | Still forbidden |
| Reconnect remint | **Forbidden** | Still forbidden |
| Session-wide waiver | **Forbidden** | Still forbidden |
| Exit codes | Non-zero on non-Allowed sufficient | Distinct success / blocked / denied taxonomy (CLI-02) |

**Pin for planner:** Phase 48 tasks must document "leave ready for Phase 50 hold" — branch on `BlockedPendingConfirmation` without consuming it as success and without re-submitting the blocked node. Do not implement full stay-connected product hold unless a plan explicitly scopes a thin internal hook without product UX.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Static sequence / `plan_next` with default one-shot adapter is the right additive API (names flexible) | Pattern 3 | Wrong name is cheap; wrong PLAN-03 break is expensive — signature discipline is NOT assumed free |
| A2 | Phase 48 can prove STREAM without CaprunIntent coding variant (test planner sufficient) | Minimal Tracer | If compile forces enum exhaustiveness elsewhere, may need a tiny runtime-core match arm — still not full CODE-01 recipe |
| A3 | Substrate stop-on-Block (exit 1) is acceptable until Phase 50 hold | Pattern 4 / Block-and-Hold table | If Phase 49 coding path needs mid-loop push before 50, order pressure — roadmap already sequences 49 then 50 |
| A4 | No new mint site required for STREAM-01/02 | Standard Stack / Don't Hand-Roll | If a proof needs a new mint, stop and DESIGN-amend Gate 3 — do not silent-add |
| A5 | Docker/Colima unavailable on research host at research time; Linux proofs still planned via mailpit/compose when CI/dev runs | Environment | Plans must keep Linux-gated tests + mailpit-verify commands even if host cannot run them now |

**If wrong:** Planner should treat A1–A3 as implementation discretion within DESIGN locks; A4 is a hard stop; A5 is environment not design.

## Open Questions

1. **Exact `plan_next` vs static-index API name**
   - What we know: DESIGN §1.3 static index sufficient; §12 allows either planning name.
   - What's unclear: Default trait method vs separate `MultiStepPlanner` trait.
   - Recommendation: Additive method on `Planner` with default one-shot adapter — least churn for DeterministicPlanner/LlmPlanner.

2. **How thin is the Phase 48 Block hook?**
   - What we know: Product hold is Phase 50; re-submit forbidden.
   - What's unclear: Whether worker should emit a machine-parseable "blocked effect_id" line for future main to parse.
   - Recommendation: Optional structured stderr/log of effect_id if already available on decision path; do not build dual-process continue protocol in 48.

3. **Tracer sink pair for bag proof**
   - What we know: process.exec → process.exec command is proven hybrid pattern (s9 / v1.7).
   - What's unclear: Whether to also cover git.commit/http.request bag insert unit-level.
   - Recommendation: Primary T3 = process.exec; unit assert bag insert does not filter by sink id (covers F-01).

4. **Should Phase 48 touch `runtime-core` at all?**
   - What we know: Coding CaprunIntent is Phase 49.
   - What's unclear: Whether `PlanStreamContext` types live in runtime-core or cli-only.
   - Recommendation: Keep stream context **cli-local** in 48 to avoid TCB type churn; promote only if Phase 49 needs shared types.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | All builds/tests | ✓ | cargo 1.89.0, rustc 1.89.0 | — |
| `scripts/check-invariants.sh` | Gate 1/3 hygiene | ✓ | PASS 2026-07-27 | — |
| Docker | mailpit-verify / compose-verify | ✗ (not running at research time) | — | Write Linux-gated tests; run when Colima/Docker up or in CI |
| Colima | Docker backend on Mac-class hosts | ✗ | — | Same as Docker |
| Mailpit image | SMTP-touching tests | via mailpit-verify when Docker up | — | Scope MAILPIT_VERIFY_CMD to non-SMTP stream tests when possible |
| Linux kernel ≥5.13 Landlock | Confinement e2e | via Docker rust:1 | — | macOS: unit tests only; cfg-linux for security legs |

**Missing dependencies with no fallback:** none for **authoring** Phase 48 code (host cargo builds). Full STREAM security claims need Linux verification when Docker is available.

**Missing dependencies with fallback:** Docker/Colima — develop + unit-test on host; defer compose/mailpit runs.

**Step 2.6 note:** External package installs: none. External services only for verification.

## Validation Architecture

> `workflow.nyquist_validation` absent in `.planning/config.json` → treat as **enabled**.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[test]` / `#[tokio::test]` via cargo (workspace) |
| Config file | per-crate `Cargo.toml`; no jest/pytest |
| Quick run command | `./scripts/check-invariants.sh && cargo test -p caprun --test planner -- --nocapture` |
| Full suite command (host) | `./scripts/check-invariants.sh && cargo test --workspace --no-fail-fast` |
| Full suite command (Linux auth) | `bash scripts/mailpit-verify.sh` (default full workspace) or scoped `MAILPIT_VERIFY_CMD=…` |
| Composed multi-sink LIVE later | `bash scripts/compose-verify.sh` (Phase 51 primarily) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| STREAM-01 | N sequential SubmitPlanNode same Session/connection | integration | `cargo test -p brokerd --test stream_multi_submit -- --nocapture` (proposed) | ❌ Wave 0 |
| STREAM-01 | Every event same DAG; `verify_chain` true | integration | same + assert `verify_chain` | ❌ Wave 0 |
| STREAM-01 | Each node independent I2 (no batch) | unit/integration | inspect path still `evaluate_plan_node_and_record` per submit; no new batch API | ❌ Wave 0 (grep Gate + code) |
| STREAM-01 | Deny aborts remaining | unit | `cargo test -p caprun stream_abort_on_deny` (proposed) | ❌ Wave 0 |
| STREAM-02 | Bag stores any `Some(output_value_id)` | unit | `cargo test -p caprun handle_bag_stores_any_output` (proposed) | ❌ Wave 0 |
| STREAM-02 | Bag is opaque ValueId only (type-level + no literal field) | unit / compile | planner/bag API tests | ❌ Wave 0 |
| STREAM-02 | Planner places bag handle in later node | unit | `cargo test -p caprun --test planner` extended | ❌ Wave 0 |
| STREAM-02 | Mid-stream ProvideIntent rejected | integration | `cargo test -p brokerd provide_intent_once` (extend existing if present) | ⚠️ partial (broker guards exist; ensure multi-submit context test) |
| STREAM-02 | Genuine taint via bagged exec output → Block | Linux integration | `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test stream_substrate taint_bag' bash scripts/mailpit-verify.sh` | ❌ Wave 0 |
| HYG / Gate | No EffectRequest; no new mint sites | script | `./scripts/check-invariants.sh` | ✅ |
| Regression | Email/file single-node green | existing | `cargo test -p caprun --test planner` + existing e2e/mailpit | ✅ |

### Sampling Rate

- **Per task commit:** `./scripts/check-invariants.sh` + focused `cargo test` for touched crate/test binary
- **Per wave merge:** `cargo test --workspace --no-fail-fast` (host) + Linux-gated stream tests via mailpit-verify when Docker available
- **Phase gate:** invariants green + STREAM T1–T7 green (Linux legs on compose/mailpit) before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `crates/brokerd/tests/stream_multi_submit.rs` (or equivalent name) — STREAM-01 multi-submit + `verify_chain`
- [ ] `cli/caprun/tests/stream_substrate.rs` (or extend planner + new module) — bag + sequential planner surface + taint-bag Linux leg
- [ ] Worker loop implementation behind tests (production path)
- [ ] Additive `Planner` multi-node surface + default one-shot adapter
- [ ] Docs-only F-01 comment drift fixes (`worker.rs`, `proto.rs`, `server.rs`) as explicit task or subtask
- [ ] Framework install: **none** — cargo already present

*(Existing infrastructure covers planner unit tests, replay multi-submit precedent, hybrid taint analogs, and invariants — Wave 0 is new stream-specific tests + implementation, not a new test harness.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A — local UDS session, not user auth product |
| V3 Session Management | yes | Broker Session id + occupancy latch; HARD-03 no client session_id on SubmitPlanNode |
| V4 Access Control | yes | ConnectionRole permits; policy pre-I2 narrowing; Landlock/seccomp boundary |
| V5 Input Validation | yes | Sink schema + slot-type + PLAN-03 handles-only; no free-form EffectRequest |
| V6 Cryptography | yes (audit) | Existing HMAC/SHA-256 audit chain; **no new crypto**; Gate 5 ring-only |

### Known Threat Patterns for multi-step plan stream

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-node taint laundering via bag | Tampering / Elevation | Opaque ValueIds; untrusted mints; per-node I2; no remint |
| ProvideIntent mid-stream remint | Elevation | Broker once-before-RequestFd guards |
| Batch authorize bypass | Tampering | Sequential SubmitPlanNode only; Gate 1 |
| Session split after Block | Tampering / Repudiation | No dual-Session product path; no re-submit blocked node |
| Instruction→value channel collapse | Tampering | `task_instruction: Option<String>` never ValueId |
| Policy mid-stream rebind / I2 override | Elevation | POLICY-02/03; policy_deny ≠ Block |
| New mint outside Gate 3 | Tampering | Gate 3 allowlist unchanged |
| Hybrid honesty overclaim | Integrity of claims | Substrate framing; LIVE DONE later |

## Sources

### Primary (HIGH confidence)

- Live code: `cli/caprun/src/worker.rs` (one-shot + discard), `cli/caprun/src/planner.rs` (Planner trait)
- Live code: `crates/brokerd/src/server.rs` (multi-request loop, ProvideIntent-once, output mints, SubmitPlanNode arm)
- Live code: `crates/brokerd/src/proto.rs` (`SubmitPlanNode` / `PlanNodeDecision`)
- Live code: `crates/runtime-core/src/plan_node.rs` (`ValueId` Hash+Eq), `executor_decision.rs`
- Live tests: `replay_cas.rs` (2× SubmitPlanNode), `s9_process_exec_block.rs`, `live_acceptance_v1_7_composed.rs` (hybrid two-node taint)
- `planning-docs/DESIGN-multi-step-plan-stream.md` (CLEARED pins)
- `planning-docs/DESIGN-GATE-RECORD-v1.10.md` (F-01/F-02/F-03, re-run triggers)
- `scripts/check-invariants.sh` Gate 1/3 PASS 2026-07-27
- `.planning/REQUIREMENTS.md` STREAM-01/02; `.planning/ROADMAP.md` Phases 48–50
- `.planning/research/{SUMMARY,ARCHITECTURE,PITFALLS,STACK}.md` (v1.10 milestone research)
- `.planning/phases/47-…/47-RESEARCH.md` + `47-PATTERNS.md` (prior gate research)

### Secondary (MEDIUM confidence)

- CLAUDE.md Linux verification policy (mailpit-verify / compose-verify) — project ops, not re-executed Docker this session

### Tertiary (LOW confidence)

- Exact future `plan_next` method name / `PlanStreamContext` field list (implementation discretion within DESIGN)

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — reuse-only; no registry packages; live workspace verified
- Architecture: **HIGH** — DESIGN CLEARED + live file:line for gap and substrate
- Pitfalls: **HIGH** — DESIGN threat model + prior v1.10 research + gate record surfaces
- Block-and-Hold split 48/50: **HIGH** (semantics) / **MEDIUM** (exact Phase 48 log/hook detail)
- Environment Docker: **LOW availability now**, plans still require Linux-gated tests

**Research date:** 2026-07-27  
**Valid until:** 2026-08-26 (30 days; re-verify `file:line` if worker/broker churn before implementation)

**No CONTEXT.md** — discuss skipped; DESIGN is locked decision authority.
