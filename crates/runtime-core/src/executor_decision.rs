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

    // ── Schema-validation variants (07-04a, HARD-01/HARD-05) ──────────────────
    // These are raised by `executor::sink_schema::validate_schema`, the FIRST
    // step of `submit_plan_node`, BEFORE any resolve/sensitivity work. Unknown
    // sink or malformed arg set fails closed here (single denial taxonomy — no
    // second error type). The `String` carries the offending name for audit/CLI.
    /// The plan node's `sink` is not in the hardcoded `KNOWN_SINKS` registry.
    /// Fails closed — an unregistered sink is never callable.
    UnknownSink(String),
    /// A plan-node arg name is not in the target sink's allowed arg set.
    UnknownArg(String),
    /// The same arg name appears more than once in the plan node.
    DuplicateArg(String),
    /// A required arg of the target sink is absent from the plan node.
    MissingArg(String),

    // ── v1.2 addition (TAINT-02, DESIGN-session-trust-state.md §7) ────────────
    /// A `CommitIrreversible` plan node was submitted while the session is
    /// `SessionStatus::Draft` and no per-arg I2 Block already fired. Carries the
    /// offending `SinkId` (RESEARCH Open Question 3), matching the existing
    /// `UnknownSink(String)` convention of carrying the offending identity for
    /// audit/CLI legibility. This is an append to the ONE denial taxonomy above —
    /// never a second, parallel denial error type.
    DraftOnlySessionDeniesCommitIrreversible { sink: crate::plan_node::SinkId },
}

impl DenyReason {
    /// Stable machine-readable code for audit/CLI matching.
    pub fn code(&self) -> &'static str {
        match self {
            DenyReason::DanglingHandle => "dangling_handle",
            DenyReason::EmptyTaintInvariantViolation => "empty_taint_invariant_violation",
            DenyReason::MissingProvenanceAnchor => "missing_provenance_anchor",
            DenyReason::UnknownSink(_) => "unknown_sink",
            DenyReason::UnknownArg(_) => "unknown_arg",
            DenyReason::DuplicateArg(_) => "duplicate_arg",
            DenyReason::MissingArg(_) => "missing_arg",
            DenyReason::DraftOnlySessionDeniesCommitIrreversible { .. } => {
                "draft_only_session_denies_commit_irreversible"
            }
        }
    }
}

impl std::fmt::Display for DenyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DenyReason::DanglingHandle => {
                write!(f, "unresolvable value handle (dangling or forged)")
            }
            DenyReason::EmptyTaintInvariantViolation => {
                write!(f, "value carried empty taint (mint invariant violated)")
            }
            DenyReason::MissingProvenanceAnchor => {
                write!(f, "value carried empty provenance chain (missing taint anchor)")
            }
            DenyReason::UnknownSink(sink) => write!(f, "unknown sink `{sink}` (not registered)"),
            DenyReason::UnknownArg(arg) => write!(f, "unknown arg `{arg}` for sink"),
            DenyReason::DuplicateArg(arg) => write!(f, "duplicate arg `{arg}` in plan node"),
            DenyReason::MissingArg(arg) => write!(f, "missing required arg `{arg}` for sink"),
            DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink } => write!(
                f,
                "draft-only session denies CommitIrreversible sink `{sink}`",
                sink = sink.0
            ),
        }
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
    /// SHA-256 digest (lowercase hex) of the byte-exact literal — the DURABLE,
    /// tamper-evident representation. Only the digest rides inside the hashed
    /// `payload` column; the raw literal is NEVER hashed into the chain, so it can
    /// be redacted (delete its `blocked_literals` side-table row) without breaking
    /// `verify_chain`. A swapped side-table literal no longer matches this digest,
    /// so tamper-evidence is preserved while redactability is gained.
    pub literal_sha256: String,
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
    /// ValueRecord verbatim; its `literal_sha256` is the digest of the blocked
    /// literal). A held-out §9 test asserts the unbroken taint chain DIRECTLY from
    /// `anchor.provenance_chain[0]` (the file_read Event id) with no second query.
    ///
    /// `literal` is the LIVE byte-exact literal, carried in-memory for the
    /// confirmation UX and as the source the broker writes to the redactable
    /// `blocked_literals` side table. It is NOT part of the hashed anchor, so it
    /// never enters the tamper-evident chain — only its digest (`literal_sha256`)
    /// does. DATA, never executed.
    BlockedPendingConfirmation {
        anchor: SinkBlockedAnchor,
        literal: String,
    },
    /// Execution denied — carries a typed `DenyReason` (never a free-form String).
    Denied { reason: DenyReason },
    /// Stub: executor not yet implemented (Phase 1 return value).
    NotImplemented,
}
