//! The OpenAI tool-calling client — the ONLY place in the workspace that
//! builds an HTTP request to an LLM provider and holds the `reqwest`
//! dependency (PLANNER-04).
//!
//! Three pieces, matching Task 1's `<action>`:
//!   (a) `build_chat_request` — pure request-body builder from a
//!       `PlannerRequest` alone (never a literal — `PlannerRequest` cannot
//!       carry one, see `llm_planner`'s doc comment).
//!   (b) `extract_tool_arguments` — pure extractor of the forced tool call's
//!       JSON-string arguments from a parsed OpenAI response body.
//!   (c) `call_openai` — the one async HTTP call, which POSTs (a)'s body,
//!       runs (b) on the result, and hands the extracted arguments to
//!       `llm_planner::parse_planner_response` for fail-closed validation.
//!
//! Every failure mode here — non-2xx status, transport error, missing tool
//! call, or `parse_planner_response` rejection — is an `Err`. There is no
//! fallback path that fabricates or guesses a `PlannerResponse`.

use llm_planner::{parse_planner_response, PlannerRequest, PlannerResponse};
use runtime_core::plan_node::ValueId;

/// OpenAI chat-completions endpoint. The sidecar's only network egress.
const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Build the OpenAI chat-completions request body from a `PlannerRequest`
/// alone (T-21-04 — no literal ever enters this function, because
/// `PlannerRequest` structurally cannot carry one). `messages` is a
/// system+user pair: a fixed system instruction plus
/// `llm_planner::build_planner_prompt(req)` as the user content (the exact
/// handle-ID + slot-hint + sink listing). `tools` names exactly one function,
/// `emit_plan_node`, whose `parameters` is
/// `llm_planner::build_tool_schema(req)` — so the model's structured output
/// is constrained to an offered handle ID / known sink by the schema's
/// `enum`s. `tool_choice` forces that function, so the model cannot answer
/// with free-text `message.content` instead of a tool call.
pub fn build_chat_request(req: &PlannerRequest, model: &str) -> serde_json::Value {
    let prompt = llm_planner::build_planner_prompt(req);
    let tool_schema = llm_planner::build_tool_schema(req);

    serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a planning assistant. You must respond by calling the \
                             `emit_plan_node` tool exactly once — never with free-text content."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "emit_plan_node",
                    "description": "Emit the chosen sink and its handle-bound arguments.",
                    "parameters": tool_schema
                }
            }
        ],
        "tool_choice": {
            "type": "function",
            "function": { "name": "emit_plan_node" }
        }
    })
}

/// Extract the forced tool call's JSON-string `arguments` from a parsed
/// OpenAI chat-completions response body:
/// `choices[0].message.tool_calls[0].function.arguments`.
///
/// Returns `Err` if that path is absent for ANY reason (no tool call, the
/// model answered with `message.content` free text instead, a malformed
/// response shape, etc.) — this function NEVER falls back to
/// `message.content`, because that would let the model bypass the
/// schema-constrained tool call entirely (T-21-05).
pub fn extract_tool_arguments(response: &serde_json::Value) -> anyhow::Result<String> {
    response
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("tool_calls"))
        .and_then(|tool_calls| tool_calls.get(0))
        .and_then(|tool_call| tool_call.get("function"))
        .and_then(|function| function.get("arguments"))
        .and_then(|arguments| arguments.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "OpenAI response has no choices[0].message.tool_calls[0].function.arguments \
                 (forced tool call missing or model answered with free-text content instead)"
            )
        })
}

/// Make the real OpenAI tool-calling HTTP call and validate the result
/// fail-closed. POSTs `build_chat_request(req, model)` to the
/// chat-completions endpoint with the given bearer `api_key`, extracts the
/// forced tool call's arguments via `extract_tool_arguments`, then hands
/// them to `llm_planner::parse_planner_response` with THIS request's own
/// offered handles and known sinks (never a wider/different allowlist).
///
/// Every failure path — non-2xx HTTP status, a transport/connection error,
/// a response body that doesn't parse as JSON, a missing tool call, or
/// `parse_planner_response` rejecting the tool call's contents — returns
/// `Err`. There is no success path that substitutes a default/fabricated
/// `PlannerResponse`.
pub async fn call_openai(
    req: &PlannerRequest,
    model: &str,
    api_key: &str,
) -> anyhow::Result<PlannerResponse> {
    let body = build_chat_request(req, model);

    let client = reqwest::Client::new();
    let http_response = client
        .post(OPENAI_CHAT_COMPLETIONS_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("OpenAI request transport error: {e}"))?;

    let status = http_response.status();
    if !status.is_success() {
        let error_body = http_response.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI request failed with status {status}: {error_body}");
    }

    let response_json: serde_json::Value = http_response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("OpenAI response body was not valid JSON: {e}"))?;

    let tool_arguments_json = extract_tool_arguments(&response_json)?;

    let offered: Vec<ValueId> = req
        .available_handles
        .iter()
        .map(|h| h.value_id.clone())
        .collect();
    let known_sinks = req.available_sinks.clone();

    parse_planner_response(&tool_arguments_json, &offered, &known_sinks)
        .map_err(|e| anyhow::anyhow!("planner response validation failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_planner::HandleLabel;

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
            ],
            available_sinks: vec!["email.send".to_string()],
        }
    }

    /// build_chat_request forces the emit_plan_node tool call
    /// (`tool_choice.function.name == "emit_plan_node"`) and embeds a tool
    /// schema whose `value_id` enum is exactly the offered handle IDs — the
    /// same structural guarantee `build_tool_schema` itself proves,
    /// asserted here at the request-body level.
    #[test]
    fn build_chat_request_forces_emit_plan_node_and_embeds_handle_enum() {
        let req = sample_request();
        let body = build_chat_request(&req, "gpt-4o-mini");

        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(
            body["tool_choice"]["function"]["name"],
            "emit_plan_node",
            "tool_choice must force emit_plan_node, got: {body}"
        );
        assert_eq!(
            body["tools"][0]["function"]["name"],
            "emit_plan_node",
            "the only declared tool must be emit_plan_node, got: {body}"
        );

        let value_id_enum = body["tools"][0]["function"]["parameters"]["properties"]["args"]
            ["items"]["properties"]["value_id"]["enum"]
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
            "embedded tool schema's value_id enum must equal exactly the offered handle IDs"
        );

        // The system+user message pair (never a single free-floating message).
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        let user_content = body["messages"][1]["content"]
            .as_str()
            .expect("user content is a string");
        for handle in &req.available_handles {
            assert!(
                user_content.contains(&handle.value_id.0.to_string()),
                "user message must contain every offered handle id"
            );
        }
    }

    /// extract_tool_arguments: Ok on a canned OpenAI tool-call response
    /// object shaped exactly like a real chat-completions body.
    #[test]
    fn extract_tool_arguments_ok_on_tool_call_response() {
        let response = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "tool_calls": [
                            {
                                "function": {
                                    "name": "emit_plan_node",
                                    "arguments": "{\"sink\":\"email.send\",\"args\":[]}"
                                }
                            }
                        ]
                    }
                }
            ]
        });

        let extracted = extract_tool_arguments(&response).expect("extraction should succeed");
        assert_eq!(extracted, "{\"sink\":\"email.send\",\"args\":[]}");
    }

    /// extract_tool_arguments: Err on a content-only response (the model
    /// answered with free text instead of the forced tool call) — this
    /// function must NEVER fall back to `message.content`.
    #[test]
    fn extract_tool_arguments_err_on_content_only_response() {
        let response = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "I would call emit_plan_node with sink email.send"
                    }
                }
            ]
        });

        let result = extract_tool_arguments(&response);
        assert!(
            result.is_err(),
            "a content-only response (no tool_calls) must be Err, got: {result:?}"
        );
    }
}
