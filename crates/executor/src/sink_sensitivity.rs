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
        "file.write" => EffectClass::CommitIrreversible,
        "process.exec" => EffectClass::CommitIrreversible,
        // GIT-01 (Phase 36), DESIGN-git-github-http-sinks.md §1.2: the FIRST
        // non-CommitIrreversible REAL sink and a DELIBERATE, justified exception
        // to the fail-closed `_ => CommitIrreversible` default below. A local
        // commit is reversible (git reset / commit --amend / branch delete) and
        // causes NO external effect — only push/pr leave the trust boundary —
        // matching `ReversibleEffect` in the locked 3-class Effect ontology
        // (runtime-core/src/effect.rs). This is what lets an Allowed git.commit
        // survive an I1-demoted (Draft) session, exactly as a reversible
        // workspace file.write would.
        "git.commit" => EffectClass::MutateReversible,
        // HTTP-01 (Phase 37), DESIGN-git-github-http-sinks.md §3.2: the FIRST
        // real `Observe` sink (only the test-only `test.observe` is Observe
        // today). A read-only GET observes external state and causes no
        // outbound mutation, so it is Allowed even in a Draft session — but its
        // inbound response body is untrusted (HttpRaw) and DEMOTES the session
        // to Draft at mint time (wired in Plan 03). Classifying it Observe here
        // is a table row only; it introduces NO new ExecutorDecision variant
        // and does not weaken I2.
        "http.request" => EffectClass::Observe,
        // GITHUB-01 (Phase 38), DESIGN-git-github-http-sinks.md §4.1: opening a
        // pull request is an external, irreversible effect that crosses the trust
        // boundary — CommitIrreversible, exactly like email.send. This is a REAL
        // explicit arm (not the `_ => CommitIrreversible` fail-closed default
        // below) so a future refactor that reorders/removes the schema gate
        // cannot silently relax github.pr's class (T-38-03).
        "github.pr" => EffectClass::CommitIrreversible,
        // HTTP-W-01 (Phase 43), DESIGN-v1.9-egress-policy §2.0 (`[rev: MAJOR-1]`):
        // a WRITE POST/PUT is an external, irreversible effect that crosses the
        // trust boundary — CommitIrreversible, exactly like email.send/github.pr.
        // This is a REAL EXPLICIT arm (not the `_ => CommitIrreversible`
        // fail-closed default below), so a future refactor that reorders/removes
        // the schema gate cannot silently relax the WRITE id's class (T-43-01) —
        // exactly the github.pr precedent. It is ALSO redundantly covered by the
        // `_ =>` default. The distinct id is LOAD-BEARING for I0: the shipped GET
        // `http.request` id classes `Observe` and would fall through to Allowed
        // even in a draft session (`lib.rs` I0 gate fires only for
        // CommitIrreversible); classing the WRITE id CommitIrreversible here is
        // what makes a draft / untrusted-seeded session I0-deny a POST (the
        // MAJOR-1 I0-escape fix).
        "http.request.write" => EffectClass::CommitIrreversible,
        // GIT-02/03 (Phase 44), DESIGN-v1.9-egress-policy §1.1: a push crosses the
        // trust boundary and lands refs on a remote — an external, irreversible
        // effect, CommitIrreversible exactly like email.send / github.pr /
        // http.request.write. This is a REAL EXPLICIT arm (not the
        // `_ => CommitIrreversible` fail-closed default below), so a future
        // refactor that reorders/removes the schema gate cannot silently relax
        // git.push's class (T-44-01) — the github.pr / http.request.write
        // precedent. It is ALSO redundantly covered by the `_ =>` default. The
        // explicit CommitIrreversible class is what gives git.push the full
        // irreversible discipline: a draft / untrusted-seeded session I0-denies a
        // push (never an Observe fall-through), and it gets I2 collect-then-Block +
        // confirm-releasable.
        "git.push" => EffectClass::CommitIrreversible,
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

/// Args of file.create that determine WHAT is written (payload), not WHERE.
/// A tainted value here Blocks via the same collect-then-Block loop as any
/// other content-sensitive arg (HARDEN-05, v1.6). Scoped to `contents` ONLY
/// — `path` stays routing-sensitive, never content-sensitive (no
/// over-widening).
pub const FILE_CREATE_CONTENT_SENSITIVE: &[&str] = &["contents"];

/// Args of file.write that determine WHERE the effect writes.
/// A tainted value in `path` → `ExecutorDecision::BlockedPendingConfirmation`.
/// `contents` is content-sensitive (WHAT is written), not routing-sensitive —
/// same split as file.create.
pub const FILE_WRITE_ROUTING_SENSITIVE: &[&str] = &["path"];

/// Args of file.write that determine WHAT is written (payload), not WHERE.
/// A tainted value here Blocks via the same collect-then-Block loop as any
/// other content-sensitive arg (FS-03). Scoped to `contents` ONLY.
pub const FILE_WRITE_CONTENT_SENSITIVE: &[&str] = &["contents"];

/// Args of process.exec that determine WHAT command runs and WHERE (routing
/// sense: `cwd` determines where the command's relative-path resolution
/// happens; `command`/`args` determine what runs). DESIGN-effect-breadth-exec.md
/// §4.2: `command`/`args` are ALSO content-sensitive (see
/// `PROCESS_EXEC_CONTENT_SENSITIVE`) — the routing/content distinction is
/// academic for these two args; the point is neither classifier ever returns
/// `false` for them, so a tainted value in either Blocks.
pub const PROCESS_EXEC_ROUTING_SENSITIVE: &[&str] = &["command", "args", "cwd"];

/// Args of process.exec that determine WHAT irreversible payload executes.
/// A tainted `command`/`args` value is arbitrary code execution — strictly
/// worse than a tainted email recipient — so both are content-sensitive too
/// (DESIGN-effect-breadth-exec.md §4.2). `cwd` is deliberately NOT included
/// here (routing-sensitive only — it doesn't determine WHAT runs, only WHERE).
pub const PROCESS_EXEC_CONTENT_SENSITIVE: &[&str] = &["command", "args"];

/// Args of git.commit that determine WHAT irreversible payload the message
/// carries. GIT-01, DESIGN-git-github-http-sinks.md §1.3: `message` is the
/// taint CARRIER — a tainted value (e.g. assembled from untrusted file content
/// or exec output) Blocks under the UNMODIFIED collect-then-Block loop, exactly
/// like an email.send `body`. It must genuinely propagate downstream and MUST
/// NEVER be re-minted clean. git.commit has NO routing-sensitive arg (no
/// path/destination), so there is no matching `GIT_COMMIT_ROUTING_SENSITIVE`.
pub const GIT_COMMIT_CONTENT_SENSITIVE: &[&str] = &["message"];

/// Args of http.request that determine WHERE the GET goes. HTTP-01,
/// DESIGN-git-github-http-sinks.md §8: `url` is routing-sensitive — a tainted
/// url (e.g. assembled from untrusted file/exec/http content) redirects the
/// request to an attacker-chosen host and MUST Block on the `url` arg.
pub const HTTP_REQUEST_ROUTING_SENSITIVE: &[&str] = &["url"];

/// Args of http.request that determine WHAT data leaves the trust boundary.
/// HTTP-01, DESIGN-git-github-http-sinks.md §8 / DESIGN-GATE-RECORD-v1.8
/// NIT-6: `url` is ALSO content-sensitive (defense-in-depth) — a secret
/// assembled into the query string is exfiltration, so a tainted url Blocks
/// under the content classifier too, not only routing.
pub const HTTP_REQUEST_CONTENT_SENSITIVE: &[&str] = &["url"];

/// Args of github.pr that determine WHERE the PR lands. GITHUB-01/03,
/// DESIGN-git-github-http-sinks.md §4.4: `owner`/`repo` name the target
/// repository; `base`/`head` name the branches the PR merges between. A tainted
/// value in any of these mis-routes the PR to an attacker-chosen destination and
/// MUST Block on that arg (T-38-02). These are routing-only — they do NOT carry
/// the PR payload, so they are deliberately absent from
/// `GITHUB_PR_CONTENT_SENSITIVE` (no over-widening).
pub const GITHUB_PR_ROUTING_SENSITIVE: &[&str] = &["owner", "repo", "base", "head"];

/// Args of github.pr that determine WHAT payload leaves the trust boundary.
/// GITHUB-01/03, DESIGN-git-github-http-sinks.md §4.4: `title`/`body` are the PR
/// text — the marquee secret-exfil-via-PR-text arg. A tainted value (assembled
/// from `http_response`/`ExecRaw`/`doc_fragment` content) Blocks under the
/// UNMODIFIED collect-then-Block loop, exactly like an email.send `body`. It must
/// genuinely propagate downstream and MUST NEVER be re-minted clean (T-38-01).
pub const GITHUB_PR_CONTENT_SENSITIVE: &[&str] = &["title", "body"];

/// Args of http.request.write that determine WHERE the WRITE (POST/PUT) lands.
/// HTTP-W-01, DESIGN-v1.9-egress-policy §2.2: `url` is routing-sensitive — a
/// tainted url (assembled from untrusted file/exec/http content) redirects the
/// write to an attacker-chosen host and MUST Block on the `url` arg. `body` is
/// deliberately ABSENT here — it is payload, not routing (no over-widening);
/// `method` is governed by the enum gate (lib.rs), not I2 sensitivity.
pub const HTTP_REQUEST_WRITE_ROUTING_SENSITIVE: &[&str] = &["url"];

/// Args of http.request.write that determine WHAT payload leaves the trust
/// boundary. HTTP-W-01, DESIGN-v1.9-egress-policy §2.2: `body` is the marquee
/// exfiltration carrier — a value assembled from untrusted content routed into
/// the POST/PUT body Blocks under the UNMODIFIED collect-then-Block loop, exactly
/// like an email.send `body` or a github.pr `title`/`body`. `url` is ALSO
/// content-sensitive (defense-in-depth, mirroring the GET
/// `HTTP_REQUEST_CONTENT_SENSITIVE`) — a secret smuggled into the query string is
/// exfiltration too, so a tainted url Blocks under the content classifier as well
/// as routing. `method` is deliberately ABSENT — it is enum-gated (lib.rs), not
/// a taint carrier.
pub const HTTP_REQUEST_WRITE_CONTENT_SENSITIVE: &[&str] = &["url", "body"];

/// Args of git.push that determine WHERE the push lands. GIT-02/03 (Phase 44),
/// DESIGN-v1.9-egress-policy §1.3: `remote` names the destination and `refspec`
/// names the ref(s) written — a tainted value in EITHER steers the push to an
/// attacker-chosen destination and MUST Block on that arg (T-44-02). Both come
/// from TRUSTED session-creation intent, never the untrusted repo `.git/config`.
/// git.push has NO content-sensitive arg: the pushed PACK content is
/// worker-controlled and is surfaced at CONFIRM (DESIGN §1.6, Plan 44-04's
/// payload-provenance surface), NOT gated as an I2 content-sensitive arg — so
/// `remote`/`refspec` are deliberately absent from any content-sensitive set
/// (no over-widening) and `is_content_sensitive(git.push, ..)` falls through to
/// the fail-safe `_ => false` default.
pub const GIT_PUSH_ROUTING_SENSITIVE: &[&str] = &["remote", "refspec"];

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
        "file.write" => FILE_WRITE_ROUTING_SENSITIVE.contains(&arg_name),
        "process.exec" => PROCESS_EXEC_ROUTING_SENSITIVE.contains(&arg_name),
        "http.request" => HTTP_REQUEST_ROUTING_SENSITIVE.contains(&arg_name),
        "http.request.write" => HTTP_REQUEST_WRITE_ROUTING_SENSITIVE.contains(&arg_name),
        "github.pr" => GITHUB_PR_ROUTING_SENSITIVE.contains(&arg_name),
        "git.push" => GIT_PUSH_ROUTING_SENSITIVE.contains(&arg_name),
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
        "file.create" => FILE_CREATE_CONTENT_SENSITIVE.contains(&arg_name),
        "file.write" => FILE_WRITE_CONTENT_SENSITIVE.contains(&arg_name),
        "process.exec" => PROCESS_EXEC_CONTENT_SENSITIVE.contains(&arg_name),
        "git.commit" => GIT_COMMIT_CONTENT_SENSITIVE.contains(&arg_name),
        "http.request" => HTTP_REQUEST_CONTENT_SENSITIVE.contains(&arg_name),
        "http.request.write" => HTTP_REQUEST_WRITE_CONTENT_SENSITIVE.contains(&arg_name),
        "github.pr" => GITHUB_PR_CONTENT_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}

/// Returns the hardcoded expected-role set for `(sink, arg_name)`, or `None`
/// if the slot is UNCONSTRAINED (v1.5, DESIGN-slot-type-binding.md §3/§7).
///
/// Contract (load-bearing — the `Option` vs empty-slice distinction is the
/// fail-closed default itself; callers must match this `Option` explicitly
/// and must never collapse the `None` and `Some(&[])` states via an
/// unwrap-with-empty-default):
///   `None`           => this slot is unconstrained — the role check is a
///                        no-op for this arg. NOT fail-open: a deliberately
///                        scoped-out slot (DESIGN §7 item 3, Assumption A2).
///   `Some(&[roles])` => this slot IS role-checked — the resolved value's
///                        `origin_role` MUST be `Some` and MUST be one of
///                        `roles`, or the caller denies (DESIGN §7 items 1/2).
///   `Some(&[])`      => MUST NEVER be constructed by this function — a
///                        zero-valid-role slot is a design bug, not a runtime
///                        state.
///
/// v1.5 scope: hardcoded per-sink-arg table, mirroring `is_routing_sensitive`
/// / `is_content_sensitive` above — a security property, not a configuration
/// knob (CON-i2-non-bypassable). Scoped to the two live sinks only.
///
/// `body`'s list includes `"doc_fragment"` alongside the trusted `"body"`
/// spelling (Phase 24 Plan 03 correction to the DESIGN §3 table, traced
/// against live code, not a re-scoping): the ONLY production vocabulary for
/// hostile-extracted body content IS `"doc_fragment"` —
/// `cli/caprun/src/worker.rs`'s `SendEmailSummary` arm reports the `Body:`
/// marker fragment as `WorkerClaim::DocFragment`, which `server.rs`'s
/// `ReportClaims` dispatch (`claim_type: "doc_fragment"`) and
/// `mint_from_read` (role reused verbatim from `claim_type`) carry through
/// unchanged — there is no separate "body" claim_type anywhere in the
/// codebase. Omitting it here would fail-closed-Deny the exact CONTENT-01/
/// CONTROL-02 hostile-body-Block acceptance flow this project has shipped
/// since Phase 14, converting an intended human-confirmable Block into an
/// unconditional structural Deny. Safe under DESIGN §3/F4's table-
/// construction invariant: `body` is content-sensitive
/// (`is_content_sensitive`), so a tainted `doc_fragment`-tagged value here
/// still hits I2's per-arg Block regardless of role match — the role check
/// never becomes the sole gate for this untrusted vocabulary.
pub fn expected_role(sink: &SinkId, arg_name: &str) -> Option<&'static [&'static str]> {
    match sink.0.as_str() {
        "email.send" => match arg_name {
            "to" | "cc" | "bcc" => Some(&["recipient", "email_address"]),
            "subject" => Some(&["subject"]),
            "body" => Some(&["body", "doc_fragment"]),
            _ => None,
        },
        "file.create" => match arg_name {
            "path" => Some(&["path", "relative_path"]),
            // HARDEN-05 (v1.6): `contents` is role-checked to `Some(&["path"])`
            // — the load-bearing, non-negotiable pin (DESIGN-security-hardening.md
            // §e). This is NOT a new role name; it's a deliberate reuse of the
            // `"path"` role because the planner (`cli/caprun/src/planner.rs:208`)
            // reuses the SAME trusted `"path"`-role `intent_value_id` in BOTH the
            // `path` and `contents` slots — no `"contents"`/`"file_body"`
            // role-producing mint site exists in the codebase. Any list omitting
            // `"path"` would hard-Deny the only live `file.create` flow. Wires the
            // slot into I2's role check for the day a real content-extraction
            // pipeline (D-12, deferred) mints a doc-derived `contents` claim; a
            // present no-op on the live path today.
            "contents" => Some(&["path"]),
            _ => None,
        },
        "file.write" => match arg_name {
            // Mirrors file.create's `path` role list verbatim (DESIGN §4.3).
            "path" => Some(&["path", "relative_path"]),
            // WIDER than file.create's `contents` role list
            // (`Some(&["path"])`) by design: unlike file.create, file.write
            // is the live sink target of a chained process.exec -> file.write
            // flow (Phase 32 EXEC-01..04). A tainted exec output is minted
            // with `origin_role = "exec_output"`, and hostile-extracted doc
            // content reuses the `"doc_fragment"` vocabulary already admitted
            // for email.send's `body` slot. Excluding either would
            // fail-closed-Deny that LEGITIMATE-shape flow at this structural
            // Step 1c role gate instead of letting it reach I2's
            // content-sensitivity Block (RESEARCH A4 / DESIGN §4.3) — the
            // security property still holds because `contents` is
            // content-sensitive above, so a tainted value here Blocks
            // regardless of role match.
            "contents" => Some(&["path", "exec_output", "doc_fragment"]),
            _ => None,
        },
        "process.exec" => match arg_name {
            // DESIGN-effect-breadth-exec.md §4.2 (Round-1 finding M2): `command`
            // and `args` are DELIBERATELY unconstrained here. There is no
            // `origin_role`-producing mint site for a legitimately-authored
            // exec command — pinning `Some(...)` would fail-closed-Deny the
            // LEGITIMATE command at this Step 1c structural gate, breaking the
            // feature rather than tightening it. The security property (a
            // tainted `command`/`args` value Blocks) is delivered entirely by
            // `is_routing_sensitive`/`is_content_sensitive` = true above, plus
            // the untrusted-taint check — independent of `expected_role`.
            // `None` here disables ONLY the structural role gate; it is NOT
            // an I2 bypass.
            "command" | "args" => None,
            // `cwd` reuses the same trusted path-role vocabulary as
            // `file.create`'s `path` (RESEARCH A3 recommendation) — DESIGN
            // pins `cwd` routing-sensitive but leaves its role list to this
            // sink table's own construction.
            "cwd" => Some(&["path", "relative_path"]),
            _ => None,
        },
        "git.commit" => match arg_name {
            // GIT-01, DESIGN-git-github-http-sinks.md §1.3: `message` is
            // DELIBERATELY unconstrained at the structural Step-1c role gate —
            // reuse the process.exec command/args rationale. There is no
            // origin_role-producing mint site for a legitimately-authored commit
            // message, so pinning Some(...) would fail-closed-Deny the legit
            // UserTrusted-message flow. The Block for a tainted message comes
            // entirely from is_content_sensitive + the untrusted-taint check —
            // this `None` disables ONLY the structural role gate; it is NOT an
            // I2 bypass.
            "message" => None,
            _ => None,
        },
        "http.request" => match arg_name {
            // HTTP-01, DESIGN-git-github-http-sinks.md §8: `url` is DELIBERATELY
            // unconstrained at the structural Step-1c role gate — reuse the
            // process.exec/git.commit rationale. There is no origin_role-
            // producing mint site for a legitimately-authored url, so pinning
            // Some(...) would fail-closed-Deny the legit UserTrusted-url flow.
            // The Block for a tainted url comes entirely from
            // is_routing_sensitive/is_content_sensitive + the untrusted-taint
            // check — this `None` disables ONLY the structural role gate; it is
            // NOT an I2 bypass.
            "url" => None,
            _ => None,
        },
        "http.request.write" => match arg_name {
            // HTTP-W-01, DESIGN-v1.9-egress-policy §2.2: url/body/method are all
            // DELIBERATELY unconstrained at the structural Step-1c role gate —
            // reuse the process.exec/git.commit/http.request/github.pr rationale.
            // There is no origin_role-producing mint site for a legitimately-
            // authored write url/body, so pinning Some(...) would fail-closed-Deny
            // the legit UserTrusted-authored WRITE flow. The Block for a tainted
            // url/body comes entirely from is_routing_sensitive/is_content_sensitive
            // + the untrusted-taint check — this `None` disables ONLY the structural
            // role gate; it is NOT an I2 bypass. `method` is governed by the
            // fail-closed enum gate in lib.rs, not this role gate.
            "url" | "body" | "method" => None,
            _ => None,
        },
        "github.pr" => match arg_name {
            // GITHUB-01/03, DESIGN-git-github-http-sinks.md §4.4: all six args
            // are DELIBERATELY unconstrained at the structural Step-1c role gate
            // — reuse the process.exec/git.commit/http.request rationale. There
            // is no origin_role-producing mint site for a legitimately-authored
            // PR field (owner/repo/base/head/title/body), so pinning Some(...)
            // would fail-closed-Deny the legit UserTrusted-authored PR flow. The
            // Block for a tainted value comes entirely from
            // is_routing_sensitive/is_content_sensitive + the untrusted-taint
            // check — this `None` disables ONLY the structural role gate; it is
            // NOT an I2 bypass.
            "owner" | "repo" | "base" | "head" | "title" | "body" => None,
            _ => None,
        },
        "git.push" => match arg_name {
            // GIT-02/03, DESIGN-v1.9-egress-policy §1.3: remote/refspec are
            // DELIBERATELY unconstrained at the structural Step-1c role gate —
            // reuse the http.request.write/github.pr/git.commit rationale. There
            // is no origin_role-producing mint site for a legitimately-authored
            // trusted-intent push destination, so pinning Some(...) would
            // fail-closed-Deny the legit flow. The Block for a tainted
            // remote/refspec comes entirely from is_routing_sensitive + the
            // untrusted-taint check — this `None` disables ONLY the structural
            // role gate; it is NOT an I2 bypass.
            "remote" | "refspec" => None,
            _ => None,
        },
        _ => None, // any other sink: unconstrained, out of v1.5 scope
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

    fn process_exec() -> SinkId {
        SinkId("process.exec".to_string())
    }

    fn file_write() -> SinkId {
        SinkId("file.write".to_string())
    }

    fn git_commit() -> SinkId {
        SinkId("git.commit".to_string())
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

    // -----------------------------------------------------------------
    // expected_role (T2-03, DESIGN-slot-type-binding.md §3)
    // -----------------------------------------------------------------

    #[test]
    fn email_send_to_cc_bcc_expect_recipient_or_email_address() {
        for arg in ["to", "cc", "bcc"] {
            assert_eq!(
                expected_role(&email(), arg),
                Some(&["recipient", "email_address"][..]),
                "email.send `{arg}` must expect [recipient, email_address]"
            );
        }
    }

    #[test]
    fn email_send_subject_expects_subject_only() {
        assert_eq!(expected_role(&email(), "subject"), Some(&["subject"][..]));
    }

    #[test]
    fn email_send_body_expects_body_or_doc_fragment() {
        // "doc_fragment" is the untrusted spelling — the ONLY vocabulary
        // production code uses for hostile-extracted `Body:` content
        // (worker.rs's SendEmailSummary arm, mirroring the recipient/path
        // dual-vocabulary pattern). Without it, a genuinely-tainted body
        // would fail-closed-Deny at Step 1c instead of reaching I2's Block.
        assert_eq!(
            expected_role(&email(), "body"),
            Some(&["body", "doc_fragment"][..])
        );
    }

    #[test]
    fn email_send_unknown_arg_is_unconstrained() {
        assert_eq!(expected_role(&email(), "attachment"), None);
    }

    #[test]
    fn file_create_path_expects_path_or_relative_path() {
        assert_eq!(
            expected_role(&file_create(), "path"),
            Some(&["path", "relative_path"][..])
        );
    }

    #[test]
    fn file_create_contents_expects_path() {
        // HARDEN-05 (v1.6): `contents` is no longer unconstrained. The ONLY
        // live production value ever routed into this slot is the reused
        // trusted `"path"`-role literal (`cli/caprun/src/planner.rs:208`) —
        // so `Some(&["path"])` is the load-bearing, non-negotiable pin that
        // keeps that flow green while structurally wiring the slot into I2
        // for the day a real content-extraction pipeline mints a
        // doc-derived `contents` claim.
        assert_eq!(
            expected_role(&file_create(), "contents"),
            Some(&["path"][..]),
            "file.create `contents` must expect [path] (HARDEN-05)"
        );
    }

    #[test]
    fn file_create_path_not_content_sensitive() {
        // Defense-in-depth guard (Pitfall 5): the content-sensitivity arm
        // must be scoped to `contents` ONLY — an unconditional `"file.create"
        // => true` would wrongly widen `path` (routing-sensitive only) into
        // content-sensitive too.
        assert!(
            !is_content_sensitive(&file_create(), "path"),
            "file.create `path` must NOT become content-sensitive (no over-widening)"
        );
    }

    #[test]
    fn file_create_contents_is_content_sensitive() {
        assert!(
            is_content_sensitive(&file_create(), "contents"),
            "file.create `contents` must be content-sensitive (HARDEN-05)"
        );
    }

    #[test]
    fn file_create_unknown_arg_is_unconstrained() {
        assert_eq!(expected_role(&file_create(), "mode"), None);
    }

    #[test]
    fn unknown_sink_expected_role_is_unconstrained() {
        assert_eq!(expected_role(&other(), "to"), None);
        assert_eq!(expected_role(&other(), "url"), None);
    }

    // -----------------------------------------------------------------
    // process.exec (EXEC-01/02, DESIGN-effect-breadth-exec.md §4.2)
    // -----------------------------------------------------------------

    #[test]
    fn process_exec_command_and_args_routing_and_content_sensitive() {
        for arg in ["command", "args"] {
            assert!(
                is_routing_sensitive(&process_exec(), arg),
                "process.exec `{arg}` must be routing-sensitive"
            );
            assert!(
                is_content_sensitive(&process_exec(), arg),
                "process.exec `{arg}` must be content-sensitive"
            );
        }
    }

    #[test]
    fn process_exec_cwd_routing_but_not_content_sensitive() {
        assert!(
            is_routing_sensitive(&process_exec(), "cwd"),
            "process.exec `cwd` determines WHERE relative paths resolve — routing-sensitive"
        );
        assert!(
            !is_content_sensitive(&process_exec(), "cwd"),
            "process.exec `cwd` does not determine WHAT runs — not content-sensitive"
        );
    }

    #[test]
    fn process_exec_is_commit_irreversible() {
        assert_eq!(
            sink_effect_class(&process_exec()),
            EffectClass::CommitIrreversible
        );
    }

    #[test]
    fn process_exec_command_and_args_expected_role_is_none() {
        // Round-1 M2: command/args are deliberately unconstrained at the
        // structural Step 1c role gate — the Block comes from
        // is_routing_sensitive/is_content_sensitive + taint, not this gate.
        assert_eq!(expected_role(&process_exec(), "command"), None);
        assert_eq!(expected_role(&process_exec(), "args"), None);
    }

    #[test]
    fn process_exec_cwd_expects_path_or_relative_path() {
        assert_eq!(
            expected_role(&process_exec(), "cwd"),
            Some(&["path", "relative_path"][..])
        );
    }

    // -----------------------------------------------------------------
    // file.write (FS-03, DESIGN-effect-breadth-exec.md §4.1/§4.3)
    // -----------------------------------------------------------------

    #[test]
    fn file_write_path_is_routing_sensitive() {
        assert!(
            is_routing_sensitive(&file_write(), "path"),
            "file.write `path` routes the write — must be routing-sensitive"
        );
    }

    #[test]
    fn file_write_contents_not_routing_sensitive() {
        assert!(
            !is_routing_sensitive(&file_write(), "contents"),
            "file.write `contents` is WHAT is written, not WHERE — not routing-sensitive"
        );
    }

    #[test]
    fn file_write_contents_is_content_sensitive() {
        assert!(
            is_content_sensitive(&file_write(), "contents"),
            "file.write `contents` must be content-sensitive"
        );
    }

    #[test]
    fn file_write_path_not_content_sensitive() {
        assert!(
            !is_content_sensitive(&file_write(), "path"),
            "file.write `path` must NOT become content-sensitive (no over-widening)"
        );
    }

    #[test]
    fn file_write_is_commit_irreversible() {
        assert_eq!(
            sink_effect_class(&file_write()),
            EffectClass::CommitIrreversible
        );
    }

    #[test]
    fn file_write_path_expects_path_or_relative_path() {
        assert_eq!(
            expected_role(&file_write(), "path"),
            Some(&["path", "relative_path"][..])
        );
    }

    #[test]
    fn file_write_contents_expects_path_exec_output_or_doc_fragment() {
        // WIDER than file.create's contents role list (Some(&["path"])) —
        // admits Phase 32's exec_output origin_role (chained
        // process.exec -> file.write) and the doc_fragment vocabulary, so a
        // tainted value here reaches I2's content-sensitivity Block rather
        // than a structural Step-1c Deny (DESIGN §4.3, RESEARCH A4).
        assert_eq!(
            expected_role(&file_write(), "contents"),
            Some(&["path", "exec_output", "doc_fragment"][..]),
            "file.write `contents` must expect [path, exec_output, doc_fragment]"
        );
    }

    #[test]
    fn file_write_unknown_arg_is_unconstrained() {
        assert_eq!(expected_role(&file_write(), "mode"), None);
    }

    // -----------------------------------------------------------------
    // git.commit (GIT-01, DESIGN-git-github-http-sinks.md §1.2/§1.3)
    // -----------------------------------------------------------------

    #[test]
    fn git_commit_is_mutate_reversible() {
        // The FIRST non-CommitIrreversible REAL sink: a local commit is
        // reversible (git reset/amend/branch-delete) with no external effect,
        // so it survives an I1-demoted (Draft) session.
        assert_eq!(
            sink_effect_class(&git_commit()),
            EffectClass::MutateReversible,
            "git.commit is a reversible local mutation, not a commit-irreversible effect"
        );
    }

    #[test]
    fn git_commit_message_is_content_sensitive() {
        // `message` is the taint CARRIER — a tainted message Blocks under the
        // unmodified collect-then-Block loop, exactly like an email.send body.
        assert!(
            is_content_sensitive(&git_commit(), "message"),
            "git.commit `message` must be content-sensitive (taint carrier)"
        );
    }

    #[test]
    fn git_commit_message_not_routing_sensitive() {
        // git.commit has NO routing-sensitive arg (no path/destination) — the
        // message falls through to the `_ => false` default rather than a table
        // row.
        assert!(
            !is_routing_sensitive(&git_commit(), "message"),
            "git.commit `message` is WHAT is committed, not WHERE — not routing-sensitive"
        );
    }

    #[test]
    fn git_commit_message_expected_role_is_none() {
        // `message` is deliberately unconstrained at the structural Step-1c role
        // gate (reuse the process.exec command/args rationale): no
        // origin_role-producing mint site exists for a legitimately-authored
        // commit message, so pinning Some(...) would fail-closed-Deny the legit
        // UserTrusted-message flow. The Block for a tainted message comes
        // entirely from is_content_sensitive + the untrusted-taint check.
        assert_eq!(expected_role(&git_commit(), "message"), None);
    }

    // -----------------------------------------------------------------
    // http.request (HTTP-01, DESIGN-git-github-http-sinks.md §3.2/§8)
    // -----------------------------------------------------------------

    fn http_request() -> SinkId {
        SinkId("http.request".to_string())
    }

    #[test]
    fn http_request_is_observe() {
        // The FIRST real Observe sink (only the test-only test.observe is
        // Observe today): a GET is a read, Allowed even in a Draft session.
        // Its inbound response demotes the session at mint time (Plan 03).
        assert_eq!(
            sink_effect_class(&http_request()),
            EffectClass::Observe,
            "http.request is a read-only Observe GET, not a commit-irreversible effect"
        );
    }

    #[test]
    fn http_request_url_is_routing_and_content_sensitive() {
        // url decides WHERE the GET goes (routing) AND can smuggle a secret
        // in the query string (content, NIT-6/§8 defense-in-depth) — a tainted
        // url Blocks under either classifier.
        assert!(
            is_routing_sensitive(&http_request(), "url"),
            "http.request `url` decides WHERE the GET goes — routing-sensitive"
        );
        assert!(
            is_content_sensitive(&http_request(), "url"),
            "http.request `url` can exfiltrate a secret in the query string — content-sensitive"
        );
    }

    #[test]
    fn http_request_url_expected_role_is_none() {
        // `url` is deliberately unconstrained at the structural Step-1c role
        // gate (reuse the process.exec/git.commit rationale): no
        // origin_role-producing mint site exists for a legitimately-authored
        // url. The Block for a tainted url comes entirely from
        // routing/content-sensitivity + the untrusted-taint check — this None
        // is NOT an I2 bypass.
        assert_eq!(expected_role(&http_request(), "url"), None);
    }

    // -----------------------------------------------------------------
    // github.pr (GITHUB-01/03, DESIGN-git-github-http-sinks.md §4.1/§4.4/§8)
    // -----------------------------------------------------------------

    fn github_pr() -> SinkId {
        SinkId("github.pr".to_string())
    }

    #[test]
    fn github_pr_is_commit_irreversible() {
        // A PR is an external, irreversible effect — an explicit arm, never the
        // `_` fail-closed default reached only incidentally (GITHUB-01, §4.1).
        assert_eq!(
            sink_effect_class(&github_pr()),
            EffectClass::CommitIrreversible,
            "github.pr is an external irreversible effect (explicit arm, not `_` default)"
        );
    }

    #[test]
    fn github_pr_title_body_content_sensitive() {
        // title/body are the payload that leaves the boundary — the marquee
        // secret-exfil-via-PR-text arg (GITHUB-03/§4.4). A tainted value Blocks
        // under the unmodified collect-then-Block loop.
        for arg in ["title", "body"] {
            assert!(
                is_content_sensitive(&github_pr(), arg),
                "github.pr `{arg}` must be content-sensitive (exfil carrier)"
            );
        }
    }

    #[test]
    fn github_pr_owner_repo_base_head_routing_sensitive() {
        // owner/repo/base/head determine WHERE the PR lands — a tainted value
        // mis-routes the PR and must Block (routing-sensitive).
        for arg in ["owner", "repo", "base", "head"] {
            assert!(
                is_routing_sensitive(&github_pr(), arg),
                "github.pr `{arg}` must be routing-sensitive (PR destination)"
            );
        }
    }

    #[test]
    fn github_pr_routing_args_not_content_sensitive() {
        // No over-widening: routing args stay routing-only.
        for arg in ["owner", "repo", "base", "head"] {
            assert!(
                !is_content_sensitive(&github_pr(), arg),
                "github.pr `{arg}` is WHERE the PR lands, not payload — not content-sensitive"
            );
        }
    }

    #[test]
    fn github_pr_expected_role_is_none() {
        // All six args deliberately unconstrained at the structural Step-1c role
        // gate (reuse the process.exec/git.commit/http.request rationale): no
        // origin_role-producing mint site exists for a legitimately-authored PR
        // field, so pinning Some(...) would fail-closed-Deny the legit flow. The
        // Block for a tainted value comes entirely from routing/content-
        // sensitivity + the untrusted-taint check — this None is NOT an I2 bypass.
        for arg in ["owner", "repo", "base", "head", "title", "body"] {
            assert_eq!(
                expected_role(&github_pr(), arg),
                None,
                "github.pr `{arg}` must be role-unconstrained (None)"
            );
        }
    }

    // -----------------------------------------------------------------
    // http.request.write (HTTP-W-01, DESIGN-v1.9-egress-policy §2.0/§2.2)
    // -----------------------------------------------------------------

    fn http_request_write() -> SinkId {
        SinkId("http.request.write".to_string())
    }

    #[test]
    fn http_request_write_is_commit_irreversible() {
        // The EXPLICIT arm (contrast with the GET `http.request` Observe row):
        // a WRITE POST/PUT is an external irreversible effect, so it gets the
        // full I0-draft-deny + I2-Block + confirm-releasable discipline. This is
        // the load-bearing MAJOR-1 I0-escape fix (§2.0) — a distinct id classed
        // CommitIrreversible, never the Observe GET id.
        assert_eq!(
            sink_effect_class(&http_request_write()),
            EffectClass::CommitIrreversible,
            "http.request.write must be CommitIrreversible (explicit arm, §2.0)"
        );
        // And the GET id is DISTINCT — still Observe (unchanged).
        assert_eq!(
            sink_effect_class(&http_request()),
            EffectClass::Observe,
            "GET http.request must stay Observe — distinct from the WRITE id"
        );
    }

    #[test]
    fn http_request_write_url_is_routing_and_content_sensitive() {
        // url decides WHERE the write lands (routing) AND can smuggle a secret in
        // the query string (content, §2.2 defense-in-depth) — Blocks under either.
        assert!(
            is_routing_sensitive(&http_request_write(), "url"),
            "http.request.write `url` decides WHERE the write lands — routing-sensitive"
        );
        assert!(
            is_content_sensitive(&http_request_write(), "url"),
            "http.request.write `url` can exfiltrate a secret in the query string — content-sensitive"
        );
    }

    #[test]
    fn http_request_write_body_content_but_not_routing_sensitive() {
        // body is the marquee exfil payload — content-sensitive (§2.2) — but it
        // is payload, not routing (no over-widening).
        assert!(
            is_content_sensitive(&http_request_write(), "body"),
            "http.request.write `body` must be content-sensitive (exfil carrier)"
        );
        assert!(
            !is_routing_sensitive(&http_request_write(), "body"),
            "http.request.write `body` is payload, not routing — not routing-sensitive"
        );
    }

    #[test]
    fn http_request_write_method_not_sensitive() {
        // method is governed by the fixed {POST,PUT} enum gate (lib.rs), NOT by
        // I2 sensitivity — so it is neither routing- nor content-sensitive.
        assert!(
            !is_routing_sensitive(&http_request_write(), "method"),
            "http.request.write `method` is enum-gated, not routing-sensitive"
        );
        assert!(
            !is_content_sensitive(&http_request_write(), "method"),
            "http.request.write `method` is enum-gated, not content-sensitive"
        );
    }

    #[test]
    fn http_request_write_expected_role_is_none() {
        // url/body/method are deliberately unconstrained at the structural Step-1c
        // role gate (reuse the http.request/github.pr rationale): no origin_role-
        // producing mint site for a legit write. The Block comes from
        // sensitivity + taint, not this gate — None is NOT an I2 bypass.
        for arg in ["url", "body", "method"] {
            assert_eq!(
                expected_role(&http_request_write(), arg),
                None,
                "http.request.write `{arg}` must be role-unconstrained (None)"
            );
        }
    }

    // -----------------------------------------------------------------
    // git.push (GIT-02/03, DESIGN-v1.9-egress-policy §1.1/§1.3)
    // -----------------------------------------------------------------

    fn git_push() -> SinkId {
        SinkId("git.push".to_string())
    }

    #[test]
    fn git_push_is_commit_irreversible() {
        // A push is an external, irreversible effect — an EXPLICIT arm, never the
        // `_` fail-closed default reached only incidentally (GIT-02/03, §1.1). The
        // explicit class is what makes a draft/untrusted-seeded session I0-deny a
        // push (never an Observe fall-through), T-44-01.
        assert_eq!(
            sink_effect_class(&git_push()),
            EffectClass::CommitIrreversible,
            "git.push is an external irreversible effect (explicit arm, not `_` default)"
        );
    }

    #[test]
    fn git_push_remote_refspec_routing_sensitive() {
        // remote/refspec determine WHERE the push lands — a tainted value
        // mis-routes the push and must Block (routing-sensitive, T-44-02).
        for arg in ["remote", "refspec"] {
            assert!(
                is_routing_sensitive(&git_push(), arg),
                "git.push `{arg}` must be routing-sensitive (push destination)"
            );
        }
    }

    #[test]
    fn git_push_has_no_content_sensitive_arg() {
        // The pushed PACK content is worker-controlled and surfaced at CONFIRM
        // (DESIGN §1.6), NOT an I2 content-sensitive arg — no over-widening.
        // remote/refspec are routing-only.
        for arg in ["remote", "refspec"] {
            assert!(
                !is_content_sensitive(&git_push(), arg),
                "git.push `{arg}` is routing/destination, not payload — not content-sensitive"
            );
        }
    }

    #[test]
    fn git_push_expected_role_is_none() {
        // remote/refspec deliberately unconstrained at the structural Step-1c role
        // gate (reuse the http.request.write/github.pr rationale): no origin_role-
        // producing mint site for a legit trusted-intent push. The Block comes from
        // routing-sensitivity + taint, not this gate — None is NOT an I2 bypass.
        for arg in ["remote", "refspec"] {
            assert_eq!(
                expected_role(&git_push(), arg),
                None,
                "git.push `{arg}` must be role-unconstrained (None)"
            );
        }
    }
}
