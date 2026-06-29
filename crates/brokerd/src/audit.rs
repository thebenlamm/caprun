/// audit — SQLite hash-linked audit DAG
///
/// Schema: two tables (sessions, events) with a SHA-256 hash chain over the
/// events table. Each event row stores its own hash and its parent's hash,
/// forming a tamper-evident append-only chain.
///
/// See RESEARCH.md Pattern 5 for the schema DDL and hash computation.
/// Wave 2 Plan 03 implements the real append and verify functions.

/// Open (or create) the audit database at `path` and run schema DDL.
///
/// Uses rusqlite with the `bundled` feature (no system SQLite dep).
/// WAL mode is enabled for concurrent read access.
///
/// # Arguments
/// * `path` — filesystem path for the SQLite file (broker-owned; worker
///   cannot access it — Landlock denies the worker's filesystem access).
///
/// Returns the open connection on success.
pub fn open_audit_db(path: &str) -> anyhow::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(SCHEMA_DDL)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(conn)
}

const SCHEMA_DDL: &str = "
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
