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

    // ── Phase 16 addition (BLOCKER-1 guard b, DESIGN-session-trust-state.md) ──
    /// A `CommitIrreversible` plan node was submitted while the session is in a
    /// non-live lifecycle state (`WaitingApproval`, `Done`, `Failed`, or
    /// `RolledBack`) and no per-arg I2 Block already fired. These lifecycle
    /// states are terminal or paused — a `CommitIrreversible` sink must never
    /// fall through to `Allowed` in any of them. Distinct from
    /// `DraftOnlySessionDeniesCommitIrreversible` (which covers `Draft`) so the
    /// two denial codes remain independently matchable for audit/CLI. Carries
    /// the offending `SinkId`, matching the existing convention.
    NonLiveSessionDeniesCommitIrreversible { sink: crate::plan_node::SinkId },

    // ── v1.5 addition (T2-04, DESIGN-slot-type-binding.md §5) ─────────────────
    /// A resolved value's origin-role tag did not match its slot's
    /// expected-role set (T2, DESIGN-slot-type-binding.md §5/§7). Structural
    /// fail-closed — never confirmable, never `BlockedPendingConfirmation`.
    ///
    /// Field types are a deliberate deviation from the `SinkId`-typed
    /// `DraftOnlySessionDeniesCommitIrreversible`/`NonLiveSessionDeniesCommitIrreversible`
    /// convention above: plain owned `String`/`Vec<String>`/`Option<String>`,
    /// never static string-slice references — `DenyReason` derives
    /// `Deserialize` and this decision crosses the IPC wire (worker.rs
    /// deserializes it); borrowed references are not deserializable
    /// (DESIGN F1, MAJOR).
    SlotTypeMismatch { sink: String, arg: String, expected: Vec<String>, found: Option<String> },

    // ── v1.9 Phase 42 addition (POLICY-01, DESIGN-v1.9-egress-policy §5.1/§8) ──
    /// The pre-I2 narrowing gate refused a call: the target sink is not in the
    /// session policy's allowlist, or a coarse arg constraint (allowlisted
    /// host/path/repo) rejected the literal. DISTINCT from an I2 Block — a
    /// policy-deny says "this call was never permitted"; an I2 Block says "this
    /// permitted call carried an attacker-tainted value into a sensitive arg"
    /// (§5.1, §6 row 12). The two stay independently attributable
    /// (`code()=="policy_deny"`) so LIVE-06 leg 3 can distinguish them; a
    /// policy-deny is a `Denied{reason}` outcome, NEVER a
    /// `BlockedPendingConfirmation`.
    ///
    /// Field types follow the v1.5 `SlotTypeMismatch` precedent EXACTLY: plain
    /// owned `String`/`Option<String>`, never borrowed `&'static str` — this
    /// enum derives `Deserialize` and the decision crosses the IPC wire
    /// (worker.rs deserializes it); borrowed references are not deserializable
    /// (DESIGN F1, MAJOR). Field semantics: `sink` = the offending sink id;
    /// `arg` = the offending arg name when arg-scoped, `None` for a sink-level
    /// deny; `constraint` = a short machine-readable tag of which rule refused
    /// (e.g. `sink-not-allowed`, `arg-not-allowlisted`).
    PolicyDeny { sink: String, arg: Option<String>, constraint: String },
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
            DenyReason::NonLiveSessionDeniesCommitIrreversible { .. } => {
                "non_live_session_denies_commit_irreversible"
            }
            DenyReason::SlotTypeMismatch { .. } => "slot_type_mismatch",
            DenyReason::PolicyDeny { .. } => "policy_deny",
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
            DenyReason::NonLiveSessionDeniesCommitIrreversible { sink } => write!(
                f,
                "non-live session (WaitingApproval/Done/Failed/RolledBack) denies \
                 CommitIrreversible sink `{sink}`",
                sink = sink.0
            ),
            DenyReason::SlotTypeMismatch {
                sink,
                arg,
                expected,
                found,
            } => write!(
                f,
                "value routed into `{arg}` of sink `{sink}` has role {found:?}, expected one of {expected:?}"
            ),
            DenyReason::PolicyDeny {
                sink,
                arg,
                constraint,
            } => match arg {
                Some(arg) => write!(
                    f,
                    "policy denies arg `{arg}` of sink `{sink}` ({constraint})"
                ),
                None => write!(f, "policy denies sink `{sink}` ({constraint})"),
            },
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

/// One blocked argument in a `BlockedPendingConfirmation` set (Phase 14, D-14).
///
/// Every field is an EXACT CLONE of the resolved broker-owned `ValueRecord` (plus
/// the broker-minted `effect_id`/`sink`/`arg` folded into `anchor`) — the executor
/// constructs NOTHING itself and NEVER sets a taint field. This is the T-04-03
/// anti-stapling invariant, preserved PER-ELEMENT in this plural shape: a
/// collect-then-Block loop that pushes N of these elements must not staple taint
/// or literal onto any of them — each one is independently a verbatim clone of
/// its own resolved record.
///
/// Phase 16 (`planning-docs/DESIGN-confirm-binding.md`, CONFIRM-03/D-19) layers a
/// `combined_digest` SHA-256-over-fixed-width-per-element-digests binding on top
/// of this collection — that field does NOT live here; `BlockedArg` stays exactly
/// this shape. The combined digest's tamper-evident source of truth is the hashed
/// `sink_blocked` Event payload (Round 5 reconciliation amendment,
/// `DESIGN-GATE-RECORD-v1.3.md`) — it is only MIRRORED onto `PendingConfirmation`
/// for the confirm process to read; that mirror copy is not itself hash-chained.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockedArg {
    /// The durable, tamper-evident per-element anchor (unchanged shape/semantics
    /// from the pre-Phase-14 singular `SinkBlockedAnchor` — only its container
    /// became plural).
    pub anchor: SinkBlockedAnchor,
    /// The LIVE byte-exact literal for this blocked arg, carried in-memory for the
    /// confirmation UX / redactable `blocked_literals` side-table write. NOT part
    /// of the hashed anchor — only `anchor.literal_sha256` enters the tamper-evident
    /// chain. DATA, never executed.
    pub literal: String,
}

/// The decision the executor returns after evaluating a PlanNode.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExecutorDecision {
    /// Plan executed and all taint checks passed.
    Allowed,
    /// Execution blocked — one or more sensitive sink arguments carried tainted
    /// values; confirmation required for the WHOLE set.
    ///
    /// Plural (Phase 14, D-14 Collect-then-Block): the per-arg loop scans EVERY
    /// arg on the plan node before returning, collecting every
    /// routing-sensitive-OR-content-sensitive AND tainted arg into `anchors` —
    /// never returning on the first match. A plan node with both a tainted `to`
    /// and a tainted `body` surfaces BOTH in this one collection; neither
    /// silently pre-empts the other (closes the B1-reincarnation risk,
    /// `planning-docs/DESIGN-content-adapter-mediation.md` "Precedence").
    ///
    /// A held-out §9 test asserts the unbroken taint chain DIRECTLY from each
    /// element's `anchor.provenance_chain[0]` (the file_read Event id) with no
    /// second query — independently, per blocked arg (D-16).
    BlockedPendingConfirmation { anchors: Vec<BlockedArg> },
    /// Execution denied — carries a typed `DenyReason` (never a free-form String).
    Denied { reason: DenyReason },
    /// Stub: executor not yet implemented (Phase 1 return value).
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_type_mismatch_code_and_display() {
        let reason = DenyReason::SlotTypeMismatch {
            sink: "email.send".to_string(),
            arg: "to".to_string(),
            expected: vec!["recipient".to_string(), "email_address".to_string()],
            found: Some("body".to_string()),
        };

        assert_eq!(reason.code(), "slot_type_mismatch");

        let rendered = reason.to_string();
        assert!(!rendered.is_empty());
        assert!(rendered.contains("email.send"));
        assert!(rendered.contains("to"));
    }

    #[test]
    fn policy_deny_code_and_display() {
        // Sink-level deny (arg = None).
        let sink_deny = DenyReason::PolicyDeny {
            sink: "git.push".to_string(),
            arg: None,
            constraint: "sink-not-allowed".to_string(),
        };
        assert_eq!(sink_deny.code(), "policy_deny");
        let rendered = sink_deny.to_string();
        assert!(!rendered.is_empty());
        assert!(rendered.contains("git.push"));

        // Arg-scoped deny names the arg too.
        let arg_deny = DenyReason::PolicyDeny {
            sink: "http.request".to_string(),
            arg: Some("url".to_string()),
            constraint: "arg-not-allowlisted".to_string(),
        };
        assert_eq!(arg_deny.code(), "policy_deny");
        let rendered = arg_deny.to_string();
        assert!(rendered.contains("http.request"));
        assert!(rendered.contains("url"));

        // A PolicyDeny is a Denied outcome, never a BlockedPendingConfirmation.
        let decision = ExecutorDecision::Denied { reason: arg_deny };
        assert!(matches!(decision, ExecutorDecision::Denied { .. }));
        assert!(!matches!(
            decision,
            ExecutorDecision::BlockedPendingConfirmation { .. }
        ));
    }
}
