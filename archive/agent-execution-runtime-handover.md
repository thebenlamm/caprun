# Agent Execution Runtime — Handover Document
**Date**: 2026-06-29  
**Status**: Current best architecture after initial thesis → critique → reframing → red-team cycle  
**Audience**: Local Grok (terminal) continuing the design  
**Owner**: Ben Lamm (solo builder, Rust + Python, Ed25519 messaging already in use, EC2/Ubuntu primary + Mac/WSL2, severe ADHD, bias to smallest useful shippable unit)

---

## 1. Executive Summary & Conversation Goal

We are designing an **agent-first execution runtime** (not just a capability broker) for LLM agents. The goal is enforceable scoped authority, safe delegation, rich approval UX, reversibility, and audit/provenance while remaining practical to build and run on stock Linux (with Mac/WSL2 support).

**Original thesis** (pressure-tested): A single capability broker at the OS boundary on a stock kernel is the load-bearing primitive. Everything else (observability = audit, guardrails = limited caps, delegation = attenuated caps, reversibility) derives from it. Agents invert the usual ocap friction problem.

**Key corrections that stuck**:
- Enforcement requires a substrate the agent cannot bypass (Landlock + seccomp + namespaces + fd/pidfd passing). Pure userland cooperation broker is insufficient.
- The conceptual center is **Intent** + **Execution Context**, not the token or broker. The broker is demoted to the reference monitor + capability mint *inside* the runtime.
- Planner/worker split + rich approval UX are first-class, not afterthoughts.
- Effects (high-level operations with declared reversibility, cost, blast radius) > resource-centric or syscall-centric ontology.
- Linux fd/SCM_RIGHTS mint-and-pass is the primary happy path for anything that can be expressed as a kernel capability. Proxy only when unavoidable.

The design is now an **Intent-driven Execution Runtime** whose authorization component is a capability-style reference monitor that prefers native kernel objects.

---

## 2. Current Architecture (Text Diagram)

```
Intent (root primitive)
  │ description, parent, status, created_by (planner|human)
  ▼
Execution Context
  ├── derived_effects: Vec<EffectDescriptor>   (high-level, planner proposes)
  ├── approval_policy + state
  ├── budget / spend tracker
  ├── memory / scratchpad + provenance subgraph (scoped to this intent)
  ├── undo_log or snapshot refs
  ├── workers: Vec<WorkerHandle> (pidfds + narrowed sub-contexts)
  └── status
        │
        ▼ Planner (LLM or deterministic) calls Discover(intent) → proposes effects + sub-intents
        │
        ▼ Workers execute steps, emit EffectRequest {context_id, effect, justification}
        │
Broker (Reference Monitor + Capability Mint)
  ├── receives EffectRequest
  ├── checks against context.derived_effects + policy + budget + approval_state
  ├── For file/socket effects inside granted scope → open once, pass fd + metadata via SCM_RIGHTS to sandboxed worker (hot path has no further broker)
  ├── For irreversible/external effects (git push, spend, email, container ops) → stay in path or execute under context
  ├── Rich approval hook: builds context payload (intent, diff/plan, confidence, historical rate, blast radius) → delivers via existing messaging → structured replies (approve-once / always-for-class / after-condition / deny)
  ├── Audit / provenance append (always by intent_id or context_id)
  └── Drives sandbox creation for new workers
        │
Sandbox (per-worker)
  ├── Landlock (deny all FS except instance tmp + broker socket)
  ├── seccomp-bpf (whitelist only broker IPC + computation; block openat/connect/exec except passed fds)
  ├── user+mount+net namespaces
  └── pidfd for lifecycle/revocation signaling from broker
        │
OS primitives + external services (via passed fds or mediated calls)
```

**Key property**: Authority is derived from and subordinate to Intent. Revocation, undo, audit, billing, memory, and delegation are all grouped and queryable by intent_id/context_id.

---

## 3. Core Primitives (Concrete Schemas)

```rust
// Root
struct Intent {
    id: Uuid,
    description: String,           // "Fix failing tests in payment module after retry refactor"
    parent: Option<Uuid>,
    created_by: Planner | Human,
    status: Active | Completed | RolledBack | Abandoned,
    created_at: u64,
    completed_at: Option<u64>,
}

// What actually executes and holds authority
struct ExecutionContext {
    id: Uuid,
    intent_id: Uuid,
    derived_effects: Vec<EffectDescriptor>,
    approval_policy: PolicyRef,        // points to rules + UX template
    budget: BudgetTracker,
    memory: ContextMemory,             // artifacts, scratchpad, provenance subgraph for this intent only
    undo_log: Vec<Undoable>,
    workers: Vec<WorkerHandle>,        // pidfds + their sub-context ids
    status: Active | WaitingApproval | Done | RolledBack,
}

// High-level effect the planner proposes and broker authorizes
struct Effect {
    kind: CompileProject | RunTestSuite | CreatePullRequest | SendNotification | GitPush | QueryDatabase | WriteArtifact | ...,
    args: Map<String, Value>,
    reversible: bool,
    estimated_cost: CostClass,         // cpu, usd, blast_radius, human_time
    justification: String,             // from planner, required for elevation/approval
    requires_approval: ApprovalClass,  // Auto | AutoAfterCondition | HumanWithContext | Never
}

struct EffectRequest {
    context_id: Uuid,
    effect: Effect,
    justification: String,
}

// Lightweight internal handle (preferred for local workers)
struct CapabilityHandle {
    id: u64,                    // or struct with type + id
    context_id: Uuid,
    effect_kind: EffectKind,
    granted_at: u64,
    // kernel object ref if fd/socket was passed
}

// Export format only when authority must leave the runtime
struct ExportedCapability {
    // Ed25519 signed (your existing keys) or Biscuit
    // carries intent_id, context_id, constraints, exp, jti
    // Used for: delegation to remote agents, persistence across restarts, cross-machine
}
```

**Broker responsibilities (now scoped)**:
- Reference monitor for EffectRequest inside a context.
- Mint/pass kernel capabilities (fds, connected sockets, pidfds) when possible.
- Rich approval hook + structured reply handling.
- Per-intent audit, provenance, undo surface, budget enforcement.
- Sandbox lifecycle for workers.

---

## 4. Enforcement Model (Linux-first)

**Primary happy path** (file read/write inside granted tree, approved outbound connections):
1. Broker opens the resource (or connects) under context policy.
2. Passes fd (or connected socket) + any metadata via SCM_RIGHTS over the broker Unix socket to the worker.
3. Worker does direct I/O on the granted fd. No further broker involvement, no repeated policy checks, no TOCTOU on path.
4. pidfd allows broker to monitor or signal the worker for revocation/lifecycle.

**Mediated path** (irreversible or external effects):
- Worker calls high-level `perform_effect` RPC to broker.
- Broker executes (or proxies) under context policy, logs to intent audit, enforces spend/rate/circuit-breaker.

**Sandbox setup** (broker-driven):
- `unshare(CLONE_NEWUSER | CLONE_NEWNS | CLONE_NEWNET | CLONE_NEWPID)`
- Landlock: deny everything except /proc/self/*, per-instance /tmp, broker socket fd.
- seccomp-bpf: kill on forbidden syscalls; only broker IPC + computation + operations on passed fds allowed.
- Worker binary/library must be built to use passed fds or the mediated RPC; arbitrary code that tries direct open/connect is killed.

**Revocation limitations acknowledged**:
- Passed fds cannot be selectively revoked without killing the worker (Linux reality).
- Mitigation: short-lived contexts, narrow derived_effects per worker, pidfd signaling for whole-worker termination on intent abort/rollback, and preference for mediated path on any high-blast-radius effect.

**Mac/WSL2 note**: Coarser (App Sandbox or container). The design accepts platform differences; Linux gets the strong path, others get best-effort mediation + approval.

---

## 5. Red-Team Attacks (Ranked by Severity for This Design)

1. **Revocation after fd handoff is unsolved on stock Linux** (Linux capability realist)  
   Once fd is passed, worker can keep using it even after context is rolled back or intent aborted. pidfd only gives blunt kill. This re-introduces ambient authority windows exactly where we claimed to close them. Strongest attack.

2. **Prompt injection / confused deputy at planner + intent creation level** (LLM security)  
   Top-level intent description or planner context can be tainted. Planner can emit malicious sub-intent or effect request that looks legitimate inside the narrow context. Broker authorizes it because policy only checks "context may propose CreatePullRequest", not whether the intent itself was subverted. No taint tracking or human gate on context creation from untrusted data.

3. **Scope and complexity tax on solo/ADHD builder** (your constraints)  
   Dynamic sandbox + fd lifecycle + context state machine + undo + rich approval hook + effect adapters + planner loop is still a large systems project. The 2-week "minimal useful unit" remains ambitious. High risk of freeze before anything ships.

4. **Hot-path latency and persistent mediated effects** (production infra)  
   Even with fd passing, context lookup + policy + audit + budget check adds cost per effect. Irreversible/external effects stay mediated forever. LLM agent loops doing hundreds/thousands of operations will feel it. No measurements or bypass strategy yet.

5. **Reference monitor is unverified and now larger** (formal/seL4 lineage)  
   The broker + context lifecycle + approval policy engine is ordinary Rust on Linux. A bug here is a full compromise. We made the ref monitor bigger and more dynamic without adding verification. Opposite of the capability tradition invoked.

Other noted weaknesses:
- Crash consistency for open fds, partial effects, approval state, and undo log.
- Planner itself needs to run under authority (potential regress).
- Effect ontology maintenance burden (hundreds of edge cases in real work).
- Heterogeneous dev environment (Mac/WSL2) forces two security models.

---

## 6. Open Questions & Tradeoffs (for local Grok to pressure-test)

- Can we make revocation practical without killing workers every time? (leases? short-lived fds + re-request? accept blunt kill for high-risk effects?)
- How do we protect the planner/intent creation layer from injection? (taint on intent descriptions? separate low-authority planner context? human gate on any context that touches external data?)
- What is the *actual* smallest 2-week slice that survives the above red-team and is independently useful tomorrow? (e.g., one intent + one context + fd-pass for file effects + CLI approval for one irreversible effect + audit by intent_id)
- How do we keep the hot path near-zero overhead for common effects while still having full context/policy/audit?
- Should ExportedCapability use simple Ed25519 + JSON (your existing style) or adopt Biscuit for native attenuation/Datalog? When is each appropriate?
- How does the runtime handle broker restart / crash mid-intent without losing fd handles or partial work?
- Is the effects ontology (CompileProject, CreatePullRequest, …) maintainable, or will we end up with a combinatorial explosion of adapters?

---

## 7. Recommended Continuation Strategy for Local Grok Session

Feed this entire document (or the file) as context. Then use one or more of these focused prompts in sequence:

**Prompt A (Strongly recommended first)**:  
"Red-team the revocation story after fd handoff. Propose the minimal practical mitigation that does not require kernel changes or killing workers on every abort. Update the architecture and the 2-week milestone accordingly. Be concrete with Linux primitives."

**Prompt B**:  
"Design the smallest shippable unit that survives red-team attacks 1-3 and is useful on day one for a solo builder with existing Ed25519 messaging and MCP servers. Define exact scope, what is deliberately left out, success criteria, and a 2-week task breakdown. Prioritize shipping over completeness."

**Prompt C**:  
"Define the Effect ontology and adapter layer for the first 5-7 high-value effects a coding/research agent actually performs. For each, declare reversible?, typical approval class, Linux resources it needs, and whether it can use the fd-pass happy path or must stay mediated. Keep the planner-facing surface high-level."

**Prompt D**:  
"Design the rich approval hook + structured reply protocol in detail. Include the exact payload the broker sends via existing messaging, the allowed reply types, how 'always for this intent class / after tests pass' is implemented and stored, and how it integrates with the context state machine."

**Prompt E**:  
"Address prompt injection at the intent/planner layer. Propose concrete mechanisms (taint, separate planner context, human gate on context creation, justification requirements) and show how they compose with the broker's authorization. Update threat model."

**Prompt F (synthesis)**:  
"Given the red-team findings, produce a revised architecture diagram + updated primitives + revised 2-week milestone that explicitly mitigates the top 3 attacks while preserving the intent-as-root insight and Linux fd-passing preference. Call out any parts of the original thesis that are now abandoned."

---

## 8. What the Local Grok Should *Not* Do

- Do not restart from the original broker-centric thesis.
- Do not add more layers or generality "just in case".
- Do not ignore the solo-builder + ADHD constraint when sizing the next increment.
- Do not treat fd revocation as solved because "we can kill the worker".
- Do not make the planner a special trusted case without justifying the authority it holds.

---

## 9. Appendix — Useful Context from Prior Turns (condensed)

- Existing stack: multiple LLM agents, custom MCP servers, persistent agent on EC2/Ubuntu, Mac + WSL2 dev, typed agent-to-agent messaging with Ed25519 signatures, comfortable in Rust and Python.
- Strong bias: ship smallest useful unit over building a platform. Actionable, implementation-ready outputs preferred.
- Health/ADHD note (for interaction style): prefer structured, low-density output, clear next actions, avoid long dense walls when possible. Hyperfocus triggers on concrete schemas, code shapes, and tight feedback loops.

---

**End of handover.**  
This document is self-contained. Paste the file path or its full content into your local Grok session and start with Prompt A or B above. The architecture is ready for targeted pressure-testing and incremental implementation design.

If you want increments (e.g., first just the revocation mitigation + updated 2-week slice, then the effect ontology, then approval hook), say so and I will produce the next slice of this doc.