/// event.rs — Event (audit DAG node)
///
/// Each Event is a node in the causal audit DAG. The taint field propagates
/// TaintLabel from raw reads through to ValueNode arguments at sinks — this
/// chain is what I2 enforcement verifies in Phase 4.
/// Event.taint reuses plan_node::TaintLabel — no duplicate definition.

use crate::executor_decision::SinkBlockedAnchor;
use crate::plan_node::ValueId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A single node in the audit Directed Acyclic Graph.
///
/// parent_id links to the causal predecessor event, enabling the runtime
/// to verify that taint propagated through genuine reads (not stapled on at
/// the sink — CON-s9-taint-genuineness).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub id: Uuid,
    /// Causal predecessor — None for session-root events.
    pub parent_id: Option<Uuid>,
    pub session_id: Uuid,
    /// Principal responsible for this event.
    pub actor: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    /// Taint labels from the data sources read during this event.
    /// Reuses TaintLabel from plan_node — same type, no duplicate.
    pub taint: Vec<crate::plan_node::TaintLabel>,
    /// The durable genuine-taint anchors for a `sink_blocked` event (ACC-07).
    ///
    /// Empty for every event type except `sink_blocked`; a `sink_blocked`
    /// event with an EMPTY `anchors` collection is non-persistable through the
    /// TCB (`audit::append_event` rejects it). `#[serde(skip_serializing_if)]`
    /// keeps pre-anchor events byte-identical (no DB migration — DESIGN §5);
    /// every anchor rides inside the hashed `payload` column, so each is
    /// tamper-evident.
    ///
    /// Plural (Phase 14, D-14 Collect-then-Block): a `sink_blocked` event
    /// carries ALL blocked anchors for the plan node in one event — not just
    /// the first match — so every blocked arg is durably recorded in the
    /// hash-chained audit DAG.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<SinkBlockedAnchor>,

    /// The value-lineage derivation payload for a `derivation` event (Phase 15,
    /// D-16 "Provenance-Threading for Transform-Derived Mints"). Empty/`None`
    /// for every event type except `derivation`. `#[serde(default,
    /// skip_serializing_if)]` on all four fields keeps every pre-derivation
    /// event byte-identical (no DB migration) — the golden byte-fixture test
    /// below proves this.
    ///
    /// The multi-input value-lineage edge V<-{A,B} rides ONLY in this hashed
    /// payload — NEVER in `parent_id` (which stays single-parent, modeling
    /// only the causal/temporal chain) and NEVER as an element inside any
    /// `provenance_chain` (two-graphs-never-share-edges; finding #10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_value_id: Option<ValueId>,
    /// The derived value's input `ValueId`s, in input order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_value_ids: Vec<ValueId>,
    /// Each input's own `provenance_chain`, in input order — the DB-alone
    /// checkable proof that `derived_value_id`'s ancestry threads back to
    /// these originating reads (EXTRACT-02 consumes this).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_provenance_chains: Vec<Vec<Uuid>>,
    /// A tag identifying the transform that produced the derived value (e.g.
    /// `"concat"`). NOTE (opus round-4, non-blocking): `"concat"`'s `@`
    /// separator is RECIPIENT-SCOPED — a Phase-15 artifact of assembling an
    /// email address specifically, not a generic join-any-strings primitive.
    /// A future phase reusing the `"concat"` tag for a non-`@` join without
    /// introducing its own distinct tag would cause `mint_from_derivation`'s
    /// byte-verify guard to false-reject a legitimate derivation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_kind: Option<String>,

    /// The CONFIRM-03 combined SHA-256 digest over the FULL current
    /// `resolved_args` set (Phase 16, Round-6 amendment,
    /// `planning-docs/DESIGN-confirm-binding.md`), computed ONCE at Block
    /// time by `crate::confirmation::combined_digest` over every arg's
    /// `(arg_name, literal)` pair — blocked AND trusted args together, not
    /// only the blocked subset. Empty/`None` for every event type except
    /// `sink_blocked`. `#[serde(default, skip_serializing_if)]` keeps every
    /// pre-Phase-16 event byte-identical (no DB migration — mirrors the
    /// `derived_value_id` precedent above), and this field rides inside the
    /// hashed `payload` column, so it is tamper-evident: any post-hoc edit
    /// to it or to any arg literal breaks the audit-DAG hash chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combined_digest: Option<String>,
    /// The ordered subset of `resolved_args` names that were BLOCKED (as
    /// opposed to merely TRUSTED-but-bound) — DISPLAY-MARKING metadata for
    /// Plan 16-02's per-arg narration only. It does NOT select
    /// `combined_digest`'s domain (that is every current `resolved_args`
    /// element, per the Round-6 amendment). Empty for every event type
    /// except `sink_blocked`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_arg_names: Vec<String>,
}

impl Event {
    /// Construct an ordinary (non-`sink_blocked`) audit event. Sets `anchors: vec![]`.
    ///
    /// This is the ONLY sanctioned way to build a non-block event — every literal
    /// `Event { .. }` site migrated to it, so adding a future field can never
    /// silently drop `anchors`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        parent_id: Option<Uuid>,
        session_id: Uuid,
        actor: String,
        event_type: String,
        timestamp: DateTime<Utc>,
        taint: Vec<crate::plan_node::TaintLabel>,
    ) -> Self {
        Event {
            id,
            parent_id,
            session_id,
            actor,
            event_type,
            timestamp,
            taint,
            anchors: vec![],
            derived_value_id: None,
            input_value_ids: vec![],
            input_provenance_chains: vec![],
            transform_kind: None,
            combined_digest: None,
            blocked_arg_names: vec![],
        }
    }

    /// Construct the broker-owned `sink_blocked` event carrying ALL durable anchors.
    ///
    /// This is the SOLE anchor-setting constructor (DESIGN §4 rule 7). It sets
    /// `event_type = "sink_blocked"`, `actor = "executor"`, and — critically —
    /// `taint` by merging every anchor's taint (DESIGN §4 rule 6; DB readers
    /// re-derive trust from each anchor's `taint`, no stored bool).
    ///
    /// `combined_digest`/`blocked_arg_names` carry the CONFIRM-03 whole-set
    /// binding (Phase 16, Round-6 amendment) — computed ONCE by the caller
    /// (`crate::confirmation::combined_digest` over the full `resolved_args`
    /// set) and threaded straight through; this constructor sets NOTHING
    /// itself (T-04-03 anti-stapling discipline, extended to the digest).
    #[allow(clippy::too_many_arguments)]
    pub fn sink_blocked(
        id: Uuid,
        parent_id: Option<Uuid>,
        session_id: Uuid,
        timestamp: DateTime<Utc>,
        anchors: Vec<SinkBlockedAnchor>,
        combined_digest: Option<String>,
        blocked_arg_names: Vec<String>,
    ) -> Self {
        let taint = anchors.iter().flat_map(|a| a.taint.clone()).collect();
        Event {
            id,
            parent_id,
            session_id,
            actor: "executor".into(),
            event_type: "sink_blocked".into(),
            timestamp,
            taint,
            anchors,
            derived_value_id: None,
            input_value_ids: vec![],
            input_provenance_chains: vec![],
            transform_kind: None,
            combined_digest,
            blocked_arg_names,
        }
    }

    /// Construct the broker-owned `derivation` event carrying the durable
    /// value-lineage payload for a transform-derived mint (Phase 15, D-16).
    ///
    /// This is the SOLE constructor for `event_type == "derivation"` — sets
    /// `actor = "confined-reader"` (the derivation records the confined
    /// worker's transform, mirroring `mint_from_read`'s file_read actor) and
    /// `anchors: vec![]` (derivation events never carry `sink_blocked`
    /// anchors). The payload fields (`derived_value_id`, `input_value_ids`,
    /// `input_provenance_chains`, `transform_kind`) are the durable DAG
    /// derivation edge (DESIGN-confirm-binding.md's explicit-DAG-derivation-
    /// edge option) — DB-alone checkable by EXTRACT-02, riding entirely in
    /// this hashed payload, NEVER in `parent_id` (two-graphs-never-share-
    /// edges).
    #[allow(clippy::too_many_arguments)]
    pub fn derivation(
        id: Uuid,
        parent_id: Option<Uuid>,
        session_id: Uuid,
        timestamp: DateTime<Utc>,
        taint: Vec<crate::plan_node::TaintLabel>,
        derived_value_id: Option<ValueId>,
        input_value_ids: Vec<ValueId>,
        input_provenance_chains: Vec<Vec<Uuid>>,
        transform_kind: Option<String>,
    ) -> Self {
        Event {
            id,
            parent_id,
            session_id,
            actor: "confined-reader".into(),
            event_type: "derivation".into(),
            timestamp,
            taint,
            anchors: vec![],
            derived_value_id,
            input_value_ids,
            input_provenance_chains,
            transform_kind,
            combined_digest: None,
            blocked_arg_names: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_node::TaintLabel;

    /// GOLDEN BYTE-FIXTURE (DESIGN §5 / §7): an `anchors: vec![]` Event serializes
    /// to JSON with NO `"anchors"` key — proving `skip_serializing_if` keeps
    /// pre-anchor events byte-identical to the pre-change format (no DB
    /// migration). The event also round-trips: `#[serde(default)]` supplies
    /// `anchors: vec![]` on deserialize.
    #[test]
    fn anchors_empty_event_serializes_byte_identical_and_round_trips() {
        let event = Event::new(
            Uuid::nil(),
            None,
            Uuid::nil(),
            "worker".to_string(),
            "file_read".to_string(),
            DateTime::from_timestamp(0, 0).expect("epoch timestamp"),
            vec![TaintLabel::ExternalUntrusted],
        );

        let json = serde_json::to_string(&event).expect("serialize");

        // Byte-exact match against the pre-anchor serialized form — NO "anchors" key.
        const GOLDEN: &str = "{\"id\":\"00000000-0000-0000-0000-000000000000\",\
\"parent_id\":null,\
\"session_id\":\"00000000-0000-0000-0000-000000000000\",\
\"actor\":\"worker\",\
\"event_type\":\"file_read\",\
\"timestamp\":\"1970-01-01T00:00:00Z\",\
\"taint\":[\"ExternalUntrusted\"]}";
        assert_eq!(
            json, GOLDEN,
            "anchors:[] Event must serialize byte-identical to the pre-anchor JSON \
             (skip_serializing_if omits the field — no DB migration)"
        );
        assert!(
            !json.contains("anchor"),
            "serialized anchors:[] Event must contain no \"anchors\" key"
        );
        assert!(
            !json.contains("combined_digest"),
            "serialized non-sink_blocked Event must contain no \"combined_digest\" key \
             (Phase 16 additive field — must not leak into non-block events)"
        );
        assert!(
            !json.contains("blocked_arg_names"),
            "serialized non-sink_blocked Event must contain no \"blocked_arg_names\" key \
             (Phase 16 additive field — must not leak into non-block events)"
        );

        // Round-trips: #[serde(default)] supplies anchors: vec![] on deserialize.
        let restored: Event = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, event, "Event must round-trip byte-identically");
        assert!(restored.anchors.is_empty(), "restored anchors must be empty");
        assert!(
            restored.combined_digest.is_none(),
            "restored combined_digest must be None"
        );
        assert!(
            restored.blocked_arg_names.is_empty(),
            "restored blocked_arg_names must be empty"
        );
    }

    /// A `sink_blocked` Event round-trips its `combined_digest` +
    /// `blocked_arg_names` intact (Phase 16, Task 1).
    #[test]
    fn sink_blocked_event_combined_digest_round_trips() {
        use crate::executor_decision::SinkBlockedAnchor;
        use crate::plan_node::{SinkId, ValueId};

        let read_event_id = Uuid::new_v4();
        let anchor = SinkBlockedAnchor {
            effect_id: Uuid::new_v4(),
            sink: SinkId("email.send".into()),
            arg: "body".into(),
            value_id: ValueId::new(),
            literal_sha256: "deadbeef".repeat(8),
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };

        let event = Event::sink_blocked(
            Uuid::new_v4(),
            None,
            Uuid::new_v4(),
            DateTime::from_timestamp(0, 0).expect("epoch timestamp"),
            vec![anchor],
            Some("abc123".to_string()),
            vec!["body".to_string()],
        );

        assert_eq!(event.combined_digest, Some("abc123".to_string()));
        assert_eq!(event.blocked_arg_names, vec!["body".to_string()]);

        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("\"combined_digest\":\"abc123\""),
            "sink_blocked JSON must carry combined_digest verbatim; got {json}"
        );
        assert!(
            json.contains("\"blocked_arg_names\":[\"body\"]"),
            "sink_blocked JSON must carry blocked_arg_names verbatim; got {json}"
        );

        let restored: Event = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, event, "sink_blocked Event must round-trip byte-identically");
    }

    /// A `derivation` event's payload carries derived_value_id, input_value_ids,
    /// input_provenance_chains, and transform_kind, and round-trips intact
    /// (Task 2 payload test).
    #[test]
    fn derivation_event_payload_round_trips() {
        use crate::plan_node::ValueId;

        let derived_value_id = ValueId::new();
        let input_a = ValueId::new();
        let input_b = ValueId::new();
        let chain_a = vec![Uuid::new_v4()];
        let chain_b = vec![Uuid::new_v4()];

        let event = Event::derivation(
            Uuid::new_v4(),
            None,
            Uuid::new_v4(),
            DateTime::from_timestamp(0, 0).expect("epoch timestamp"),
            vec![TaintLabel::ExternalUntrusted, TaintLabel::WorkerExtracted],
            Some(derived_value_id.clone()),
            vec![input_a.clone(), input_b.clone()],
            vec![chain_a.clone(), chain_b.clone()],
            Some("concat".to_string()),
        );

        assert_eq!(event.event_type, "derivation");
        assert_eq!(event.derived_value_id, Some(derived_value_id));
        assert_eq!(event.input_value_ids, vec![input_a, input_b]);
        assert_eq!(event.input_provenance_chains, vec![chain_a, chain_b]);
        assert_eq!(event.transform_kind, Some("concat".to_string()));
        assert!(event.anchors.is_empty(), "derivation events never carry anchors");

        let json = serde_json::to_string(&event).expect("serialize");
        let restored: Event = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, event, "derivation Event must round-trip byte-identically");
    }
}
