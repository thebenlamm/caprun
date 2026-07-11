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
///
/// Phase 15 (15-04, finding #5): `plan_from_intent` gained a named-Option
/// signature — `derived_recipient: Option<ValueId>`, `body: Option<ValueId>`,
/// plus the two always-present `trusted_subject_handle`/`trusted_body_handle`
/// handles (finding #6) — replacing the old `file_value_ids: &[ValueId]`
/// slice. The three tests below that asserted the OLD `to`-only,
/// file-handle-ignoring shape are UPDATED (not deleted) to the new shape.

// Include the planner module directly so these integration tests can call
// `plan_from_intent` without requiring a lib target in the caprun crate.
#[path = "../src/planner.rs"]
mod planner;

use llm_planner::{PlannerResponse, ResponseArg};
use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};

/// Find a plan arg by name (test helper).
fn arg<'a>(plan: &'a PlanNode, name: &str) -> &'a PlanArg {
    plan.args
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("plan must carry a `{name}` arg"))
}

/// A SendEmailSummary intent with a distinct subject/body literal — the
/// literal content is never visible to the planner (PLAN-03); only present so
/// every test constructs a realistic post-15-04 intent shape (finding #6).
fn email_intent(recipient: &str) -> CaprunIntent {
    CaprunIntent::SendEmailSummary {
        recipient: recipient.into(),
        subject: "Q3 summary".into(),
        body: "See attached.".into(),
    }
}

/// CreateFileFromReport CLEAN path: with NO derived path handle, the planner
/// routes the trusted intent handle into `file.create/path` (→ Allow downstream).
#[test]
fn plan_from_intent_create_file_clean_routes_intent_path() {
    let intent_vid = ValueId::new();
    let intent = CaprunIntent::CreateFileFromReport { path: "report.txt".into() };

    let plan = planner::plan_from_intent(
        &intent,
        intent_vid.clone(),
        None,
        None,
        intent_vid.clone(),
        intent_vid.clone(),
    );

    assert_eq!(plan.sink, SinkId("file.create".into()));
    assert_eq!(plan.args.len(), 2, "file.create must carry path + contents");
    assert_eq!(
        arg(&plan, "path").value_id,
        intent_vid,
        "clean path: `path` must carry the UserTrusted intent handle"
    );
    assert_eq!(
        arg(&plan, "contents").value_id,
        intent_vid,
        "`contents` resolves via the trusted intent handle"
    );
}

/// CreateFileFromReport HOSTILE path: when the workspace read yielded a tainted
/// RelativePath handle, the planner routes THAT (attacker-controlled) handle into
/// `file.create/path` (→ Block downstream), never the intent handle. The tainted
/// handle is threaded through the shared `derived_recipient` call-site-convention
/// slot (finding #7 — the planner never inspects provenance, only places
/// whichever handle the caller hands it).
#[test]
fn plan_from_intent_create_file_hostile_routes_tainted_path() {
    let intent_vid = ValueId::new();
    let file_vid = ValueId::new();
    let intent = CaprunIntent::CreateFileFromReport { path: "safe.txt".into() };

    let plan = planner::plan_from_intent(
        &intent,
        intent_vid.clone(),
        Some(file_vid.clone()),
        None,
        intent_vid.clone(),
        intent_vid.clone(),
    );

    assert_eq!(plan.sink, SinkId("file.create".into()));
    assert_eq!(
        arg(&plan, "path").value_id,
        file_vid,
        "hostile path: `path` must carry the tainted file handle → Block"
    );
    assert_ne!(
        arg(&plan, "path").value_id,
        intent_vid,
        "hostile path must NOT be laundered to the trusted intent handle"
    );
}

/// Core mapping: SendEmailSummary + intent_vid → PlanNode for email.send.
///
/// UPDATED (finding #5, was `plan_from_intent_send_email_summary_maps_to_email_send`):
/// the plan now carries THREE args (`to`/`subject`/`body`, RESEARCH Pitfall 2
/// closed) instead of one. Asserts:
///   - sink is "email.send"
///   - exactly three args: to, subject, body
///   - `to` carries intent_value_id when derived_recipient is None (benign)
///   - `subject`/`body` carry the trusted handles passed in (finding #6)
#[test]
fn plan_from_intent_send_email_summary_emits_to_subject_body() {
    let intent_vid = ValueId::new();
    let trusted_subject = ValueId::new();
    let trusted_body = ValueId::new();
    let intent = email_intent("boss@company.com");

    let plan = planner::plan_from_intent(
        &intent,
        intent_vid.clone(),
        None,
        None,
        trusted_subject.clone(),
        trusted_body.clone(),
    );

    assert_eq!(
        plan.sink,
        SinkId("email.send".into()),
        "planner must route SendEmailSummary to the email.send sink"
    );
    assert_eq!(
        plan.args.len(),
        3,
        "email.send must carry exactly three args: to, subject, body"
    );
    assert_eq!(
        arg(&plan, "to").value_id,
        intent_vid,
        "benign case: `to` must carry the intent_value_id (UserTrusted handle) \
         when derived_recipient is None"
    );
    assert_eq!(
        arg(&plan, "subject").value_id,
        trusted_subject,
        "`subject` must always carry the trusted subject handle"
    );
    assert_eq!(
        arg(&plan, "body").value_id,
        trusted_body,
        "benign case: `body` must carry the trusted body handle when body is None"
    );
}

/// UPDATED (finding #5, was `plan_from_intent_ignores_file_value_ids`): under
/// the new named-Option signature there is no `file_value_ids` slice. Asserts
/// BOTH halves of finding #8's resolved fork:
///   - benign (derived_recipient = None): `to` carries intent_value_id.
///   - hostile (derived_recipient = Some(x)): `to` carries x — the phase now
///     mandates this reachable path (PLAN-03 intent preserved: the planner
///     never fabricates a routing handle itself, it only places the one the
///     caller hands it).
#[test]
fn plan_from_intent_to_routes_by_derived_recipient_presence() {
    let intent_vid = ValueId::new();
    let derived_vid = ValueId::new();
    let trusted_subject = ValueId::new();
    let trusted_body = ValueId::new();

    let intent = email_intent("summary@example.com");

    // Benign case: derived_recipient = None -> `to` = intent_value_id.
    let benign_plan = planner::plan_from_intent(
        &intent,
        intent_vid.clone(),
        None,
        None,
        trusted_subject.clone(),
        trusted_body.clone(),
    );
    assert_eq!(
        arg(&benign_plan, "to").value_id,
        intent_vid,
        "benign: `to` must carry intent_value_id when derived_recipient is None"
    );
    assert_ne!(
        arg(&benign_plan, "to").value_id,
        derived_vid,
        "benign: `to` must NOT accidentally carry an unrelated derived handle"
    );

    // Hostile case: derived_recipient = Some(x) -> `to` = x, NEVER laundered
    // back to the trusted intent handle.
    let hostile_plan = planner::plan_from_intent(
        &intent,
        intent_vid.clone(),
        Some(derived_vid.clone()),
        None,
        trusted_subject,
        trusted_body,
    );
    assert_eq!(
        arg(&hostile_plan, "to").value_id,
        derived_vid,
        "hostile: `to` must carry the derived recipient handle when Some"
    );
    assert_ne!(
        arg(&hostile_plan, "to").value_id,
        intent_vid,
        "hostile: `to` must NOT be laundered to the trusted intent handle"
    );
}

/// Recipient literal is ignored by the planner (it lives in the broker's ValueStore).
///
/// Two intents with different `recipient` strings must produce the same PlanNode
/// shape (only the ValueId differs — and that ValueId is the caller's handle,
/// not derived from the recipient string inside plan_from_intent). Updated to
/// the new {recipient, subject, body} intent shape and named-Option signature
/// (finding #5); still valid — the planner never accesses the literal.
#[test]
fn plan_from_intent_recipient_literal_is_not_visible_to_planner() {
    let vid_a = ValueId::new();
    let vid_b = ValueId::new();
    let trusted_subject = ValueId::new();
    let trusted_body = ValueId::new();

    let intent_a = email_intent("a@example.com");
    let intent_b = email_intent("b@example.com");

    let plan_a = planner::plan_from_intent(
        &intent_a,
        vid_a.clone(),
        None,
        None,
        trusted_subject.clone(),
        trusted_body.clone(),
    );
    let plan_b = planner::plan_from_intent(
        &intent_b,
        vid_b.clone(),
        None,
        None,
        trusted_subject,
        trusted_body,
    );

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

// ─────────────────────────────────────────────────────────────────────────
// LlmPlanner support — unit tests for the pure `build_planner_request` and
// `response_to_plan_node` helpers (Phase 21 Plan 03 / PLANNER-03). These do
// NOT require a live sidecar: both functions are pure, so the fail-closed
// decisions they make are directly testable.
// ─────────────────────────────────────────────────────────────────────────

/// `build_planner_request` offers exactly {recipient, subject, body} handles,
/// tagged with the correct slot hints, using the SAME override rule
/// `plan_from_intent` uses (derived_recipient/body win when Some; otherwise
/// the trusted fallbacks). The constructed `PlannerRequest` carries only
/// `ValueId` handles + slot hints — no other value-bearing field (the type
/// itself is structurally incapable of carrying a literal, per llm-planner's
/// own key-set tests; this test additionally proves OUR builder places the
/// right handle behind the right hint).
#[test]
fn build_planner_request_offers_recipient_subject_body_handles() {
    let intent_vid = ValueId::new();
    let derived_vid = ValueId::new();
    let trusted_subject = ValueId::new();
    let trusted_body = ValueId::new();
    let intent = email_intent("boss@company.com");

    let (request, offered, known_sinks, canonical_names) = planner::build_planner_request(
        &intent,
        &intent_vid,
        Some(&derived_vid),
        None,
        &trusted_subject,
        &trusted_body,
    );

    assert_eq!(request.intent_kind, "SendEmailSummary");
    assert_eq!(request.available_sinks, vec!["email.send".to_string()]);
    assert_eq!(known_sinks, vec!["email.send".to_string()]);

    // canonical_names maps each offered handle to the EXACT arg name
    // `crates/executor/src/sink_schema.rs`'s hardcoded `email.send` schema
    // requires — never the model's own naming (Plan 21-04's live-run bug fix).
    let canonical_name_for = |vid: &ValueId| {
        canonical_names
            .iter()
            .find(|(v, _)| v == vid)
            .map(|(_, n)| n.as_str())
    };
    assert_eq!(canonical_name_for(&derived_vid), Some("to"));
    assert_eq!(canonical_name_for(&trusted_subject), Some("subject"));
    assert_eq!(canonical_name_for(&trusted_body), Some("body"));

    assert_eq!(request.available_handles.len(), 3);
    let by_hint = |hint: &str| {
        request
            .available_handles
            .iter()
            .find(|h| h.slot_hint == hint)
            .unwrap_or_else(|| panic!("must offer a `{hint}` handle"))
            .value_id
            .clone()
    };
    assert_eq!(
        by_hint("recipient"),
        derived_vid,
        "recipient must carry derived_recipient when Some (matches plan_from_intent's override)"
    );
    assert_eq!(by_hint("subject"), trusted_subject);
    assert_eq!(
        by_hint("body"),
        trusted_body,
        "body must fall back to trusted_body_handle when body is None"
    );

    assert_eq!(offered.len(), 3);
    assert!(offered.contains(&derived_vid));
    assert!(offered.contains(&trusted_subject));
    assert!(offered.contains(&trusted_body));
}

/// `build_planner_request` for `CreateFileFromReport` offers `file.create` as
/// the only sink.
#[test]
fn build_planner_request_create_file_offers_file_create_sink() {
    let intent_vid = ValueId::new();
    let trusted_subject = ValueId::new();
    let trusted_body = ValueId::new();
    let intent = CaprunIntent::CreateFileFromReport { path: "report.txt".into() };

    let (request, _offered, known_sinks, _canonical_names) = planner::build_planner_request(
        &intent,
        &intent_vid,
        None,
        None,
        &trusted_subject,
        &trusted_body,
    );

    assert_eq!(request.intent_kind, "CreateFileFromReport");
    assert_eq!(request.available_sinks, vec!["file.create".to_string()]);
    assert_eq!(known_sinks, vec!["file.create".to_string()]);
}

/// `response_to_plan_node`: Ok with the expected PlanNode for a valid
/// response whose sink and every arg value_id are in the caller-supplied
/// offered/known sets.
#[test]
fn response_to_plan_node_ok_for_valid_response() {
    let offered = vec![ValueId::new(), ValueId::new()];
    let known_sinks = vec!["email.send".to_string()];
    let canonical_names = vec![
        (offered[0].clone(), "to".to_string()),
        (offered[1].clone(), "subject".to_string()),
    ];

    let resp = PlannerResponse {
        sink: "email.send".to_string(),
        args: vec![
            ResponseArg { name: "to".to_string(), value_id: offered[0].clone() },
            ResponseArg { name: "subject".to_string(), value_id: offered[1].clone() },
        ],
    };

    let plan = planner::response_to_plan_node(&resp, &offered, &known_sinks, &canonical_names)
        .expect("valid response must map to a PlanNode");

    assert_eq!(plan.sink, SinkId("email.send".into()));
    assert_eq!(plan.args.len(), 2);
    assert_eq!(arg(&plan, "to").value_id, offered[0]);
    assert_eq!(arg(&plan, "subject").value_id, offered[1]);
}

/// `response_to_plan_node` NEVER trusts the model's own `arg.name` — it
/// always uses the caller-supplied `canonical_names` mapping (keyed by
/// `value_id`) instead. Plan 21-04's live-run bug fix: a real model named
/// the recipient arg after its `slot_hint` ("recipient") rather than the
/// sink's required name ("to"), which `crates/executor/src/sink_schema.rs`
/// then correctly `Denied(UnknownArg)`. This test proves the remap makes
/// the FINAL `PlanArg.name` the canonical one regardless of what string the
/// (simulated) model chose.
#[test]
fn response_to_plan_node_canonicalizes_arg_name_ignoring_model_naming() {
    let offered = vec![ValueId::new()];
    let known_sinks = vec!["email.send".to_string()];
    let canonical_names = vec![(offered[0].clone(), "to".to_string())];

    let resp = PlannerResponse {
        sink: "email.send".to_string(),
        // The model named this arg "recipient" (matching the slot_hint it
        // was shown), NOT the sink's required "to".
        args: vec![ResponseArg { name: "recipient".to_string(), value_id: offered[0].clone() }],
    };

    let plan = planner::response_to_plan_node(&resp, &offered, &known_sinks, &canonical_names)
        .expect("a response naming an offered handle under any string must still map");

    assert_eq!(
        plan.args[0].name, "to",
        "the final PlanArg name must be the canonical sink-required name, never the model's own \
         arg.name string"
    );
}

/// `response_to_plan_node`: Err when the response names a sink not in
/// `known_sinks` — fails closed, never fabricates or substitutes.
#[test]
fn response_to_plan_node_err_for_unknown_sink() {
    let offered = vec![ValueId::new()];
    let known_sinks = vec!["email.send".to_string()];
    let canonical_names: Vec<(ValueId, String)> = vec![];

    let resp = PlannerResponse {
        sink: "git.push".to_string(),
        args: vec![],
    };

    let result = planner::response_to_plan_node(&resp, &offered, &known_sinks, &canonical_names);
    assert!(result.is_err(), "unknown sink must be rejected");
}

/// `response_to_plan_node`: Err when a response arg's value_id is not a
/// member of `offered` — the sidecar referenced a handle it was never shown
/// (or fabricated one); never substituted with a fallback.
#[test]
fn response_to_plan_node_err_for_unoffered_handle() {
    let offered = vec![ValueId::new()];
    let known_sinks = vec!["email.send".to_string()];
    let canonical_names: Vec<(ValueId, String)> = vec![];
    let fabricated = ValueId::new();

    let resp = PlannerResponse {
        sink: "email.send".to_string(),
        args: vec![ResponseArg { name: "to".to_string(), value_id: fabricated }],
    };

    let result = planner::response_to_plan_node(&resp, &offered, &known_sinks, &canonical_names);
    assert!(result.is_err(), "unoffered handle must be rejected");
}
