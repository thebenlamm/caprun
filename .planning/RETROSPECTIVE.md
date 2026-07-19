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

## Milestone: v1.3 — Doc → Action Assistant

**Shipped:** 2026-07-09
**Phases:** 6 (12-17) | **Plans:** 21 | **Tasks:** 49

### What Was Built

The hero demo: a confined worker deterministically extracts a "send to X" action from an untrusted document (no LLM planner), a tainted recipient AND body both Block at the sink, and confirm/deny/clean-control compose into one live-verified acceptance run:

- **Design gate (Phase 12)** — `DESIGN-content-adapter-mediation.md` (collect-then-Block executor hardening, real broker-mediated SMTP adapter mediation, CRLF/header-injection defense) + `DESIGN-confirm-binding.md` (full-set name-bound `combined_digest`). Round-1 review and 6 amendment rounds; DESIGN-01 adversarial gate closed without the authoring session self-reviewing.
- **Real SMTP adapter (Phase 13)** — broker-resident `email_smtp.rs` via `lettre`, atomic `pending→confirmed` CAS + durable send-attempt ledger in ONE SQLite transaction, kernel-denied negative-net control, live Mailpit-captured send + CRLF-injection defense proven on real Linux.
- **Content-sensitive blocking (Phase 14)** — executor's collect-then-Block reshape: `ExecutorDecision`/`SinkBlockedAnchor` made plural so a tainted body Blocks alongside a tainted recipient in the SAME decision, never first-match-wins.
- **Deterministic doc→action extraction (Phase 15)** — `mint_from_derivation` closes the milestone's #1 laundering risk: a transform-derived value's provenance_chain threads its inputs' own read-rooted chains, never a fresh transform-local root, with a byte-verified concat check and two anti-staple negative controls.
- **Confirm UX & negative controls (Phase 16)** — full-set name-bound `combined_digest`, verbatim block narration, `verify_chain` wired into confirm/deny, CONTROL-01/02 live A/B proof. Round-1 mandate (email.send Allowed-dispatch) opened a real live-exfiltration hole (a worker could skip `ReportClaims` and inject an arbitrary recipient via `ProvideIntent`) — caught by the coordinator's own panel, closed with 3 guards in the same phase.
- **Live acceptance & framing honesty (Phase 17)** — composed 3-session live test sharing ONE audit.db: confirm sends exactly once, a SEPARATE hostile block is denied sending nothing (Mailpit count AND ledger), a clean control delivers ungated, all sessions independently `verify_chain`-true; the milestone's HARD GATE re-proven against these live anchors; all 8 DOC-01 honesty points landed in PROJECT.md.

### What Worked

- **The coordinator's own round-1 mandate introduced a real vulnerability, and the same discipline caught and closed it.** Phase 16's email.send Allowed-dispatch (mandated by caprun-opus-77 itself) opened a live exfiltration path; the coordinator's own adversarial panel found it, owned the mistake explicitly, and mandated the fix as its own dedicated plan with its own review pass. The gate caught its own author's error — exactly what the discipline is for.
- **Flagging a locked-decision contradiction instead of self-resolving, twice.** Both the planner (Phase 17: the coordinator's "byte-identical fixture" ruling collided with the deny-leg's live Mailpit count==0 requirement and an existing passing test's fixed-literal assertion) and the orchestrator (the multi-session-vs-single-chain interpretation of "one unbroken audit DAG") surfaced genuine tensions in a coordinator ruling rather than picking a side unilaterally. Both times the coordinator revised its own ruling in response — a correction that would have shipped silently wrong if either party had "just made it work."
- **A fresh-context adversarial panel caught a real DAG-fork bug pre-execution.** Phase 17 round 1: the planner's anti-staple Control B copied a chaining pattern from the Phase-15 single-session template (mint onto `sink_blocked`'s hash) into a composed multi-event context where `sink_blocked` is no longer the chain head — would have forked the DAG and failed `verify_chain` on real Linux. Caught by the coordinator's panel before any executor touched the code, same class as Phase 16's MAJOR-7 fix recurring in a new harness.
- **Independent live-Linux re-verification, now 4-for-4 across the whole project.** The orchestrator re-ran the full unscoped `scripts/mailpit-verify.sh` itself (250/250, exit 0 captured before any pipe) before flagging DONE; the coordinator separately re-ran the same proof under its own execution AND personally read the final DOC-01 prose for points 2/3, rather than accepting either party's SUMMARY.md.
- **`scripts/mailpit-verify.sh` scaled cleanly to a shared multi-session harness.** No changes to the verification recipe itself were needed to support 3 sequential `caprun` invocations sharing one audit.db and one Mailpit sidecar — the existing sidecar-per-verification-run design already assumed multiple sends into one inbox (recipient-scoped assertions, not whole-inbox counts).

### What Was Inefficient

- **The `LIMIT 1` session-lookup anti-pattern was invisible until composition exposed it.** Every prior live test used a fresh tmp-dir/audit.db per invocation, so `SELECT id FROM sessions LIMIT 1` was correct by construction, never by design — for 6+ call sites across 2 test files, across 2 milestones. Phase 17's research pass caught it before code was written by reading the actual queries, not by a failure; costs nothing when caught in research, would have been a confusing silent-wrong-session bug if it shipped.
- **A locked ruling was over-specified on the first pass and needed a same-day correction.** The coordinator's "byte-identical hostile fixture" ruling was internally unsatisfiable (collided with its own tooth-#4 requirement and an existing test's fixed-literal assertion) — caught mid-planning, not mid-review, costing one extra FAMP round-trip but no wasted code.

### Patterns Established

- **A coordinator's mandate is not exempt from the adversarial gate it enforces on everyone else.** When a fresh panel finds a real vulnerability traceable to the coordinator's own prior instruction, the fix is: the coordinator owns the mistake explicitly, mandates the fix as its own plan, and gets its own fresh review — not a quiet patch folded into unrelated work.
- **When a locked ruling collides with another locked constraint, flag before encoding — even mid-execution.** Both the orchestrator (architectural interpretation) and the planner (fixture rule) treated a coordinator ruling as revisable-on-conflict rather than immutable-once-issued, and both times the coordinator's revision was substantively better than either the original ruling or a silent workaround would have been.
- **Composition phases re-prove, they don't reuse-by-assumption.** Phase 17's HARD GATE explicitly re-ran Phase 15's genuine-taint proof against the LIVE composed run's own anchors rather than treating Phase 15's own test coverage as sufficient — "the composition must re-prove descent here, or the 'one DAG' claim is decorative."
- **A DAG-fork bug class recurs whenever a proof pattern is copied from a single-session template into a multi-event composed context.** Anything that "chains onto event X's hash" must chain onto the CURRENT chain head at append time, never a specific named event that predates later appends — check this explicitly whenever composing sessions/scenarios that were individually proven in isolation.

### Key Lessons

- The project's own security-gate discipline is not immune to being the source of a new vulnerability — Phase 16 proved that a coordinator's mandate can introduce a real hole, and the same discipline (fresh panels, independent re-runs, explicit ownership of mistakes) is what catches it. Don't treat "the coordinator said so" as a lower-scrutiny path than any other change.
- "Independently re-run the live proof at verification time" is now a 4-for-4 confirmed pattern (v1.1, v1.2, and twice more within v1.3's own close) across two independent parties (executor-side orchestrator AND the delegated coordinator) — the redundancy has caught nothing wrong yet, but the discipline is what makes that a meaningful signal rather than luck.
- A locked ruling is a starting point for execution, not a substitute for verifying it against the actual code and existing tests before encoding it into a plan — over-specification (e.g. "byte-identical") is as real a risk as under-specification.

### Cost Observations

- Model mix: orchestrator Sonnet 5; planner on Opus; researcher and plan-checker on Sonnet; coordinator (caprun-opus-77, a separate delegated FAMP agent) ran all adversarial panels independently.
- 6 phases, ~4 adversarial FAMP rounds each (research/plan → coordinator review → revision → re-check), every round found something real — none were rubber-stamps.
- Notable: Phase 17 alone required 2 full adversarial rounds with the coordinator (1 BLOCKER + 5 MEDIUM + 1 NIT on round 1) before clearance, on top of the orchestrator's own independent gsd-plan-checker pass and a separate fresh-check pass after the revision — 3 independent verification layers before execution began.

## Milestone: v1.5 — Slot-Type Binding Enforcement (T2)

**Shipped:** 2026-07-12
**Phases:** 3 (23-25) | **Plans:** 8

### What Was Built
Closed v1.4's accepted residual #5 (T2). Phase 23 authored `DESIGN-slot-type-binding.md` and cleared a fresh non-self adversarial review before any TCB code. Phase 24 threaded an additive `origin_role` mint-time tag through every mint site, added a hardcoded `expected_role()` table and an exhaustive `DenyReason::SlotTypeMismatch`, and wired a fail-closed "Step 1c" per-arg hard-Deny into `submit_plan_node` (I0/I2 precedence unchanged). Phase 25 proved it live: a held-out swapped subject↔recipient deny test through the real broker path with a genuine audit chain, a 0-bypass regression audit, and an independent bare `mailpit-verify.sh` re-run green on real Linux (309 passed/0 failed) with human milestone-close sign-off.

### What Worked
- **Parallel worktree wave-1** (25-01 + 25-02, disjoint files) merged with zero conflicts — disjoint `files_modified` assignment + one-at-a-time dispatch (avoiding `.git/config.lock` contention) is a reliable recipe.
- **Orchestrator-run T2-08 gate** (not delegated) kept the live-proof non-laundered — true exit captured before any pipe, asserted on sentinel + named counts + held-out-test presence.
- **Independent verifier caught a real close-time gap** — the sign-off was recorded to the repo only after the auto-rollup marked the phase complete, and REQUIREMENTS traceability lagged; a fresh verifier (blind to the chat) correctly tripped `gaps_found`, forcing reconciliation before close.
- **Design-gate-first discipline held** (5th milestone running) — the DESIGN doc's expected_role table matched the shipped `expected_role()` impl exactly (bar one documented sound amendment), confirmed by the integration checker's code trace.

### What Was Inefficient
- Marking the last plan complete auto-rolled ROADMAP to Complete before verification + before the sign-off was written to the artifact — creating a transient ROADMAP/REQUIREMENTS inconsistency the verifier had to catch. Recording the sign-off into the SUMMARY *before* flipping the last plan would have avoided the extra verifier round.

### Patterns Established
- **Record human sign-off into the artifact BEFORE flipping the last plan complete** — else the auto-rollup + independent-verifier sequence (correctly) trips on a repo that doesn't yet reflect the approval.
- **Worktree-run and orchestrator-run plans don't write REQUIREMENTS.md** — only main-tree sequential plans do; expect `phase.complete` to reconcile the traceability lag.

### Key Lessons
- An independent verifier that traces code (and even runs the tests itself) will catch bookkeeping/record inconsistencies a pure "does the code work" check misses — and that is exactly its value at a milestone-close gate.
- The Linux vs Mac pass-count delta (309 vs 269) is positive evidence the Linux-only kernel-enforced suite actually ran, not a discrepancy.

### Cost Observations
- Model mix: orchestrator Opus 4.8 (1M); executors + verifier + integration checker on Sonnet.
- Wave-1 executors ran in parallel worktrees (~20-22 min each); the live-Linux gate (full-workspace compile in a `rust:1` container) was the long pole.
- Notable: the independent verifier's initial `gaps_found` → reconcile → re-verify `passed` loop added one round but caught a real record inconsistency — a worthwhile trade, not a rubber-stamp.

## Milestone: v1.6 — Security Hardening (close the residuals)

**Shipped:** 2026-07-17
**Phases:** 5 (26-30) | **Plans:** 14

### What Was Built
Turned the five standing TCB-local security residuals v1.1–v1.5 documented as accepted caveats into enforced, live-proven guarantees: HARDEN-01 (fd-release demotes the session via fstat inode identity + a folded-in X-04 shared-`session_status` fix), HARDEN-02 (keyed HMAC-SHA256 audit chain + MAC'd anchor truncation/orphan detection + pending_confirmations whole-row MAC), HARDEN-03 (content-derived idempotency-key CAS making a replayed Allowed `email.send` at-most-once), HARDEN-04 (forced-Active `CreateSession` mint compiled OUT of the production binary), HARDEN-05 (`file.create` `contents` role-checked + content-sensitive under I2). Phase 30 added `scripts/verify-harden04-featureless.sh` and re-ran the whole workspace green on real Linux (331/0, 49 suites).

### What Worked
- **Design-gate-first held again** (Phase 26): the DESIGN doc cleared a fresh non-self Fable-5 review that caught a real BLOCKER (HMAC key file worker-reachable via RequestFd) before any TCB code.
- **Independent adversarial code-trace on the final diff** (Phase 29): a fresh reviewer traced all 14 claims and confirmed the CAS/chain/slot-type wiring — catching 2 stale TCB doc comments a passing verifier + green gates missed. The P26/27/28 pattern (adversarial trace finds what the verifier misses) held a fourth time.
- **Orchestrator-run, non-laundered Linux gates** with true-exit-before-pipe: caught the real value of the milestone (criterion-4's *self-skipping* test) rather than rubber-stamping a laundered pass.
- Parallel worktree waves + a warm Colima kept wall-clock down despite the container compile being the long pole.

### What Was Inefficient
- The criterion-2 command shipped in the plan (`cargo test ... a b c` without `--`) was wrong; caught at gate time. Minor, but a reminder that multi-filter cargo test needs `--`.
- Two mtime-ordering `stale`-verification snags (writing SUMMARY after VERIFICATION) needed a `touch` — a freshness-contract artifact, not a real staleness.

### Patterns Established
- **False-assurance gate as a first-class deliverable**: a negative test whose skip path is triggered by the very thing it should catch (harden04 self-skip under `--workspace` feature unification) is worse than no test; the fix is a *scoped* gate script that FAILs on the skip sentinel and requires the named test's `ok` line.
- Regression-audit sweep (`30-REGRESSION-AUDIT.md`) for `#[ignore]`/weakened-assertion drift across the milestone's prior test files.

### Key Lessons
- cfg(linux) test-blindness remains the sharpest recurring hazard: a green macOS build proves nothing about the Linux-gated tests; only the bare recipe on real Linux is authoritative.
- The expected_base "record dispatch-time HEAD, not the executor's self-reported field" worktree discipline held clean across all 4 execution waves.

### Cost Observations
- Model mix: orchestrator Opus 4.8 (1M); executors + verifier + planner (opus) + checker + integration checker + researcher on Sonnet/Opus; the independent adversarial reviewer on Fable-5.
- Sessions: 1 (single autonomous run, phase 29 → phase 30 → milestone close).
- Notable: the whole milestone (Phase 29 execute → Phase 30 plan+execute → audit → close) ran autonomously in one session; the long poles were the two live-Linux container gates.

## Milestone: v1.7 — Effect Breadth I (`process.exec` + Filesystem Breadth)

**Shipped:** 2026-07-18
**Phases:** 4 (31-34) | **Plans:** 17

### What Was Built
Gave caprun the two effect primitives a coding agent minimally needs, each routed through the unchanged plan-node → taint → executor(I2) → audit path. **`process.exec`** (Phase 32): a broker-spawned kernel-confined child (`caprun-exec-launcher` self-confines Landlock+seccomp post-fork and execs the target; the confined worker never `execve`s), stdout/stderr captured and `mint_from_exec`-minted non-stapled (rooted on a `process_exited` Event), I2-governed. **Filesystem breadth** (Phase 33): `WorkspaceRoot::write_within` (O_WRONLY|O_TRUNC existing-file-only) + `file.write` broker sink + per-session `RequestFd` count limiter + `file.write` I2 schema/sensitivity/slot-role tables. **EXEC-05 confirm-release** (Phase 34): `caprun confirm` releases a Blocked `process.exec` exactly-once via `invoke_process_exec_from_resolved` + async `confirm()` (Step-4.75 guard / Step-4.8 precheck / Step-7 dispatch). Closed with a composed 4-leg live proof (`live_acceptance_v1_7_composed.rs`, LIVE-01 true-exit-0) and a full-workspace Linux regression (LIVE-02, 391/0, no v1.0–v1.6 regression).

### What Worked
- **Design-gate-first held a seventh time** (Phase 31): the effect-breadth DESIGN doc cleared a fresh non-self Fable-5 trace that caught 1 BLOCKER + 3 MAJOR (incl. an unrealizable stateless-BPF seccomp recursion-deny → pinned the launcher Option-B model) before any TCB code.
- **The fresh non-self adversarial code-trace earned its keep an 8th time** (Phase 34): on the EXEC-05 confirm-release TCB diff it caught a real MAJOR — the pre-spawn `?` legs of `invoke_process_exec_from_resolved` propagated AFTER confirm() burned the one-shot confirmation, leaving a dangling `confirm_granted` with no terminal event (the exact P33 MAJOR-1 audit-gap class) — that the passing verifier + green Linux gates both missed. Fixed (Step-4.8 precheck + folded `process_spawn_failed` append + regression test) and re-traced APPROVED.
- **Orchestrator-owned release gates** (Phase 34, autonomous:false) genuinely executed rather than rubber-stamped even under AUTO_MODE: the Linux compile-check (D-15, true-exit-0 ×2) and the fresh Fable-5 trace (D-16) both blocked LIVE-01 until green.
- **The verifier independently re-ran the highest-risk Linux suites** (not just trusting SUMMARY logs) and flagged a real spec-vs-code deviation (the removed confirm-release mint) that was reconciled rather than silently shipped.

### What Was Inefficient
- Two mtime-ordering `stale`-verification snags again (Phase 33 VERIFICATION older than a SUMMARY) needed a `touch` — the recurring freshness-contract artifact, not real staleness.
- The confirm-release mint was authored per the plan in 34-02, then removed in 34-03 as dead ceremony — a small round-trip the design could have avoided had the "no live ValueStore at confirm time" fact been surfaced at plan time.

### Patterns Established
- **Adversarial-review-driven spec reconciliation:** when an independent trace correctly removes planned code (the dead mint), reconcile the requirement text to the shipped, validated mechanism via a recorded VERIFICATION override rather than reverting correct code or shipping a spec/code contradiction.
- **env-inheritance as a defense-in-depth debt class:** `env_clear()` every broker-spawned child (exec-child, worker) so a prompt-injected or UserTrusted command cannot surface `OPENAI_API_KEY`/`CAPRUN_SMTP_*` into minted/audited/confirmable output; the trace-drives-the-sweep pattern surfaced all three spawn sites (sidecar deferred with reasoning).

### Key Lessons
- The audit-gap MAJOR class (a terminal state written before the terminal event that justifies it) recurs across sinks (P33 file.write MAJOR-1, P34 process.exec) — the fix pattern is a pre-burn precheck + routing every failure through the terminal-event append; worth a standing checklist item for any new confirm-release sink.
- cfg(linux) test-blindness remained the sharpest hazard: the D-15 compile-check (`cargo build --tests --workspace --keep-going` on real Linux, true-exit-before-pipe) is what proves the confirm-release + env_clear paths actually compile+run, not the green macOS build.

### Cost Observations
- Model mix: orchestrator Opus 4.8 (1M); executors + verifier on Sonnet; the independent adversarial reviewers (×4: two confirm-release traces + two env_clear traces) on Fable-5; PROJECT.md evolution on the sidekick.
- Sessions: 1 (single autonomous run — execute-phase 34 → env_clear gap-closure → milestone close).
- Notable: the milestone-closing MAJOR + a new-finding env_clear sweep both surfaced from fresh adversarial traces after the phase already verified 10/10; the traces, not the gates, were the long pole on quality.

## Milestone: v1.9 — Authorized Egress + Policy & Audit Surface

**Shipped:** 2026-07-18
**Phases:** 6 (41-46) | **Plans:** 22

### What Was Built
Completed the authorized-write-egress loop and added caprun's first usability/trust-surface layer — caprun is now a design-partner-runnable slice. **Design gate (Phase 41):** one DESIGN doc pinning git.push egress + http-write egress + the policy-vs-I2 boundary (incl. POLICY-03 binding/provenance) cleared a fresh non-self orchestrator-owned adversarial code-trace before any TCB code. **Policy layer (Phase 42):** a minimal hardcoded-schema (NOT Cedar) per-session policy that narrows WHICH sinks/args are callable with a DISTINCT `policy_deny` outcome, broker-bound at session creation from a trusted source outside the worker's reach (F1 containment), immutable, hash-chained `policy_bound` event — a pre-I2 narrowing gate that can NEVER override I2 (POLICY-02 by construction; I2 stays hardcoded in the Rust TCB). **`http.request.write` (Phase 43):** a distinct CommitIrreversible POST/PUT sink (the I0-escape fix), taint-governed body/url under I2, exact {POST,PUT} method-enum gate, distinct fail-closed `WRITE_HOST_ALLOWLIST` reusing the shipped SSRF resolve-and-pin, broker-env-only credential, opaque non-minting audit, confirm-release. **`git.push` (Phase 44) — SHIPPED, not deferred a 3rd time:** a fully-unprivileged broker-performed smart-HTTP transfer (info/refs GET + git-receive-pack POST over the shipped reqwest-ring resolve-and-pin, IP frozen across both requests, redirect refused), pack-gen child kept net-denied, remote/refspec from TRUSTED intent, force/delete hard-denied by construction, broker-env credential scrubbed, ALWAYS confirm-gated with an anti-TOCTOU frozen-oid, ZERO new crates. **Thin CLI/SDK + audit viewer (Phase 45):** `caprun run [--policy]` binds the trusted policy at session creation + surfaces the blocked effect_id on an I2 Block, with the M7 anti-laundering fix (file-derived content minted TAINTED via `mint_from_read`, not laundered); `caprun audit` is a read-only, load-only fail-closed viewer surfacing `verify_chain`, neutralizing every displayed literal (hardened to the Trojan-Source BiDi/zero-width class, CVE-2021-42574). **Composed live proof (Phase 46):** the full authorized-write loop over ONE shared audit.db through the REAL broker arms, inspected via a genuine `caprun audit` subprocess + a genuine `caprun run` Block leg, plus 5 independently-attributable negative legs — independent compose-verify 696/0 on real Linux, no v1.0–v1.8 regression.

### What Worked
- **Design-gate-first held an eighth+ milestone running, and the orchestrator-owned fresh non-self Fable-5 adversarial code-trace caught a real defect at EVERY TCB phase** — the I0-escape BLOCKER at P41, the ArgConstraint boundary bypass at P42, the M7 laundering path (pre-execution) at P45, a LIVE decision-surface BiDi confirm-prompt spoof at P45, and the leg-5b vacuity at P46. The guardrail is load-bearing, not ceremony.
- **The plan-checker gate caught the M7 anti-laundering BLOCKER before any code was written** — the SDK binding a trusted policy risked becoming a laundering site; caught at plan time, fixed by minting file-derived content TAINTED via `mint_from_read` (no new mint site).
- **Sequential-on-main for TCB caught inter-plan bugs** that parallel worktrees would have hidden — the write-egress and policy TCB paths share enough surface that one-at-a-time on main was the right posture.
- **The FULL compose-verify as the authoritative Linux gate** was decisive at P44: scoped runs hid two cfg-linux-blindness defects that only the full compose-verify surfaced.
- **Requirement-review-before-roadmap** (matt-essentialist + Fable-5 converged on POLICY-03 — where policy comes from / how it binds) shaped the milestone before phases were cut, so the policy-vs-I2 boundary was pinned as its own requirement rather than discovered mid-build.
- **`git.push` shipped rather than deferring a 3rd time** — the v1.8 BLOCKER-1 (seccomp can't pin a `connect()`) was genuinely resolved by moving egress to the broker application layer that SEES the destination; the design-gate's flagged trust-posture change was designed, reviewed, built, and live-proven this milestone.

### What Was Inefficient
- **cfg(linux)-test-blindness re-hit** (the recurring sharpest hazard): scoped test runs on P44 passed while hiding two Linux-only defects, forcing full-compose-verify reruns to surface them — the full recipe on real Linux remained the only authoritative signal.
- Framing-honesty on LIVE-05 required an explicit ratification round (the hybrid-composition call) rather than being decidable purely mechanically — a small but real coordination cost, correctly spent rather than papered over.

### Patterns Established
- **Always-confirm-gate a sink whose payload is not in its I2 args.** `git.push` has no tainted payload in its `{remote, refspec}` I2 args (the payload is the pack), so a clean Allowed decision synthesizes a `BlockedPendingConfirmation` with a MAC'd frozen-new-oid pending row — the human always sees the pushed range before it leaves. No auto-dispatch arm.
- **Broker-performed egress with an app-layer destination pin frozen across requests.** When seccomp cannot pin a child's `connect()`, move the egress into the broker (which sees the destination), resolve-and-pin the IP, and FREEZE that IP across every request of a multi-request exchange (info/refs GET + receive-pack POST) so a redirect/rebind cannot retarget it.
- **Hybrid-composition + blunt framing-honesty for an acceptance proof** (v1.3 DOC-01 lineage): when a shipped CLI drives single-node intents but the acceptance clause says "driven via the CLI," meet it honestly with the real broker arms + a genuine CLI leg + genuine CLI inspection, machine-check against the overclaim, and disclose — rather than building new planner/TCB scope just to satisfy the literal phrasing.

### Key Lessons
- **The fresh adversarial trace found a LIVE confirm-prompt BiDi spoof and a laundering path that all green gates missed** — a viewer/decision-surface that renders attacker-tainted literals is itself an attack surface (Trojan-Source, CVE-2021-42574); the guardrail catching these at every TCB phase is why it stays mandatory, not optional.
- **A "driven via the CLI" acceptance clause can be met honestly via the real broker arms without building new planner code** — the honest hybrid + a machine-checked no-overclaim assertion is stronger than either a literal-but-hollow single-`caprun run` driver or a silent scope expansion.
- The terminal-state-before-terminal-event audit-gap discipline (P33/P34) applied cleanly to every new confirm-release sink this milestone (http.request.write, git.push) — a standing checklist item that keeps paying off.

### Cost Observations
- Model mix: orchestrator Opus 4.8 (1M); gsd-executors on Sonnet; the independent adversarial reviewers on Fable-5.
- Sessions: 1 (single autonomous run across all 6 phases through milestone close).
- Notable: `git.push` shipped this milestone (the v1.8 gate-deferred sink) — the design-gate's flagged new trust posture was designed, reviewed, implemented, and live-proven in one pass, with the fresh Fable-5 trace clearing 8 push surfaces at 0 security defects.

## Cross-Milestone Trends

| Milestone | Phases | Plans | Shipped | Notes |
|-----------|--------|-------|---------|-------|
| v1.0 MVP  | 4      | 15    | 2026-06-30 | v0 DONE — genuine-taint §9 gate cleared |
| v1.1 Usable Runtime | 3 | 15 | 2026-07-01 | Live §9 from the CLI — real `file.create` sink, DB-durable taint chain, Linux-verified |
| v1.2 Tainted Session, Human Gate | 4 | 11 | 2026-07-07 | Draft-only session demotion (I1/I0) + single-shot confirmation loop, live-proven on real Linux; independent live-run re-verification now a confirmed 2/2 pattern |
| v1.3 Doc → Action Assistant | 6 | 21 | 2026-07-09 | Doc→action hero demo: genuine-taint extraction, collect-then-Block, full-set confirm binding, a real live email send, closed exfiltration path, composed confirm/deny/clean live acceptance, honest DOC-01 framing; independent live-run re-verification now 4/4, across two independent parties |
| v1.4 Trust-Boundary Integrity & Adversarial Planner | 5 | 14 | 2026-07-11 | Fixed a live cross-connection trust bypass (one-way occupancy latch); real `Planner` trait + broker capability split; genuine OpenAI-backed adversarial planner in an isolated sidecar; HARD GATE proved the boundary holds regardless of planner intelligence; closing full-recipe re-run caught a real Cargo artifact-placement bug |
| v1.5 Slot-Type Binding Enforcement (T2) | 3 | 8 | 2026-07-12 | Closed v1.4's T2 residual: fail-closed Step 1c slot-type check in the TCB; held-out swapped-handle deny test with genuine audit chain; 0-bypass regression audit; independent bare `mailpit-verify.sh` green on real Linux (309/0); independent verifier caught a close-time sign-off/traceability record lag before allowing close |
| v1.6 Security Hardening (close the residuals) | 5 | 14 | 2026-07-17 | Enforced all 5 standing TCB-local residuals (fd-demote, keyed-MAC audit chain, replay CAS, compile-out forced-Active mint, file.create contents slot) + folded X-04; independent adversarial code-trace APPROVED the diff (found 2 stale TCB comments the verifier missed); closed a real false-assurance gap (harden04 self-skip) with a scoped featureless-build gate; full workspace green on real Linux (331/0, 49 suites); milestone audit PASSED 8/8, 5/5 seams; ran end-to-end autonomously in one session |
| v1.7 Effect Breadth I | 4 | 17 | 2026-07-18 | Added `process.exec` (broker-spawned confined-child sink, non-stapled `mint_from_exec` rooted on `process_exited`) + filesystem read/write breadth (`file.write` + RequestFd limiter) + EXEC-05 confirm-release; a fresh Fable-5 trace caught the guardrail's 8th real defect (confirm-release audit-gap MAJOR the verifier+gates missed), fixed + re-traced APPROVED; verifier independently re-ran Linux suites + flagged a spec deviation (reconciled via override); post-close env_clear gap-closure (exec-child + worker broker-secret inheritance) fixed + APPROVED, sidecar deferred to v1.8; LIVE-01 composed 4-leg + LIVE-02 391/0 on real Linux; human DONE sign-off + push |
| v1.9 Authorized Egress + Policy & Audit Surface | 6 | 22 | 2026-07-18 | Completed the authorized-write-egress loop + the first trust-surface layer: a minimal per-session policy (narrows sinks/args, NEVER overrides I2 — POLICY-02 by construction), `http.request.write` POST/PUT, **`git.push` SHIPPED not deferred a 3rd time** (broker-performed destination-pinned smart-HTTP, net-denied child, always confirm-gated, zero new crates), and a thin `caprun run`/`caprun audit` CLI + read-only audit-DAG viewer (M7 anti-laundering + Trojan-Source BiDi neutralization); the orchestrator-owned fresh Fable-5 adversarial trace caught a real defect at EVERY TCB phase (I0-escape, ArgConstraint bypass, M7 laundering, LIVE BiDi confirm-prompt spoof, leg-5b vacuity) + the plan-checker caught the M7 BLOCKER pre-code; LIVE-05 composed loop + 5 attributable negative legs, independent compose-verify 696/0 on real Linux, no v1.0–v1.8 regression; single autonomous session |
