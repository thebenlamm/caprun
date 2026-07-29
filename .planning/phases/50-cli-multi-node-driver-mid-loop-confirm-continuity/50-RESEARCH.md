# Phase 50: CLI Multi-node Driver & Mid-loop Confirm Continuity - Research

**Researched:** 2026-07-29
**Domain:** Product CLI driver for multi-node coding stream + Block-and-Hold same-Session confirm continuity (Rust TCB; zero new crates)
**Confidence:** HIGH

## Summary

Phase 50 is the **product surface** that makes Phase 48/49 real for a design partner. Phase 49 already shipped `CaprunIntent::SafeCodingWorkflow` (13 operator fields), ProvideIntent multi-mint → `named_handles`, worker bag seed (no RequestFd demotion), and `DeterministicPlanner::plan_next` emitting `file.write → process.exec → git.commit → git.push → github.pr`. Phase 48 shipped sequential N× `SubmitPlanNode`, opaque bag, and fail-closed Block/Deny branches. What is **missing** is entirely in `cli/caprun`: (1) `main.rs` still fail-closes on unknown intent kinds (email/file only), (2) worker on `BlockedPendingConfirmation` **exits 1** with no hold, (3) main **wait-then-abort-broker** treats any non-zero worker exit as terminal, and (4) exit codes do not distinguish success / blocked / denied.

The load-bearing constraint is **Block-and-Hold same Session**. Always-confirm `git.push` rewrites clean Allowed → `BlockedPendingConfirmation` before dispatch, so the coding **success path always mid-loop Blocks at push**. Confirm executes the sink from the durable snapshot and **must not** re-submit the blocked node. Remaining nodes (at least `github.pr`) continue only after Allowed release, on the **same** Session id, policy bind, audit chain, and worker connection occupancy. Reconnect-remint and dual-Session stitch are **rejected**.

**Primary recommendation:** Extend `caprun run` with a `safe-coding-workflow` intent kind that builds `CaprunIntent::SafeCodingWorkflow` from a JSON intent file (13 fields), reusing existing `--policy` / POLICY-03 bind-once. Change the worker Block branch from exit-1 to **stay-connected hold** (signal `effect_id` + wait for parent proceed/abort). Change main from fire-and-forget `child.wait()` to **orchestrate mid-loop confirm** (in-process `confirmation::confirm`/`deny` preferred; dual-terminal documented) without aborting the broker until the stream ends. Map stream terminals to distinct exit codes. Preserve email/file single-node paths and existing `review`/`confirm`/`deny`/`grant`/`audit` verbs.

<user_constraints>
## User Constraints

> No `*-CONTEXT.md` for this phase (discuss-phase skipped under `--auto`). Locked authority is derived from REQUIREMENTS (CLI-01/02, CONFIRM-01), `planning-docs/DESIGN-multi-step-plan-stream.md` (CLEARED), ROADMAP Phase 50 success criteria, Phase 49 shipped coding planner + trusted mint, Phase 48 stream substrate, and CLAUDE.md hard constraints.

### Locked Decisions (must honor — do not re-litigate)

1. **CLI-01:** `caprun run` (or documented sibling verb) accepts coding multi-step intent + workspace + trusted `--policy`, binds policy at session creation (POLICY-03), drives full multi-node coding chain end-to-end. Existing Block → `review`/`confirm`/`deny`/`grant` surfaces preserved and pointed at. [VERIFIED: `.planning/REQUIREMENTS.md`]
2. **CLI-02:** Stream stop semantics honest and machine-checkable: I2 Block → stop or Block-and-Hold, surface `effect_id` + review pointer; `policy_deny` → distinct outcome; Deny → abort remaining; full success → clear success exit; exit codes distinguish success vs blocked vs denied/aborted; silent continue-past-Block **forbidden**. [VERIFIED: REQUIREMENTS.md]
3. **CONFIRM-01:** Mid-stream `BlockedPendingConfirmation` holds the **same Session**: worker stays connected or designed same-Session resume that does **not** re-open ProvideIntent, re-bind policy, or mint new trusted values. Human confirm/deny acts on durable pending row; remaining nodes continue only after Allowed release (or abort on deny). No dual-Session stitch; no session-wide confirm waiver. [VERIFIED: REQUIREMENTS.md]
4. **Block-and-Hold (DESIGN §3):** Worker stays connected (or design-locked same-Session hold that does not exit occupancy latch / remint). Confirm acts on durable `PendingConfirmation`; worker **MUST NOT re-submit** blocked node; remaining nodes under same Session id, same policy bind, same audit chain. [CITED: DESIGN-multi-step-plan-stream.md §3.1]
5. **Always-confirm `git.push` is first-class mid-loop hold** — coding success path **will** Block at push even without taint. [VERIFIED: `crates/brokerd/src/server.rs:807-848`; CITED: DESIGN §3.2]
6. **Deny / `policy_deny` → abort remaining** fail-closed; Block holds; sequential order only. Exit-code taxonomy detail is this phase's job; abort semantics already locked. [CITED: DESIGN §6]
7. **ProvideIntent exactly once**; post-confirm intermediate outputs **out of bag**; no re-submit of blocked node (F-02). [CITED: DESIGN §2.2–2.3]
8. **POLICY-03 bind once** at session create from trusted path outside worker; immutable; not re-bound mid-stream. [VERIFIED: `cli/caprun/src/main.rs:376-396` bind_policy]
9. **HYG-02:** Zero new crates; no `EffectRequest`; Gate 3 mint list unchanged; `check-invariants.sh` green. [CITED: DESIGN §8 / HYG-02]
10. **Phase boundaries:** Phase 50 = CLI multi-node driver + confirm continuity. **Not** LIVE-07/08 non-hybrid proof (51), **not** packaging (52), **not** LLM multi-step, **not** new sinks/crates.

### Claude's Discretion (recommend in plan)

- Exact coding intent argv shape (JSON file vs 13 positionals vs flags). **Recommend JSON intent file** as the single positional after kind.
- Interactive-in-run confirm vs dual-terminal only. **Recommend interactive-in-run as primary** (main calls `confirmation::confirm`/`deny` in-process after Block signal) with dual-terminal documented as alternate when stdin is non-TTY / `CAPRUN_CONFIRM=external`.
- Worker↔main hold protocol (stdin line protocol vs broker poll verb vs side UDS). **Recommend parent-pipe stdin/stdout machine lines** — no new broker IPC verb; stays inside same-Session constraint.
- Exit-code integers (exact mapping). **Recommend table below** (0/2/3/1).
- Whether to auto-print `caprun grant` pointer at coding session start (github.pr grant gate). **Recommend yes** — print `session_id` + grant command before/at first Block; do not auto-grant (grant is a distinct human capability).
- Whether email/file Block still exits non-zero immediately (no multi-node hold). **Recommend preserve single-node stop-and-surface** for email/file; full hold only for multi-node streams / coding (or any stream that has remaining `plan_next` steps).

### Deferred Ideas (OUT OF SCOPE)

- Phase 51 LIVE-07/08 non-hybrid live proof on real Linux (compose-verify proof family)
- Phase 52 packaging
- LLM multi-step / ReAct (LLM-MS-01)
- New crates, EffectRequest path, session-wide confirm waiver, dual-Session stitch
- github.pr merge/comment, replan-from-observation (CODE-BREADTH-01)
- Batch DAG authorize, reconnect-remint `caprun continue`, auto-confirm mid-loop
</user_constraints>

## Project Constraints (from CLAUDE.md)

Treat with the same authority as locked DESIGN decisions:

1. **Source of truth:** `planning-docs/PLAN.md` wins on doc/code conflicts.
2. **Effect path locked:** `submit_plan_node(session_id, PlanNode { sink, args: ValueIds })` only — never raw `EffectRequest`. Gate 1 fails if `EffectRequest` appears under `crates/`.
3. **I0 / I1 / I2:** I2 hardcoded in Rust executor; policy never disables I2; untrusted seed → draft-only; no ambient authority for workers.
4. **Terminology locked:** Intent, Session, Planner, Worker, Broker, Adapter, Effect, Artifact, Event. Project/binary = `caprun`.
5. **TCB is Rust.** Linux-only security claims; macOS stubs expected (`0 passed` on macOS for cfg-gated tests is expected).
6. **From Phase 16+:** Linux verification that may touch SMTP uses `scripts/mailpit-verify.sh`; full composed LIVE uses `scripts/compose-verify.sh`. Never bare `docker run rust:1` alone when SMTP may fire. Never bind named Docker volumes for `CARGO_TARGET_DIR` as a manual speed hack.
7. **v0/v1 DONE lineage:** substrate working ≠ done. v1.10 DONE = non-hybrid CLI multi-node LIVE (Phase 51), not hybrid rebrand. Phase 50 is the product path Phase 51 will drive.
8. **Out of scope:** agent frameworks, memory, marketplace, Cedar, web UI, cross-host/Biscuit, gVisor/Firecracker, LLM multi-step until relevant gates.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `caprun run` (or sibling) accepts coding multi-step intent + workspace + trusted `--policy`, binds policy at session creation (POLICY-03), drives full multi-node coding chain; preserves review/confirm/deny/grant surfaces | §§ Architecture Patterns 1–2, Standard Stack, Code Examples (intent argv + policy bind), Don't Hand-Roll |
| CLI-02 | Honest machine-checkable stop semantics + exit codes: Block → hold/stop + effect_id + review pointer; policy_deny distinct; Deny abort remaining; success clear; no silent continue-past-Block | §§ Exit-code design, Pattern 3 (branch table), Validation Architecture, Common Pitfalls |
| CONFIRM-01 | Mid-stream Block-and-Hold same Session; worker stays connected or designed same-Session resume; no ProvideIntent remint / policy rebind / dual-Session; remaining nodes only after Allowed release | §§ Block-and-Hold design options, Pattern 2, Architecture diagram, Security Domain, Common Pitfalls |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Coding intent argv → `CaprunIntent::SafeCodingWorkflow` | CLI main (`cli/caprun`) | `runtime-core` types (already shipped) | Product construction of closed enum; no new type work |
| POLICY-03 bind once + `policy_bound` event | CLI main | `brokerd::policy::bind_policy` | Already shipped for email/file; reuse for coding |
| Session create + broker lifetime | CLI main | `brokerd::server::run_broker_server` | Main owns broker task; must **not** abort broker until stream fully ends (hold requires broker alive) |
| Sequential plan stream + bag | Worker (Phase 48/49) | Planner `plan_next` | Already shipped; Phase 50 changes Block branch only |
| Per-node policy pre-I2 + I2 + always-confirm push | Broker + executor TCB | — | Unchanged; product path consumes decisions |
| Block-and-Hold wait / proceed | Worker + main orchestrator | Durable `pending_confirmations` | Worker stays connected; main confirms durable row |
| Confirm / deny / review / grant product verbs | CLI main (existing) | `brokerd::confirmation` / audit | Preserve; point at from driver; grant required before successful github.pr |
| Exit-code taxonomy | CLI main (+ worker exit mapping) | Stream terminal enum | CLI-02 machine-checkable outcomes |
| LIVE multi-node proof | compose-verify / CLI | — | **Phase 51** — Phase 50 must leave runnable product path |

## Standard Stack

### Core (reuse only — zero new crates / packages)

| Library / artifact | Version / locus | Purpose | Why Standard |
|--------------------|-----------------|---------|--------------|
| Rust edition 2021, workspace resolver 3 | root `Cargo.toml` | TCB language | Locked project stack [VERIFIED: Cargo.toml + `cargo 1.97.1` / `rustc 1.97.1`] |
| `cli/caprun` main | `cli/caprun/src/main.rs` | Intent argv, policy bind, spawn worker/broker, confirm UX, exit codes | Primary product surface to **extend** [VERIFIED: main.rs] |
| `cli/caprun` worker | `cli/caprun/src/worker.rs` | Stream loop + bag; Block branch → hold | Hold lives here [VERIFIED: worker.rs:414-443] |
| `cli/caprun` planner | `cli/caprun/src/planner.rs` | `plan_coding_next` five-node recipe | Already CODE-01 complete [VERIFIED: planner.rs:247+] |
| `runtime-core` | workspace | `CaprunIntent::SafeCodingWorkflow`, `ExecutorDecision`, `DenyReason` | Shipped Phase 49 [VERIFIED: intent.rs:65-80] |
| `brokerd` | workspace | ProvideIntent multi-mint, always-confirm push, confirm/deny, grant, policy | Shipped; no new mint sites [VERIFIED: server.rs, confirmation.rs] |
| `scripts/check-invariants.sh` | repo | Gates 1–6 | HYG-02 [VERIFIED: scripts] |
| `scripts/mailpit-verify.sh` / `compose-verify.sh` | repo | Linux authority | CLAUDE.md Phase 16+ [CITED: CLAUDE.md] |

### Supporting

| Artifact | Purpose | When to Use |
|----------|---------|-------------|
| `planning-docs/DESIGN-multi-step-plan-stream.md` | Authoritative pins (§3 hold, §6 abort) | Every task decision |
| `planning-docs/DESIGN-confirmation-release.md` | Confirm never re-submits blocked node | Hold proceed semantics |
| Phase 48 `cli/caprun/tests/stream_substrate.rs` | `drive_stream` branch table | Extend for HoldContinue / HoldAbort |
| Phase 49 `cli/caprun/tests/planner.rs` | Coding emission + anti-launder | Regression only |
| `cli/caprun/tests/confirm.rs` / `grant.rs` | Cross-process confirm/grant | Dual-terminal + grant integration |
| `cli/caprun/tests/e2e.rs` | Real binary email path | Email/file regression; coding e2e sibling |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Stay-connected hold + parent pipe | Exit worker + reconnect / `caprun continue` | **REJECTED** — occupancy latch one-way, empty ValueStore, ProvideIntent laundering [CITED: DESIGN §3.3] |
| Dual-Session stitch after push | Same Session hold | **REJECTED** — splits audit/policy [CITED: DESIGN §3.3] |
| Auto-confirm mid-loop | Human confirm per effect | **REJECTED** — defeats always-confirm + I2 human gate |
| New broker `WaitForConfirm` IPC | Parent stdin/stdout protocol | New IPC surface unnecessary; main already holds audit DB + can call confirm in-process |
| 13 positional argv fields | JSON intent file | Positionals brittle; JSON reuses serde of CaprunIntent |
| New clap CLI crate | Hand-rolled argv (existing) | HYG-02 / project convention [CITED: research/STACK.md] |
| Session-wide confirm waiver | Per-effect single-shot | **REJECTED** permanently |

**Installation:** none — **zero** external packages.

**Version verification:** No new packages. Host toolchain `cargo 1.97.1` / `rustc 1.97.1` [VERIFIED: local].

## Package Legitimacy Audit

> Phase 50 installs **zero** external packages (HYG-02 continues). Package Legitimacy Gate **N/A**.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| *(none)* | — | — | — | — | — | No installs |

**Packages removed due to [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
Operator
  │  caprun run [--policy P] safe-coding-workflow <intent.json> <workspace-file> [audit.db]
  ▼
Main (orchestrator) — Phase 50 primary touch
  1. Parse coding intent JSON → CaprunIntent::SafeCodingWorkflow { 13 fields }
  2. bind_policy(--policy | CAPRUN_POLICY | broker_default)  [POLICY-03, once]
  3. create_session(TrustedArg) + policy_bound event
  4. Print session_id + grant pointer (github.pr later)
  5. spawn broker (run_broker_server) — KEEP ALIVE for entire hold window
  6. spawn worker with piped stdin/stdout + INTENT env (serde CaprunIntent)
  7. Loop orchestration:
       read worker stdout line
         "NODE_ALLOWED step=N sink=…" → continue
         "BLOCKED effect_id=E sink=S" → surface review/confirm/deny pointers
              interactive: review display + confirm()/deny() in-process
              external: poll pending until terminal (dual-terminal)
              on Released → write "PROCEED\n" to worker stdin
              on Denied   → write "ABORT\n"  to worker stdin
         "DENIED reason=policy_deny|…" → map exit 2; wait worker; abort broker
         "STREAM_DONE" → wait worker exit 0
  8. print audit DAG + verify_chain; map exit codes
  ▼
Worker (confined) — Phase 50 Block branch change only
  connect → confine → ProvideIntent ONCE → bag seed (coding: named_handles)
  loop plan_next → SubmitPlanNode:
    Allowed → bag out_{step}; step++; continue; emit NODE_ALLOWED
    BlockedPendingConfirmation { anchors } →
        effect_id = anchors[0].anchor.effect_id
        emit BLOCKED effect_id=… sink=…
        **DO NOT re-submit**; **DO NOT ProvideIntent**
        read parent line: PROCEED → step++; continue
                         ABORT   → exit deny code
    Denied { reason } → emit DENIED code=reason.code(); abort remaining; exit 2
  STREAM_DONE → exit 0
  ▼
Broker (unchanged core)
  per-node policy pre-I2 → I2 → always-confirm git.push rewrite → pending insert
  confirm() (main or dual-terminal process) runs sink from durable snapshot
  github.pr Allowed dispatch requires session grant (has_github_grant)
```

### Recommended Project Structure (touch points)

```
cli/caprun/src/
├── main.rs          # ADD safe-coding-workflow intent kind; orchestrated wait;
│                    #   mid-loop confirm; exit-code map; keep email/file
├── worker.rs        # CHANGE Block branch: hold + stdin protocol; Deny exit 2;
│                    #   machine-readable stdout lines; no re-submit
└── planner.rs       # NO product change (recipe already shipped)

cli/caprun/tests/
├── stream_substrate.rs   # EXTEND: HoldContinue after Block; no re-submit after hold
├── e2e.rs or coding_cli.rs  # NEW: argv constructs SafeCodingWorkflow; policy bind
├── confirm.rs / grant.rs    # REGRESSION + dual-terminal hold integration
└── planner.rs               # REGRESSION only

# DO NOT for Phase 50 product scope:
# - LIVE-07/08 compose-verify SUCCESS claim (Phase 51)
# - New crates / Gate 3 mint helpers / EffectRequest
# - reconnect-remint / dual-Session / session-wide waiver
```

### Pattern 1: Coding intent on existing `caprun run` (CLI-01)

**What:** Add one intent kind to the existing hand-rolled argv path. Do **not** invent a sibling binary.

**Prescriptive argv (recommended):**

```text
caprun [run] [--policy <path>] safe-coding-workflow <coding-intent.json> <workspace-file> [audit-db-path]
```

- `<coding-intent.json>` deserializes to the **fields** of `SafeCodingWorkflow` (or full tagged `CaprunIntent` JSON with `"kind":"SafeCodingWorkflow"`). Fail-closed on missing fields / unknown keys as appropriate.
- `<workspace-file>` still required: main derives workspace root from parent (HARD-04 / F1); pre-create write target for `file.write` O_TRUNC [VERIFIED: Phase 49 research pitfall 5].
- `--policy` / `CAPRUN_POLICY` / neither → same single `bind_policy` call already at `main.rs:391-396`.

**Why JSON over 13 positionals:** CaprunIntent already has serde; tests and design partners can commit fixtures; usage errors are schema errors not order errors.

**Construct intent (prescriptive):**

```rust
// main.rs — ADD arm (email/file arms unchanged)
"safe-coding-workflow" => {
    let raw = std::fs::read_to_string(&intent_param)
        .with_context(|| format!("read coding intent {intent_param}"))?;
    // Prefer full CaprunIntent JSON so kind is explicit:
    let intent: CaprunIntent = serde_json::from_str(&raw)
        .context("parse SafeCodingWorkflow intent JSON")?;
    match &intent {
        CaprunIntent::SafeCodingWorkflow { .. } => intent,
        _ => anyhow::bail!("coding intent JSON must be kind SafeCodingWorkflow"),
    }
}
```

**Do not** accept `--seed-from-file` for coding success path (ProvideIntent rejects `primary_file_derived=true` for SafeCodingWorkflow — Phase 49). Fail-closed if operator tries file-derived coding multi-mint.

### Pattern 2: Block-and-Hold — stay-connected + parent orchestrated confirm (CONFIRM-01)

**What:** On `BlockedPendingConfirmation`, worker stays on the broker connection and waits; main confirms the durable pending row (in-process preferred); worker advances `step_index` **without** re-submitting the blocked node.

**Why stay-connected is the only product path:**

| Constraint | Code fact | Implication |
|------------|-----------|-------------|
| Occupancy latch one-way | `server.rs` session-lifetime latch; never released | Second worker connect rejected for Session |
| ValueStore dies with connection | Per-connection store | Reconnect cannot resolve prior handles |
| ProvideIntent once | `server.rs:2370-2391` | Remint forbidden; second ProvideIntent Error |
| Confirm does not re-submit | `confirmation.rs` Step-7 from snapshot | Worker must not SubmitPlanNode for blocked node |
| Post-confirm outputs out-of-bag | DESIGN §2.2 F-02 | Success path already uses trusted intent only — OK |

**Hold protocol (prescriptive, no new broker verb):**

| Direction | Line format | Meaning |
|-----------|-------------|---------|
| Worker → Main (stdout) | `caprun-stream: BLOCKED effect_id=<uuid> sink=<id>` | Hold; durable pending exists |
| Worker → Main | `caprun-stream: DENIED code=<DenyReason::code()> sink=<id>` | Abort remaining |
| Worker → Main | `caprun-stream: NODE_ALLOWED step=<n> sink=<id>` | Optional progress |
| Worker → Main | `caprun-stream: STREAM_DONE submitted=<n>` | Success terminal |
| Main → Worker (stdin) | `PROCEED` | Confirm released; advance step; continue |
| Main → Worker (stdin) | `ABORT` | Human deny or operator abort; worker exits denied |

**Main confirm path (primary):**

1. Parse BLOCKED line → `effect_id`.
2. Print existing review/confirm/deny pointers (same text as post-exit surface at `main.rs:651-660`).
3. If interactive (TTY and not `CAPRUN_CONFIRM=external`): call `brokerd::confirmation::confirm` / `deny` / optionally `review` **in-process** against the **same** audit DB path (key load via existing `key::load_or_create_key`).
4. On `ConfirmOutcome::Released` → write `PROCEED`.
5. On `ConfirmOutcome::Denied` → write `ABORT`.
6. On sink-fail outcomes (`ConfirmedButSinkFailed`, `EmailSendFailed`, …) → treat as stream failure (recommend exit 1 or distinct ≥3; document).

**Dual-terminal alternate:** Main prints pointers and polls `list_pending_confirmations_for_session` until the effect is no longer `pending`, then checks terminal state (confirmed → PROCEED, denied → ABORT). Existing `caprun confirm`/`deny` subprocesses work against the durable DB while broker+worker stay up.

**Critical main lifecycle change:** Today main does `child.wait()` then `broker_task.abort()` (`main.rs:600-619`). Hold requires:

- **Do not** abort broker until stream fully terminal.
- Spawn worker with `Stdio::piped()` for stdin/stdout (stderr can inherit for human-visible logs).
- Orchestrate until STREAM_DONE / DENIED / worker death.

**After PROCEED:** worker does `step_index += 1; continue` — **never** re-issues the blocked `SubmitPlanNode`. Next `plan_next` emits `github.pr` (for coding).

### Pattern 3: Exit codes and stop semantics (CLI-02)

**Prescriptive taxonomy for `caprun run` stream outcomes:**

| Exit | Machine meaning | When |
|------|-----------------|------|
| **0** | Full success | Every submitted node terminal Allowed **or** Block-released + remaining Allowed; `submitted ≥ 1`; `verify_chain` printed true (soft) |
| **2** | Denied / aborted | `ExecutorDecision::Denied` (incl. `policy_deny`), `NotImplemented`, human deny mid-hold, ABORT path |
| **3** | Blocked / hold incomplete | Stream ended with durable pending still open (worker died on Block without release; external mode abandoned) — surfaces effect_id + review pointer |
| **1** | Usage / infra / empty stream / crash | Parse errors, spawn failures, empty plan stream, worker panic, MAC/key failures |

**Notes:**

- Aligns with existing confirm/deny codes where possible (`deny` already uses exit 2 for Denied — `main.rs:754`).
- Email/file single-node Block today → worker exit 1 + main bail. **Recommend:** map single-node Block to exit **3** (blocked) for honesty, with email/file regression updated; or keep exit 1 for email/file only if updating e2e is scoped carefully — prefer **unify on 3 for Block** so CLI-02 is one contract.
- `policy_deny` is `Denied { reason: PolicyDeny { .. } }` with `reason.code() == "policy_deny"` [VERIFIED: `executor_decision.rs:133`]. Worker should print `code=policy_deny` so main can label the DENIED path distinctly in stdout even when exit code shares 2 with other denies. Optional finer code 4 for policy_deny only if tests need it — **not required** if stdout carries `code=` and exit 2 is documented as "denied/aborted".

**Silent continue-past-Block is forbidden:** any path that treats `BlockedPendingConfirmation` as success or advances without PROCEED is a defect. Unit-test the branch table.

### Pattern 4: github.pr grant continuity (product completeness)

**What:** Allowed `github.pr` dispatch requires `has_github_grant` for the Session [VERIFIED: `server.rs:1474-1513`]. Without grant, broker appends `github_pr_denied` but still returns the prior `Allowed` decision — a **silent no-PR** hazard if ignored.

**Prescriptive Phase 50 behavior:**

1. At coding session start, print:
   ```text
   session_id=<uuid>
   grant: caprun grant <session_id> <audit-db-path>
   ```
2. Before PROCEED after push confirm (or before worker reaches PR step), ensure grant exists — either operator ran grant (dual-terminal / interactive prompt) or test harness called `record_github_grant`.
3. Do **not** auto-grant inside `caprun run` (grant is a distinct human capability — GITHUB-02).
4. Point at existing `caprun grant` verb; do not fold grant into confirm.

### Pattern 5: Preserve single-node email/file + existing verbs

**What:** `send-email-summary` / `create-file-from-report` paths stay green. `confirm` / `deny` / `review` / `grant` / `audit` stay first-branch verbs (`main.rs:80-200`).

**Hold scope recommendation:** Multi-node hold (wait for PROCEED) when the planner can still emit later steps **or** always for coding intent. Simplest rule: **if intent is `SafeCodingWorkflow`, hold; else exit-on-Block as today (mapped to exit 3)**. That avoids changing email e2e semantics beyond exit-code mapping.

### Anti-Patterns to Avoid

- **Worker exit on Block + new Session for PR tail:** dual-Session stitch — rejected.
- **Worker exit on Block + reconnect same Session:** occupancy latch + empty ValueStore — rejected.
- **Re-submit blocked node after confirm:** F-02 / confirmation DESIGN — rejected.
- **Main aborts broker while worker is holding:** kills connection mid-hold.
- **Silent continue past Block:** CLI-02 forbidden.
- **Session-wide confirm waiver / YOLO:** permanently rejected.
- **Auto-grant or auto-confirm:** defeats human gates.
- **Hybrid in-crate multi-submit sold as CLI multi-node:** honesty class — Phase 50 must drive real binary path; Phase 51 owns LIVE claim.
- **Mid-stream ProvideIntent / policy rebind:** laundering / POLICY-03 breach.
- **13 fragile positionals without schema:** avoid; use JSON.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-node submit loop | New orchestrator crate | Phase 48 worker loop | STREAM-01 already done |
| Coding plan emission | New planner crate | `DeterministicPlanner::plan_next` | CODE-01 shipped |
| Trusted multi-arg mint | Mid-stream remint | ProvideIntent-once multi-mint | Gate 3 + Phase 49 |
| Confirm release | Re-submit PlanNode | `brokerd::confirmation::confirm` | Snapshot path; single-shot |
| Pending listing | Grep audit logs | `list_pending_confirmations_for_session` | Already used by main post-Block |
| Policy binding | Second binder / workspace policy | Existing `bind_policy` | POLICY-03 F1 containment |
| Argv parsing framework | clap / structopt crate | Hand-rolled argv (project norm) | HYG-02 / STACK.md |
| Hold IPC | New broker Wait verb | Parent stdin/stdout lines | Same-Session; minimal surface |
| Exit taxonomy | Free-form strings only | Integer codes + structured stdout | CLI-02 machine-checkable |

**Key insight:** Phase 50 is an **orchestration + productization** phase on finished substrate — the security physics (I2, always-confirm, confirm snapshot, occupancy) already exist; composition must not invent resume valves.

## Block-and-Hold Design Options (decision matrix)

| Option | Same Session | ProvideIntent | Occupancy | ValueStore | Product viability | Verdict |
|--------|--------------|---------------|-----------|------------|-------------------|---------|
| **A. Stay-connected + parent confirm (recommended)** | Yes | Once | Held | Live | Full multi-node after push | **PRIMARY** |
| B. Stay-connected + dual-terminal only | Yes | Once | Held | Live | OK; worse UX | **Alternate mode** |
| C. Worker exit + `caprun continue` remint | Broken | Remint risk | Latch blocks reconnect | Empty | Laundering tripwire | **REJECTED** |
| D. Dual-Session stitch | No | New session | New latch | New store | Splits audit/policy | **REJECTED** |
| E. Auto-confirm mid-loop | Yes | Once | Held | Live | Defeats human gate | **REJECTED** |
| F. Main submits remaining PlanNodes unconfined | Ambiguous | N/A | N/A | N/A | Breaks confined egress story | **REJECTED** |

**Recommendation:** Option A primary, Option B as `CAPRUN_CONFIRM=external` (or non-TTY default).

## Exit-Code / Stop-Semantics Design Options

| Option | Codes | Pros | Cons | Verdict |
|--------|-------|------|------|---------|
| **A. 0 success / 2 denied / 3 blocked / 1 infra (recommended)** | 4 outcomes | Matches deny=2 precedent; machine-simple | Email e2e may need update if Block was exit 1 | **PRIMARY** |
| B. Keep all non-success as 1 | 0/1 only | Zero e2e churn | **Fails CLI-02** distinct outcomes | Rejected |
| C. Mirror full confirm codes (0–8) on run | Many | Fine-grained | Collides confirm verb semantics; noisy | Rejected for run |

## Codebase Patterns (evidence)

| Surface | Current behavior | Phase 50 change |
|---------|------------------|-----------------|
| Intent kinds | email/file only; unknown → bail (`main.rs:309-318`) | Add `safe-coding-workflow` |
| Policy | `--policy` / `CAPRUN_POLICY` / default (`main.rs:376-396`) | Reuse unchanged |
| Worker spawn | `env_clear` + INTENT + wait (`main.rs:550-603`) | Piped stdio; orchestrate hold |
| Broker lifetime | abort after worker exit (`main.rs:619`) | Abort only after stream terminal |
| Post-Block surface | list pending + print review/confirm/deny (`main.rs:634-667`) | Same text **during** hold, not only after exit |
| Worker Block | exit 1 no re-submit (`worker.rs:427-434`) | Hold + PROCEED/ABORT |
| Worker Deny | exit 1 (`worker.rs:436-441`) | exit 2 + DENIED line with `code=` |
| Coding bag/recipe | shipped Phase 49 | No recipe change |
| Always-confirm push | `server.rs:807-848` | Consumed by hold (expected Block) |
| Confirm outcomes | exit 0/2/3/4/5/6/7/8 (`main.rs:752-774`) | In-process from main during hold |
| Grant | separate verb (`main.rs:122-149`) | Point at; required before PR |

## Common Pitfalls

### Pitfall 1: Aborting broker on first Block
**What goes wrong:** Main still `child.wait()` + `broker_task.abort()`; hold impossible.  
**Why:** Pre-Phase-50 single-node lifecycle.  
**How to avoid:** Orchestrated wait; abort broker only after STREAM_DONE/DENIED/worker death.  
**Warning signs:** Integration test can't confirm mid-run; second SubmitPlanNode never happens.

### Pitfall 2: Re-submit after confirm
**What goes wrong:** Worker on PROCEED re-sends the same PlanNode → double-effect / wrong semantics.  
**Why:** Instinct that "Allowed" must come from submit.  
**How to avoid:** PROCEED only advances `step_index`; confirm already executed sink from snapshot.  
**Warning signs:** Two push attempts; tests that expect second SubmitPlanNode for blocked sink.

### Pitfall 3: Treating always-confirm push as failure
**What goes wrong:** Coding "success" tests expect five Allowed without hold.  
**Why:** Push always rewrites Allowed → Block.  
**How to avoid:** Success path = Allowed×3 + Block(push) + confirm + Allowed(PR).  
**Warning signs:** Tests require `git.push` Allowed decision.

### Pitfall 4: Missing github.pr grant
**What goes wrong:** PR step "Allowed" but no PR created (`github_pr_denied`).  
**Why:** Grant gate is dispatch-side; decision may still look Allowed.  
**How to avoid:** Print grant pointer; tests call grant; document operator checklist.  
**Warning signs:** STREAM_DONE but no PR event / mock GitHub empty.

### Pitfall 5: Dual-Session "finish the chain later"
**What goes wrong:** Partner runs push confirm in session A, PR in session B.  
**Why:** Feels natural after process exit.  
**How to avoid:** Product path never exits worker across push→PR; docs forbid stitch.  
**Warning signs:** Two session_ids in design-partner script.

### Pitfall 6: file.write target missing / unstaged commit
**What goes wrong:** Early nodes fail closed before push hold.  
**Why:** O_TRUNC needs existing file; git.commit needs staged changes.  
**How to avoid:** Fixtures pre-create path; fold `git add` into `test_command`/`test_args_json` (Phase 49).  
**Warning signs:** Failures at step 0/2 in coding e2e.

### Pitfall 7: Email/file regression from hold plumbing
**What goes wrong:** Piped stdio / exit-code map breaks e2e.  
**Why:** Shared main/worker paths.  
**How to avoid:** Hold only for SafeCodingWorkflow; keep email/file branch simple; run e2e + stream_substrate.  
**Warning signs:** `cli/caprun/tests/e2e.rs` red.

### Pitfall 8: Hybrid composition sold as Phase 50 DONE
**What goes wrong:** In-crate multi-submit claimed as CLI multi-node.  
**Why:** LIVE-05 honesty class.  
**How to avoid:** Phase 50 success = real `caprun` binary path; LIVE claim stays Phase 51.  
**Warning signs:** ROADMAP Phase 51 checked early.

### Pitfall 9: Confirm-hold DESIGN re-run trigger
**What goes wrong:** Implementing reconnect-remint or dual-Session as "easier hold".  
**Why:** DESIGN §13.2 re-runs on confirm-hold pivot.  
**How to avoid:** Stay inside CLEARED Option A; any pivot requires orchestrator-owned re-trace.  
**Warning signs:** New `caprun continue` verb that ProvideIntents again.

## Code Examples

### Worker Block-and-Hold branch (prescriptive)

```rust
// cli/caprun/src/worker.rs — REPLACE exit-1 Block arm
// Source: DESIGN §3 + ARCHITECTURE Pattern 3; effect_id from anchors
ExecutorDecision::BlockedPendingConfirmation { anchors } => {
    let effect_id = anchors
        .first()
        .map(|a| a.anchor.effect_id)
        .ok_or_else(|| anyhow::anyhow!("Block without anchors (invariant)"))?;
    let sink = anchors
        .first()
        .map(|a| a.anchor.sink.0.clone())
        .unwrap_or_else(|| "unknown".into());
    // Machine line for main (also human-visible)
    println!("caprun-stream: BLOCKED effect_id={effect_id} sink={sink}");
    // Surface for dual-terminal operators (stderr OK)
    eprintln!(
        "[worker] BLOCKED pending confirmation effect_id={effect_id} sink={sink}: holding stream"
    );
    // Wait for parent — do NOT re-submit, do NOT ProvideIntent
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    match line.trim() {
        "PROCEED" => {
            // Confirm already executed sink from snapshot — advance only
            step_index += 1;
            continue;
        }
        "ABORT" => {
            eprintln!("[worker] ABORT after Block — exiting 2");
            std::process::exit(2);
        }
        other => anyhow::bail!("unknown hold resume token: {other:?}"),
    }
}
```

### Main mid-loop confirm (prescriptive sketch)

```rust
// After spawning worker with piped stdio — do NOT immediately wait+abort broker
// Source: main.rs confirm path (run_confirm_or_deny) + DESIGN §3.3
loop {
    let line = read_worker_stdout_line(&mut child_stdout)?;
    if let Some(blocked) = parse_blocked(&line) {
        println!("=== Blocked pending confirmation ===");
        println!("  effect_id={}  sink={}", blocked.effect_id, blocked.sink);
        println!("    review:  caprun review {} {}", blocked.effect_id, audit_path);
        println!("    confirm: caprun confirm {} {}", blocked.effect_id, audit_path);
        println!("    deny:    caprun deny {} {}", blocked.effect_id, audit_path);
        let outcome = interactive_or_external_confirm(&blocked, &audit_path, &workspace_root)?;
        match outcome {
            HoldResume::Proceed => writeln!(child_stdin, "PROCEED")?,
            HoldResume::Abort => {
                writeln!(child_stdin, "ABORT")?;
                // wait worker; abort broker; exit 2
            }
        }
        continue;
    }
    if line.starts_with("caprun-stream: STREAM_DONE") {
        break;
    }
    if let Some(denied) = parse_denied(&line) {
        // exit 2; surface code= including policy_deny
        let _ = denied;
        break;
    }
}
// only now: wait child, abort broker, verify_chain, map exit
```

### Coding intent JSON fixture shape

```json
{
  "kind": "SafeCodingWorkflow",
  "path": "src/hello.txt",
  "contents": "hello from caprun\n",
  "test_command": "sh",
  "test_args_json": "[\"-c\", \"git add -A && true\"]",
  "commit_message": "caprun: safe coding demo",
  "remote": "origin",
  "refspec": "HEAD:refs/heads/caprun-demo",
  "owner": "acme",
  "repo": "demo",
  "base": "main",
  "head": "caprun-demo",
  "pr_title": "caprun safe coding demo",
  "pr_body": "Opened by multi-node stream"
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| One-shot worker exit on any non-Allowed | Sequential loop; Block exit 1 substrate | Phase 48 | STREAM-01/02 |
| Email/file intents only on CLI | + SafeCodingWorkflow types/mint/recipe | Phase 49 | CODE-01/02; CLI still closed |
| Hybrid in-crate multi-node LIVE-05 | CLI multi-node product path (this phase) | Phase 50 | Closes honesty gap substrate for Phase 51 |
| End-of-run Block surface only | Mid-loop hold + same surfaces during run | Phase 50 | CONFIRM-01 |

**Deprecated/outdated:**

- "Worker exit 1 on Block is the product path" — substrate only; product is hold (DESIGN §3).
- "Reconnect after Block" — rejected by occupancy + ProvideIntent pins.
- Hybrid composition as multi-step DONE claim — still forbidden; Phase 51 owns LIVE.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | JSON intent file is preferred argv shape over 13 positionals | Pattern 1 | UX churn if operator wanted pure positionals; low security risk |
| A2 | Parent stdin/stdout line protocol is sufficient without new broker IPC | Pattern 2 | If confinement unexpectedly blocks stdin read, need alternate same-Session signal |
| A3 | Interactive in-process confirm is acceptable primary UX | Discretion | Partner may prefer dual-terminal only — both DESIGN-allowed |
| A4 | Exit 3 for blocked / hold-incomplete is the right integer | Exit codes | e2e updates; can remap if user locks different numbers |
| A5 | Hold only for SafeCodingWorkflow (email/file keep stop-on-Block) | Pattern 5 | If email multi-node ever appears, hold rule must generalize |
| A6 | github.pr without grant returning Allowed + denied event remains as shipped | Pattern 4 | Product must compensate with grant UX; not a Phase 50 executor change |

**If this table is empty:** N/A — several discretionary claims need planner confirmation via A1–A6.

## Open Questions

1. **Exact exit integers for policy_deny vs other Deny**
   - What we know: CLI-02 requires distinct outcome for policy_deny; `DenyReason::code()` already distinguishes.
   - What's unclear: whether distinct **exit integer** is required vs structured stdout + shared exit 2.
   - Recommendation: exit 2 for all denied/aborted; always print `code=policy_deny` on stdout; refine only if tests demand.

2. **TTY detection for interactive vs external confirm**
   - What we know: both DESIGN-allowed.
   - What's unclear: default when stdin is a pipe (CI).
   - Recommendation: non-TTY → external/poll mode or env `CAPRUN_CONFIRM=auto|interactive|external`.

3. **Whether Phase 50 includes a thin coding e2e on host that stops at push Block + confirm without full git/network**
   - What we know: full LIVE is Phase 51; host may lack Docker (this research host: docker unavailable).
   - Recommendation: Wave 0 unit/integration for hold protocol + argv + exit codes on host; optional Linux coding smoke only if environment allows; do not claim LIVE-07.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo | Build + unit tests | ✓ | cargo 1.97.1 / rustc 1.97.1 | — |
| `scripts/check-invariants.sh` | HYG gates | ✓ | present | — |
| `scripts/mailpit-verify.sh` | Linux suite if SMTP touched | ✓ script | present | Host unit tests for Phase 50 core (no SMTP required for coding hold) |
| `scripts/compose-verify.sh` | Full Linux authority | ✓ script | present | Phase 51 primary; Phase 50 optional smoke |
| Docker / Colima | Linux security tests | ✗ (unavailable this host) | — | Host-safe unit/integration for hold/argv/exit codes; Linux deferred to env with Docker |
| pkg-config / libssl | lettre native-tls builds | partial (user-local debs historically) | — | Same as Phase 49 host workaround; Docker images install via mailpit-verify |

**Missing dependencies with no fallback:** none for Phase 50 host-safe core (CLI argv + hold protocol unit tests).

**Missing dependencies with fallback:** Docker — use host unit tests for protocol; Linux e2e when Docker available / Phase 51.

## Validation Architecture

> `workflow.nyquist_validation` absent in `.planning/config.json` → treat as **enabled**.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (workspace), crate bins `caprun` / `brokerd` |
| Config file | workspace `Cargo.toml` (no jest/pytest) |
| Quick run command | `cargo test -p caprun --test stream_substrate --test planner -- --test-threads=1` |
| Full suite command | `./scripts/check-invariants.sh && cargo test --workspace --no-fail-fast` (Linux: `bash scripts/mailpit-verify.sh` / `compose-verify.sh`) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `safe-coding-workflow` argv builds SafeCodingWorkflow + bind_policy once | unit / bin | `cargo test -p caprun --test coding_cli` (or e2e sibling) | ❌ Wave 0 |
| CLI-01 | Existing email/file run still works | integration | `cargo test -p caprun --test e2e` | ✅ |
| CLI-01 | review/confirm/deny/grant still dispatch | integration | `cargo test -p caprun --test confirm`; `… --test grant` | ✅ |
| CLI-02 | Block surfaces effect_id + review pointer (no silent continue) | unit | extend `stream_substrate` + worker/main protocol tests | ❌ Wave 0 (extend ✅ stream_substrate) |
| CLI-02 | Deny / policy_deny abort remaining; distinct code/label | unit | `stream_substrate` deny/policy_deny + new exit-map tests | ✅ partial / ❌ full exit map |
| CLI-02 | Full success exit 0 | unit/integration | coding hold harness with mocked PROCEED after push | ❌ Wave 0 |
| CONFIRM-01 | After Block, PROCEED advances without re-submit of blocked node | unit | `drive_stream` HoldContinue assertion (submit count) | ❌ Wave 0 |
| CONFIRM-01 | ABORT after Block stops remaining nodes | unit | HoldAbort branch | ❌ Wave 0 |
| CONFIRM-01 | No mid-stream ProvideIntent / no dual Session in product path | code review + invariant | `check-invariants.sh`; grep no remint path | ✅ gates |
| CONFIRM-01 | Same audit Session across hold (integration when possible) | integration | coding session: push Block → confirm → PR under one session_id | ❌ Wave 0 (host or Linux) |

### Sampling Rate

- **Per task commit:** `./scripts/check-invariants.sh` + targeted `cargo test -p caprun --test stream_substrate --test planner`
- **Per wave merge:** `cargo test -p caprun -- --test-threads=1` (host-safe subset) + invariants
- **Phase gate:** Invariants green; new hold/exit/argv tests green; email/file + confirm/grant regression green; no LIVE-07 claim

### Wave 0 Gaps

- [ ] `cli/caprun/tests/coding_cli.rs` (or e2e extension) — CLI-01 argv → INTENT JSON SafeCodingWorkflow; unknown kind still fail-closed; policy flag accepted
- [ ] Extend `cli/caprun/tests/stream_substrate.rs` — HoldContinue (Block then PROCEED semantics: no re-submit; step advances); HoldAbort
- [ ] Exit-code map tests — success=0, denied=2, blocked=3 (unit over pure mapper fn if extracted)
- [ ] Worker/main protocol contract tests — parse BLOCKED/DENIED/STREAM_DONE lines; PROCEED/ABORT tokens
- [ ] Optional: integration harness that drives real worker hold with in-process confirm against temp audit DB (Linux preferred for full push; host can mock decision branch)

*(Framework already present — no new test runner install.)*

## Security Domain

> `security_enforcement` not disabled in config → included.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (local CLI, no user auth system) | — |
| V3 Session Management | yes (Session occupancy, ProvideIntent once) | Broker occupancy latch; no reconnect remint |
| V4 Access Control | yes (policy bind, grant capability) | POLICY-03; `caprun grant` session-scoped |
| V5 Input Validation | yes | Closed CaprunIntent serde; fail-closed unknown kind; UUID parse on confirm/grant |
| V6 Cryptography | yes (audit MAC) | Existing keyed chain; `load_or_create_key` F1 |

### Known Threat Patterns for multi-node CLI / confirm-hold

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Reconnect remint as UserTrusted | Elevation | Occupancy latch + ProvideIntent once; hold stays connected |
| Dual-Session stitch | Tampering / audit split | Product path one Session only |
| Silent continue past I2 Block | Tampering | Branch table; no PROCEED without confirm; tests |
| Session-wide confirm waiver | Elevation | Single-shot per effect_id; subsequent nodes full I2 |
| Policy rebind mid-stream from workspace | Elevation | POLICY-03 bind once outside worker; F1 path refuse |
| Auto-confirm / auto-grant | Elevation | Human verbs only; print pointers |
| Hybrid sold as CLI multi-step | Spoofing (claim) | Real binary path; LIVE honesty Phase 51 |
| EffectRequest free-form tools | Elevation | Gate 1 ban |
| Post-confirm output laundering into next args | Elevation | F-02 out-of-bag; success path trusted intent only |

## Sources

### Primary (HIGH confidence)

- [VERIFIED: codebase] `cli/caprun/src/{main,worker,planner}.rs` — CLI gaps, hold site, recipe
- [VERIFIED: codebase] `crates/brokerd/src/server.rs:807-848,1474-1513` — always-confirm push; github.pr grant gate
- [VERIFIED: codebase] `crates/brokerd/src/confirmation.rs` — confirm/deny outcomes; no re-submit
- [VERIFIED: codebase] `crates/runtime-core/src/{intent,executor_decision,policy}.rs` — SafeCodingWorkflow; DenyReason codes; broker_default sinks
- [VERIFIED: codebase] Phase 48/49 tests + VERIFICATION/SUMMARY artifacts
- [CITED: planning-docs/DESIGN-multi-step-plan-stream.md] §§1–6, §3 Block-and-Hold, §6 deny/abort, §13 re-run triggers
- [CITED: .planning/REQUIREMENTS.md] CLI-01/02, CONFIRM-01
- [CITED: .planning/research/{SUMMARY,ARCHITECTURE,PITFALLS}.md] Pattern 3 hold; CLI phase guidance

### Secondary (MEDIUM confidence)

- [CITED: DESIGN-confirmation-release.md] via DESIGN multi-step F-02 / confirm no re-submit
- Phase 49 RESEARCH argv/hold deferrals (now this phase's job)

### Tertiary (LOW confidence)

- Interactive TTY default heuristics (A2/A3) — product discretion, not security physics

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — zero new packages; pure extension of shipped crates
- Architecture: **HIGH** — DESIGN CLEARED + code-verified gaps and hold constraints
- Pitfalls: **HIGH** — occupancy/ProvideIntent/always-confirm/grant hazards code-verified
- Exit integers / argv JSON shape: **MEDIUM** — discretionary but constrained by CLI-02

**Research date:** 2026-07-29  
**Valid until:** 2026-08-28 (30 days; stable TCB domain)

## RESEARCH COMPLETE readiness

Planner can create PLAN.md files for:

1. **Wave 0 / validation:** stream hold branch tests + exit mapper + coding argv tests  
2. **Worker hold protocol:** Block → signal → PROCEED/ABORT; Deny exit 2; STREAM_DONE  
3. **Main coding driver + orchestrated lifecycle:** safe-coding-workflow JSON; piped worker; mid-loop confirm; broker lifetime; exit codes; grant pointer  
4. **Regression:** email/file e2e, confirm/grant/review/audit, planner/stream_substrate, check-invariants  

Do **not** plan LIVE-07/08 SUCCESS claims or packaging into this phase.
