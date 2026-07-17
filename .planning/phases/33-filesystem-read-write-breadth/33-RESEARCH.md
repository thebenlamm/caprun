# Phase 33: Filesystem Read/Write Breadth - Research

**Researched:** 2026-07-17
**Domain:** Rust TCB implementation — extending the existing `adapter-fs`/`brokerd`/`executor` fs-mediation seam (openat2 single-syscall resolution, two-phase durable audit, I2 sink table) with a bounded multi-file read and a new existing-file-only write/edit sink.
**Confidence:** HIGH for every claim traced to a specific file:line read this session (marked `[VERIFIED: local code, read this session]`); the design model itself is `[CITED: planning-docs/DESIGN-effect-breadth-exec.md]` (a cleared design gate, binding, not re-litigated); a small number of deployment-constant recommendations (the `RequestFd` counter's numeric bound, the sink id string, new function names) are `[ASSUMED]` and flagged explicitly with rationale. No external network research was needed or used — this phase is 100% internal-codebase grounding, mirroring Phase 32's research posture.

This is an **implementation** phase. The security model is pinned and gate-cleared (`planning-docs/DESIGN-effect-breadth-exec.md` §3/§4/§5/§6/§7, cleared Round 1, `planning-docs/DESIGN-GATE-RECORD-v1.7.md`). This document does not re-derive the model — it grounds it against the CURRENT code (which has moved since Phase 31's research, because Phase 32 landed in between and touched several of the same shared files: `server.rs`'s `evaluate_plan_node_and_record`, `plan_node.rs`'s `TaintLabel` enum, `sink_schema.rs`/`sink_sensitivity.rs`, `check-invariants.sh` Gate 3) and resolves the two DESIGN-doc-deferred deployment constants (§8 items 2 and the sink-id literal).

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-------------------|
| FS-01 | Worker can read multiple workspace files beyond the single current read path, each `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)`, taint-minted untrusted like the existing read path | File-by-file §server.rs `RequestFd` arm, Pattern 1 (per-session read-count limiter), Code Examples |
| FS-02 | A write/edit sink modifies an EXISTING file within `WorkspaceRoot` (beyond `file.create`'s `O_EXCL` new-file-only), fail-closed on path schema, kernel-confined, durably audited | File-by-file §workspace.rs (`write_within`), §sinks/file_write.rs, Pattern 2, Validation Architecture FS-02 row |
| FS-03 | fs write/edit sink args governed by executor under the same I2/slot-type-binding discipline — tainted path/contents in a sensitive slot Blocks; no I2 bypass, no new raw `EffectRequest` | File-by-file §sink_schema.rs/§sink_sensitivity.rs, Code Examples, Security Domain |

## DESIGN-doc-vs-current-code drift this research resolves (read first)

Phase 31's DESIGN doc and Phase 32's own research were grounded against code that has since changed — Phase 32 (EXEC-01..04) landed in full between then and now. Three things drifted that matter for planning FS-01..03:

1. **`evaluate_plan_node_and_record`'s Allowed-dispatch shape now has THREE arms, not two.** `crates/brokerd/src/server.rs:868-1074` — `file.create` (868-897), `email.send` (899-1035), `process.exec` (1037-1074, newly landed). The function returns `anyhow::Result<(ExecutorDecision, Option<ValueId>)>` (`server.rs:664-667`) — the `Option<ValueId>` (`output_value_id`) field was ADDED by Phase 32 for `process.exec`'s non-terminal output handle. **FS-02's new `file.write` arm is a FOURTH sibling arm, mirroring `file.create`'s shape exactly (terminal effect, no new taint-tracked value, `output_value_id` stays `None`)** — it does NOT need the `process.exec`-style value-handle threading; `file.write` never returns a new `ValueId` to the worker, exactly like `file.create` today.
2. **`TaintLabel` already grew an 8th variant (`ExecRaw`) in Phase 32** (`crates/runtime-core/src/plan_node.rs:13-29`, `is_untrusted()` at `:45-56`). **FS-01/02/03 need ZERO new `TaintLabel` variants** — the fs write/edit sink consumes existing taint (via I2's per-arg check), it does not mint a NEW kind of untrusted origin. This is a materially simpler shape than Phase 32's exec-output mint.
3. **`check-invariants.sh` Gate 3 was already extended for `mint_from_exec(` in Phase 32** (`scripts/check-invariants.sh:136`, confirmed present this session). **FS-02/03 need NO Gate 3 change** — the new `file.write` sink never calls `mint_from_read`/`mint_from_derivation`/`mint_from_exec`/`.mint(` anywhere; it only *resolves* existing handles (`ValueStore::resolve`, read-only), exactly like `file_create.rs`'s `invoke_file_create` does today. Confirm this explicitly in the Phase 33 plan review — a planner unfamiliar with Phase 32's Gate-3 extension might assume a new sink always needs a Gate-3 line; it does not, unless it mints.

## Existing-pattern → new-code map

| New symbol / file | Closest existing analog (file:line, read this session) | Reuse vs. new |
|---|---|---|
| `WorkspaceRoot::write_within` (Linux) | `WorkspaceRoot::create_exclusive_within` (`crates/adapter-fs/src/workspace.rs:132-151`) | NEW method, same file, `OFlag::O_WRONLY\|O_TRUNC` instead of `O_CREAT\|O_EXCL\|O_WRONLY` — no `mode()` call needed (mode only matters with `O_CREAT`) |
| `WorkspaceRoot::write_within` (non-Linux stub) | `create_exclusive_within`'s non-Linux stub (`workspace.rs:158-167`) | NEW stub, `OpenOptions::new().write(true).truncate(true)` — deliberately NO `.create(true)`/`.create_new(true)`, so a missing target still errors on macOS too (dev-machine parity, no security claim either way) |
| `write_within`'s negative test set (`absolute_path_rejected`, `parent_traversal_rejected`, `symlink_escape_rejected`, PLUS a NEW `missing_target_enoent` test) | `create_exclusive_within`'s existing 4 tests (`workspace.rs:290-401`) | NEW tests, same file, same `unique_tmp_root` harness — DESIGN §3.2 is explicit these are NOT inherited from the `O_RDONLY`/`O_CREAT\|O_EXCL` coverage |
| `crates/brokerd/src/sinks/file_write.rs::invoke_file_write()` | `crates/brokerd/src/sinks/file_create.rs::invoke_file_create()` (`file_create.rs:65-116`) | NEW module, two-phase-audit structure copied verbatim (own private `resolve_arg` helper too — no shared helper exists across sink modules today, `file_create.rs` and `process_exec.rs` each keep their own copy; mirror that convention, don't introduce a shared one in this phase) |
| `crates/brokerd/src/sinks.rs` — `pub mod file_write;` | `pub mod process_exec;` (added Phase 32) | Trivial addition |
| New Allowed-dispatch arm in `evaluate_plan_node_and_record` for `"file.write"` | The `"file.create"` arm (`server.rs:868-897`) | NEW arm, same locking/head-advance shape, NO mint call (mirrors `file.create`, not `process.exec`) |
| `KNOWN_SINKS` entry for `"file.write"` | the `"file.create"` entry (`crates/executor/src/sink_schema.rs:53-57`) | NEW table row |
| `sink_sensitivity.rs` — `FILE_WRITE_ROUTING_SENSITIVE`/`FILE_WRITE_CONTENT_SENSITIVE` consts + 4 match arms (`sink_effect_class`, `is_routing_sensitive`, `is_content_sensitive`, `expected_role`) | `FILE_CREATE_ROUTING_SENSITIVE`/`FILE_CREATE_CONTENT_SENSITIVE` + their arms (`sink_sensitivity.rs:64-102,182-197`) | NEW consts + arms, same file |
| Per-session `RequestFd` count limiter (`fd_request_count: &mut u32`, threaded param) | The existing `fd_requested: &mut bool` / `intent_provided: &mut bool` per-connection ordering state (`server.rs:496-497,1254-1256`) | NEW counter, same threading pattern (declared in `handle_connection`, passed `&mut` into `dispatch_request`, mutated inside the `RequestFd` arm) |
| `crates/adapter-fs/tests` or inline `workspace.rs` test additions | N/A — extends the existing inline `#[cfg(test)] mod tests` (`workspace.rs:170-402`) | Additive to the SAME module, not a new test file (matches how `create_exclusive_within`'s tests were added inline rather than in a new `tests/` file) |
| `cli/caprun/tests/s9_file_write_block.rs` | `cli/caprun/tests/s9_process_exec_block.rs` (Phase 32's own genuine-taint-Block acceptance test, full file read this session) | NEW test file, same shape: mint a tainted value into `file.write`'s `path` or `contents` slot, call `executor::submit_plan_node`, assert `BlockedPendingConfirmation` with an unbroken `provenance_chain[0]` anchor — no live spawn/confinement machinery needed here (fs write has no exec-child analog) |

## File-by-file change list

| File | Change |
|---|---|
| `crates/adapter-fs/src/workspace.rs` | ADD `write_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()>` (Linux + non-Linux stub variants), beside `create_exclusive_within`. ADD negative tests (Pattern 1 below). |
| `crates/brokerd/src/sinks/file_write.rs` | NEW. `invoke_file_write()` — mirrors `invoke_file_create` verbatim (two-phase `sink_executed`/`sink_execution_failed` audit, `actor = format!("sink:file.write:{effect_id}")`), calling `workspace_root.write_within(&path, contents.as_bytes())` instead of `create_exclusive_within`. Own private `resolve_arg` helper (copy `file_create.rs`'s, do not import cross-module). |
| `crates/brokerd/src/sinks.rs` | ADD `pub mod file_write;`. |
| `crates/brokerd/src/server.rs` | (a) ADD a fourth Allowed-dispatch arm in `evaluate_plan_node_and_record`, `plan_node.sink.0 == "file.write"`, mirroring the `file.create` arm (868-897) — NO `output_value_id` involvement (stays `None`, unchanged from today's default for every non-`process.exec` sink). Place it adjacent to the `file.create` arm for readability (order does not affect behavior — each arm is gated on a disjoint `plan_node.sink.0` string). (b) ADD the per-session `RequestFd` count limiter: a new `let mut fd_request_count: u32 = 0;` beside `intent_provided`/`fd_requested` (`server.rs:496-497`), a new `fd_request_count: &mut u32` parameter threaded through `dispatch_request`'s signature and its production call site (`server.rs:616-630`), incremented and bound-checked at the TOP of the `RequestFd` arm (Pattern 2 below) — touches the EXISTING single-read path, not merely new code (DESIGN §11 m1). |
| `crates/brokerd/src/server.rs` (test module) | UPDATE the 5 existing inline unit-test call sites of `dispatch_request` (`server.rs:1953,1992,2014,2075,2106` at research time) to pass a new `&mut 0u32` (or a locally-declared counter) for the new parameter — a compile error otherwise, not a silent gap. |
| `crates/executor/src/sink_schema.rs` | ADD `KNOWN_SINKS` entry: `SinkSchema { sink: "file.write", allowed: &["path","contents"], required: &["path","contents"] }` (mirrors `file.create`'s exact-match shape — both args required, no optional-arg asymmetry like `process.exec`'s `args`/`cwd`). |
| `crates/executor/src/sink_sensitivity.rs` | ADD `FILE_WRITE_ROUTING_SENSITIVE: &[&str] = &["path"]`, `FILE_WRITE_CONTENT_SENSITIVE: &[&str] = &["contents"]`; ADD `"file.write"` arms to `sink_effect_class` (→ `CommitIrreversible`), `is_routing_sensitive`, `is_content_sensitive`, `expected_role` (see Code Examples). |
| `scripts/check-invariants.sh` | **NO change.** `file.write` never mints (confirmed drift-note #3 above) — Gate 3's existing 4-token list stays as-is. Gate 1 (`EffectRequest` absence) also stays green with zero new hits — `file.write` is a `PlanNode{sink,args}` from spawn, same as every other sink. |
| `cli/caprun/tests/s9_file_write_block.rs` | NEW. FS-03's genuine-non-stapled-taint-Block acceptance test, mirroring `s9_process_exec_block.rs`'s structure but WITHOUT any spawn/confinement machinery (fs write has no exec-child analog — it is a single in-broker `openat2` call). |

## Standard Stack

No new crates, no new Cargo features. `[VERIFIED: Cargo.toml/Cargo.lock, read this session]`

### Core (already vendored, reused unchanged)

| Library | Version | Purpose | New usage this phase |
|---|---|---|---|
| `nix` | 0.31.3 (workspace, `fs` feature already enabled) | `openat2`, `OpenHow`, `ResolveFlag`, `OFlag` | Reused UNCHANGED — `write_within` calls the identical `openat2` API `read_within`/`create_exclusive_within` already use, just a different `OFlag` combination. No new nix feature needed (`process`/`socket`/etc. features are for the exec-child work, orthogonal to this phase). |
| `std::fs::File` / `std::io::Write` | stdlib | Wrap the returned fd, write bytes, `sync_all()` | Reused unchanged (identical to `create_exclusive_within`'s tail). |

No `serde_json`, no `tokio::process`, no new async-cancellation machinery is needed anywhere in this phase — `file.write` is a synchronous, single-syscall effect exactly like `file.create`, dispatched from the SAME synchronous branch of `evaluate_plan_node_and_record` (the function itself is `async fn` only because `process.exec`'s arm needs `.await`; the new `file.write` arm is plain synchronous code inside it, mirroring `file.create`'s arm).

### Alternatives considered

| Instead of | Could use | Tradeoff |
|---|---|---|
| A brand-new `write_within` method | Overloading `create_exclusive_within` with an `overwrite: bool` parameter | REJECTED — DESIGN §3.2 explicitly warns against blurring the two semantics into one function; two distinct methods (like the existing `read_within`/`create_exclusive_within` split) keep the schema/sensitivity table unambiguous about which sink owns "can create" vs "can overwrite" authority. |
| A per-session `RequestFd` counter as a plain `u32` local | A dedicated `RequestFdBudget` struct with a configurable limit | The plain counter matches this codebase's existing "no runtime-configurable security parameter" discipline (hardcoded constant, not a config knob) — a struct would only be warranted if the limit needed to vary per-session, which DESIGN §8 item 2 does not call for. |

## Package Legitimacy Audit

**Not applicable.** No new external registry dependency this phase — `nix` and `std` are already vendored and legitimacy-checked in prior milestones. No new workspace member, no new Cargo feature flag.

## Architecture Patterns

### System Architecture Diagram — FS-01 (bounded multi-file read) and FS-02/03 (write/edit sink under I2)

```
 Worker (kernel-confined)              Broker (brokerd)                      Kernel
      │                                      │                          (openat2, Landlock*)
      │  RequestFd{path: "a.rs"}             │
      ├─────────────────────────────────────►│
      │                                fd_requested = true
      │                                fd_request_count += 1
      │                                if count > MAX_REQUEST_FD_PER_SESSION:
      │                                  → BrokerResponse::Error, deny THIS request,
      │                                    connection stays open (fail-closed, not fatal)
      │                                else:
      │                                  workspace_root.read_within(path)
      │                                    openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS) ──►│
      │                                  fstat-compare → session_demoted if untrusted
      │                                  pass_fd (SCM_RIGHTS)                              │
      │◄─────────────────────────────────────┤ FdGranted                                   │
      │  (repeat RequestFd N times — FS-01 "no new mechanism, only documented multiplicity")│
      │                                      │                                             │
      │  SubmitPlanNode{sink:"file.write",   │                                             │
      │    args:[path, contents]}            │                                             │
      ├─────────────────────────────────────►│                                             │
      │                                Step 0 schema gate (KNOWN_SINKS "file.write")        │
      │                                Step 1c role check (path, contents)                  │
      │                                Collect-then-Block (path/contents tainted → Block)   │
      │                                ── if Allowed ──►                                    │
      │                          sinks::file_write::invoke_file_write()                     │
      │                            resolve path/contents from ValueStore                    │
      │                            workspace_root.write_within(path, contents)               │
      │                              openat2(O_WRONLY|O_TRUNC, RESOLVE_BENEATH|              │
      │                                RESOLVE_NO_SYMLINKS) — NO O_CREAT ─────────────────►│
      │                              ENOENT if target missing (fail-closed, no silent create)│
      │                            append sink_executed / sink_execution_failed              │
      │◄── PlanNodeDecision{Allowed, output_value_id: None} ─┤ (no new value handle —        │
      │                                                        file.write is terminal, same  │
      │                                                        as file.create)               │
```
`*` — no NEW Landlock/seccomp confinement primitive is introduced this phase; `WorkspaceRoot`'s existing broker-side dirfd-anchored `openat2` mediation is reused unmodified (unlike `process.exec`, there is no separate confined child process here).

### Pattern 1: `write_within` — existing-file-only sibling of `create_exclusive_within`

```rust
// crates/adapter-fs/src/workspace.rs — NEW, beside create_exclusive_within
// [VERIFIED: openat2/OpenHow/ResolveFlag API identical to read_within/
// create_exclusive_within, already used in this exact file, workspace.rs:90-102,132-151]

/// Write into an EXISTING file resolved BENEATH the workspace-root anchor
/// (Linux; write/edit side of FS-02, a sibling of `create_exclusive_within`
/// with a DIFFERENT OFlag set).
///
/// `O_WRONLY | O_TRUNC` — explicitly NO `O_CREAT`, NO `O_EXCL`. A missing
/// target fails closed with `ENOENT` (never silently creates the file) —
/// this is the semantic split from `create_exclusive_within`'s new-file-only
/// `O_CREAT|O_EXCL` (DESIGN §3.2). Same `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`
/// single-syscall, TOCTOU-safe kernel resolution as every other
/// workspace-scoped path in this codebase.
///
/// Do NOT add `O_CREAT` here (with or without `O_EXCL`) — doing so would
/// blur this sink's new-file-permitting behavior with `file.create`'s
/// new-file-ONLY semantics (DESIGN §3.2's explicit warning).
///
/// # Errors
/// `ENOENT` if `rel_path` does not exist, `EXDEV` (or other raw OS error)
/// for a `RESOLVE_*` violation, or a write error.
#[cfg(target_os = "linux")]
pub fn write_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
    use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
    use std::io::Write;
    use std::os::fd::AsFd;

    let how = OpenHow::new()
        .flags(OFlag::O_WRONLY | OFlag::O_TRUNC)
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);

    let fd = openat2(self.dirfd.as_fd(), rel_path, how)
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

    let mut file = std::fs::File::from(fd);
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
}

/// Non-Linux stub — NO security claim (dev-machine compilation only).
/// Deliberately NO `.create(true)`/`.create_new(true)` — a missing target
/// still errors here too, mirroring the Linux path's ENOENT contract.
#[cfg(not(target_os = "linux"))]
pub fn write_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(self.root_path.join(rel_path))?;
    file.write_all(contents)?;
    Ok(())
}
```

`[ASSUMED — method name]`: `write_within` is this research's recommended name (symmetric with `read_within`, distinct from `create_exclusive_within`). The DESIGN doc does not pin a literal name, only "a straightforward sibling." Flag for confirmation, low-stakes (naming only, no security consequence either way).

### Pattern 2: `RequestFd` per-session read-count limiter (FS-01, resolves DESIGN §8 item 2)

**Recommended numeric bound: 256.** `[ASSUMED — MEDIUM confidence]` DESIGN §8 item 2 explicitly defers the exact value to Phase 33. Rationale: a "read many workspace files" coding-agent workflow plausibly touches low hundreds of files in a single session (e.g. a moderate-sized repo's source tree) without ever needing thousands; 256 is generous enough not to break legitimate multi-file-read workflows while still bounding worst-case per-connection resource consumption (this is a resource-exhaustion guard, not a functional gate — DESIGN §3.1 is explicit this is NOT meant to constrain legitimate use). Recommend the human reviewer confirm this number is not surprising before Phase 33 lands; it is trivially bumpable later (a hardcoded `const`, not a runtime policy knob, matching this codebase's "security parameters are hardcoded, not configurable" discipline).

**Placement and semantics — deny THIS request, keep the connection alive** (mirrors `ReportClaims`'s existing error-handling shape, `server.rs:1499-1509`: send an `Error` response, then `return Ok(())` from `dispatch_request` — NOT `break`, which would end the whole connection). The counter increments on EVERY `RequestFd` attempt (success or eventual failure), at the very top of the arm — mirrors `*fd_requested = true;`'s existing "set at entry, before any other work" discipline (`server.rs:1279`) so a worker cannot dodge the counter by triggering read failures.

```rust
// crates/brokerd/src/server.rs — inside dispatch_request, new param + top of
// the RequestFd arm. MAX_REQUEST_FD_PER_SESSION is a module-level const.
const MAX_REQUEST_FD_PER_SESSION: u32 = 256;

// ... inside `BrokerRequest::RequestFd { path } => { ... }`, immediately
// after `*fd_requested = true;` (server.rs:1279):
*fd_request_count += 1;
if *fd_request_count > MAX_REQUEST_FD_PER_SESSION {
    send_response(
        stream,
        &BrokerResponse::Error {
            message: format!(
                "RequestFd count ({}) exceeded the per-session limit ({MAX_REQUEST_FD_PER_SESSION}) \
                 — fail-closed resource-exhaustion guard",
                *fd_request_count
            ),
        },
    )
    .await?;
    return Ok(());
}
// ... existing read_within/fstat/pass_fd logic follows unchanged ...
```

The new parameter threads exactly like `fd_requested`/`intent_provided`: declared `let mut fd_request_count: u32 = 0;` in `handle_connection` (beside `server.rs:496-497`), passed as `&mut fd_request_count` at the production call site (`server.rs:616-630`), and the 5 existing inline unit tests that call `dispatch_request` directly (`server.rs:1953,1992,2014,2075,2106` at research time — confirm exact count via `grep -n "dispatch_request(" crates/brokerd/src/server.rs` at implementation time, since Phase 32 may have shifted line numbers again) need a `&mut 0u32` (or a named local) added to their call sites — a compile error surfaces every missed site, never a silent gap.

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---|---|---|---|
| Path traversal / symlink-escape prevention for the write/edit target | Manual `..`-stripping or a canonicalize-then-compare check | The SAME `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)` kernel-atomic resolution `read_within`/`create_exclusive_within` already use | Consistency + TOCTOU-safety; a second, hand-rolled path-safety mechanism would double the surface a fresh adversarial review must trace, for zero benefit |
| Per-session resource-exhaustion bounding | A generic rate-limiter crate or token-bucket abstraction | A plain hardcoded `u32` counter + `const` bound, mirroring the existing `fd_requested`/`intent_provided` per-connection state shape | This codebase's standing discipline: security parameters are hardcoded constants in the Rust TCB, never a runtime-configurable policy surface (mirrors `sink_sensitivity`'s own "hardcoded, no dynamic registry" doc comment) |
| Two-phase durable audit for the new sink | A shared "sink audit" trait/abstraction extracted across `file_create.rs`/`process_exec.rs`/`file_write.rs` | Copy the existing two-phase `sink_executed`/`sink_execution_failed` shape verbatim into the new module, as `process_exec.rs` already did for its own two-phase pair | No such shared abstraction exists yet in this codebase (each sink module keeps its own copy) — introducing one in this phase would be an unrequested refactor outside FS-01..03's scope; mirror the existing convention, don't extract prematurely |

**Key insight:** every new mechanism in this phase is a narrow, same-shape extension of an already-verified pattern (`openat2` resolution, two-phase audit, hardcoded sink tables) — there is no genuinely novel composition here, unlike Phase 32's launcher/`pre_exec`/async-capture work. This phase should be lower-risk to review than Phase 32.

## Common Pitfalls / Landmines

### Pitfall 1: reintroducing `O_CREAT` on `write_within`
Blurs the write/edit sink's existing-file-only semantics with `file.create`'s new-file-only semantics, creating two sinks with overlapping "can create a file" authority (DESIGN §3.2's explicit warning, re-confirmed against the live `create_exclusive_within` code this session). `write_within` MUST use `O_WRONLY | O_TRUNC` only, never `O_CREAT`.

### Pitfall 2: assuming the existing negative tests cover the new flag combination
`workspace.rs`'s existing `absolute_path_rejected`/`parent_traversal_rejected`/`symlink_escape_rejected` tests (`workspace.rs:220-288`) exercise `O_RDONLY` only; the `create_exclusive_*` siblings (`workspace.rs:290-401`) exercise `O_CREAT|O_EXCL|O_WRONLY` only. Neither proves `O_WRONLY|O_TRUNC` behaves the same way. Phase 33 MUST write the equivalent set for the new flag combination, PLUS a NEW `ENOENT`-on-missing-target test that has no analog in either existing set (DESIGN §3.2, re-confirmed this session — no such test exists yet in `workspace.rs`).

### Pitfall 3: assuming a new sink always needs a `check-invariants.sh` Gate 3 change
Phase 32 needed a Gate-3 extension because `mint_from_exec` is a NEW mint call site. `file.write` never mints — it only resolves existing handles via `ValueStore::resolve` (read-only), exactly like `file.create`'s `invoke_file_create` does today. Do NOT add a `file.write`/`invoke_file_write` line to Gate 3; there is nothing for it to restrict.

### Pitfall 4: forgetting the `RequestFd` counter modifies the EXISTING single-read path
The counter is not purely additive "new multi-file code" — it wraps the SAME `RequestFd` arm every single-file read already goes through (DESIGN §11 m1, re-confirmed against `server.rs:1274-1439` this session: there is exactly ONE `RequestFd` arm, used by both a single read and N repeated reads). A plan that treats this as "add a new code path for multi-file reads" will miss that the counter must live in the one arm that already exists.

### Pitfall 5: `cfg(linux)` test-blindness (this project's own standing lesson, still applies)
`cargo test` on macOS compiles ZERO `#[cfg(target_os="linux")]` test targets — `write_within`'s Linux implementation and its negative tests are behind this cfg gate. A green Mac `cargo build --workspace` proves the crate COMPILES but proves NOTHING about the new `openat2` flag combination's actual behavior. Phase 33 verification MUST run the Linux container (`scripts/mailpit-verify.sh` or a scoped `MAILPIT_VERIFY_CMD` override) before claiming FS-02's negative tests pass.

### Pitfall 6: conflating an `ENOENT` sink-invocation failure with an executor `Denied`/`Block` decision
`ENOENT` on a missing write target is an OS-level error surfaced through `invoke_file_write`'s `Err` branch (recorded as `sink_execution_failed`, then propagated) — it is NOT an `ExecutorDecision::Denied` or `BlockedPendingConfirmation`. DESIGN §5's table phrasing ("missing target → Deny") uses "Deny" colloquially for "kernel-level rejection," not the `DenyReason` enum. Do not implement a new `DenyReason` variant for this — mirror `file.create`'s existing `EEXIST` handling shape exactly (an `Err` path through the sink module, not a new executor decision variant).

## Code Examples

### `KNOWN_SINKS` entry (exact table row)
```rust
// crates/executor/src/sink_schema.rs — add to KNOWN_SINKS
SinkSchema {
    sink: "file.write",
    allowed: &["path", "contents"],
    required: &["path", "contents"],
},
```

### `sink_sensitivity.rs` additions
```rust
// crates/executor/src/sink_sensitivity.rs
pub const FILE_WRITE_ROUTING_SENSITIVE: &[&str] = &["path"];
pub const FILE_WRITE_CONTENT_SENSITIVE: &[&str] = &["contents"];

// sink_effect_class match arm:
"file.write" => EffectClass::CommitIrreversible,

// is_routing_sensitive match arm:
"file.write" => FILE_WRITE_ROUTING_SENSITIVE.contains(&arg_name),

// is_content_sensitive match arm:
"file.write" => FILE_WRITE_CONTENT_SENSITIVE.contains(&arg_name),

// expected_role match arm:
"file.write" => match arg_name {
    // Mirrors file.create's `path` role list verbatim (DESIGN §4.3).
    "path" => Some(&["path", "relative_path"]),
    // Mirrors HARDEN-05's file.create `contents` reuse of the trusted
    // "path" role, PLUS the untrusted "exec_output" (Phase 32's new
    // origin_role) and "doc_fragment" (email.send body precedent) roles —
    // so a tainted exec-output or doc-fragment value routed here is
    // role-admissible and reaches I2's per-arg sensitivity Block instead
    // of a structural Step-1c Deny (DESIGN §4.3).
    "contents" => Some(&["path", "exec_output", "doc_fragment"]),
    _ => None,
},
```

### `invoke_file_write` (sketch, mirrors `invoke_file_create` verbatim)
```rust
// crates/brokerd/src/sinks/file_write.rs — NEW
use anyhow::{Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode};
use uuid::Uuid;

use adapter_fs::workspace::WorkspaceRoot;
use crate::audit::append_event;

#[allow(clippy::too_many_arguments)]
pub fn invoke_file_write(
    conn: &rusqlite::Connection,
    key: &[u8],
    value_store: &ValueStore,
    session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let path = resolve_arg(value_store, plan_node, "path")?;
    let contents = resolve_arg(value_store, plan_node, "contents")?;

    match workspace_root.write_within(&path, contents.as_bytes()) {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(), Some(parent_id), session_id,
                format!("sink:file.write:{effect_id}"), "sink_executed".into(),
                Utc::now(), vec![],
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_executed")?;
            Ok((event.id, hash))
        }
        Err(e) => {
            let event = Event::new(
                Uuid::new_v4(), Some(parent_id), session_id,
                format!("sink:file.write:{effect_id}"), "sink_execution_failed".into(),
                Utc::now(), vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_execution_failed")?;
            Err(anyhow::Error::new(e).context("file.write write_within failed"))
        }
    }
}

fn resolve_arg(store: &ValueStore, plan_node: &PlanNode, name: &str) -> Result<String> {
    let arg = plan_node.args.iter().find(|a| a.name == name)
        .ok_or_else(|| anyhow::anyhow!("file.write plan node missing `{name}` arg"))?;
    let record = store.resolve(&arg.value_id)
        .ok_or_else(|| anyhow::anyhow!("file.write `{name}` handle did not resolve"))?;
    Ok(record.literal.clone())
}
```

### New Allowed-dispatch arm in `evaluate_plan_node_and_record` (sketch, mirrors the `file.create` arm)
```rust
// crates/brokerd/src/server.rs — inside evaluate_plan_node_and_record,
// adjacent to the existing file.create arm (server.rs:868-897)
if matches!(decision, runtime_core::ExecutorDecision::Allowed)
    && plan_node.sink.0 == "file.write"
{
    let (sink_event_id, sink_hash) = {
        let locked = conn.lock().map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        crate::sinks::file_write::invoke_file_write(
            &locked, key, value_store, session_id, effect_id, plan_node,
            workspace_root, *last_event_id, last_event_hash,
        )?
    };
    *last_event_id = sink_event_id;
    *last_event_hash = sink_hash;
}
// No output_value_id change here — stays None, exactly like file.create's
// existing behavior (never touched by this arm).
```

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|---|---|---|
| A1 | The new sink id literal is `"file.write"` | throughout | DESIGN §3.3 only pins the ACTOR-field convention example (`sink:file.write:<effect_id>`), not the literal `KNOWN_SINKS` id — but that example string IS `"file.write"`, and it collides with nothing in `KNOWN_SINKS` (`email.send`, `file.create`, `process.exec`). Low risk; flag for a one-line human confirm before Phase 33 plan finalization. |
| A2 | `MAX_REQUEST_FD_PER_SESSION = 256` is a reasonable default | Pattern 2 | If wrong, either legitimate large-repo multi-file-read workflows get prematurely denied (too low) or the resource-exhaustion guard is too permissive to matter (too high) — low security risk either way since this is a DoS guard, not an I2 control; easily bumped, recommend confirming with the human reviewer rather than silently deciding |
| A3 | `write_within` is the right method name (vs. e.g. `edit_within`/`overwrite_within`) | Pattern 1 | Purely cosmetic/API-naming; no functional or security consequence |
| A4 | `file.write`'s `contents` `expected_role` should include `"exec_output"` alongside `"doc_fragment"`/`"path"` | Code Examples, DESIGN §4.3 | If `"exec_output"` is omitted, a legitimate chained `process.exec` → `file.write` flow (writing captured, still-tainted command output into a file) would fail-closed-Deny at Step 1c instead of reaching I2's content-sensitivity Block — the same HARDEN-05/M2 trap this project has hit repeatedly. DESIGN §4.3 explicitly pins this inclusion, so confidence is HIGH, not LOW, but flagged here because it is easy to omit by pattern-matching only against `file.create`'s existing (pre-`process.exec`) role list. |

**No claims in this research are tagged LOW-confidence `[ASSUMED]` in a way that risks a security-property regression** — every genuinely open question (A1-A4) is a naming/numeric-tuning choice, not a model decision. The security MODEL itself (I2 slot classification, fail-closed defaults, no-mint discipline) is fully pinned by the cleared DESIGN doc and re-verified against live code this session.

## Open Questions

1. **Should `file.write`'s two-phase audit event pair be named `sink_executed`/`sink_execution_failed` (reusing `file.create`'s literal event-type strings) or a distinct pair (e.g. mirroring `process.exec`'s distinct `process_exited`/`process_spawn_failed` naming)?**
   - What we know: `file.create` and (this research's recommendation) `file.write` are BOTH synchronous, single-syscall, terminal effects with no new taint-tracked output — the closest structural analog. `process.exec` chose distinct names because it needed a DISTINCT semantic (its success event doubles as a taint-mint anchor).
   - What's unclear: whether reusing the exact literal strings `"sink_executed"`/`"sink_execution_failed"` across BOTH `file.create` and `file.write` could make an audit-DAG query ambiguous about WHICH sink produced a given event (the `actor` field disambiguates via `sink:file.write:<effect_id>` vs `sink:file.create:<effect_id>`, so this is likely a non-issue, but worth an explicit planner decision rather than a silent default).
   - Recommendation: reuse `"sink_executed"`/`"sink_execution_failed"` verbatim (matches `file.create`'s literal strings) — the `actor` field already carries the sink-specific disambiguation, and inventing new event-type strings for a structurally-identical two-phase pattern would only fragment `find_event_by_type`-style queries across near-duplicate names for no benefit.

## Environment Availability

Skipped — this phase has no external dependencies beyond what is already vendored and verified (`nix` 0.31.3's existing `openat2` support, already exercised by `read_within`/`create_exclusive_within`). No new crate, no new Cargo feature flag, no new external tool or service.

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Cargo built-in (`cargo test`), workspace-wide `cargo test --workspace --no-fail-fast` |
| Config file | none — root `Cargo.toml` |
| Quick run (Mac, per-task commit) | `cargo build --workspace` (compiles the new method/module/table entries; `#[cfg(target_os="linux")]` tests do not run on Mac — expected, not a gap, per this project's standing convention) |
| Linux compile-check (mandatory, Pitfall 5) | `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo build --tests --keep-going` |
| Full Linux suite | `bash scripts/mailpit-verify.sh` (CLAUDE.md: mandatory from Phase 16 onward — `file.write` itself has no SMTP surface, but the shared verification harness stays the default since it also re-runs every prior sink's regression) |
| Scoped Linux run | `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p adapter-fs write_within && cargo test -p brokerd file_write && cargo test -p executor file_write' bash scripts/mailpit-verify.sh` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test type | Command | File exists? |
|---|---|---|---|---|
| FS-01 | `RequestFd` repeated N times succeeds (no new mechanism); per-session count exceeding the bound fails closed via `BrokerResponse::Error`, connection stays open | unit (Mac-compiling, in-process) | `cargo test -p brokerd request_fd_count_limit` (new, inline in `server.rs`'s existing `#[cfg(test)] mod tests`) | ❌ Wave 0 |
| FS-02 | `write_within` writes an existing file; rejects absolute/`..`/symlink paths at kernel resolution; `ENOENT` on a missing target | unit (Linux-gated) | `cargo test -p adapter-fs write_within` (new, inline in `workspace.rs`'s existing `#[cfg(test)] mod tests`) | ❌ Wave 0 |
| FS-02 | `invoke_file_write` two-phase audit (success → `sink_executed`; failure → `sink_execution_failed`, no retry) | unit (Mac-compiling, in-process rusqlite `:memory:`) | `cargo test -p brokerd invoke_file_write` (new, mirrors `file_create.rs`'s own test module shape) | ❌ Wave 0 |
| FS-03 | `file.write` schema/sensitivity/role table entries: tainted `path`/`contents` classified sensitive, `expected_role` accepts the untrusted `exec_output`/`doc_fragment` roles | unit (Mac-compiling) | `cargo test -p executor file_write` (new, inline in `sink_schema.rs`/`sink_sensitivity.rs`'s existing `#[cfg(test)] mod tests`) | ❌ Wave 0 |
| FS-03 | Tainted `path` or `contents` routed to `file.write` → deterministic `BlockedPendingConfirmation`, genuine (non-stapled) taint chain, unbroken `provenance_chain[0]` anchor | integration (Mac-compiling, mirrors `s9_process_exec_block.rs`'s non-live half) | `cargo test -p caprun --test s9_file_write_block` (new) | ❌ Wave 0 |

### Sampling rate
- **Per task commit:** `cargo build --workspace` (Mac) — confirms compile.
- **Per wave merge:** `cargo build --tests --keep-going` inside `rust:1` (Linux compile enumeration, Pitfall 5) — MANDATORY before declaring any wave touching `crates/adapter-fs`/`crates/brokerd`/`crates/executor` complete.
- **Phase gate:** `bash scripts/mailpit-verify.sh` full suite green (or a scoped `MAILPIT_VERIFY_CMD` covering every new test target above) — true-exit-before-pipe, asserted on named test counts, never `script | tail` exit-code laundering (`[[verification-exit-code-through-pipe]]`).
- **Full live composed-acceptance (§9-style, exec+fs combined) is Phase 34's scope (LIVE-01/02)** — Phase 33's gate is the per-requirement tests above, not the full composed live acceptance.

### Wave 0 gaps
- [ ] `crates/adapter-fs/src/workspace.rs` — `write_within` negative tests (absolute, `..`-traversal, symlink, PLUS the new `ENOENT`-on-missing-target test — none inherited from existing coverage, Pitfall 2)
- [ ] `crates/brokerd/src/sinks/file_write.rs` — NEW module + inline `#[cfg(test)]` tests
- [ ] `crates/brokerd/src/server.rs` — inline test asserting the `RequestFd` per-session counter denies past `MAX_REQUEST_FD_PER_SESSION`, and that a normal (under-bound) sequence of repeated reads still succeeds
- [ ] `crates/executor/src/{sink_schema.rs,sink_sensitivity.rs}` — inline test additions for `file.write` (schema exact-match, routing/content sensitivity, `expected_role` including the untrusted-role acceptance)
- [ ] `cli/caprun/tests/s9_file_write_block.rs` — FS-03 genuine-non-stapled-taint-Block acceptance test

## Security Domain

`security_enforcement` is absent from `.planning/config.json` → enabled by default; this section is required.

### Applicable ASVS categories

| ASVS category | Applies | Standard control |
|---|---|---|
| V1 Architecture | yes | Design-gate-first discipline already satisfied (Phase 31 cleared); this phase is a mechanical, table-entries-only realization, not a new architectural decision |
| V4 Access Control | yes | `KNOWN_SINKS` schema gate + sensitivity/role tables, table-entries-only extension (unchanged `submit_plan_node` enforcement logic, re-verified against live `lib.rs:54-255` this session) |
| V5 Input Validation | yes | `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)` closes path-traversal/symlink-escape by kernel-atomic construction, not string filtering; the per-session `RequestFd` counter closes an unbounded-repeat-call resource-exhaustion gap |
| V6 Cryptography | no | No new key material this phase |
| V8 Data Protection (informative) | yes | `file.write`'s `contents` slot correctly admits (but does not TRUST) untrusted `exec_output`/`doc_fragment`-tagged values — I2's per-arg Block still fires on the untrusted taint regardless of role admissibility |

### Known threat patterns

| Pattern | STRIDE | Mitigation |
|---|---|---|
| Path traversal / symlink escape via a worker-controlled `path` | Tampering / Elevation of Privilege | `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)`, kernel-atomic, reused unmodified from `read_within`/`create_exclusive_within` |
| A tainted `path` silently overwriting an unintended existing file | Tampering | `path` is routing-sensitive (`FILE_WRITE_ROUTING_SENSITIVE`) — a tainted value Blocks before the write ever happens (I2, collect-then-Block) |
| A tainted `contents` value (e.g. exec output, a doc fragment) planting attacker-controlled data into a workspace file | Information Disclosure / Tampering | `contents` is content-sensitive (`FILE_WRITE_CONTENT_SENSITIVE`) — Blocks the same way, exactly mirroring `email.send`'s `body` and `file.create`'s `contents` (HARDEN-05) precedent |
| `write_within` silently creating a new file where none existed (semantic confusion with `file.create`) | Tampering (unintended write authority) | No `O_CREAT` in `write_within` — `ENOENT` fail-closed on a missing target, never a silent create (Pitfall 1) |
| Unbounded repeated `RequestFd` calls exhausting broker resources (fd churn, audit-DAG growth) | Denial of Service | NEW per-session `RequestFd` count limiter (`MAX_REQUEST_FD_PER_SESSION`, Pattern 2) — a resource-exhaustion guard, independent of and additional to I2 |
| A new sink silently escaping the I2 collect-then-Block loop via a hand-rolled fast path | Elevation of Privilege | `file.write` is a table-entries-only extension of the UNMODIFIED `submit_plan_node` (`lib.rs:54-255`, re-verified this session, zero new enforcement logic) — no new code path exists for it to escape through |

## Sources

### Primary (HIGH confidence — direct code reads this session)
- `planning-docs/DESIGN-effect-breadth-exec.md` (full read, all sections + amendments — the binding, cleared design model)
- `planning-docs/DESIGN-GATE-RECORD-v1.7.md` (full read — gate clearance record)
- `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/config.json` (full reads)
- `.planning/phases/32-process-exec-sink-broker-spawned-confined-child/32-RESEARCH.md` (full read — sibling-phase research, reused for Validation Architecture shape and drift-detection)
- `planning-docs/PLAN.md` (full read — canonical build-order/architecture reference)
- `crates/adapter-fs/src/workspace.rs` (full read — `WorkspaceRoot`, `read_within`, `create_exclusive_within`, all existing negative tests)
- `crates/brokerd/src/server.rs` (targeted reads: `evaluate_plan_node_and_record` full body lines 640-1077 including the newly-landed `process.exec` arm; the `RequestFd` arm and its surrounding doc comments, lines 1226-1439; `dispatch_request`'s per-connection state setup, lines 480-520; the production `dispatch_request` call site and `SubmitPlanNode`/`PlanNodeDecision` wiring, lines 596-634, 1541-1576; `ConnectionRole::permits`, lines 80-125)
- `crates/brokerd/src/sinks/file_create.rs` (full read — the two-phase-audit template)
- `crates/brokerd/src/sinks/process_exec.rs` (full read — the most-recently-landed sink module, confirms current conventions)
- `crates/brokerd/src/quarantine.rs` (targeted reads — `mint_from_read`, `mint_from_derivation`, `mint_from_exec` signatures and bodies, confirming `mint_from_exec`'s ACTUAL shape differs slightly from the DESIGN doc's original sketch — it does not build its own Event, it reuses the caller-supplied `spawn_event_id`)
- `crates/executor/src/sink_schema.rs` (full read — `KNOWN_SINKS`, `validate_schema`, all inline tests including the `process.exec` additions)
- `crates/executor/src/sink_sensitivity.rs` (full read — all sensitivity/role tables and match arms including the `process.exec` additions)
- `crates/executor/src/lib.rs` (full read — `submit_plan_node`, confirming the collect-then-Block loop is unmodified and table-driven)
- `crates/runtime-core/src/plan_node.rs` (targeted read — `TaintLabel` enum + `is_untrusted()`, confirming `ExecRaw` already landed and no new variant is needed this phase)
- `crates/brokerd/src/proto.rs` (targeted grep — `PlanNodeDecision` variant shape, confirming `output_value_id` already exists and needs no change for `file.write`)
- `scripts/check-invariants.sh` (targeted grep — Gate 1/Gate 3 exact content, confirming Gate 3 already includes `mint_from_exec(` and needs no further extension)
- `cli/caprun/tests/s9_process_exec_block.rs`, `crates/brokerd/tests/*.rs` file listing, `crates/sandbox/tests/*.rs` file listing, `crates/sandbox/tests/confinement_integration.rs` (targeted reads — test-naming precedent for the Validation Architecture section)

### Secondary (MEDIUM confidence)
- None this phase — every claim traces to a direct code read this session; no WebSearch/Context7 lookup was used or needed (100% internal-codebase grounding, same posture as Phase 32's research).

### Tertiary (LOW confidence, flagged `[ASSUMED]` inline)
- The `RequestFd` counter's exact numeric bound (A2) and the `write_within` method name (A3) — deployment-constant/naming choices the DESIGN doc explicitly defers, not security-model claims.

## Metadata

**Confidence breakdown:**
- Existing-pattern grounding (openat2 resolution, two-phase audit, sink-table extension, `submit_plan_node`'s unmodified collect-then-Block loop): HIGH — every claim traces to a specific file:line read this session, including re-confirming that Phase 32's landing did not change any of the FS-relevant mechanics.
- New-mechanism grounding (`write_within`'s new `OFlag` combination, the `RequestFd` counter): HIGH — both are narrow, same-shape extensions of already-verified primitives; no genuinely novel composition (unlike Phase 32's launcher/`pre_exec`/async-capture work).
- Deployment-constant recommendations (counter bound, sink id, method names): MEDIUM — reasoned defaults with explicit rationale, flagged for a low-stakes human confirm, not presented as settled model decisions.

**Research date:** 2026-07-17
**Valid until:** ~14 days (fast-moving relative to this project's pace; re-verify file:line citations if Phase 33 begins substantially later, per this project's own convention — Phase 32 alone shifted several of the citations this research had to re-verify from Phase 31's original research).
