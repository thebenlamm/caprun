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

/// The durable genuine-taint anchor for a `sink_blocked` decision (ACC-07).
///
/// Every field is an EXACT CLONE of the resolved broker-owned `ValueRecord`
/// (plus the broker-minted `effect_id` and the `sink`/`arg` read from the
/// `PlanNode`/`PlanArg`). The executor constructs NOTHING itself and NEVER sets a
/// taint field — this is the T-04-03 anti-stapling invariant. A DB reader
/// re-derives untrusted-ness by calling `TaintLabel::is_untrusted()` on
/// `anchor.taint`; NO precomputed trust boolean is persisted
/// (DESIGN-durable-anchor-and-label-partition §2, §4).
///
/// This anchor rides inside the hashed `payload` column of the audit event, so it
/// is tamper-evident for free (`compute_event_hash` covers `payload`) with no DB
/// migration (DESIGN §5).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SinkBlockedAnchor {
    /// BROKER-minted effect identity, passed into `submit_plan_node` — keeps the
    /// executor a pure function (DESIGN §4 rule 2).
    pub effect_id: uuid::Uuid,
    /// The sink the blocked plan node targeted (`plan_node.sink`).
    pub sink: crate::plan_node::SinkId,
    /// The routing-sensitive argument name (`PlanArg.name`); `String`, no newtype.
    pub arg: String,
    /// The opaque handle for the blocked value (`record.id`).
    pub value_id: crate::plan_node::ValueId,
    /// Byte-exact literal. DATA AT REST, never executed.
    pub literal: String,
    /// Verbatim clone of the record's taint labels.
    pub taint: Vec<crate::plan_node::TaintLabel>,
    /// Verbatim clone of the record's provenance chain; `[0]` is the root read Event id.
    pub provenance_chain: Vec<uuid::Uuid>,
    /// The root read Event id — equals `provenance_chain[0]` (anchor-internal invariant).
    pub read_event_id: uuid::Uuid,
}

/// The decision the executor returns after evaluating a PlanNode.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExecutorDecision {
    /// Plan executed and all taint checks passed.
    Allowed,
    /// Execution blocked — tainted value in sensitive sink argument; confirmation required.
    ///
    /// Carries the durable `SinkBlockedAnchor` (built by cloning the resolved
    /// ValueRecord verbatim). A held-out §9 test asserts the unbroken taint chain
    /// DIRECTLY from `anchor.provenance_chain[0]` (the file_read Event id) with no
    /// second query.
    BlockedPendingConfirmation { anchor: SinkBlockedAnchor },
    /// Execution denied — carries a typed `DenyReason` (never a free-form String).
    Denied { reason: DenyReason },
    /// Stub: executor not yet implemented (Phase 1 return value).
    NotImplemented,
}
