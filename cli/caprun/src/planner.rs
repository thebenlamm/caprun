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
        recv_framed(&stream).context("receive PlannerResponse from sidecar")
    }
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
        let (request, offered, known_sinks) = build_planner_request(
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

        match response_to_plan_node(&response, &offered, &known_sinks) {
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
/// Returns the request alongside the `offered` handle set and `known_sinks`
/// list so the caller can pass them, UNCHANGED, into `response_to_plan_node`
/// — the validator's allowlists are always exactly what this function put on
/// the wire, never re-derived or guessed. Carries only `ValueId` handles +
/// slot hints and a typed `intent_kind`/`available_sinks` label — no other
/// value-bearing field (the `PlannerRequest`/`HandleLabel` types themselves
/// are structurally incapable of carrying a literal, per `llm-planner`'s own
/// key-set tests).
pub fn build_planner_request(
    intent: &CaprunIntent,
    intent_value_id: &ValueId,
    derived_recipient: Option<&ValueId>,
    body: Option<&ValueId>,
    trusted_subject_handle: &ValueId,
    trusted_body_handle: &ValueId,
) -> (PlannerRequest, Vec<ValueId>, Vec<String>) {
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

    let offered = vec![recipient, subject, body_handle];

    let request = PlannerRequest {
        intent_kind: intent_kind.to_string(),
        available_handles,
        available_sinks: available_sinks.clone(),
    };

    (request, offered, available_sinks)
}

/// Map a validated sidecar `PlannerResponse` to a `PlanNode`, failing closed
/// (T-21-08): `Ok` ONLY when `resp.sink` is a member of `known_sinks` AND
/// every `ResponseArg.value_id` is a member of `offered`. Never fabricates or
/// substitutes a handle — any violation is a hard `Err`. Pure and
/// unit-testable without a live sidecar.
pub fn response_to_plan_node(
    resp: &PlannerResponse,
    offered: &[ValueId],
    known_sinks: &[String],
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
        args.push(PlanArg {
            name: response_arg.name.clone(),
            value_id: response_arg.value_id.clone(),
        });
    }

    Ok(PlanNode { sink: SinkId(resp.sink.clone()), args })
}

/// Connect to `\0` + `planner_sock` with a bounded connect-retry, mirroring
/// the worker's own broker-connect loop (`cli/caprun/src/worker.rs`) — the
/// sidecar (spawned by `caprun` main just before the worker) may not have
/// reached its synchronous `bind()` yet when the worker calls `plan()`.
fn connect_to_sidecar(planner_sock: &str) -> anyhow::Result<UnixStream> {
    const CONNECT_BUDGET: Duration = Duration::from_secs(2);
    const RETRY_DELAY: Duration = Duration::from_millis(25);
    let sock_path = format!("\0{planner_sock}");
    let deadline = Instant::now() + CONNECT_BUDGET;
    loop {
        match UnixStream::connect(&sock_path) {
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
