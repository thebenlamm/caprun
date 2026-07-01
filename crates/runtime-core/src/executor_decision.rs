/// executor_decision.rs — ExecutorDecision enum
///
/// Returned by submit_plan_node(). Phase 1 stub returns NotImplemented.
/// Phase 4 will return Allowed/BlockedPendingConfirmation/Denied based on I2 enforcement.
/// Using a typed enum (not todo!()/panic) so the caller can match the result.

/// Typed reason an `ExecutorDecision::Denied` was returned.
///
/// This is the ONE base denial error enum for Phase 7. 07-04 EXTENDS it with
/// schema-validation variants — never introduce a second denial error type, and
/// never reintroduce a free-form `reason: String` on `Denied`. A typed taxonomy
/// gives the audit/CLI a stable, matchable set of denial codes (DESIGN
/// -durable-anchor-and-label-partition §3, §6 decision 2 — unanimous).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DenyReason {
    /// A `ValueId` did not resolve to any record in the broker-owned store
    /// (dangling/forged handle). Never becomes `Allowed`.
    DanglingHandle,
    /// A resolved record carried an empty taint vec. An empty-taint value would
    /// skip the routing-sensitivity `any(is_untrusted)` block and fail open.
    EmptyTaintInvariantViolation,
    /// A resolved record carried an empty provenance_chain — the genuine-taint
    /// anchor (`provenance_chain[0]`) is missing.
    MissingProvenanceAnchor,
}

impl DenyReason {
    /// Stable machine-readable code for audit/CLI matching.
    pub fn code(&self) -> &'static str {
        match self {
            DenyReason::DanglingHandle => "dangling_handle",
            DenyReason::EmptyTaintInvariantViolation => "empty_taint_invariant_violation",
            DenyReason::MissingProvenanceAnchor => "missing_provenance_anchor",
        }
    }
}

impl std::fmt::Display for DenyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DenyReason::DanglingHandle => "unresolvable value handle (dangling or forged)",
            DenyReason::EmptyTaintInvariantViolation => {
                "value carried empty taint (mint invariant violated)"
            }
            DenyReason::MissingProvenanceAnchor => {
                "value carried empty provenance chain (missing taint anchor)"
            }
        };
        write!(f, "{text}")
    }
}

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
    /// Execution denied — carries a typed `DenyReason` (never a free-form String).
    Denied { reason: DenyReason },
    /// Stub: executor not yet implemented (Phase 1 return value).
    NotImplemented,
}
