/// session.rs — Session and SessionStatus
///
/// A Session is the execution context for an Intent. Every external effect
/// is authorized against a Session. Public API uses `Session` — `ExecutionContext`
/// is internal only (DEC-terminology).

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The status of a Session through its lifecycle.
///
/// `Draft` (v1.2, DESIGN-session-trust-state.md §1): a session demoted from
/// `Active` after touching untrusted content (I1), or created directly as
/// `Draft` from a file-derived seed (I0, §3). The `Active -> Draft` transition
/// is one-way/monotonic — a `Draft` session MUST NOT transition back to
/// `Active`. There is no `Draft -> Active` transition in this milestone's scope.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SessionStatus {
    Active,
    Draft,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}

/// The trusted-path-determined provenance of a Session's seed (ORIGIN-01/02,
/// DESIGN-session-trust-state.md §3, RESEARCH OQ2).
///
/// Decided by the `caprun` CLI at intent-parsing time (it alone knows whether
/// the intent came from `argv` or was read from a file) and consumed by the
/// broker's `create_session` path to decide the session's initial
/// `SessionStatus`. A typed enum — never a bool or raw string — so a future
/// third provenance kind is a compile error at every match site, not a silent
/// fail-open. This plan does not thread `SeedProvenance` into any struct field;
/// later plans (03/04) decide where it is threaded/persisted.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SeedProvenance {
    /// The intent was supplied directly as a trusted CLI argument.
    TrustedArg,
    /// The intent content was read from a workspace file (untrusted source).
    FileDerived,
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
