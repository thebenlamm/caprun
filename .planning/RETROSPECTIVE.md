# AgentOS — Retrospective

A living record of what worked, what didn't, and the patterns established across milestones.

## Milestone: v1.0 — MVP (AgentOS v0)

**Shipped:** 2026-06-30
**Phases:** 4 | **Plans:** 15 | **Tasks:** 16

### What Was Built

A kernel-confined Intent Runtime that proves I2 value-injection defense end-to-end:

- **runtime-core** — pure domain types; `ValueNode` carries literal+provenance+taint from the first commit; opaque `ValueId`/`PlanArg` handle model keeps literals/taint away from the planner.
- **Security design gate** — `DESIGN-taint-model.md` + `DESIGN-plan-executor.md` formalize I0/I1/I2 and hard-gate all executor code.
- **Confinement & mediation** — namespaces + Landlock + seccomp worker confinement, broker reference monitor, SCM_RIGHTS fd-pass fs adapter, hash-chained audit DAG. No-LLM substrate demo Linux-verified 29/29.
- **Deterministic executor** — `crates/executor` pure decision function over a broker-owned `ValueStore` (sole taint writer) + hardcoded email.send sensitivity map.
- **Genuine taint** — quarantined reader + `mint_from_read` as the sole broker mint site; `provenance_chain` anchored to the real `file_read` Event.
- **§9 acceptance test** — the v0-DONE gate: blocks a tainted address at a mediated sink with literal-value confirmation; the two-sided `provenance_chain[0] == read_event_id` backstop fails any stapled-taint shortcut. `cargo test --workspace` = 51 green.

### What Worked

- **Locking the architecture on day one** (PlanNode/ValueNode API, taint triple in the type) meant no later phase could accidentally open a raw `EffectRequest→sink` bypass. The structural grep-gate caught drift cheaply.
- **The design gate before `crates/executor`** — writing the taint/executor model docs first (and only unblocking the executor crate after round-2 APPROVED) avoided building a wrong-shape enforcer.
- **The genuine-taint backstop as a held-out assertion** — `provenance_chain[0] == read_event_id` is the one test that distinguishes a real defense from a demo that staples taint at the sink. Making it the single non-negotiable gate kept the whole milestone honest.
- **Wave-based parallel execution with worktree isolation** — Wave 2's two independent plans (executor crate vs broker stubs, non-overlapping files) ran in parallel and merged cleanly; post-merge `cargo test --workspace` caught integration at each wave boundary.

### What Was Inefficient

- **One executor stall** (Phase 4, plan 04-01) hit the 600s watchdog mid-task with 0 commits. Resuming the agent (rather than restarting) preserved its partial worktree edit and it finished cleanly — but ~10 min was lost to the stall + recovery decision.
- **SUMMARY one-liner extraction is fragile** — several phase SUMMARYs led with a deviation line or a bare filename, so the auto-generated MILESTONES.md accomplishments came out noisy and needed a manual rewrite. Worth standardizing the SUMMARY first line.

### Patterns Established

- **Sole-mint-site discipline:** exactly one function may write taint (`ValueStore::mint`, reached only via `mint_from_read`), verified by negative grep in tests. Anti-stapling is a structural invariant, not a convention.
- **Two-sided provenance assertion:** assert the chain anchor from both directions (`chain[0] == event.id` AND `event.id == chain[0]`) so neither a missing event nor a fabricated id slips through.
- **Per-wave post-merge gate:** always run `cargo build/test --workspace` on `main` after merging a wave's worktrees, before the next wave forks — isolated self-checks pass even when a merge drops code.

### Key Lessons

- A security demo's value lives entirely in the one assertion that fails for the lazy implementation. Identify it first, make it the gate, build backward.
- Stalled worktree agents are usually resumable — check for committed work + dirty state before discarding; restarting from scratch is the more expensive default.

### Cost Observations

- Model mix: orchestrator Opus 4.8; all executors + verifier on Sonnet (`executor_model: sonnet`).
- ~106 commits total across the project; v1.0 built in ~1 day of wall-clock.
- Notable: parallel worktree execution kept wall-clock near the slowest single plan per wave rather than the sum.

## Milestone: v1.1 — Usable Runtime (Live §9 from the CLI)

**Shipped:** 2026-07-01
**Phases:** 3 (5-7) | **Plans:** 15

### What Was Built

The v1.0 mechanism proof became a real `caprun` run:

- **Unified runtime spine (Phase 5)** — one `brokerd::server` dispatch path (no second executor loop), typed `ReportClaims` IPC, session-scoped handles, durable fail-closed `sink_blocked` (ACC-02, HARD-03).
- **Deterministic planner & intent input (Phase 6)** — typed `CaprunIntent` → `PlanNode` over opaque handles; `mint_from_intent` `[UserTrusted]` values; executor predicate over `is_untrusted()` so the clean allow-path is reachable (HARD-02).
- **Hardened `file.create` sink + full acceptance (Phase 7)** — mint invariant at source (HARD-05), typed `DenyReason`, `WorkspaceRoot` dirfd + `openat2 RESOLVE_BENEATH` (HARD-04/SINK-04), `O_EXCL`, arg-schema gate, durable genuine-taint anchor (ACC-07); full live §9 (ACC-03/04/05) green on real Linux.

### What Worked

- **Small, dependency-ordered waves** — 5 of 7 waves were single-plan dependency links; the one parallel wave (07-01 ⨯ 07-03, disjoint files) merged cleanly. The strict wave graph meant each executor built on a verified base.
- **The verifier independently re-ran the Linux proof** — rather than trusting the executor's SUMMARY narration, `gsd-verifier` re-ran the Colima/Docker recipe itself and line-read the modified TCB. This caught nothing wrong but is the right posture for a security milestone.
- **Fail-closed worktree guards did their job** — the initial dispatch omitted `isolation="worktree"`; the executor's `worktree-branch-check` halted on `main` (exit 42) with zero writes instead of committing plan work onto `main`. The guard, not luck, prevented the damage.

### What Was Inefficient

- **Orchestrator dispatch bug cost a retry** — the first two Wave-1 executors ran without `isolation="worktree"` and halted harmlessly; both had to be re-dispatched. A one-line omission, but it doubled Wave 1's spawn count.
- **Post-merge `cargo test` timed out at the 300s budget** after the anchor-reshape merge (compile-bound, not a failure). Had to re-run at 540s to get a definitive green. Rust workspace test-compile after a merge routinely exceeds 5 min — the default gate budget is too tight for this project.
- **Noisy auto-extracted accomplishments again** — same v1.0 issue: SUMMARY files led with `[Rule 3 - Blocking]` deviation headers, so MILESTONES.md needed a manual rewrite.

### Patterns Established

- **The v1.1-DONE bug caught by ACC-05:** 07-05 found `verify_chain` returning false on the live DAG because both mint sites hardcoded `parent_id: None` (multiple roots). The causal-chain assertion, run end-to-end, exposed a real TCB gap that per-unit tests missed — "substrate working ≠ done" made concrete.
- **After-exit, DB-alone acceptance:** the canonical ACC-07 proof reads the persisted audit DB *after the worker exits* and verifies the chain from the DB alone — the strongest anti-stapling posture (an in-process assertion can be fooled by live state).

### Key Lessons

- For Rust workspaces, set the post-merge test gate budget to ≥540s — the 300s default reads as an inconclusive timeout, not a pass, and forces a re-run.
- When honoring `use_worktrees`, the `isolation="worktree"` parameter is load-bearing on *every* executor dispatch — the branch-check will catch its absence, but at the cost of a wasted spawn.

### Cost Observations

- Model mix: orchestrator Opus 4.8; all executors + verifier on Sonnet.
- 6 plans across 5 waves; wall-clock dominated by Rust workspace compile per worktree (each isolated `target/`).
- Notable: the live-Linux proof was run twice (executor + verifier independently) — deliberate redundancy for the security gate.

## Cross-Milestone Trends

| Milestone | Phases | Plans | Shipped | Notes |
|-----------|--------|-------|---------|-------|
| v1.0 MVP  | 4      | 15    | 2026-06-30 | v0 DONE — genuine-taint §9 gate cleared |
| v1.1 Usable Runtime | 3 | 15 | 2026-07-01 | Live §9 from the CLI — real `file.create` sink, DB-durable taint chain, Linux-verified |
