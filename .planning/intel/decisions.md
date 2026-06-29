# Decisions

Locked decisions extracted from the canonical SPEC. Every `(DECIDED)` item in
`planning-docs/PLAN.md` is a closed decision per ingest instruction — treated as
**locked** (cannot be auto-overridden downstream). Source doc is internally
reconciled; no contradictions.

---

## DEC-platform-linux-only

- **source:** planning-docs/PLAN.md (§ What We Are Building)
- **status:** locked
- **scope:** target platform, v0 security claims
- **decision:** M0/M1 target Linux (Ubuntu) only. Mac/WSL2 deferred to post-v0
  best-effort. All v0 security claims are Linux-only.

## DEC-product-boundary

- **source:** planning-docs/PLAN.md (§ What We Are Building)
- **status:** locked
- **scope:** product definition / non-goals
- **decision:** Build an Intent Runtime — user-space execution layer on stock
  Linux where agents have no ambient authority, every external effect is
  authorized against a Session, and confinement is kernel-enforced. NOT a kernel
  fork, agent framework, desktop automation platform, memory product, or
  marketplace. v0 binary is `caprun`. Repo root is a single Rust workspace;
  crates live at `AgentOS/crates/` (no separate `caprunner/` subdir).

## DEC-security-invariants

- **source:** planning-docs/PLAN.md (§ Security Model — DECIDED)
- **status:** locked
- **scope:** security invariants I0 / I1 / I2
- **decision:** I1 and I2 are both required for v0 DONE; I0 is the
  creation-time rule.
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

## DEC-layer-roles

- **source:** planning-docs/PLAN.md (§ Security Model — Layer roles)
- **status:** locked
- **scope:** component responsibilities
- **decision:** Sandbox = security boundary (namespaces, Landlock, seccomp,
  default-deny net). Broker = reference monitor / control plane, NOT the
  boundary. Executor = I2 enforcement, the security differentiator. Adapters =
  the only paths to effects (v0: fs + one mediated sink stub).

## DEC-fd-pass-policy

- **source:** planning-docs/PLAN.md (§ fd-pass policy — DECIDED)
- **status:** locked
- **scope:** fd handoff vs mediated effects, revocation
- **decision:** fd-pass (SCM_RIGHTS) is for read-only workspace I/O and test
  output (low-risk, short-lived, disposable workers). External, irreversible,
  high blast-radius effects are mediated only. Revocation = kill the worker via
  pidfd; do not describe leases as revocation.

## DEC-terminology

- **source:** planning-docs/PLAN.md (§ Terminology — DECIDED)
- **status:** locked
- **scope:** public API vocabulary
- **decision:** Public API and docs use exactly: Intent, Session, Planner,
  Worker, Broker, Adapter, Effect, Artifact, Event. `ExecutionContext` is an
  internal Rust struct backing a Session — never in public API. Planner proposes
  Effects (`RunTests`, `ApplyPatch`, …); broker/adapters use typed resources
  (`fs.path:…`) internally. Grow the effect ontology from audit DAG
  observations, not upfront speculation.

## DEC-architectural-lock-plan-nodes

- **source:** planning-docs/PLAN.md (§ Architectural Lock — DECIDED)
- **status:** locked
- **scope:** broker effect path API shape
- **decision:** Broker effect path takes plan nodes from day one.
  `submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> }) ->
  ExecutorDecision`, where each ValueNode carries literal + provenance + taint.
  Do NOT authorize raw `EffectRequest { effect, args: Map }` straight to sinks.
  Week-2 executor is a minimal stub walking this shape. The API shape is not
  optional.

## DEC-canonical-docs

- **source:** planning-docs/PLAN.md (§ Canonical Documentation — DECIDED)
- **status:** locked
- **scope:** documentation authority
- **decision:** PLAN.md is the single source of truth; on any conflict, PLAN.md
  wins. Background detail lives under `archive/` (Security →
  archive/AGENT-RUNTIME-HANDOVER.md; Architecture narrative →
  archive/multi-part/*; Red-team/open risks →
  archive/agent-execution-runtime-handover.md). Gates before executor code:
  DESIGN-taint-model.md then DESIGN-plan-executor.md.

## DEC-repo-layout

- **source:** planning-docs/PLAN.md (§ Repository Layout — DECIDED)
- **status:** locked
- **scope:** workspace structure and stack
- **decision:** Repo root = Cargo workspace. Crates: `runtime-core` (Intent,
  Session, Effect, Artifact, Event — no I/O), `brokerd` (session lifecycle,
  policy, audit DAG, adapters), `executor` (deterministic I2 interpreter, after
  DESIGN doc), `sandbox` (bubblewrap, seccomp, Landlock, cgroups), `adapters/fs`,
  `captoken` (v0 minimal; broker DB is authority on single host). `cli/caprun`.
  Stack: Rust (tokio, serde, sqlx/SQLite, nix/rustix, landlock, seccompiler,
  ed25519-dalek). Python OK for non-TCB experiments only.
