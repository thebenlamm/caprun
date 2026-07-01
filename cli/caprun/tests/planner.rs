/// planner — unit tests for `plan_from_intent` (cli/caprun/src/planner.rs)
///
/// Tests that the deterministic, non-LLM planner correctly maps a typed
/// `CaprunIntent` + opaque `ValueId` handles to a `PlanNode`. These tests are
/// NOT Linux-gated: the planner is a pure function with no I/O, no async,
/// and no platform-specific code — it compiles and runs identically on macOS.
///
/// PLAN-03 type-level guarantee: `plan_from_intent` accepts only `ValueId`
/// (opaque handle), never a `ValueRecord`, literal, or taint label. The
/// function signature enforces this at compile time; no explicit test is needed
/// for compile-time properties.

// Include the planner module directly so these integration tests can call
// `plan_from_intent` without requiring a lib target in the caprun crate.
#[path = "../src/planner.rs"]
mod planner;

use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};

/// Core mapping: SendEmailSummary + intent_vid → PlanNode for email.send.
///
/// Asserts:
///   - sink is "email.send"
///   - exactly one arg named "to"
///   - arg's value_id equals the passed intent_value_id (not any file value)
#[test]
fn plan_from_intent_send_email_summary_maps_to_email_send() {
    let intent_vid = ValueId::new();
    let intent = CaprunIntent::SendEmailSummary {
        recipient: "boss@company.com".into(),
    };

    let plan = planner::plan_from_intent(&intent, intent_vid.clone(), &[]);

    assert_eq!(
        plan.sink,
        SinkId("email.send".into()),
        "planner must route SendEmailSummary to the email.send sink"
    );
    assert_eq!(plan.args.len(), 1, "exactly one arg expected for email.send");
    assert_eq!(plan.args[0].name, "to", "the single arg must be named 'to'");
    assert_eq!(
        plan.args[0].value_id, intent_vid,
        "the 'to' arg must carry the intent_value_id (UserTrusted handle), not a file handle"
    );
}

/// File value IDs are ignored: planner uses only the intent handle.
///
/// Passes non-empty `file_value_ids` and asserts the plan still uses
/// `intent_value_id` — the planner must never route a file-derived (tainted)
/// handle into the plan node on this path (PLAN-03 / I2).
#[test]
fn plan_from_intent_ignores_file_value_ids() {
    let intent_vid = ValueId::new();
    let file_vid = ValueId::new();

    let intent = CaprunIntent::SendEmailSummary {
        recipient: "summary@example.com".into(),
    };

    let plan = planner::plan_from_intent(&intent, intent_vid.clone(), &[file_vid.clone()]);

    // The plan must use intent_vid, NOT file_vid.
    assert_eq!(
        plan.args[0].value_id, intent_vid,
        "planner must use intent_value_id, not file_value_ids[0]"
    );
    assert_ne!(
        plan.args[0].value_id, file_vid,
        "planner must NOT route the tainted file handle to a routing-sensitive arg"
    );
}

/// Recipient literal is ignored by the planner (it lives in the broker's ValueStore).
///
/// Two intents with different `recipient` strings must produce the same PlanNode
/// shape (only the ValueId differs — and that ValueId is the caller's handle,
/// not derived from the recipient string inside plan_from_intent).
#[test]
fn plan_from_intent_recipient_literal_is_not_visible_to_planner() {
    let vid_a = ValueId::new();
    let vid_b = ValueId::new();

    let intent_a = CaprunIntent::SendEmailSummary {
        recipient: "a@example.com".into(),
    };
    let intent_b = CaprunIntent::SendEmailSummary {
        recipient: "b@example.com".into(),
    };

    let plan_a = planner::plan_from_intent(&intent_a, vid_a.clone(), &[]);
    let plan_b = planner::plan_from_intent(&intent_b, vid_b.clone(), &[]);

    // Sink is identical regardless of recipient.
    assert_eq!(plan_a.sink, plan_b.sink);
    // Arg name is identical.
    assert_eq!(plan_a.args[0].name, plan_b.args[0].name);
    // Value IDs differ (they come from the caller, not from the literal).
    assert_ne!(
        plan_a.args[0].value_id, plan_b.args[0].value_id,
        "different callers provide different handles; planner does not derive them from the literal"
    );
}
