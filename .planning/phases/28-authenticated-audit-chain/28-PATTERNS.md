# Phase 28: Authenticated Audit Chain - Pattern Map

**Mapped:** 2026-07-12
**Files analyzed:** 6 (modified) + 7 test fixtures (directory-layout migration only)
**Analogs found:** 6 / 6 (all analogs are in-tree, same-file or same-crate — this phase mostly extends existing functions rather than creating new files)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/brokerd/src/audit.rs` — `compute_event_hash` → keyed HMAC | service (crypto util) | transform | itself, `compute_event_hash` (unkeyed, lines 243-259) | exact — in-place signature change |
| `crates/brokerd/src/audit.rs` — `append_event` → thread key + `chain_anchor` upsert | service (CRUD/event-sourcing) | event-driven | itself (lines 274-315); atomicity model from `quarantine.rs::mint_from_read` (two-write same-lock) | exact / role-match |
| `crates/brokerd/src/audit.rs` — new `chain_anchor` table + migration fn | migration | CRUD (schema) | `migrate_pending_confirmations_schema` (lines 120-145) | exact — literal idiom to mirror |
| `crates/brokerd/src/audit.rs` — `verify_chain` → anchor cross-check | service (integrity check) | transform | itself (lines 477-539) | exact — in-place extension |
| `crates/brokerd/src/confirmation.rs` — `pending_confirmations` MAC fold (`insert_pending_confirmation`, `transition_state`, `find_pending_confirmation`) | service (CRUD) | CRUD | `transition_state` (lines 296-306, CAS-via-`UPDATE...WHERE` idiom) | exact |
| `crates/brokerd/src/confirmation.rs` — `confirm()` Step 4.5a key/MAC gate; `deny()` gains same gate (Open Question 1, recommended yes) | controller (decision handler) | request-response | `confirm()`'s existing Step 4.5a `verify_chain` call site (line ~599) | exact for confirm; deny is a role-match copy-forward |
| `cli/caprun/src/main.rs` — new `load_or_create_key(audit_path, workspace_root)` shared helper + F1 refusal | utility / config (startup) | file-I/O | the two existing `open_audit_db` call sites (lines 172-174, 384) | exact — new function, called from both existing sites |
| `crates/brokerd/Cargo.toml` — add `hmac`, `getrandom` deps | config | — | existing `sha2`/`hex` dep entries | exact |
| 7 test fixtures (`s9_live_block.rs`, `e2e.rs`, `live_acceptance_tainted_session.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`, `llm_planner_live_accept.rs`, `origin_seed_provenance.rs`) | test | file-I/O (fixture layout) | `cli/caprun/tests/confirm.rs` lines 221-223 (already F1-safe layout) | exact |

## Pattern Assignments

### `crates/brokerd/src/audit.rs` — keyed `compute_event_hash`

**Analog:** itself, current unkeyed version (`audit.rs:243-259`)

**Current (to replace):**
```rust
pub fn compute_event_hash(
    parent_hash: Option<&str>,
    id: &str,
    session_id: &str,
    event_type: &str,
    payload: &str,
    taint: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parent_hash.unwrap_or(""));
    hasher.update(id);
    hasher.update(session_id);
    hasher.update(event_type);
    hasher.update(payload);
    hasher.update(taint);
    hex::encode(hasher.finalize())
}
```

**Target shape (per RESEARCH Code Examples, verified compiles against `sha2 0.10`):**
```rust
use hmac::{Hmac, Mac};
type HmacSha256 = Hmac<Sha256>;

pub fn compute_event_hash(
    key: &[u8],
    parent_hash: Option<&str>,
    id: &str,
    session_id: &str,
    event_type: &str,
    payload: &str,
    taint: &str,
) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .expect("HMAC can take a key of any length");
    mac.update(parent_hash.unwrap_or("").as_bytes());
    mac.update(id.as_bytes());
    mac.update(session_id.as_bytes());
    mac.update(event_type.as_bytes());
    mac.update(payload.as_bytes());
    mac.update(taint.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
```
Comparison MUST use `Mac::verify_slice` (constant-time), never `==`/`!=` on the hex string — this is a NEW rule relative to the existing unkeyed pattern (Don't Hand-Roll table, RESEARCH).

---

### `crates/brokerd/src/audit.rs` — `append_event` (fold in `chain_anchor` atomicity)

**Analog:** itself (`audit.rs:274-315`), same-lock atomicity model from `quarantine.rs::mint_from_read` (`quarantine.rs:301-`)

**Current core pattern (imports/error handling already present, extend in place):**
```rust
pub fn append_event(
    conn: &rusqlite::Connection,
    event: &Event,
    parent_hash: Option<&str>,
) -> Result<String> {
    if event.event_type == "sink_blocked" && event.anchors.is_empty() {
        return Err(anyhow::anyhow!(
            "sink_blocked event requires at least one anchor (Defect B guard)"
        ));
    }
    let payload = serde_json::to_string(event)?;
    let taint_str = serde_json::to_string(&event.taint)?;
    let hash = compute_event_hash(
        parent_hash, &event.id.to_string(), &event.session_id.to_string(),
        &event.event_type, &payload, &taint_str,
    );
    conn.execute("INSERT INTO events (...) VALUES (...)", rusqlite::params![...])?;
    Ok(hash)
}
```
Extend: add `key: &[u8]` param (threads into `compute_event_hash`), then — under the SAME already-held `conn`/lock — read-then-write (or `RETURNING`) the `chain_anchor` row's `event_count`, recompute its own MAC, and `INSERT ... ON CONFLICT(session_id) DO UPDATE`. Every one of the 19 production call sites (`server.rs:799,923,1046,1180,1263`; `quarantine.rs:371,416,491,781`; `confirmation.rs:550,656,717,776`; `sinks/file_create.rs:94,110,199,216`; `sinks/email_smtp.rs:260,293`) already holds the lock — do NOT add a second call at any of them (Pitfall 4).

**Atomicity precedent to mirror** (`quarantine.rs::mint_from_read` signature, showing the same-lock two-write discipline):
```rust
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_id: Option<Uuid>,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId, Uuid, String)> {
    // Step 1: append_event for file_read (audit row), THEN mint the ValueRecord
    // that references that Event's id — both under the caller's already-held lock.
```

---

### `crates/brokerd/src/audit.rs` — new `chain_anchor` table + migration

**Analog:** `migrate_pending_confirmations_schema` (`audit.rs:120-145`) — copy this EXACT idiom (`PRAGMA table_info` gate before any `ALTER`/`CREATE`, idempotent, re-run-safe on every `open_audit_db` call):

```rust
fn migrate_pending_confirmations_schema(conn: &rusqlite::Connection) -> Result<()> {
    let mut existing_columns: Vec<String> = Vec::new();
    {
        let mut stmt = conn.prepare("PRAGMA table_info(pending_confirmations)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            existing_columns.push(name);
        }
    }
    if !existing_columns.iter().any(|c| c == "blocked_arg_names") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN blocked_arg_names TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }
    if !existing_columns.iter().any(|c| c == "combined_digest") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN combined_digest TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}
```
For `chain_anchor` (a whole NEW table, not a new column): add `CREATE TABLE IF NOT EXISTS chain_anchor (session_id TEXT PRIMARY KEY, head_event_id TEXT NOT NULL, head_hash TEXT NOT NULL, event_count INTEGER NOT NULL, mac TEXT NOT NULL) STRICT;` to `SCHEMA_DDL` (same file, near the existing `sessions`/`events` DDL block, lines ~24-43), PLUS a separate runtime "legacy DB, no anchor row for this session → untrusted until re-anchored" fail-closed check inside `verify_chain` (not a schema migration per se — see below).

---

### `crates/brokerd/src/audit.rs` — `verify_chain` (anchor cross-check)

**Analog:** itself (`audit.rs:477-539`) — recursive-CTE walk from `parent_id IS NULL`, currently only checks `found_any`.

Extend: after the walk, (1) load the `chain_anchor` row for `session_id`, verify ITS MAC with `Mac::verify_slice`; (2) if absent → return `false` (legacy/untrusted, migration pin); (3) assert walk's final `(id, hash)` and total row-count equal the anchor's `head_event_id`/`head_hash`/`event_count`. Existing tamper-simulation tests to re-verify still fail closed post-change: `audit_dag.rs:94` (`tamper_breaks_chain`), `confirmation.rs:1648` (`confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently`) — the closest existing analog to the new Success-Criterion-1 forgery test.

---

### `crates/brokerd/src/confirmation.rs` — `pending_confirmations` MAC fold

**Analog:** `transition_state` (`confirmation.rs:296-306`) — the CAS-via-`UPDATE...WHERE` idiom to extend, not replace:

```rust
pub fn transition_state(
    conn: &rusqlite::Connection,
    effect_id: &str,
    new_state: PendingConfirmationState,
) -> Result<usize> {
    let affected = conn.execute(
        "UPDATE pending_confirmations SET state = ?1 WHERE effect_id = ?2 AND state = 'pending'",
        params![new_state.as_str(), effect_id],
    )?;
    Ok(affected)
}
```
Extend: recompute a whole-row MAC (RESEARCH Assumption A3, recommended) alongside `state` in the same `UPDATE`, under the same lock. `insert_pending_confirmation` is the sole INSERT site — the initial MAC gets computed there. `find_pending_confirmation` must MAC-verify the row IMMEDIATELY after fetch, BEFORE `confirm()`/`deny()`'s Step 2 terminal-state branch reads `pc.state` for a decision (Pitfall 5 — earliest-possible placement).

---

### `crates/brokerd/src/confirmation.rs` — `confirm()`/`deny()` gate placement

**Analog:** `confirm()`'s existing Step 4.5a `verify_chain` call (`confirmation.rs:599`, referenced from RESEARCH anchor table) — the pattern to copy into `deny()` (`confirmation.rs:753-785`, currently has ZERO such check; RESEARCH's Open Question 1 recommends adding the SAME gate per the locked X-02 ruling).

---

### `cli/caprun/src/main.rs` — `load_or_create_key` + F1 refusal (shared helper)

**Analog:** the two existing `open_audit_db` call sites:

**Call site 1** (`main.rs:172-174`, `caprun run` path):
```rust
let conn = Arc::new(Mutex::new(
    open_audit_db(&audit_path).context("open_audit_db")?,
));
```
followed shortly after by `workspace_root_dir` derivation (`main.rs:182-193`):
```rust
let ws_path = Path::new(&workspace_path);
let workspace_root_dir = match ws_path.parent() {
    Some(p) if !p.as_os_str().is_empty() => p,
    _ => Path::new("."),
};
```
**Call site 2** (`main.rs:384`, `run_confirm_or_deny`) — workspace root not known until `find_pending_confirmation` returns `Some(pc)` and `pc.workspace_root_path` is read.

New shared fn (special-case `:memory:`; canonicalize + containment-check against `workspace_root_dir`/`workspace_root_path`; 0600 file read-or-`getrandom::fill`-then-write) must be called from BOTH sites, sequenced per RESEARCH's Architecture Patterns diagram: call site 1 — before `run_broker_server` spawn (reorder key-load after `workspace_root_dir` is derived); call site 2 — after `find_pending_confirmation` succeeds, before Step 4.5a's `verify_chain`.

**F1 canonicalization target** — `WorkspaceRoot::open`/`root_path()` (`crates/adapter-fs/src/workspace.rs:51-73`):
```rust
pub fn open(root: &Path) -> std::io::Result<Self> {
    use nix::fcntl::{open, OFlag};
    use nix::sys::stat::Mode;
    let dirfd = open(root, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
    Ok(Self { dirfd, root_path: root.to_path_buf() })
}

pub fn root_path(&self) -> &Path {
    &self.root_path
}
```
`main.rs` already has `workspace_root_dir: &Path` in scope BEFORE `WorkspaceRoot::open` is even called — a plain `std::fs::canonicalize` comparison on `workspace_root_dir` directly suffices for the refusal check; no need to construct a full `WorkspaceRoot` just for this.

---

### 7 test fixtures — directory-layout migration

**Analog:** `cli/caprun/tests/confirm.rs:221-223` (already F1-safe):
```rust
let workspace = tmp.join("workspace");   // subdirectory
let db_path = tmp.join("audit.db");      // sibling of the subdirectory, NOT of the file
```
Migrate all 7 vulnerable fixtures (`s9_live_block.rs`, `e2e.rs`, `live_acceptance_tainted_session.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`, `llm_planner_live_accept.rs`, `origin_seed_provenance.rs`) from their current `let workspace_file = tmp.join("workspace.txt"); let audit_db = tmp.join("audit.db");` (siblings — vulnerable) to this pattern. Sequence FIRST, before F1 refusal logic lands (Pitfall 1).

---

## Shared Patterns

### Idempotent schema migration
**Source:** `crates/brokerd/src/audit.rs:120-145` (`migrate_pending_confirmations_schema`)
**Apply to:** `chain_anchor` table creation/legacy-DB detection.
`PRAGMA table_info`/`sqlite_master` presence check before any `ALTER`/`CREATE` — never a blind statement. Runs on every `open_audit_db` call, must be safe to re-run.

### Same-lock atomicity (no SQL transaction needed except the one existing `conn.transaction()?` at `confirmation.rs:701`)
**Source:** `crates/brokerd/src/quarantine.rs::mint_from_read`
**Apply to:** `append_event`'s internalized `chain_anchor` upsert — every one of the 19 call sites already holds the lock, so folding the anchor write into `append_event` gets atomicity "for free."

### CAS via `UPDATE ... WHERE <guard>` (rows_affected as the check)
**Source:** `crates/brokerd/src/confirmation.rs:296-306` (`transition_state`)
**Apply to:** `pending_confirmations` MAC-fold `UPDATE`; NOT applicable to the new Phase-29 `sent_plan_nodes` table (that uses INSERT-PK-violation-as-CAS instead — different idiom, noted for the planner's awareness only, out of Phase 28 scope).

### Constant-time MAC comparison
**Source:** `hmac::Mac::verify_slice` (RESEARCH Don't-Hand-Roll table)
**Apply to:** every new MAC check in `verify_chain`, `pending_confirmations` fetch, `chain_anchor` verification. Never port the existing unkeyed-hash `!=` comparison pattern forward — that is a NEW timing-side-channel bug class once a secret-dependent MAC is involved.

## No Analog Found

None — every file/function this phase touches already exists in the codebase and is being extended in place; no wholly new module has no precedent. The one genuinely new construct (`chain_anchor` table + `load_or_create_key` helper) has direct idiomatic analogs listed above.

## Metadata

**Analog search scope:** `crates/brokerd/src/{audit.rs,confirmation.rs,server.rs,quarantine.rs,sinks/}`, `crates/adapter-fs/src/workspace.rs`, `cli/caprun/src/main.rs`, `cli/caprun/tests/*.rs`
**Files scanned:** 6 primary source files + 8 test fixtures (full reads, per RESEARCH's Sources section)
**Pattern extraction date:** 2026-07-12
