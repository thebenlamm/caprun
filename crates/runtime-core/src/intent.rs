/// intent.rs — Intent and IntentStatus
///
/// An Intent is the user's goal. The runtime works to fulfill it within
/// the constraints of the session's authorization (I0/I1/I2 invariants).
/// Public API uses `Intent` — never expose `ExecutionContext` (DEC-terminology).

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The status of an Intent through its lifecycle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IntentStatus {
    Active,
    WaitingApproval,
    Completed,
    RolledBack,
    Abandoned,
    Failed,
}

/// A user's goal to be executed by the runtime.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Intent {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub description: String,
    /// Principal who created this intent — simple string for v0.
    pub created_by: String,
    pub status: IntentStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
