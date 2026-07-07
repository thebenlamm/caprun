/// session — Session lifecycle management
///
/// Reuses `runtime_core::Session` and `runtime_core::SessionStatus` directly
/// rather than redefining them (RESEARCH.md §Architectural Responsibility Map).
/// brokerd owns the persistence and lifecycle transitions; runtime-core owns
/// the domain types.

use chrono::Utc;
use runtime_core::{SeedProvenance, Session, SessionStatus};
use uuid::Uuid;

/// Create a new in-memory Session for the given intent.
///
/// Generates a fresh v4 UUID for the session id, and stamps both `created_at`
/// and `updated_at` to the current UTC time.
///
/// # I0 creation rule (ORIGIN-02, DESIGN-session-trust-state.md §3)
///
/// The initial `status` is conditional on `seed_provenance` — a trusted-path
/// determination the `caprun` CLI makes at intent-parsing time (it uniquely
/// knows whether the intent came from `argv` or was read from a file) and
/// passes in here. `create_session` itself is what actually SETS the status
/// from that provenance; the initial status is never self-declared by the
/// (potentially injected) caller:
///
/// * `SeedProvenance::FileDerived` → `SessionStatus::Draft` — starts Draft at
///   creation, never `Active` followed by a later demotion (the I0 case).
/// * `SeedProvenance::TrustedArg`  → `SessionStatus::Active` — today's existing
///   behavior, unchanged.
///
/// Exhaustive match, no `_` arm (security enum discipline).
///
/// Call `persist_session` to write it to the SQLite audit DB.
pub fn create_session(intent_id: Uuid, seed_provenance: SeedProvenance) -> Session {
    let now = Utc::now();
    let status = match seed_provenance {
        SeedProvenance::FileDerived => SessionStatus::Draft,
        SeedProvenance::TrustedArg => SessionStatus::Active,
    };
    Session {
        id: Uuid::new_v4(),
        intent_id,
        status,
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

/// Update a persisted Session's `status` column in place.
///
/// This is the monotonic `Active -> Draft` UPDATE path (TAINT-01/04,
/// DESIGN-session-trust-state.md §5) — no such path existed before this plan;
/// `persist_session` above only ever `INSERT`s. Mirrors `persist_session`'s
/// exact param/error-handling style: the same JSON-serialized `SessionStatus`
/// encoding, `anyhow::Result<()>` returned directly from `conn.execute(...)?`,
/// no custom error type.
///
/// Callers are responsible for running this under the SAME lock/transaction as
/// any co-located audit-event append that triggers the demotion (Pitfall 5) —
/// this function itself does not acquire a lock; it takes an already-open
/// `&rusqlite::Connection`.
///
/// # Arguments
/// * `conn` — open rusqlite connection (broker-owned).
/// * `session_id` — the session row to update.
/// * `status` — the new status to write.
pub fn update_session_status(
    conn: &rusqlite::Connection,
    session_id: Uuid,
    status: &SessionStatus,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE sessions SET status = ?1 WHERE id = ?2",
        rusqlite::params![serde_json::to_string(status)?, session_id.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::open_audit_db;

    /// ORIGIN-02: a file-derived session starts `Draft` at creation, never
    /// `Active` followed by a later demotion.
    #[test]
    fn create_session_file_derived_starts_draft() {
        let session = create_session(Uuid::new_v4(), SeedProvenance::FileDerived);
        assert_eq!(session.status, SessionStatus::Draft);
    }

    /// A trusted-arg seed starts `Active` — today's existing behavior, unchanged.
    #[test]
    fn create_session_trusted_arg_starts_active() {
        let session = create_session(Uuid::new_v4(), SeedProvenance::TrustedArg);
        assert_eq!(session.status, SessionStatus::Active);
    }

    /// `update_session_status` mutates a persisted row's status: a session
    /// created (and persisted) as `Active` reads back as `Draft` after the
    /// UPDATE path runs.
    #[test]
    fn update_session_status_mutates_persisted_row() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session = create_session(Uuid::new_v4(), SeedProvenance::TrustedArg);
        persist_session(&conn, &session).expect("persist_session");

        update_session_status(&conn, session.id, &SessionStatus::Draft)
            .expect("update_session_status");

        let status_json: String = conn
            .query_row(
                "SELECT status FROM sessions WHERE id = ?1",
                rusqlite::params![session.id.to_string()],
                |row| row.get(0),
            )
            .expect("query status");
        let status: SessionStatus =
            serde_json::from_str(&status_json).expect("deserialize status");
        assert_eq!(status, SessionStatus::Draft);
    }
}
