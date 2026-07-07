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

## Milestone: v1.2 — Tainted Session, Human Gate

**Shipped:** 2026-07-07
**Phases:** 4 (8-11) | **Plans:** 11 | **Tasks:** 25

### What Was Built

Draft-only session demotion and single-shot human confirmation, proven live on real Linux:

- **Design gate (Phase 8)** — `DESIGN-session-trust-state.md` (I1 dynamic demotion, I0 creation rule, `SessionStatus::Draft`, one executor deny function) + `DESIGN-confirmation-release.md` (`PendingConfirmation` checkpoint, confirm/deny semantics, CLI contract). Round-1 adversarial review caught a genuine architectural blocker (B1: I1-deny-before-I2-Block precedence would have made a Draft session's tainted arg unconfirmable — a dead end) before any executor code existed.
- **Session trust state (Phase 9)** — `mint_from_read` atomically demotes a session to `Draft` with a causally-linked `session_demoted` event (TAINT-01/04); a single post-loop Step 0.5 in the executor denies `Draft`+`CommitIrreversible` without ever pre-empting the existing I2 Block (TAINT-02/03); `--seed-from-file` gives I0 (ORIGIN-01/02) a real CLI on-ramp.
- **Single-shot confirmation loop (Phase 10)** — durable `pending_confirmations` side table, block-time full-arg snapshot persisted atomically with `sink_blocked`, TCB-resident `confirm`/`deny` in `crates/brokerd/src/confirmation.rs`, 6-way exit-code CLI contract, cross-process integration test proving single-shot release and durable deny across separate OS processes.
- **Live acceptance (Phase 11)** — a new Linux-gated integration test composing the already-proven Phase 9/10 mechanisms into one live end-to-end run: hostile read → I1 demotion → I2 block → `caprun deny`/`caprun confirm`, with one unbroken audit-DAG causal chain for both outcomes (ACC-01/02/03), verified via Colima+Docker.

### What Worked

- **The design gate caught a real bug, not a rubber stamp.** The round-1 adversarial review of `DESIGN-session-trust-state.md` found that the I1 draft-only deny and the I2 taint-Block, as originally specified, composed into a dead end — a Draft session's tainted arg would Deny before ever reaching a confirmable Block. This was fixed in the design doc *before* Phase 9 wrote a line of executor code — the cheapest place a bug like this can be caught.
- **Source-grounded research answered the hardest question and found a bug nobody was looking for.** Phase 11's RESEARCH.md traced the actual `parent_id` wiring through three independent source locations (not summaries) to answer ACC-03's causal-chain question definitively — and in the process discovered that `s9_live_block.rs`'s existing hostile-block assertion had been silently stale since Phase 9's chain-head fix, never caught because it's Linux-gated and never run on the macOS dev box. Fixing it became an explicit plan task instead of a surprise mid-execution.
- **Independent live-Linux re-verification, now 2-for-2.** As in v1.1, the orchestrator did not accept the executor's SUMMARY.md claims at face value for the phase's core runtime-behavior assertion — it re-ran the actual Colima+Docker commands itself at verification time and confirmed matching output before upgrading VERIFICATION.md from `human_needed` to `passed`. Caught nothing wrong either time, but this is now a confirmed standing practice for this project's security-critical DONE gates, not a one-off.
- **The mechanical decision-coverage gate caught a real translation gap.** `check.decision-coverage-plan` blocked planning completion because the plan substantively addressed all 6 CONTEXT.md decisions but never literally cited their `D-NN` ids in a gate-scanned location (YAML `must_haves`/`truths`/`objective` frontmatter keys, or `<objective>`/`<tasks>`/`<action>` XML bodies) — the independent `gsd-plan-checker` had already scored this dimension PASS via semantic reading, so the two checks genuinely disagreed. Resolved with a two-sentence addition inside `<objective>`; cheap, and it caught something the semantic-only checker didn't.

### What Was Inefficient

- **Recorded the wrong `expected_base` for a worktree merge.** When calling `worktree.record-agent` after the Phase 11 executor returned, the orchestrator used the value from the executor's own `<worktree_metadata>` return block — which turns out to hold the branch's own final-commit hash, not the dispatch-time fork point `worktree.cleanup-wave` actually needs (it computes `git merge-base HEAD <branch>` and checks that against `expected_base`). This produced a false `base_mismatch` block. Diagnosed by reading `worktree-safety.cjs`'s actual merge logic and confirming with `git merge-base` directly; fixed by editing the manifest JSON to the orchestrator's captured dispatch-time `EXPECTED_BASE` instead. Cheap fix, but worth remembering: `<worktree_metadata>`'s `expected_base` field is not the value `record-agent --base` wants.

### Patterns Established

- **Composition-proof phases stay thin.** Phase 11 needed only 1 plan / 3 tasks because Phases 9 and 10 had already individually proven every underlying mechanism — the actual new work was live end-to-end wiring plus catching drift between phases (the stale assertion), not new mechanism. Don't over-plan a phase whose job is proving composition, not building.
- **Cite `D-NN` decision ids inside `<objective>` (or YAML `must_haves`/`truths`).** The decision-coverage gate only scans specific locations (YAML frontmatter `must_haves`/`truths`/`objective` keys, and `<objective>`/`<tasks>`/`<task>`/`<action>` XML tag bodies) — a custom-named XML block anywhere else is invisible to it even if the content is substantively correct.
- **Record the dispatch-time orchestrator HEAD as a worktree's `expected_base`**, captured via `git rev-parse HEAD` *before* spawning the agent — never a value read back from the executor's own return metadata.

### Key Lessons

- Adversarial design review earns its cost when it's actually adversarial: the v1.2 design gate found a real precedence bug, not just prose polish. Keep gating executor code behind reviewed design docs for any milestone that adds new deny/enforcement logic.
- "Independently re-run the live proof at verification time" is no longer a one-off caution from v1.1 — it's now this project's standing practice for security DONE gates, confirmed twice. Keep doing it even when the executor's self-report looks solid.
- A mechanical gate and a semantic reviewer can legitimately disagree without either being wrong — the mechanical gate checks a narrower, stricter thing (literal traceability in specific locations) that the semantic reviewer doesn't. Don't drop either check in favor of the other.

### Cost Observations

- Model mix: orchestrator Sonnet 5; planner on Opus; researcher, pattern-mapper, plan-checker, executor, and verifier all on Sonnet.
- 111 commits across the milestone (`git log v1.1..v1.2`); 97 files changed, +16569/-231 lines; 6 days wall-clock (2026-07-01 → 2026-07-07).
- Notable: the live-Linux proof was run twice again (executor + orchestrator independently) — deliberate redundancy for the security gate, same as v1.1, now a confirmed pattern rather than a one-off.

## Cross-Milestone Trends

| Milestone | Phases | Plans | Shipped | Notes |
|-----------|--------|-------|---------|-------|
| v1.0 MVP  | 4      | 15    | 2026-06-30 | v0 DONE — genuine-taint §9 gate cleared |
| v1.1 Usable Runtime | 3 | 15 | 2026-07-01 | Live §9 from the CLI — real `file.create` sink, DB-durable taint chain, Linux-verified |
| v1.2 Tainted Session, Human Gate | 4 | 11 | 2026-07-07 | Draft-only session demotion (I1/I0) + single-shot confirmation loop, live-proven on real Linux; independent live-run re-verification now a confirmed 2/2 pattern |
