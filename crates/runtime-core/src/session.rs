/// session.rs — Session and SessionStatus
///
/// A Session is the execution context for an Intent. Every external effect
/// is authorized against a Session. Public API uses `Session` — `ExecutionContext`
/// is internal only (DEC-terminology).

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The status of a Session through its lifecycle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SessionStatus {
    Active,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}

/// An authorized execution context for an Intent.
///
/// Note: `ExecutionContext` is the internal Rust backing struct — it is never
/// exposed in the public API. Public code uses `Session` (DEC-terminology).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
