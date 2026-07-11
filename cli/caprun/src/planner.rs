/// planner — deterministic, non-LLM intent-to-plan-node mapper (PLAN-02)
///
/// # Security invariants (PLAN-03 / I2)
///
/// This module holds ONLY opaque `ValueId` handles — it NEVER sees:
///  - A `ValueRecord` (literal + taint + provenance_chain)
///  - A raw byte slice or string from untrusted content
///  - A taint label
///
/// The function signature enforces this at compile time: the only value-typed
/// parameters are `intent: &CaprunIntent` (typed, user-trusted) and `ValueId`
/// handles. The broker-owned `ValueStore` keeps the literals and taint; the
/// planner references values by their opaque handles only.
///
/// # No I/O, no async, infallible
///
/// `plan_from_intent` is a pure function. It performs no I/O, no async
/// operations, and is infallible (`-> PlanNode`, not `-> Result<PlanNode>`).
/// It MUST NOT call `ValueStore::mint` or construct a `ValueRecord`.
///
/// # Routed by CALL-SITE CONVENTION, not provenance (Phase 15 finding #7)
///
/// `to`/`path` and `body` are placed by whichever handle the CALLER (the
/// confined worker) hands in via `derived_recipient`/`body` — the planner
/// structurally CANNOT see provenance or taint (PLAN-03), so it never makes a
/// "which handle is tainted" decision itself; it only places named handles by
/// convention. The executor (not the planner) makes the trust decision.
///
/// # The `Planner` seam (PLANNER-01 / PLANNER-04, Phase 20)
///
/// `pub trait Planner` below is the swappable seam: `DeterministicPlanner`
/// (this module) implements it today by delegating to `plan_from_intent`
/// unchanged; Phase 21's adversarial `LlmPlanner` will implement the SAME
/// trait, letting the worker call site (`cli/caprun/src/worker.rs`) swap
/// planners without any change to its own code. The trait method carries the
/// identical PLANNER-04 structural guarantee `plan_from_intent` already
/// documents above: its only value-typed parameters are `&CaprunIntent`
/// (typed, user-trusted) and opaque `ValueId` handles — never a
/// `ValueRecord`, a raw byte slice/string from untrusted content, or a taint
/// label. This is enforced at compile time by the trait method's own
/// signature, exactly as it is for the free fn.

use anyhow::Context;
use llm_planner::{HandleLabel, PlannerRequest, PlannerResponse};
use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
// Duration/Instant back the connect-retry loop in the Linux-only
// `connect_to_sidecar` below; unused on the non-Linux stub sibling.
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

/// The `Planner` seam (PLANNER-01): maps a typed intent + opaque `ValueId`
/// handles to a `PlanNode`. See the module doc above for the PLANNER-04
/// compile-time boundary this trait method preserves — implementors may
/// never accept a `ValueRecord`, a raw byte slice/string from untrusted
/// content, or a taint label.
pub trait Planner {
    /// Map a typed `CaprunIntent` + opaque `ValueId` handles to a `PlanNode`.
    /// Parameters mirror `plan_from_intent` exactly (see its doc below).
    fn plan(
        &self,
        intent: &CaprunIntent,
        intent_value_id: ValueId,
        derived_recipient: Option<ValueId>,
        body: Option<ValueId>,
        trusted_subject_handle: ValueId,
        trusted_body_handle: ValueId,
    ) -> PlanNode;
}

/// The deterministic planner implementation (PLAN-02): delegates to
/// `plan_from_intent` unchanged. This is the concrete `Planner` the worker
/// constructs today; Phase 21's `LlmPlanner` will implement the same trait.
pub struct DeterministicPlanner;

impl Planner for DeterministicPlanner {
    fn plan(
        &self,
        intent: &CaprunIntent,
        intent_value_id: ValueId,
        derived_recipient: Option<ValueId>,
        body: Option<ValueId>,
        trusted_subject_handle: ValueId,
        trusted_body_handle: ValueId,
    ) -> PlanNode {
        plan_from_intent(
            intent,
            intent_value_id,
            derived_recipient,
            body,
            trusted_subject_handle,
            trusted_body_handle,
        )
    }
}

/// Map a typed `CaprunIntent` to a single `PlanNode`.
///
/// The planner holds ONLY opaque `ValueId` handles — never the literal or taint.
/// Taint lives in the broker-owned `ValueStore`; the planner is not aware of it.
///
/// # Arguments
/// * `intent`           — the typed user intent (enum, never free-form text).
/// * `intent_value_id`  — the `UserTrusted` `ValueId` minted by `mint_from_intent`
///                        for the intent's primary trusted literal (email
///                        recipient / file.create path).
/// * `derived_recipient` — routed by CALL-SITE CONVENTION (finding #7), not
///                        provenance: for `SendEmailSummary`, `Some` iff the
///                        confined worker derived a genuine multi-fragment
///                        recipient (finding #8's resolved fork); for
///                        `CreateFileFromReport`, `Some` iff a tainted
///                        workspace-derived path claim exists. `None` in
///                        either case falls back to `intent_value_id`.
/// * `body`             — routed by CALL-SITE CONVENTION: `Some` iff the
///                        worker extracted a tainted body fragment (email
///                        only); `None` falls back to `trusted_body_handle`.
///                        Unused by `CreateFileFromReport`.
/// * `trusted_subject_handle` — the UserTrusted handle for `email.send/subject`
///                        (Phase 15 finding #6 — always a DISTINCT handle from
///                        `intent_value_id`, never the literal). Unused by
///                        `CreateFileFromReport`.
/// * `trusted_body_handle` — the UserTrusted fallback handle for
///                        `email.send/body` when `body` is `None` (finding
///                        #6 — distinct from `intent_value_id`/
///                        `trusted_subject_handle`). Unused by
///                        `CreateFileFromReport`.
///
/// # Returns
///
/// A `PlanNode` with the appropriate sink and args. All args are opaque `ValueId`
/// handles; no literals or taint labels appear in the returned node.
///
/// # Security (PLAN-03)
///
/// The `..` in each match arm intentionally ignores struct fields (e.g. `recipient`)
/// inside the `CaprunIntent` variant. The literal already lives in the broker's
/// `ValueStore`, accessible only via the returned `ValueId` handle — the planner
/// never needs (and must never access) the literal directly.
pub fn plan_from_intent(
    intent: &CaprunIntent,
    intent_value_id: ValueId,
    derived_recipient: Option<ValueId>,
    body: Option<ValueId>,
    trusted_subject_handle: ValueId,
    trusted_body_handle: ValueId,
) -> PlanNode {
    match intent {
        CaprunIntent::SendEmailSummary { .. } => {
            // `to`: the derived (doc-sourced) recipient handle when present
            // (hostile path — routing-sensitive, tainted → Block downstream),
            // else the UserTrusted intent handle (clean path → Allowed).
            let to = derived_recipient.unwrap_or_else(|| intent_value_id.clone());
            // `body`: the derived (doc-sourced) tainted body handle when
            // present (content-sensitive, tainted → Block downstream), else
            // the UserTrusted trusted-body handle (clean path → Allowed).
            let body_value_id = body.unwrap_or(trusted_body_handle);
            PlanNode {
                sink: SinkId("email.send".into()),
                args: vec![
                    PlanArg { name: "to".into(), value_id: to },
                    // `subject` is ALWAYS the UserTrusted handle — Phase 15
                    // (EXTRACT-01) introduces no doc-derived subject
                    // extraction; a genuinely distinct handle from
                    // `intent_value_id`/`trusted_body_handle` (finding #6),
                    // so a clean send is not degenerately
                    // to==subject==body==recipient.
                    PlanArg { name: "subject".into(), value_id: trusted_subject_handle },
                    PlanArg { name: "body".into(), value_id: body_value_id },
                ],
            }
        }
        CaprunIntent::CreateFileFromReport { .. } => {
            // Route `path` (routing-sensitive) by handle CALL-SITE CONVENTION,
            // making both §9 paths reachable for 07-05:
            //   * hostile — if the workspace read yielded a tainted
            //     RelativePath claim, the caller passes it as
            //     `derived_recipient` (ExternalUntrusted, PathRaw) → the
            //     executor sees an untrusted routing arg → Block.
            //   * clean   — otherwise `derived_recipient` is None → route the
            //     UserTrusted intent path → Allowed → the broker invokes the
            //     live file.create sink.
            // The planner only chooses a handle; it never sees the literal or
            // taint (PLAN-03) — the broker-owned ValueStore holds those, and
            // the executor (not the planner) makes the trust decision.
            let path_value_id = derived_recipient.unwrap_or_else(|| intent_value_id.clone());
            PlanNode {
                sink: SinkId("file.create".into()),
                args: vec![
                    PlanArg { name: "path".into(), value_id: path_value_id },
                    // `contents` is content-sensitive (WHAT is written), never
                    // routing-sensitive — a tainted value here does not block. Use
                    // the trusted intent handle so a value always resolves.
                    PlanArg { name: "contents".into(), value_id: intent_value_id },
                ],
            }
        }
    }
}

/// # LlmPlanner — the adversarial LLM-backed planner (Phase 21 / PLANNER-03)
///
/// Implements the SAME `Planner` trait as `DeterministicPlanner` above — the
/// worker constructs whichever concrete planner `CAPRUN_PLANNER` selects and
/// calls `.plan()` through the trait object identically either way. There is
/// no cross-connection handle resolution and no change to the worker's own
/// broker connection or submission path: `LlmPlanner::plan()` only computes
/// and returns a `PlanNode`; the worker still submits it via
/// `BrokerRequest::SubmitPlanNode` on its own existing connection, exactly as
/// it does today with `DeterministicPlanner`.
///
/// `LlmPlanner` is a thin, non-network shim INSIDE the confined worker: it
/// forwards only the opaque `ValueId` handles it was given (tagged with a
/// human-readable slot hint) to the off-process `caprun-planner` sidecar over
/// an abstract-namespace UDS, and maps the sidecar's tool-call reply back to
/// a `PlanNode` via the pure, unit-testable `response_to_plan_node`
/// validator. It NEVER holds a literal, a `ValueRecord`, or a taint label
/// (PLAN-03) — the same compile-time boundary `Planner::plan`'s signature
/// already enforces for `DeterministicPlanner`.
///
/// # Fail-closed (T-21-08)
///
/// Any sidecar/transport/validation failure — connect failure, a malformed
/// or unparseable reply, an unknown sink, or a `value_id` the sidecar was
/// never offered — causes `plan()` to print a clear message to stderr and
/// `std::process::exit(1)` immediately. No `PlanNode` is ever returned or
/// submitted on that path, and therefore no effect can run (mirrors the
/// worker's existing fail-closed exit on a §9 block, see `worker.rs`).
pub struct LlmPlanner {
    planner_sock: String,
}

impl LlmPlanner {
    /// `planner_sock` is the abstract-socket name WITHOUT the leading NUL
    /// (mirrors `BROKER_SOCK`'s convention, see `worker.rs`) — `LlmPlanner`
    /// prepends the NUL itself when connecting.
    pub fn new(planner_sock: String) -> Self {
        LlmPlanner { planner_sock }
    }

    /// Connect to `\0` + `self.planner_sock`, send the framed `PlannerRequest`,
    /// and receive the framed reply. Any connect/read/parse failure surfaces
    /// as an `Err` — `plan()` treats that identically to any other sidecar
    /// failure: fail closed, submit no `PlanNode`.
    fn request_plan_from_sidecar(&self, request: &PlannerRequest) -> anyhow::Result<PlannerResponse> {
        let stream = connect_to_sidecar(&self.planner_sock)?;
        send_framed(&stream, request).context("send PlannerRequest to sidecar")?;
        let reply: SidecarReply =
            recv_framed(&stream).context("receive SidecarReply from sidecar")?;
        match reply {
            SidecarReply::Ok { response } => Ok(response),
            SidecarReply::Error { message } => {
                anyhow::bail!("sidecar returned an error reply: {message}")
            }
        }
    }
}

/// Mirror of `cli/caprun-planner`'s `SidecarReply` wire shape (see that
/// crate's `main.rs` doc comment, which documents this exact JSON shape for
/// Plan 21-03's proxy to match). Defined independently here (not shared via a
/// common crate), per Plan 21-03's own documented decision — Plan 21-01 is
/// closed and neither sibling plan's file list touches `llm-planner`.
///
/// # Bug found and fixed during Plan 21-04's live composed run
///
/// The ORIGINAL implementation called `recv_framed::<PlannerResponse>` —
/// deserializing the frame body DIRECTLY as a bare `PlannerResponse` — but
/// the sidecar's `handle_connection` ALWAYS wraps its reply in
/// `{"status":"ok","response":{"sink":...,"args":[...]}}` /
/// `{"status":"error","message":"..."}`. A bare-`PlannerResponse` parse
/// therefore failed on EVERY real sidecar reply (the top-level JSON object
/// has no `sink`/`args` keys — those are nested one level down, under
/// `response`), 100% reproducible against the real sidecar — empirically
/// confirmed live on Linux (`scripts/mailpit-verify.sh`, Plan 21-04): the
/// real OpenAI call DID complete inside the sidecar (confirmed via the
/// sidecar's own startup log line reaching stderr), but the worker-side
/// proxy could never parse ANY reply, success or failure, before this fix.
/// This is exactly the composition risk Plan 21-03's own SUMMARY flagged as
/// unverified pending this plan's live run: the two sibling plans (21-02,
/// 21-03) were built in parallel worktrees against a documented-but-untested
/// wire contract, and the contract text and the implementation had silently
/// diverged.
#[derive(serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SidecarReply {
    Ok { response: PlannerResponse },
    Error { message: String },
}

impl Planner for LlmPlanner {
    fn plan(
        &self,
        intent: &CaprunIntent,
        intent_value_id: ValueId,
        derived_recipient: Option<ValueId>,
        body: Option<ValueId>,
        trusted_subject_handle: ValueId,
        trusted_body_handle: ValueId,
    ) -> PlanNode {
        let (request, offered, known_sinks, canonical_names) = build_planner_request(
            intent,
            &intent_value_id,
            derived_recipient.as_ref(),
            body.as_ref(),
            &trusted_subject_handle,
            &trusted_body_handle,
        );

        let response = match self.request_plan_from_sidecar(&request) {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!(
                    "[llm-planner] sidecar request failed: {e} — failing closed, no PlanNode submitted"
                );
                std::process::exit(1);
            }
        };

        match response_to_plan_node(&response, &offered, &known_sinks, &canonical_names) {
            Ok(plan_node) => plan_node,
            Err(e) => {
                eprintln!(
                    "[llm-planner] response validation failed: {e} — failing closed, no PlanNode submitted"
                );
                std::process::exit(1);
            }
        }
    }
}

/// Build the `PlannerRequest` for `intent` from the SAME six routing params
/// `Planner::plan` receives, choosing the effective handle for each of the
/// three offered slots (`recipient`/`subject`/`body`) via the IDENTICAL
/// override rule `plan_from_intent` above already uses:
///   - `recipient` = `derived_recipient` when `Some`, else `intent_value_id`.
///   - `subject`   = `trusted_subject_handle` (never overridden — matches
///                   `plan_from_intent`'s unconditional use).
///   - `body`      = `body` when `Some`, else `trusted_body_handle`.
///
/// Returns the request alongside the `offered` handle set, `known_sinks`
/// list, and `canonical_names` mapping so the caller can pass them,
/// UNCHANGED, into `response_to_plan_node` — the validator's allowlists are
/// always exactly what this function put on the wire, never re-derived or
/// guessed. Carries only `ValueId` handles + slot hints and a typed
/// `intent_kind`/`available_sinks` label — no other value-bearing field (the
/// `PlannerRequest`/`HandleLabel` types themselves are structurally
/// incapable of carrying a literal, per `llm-planner`'s own key-set tests).
///
/// `canonical_names` pairs each offered handle with the EXACT arg name
/// `crates/executor/src/sink_schema.rs`'s hardcoded per-sink schema requires
/// (`"to"`/`"subject"`/`"body"` for `email.send`) — see `response_to_plan_node`'s
/// doc comment for why this exists and is NEVER the model's own `arg.name`.
pub fn build_planner_request(
    intent: &CaprunIntent,
    intent_value_id: &ValueId,
    derived_recipient: Option<&ValueId>,
    body: Option<&ValueId>,
    trusted_subject_handle: &ValueId,
    trusted_body_handle: &ValueId,
) -> (PlannerRequest, Vec<ValueId>, Vec<String>, Vec<(ValueId, String)>) {
    let recipient = derived_recipient
        .cloned()
        .unwrap_or_else(|| intent_value_id.clone());
    let subject = trusted_subject_handle.clone();
    let body_handle = body.cloned().unwrap_or_else(|| trusted_body_handle.clone());

    let (intent_kind, available_sinks): (&str, Vec<String>) = match intent {
        CaprunIntent::SendEmailSummary { .. } => {
            ("SendEmailSummary", vec!["email.send".to_string()])
        }
        CaprunIntent::CreateFileFromReport { .. } => {
            ("CreateFileFromReport", vec!["file.create".to_string()])
        }
    };

    let available_handles = vec![
        HandleLabel { slot_hint: "recipient".to_string(), value_id: recipient.clone() },
        HandleLabel { slot_hint: "subject".to_string(), value_id: subject.clone() },
        HandleLabel { slot_hint: "body".to_string(), value_id: body_handle.clone() },
    ];

    let offered = vec![recipient.clone(), subject.clone(), body_handle.clone()];

    // The recipient/subject/body slot -> canonical sink-arg-name mapping,
    // per intent kind. `CreateFileFromReport` only maps its routing
    // (`recipient`-slot) handle to `"path"` — `file.create`'s OTHER required
    // arg (`"contents"`) is always `intent_value_id` in `plan_from_intent`'s
    // own deterministic mapping, never one of this function's three offered
    // slots, so `file.create` via the LLM planner is intentionally left
    // unmapped for `subject`/`body` here (out of this plan's tested scope —
    // only the `SendEmailSummary` clean path is proven live by Plan 21-04; a
    // `file.create` mismatch still fails closed via `validate_schema`,
    // unchanged from before this fix).
    let canonical_names: Vec<(ValueId, String)> = match intent {
        CaprunIntent::SendEmailSummary { .. } => vec![
            (recipient.clone(), "to".to_string()),
            (subject.clone(), "subject".to_string()),
            (body_handle.clone(), "body".to_string()),
        ],
        CaprunIntent::CreateFileFromReport { .. } => {
            vec![(recipient.clone(), "path".to_string())]
        }
    };

    let request = PlannerRequest {
        intent_kind: intent_kind.to_string(),
        available_handles,
        available_sinks: available_sinks.clone(),
    };

    (request, offered, available_sinks, canonical_names)
}

/// Map a validated sidecar `PlannerResponse` to a `PlanNode`, failing closed
/// (T-21-08): `Ok` ONLY when `resp.sink` is a member of `known_sinks` AND
/// every `ResponseArg.value_id` is a member of `offered`. Never fabricates or
/// substitutes a handle — any violation is a hard `Err`. Pure and
/// unit-testable without a live sidecar.
///
/// # Bug found and fixed during Plan 21-04's live composed run: arg names are
/// NEVER taken from the model's own `response_arg.name`
///
/// The ORIGINAL implementation copied `response_arg.name` verbatim into the
/// final `PlanArg`. Nothing in `build_planner_prompt`/`build_tool_schema`
/// (`crates/llm-planner`) tells the model which literal arg-name string a
/// given sink expects — the model only sees `slot_hint`s
/// ("recipient"/"subject"/"body"), which do NOT match
/// `crates/executor/src/sink_schema.rs`'s hardcoded `email.send` schema
/// (`{"to","cc","bcc","subject","body"}`). A real model reliably named the
/// recipient arg something other than `"to"` (matching the `slot_hint` it
/// was shown instead), so `sink_schema::validate_schema` correctly
/// `Denied(UnknownArg(..))` the resulting plan node on EVERY real run before
/// this fix — empirically confirmed live on Linux
/// (`scripts/mailpit-verify.sh`): `plan_node_evaluated` was recorded (a
/// `Denied` outcome uses the same generic event as `Allowed` — only
/// `BlockedPendingConfirmation` gets its own `sink_blocked` event type), but
/// NO `email_send_attempted`/`email_send_succeeded` event ever followed, and
/// (compounding the symptom) `cli/caprun/src/worker.rs` only exited non-zero
/// on `BlockedPendingConfirmation`, silently exiting 0 on `Denied` too (see
/// that file's own fix note) — together making a 100%-reproducible schema
/// rejection look, from the outside, like a quiet no-op success.
///
/// The FIX never trusts the model's `arg.name` string at all: it looks up
/// the CALLER-SUPPLIED `canonical_names` mapping (built by
/// `build_planner_request`, which alone knows which offered `ValueId` is the
/// recipient/subject/body slot for the CHOSEN sink) by `value_id` identity,
/// and uses THAT name. This keeps the "never trust the planner" security
/// posture consistent with the rest of this module (PLANNER-04): the model
/// only ever gets to pick WHICH offered handle occupies a slot (already
/// enforced by the `offered`-membership check below) and WHICH sink to use
/// (already enforced by the `known_sinks` check below) — never the arg name
/// a sink's schema requires. A `value_id` absent from `canonical_names`
/// falls back to the response's own name (harmless: `validate_schema` still
/// fails it closed exactly as before, e.g. the `file.create` subject/body
/// slots noted in `build_planner_request`'s doc comment).
pub fn response_to_plan_node(
    resp: &PlannerResponse,
    offered: &[ValueId],
    known_sinks: &[String],
    canonical_names: &[(ValueId, String)],
) -> anyhow::Result<PlanNode> {
    if !known_sinks.iter().any(|s| s == &resp.sink) {
        anyhow::bail!("llm planner response named unknown sink: {}", resp.sink);
    }

    let mut args = Vec::with_capacity(resp.args.len());
    for response_arg in &resp.args {
        if !offered.iter().any(|v| v == &response_arg.value_id) {
            anyhow::bail!(
                "llm planner response referenced an unoffered handle: {:?}",
                response_arg.value_id
            );
        }
        let name = canonical_names
            .iter()
            .find(|(vid, _)| vid == &response_arg.value_id)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| response_arg.name.clone());
        args.push(PlanArg {
            name,
            value_id: response_arg.value_id.clone(),
        });
    }

    Ok(PlanNode { sink: SinkId(resp.sink.clone()), args })
}

/// Connect to the abstract-namespace socket named `planner_sock` with a
/// bounded connect-retry, mirroring the worker's own broker-connect loop
/// (`cli/caprun/src/worker.rs`) — the sidecar (spawned by `caprun` main just
/// before the worker) may not have reached its synchronous `bind()` yet when
/// the worker calls `plan()`.
///
/// # Bug found and fixed during Plan 21-04's live composed run
///
/// The ORIGINAL implementation called `UnixStream::connect(&format!("\0{planner_sock}"))`
/// — i.e. `std::os::unix::net::UnixStream`'s plain path-based `connect`, given
/// a string with a LEADING NUL byte, on the (correct, in isolation) theory
/// that this is "the same convention" as the broker/worker's abstract-socket
/// connect elsewhere in this codebase (`cli/caprun/src/worker.rs`,
/// `crates/brokerd/src/server.rs`). It is NOT: those call sites use
/// `tokio::net::UnixStream`, whose implementation specifically detects a
/// leading NUL byte and constructs an abstract-namespace `SocketAddr`
/// internally (bypassing the interior-nul check — see those modules' own doc
/// comments). Plain `std::os::unix::net`'s path-based `connect`/`bind`,
/// however, ALWAYS goes through the generic `sockaddr_un` path-construction
/// helper, which unconditionally rejects ANY nul byte in the path —
/// including a leading one — with `io::ErrorKind::InvalidInput` ("paths must
/// not contain interior null bytes"). Because `InvalidInput` is neither
/// `ConnectionRefused` nor `NotFound`, the retry loop's guard never matched:
/// every real invocation failed on the FIRST attempt, in well under a
/// millisecond, indistinguishable at a glance from "the sidecar never came
/// up" — empirically confirmed live on Linux (`scripts/mailpit-verify.sh`,
/// Plan 21-04): the real OpenAI call was NEVER REACHED, `LlmPlanner::plan()`
/// failed closed on every run before this fix, 100% reproducible, unrelated
/// to any timing race.
///
/// The FIX uses the stable (since Rust 1.70) Linux-only
/// `std::os::linux::net::SocketAddrExt::from_abstract_name` to construct the
/// abstract-namespace `SocketAddr` explicitly, then
/// `UnixStream::connect_addr`, which never routes through the
/// interior-nul-rejecting path helper. This is std's own sanctioned API for
/// exactly this use case (as opposed to tokio's incidental leading-NUL
/// special-casing) and requires no new dependency. Gated `#[cfg(target_os =
/// "linux")]` (the `std::os::linux::net` module does not exist on macOS) with
/// a `#[cfg(not(target_os = "linux"))]` sibling that still compiles on the
/// macOS dev box but fails fast at runtime, mirroring every other Linux-only
/// abstract-socket path in this codebase (see CLAUDE.md, "Linux-only
/// security tests").
#[cfg(target_os = "linux")]
fn connect_to_sidecar(planner_sock: &str) -> anyhow::Result<UnixStream> {
    use std::os::linux::net::SocketAddrExt;
    const CONNECT_BUDGET: Duration = Duration::from_secs(2);
    const RETRY_DELAY: Duration = Duration::from_millis(25);
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(planner_sock.as_bytes())
        .context("build abstract-namespace socket address for LLM planner sidecar")?;
    let deadline = Instant::now() + CONNECT_BUDGET;
    loop {
        match UnixStream::connect_addr(&addr) {
            Ok(stream) => return Ok(stream),
            // Transient: the sidecar subprocess has not reached bind() yet.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(RETRY_DELAY);
            }
            Err(e) => return Err(e).context("connect to LLM planner sidecar abstract UDS"),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn connect_to_sidecar(_planner_sock: &str) -> anyhow::Result<UnixStream> {
    anyhow::bail!(
        "LLM planner sidecar connect uses an abstract-namespace UDS, which is Linux-only \
         (std::os::linux::net); this path only needs to COMPILE on macOS, exactly like the \
         worker's own broker-connect and caprun-planner's own listener bind"
    )
}

/// Write a framed message (4-byte LE length prefix + JSON body) — same wire
/// format as the worker's broker helpers of the same name
/// (`cli/caprun/src/worker.rs`); duplicated here (not imported) because this
/// module is ALSO compiled standalone by `tests/planner.rs` via `#[path]`,
/// which has no access to `worker.rs`'s private items.
fn send_framed(stream: &UnixStream, msg: &impl serde::Serialize) -> anyhow::Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    (&*stream).write_all(&len)?;
    (&*stream).write_all(&body)?;
    Ok(())
}

/// Read a framed message (4-byte LE length prefix + JSON body) — see
/// `send_framed`'s doc for why this duplicates (rather than imports)
/// `worker.rs`'s helper of the same name.
fn recv_framed<T: serde::de::DeserializeOwned>(stream: &UnixStream) -> anyhow::Result<T> {
    let mut len_buf = [0u8; 4];
    (&*stream).read_exact(&mut len_buf)?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    (&*stream).read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
