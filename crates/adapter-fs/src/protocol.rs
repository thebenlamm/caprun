/// protocol — IPC message types for the filesystem adapter
///
/// These types are shared between the broker (sender) and the worker (receiver)
/// to coordinate fd-passing requests and grants (REQ-adapters-fs).

/// Request an open file descriptor from the broker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RequestFd {
    /// Workspace-relative path to open.
    pub path: String,
    /// The session this request belongs to.
    pub session_id: uuid::Uuid,
}

/// Confirmation that an fd has been sent via SCM_RIGHTS.
///
/// The fd itself is delivered out-of-band as SCM_RIGHTS ancillary data;
/// this message carries the path echo for audit-log correlation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FdGranted {
    /// Path that was opened, echoed back for audit-DAG correlation.
    pub path: String,
}
