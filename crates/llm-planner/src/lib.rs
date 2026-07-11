//! llm-planner — the literal-free wire contract between the worker-side
//! `LlmPlanner` proxy and the out-of-process LLM sidecar (PLANNER-03/04).
//!
//! This crate is pure: no network code, no `reqwest`, no `tokio`. The
//! sidecar (Phase 21 Plan 02) and the worker-side proxy (Phase 21 Plan 03)
//! both depend on it and exchange ONLY the types defined here — none of
//! which carries a resolved literal. The LLM can reference a value only by
//! its opaque `ValueId` handle; the literal itself never crosses this wire.
//!
//! # Structural literal-incapability (T-21-01)
//!
//! `PlannerRequest` and `HandleLabel` have NO literal/value/text field.
//! `HandleLabel` carries only a `slot_hint` (a human-readable label like
//! "recipient"/"subject"/"body") and an opaque `value_id: ValueId`. This is
//! proven by the key-set serde tests below, not just asserted in prose: a
//! future field addition that introduces a literal-carrying field would
//! break those tests.

use runtime_core::plan_node::ValueId;

/// Request sent to the LLM sidecar: what the planner is being asked to do
/// (`intent_kind`), the opaque handles it may reference (`available_handles`),
/// and the sinks it may call (`available_sinks`). Carries NO literal field —
/// only handle IDs + slot hints + a typed intent-kind label.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlannerRequest {
    pub intent_kind: String,
    pub available_handles: Vec<HandleLabel>,
    pub available_sinks: Vec<String>,
}

/// One offered handle: a human-readable slot hint ("recipient" / "subject" /
/// "body") paired with the opaque `ValueId` the LLM may reference for that
/// slot. There is no literal/value/text field here — the LLM sees only the
/// hint and the handle, never the underlying value.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HandleLabel {
    pub slot_hint: String,
    pub value_id: ValueId,
}

/// The LLM sidecar's tool-call response: a chosen sink and the args it wants
/// to bind, each naming a slot and the handle it should resolve to.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlannerResponse {
    pub sink: String,
    pub args: Vec<ResponseArg>,
}

/// One arg binding in a `PlannerResponse`: a named slot mapped to an opaque
/// handle. Carries NO literal — `value_id` must be one of the handles
/// `PlannerRequest.available_handles` offered this request (validated by
/// `parse_planner_response`, added in Task 2).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResponseArg {
    pub name: String,
    pub value_id: ValueId,
}

/// Fail-closed errors `parse_planner_response` returns. Never fabricates or
/// substitutes a handle — every non-Ok case is a hard rejection (T-21-02).
#[derive(Debug, Clone, PartialEq)]
pub enum PlannerError {
    /// The tool-call arguments were not valid JSON, or did not match the
    /// expected `PlannerResponse` shape.
    MalformedJson(String),
    /// The response named a sink not present in the caller-supplied
    /// `known_sinks` set.
    UnknownSink(String),
    /// A response arg's `value_id` was not a member of the caller-supplied
    /// `offered` handle set — the LLM referenced a handle it was never
    /// shown (or fabricated one).
    UnknownHandle(ValueId),
}

impl std::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlannerError::MalformedJson(msg) => {
                write!(f, "malformed planner response JSON: {msg}")
            }
            PlannerError::UnknownSink(sink) => {
                write!(f, "planner response named unknown sink: {sink}")
            }
            PlannerError::UnknownHandle(value_id) => {
                write!(f, "planner response referenced unoffered handle: {value_id:?}")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

/// Build the LLM prompt from a `PlannerRequest` alone — never from a
/// literal, because `PlannerRequest` cannot carry one (T-21-01). Composes a
/// system+user prompt naming the `emit_plan_node` tool and listing every
/// offered `(slot_hint, value_id)` pair plus the available sinks, instructing
/// the model to copy handle IDs verbatim into arg `value_id`s.
///
/// This is a single pure function with NO I/O — the exact seam Phase 22's
/// GATE-04 sentinel assertion targets: feed it a sentinel-tagged
/// `PlannerRequest` and assert the sentinel bytes never appear in the
/// output. Because this function reads only handle IDs + slot hints +
/// intent_kind + sink names, there is no field through which a literal
/// could enter.
pub fn build_planner_prompt(request: &PlannerRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "You are a planning assistant. You must call the `emit_plan_node` tool exactly once, \
         choosing a sink and binding each of its arguments to one of the handle IDs listed \
         below. Copy each handle ID verbatim — never invent, alter, or guess a handle ID, and \
         never emit a literal value.\n\n",
    );
    prompt.push_str(&format!("Intent kind: {}\n\n", request.intent_kind));

    prompt.push_str("Available handles:\n");
    for handle in &request.available_handles {
        prompt.push_str(&format!(
            "- slot_hint: {}, value_id: {}\n",
            handle.slot_hint, handle.value_id.0
        ));
    }
    prompt.push('\n');

    prompt.push_str("Available sinks:\n");
    for sink in &request.available_sinks {
        prompt.push_str(&format!("- {sink}\n"));
    }
    prompt.push('\n');

    prompt.push_str(
        "Call `emit_plan_node` with a `sink` from the available sinks above and `args` whose \
         each `value_id` is copied verbatim from the handle IDs listed above.\n",
    );

    prompt
}

/// Build the `emit_plan_node` tool's JSON-schema `parameters` object from a
/// `PlannerRequest`. The `sink` field's `enum` is exactly
/// `request.available_sinks`; each arg's `value_id` field's `enum` is
/// exactly the set of offered handle-ID strings — so a conforming tool-call
/// structurally cannot reference anything but an offered handle (T-21-01,
/// T-21-03).
pub fn build_tool_schema(request: &PlannerRequest) -> serde_json::Value {
    let handle_id_strings: Vec<String> = request
        .available_handles
        .iter()
        .map(|h| h.value_id.0.to_string())
        .collect();

    serde_json::json!({
        "type": "object",
        "properties": {
            "sink": {
                "type": "string",
                "enum": request.available_sinks,
            },
            "args": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "value_id": {
                            "type": "string",
                            "enum": handle_id_strings,
                        },
                    },
                    "required": ["name", "value_id"],
                },
            },
        },
        "required": ["sink", "args"],
    })
}

/// Parse the LLM sidecar's tool-call arguments into a `PlannerResponse`,
/// failing closed (T-21-02). Returns `Ok` ONLY when the JSON is well-formed
/// AND the sink is a member of `known_sinks` AND every arg's `value_id` is a
/// member of `offered`. Never fabricates or substitutes a handle — any
/// violation is a hard `Err`, with no wildcard fallback.
pub fn parse_planner_response(
    tool_arguments_json: &str,
    offered: &[ValueId],
    known_sinks: &[String],
) -> Result<PlannerResponse, PlannerError> {
    let response: PlannerResponse = serde_json::from_str(tool_arguments_json)
        .map_err(|e| PlannerError::MalformedJson(e.to_string()))?;

    if !known_sinks.iter().any(|s| s == &response.sink) {
        return Err(PlannerError::UnknownSink(response.sink));
    }

    for arg in &response.args {
        if !offered.iter().any(|v| v == &arg.value_id) {
            return Err(PlannerError::UnknownHandle(arg.value_id.clone()));
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_handle() -> HandleLabel {
        HandleLabel {
            slot_hint: "recipient".to_string(),
            value_id: ValueId::new(),
        }
    }

    #[test]
    fn planner_request_round_trips_through_serde_json() {
        let req = PlannerRequest {
            intent_kind: "SendEmailSummary".to_string(),
            available_handles: vec![sample_handle()],
            available_sinks: vec!["email.send".to_string()],
        };
        let json = serde_json::to_string(&req).expect("serialize PlannerRequest");
        let recovered: PlannerRequest =
            serde_json::from_str(&json).expect("deserialize PlannerRequest");
        assert_eq!(req, recovered);
    }

    #[test]
    fn planner_response_round_trips_through_serde_json() {
        let resp = PlannerResponse {
            sink: "email.send".to_string(),
            args: vec![ResponseArg {
                name: "to".to_string(),
                value_id: ValueId::new(),
            }],
        };
        let json = serde_json::to_string(&resp).expect("serialize PlannerResponse");
        let recovered: PlannerResponse =
            serde_json::from_str(&json).expect("deserialize PlannerResponse");
        assert_eq!(resp, recovered);
    }

    /// Structural proof (T-21-01): the serialized `PlannerRequest` JSON
    /// object's key set is EXACTLY {intent_kind, available_handles,
    /// available_sinks} — no literal-carrying field exists. Asserted on the
    /// parsed `serde_json::Value` key set, not a raw string grep, so it is
    /// robust to formatting and immediately fails if a new field is added
    /// without updating this test.
    #[test]
    fn planner_request_key_set_has_no_literal_field() {
        let req = PlannerRequest {
            intent_kind: "SendEmailSummary".to_string(),
            available_handles: vec![sample_handle()],
            available_sinks: vec!["email.send".to_string()],
        };
        let json = serde_json::to_value(&req).expect("serialize PlannerRequest to Value");
        let obj = json.as_object().expect("PlannerRequest serializes to a JSON object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["available_handles", "available_sinks", "intent_kind"],
            "PlannerRequest must carry EXACTLY {{intent_kind, available_handles, \
             available_sinks}} — no literal-carrying field, got keys {keys:?}"
        );
    }

    /// Structural proof (T-21-01): a `HandleLabel`'s key set is EXACTLY
    /// {slot_hint, value_id} — no literal/value/text field.
    #[test]
    fn handle_label_key_set_has_no_literal_field() {
        let label = sample_handle();
        let json = serde_json::to_value(&label).expect("serialize HandleLabel to Value");
        let obj = json.as_object().expect("HandleLabel serializes to a JSON object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["slot_hint", "value_id"],
            "HandleLabel must carry EXACTLY {{slot_hint, value_id}} — no literal-carrying \
             field, got keys {keys:?}"
        );
    }

    fn sample_request() -> PlannerRequest {
        PlannerRequest {
            intent_kind: "SendEmailSummary".to_string(),
            available_handles: vec![
                HandleLabel {
                    slot_hint: "recipient".to_string(),
                    value_id: ValueId::new(),
                },
                HandleLabel {
                    slot_hint: "subject".to_string(),
                    value_id: ValueId::new(),
                },
                HandleLabel {
                    slot_hint: "body".to_string(),
                    value_id: ValueId::new(),
                },
            ],
            available_sinks: vec!["email.send".to_string()],
        }
    }

    /// build_planner_prompt output contains every offered handle's UUID
    /// string AND its slot_hint.
    #[test]
    fn build_planner_prompt_contains_every_handle_id_and_slot_hint() {
        let req = sample_request();
        let prompt = build_planner_prompt(&req);
        for handle in &req.available_handles {
            assert!(
                prompt.contains(&handle.value_id.0.to_string()),
                "prompt must contain handle id {}, got: {prompt}",
                handle.value_id.0
            );
            assert!(
                prompt.contains(&handle.slot_hint),
                "prompt must contain slot_hint {}, got: {prompt}",
                handle.slot_hint
            );
        }
    }

    /// build_tool_schema's value_id enum equals exactly the offered handle
    /// IDs — no extras, none missing.
    #[test]
    fn build_tool_schema_value_id_enum_equals_offered_handles() {
        let req = sample_request();
        let schema = build_tool_schema(&req);

        let value_id_enum = schema["properties"]["args"]["items"]["properties"]["value_id"]
            ["enum"]
            .as_array()
            .expect("value_id enum is an array")
            .iter()
            .map(|v| v.as_str().expect("enum entry is a string").to_string())
            .collect::<std::collections::BTreeSet<_>>();

        let expected: std::collections::BTreeSet<String> = req
            .available_handles
            .iter()
            .map(|h| h.value_id.0.to_string())
            .collect();

        assert_eq!(
            value_id_enum, expected,
            "value_id enum must equal exactly the offered handle IDs, no extras"
        );

        let sink_enum = schema["properties"]["sink"]["enum"]
            .as_array()
            .expect("sink enum is an array")
            .iter()
            .map(|v| v.as_str().expect("enum entry is a string").to_string())
            .collect::<Vec<_>>();
        assert_eq!(sink_enum, req.available_sinks);
    }

    /// parse_planner_response: Ok for a valid response whose sink and every
    /// arg value_id are in the caller-supplied offered/known sets.
    #[test]
    fn parse_planner_response_ok_for_valid_response() {
        let req = sample_request();
        let offered: Vec<ValueId> = req
            .available_handles
            .iter()
            .map(|h| h.value_id.clone())
            .collect();
        let known_sinks = req.available_sinks.clone();

        let resp = PlannerResponse {
            sink: "email.send".to_string(),
            args: vec![ResponseArg {
                name: "to".to_string(),
                value_id: offered[0].clone(),
            }],
        };
        let json = serde_json::to_string(&resp).expect("serialize PlannerResponse");

        let parsed = parse_planner_response(&json, &offered, &known_sinks);
        assert_eq!(parsed, Ok(resp));
    }

    /// parse_planner_response: Err(UnknownSink) when the sink is not in
    /// known_sinks.
    #[test]
    fn parse_planner_response_err_for_unknown_sink() {
        let req = sample_request();
        let offered: Vec<ValueId> = req
            .available_handles
            .iter()
            .map(|h| h.value_id.clone())
            .collect();
        let known_sinks = req.available_sinks.clone();

        let resp = PlannerResponse {
            sink: "git.push".to_string(),
            args: vec![],
        };
        let json = serde_json::to_string(&resp).expect("serialize PlannerResponse");

        let parsed = parse_planner_response(&json, &offered, &known_sinks);
        assert_eq!(parsed, Err(PlannerError::UnknownSink("git.push".to_string())));
    }

    /// parse_planner_response: Err(UnknownHandle) when an arg's value_id is
    /// not in `offered` — a fabricated/unshown handle is rejected, never
    /// substituted or fabricated on the parser's side.
    #[test]
    fn parse_planner_response_err_for_unoffered_handle() {
        let req = sample_request();
        let offered: Vec<ValueId> = req
            .available_handles
            .iter()
            .map(|h| h.value_id.clone())
            .collect();
        let known_sinks = req.available_sinks.clone();

        let fabricated = ValueId::new();
        let resp = PlannerResponse {
            sink: "email.send".to_string(),
            args: vec![ResponseArg {
                name: "to".to_string(),
                value_id: fabricated.clone(),
            }],
        };
        let json = serde_json::to_string(&resp).expect("serialize PlannerResponse");

        let parsed = parse_planner_response(&json, &offered, &known_sinks);
        assert_eq!(parsed, Err(PlannerError::UnknownHandle(fabricated)));
    }

    /// parse_planner_response: Err(MalformedJson) on invalid JSON.
    #[test]
    fn parse_planner_response_err_for_malformed_json() {
        let req = sample_request();
        let offered: Vec<ValueId> = req
            .available_handles
            .iter()
            .map(|h| h.value_id.clone())
            .collect();
        let known_sinks = req.available_sinks.clone();

        let parsed = parse_planner_response("not json", &offered, &known_sinks);
        assert!(matches!(parsed, Err(PlannerError::MalformedJson(_))));
    }
}
