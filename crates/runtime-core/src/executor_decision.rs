/// executor_decision.rs — ExecutorDecision enum
///
/// Returned by submit_plan_node(). Phase 1 stub returns NotImplemented.
/// Phase 4 will return Allowed/BlockedPendingConfirmation/Denied based on I2 enforcement.
/// Using a typed enum (not todo!()/panic) so the caller can match the result.

/// The decision the executor returns after evaluating a PlanNode.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExecutorDecision {
    /// Plan executed and all taint checks passed.
    Allowed,
    /// Execution blocked — tainted value in sensitive sink argument; confirmation required.
    ///
    /// Carries the literal_value, sink, and arg_name read from the broker-owned
    /// ValueRecord, plus the `taint` labels and the ordered `provenance_chain`.
    /// A held-out §9 test can assert the unbroken taint chain DIRECTLY from this
    /// payload (no second query): `provenance_chain[0]` equals the file_read Event id.
    BlockedPendingConfirmation {
        literal_value: String,
        sink: String,
        arg_name: String,
        /// Taint labels carried by the blocked value (from its ValueRecord).
        taint: Vec<crate::plan_node::TaintLabel>,
        /// Ordered derivation edges; `provenance_chain[0]` is the file_read Event id.
        provenance_chain: Vec<uuid::Uuid>,
    },
    /// Execution denied by policy.
    Denied { reason: String },
    /// Stub: executor not yet implemented (Phase 1 return value).
    NotImplemented,
}
