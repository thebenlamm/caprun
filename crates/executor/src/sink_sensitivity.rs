/// sink_sensitivity.rs — hardcoded email.send sensitivity map (v0).
///
/// DESIGN-plan-executor §"Sink Sensitivity Map (v0: hardcoded)":
/// The map is hardcoded in Rust — no Cedar, no schema, no runtime-configurable map.
/// Sensitivity is a security property, not a configuration knob. CON-i2-non-bypassable.
///
/// v0 scope: only email.send is live. Other sinks (http.post, file.write, exec,
/// db.query) are documented in the DESIGN for the per-sink rule but NOT implemented.

use runtime_core::plan_node::SinkId;

/// Args of email.send that determine WHERE the effect is delivered.
/// A tainted value in any of these args → `ExecutorDecision::BlockedPendingConfirmation`.
pub const EMAIL_SEND_ROUTING_SENSITIVE: &[&str] = &["to", "cc", "bcc"];

/// Args of email.send that determine WHAT irreversible payload leaves the trust boundary.
/// A tainted value here does NOT auto-Block in v0 but MUST be surfaced for Tier-4
/// verbatim review at approval time (post-v0 approval hook).
pub const EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body", "attachment"];

/// Returns `true` iff `arg_name` is a routing-sensitive argument of `sink`.
///
/// Routing-sensitive means: the attacker who controls this arg value redirects
/// the effect (e.g., changes who receives the email). A tainted value here → Block.
///
/// v0 rule: hardcoded membership test on sink name + arg name. No dynamic lookup.
pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        // v0: all other sinks — no routing-sensitive args defined yet.
        _ => false,
    }
}

/// Returns `true` iff `arg_name` is a content-sensitive argument of `sink`.
///
/// Content-sensitive means: the attacker who controls this arg cannot redirect the
/// effect but CAN exfiltrate or plant data through the payload. Does NOT Block in
/// v0; surfaced for Tier-4 verbatim review at approval.
pub fn is_content_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_CONTENT_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::plan_node::SinkId;

    fn email() -> SinkId {
        SinkId("email.send".to_string())
    }

    fn other() -> SinkId {
        SinkId("http.post".to_string())
    }

    #[test]
    fn email_send_routing_sensitive_args() {
        assert!(is_routing_sensitive(&email(), "to"));
        assert!(is_routing_sensitive(&email(), "cc"));
        assert!(is_routing_sensitive(&email(), "bcc"));
    }

    #[test]
    fn email_send_content_args_not_routing_sensitive() {
        assert!(!is_routing_sensitive(&email(), "subject"));
        assert!(!is_routing_sensitive(&email(), "body"));
        assert!(!is_routing_sensitive(&email(), "attachment"));
    }

    #[test]
    fn unknown_sink_not_routing_sensitive() {
        assert!(!is_routing_sensitive(&other(), "to"));
        assert!(!is_routing_sensitive(&other(), "url"));
    }
}
