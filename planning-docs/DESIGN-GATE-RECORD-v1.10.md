# DESIGN GATE RECORD — v1.10 (Multi-step Plan Stream)

**Phase:** 47 — Multi-step Plan Stream Design Gate
**DESIGN doc under review:** `planning-docs/DESIGN-multi-step-plan-stream.md`
**Requirements gated:** DESIGN-19 (pin multi-step TCB mechanisms), DESIGN-20
(clear a fresh non-self adversarial code-trace before any multi-step TCB code),
HYG-02 (zero new crates default / Gate 1+3 / check-invariants / compose-verify)
**Status:** ✅ **CLEARED** (Round-1 amendments) — Phases 48–52 multi-step TCB /
worker submit / confirm-hold code authorized under the pins in the DESIGN.
**Date:** 2026-07-27

## Gate discipline (standing precedent, unbroken v1.0 P2 → v1.9 P41 → v1.10 P47)

No multi-step change under `crates/{executor,brokerd,sandbox,runtime-core}` or the
worker submit / confirm-hold path in `cli/caprun` may land until this DESIGN doc
clears a **fresh, non-self, ORCHESTRATOR-owned** adversarial code-trace. The
orchestrator (not a gsd-executor — gsd-executors have no Agent tool) owns the
review spawn and the finding-fold. The doc was authored by a gsd-executor
(Plan 47-01); the review was spawned by the orchestrator against a genuinely
fresh reviewer with no authoring involvement.

A gsd-executor self-read of the DESIGN is **not** DESIGN-20 clearance.

## Reviewer identity and independence

| Field | Value |
|-------|-------|
| Reviewer agent | Independent explore-class code-trace subagent (`gsd-explore` / read-only), subagent_id `019fa3e7-7937-7831-bed8-031376869660` |
| Authoring context | Separate prior `gsd-executor` for Plan 47-01 (commits `0ef4ee1`, `976b830`, `d4c72fa`) |
| Independence | **author ≠ reviewer**; fresh context; first-pass code-trace — not a prose skim of the DESIGN alone |
| Model/runtime | Grok/xAI orchestrated spawn; explore agent with read-only capability |
| Effort | ~50 tool calls / ~189s wall; full DESIGN + mandatory live trees opened |
| Orchestrator role | Spawned reviewer; independently re-verified each finding against live code before fold; wrote this record |

### Files the reviewer opened (code-trace, not prose-only)

- `planning-docs/DESIGN-multi-step-plan-stream.md` — document under review
- `cli/caprun/src/planner.rs` — PLAN-03 / `task_instruction`
- `cli/caprun/src/worker.rs` — one-shot submit; discard `output_value_id`
- `crates/brokerd/src/proto.rs` — `SubmitPlanNode` / `PlanNodeDecision` / ProvideIntent
- `crates/brokerd/src/server.rs` — multi-request loop; occupancy latch; ProvideIntent-once; evaluate; git.push always-confirm; output mints
- `crates/brokerd/src/confirmation.rs` — confirm MUST NOT re-`submit_plan_node`; precheck-before-burn
- `crates/runtime-core/src/plan_node.rs` — handle-only `PlanArg` / `PlanNode`
- `crates/runtime-core/src/executor_decision.rs` — Allowed / Block / Denied{PolicyDeny}
- `crates/runtime-core/src/intent.rs` — closed `CaprunIntent`
- `crates/executor/src/sink_sensitivity.rs` — effect-class table
- `crates/executor/src/lib.rs` — policy pre-I2; I2; Step 0.5 Draft×CommitIrreversible
- `crates/executor/src/policy_gate.rs` — deny-only gate; no skip-I2
- `crates/brokerd/src/policy.rs` — bind-once immutability
- `crates/brokerd/src/quarantine.rs` — `mint_from_exec` / `mint_from_http` always untrusted
- `scripts/check-invariants.sh` — Gate 1 EffectRequest; Gate 3 mint loci
- `planning-docs/DESIGN-plan-executor.md` — PLAN-03
- `planning-docs/DESIGN-confirmation-release.md` — confirm MUST NOT re-invoke submit
- `cli/caprun/tests/live_acceptance_v1_9_composed.rs` — LIVE-05 hybrid honesty class (spot)

## Revision history

| Round | Date | Reviewer | Findings by severity | Result |
|-------|------|----------|----------------------|--------|
| 1 | 2026-07-27 | Independent explore code-trace (orchestrator-spawned) | 0 BLOCKER, 0 MAJOR, 2 MINOR, 1 NIT | CLEARED after Round-1 fold |

## Findings and resolutions

| # | Sev | Claim | Code evidence (re-verified by orchestrator) | Resolution |
|---|-----|-------|-----------------------------------------------|------------|
| F-01 | MINOR | Intermediate `output_value_id` described as process.exec-only | **CONFIRMED:** `server.rs:1274-1299` process.exec; `:1308-1332` git.commit; `:1343-1401` http.request. Stale comments: `worker.rs:376-381`, `proto.rs:242-253`, `server.rs:2257-2259` | **DESIGN §2.2 tightened:** bag stores **any** `Some(output_value_id)`; stale comments are drift not authority. No laundering hole (all mints untrusted). |
| F-02 | MINOR | Confirm-released intermediate mint never enters worker bag | **CONFIRMED:** `confirmation.rs:819-833`, `:1204-1217` — no mint, no live ValueStore, no re-submit | **DESIGN §2.2 tightened:** post-confirm outputs out-of-bag; re-submit/remint forbidden without new design gate (preserves surface-5 rejection). |
| F-03 | NIT | Gate 1 cited as `check-invariants.sh:7-31` | **CONFIRMED:** Gate 1 body is `:29-36` | **DESIGN §8.1 citation fixed** |

**BLOCKER count:** 0  
**MAJOR count:** 0  
No open blockers. Gate clears on Round 1.

## Verified as sound (10 Phase-47 attack surfaces)

All ten RESEARCH / PLAN pressure surfaces were code-traced SOUND:

1. **Cross-node taint laundering via `output_value_id`** — broker mints always untrusted (`quarantine.rs` mint_from_exec / mint_from_http); planner cannot mint/strip; each node re-runs full I2.
2. **ProvideIntent reopened mid-stream** — `intent_provided` / `fd_requested` guards (`server.rs:2370-2391`); occupancy latch prevents reconnect remint (`server.rs:253-270`).
3. **Draft demotion fixed by weakening CommitIrreversible Step 0.5** — Step 0.5 remains (`executor/src/lib.rs:255-276`); DESIGN rejects class weaken.
4. **Batch authorize / EffectRequest / free-form effect path** — only `SubmitPlanNode`; Gate 1 EffectRequest ban (`check-invariants.sh:29-36`).
5. **Mid-loop confirm Session split / reconnect-remint / session-wide waiver** — confirm never re-`submit_plan_node` (`confirmation.rs:816-821`); single-shot; occupancy latch.
6. **Instruction channel collapsed into bindable ValueId** — `task_instruction: Option<String>` never ValueId (`planner.rs:64-79`).
7. **Policy mid-stream rebind or I2 override** — policy bind-once (`server.rs:284-301`); policy_gate deny-only before I2 (`policy_gate.rs:8-19`).
8. **Hybrid composition framed as CLI multi-step DONE** — DESIGN §14 + LIVE-05 honesty framing; DONE requires real multi-node `caprun run`.
9. **New mint site outside Gate 3** — Gate 3 allowlist; DESIGN default zero new mint sites.
10. **P33/P34 confirm-release under multi-confirm Session** — precheck-before-burn per effect_id; sequential product path at most one mid-loop hold at a time.

## Re-run triggers (DESIGN-20 idempotency)

The adversarial code-trace **MUST re-run** (new fresh non-self review + gate
record amendment — **not** silent re-use of this CLEARED) if any of the
following pivot mid-implementation of Phases 48–52:

1. **Stream shape** — batch authorize, free-form effect path, multi-connection
   product path, or non-sequential multi-node authorize.
2. **Confirm-hold mechanism** — reconnect-remint resume, dual-Session stitch,
   session-wide confirm waiver, or confirm re-invoking `submit_plan_node`.
3. **Trusted-arg mint path** — mid-stream ProvideIntent, second UserTrusted mint
   verb, or post-confirm remint into the worker bag without a design-gated path.

Standing per-phase TCB-diff adversarial discipline for Phases 48–52 remains the
next guardrail layer.

## No-TCB-code reconfirmation

At gate clear (2026-07-27):

- `git status --porcelain -- crates cli` → **empty**
- `scripts/check-invariants.sh` → **all gates PASSED**
- Multi-step mechanism names (`handle bag`, sequential stream loop, Block-and-Hold
  product driver) appear **only** in `planning-docs/` prose — not as landed
  multi-step TCB / worker-loop implementation under `crates/` or `cli/`
- Git diff for Phase 47 touches planning docs (and `.planning/` summaries) only

## Outcome

- **DESIGN-19:** ✅ the doc pins plan-stream shape, handle bag, Block-and-Hold,
  I1×coding-loop bounds, instruction vs value channels, deny/abort mid-stream,
  carry-forwards, HYG-02, threat model, and DESIGN-20 declaration.
- **DESIGN-20:** ✅ cleared a fresh non-self orchestrator-owned adversarial
  code-trace (0 BLOCKER, 0 MAJOR, 2 MINOR, 1 NIT — all folded Round-1 and
  orchestrator-re-verified against live code).
- **HYG-02:** ✅ reconfirmed — zero new crates default; Gate 1/3; check-invariants
  green; compose-verify remains authoritative Linux gate; no TCB code this phase.
- **Gate:** ✅ **CLEARED.** Phases 48–52 multi-step TCB / worker submit /
  confirm-hold code authorized **subject to** the DESIGN pins and re-run triggers
  above. No multi-step TCB code was written during Phase 47.

## Verdict

**APPROVE / CLEARED** — Phase 47 design gate is closed. Multi-step implementation
may proceed from Phase 48 under the locked decisions in
`planning-docs/DESIGN-multi-step-plan-stream.md`.

## Gate: audit append-at-head concurrency (Phase 51 gap closure)

**DESIGN doc under review:** `planning-docs/DESIGN-audit-append-concurrency.md`  
**Requirements gated:** LIVE-07, LIVE-08, and the standing DESIGN-20 fresh-non-self-adversarial-trace obligation  
**Status:** ⏳ **PENDING** — no change under `crates/brokerd/src/audit.rs`, `server.rs`, or `confirmation.rs` for this defect class may be declared cleared until an orchestrator-owned fresh non-self adversarial code-trace runs against the landed diff.

This entry authorises no self-clearance. A gsd-executor self-read is not clearance; Plan 51-08 owns the independent trace and any resulting findings, date, and verdict.

**Implementation amendment (2026-08-05, user-authorised):** the autocommit branch of `append_event` may use `Transaction::new_unchecked(conn, TransactionBehavior::Immediate)` so the locked public `append_event(&Connection, ...)` signature is preserved. The explicit `is_autocommit` re-entrancy branch remains required, and the three mutable enclosing sites still use `transaction_with_behavior(TransactionBehavior::Immediate)`. This amendment does not clear the PENDING independent-review gate or weaken any verifier, provenance, MAC, or append-at-head invariant.

### Reviewer identity and independence

| Field | Value |
|---|---|
| Reviewer agent | TBD |
| Authoring context | TBD |
| Independence | TBD |
| Model/runtime | TBD |
| Effort | TBD |
| Orchestrator role | TBD |

### Revision history

| Round | Date | Reviewer | Findings by severity | Result |
|---|---|---|---|---|
