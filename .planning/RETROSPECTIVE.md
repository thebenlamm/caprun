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

## Cross-Milestone Trends

| Milestone | Phases | Plans | Shipped | Notes |
|-----------|--------|-------|---------|-------|
| v1.0 MVP  | 4      | 15    | 2026-06-30 | v0 DONE — genuine-taint §9 gate cleared |
