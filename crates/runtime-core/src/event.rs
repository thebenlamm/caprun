/// event.rs — Event (audit DAG node)
///
/// Each Event is a node in the causal audit DAG. The taint field propagates
/// TaintLabel from raw reads through to ValueNode arguments at sinks — this
/// chain is what I2 enforcement verifies in Phase 4.
/// Event.taint reuses plan_node::TaintLabel — no duplicate definition.

use crate::executor_decision::SinkBlockedAnchor;
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
    /// The durable genuine-taint anchor for a `sink_blocked` event (ACC-07).
    ///
    /// `None` for every event type except `sink_blocked`; a `sink_blocked`
    /// event with `anchor == None` is non-persistable through the TCB
    /// (`audit::append_event` rejects it). `#[serde(skip_serializing_if)]` keeps
    /// pre-anchor events byte-identical (no DB migration — DESIGN §5); the
    /// anchor rides inside the hashed `payload` column, so it is tamper-evident.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<SinkBlockedAnchor>,
}

impl Event {
    /// Construct an ordinary (non-`sink_blocked`) audit event. Sets `anchor: None`.
    ///
    /// This is the ONLY sanctioned way to build a non-block event — every literal
    /// `Event { .. }` site migrated to it, so adding a future field can never
    /// silently drop `anchor`.
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
            anchor: None,
        }
    }

    /// Construct the broker-owned `sink_blocked` event carrying the durable anchor.
    ///
    /// This is the SOLE anchor-setting constructor (DESIGN §4 rule 7). It sets
    /// `event_type = "sink_blocked"`, `actor = "executor"`, and — critically —
    /// `taint = anchor.taint.clone()` so `Event.taint == anchor.taint` (DESIGN
    /// §4 rule 6; DB readers re-derive trust from `anchor.taint`, no stored bool).
    pub fn sink_blocked(
        id: Uuid,
        parent_id: Option<Uuid>,
        session_id: Uuid,
        timestamp: DateTime<Utc>,
        anchor: SinkBlockedAnchor,
    ) -> Self {
        Event {
            id,
            parent_id,
            session_id,
            actor: "executor".into(),
            event_type: "sink_blocked".into(),
            timestamp,
            taint: anchor.taint.clone(),
            anchor: Some(anchor),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_node::TaintLabel;

    /// GOLDEN BYTE-FIXTURE (DESIGN §5 / §7): an `anchor: None` Event serializes to
    /// JSON with NO `"anchor"` key — proving `skip_serializing_if` keeps pre-anchor
    /// events byte-identical to the pre-change format (no DB migration). The event
    /// also round-trips: `#[serde(default)]` supplies `anchor: None` on deserialize.
    #[test]
    fn anchor_none_event_serializes_byte_identical_and_round_trips() {
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

        // Byte-exact match against the pre-anchor serialized form — NO "anchor" key.
        const GOLDEN: &str = "{\"id\":\"00000000-0000-0000-0000-000000000000\",\
\"parent_id\":null,\
\"session_id\":\"00000000-0000-0000-0000-000000000000\",\
\"actor\":\"worker\",\
\"event_type\":\"file_read\",\
\"timestamp\":\"1970-01-01T00:00:00Z\",\
\"taint\":[\"ExternalUntrusted\"]}";
        assert_eq!(
            json, GOLDEN,
            "anchor:None Event must serialize byte-identical to the pre-anchor JSON \
             (skip_serializing_if omits the field — no DB migration)"
        );
        assert!(
            !json.contains("anchor"),
            "serialized anchor:None Event must contain no \"anchor\" key"
        );

        // Round-trips: #[serde(default)] supplies anchor: None on deserialize.
        let restored: Event = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, event, "Event must round-trip byte-identically");
        assert!(restored.anchor.is_none(), "restored anchor must be None");
    }
}
