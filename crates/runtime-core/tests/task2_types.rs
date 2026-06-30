/// Task 2 TDD: field-presence + serde round-trip for ValueNode, Effect, ExecutorDecision
use runtime_core::{
    ExecutorDecision, Effect, ObserveEffect, ReversibleEffect, IrreversibleEffect,
    PlanArg, PlanNode, ValueId, ValueNode, SinkId, TaintLabel, Provenance,
};

#[test]
fn value_node_has_literal_provenance_taint_fields() {
    let node = ValueNode {
        literal: serde_json::json!("test-value"),
        provenance: Provenance {
            source_event_id: None,
            source_artifact_id: None,
            description: "unit test".to_string(),
            provenance_chain: vec![],
        },
        taint: vec![TaintLabel::UserTrusted],
    };
    // The taint field must be present and carry the value
    assert_eq!(node.taint.len(), 1);
    assert_eq!(node.taint[0], TaintLabel::UserTrusted);
    assert_eq!(node.literal, serde_json::json!("test-value"));
}

#[test]
fn value_node_serde_round_trip_preserves_taint() {
    let original = ValueNode {
        literal: serde_json::json!(42),
        provenance: Provenance {
            source_event_id: None,
            source_artifact_id: None,
            description: "provenance test".to_string(),
            provenance_chain: vec![],
        },
        taint: vec![TaintLabel::ExternalUntrusted, TaintLabel::LlmGenerated],
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: ValueNode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
    assert_eq!(restored.taint.len(), 2);
}

#[test]
fn effect_is_three_variant_enum() {
    // Observe variant
    let obs = Effect::Observe(ObserveEffect::ReadWorkspaceFile { path: "/tmp/x".to_string() });
    // MutateReversible variant
    let rev = Effect::MutateReversible(ReversibleEffect::WriteArtifact {
        name: "out.txt".to_string(),
        content_hash: "abc123".to_string(),
    });
    // CommitIrreversible variant
    let irr = Effect::CommitIrreversible(IrreversibleEffect::GitPush {
        remote: "origin".to_string(),
        branch: "main".to_string(),
    });
    // All three must exist
    let _ = (obs, rev, irr);
}

#[test]
fn executor_decision_has_not_implemented_variant() {
    let decision = ExecutorDecision::NotImplemented;
    assert_eq!(decision, ExecutorDecision::NotImplemented);
}

#[test]
fn executor_decision_has_all_variants() {
    let _allowed = ExecutorDecision::Allowed;
    let _blocked = ExecutorDecision::BlockedPendingConfirmation {
        literal_value: "val".to_string(),
        sink: "email.send".to_string(),
        arg_name: "to".to_string(),
        taint: vec![],
        provenance_chain: vec![],
    };
    let _denied = ExecutorDecision::Denied { reason: "policy".to_string() };
    let _ni = ExecutorDecision::NotImplemented;
}

#[test]
fn blocked_decision_carries_taint_and_provenance_chain() {
    // The Block payload carries taint + provenance_chain so a held-out test can assert
    // the unbroken chain directly from the decision (no second query). provenance_chain[0]
    // equals the file_read Event id.
    let event_id = uuid::Uuid::new_v4();
    let decision = ExecutorDecision::BlockedPendingConfirmation {
        literal_value: "attacker@evil.example".to_string(),
        sink: "email.send".to_string(),
        arg_name: "to".to_string(),
        taint: vec![TaintLabel::EmailRaw, TaintLabel::ExternalUntrusted],
        provenance_chain: vec![event_id],
    };
    // serde round-trips losslessly with the new fields.
    let json = serde_json::to_string(&decision).expect("serialize");
    let restored: ExecutorDecision = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decision, restored);
    match restored {
        ExecutorDecision::BlockedPendingConfirmation { taint, provenance_chain, .. } => {
            assert_eq!(taint.len(), 2);
            assert_eq!(provenance_chain[0], event_id);
        }
        _ => panic!("expected BlockedPendingConfirmation"),
    }
}

#[test]
fn plan_node_construction() {
    // PlanNode.args is now Vec<PlanArg> — the planner holds only opaque ValueId handles.
    let node = PlanNode {
        sink: SinkId("email.send".to_string()),
        args: vec![PlanArg {
            name: "to".to_string(),
            value_id: ValueId::new(),
        }],
    };
    assert_eq!(node.sink.0, "email.send");
    assert_eq!(node.args.len(), 1);
    assert_eq!(node.args[0].name, "to");
}
