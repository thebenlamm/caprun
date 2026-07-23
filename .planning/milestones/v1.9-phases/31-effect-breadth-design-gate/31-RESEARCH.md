# Phase 31: Effect-Breadth Design Gate - Research

**Researched:** 2026-07-17
**Domain:** Kernel confinement (Landlock/seccomp-bpf) for a broker-spawned arbitrary child process; filesystem write/edit breadth via `openat2`; taint-mint + I2/slot-type-binding extension in the existing Rust TCB.
**Confidence:** MEDIUM-HIGH — every mechanism cited below traces to a specific file:line in this repo; the exec-child confinement shape is the one genuinely novel piece (no prior-art call site exists), flagged throughout as `[ASSUMED]`/open decision rather than verified.

## Summary

This phase produces `planning-docs/DESIGN-effect-breadth-exec.md` — no code. The design has two halves that reuse different existing patterns.

**`process.exec`:** the confined worker cannot `execve` (seccomp denies it unconditionally,
`crates/sandbox/src/seccomp.rs:64-66`), so a command MUST run as a **separate process spawned by
the broker**, mirroring the two existing broker-spawn precedents: the v1.4 `caprun-planner`
sidecar (`cli/caprun/src/main.rs:311-332`, unconfined, `std::process::Command::spawn()`) and the
worker itself (`cli/caprun/src/main.rs:334-357`, spawned normally then **self**-confines after
connecting). Neither precedent fits directly: the exec target is arbitrary, non-caprun code, so it
cannot self-confine after an IPC handshake the way the worker does. The only way to kernel-confine
an arbitrary child is to apply confinement in the fork **before** its own `execve` — via
`std::process::Command::pre_exec()`. **No `.pre_exec(` call exists anywhere in this codebase
today** (verified by full-tree grep) — this is genuinely new. The confinement primitives
themselves (`rlimits::apply_rlimits`, `landlock::deny_all_filesystem`, `seccomp::apply_worker_filter`)
already carry doc comments anticipating this ("Returns `std::io::Result<()>` for `Command::pre_exec`
compatibility" — `crates/sandbox/src/rlimits.rs:11`, `landlock.rs:15`, `seccomp.rs:53`) but a
deny-all Landlock ruleset (today's only variant) would block the exec child from even loading its
own target binary — a **new, narrower Landlock ruleset variant** (allow read+execute on system
paths, deny write everywhere but an explicit workspace scratch area, deny network) is required.
stdout/stderr must be captured via `Stdio::piped()` and taint-minted through a **new**
`mint_from_exec`-shaped helper in `crates/brokerd/src/quarantine.rs` (the sole sanctioned mint
locus, alongside `server.rs` and `executor/src/value_store.rs:61` — `check-invariants.sh` Gate 3),
rooted at a new `process_exited` Event type — never stapled at a sink.

**Filesystem breadth:** the existing read (`RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS` via
`adapter-fs/src/workspace.rs:77-105`) and `file.create` (`O_CREAT|O_EXCL|O_WRONLY`,
`adapter-fs/src/workspace.rs:117-150`) already establish the single-syscall, TOCTOU-safe pattern.
Reading multiple files (FS-01) appears to already be mechanically supported — no per-session
read-count limiter was found guarding `RequestFd` (`crates/brokerd/src/server.rs:1229+`); this
needs explicit confirmation in the DESIGN doc, not an assumption. Writing an **existing** file
(FS-02) is a straightforward sibling of `create_exclusive_within` with `O_WRONLY` (no `O_EXCL`, no
`O_CREAT` — `ENOENT` on a missing file enforces "existing file only"), same kernel resolution
guarantees, same single-syscall TOCTOU safety.

**Both new sinks slot into the existing I2 / slot-type-binding machinery unchanged in shape**:
`submit_plan_node`'s Step 0 schema gate (`crates/executor/src/sink_schema.rs`), Step 1c role check
(`crates/executor/src/lib.rs:111-148`), and the routing/content-sensitivity collect-then-Block loop
(`crates/executor/src/sink_sensitivity.rs`) are all table-driven — adding `process.exec` and the fs
write/edit sink means adding entries to `KNOWN_SINKS`, `sink_effect_class`, `is_routing_sensitive` /
`is_content_sensitive`, and `expected_role`, never new logic. **No new raw `EffectRequest` path is
possible or needed** — both sinks are `PlanNode { sink, args }` from day one, exactly like
`file.create`.

**Primary recommendation:** pin the exec-child confinement as broker-`pre_exec`'d (new Landlock
ruleset variant + reused seccomp/rlimits primitives + a **new wall-clock timeout**, since
`RLIMIT_CPU` alone does not bound an idle/sleeping child), pin exec-output taint minting as a new
`mint_from_exec` helper following `mint_from_read`'s exact shape (fresh-rooted, fail-closed on
unknown output classification), and pin both new sinks as `CommitIrreversible` with `process.exec`'s
own `command`/`args` treated as **routing-and-content-sensitive** (a tainted command is arbitrary
code execution — strictly worse than a tainted email recipient) alongside the exec-output's
consuming-sink args.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DESIGN-13 | DESIGN doc pins broker-spawned confined-child-`exec` model + fs read/write-breadth model, clears fresh non-self adversarial code-trace before TCB code | Existing-Pattern Map, Open Design Decisions §1-3, Landmines |
| DESIGN-14 | DESIGN doc pins fail-closed defaults for new sinks (exec arg-schema/posture, exec-output taint label + `origin_role`, fs read/write path & slot constraints) consistent with I0/I1/I2 + slot-type binding | Open Design Decisions §4-8, Security-Invariant Checklist |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `process.exec` spawn + kernel confinement | Broker (`brokerd`, unprivileged parent) | Sandbox (`crates/sandbox` primitives, reused via `pre_exec`) | Only the broker can fork+`pre_exec`+exec; the worker cannot (seccomp denies `execve`), and confinement primitives must apply between fork and exec |
| exec stdout/stderr capture + taint mint | Broker (`quarantine.rs`, new `mint_from_exec`) | Adapter-shaped module `brokerd/src/sinks/process_exec.rs` (dispatch) | Mirrors `mint_from_read`'s "sole mint site rooted at the read Event" pattern — mint must live in the sanctioned locus (Gate 3) |
| I2 block of tainted exec-output at a sensitive sink arg | Executor (`crates/executor`) | — | Existing `submit_plan_node` collect-then-Block loop, table-driven, no new logic |
| fs read (multiple files) | Adapter (`adapter-fs`) + Broker (`RequestFd` dispatch) | — | Existing `read_within` / `RequestFd` path; extend by invocation count, not new mechanism (needs confirmation) |
| fs write/edit (existing file) | Adapter (`adapter-fs`, new `WorkspaceRoot` method) + Broker (new `sinks/file_write.rs`, mirrors `file_create.rs`) | Executor (schema + sensitivity) | Same `openat2` single-syscall pattern as `file.create`, different `OFlag` set |
| Durable audit of exec spawn/exit + fs write | Broker (`audit::append_event`, `Event.event_type`) | — | Two-phase (attempt-then-outcome) pattern already established by `file_create.rs` `sink_executed`/`sink_execution_failed` |
| Sink authorization posture (fail-closed schema, sensitivity, role table) | Executor (Rust TCB, hardcoded) | — | `KNOWN_SINKS`, `sink_effect_class`, `is_routing_sensitive`/`is_content_sensitive`, `expected_role` — CON-i2-non-bypassable, no config file |

## Standard Stack

No new external crates are required — this phase reuses the exact crates already vendored for confinement and fs resolution. `[VERIFIED: workspace Cargo.toml]`

### Core (already in workspace `Cargo.toml`, reused unchanged)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `landlock` | 0.4.5 (`Cargo.toml:20`) | Filesystem LSM restriction (ABI::V3, negotiates down to V1) | Already the project's sole Landlock binding; a second Landlock crate would violate "one confinement stack" |
| `seccompiler` | 0.5.0 (`Cargo.toml:21`) | seccomp-bpf filter construction/install | Already verified against this exact API surface (`crates/sandbox/src/seccomp.rs:9` "VERIFIED seccompiler 0.5.0 API — confirmed by reading crate source") |
| `nix` | 0.31.3, features `fs,socket,resource,process,signal,uio` (`Cargo.toml:17`) | `openat2`, `setrlimit`, `SCM_RIGHTS`, `fcntl` | Already the project's sole raw-syscall binding |
| `std::process::Command` | stdlib | Child process spawn, `pre_exec` hook (`std::os::unix::process::CommandExt`) | Already used twice for broker-spawned processes (`cli/caprun/src/main.rs:320,340`); `pre_exec` is a stdlib extension trait, not a new dependency |

### Open question: does `nix` 0.31.3's `resource` feature already expose the rlimit needed for a wall-clock (not just CPU-time) timeout?

`RLIMIT_CPU` (`crates/sandbox/src/rlimits.rs:16-24`) bounds CPU **seconds consumed**, not
wall-clock elapsed time — a child that blocks on I/O or sleeps evades it entirely. There is no
existing wall-clock timeout mechanism in this codebase. `[ASSUMED]` the DESIGN doc should pin
either (a) `Command::spawn()` + a broker-side `tokio::time::timeout` wrapped around
`child.wait()` (kills the child via `pidfd`/`SIGKILL` on expiry — this repo already tears down the
planner sidecar via `child.kill()` at `cli/caprun/src/main.rs:372-378`, so the primitive exists) or
(b) a Linux `alarm()`/`SIGALRM` set inside `pre_exec`. Recommend (a) — no new syscall surface, reuses
an already-proven kill path.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Broker `pre_exec`-confines the exec child directly | A dedicated `caprun-exec-launcher` helper binary (mirrors `caprun-worker`'s separate-binary pattern), spawned unconfined, which itself calls `sandbox::apply_confinement()`-like self-confinement then `execve`s the target | Extra binary + IPC round-trip to receive the target command; but avoids requiring `pre_exec` closures to be async-signal-safe (see Landmines). Worth raising as Option B in the DESIGN doc rather than assuming Option A is the only shape |
| New `TaintLabel::ExecRaw` variant | Reuse `TaintLabel::ExternalUntrusted` alone for exec output | A new label follows the existing naming convention (`EmailRaw`, `PdfRaw`, `PathRaw` — `crates/runtime-core/src/plan_node.rs:13-24`) and keeps provenance human-legible in the audit DAG; reusing `ExternalUntrusted` alone is simpler but loses that legibility. Recommend the new label — cheap (one `match` arm, compiler-enforced exhaustive) |

## Package Legitimacy Audit

**Not applicable this phase.** Phase 31 produces a design document only — no `Cargo.toml` changes,
no new dependency. All crates cited above are already present in the workspace and were vendored in
prior phases (v1.0 M0 substrate). If Phase 32/33 implementation later needs anything beyond `nix`/
`landlock`/`seccompiler`/stdlib (e.g., a crate for process-group/wall-clock timeout ergonomics), run
the Package Legitimacy Gate at that time, not here.

## Architecture Patterns

### System Architecture Diagram — `process.exec` broker-spawned confined child

```
 Worker (kernel-confined,        Broker (brokerd, unprivileged,       Kernel
 Landlock deny-all +             tokio async runtime)                 (Landlock LSM,
 seccomp deny-execve)                                                 seccomp-bpf)
      │                                 │                                 │
      │ submit_plan_node(               │                                 │
      │   PlanNode{sink:"process.exec", │                                 │
      │   args:[command,args,cwd]})     │                                 │
      ├────────────────────────────────►│                                 │
      │                                 │ Step 0 schema gate               │
      │                                 │ Step 1c role check (command/args │
      │                                 │   must be UserTrusted-rooted)    │
      │                                 │ Collect-then-Block (tainted      │
      │                                 │   command/args → BlockedPending) │
      │                                 │ ── if Allowed ──►                │
      │                                 │ std::process::Command::new(cmd)  │
      │                                 │   .pre_exec(|| {                 │
      │                                 │     apply_rlimits() +            │
      │                                 │     landlock_exec_child_ruleset()│
      │                                 │     seccomp_exec_child_filter()  │
      │                                 │   })                             │
      │                                 │   .stdout/.stderr(Stdio::piped())│
      │                                 │   .spawn()                       │
      │                                 ├────────────────fork()───────────►│
      │                                 │                          child: pre_exec
      │                                 │                          closures run,
      │                                 │                          THEN execve(cmd)
      │                                 │                          under the new
      │                                 │                          confinement ──►
      │                                 │◄──── stdout/stderr bytes ────────┤ (runs, confined:
      │                                 │      (piped, captured)           │  no net, workspace-
      │                                 │                                  │  scoped fs, timed)
      │                                 │ tokio::time::timeout(child.wait())
      │                                 │ mint_from_exec(quarantine.rs):   │
      │                                 │   new Event{event_type:          │
      │                                 │     "process_exited"}            │
      │                                 │   ValueStore::mint(stdout+stderr,│
      │                                 │     taint=[ExternalUntrusted,    │
      │                                 │       ExecRaw], provenance_chain │
      │                                 │       =[process_exited.id],      │
      │                                 │     origin_role="exec_output")   │
      │◄──── ValueId handle only ───────┤                                 │
      │  (worker never sees raw bytes,  │                                 │
      │   only the opaque handle — same│                                 │
      │   handle model as file reads)  │                                 │
```

Downstream: a later `PlanNode` routes that `ValueId` into a sensitive slot of another sink (e.g.
`email.send` `body`, or the new fs write sink's `contents`). `submit_plan_node`'s existing
collect-then-Block loop Blocks it there — **no new I2 logic**, only new table entries.

### Recommended additions to project structure (informative — Phase 32/33 scope, not this phase's deliverable)

```
crates/brokerd/src/
├── quarantine.rs         # + mint_from_exec() alongside mint_from_read/mint_from_derivation
├── sinks/
│   ├── file_create.rs    # existing — template for the two new sink modules
│   ├── process_exec.rs   # NEW — invoke_process_exec, mirrors invoke_file_create's
│   │                     #   two-phase durable audit (sink_executed/sink_execution_failed)
│   └── file_write.rs     # NEW — invoke_file_write, mirrors invoke_file_create with
│                         #   O_WRONLY (no O_EXCL/O_CREAT) instead of O_CREAT|O_EXCL
crates/sandbox/src/
├── landlock.rs            # + a second ruleset constructor: exec_child_ruleset() with
│                          #   allow-rules (read+execute system paths, no write outside
│                          #   an explicit scratch dir), distinct from deny_all_filesystem()
├── seccomp.rs              # + exec_child_filter(): deny socket(AF_INET/6) (reuse), do
│                          #   NOT deny execve (the child's own exec is legitimate — this
│                          #   filter installs INSIDE pre_exec, BEFORE that one exec call)
```

### Pattern 1: Broker-spawned, unconfined-until-pre_exec child (NEW — no direct precedent)

**What:** the broker forks via `std::process::Command::spawn()` with a `.pre_exec()` closure that
applies confinement primitives in the child, between `fork()` and the child's own `execve()`.

**When to use:** exactly `process.exec` — any case where the child runs **arbitrary,
non-caprun code** that cannot self-confine (self-confinement, as used by `caprun-worker`, requires
the child to be caprun's own binary that knows to call `sandbox::apply_confinement()` after an IPC
handshake — `crates/sandbox/src/lib.rs:7-18` explicitly documents why `pre_exec` is wrong for
*that* case: Landlock deny-all would block loading the worker's own binary. The reasoning inverts
here: the target of `process.exec` is never caprun's own binary, so there is no "load my own binary
under deny-all" problem — but Landlock deny-all-with-NO-allow-rules would *still* block loading the
**target** binary, which is why a new ruleset variant with narrow allow-rules is required, not a
reuse of `deny_all_filesystem()` verbatim).

**Example (existing broker-spawn precedent this pattern extends — NOT yet exec-confined):**
```rust
// Source: cli/caprun/src/main.rs:320-328 (v1.4 planner sidecar spawn — unconfined precedent)
let mut cmd = std::process::Command::new(&planner_binary);
cmd.env("PLANNER_SOCK", &planner_sock);
if let Ok(key) = std::env::var("OPENAI_API_KEY") {
    cmd.env("OPENAI_API_KEY", key);
}
let child = cmd.spawn().context("spawn caprun-planner sidecar")?;
```
The `process.exec` sink needs the same `std::process::Command` shape, PLUS (new) a `pre_exec`
closure and `Stdio::piped()` stdout/stderr capture — those two additions are the entire net-new
surface for spawn mechanics.

### Pattern 2: Single-syscall, TOCTOU-safe `openat2` resolution (existing — reuse directly)

**What:** resolve-and-act in ONE `openat2` call with `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`, so
there is no separate stat-then-open race window and no symlink-following escape.

**When to use:** every new fs sink (write/edit).

**Example:**
```rust
// Source: crates/adapter-fs/src/workspace.rs:134-144 (file.create's exclusive-create — the
// existing pattern the new write/edit sink should mirror with a different OFlag set)
use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
let how = OpenHow::new()
    .flags(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY)
    .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
let fd = openat2(self.dirfd.as_fd(), rel_path, how)?;
```
For an existing-file write/edit sink, the recommended flag set is `O_WRONLY | O_TRUNC` (no
`O_CREAT`, no `O_EXCL`) — a missing target path fails closed with `ENOENT` (never silently
creates), and `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS` still reject absolute paths, `..` escape, and
symlink traversal exactly as they do today (`adapter-fs/src/workspace.rs:220-283` negative tests
already prove this behavior for the read/create paths; equivalent tests are needed for the new
flag combination — not yet proven for it specifically).

### Pattern 3: Fresh-rooted taint mint at a read/spawn Event (existing — reuse the shape, not the code)

**What:** taint is set exactly once, at the function that ALSO records the originating audit Event
— never at the sink.

**Example:**
```rust
// Source: crates/brokerd/src/quarantine.rs:301-340 (mint_from_read — the template for a new
// mint_from_exec; note the fail-closed unknown-claim_type discipline (T-07-47) that a new
// mint_from_exec must mirror: only recognized exec-output classifications get taint set — no
// default-tagging of an unrecognized case)
pub fn mint_from_read(
    conn: &rusqlite::Connection, key: &[u8], store: &mut ValueStore,
    session_id: Uuid, claim: &Claim, parent_id: Option<Uuid>, parent_hash: Option<&str>,
) -> Result<(Uuid, String, ValueId, Uuid, String)> {
    let taint = match claim.claim_type.as_str() {
        "email_address" => vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
        "relative_path" => vec![TaintLabel::ExternalUntrusted, TaintLabel::PathRaw],
        // ... fail-closed on unrecognized claim_type (T-07-47)
    };
    // ... appends the file_read Event FIRST, then store.mint(...) with
    // provenance_chain = [that Event's id] — this ordering IS the genuine-taint guarantee.
}
```
A `mint_from_exec` follows the identical shape: append a `process_exited` Event first, then
`store.mint(combined_output, vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw],
vec![process_exited_event.id], Some("exec_output".into()))`.

### Anti-Patterns to Avoid

- **Stapling exec-output taint at the consuming sink instead of at spawn/capture time:** the §9
  acceptance discipline this entire project is built on (`planning-docs/PLAN.md:186` "If taint is
  stapled on at the sink instead of propagated through the DAG, the demo fails") applies identically
  here. The mint MUST happen in the same function that records the `process_exited` Event, exactly
  like `mint_from_read`.
- **Reusing `landlock::deny_all_filesystem()` verbatim for the exec child:** it has zero allow-rules
  — the target binary itself could not load. A distinct ruleset constructor is required.
- **Calling `sandbox::apply_confinement()` (the worker's self-confinement path) for the exec
  child:** wrong ordering model entirely — that function is designed to run AFTER the calling
  process has already loaded and connected, not `pre_exec`-style before its own exec
  (`crates/sandbox/src/lib.rs:7-18` explicitly documents this distinction and must not be
  conflated).
- **Treating `process.exec`'s own `command`/`args` as unconstrained (`None` in `expected_role`):**
  unlike `email.send`'s `attachment` (a documented, deliberate scope-out —
  `crates/executor/src/sink_sensitivity.rs:118-124`), a tainted `command` is not a data-exfiltration
  risk but an **arbitrary-code-execution** risk — it must be both routing- and content-sensitive so
  a tainted value here Blocks under the existing collect-then-Block loop, never falls through
  unconstrained.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Kernel confinement of the exec child | A custom `ptrace`-based sandbox, a container runtime shell-out, or a bespoke seccomp filter builder | `landlock` 0.4.5 + `seccompiler` 0.5.0 — same crates, extend with a second ruleset/filter constructor | Both are already verified-against-source in this repo (`seccomp.rs:9`); a second confinement stack would violate "keep the broker small" (`planning-docs/PLAN.md:214`) and multiply the TCB surface the adversarial review must trace |
| Path traversal / symlink-escape prevention for the new fs write sink | Manual `..`-stripping, `canonicalize()`-then-compare, or a stat-then-open two-step check | `openat2(RESOLVE_BENEATH \| RESOLVE_NO_SYMLINKS)` — same single syscall the read/create paths already use | `canonicalize`-then-compare is a classic TOCTOU hole (file can be swapped for a symlink between the check and the open); `openat2`'s resolve flags reject the escape AT kernel resolution time atomically |
| Wall-clock timeout enforcement for the exec child | A custom `SIGALRM`/timer-thread inside the child | `tokio::time::timeout()` wrapping the broker's `child.wait()`, killing via the SAME `child.kill()` path already used to tear down the planner sidecar (`cli/caprun/src/main.rs:372-378`) | Reuses a proven, already-tested kill path instead of adding new signal-handling code inside a process about to be further confined |
| Process-output "is this safe" classification | A heuristic/regex scanner over stdout/stderr content | Nothing — taint it unconditionally as untrusted (`ExternalUntrusted` + new `ExecRaw`), let I2 do the enforcement at the consuming sink | This project's entire model is "taint by origin, not by content inspection" (`DESIGN-taint-model.md` — schema validation checks shape, not truth); a content classifier would be a new, unverified trust decision inside the TCB |

**Key insight:** every new mechanism in this phase is a *narrow extension* of an existing pattern
(fork+`pre_exec` extends the already-imported `Command::pre_exec`-shaped primitives; the fs write
sink extends `openat2`; taint minting extends `mint_from_read`'s shape; I2 enforcement extends
table entries only). The one place with **no existing call site at all** is `pre_exec` itself —
that is where the DESIGN doc and its adversarial review must spend the most scrutiny.

## Open Design Decisions the DESIGN Doc Must Resolve

Each maps to DESIGN-14 (fail-closed defaults).

### 1. Who forks the exec child — the broker's own tokio task, or a dedicated launcher binary?

- **Option A (broker `pre_exec`s directly):** `brokerd`'s dispatch handler (already inside a
  `tokio::spawn`'d task per connection, `crates/brokerd/src/server.rs:271,308`) calls
  `std::process::Command::new(cmd).pre_exec(...).spawn()` inline. Simpler, fewer moving parts,
  reuses the exact `Command::spawn()` call shape already used twice in `main.rs`.
- **Option B (dedicated `caprun-exec-launcher` binary):** the broker spawns a small unconfined
  helper binary (mirrors `caprun-worker`'s separate-binary pattern) which receives the target
  command over the existing UDS/env-var channel and does its own fork+`pre_exec`+exec outside the
  broker's async runtime entirely.
- **Fail-closed default recommendation:** **Option A**, because `std::process::Command::spawn()` on
  Linux does fork+exec as a single library call (not a raw manual `fork()` inside a
  multi-threaded async context) and this exact call shape is already exercised twice in this
  codebase without incident. The DESIGN doc MUST still explicitly address the async-signal-safety
  caveat below (item 2) rather than silently assuming it away.
- `[ASSUMED]` — not verified against Rust's own `pre_exec` safety documentation this session
  beyond training knowledge; the DESIGN doc author should re-verify Rust's `pre_exec` contract
  (closures must only call async-signal-safe functions between fork and exec) against the actual
  `landlock`/`seccompiler` crate internals (do their `Ruleset::create()`/`apply_filter()` calls
  allocate? `Vec`/heap allocation between fork and exec is a well-known soft violation many
  sandboxing tools accept in practice, but it should be an explicit, documented residual risk, not
  a silent assumption).

### 2. Landlock ruleset for the exec child — what does it allow?

Must NOT be `deny_all_filesystem()` (blocks loading the target binary itself). Options:
- **Narrow allow-list:** allow `ReadFile`+`Execute` on `/usr`, `/bin`, `/lib`, `/lib64` (standard
  binary/shared-library paths), allow `ReadFile`+`WriteFile` on `WorkspaceRoot` only, deny
  everything else (including `~/.ssh`, matching the existing M0 negative assertion
  `planning-docs/PLAN.md:152`).
- **Fail-closed default recommendation:** the narrow allow-list above, explicitly enumerated (not
  a wildcard), consistent with this project's "sink sensitivity map is hardcoded, no runtime
  registry" discipline (`sink_sensitivity.rs:1-9`) applied to the confinement layer too.
- `[ASSUMED]` — the exact system paths needed are environment-dependent (differ between the Linux
  container image used for `mailpit-verify.sh` verification and a bare-metal host); the DESIGN doc
  should pin a method (e.g., resolve via `PATH` + `ldd`-style dependency walk at broker startup) or
  accept a documented, narrow hardcoded list scoped to the verification container's known layout.

### 3. seccomp filter for the exec child — deny network, but NOT deny execve

The worker's filter (`crates/sandbox/src/seccomp.rs:64-93`) denies `execve`/`execveat`
unconditionally — correct for the worker, wrong for the exec child (which needs exactly ONE
`execve` to run, inside `pre_exec`, before its own filter would even matter for that call — seccomp
filters installed via `pre_exec` apply to the process AFTER the filter-installing syscall returns,
and persist across the child's own subsequent `execve` per standard Linux seccomp-BPF semantics
`[ASSUMED — general kernel behavior, not re-verified against a kernel source this session]`).
Recommend: reuse the existing `socket(AF_INET/AF_INET6)` deny rule unchanged, do NOT add an execve
deny (the child's legitimate one-time exec must succeed), but consider denying `execve`/`execveat`
for any **further** exec by the target command itself (i.e., `bash -c "curl ... | sh"` would exec a
second time) — this is a genuine open question: should the exec child be allowed to spawn its OWN
children? **Fail-closed default recommendation: deny it** — a `process.exec` sink that permits
unbounded recursive exec defeats the entire confinement/audit purpose (an unaudited grandchild could
make network calls or spawn shells the executor never scored). If a "run a shell script" use case
is truly required, that is a v1.8+ product decision (`git`/`http.request` also deferred), not this
milestone's default.

### 4. `process.exec` arg schema shape

- **Fail-closed default recommendation:** `command: String` (a single resolved binary path or
  `PATH`-relative name — no shell metacharacter interpretation, i.e. NEVER pass through `sh -c`),
  `args: Vec<String>` (each a separate `execve` argv element, not a shell-joined string — this
  closes shell-injection-via-argument-concatenation by construction), `cwd: Option<String>`
  (workspace-relative, resolved via the same `RESOLVE_BENEATH` discipline). No environment-variable
  passthrough by default (mirrors the existing precedent that `OPENAI_API_KEY` is forwarded to the
  planner sidecar ONLY, never the worker — `cli/caprun/src/main.rs:309-310`).

### 5. `process.exec` (dis)allow posture — is there a command allowlist?

- **Option A:** no allowlist — any `command` is permitted, confinement (Landlock/seccomp/rlimits/
  network-deny) is the sole control.
- **Option B:** a hardcoded allowlist of permitted binaries (mirrors `sink_sensitivity.rs`'s
  "hardcoded, no config file" discipline applied to commands, not just sinks).
- **Fail-closed default recommendation:** **Option A** for v1.7 (matches the milestone's stated
  scope — "the two effect primitives a coding agent minimally needs," `.planning/REQUIREMENTS.md`
  — an allowlist would need product-level curation deferred to a later milestone alongside `POL-01`
  declarative policy, `.planning/REQUIREMENTS.md` Future Requirements). The DESIGN doc must state
  this explicitly as a scoping decision, not an oversight, and note the accepted residual (an
  Allowed exec of e.g. `curl` inside a confined, network-denied child is inert — network egress is
  already denied by the reused seccomp rule regardless of allowlist).

### 6. Exec-output taint label + `origin_role`

- **Fail-closed default recommendation:** new `TaintLabel::ExecRaw` variant (mirrors `PathRaw`/
  `EmailRaw`/`PdfRaw` naming — `crates/runtime-core/src/plan_node.rs:13-24`), always paired with
  `ExternalUntrusted` (matches every existing untrusted-origin mint site's pattern of a 2-label
  vector). `origin_role = Some("exec_output")`. Adding a variant requires updating
  `TaintLabel::is_untrusted()`'s exhaustive match (`plan_node.rs:40-50`) — the compiler enforces
  this cannot be silently missed (Pitfall 5's discipline already documented in that file).

### 7. fs write/edit sink slot roles

- **Fail-closed default recommendation:** `path` is routing-sensitive (mirrors `file.create`'s
  `FILE_CREATE_ROUTING_SENSITIVE`, `sink_sensitivity.rs:66`) with `expected_role =
  Some(&["path", "relative_path"])` (mirrors `file.create`'s existing `path` role check — need to
  read the exact existing entry at `sink_sensitivity.rs:326+` when authoring the doc, cite it
  verbatim rather than re-deriving). `contents` is content-sensitive (mirrors HARDEN-05's extension
  of `file.create`'s `contents` to content-sensitive, `sink_sensitivity.rs:80-85`), with
  `origin_role` accepting both a trusted-authored role AND `"exec_output"`/`"doc_fragment"` (so a
  tainted exec-output routed into a file write's `contents` Blocks, exactly like the email `body`
  precedent that already accepts `"doc_fragment"` — `sink_sensitivity.rs:138-154`).

### 8. Symlink / path-escape handling for the fs write sink

- **Fail-closed default recommendation:** identical to today's `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`
  — no new escape surface, no exception. The existing negative tests
  (`adapter-fs/src/workspace.rs:220-283`, absolute-path rejection, `..` rejection, symlink rejection)
  are the template; equivalent tests must be written for the new `O_WRONLY|O_TRUNC` flag
  combination specifically (not assumed to inherit coverage from the read/create tests).

## Security-Invariant Checklist the Doc Must Satisfy

- [ ] **I0** — unaffected; no new session-creation path. State explicitly that neither new sink
  changes session-creation semantics.
- [ ] **I1** — unaffected; the worker still never holds raw exec-output bytes, only the `ValueId`
  handle (mirrors the existing read-extraction handle model, `DESIGN-taint-model.md:148-154`).
- [ ] **I2** — both new sinks route through the unmodified `submit_plan_node` collect-then-Block
  loop; NO new bypass logic, only table entries (`KNOWN_SINKS`, `sink_effect_class`,
  `is_routing_sensitive`/`is_content_sensitive`, `expected_role`).
- [ ] **No I2 bypass** — `process.exec`'s own `command`/`args` MUST be sensitivity-classified (Open
  Decision 8 above), not left as an unconstrained pass-through that could carry a tainted value into
  arbitrary code execution unblocked.
- [ ] **No raw `EffectRequest` path** — both sinks are `PlanNode { sink, args }` from spawn; verify
  `check-invariants.sh` Gate 1 still passes with zero new hits.
- [ ] **Genuine, non-stapled taint chain** — exec-output taint is set ONLY inside the new
  `mint_from_exec`, at the same call that appends the `process_exited` Event; `provenance_chain[0]`
  MUST equal that Event's id (mirrors the `mint_from_read_anchor_identity` test pattern,
  `quarantine.rs:856-880`).
- [ ] **Mint-call-site restriction (Gate 3)** — `check-invariants.sh` Gate 3 currently greps only
  for `mint_from_read(`, `mint_from_derivation(`, and `.mint(` (script lines ~120-135). **A new
  `mint_from_exec(` call will NOT be caught by Gate 3 as currently written** — the DESIGN doc MUST
  mandate extending Gate 3 with a `mint_from_exec(` check (same sanctioned loci:
  `crates/brokerd/src/quarantine.rs`, `crates/brokerd/src/server.rs`) as part of Phase 32, or the
  new mint site's call-site restriction is silently unenforced. This is a concrete, actionable
  finding — not a general reminder.
- [ ] **Kernel-confined exec child** — Landlock (narrow allow-list, not deny-all) + seccomp
  (network-deny, no execve-deny for the one legitimate exec, recursion-exec-deny recommended) +
  rlimits (`RLIMIT_AS`/`RLIMIT_CPU` reused) + a NEW wall-clock timeout (does not exist today).
- [ ] **Fail-closed arg-schema** — `process.exec` and the fs write sink both get `KNOWN_SINKS`
  entries with explicit `allowed`/`required` sets (mirrors `file.create`'s exact-match schema,
  `sink_schema.rs:53-57`); an unregistered sink or unknown/duplicate/missing arg Denies at Step 0,
  before any resolve/sensitivity check.
- [ ] **Durable audit** — both sinks use the two-phase pattern (`sink_executed` /
  `sink_execution_failed`, or an exec-specific pair e.g. `process_exited` /
  `process_spawn_failed`), chained onto `parent_id`/`parent_hash` exactly like `invoke_file_create`
  (`crates/brokerd/src/sinks/file_create.rs:84-113`).

## Landmines / Pitfalls

### Pitfall 1: Assuming `deny_all_filesystem()` can be reused verbatim for the exec child
**What goes wrong:** the exec child's own binary fails to load — Landlock's `Execute` access right
is denied with zero allow-rules, so the kernel refuses the `execve` itself.
**Why it happens:** `deny_all_filesystem()` (`crates/sandbox/src/landlock.rs:16-32`) was designed
for the WORKER, which self-confines AFTER it has already loaded and is running — a fundamentally
different ordering than `pre_exec`, which must apply BEFORE the target's own `execve`.
**How to avoid:** a distinct ruleset constructor with explicit allow-rules (Open Decision 2).
**Warning signs:** every exec attempt returns `EACCES`/`ENOEXEC` in the confined child; a naive
implementer might mistake this for a broken command path rather than an over-broad Landlock deny.

### Pitfall 2: `mint_from_exec` escaping Gate 3's call-site restriction
**What goes wrong:** a new mint helper is added somewhere outside `quarantine.rs`/`server.rs`
(e.g. directly inside the new `sinks/process_exec.rs`), and `check-invariants.sh`'s Gate 3 — which
only greps for the THREE hardcoded literal patterns `mint_from_read(`, `mint_from_derivation(`,
`.mint(` — does not catch it, silently reintroducing an unrestricted mint site.
**Why it happens:** Gate 3 is pattern-string-based, not type-based; it only defends the patterns it
was told to look for.
**How to avoid:** extend Gate 3 in the same commit that adds `mint_from_exec` (see checklist item
above).
**Warning signs:** a fresh adversarial reviewer greps for `mint_from_exec(` and finds it outside
`quarantine.rs`/`server.rs` with no gate catching it.

### Pitfall 3: Wall-clock timeout gap
**What goes wrong:** an exec child that sleeps or blocks on stdin (no CPU consumed) runs
indefinitely — `RLIMIT_CPU=30` never fires because it counts CPU-seconds, not wall time.
**Why it happens:** the existing rlimit primitive (`crates/sandbox/src/rlimits.rs:16-24`) was
designed for the worker's own compute-bound workload, not an arbitrary external command.
**How to avoid:** broker-side `tokio::time::timeout` around `child.wait()`, killing via the
existing `child.kill()` path (Open Decision 1's Standard Stack entry).
**Warning signs:** a `sleep 999999` (or an interactive prompt with no stdin) as the `command` value
hangs the session indefinitely in acceptance testing.

### Pitfall 4: Confusing `exec.shell` (a test fixture) with prior art
**What goes wrong:** a grep for `"exec"` in `crates/executor/src/sink_schema.rs` finds
`"exec.shell"` at line 196 and a naive reader concludes exec-sink schema work has already started.
**Why it happens:** `"exec.shell"` is used ONLY as a `validate_schema` `UnknownSink` test fixture
(`sink_schema.rs:190-198`) — it asserts the OPPOSITE: that this string is currently rejected. It is
not, and must not become, the real `process.exec` sink id without deliberate naming reconciliation.
**How to avoid:** the DESIGN doc should explicitly note the real sink id is `process.exec` (per
REQUIREMENTS.md, EXEC-01) and that `"exec.shell"` remains a distinct, permanently-rejected test
fixture string — no accidental collision.
**Warning signs:** a plan or PR that reuses the literal string `"exec.shell"` anywhere in production
code.

### Pitfall 5: Async-signal-safety inside `pre_exec` closures
**What goes wrong:** `landlock::Ruleset::create()`/`restrict_self()` and `seccompiler::apply_filter()`
likely allocate (both build `Vec`-backed structures) between `fork()` and `execve()`. Rust's own
`pre_exec` documentation requires closures to only call async-signal-safe functions in that window;
allocator state inherited from the parent can be in an inconsistent state in the child of a
multi-threaded process, risking a hang or corruption under rare scheduling.
**Why it happens:** this is a widely-known, widely-accepted soft violation in the Rust sandboxing
ecosystem — few crates offer a strictly async-signal-safe Landlock/seccomp setup path — but it has
NOT been exercised in THIS codebase before (the worker's self-confinement runs long after its own
fork, not inside a `pre_exec` closure).
**How to avoid:** the DESIGN doc should treat this as an explicit, named residual risk (matching
this project's convention of an "Accepted Residual Risks" section in every prior DESIGN doc,
e.g. `DESIGN-taint-model.md:276`), not silently ignore it. If the adversarial reviewer flags it as
a BLOCKER, Option B (dedicated launcher binary, which can perform its OWN post-fork, pre-exec
self-confinement using the SAME ordering already proven safe for the worker) is the documented
fallback.
**Warning signs:** intermittent, non-reproducible hangs specifically in the exec-child spawn path
under CI/CD load (classic symptom of an async-signal-safety violation manifesting only under
scheduler pressure).

## Code Examples

### Existing fd-pass mediation (adapter-fs) — the "broker mediates, worker never has ambient access" template

```rust
// Source: crates/adapter-fs/src/lib.rs:40-52 (pass_fd) — the broker's side of every
// existing mediated-fs-access flow. Exactly one ControlMessage::ScmRights slice per
// sendmsg (nix issue #464); at least one iov payload byte required by some kernels.
pub fn pass_fd(socket_raw_fd: RawFd, file_raw_fd: RawFd) -> nix::Result<()> {
    use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
    use std::io::IoSlice;
    let iov = [IoSlice::new(b"\x00")];
    let fds = [file_raw_fd];
    let cmsg = ControlMessage::ScmRights(&fds);
    sendmsg::<()>(socket_raw_fd, &iov, &[cmsg], MsgFlags::empty(), None)?;
    Ok(())
}
```
This is NOT the pattern `process.exec` uses (exec output arrives via `Stdio::piped()` bytes, not an
fd handoff) — cited here only to make clear the fd-pass model is the WRONG template for exec
output, avoiding a design mistake of trying to fd-pass a pipe when a direct captured-bytes read is
simpler and already what `std::process::Command` + `Stdio::piped()` provides natively.

### Existing two-phase durable audit — the template for the new sinks' outcome recording

```rust
// Source: crates/brokerd/src/sinks/file_create.rs:82-113 (abbreviated)
match workspace_root.create_exclusive_within(&path, contents.as_bytes()) {
    Ok(()) => {
        let event = Event::new(Uuid::new_v4(), Some(parent_id), session_id,
            format!("sink:file.create:{effect_id}"), "sink_executed".into(), Utc::now(), vec![]);
        let hash = append_event(conn, key, &event, Some(parent_hash))?;
        Ok((event.id, hash))
    }
    Err(e) => {
        let event = Event::new(Uuid::new_v4(), Some(parent_id), session_id,
            format!("sink:file.create:{effect_id}"), "sink_execution_failed".into(), Utc::now(), vec![]);
        append_event(conn, key, &event, Some(parent_hash))?;
        Err(anyhow::Error::new(e).context("file.create create_exclusive_within failed"))
    }
}
```
`process.exec` and the fs write sink should each produce their own analogous
`sink_executed`/`sink_execution_failed` pair (or exec-specific event-type names), preserving the
`actor = "sink:<sink_id>:<effect_id>"` convention.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| Every broker-spawned child either self-confines after connecting (worker) or runs fully unconfined (planner sidecar, an internal trusted process) | A broker-spawned child running ARBITRARY, non-caprun code, confined via `pre_exec` BEFORE its own `execve` | This phase (design) → Phase 32 (implementation) | First genuinely new confinement mechanism since v1.0 M0's substrate (worker self-confinement) — the adversarial review should treat it with v1.0-Phase-2-level scrutiny, not as an incremental extension |
| Single-file read (`RequestFd`) and single-file exclusive-create (`file.create`, `O_EXCL` new-file-only) | Multi-file read (extends existing `RequestFd` invocation, pending confirmation) + existing-file write/edit (`O_WRONLY|O_TRUNC`, new) | This phase (design) → Phase 33 (implementation) | Extends the `openat2` single-syscall pattern; no new kernel-resolution mechanism, only new `OFlag` combinations and, for write/edit, a genuinely new "modify existing content" side effect class distinct from "never overwrites" |

**Deprecated/outdated:** nothing in this milestone deprecates prior mechanisms — `file.create`'s
`O_EXCL` semantics remain unchanged; the new write/edit sink is additive, not a replacement.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `std::process::Command::spawn()`'s internal fork+exec is safe to call from within a `tokio::spawn`'d async task without a dedicated OS thread, given this codebase already does so twice | Open Decision 1, Pattern 1 | If wrong, the exec-child spawn path could intermittently hang under scheduler pressure; mitigation (Option B, dedicated launcher) is already documented as a fallback |
| A2 | `landlock::Ruleset::create()`/`restrict_self()` and `seccompiler::apply_filter()` allocate heap memory internally (making them not strictly async-signal-safe inside `pre_exec`) | Landmine 5 | If the allocation assumption is wrong (i.e., these calls are actually alloc-free), the residual risk section overstates a non-issue — low-cost to be wrong in this direction (extra caution, not a missed hole) |
| A3 | Landlock ABI negotiation (`ABI::V3` down to `ABI::V1`, kernel ≥5.13) and `openat2` RESOLVE_* flags (kernel ≥5.6) version floors, per training knowledge, not re-verified against kernel source this session | Existing-Pattern Map, Environment Availability | If the target verification container's kernel is older than assumed, Landlock/openat2 silently degrade or fail — should be confirmed against the actual `rust:1` Docker image kernel used by `scripts/mailpit-verify.sh` |
| A4 | No per-session read-count limiter blocks multiple sequential `RequestFd` calls today (FS-01 "multiple workspace files" may already be mechanically supported by the existing single-file path called N times) | Summary, Architectural Responsibility Map | If a limiter exists elsewhere in `server.rs` not surfaced by this session's greps, FS-01 requires new mechanism, not just documentation — the DESIGN doc author must re-verify this specific claim against the full `RequestFd` handler before pinning the fail-closed default |
| A5 | Seccomp filters installed via `pre_exec` persist across the child's own subsequent `execve` (standard Linux seccomp-BPF inheritance semantics) | Open Decision 3 | If this general kernel behavior assumption is wrong for some reason specific to this setup, the network-deny protection on the exec child would not actually apply post-exec — this is standard, well-documented Linux kernel behavior, low likelihood of being wrong, but not re-verified against a kernel source this session |

## Open Questions

1. **Does `RequestFd` currently have any implicit or explicit limit on repeat calls per session?**
   - What we know: no counter/guard was found in this session's grep of `server.rs`'s `RequestFd`
     arm (`crates/brokerd/src/server.rs:1229+`); `ProvideIntent` explicitly documents an
     "ONCE and ONLY BEFORE any RequestFd" constraint (`server.rs:1194,1626-1639`), but no
     equivalent language was found for `RequestFd` itself.
   - What's unclear: whether the full ~400-line `RequestFd` handler (not fully read this session)
     contains a guard this grep missed.
   - Recommendation: the DESIGN doc author must read the complete `RequestFd` arm before pinning
     FS-01's fail-closed default; if genuinely unlimited, the doc should still pin an explicit
     upper bound (e.g., a session-scoped read counter) as a resource-exhaustion guard, since
     "unlimited" was never a deliberate prior decision — it may just be unexercised, not endorsed.

2. **Should the exec child be permitted its own further `execve` (i.e., running a shell script
   that itself execs sub-commands)?**
   - What we know: the worker's own seccomp filter denies `execve` unconditionally; nothing today
     answers this question for a `process.exec`-spawned child.
   - What's unclear: whether "run a command with captured output" (this milestone's stated scope)
     implicitly requires shell-script support (which inherently re-execs), or whether that's
     explicitly out of scope until `git`/`http.request` (v1.8+, per REQUIREMENTS.md Future
     Requirements) formalizes broader command patterns.
   - Recommendation: pin "no further exec" (recursion-deny) as the v1.7 fail-closed default (Open
     Decision 3), explicitly scoped, revisit only if a future milestone's acceptance criteria
     require shell scripting.

3. **Exact system paths needed in the exec child's Landlock allow-list.**
   - What we know: the verification recipe runs inside a `rust:1` Docker container
     (`scripts/mailpit-verify.sh`); the allow-list must at minimum cover whatever `command` values
     the Phase 32/34 acceptance tests actually invoke.
   - What's unclear: the definitive minimal path set without inspecting the container's actual
     filesystem layout (`ldd`/`which` output for candidate test commands).
   - Recommendation: defer the exact enumerated list to Phase 32 implementation, informed by the
     specific test commands chosen there; the DESIGN doc should pin the METHOD (explicit
     hardcoded allow-list, narrowest-that-works) rather than the literal path strings.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker (via Colima) | Linux-only confinement verification (`scripts/mailpit-verify.sh`) for Phase 32/33/34 | ✓ | Docker Engine 29.6.1, Colima running (macOS Virtualization.Framework, aarch64) | — |
| `rust:1` base image | Verification container used by `mailpit-verify.sh` | Not pulled/checked this session | — | Pull on first Phase-32 verification run |
| Linux kernel ≥5.13 (Landlock) / ≥5.6 (`openat2` RESOLVE_*) in the verification container | Both new sinks' kernel-enforcement claims | `[ASSUMED]` — not queried this session | — | Confirm kernel version inside the `rust:1` container before Phase 32/33 land; this is a design-phase research gap, not a blocker for THIS phase (no code runs yet) |

This is a design-only phase — no code executes, so no dependency is currently blocking. The table
above is forward-looking for the phases this design gate unblocks.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Cargo's built-in test harness (`cargo test`), workspace-wide via `cargo test --workspace --no-fail-fast` |
| Config file | none — `Cargo.toml` workspace manifest at repo root |
| Quick run command | `cargo build --workspace` (this phase produces no code — no test to run) |
| Full suite command | `bash scripts/mailpit-verify.sh` (Linux-only enforcement tests; Phase 32/33/34 scope, not this phase) |

### Phase Requirements → Test Map

This is a design-gate phase — DESIGN-13/14 are satisfied by a reviewed document and a gate record,
not by automated tests. The map below is forward-looking for the phases this design unblocks.

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DESIGN-13 | `planning-docs/DESIGN-effect-breadth-exec.md` exists, pins exec-child model + fs breadth model | manual (doc review) | n/a — human/reviewer inspection | N/A this phase |
| DESIGN-14 | Doc pins fail-closed defaults, no I2 bypass, no raw `EffectRequest` | manual (doc review) + `bash scripts/check-invariants.sh` (Gates 1-4 re-run, no regression) | `bash scripts/check-invariants.sh` | ✅ exists (`scripts/check-invariants.sh`) |
| (forward) EXEC-01..04 | `process.exec` sink spawn/confine/taint/I2/audit | integration, negative-per-sink | `cargo test -p brokerd process_exec` (name TBD Phase 32) | ❌ Wave 0, Phase 32 |
| (forward) FS-01..03 | fs read-breadth + write/edit sink under I2/slot-type-binding | integration, negative-per-sink | `cargo test -p brokerd file_write` (name TBD Phase 33) | ❌ Wave 0, Phase 33 |
| (forward) LIVE-01/02 | Composed Linux acceptance, full regression | live e2e | `bash scripts/mailpit-verify.sh` (or an exec-scoped equivalent per LIVE-01's wording) | ✅ script exists; new test targets TBD Phase 34 |

### Sampling Rate

- **Per task commit (this phase):** re-run `bash scripts/check-invariants.sh` after every edit to
  `planning-docs/DESIGN-effect-breadth-exec.md` that touches a code-adjacent claim (sanity: the
  doc's claims about current Gate behavior stay accurate as the doc is drafted/amended).
- **Per wave merge:** n/a — this phase has no waves of code.
- **Phase gate:** the fresh non-self adversarial code-trace review (DESIGN-13) IS the phase gate —
  see Gate-Record Shape below. No `cargo test` run is required to close Phase 31 itself (no TCB
  code exists yet), but the reviewer should independently re-run `cargo build --workspace` and
  `bash scripts/check-invariants.sh` to confirm the doc's code citations are current, not stale.

### Wave 0 Gaps (forward-looking, Phase 32/33 scope)

- [ ] `crates/brokerd/tests/process_exec_*.rs` — spawn/confine/taint/I2/audit coverage for EXEC-01..04
- [ ] `crates/brokerd/tests/file_write_*.rs` — write/edit sink coverage for FS-01..03
- [ ] `crates/sandbox/tests/exec_child_confinement.rs` — negative assertions for the new Landlock/
      seccomp exec-child variant, mirroring `crates/sandbox/tests/confinement_integration.rs`'s
      existing pattern (worker cannot read `~/.ssh`, cannot reach network, cannot exec
      un-allowlisted binaries — `PLAN.md:152`)
- [ ] `scripts/check-invariants.sh` Gate 3 extension for `mint_from_exec(` (see checklist)

*(Phase 31 itself: None — a design document plus its gate record are the only artifacts; no test
gap exists for prose.)*

## Security Domain

`security_enforcement` is not set to `false` in `.planning/config.json` (absent = enabled) — this
section is required, and doubly so given this phase's entire subject is TCB confinement design.

### Applicable ASVS Categories

This project's own I0/I1/I2 invariant model is more specific than generic ASVS, but the mapping
below is useful for cross-checking no generic category is silently missed.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V1 Architecture | yes | Design-gate-first discipline itself (this phase) — no TCB code before a reviewed doc clears adversarial review, mirroring v1.0 P2 through v1.6 P26 |
| V4 Access Control | yes | Sink-level authorization: `KNOWN_SINKS` schema gate + `expected_role` slot-type binding (existing `crates/executor` machinery, extended by table entries only) |
| V5 Input Validation | yes | `process.exec` arg schema (Open Decision 4) — argv-array (never shell-string) command construction closes shell-injection by construction; `openat2` path resolution closes path-traversal by construction |
| V6 Cryptography | no (this phase) | No new key material; the existing HMAC-SHA256 audit chain (v1.6 HARDEN-02) is unaffected |
| V8 Data Protection (informative, not a numbered ASVS category in this project's usage) | yes | Exec-output taint labeling (`ExecRaw` + `ExternalUntrusted`) is the data-protection control — untrusted process output is never trusted-by-default |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Arbitrary command execution via a tainted `command`/`args` value | Elevation of Privilege | `command`/`args` classified routing- AND content-sensitive under I2 (Open Decision 8) — a tainted command Blocks before spawn |
| Shell metacharacter injection (`; rm -rf`, `$(...)`) if `args` were shell-joined | Tampering | `args: Vec<String>` passed directly to `execve`'s argv array — never through `sh -c` (Open Decision 4) |
| Symlink-escape / path-traversal on the fs write sink | Tampering / Information Disclosure | `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`, same as existing read/create paths |
| Exec child resource exhaustion (fork bomb, infinite loop, unbounded output) | Denial of Service | `RLIMIT_AS`/`RLIMIT_CPU` (reused) + new wall-clock timeout (Open Decision, Standard Stack) + a bound on captured stdout/stderr byte count (not yet pinned — recommend the DESIGN doc add an explicit cap, e.g. mirroring a sane default like 10MiB, denying/truncating beyond it, fail-closed not fail-open) |
| Network exfiltration from inside the exec child | Information Disclosure | Reused seccomp `socket(AF_INET/AF_INET6)` deny rule |
| Recursive/nested exec escaping the intended one-shot confinement | Elevation of Privilege | Recommend denying `execve`/`execveat` for the exec child's OWN descendants (Open Decision 3) |

## Gate-Record Shape

Mirrors `planning-docs/DESIGN-GATE-RECORD-v1.5.md` and `-v1.6.md` — the orchestrator (not a
`gsd-executor`) owns the review spawn per `.planning/REQUIREMENTS.md`'s "Standing precedent."

**File:** `planning-docs/DESIGN-GATE-RECORD-v1.7.md` (new — v1.7's first design gate).

**Required contents (per the v1.5/v1.6 precedent structure):**

1. **Header** — milestone, document under review (`DESIGN-effect-breadth-exec.md`), gate purpose,
   requirements gated (DESIGN-13, DESIGN-14).
2. **Gate status** — CLEARED / BLOCKED, with round number and date.
3. **Reviewer identity & independence** — MUST be a fresh, non-self agent (e.g. a distinct Fable-5
   agent per `[[fresh-context-adversarial-review]]`/`[[advisor-tool-unavailable-fable-fallback]]`
   prior-incident memory) — explicitly state the mechanism, that it is NOT the doc's authoring
   context, and that the review is **code-traced, not prose-read** (list every file the reviewer
   actually opened — mirrors v1.6's explicit file list, `DESIGN-GATE-RECORD-v1.6.md`).
4. **Revision history table** — round, date, reviewer, findings count by severity, result.
5. **Findings & resolutions** — per finding: Claim, Code evidence (file:line, orchestrator
   re-verified against live code — this project's standing discipline that AI reviewers generate
   false positives, so every finding gets independently re-checked before folding), Resolution
   (which DESIGN doc section absorbed the fix).
6. **Confirmation no TCB code was written** during the design-gate phase — explicit statement,
   mirrors every prior gate record's closing line.

**What the reviewer must specifically check, given this phase's content (beyond generic doc
review):**
- The `pre_exec` confinement ordering claim (Open Decision 1/Pitfall 5) — does the reviewer accept
  the async-signal-safety residual as documented, or does it independently trace `landlock`/
  `seccompiler` crate internals to check for allocation, and does it agree Option A is acceptable
  vs. requiring Option B?
- Whether `process.exec`'s `command`/`args` are genuinely sensitivity-classified (not left
  `expected_role = None`) — this is the single highest-consequence design decision in the doc; the
  reviewer should specifically look for it and confirm it is not silently missing.
- Whether the doc explicitly mandates the Gate 3 (`check-invariants.sh`) extension for
  `mint_from_exec(` — a missing mandate here is exactly the kind of "found no NEEDS-FIX but still
  wrong" gap the fresh-review discipline exists to catch.
- Whether the fs write/edit sink's flag combination (`O_WRONLY|O_TRUNC`, no `O_CREAT`/`O_EXCL`) is
  pinned precisely enough that Phase 33 cannot accidentally reintroduce an overwrite-capable
  `O_CREAT` path that would also silently satisfy `file.create`-style new-file creation through the
  "wrong" sink (schema/sensitivity confusion between the two fs sinks).

## Sources

### Primary (HIGH confidence — direct code reads this session)
- `crates/adapter-fs/src/lib.rs`, `crates/adapter-fs/src/workspace.rs` — fd-pass mediation, `openat2` read/create patterns
- `crates/sandbox/src/lib.rs`, `landlock.rs`, `seccomp.rs`, `rlimits.rs` — worker self-confinement model, `pre_exec`-compatibility doc comments, explicit rationale for why `pre_exec` is wrong for the worker's own binary
- `cli/caprun/src/main.rs` — planner sidecar spawn (unconfined precedent), worker spawn (self-confine precedent)
- `crates/runtime-core/src/plan_node.rs`, `value_record.rs`, `event.rs` — `TaintLabel`, `ValueRecord`, `origin_role`, `Event` shape
- `crates/executor/src/lib.rs`, `sink_schema.rs`, `sink_sensitivity.rs` — `submit_plan_node` decision logic, schema/sensitivity/role tables
- `crates/brokerd/src/quarantine.rs` — `mint_from_read`/`mint_from_derivation` (the mint-site template), fail-closed unknown-claim_type discipline
- `crates/brokerd/src/sinks/file_create.rs` — the two-phase durable audit dispatch template
- `scripts/check-invariants.sh` — Gates 1-4, especially Gate 3's exact call-site restriction mechanism
- `planning-docs/PLAN.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `CLAUDE.md` — locked invariants, terminology, hard constraints, standing design-gate precedent

### Secondary (MEDIUM confidence)
- `planning-docs/DESIGN-slot-type-binding.md`, `DESIGN-plan-executor.md`, `DESIGN-taint-model.md` (section headers read; full prose not exhaustively re-read this session) — structural template for the new DESIGN doc
- `planning-docs/DESIGN-GATE-RECORD-v1.5.md`, `-v1.6.md` — gate-record shape template
- Workspace `Cargo.toml` — confirmed exact pinned versions of `landlock`/`seccompiler`/`nix`

### Tertiary (LOW confidence — training knowledge, flagged `[ASSUMED]` inline)
- Landlock kernel version floor (ABI::V3 ≥5.19, negotiates to ABI::V1 ≥5.13) and `openat2`
  RESOLVE_* flag kernel floor (≥5.6) — general kernel knowledge, not re-verified against a kernel
  changelog this session
- `std::process::Command::pre_exec` async-signal-safety contract and whether `landlock`/
  `seccompiler` internals allocate — flagged as an explicit open residual risk, not asserted as fact

## Metadata

**Confidence breakdown:**
- Existing-pattern grounding (fd-pass, self-confinement, openat2, mint sites, I2/slot-type-binding
  tables): HIGH — every claim traces to a specific file:line read this session.
- Exec-child confinement mechanism itself (`pre_exec`, new Landlock ruleset, wall-clock timeout,
  async-signal-safety): MEDIUM — grounded in the absence of prior art (verified by grep) plus
  reasoned extension of existing primitives, but genuinely novel and explicitly flagged for
  adversarial-review scrutiny rather than presented as settled.
- Kernel version floors and general seccomp/exec inheritance semantics: LOW-MEDIUM — training
  knowledge, tagged `[ASSUMED]`, recommended for confirmation against the actual verification
  container before Phase 32 implementation.

**Research date:** 2026-07-17
**Valid until:** ~14 days (fast-moving relative to this project's own pace — multiple phases
typically land within a week; re-verify file:line citations if Phase 32/33 begins after
substantial further commits, per this project's own convention noted in
`DESIGN-slot-type-binding.md`'s Grounding line: "re-verify if Phase 24 begins many commits later").
