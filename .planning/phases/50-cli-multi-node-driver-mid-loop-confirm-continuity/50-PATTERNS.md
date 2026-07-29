# Phase 50: CLI Multi-node Driver & Mid-loop Confirm Continuity - Pattern Map

**Mapped:** 2026-07-29
**Files analyzed:** 8
**Analogs found:** 8 / 8

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cli/caprun/src/main.rs` | controller / CLI orchestrator | request-response + event-driven (hold loop) | same file: intent match, worker spawn, post-Block surface, `run_confirm_or_deny` | exact |
| `cli/caprun/src/worker.rs` | controller (worker stream loop) | request-response + event-driven | same file: sequential stream match + Block exit-1 arm | exact |
| `cli/caprun/src/planner.rs` | service (reference only) | transform | same file: `plan_coding_next` / SafeCodingWorkflow | exact (no product edit) |
| `cli/caprun/tests/stream_substrate.rs` | test | transform / batch | same file: `drive_stream` + `block_stops_without_resubmit` | exact |
| `cli/caprun/tests/coding_cli.rs` (NEW Wave 0) | test | request-response | `cli/caprun/tests/e2e.rs` + `grant.rs` binary spawn | role-match |
| `cli/caprun/tests/e2e.rs` | test (regression) | request-response | same file email path | exact (preserve) |
| `cli/caprun/tests/confirm.rs` | test (regression + dual-terminal) | request-response | same file cross-process confirm | exact |
| `cli/caprun/tests/grant.rs` | test (regression + grant pointer) | request-response | same file `run_caprun_grant` | exact |

## Pattern Assignments

### `cli/caprun/src/main.rs` (controller / CLI orchestrator)

**Analog:** same file — hand-rolled argv, POLICY-03 bind, broker+worker lifecycle, confirm exit map

**Early verb dispatch — preserve first** (lines 71–201): `confirm`/`deny`/`review`/`grant`/`audit` branch **before** `run` alias and intent-kind parse. Do not shadow these with coding intent work.

```rust
// lines 80–111 — confirm/deny/review first-branch exit
if let Some(verb) = raw_args.first().map(String::as_str) {
    if verb == "confirm" || verb == "deny" || verb == "review" {
        // UUID fail-closed → run_confirm_or_deny → std::process::exit(code)
    }
    // grant (122–149), audit (163–200) same shape
}
// lines 213–215 — `run` alias only
if raw_args.first().map(String::as_str) == Some("run") {
    idx = 1;
}
```

**Intent kind match — ADD arm** (lines 309–318):

```rust
let intent = match intent_kind.as_str() {
    "send-email-summary" => CaprunIntent::SendEmailSummary { /* … */ },
    "create-file-from-report" => CaprunIntent::CreateFileFromReport { path: intent_param },
    _ => anyhow::bail!("unknown intent kind: {intent_kind}"),
};
```

**Pattern to copy for Phase 50 coding arm:**
- Keep email/file arms byte-stable.
- Add `"safe-coding-workflow"` that treats `intent_param` as a **JSON path**:
  - `std::fs::read_to_string` + `serde_json::from_str::<CaprunIntent>`
  - Fail-closed unless `CaprunIntent::SafeCodingWorkflow { .. }`
  - Reject `--seed-from-file` for coding (broker already rejects `primary_file_derived=true` for this variant — surface at CLI too).
- Do **not** invent clap; stay hand-rolled (project norm / HYG-02).

**POLICY-03 bind once** (lines 376–396) — **reuse unchanged**:

```rust
let policy_path = policy_flag_path.or_else(|| std::env::var("CAPRUN_POLICY").ok());
let (session_policy, policy_hash) = brokerd::policy::bind_policy(
    policy_path.as_deref().map(Path::new),
    workspace_root_dir,
)
.context("bind_policy (POLICY-03 fail-closed trusted-source policy binding)")?;
```

**Broker spawn + yield** (lines 472–495) — keep alive for hold window:

```rust
let broker_task = tokio::spawn(async move {
    brokerd::server::run_broker_server(/* session_id, conn, policy_bound head, … */).await
});
tokio::task::yield_now().await;
```

**Worker spawn env_clear allowlist** (lines 556–593) — keep security posture; **add** `Stdio::piped()` for stdin/stdout:

```rust
let mut child = std::process::Command::new(&worker_binary)
    .env_clear()
    .env("PATH", "/usr/bin:/bin:/usr/local/bin")
    .env("BROKER_SOCK", format!("/agentos/{session_id}"))
    .env("WORKSPACE_FILE", workspace_rel)
    .env("INTENT", serde_json::to_string(&intent)?)
    .env("PRIMARY_SEED_FILE_DERIVED", if primary_file_derived { "1" } else { "0" })
    .envs(worker_planner_env)
    // Phase 50: .stdin(Stdio::piped()).stdout(Stdio::piped()) for hold protocol
    .spawn()
    .context("spawn caprun-worker")?;
```

**Current wait-then-abort — REPLACE for multi-node hold** (lines 595–619):

```rust
// TODAY (single-node): fire-and-forget wait + abort broker
let child_status = tokio::task::spawn_blocking(move || child.wait()).await??;
// …
broker_task.abort();
```

**Pattern to copy for Phase 50 orchestration:**
- Do **not** abort broker until STREAM_DONE / DENIED / worker death.
- Loop on worker stdout lines (`caprun-stream: …`); on BLOCKED call in-process confirm path; write `PROCEED`/`ABORT` to worker stdin.
- Email/file may keep simpler wait (research: hold only for `SafeCodingWorkflow`).

**Post-Block operator surface** (lines 634–667) — **reuse text mid-hold**, not only after exit:

```rust
println!("\n=== Blocked pending confirmation ({} effect{}) ===", rows.len(), /* … */);
for (effect_id, sink) in &rows {
    println!("  effect_id={effect_id}  sink={sink}");
    println!("    review:  caprun review {effect_id} {audit_path}");
    println!("    confirm: caprun confirm {effect_id} {audit_path}");
    println!("    deny:    caprun deny {effect_id} {audit_path}");
}
```

**In-process confirm during hold — copy `run_confirm_or_deny`** (lines 687–778):

```rust
// Open same audit DB + load F1 key + confirm()/deny()/review()
// Exit-code map for confirm verb (reuse Released=0, Denied=2, …):
ConfirmOutcome::Released => (0, None),
ConfirmOutcome::Denied => (2, Some("denied")),
ConfirmOutcome::ConfirmedButSinkFailed => (3, Some("confirmed, but the sink invocation failed")),
// …
```

**Map for `caprun run` stream exits (CLI-02 — new, align with deny=2):**

| Exit | Meaning |
|------|---------|
| 0 | Full stream success (incl. Block-released + remaining Allowed) |
| 2 | Denied / aborted (incl. policy_deny, human ABORT) |
| 3 | Blocked / hold incomplete (durable pending still open) |
| 1 | Usage / infra / empty stream / crash |

**Grant pointer at coding session start — copy `run_grant` print shape** (lines 799–806):

```rust
// After session_id known, for SafeCodingWorkflow:
println!("session_id={session_id}");
println!("grant: caprun grant {session_id} {audit_path}");
// Do NOT auto-grant (GITHUB-02).
```

---

### `cli/caprun/src/worker.rs` (controller, stream loop)

**Analog:** same file sequential plan-stream loop (lines 379–454)

**Doc contract** (lines 45–52) — update when hold lands: today documents exit-1 on Block as substrate; Phase 50 product hold stays connected.

**Core branch table** (lines 418–442) — **REPLACE Block arm; tighten Deny exit**:

```rust
match decision {
    ExecutorDecision::Allowed => {
        if let Some(id) = output_value_id {
            bag.insert(format!("out_{step_index}"), id);
        }
        step_index += 1;
        continue;
    }
    ExecutorDecision::BlockedPendingConfirmation { .. } => {
        eprintln!(/* … */);
        std::process::exit(1); // Phase 48 substrate — Phase 50: HOLD
    }
    ExecutorDecision::Denied { .. } | ExecutorDecision::NotImplemented => {
        eprintln!(/* … */);
        std::process::exit(1); // Phase 50: exit 2 + DENIED line with code=
    }
}
```

**Prescriptive hold arm (from RESEARCH; effect_id from anchors):**

```rust
ExecutorDecision::BlockedPendingConfirmation { anchors } => {
    let effect_id = anchors.first().map(|a| a.anchor.effect_id)
        .ok_or_else(|| anyhow::anyhow!("Block without anchors"))?;
    let sink = anchors.first()
        .map(|a| a.anchor.sink.0.clone())
        .unwrap_or_else(|| "unknown".into());
    println!("caprun-stream: BLOCKED effect_id={effect_id} sink={sink}");
    // Wait parent — NO re-submit, NO ProvideIntent
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    match line.trim() {
        "PROCEED" => { step_index += 1; continue; }
        "ABORT" => std::process::exit(2),
        other => anyhow::bail!("unknown hold resume token: {other:?}"),
    }
}
```

**Machine lines (protocol contract):**

| Direction | Line |
|-----------|------|
| Worker → Main | `caprun-stream: BLOCKED effect_id=<uuid> sink=<id>` |
| Worker → Main | `caprun-stream: DENIED code=<DenyReason::code()> sink=<id>` |
| Worker → Main | `caprun-stream: NODE_ALLOWED step=<n> sink=<id>` (optional) |
| Worker → Main | `caprun-stream: STREAM_DONE submitted=<n>` |
| Main → Worker | `PROCEED` / `ABORT` |

**Also keep:**
- ProvideIntent **exactly once** before stream loop (existing).
- Coding skips RequestFd (Phase 49 — already shipped).
- Empty stream fail-closed (lines 446–452).
- Framed IPC helpers `send_framed` / `recv_framed` (lines 510–529) unchanged.

---

### `cli/caprun/src/planner.rs` (service — reference only)

**Analog:** same file `plan_coding_next` (from line 247) + `DeterministicPlanner::plan_next` match arm for `SafeCodingWorkflow`.

**Pattern:** Phase 50 does **not** change the five-node recipe (`file.write → process.exec → git.commit → git.push → github.pr`). Main/worker consume it. After PROCEED on push Block, next `plan_next` emits `github.pr` because `step_index` advanced without re-submit.

**LlmPlanner fail-closed on coding** (planner ~548–553): leave as-is.

---

### `cli/caprun/tests/stream_substrate.rs` (test — EXTEND)

**Analog:** same file `drive_stream` + `apply_stream_decision` + `block_stops_without_resubmit`

**Branch mirror** (lines 74–92):

```rust
fn apply_stream_decision(/* … */) -> StreamBranch {
    match decision {
        ExecutorDecision::Allowed => { /* bag out_{step}; Continue */ }
        ExecutorDecision::BlockedPendingConfirmation { .. } => StreamBranch::StopBlocked,
        ExecutorDecision::Denied { .. } | ExecutorDecision::NotImplemented => {
            StreamBranch::AbortDenied
        }
    }
}
```

**Phase 50 extension pattern:**
- Add `StreamBranch::HoldContinue` / `HoldAbort` (or parallel helper) that models:
  - On Block: **do not** count a second submit of the blocked node
  - On PROCEED: `step_index += 1`, continue `plan_next` (next node only)
  - On ABORT: terminal AbortDenied without further submits
- Extend `StreamTerminal` with success-after-hold vs incomplete-hold if needed for exit-map unit tests.
- Keep existing `block_stops_without_resubmit` green (substrate stop still valid for non-coding / until hold flag).

**No-resubmit proof style** (lines 397–457):

```rust
assert_eq!(submitted, 1, "Block must stop … exactly one SubmitPlanNode");
// After hold PROCEED: submitted may grow for *later* nodes only —
// never a second submit with the blocked node's sink at the same step.
```

---

### `cli/caprun/tests/coding_cli.rs` (NEW Wave 0 test)

**Analog A:** `cli/caprun/tests/e2e.rs` — real binary spawn + temp workspace/audit layout

```rust
// e2e.rs lines 62–88 — F1-safe layout + CARGO_BIN_EXE_caprun
let ws_dir = tmp.join("workspace");
let workspace_file = ws_dir.join("workspace.txt");
let audit_db_path = tmp.join("audit.db");
let caprun_bin = env!("CARGO_BIN_EXE_caprun");
let output = std::process::Command::new(caprun_bin)
    .arg("send-email-summary")
    .arg("demo@example.test")
    .arg(workspace_file.to_str().unwrap())
    .arg(audit_db_path.to_str().unwrap())
    .output()
    .expect("spawn caprun");
```

**Analog B:** `cli/caprun/tests/grant.rs` — host-portable argv + exit-code asserts without confined worker when possible.

**Pattern for coding_cli:**
- Host-safe unit tests: parse JSON fixture → `CaprunIntent::SafeCodingWorkflow`; unknown kind still bail; `--policy` accepted (may test via binary smoke where Linux available).
- Linux-gated full spawn only if environment allows; do **not** claim LIVE-07.
- Fixture JSON shape from RESEARCH (kind + 13 fields).
- Pre-create `file.write` target path (O_TRUNC); fold `git add` into test_args if driving real sinks later.

---

### `cli/caprun/tests/e2e.rs` / `confirm.rs` / `grant.rs` (regression)

| File | Preserve | Phase 50 note |
|------|----------|---------------|
| `e2e.rs` | email substrate_demo exit 0 | If Block exit unified to 3, update only if email path now returns 3 on Block; prefer hold-only-for-coding to minimize churn |
| `confirm.rs` | seed pending + real `caprun confirm`/`deny` subprocess | Dual-terminal hold uses same verbs against live DB while broker+worker stay up |
| `grant.rs` | `run_caprun_grant` exit 0 + session-scoped | Coding success needs grant before PR dispatch; tests call grant API or binary |

**confirm seed pattern** (confirm.rs lines 64–75): `seed_pending_file_create_block` via brokerd public API — usable for host-safe hold/confirm integration without confined worker.

---

## Shared Patterns

### Hand-rolled argv (no clap)

**Source:** `cli/caprun/src/main.rs` lines 67–318  
**Apply to:** Coding intent kind + flags (`--policy`, optional `run` verb)  
**Rule:** Fail-closed unknown kinds; UUID parse fail-closed on confirm/grant/audit; early verbs never fall through to intent parse.

### POLICY-03 bind once outside worker

**Source:** `cli/caprun/src/main.rs` lines 376–396 + policy_bound event chain  
**Apply to:** Coding multi-node sessions  
**Rule:** Single `bind_policy`; never re-bind mid-stream; refuse paths beneath workspace root.

### env_clear worker spawn

**Source:** `cli/caprun/src/main.rs` lines 556–591  
**Apply to:** Coding worker spawn  
**Rule:** Explicit non-secret env only (`BROKER_SOCK`, `WORKSPACE_FILE`, `INTENT`, `PRIMARY_SEED_FILE_DERIVED`, optional planner). Phase 50 adds piped stdio, not ambient env.

### Confirm/deny/review/grant product verbs

**Source:** `main.rs` early dispatch + `run_confirm_or_deny` / `run_grant`  
**Apply to:** Mid-loop hold UX (in-process preferred) and dual-terminal (`CAPRUN_CONFIRM=external`)  
**Rule:** Confirm acts on durable pending row; never re-submits PlanNode; grant is distinct capability (never auto).

### Stream decision branch table (no silent continue-past-Block)

**Source:** `worker.rs` 418–442 + `stream_substrate.rs` `apply_stream_decision`  
**Apply to:** Worker hold + unit tests  
**Rule:** Block → hold or stop; Deny → abort remaining; PROCEED advances step without re-submit; empty stream fail-closed.

### ProvideIntent once / occupancy latch

**Source:** broker ProvideIntent + worker doc (worker.rs lines 1–50); DESIGN multi-step §3  
**Apply to:** All hold design choices  
**Rule:** Stay-connected hold only; reject reconnect-remint and dual-Session stitch.

### Exit-code honesty (CLI-02)

**Source:** confirm map `main.rs` 752–774 (deny=2 precedent); RESEARCH taxonomy  
**Apply to:** `caprun run` stream terminal mapping in main (+ worker exit codes)  
**Rule:** 0 success / 2 denied-aborted / 3 blocked-incomplete / 1 infra; print `code=policy_deny` on DENIED lines even if exit shares 2.

### Linux cfg + mailpit/compose gates

**Source:** CLAUDE.md; e2e `#[cfg(target_os = "linux")]`  
**Apply to:** Any full binary coding e2e  
**Rule:** Host unit tests for protocol/argv/exit map; Linux security claims only under mailpit-verify/compose-verify; macOS 0-passed cfg gates expected.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| *(none for product files)* | — | — | Parent-pipe hold protocol is **new** but built from existing stdin/stdout + confirm APIs; no separate crate |
| Live multi-node CLI SUCCESS claim | — | — | **Phase 51** (LIVE-07/08) — do not pattern as Phase 50 DONE |

Closest partial analog for hold IPC: none in-repo (research rejects new broker Wait verb). Implement as main↔worker line protocol only.

## Metadata

**Analog search scope:** `cli/caprun/src/{main,worker,planner}.rs`, `cli/caprun/tests/{stream_substrate,e2e,confirm,grant}.rs`, `crates/runtime-core/src/intent.rs` (SafeCodingWorkflow type), Phase 49 PATTERNS style  
**Files scanned:** ~12 primary sources + RESEARCH file list  
**Pattern extraction date:** 2026-07-29  
**Requirements covered:** CLI-01, CLI-02, CONFIRM-01 (not LIVE-07/08, not PKG-01)
