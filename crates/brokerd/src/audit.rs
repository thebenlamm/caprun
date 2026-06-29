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
";

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
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(conn)
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
