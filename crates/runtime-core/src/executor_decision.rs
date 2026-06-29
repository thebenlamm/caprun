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
    BlockedPendingConfirmation {
        literal_value: String,
        sink: String,
        arg_name: String,
    },
    /// Execution denied by policy.
    Denied { reason: String },
    /// Stub: executor not yet implemented (Phase 1 return value).
    NotImplemented,
}
