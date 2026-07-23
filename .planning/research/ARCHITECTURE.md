# Architecture Research

**Domain:** Multi-step Safe Coding Agent loop on caprun Intent Runtime
**Researched:** 2026-07-23
**Confidence:** HIGH (primary sources: live crates + locked DESIGN docs; not external ecosystem speculation)
**Milestone:** v1.10 — Multi-step Safe Coding Agent Loop on EXISTING architecture

## Standard Architecture

### System Overview (as shipped through v1.9)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  CLI / Orchestrator  (cli/caprun — unconfined host process)               │
│  · caprun run / confirm / deny / review / audit / grant                  │
│  · binds policy at session create (POLICY-03)                            │
│  · spawns broker task + confined worker (+ optional planner sidecar)     │
└───────────────┬───────────────────────────────┬──────────────────────────┘
                │ session create / policy bind  │ spawn
                ▼                               ▼
┌───────────────────────────────┐   ┌──────────────────────────────────────┐
│  Broker (brokerd)             │   │  Worker (caprun-worker)              │
│  Reference monitor — NOT      │◄──│  · connect abstract UDS              │
│  the boundary                 │   │  · self-confine (Landlock+seccomp)   │
│  · Session + audit DAG        │   │  · ProvideIntent / RequestFd         │
│  · ValueStore (mint sites)    │   │  · ReportClaims / ReportDerivedClaim │
│  · SubmitPlanNode → executor  │   │  · Planner::plan → ONE PlanNode      │
│  · ConnectionRole gate        │   │  · SubmitPlanNode once → exit        │
│  · adapters (fs/exec/git/…)   │   └──────────────────┬───────────────────┘
└───────────────┬───────────────┘                      │ handles only
                │                                      ▼
                │                         ┌────────────────────────────┐
                │                         │ Planner seam (v1.4)        │
                │                         │ · DeterministicPlanner     │
                │                         │ · LlmPlanner → sidecar UDS │
                │                         │ · NEVER mints / sees taint │
                │                         └────────────────────────────┘
                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  Executor (crates/executor) — I2 TCB                                     │
│  · per-arg taint + slot-type + schema + Draft CommitIrreversible gate    │
│  · policy pre-narrows (never overrides I2)                               │
└──────────────────────────────────────────────────────────────────────────┘
                │
                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  Sandbox (crates/sandbox) — kernel boundary                              │
│  Landlock deny-all · seccomp · no_new_privs · rlimits · default-deny net │
└──────────────────────────────────────────────────────────────────────────┘
```

**Today's worker loop (the product gap v1.10 closes):**

```
ProvideIntent → RequestFd → extract/mint claims → plan() → SubmitPlanNode ONCE → exit
```

v1.9 LIVE-05 composed edit→test→commit→push→PR **through real broker arms in-crate**, not as one CLI Session — honest hybrid disclosure (DOC-01 lineage).

### Component Responsibilities (locked roles)

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Sandbox | Security **boundary** | Landlock + seccomp-bpf + rlimits; worker self-confines post-connect |
| Broker | Reference monitor / control plane | Session lifecycle, SQLite audit DAG, UDS IPC, mint sites, adapter dispatch |
| Executor | I2 enforcement (security differentiator) | Hardcoded Rust TCB; policy cannot disable |
| Planner | Propose `PlanNode{sink, args: ValueId handles}` only | Trait in `cli/caprun/src/planner.rs`; never mints |
| Worker | Confined mint + submit agent | `cli/caprun/src/worker.rs`; only egress is broker-mediated plan nodes |
| Adapters | Only paths to effects | `adapter-fs`, broker sinks (`email`, `exec`, `git`, `http`, `github`) |
| CLI | Orchestrator + human gate surface | `caprun run/confirm/deny/review/audit` |

## Recommended Project Structure (v1.10 touch points)

```
crates/
├── runtime-core/          # MODIFY: CaprunIntent coding variant; optional PlanStream types
│   └── src/intent.rs      # NEW variant (e.g. SafeCodingWorkflow) — closed enum, no free-form
├── brokerd/               # MOSTLY UNCHANGED for multi-submit; SMALL adds for mid-loop confirm
│   ├── src/server.rs      # already accepts N× SubmitPlanNode per connection
│   ├── src/confirmation.rs# already cross-process pause/resume for one blocked effect
│   └── src/proto.rs       # optional: poll/await-confirm verb OR status signal only
├── executor/              # UNCHANGED I2 path (each node evaluated independently)
├── sandbox/               # UNCHANGED
└── adapter-fs/            # UNCHANGED
cli/caprun/src/
├── main.rs                # MODIFY: new intent kind + mid-loop confirm orchestration UX
├── worker.rs              # MODIFY: sequential plan stream + handle bag (critical)
└── planner.rs             # MODIFY: multi-node trait surface + DeterministicCodingPlanner
cli/caprun-planner/        # OUT OF SCOPE for v1.10 (LLM multi-step deferred)
planning-docs/
└── DESIGN-multi-step-plan-stream.md   # NEW design-gate (blocks all of the above)
```

### Structure Rationale

- **Keep multi-step in worker + planner seam**, not a new agent framework crate — product boundary stays Intent Runtime.
- **Do not put plan submission in the unconfined orchestrator as the primary path** — that recreates ambient authority around the confined worker.
- **Executor/broker effect path stays PlanNode-only** — no `EffectRequest`, no batch-authorize-all bypass.
- **LLM multi-step stays out of the v1.10 TCB change set** — deterministic coding planner first (manual-ops-first).

## Architectural Patterns

### Pattern 1: Sequential Plan Stream (recommended for v1.10)

**What:** One Session, one worker connection, N sequential `SubmitPlanNode` calls. Each node is independently I2-evaluated. Outputs (`output_value_id` from Allowed `process.exec` / `git.commit` / `http.request`) accumulate in a worker-side **handle bag** for later nodes.

**When to use:** Deterministic multi-step coding workflow (edit → test → commit → push → PR) where the full step list is known from a typed intent.

**Trade-offs:**
- Pros: reuses locked broker API; each node gets full I2/policy/audit; one audit DAG; no new effect path
- Cons: mid-loop `BlockedPendingConfirmation` (esp. always-confirm `git.push`) needs an explicit continuity design (see Pattern 3)

**Shape (opinionated):**

```rust
// DESIGN-gate target — not shipped yet
pub trait Planner {
    /// Existing single-node seam (keep for email/file + LlmPlanner compatibility).
    fn plan(/* … */) -> PlanNode;

    /// v1.10 additive: emit the NEXT node given opaque handles only.
    /// Returns None when the stream is complete.
    /// MUST NOT accept ValueRecord, raw untrusted bytes, or taint labels (PLAN-03).
    fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode>;
}

/// Worker-owned, never broker-trusted as authority:
/// only opaque ValueIds + typed intent kind + prior decision booleans.
struct PlanStreamContext {
    intent: CaprunIntent,                 // typed, user-supplied
    trusted_handles: Vec<(Slot, ValueId)>,// from ProvideIntent
    derived_handles: Vec<(Slot, ValueId)>,// from ReportClaims / derivation / outputs
    step_index: usize,
    last_decision_allowed: bool,          // not a taint oracle for planner-role
}
```

**Static stream is enough for v1.10:** `DeterministicCodingPlanner` can implement `plan_next` as an index into a fixed `Vec` of sink templates filled from the handle bag. Reactive LLM tool-use loops are explicit future work.

### Pattern 2: Handle Bag + Output Threading (already half-wired)

**What:** On Allowed `process.exec` / related minting sinks, broker returns `PlanNodeDecision { output_value_id: Some(handle) }`. Worker today binds and **discards** it (`let _ = &output_value_id`). Multi-step makes that handle load-bearing.

**When to use:** Any later plan node that should consume prior sink output (e.g. tainted exec stdout → attempted PR body for the mid-loop I2 Block proof).

**Trade-offs:**
- Pros: genuine taint chain (mint_from_exec rooted on process_exited) — §9 standard
- Cons: planner must be offered handles by **call-site convention**, never by reading provenance (finding #7)

**Invariant:** Planner places handles; executor decides trust. Never let the planner "upgrade" an output handle.

### Pattern 3: Mid-Loop Confirm Continuity (design-gate critical)

**What:** `git.push` is always rewritten to `BlockedPendingConfirmation` even when I2 would Allow. Confirm is a **second process** with a durable `PendingConfirmation` resolved snapshot (`DESIGN-confirmation-release.md`). Worker process + in-memory `ValueStore` do not survive exit. Occupancy latch is one-way for the broker process lifetime.

**When to use:** Any multi-node stream that includes CommitIrreversible always-confirm sinks mid-sequence.

**Recommended v1.10 approach (opinionated): Block-and-Hold in one `caprun run` process**

```
Worker loop                     Main (orchestrator)              Confirm surface
    |                                |                                |
    |-- SubmitPlanNode (push) ------>|                                |
    |<-- BlockedPendingConfirmation -|                                |
    |-- signal effect_id ----------->|                                |
    |   (stay connected; wait)       |-- print review/confirm cmds -->|
    |                                |   OR interactive confirm       |
    |                                |<-- operator confirms ----------|
    |                                |-- confirmation::confirm() ---->|
    |                                |   (same audit.db; sink runs)   |
    |<-- proceed-next (pipe/UDS) ----|                                |
    |-- SubmitPlanNode (github.pr)-> |                                |
```

**Why this over multi-process `caprun continue`:**
- ValueStore + session occupancy stay coherent inside one broker lifetime
- Avoids re-mint / reconnect laundering risks against HARD-03 + Phase-19 latch
- Confirm path already executes the blocked sink from the durable snapshot — worker must **not** re-submit the blocked node

**Rejected alternatives:**
| Alternative | Why reject |
|-------------|------------|
| Orchestrator submits PlanNodes itself (no worker) | Unconfined ambient path; weakens "kernel-confined worker is only egress" story |
| Exit worker on Block; new process reconnects to same session | Occupancy latch + empty ValueStore; ProvideIntent re-mint is a laundering tripwire |
| Auto-confirm mid-loop | Defeats human gate; violates confirm single-shot product model |
| Batch `SubmitPlanDAG` authorize-all | New bypass surface; violates per-node I2 standing |

### Pattern 4: Instruction Channel ≠ Value Channel (preserve under multi-step)

**What (shipped v1.4 GATE-01):** Injection text reaches the planner only as `task_instruction: Option<String>` — **never** a `ValueId`, never bindable as a sink arg. Values reach sinks only as opaque handles minted at worker/broker mint sites.

**When to use:** Any multi-step extension of the Planner trait.

**Trade-offs:** Instruction can influence *which* offered handle is chosen; it cannot manufacture a trusted handle. Multi-step must not collapse these channels (e.g. do not mint `task_instruction` into a `UserTrusted` body handle "for convenience").

### Pattern 5: Policy Bound Once, Evaluated Per Node

**What:** `bind_policy` at session creation; immutable; `policy_bound` audit event; each `SubmitPlanNode` runs policy pre-gate then I2.

**When to use:** Entire multi-node Session.

**Invariant:** Policy never overrides I2 (POLICY-02). Multi-step must not re-bind policy mid-stream from worker-reachable paths.

## Data Flow

### Multi-node Session Flow (target v1.10)

```
Operator
  │  caprun run --policy P safe-coding-workflow <params> <workspace> [audit.db]
  ▼
Main: create Session (seed provenance) → bind_policy → policy_bound event
  → start broker (session_status, trusted_inode, occupancy latch)
  → spawn worker (INTENT, BROKER_SOCK, PRIMARY_SEED_FILE_DERIVED, …)
  ▼
Worker (confined):
  ProvideIntent → mint_from_intent handles (UserTrusted)     [mint site]
  [optional] RequestFd seed file → HARDEN-01 inode check
  [optional] ReportClaims / derivation → untrusted handles   [mint site]
  loop:
      plan_next(handle_bag) → PlanNode{sink, args: ValueIds}  [no mint]
      SubmitPlanNode
         ├─ policy_deny → fail closed (distinct outcome)
         ├─ Denied (schema/slot/Draft class) → fail closed
         ├─ BlockedPendingConfirmation → hold; human confirm; do NOT re-submit
         └─ Allowed → dispatch adapter; maybe output_value_id → handle_bag
  end loop → exit 0
  ▼
Main: list pending (if any) · caprun audit · verify_chain true for THIS session
```

### Key Data Flows

1. **Trusted operator params:** CLI → `CaprunIntent` → `ProvideIntent` → `mint_from_intent` → `ValueId` → planner routing → sink arg. Stays UserTrusted only if not file-seed-laundered (M7).
2. **Untrusted observation:** `RequestFd` / `mint_from_read` / `mint_from_exec` / `mint_from_http` → untrusted `ValueId` → if routed to sensitive arg → I2 Block (genuine chain).
3. **Instruction injection (deferred LLM multi-step):** raw fragment kept worker-side as `task_instruction` string; audit-minted for provenance only; never a PlanArg.
4. **Confirm release:** Block → durable `PendingConfirmation` snapshot → separate confirm process OR main in-process confirm → sink re-invoke from snapshot → audit edge; stream continues only after terminal confirm state.

### State Management

```
Session (durable DB)
  status: Active | Draft | …     # monotonic demotion Active→Draft only
  policy: immutable binding
  audit DAG: hash-chained (+ MAC)

Connection (in-memory, per broker life)
  occupancy latch (one worker)
  optional planner-role connection (SubmitPlanNode only, reduced decision)
  ValueStore (handles die with process)
  intent_provided / fd_requested guards

Worker (confined process)
  handle bag (ValueIds only)
  plan stream cursor (step_index)
```

## Integration Points

### New vs Modified Components

| Piece | New / Modified | Role in multi-step | Trust notes |
|-------|----------------|--------------------|-------------|
| `DESIGN-multi-step-plan-stream.md` | **NEW** (gate) | Locks stream shape, confirm continuity, I1 interaction | Blocks all TCB-touching code |
| `CaprunIntent::SafeCodingWorkflow` (name TBD) | **NEW** variant | Typed multi-step intent; closed enum | Literals only via ProvideIntent mint |
| `Planner::plan_next` / stream API | **MODIFY** trait | Multi-node emission | PLAN-03: handles only; no mint |
| `DeterministicCodingPlanner` | **NEW** impl | Hardcoded edit→test→commit→push→PR | No LLM; scripted sinks |
| `worker.rs` loop | **MODIFY** | N× SubmitPlanNode + handle bag | Stay fail-closed on non-Allowed (except hold-for-confirm) |
| `main.rs` intent + confirm UX | **MODIFY** | Drive multi-node CLI; mid-loop confirm orchestration | Policy bind still outside worker |
| `output_value_id` consumption | **MODIFY** (wire already exists) | Thread exec/http outputs | Taint stays on mint, not planner |
| Broker `SubmitPlanNode` arm | **Unchanged** core | Already multi-call safe | Do not add batch authorize |
| Executor I2 | **Unchanged** | Per-node evaluation | No stream-wide waiver |
| `LlmPlanner` multi-step | **OUT** of v1.10 | Future | Keep single-shot adversarial seam |
| New sinks / Cedar / web UI | **OUT** | — | — |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Worker ↔ Broker | Framed JSON UDS + SCM_RIGHTS | Worker mints; submits PlanNodes; never ambient fs/net |
| Worker ↔ Planner (in-process) | Trait call | Opaque handles + optional task_instruction only |
| Worker ↔ Planner sidecar | Abstract UDS (optional) | LlmPlanner only; v1.10 deterministic path unused |
| Main ↔ Confirm | Direct `confirmation::confirm` and/or CLI | Durable snapshot; not ValueStore |
| Planner ↔ Mint sites | **Forbidden** | ConnectionRole::Planner denies all mint verbs |
| Policy file ↔ Worker | **Forbidden** | Bound at session create from trusted path outside workspace |

### External Services (unchanged sinks, multi-node consumers)

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Local SMTP / Mailpit | Broker-mediated `email.send` | Not required for coding workflow |
| Git smart-HTTP remote | Broker-performed `git.push` | Always confirm-gated |
| GitHub API | Broker-held token + `github.pr` | Auth-grant distinct from confirm |
| HTTP egress | `http.request` / `http.request.write` | SSRF pin; write is CommitIrreversible |
| process.exec child | `caprun-exec-launcher` | Output mint_from_exec → untrusted |

## Critical Constraint: I1 Draft-Only vs Coding Sinks

**Fact (HIGH confidence, code-verified):**

| Sink | Effect class | Draft session |
|------|--------------|---------------|
| `git.commit` | MutateReversible | Allowed (class gate) |
| `http.request` GET | Observe | Allowed |
| `file.write` | **CommitIrreversible** | **Denied** (Step 0.5) |
| `process.exec` | **CommitIrreversible** | **Denied** |
| `git.push` | **CommitIrreversible** | **Denied** |
| `github.pr` | **CommitIrreversible** | **Denied** |
| `http.request.write` | **CommitIrreversible** | **Denied** |

HARDEN-01: `RequestFd` demotes to Draft unless the granted fd's inode matches the single CLI-designated trusted workspace file. Multi-file repo reads therefore demote the Session — after which the coding loop's irreversible sinks cannot run.

v1.9 LIVE-05 avoided this by **separate Active sessions** and in-crate `mint_from_intent` legs (no multi-file RequestFd demotion on the success chain).

### Opinionated v1.10 resolution

**Success path = trusted-intent-driven plan nodes only** (operator-typed paths, commands, commit message, remote/refspec, PR title/body via `ProvideIntent`). Do **not** require multi-file untrusted `RequestFd` before CommitIrreversible steps in the MVP coding stream.

**Mid-loop I2 Block path = route a genuinely tainted handle** (e.g. `mint_from_exec` output, or a deliberately reported claim) into a content/routing-sensitive arg (`github.pr` body/title, `git.push` refspec, etc.) without inventing a session-wide I1 waiver.

**Do not** "fix" multi-step by:
- forcing Session back to Active after demotion,
- auto-confirming Draft CommitIrreversible,
- or classing push/PR as Observe.

If product later needs "read many untrusted files then still push," that is a **separate design-gate** (trusted workspace snapshot / endorse / split sessions) — not a silent v1.10 assumption.

## Anti-Patterns

### Anti-Pattern 1: Raw `EffectRequest` or batch-authorize DAG

**What people do:** Submit `{effect, args: Map}` or "authorize whole plan then run."
**Why it's wrong:** Nowhere for I2 to stand per sensitive arg; forbidden by `DEC-architectural-lock-plan-nodes` and `check-invariants.sh` Gate 1.
**Do this instead:** One `PlanNode` per effect; N sequential `SubmitPlanNode`s.

### Anti-Pattern 2: Planner-minted or planner-held literals

**What people do:** Let multi-step planner invent path/URL/refspec strings and treat them as trusted.
**Why it's wrong:** Breaks `UserTrusted == human-typed`; reopens T1 ProvideIntent hole class.
**Do this instead:** Planner only routes pre-minted `ValueId`s; new literals only via ProvideIntent / mint_from_read / mint_from_exec / etc.

### Anti-Pattern 3: Collapsing instruction and value channels

**What people do:** Put injection text into a bindable handle "so the model can use it."
**Why it's wrong:** Turns instruction injection into value injection with a trusted-looking handle.
**Do this instead:** Keep `task_instruction: Option<String>` non-handle; values only as offered handles.

### Anti-Pattern 4: Hybrid composition claimed as CLI multi-node

**What people do:** Keep LIVE-05-style in-crate chains and market as `caprun run` coding agent.
**Why it's wrong:** DOC-01 honesty failure; v1.10 EXISTS to close this gap.
**Do this instead:** One Session, CLI-driven stream, `verify_chain` true; hybrid only as interim test scaffold, never DONE claim.

### Anti-Pattern 5: Reconnect-and-remint resume after Block

**What people do:** Exit worker on push Block; new process ProvideIntent again into same session.
**Why it's wrong:** Occupancy latch, ValueStore loss, and ProvideIntent as laundering surface.
**Do this instead:** Block-and-Hold within one broker lifetime (Pattern 3), confirm from durable snapshot, continue stream.

### Anti-Pattern 6: Mid-stream policy rebind from workspace

**What people do:** Agent writes a "policy" file and reloads it.
**Why it's wrong:** Breaks F1 containment / POLICY-03 trusted source.
**Do this instead:** Policy immutable for Session; bound before worker starts.

### Anti-Pattern 7: Treating multi-step as an agent framework

**What people do:** Memory stores, tool registries, free-form goals, plugin marketplace.
**Why it's wrong:** Violates product boundary (Intent Runtime, not agent framework).
**Do this instead:** Closed `CaprunIntent` variant + deterministic plan stream over shipped sinks.

## Suggested Build Order

Order is dependency-driven; design gate is hard-blocking.

```
1. DESIGN GATE (no TCB code)
   planning-docs/DESIGN-multi-step-plan-stream.md
   + fresh non-self adversarial code-trace (Fable-5 / project precedent)
   Must lock: stream shape, handle bag, mid-loop confirm, I1/Draft interaction,
   instruction vs value channels, no EffectRequest, fail-closed rules.

2. runtime-core types
   CaprunIntent coding variant (+ serde); optional PlanStreamContext types
   (pure; check-invariants Gate 2 still holds)

3. Planner seam extension (cli/caprun planner.rs)
   Additive plan_next / stream API
   DeterministicCodingPlanner: fixed edit→test→commit→push→PR templates
   Keep DeterministicPlanner + LlmPlanner single-node paths green

4. Worker sequential loop + handle bag
   Consume output_value_id
   Fail-closed on Denied/policy_deny
   Block-and-Hold hook for BlockedPendingConfirmation (no re-submit)

5. Main / CLI orchestration
   caprun run <coding-intent> …
   Surface effect_id + review/confirm on mid-loop Block
   In-process or documented dual-terminal confirm; then continue stream
   Policy still bound once at session create

6. LIVE non-hybrid proof (Linux / compose-verify or mailpit-verify as appropriate)
   a) Success: full chain one Session, verify_chain true, real sinks
   b) Mid-loop I2 Block: tainted handle into sensitive arg → Block, no effect
   c) No claim that hybrid in-crate composition is the DONE path

7. Minimal packaging / install docs
   Single binary + env/credentials for design partner (non-TCB polish)
```

**Do not start 3–6 before 1 clears.** Same discipline as v1.2 P8, v1.4 P18, v1.5 P23, v1.6–v1.9 design gates.

**Parallelization:** packaging docs (7) may draft in parallel after gate; must not gate security claims.

## Where Design-Gate Adversarial Review MUST Land

| Gate artifact | Before any code in | Must decide |
|---------------|-------------------|-------------|
| `DESIGN-multi-step-plan-stream.md` | `cli/caprun/src/worker.rs` multi-submit loop | Stream = sequential PlanNodes in one connection (not batch DAG authorize) |
| same | `cli/caprun/src/planner.rs` trait change | Additive stream API; PLAN-03 preserved; LLM multi-step out |
| same | `runtime-core` new intent variant | Closed enum fields; which literals are UserTrusted |
| same | mid-loop confirm protocol in main/broker | Block-and-Hold vs continue-session; **no** Active re-seed; no auto-confirm |
| same | handle bag / output_value_id routing | Call-site convention; which sinks mint outputs; attack demo path |
| same | I1 × multi-file read | v1.10 success path = trusted-intent-only; no Draft waiver |
| same | audit/session continuity | One Session id; policy_bound once; verify_chain scope |
| Fresh adversarial code-trace of DESIGN | Implementation phases | Catch reconnect-remint, instruction/value collapse, policy override, EffectRequest creep |
| Fresh adversarial code-trace of final multi-step diff | LIVE DONE claim | Same as every TCB milestone (v1.9 caught defects at every TCB phase) |

**Explicit non-goals for this gate:** Cedar, new sink families, pack-cap lift, cross-host, gVisor, LLM multi-step tool-use, web UI.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Design-partner demo (1 session, ~5–10 nodes) | Sequential stream in one worker; SQLite audit; Block-and-Hold confirm — **this is v1.10** |
| Habitual local use (many sessions/day) | Same architecture; audit.db growth + confirm UX polish; still single-host |
| Multi-worker / parallel steps | Out of scope (PLAN.md post-v0); would need occupancy + ValueStore model redesign |
| Cross-host agents | Out of scope (Biscuit/federation v3) |

### Scaling Priorities (only if pulled later)

1. **First bottleneck:** Human confirm latency mid-loop (product), not throughput.
2. **Second bottleneck:** Durable resume across process crash without remint laundering (harder than Block-and-Hold).

## Trust-Boundary Implications (summary)

| Invariant | Multi-step rule |
|-----------|-----------------|
| **I0** | Session seed provenance still sets initial Draft/Active; coding intent from `--seed-from-file` stays tainted (M7); no mid-stream "promote to Active" |
| **I1** | Demotion monotonic; instruction text never grants authority; multi-file untrusted read demotes — MVP avoids needing CommitIrreversible after demotion |
| **I2** | Every node independently evaluated; no stream-level taint waiver; confirm is single-shot per effect snapshot |
| **PLAN-03** | Planner never mints, never sees ValueRecord/taint |
| **PLANNER-02** | Planner-role connection still mint-deny; reduced decision signal |
| **Policy** | Pre-I2 narrowing only; bound once outside worker |
| **Effect path** | PlanNode only; never EffectRequest |

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Existing one-shot Planner/Worker/Broker shape | HIGH | Direct from `worker.rs`, `planner.rs`, `server.rs` |
| Broker already allows N× SubmitPlanNode | HIGH | No once-only seal on SubmitPlanNode; evaluate path is per call |
| `output_value_id` exists but unused in worker | HIGH | worker.rs binds then `_` |
| Mid-loop confirm is the hard integration | HIGH | DESIGN-confirmation-release + always-confirm git.push + occupancy latch |
| I1 Draft vs CommitIrreversible coding sinks | HIGH | sink_sensitivity + executor Step 0.5 + HARDEN-01 |
| Exact Block-and-Hold IPC shape | MEDIUM | Recommended; final wire protocol is design-gate output |
| Naming of coding CaprunIntent variant | MEDIUM | Product naming only; structure is closed enum |

## Gaps to Address in Design Gate (not resolved here)

1. Exact worker↔main signal for "blocked, waiting" (stdout convention vs side UDS vs broker verb).
2. Whether interactive confirm may run **in-process** in `caprun run` (design partner UX) while keeping `caprun confirm` for non-interactive/tests.
3. Whether `file.write`/`process.exec` CommitIrreversible class should be re-litigated for coding ergonomics (default: **no** in v1.10 — too easy to weaken I1).
4. How many workspace files the success path may `RequestFd` without demotion (default: seed file only / none).
5. Residual plan after deny mid-loop (abort session vs skip node) — recommend **abort fail-closed**.

## Sources

- `planning-docs/PLAN.md` — architectural lock (plan nodes, I0/I1/I2, layer roles)
- `planning-docs/DESIGN-plan-executor.md` — PlanDAG as linear `Vec<PlanNode>`; per-node I2
- `planning-docs/DESIGN-session-trust-coherence.md` — occupancy latch; planner capability split; reduced decision oracle
- `planning-docs/DESIGN-confirmation-release.md` — cross-process confirm; PendingConfirmation snapshot
- `planning-docs/DESIGN-security-hardening.md` — HARDEN-01 demote-at-RequestFd (inode identity)
- `planning-docs/CANDIDATE-v1.7plus-productization-sketch.md` — Safe Coding Agent anchor; multi-step deferred then
- `.planning/PROJECT.md` — v1.10 goals; LIVE-05 hybrid honesty
- `cli/caprun/src/worker.rs` — one-shot loop; output_value_id unused
- `cli/caprun/src/planner.rs` — Planner trait; PLAN-03; LlmPlanner sidecar
- `crates/brokerd/src/server.rs` — SubmitPlanNode, ConnectionRole, output_value_id mint sites
- `crates/executor/src/sink_sensitivity.rs` — effect classes for coding sinks
- `crates/runtime-core/src/intent.rs` — only SendEmailSummary / CreateFileFromReport today
- `cli/caprun/tests/live_acceptance_v1_9_composed.rs` — LIVE-05 hybrid framing (what v1.10 must not repeat as DONE)

---
*Architecture research for: caprun v1.10 multi-step Safe Coding Agent loop*
*Researched: 2026-07-23*
*Mode: ecosystem/architecture integration (existing stack)*
