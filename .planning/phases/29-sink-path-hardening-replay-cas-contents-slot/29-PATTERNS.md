# Phase 29: Sink-Path Hardening — Replay CAS & contents Slot - Pattern Map

**Mapped:** 2026-07-17
**Files analyzed:** 3 modified (server.rs, audit.rs, sink_sensitivity.rs), 1 new-or-extended test file
**Analogs found:** 4 / 4 (all in-repo, same-crate precedents — no external patterns needed)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/brokerd/src/server.rs` (Allowed `email.send` block, `:901-977`, re-verified — DESIGN doc's `:792` anchor has shifted ~110 lines since Phase 28) | controller/dispatch (reference-monitor sink dispatch) | request-response + side-effecting (SMTP) | `crates/brokerd/src/confirmation.rs` SEND-01 block (`:868-896`) | exact — same crate, same sink, same "CAS-then-append-then-commit-then-SMTP" shape, only the key derivation differs |
| `crates/brokerd/src/audit.rs` (new `sent_plan_nodes` table DDL + `migrate_sent_plan_nodes_schema` fn + `open_audit_db` wiring) | migration/schema | CRUD (schema-only) | `migrate_chain_anchor_schema` (`:280-304`) + its `SCHEMA_DDL` `chain_anchor` table (`:194` region) | exact — whole-new-table case, identical shape needed |
| new idempotency-key fn (likely `crates/brokerd/src/server.rs` or a small helper module) | utility (pure hash derivation) | transform | `combined_digest` (`crates/brokerd/src/confirmation.rs:79-109`) | role-match — same sort-then-hash discipline, different domain (`value_id` pairs, not literal pairs) |
| `crates/executor/src/sink_sensitivity.rs` (`is_content_sensitive` `:102-107`, `expected_role` `:147-162` incl. `file.create` arm at `:157-160`, test `:313-317`) | validation/policy table (I2 TCB) | transform (pure classification) | itself — the `email.send` arms in the SAME functions are the mirror (`EMAIL_SEND_CONTENT_SENSITIVE`, `Some(&["body","doc_fragment"])` shape) | exact — same file, same function, sibling sink arm |
| `crates/brokerd/tests/` new-or-extended file (e.g. `replay_cas.rs` or extend `email_smtp_acceptance.rs`) | test (integration, Linux-only, real SMTP) | event-driven / request-response | `crates/brokerd/tests/email_smtp_acceptance.rs` (drives `confirm()` directly against Mailpit) | role-match — same Mailpit-backed harness shape; open question (research #2) on placement |

## Pattern Assignments

### `crates/brokerd/src/server.rs` — Allowed `email.send` CAS (controller/dispatch)

**Analog:** `crates/brokerd/src/confirmation.rs:868-896` (SEND-01)

**Current code to be edited** (`server.rs:901-977`, exact current text):
```rust
if matches!(decision, runtime_core::ExecutorDecision::Allowed)
    && plan_node.sink.0 == "email.send"
{
    // ... resolve args into Vec<ResolvedArg> ...
    let locked = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
    // MAJOR-4: append durable email_send_attempted event ...
    let attempted_event = Event::new(/* ... */);
    let attempted_hash = append_event(&locked, key, &attempted_event, Some(last_event_hash))?;
    *last_event_id = attempted_event_id;
    *last_event_hash = attempted_hash.clone();
    // REPLAY RESIDUAL RISK comment — THIS is what HARDEN-03 closes,
    // remove/update this comment once the CAS lands.
    let (sink_event_id, sink_hash) =
        crate::sinks::email_smtp::invoke_email_smtp_from_resolved(/* &locked, ... */)?;
    *last_event_id = sink_event_id;
    *last_event_hash = sink_hash;
}
```

**Mirror-target CAS ordering** (`confirmation.rs:868-896`, copy this discipline verbatim, NOT the key):
```rust
"email.send" => {
    // SEND-01: the CAS (`pending -> confirmed`) and the durable
    // `email_send_attempted` append MUST commit in ONE atomic SQLite
    // transaction, BEFORE any SMTP connection is opened.
    let tx = conn.transaction()?;
    let affected = transition_state(&tx, key, &pc, PendingConfirmationState::Confirmed)?;
    if affected == 0 {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }
    let attempted_event = runtime_core::Event::new(/* ... */);
    let attempted_event_id = attempted_event.id;
    let attempted_hash =
        crate::audit::append_event(&tx, key, &attempted_event, Some(&granted_hash))?;
    tx.commit()?;
    // AFTER commit — only now does an SMTP connection ever open.
    match crate::sinks::email_smtp::invoke_email_smtp_from_resolved(/* conn, not tx */) { ... }
}
```

**Landmine (confirmed live, not hypothetical):** `server.rs:926-928` currently binds
`let locked = conn.lock()...?;` — **immutable**. `rusqlite::Connection::transaction(&mut self)`
requires `&mut`. The port MUST change this to `let mut locked = conn.lock()...?;` before calling
`locked.transaction()?` (MutexGuard implements DerefMut, so this compiles once mutable). This is
NOT a discrepancy with the analog — `confirmation.rs`'s `conn` parameter is already `&mut
Connection` (single-threaded process, no Arc<Mutex<>>), so the analog never hit this wrinkle. The
Allowed-path's `conn: &Arc<Mutex<Connection>>` is architecturally different (concurrent broker
tasks) — document this divergence explicitly in the plan.

**Key derivation — do NOT copy `transition_state`'s key.** `confirmation.rs`'s CAS keys on
`effect_id` because `effect_id` is stable (persisted at Block-time in the `PendingConfirmation`
row). The Allowed path mints `effect_id` FRESH every call (`Uuid::new_v4()` — confirmed at
`server.rs`'s top of `evaluate_plan_node_and_record`), so an `effect_id`-keyed CAS on this path
does nothing. Use the content-derived key instead (see idempotency-key fn below).

---

### idempotency-key fn (utility, transform)

**Analog:** `combined_digest` (`crates/brokerd/src/confirmation.rs:79-109`) — sort-by-name, then
hash. Signature differs: `combined_digest(&[(&str, &str)])` operates on resolved LITERALS;
the new fn must operate on `(arg_name, value_id)` pairs (D-08 pins `value_id`-scope, not
literal-scope) — a genuinely new function, not a call-site reuse.

**Pattern to mirror (sort discipline, from `confirmation.rs:79-91`):**
```rust
pub fn combined_digest(args: &[(&str, &str)]) -> String {
    let mut sorted: Vec<(&str, &str)> = args.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for pair in sorted.windows(2) {
        assert_ne!(pair[0].0, pair[1].0, "combined_digest: duplicate arg_name ...");
    }
    let mut hasher = Sha256::new();
    for (name, literal) in &sorted {
        let name_digest = { let mut h = Sha256::new(); h.update(name.as_bytes()); hex::encode(h.finalize()) };
        // ... literal_digest similarly, both fed into outer hasher ...
    }
    hex::encode(hasher.finalize())
}
```
**New fn shape (per RESEARCH.md Pattern 3 — simpler direct-concat is acceptable, state choice explicitly):**
```rust
pub(crate) fn plan_node_idempotency_key(sink: &SinkId, args: &[PlanArg]) -> String {
    let mut sorted: Vec<&PlanArg> = args.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut hasher = Sha256::new();
    hasher.update(sink.0.as_bytes());
    for arg in sorted {
        hasher.update(arg.name.as_bytes());
        hasher.update(arg.value_id.0.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}
```

---

### `crates/brokerd/src/audit.rs` — new `sent_plan_nodes` table + migration (migration/schema)

**Analog:** `migrate_chain_anchor_schema` (`audit.rs:280-304`) — the whole-new-table
presence-check pattern (distinct from `migrate_pending_confirmations_schema`'s
column-widening `ALTER TABLE` pattern used for the `mac` column, `audit.rs:250-260`).

**Mirror verbatim** (`audit.rs:280-304`):
```rust
fn migrate_chain_anchor_schema(conn: &rusqlite::Connection) -> Result<()> {
    let table_exists: bool = match conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chain_anchor'",
        [], |_row| Ok(()),
    ) {
        Ok(()) => true,
        Err(rusqlite::Error::QueryReturnedNoRows) => false,
        Err(e) => return Err(anyhow::Error::from(e)),
    };
    if !table_exists {
        return Err(anyhow::anyhow!(
            "chain_anchor table missing after SCHEMA_DDL ran — open_audit_db invariant violated"
        ));
    }
    Ok(())
}
```
New table DDL goes in `SCHEMA_DDL` (`audit.rs:92` const, alongside `chain_anchor`'s `CREATE TABLE
IF NOT EXISTS` at `:194`), pinned shape: `sent_plan_nodes(idempotency_key TEXT PRIMARY KEY,
effect_id TEXT NOT NULL, session_id TEXT NOT NULL, sent_at TEXT NOT NULL)`. Wire the new
`migrate_sent_plan_nodes_schema(&conn)?` call into `open_audit_db` (`audit.rs:315-322`) right
after `migrate_chain_anchor_schema(&conn)?`, identical position/shape.

---

### `crates/executor/src/sink_sensitivity.rs` — HARDEN-05 (validation/policy, I2 TCB)

**Analog:** the SAME functions' `email.send` arms (self-mirroring file).

**Edit 1 — `is_content_sensitive` (`:102-107` current):**
```rust
pub fn is_content_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_CONTENT_SENSITIVE.contains(&arg_name),
        _ => false,   // <- add "file.create" => FILE_CREATE_CONTENT_SENSITIVE.contains(&arg_name) here
    }
}
```
New const: `const FILE_CREATE_CONTENT_SENSITIVE: &[&str] = &["contents"];` (mirrors
`EMAIL_SEND_CONTENT_SENSITIVE`'s declaration shape). Scoped to `&["contents"]` ONLY — `path` must
never become content-sensitive (Pitfall 5).

**Edit 2 — `expected_role`'s `file.create` arm (`:154-160` current, confirmed live):**
```rust
"file.create" => match arg_name {
    "path" => Some(&["path", "relative_path"]),
    "contents" => None, // unconstrained for v1.5 — Assumption A2 (DESIGN §3/§10)
    _ => None,
},
```
Flip `"contents" => None` to `"contents" => Some(&["path"])`. This is the load-bearing, pinned,
non-negotiable value — end-to-end-traced to `cli/caprun/src/planner.rs:208`'s reuse of the
trusted `"path"`-role `intent_value_id` in the `contents` slot. Any list omitting `"path"` breaks
the only live `file.create` flow (`s9_live_file_create_clean_allow`).

**Test to invert (`:308-317` current, confirmed live):**
```rust
#[test]
fn file_create_contents_is_unconstrained() {
    assert_eq!(
        expected_role(&file_create(), "contents"),
        None,
        "file.create `contents` stays unconstrained for v1.5 (Assumption A2)"
    );
}
```
Rename + invert to (per RESEARCH.md Code Example #2):
```rust
#[test]
fn file_create_contents_expects_path() {
    assert_eq!(
        expected_role(&file_create(), "contents"),
        Some(&["path"][..]),
        "file.create `contents` is role-checked as of v1.6 HARDEN-05 — the \
         only live production value is the reused trusted `path`-role literal"
    );
}
```
Add a defensive companion test (Pitfall 5 guard, not in DESIGN doc but recommended — mirrors the
existing `file_create_contents_not_routing_sensitive` shape): assert `path` is NOT
content-sensitive after the change.

---

## Shared Patterns

### Atomic CAS-then-append-then-commit-then-effect
**Source:** `crates/brokerd/src/confirmation.rs:868-896` (SEND-01)
**Apply to:** `server.rs`'s Allowed `email.send` block only (the sole Allowed-dispatch-CAS sink
today per RESEARCH Open Question #1's recommendation — no other sink needs this yet).

### Idempotent whole-new-table schema migration
**Source:** `crates/brokerd/src/audit.rs:280-304` (`migrate_chain_anchor_schema`)
**Apply to:** the new `migrate_sent_plan_nodes_schema` fn + `open_audit_db` wiring.

### Hardcoded per-sink-arg classification table (never a swappable policy file)
**Source:** `crates/executor/src/sink_sensitivity.rs`'s `is_content_sensitive`/`is_routing_sensitive`/`expected_role`
**Apply to:** the two `file.create` edits — CON-i2-non-bypassable discipline, matched exactly (no
`.unwrap_or(&[])`, never construct `Some(&[])`).

## No Analog Found

None — every file this phase touches has a proven, shipped precedent in-repo (Phase 27/28
HARDEN-01/02/04 machinery), per RESEARCH.md's explicit "no new architecture" framing.

## Divergences from DESIGN doc's stated current state (confirmed live, re-verified this session)

- DESIGN doc cites Allowed `email.send` block at `server.rs:792`; live code is at `:901-977`
  (~110-line shift from Phase 28's HARDEN-02 code landing above it). RESEARCH.md's anchors are
  correct; use those, not the DESIGN doc's raw line numbers.
- The `conn.lock()` immutable-binding landmine (Pitfall 1) is confirmed present verbatim at
  `server.rs:926-928` (`let locked = conn.lock()...?;`) — must become `let mut locked`.
- `sink_sensitivity.rs`'s `file.create` `expected_role` arm confirmed at `:154-160` (not exactly
  DESIGN's `:147-162` span, but same content) — `"contents" => None` is live and unchanged since
  v1.5, confirming the DESIGN doc's premise is accurate.

## Metadata

**Analog search scope:** `crates/brokerd/src/{server,audit,confirmation}.rs`,
`crates/executor/src/sink_sensitivity.rs`, `crates/brokerd/tests/email_smtp_acceptance.rs`
**Files scanned:** 6 (all re-read live this session, no stale DESIGN-doc line numbers relied upon)
**Pattern extraction date:** 2026-07-17
</content>
