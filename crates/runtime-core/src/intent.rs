/// intent.rs — Intent and IntentStatus
///
/// An Intent is the user's goal. The runtime works to fulfill it within
/// the constraints of the session's authorization (I0/I1/I2 invariants).
/// Public API uses `Intent` — never expose `ExecutionContext` (DEC-terminology).

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The v0 typed caprun intent — the deterministic planner's only input surface.
///
/// Each variant maps deterministically to exactly one `PlanNode`. The enum is
/// small and closed: adding a new action requires a new variant (no wildcard dispatch).
///
/// LLM-driven intent is v2 (PLAN-F1) — not implemented here.
///
/// `#[serde(tag = "kind")]` ensures struct variants serialize with a stable, explicit
/// shape (`{ "kind": "SendEmailSummary", "recipient": "..." }`) so the JSON produced
/// by `caprun main` and consumed by the worker deserializes identically (Pitfall 4).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum CaprunIntent {
    /// Send an email summary to a known, user-trusted recipient.
    ///
    /// `recipient` is a user-provided literal that will be minted as `UserTrusted`
    /// by `mint_from_intent` in the broker — the planner receives only the opaque
    /// `ValueId` handle, never the literal directly.
    SendEmailSummary { recipient: String },
}

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
