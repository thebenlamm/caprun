# Phase 10: Single-Shot Confirmation Loop - Pattern Map

**Mapped:** 2026-07-07
**Files analyzed:** 6 (from RESEARCH.md Wave 0 Gaps)
**Analogs found:** 6 / 6

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|--------------------|------|-----------|-----------------|----------------|
| `crates/brokerd/src/confirmation.rs` (NEW) | service (TCB decision logic) | CRUD (side-table state machine) | `crates/brokerd/src/audit.rs` (`insert_blocked_literal`/`get_blocked_literal`/`redact_blocked_literal`) | role-match |
| `crates/brokerd/src/audit.rs` (MODIFIED — add DDL + fns) | model/migration (schema) | CRUD | same file, `blocked_literals` DDL block + accessor fns | exact |
| `crates/brokerd/src/sinks/file_create.rs` (MODIFIED — add `invoke_file_create_from_resolved`) | service (sink adapter) | file-I/O / request-response | same file's existing `invoke_file_create` | exact |
| `cli/caprun/src/main.rs` (MODIFIED — add confirm/deny dispatch) | controller (CLI entrypoint) | request-response | same file's `--seed-from-file` first-arg branch pattern | exact |
| `cli/caprun/tests/confirm.rs` (NEW) | test (integration, cross-process) | request-response | `cli/caprun/tests/e2e.rs` | exact |
| `cli/caprun/Cargo.toml` (MODIFIED — add `[[test]]` target) | config | — | same file's existing `e2e`/`planner` test-target entries | exact |

## Pattern Assignments

### `crates/brokerd/src/confirmation.rs` (NEW — service, CRUD side-table + state machine)

**Analog:** `crates/brokerd/src/audit.rs` (side-table accessor functions) + `crates/brokerd/src/server.rs`'s `SubmitPlanNode` arm (decision/state pattern)

**DDL pattern to mirror** (`crates/brokerd/src/audit.rs` lines 52-56, `blocked_literals`):
```rust
CREATE TABLE IF NOT EXISTS blocked_literals (
    event_id TEXT PRIMARY KEY,
    literal  TEXT NOT NULL
) STRICT;
```
Add an analogous `pending_confirmations` table (RESEARCH.md lines 438-445) with `effect_id TEXT PRIMARY KEY`, `session_id`, `sink`, `resolved_args` (JSON TEXT blob — mirrors `events.payload`'s "serialize the whole struct" convention, never a normalized child table per Open Question 2), `state TEXT` ("pending"|"confirmed"|"denied"), and (per Open Question 1) `workspace_root_path TEXT NOT NULL`.

**Accessor-function shape to mirror** (`crates/brokerd/src/audit.rs` lines 82-118, `insert_blocked_literal`/`get_blocked_literal`/`redact_blocked_literal`):
```rust
pub fn insert_blocked_literal(
    conn: &rusqlite::Connection,
    event_id: &str,
    literal: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO blocked_literals (event_id, literal) VALUES (?1, ?2)",
        rusqlite::params![event_id, literal],
    )?;
    Ok(())
}
```
Write `insert_pending_confirmation`, `find_pending_confirmation` (indexed `SELECT ... WHERE effect_id = ?1`, same shape as `get_blocked_literal`), and `transition_state` (an `UPDATE ... SET state = ?1 WHERE effect_id = ?2 AND state = 'pending'` — the `AND state = 'pending'` guard is the CONFIRM-03 fail-closed check baked into the SQL itself, not a separate read-then-write race).

**Terminal-state check discipline (CONFIRM-03):** every call MUST `find_pending_confirmation` fresh from `conn` — never cache. No analog needed here; this is an anti-pattern warning (RESEARCH Pitfall 5), not a pattern to copy.

**Never re-run `submit_plan_node`:** `confirm`/`deny` in this module call directly into the sink adapter (Pattern 3 below), never into `executor::submit_plan_node`. Verified no such re-entrant call exists in `crates/executor` or `crates/brokerd` today.

---

### `crates/brokerd/src/audit.rs` (MODIFIED — extend `SCHEMA_DDL`)

**Analog:** same file, existing `blocked_literals` DDL block (lines 45-56) and `SCHEMA_DDL` const (lines 25-56).

**Core pattern:** append the new `CREATE TABLE IF NOT EXISTS pending_confirmations (...) STRICT;` block to the `SCHEMA_DDL` string constant — same `STRICT` typed-column discipline, same `IF NOT EXISTS` idempotency, no migration machinery (this project has none; schema additions are additive DDL only, verified — no migration framework anywhere in `crates/brokerd`).

---

### `crates/brokerd/src/sinks/file_create.rs` (MODIFIED — add `invoke_file_create_from_resolved`)

**Analog:** same file's `invoke_file_create` (lines 64-114) — copy the two-phase durable-audit shape, change only the arg-resolution source.

**Imports pattern** (lines 29-37):
```rust
use anyhow::{Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode};
use uuid::Uuid;
use adapter_fs::workspace::WorkspaceRoot;
use crate::audit::append_event;
```
For the new function, drop `ValueStore`/`PlanNode` imports and add whatever `ResolvedArg` type is defined in `confirmation.rs`.

**Core pattern — two-phase durable audit** (lines 82-114, verbatim structure to reuse):
```rust
match workspace_root.create_exclusive_within(&path, contents.as_bytes()) {
    Ok(()) => {
        let event = Event::new(
            Uuid::new_v4(), Some(parent_id), session_id,
            format!("sink:file.create:{effect_id}"),
            "sink_executed".into(), Utc::now(), vec![],
        );
        let hash = append_event(conn, &event, Some(parent_hash))
            .context("append sink_executed")?;
        Ok((event.id, hash))
    }
    Err(e) => {
        let event = Event::new(
            Uuid::new_v4(), Some(parent_id), session_id,
            format!("sink:file.create:{effect_id}"),
            "sink_execution_failed".into(), Utc::now(), vec![],
        );
        append_event(conn, &event, Some(parent_hash))
            .context("append sink_execution_failed")?;
        Err(anyhow::Error::new(e).context("file.create create_exclusive_within failed"))
    }
}
```
**Change vs. analog:** replace `resolve_arg(value_store, plan_node, "path")` (lines 77-78, which calls `store.resolve(&arg.value_id)` at line 124) with a direct lookup into `resolved_args: &[ResolvedArg]` (frozen literals, no `ValueId`/`ValueStore` involved — RESEARCH Pitfall 1). Signature shape given in RESEARCH.md lines 255-270.

**Actor-field convention to reuse exactly** (`format!("sink:file.create:{effect_id}")`, line 88/104): the same convention, with `confirm:{effect_id}` / `deny:{effect_id}` actor strings, is reused in `confirmation.rs`'s `confirm_granted`/`confirm_denied` events (RESEARCH.md lines 232-241).

**Error handling:** identical `Context`/`anyhow::Error::new(...).context(...)` pattern — no new error type introduced.

---

### `cli/caprun/src/main.rs` (MODIFIED — add `confirm`/`deny` first-arg branch)

**Analog:** same file's existing `--seed-from-file` flag pre-parse (lines 45-67) — the precedent for special-casing the first argument(s) BEFORE the general intent-kind parse.

**Pattern to mirror** (lines 45-67):
```rust
let raw_args: Vec<String> = std::env::args().skip(1).collect();
let mut idx = 0usize;

let seed_from_file_path: Option<String> =
    if raw_args.get(idx).map(String::as_str) == Some("--seed-from-file") {
        idx += 1;
        let path = raw_args.get(idx).cloned().ok_or_else(|| {
            anyhow::anyhow!("usage: ...")
        })?;
        idx += 1;
        Some(path)
    } else {
        None
    };
```
**Required change (RESEARCH Pitfall 6):** the `confirm`/`deny` check MUST be the very FIRST branch in `main()`, checked even before this `--seed-from-file` check, since `confirm <effect_id> [audit-db-path]` has a completely different arg shape than `<intent-kind> <intent-param> <workspace-file> [audit-db-path]`. Add:
```rust
if raw_args.first().map(String::as_str) == Some("confirm")
    || raw_args.first().map(String::as_str) == Some("deny")
{
    // dispatch to a new fn, e.g. run_confirm_or_deny(&raw_args), return its exit code
}
```

**DB-open pattern to reuse** (lines 110-113):
```rust
let conn = Arc::new(Mutex::new(
    open_audit_db(&audit_path).context("open_audit_db")?,
));
```
The confirm/deny dispatch function opens the SAME persistent DB file this way (never `:memory:` per RESEARCH.md's architecture diagram) — `brokerd::audit::open_audit_db` import already present at line 34.

**Error handling:** `anyhow::Context`/`anyhow::bail!` throughout — same idiom for "unknown effect_id" / "already terminal" fail-closed exits (CONFIRM-03), matching line 107's `anyhow::bail!("unknown intent kind: {intent_kind}")`.

---

### `cli/caprun/tests/confirm.rs` (NEW — integration test, cross-process)

**Analog:** `cli/caprun/tests/e2e.rs` (whole file) — the template for driving the compiled binary as a real subprocess, required because CONFIRM-03's cross-process durability claim cannot be honestly tested within one process (RESEARCH Pitfall 5).

**Spawn-the-real-binary pattern** (lines 66-74):
```rust
let caprun_bin = env!("CARGO_BIN_EXE_caprun");
let output = std::process::Command::new(caprun_bin)
    .arg("send-email-summary")
    .arg("demo@example.test")
    .arg(workspace_file.to_str().unwrap())
    .arg(audit_db_path.to_str().unwrap())
    .output()
    .expect("spawn caprun");
```
For Phase 10: spawn `caprun <intent-kind> ...` first to produce a genuine Block + `pending_confirmations` row against a **persistent** `audit_db_path` (never `:memory:` — lines 58-59 pattern of `tmp.join("audit.db")`), then spawn `caprun confirm <effect_id> <audit_db_path>` (or `deny`) as a SEPARATE `Command::new(caprun_bin)` invocation reusing the same `audit_db_path`, asserting exit code per RESEARCH.md's exit-code table. Repeat the confirm invocation a second time to assert CONFIRM-03 (already-terminal refusal).

**Setup pattern to mirror** (lines 54-59, temp dir + persistent audit DB, NOT `:memory:`):
```rust
let run_id = uuid::Uuid::new_v4();
let tmp = std::env::temp_dir().join(format!("caprun_e2e_{run_id}"));
std::fs::create_dir_all(&tmp).expect("create tmp dir");
let workspace_file = tmp.join("workspace.txt");
let audit_db_path = tmp.join("audit.db");
```

**Assertion pattern via `open_audit_db` + raw SQL** (lines 94-114): open the same DB after both subprocess runs and query `pending_confirmations`/`events` directly to assert `confirm_granted`/`confirm_denied` events and `parent_id` linkage (CONFIRM-04).

**Cleanup pattern** (line 117): `std::fs::remove_dir_all(&tmp).ok();`

**Note:** unlike `e2e.rs`'s two tests (`#[cfg(target_os = "linux")]`-gated because they exercise Landlock/seccomp confinement), the confirm/deny flow itself does not require kernel confinement to test meaningfully on macOS — `adapter_fs::workspace::WorkspaceRoot`'s non-Linux stub (`std::fs::File::options().create_new(true)`) makes `confirm.rs` macOS-runnable per RESEARCH.md's Environment Availability table. Do not blanket-gate this file behind `#[cfg(target_os = "linux")]`; only gate the pieces that specifically need real Landlock/seccomp enforcement (likely none, for this phase).

---

### `cli/caprun/Cargo.toml` (MODIFIED — add `[[test]]` target)

**Analog:** same file's existing `e2e`/`planner`/`s9_live_block`/`origin_seed_provenance` `[[test]]` entries.

**Pattern to mirror:** read the existing entries and add a parallel block, e.g.:
```toml
[[test]]
name = "confirm"
path = "tests/confirm.rs"
```
(Exact existing entry syntax should be copied verbatim from the file rather than assumed — confirm field names match neighboring entries before finalizing the plan.)

## Shared Patterns

### Effect-id-in-actor convention (not a new Event column)
**Source:** `crates/brokerd/src/sinks/file_create.rs` lines 88, 104 — `format!("sink:file.create:{effect_id}")`
**Apply to:** `confirmation.rs`'s `confirm_granted`/`confirm_denied` `Event` constructors — `format!("confirm:{effect_id}")` / `format!("deny:{effect_id}")`. This is the ONLY way `effect_id` reaches an `Event` row; `Event` gets no new column (golden byte-fixture test in `crates/runtime-core/src/event.rs` must not change).

### Broker-owned Mutex-locked connection scope for atomic side-table + event writes
**Source:** `crates/brokerd/src/server.rs` lines 444-468 (the `SubmitPlanNode` arm's lock scope)
```rust
let new_hash = {
    let locked = conn.lock().map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
    let hash = append_event(&locked, &audit_event, Some(last_event_hash))?;
    if let Some(literal) = &blocked_literal {
        crate::audit::insert_blocked_literal(&locked, &audit_event.id.to_string(), literal)?;
    }
    hash
};
```
**Apply to:** every write site in `confirmation.rs` that pairs an `Event` append with a `pending_confirmations` write (insert at Block time; state transition at confirm/deny time) — same lock, same "everything durable before responding/exiting" discipline. Note per RESEARCH Pitfall 3: state-transition-then-sink-invocation is explicitly NOT one atomic unit (the sink syscall can't join a SQL transaction) — persist `Confirmed` state first, in its own lock scope, THEN invoke the sink separately.

### Sink dispatch by `sink.0` string match
**Source:** `crates/brokerd/src/server.rs` lines 480-482 — `if matches!(decision, ExecutorDecision::Allowed) && plan_node.sink.0 == "file.create"`
**Apply to:** confirm's sink-dispatch logic (Open Question 3) — mirror this same `if`/`match` shape by `sink.0`/stored `sink` column value, with `file.create` calling `invoke_file_create_from_resolved` and `email.send` doing its existing no-op stub-event append equivalent.

### Fail-closed anyhow::Context/bail idiom
**Source:** `cli/caprun/src/main.rs` throughout (e.g. line 107 `anyhow::bail!("unknown intent kind: {intent_kind}")`, line 112 `.context("open_audit_db")`)
**Apply to:** all new confirm/deny error paths (malformed `effect_id` UUID, unknown `effect_id`, already-terminal state) — same `anyhow` idiom, no new error-handling machinery.

## No Analog Found

None — every file in Wave 0's gap list has a direct, verified analog in the existing codebase (this phase is disciplined reuse of proven patterns per RESEARCH.md's own framing, not new-mechanism invention).

## Metadata

**Analog search scope:** `crates/brokerd/src/{audit.rs,server.rs,sinks/file_create.rs}`, `cli/caprun/{src/main.rs,tests/e2e.rs,Cargo.toml}`
**Files scanned:** 6 (all read directly this session, no Glob/Grep-only inference)
**Pattern extraction date:** 2026-07-07
