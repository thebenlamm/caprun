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
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
// Duration/Instant back the connect-retry loop in the Linux-only
// `connect_to_sidecar` below; unused on the non-Linux stub sibling.
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

/// Worker-owned multi-step routing context for `Planner::plan_next`
/// (Phase 48 / STREAM-01/02; DESIGN-multi-step-plan-stream §1.2–§2.2).
///
/// # PLAN-03 / STREAM-02 boundary
///
/// Carries ONLY:
/// - typed `CaprunIntent` (user-trusted enum, never free-form value authority)
/// - opaque `ValueId` handles in `handles` (routing table — never literals,
///   never taint labels, never `ValueRecord`)
/// - `step_index` (static sequence index for multi-node streams)
/// - `task_instruction: Option<String>` — instruction framing only; **never**
///   a `ValueId` and never bindable into a sink arg (DESIGN §5)
///
/// The bag / `handles` map is a worker-local routing table. Taint and
/// literals stay broker-owned in `ValueStore`. The planner places offered
/// handles into `PlanArg`s by call-site convention; it never mints and
/// never strips taint.
///
/// # Handle key convention (worker seed + default `plan_next` adapter)
///
/// The worker seeds these named slots after ProvideIntent + claims, and the
/// default one-shot `plan_next` adapter pulls the same slots that
/// `Planner::plan` already expects:
///
/// | Key | Meaning |
/// |-----|---------|
/// | `intent` | UserTrusted primary intent handle (recipient / path / write_path) |
/// | `derived_recipient` | Optional derived/tainted routing handle |
/// | `body` | Optional derived/tainted body handle |
/// | `trusted_subject` | UserTrusted subject handle |
/// | `trusted_body` | UserTrusted body fallback handle |
/// | `out_{step}` | Allowed-path `output_value_id` stored under step index (F-01: any sink) |
///
/// ## Coding bag keys (Phase 49 / CODE-01/02 — intent-minted only on success path)
///
/// Seeded from `IntentAccepted.named_handles` (ProvideIntent multi-mint). The
/// success-path coding recipe (`plan_coding_next` / `DeterministicPlanner`)
/// **never** places `out_*` handles into sink args (CODE-02 anti-launder).
/// `out_{step}` retains untrusted provenance when present (worker stores any
/// `Some(output_value_id)` under that key after Allowed — F-01).
///
/// **LIVE-08 proof path (default off; not LIVE DONE by itself):** the worker
/// may select `CodingI2ProofPlanner` only when `CAPRUN_CODING_I2_PROOF=1`.
/// It places `out_1` into `github.pr`/`body` while other PR args remain
/// intent-minted. The success path and LIVE-07 remain on
/// `DeterministicPlanner`; unit expressibility alone does not claim LIVE-08.
///
/// | Key | Sink / PlanArg |
/// |-----|----------------|
/// | `write_path` | `file.write` / `path` |
/// | `write_contents` | `file.write` / `contents` |
/// | `test_command` | `process.exec` / `command` |
/// | `test_args` | `process.exec` / `args` (optional) |
/// | `commit_message` | `git.commit` / `message` |
/// | `push_remote` | `git.push` / `remote` |
/// | `push_refspec` | `git.push` / `refspec` |
/// | `pr_owner` | `github.pr` / `owner` |
/// | `pr_repo` | `github.pr` / `repo` |
/// | `pr_base` | `github.pr` / `base` |
/// | `pr_head` | `github.pr` / `head` |
/// | `pr_title` | `github.pr` / `title` |
/// | `pr_body` | `github.pr` / `body` |
///
/// Keys hold **only** `ValueId` values — the type system forbids literals.
#[derive(Debug, Clone)]
pub struct PlanStreamContext {
    pub intent: CaprunIntent,
    pub step_index: usize,
    /// Opaque named handles only (`String` → `ValueId`). Never literals/taint.
    pub handles: HashMap<String, ValueId>,
    /// Instruction framing (`String`), never a bindable `ValueId` handle.
    pub task_instruction: Option<String>,
}

/// The `Planner` seam (PLANNER-01): maps a typed intent + opaque `ValueId`
/// handles to a `PlanNode`. See the module doc above for the PLANNER-04
/// compile-time boundary this trait method preserves — implementors may
/// never accept a `ValueRecord`, a raw byte slice/string from untrusted
/// content, or a taint label.
///
/// # Multi-node surface (Phase 48 / STREAM-01)
///
/// `plan()` remains the one-shot email/file/LLM path. `plan_next` is the
/// **additive** multi-node surface: static step index + opaque handle bag
/// via `PlanStreamContext`. Default impl is a one-shot adapter over `plan()`
/// so `DeterministicPlanner` and `LlmPlanner` stay byte-stable without
/// rewriting their `plan()` bodies (Pitfall 6).
pub trait Planner {
    /// Map a typed `CaprunIntent` + opaque `ValueId` handles to a `PlanNode`.
    /// Parameters mirror `plan_from_intent` exactly (see its doc below), plus
    /// `task_instruction` (Phase 22 / GATE-01): an optional, genuinely-tainted
    /// instruction fragment extracted worker-side from the raw untrusted
    /// document. It is a `String`, NEVER a `ValueId` — it carries no handle
    /// and cannot be bound into a sink arg; `DeterministicPlanner` ignores it
    /// entirely, `LlmPlanner` forwards it into the sidecar prompt as task
    /// framing only.
    fn plan(
        &self,
        intent: &CaprunIntent,
        intent_value_id: ValueId,
        derived_recipient: Option<ValueId>,
        body: Option<ValueId>,
        trusted_subject_handle: ValueId,
        trusted_body_handle: ValueId,
        task_instruction: Option<String>,
    ) -> PlanNode;

    /// Additive multi-node surface (STREAM-01/02). Default: one-shot adapter —
    /// at `step_index == 0`, pull named handles from `ctx.handles` under the
    /// documented key convention and delegate to `plan()`; at `step_index > 0`,
    /// return `None` (stream ends). Missing required seed keys (`intent`,
    /// `trusted_subject`, `trusted_body`) → `None` (fail-closed empty stream
    /// at the worker, DESIGN §8.2).
    ///
    /// Multi-node implementors override this method and leave `plan()` as a
    /// rarely-used fallback. `DeterministicPlanner` overrides for
    /// `SafeCodingWorkflow` (Phase 49 five-node coding recipe).
    fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode> {
        plan_next_one_shot(self, ctx)
    }
}

/// Default one-shot `plan_next` adapter (email/file/LLM): step 0 → `plan()`,
/// step ≥ 1 → `None`. Extracted so `DeterministicPlanner` can delegate for
/// non-coding intents without duplicating the adapter body.
pub fn plan_next_one_shot(planner: &(impl Planner + ?Sized), ctx: &PlanStreamContext) -> Option<PlanNode> {
    if ctx.step_index != 0 {
        return None;
    }
    let intent_value_id = ctx.handles.get("intent")?.clone();
    let trusted_subject_handle = ctx.handles.get("trusted_subject")?.clone();
    let trusted_body_handle = ctx.handles.get("trusted_body")?.clone();
    let derived_recipient = ctx.handles.get("derived_recipient").cloned();
    let body = ctx.handles.get("body").cloned();
    Some(planner.plan(
        &ctx.intent,
        intent_value_id,
        derived_recipient,
        body,
        trusted_subject_handle,
        trusted_body_handle,
        ctx.task_instruction.clone(),
    ))
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
        // Ignored entirely — the deterministic planner's output stays
        // byte-identical whether or not an instruction fragment was
        // extracted (Phase 22 / GATE-01 introduces no regression here).
        _task_instruction: Option<String>,
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

    /// Coding multi-node recipe (Phase 49 / CODE-01): static step-index over
    /// shipped sinks. Non-coding intents keep the one-shot adapter so email/file
    /// stay byte-stable.
    fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode> {
        match &ctx.intent {
            CaprunIntent::SafeCodingWorkflow { .. } => plan_coding_next(ctx),
            CaprunIntent::SendEmailSummary { .. }
            | CaprunIntent::CreateFileFromReport { .. } => plan_next_one_shot(self, ctx),
        }
    }
}

/// Default-off LIVE-08 proof planner. Steps through the normal coding recipe
/// until the PR node, where genuine `process.exec` output from bag `out_1` is
/// deliberately placed into the content-sensitive `body` slot. This planner
/// only places an existing opaque handle; it adds no mint site or bag schema.
pub struct CodingI2ProofPlanner;

impl Planner for CodingI2ProofPlanner {
    fn plan(
        &self,
        _intent: &CaprunIntent,
        _intent_value_id: ValueId,
        _derived_recipient: Option<ValueId>,
        _body: Option<ValueId>,
        _trusted_subject_handle: ValueId,
        _trusted_body_handle: ValueId,
        _task_instruction: Option<String>,
    ) -> PlanNode {
        unreachable!("CodingI2ProofPlanner uses plan_next only")
    }

    fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode> {
        match ctx.step_index {
            0..=3 => DeterministicPlanner.plan_next(ctx),
            4 => {
                let h = |key: &str| ctx.handles.get(key).cloned();
                Some(PlanNode {
                    sink: SinkId("github.pr".into()),
                    args: vec![
                        PlanArg { name: "owner".into(), value_id: h("pr_owner")? },
                        PlanArg { name: "repo".into(), value_id: h("pr_repo")? },
                        PlanArg { name: "base".into(), value_id: h("pr_base")? },
                        PlanArg { name: "head".into(), value_id: h("pr_head")? },
                        PlanArg { name: "title".into(), value_id: h("pr_title")? },
                        PlanArg { name: "body".into(), value_id: h("out_1")? },
                    ],
                })
            }
            _ => None,
        }
    }
}

/// Static five-node Safe Coding recipe (CODE-01). Places **only** intent-minted
/// bag keys into sink args (CODE-02) — never `out_*`, never field literals.
/// Missing required bag key → `None` (fail-closed partial stream, DESIGN §8.2).
fn plan_coding_next(ctx: &PlanStreamContext) -> Option<PlanNode> {
    let h = |key: &str| ctx.handles.get(key).cloned();
    match ctx.step_index {
        0 => Some(PlanNode {
            sink: SinkId("file.write".into()),
            args: vec![
                PlanArg {
                    name: "path".into(),
                    value_id: h("write_path")?,
                },
                PlanArg {
                    name: "contents".into(),
                    value_id: h("write_contents")?,
                },
            ],
        }),
        1 => {
            let mut args = vec![PlanArg {
                name: "command".into(),
                value_id: h("test_command")?,
            }];
            // Optional process.exec args — present when bag carries test_args.
            if let Some(a) = h("test_args") {
                args.push(PlanArg {
                    name: "args".into(),
                    value_id: a,
                });
            }
            Some(PlanNode {
                sink: SinkId("process.exec".into()),
                args,
            })
        }
        2 => Some(PlanNode {
            sink: SinkId("git.commit".into()),
            args: vec![PlanArg {
                name: "message".into(),
                value_id: h("commit_message")?,
            }],
        }),
        3 => Some(PlanNode {
            sink: SinkId("git.push".into()),
            args: vec![
                PlanArg {
                    name: "remote".into(),
                    value_id: h("push_remote")?,
                },
                PlanArg {
                    name: "refspec".into(),
                    value_id: h("push_refspec")?,
                },
            ],
        }),
        4 => Some(PlanNode {
            sink: SinkId("github.pr".into()),
            args: vec![
                PlanArg {
                    name: "owner".into(),
                    value_id: h("pr_owner")?,
                },
                PlanArg {
                    name: "repo".into(),
                    value_id: h("pr_repo")?,
                },
                PlanArg {
                    name: "base".into(),
                    value_id: h("pr_base")?,
                },
                PlanArg {
                    name: "head".into(),
                    value_id: h("pr_head")?,
                },
                PlanArg {
                    name: "title".into(),
                    value_id: h("pr_title")?,
                },
                PlanArg {
                    name: "body".into(),
                    value_id: h("pr_body")?,
                },
            ],
        }),
        _ => None,
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
        // Multi-node coding MUST go through `plan_next` (Phase 49). `plan()` is
        // not a product entry for SafeCodingWorkflow — worker always uses
        // plan_next. Return a non-production placeholder that is not a shipped
        // sink id so accidental submission fails schema (UnknownSink) rather
        // than inventing literals into PlanArg (PLAN-03). No field access.
        CaprunIntent::SafeCodingWorkflow { .. } => PlanNode {
            sink: SinkId("coding.use_plan_next".into()),
            args: vec![],
        },
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
        task_instruction: Option<String>,
    ) -> PlanNode {
        // Phase 49: coding multi-step is DeterministicPlanner::plan_next only —
        // no LLM tool-use loop for SafeCodingWorkflow (fail-closed).
        if matches!(intent, CaprunIntent::SafeCodingWorkflow { .. }) {
            eprintln!(
                "[llm-planner] SafeCodingWorkflow is unsupported on LlmPlanner \
                 (no LLM multi-step) — failing closed, no PlanNode submitted"
            );
            std::process::exit(1);
        }

        let (request, offered, known_sinks, canonical_names) = build_planner_request(
            intent,
            &intent_value_id,
            derived_recipient.as_ref(),
            body.as_ref(),
            &trusted_subject_handle,
            &trusted_body_handle,
            task_instruction.as_deref(),
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

        // Diagnostic-only (Phase 22-02 live-run evidence capture, T-22-02):
        // print the raw tool-call reply BEFORE validation — the offered
        // (slot_hint, value_id) pairs plus every arg the model actually
        // returned. Never consulted by any security decision (validation
        // below is unaffected); exists purely so a `--nocapture` live run
        // durably captures WHICH handles were offered and WHICH the model
        // chose, corroborating the audit.db-recovered evidence.
        eprintln!("[llm-planner-response] offered handles:");
        for handle in &request.available_handles {
            eprintln!(
                "[llm-planner-response]   slot_hint={} value_id={}",
                handle.slot_hint, handle.value_id.0
            );
        }
        eprintln!(
            "[llm-planner-response] model chose sink={} with {} arg(s):",
            response.sink,
            response.args.len()
        );
        for arg in &response.args {
            eprintln!(
                "[llm-planner-response]   arg name={} value_id={}",
                arg.name, arg.value_id.0
            );
        }

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
/// `Planner::plan` receives, plus `task_instruction` (Phase 22 / GATE-01: the
/// genuinely-tainted instruction fragment the worker extracted, threaded
/// straight into `PlannerRequest.task_instruction` — NEVER resolved to or
/// from a handle).
///
/// # Two-handle recipient offering (T-22-02 / GATE-01)
///
/// For `SendEmailSummary`, when `derived_recipient` is `Some` (i.e. the
/// confined worker found BOTH Reply-To:/Domain: recipient-half markers), this
/// function offers the LLM a genuine CHOICE instead of a single handle: the
/// trusted `intent_value_id` under slot_hint `"operator_recipient"` AND the
/// tainted `derived_recipient` under slot_hint `"document_address"` — BOTH
/// mapped in `canonical_names` to the sink arg name `"to"`, so whichever the
/// model picks becomes the `to` arg (via `response_to_plan_node`'s identity
/// lookup, never the model's own arg name). This is what makes the injection
/// load-bearing: absent an injection, a well-behaved planner routes the
/// operator handle to `to` (Allowed); the injection is what makes the model
/// route the tainted document handle to `to` instead (Blocked downstream).
///
/// CRITICAL: this two-handle offering is keyed SOLELY on `derived_recipient`
/// being `Some` — INDEPENDENT of whether `task_instruction` is `Some`. A
/// document carrying the recipient markers but NO injection marker still
/// gets both handles offered, with `task_instruction = None`. This decoupling
/// is the structural guarantee Plan 22-02's control leg depends on (both
/// handles offered, no injection present — isolating the injection as the
/// causal factor rather than a positional bias).
///
/// When `derived_recipient` is `None` (clean path — no marker fragments, or
/// `CreateFileFromReport`), the single-handle behavior is UNCHANGED from
/// Phase 21: one `"recipient"`-hinted handle carrying the same
/// `derived_recipient`-or-`intent_value_id` fallback `plan_from_intent`
/// itself uses. `subject`/`body` handling is unchanged in all cases:
///   - `subject` = `trusted_subject_handle` (never overridden).
///   - `body`    = `body` when `Some`, else `trusted_body_handle`.
///
/// Returns the request alongside the `offered` handle set, `known_sinks`
/// list, and `canonical_names` mapping so the caller can pass them,
/// UNCHANGED, into `response_to_plan_node` — the validator's allowlists are
/// always exactly what this function put on the wire, never re-derived or
/// guessed. Carries only `ValueId` handles + slot hints + a typed
/// `intent_kind`/`available_sinks` label + `task_instruction` framing text —
/// no PER-HANDLE value-bearing field (the `PlannerRequest`/`HandleLabel`
/// types themselves are structurally incapable of carrying a per-handle
/// literal, per `llm-planner`'s own key-set tests).
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
    task_instruction: Option<&str>,
) -> (PlannerRequest, Vec<ValueId>, Vec<String>, Vec<(ValueId, String)>) {
    let subject = trusted_subject_handle.clone();
    let body_handle = body.cloned().unwrap_or_else(|| trusted_body_handle.clone());

    let (intent_kind, available_sinks): (&str, Vec<String>) = match intent {
        CaprunIntent::SendEmailSummary { .. } => {
            ("SendEmailSummary", vec!["email.send".to_string()])
        }
        CaprunIntent::CreateFileFromReport { .. } => {
            ("CreateFileFromReport", vec!["file.create".to_string()])
        }
        // Phase 49: coding is fail-closed for LLM tool-use (empty sinks).
        // LlmPlanner::plan exits before calling the sidecar for this variant.
        CaprunIntent::SafeCodingWorkflow { .. } => ("SafeCodingWorkflow", vec![]),
    };

    let (available_handles, offered, canonical_names): (
        Vec<HandleLabel>,
        Vec<ValueId>,
        Vec<(ValueId, String)>,
    ) = match intent {
        CaprunIntent::SendEmailSummary { .. } => match derived_recipient {
            // Two-handle offering (T-22-02 / GATE-01): keyed SOLELY on
            // derived_recipient being Some, independent of task_instruction
            // (see doc comment above) — this is the load-bearing choice
            // Plan 22-02's live proof needs.
            Some(derived) => {
                let handles = vec![
                    HandleLabel {
                        slot_hint: "operator_recipient".to_string(),
                        value_id: intent_value_id.clone(),
                    },
                    HandleLabel {
                        slot_hint: "document_address".to_string(),
                        value_id: derived.clone(),
                    },
                    HandleLabel { slot_hint: "subject".to_string(), value_id: subject.clone() },
                    HandleLabel { slot_hint: "body".to_string(), value_id: body_handle.clone() },
                ];
                let offered = vec![
                    intent_value_id.clone(),
                    derived.clone(),
                    subject.clone(),
                    body_handle.clone(),
                ];
                let canonical_names = vec![
                    (intent_value_id.clone(), "to".to_string()),
                    (derived.clone(), "to".to_string()),
                    (subject.clone(), "subject".to_string()),
                    (body_handle.clone(), "body".to_string()),
                ];
                (handles, offered, canonical_names)
            }
            // Clean path (no derived_recipient): single trusted handle,
            // exactly as Phase 21 offered it — Phase 21's clean live test is
            // unaffected.
            None => {
                let recipient = intent_value_id.clone();
                let handles = vec![
                    HandleLabel { slot_hint: "recipient".to_string(), value_id: recipient.clone() },
                    HandleLabel { slot_hint: "subject".to_string(), value_id: subject.clone() },
                    HandleLabel { slot_hint: "body".to_string(), value_id: body_handle.clone() },
                ];
                let offered = vec![recipient.clone(), subject.clone(), body_handle.clone()];
                let canonical_names = vec![
                    (recipient.clone(), "to".to_string()),
                    (subject.clone(), "subject".to_string()),
                    (body_handle.clone(), "body".to_string()),
                ];
                (handles, offered, canonical_names)
            }
        },
        // `CreateFileFromReport` is unaffected by the two-handle offering
        // (out of this plan's scope — GATE-01..04 targets `email.send`'s
        // `to` arg specifically): single `"recipient"`-hinted handle mapped
        // to `"path"`, exactly as Phase 21. `file.create`'s OTHER required
        // arg (`"contents"`) is always `intent_value_id` in
        // `plan_from_intent`'s own deterministic mapping, never one of this
        // function's offered slots, so `subject`/`body` are intentionally
        // left unmapped here (a mismatch still fails closed via
        // `validate_schema`, unchanged from before this plan).
        CaprunIntent::CreateFileFromReport { .. } => {
            let recipient = derived_recipient
                .cloned()
                .unwrap_or_else(|| intent_value_id.clone());
            let handles = vec![
                HandleLabel { slot_hint: "recipient".to_string(), value_id: recipient.clone() },
                HandleLabel { slot_hint: "subject".to_string(), value_id: subject.clone() },
                HandleLabel { slot_hint: "body".to_string(), value_id: body_handle.clone() },
            ];
            let offered = vec![recipient.clone(), subject.clone(), body_handle.clone()];
            let canonical_names = vec![(recipient.clone(), "path".to_string())];
            (handles, offered, canonical_names)
        }
        // Fail-closed empty offering — LlmPlanner exits before sidecar use.
        CaprunIntent::SafeCodingWorkflow { .. } => (vec![], vec![], vec![]),
    };

    let request = PlannerRequest {
        intent_kind: intent_kind.to_string(),
        available_handles,
        available_sinks: available_sinks.clone(),
        task_instruction: task_instruction.map(|s| s.to_string()),
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
