# Phase 32: `process.exec` Sink — Broker-Spawned Confined Child - Research

**Researched:** 2026-07-17
**Domain:** Rust TCB implementation — Landlock/seccomp confinement of an arbitrary broker-spawned child, `std::process`/`tokio::process` spawn mechanics, taint-mint wiring, I2 sink-table extension.
**Confidence:** HIGH for every claim traced to a specific file:line read this session (marked `[VERIFIED: local code/crate source]`); MEDIUM for reasoned extensions not explicitly pinned by the DESIGN doc (marked `[ASSUMED]`, each carries a recommendation); this phase makes NO external network calls — no WebSearch/Context7 was used or needed (all providers disabled in this environment; the domain is 100% internal-codebase + locally-vendored-crate-source grounding).

This is an **implementation** phase. The design is pinned and gate-cleared
(`planning-docs/DESIGN-effect-breadth-exec.md`, cleared Round 1,
`planning-docs/DESIGN-GATE-RECORD-v1.7.md`). This document does not re-litigate
the design — it grounds file-by-file wiring, exact crate APIs (verified against
the vendored `landlock-0.4.5` source at
`~/.cargo/registry/src/.../landlock-0.4.5/`, not just training knowledge), and
surfaces THREE concrete implementation gaps the DESIGN doc leaves unresolved
(see "DESIGN-doc gaps this research resolves" below) that the planner must
address explicitly.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-------------------|
| EXEC-01 | `process.exec` sink runs a broker-spawned confined child, never via the worker's own execve | Existing-Pattern Map row 1-3, Pattern 1 (launcher), File-by-file §cli/caprun-exec-launcher |
| EXEC-02 | child stdout/stderr captured and taint-minted untrusted, sole mint site, no stapling | Pattern 3 (mint_from_exec), File-by-file §quarantine.rs/§server.rs |
| EXEC-03 | tainted exec-output routed to a sensitive sink arg deterministically Blocked, unbroken audit-DAG edge, verify_chain true | File-by-file §sink_sensitivity.rs/§sink_schema.rs, Validation Architecture EXEC-03 row |
| EXEC-04 | exec child kernel-confined, fail-closed arg-schema, durable spawn+exit audit Event | Pattern 1/2, File-by-file §landlock.rs/§seccomp.rs/§sinks/process_exec.rs |

## DESIGN-doc gaps this research resolves (read first — load-bearing for planning)

The DESIGN doc pins the SECURITY model precisely but leaves three WIRING
questions unresolved that are load-bearing for a buildable plan. Each is
grounded in code read this session, not assumption:

1. **`args: Vec<String>` has no natural encoding in the current `PlanNode` data
   model.** `PlanNode.args: Vec<PlanArg>` and `PlanArg { name: String, value_id:
   ValueId }` bind exactly ONE name to ONE `ValueId`, which resolves to exactly
   ONE `ValueRecord.literal: String`
   (`crates/runtime-core/src/plan_node.rs:107-111`,
   `crates/executor/src/value_store.rs:61-84` — `literal: String`, not
   `Vec<String>`). There is no `Vec<ValueId>`-per-name capability anywhere in
   this model, and DESIGN §4.4 explicitly pins "no new `PlanNode` shape." A
   single `"args"` `PlanArg` therefore cannot hold N independent argv elements
   as N independently-taint-tracked values under the CURRENT model.
   **Recommendation (this research, MEDIUM confidence):** encode `args` as ONE
   `PlanArg` whose resolved literal is a JSON-serialized `Vec<String>`
   (`serde_json::to_string(&args)`); `crates/brokerd/src/sinks/process_exec.rs`
   deserializes it back to `Vec<String>` immediately before
   `Command::args(...)`, broker-side, never a shell join. The WHOLE JSON blob
   is one taint-tracked unit — content- and routing-sensitive as pinned by
   DESIGN §4.2 — so a tainted args array (any element) Blocks as a whole. This
   preserves DESIGN §1.5's "never shell-joined, each argv element separate at
   `execve` time" security property (the JSON round-trip happens broker-side,
   after I2 already cleared the value, and `Command::args()` still passes each
   deserialized element as a distinct argv slot) while staying inside the
   existing `PlanNode{sink,args}` shape. **Flag this decision explicitly to
   the human reviewer before Phase 32 starts** — it is a real design choice,
   not merely an implementation detail, even though it does not touch the
   security invariants DESIGN gates.
2. **No wire mechanism exists today to return a newly-minted `ValueId` from a
   sink invocation back to the worker.** `BrokerResponse::PlanNodeDecision {
   decision: ExecutorDecision }` (`crates/brokerd/src/proto.rs:219`) and
   `ExecutorDecision::Allowed` (`crates/runtime-core/src/executor_decision.rs:211`)
   both carry NO value payload. `file.create`/`email.send` never needed this —
   they are terminal effects. `process.exec` is NOT terminal: EXEC-02's entire
   point is that the captured output becomes usable as a LATER `PlanNode`'s
   arg (e.g. `file.write`'s `contents`), which requires the worker to learn the
   new `ValueId` after a successful exec. **This is a genuine protocol gap the
   DESIGN doc does not address** (it covers confinement/taint/I2, not the
   worker-facing response shape). **Recommendation:** add a field to
   `BrokerResponse::PlanNodeDecision`, e.g. `output_value_id: Option<ValueId>`
   populated ONLY when `plan_node.sink.0 == "process.exec"` AND the decision is
   `Allowed` — `None` for every other sink/decision (zero behavior change for
   `file.create`/`email.send`). `BrokerResponse::PlanNodeDecisionReduced {
   blocked: bool }` (planner-role wire, line 236) needs no equivalent change —
   the planner role never needs the raw output handle back (T-04-02 handle
   discipline already scopes literal access to the broker).
3. **Async output-capture + wall-clock timeout is genuinely new — the existing
   `child.kill()` precedent (`main.rs:372-378`) does not capture piped
   output, so it is not a complete template.** See Pattern 1 code example
   below for the concrete resolution (spawn via `tokio::process::Command`,
   which needs a Cargo feature this workspace does not currently enable
   anywhere — see Standard Stack).

## Existing-pattern → new-code map

| New symbol / file | Closest existing analog (file:line) | Reuse vs. new |
|---|---|---|
| `cli/caprun-exec-launcher/{Cargo.toml,src/main.rs}` | `cli/caprun-planner/{Cargo.toml,src/main.rs}` — sibling binary crate, resolved via `current_exe().parent().join(...)` | NEW crate, structural mirror |
| Broker spawn of the launcher (in `crates/brokerd/src/sinks/process_exec.rs`) | `cli/caprun/src/main.rs:320-328` (planner sidecar spawn, unconfined `Command::spawn()`) | Reuse the SPAWN shape; new: `Stdio::piped()` + async wait via `tokio::process` (see gap 3) |
| Launcher self-confinement (rlimits → Landlock narrow-allow → seccomp net-deny) | `crates/sandbox/src/lib.rs::apply_confinement()` (`lib.rs:56-61`) — SAME three-primitive ordering, different Landlock/seccomp variants | Reuse the ORDERING; new Landlock/seccomp variant constructors |
| Launcher's own `execve` of the target | `std::os::unix::process::CommandExt::exec()` — no existing call site in this codebase (confirmed: no `.exec()` or `.pre_exec()` calls found anywhere under `crates/`/`cli/` today) | NEW — first self-replacing exec in this codebase |
| `sandbox::landlock::exec_child_ruleset()` | `sandbox::landlock::deny_all_filesystem()` (`landlock.rs:16-32`) | NEW constructor, same file, same crate/ABI usage, opposite allow-rule posture |
| `sandbox::seccomp::exec_child_filter()` | `sandbox::seccomp::apply_worker_filter()` (`seccomp.rs:55-112`) | NEW constructor, same file, same `SeccompFilter` builder, drops the execve-deny rule pair |
| `sandbox::rlimits::apply_rlimits()` | itself (`rlimits.rs:13-27`) | REUSED UNCHANGED — no exec-specific rlimit variant needed |
| `crates/brokerd/src/quarantine.rs::mint_from_exec()` | `mint_from_read()` (`quarantine.rs:301-420`) | NEW function, SAME FILE, mirrors append-Event-then-mint ordering exactly |
| Call site of `mint_from_exec(...)` | `mint_from_read(...)` called from `server.rs`'s `ReportClaims` arm (`server.rs:1396-1494`, specifically the call at `1431`) | NEW call site, in `server.rs`, NOT in the new sink module (Gate-3-mandated locus, DESIGN §2.4/M1) |
| `crates/brokerd/src/sinks/process_exec.rs::invoke_process_exec()` | `crates/brokerd/src/sinks/file_create.rs::invoke_file_create()` (`file_create.rs:65-116`) | NEW module, mirrors two-phase durable-audit STRUCTURE; the spawn/capture logic itself has no analog (file.create is a single syscall, not a subprocess) |
| `crates/brokerd/src/sinks.rs` — `pub mod process_exec;` | `pub mod file_create;` (`sinks.rs:16`) | Trivial addition |
| `KNOWN_SINKS` entry for `"process.exec"` | the `"file.create"` entry (`sink_schema.rs:53-57`) | NEW table row, same struct shape |
| `sink_sensitivity.rs` — `PROCESS_EXEC_ROUTING_SENSITIVE` / `PROCESS_EXEC_CONTENT_SENSITIVE` consts + match arms | `FILE_CREATE_ROUTING_SENSITIVE`/`FILE_CREATE_CONTENT_SENSITIVE` (`sink_sensitivity.rs:63-85`) and their match arms (`:93-115`) | NEW consts + match arms, same file |
| `sink_sensitivity.rs::expected_role` `"process.exec"` arm | the `"file.create"` arm (`sink_sensitivity.rs:163-178`) | NEW match arm, same file |
| `TaintLabel::ExecRaw` | `TaintLabel::PathRaw` (`plan_node.rs:23`) — most recently added label, same shape | NEW enum variant, forces every non-wildcard `match TaintLabel` to add an arm (compiler-enforced — see Pitfall below for where those matches live) |
| `check-invariants.sh` Gate 3 `check_mint_token "mint_from_exec("` | the existing three `check_mint_token` calls (`check-invariants.sh:133-135`) | NEW line, same script, same two sanctioned loci (`quarantine.rs`, `server.rs`) |
| `BrokerResponse::PlanNodeDecision.output_value_id` | n/a — genuinely new field (gap 2 above) | NEW field on an existing struct-variant |
| `crates/sandbox/tests/exec_child_confinement.rs` | `crates/sandbox/tests/confinement_integration.rs` (`assert_probe_blocked` pattern, spawns `confine-probe`) | NEW test file; needs a NEW probe op or a dedicated launcher-invocation harness (see Validation Architecture) |
| `crates/brokerd/tests/process_exec_*.rs` | none directly — closest is `cli/caprun/tests/e2e.rs`'s live-flow shape | NEW integration test target |

## File-by-file change list

| File | Change |
|---|---|
| `cli/caprun-exec-launcher/Cargo.toml` | NEW. `[[bin]] name="caprun-exec-launcher"`. Deps: `sandbox` (path), `nix` (workspace, for raw `execve`/env if not using `CommandExt::exec`), `anyhow`. No `tokio` needed (single-shot, no async work post-confinement). |
| `cli/caprun-exec-launcher/src/main.rs` | NEW. Reads `EXEC_COMMAND`/`EXEC_ARGS_JSON`/`EXEC_CWD` env vars (mirrors worker's `INTENT` env var pattern, `main.rs:351`), calls `sandbox::rlimits::apply_rlimits()` → `sandbox::landlock::exec_child_ruleset(workspace_root)` → `sandbox::seccomp::exec_child_filter()` in that exact order, then `std::process::Command::new(command).args(args).current_dir(cwd)...exec()` (self-replacing; only returns on failure — exits non-zero with the io::Error printed to stderr, per DESIGN §5's "launcher aborts before execve on any confinement failure"). |
| `Cargo.toml` (workspace) | `members` gains `"cli/caprun-exec-launcher"`. |
| `cli/caprun/src/main.rs` | No spawn-site change needed here — the launcher is spawned from `crates/brokerd/src/sinks/process_exec.rs`, not from the CLI orchestrator (unlike the planner sidecar/worker, which the CLI spawns). Only reads `current_exe().parent()` resolution needs the launcher binary sibling-present at test/build time (Wave-0 gotcha, see Pitfalls). |
| `crates/sandbox/src/landlock.rs` | ADD `exec_child_ruleset(workspace_root: &Path) -> std::io::Result<()>` beside `deny_all_filesystem()`. Uses `path_beneath_rules` + `PathFd` (verified API, see Pattern 2). |
| `crates/sandbox/src/seccomp.rs` | ADD `exec_child_filter() -> std::io::Result<()>` beside `apply_worker_filter()`. Same `SeccompFilter` builder, same reused socket-deny rules, drops the two `SYS_execve`/`SYS_execveat` deny entries. |
| `crates/sandbox/Cargo.toml` | No change — `landlock`/`seccompiler`/`nix`/`libc` already Linux-gated deps here. |
| `crates/brokerd/src/quarantine.rs` | ADD `pub fn mint_from_exec(conn, key, store, session_id, combined_output: String, spawn_event_id, spawn_hash, parent_id, parent_hash) -> Result<(...)>` mirroring `mint_from_read`'s Step-1(build Event)→Step-2(append)→Step-3(mint) ordering. Fail-closed: this function has exactly one recognized shape (combined stdout+stderr bytes), per DESIGN §2.3 no branching classification — no `match` needed, but the doc comment must state the fail-closed discipline explicitly per that section. |
| `crates/brokerd/src/server.rs` | (a) inside `evaluate_plan_node_and_record`, add an `Allowed && plan_node.sink.0 == "process.exec"` arm mirroring the `file.create`/`email.send` arms at `862-951`: call `sinks::process_exec::invoke_process_exec(...)` (spawn+confine+capture+two-phase audit), THEN call `mint_from_exec(...)` with the captured output (call site MUST live here, not in the sink module — DESIGN §2.4/M1). (b) thread the returned `ValueId` out through a new local, and populate the new `output_value_id` field on the `PlanNodeDecision`/`PlanNodeDecisionReduced` response construction sites (gap 2). |
| `crates/brokerd/src/sinks.rs` | ADD `pub mod process_exec;`. |
| `crates/brokerd/src/sinks/process_exec.rs` | NEW. `invoke_process_exec()` — resolves `command`/`args`/`cwd` from the `ValueStore` (mirrors `file_create.rs::resolve_arg`), JSON-decodes `args` (gap 1), spawns the launcher via `tokio::process::Command` with `Stdio::piped()`, wraps the wait+read in `tokio::time::timeout`, applies the byte cap (fail-closed truncate/deny), and appends the two-phase `process_exited`/`process_spawn_failed` Event pair (mirrors `invoke_file_create`'s `sink_executed`/`sink_execution_failed` shape, `file_create.rs:84-116`). Returns `(event_id, hash, combined_output: String)` — NOT a `ValueId`; minting stays out of this file per DESIGN §2.4. |
| `crates/brokerd/src/proto.rs` | `BrokerResponse::PlanNodeDecision` gains `output_value_id: Option<runtime_core::plan_node::ValueId>` (gap 2). Every existing construction site of this variant needs a `None` added (grep for `PlanNodeDecision {` to enumerate — not read exhaustively this session; flag for Phase 32 to confirm count). |
| `crates/brokerd/Cargo.toml` | `tokio = { workspace = true, features = ["time", "process", "io-util"] }` — `"process"` is a NEW feature this crate does not currently request (workspace-level tokio only has `net, io-util, rt-multi-thread, macros`; brokerd already scopes `"time"` explicitly per its own comment at `Cargo.toml:13-20` — mirror that same scoped-addition discipline for `"process"`). `io-util` may already be workspace-unified in but confirm via `cargo build -p brokerd` in isolation, not `--workspace` (the crate's own existing comment on why: unified-feature masking). |
| `crates/runtime-core/src/plan_node.rs` | ADD `TaintLabel::ExecRaw` variant to the enum (`plan_node.rs:13-24`) AND to the exhaustive `is_untrusted()` match (`:41-50`, untrusted arm). Search the WHOLE crate tree for every other non-wildcard `match` over `TaintLabel` (compiler will catch missed ones on `cargo build`, but grep first — `grep -rn "match.*taint\|match self" crates/ | grep -i taintlabel` as a starting point) — DESIGN §11 n1 flags this explicitly. |
| `crates/executor/src/sink_schema.rs` | ADD `KNOWN_SINKS` entry: `SinkSchema { sink: "process.exec", allowed: &["command","args","cwd"], required: &["command"] }`. |
| `crates/executor/src/sink_sensitivity.rs` | ADD `PROCESS_EXEC_ROUTING_SENSITIVE: &[&str] = &["command","args","cwd"]` and `PROCESS_EXEC_CONTENT_SENSITIVE: &[&str] = &["command","args"]`; add `"process.exec"` match arms to `sink_effect_class` (→ `CommitIrreversible`), `is_routing_sensitive`, `is_content_sensitive`, `expected_role` (→ `"command"`/`"args"` = `None`; `"cwd"` = `Some(&["path","relative_path"])`, mirroring `file.create`'s `path` role list — `[ASSUMED]`, DESIGN doesn't pin `cwd`'s exact role list explicitly, only that it is routing-sensitive; recommend reusing the existing `"path"/"relative_path"` vocabulary since no new role-producing mint site is proposed). |
| `scripts/check-invariants.sh` | ADD `check_mint_token "mint_from_exec(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs"` immediately after the existing three calls (`:133-135`), in the SAME commit that introduces `mint_from_exec` (DESIGN §2.4 mandate). |
| `crates/sandbox/tests/exec_child_confinement.rs` | NEW. Negative-assertion tests mirroring `confinement_integration.rs`'s `assert_probe_blocked` shape — but this needs a NEW probe binary or invocation path (`confine-probe` today applies `apply_confinement()`, the WORKER variant; a new op or a new probe binary is needed to exercise `exec_child_ruleset`/`exec_child_filter` specifically — see Open Implementation Questions). |
| `crates/brokerd/tests/process_exec_*.rs` | NEW. Per-requirement integration coverage (see Validation Architecture). |

## Standard Stack

No new crates beyond one feature-flag addition. `[VERIFIED: Cargo.lock + workspace Cargo.toml, read this session]`

### Core (already vendored, reused)

| Library | Version (Cargo.lock) | Purpose | New usage this phase |
|---|---|---|---|
| `landlock` | 0.4.5 | Filesystem LSM restriction | NEW `exec_child_ruleset()` — first use of `PathBeneath`/`PathFd`/`path_beneath_rules`/`add_rules` in this codebase (previously only `deny_all_filesystem()`'s zero-allow-rule shape was used) |
| `seccompiler` | 0.5.0 | seccomp-bpf filter | NEW `exec_child_filter()` — same builder API already verified in `seccomp.rs`'s doc comment (`seccomp.rs:9`) |
| `nix` | 0.31.3 (features: `fs,socket,resource,process,signal,uio`) | `openat2`, rlimits, raw syscalls | Reused unchanged; the `process` feature is already enabled workspace-wide |
| `std::os::unix::process::CommandExt` | stdlib | `.exec()` (self-replacing execve) in the launcher | NEW — first `.exec()`/`.pre_exec()` call site in this codebase (confirmed absent by grep this session and by the DESIGN doc's own Round-1 review grep) |
| `tokio::process::Command` | 1.52.3, requires the `"process"` feature | Async spawn + async stdout/stderr capture + timeout-cancellable wait in `invoke_process_exec` | **NEW feature flag** — `"process"` is not currently enabled anywhere in this workspace (`Cargo.toml:18` workspace tokio features = `net, io-util, rt-multi-thread, macros`; `cli/caprun/Cargo.toml` adds `time`; `crates/brokerd/Cargo.toml` adds `time`). Phase 32 must add `"process"` (and confirm `"io-util"` is visible to a scoped `cargo build -p brokerd`, not just via workspace unification) to `crates/brokerd/Cargo.toml`'s own `tokio` line, mirroring the existing "declared here explicitly, not relying on incidental unification" discipline already documented at `Cargo.toml:13-20`. |

### Alternatives considered

| Instead of | Could use | Tradeoff |
|---|---|---|
| `tokio::process::Command` for the broker-side launcher spawn+capture | `std::process::Command::spawn()` + `tokio::task::spawn_blocking(move \|\| child.wait_with_output())` | Avoids a new tokio feature; BUT `Child::wait_with_output()` consumes `self`, so the async task holding the `spawn_blocking` handle has no way to `kill()` the child if `tokio::time::timeout` elapses — you would leak a running child + a blocked OS thread until it exits on its own. `tokio::process::Command` gives a cancellable, killable async `Child` natively (`child.kill().await` + a `tokio::select!` against a sleep), which is the correct shape for a wall-clock timeout that must actually terminate the child. Recommend `tokio::process::Command`. |
| Broker `pre_exec`-confines directly (DESIGN Option A) | Dedicated `caprun-exec-launcher` (DESIGN Option B, PINNED) | Already resolved at the design-gate — Option B is locked. Not re-litigated here. |

## Package Legitimacy Audit

**Not applicable.** No new external registry dependency this phase — every crate used (`landlock`, `seccompiler`, `nix`, `tokio`) is already vendored and was legitimacy-checked in prior milestones (v1.0 M0 substrate). The only change is an existing crate's Cargo feature flags (`tokio`'s `"process"` feature), which introduces no new supply-chain surface. `cli/caprun-exec-launcher` is a NEW workspace member but is caprun's own first-party code, not an external package.

## Architecture Patterns

### System Architecture Diagram — end-to-end `process.exec` flow

```
 Worker (kernel-confined)      Broker (brokerd, tokio async)         caprun-exec-launcher       Kernel
      │                              │                                (unconfined until        (Landlock,
      │ SubmitPlanNode{               │                                 self-confined below)     seccomp)
      │  sink:"process.exec",         │                                       │                     │
      │  args:[command,args,cwd]}     │                                       │                     │
      ├──────────────────────────────►│                                       │                     │
      │                          Step 0 schema gate (KNOWN_SINKS)             │                     │
      │                          Step 1c role check (cwd only)                │                     │
      │                          Collect-then-Block (command/args/cwd         │                     │
      │                            tainted → BlockedPendingConfirmation)      │                     │
      │                          ── if Allowed ──►                            │                     │
      │                     sinks::process_exec::invoke_process_exec()        │                     │
      │                       resolve command/args(JSON)/cwd from ValueStore  │                     │
      │                       tokio::process::Command::new(launcher_binary)   │                     │
      │                         .env("EXEC_COMMAND", cmd)                     │                     │
      │                         .env("EXEC_ARGS_JSON", json)                  │                     │
      │                         .env("EXEC_CWD", cwd)                         │                     │
      │                         .stdout/.stderr(Stdio::piped())               │                     │
      │                         .spawn()  ───────fork+exec (unconfined)──────►│                     │
      │                                                                  reads EXEC_* env vars       │
      │                                                                  apply_rlimits()             │
      │                                                                  exec_child_ruleset(ws) ────►│ Landlock
      │                                                                  exec_child_filter() ───────►│ seccomp
      │                                                                  Command::new(cmd).exec() ──►│ execve
      │                                                                       │            (self-replacing;
      │                                                                       │◄───────────target runs, confined:
      │                                                                       │             no net, workspace-
      │                                                                       │             scoped fs, one-shot exec)
      │                       tokio::select! { child.wait() vs timeout }      │                     │
      │                       read stdout/stderr concurrently, byte-capped    │                     │
      │                     ◄─── combined_output bytes ──────────────────────┤(inherited pipes)     │
      │                     append process_exited/process_spawn_failed Event │                     │
      │                       (in sinks/process_exec.rs, chained onto        │                     │
      │                        plan_node_evaluated head)                     │                     │
      │                     mint_from_exec(quarantine.rs) — CALLED FROM       │                     │
      │                       server.rs (NOT the sink module — Gate 3):      │                     │
      │                       new Event{"process_exited"} already appended   │                     │
      │                       above by the sink; mint uses that event's id   │                     │
      │                       as provenance_chain[0], taint=[ExternalUntrusted,│                    │
      │                       ExecRaw], origin_role="exec_output"            │                     │
      │◄── PlanNodeDecision{Allowed, output_value_id: Some(vid)} ────────────┤                     │
      │  (worker never sees raw bytes — only the opaque ValueId handle)      │                     │
```

Downstream: a LATER `PlanNode` (e.g. the fs write/edit sink's `contents`, Phase
33) references that `ValueId`. `submit_plan_node`'s existing collect-then-Block
loop Blocks it there if routed to a sensitive slot — no new I2 logic, table
entries only (unchanged from DESIGN §4.1).

### Pattern 1: `caprun-exec-launcher` — dedicated self-confining spawn binary

**What:** a new sibling binary, spawned unconfined by the broker (mirrors
`caprun-planner`'s spawn shape exactly), which reads its target command from
env vars, applies confinement to ITSELF (same three-primitive ordering as
`apply_confinement()`), then self-replaces via `execve` into the target.

**Grounded template — `caprun-planner`'s existing structure (mirror this
exactly for the new crate):**
```toml
# Source: cli/caprun-planner/Cargo.toml (existing sibling-binary crate this
# phase's cli/caprun-exec-launcher/Cargo.toml should mirror)
[package]
name             = "caprun-exec-launcher"
version.workspace  = true
edition.workspace  = true
license.workspace  = true

[dependencies]
sandbox = { path = "../../crates/sandbox" }
anyhow  = { workspace = true }
serde_json = { workspace = true }   # decode EXEC_ARGS_JSON
```
The broker resolves this binary the SAME way `main.rs` resolves
`caprun-planner`/`caprun-worker` — `std::env::current_exe().parent().join(...)`
(`main.rs:315-319,335-339`). Because `caprun-exec-launcher` is spawned from
`crates/brokerd` (an async tokio task inside `brokerd`, not from `cli/caprun`'s
`main()`), `current_exe()` inside a library crate still resolves to the
CURRENTLY RUNNING PROCESS's own binary (`caprun`, since brokerd is a library
linked into the `caprun` binary) — the resolution logic is identical, just
invoked from `crates/brokerd/src/sinks/process_exec.rs` instead of
`cli/caprun/src/main.rs`.

**Launcher self-confinement + exec — grounded in the proven
`apply_confinement()` ordering (`crates/sandbox/src/lib.rs:56-61`) applied to
the new exec-child primitives:**
```rust
// cli/caprun-exec-launcher/src/main.rs (sketch)
use std::os::unix::process::CommandExt;

fn main() -> ! {
    let command = std::env::var("EXEC_COMMAND").unwrap_or_else(|_| {
        eprintln!("[caprun-exec-launcher] EXEC_COMMAND env var required");
        std::process::exit(2);
    });
    let args: Vec<String> = std::env::var("EXEC_ARGS_JSON")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let cwd = std::env::var("EXEC_CWD").ok();
    let workspace_root = std::env::var("EXEC_WORKSPACE_ROOT")
        .expect("EXEC_WORKSPACE_ROOT env var required");

    // SAME three-primitive ordering as sandbox::apply_confinement(), applied
    // to the exec-child variants — rlimits, then Landlock, then seccomp.
    sandbox::rlimits::apply_rlimits().expect("apply_rlimits");
    sandbox::landlock::exec_child_ruleset(std::path::Path::new(&workspace_root))
        .expect("exec_child_ruleset");
    sandbox::seccomp::exec_child_filter().expect("exec_child_filter");

    // Self-replacing exec — only returns on failure (io::Error). The launcher
    // process image IS the target after this call succeeds; stdout/stderr
    // fds set up by the BROKER's original spawn (Stdio::piped()) are
    // inherited across exec unchanged (standard fd-inheritance semantics —
    // no FD_CLOEXEC on fd 1/2 unless explicitly set), so the broker's
    // existing pipe reader sees the TARGET's output with zero extra plumbing.
    let mut cmd = std::process::Command::new(&command);
    cmd.args(&args);
    if let Some(dir) = &cwd {
        cmd.current_dir(dir);
    }
    let err = cmd.exec(); // never returns on success
    eprintln!("[caprun-exec-launcher] exec failed: {err}");
    std::process::exit(2);
}
```
`[ASSUMED — MEDIUM confidence]`: `EXEC_WORKSPACE_ROOT` as a plain path string
env var, resolved by the launcher itself to build the Landlock
`WorkspaceRoot`-equivalent allow-rule (the launcher cannot reuse the broker's
in-process `WorkspaceRoot` dirfd object — it is a different process — so it
must independently `PathFd::new(workspace_root)` the path). This is the
launcher↔broker channel choice (env-var only, no UDS) — see Open Implementation
Questions; recommended because this is a single one-shot spawn-confine-exec
with no interactive protocol need (unlike the worker, which maintains an
ongoing session).

### Pattern 2: `exec_child_ruleset()` — Landlock narrow allow-list (VERIFIED against vendored crate source)

`[VERIFIED: local landlock-0.4.5 crate source,
~/.cargo/registry/src/index.crates.io-*/landlock-0.4.5/examples/sandboxer.rs
and src/fs.rs, read this session]` — this is NOT training-knowledge-only; the
exact API (`path_beneath_rules`, `PathFd`, `PathBeneath`, `add_rules`) was
confirmed present and used in this shape in the vendored crate's own official
example, which ships with the exact version (`0.4.5`) this workspace pins.

```rust
// crates/sandbox/src/landlock.rs — NEW, beside deny_all_filesystem()
// API confirmed against the vendored crate's own examples/sandboxer.rs
#[cfg(target_os = "linux")]
pub fn exec_child_ruleset(workspace_root: &std::path::Path) -> std::io::Result<()> {
    use landlock::{
        path_beneath_rules, Access, AccessFs, ABI, PathFd, Ruleset, RulesetAttr,
        RulesetCreatedAttr,
    };
    let map_err = |e: std::fmt::Arguments| std::io::Error::new(std::io::ErrorKind::Other, e.to_string());

    let abi = ABI::V3;
    // System paths: ReadFile + Execute only (loading + running the target
    // binary and its shared libs). Exact literal path list is an Open Item
    // (§8 of the DESIGN doc) — resolved against the verification container's
    // real layout at Phase 32 implementation time (see Open Questions).
    let system_paths = ["/usr", "/bin", "/lib", "/lib64"];
    let system_access = AccessFs::ReadFile | AccessFs::Execute;

    // Workspace: ReadFile + WriteFile only — no Execute (never run a
    // worker-planted binary), matching "narrowest that works."
    let workspace_access = AccessFs::ReadFile | AccessFs::WriteFile;

    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .create()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .add_rules(path_beneath_rules(system_paths.iter(), system_access))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .add_rules(path_beneath_rules(
            std::iter::once(PathFd::new(workspace_root).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))
            })?),
            workspace_access,
        ))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .restrict_self()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    eprintln!("[caprun-exec-launcher] Landlock exec_child_ruleset status: {:?}", status.ruleset);
    Ok(())
}
```
Note `handle_access(AccessFs::from_all(abi))` is REUSED verbatim from
`deny_all_filesystem()` (`landlock.rs:22`) — this declares the FULL set of
access rights the ruleset will govern; `add_rules` then carves out the
specific allow-exceptions. This mirrors the vendored example's
`ruleset.handle_access(AccessFs::from_all(abi))?` + `.add_rules(...)` shape
exactly (`sandboxer.rs:132,178-179`).

### Pattern 3: `exec_child_filter()` — seccomp, reused net-deny, no execve-deny

```rust
// crates/sandbox/src/seccomp.rs — NEW, beside apply_worker_filter()
// Same seccompiler 0.5.0 builder already VERIFIED in this file's own header
// comment (seccomp.rs:9) — this function drops the two execve-deny entries
// from apply_worker_filter()'s rule vec and keeps everything else identical.
#[cfg(target_os = "linux")]
pub fn exec_child_filter() -> std::io::Result<()> {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule,
    };
    use std::convert::TryInto;

    let filter = SeccompFilter::new(
        vec![
            // NO execve/execveat deny here — the launcher's own upcoming
            // exec() must succeed (DESIGN §1.4 B1 resolution: no seccomp
            // recursion-deny is realizable with a stateless BPF program;
            // grandchild bound is Landlock Execute allow-list + this same
            // persistent net-deny, not a seccomp execve-deny).
            (
                libc::SYS_socket,
                vec![
                    SeccompRule::new(vec![SeccompCondition::new(
                        0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, libc::AF_INET as u64,
                    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?])
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?,
                    SeccompRule::new(vec![SeccompCondition::new(
                        0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, libc::AF_INET6 as u64,
                    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?])
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?,
                ],
            ),
        ].into_iter().collect(),
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        std::env::consts::ARCH.try_into().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?,
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    let program: BpfProgram = filter.try_into().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;
    seccompiler::apply_filter(&program).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))
}
```

### Pattern 4: `mint_from_exec` — sole mint site, non-stapled taint (mirrors `mint_from_read` exactly)

```rust
// crates/brokerd/src/quarantine.rs — NEW, beside mint_from_read (Pattern 3
// template already read in full this session, quarantine.rs:301-420)
pub fn mint_from_exec(
    conn: &rusqlite::Connection,
    key: &[u8],
    store: &mut ValueStore,
    session_id: Uuid,
    combined_output: String,
    parent_id: Option<Uuid>,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId)> {
    // Step 1: build the process_exited Event FIRST — taint set HERE, never
    // at a downstream sink (anti-stapling, mirrors mint_from_read Step 1).
    let event_id = Uuid::new_v4();
    let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw];
    let event = Event::new(
        event_id, parent_id, session_id, "confined-exec-launcher".into(),
        "process_exited".into(), Utc::now(), taint.clone(),
    );
    // Step 2: append, obtaining the row hash.
    let exec_hash = append_event(conn, key, &event, parent_hash)?;
    // Step 3: mint — provenance_chain[0] == event_id (genuine-taint anchor).
    let value_id = store
        .mint(combined_output, taint, vec![event_id], Some("exec_output".to_string()))
        .map_err(|e| anyhow::anyhow!("mint invariant: {e:?}"))?;
    Ok((event_id, exec_hash, value_id))
}
```
Note: unlike `mint_from_read`, this function does NOT demote the session
(`mint_from_read`'s Step 4 is I1-specific to the `ReportClaims`/worker-report
path, per T-04-03/T-04-04). `process.exec`'s taint is set structurally, not
via a worker self-report — no I1 trust-flip is implicated here; this mirrors
`mint_from_derivation`'s "does NOT demote" precedent
(`quarantine.rs:583-585`), not `mint_from_read`'s. `[ASSUMED — MEDIUM
confidence, not explicitly stated in DESIGN doc]`: flag for confirmation
during Phase 32 — a fresh adversarial reviewer should specifically check
whether `process.exec` output ought to ALSO demote the session (an argument
could be made that ANY externally-sourced untrusted content should demote,
mirroring `mint_from_read`). Recommend treating this as an open question for
the Phase 32 plan review, not silently deciding either way.

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---|---|---|---|
| Kernel confinement of the exec child | Custom seccomp filter builder, ptrace sandbox, container shell-out | `landlock` 0.4.5 (`path_beneath_rules`/`PathFd`) + `seccompiler` 0.5.0, extending the SAME two files (`landlock.rs`/`seccomp.rs`) already in the TCB | Both APIs are already verified-against-source in this repo; a parallel confinement stack doubles the TCB surface a fresh adversarial review must trace |
| Wall-clock timeout | Custom `SIGALRM`/timer thread inside the launcher | `tokio::time::timeout` wrapping the broker's async wait on `tokio::process::Child` | Cancellable, kill-capable, reuses the proven `child.kill()` teardown discipline already exercised at `main.rs:372-378` |
| Output-capture pipe deadlock avoidance | Sequential blocking reads of stdout then stderr | `tokio::process::Command`'s async `Stdio::piped()` + concurrent `AsyncReadExt::read_to_end` on both streams (or `tokio::join!`) | A full pipe buffer on one stream while blocking-reading the other synchronously is a classic deadlock; async concurrent reads (or `std::process::Child::wait_with_output`'s internal thread-per-pipe approach) avoid it — but `wait_with_output` consumes `self`, defeating a cancellable timeout (see Standard Stack "Alternatives") |
| Path traversal prevention for `cwd` resolution | Manual `..`-stripping | Same `RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS` discipline the read/create/write paths already use (`workspace.rs:94-96,139-142`) | Consistency + kernel-atomic resolution, no new escape surface |
| Process-output "is this safe" classification | A heuristic/regex content scanner | Nothing — taint unconditionally as `[ExternalUntrusted, ExecRaw]`, let I2 enforce at the consuming sink | This project's entire model is taint-by-origin, not taint-by-content-inspection (`DESIGN-taint-model.md`) |

**Key insight:** every new mechanism here is a narrow extension of an already-
verified pattern EXCEPT the launcher's own `execve` and the async
spawn/capture/timeout composition — those two are genuinely novel to this
codebase and deserve the most adversarial-review scrutiny in Phase 32.

## Common Pitfalls / Landmines

### Pitfall 1: `deny_all_filesystem()` reused verbatim for the exec child
Blocks the launcher's own target-binary load (`EACCES`/`ENOEXEC`) — it has
zero allow-rules including `Execute`. Use the new `exec_child_ruleset()`
instead. (Carried from DESIGN §9/RESEARCH-31 Pitfall 1, re-confirmed against
`landlock.rs:16-32` this session.)

### Pitfall 2: `mint_from_exec` escaping Gate 3's call-site restriction
A mint call added inside `crates/brokerd/src/sinks/process_exec.rs` (NOT
`quarantine.rs`/`server.rs`) silently escapes the mandated Gate 3 extension —
`check-invariants.sh`'s `check_mint_token` only scans the two sanctioned loci
(`check-invariants.sh:133-135` present state). Extend Gate 3 in the SAME
commit that adds `mint_from_exec`, and confirm the call site is in
`server.rs`.

### Pitfall 3: the sibling-binary `cargo build` gotcha (this project's own standing lesson)
A bare `cargo test --workspace` does not reliably place a bin-only sibling
crate's nice-named `target/<profile>/<bin>` copy — confirmed for
`caprun-planner` (`scripts/mailpit-verify.sh:51-66`'s own documented finding).
`caprun-exec-launcher` is the SAME shape (bin-only, resolved via
`current_exe().parent().join(...)`) — the SAME gotcha WILL recur. Every
verification invocation for this phase MUST run `cargo build --workspace`
(or the `mailpit-verify.sh` default, which already does) BEFORE any test that
spawns `caprun-exec-launcher`.

### Pitfall 4: `cfg(linux)` test-blindness (this project's own standing lesson)
`cargo test` on macOS compiles ZERO `#[cfg(target_os="linux")]` test targets —
a green Mac build can hide broken Linux-only call sites (documented
project-wide precedent: 279/279 macOS-green once hid 8 broken sites at Phase
28). **Phase 32 verification MUST run `cargo build --tests --keep-going`
inside the Linux container (Colima+Docker, via `scripts/mailpit-verify.sh` or
a scoped `MAILPIT_VERIFY_CMD` override) to enumerate every Linux compile error
BEFORE claiming the phase compiles**, not merely rely on a Mac-green `cargo
build --workspace`.

### Pitfall 5: naively wrapping a blocking `wait_with_output()` in `tokio::time::timeout`
`std::process::Child::wait_with_output()` consumes `self` — once moved into a
`spawn_blocking` closure, the calling async task has no handle left to
`kill()` the child if the timeout elapses. A naive
`tokio::time::timeout(dur, spawn_blocking(move || child.wait_with_output()))`
will, on timeout, stop AWAITING the future but NOT actually kill the child or
the blocking OS thread — both leak. Use `tokio::process::Command`'s async
`Child` (kill-capable, cancellable) instead (Pattern 1/Standard Stack).

### Pitfall 6: async-signal-safety concerns do NOT apply on the pinned Option-B path
Do not import Option A's `pre_exec`-closure async-signal-safety residual
(DESIGN §2.5/§9) into the Phase 32 implementation — it was explicitly RETIRED
by the Round-1 gate resolution (M3). The launcher's confinement calls
(`apply_rlimits`/`exec_child_ruleset`/`exec_child_filter`) run in the
launcher's OWN address space, long after its own `fork()`, exactly like the
worker's `apply_confinement()` — this is ordinary, safe, non-`pre_exec` code.
No `pre_exec` closure exists anywhere in this phase's design.

### Pitfall 7: `args` JSON round-trip must not silently become a shell join
Because the JSON-encoded `args` blob (this research's gap-1 resolution) is a
single string in transit, a careless implementation could be tempted to pass
it to `Command::new("sh").arg("-c").arg(joined_string)` for "simplicity."
This MUST NOT happen — deserialize the JSON back to `Vec<String>` and pass
each element to `Command::args(&[...])` (or the launcher's
`.args(&args)`/`.exec()` argv array) exactly as DESIGN §1.5 pins: never
through a shell interpreter.

### Pitfall 8: `output_value_id` must default to `None` for every non-exec sink
Adding a field to `BrokerResponse::PlanNodeDecision` (gap 2) touches EVERY
existing construction site of that variant. Missing a `None` at one of them
is a compile error (good — Rust forces it), but a careless
`#[serde(default)]`-style shortcut on the wire type could silently paper over
a missed site on the wire-deserialize side for OLDER clients. Since this is a
single-binary-version project (no cross-version wire compatibility
requirement documented anywhere in PLAN.md/REQUIREMENTS.md), a plain required
field (no `#[serde(default)]`) is the fail-closed choice — every construction
site must explicitly state its intent.

## Code Examples

### `KNOWN_SINKS` entry (exact table row)
```rust
// crates/executor/src/sink_schema.rs — add to KNOWN_SINKS
SinkSchema {
    sink: "process.exec",
    allowed: &["command", "args", "cwd"],
    required: &["command"],
},
```

### `sink_sensitivity.rs` additions
```rust
// crates/executor/src/sink_sensitivity.rs
pub const PROCESS_EXEC_ROUTING_SENSITIVE: &[&str] = &["command", "args", "cwd"];
pub const PROCESS_EXEC_CONTENT_SENSITIVE: &[&str] = &["command", "args"];

// sink_effect_class match arm:
"process.exec" => EffectClass::CommitIrreversible,

// is_routing_sensitive match arm:
"process.exec" => PROCESS_EXEC_ROUTING_SENSITIVE.contains(&arg_name),

// is_content_sensitive match arm:
"process.exec" => PROCESS_EXEC_CONTENT_SENSITIVE.contains(&arg_name),

// expected_role match arm:
"process.exec" => match arg_name {
    "command" | "args" => None, // DESIGN §4.2/M2 — no origin_role-producing
                                  // mint site for a trusted-authored command;
                                  // Some(...) would fail-closed-Deny the
                                  // legitimate value at Step 1c. The Block
                                  // property comes from routing+content
                                  // sensitivity, not this gate.
    "cwd" => Some(&["path", "relative_path"]), // [ASSUMED] mirrors
                                                 // file.create's `path` role
                                                 // list — DESIGN pins cwd as
                                                 // routing-sensitive but does
                                                 // not pin its expected_role
                                                 // list explicitly.
    _ => None,
},
```

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Cargo built-in (`cargo test`), workspace-wide `cargo test --workspace --no-fail-fast` |
| Config file | none — root `Cargo.toml` |
| Quick run (Mac, per-task commit) | `cargo build --workspace` (compiles the new crate + confirms the sibling-binary placement; `#[cfg(target_os="linux")]` tests do not run on Mac — expected, not a gap) |
| Linux compile-check (mandatory, per Pitfall 4) | `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo build --tests --keep-going` — enumerate every Linux-gated compile error before claiming the phase builds |
| Full Linux suite | `bash scripts/mailpit-verify.sh` (per CLAUDE.md: mandatory from Phase 16 onward, since a benign exec that itself triggers `email.send` could hit real SMTP — though `process.exec` itself has no SMTP surface, the shared verification harness stays the default) |
| Scoped Linux run | `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p brokerd process_exec' bash scripts/mailpit-verify.sh` (or `-p sandbox exec_child`) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test type | Command | File exists? |
|---|---|---|---|---|
| EXEC-01 | `process.exec` spawns via broker+launcher, never worker execve | integration (Linux) | `cargo test -p sandbox exec_child` (new) + `cargo test -p brokerd process_exec_spawns_launcher` (new) | ❌ Wave 0 |
| EXEC-02 | exec output taint-minted, provenance_chain[0] == process_exited event id, no stapling | unit (Mac-compiling) | `cargo test -p brokerd mint_from_exec_anchor_identity` — mirrors `mint_from_read_anchor_identity` (`quarantine.rs:855-883`) exactly | ❌ Wave 0 |
| EXEC-03 | tainted exec-output → sensitive sink arg → deterministic Block, verify_chain true | integration (Linux, §9-style) | `cargo test -p caprun --test s9_process_exec_block` (new, mirrors the existing `s9_live_block`/`s9_control_ab_taint_driven` shape referenced in `mailpit-verify.sh:45`) | ❌ Wave 0 |
| EXEC-04 | kernel-confined child (Landlock+seccomp+rlimits+timeout+byte-cap), fail-closed schema, durable spawn+exit audit | integration (Linux, negative) | `cargo test -p sandbox exec_child_confinement` (negative assertions, mirrors `confinement_integration.rs`) + `cargo test -p executor process_exec_schema` (Mac-compiling, schema/sensitivity table unit tests) | ❌ Wave 0 |

### Sampling rate
- **Per task commit:** `cargo build --workspace` (Mac) — confirms compile + sibling-binary placement.
- **Per wave merge:** `cargo build --tests --keep-going` inside `rust:1` (Linux compile enumeration, Pitfall 4) — MANDATORY before declaring any wave touching `crates/sandbox`/`crates/brokerd`/`crates/executor` complete.
- **Phase gate:** `bash scripts/mailpit-verify.sh` full suite green (or a scoped `MAILPIT_VERIFY_CMD` covering every new test target named above) — true-exit-before-pipe, asserted on named test counts, never `script | tail` exit-code laundering (this project's own standing incident).
- **Full live proof (§9-style composed acceptance) is Phase 34's scope**, per the milestone's own phase split (LIVE-01/02) — Phase 32's gate is the per-requirement integration tests above, not the full composed live acceptance.

### Wave 0 gaps
- [ ] `crates/sandbox/tests/exec_child_confinement.rs` — negative assertions for `exec_child_ruleset`/`exec_child_filter` (needs a NEW probe mechanism — see Open Implementation Questions, since `confine-probe` today only exercises the WORKER's `apply_confinement()`, not the new exec-child variants)
- [ ] `crates/brokerd/tests/process_exec_spawn.rs` — spawn/capture/timeout/byte-cap coverage
- [ ] `crates/brokerd/tests/process_exec_taint.rs` — `mint_from_exec` unit tests (Mac-compiling)
- [ ] `crates/executor/tests/process_exec_schema.rs` OR inline unit tests in `sink_schema.rs`/`sink_sensitivity.rs` (Mac-compiling, mirrors the existing inline `#[cfg(test)]` modules in both files)
- [ ] `cli/caprun/tests/s9_process_exec_block.rs` — the EXEC-03 genuine-non-stapled-taint acceptance test
- [ ] `scripts/check-invariants.sh` Gate 3 extension (same commit as `mint_from_exec`)

## Security Domain

`security_enforcement` is absent from `.planning/config.json` → enabled by
default; this section is required.

### Applicable ASVS categories

| ASVS category | Applies | Standard control |
|---|---|---|
| V1 Architecture | yes | Design-gate-first discipline already satisfied (Phase 31 cleared); this phase is a mechanical realization, not a new architectural decision |
| V4 Access Control | yes | `KNOWN_SINKS` schema gate + sensitivity/role tables, table-entries-only extension (unchanged enforcement logic) |
| V5 Input Validation | yes | argv-array (never shell-string) command construction closes shell-injection by construction (Pitfall 7); `openat2` path resolution for `cwd` closes path-traversal by construction |
| V6 Cryptography | no | No new key material this phase |
| V8 Data Protection (informative) | yes | Exec-output taint labeling (`ExecRaw`+`ExternalUntrusted`) — untrusted process output is never trusted by default |

### Known threat patterns

| Pattern | STRIDE | Mitigation |
|---|---|---|
| Arbitrary command execution via tainted `command`/`args` | Elevation of Privilege | `command`/`args` routing- AND content-sensitive (DESIGN §4.2) — tainted value Blocks before spawn |
| Shell metacharacter injection | Tampering | argv array, never `sh -c` (Pitfall 7) |
| JSON-encoding round-trip smuggling a shell string into a single `args` element | Tampering | Each deserialized `Vec<String>` element still passes through `Command::args()`'s per-argv-slot mechanism — no re-parsing of the JSON content as shell syntax anywhere |
| Exec child resource exhaustion (fork bomb, infinite loop, unbounded output) | Denial of Service | `RLIMIT_AS`/`RLIMIT_CPU` (reused) + NEW wall-clock `tokio::time::timeout` + NEW captured-output byte cap (recommend 10 MiB, fail-closed truncate/deny) |
| Network exfiltration from inside the exec child or a grandchild | Information Disclosure | Reused seccomp `socket(AF_INET/AF_INET6)` deny, persists across `execve` (kernel BPF inheritance — DESIGN §9 A5, unverified against kernel source this session, standard assumption) |
| Grandchild `execve` of an un-enumerated binary | Elevation of Privilege | Bounded by the Landlock `Execute` allow-list (system paths only) — no seccomp recursion-deny exists (unrealizable, DESIGN B1) |
| Leaked child/thread on timeout (Pitfall 5) | Denial of Service (resource leak) | `tokio::process::Command`'s cancellable async `Child::kill()`, not a blocking `wait_with_output()` |

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|---|---|---|
| A1 | `args` should be encoded as a single JSON-serialized literal in one `PlanArg` (gap 1 resolution) | DESIGN-doc gaps §1, Code Examples | If a different encoding is chosen (e.g. positional `arg0..argN` names), the schema table/sensitivity table/sink module all need a different shape than sketched here — flag to human reviewer before Phase 32 plan is finalized |
| A2 | `mint_from_exec` does NOT demote the session (unlike `mint_from_read`) | Pattern 4 | If wrong, a session that execs a command and never explicitly demotes could retain `Active` status inappropriately for a CommitIrreversible-adjacent action; low risk since I2's per-arg Block still fires regardless of session status, but worth an explicit design decision, not a silent default |
| A3 | `cwd`'s `expected_role` should reuse `["path","relative_path"]` (file.create's role list) | Code Examples, File-by-file §sink_sensitivity.rs | If wrong (e.g. cwd needs its own role or should stay `None`), a legitimate cwd value could fail-closed-Deny at Step 1c — mirrors the exact HARDEN-05/M2 trap this project has hit twice before (`sink_sensitivity.rs:163-176` comment, DESIGN §4.2) |
| A4 | Landlock system-path allow-list (`/usr,/bin,/lib,/lib64`) is sufficient for the `rust:1` verification container's candidate test commands | Pattern 2 | DESIGN §8 item 1 explicitly defers this — must be confirmed via `ldd`/`which` against the actual chosen test commands before Phase 32 lands, not assumed from this generic list |
| A5 | `tokio::process::Command`'s `Child::kill()` + `wait()` combination is cancel-safe and correctly reaps the process (no zombie) when raced against `tokio::time::timeout` via `tokio::select!` | Standard Stack, Pattern 1 | This is standard, well-documented tokio behavior (public API, unlikely to have changed materially across tokio 1.x) but was not independently re-verified against tokio 1.52.3's exact source this session — `[ASSUMED]`, MEDIUM confidence, recommend a smoke test early in Phase 32 rather than discovering an issue at the full-suite gate |
| A6 | Seccomp filter persists across the launcher's own `execve` (net-deny still active post-exec) | Pattern 3, carried from DESIGN §9 A5 | Load-bearing for the B1 resolution (grandchild egress bound). Standard Linux kernel seccomp-BPF inheritance semantics, not re-verified against kernel source this session — MUST be confirmed empirically in Phase 32 (e.g. a negative test that execs a target which itself attempts `socket(AF_INET)` and confirms EPERM) |

## Open Implementation Questions

1. **Exact Landlock system-path allow-list for the `rust:1` verification
   container.** DESIGN §8 item 1 defers this explicitly. Recommendation:
   resolve via `ldd $(which <candidate-test-command>)` inside the container at
   Phase 32 authoring time, hardcode the resulting list (never a runtime
   `PATH` walk — matches this project's "explicit hardcoded, no dynamic
   registry" discipline).
2. **Launcher↔broker channel: env-var-only (this research's recommendation) vs.
   a UDS round-trip (DESIGN §1.3's "env-var/UDS channel" phrasing left both
   open).** Recommend env-var-only (Pattern 1) — simpler, matches the worker's
   own `INTENT` env var precedent, and this is a one-shot spawn-confine-exec
   with no need for an interactive protocol. Flag to a human reviewer as a
   real (if low-stakes) decision, not silently assumed.
3. **`RequestFd` per-session read-count upper bound's exact numeric value** —
   DESIGN §8 item 2 explicitly defers this to Phase 33 (fs breadth), not
   Phase 32 — noted here only so Phase 32's plan does not accidentally absorb
   Phase 33 scope.
4. **How does `crates/sandbox/tests/exec_child_confinement.rs` actually drive
   `exec_child_ruleset()`/`exec_child_filter()`?** The existing
   `confine-probe` binary applies `apply_confinement()` (the WORKER variant)
   to itself — there is no existing probe path for the NEW exec-child
   variants. Two options: (a) add a NEW op to `confine-probe` (e.g.
   `confine-probe exec-child <target>`) that applies the exec-child
   primitives then executes a supplied target and reports whether
   confinement held; (b) directly invoke `caprun-exec-launcher` itself as the
   test subject (spawn it with a benign `EXEC_COMMAND` that probes forbidden
   ops, mirroring `confine-probe`'s own op-based design). Recommend (b) —
   testing the ACTUAL launcher binary is more representative than a parallel
   probe that could drift from the real implementation. Not resolved by
   DESIGN doc; flag for the Phase 32 plan to decide explicitly.
5. **Captured-output byte-cap exact value** — DESIGN §8 item 4 defers this;
   this research recommends 10 MiB (matching the DESIGN doc's own "order of
   10 MiB" suggestion, §1.4) as a starting default, fail-closed
   truncate-or-deny (not fail-open).

## Sources

### Primary (HIGH confidence — direct code/crate-source reads this session)
- `planning-docs/DESIGN-effect-breadth-exec.md` (full read, all 11 sections + amendments)
- `planning-docs/DESIGN-GATE-RECORD-v1.7.md` (full read, all findings + resolutions)
- `.planning/phases/31-effect-breadth-design-gate/31-RESEARCH.md` (full read)
- `crates/sandbox/src/{lib.rs, landlock.rs, seccomp.rs, rlimits.rs, bin/confine-probe.rs}`
- `crates/sandbox/tests/confinement_integration.rs`
- `crates/sandbox/Cargo.toml`
- `cli/caprun/src/main.rs` (lines 280-410, spawn sites + teardown)
- `cli/caprun/Cargo.toml`, `cli/caprun-planner/{Cargo.toml,src/main.rs}` (full)
- `Cargo.toml` (workspace), `Cargo.lock` (landlock/seccompiler/nix/libc/tokio version rows)
- `~/.cargo/registry/src/index.crates.io-*/landlock-0.4.5/{examples/sandboxer.rs, src/fs.rs, src/ruleset.rs}` — vendored crate source, NOT training knowledge
- `crates/brokerd/src/{quarantine.rs (full), server.rs (lines 634-1500), sinks.rs, sinks/file_create.rs (full), proto.rs (partial grep)}`
- `crates/brokerd/Cargo.toml`
- `crates/runtime-core/src/{plan_node.rs (full), executor_decision.rs (partial)}`
- `crates/executor/src/{lib.rs (full), sink_schema.rs (full), sink_sensitivity.rs (full), value_store.rs (partial)}`
- `crates/adapter-fs/src/workspace.rs` (full), `crates/adapter-fs/Cargo.toml`
- `scripts/check-invariants.sh` (Gate 1-3), `scripts/mailpit-verify.sh` (full)
- `.planning/config.json`

### Secondary (MEDIUM confidence)
- `tokio::process::Command`'s async spawn/kill/wait behavior — well-known stable public API, version-consistent across tokio 1.x, not independently re-read from tokio 1.52.3's own source this session

### Tertiary (LOW confidence, flagged `[ASSUMED]` inline)
- Seccomp-BPF filter persistence across `execve` (A6) — standard kernel behavior, not re-verified against kernel source
- Landlock ABI/kernel version floors — carried unchanged from Phase 31's research, not re-verified this session

## Metadata

**Confidence breakdown:**
- Existing-pattern grounding (spawn precedents, mint-site shape, sink-table
  extension, workspace resolution): HIGH — every claim traces to a specific
  file:line read this session, including the vendored `landlock` crate source
  itself.
- New-mechanism grounding (launcher `.exec()`, `tokio::process` async
  spawn/capture/timeout, the `args` JSON-encoding resolution, the
  `output_value_id` wire-protocol gap): MEDIUM — reasoned from verified
  primitives but genuinely novel composition; each is flagged with an explicit
  recommendation and risk statement, not presented as settled.
- Kernel-semantics assumptions (seccomp persistence across exec, Landlock ABI
  floors): LOW-MEDIUM — carried from Phase 31, unchanged, still unverified
  against a kernel source.

**Research date:** 2026-07-17
**Valid until:** ~14 days (fast-moving relative to this project's pace; re-verify file:line citations if Phase 32 begins substantially later, per this project's own convention).
