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

## Current Milestone: v1.3 "Doc → Action Assistant"

**Goal:** caprun ingests an untrusted document containing an embedded
injection, deterministically extracts a "send to X" action (recipient + body
derived from the doc's content, no LLM planner), and attempts a real email
send. The read demotes the session (I1, existing); the tainted recipient AND
body both block at the sink (I2 + new CONTENT-01); `caprun confirm`/`deny`
shows verbatim recipient+body+provenance; confirm sends exactly once via a
real broker-mediated SMTP adapter, deny sends nothing — one unbroken audit DAG
for both outcomes, plus a clean-send negative control in the same run, proven
live on real Linux via Colima+Docker.

**Target features:**
- Real broker-mediated SMTP adapter (worker never sends; secrets live only in
  the broker; gate test targets a local capture SMTP — MailHog/Mailpit)
- CONTENT-01: content-sensitive sink-arg blocking (body, not just
  recipient/routing) — reopens a decision deferred at v1.2 scoping
- Deterministic (non-LLM), confined doc→action extraction with a manipulation
  variant proving taint survives transformation, not just copying
- Negative controls (trusted send proceeds ungated; tainted-body/trusted-
  recipient still blocks) so the demo is a controlled experiment, not anecdote
- Confirm-binding to resolved literals (anti-TOCTOU) and idempotent, failure-
  safe send

**Explicitly not reopened:** the LLM planner stays out/deterministic. v1.3
proves taint *enforcement* through a deterministic extractor — it does not
claim taint survives a real LLM planner's regeneration ("laundering"); that is
v1.4+ (see `DOC-01`).

Full v1.2 detail archived in
[`milestones/v1.2-ROADMAP.md`](milestones/v1.2-ROADMAP.md) and
[`milestones/v1.2-REQUIREMENTS.md`](milestones/v1.2-REQUIREMENTS.md).

<details>
<summary>✅ v1.2 — Tainted Session, Human Gate — SHIPPED 2026-07-07</summary>

**Goal:** A session that touches untrusted content is mechanically demoted to
draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked sink arg
can be released only by literal-value human confirmation — all deterministic,
all in the audit DAG.

**Delivered:**
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
  human decision — proven live on real Linux via Colima+Docker in Phase 11.

**Design gate:** a DESIGN doc for session-trust-state / confirmation semantics
gated the phases that added executor behavior (same discipline as the v1.0
executor gate) — `planning-docs/DESIGN-session-trust-state.md` +
`planning-docs/DESIGN-confirmation-release.md`, Phase 8.

**Explicitly not in v1.2:** more sinks, real LLM planner, Git/GitHub adapters,
Cedar, cross-host delegation, content-sensitive arg blocking (deferred to v2 —
tracked as `CONTENT-01`/`DOC-01`). README-vs-CaMeL positioning remains a small
optional add-on, still not done.

**Seed:** `planning-docs/MILESTONE-v1.2-SEED.md` (2026-07-01 post-v1.1
assessment). PLAN.md wins on any conflict.

</details>

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

Shipped in **v1.2 — Tainted Session, Human Gate** (2026-07-07). Full
traceability archived in `.planning/milestones/v1.2-REQUIREMENTS.md`.

- ✓ Session taint state: `mint_from_read` demotes the session to draft-only;
  draft-only denies `CommitIrreversible` plan nodes in the executor, one TCB
  deny function (TAINT-01..04) — v1.2
- ✓ I0 creation rule: externally-seeded sessions start draft-only via
  `--seed-from-file` (ORIGIN-01/02) — v1.2
- ✓ Confirmation loop: `caprun confirm`/`caprun deny <effect_id>` releases or
  durably blocks exactly one (sink, arg, literal-digest) triple, TCB-resident,
  single-shot (CONFIRM-01..04) — v1.2
- ✓ DESIGN doc (session-trust-state + confirmation semantics) gated all
  executor behavior changes before code (PROC-01) — v1.2
- ✓ **Live acceptance on real Linux (v1.2 DONE gate):** hostile read → I1
  demotion → I2 block → human deny (nothing sent) / human confirm (effect
  proceeds exactly once), one unbroken audit-DAG causal chain for both
  outcomes, proven via Colima+Docker (ACC-01/02/03). Caught and fixed a
  pre-existing stale test assertion (`s9_live_block.rs`, dating to Phase 9,
  never previously exercised on Linux) in the process.

### Active

Scoped for **v1.3 "Doc → Action Assistant"** — see
`.planning/REQUIREMENTS.md` for full REQ-ID list (SMTP, CONTENT, EXTRACT,
CONFIRM, CONTROL, SEND, DESIGN, DOC, ACCEPT categories).

### Out of Scope

Non-goals, reviewed at each milestone close (v0/v1.1/v1.2/v1.3) — still valid
as of 2026-07-07 unless noted:

- Git / GitHub adapters — post-v1.2, no milestone has needed them yet
- Cedar policy engine — simple TOML/rules for sink access is fine; I2 stays in
  Rust (still true through v1.3 — the executor's `sink_effect_class` table
  remains hardcoded, not policy-driven)
- Cross-host delegation / Biscuit crypto — v3 concern
- gVisor / Firecracker — bubblewrap + seccomp + Landlock remains the boundary
  through v1.3
- LLM planner — a hard-coded / deterministic planner remains sufficient;
  re-affirmed at v1.3 scoping (NOT reopened alongside CONTENT-01/adapter — see
  `DOC-01`, which scopes what v1.3 does and does not prove about it)
- Live SES / real inbox send — **downgraded from a v1.3 requirement to an
  optional post-milestone config-swap** (was `SMTP-04` in the initial draft).
  MailHog/Mailpit IS a real SMTP send with a web UI showing arrival, which
  satisfies "real send" for the gate; live SES adds credentials/DNS/
  deliverability/throttling fragility and a live exception to default-deny-net
  at the exact claim being demoed, for ~zero legibility gain. (caprun-opus-77
  + advisor panel, 2026-07-07)
- General content-classification taxonomy/abstraction — `CONTENT-02` hardcodes
  sensitivity for the email sink's args only (one match arm), not a reusable
  framework
- Rich approval-policy learning, undo snapshots, broad effect taxonomy
- Web UI, marketplace, long-term memory, browser control, natural-language
  policy authoring
- Mac / WSL2 support — deferred best-effort; all security claims remain
  Linux-only through v1.3

## Context

- **Current state (v1.2 shipped 2026-07-07):** v0 done (v1.0) + Usable Runtime
  (v1.1) + Tainted Session, Human Gate (v1.2). 11 phases, 34 plans across `runtime-core`,
  `sandbox`, `brokerd`, `executor`, `adapter-fs`, and the `caprun` binary.
  Live on real Linux: a session demoted mid-run by a hostile read (I1) has its
  tainted routing arg Blocked at `file.create` (I2), and a human `caprun
  deny`/`caprun confirm` either durably blocks the effect or releases it
  exactly once — one unbroken audit-DAG causal chain
  (`fd_granted→file_read→session_demoted→sink_blocked→confirm_{denied,granted}`)
  proven for both outcomes via Colima+Docker (ACC-01/02/03). `cargo test
  --workspace` green on macOS (Linux-gated tests correctly show as excluded,
  not "0 passed" gaps).
- **Prior state (v1.1 shipped 2026-07-01):** v0 done (v1.0) + Usable Runtime
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
- **Next milestone:** unscoped — run `/gsd-new-milestone` (questioning →
  research → requirements → roadmap).
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
| v1.2: programmatic `caprun confirm`/`caprun deny` invocation (by an integration test or an agent) satisfies "human decision" for ACC-01/02 live-acceptance purposes — Ben typing the commands himself is additive, not required | Consistent with `DEC-ai-review-satisfies-human-gate`'s precedent; the confirm/deny CLI verbs ARE the human-interface artifact regardless of who invokes them, and Phase 10's `confirm.rs` already proved the mechanism this way | — Locked (Phase 11, discuss-phase D-05). Independently re-verified anyway: the orchestrator ran the live Colima+Docker proof itself at Phase 11 verification, closing gsd-verifier's `human_needed` gap with real evidence rather than relying solely on the executor's self-report. |
| **REOPENED v1.3** — Content-sensitive sink-arg blocking (`CONTENT-01`), deferred to v2 at v1.2 scoping, is now IN | The doc→action hero demo requires blocking a tainted email *body*, not just recipient/routing — v1.2's routing-only I2 scope can't demonstrate it | — Reopened 2026-07-07 (Ben + caprun-opus-77). Hardcoded sensitivity for the email sink's args only (`CONTENT-02`), not a general taxonomy — scope guard from the advisor panel. |
| **REOPENED v1.3** — Real broker-mediated SMTP adapter, previously a mediated sink *stub* per `DEC-layer-roles`, is now IN | The hero demo requires an actual send (confirm → email arrives) to be a genuine live-acceptance proof, not a stub invocation | — Reopened 2026-07-07 (Ben + caprun-opus-77). Confined worker never performs the SMTP call; secrets live only in the broker; gate test targets local MailHog/Mailpit — live SES is optional and NOT gated (see Out of Scope). |
| **NOT reopened v1.3** — LLM planner stays out/deterministic (`DEC-security-invariants`, `DEC-canonical-docs`) | v1.3 proves taint *enforcement* through a deterministic extractor; it explicitly does not claim taint survives a real LLM planner's regeneration ("laundering" — a real model can re-emit a tainted value as fresh model-authored tokens with no provenance). That is a v1.4+ concern. | — Confirmed 2026-07-07. `DOC-01` requires PROJECT.md/external claims to state this scope honestly — no claim that v1.3 proves taint-survives-a-real-agent. |

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
*Last updated: 2026-07-07 after starting v1.3 "Doc → Action Assistant"
milestone (`/gsd-new-milestone`). Reopened `CONTENT-01` and the real SMTP
adapter (see Key Decisions); LLM planner remains out. Prior:
v1.2's DONE gate (Phase 11):
a new Linux-gated integration test
(`cli/caprun/tests/live_acceptance_tainted_session.rs`) proves ACC-01/02/03
live on real Linux via Colima+Docker — hostile read → I1 demotion → I2 block
→ human deny (nothing sent) / human confirm (effect proceeds exactly once),
one unbroken causal chain (`verify_chain()` true, corrected `parent_id` walk)
for both outcomes. A pre-existing stale assertion in `s9_live_block.rs`
(dating to Phase 9's chain-head fix, never previously exercised on Linux) was
caught and fixed as part of this phase. VERIFICATION.md records both the
initial gsd-verifier pass (macOS, correctly scored human_needed for the
Linux-only claims) and the orchestrator's independent same-session
Colima+Docker re-run that closed the gap with real evidence. v1.0 shipped
the mechanism proof; v1.1 shipped the live runtime; **v1.2 shipped the
tainted-session human gate** — draft-only demotion (I1/I0) and single-shot
confirmation (CONFIRM-01..04) are now proven live, not just unit-tested.
Full v1.2 detail archived to `.planning/milestones/`. Next: unscoped — run
`/gsd-new-milestone`.*
