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
/// Each sink declares two sets:
/// - `allowed` — every arg name the sink accepts. An arg not in this set →
///   `UnknownArg`; a repeated arg → `DuplicateArg`. Extra args always fail closed.
/// - `required` — the subset that MUST be present; absent → `MissingArg`.
///
/// `file.create` requires exactly `{path, contents}` (both allowed and required —
/// SINK-01). `email.send` keeps its pre-07-04a semantics: this plan registers its
/// arg NAMES (rejecting unknown/duplicate args) but adds NO required-arg gate, so
/// existing single-arg `email.send` evaluations are unchanged. Its live invocation
/// shape is finalized in 07-04b.
use runtime_core::plan_node::PlanNode;
use runtime_core::DenyReason;

/// A hardcoded per-sink argument schema.
pub struct SinkSchema {
    /// Sink id (e.g. `"file.create"`).
    pub sink: &'static str,
    /// Every accepted arg name. Anything outside this set → `UnknownArg`.
    pub allowed: &'static [&'static str],
    /// Args that MUST be present. Absent → `MissingArg`.
    pub required: &'static [&'static str],
}

/// Hardcoded registry of every callable sink and its arg schema.
///
/// A sink absent from this table is not callable (`UnknownSink`, fail-closed).
pub const KNOWN_SINKS: &[SinkSchema] = &[
    SinkSchema {
        // Matches the current live shape: routing args (to/cc/bcc) +
        // content-sensitive args (subject/body, per
        // `sink_sensitivity::EMAIL_SEND_CONTENT_SENSITIVE`). Attachment support
        // is DESCOPED for v1.3 (D-23) — removed from BOTH this set and
        // `EMAIL_SEND_CONTENT_SENSITIVE` atomically, so a plan node carrying
        // that arg is `Denied(UnknownArg)` here at Step 0, before any
        // sensitivity evaluation ever runs.
        sink: "email.send",
        allowed: &["to", "cc", "bcc", "subject", "body"],
        required: &[],
    },
    SinkSchema {
        sink: "file.create",
        allowed: &["path", "contents"],
        required: &["path", "contents"],
    },
    SinkSchema {
        // FS-03 (Phase 33 Plan 02): mirrors file.create exactly — both args
        // required, exact-match. No optional-arg asymmetry like
        // process.exec's args/cwd.
        sink: "file.write",
        allowed: &["path", "contents"],
        required: &["path", "contents"],
    },
    SinkSchema {
        // DESIGN-effect-breadth-exec.md §1.5/§4.1: `command` is required;
        // `args`/`cwd` are optional. Both `command` and `args` are
        // routing- AND content-sensitive (sink_sensitivity.rs) — a tainted
        // value Blocks rather than Denies here; this schema gate only
        // enforces the arg NAME set, not taint.
        sink: "process.exec",
        allowed: &["command", "args", "cwd"],
        required: &["command"],
    },
    SinkSchema {
        // GIT-01 (Phase 36 Plan 01), DESIGN-git-github-http-sinks.md §1.3 /
        // CONTEXT decision 3: exact-match, single-arg schema. `message` is the
        // sole arg — both allowed AND required, mirroring file.write's
        // exact-match shape (NOT process.exec's optional-arg asymmetry). No
        // paths/pathspec arg is modeled: Phase 36 commits already-STAGED
        // workspace changes (ROADMAP Phase 36 success criterion 1), so
        // `git commit -m <message>` needs no pathspec — keeping the arg set to a
        // single exact-match arg tightens the Step-0 fail-closed gate. `message`
        // is content-sensitive (sink_sensitivity.rs) — a tainted value Blocks
        // rather than Denies here; this schema gate enforces only the arg NAME
        // set, not taint.
        sink: "git.commit",
        allowed: &["message"],
        required: &["message"],
    },
    SinkSchema {
        // HTTP-01 (Phase 37 Plan 01), DESIGN-git-github-http-sinks.md §3.1/§3.2
        // / CONTEXT decision 2: exact-match, single-arg schema. `url` is the
        // sole arg — both allowed AND required. GET only this milestone: NO
        // method/headers/body args are modeled (DESIGN §0/§3.1), so a plan node
        // carrying any of those is Denied(UnknownArg) here at Step 0. `url` is
        // routing- AND content-sensitive (sink_sensitivity.rs) — a tainted url
        // Blocks downstream, it does NOT Deny here; this schema gate enforces
        // only the arg NAME set, not taint.
        sink: "http.request",
        allowed: &["url"],
        required: &["url"],
    },
    SinkSchema {
        // GITHUB-01/03 (Phase 38 Plan 01), DESIGN-git-github-http-sinks.md
        // §4.1/§4.4 / CONTEXT decisions 1/4: exact-match, all-six-required
        // schema. `allowed` and `required` are BOTH exactly
        // {owner,repo,base,head,title,body} — mirroring file.write's
        // exact-match shape, NOT process.exec's optional-arg asymmetry. No
        // draft/maintainer_can_modify/headers/method args are modeled this
        // milestone (PR-create scope), so a plan node carrying any is
        // Denied(UnknownArg) here at Step 0. title/body are content-sensitive
        // and owner/repo/base/head routing-sensitive per sink_sensitivity.rs —
        // a tainted value Blocks downstream, it does NOT Deny here; this schema
        // gate enforces only the arg NAME set, not taint.
        sink: "github.pr",
        allowed: &["owner", "repo", "base", "head", "title", "body"],
        required: &["owner", "repo", "base", "head", "title", "body"],
    },
];

/// Test-fixture-only sink registry (RESEARCH.md Pitfall 3 / DESIGN §9 Pitfall
/// m2): a fixture sink with a minimal/empty arg schema, existing solely so
/// TAINT-03 (`Draft` + `Observe` still Allowed) can be driven through the FULL
/// `submit_plan_node` path — Step 0 schema gate -> per-arg loop -> Step 0.5 —
/// rather than unit-testing `sink_effect_class` in isolation. Both live
/// entries in `KNOWN_SINKS` are `CommitIrreversible`, so without this fixture
/// TAINT-03 has no real sink to exercise. This NEVER appears in the production
/// `KNOWN_SINKS` surface.
///
/// Gated on `any(test, feature = "test-fixtures")` rather than plain
/// `#[cfg(test)]`: bare `#[cfg(test)]` items are invisible to integration
/// tests in `tests/` (they link the crate as a normal, non-`--cfg test`
/// dependency) — the `test-fixtures` feature, enabled only via this crate's
/// self dev-dependency in Cargo.toml, makes the fixture visible there too
/// while still being absent from any production build.
#[cfg(any(test, feature = "test-fixtures"))]
pub const TEST_KNOWN_SINKS: &[SinkSchema] = &[SinkSchema {
    sink: "test.observe",
    allowed: &[],
    required: &[],
}];

/// The schema for `sink`, or `None` if the sink is not registered.
pub fn schema_for(sink: &str) -> Option<&'static SinkSchema> {
    KNOWN_SINKS
        .iter()
        .find(|s| s.sink == sink)
        .or_else(|| test_schema_for(sink))
}

/// Test-fixture-only lookup into `TEST_KNOWN_SINKS`. Under a production build
/// this always returns `None` (`test.observe` is not a callable sink in
/// production).
#[cfg(any(test, feature = "test-fixtures"))]
fn test_schema_for(sink: &str) -> Option<&'static SinkSchema> {
    TEST_KNOWN_SINKS.iter().find(|s| s.sink == sink)
}

#[cfg(not(any(test, feature = "test-fixtures")))]
fn test_schema_for(_sink: &str) -> Option<&'static SinkSchema> {
    None
}

/// Validate a plan node's sink + arg set against the hardcoded schema.
///
/// Ordering (fail-closed, checked BEFORE resolve/sensitivity in `submit_plan_node`):
///   1. Unknown sink → `UnknownSink` (nothing else is checked).
///   2. Per arg, in order: not in `allowed` → `UnknownArg`; already seen →
///      `DuplicateArg`.
///   3. After scanning args: any `required` arg absent → `MissingArg`.
///
/// Returns `Ok(())` iff the sink is registered, carries no unknown/duplicate args,
/// and includes every required arg.
pub fn validate_schema(plan_node: &PlanNode) -> Result<(), DenyReason> {
    // Step 1: the sink must be registered. An unregistered sink fails closed.
    let schema = match schema_for(plan_node.sink.0.as_str()) {
        Some(s) => s,
        None => return Err(DenyReason::UnknownSink(plan_node.sink.0.clone())),
    };

    // Step 2: every supplied arg must be allowed and appear at most once.
    let mut seen: Vec<&str> = Vec::with_capacity(plan_node.args.len());
    for arg in &plan_node.args {
        let name = arg.name.as_str();
        if !schema.allowed.contains(&name) {
            return Err(DenyReason::UnknownArg(name.to_string()));
        }
        if seen.contains(&name) {
            return Err(DenyReason::DuplicateArg(name.to_string()));
        }
        seen.push(name);
    }

    // Step 3: every required arg must be present.
    for required in schema.required {
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
    fn email_send_subset_args_ok() {
        // email.send has NO required args (pre-07-04a semantics preserved): a
        // single-arg node (as the executor/broker tests build) still validates.
        let n = node("email.send", vec![arg("to")]);
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

    // -----------------------------------------------------------------
    // process.exec (EXEC-01/02, DESIGN-effect-breadth-exec.md §1.5/§4.1)
    // -----------------------------------------------------------------

    #[test]
    fn process_exec_command_only_ok() {
        let n = node("process.exec", vec![arg("command")]);
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn process_exec_full_args_ok() {
        let n = node(
            "process.exec",
            vec![arg("command"), arg("args"), arg("cwd")],
        );
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn process_exec_missing_command_denied() {
        let n = node("process.exec", vec![arg("args")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::MissingArg("command".to_string()))
        );
    }

    #[test]
    fn process_exec_unknown_arg_denied() {
        let n = node("process.exec", vec![arg("command"), arg("env")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownArg("env".to_string()))
        );
    }

    #[test]
    fn process_exec_duplicate_arg_denied() {
        let n = node("process.exec", vec![arg("command"), arg("command")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::DuplicateArg("command".to_string()))
        );
    }

    // -----------------------------------------------------------------
    // file.write (FS-03, DESIGN-effect-breadth-exec.md §4.1/§4.3)
    // -----------------------------------------------------------------

    #[test]
    fn file_write_exact_args_ok() {
        let n = node("file.write", vec![arg("path"), arg("contents")]);
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn file_write_unknown_arg_denied() {
        let n = node("file.write", vec![arg("path"), arg("contents"), arg("mode")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownArg("mode".to_string()))
        );
    }

    #[test]
    fn file_write_missing_required_arg_denied() {
        let n = node("file.write", vec![arg("path")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::MissingArg("contents".to_string()))
        );
    }

    // -----------------------------------------------------------------
    // git.commit (GIT-01, DESIGN-git-github-http-sinks.md §1.2/§1.3)
    // -----------------------------------------------------------------

    #[test]
    fn git_commit_is_registered_sink() {
        // Registered => schema_for returns Some => never UnknownSink at Step 0.
        assert!(
            schema_for("git.commit").is_some(),
            "git.commit must be a registered sink (never UnknownSink)"
        );
    }

    #[test]
    fn git_commit_exact_args_ok() {
        let n = node("git.commit", vec![arg("message")]);
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn git_commit_unknown_arg_denied() {
        // `paths`/pathspec is NOT modeled — Phase 36 commits already-staged
        // changes, so `git commit -m <message>` needs no pathspec arg.
        let n = node("git.commit", vec![arg("message"), arg("paths")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownArg("paths".to_string()))
        );
    }

    #[test]
    fn git_commit_duplicate_arg_denied() {
        let n = node("git.commit", vec![arg("message"), arg("message")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::DuplicateArg("message".to_string()))
        );
    }

    #[test]
    fn git_commit_missing_required_arg_denied() {
        let n = node("git.commit", vec![]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::MissingArg("message".to_string()))
        );
    }

    // -----------------------------------------------------------------
    // http.request (HTTP-01, DESIGN-git-github-http-sinks.md §3.1/§3.2)
    // -----------------------------------------------------------------

    #[test]
    fn http_request_is_registered_sink() {
        // Registered => schema_for returns Some => never UnknownSink at Step 0.
        assert!(
            schema_for("http.request").is_some(),
            "http.request must be a registered sink (never UnknownSink)"
        );
    }

    #[test]
    fn http_request_exact_args_ok() {
        let n = node("http.request", vec![arg("url")]);
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn http_request_unknown_arg_denied() {
        // GET only this milestone — no method/headers/body args modeled.
        let n = node("http.request", vec![arg("url"), arg("method")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownArg("method".to_string()))
        );
    }

    #[test]
    fn http_request_duplicate_arg_denied() {
        let n = node("http.request", vec![arg("url"), arg("url")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::DuplicateArg("url".to_string()))
        );
    }

    #[test]
    fn http_request_missing_required_arg_denied() {
        let n = node("http.request", vec![]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::MissingArg("url".to_string()))
        );
    }

    // -----------------------------------------------------------------
    // github.pr (GITHUB-01/03, DESIGN-git-github-http-sinks.md §4.1/§4.4)
    // -----------------------------------------------------------------

    #[test]
    fn github_pr_is_registered_sink() {
        // Registered => schema_for returns Some => never UnknownSink at Step 0.
        assert!(
            schema_for("github.pr").is_some(),
            "github.pr must be a registered sink (never UnknownSink)"
        );
    }

    #[test]
    fn github_pr_exact_args_ok() {
        let n = node(
            "github.pr",
            vec![
                arg("owner"),
                arg("repo"),
                arg("base"),
                arg("head"),
                arg("title"),
                arg("body"),
            ],
        );
        assert_eq!(validate_schema(&n), Ok(()));
    }

    #[test]
    fn github_pr_unknown_arg_denied() {
        // No draft/maintainer_can_modify/headers/method args modeled this
        // milestone (PR-create scope) — any extra fails closed at Step 0.
        let n = node(
            "github.pr",
            vec![
                arg("owner"),
                arg("repo"),
                arg("base"),
                arg("head"),
                arg("title"),
                arg("body"),
                arg("draft"),
            ],
        );
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownArg("draft".to_string()))
        );
    }

    #[test]
    fn github_pr_duplicate_arg_denied() {
        let n = node(
            "github.pr",
            vec![
                arg("owner"),
                arg("owner"),
                arg("repo"),
                arg("base"),
                arg("head"),
                arg("title"),
                arg("body"),
            ],
        );
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::DuplicateArg("owner".to_string()))
        );
    }

    #[test]
    fn github_pr_missing_required_arg_denied() {
        // All six args are required — any absent → MissingArg.
        let n = node(
            "github.pr",
            vec![
                arg("owner"),
                arg("repo"),
                arg("base"),
                arg("head"),
                arg("title"),
            ],
        );
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::MissingArg("body".to_string()))
        );
    }

    #[test]
    fn exec_shell_fixture_remains_distinct_unknown_sink() {
        // Regression guard (RESEARCH Pitfall): "exec.shell" is a permanently-
        // rejected UnknownSink test fixture string (see unknown_sink_denied
        // above), NOT a collision with the real "process.exec" sink id.
        let n = node("exec.shell", vec![arg("cmd")]);
        assert_eq!(
            validate_schema(&n),
            Err(DenyReason::UnknownSink("exec.shell".to_string()))
        );
    }
}
