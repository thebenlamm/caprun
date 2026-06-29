/// session — Session lifecycle management
///
/// Reuses `runtime_core::Session` and `runtime_core::SessionStatus` directly
/// rather than redefining them (RESEARCH.md §Architectural Responsibility Map).
/// brokerd owns the persistence and lifecycle transitions; runtime-core owns
/// the domain types.

use runtime_core::{Session, SessionStatus};
use uuid::Uuid;
use chrono::Utc;

/// Create a new in-memory Session for the given intent.
///
/// The caller is responsible for persisting the session to SQLite via
/// `brokerd::audit::open_audit_db` (Wave 2 Plan 03).
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
