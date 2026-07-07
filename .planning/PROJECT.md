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

## Current Milestone: v1.2 — Tainted Session, Human Gate

**Goal:** A session that touches untrusted content is mechanically demoted to
draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked sink arg
can be released only by literal-value human confirmation — all deterministic,
all in the audit DAG.

**Target features:**
- **Session taint state (I1 dynamic default):** broker tracks per-session trust
  state; the `mint_from_read` path (raw untrusted read Event) flips the session
  to draft-only. Draft-only sessions: `CommitIrreversible`-class plan nodes are
  Denied (new `DenyReason` variant, decided **in the executor** — one TCB deny
  function, one DenyReason taxonomy); `MutateReversible`/`Observe` still
  allowed. Demotion recorded as an audit event with the causal edge to the read.
- **I0 creation rule:** a Session whose intent/seed derives from external
  content starts draft-only and cannot auto-authorize Tier 3+. Seed-provenance
  field at session creation; the `caprun` CLI decides trusted-arg vs
  file-derived seed.
- **Confirmation loop:** `BlockedPendingConfirmation` surfaces the verbatim
  literal + provenance to the human via a **second command**
  (`caprun confirm <effect_id>` — testable, non-interactive-friendly); records
  confirm/deny as an audit event anchored to `SinkBlockedAnchor.effect_id`; on
  confirm releases exactly that (sink, arg, literal-digest) triple —
  **single-shot**, not a session-wide waiver or standing policy. Deny is
  durable. The release path lives in the TCB, not policy.
- **Live acceptance (§9-style, from the CLI):** hostile workspace file → worker
  reads it → session demoted (I1) → tainted routing arg Blocked (I2, existing)
  → human denies → nothing sent; separately, human confirms → effect proceeds
  exactly once; audit DAG shows the unbroken chain read → demotion → block →
  human decision.

**Design gate:** a DESIGN doc for session-trust-state / confirmation semantics
gates the phases that add executor behavior (same discipline as the v1.0
executor gate).

**Explicitly not in v1.2:** more sinks, real LLM planner, Git/GitHub adapters,
Cedar, cross-host delegation, content-sensitive arg blocking (deferred by
design). README-vs-CaMeL positioning is a small optional add-on.

**Seed:** `planning-docs/MILESTONE-v1.2-SEED.md` (2026-07-01 post-v1.1
assessment). PLAN.md wins on any conflict.

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

Shipped in **v1.1 — Usable Runtime (Live §9 from the CLI)** (2026-07-01). Full
traceability archived in `.planning/milestones/v1.1-REQUIREMENTS.md`.

- ✓ Unified `caprun` onto the `brokerd::server` dispatch (no second executor path) — v1.1
- ✓ Typed `ReportClaims` IPC from the confined worker — raw bytes never reach the planner — v1.1
- ✓ Session-scoped handles; cross-session resolution denied (HARD-03) — v1.1
- ✓ Deterministic intent → PlanNode planner (handles only) + `mint_from_intent`; clean allow-path reachable (HARD-02) — v1.1
- ✓ `file.create` sink: arg-schema fail-closed, `O_EXCL`, dirfd + `openat2 RESOLVE_BENEATH` (SINK-01..04, HARD-04) — v1.1
- ✓ Mint invariant at source (HARD-05), typed `DenyReason`, broker-minted `effect_id` (HARD-06) — v1.1
- ✓ Durable genuine-taint anchor (ACC-07) + full live §9 acceptance green on real Linux (ACC-01/03/04/05/06) — v1.1

### Active

**v1.2 — Tainted Session, Human Gate** (scoped 2026-07-01):

- [x] Session taint state: `mint_from_read` demotes the session to draft-only;
      draft-only denies `CommitIrreversible` plan nodes in the executor (new
      `DenyReason` variant); demotion is an audited event with a causal edge to
      the read. — validated in Phase 9 (2026-07-07)
- [x] I0 creation rule: externally-seeded sessions start draft-only
      (seed-provenance field at session creation). — validated in Phase 9
      (2026-07-07)
- [x] Confirmation loop: `caprun confirm <effect_id>` releases exactly one
      (sink, arg, literal-digest) triple, single-shot; confirm/deny audited and
      anchored to `SinkBlockedAnchor.effect_id`; deny durable. — validated in
      Phase 10 (2026-07-07)
- [ ] DESIGN doc (session-trust-state + confirmation semantics) gates executor
      behavior changes.
- [ ] Live §9-style acceptance: read → demotion → block → human deny (nothing
      sent) / human confirm (exactly once), unbroken audit chain.

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

- **Current state (v1.1 shipped 2026-07-01):** v0 done (v1.0) + Usable Runtime
  (v1.1). 7 phases, 30 plans across `runtime-core`, `sandbox`, `brokerd`,
  `executor`, `adapter-fs`, and the `caprun` binary. A real kernel-confined
  `caprun` run drives a live `file.create` sink: hostile input is
  deterministically blocked on a genuine, DB-durable taint chain; a trusted
  intent path is allowed. `cargo test --workspace` green on macOS; full live §9
  acceptance (ACC-03/04/05/07) green on real Linux (Colima+Docker). Security
  claims remain Linux-only.
- **v1.1 delivered (Phase 7 complete, 2026-07-01):** `file.create` is a real,
  hardened sink (schema gate, fail-closed, `O_EXCL`, dirfd + `openat2`
  `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`); mint invariant enforced at the source;
  typed `DenyReason`; durable genuine-taint anchor (ACC-07) persisted across
  process exit; live §9 hostile-block + clean-allow + unbroken causal chain green
  on real Linux `caprun`. Verifier independently re-ran the Colima/Docker recipe.
- **Next milestone:** unscoped — start with `/gsd-complete-milestone` (archive
  v1.1), then `/gsd-new-milestone`.
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
| v1.2: draft-only deny decided in the executor, not a broker pre-check | Keep all deny logic in one TCB function with one DenyReason taxonomy | — Locked (Phase 9) |
| v1.2: confirmation UX = `caprun confirm <effect_id>` second command | Testable and non-interactive-friendly vs a TTY prompt | — Locked (Phase 10) |
| v1.2: confirm is single-shot (one (sink, arg, literal-digest) triple) | Standing exact-match policy is scope creep for v1.2 | — Locked (Phase 10) |
| **DEC-ai-review-satisfies-human-gate** (2026-07-06): an AI-performed adversarial re-read (by the current best-available Claude model) may satisfy the "human reviewer" requirement in design-gate checkpoints (e.g. `08-03-PLAN.md` Task 2's `checkpoint:human-verify`), when Ben Lamm explicitly authorizes it in place of his own read | Ben's explicit call after being shown the tension directly: this reverses the checkpoint's original intent — mirrored from v1.0 Phase 2 and from this milestone's own core value (AI/agent judgment is insufficient for consequential decisions; hence I0/I1/I2 + human confirmation) — but he chose to accept an AI review (Fable 5) as equivalent to his own for Phase 8's gate, after the tradeoff was named explicitly (raised: self-review of one's own prior finding is a weaker check than independent human adversarial judgment; a fresh-session independent AI check was offered as a middle ground and declined) | **Locked, retroactive to Phase 8's round-2 gate.** Applies going forward to future design-gate checkpoints unless revisited. Does NOT retroactively bless anything already recorded as "reviewed by Ben personally" elsewhere (e.g. round 1, `planning-docs/DESIGN-REVIEW-v1.2-round1.md`, is now understood to have also been Fable-authored — accepted under this same decision, not because it was independently re-verified as human work). |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-07 after Phase 10 (Single-Shot Confirmation Loop)
completed and verified: `caprun confirm`/`caprun deny` release/deny a blocked
effect via a durable, effect_id-keyed `PendingConfirmation` checkpoint;
confirm/deny live in the TCB (`crates/brokerd`), never re-invoke
`executor::submit_plan_node`, and are anchored to the `sink_blocked` event via
`parent_id`. Cross-process integration tests prove single-shot release and
durable deny across separate `caprun` invocations (`cargo test --workspace
--no-fail-fast` green). Next: Phase 11 (live acceptance — tainted session,
human gate). v1.0 shipped the mechanism proof; v1.1 shipped the live runtime;
v1.2 is now confirmation-loop complete, pending live acceptance.*
