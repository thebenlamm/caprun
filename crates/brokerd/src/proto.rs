/// proto — IPC message types for the broker ↔ worker protocol
///
/// Wire format: JSON via serde_json, with a 4-byte LE length prefix.
/// These types are shared between brokerd (server) and workers (clients).
/// See RESEARCH.md Pattern 4 for the framing protocol.

/// Request from a worker to the broker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerRequest {
    /// Create a new broker session for the given intent.
    CreateSession { intent_id: uuid::Uuid },
    /// Request an open file descriptor for `path`.
    /// The broker opens the file and delivers the fd via SCM_RIGHTS.
    RequestFd { path: String },
    /// Report that the worker read `bytes_read` bytes via a previously
    /// granted fd. Appended to the audit DAG as a file_read event.
    ReportRead { bytes_read: u64 },
}

/// Response from the broker to a worker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerResponse {
    /// Session created; the session_id identifies this worker's audit chain.
    SessionCreated { session_id: uuid::Uuid },
    /// The requested fd has been sent via SCM_RIGHTS out-of-band.
    FdGranted,
    /// Generic acknowledgement for ReportRead and other fire-and-forget messages.
    Ack,
    /// The broker encountered an error; the worker should log and exit.
    Error { message: String },
}
