/// audit — SQLite hash-linked audit DAG
///
/// Schema: two tables (sessions, events) with a SHA-256 hash chain over the
/// events table. Each event row stores its own hash and its parent's hash,
/// forming a tamper-evident append-only chain.
///
/// The hash input is the canonical concatenation:
///   SHA-256(parent_hash || id || session_id || event_type || payload || taint)
/// where `||` means sequential `Digest::update` calls (no separator needed
/// because each field has a fixed or content-delimited role in the chain).
///
/// Append-only invariant: brokerd issues NO UPDATE or DELETE on the `events`
/// table. Only the test suite uses raw SQL to simulate tampering.
///
/// See 03-RESEARCH.md Pattern 5 for the schema DDL and hash computation.

use anyhow::Result;
use runtime_core::Event;
use sha2::{Digest, Sha256};

/// SQLite schema — STRICT tables enforce column type constraints.
///
/// `sessions`: broker-created session rows (id, intent, status, created_at).
/// `events`: append-only audit chain (id, parent_id, hash, parent_hash, …).
pub const SCHEMA_DDL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    intent_id  TEXT NOT NULL,
    status     TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT,
    session_id  TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    actor       TEXT NOT NULL,
    payload     TEXT NOT NULL,
    taint       TEXT NOT NULL,
    parent_hash TEXT,
    hash        TEXT NOT NULL
) STRICT;

-- Redactable side table for blocked-literal data at rest. The raw literal from a
-- `sink_blocked` event lives HERE, keyed by (event_id, arg) — NEVER in the hashed
-- `events.payload` (only its SHA-256 digest is in the anchor/chain). A `sink_blocked`
-- event carries a PLURAL anchors collection (Phase 14, D-14 Collect-then-Block), so
-- one event can have MULTIPLE blocked-literal rows, one per blocked arg — `arg` is
-- part of the primary key precisely so every blocked arg's literal is persisted, not
-- just the first (a plan node cannot have two args of the same name — validate_schema
-- already guarantees `arg` is unique within one event's blocked set). Redaction is a
-- single `DELETE FROM blocked_literals WHERE event_id = ?`, which removes ALL rows for
-- that event at once (the whole block resolves confirm/deny together): the digests
-- remain in the tamper-evident chain as proof content existed, but the literals are
-- gone. This resolves the tamper-evidence-vs-redactability conflict of storing the
-- literal in the chain.
CREATE TABLE IF NOT EXISTS blocked_literals (
    event_id TEXT NOT NULL,
    arg      TEXT NOT NULL,
    literal  TEXT NOT NULL,
    PRIMARY KEY (event_id, arg)
) STRICT;

-- Redactable/mutable side table for the durable confirm/deny checkpoint, keyed by
-- `effect_id` (the same identity as `SinkBlockedAnchor.effect_id`) — playing the
-- same role `blocked_literals` plays for the raw literal: state that must survive
-- past the blocking process's exit, kept OUT of the hashed `events.payload` column
-- so it stays mutable/redactable without breaking `verify_chain`. `caprun confirm`/
-- `caprun deny` are ALWAYS separate, later OS processes from the one that created
-- the block (DESIGN-confirmation-release.md, section: The Problem Being Solved); this
-- table is the ONLY thing that survives to resume from. `blocked_event_id` is the id
-- of the anchoring `sink_blocked` event (never an `events`-table column) — used later
-- to chain the confirm/deny event's `parent_id` and to gate on blocked_literals
-- redaction. `resolved_args` is a JSON-serialized `Vec<ResolvedArg>` blob, mirroring
-- the `events.payload` convention of serializing the whole struct to one TEXT column —
-- never a normalized child table (RESEARCH Open Question 2).
--
-- `blocked_arg_names`/`combined_digest` (Phase 16, CONFIRM-03, DESIGN-confirm-
-- binding.md Round-6) are additive: `blocked_arg_names` is a JSON-serialized
-- `Vec<String>` (DISPLAY-MARKING metadata only), and `combined_digest` is the
-- SHA-256 digest mirrored from the hashed `sink_blocked` Event payload. A
-- pre-existing DB predating these columns is widened idempotently by
-- `migrate_pending_confirmations_schema` below — `CREATE TABLE IF NOT EXISTS`
-- only ever fires on a FRESH database.
CREATE TABLE IF NOT EXISTS pending_confirmations (
    effect_id           TEXT PRIMARY KEY,
    session_id          TEXT NOT NULL,
    blocked_event_id    TEXT NOT NULL,
    sink                TEXT NOT NULL,
    resolved_args       TEXT NOT NULL,
    blocked_arg_names   TEXT NOT NULL,
    combined_digest     TEXT NOT NULL,
    workspace_root_path TEXT NOT NULL,
    state               TEXT NOT NULL
) STRICT;
";

/// Idempotently widen a pre-existing `pending_confirmations` table with the
/// two Phase-16 CONFIRM-03 columns (`blocked_arg_names`, `combined_digest`).
///
/// `CREATE TABLE IF NOT EXISTS` in `SCHEMA_DDL` above only ever creates the
/// table on a FRESH database — a pre-existing `audit.db` (created before this
/// change) keeps its original 7-column shape forever unless explicitly
/// migrated here. Without this, the widened `insert_pending_confirmation`
/// would fail with "no such column" the moment `caprun confirm` reuses a
/// run's original DB (`main.rs:321`).
///
/// Gated on a `PRAGMA table_info(pending_confirmations)` column-presence
/// check — never a blind `ALTER TABLE ... ADD COLUMN`, which errors on a
/// column that already exists. This function runs on EVERY `open_audit_db`
/// call, so it MUST be safe to re-run against an already-migrated (or
/// freshly-created, already-widened) DB — verified by
/// `pending_confirmations_migration_is_idempotent` below.
///
/// A legacy row that predates this migration gets the DEFAULT values
/// (`''`/`'[]'`) — `confirm()`'s recompute-and-compare (Plan 16-02) will fail
/// its digest compare closed for such a row, which is the correct, accepted
/// behavior for data that predates the binding.
fn migrate_pending_confirmations_schema(conn: &rusqlite::Connection) -> Result<()> {
    let mut existing_columns: Vec<String> = Vec::new();
    {
        let mut stmt = conn.prepare("PRAGMA table_info(pending_confirmations)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            // PRAGMA table_info columns: cid, name, type, notnull, dflt_value, pk.
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

/// Open (or create) the audit database at `path` and run schema DDL.
///
/// Uses rusqlite with the `bundled` feature (no system SQLite dep).
/// WAL mode is enabled for concurrent read access.
///
/// Pass `":memory:"` for an in-process, ephemeral DB suitable for tests.
///
/// # Arguments
/// * `path` — filesystem path for the SQLite file, or `":memory:"`.
pub fn open_audit_db(path: &str) -> Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(SCHEMA_DDL)?;
    migrate_pending_confirmations_schema(&conn)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(conn)
}

/// Persist the raw blocked literal for ONE blocked arg of a `sink_blocked` event
/// into the redactable `blocked_literals` side table, keyed by (event id, arg).
///
/// The literal lives ONLY here — never in `events.payload` (the hashed anchor
/// carries only its SHA-256 digest). This keeps the raw literal (attacker content
/// / PII) out of the tamper-evident chain so it can later be redacted without
/// breaking `verify_chain`. A plural (Phase 14) block calls this ONCE PER anchor
/// in the event's `anchors` collection — every blocked arg's literal is persisted,
/// not just the first. Caller should invoke this under the same broker-owned
/// connection lock as the `append_event` that wrote the `sink_blocked` row.
pub fn insert_blocked_literal(
    conn: &rusqlite::Connection,
    event_id: &str,
    arg: &str,
    literal: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO blocked_literals (event_id, arg, literal) VALUES (?1, ?2, ?3)",
        rusqlite::params![event_id, arg, literal],
    )?;
    Ok(())
}

/// Fetch a raw literal for a blocked event, or `None` if none was ever stored or
/// all have been redacted. Verify tamper-evidence by comparing `sha256(literal)`
/// to the matching anchor's `literal_sha256`.
///
/// This is a presence/redaction-gate check (`confirmation::confirm`'s Step 3):
/// for a single-blocked-arg event (today's only exercised shape) this returns
/// THE literal. For a genuinely-plural block it returns an arbitrary one of the
/// surviving rows — sufficient for the presence check (redaction removes every
/// row for the event at once, so "any row survives" is a well-defined, atomic
/// property), but NOT a stable per-arg accessor; a dedicated per-arg query is
/// Phase 16 scope alongside the rest of multi-arg narration (CONFIRM-04).
pub fn get_blocked_literal(conn: &rusqlite::Connection, event_id: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT literal FROM blocked_literals WHERE event_id = ?1")?;
    let mut rows = stmt.query(rusqlite::params![event_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

/// Redact the raw literal for a blocked event: delete its `blocked_literals` row.
///
/// This does NOT touch the `events` chain — the anchor's `literal_sha256` digest
/// remains, so `verify_chain` still passes and the audit record still proves that
/// content of that digest was blocked. Returns the number of rows removed (0 if
/// already absent). Redaction is intentionally idempotent.
pub fn redact_blocked_literal(conn: &rusqlite::Connection, event_id: &str) -> Result<usize> {
    let removed = conn.execute(
        "DELETE FROM blocked_literals WHERE event_id = ?1",
        rusqlite::params![event_id],
    )?;
    Ok(removed)
}

/// Fetch the stored `hash` for a known event id, or `None` if no event with that
/// id exists.
///
/// Lets a later confirm/deny process fetch the anchoring `sink_blocked` event's
/// hash so it can append its confirm/deny event with the correct `parent_hash`
/// (keeping `verify_chain` linear), without needing the full deserialized `Event`.
pub fn event_hash_by_id(conn: &rusqlite::Connection, event_id: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT hash FROM events WHERE id = ?1")?;
    let mut rows = stmt.query(rusqlite::params![event_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

/// Compute the SHA-256 hash for an audit event row.
///
/// Input is the ordered concatenation (no separator) of:
///   parent_hash (empty string if None) || id || session_id ||
///   event_type || payload || taint
///
/// Returns hex-encoded lowercase SHA-256 digest.
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

/// Append an event to the audit DAG and return its SHA-256 hash.
///
/// Reuses `runtime_core::Event` directly — no duplicate type definition.
/// The full event is serialized to JSON for the `payload` column; `event.taint`
/// is serialized separately for the `taint` column.
///
/// # Arguments
/// * `conn` — open rusqlite connection (broker-owned; never shared with workers).
/// * `event` — the event to persist.
/// * `parent_hash` — hash of the parent event row (`None` for session-root events).
///
/// # Append-only invariant
/// This function only ever issues `INSERT`. No UPDATE or DELETE on `events`.
pub fn append_event(
    conn: &rusqlite::Connection,
    event: &Event,
    parent_hash: Option<&str>,
) -> Result<String> {
    // Defect B guard (DESIGN §4 rule 7): a `sink_blocked` event with no anchors is
    // a security-meaningless bare marker. Reject it here so it is NON-PERSISTABLE
    // through the TCB — not merely never-triggered. Plural (Phase 14): the guard
    // now checks the whole collection is non-empty, not a singular `Option`.
    if event.event_type == "sink_blocked" && event.anchors.is_empty() {
        return Err(anyhow::anyhow!(
            "sink_blocked event requires at least one anchor (Defect B guard)"
        ));
    }
    let payload = serde_json::to_string(event)?;
    let taint_str = serde_json::to_string(&event.taint)?;
    let hash = compute_event_hash(
        parent_hash,
        &event.id.to_string(),
        &event.session_id.to_string(),
        &event.event_type,
        &payload,
        &taint_str,
    );
    conn.execute(
        "INSERT INTO events \
         (id, parent_id, session_id, event_type, actor, payload, taint, parent_hash, hash) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            event.id.to_string(),
            event.parent_id.map(|id| id.to_string()),
            event.session_id.to_string(),
            &event.event_type,
            &event.actor,
            &payload,
            &taint_str,
            parent_hash,
            &hash,
        ],
    )?;
    Ok(hash)
}

/// Return all Events recorded for `session_id`, ordered by insertion (rowid).
///
/// Each row's `payload` column contains the full serialized `Event` (written by
/// `append_event`). Deserializes each payload back into a `runtime_core::Event`
/// so the returned Events carry their original `taint` labels intact.
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to query (as a string).
pub fn query_events_by_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT payload FROM events WHERE session_id = ?1 ORDER BY rowid",
    )?;
    let events = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let payload: String = row.get(0)?;
            Ok(payload)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    events
        .into_iter()
        .map(|p| serde_json::from_str::<Event>(&p).map_err(anyhow::Error::from))
        .collect()
}

/// Locate the first Event of `event_type` within `session_id`.
///
/// Deserializes the `payload` column so the returned Event's `taint` reflects
/// what was originally persisted — required by the §9 held-out test to assert
/// the taint chain anchor exists in the DAG.
///
/// Returns `None` if no matching event exists for the session.
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to search (as a string).
/// * `event_type` — the event_type string to match (e.g., `"file_read"`, `"email_send_stub"`).
pub fn find_event_by_type(
    conn: &rusqlite::Connection,
    session_id: &str,
    event_type: &str,
) -> Result<Option<Event>> {
    let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
        "SELECT payload FROM events \
         WHERE session_id = ?1 AND event_type = ?2 \
         ORDER BY rowid \
         LIMIT 1",
        rusqlite::params![session_id, event_type],
        |row| row.get(0),
    );
    match result {
        Ok(payload) => {
            let event = serde_json::from_str::<Event>(&payload)?;
            Ok(Some(event))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::Error::from(e)),
    }
}

/// Locate a SPECIFIC Event by its exact `id` within `session_id`.
///
/// Unlike `find_event_by_type` (first-of-type only, `LIMIT 1`), this resolves
/// exactly ONE event by its unique primary key (`id`), scoped additionally to
/// `session_id` for defense in depth (a cross-session id must never resolve).
/// This is the accessor the EXTRACT-02 per-anchor audit walk needs: once a
/// session has ≥2 events of the same `event_type` (true as soon as multi-field
/// extraction produces ≥2 `file_read` events), `find_event_by_type` can no
/// longer disambiguate WHICH one a given `provenance_chain[i]` refers to
/// (15-RESEARCH.md Pitfall 3).
///
/// Returns `None` if no event with that id exists, or if it exists but under a
/// DIFFERENT session_id (session-scoped, never leaks cross-session events).
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to search (as a string).
/// * `id` — the exact event id to resolve (as a string).
pub fn find_event_by_id(
    conn: &rusqlite::Connection,
    session_id: &str,
    id: &str,
) -> Result<Option<Event>> {
    let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
        "SELECT payload FROM events \
         WHERE id = ?1 AND session_id = ?2",
        rusqlite::params![id, session_id],
        |row| row.get(0),
    );
    match result {
        Ok(payload) => {
            let event = serde_json::from_str::<Event>(&payload)?;
            Ok(Some(event))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::Error::from(e)),
    }
}

/// Return the session's LEAF event — the event that is no other event's
/// `parent_id` within this `session_id` — as `(id, hash)`, or `None` if the
/// session has no events at all.
///
/// On a LINEAR chain (the only shape `append_event`'s single-caller-at-a-time
/// discipline is meant to produce) this is exactly one row: the last event
/// appended. Callers that continue the session's causal chain (`confirm()`,
/// `deny()` — Plan 16-02, MAJOR-7) MUST parent their next appended event on
/// THIS head, never on a mid-chain id like `blocked_event_id` — parenting on
/// a mid-chain id makes that id have two children (a FORK), which
/// `verify_chain`'s single-linear-chain recursive-CTE walk cannot traverse
/// past (empirically discovered in `quarantine.rs`'s `mint_from_read`
/// `chain_head_id`/`chain_head_hash` threading, mirrored here).
///
/// If the chain is ALREADY forked (more than one leaf), this query can return
/// more than one row; `ORDER BY rowid DESC LIMIT 1` picks the most-recently
/// -appended one so the function still returns a single answer, but a forked
/// chain has already failed `verify_chain` — callers MUST gate on
/// `verify_chain` FIRST (Plan 16-02's `confirm()` does exactly this) so this
/// function is only ever consulted against a chain already known to be
/// linear.
pub fn current_chain_head(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<(uuid::Uuid, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, hash FROM events \
         WHERE session_id = ?1 AND id NOT IN ( \
             SELECT parent_id FROM events \
             WHERE session_id = ?1 AND parent_id IS NOT NULL \
         ) \
         ORDER BY rowid DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(rusqlite::params![session_id])?;
    match rows.next()? {
        Some(row) => {
            let id: String = row.get(0)?;
            let hash: String = row.get(1)?;
            Ok(Some((uuid::Uuid::parse_str(&id)?, hash)))
        }
        None => Ok(None),
    }
}

/// Walk the audit chain for `session_id` and verify every hash link.
///
/// Uses a recursive CTE to traverse from the root event (parent_id IS NULL)
/// through each subsequent event linked by `parent_id`. For each row:
///
/// 1. Asserts that `stored.parent_hash` matches the hash of the previous event.
/// 2. Recomputes `compute_event_hash(...)` from stored fields.
/// 3. Asserts the recomputed hash equals `stored.hash`.
///
/// Returns `false` if:
/// - Any hash mismatch is detected (tampering).
/// - Any `parent_hash` link is broken.
/// - No events exist for the session (empty chain is considered unverified).
pub fn verify_chain(conn: &rusqlite::Connection, session_id: &str) -> bool {
    let result: Result<bool> = (|| {
        let mut stmt = conn.prepare(
            "WITH RECURSIVE chain(
                 id, session_id, event_type, payload, taint, parent_hash, hash, depth
             ) AS (
                 -- Root: event with no causal predecessor
                 SELECT id, session_id, event_type, payload, taint, parent_hash, hash, 0
                 FROM events
                 WHERE session_id = ?1 AND parent_id IS NULL
               UNION ALL
                 -- Recursive step: find the next event in the chain
                 SELECT e.id, e.session_id, e.event_type, e.payload, e.taint,
                        e.parent_hash, e.hash, c.depth + 1
                 FROM events e
                 JOIN chain c ON e.parent_id = c.id
                 WHERE e.session_id = ?1
             )
             SELECT id, session_id, event_type, payload, taint, parent_hash, hash
             FROM chain
             ORDER BY depth",
        )?;

        let mut rows = stmt.query(rusqlite::params![session_id])?;
        let mut prev_hash: Option<String> = None;
        let mut found_any = false;

        while let Some(row) = rows.next()? {
            found_any = true;
            let id: String = row.get(0)?;
            let sid: String = row.get(1)?;
            let event_type: String = row.get(2)?;
            let payload: String = row.get(3)?;
            let taint: String = row.get(4)?;
            let stored_parent_hash: Option<String> = row.get(5)?;
            let stored_hash: String = row.get(6)?;

            // Check parent_hash linkage: must equal the previous event's hash
            if stored_parent_hash != prev_hash {
                return Ok(false);
            }

            // Recompute hash from stored fields and compare
            let computed = compute_event_hash(
                stored_parent_hash.as_deref(),
                &id,
                &sid,
                &event_type,
                &payload,
                &taint,
            );
            if computed != stored_hash {
                return Ok(false);
            }

            prev_hash = Some(stored_hash);
        }

        Ok(found_any)
    })();

    result.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use runtime_core::TaintLabel;
    use uuid::Uuid;

    /// Build a minimal file_read Event with taint labels [ExternalUntrusted, EmailRaw].
    fn make_file_read_event(session_id: Uuid) -> Event {
        Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "worker".to_string(),
            "file_read".to_string(),
            Utc::now(),
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
        )
    }

    #[test]
    fn find_event_by_type_returns_event_with_taint_intact() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        let event_id = event.id;

        append_event(&conn, &event, None).expect("append_event");

        let found = find_event_by_type(&conn, &session_id.to_string(), "file_read")
            .expect("find_event_by_type")
            .expect("event should be present");

        assert_eq!(found.id, event_id);
        assert_eq!(
            found.taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
            "taint must survive the payload round-trip"
        );
    }

    #[test]
    fn find_event_by_type_returns_none_for_missing_type() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);

        append_event(&conn, &event, None).expect("append_event");

        let found = find_event_by_type(&conn, &session_id.to_string(), "email_send_stub")
            .expect("find_event_by_type");

        assert!(found.is_none(), "no email_send_stub event should be found");
    }

    /// On a linear chain, `current_chain_head` returns the LAST-appended
    /// event's (id, hash) — never an earlier event, even though the root
    /// event still exists in the same `events` table (Plan 16-02, MAJOR-7).
    #[test]
    fn current_chain_head_returns_last_appended_event_on_linear_chain() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let root = make_file_read_event(session_id);
        let root_hash = append_event(&conn, &root, None).expect("append root");

        let child = Event::new(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            "worker".to_string(),
            "sink_ok".to_string(),
            Utc::now(),
            vec![],
        );
        let child_hash = append_event(&conn, &child, Some(&root_hash)).expect("append child");

        let head = current_chain_head(&conn, &session_id.to_string())
            .expect("current_chain_head")
            .expect("a linear chain must have exactly one leaf");
        assert_eq!(
            head,
            (child.id, child_hash),
            "current_chain_head must return the LAST-appended event, not the root"
        );
    }

    /// A session with no events at all returns `None` (no leaf to report).
    #[test]
    fn current_chain_head_returns_none_for_session_with_no_events() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let head = current_chain_head(&conn, &session_id.to_string()).expect("current_chain_head");
        assert!(head.is_none());
    }

    /// find_event_by_id disambiguates among TWO events of the SAME event_type
    /// in one session — resolving EACH to its own distinct event, which
    /// `find_event_by_type` (LIMIT 1) cannot do (15-RESEARCH.md Pitfall 3).
    #[test]
    fn find_event_by_id_disambiguates_two_same_type_events() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let event_a = make_file_read_event(session_id);
        let event_b = make_file_read_event(session_id);
        assert_ne!(event_a.id, event_b.id, "sanity: distinct event ids");

        append_event(&conn, &event_a, None).expect("append event_a");
        append_event(&conn, &event_b, None).expect("append event_b");

        let found_a = find_event_by_id(&conn, &session_id.to_string(), &event_a.id.to_string())
            .expect("find_event_by_id a")
            .expect("event_a must resolve");
        let found_b = find_event_by_id(&conn, &session_id.to_string(), &event_b.id.to_string())
            .expect("find_event_by_id b")
            .expect("event_b must resolve");

        assert_eq!(found_a.id, event_a.id, "must resolve event_a to its OWN id");
        assert_eq!(found_b.id, event_b.id, "must resolve event_b to its OWN id");
        assert_ne!(
            found_a.id, found_b.id,
            "the two same-type events must resolve to DISTINCT events"
        );
    }

    /// A uuid never appended to the DAG resolves to `None` — the anti-staple
    /// negative-space case (a fabricated root never resolves).
    #[test]
    fn find_event_by_id_returns_none_for_never_appended_uuid() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        append_event(&conn, &event, None).expect("append_event");

        let fabricated_id = Uuid::new_v4();
        let found = find_event_by_id(&conn, &session_id.to_string(), &fabricated_id.to_string())
            .expect("find_event_by_id");
        assert!(
            found.is_none(),
            "a uuid never appended to the DAG must resolve to None"
        );
    }

    /// A real event id from a DIFFERENT session must resolve to `None`
    /// (session-scoped lookup — never leaks cross-session events).
    #[test]
    fn find_event_by_id_returns_none_for_cross_session_id() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_a = Uuid::new_v4();
        let session_b = Uuid::new_v4();
        let event = make_file_read_event(session_a);
        append_event(&conn, &event, None).expect("append_event");

        let found = find_event_by_id(&conn, &session_b.to_string(), &event.id.to_string())
            .expect("find_event_by_id");
        assert!(
            found.is_none(),
            "an event id from a DIFFERENT session must resolve to None (session-scoped)"
        );
    }

    #[test]
    fn event_hash_by_id_returns_hash_for_known_event() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        let event_id = event.id.to_string();

        let hash = append_event(&conn, &event, None).expect("append_event");

        let found = event_hash_by_id(&conn, &event_id).expect("event_hash_by_id");
        assert_eq!(found, Some(hash));
    }

    #[test]
    fn event_hash_by_id_returns_none_for_unknown_id() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");

        let found = event_hash_by_id(&conn, &Uuid::new_v4().to_string())
            .expect("event_hash_by_id");
        assert!(found.is_none());
    }

    #[test]
    fn pending_confirmations_insert_and_duplicate_effect_id_fails() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let effect_id = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let blocked_event_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                effect_id,
                session_id,
                blocked_event_id,
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        )
        .expect("first insert should succeed");

        let duplicate = conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                effect_id,
                session_id,
                blocked_event_id,
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        );

        assert!(
            duplicate.is_err(),
            "duplicate effect_id insert must fail the PRIMARY KEY constraint"
        );
    }

    /// (Task 2 migration test) A DB whose `pending_confirmations` table
    /// PREDATES the Phase-16 `blocked_arg_names`/`combined_digest` columns
    /// (the OLD 7-column shape) is widened idempotently by
    /// `migrate_pending_confirmations_schema` — reopening it a second time
    /// does not error, and a widened 9-column INSERT then succeeds where it
    /// would otherwise fail with "no such column."
    #[test]
    fn pending_confirmations_migration_widens_legacy_schema_idempotently() {
        // Build a temp-file DB (":memory:" can't be reopened across
        // connections) with the OLD 7-column `pending_confirmations` shape —
        // bypassing `open_audit_db`/`SCHEMA_DDL` entirely to simulate a
        // pre-Phase-16 database on disk.
        let mut path = std::env::temp_dir();
        path.push(format!("caprun_legacy_pending_confirmations_{}.db", Uuid::new_v4()));

        {
            let legacy_conn = rusqlite::Connection::open(&path).expect("open legacy db");
            legacy_conn
                .execute_batch(
                    "CREATE TABLE pending_confirmations (
                        effect_id           TEXT PRIMARY KEY,
                        session_id          TEXT NOT NULL,
                        blocked_event_id    TEXT NOT NULL,
                        sink                TEXT NOT NULL,
                        resolved_args       TEXT NOT NULL,
                        workspace_root_path TEXT NOT NULL,
                        state               TEXT NOT NULL
                    ) STRICT;",
                )
                .expect("create legacy 7-column table");
        }

        // First open: `open_audit_db` runs `SCHEMA_DDL` (a no-op — the table
        // already exists) then the migration, which MUST widen the legacy
        // table in place.
        let conn = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (first, migrating)");

        let widened = conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        );
        assert!(
            widened.is_ok(),
            "a widened 9-column INSERT must succeed after migrating a legacy \
             7-column pending_confirmations table: {widened:?}"
        );
        drop(conn);

        // Second open (a fresh connection, simulating `caprun confirm` reusing
        // the same DB file later): the migration must be idempotent — no
        // "duplicate column name" error, and the widened INSERT still works.
        let conn2 = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (second, already-migrated)");
        let widened_again = conn2.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        );
        assert!(
            widened_again.is_ok(),
            "re-opening an already-migrated DB must not error, and a second \
             widened INSERT must still succeed: {widened_again:?}"
        );

        drop(conn2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn query_events_by_session_returns_all_events() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        let event_id = event.id;

        append_event(&conn, &event, None).expect("append_event");

        let events = query_events_by_session(&conn, &session_id.to_string())
            .expect("query_events_by_session");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event_id);
        assert_eq!(
            events[0].taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw]
        );
    }
}
