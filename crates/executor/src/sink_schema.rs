/// sink_schema.rs — hardcoded per-sink argument schema + `validate_schema`
/// (HARD-01 / HARD-05 arg-schema gate).
///
/// This is the FIRST enforcement step of `submit_plan_node`: a plan node whose
/// sink is unregistered, or whose arg set is malformed (unknown arg, duplicate,
/// or missing required arg), is `Denied` BEFORE any handle resolve, taint check,
/// or sensitivity evaluation. Fail-closed: only sinks in `KNOWN_SINKS` are
/// callable, and each is callable ONLY with its exact declared arg set.
///
/// Like `sink_sensitivity`, the schema is hardcoded in the Rust TCB — no runtime
/// registry, no config file. It EXTENDS the single `DenyReason` taxonomy (07-01)
/// rather than introducing a second error type (CON-i2-non-bypassable).
///
/// v0 exact-match semantics: the declared arg set is BOTH the allowed set and the
/// required set — a plan node must carry exactly those args, no more, no fewer,
/// none duplicated. This is the strictest fail-closed posture; 07-04b wires the
/// live sink on top of this gate. (The `email.send` shape is the decision-side
/// registration; its live invocation shape is finalized in 07-04b.)
use runtime_core::plan_node::PlanNode;
use runtime_core::DenyReason;

/// Hardcoded registry: sink id → its exact declared arg-name set.
///
/// `email.send` → the current live shape `[to, cc, bcc, subject, body]`.
/// `file.create` → `[path, contents]` (SINK-01).
pub const KNOWN_SINKS: &[(&str, &[&str])] = &[
    ("email.send", &["to", "cc", "bcc", "subject", "body"]),
    ("file.create", &["path", "contents"]),
];

/// The declared arg set for `sink`, or `None` if the sink is not registered.
pub fn known_sink_args(sink: &str) -> Option<&'static [&'static str]> {
    KNOWN_SINKS
        .iter()
        .find(|(name, _)| *name == sink)
        .map(|(_, args)| *args)
}

/// Validate a plan node's sink + arg set against the hardcoded schema.
///
/// Ordering (fail-closed, checked BEFORE resolve/sensitivity in `submit_plan_node`):
///   1. Unknown sink → `UnknownSink` (nothing else is checked).
///   2. Per arg, in order: not in the sink's set → `UnknownArg`; already seen →
///      `DuplicateArg`.
///   3. After scanning args: any required (== declared) arg absent → `MissingArg`.
///
/// Returns `Ok(())` iff the plan node carries exactly the sink's declared args.
pub fn validate_schema(plan_node: &PlanNode) -> Result<(), DenyReason> {
    // Step 1: the sink must be registered. An unregistered sink fails closed.
    let allowed = match known_sink_args(plan_node.sink.0.as_str()) {
        Some(args) => args,
        None => return Err(DenyReason::UnknownSink(plan_node.sink.0.clone())),
    };

    // Step 2: every supplied arg must be allowed and appear at most once.
    let mut seen: Vec<&str> = Vec::with_capacity(plan_node.args.len());
    for arg in &plan_node.args {
        let name = arg.name.as_str();
        if !allowed.contains(&name) {
            return Err(DenyReason::UnknownArg(name.to_string()));
        }
        if seen.contains(&name) {
            return Err(DenyReason::DuplicateArg(name.to_string()));
        }
        seen.push(name);
    }

    // Step 3: every required (declared) arg must be present.
    for required in allowed {
        if !seen.contains(required) {
            return Err(DenyReason::MissingArg((*required).to_string()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::plan_node::{PlanArg, PlanNode, SinkId, ValueId};

    fn arg(name: &str) -> PlanArg {
        PlanArg {
            name: name.to_string(),
            value_id: ValueId::new(),
        }
    }

    fn node(sink: &str, args: Vec<PlanArg>) -> PlanNode {
        PlanNode {
            sink: SinkId(sink.to_string()),
            args,
        }
    }

    #[test]
    fn file_create_exact_args_ok() {
        let n = node("file.create", vec![arg("path"), arg("contents")]);
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn email_send_exact_args_ok() {
        let n = node(
            "email.send",
            vec![
                arg("to"),
                arg("cc"),
                arg("bcc"),
                arg("subject"),
                arg("body"),
            ],
        );
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn unknown_sink_denied() {
        let n = node("exec.shell", vec![arg("cmd")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownSink("exec.shell".to_string()))
        );
    }

    #[test]
    fn unknown_arg_denied() {
        let n = node("file.create", vec![arg("path"), arg("mode")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownArg("mode".to_string()))
        );
    }

    #[test]
    fn duplicate_arg_denied() {
        let n = node("file.create", vec![arg("path"), arg("path"), arg("contents")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::DuplicateArg("path".to_string()))
        );
    }

    #[test]
    fn missing_arg_denied() {
        let n = node("file.create", vec![arg("path")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::MissingArg("contents".to_string()))
        );
    }

    #[test]
    fn unknown_sink_checked_before_args() {
        // An unregistered sink is rejected as UnknownSink even with a bogus arg —
        // the sink check short-circuits before any per-arg evaluation.
        let n = node("http.post", vec![arg("nonsense")]);
        assert!(matches!(
            validate_schema(&n),
            Err(DenyReason::UnknownSink(_))
        ));
    }
}
