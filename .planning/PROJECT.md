# AgentOS

## What This Is

AgentOS is an **Intent Runtime** on stock Linux: a user-space execution layer
where agents have no ambient authority, every external effect is authorized
against a Session, and confinement is kernel-enforced. The v0 binary is
`caprun`. It is **not** a kernel fork, an agent framework, a desktop-automation
platform, a memory product, or a marketplace.

## Core Value

A kernel-confined worker can only cause external effects through
broker-mediated plan nodes, and a genuinely-propagated taint chain (raw read
Event → ValueNode → sensitive sink argument) deterministically blocks
value-injection at the sink. If everything else fails, **I2 enforcement on a
genuine taint chain must hold.**

## Requirements

### Validated

Shipped in **v1.0 — AgentOS v0** (2026-06-30). Full traceability archived in
`.planning/milestones/v1.0-REQUIREMENTS.md`.

- ✓ Substrate (M0): runtime-core, sandbox, brokerd, fs adapter, substrate demo,
  locked plan-node API — v1.0
- ✓ Design gate (M0-design): DESIGN-taint-model.md, DESIGN-plan-executor.md
  (hard gate before any executor code) — v1.0
- ✓ Security demo (M1 = v0 DONE): quarantined reader, deterministic executor,
  mediated sink stub, approval hook, §9 value-injection acceptance test — v1.0
- ✓ **v0 DONE gate cleared:** the §9 test passes on a kernel-confined worker
  with a genuine, audited taint chain (`mint_from_read` is the sole broker
  taint-mint site; stapled taint fails the test). `cargo test --workspace` = 51 green.

### Active

(None yet — next milestone unscoped. Run `/gsd-new-milestone` to define v1.1.)

Candidate directions pulled from the v0 Out-of-Scope list: real sinks beyond the
email.send stub, an LLM (or richer stub) planner feeding PlanNodes, Git/GitHub
adapters, and broadening the sink sensitivity map beyond the hardcoded v0 map.

### Out of Scope

Do not build any of these until §9 holds (non-goals for v0):

- Git / GitHub adapters — defer to v1 (post-§9)
- Cedar policy engine — simple TOML/rules for sink access is fine; I2 stays in
  Rust
- Cross-host delegation / Biscuit crypto — v3 concern
- gVisor / Firecracker — bubblewrap + seccomp + Landlock is the v0 boundary
- LLM planner — a hard-coded / stub planner is sufficient for v0
- Rich approval-policy learning, undo snapshots, broad effect taxonomy
- Web UI, marketplace, long-term memory, browser control, natural-language
  policy authoring
- Mac / WSL2 support — deferred post-v0 best-effort; all v0 security claims are
  Linux-only

## Context

- **Current state (v1.0 shipped 2026-06-30):** v0 is DONE. 4 phases, 15 plans,
  ~4,600 Rust LOC across `runtime-core`, `sandbox`, `brokerd`, `executor`,
  `adapter-fs`, and the `caprun` binary. `cargo test --workspace` = 51 green;
  the §9 value-injection acceptance test passes with a genuine, audited taint
  chain. Security claims are Linux-only (verified via Colima+Docker from Mac).
- **Next milestone:** unscoped — start with `/gsd-new-milestone`. Likely v1.1
  candidates: real sinks beyond the email.send stub, an LLM/richer planner,
  Git/GitHub adapters, broader sink sensitivity map.
- **Source of truth:** `planning-docs/PLAN.md` ("AgentOS v0 — Definitive Plan").
  On any conflict, PLAN.md wins. Background detail lives under `archive/`
  (security: `archive/AGENT-RUNTIME-HANDOVER.md`; architecture narrative:
  `archive/multi-part/*`; red-team / open risks:
  `archive/agent-execution-runtime-handover.md`).
- **Thesis:** Humans execute programs; agents execute intents. Object-capability
  scoping is natural for machines. The runtime gives agents no ambient
  authority; every external effect is authorized against a Session; confinement
  is kernel-enforced.
- **Convergence:** Plan agreed by AoS-claude, AoS-codex, AoS-grok (2026-06-29),
  `#aos-session0` convergence. Debate closed on all `(DECIDED)` items.
- **Residual risks (acknowledged, not solved in v0):** an fd cannot be
  selectively revoked after SCM_RIGHTS handoff (mitigated by disposable workers
  + mediated high-risk effects); planner/intent-creation injection (mitigated by
  the I0 draft-only rule); steganographic encoding in extract values (accepted,
  documented in the threat model); broker bugs = full compromise (mitigated by
  keeping the broker small).
- **Post-v0 roadmap:** v1 — Git/GitHub/test adapters, patch/PR, workspace
  snapshots, rich approval. v2 — multi-worker decomposition, parallel execution.
  v3 — cross-machine Sessions, Ed25519 export, broker federation. v4 — general
  adapters (email, cloud, MCP ecosystem).

## Constraints

- **Platform**: Linux (Ubuntu) only for M0/M1 — all v0 security claims are
  Linux-only (`CON-platform-linux-only`).
- **Stack / TCB**: Rust (tokio, serde, sqlx/SQLite, nix/rustix, landlock,
  seccompiler, ed25519-dalek). Python permitted for non-TCB experiments only —
  never in the trusted computing base. I2 enforcement is a deterministic,
  non-LLM plan executor hardcoded in the Rust TCB (`CON-stack-tcb`).
- **Broker API shape**: the broker effect path takes plan nodes from day one —
  `submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> }) ->
  ExecutorDecision`. Raw `EffectRequest { effect, args: Map }` straight to sinks
  is forbidden (`CON-broker-api-shape`).
- **Effect classes (v0)**: Observe / MutateReversible / CommitIrreversible at
  the planner surface; grow the ontology from audit-DAG observations, not
  upfront speculation (`CON-effect-classes`).
- **Repo layout**: single Cargo workspace at repo root; crates at
  `crates/` (`CON-repo-layout`, `DEC-repo-layout`).
- **I2 non-bypassable**: policy files may gate which sinks are callable but
  cannot disable I2; sink sensitivity map is hardcoded in v0
  (`CON-i2-non-bypassable`).
- **§9 taint genuineness**: if taint is stapled on at the sink instead of
  propagated through the DAG, the demo proves nothing and fails
  (`CON-s9-taint-genuineness`).

## Locked Decisions

All decisions below are **locked** — synthesized from the canonical SPEC's
`(DECIDED)` sections. They cannot be auto-overridden downstream; changing one
requires explicit re-opening with the user.

<decisions>

### DEC-platform-linux-only — LOCKED
M0/M1 target Linux (Ubuntu) only. Mac/WSL2 deferred to post-v0 best-effort. All
v0 security claims are Linux-only.

### DEC-product-boundary — LOCKED
Build an Intent Runtime: a user-space execution layer on stock Linux where
agents have no ambient authority, every external effect is authorized against a
Session, and confinement is kernel-enforced. NOT a kernel fork, agent framework,
desktop-automation platform, memory product, or marketplace. v0 binary is
`caprun`. Repo root is a single Rust workspace; crates live at `crates/` (no
separate `caprunner/` subdir).

### DEC-security-invariants (I0 / I1 / I2) — LOCKED
I1 and I2 are both required for v0 DONE; I0 is the creation-time rule.
- **I1 (instruction injection):** No LLM context may simultaneously hold
  untrusted content and authority to cause irreversible/external effects.
  Default = dynamic taint (reading raw untrusted bytes taints the context →
  draft-only thereafter). High-risk (Tier 3+) = hard planner/worker split.
- **I2 (value injection):** No attacker-tainted value may occupy a sensitive
  argument of an irreversible/external sink without literal-value human
  confirmation (or exact standing policy match). Enforced by a deterministic,
  non-LLM plan executor hardcoded in the Rust TCB. Policy files may gate which
  sinks are callable; they cannot disable I2.
- **I0 (intent/session-creation injection):** A Session whose intent text or
  seed derives from external/untrusted content starts draft-only and cannot
  auto-authorize Tier 3+ effects. Human gate required on context creation from
  tainted data.

### DEC-layer-roles — LOCKED
Sandbox = security boundary (namespaces, Landlock, seccomp, default-deny net).
Broker = reference monitor / control plane, NOT the boundary. Executor = I2
enforcement, the security differentiator. Adapters = the only paths to effects
(v0: fs + one mediated sink stub).

### DEC-fd-pass-policy — LOCKED
fd-pass (SCM_RIGHTS) is for read-only workspace I/O and test output (low-risk,
short-lived, disposable workers). External, irreversible, high blast-radius
effects are mediated only. Revocation = kill the worker via pidfd; leases are
not revocation.

### DEC-terminology — LOCKED
Public API and docs use exactly: Intent, Session, Planner, Worker, Broker,
Adapter, Effect, Artifact, Event. `ExecutionContext` is an internal Rust struct
backing a Session — never in the public API. Planner proposes Effects
(`RunTests`, `ApplyPatch`, …); broker/adapters use typed resources
(`fs.path:…`) internally. Grow the effect ontology from audit-DAG observations.

### DEC-architectural-lock-plan-nodes — LOCKED
Broker effect path takes plan nodes from day one:
`submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> }) ->
ExecutorDecision`, where each ValueNode carries literal + provenance + taint. Do
NOT authorize raw `EffectRequest { effect, args: Map }` straight to sinks. The
Week-2 executor is a minimal stub walking this shape. The API shape is not
optional.

### DEC-canonical-docs — LOCKED
PLAN.md is the single source of truth; on any conflict, PLAN.md wins. Background
detail lives under `archive/` (Security → `archive/AGENT-RUNTIME-HANDOVER.md`;
Architecture narrative → `archive/multi-part/*`; Red-team / open risks →
`archive/agent-execution-runtime-handover.md`). Gates before executor code:
DESIGN-taint-model.md then DESIGN-plan-executor.md.

### DEC-repo-layout — LOCKED
Repo root = single Cargo workspace. Crates: `runtime-core` (Intent, Session,
Effect, Artifact, Event — no I/O), `brokerd` (session lifecycle, policy, audit
DAG, adapters), `executor` (deterministic I2 interpreter, after DESIGN doc),
`sandbox` (bubblewrap, seccomp, Landlock, cgroups), `adapters/fs`, `captoken`
(v0 minimal; broker DB is authority on single host), `cli/caprun`. Stack: Rust
(tokio, serde, sqlx/SQLite, nix/rustix, landlock, seccompiler, ed25519-dalek).
Python OK for non-TCB experiments only.

</decisions>

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Linux-only for v0 (DEC-platform-linux-only) | All security claims rest on Linux kernel primitives (namespaces, Landlock, seccomp, pidfd) | — Locked |
| Intent Runtime, not a framework/platform (DEC-product-boundary) | Keep the product boundary tight; the differentiator is kernel-enforced confinement + I2, not breadth | — Locked |
| Plan-node API from day one (DEC-architectural-lock-plan-nodes) | Raw EffectRequest→sink bakes in a path where tainted values reach sensitive args with nowhere for the executor to stand | — Locked |
| I2 in deterministic Rust TCB, never LLM (DEC-security-invariants) | LLM cannot be trusted to enforce a security invariant; enforcement must be deterministic | — Locked |
| DESIGN docs gate executor code (DEC-canonical-docs) | Writing crates/executor before the taint/executor model is reviewed risks a wrong-shape enforcer | — Locked |
| §9 with genuine taint = the only v0-DONE gate | Substrate proves mediation but not value-injection defense; stapled taint proves nothing | — Locked |

---
*Last updated: 2026-06-30 after v1.0 milestone — §9 acceptance test passes with a genuine, audited taint chain. **v0 is DONE.***
