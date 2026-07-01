# Requirements: AgentOS — Milestone v1.1 (Usable Runtime — Live §9 from the CLI)

**Defined:** 2026-06-30
**Core Value:** A kernel-confined worker can only cause external effects through
broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically
blocks value-injection at sensitive sink arguments.
**Milestone goal:** Turn the proven-in-tests value-injection defense into a real
`caprun` run. One bounded new capability surface (`file.create`); no
network/shell/destructive overwrite.
**Scope reviewed by:** `#caprun-630` — codex + grok (2026-06-30).

## v1 Requirements

Requirements for milestone v1.1. Each maps to exactly one roadmap phase.

### Runtime Assembly (ASM)

The spine: collapse the dual dispatch and make the executor reachable from a live run.

- [x] **ASM-01**: `caprun` uses the single `brokerd::server` dispatch path for
  RequestFd, read reporting, mint, evaluate, audit, and sink invocation — caprun
  does NOT carry a second executor-dispatch implementation

- [x] **ASM-02**: The stale `"SubmitPlanNode not wired until Plan 04"` stub is
  removed; `executor::submit_plan_node` runs through the live broker path

- [x] **ASM-03**: A confined worker reads the passed fd and emits a `ReportClaims`
  IPC message defined as a **bounded tagged enum** (Phase 5 ships `EmailAddress`;
  `RelativePath` added in Phase 7). The broker validates each variant's size/shape
  and assigns taint/provenance itself; unknown claim kinds fail closed. Raw source
  bytes never cross into the planner

- [x] **ASM-04**: The broker mints authoritative `ValueId`s from worker-reported
  claims via `mint_from_read`, anchored to the real `file_read` audit event

### Planner & Intent (PLAN)

- [x] **PLAN-01**: `caprun` accepts an intent input alongside the workspace (not
  just a bare file path)

- [x] **PLAN-02**: A deterministic non-LLM planner maps a small typed intent enum
  to `PlanNode{sink, args}`, emitting only `SinkId` + existing `ValueId` handles

- [x] **PLAN-03**: The planner never sees raw bytes or taint labels — handles only
- [x] **PLAN-04**: A broker-owned `mint_from_intent` mints trusted values for
  clean/user-provided inputs, anchored to an `intent_received` audit event,
  separate from `mint_from_read`

### File Sink (SINK)

- [x] **SINK-01**: A `file.create` sink exists with an explicit arg schema
  (`path`, `contents`); missing, duplicate, or unknown args are rejected

- [x] **SINK-02**: `file.create`'s `path` arg is routing-sensitive in the
  sensitivity map

- [x] **SINK-03**: `file.create` uses exclusive creation (`O_EXCL`) — it never
  overwrites an existing file

- [x] **SINK-04**: `file.create` resolves paths via `openat2`
  (`RESOLVE_BENEATH`/`RESOLVE_NO_SYMLINKS`) under a workspace dirfd; absolute paths
  and traversal/symlink escapes are rejected; no validate-then-write (TOCTOU-safe).
  Shares the workspace-root capability model with `HARD-04` (read-side prerequisite)

### Enforcement Hardening (HARD)

Constraints raised by channel review that must hold for the live path to be sound.

- [x] **HARD-01**: Unknown sinks and unknown args fail closed (deny), validated
  before any sensitivity or executor step

- [x] **HARD-02**: The executor's blocking predicate is defined over
  explicitly-untrusted taint labels; `UserTrusted`/`LocalWorkspace`-only
  provenance does NOT block (clean allow-path is reachable)

- [x] **HARD-03**: `ValueRecord`s are session-scoped; a handle minted in one
  session is denied in another; the broker connection is bound to its session and
  a request-supplied `session_id` is never trusted

- [x] **HARD-04**: `RequestFd` reads are capability-restricted to the workspace
  root — the worker cannot nominate an arbitrary broker-opened path. Shares the
  workspace-root capability model with `SINK-04` (write-side); `HARD-04` is the
  read-side prerequisite for `SINK-04`

- [x] **HARD-05**: Effect-path ordering is enforced: validate schema → capability
  check → executor decision → durable authorization audit → sink invocation →
  durable result audit; audit failure fails closed; the causal parent is preserved
  (no `parent_id: None` best-effort append)

- [x] **HARD-06**: Each sink attempt carries an effect/request id; authorization is
  durably recorded before invocation; a crash after invocation leaves an explicit
  indeterminate record and triggers no automatic retry

### Acceptance (ACC)

The §9 live contract — the only definition of "done" for v1.1.

- [x] **ACC-01**: `BlockedPendingConfirmation` is operationally defined: zero sink
  invocations + a stable non-success CLI result + a durable `sink_blocked` event

- [x] **ACC-02**: Live §9 (email.send) — a real `caprun` run blocks a tainted
  routing-sensitive arg through the unified broker path, recording a **durable
  causal `sink_blocked` event** (the blocked-path audit primitive: causal parent
  preserved, append-failure fails closed, block durable before the CLI returns)

- [x] **ACC-03**: Live `file.create` block — hostile input → typed path claim →
  `mint_from_read` → `file.create` blocked, with no file written

- [x] **ACC-04**: Clean allow-path — a broker-minted trusted intent path creates
  the exact expected file under the workspace root

- [x] **ACC-05**: The audit DB shows one causal chain `fd_granted → file_read →
  plan_node_evaluated → sink_blocked/sink_executed` for the run

- [x] **ACC-06**: Forged handles and unknown sink/arg cases are denied
- [x] **ACC-07**: Genuine-taint sentinel (anti-stapling) — the blocked PlanArg's
  `ValueId` resolves to a `ValueRecord` whose `provenance_chain[0]` equals the
  actual `file_read` event id, and the **durable** audit evidence
  (`sink_blocked`/evaluation) links `effect_id + sink + arg + ValueId + provenance
  anchor` so the proof survives process exit. An event-order-only assertion is
  insufficient; this is the test that fails for any stapled-taint implementation

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
| Git/GitHub adapters, Cedar | Post-v0 per PLAN.md; `file.create` is the only new capability surface this milestone |
| Mac/WSL2 support | All v1.1 security claims remain Linux-only |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| ASM-01 | Phase 5 | Complete |
| ASM-02 | Phase 5 | Complete |
| ASM-03 | Phase 5 | Complete |
| ASM-04 | Phase 5 | Complete |
| HARD-03 | Phase 5 | Complete |
| ACC-02 | Phase 5 | Complete |
| PLAN-01 | Phase 6 | Complete |
| PLAN-02 | Phase 6 | Complete |
| PLAN-03 | Phase 6 | Complete |
| PLAN-04 | Phase 6 | Complete |
| HARD-02 | Phase 6 | Complete |
| SINK-01 | Phase 7 | Complete |
| SINK-02 | Phase 7 | Complete |
| SINK-03 | Phase 7 | Complete |
| SINK-04 | Phase 7 | Complete |
| HARD-01 | Phase 7 | Complete |
| HARD-04 | Phase 7 | Complete |
| HARD-05 | Phase 7 | Complete |
| HARD-06 | Phase 7 | Complete |
| ACC-01 | Phase 7 | Complete |
| ACC-03 | Phase 7 | Complete |
| ACC-04 | Phase 7 | Complete |
| ACC-05 | Phase 7 | Complete |
| ACC-06 | Phase 7 | Complete |
| ACC-07 | Phase 7 | Complete |

**Coverage:**

- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2026-06-30*
*Last updated: 2026-06-30 — traceability updated after roadmap revision (peer review #caprun-630 deltas applied: HARD-03 moved Phase 7→5; ACC-07 added to Phase 7; blocked-path audit primitive in Phase 5; ASM-03 phased EmailAddress/RelativePath; HARD-04+SINK-04 shared capability model noted)*
