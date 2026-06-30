/// proto — IPC message types for the broker ↔ worker protocol
///
/// Wire format: JSON via serde_json, with a 4-byte LE length prefix.
/// These types are shared between brokerd (server) and workers (clients).
/// See RESEARCH.md Pattern 4 for the framing protocol.

/// A typed, lossy claim extracted by a confined worker from file contents.
///
/// SECURITY CONTRACT (ASM-03 / I2):
/// - Raw source bytes NEVER appear here — only the extracted typed value crosses
///   the IPC boundary. The surrounding hostile sentence is discarded inside the
///   confined worker before this message is constructed.
/// - Unknown `kind` values fail closed: the exhaustive enum (no wildcard / other-arm)
///   causes serde to return a deserialize error for any unrecognized tag, so the
///   broker never silently coerces an unknown claim kind to a known one.
///
/// Phase 5 ships one variant: `EmailAddress`.
/// `RelativePath(String)` is planned for Phase 7 — do not implement here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim {
    /// An email address extracted by the quarantine extractor.
    /// Carries ONLY the address string — never the raw surrounding sentence.
    EmailAddress(String),
    // RelativePath(String),  // Phase 7
}

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
    /// Worker extracted typed claims from a file read. Raw bytes NOT included.
    ///
    /// The broker mints a ValueRecord per claim via `mint_from_read` and returns
    /// opaque `ValueId` handles. Raw source bytes are never included in this
    /// message — only the extracted typed values cross the IPC boundary.
    ReportClaims { claims: Vec<WorkerClaim> },
    /// Submit a PlanNode for executor evaluation.
    ///
    /// The broker resolves each PlanArg handle to the broker-owned ValueRecord
    /// (literal + taint + provenance_chain) and evaluates taint policy.
    /// Closes RESEARCH Gap 3: surfaces the Block data (literal_value, taint,
    /// provenance_chain) to the broker-side confirmation-prompt builder.
    ///
    /// SECURITY CONTRACT (HARD-03): this message carries NO `session_id`. The
    /// broker evaluates against the connection-established session identity
    /// threaded through `handle_connection` — it NEVER trusts a session_id
    /// supplied in the IPC message (spoofing defense T-05-03).
    SubmitPlanNode {
        plan_node: runtime_core::PlanNode,
    },
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
    /// Acknowledgement for ReportClaims: opaque ValueId handles per minted claim,
    /// in the same order as the claims submitted in the ReportClaims message.
    ClaimsReceived {
        value_ids: Vec<runtime_core::plan_node::ValueId>,
    },
    /// Decision returned after evaluating a SubmitPlanNode request.
    ///
    /// When `decision` is `ExecutorDecision::BlockedPendingConfirmation { .. }`,
    /// the broker constructs a `ConfirmationPrompt` from the Block payload and
    /// delivers it to the human via FAMP before proceeding.
    PlanNodeDecision { decision: runtime_core::ExecutorDecision },
}
