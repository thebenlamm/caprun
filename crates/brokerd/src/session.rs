/// session — Session lifecycle management
///
/// Reuses `runtime_core::Session` and `runtime_core::SessionStatus` directly
/// rather than redefining them (RESEARCH.md §Architectural Responsibility Map).
/// brokerd owns the persistence and lifecycle transitions; runtime-core owns
/// the domain types.

use chrono::Utc;
use runtime_core::{Session, SessionStatus};
use uuid::Uuid;

/// Create a new in-memory Session for the given intent.
///
/// Generates a fresh v4 UUID for the session id, sets status to `Active`,
/// and stamps both `created_at` and `updated_at` to the current UTC time.
///
/// Call `persist_session` to write it to the SQLite audit DB.
pub fn create_session(intent_id: Uuid) -> Session {
    let now = Utc::now();
    Session {
        id: Uuid::new_v4(),
        intent_id,
        status: SessionStatus::Active,
        created_at: now,
        updated_at: now,
    }
}

/// Persist a newly created Session to the SQLite `sessions` table.
///
/// Inserts one row: `(id, intent_id, status, created_at)`.
/// `status` is JSON-serialized via serde_json so it round-trips cleanly.
///
/// # Arguments
/// * `conn` — open rusqlite connection (broker-owned).
/// * `session` — the session to persist.
pub fn persist_session(conn: &rusqlite::Connection, session: &Session) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, intent_id, status, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            session.id.to_string(),
            session.intent_id.to_string(),
            serde_json::to_string(&session.status)?,
            session.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}
