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
/// Phase 5 shipped `EmailAddress`; Phase 7 (07-04b) adds `RelativePath` so a
/// workspace-derived path can cross the IPC boundary and be minted
/// `[ExternalUntrusted, PathRaw]` by the broker (never `LocalWorkspace`).
///
/// Phase 15 (15-03) adds `DocFragment`, additively — no existing variant changes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim {
    /// An email address extracted by the quarantine extractor.
    /// Carries ONLY the address string — never the raw surrounding sentence.
    EmailAddress(String),
    /// A root-relative path string extracted from untrusted workspace content.
    /// Carries ONLY the path token — never the raw surrounding sentence. The
    /// broker mints it `[ExternalUntrusted, PathRaw]` (routing-sensitive on
    /// `file.create/path` → Block); the worker cannot launder it to a trusted label.
    RelativePath(String),
    /// A raw doc fragment extracted by the quarantine extractor — e.g. one half
    /// of a `Reply-To:`/`Domain:` pair the worker will concat-transform into a
    /// recipient BEFORE reporting the result via `ReportDerivedClaim`. Carries
    /// ONLY the fragment token — never the raw surrounding sentence. The broker
    /// mints it `[ExternalUntrusted]` via `mint_from_read`'s `doc_fragment` arm,
    /// which fails closed (`quarantine::looks_like_doc_fragment`) if the value
    /// already contains `'@'` — the concat transform's OWN OUTPUT can never
    /// re-enter here (finding #1a/#1c); the worker cannot launder an assembled
    /// recipient as a fresh raw fragment.
    DocFragment(String),
}

/// A tag identifying the deterministic transform the confined worker applied
/// worker-side to produce a `ReportDerivedClaim`'s `transformed_literal`.
///
/// Additive; Phase 15 defines only `Concat` (a fixed `'@'`-join over doc
/// fragments — see `quarantine::concat_doc_fragments`). A future phase adding
/// a different transform (e.g. a differently-delimited join, or a base64
/// decode) MUST introduce its own distinct variant — never reuse `Concat` for
/// a different separator: `mint_from_derivation`'s byte-verify guard is
/// separator-specific (`join(input_literals, '@')`), and reusing the tag for a
/// different join would either false-reject a legitimate derivation or,
/// worse, false-accept a mismatched one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransformKind {
    /// Two doc-fragment claims concatenated with a literal `'@'` separator
    /// (`quarantine::concat_doc_fragments`) — the only transform Phase 15 ships.
    Concat,
}

impl TransformKind {
    /// The `&str` tag `quarantine::mint_from_derivation`'s `transform_kind`
    /// argument matches on. Kept as a single explicit, exhaustively-matched
    /// method (not a `From`/`Display` impl) so the wire-tag ↔ mint-tag mapping
    /// has exactly one call site.
    pub fn as_mint_tag(&self) -> &'static str {
        match self {
            TransformKind::Concat => "concat",
        }
    }
}

/// Request from a worker to the broker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerRequest {
    /// Create a new broker session for the given intent.
    CreateSession { intent_id: uuid::Uuid },
    /// Worker declares the user's typed intent. Broker calls `mint_from_intent` and
    /// returns an opaque `UserTrusted` ValueId handle.
    ///
    /// Sent BEFORE `RequestFd` (matches the ordering invariant in worker.rs:
    /// connect → set_nonblocking → apply_confinement → ProvideIntent → RequestFd).
    ///
    /// SECURITY CONTRACT: The literal flows from the trusted orchestrator env var;
    /// the broker mints the ValueRecord authoritatively — the worker NEVER constructs
    /// a ValueRecord or sets taint. The per-connection ValueStore ensures the returned
    /// ValueId resolves only within this session's executor evaluation (HARD-03 / Pitfall 1).
    ///
    /// `primary_file_derived` (M7 / WG-1) is the PER-LITERAL provenance of the
    /// PRIMARY intent literal (the recipient for `SendEmailSummary`, the path for
    /// `CreateFileFromReport`): `true` iff that literal was read from external
    /// file/stream content (`--seed-from-file`). The broker's ProvideIntent arm
    /// mints a file-derived primary literal via `mint_from_read` (TAINTED,
    /// `ExternalUntrusted` + a real `file_read` event + session-demote) instead
    /// of `mint_from_intent` (trusted) — closing the laundering path where file
    /// content minted `UserTrusted` escapes the I2 value-injection Block. Session
    /// status alone cannot carry this: it is per-session, not per-literal, and a
    /// file-derived recipient must Block even when an operator-typed literal in
    /// the same session would be Allowed. Subject/body defaults stay operator-
    /// trusted (`mint_from_intent`), DISJOINT from file/env ingestion.
    ///
    /// A plain required field (deliberately NOT `#[serde(default)]`, mirroring
    /// `PlanNodeDecision.output_value_id`'s Pitfall-8 discipline): every
    /// construction/destructure site must explicitly acknowledge the provenance
    /// signal, so a future caller cannot silently forget to forward it and
    /// re-open the laundering path.
    ProvideIntent {
        intent: runtime_core::intent::CaprunIntent,
        primary_file_derived: bool,
    },
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
    /// Worker reports a transform-derived claim: the already-transformed
    /// literal plus the handles of the raw inputs it was derived from
    /// (RESEARCH.md Open Question 2, option (a) — a separate message
    /// referencing `ReportClaims`'s returned `value_ids`, rather than
    /// inlining raw inputs in one message).
    ///
    /// The broker resolves each `input_value_ids` handle to its broker-owned
    /// `ValueRecord` and mints the derived value via `mint_from_derivation`
    /// (Plan 01) — provenance-threading from the inputs' OWN read-rooted
    /// chains, NEVER a fresh transform-local root. The broker NEVER
    /// re-applies the transform itself; `mint_from_derivation`'s own
    /// byte-verify guard checks `transformed_literal` against the resolved
    /// inputs' literals (MAJOR-1).
    ///
    /// SECURITY CONTRACT (HARD-03): this message carries NO `session_id` —
    /// same contract as `SubmitPlanNode`/`ReportClaims`; the broker evaluates
    /// against the connection-established session identity, never a
    /// message-supplied one (spoofing defense T-05-03).
    ReportDerivedClaim {
        /// The worker's claimed already-transformed literal (e.g. the
        /// concatenated recipient). Byte-verified broker-side against
        /// `join(input_literals, '@')` for the `Concat` transform — never
        /// trusted at face value (MAJOR-1).
        transformed_literal: String,
        /// The transform tag applied worker-side to produce `transformed_literal`.
        transform: TransformKind,
        /// The handles of the inputs `transformed_literal` was derived from,
        /// in the order the worker applied the transform. Each MUST resolve
        /// within THIS connection's `ValueStore`; any unresolved handle fails
        /// closed (`Error`, mints nothing — Pitfall 1).
        input_value_ids: Vec<runtime_core::plan_node::ValueId>,
    },
    /// Phase 20 (PLANNER-02/04) establishment handshake: a connection sends
    /// this as its FIRST framed message to request planner-role capabilities.
    ///
    /// Only meaningful at connection establishment — it carries no
    /// operational effect and mints nothing. `crates/brokerd/src/server.rs`'s
    /// accept-loop classification (`DESIGN-session-trust-coherence.md` §3)
    /// reads a SUBSEQUENT connection's first frame; if it deserializes to
    /// this variant AND the one-way planner slot is still free, that
    /// connection is admitted as the session's single capability-restricted
    /// planner connection (`ConnectionRole::Planner`), which `permits` only
    /// `SubmitPlanNode` — never `ProvideIntent`/`ReportClaims`/
    /// `ReportDerivedClaim`/`CreateSession` (no mint verb) and never
    /// `RequestFd`/`ReportRead` (no raw-bytes fd).
    ///
    /// Additive: existing clients (the worker, `uds_ipc` tests, Phase 19's
    /// regression tests) never send it, so they are classified as the
    /// worker-role first connection exactly as before — no behavior change
    /// to the existing exhaustive serde derive or any prior round-trip.
    /// Sending it AGAIN mid-stream on an already-classified connection is
    /// itself a non-permitted verb for a planner connection (denied by
    /// `ConnectionRole::permits`) — role is decided ONCE, never re-derived
    /// per-message.
    DeclarePlannerRole,
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
    /// Acknowledgement for `ProvideIntent`: opaque handle for the minted `UserTrusted`
    /// ValueRecord. Mirrors `ClaimsReceived` but singular per literal.
    ///
    /// The `value_id` resolves ONLY within the per-connection ValueStore created for
    /// this session; using it in a different connection yields `Denied` (HARD-03 / Pitfall 1).
    ///
    /// `subject_value_id`/`body_value_id` are ADDITIVE (Phase 15, 15-04,
    /// finding #6): for a `SendEmailSummary` intent the broker mints THREE
    /// DISTINCT `UserTrusted` handles (recipient in `value_id`, subject and
    /// body here) via three sequential `mint_from_intent` calls — never
    /// degenerately reusing `value_id` for all three. For `CreateFileFromReport`
    /// (which has no subject/body fields) both are `None`.
    IntentAccepted {
        value_id: runtime_core::plan_node::ValueId,
        subject_value_id: Option<runtime_core::plan_node::ValueId>,
        body_value_id: Option<runtime_core::plan_node::ValueId>,
    },
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
    ///
    /// Sent ONLY on a `ConnectionRole::Worker` connection's `SubmitPlanNode`
    /// (`crates/brokerd/src/server.rs`). The worker already holds every
    /// literal in its own `ValueStore` (the confirmation UX depends on this,
    /// `DESIGN-session-trust-coherence.md` §7/§9 residual #3) — never sent to
    /// a `ConnectionRole::Planner` connection, which receives
    /// `PlanNodeDecisionReduced` instead.
    ///
    /// `output_value_id` (32-05, EXEC-02/EXEC-03 wiring): the opaque `ValueId`
    /// handle to a minted `process.exec` output, `Some(..)` ONLY when
    /// `plan_node.sink.0 == "process.exec"` AND `decision` is `Allowed` —
    /// `None` for every other sink/decision (zero behavior change for
    /// `file.create`/`email.send`). A plain required field, deliberately NOT
    /// `#[serde(default)]` (Pitfall 8): every construction/destructure site
    /// must explicitly acknowledge it, so a future new sink cannot silently
    /// forget to populate/consume the handle. The worker learns only this
    /// opaque handle, never the raw captured bytes (I1).
    PlanNodeDecision {
        decision: runtime_core::ExecutorDecision,
        output_value_id: Option<runtime_core::plan_node::ValueId>,
    },
    /// Phase 20 (PLANNER-04) reduced decision signal: the ONLY decision shape
    /// a `ConnectionRole::Planner` connection ever receives for
    /// `SubmitPlanNode` (`DESIGN-session-trust-coherence.md` §7's ruling,
    /// closing the decision-oracle for the planner connection).
    ///
    /// `blocked` is a straight projection of the full `ExecutorDecision`:
    /// `Allowed` -> `false`; `BlockedPendingConfirmation { .. }`, `Denied
    /// { .. }`, and `NotImplemented` (every non-`Allowed` outcome) -> `true`.
    ///
    /// Deliberately carries NO `anchors`, NO `literal_sha256` (the offline
    /// literal-guess confirmer `DESIGN-session-trust-coherence.md` §7 names),
    /// and NO plaintext `literal` — the planner learns only enough to decide
    /// whether to proceed or stop for the turn, consistent with PLANNER-04's
    /// "typed extracts + handle IDs only, never literals" boundary. The
    /// broker still durably records the full evaluation event in the audit
    /// DAG exactly as the worker path does — only this RESPONSE is reduced.
    PlanNodeDecisionReduced { blocked: bool },
    /// Acknowledgement for `ReportDerivedClaim`: the opaque handle to the
    /// minted derived `ValueRecord`. Resolves ONLY within the per-connection
    /// `ValueStore` (same HARD-03 / Pitfall 1 contract as `ClaimsReceived`/
    /// `IntentAccepted`).
    DerivedClaimReceived {
        value_id: runtime_core::plan_node::ValueId,
    },
}

// -----------------------------------------------------------------------
// Phase 15 (15-03) GREEN: serde round-trip tests for the additive
// DocFragment/TransformKind/ReportDerivedClaim/DerivedClaimReceived wire
// types added above.
// -----------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::plan_node::ValueId;

    /// `WorkerClaim::DocFragment` round-trips through serde_json to an equal
    /// value (additive variant — the existing `EmailAddress`/`RelativePath`
    /// tests already prove the pre-existing variants are untouched).
    #[test]
    fn doc_fragment_claim_round_trips() {
        let claim = WorkerClaim::DocFragment("accounts".to_string());
        let json = serde_json::to_value(&claim).expect("serialize DocFragment claim");
        let recovered: WorkerClaim =
            serde_json::from_value(json).expect("deserialize DocFragment claim");
        assert_eq!(claim, recovered);
    }

    /// `TransformKind::Concat` round-trips through serde_json to an equal value.
    #[test]
    fn transform_kind_concat_round_trips() {
        let kind = TransformKind::Concat;
        let json = serde_json::to_value(&kind).expect("serialize TransformKind");
        let recovered: TransformKind =
            serde_json::from_value(json).expect("deserialize TransformKind");
        assert_eq!(kind, recovered);
        assert_eq!(kind.as_mint_tag(), "concat");
    }

    /// `BrokerRequest::ReportDerivedClaim` round-trips through serde_json to
    /// an equal value.
    #[test]
    fn report_derived_claim_request_round_trips() {
        let req = BrokerRequest::ReportDerivedClaim {
            transformed_literal: "accounts@ev1l.com".to_string(),
            transform: TransformKind::Concat,
            input_value_ids: vec![ValueId::new(), ValueId::new()],
        };
        let json = serde_json::to_value(&req).expect("serialize ReportDerivedClaim request");
        let recovered: BrokerRequest =
            serde_json::from_value(json).expect("deserialize ReportDerivedClaim request");
        assert_eq!(req, recovered);
    }

    /// `BrokerResponse::DerivedClaimReceived` round-trips through serde_json
    /// to an equal value.
    #[test]
    fn derived_claim_received_response_round_trips() {
        let resp = BrokerResponse::DerivedClaimReceived {
            value_id: ValueId::new(),
        };
        let json = serde_json::to_value(&resp).expect("serialize DerivedClaimReceived response");
        let recovered: BrokerResponse =
            serde_json::from_value(json).expect("deserialize DerivedClaimReceived response");
        assert_eq!(resp, recovered);
    }
}
