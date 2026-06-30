# Requirements: AgentOS — Milestone v1.1 (Usable Runtime — Live §9 from the CLI)

**Defined:** 2026-06-30
**Core Value:** A kernel-confined worker can only cause external effects through
broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically
blocks value-injection at sensitive sink arguments.
**Milestone goal:** Turn the proven-in-tests value-injection defense into a real
`caprun` run. Runtime assembly only — no new capability surface.
**Scope reviewed by:** `#caprun-630` — codex + grok (2026-06-30).

## v1 Requirements

Requirements for milestone v1.1. Each maps to exactly one roadmap phase.

### Runtime Assembly (ASM)

The spine: collapse the dual dispatch and make the executor reachable from a live run.

- [ ] **ASM-01**: `caprun` uses the single `brokerd::server` dispatch path for
  RequestFd, read reporting, mint, evaluate, audit, and sink invocation — caprun
  does NOT carry a second executor-dispatch implementation
- [ ] **ASM-02**: The stale `"SubmitPlanNode not wired until Plan 04"` stub is
  removed; `executor::submit_plan_node` runs through the live broker path
- [ ] **ASM-03**: A confined worker reads the passed fd and emits a typed
  `ReportClaims`-style IPC message; raw source bytes never cross into the planner
- [ ] **ASM-04**: The broker mints authoritative `ValueId`s from worker-reported
  claims via `mint_from_read`, anchored to the real `file_read` audit event

### Planner & Intent (PLAN)

- [ ] **PLAN-01**: `caprun` accepts an intent input alongside the workspace (not
  just a bare file path)
- [ ] **PLAN-02**: A deterministic non-LLM planner maps a small typed intent enum
  to `PlanNode{sink, args}`, emitting only `SinkId` + existing `ValueId` handles
- [ ] **PLAN-03**: The planner never sees raw bytes or taint labels — handles only
- [ ] **PLAN-04**: A broker-owned `mint_from_intent` mints trusted values for
  clean/user-provided inputs, anchored to an `intent_received` audit event,
  separate from `mint_from_read`

### File Sink (SINK)

- [ ] **SINK-01**: A `file.create` sink exists with an explicit arg schema
  (`path`, `contents`); missing, duplicate, or unknown args are rejected
- [ ] **SINK-02**: `file.create`'s `path` arg is routing-sensitive in the
  sensitivity map
- [ ] **SINK-03**: `file.create` uses exclusive creation (`O_EXCL`) — it never
  overwrites an existing file
- [ ] **SINK-04**: `file.create` resolves paths via `openat2`
  (`RESOLVE_BENEATH`/`RESOLVE_NO_SYMLINKS`) under a workspace dirfd; absolute paths
  and traversal/symlink escapes are rejected; no validate-then-write (TOCTOU-safe)

### Enforcement Hardening (HARD)

Constraints raised by channel review that must hold for the live path to be sound.

- [ ] **HARD-01**: Unknown sinks and unknown args fail closed (deny), validated
  before any sensitivity or executor step
- [ ] **HARD-02**: The executor's blocking predicate is defined over
  explicitly-untrusted taint labels; `UserTrusted`/`LocalWorkspace`-only
  provenance does NOT block (clean allow-path is reachable)
- [ ] **HARD-03**: `ValueRecord`s are session-scoped; a handle minted in one
  session is denied in another; the broker connection is bound to its session and
  a request-supplied `session_id` is never trusted
- [ ] **HARD-04**: `RequestFd` reads are capability-restricted to the workspace
  root — the worker cannot nominate an arbitrary broker-opened path (same
  restriction as the write sink)
- [ ] **HARD-05**: Effect-path ordering is enforced: validate schema → capability
  check → executor decision → durable authorization audit → sink invocation →
  durable result audit; audit failure fails closed; the causal parent is preserved
  (no `parent_id: None` best-effort append)
- [ ] **HARD-06**: Each sink attempt carries an effect/request id; authorization is
  durably recorded before invocation; a crash after invocation leaves an explicit
  indeterminate record and triggers no automatic retry

### Acceptance (ACC)

The §9 live contract — the only definition of "done" for v1.1.

- [ ] **ACC-01**: `BlockedPendingConfirmation` is operationally defined: zero sink
  invocations + a stable non-success CLI result + a durable `sink_blocked` event
- [ ] **ACC-02**: Live §9 (email.send) — a real `caprun` run blocks a tainted
  routing-sensitive arg through the unified broker path
- [ ] **ACC-03**: Live `file.create` block — hostile input → typed path claim →
  `mint_from_read` → `file.create` blocked, with no file written
- [ ] **ACC-04**: Clean allow-path — a broker-minted trusted intent path creates
  the exact expected file under the workspace root
- [ ] **ACC-05**: The audit DB shows one causal chain `fd_granted → file_read →
  plan_node_evaluated → sink_blocked/sink_executed` for the run
- [ ] **ACC-06**: Forged handles and unknown sink/arg cases are denied

## v2 Requirements

Deferred to a future milestone. Tracked, not in this roadmap.

### Planner

- **PLAN-F1**: LLM-driven planner turning natural-language intent into PlanNodes
  (re-opens I0/I1 surface; large lift)

### Sinks

- **SINK-F1**: HTTP POST sink (URL routing-sensitive) with network egress carve-out
- **SINK-F2**: Shell-exec sink
- **SINK-F3**: `file.write` with destructive overwrite (beyond `O_EXCL` create)

### Approval

- **APPR-F1**: Interactive human literal-value confirmation UX in the CLI
- **APPR-F2**: Multi-step agent loop (plan → execute → observe → replan)

## Out of Scope

Explicitly excluded for v1.1. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| LLM planner | Re-opens I0/I1 surface; deterministic planner proves the runtime first |
| HTTP / shell sinks | `file.create` is the bounded first real sink; others are v2 |
| Interactive approval UX | The block must *fire* + be auditable; UX is v2 |
| Multi-step agent loop | Single-shot plan→execute→block proves "usable"; loop is v2 |
| Git/GitHub adapters, Cedar | Post-v0 per PLAN.md; "no new capability surface" this milestone |
| Mac/WSL2 support | All v1.1 security claims remain Linux-only |

## Traceability

Populated during roadmap creation (phase numbering continues from v1.0 — starts at Phase 05).

| Requirement | Phase | Status |
|-------------|-------|--------|
| ASM-01 | TBD | Pending |
| ASM-02 | TBD | Pending |
| ASM-03 | TBD | Pending |
| ASM-04 | TBD | Pending |
| PLAN-01 | TBD | Pending |
| PLAN-02 | TBD | Pending |
| PLAN-03 | TBD | Pending |
| PLAN-04 | TBD | Pending |
| SINK-01 | TBD | Pending |
| SINK-02 | TBD | Pending |
| SINK-03 | TBD | Pending |
| SINK-04 | TBD | Pending |
| HARD-01 | TBD | Pending |
| HARD-02 | TBD | Pending |
| HARD-03 | TBD | Pending |
| HARD-04 | TBD | Pending |
| HARD-05 | TBD | Pending |
| HARD-06 | TBD | Pending |
| ACC-01 | TBD | Pending |
| ACC-02 | TBD | Pending |
| ACC-03 | TBD | Pending |
| ACC-04 | TBD | Pending |
| ACC-05 | TBD | Pending |
| ACC-06 | TBD | Pending |

**Coverage:**
- v1 requirements: 24 total
- Mapped to phases: 0 (roadmap pending)
- Unmapped: 24 ⚠️

---
*Requirements defined: 2026-06-30*
*Last updated: 2026-06-30 after milestone v1.1 definition (channel-reviewed)*
