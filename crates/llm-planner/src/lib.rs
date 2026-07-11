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
}
