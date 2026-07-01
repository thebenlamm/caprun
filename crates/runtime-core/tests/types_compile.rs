/// types_compile.rs — field-presence + serde round-trip test for all domain types
///
/// Tests that every domain type (Intent, Session, Event, Artifact, ValueNode, ValueId,
/// PlanArg, ValueRecord) constructs and round-trips through serde without panics.
/// The taint invariant is the primary structural guarantee: ValueNode.taint must survive
/// serialization. The handle-model invariant: PlanArg carries ONLY a ValueId (no literal,
/// no taint field) — the planner never touches broker-owned literals.
use runtime_core::{
    Artifact, ArtifactRef, Effect, Event, ExecutorDecision, IrreversibleEffect, Intent,
    IntentStatus, ObserveEffect, PlanArg, PlanNode, Provenance, ReversibleEffect, Session,
    SessionStatus, SinkId, TaintLabel, ValueId, ValueNode, ValueRecord,
};
use uuid::Uuid;
use chrono::Utc;

#[test]
fn value_id_is_opaque_handle_with_no_literal_or_taint() {
    // PlanArg carries only a ValueId — no literal, no taint field.
    // This is the core invariant of the handle model (T-04-02 mitigation).
    let vid = ValueId::new();
    let arg = PlanArg {
        name: "to".to_string(),
        value_id: vid.clone(),
    };
    assert_eq!(arg.name, "to");
    assert_eq!(arg.value_id, vid);
    // PlanArg has no .literal or .taint fields — the types enforce this statically.
}

#[test]
fn value_id_serde_round_trip() {
    let original = ValueId::new();
    let json = serde_json::to_string(&original).expect("ValueId serializes");
    let restored: ValueId = serde_json::from_str(&json).expect("ValueId deserializes");
    assert_eq!(original, restored, "ValueId serde round-trip must be lossless");
}

#[test]
fn value_record_carries_literal_taint_provenance_chain() {
    // ValueRecord is broker-owned and carries the full picture including provenance chain.
    let event_id = Uuid::new_v4();
    let record = ValueRecord {
        id: ValueId::new(),
        literal: "attacker@evil.example".to_string(),
        taint: vec![TaintLabel::EmailRaw, TaintLabel::ExternalUntrusted],
        provenance_chain: vec![event_id],
    };
    assert_eq!(record.taint.len(), 2);
    assert_eq!(record.provenance_chain[0], event_id);

    let json = serde_json::to_string(&record).expect("ValueRecord serializes");
    let restored: ValueRecord = serde_json::from_str(&json).expect("ValueRecord deserializes");
    assert_eq!(record, restored, "ValueRecord serde round-trip must be lossless");
    // provenance_chain[0] MUST equal the originating file_read Event id.
    assert_eq!(restored.provenance_chain[0], event_id);
}

#[test]
fn provenance_has_provenance_chain_field() {
    // Provenance gains provenance_chain: Vec<uuid::Uuid> for the ordered derivation edges.
    let event_id = Uuid::new_v4();
    let prov = Provenance {
        source_event_id: Some(event_id),
        source_artifact_id: None,
        description: "file_read provenance".to_string(),
        provenance_chain: vec![event_id],
    };
    assert_eq!(prov.provenance_chain.len(), 1);
    assert_eq!(prov.provenance_chain[0], event_id);
}

#[test]
fn value_node_taint_survives_serde_round_trip() {
    let original = ValueNode {
        literal: serde_json::json!({ "email": "attacker@evil.example" }),
        provenance: Provenance {
            source_event_id: Some(Uuid::new_v4()),
            source_artifact_id: None,
            description: "parsed from external email body".to_string(),
            provenance_chain: vec![],
        },
        taint: vec![TaintLabel::EmailRaw, TaintLabel::ExternalUntrusted],
    };
    let json = serde_json::to_string(&original).expect("ValueNode serializes");
    let restored: ValueNode = serde_json::from_str(&json).expect("ValueNode deserializes");
    assert_eq!(original, restored, "ValueNode serde round-trip must be lossless");
    assert_eq!(restored.taint.len(), 2, "taint labels must be preserved");
    assert_eq!(restored.taint[0], TaintLabel::EmailRaw);
    assert_eq!(restored.taint[1], TaintLabel::ExternalUntrusted);
}

#[test]
fn intent_constructs_and_round_trips() {
    let intent = Intent {
        id: Uuid::new_v4(),
        parent_id: None,
        description: "Run the test suite".to_string(),
        created_by: "user:ben".to_string(),
        status: IntentStatus::Active,
        created_at: Utc::now(),
        completed_at: None,
    };
    let json = serde_json::to_string(&intent).expect("Intent serializes");
    let restored: Intent = serde_json::from_str(&json).expect("Intent deserializes");
    assert_eq!(intent.id, restored.id);
    assert_eq!(restored.status, IntentStatus::Active);
}

#[test]
fn session_constructs_and_round_trips() {
    let session = Session {
        id: Uuid::new_v4(),
        intent_id: Uuid::new_v4(),
        status: SessionStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let json = serde_json::to_string(&session).expect("Session serializes");
    let restored: Session = serde_json::from_str(&json).expect("Session deserializes");
    assert_eq!(session.id, restored.id);
    assert_eq!(restored.status, SessionStatus::Active);
}

#[test]
fn event_constructs_and_uses_taint_label_from_plan_node() {
    let event = Event::new(
        Uuid::new_v4(),
        None,
        Uuid::new_v4(),
        "worker:1".to_string(),
        "read_file".to_string(),
        Utc::now(),
        vec![TaintLabel::LocalWorkspace],
    );
    // Event.taint uses the same TaintLabel from plan_node — no duplicate definition
    assert_eq!(event.taint[0], TaintLabel::LocalWorkspace);
    let json = serde_json::to_string(&event).expect("Event serializes");
    let _restored: Event = serde_json::from_str(&json).expect("Event deserializes");
}

#[test]
fn artifact_constructs_and_round_trips() {
    let artifact = Artifact {
        id: Uuid::new_v4(),
        name: "output.txt".to_string(),
        artifact_type: "text/plain".to_string(),
        content_hash: "sha256:abc123".to_string(),
        created_at: Utc::now(),
    };
    let json = serde_json::to_string(&artifact).expect("Artifact serializes");
    let restored: Artifact = serde_json::from_str(&json).expect("Artifact deserializes");
    assert_eq!(artifact.id, restored.id);
    assert_eq!(restored.name, "output.txt");
}

#[test]
fn artifact_ref_constructs() {
    let aref = ArtifactRef {
        id: Uuid::new_v4(),
        name: "report.pdf".to_string(),
    };
    let json = serde_json::to_string(&aref).expect("ArtifactRef serializes");
    let restored: ArtifactRef = serde_json::from_str(&json).expect("ArtifactRef deserializes");
    assert_eq!(aref.id, restored.id);
}

#[test]
fn all_domain_types_compose_in_a_plan_node() {
    // Build a plan node carrying an opaque handle (PlanArg) — no literal, no taint in PlanNode.
    // The broker-owned ValueRecord carries the literal+taint+provenance_chain separately.
    let value_id = ValueId::new();
    let plan_node = PlanNode {
        sink: SinkId("email.send".to_string()),
        args: vec![PlanArg {
            name: "to".to_string(),
            value_id: value_id.clone(),
        }],
    };
    assert_eq!(plan_node.args[0].name, "to");
    assert_eq!(plan_node.args[0].value_id, value_id);

    // The executor stub returns NotImplemented (typed, not panic)
    let decision = ExecutorDecision::NotImplemented;
    assert_eq!(decision, ExecutorDecision::NotImplemented);

    // Effects cover all 3 classes
    let _obs = Effect::Observe(ObserveEffect::RunTests { command: "cargo test".to_string() });
    let _rev = Effect::MutateReversible(ReversibleEffect::WriteArtifact {
        name: "result.txt".to_string(),
        content_hash: "sha256:deadbeef".to_string(),
    });
    let _irr = Effect::CommitIrreversible(IrreversibleEffect::SendEmail {
        draft_hash: "sha256:draft".to_string(),
        to: vec!["user@example.com".to_string()],
    });
}
