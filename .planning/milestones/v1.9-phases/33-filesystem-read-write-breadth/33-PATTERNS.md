# Phase 33: Filesystem Read/Write Breadth - Pattern Map

**Mapped:** 2026-07-17
**Files analyzed:** 8 (new/modified)
**Analogs found:** 8 / 8 (all have a strong same-file or same-role analog; two items — the `ENOENT` test and the `RequestFd` counter — have a partial/structural analog only, called out below)

All line numbers below were re-verified against the live tree this session (research's citations drifted slightly — e.g. `server.rs`'s `file.create` arm is now at 877-897, not 868-897 — confirming CLAUDE.md's standing warning that Phase 32 shifted several shared files since the design gate).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/adapter-fs/src/workspace.rs::write_within` | utility (fs primitive) | file-I/O | `WorkspaceRoot::create_exclusive_within` (same file, lines 133-167) | exact (sibling method, same struct, same syscall API) |
| `crates/adapter-fs/src/workspace.rs` new tests | test | file-I/O | `create_exclusive_*` test set (lines 294-401) + `absolute_path_rejected`/`parent_traversal_rejected`/`symlink_escape_rejected` (lines 223-288) | exact for the 3 negative-path tests; **no analog** for the new `ENOENT`-on-missing-target test |
| `crates/brokerd/src/sinks/file_write.rs::invoke_file_write` | service (sink invocation) | CRUD (write) | `crates/brokerd/src/sinks/file_create.rs::invoke_file_create` (lines 65-116) | exact (two-phase audit, same signature shape) |
| `crates/brokerd/src/sinks.rs` (`pub mod file_write;`) | config/module-decl | — | `pub mod process_exec;` (added Phase 32) | exact |
| `crates/brokerd/src/server.rs` — new `"file.write"` Allowed-dispatch arm | controller (dispatch) | request-response | `"file.create"` arm, `server.rs:877-897` | exact |
| `crates/brokerd/src/server.rs` — `RequestFd` per-session counter | middleware (resource guard) | request-response | `fd_requested`/`intent_provided` per-connection bool state (`server.rs:496-497`, mutated at `:1279`, threaded through `dispatch_request:1243-1256`) | structural only — no existing *counter*, only a boolean-flag precedent for the threading pattern |
| `crates/executor/src/sink_schema.rs` `KNOWN_SINKS` entry | config (schema table) | request-response | `"file.create"` entry (`sink_schema.rs:53-57`) | exact |
| `crates/executor/src/sink_sensitivity.rs` new consts + 4 match arms | config (policy table) | request-response | `"file.create"` arms + `FILE_CREATE_*` consts (`sink_sensitivity.rs:43,64-67,81-86,113,130,182-196`) | exact |
| `cli/caprun/tests/s9_file_write_block.rs` | test (integration) | request-response | `cli/caprun/tests/s9_process_exec_block.rs` (structure only — no spawn/confinement machinery needed) | role-match, simpler (fs write has no exec-child analog) |

## Pattern Assignments

### `crates/adapter-fs/src/workspace.rs::write_within` (utility, file-I/O)

**Analog:** `create_exclusive_within`, same file, lines 133-167 (Linux impl 133-151, non-Linux stub 156-167).

**Core pattern — Linux impl** (mirror lines 133-151, change ONLY the `OFlag`):
```rust
pub fn create_exclusive_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
    use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
    use std::io::Write;
    use std::os::fd::AsFd;

    let how = OpenHow::new()
        .flags(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY)
        .mode(nix::sys::stat::Mode::from_bits_truncate(0o600))
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);

    let fd = openat2(self.dirfd.as_fd(), rel_path, how)
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

    let mut file = std::fs::File::from(fd);
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
}
```
`write_within` uses the SAME structure, `OFlag::O_WRONLY | OFlag::O_TRUNC` (no `O_CREAT`, no `.mode(...)` call — mode only matters with `O_CREAT`), same `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`. Missing target surfaces as `ENOENT` through the same `.map_err(std::io::Error::from_raw_os_error)` path — no new error handling needed.

**Non-Linux stub pattern** (mirror lines 156-167): use `std::fs::OpenOptions::new().write(true).truncate(true).open(...)` — deliberately omit `.create(true)`/`.create_new(true)` so a missing target still errors on macOS too.

**Test pattern** — mirror one negative test verbatim, e.g. `create_exclusive_absolute_path_rejected` (lines 334-345-ish) for the `write_within` absolute-path case, and the `unique_tmp_root` harness (line 192) for setup. **No existing test asserts `ENOENT` on a missing target for ANY `openat2` call in this file** — this is a genuinely new test with no analog; write it against `write_within`'s own doc-comment contract (`ENOENT` if `rel_path` does not exist).

### `crates/brokerd/src/sinks/file_write.rs::invoke_file_write` (service, CRUD-write)

**Analog:** `crates/brokerd/src/sinks/file_create.rs::invoke_file_create`, lines 65-116 (full function verified this session):
```rust
pub fn invoke_file_create(
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

    match workspace_root.create_exclusive_within(&path, contents.as_bytes()) {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(), Some(parent_id), session_id,
                format!("sink:file.create:{effect_id}"),
                "sink_executed".into(), Utc::now(),
                vec![], // the executed effect carries no taint (path was UserTrusted)
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_executed")?;
            Ok((event.id, hash))
        }
        Err(e) => {
            let event = Event::new(
                Uuid::new_v4(), Some(parent_id), session_id,
                format!("sink:file.create:{effect_id}"),
                "sink_execution_failed".into(), Utc::now(), vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_execution_failed")?;
            Err(anyhow::Error::new(e).context("file.create create_exclusive_within failed"))
        }
    }
}
```
Copy verbatim, renaming `invoke_file_create`→`invoke_file_write`, the actor literal `"sink:file.create:{effect_id}"`→`"sink:file.write:{effect_id}"`, the context strings, and swapping `workspace_root.create_exclusive_within(...)`→`workspace_root.write_within(...)`. Also copy the file's own private `resolve_arg` helper (`file_create.rs:119` on) — do NOT import a shared helper; `file_create.rs` and `process_exec.rs` each keep their own private copy, matching this codebase's per-module-helper convention.

**Imports pattern** (mirror `file_create.rs:29-38`, drop the `ResolvedArg`/`confirmation` import which is only used by `file_create.rs`'s separate `invoke_file_create_from_resolved` variant — `file_write.rs` needs only the base variant):
```rust
use anyhow::{Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode};
use uuid::Uuid;

use adapter_fs::workspace::WorkspaceRoot;
use crate::audit::append_event;
```

**Module registration:** `crates/brokerd/src/sinks.rs` — add `pub mod file_write;` beside the existing `pub mod process_exec;` line.

### `crates/brokerd/src/server.rs` — new `"file.write"` Allowed-dispatch arm (controller, request-response)

**Analog:** the `"file.create"` arm, `server.rs:877-897` (verified this session; research's citation of 868-897 has drifted +9 lines — Gate-3/comment additions elsewhere in the file, not a structural change):
```rust
if matches!(decision, runtime_core::ExecutorDecision::Allowed)
    && plan_node.sink.0 == "file.create"
{
    let (sink_event_id, sink_hash) = {
        let locked = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        crate::sinks::file_create::invoke_file_create(
            &locked, key, value_store, session_id, effect_id,
            plan_node, workspace_root, *last_event_id, last_event_hash,
        )?
    };
    *last_event_id = sink_event_id;
    *last_event_hash = sink_hash;
}
```
Add a FOURTH sibling arm (after `file.create`/`email.send`/`process.exec`, which are at `877`, `~908`, `~1049` per the grep this session — exact current numbers: `877`, `908`, `1049`) gated on `plan_node.sink.0 == "file.write"`, calling `crate::sinks::file_write::invoke_file_write(...)` with the identical parameter list. **No `output_value_id` involvement** — leave it untouched (stays `None`), exactly as `file.create`'s arm does today (unlike the `process.exec` arm, which DOES set it).

### `crates/brokerd/src/server.rs` — `RequestFd` per-session counter (middleware, request-response)

**Analog (structural only, no counter precedent exists):** the `fd_requested`/`intent_provided` per-connection `bool` state:
- Declared: `server.rs:496-497` — `let mut intent_provided = false;` / `let mut fd_requested = false;`
- Threaded into `dispatch_request`'s signature: `server.rs:1255-1256` (`intent_provided: &mut bool, fd_requested: &mut bool`)
- Mutated at entry of the `RequestFd` arm: `server.rs:1279` — `*fd_requested = true;` (set FIRST, before any other work in the arm — mirror this "set-at-entry" discipline for the new counter so a worker cannot dodge it via early failure)
- Production call site passing both: `server.rs:628-629`
- 5 existing inline test call sites needing a new `&mut 0u32` parameter added: `server.rs:1953, 1992, 2014, 2075, 2106` (re-verified this session — matches research's citation exactly)

Add `const MAX_REQUEST_FD_PER_SESSION: u32 = 256;` as a module-level const, a new `fd_request_count: &mut u32` parameter threaded exactly like `fd_requested`, incremented at the top of the `RequestFd` arm (immediately after `*fd_requested = true;` at line 1279), with an over-limit path that sends `BrokerResponse::Error` and `return Ok(())` (NOT `break`) — mirror the existing error-then-continue shape used elsewhere in `dispatch_request` (e.g. the `intent_provided || fd_requested` reject-path around line 1688-1693) rather than terminating the connection.

### `crates/executor/src/sink_schema.rs` `KNOWN_SINKS` entry (config, request-response)

**Analog:** `sink_schema.rs:53-57`:
```rust
SinkSchema {
    sink: "file.create",
    allowed: &["path", "contents"],
    required: &["path", "contents"],
},
```
New row, add after the `file.create` entry:
```rust
SinkSchema {
    sink: "file.write",
    allowed: &["path", "contents"],
    required: &["path", "contents"],
},
```

### `crates/executor/src/sink_sensitivity.rs` (config, request-response)

**Analog — 4 arms + 2 consts**, all verified this session:

`EffectClass` arm (`sink_sensitivity.rs:43`):
```rust
"file.create" => EffectClass::CommitIrreversible,
```
→ add `"file.write" => EffectClass::CommitIrreversible,`

Consts (`sink_sensitivity.rs:67`, `:86`):
```rust
pub const FILE_CREATE_ROUTING_SENSITIVE: &[&str] = &["path"];
pub const FILE_CREATE_CONTENT_SENSITIVE: &[&str] = &["contents"];
```
→ add `FILE_WRITE_ROUTING_SENSITIVE: &[&str] = &["path"];` and `FILE_WRITE_CONTENT_SENSITIVE: &[&str] = &["contents"];`

`is_routing_sensitive`/`is_content_sensitive` arms (`:113`, `:130`):
```rust
"file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
"file.create" => FILE_CREATE_CONTENT_SENSITIVE.contains(&arg_name),
```
→ add matching `"file.write" =>` arms referencing the new consts.

`expected_role` arm (`sink_sensitivity.rs:182-196`, verbatim, re-verified this session — confirms research's Code Example matches live code exactly for `file.create`):
```rust
"file.create" => match arg_name {
    "path" => Some(&["path", "relative_path"]),
    // HARDEN-05 (v1.6): `contents` is role-checked to `Some(&["path"])`
    // ...
    "contents" => Some(&["path"]),
    _ => None,
},
```
**New `"file.write"` arm must NOT simply copy `file.create`'s `contents` role list.** Per DESIGN §4.3 (cited in RESEARCH.md A4), `file.write`'s `contents` needs a WIDER role set than `file.create`'s: `Some(&["path", "exec_output", "doc_fragment"])` — admitting Phase 32's `exec_output` role (chained `process.exec` → `file.write`) and the `email.send`-precedent `doc_fragment` role, so a tainted value routed here reaches I2's content-sensitivity Block instead of a structural Step-1c Deny. `path`'s role stays `Some(&["path", "relative_path"])`, identical to `file.create`.

### `cli/caprun/tests/s9_file_write_block.rs` (test, request-response)

**Analog:** `cli/caprun/tests/s9_process_exec_block.rs` — mirror the structure (mint a tainted value into `file.write`'s `path` or `contents` slot, call `executor::submit_plan_node`, assert `BlockedPendingConfirmation` with an unbroken `provenance_chain[0]` anchor) but DROP all spawn/confinement setup — `file.write` is a single in-broker `openat2` call, not a child process, so none of `s9_process_exec_block.rs`'s launcher/pre_exec machinery applies.

## Shared Patterns

### Two-phase durable audit (sink_executed / sink_execution_failed)
**Source:** `crates/brokerd/src/sinks/file_create.rs:87-116`
**Apply to:** `file_write.rs::invoke_file_write` — reuse the literal event-type strings `"sink_executed"`/`"sink_execution_failed"` (per RESEARCH.md's Open-Questions recommendation — the `actor` field, e.g. `"sink:file.write:{effect_id}"`, already disambiguates which sink produced the event; do not invent new event-type strings).

### `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)` kernel-atomic path resolution
**Source:** `crates/adapter-fs/src/workspace.rs:90-102` (`read_within`) and `:133-151` (`create_exclusive_within`)
**Apply to:** `write_within` — identical resolve-flag combination, only the `OFlag` open-mode set differs (`O_WRONLY|O_TRUNC`, no `O_CREAT`).

### Table-driven, no-new-code-path sink authorization
**Source:** `crates/executor/src/lib.rs::submit_plan_node` (unmodified, per RESEARCH.md drift-note #3 and #2 — no new `TaintLabel` variant, no new Gate-3 line)
**Apply to:** every `file.write` schema/sensitivity table addition — the collect-then-Block enforcement loop itself needs ZERO changes; `file.write` is purely new table rows.

## No Analog Found

| File/Item | Role | Data Flow | Reason |
|---|---|---|---|
| `workspace.rs` `ENOENT`-on-missing-target test | test | file-I/O | Neither the `O_RDONLY` read-path tests (lines 223-288) nor the `O_CREAT\|O_EXCL` create tests (294-401) assert `ENOENT` on a missing target — write against `write_within`'s own doc-comment contract, no code to mirror. |
| `RequestFd` per-session numeric counter | middleware (resource guard) | request-response | Only a boolean-flag precedent (`fd_requested`/`intent_provided`) exists; there is no existing bounded-counter/rate-limiter pattern anywhere in `brokerd`. Use the plain hardcoded `const` + `u32` shape described above — do not introduce a new abstraction (RESEARCH.md "Don't Hand-Roll" table explicitly rejects a rate-limiter crate). |

## Metadata

**Analog search scope:** `crates/adapter-fs/src/workspace.rs`, `crates/brokerd/src/{server.rs,sinks.rs,sinks/file_create.rs,sinks/process_exec.rs}`, `crates/executor/src/{sink_schema.rs,sink_sensitivity.rs,lib.rs}`, `cli/caprun/tests/s9_process_exec_block.rs`
**Files scanned:** 8 direct reads/greps this session, all line numbers re-verified against live tree (superseding 33-RESEARCH.md's citations where drifted, e.g. `server.rs` `file.create` arm 877-897 not 868-897, arms at 877/908/1049)
**Pattern extraction date:** 2026-07-17
**Source of truth for the model itself:** `planning-docs/DESIGN-effect-breadth-exec.md` §3/§4/§5 — not re-derived here, only mapped to concrete code excerpts.

## PATTERN MAPPING COMPLETE
