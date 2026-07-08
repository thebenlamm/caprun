/// sink_sensitivity.rs — hardcoded email.send sensitivity map (v0).
///
/// DESIGN-plan-executor §"Sink Sensitivity Map (v0: hardcoded)":
/// The map is hardcoded in Rust — no Cedar, no schema, no runtime-configurable map.
/// Sensitivity is a security property, not a configuration knob. CON-i2-non-bypassable.
///
/// v0 scope: only email.send is live. Other sinks (http.post, file.write, exec,
/// db.query) are documented in the DESIGN for the per-sink rule but NOT implemented.

use runtime_core::plan_node::SinkId;

/// A sink-level effect classification (DESIGN-session-trust-state.md §6),
/// mirroring the locked 3-class `Effect` ontology in `runtime_core::effect`.
/// Exactly three variants — do NOT add a fourth. This is returned by a
/// hardcoded classifier keyed by `SinkId`, never a `PlanNode` field
/// (`CON-i2-non-bypassable`, `DEC-architectural-lock-plan-nodes`).
#[derive(Debug, Clone, PartialEq)]
pub enum EffectClass {
    Observe,
    MutateReversible,
    CommitIrreversible,
}

/// Returns the hardcoded `EffectClass` for `sink`.
///
/// v0/v1.2 mapping: both live sinks (`email.send`, `file.create`) are
/// `CommitIrreversible` (irreversible/external effects). Unknown sinks are
/// fail-closed to `CommitIrreversible` (the most restrictive class) — never a
/// permissive default. In practice this branch is unreachable in the live
/// path because Step 0's schema gate (`sink_schema::validate_schema`) already
/// rejects unregistered sinks before `sink_effect_class` is ever consulted
/// (DESIGN §6, Accepted Residual Risk 2); it is specified explicitly here so a
/// future refactor that reorders/removes that gate cannot silently reintroduce
/// a permissive default.
///
/// This is an internal `&str` match on the sink name (permitted to keep a `_`
/// arm per DESIGN §10) — NOT a match over the `EffectClass` enum itself; every
/// call site that matches on the RETURNED `EffectClass` must still be
/// exhaustive with no wildcard.
pub fn sink_effect_class(sink: &SinkId) -> EffectClass {
    match sink.0.as_str() {
        "email.send" => EffectClass::CommitIrreversible,
        "file.create" => EffectClass::CommitIrreversible,
        // Test-fixture-only arm (DESIGN §9 Pitfall m2 / RESEARCH Pitfall 3): the
        // ONLY vehicle that makes TAINT-03 (Draft + Observe still Allowed)
        // testable end-to-end, since both live sinks are CommitIrreversible.
        // Gated on `any(test, feature = "test-fixtures")` (not bare
        // `#[cfg(test)]`) so it is also visible to integration tests in
        // `tests/`, which link this crate via the `test-fixtures` self
        // dev-dependency rather than with `--cfg test` — see sink_schema.rs's
        // `TEST_KNOWN_SINKS` doc comment for the full rationale. Never
        // present in a production build either way.
        #[cfg(any(test, feature = "test-fixtures"))]
        "test.observe" => EffectClass::Observe,
        _ => EffectClass::CommitIrreversible,
    }
}

/// Args of email.send that determine WHERE the effect is delivered.
/// A tainted value in any of these args → `ExecutorDecision::BlockedPendingConfirmation`.
pub const EMAIL_SEND_ROUTING_SENSITIVE: &[&str] = &["to", "cc", "bcc"];

/// Args of file.create that determine WHERE the effect writes.
/// A tainted value in `path` → `ExecutorDecision::BlockedPendingConfirmation`.
/// `contents` is content-sensitive (WHAT is written), not routing-sensitive.
pub const FILE_CREATE_ROUTING_SENSITIVE: &[&str] = &["path"];

/// Args of email.send that determine WHAT irreversible payload leaves the trust boundary.
/// A tainted value here Blocks (Phase 14, CONTENT-01) via the same collect-then-Block
/// loop as routing-sensitive args (`crates/executor/src/lib.rs`) — content-sensitive
/// classification is no longer a no-op.
///
/// Attachment support is DESCOPED for v1.3 (D-23, `DESIGN-content-adapter-mediation.md`) —
/// removed here AND from `email.send`'s schema `allowed` set
/// (`crates/executor/src/sink_schema.rs`) atomically, so a plan node carrying that
/// arg is `Denied(UnknownArg)` at the Step 0 schema gate, before any sensitivity
/// evaluation. Missing either edge is a fail-open bug.
pub const EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body"];

/// Returns `true` iff `arg_name` is a routing-sensitive argument of `sink`.
///
/// Routing-sensitive means: the attacker who controls this arg value redirects
/// the effect (e.g., changes who receives the email). A tainted value here → Block.
///
/// v0 rule: hardcoded membership test on sink name + arg name. No dynamic lookup.
pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
        // v0: all other sinks — no routing-sensitive args defined yet.
        _ => false,
    }
}

/// Returns `true` iff `arg_name` is a content-sensitive argument of `sink`.
///
/// Content-sensitive means: the attacker who controls this arg cannot redirect the
/// effect but CAN exfiltrate or plant data through the payload. As of Phase 14
/// (CONTENT-01), this Blocks via `submit_plan_node`'s collect-then-Block loop
/// exactly like a routing-sensitive tainted arg — this function's classification
/// logic is unchanged (D-21); only its CONSEQUENCE in the caller changed.
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

    fn file_create() -> SinkId {
        SinkId("file.create".to_string())
    }

    #[test]
    fn file_create_path_is_routing_sensitive() {
        assert!(
            is_routing_sensitive(&file_create(), "path"),
            "file.create `path` routes the write — must be routing-sensitive"
        );
    }

    #[test]
    fn file_create_contents_not_routing_sensitive() {
        assert!(
            !is_routing_sensitive(&file_create(), "contents"),
            "file.create `contents` is WHAT is written, not WHERE — not routing-sensitive"
        );
    }

    #[test]
    fn email_send_routing_sensitive_args() {
        assert!(is_routing_sensitive(&email(), "to"));
        assert!(is_routing_sensitive(&email(), "cc"));
        assert!(is_routing_sensitive(&email(), "bcc"));
    }

    #[test]
    fn email_send_content_args_not_routing_sensitive() {
        // Phase 14 (D-23): the third pre-v1.3 content-sensitive arg name is
        // descoped entirely (no longer a valid email.send arg at all — see
        // sink_schema.rs), so only the two live content-sensitive args are
        // asserted here.
        assert!(!is_routing_sensitive(&email(), "subject"));
        assert!(!is_routing_sensitive(&email(), "body"));
    }

    #[test]
    fn unknown_sink_not_routing_sensitive() {
        assert!(!is_routing_sensitive(&other(), "to"));
        assert!(!is_routing_sensitive(&other(), "url"));
    }

    #[test]
    fn unknown_sink_not_content_sensitive() {
        // CONTENT-02 scope guard: content-sensitivity classification is scoped
        // to email.send ONLY — a non-email sink is never content-sensitive,
        // even for arg names that are content-sensitive on email.send.
        assert!(!is_content_sensitive(&other(), "body"));
        assert!(!is_content_sensitive(&other(), "subject"));
    }

    // -----------------------------------------------------------------
    // sink_effect_class (TAINT-02/03 classifier)
    // -----------------------------------------------------------------

    #[test]
    fn email_send_is_commit_irreversible() {
        assert_eq!(
            sink_effect_class(&SinkId("email.send".to_string())),
            EffectClass::CommitIrreversible
        );
    }

    #[test]
    fn file_create_is_commit_irreversible() {
        assert_eq!(
            sink_effect_class(&SinkId("file.create".to_string())),
            EffectClass::CommitIrreversible
        );
    }

    #[test]
    fn unregistered_sink_is_fail_closed_commit_irreversible() {
        assert_eq!(
            sink_effect_class(&SinkId("http.post".to_string())),
            EffectClass::CommitIrreversible,
            "unknown sink must fail-closed to the most restrictive class"
        );
    }

    #[test]
    fn test_observe_fixture_is_observe() {
        assert_eq!(
            sink_effect_class(&SinkId("test.observe".to_string())),
            EffectClass::Observe
        );
    }
}
