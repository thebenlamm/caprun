/// caprun-worker — self-confining worker binary
///
/// # Self-Confinement Order (load-bearing)
///
///   1. Connect to broker's abstract UDS (BROKER_SOCK env var, WITHOUT leading NUL).
///   2. Convert the tokio stream to a blocking std UnixStream for all subsequent I/O.
///   3. Call `sandbox::apply_confinement()` on self — AFTER connecting, so the
///      already-open broker socket fd survives Landlock deny-all.
///   4. Send `BrokerRequest::ProvideIntent { intent }` (4-byte LE prefix + JSON).
///      Deserialised from the `INTENT` env var set by caprun main. Sent AFTER
///      self-confinement (ordering invariant: connect → set_nonblocking →
///      apply_confinement → ProvideIntent → RequestFd). The broker mints a
///      UserTrusted ValueRecord for the intent literal and returns an opaque handle.
///   5. Receive `BrokerResponse::IntentAccepted { value_id, subject_value_id,
///      body_value_id, named_handles }` → `intent_value_id` (the recipient/path
///      / write_path handle) plus the trusted subject/body handles (Phase 15
///      finding #6 — `SendEmailSummary` mints THREE distinct UserTrusted
///      handles; `CreateFileFromReport` mints only `value_id` and returns
///      `None` for the other two) and, for `SafeCodingWorkflow` (Phase 49),
///      the full named bag-key → ValueId map from multi-mint.
///   6–11. Email/file only: RequestFd → recv_fd → FdGranted → read via fd →
///      extract typed claims LOCALLY → ReportClaims / ReportDerivedClaim.
///      **Coding (`SafeCodingWorkflow`) skips this path entirely** (CODE-02 /
///      DESIGN §4 trusted-intent success path): operator-typed args are already
///      UserTrusted via ProvideIntent multi-mint; no multi-file untrusted
///      claim extract, no ReportClaims demotion before irreversible sinks.
///      Connect → confine → ProvideIntent order is preserved; RequestFd is
///      simply omitted for coding (not reordered). Documented choice: skip
///      RequestFd for coding rather than keep a dummy seed-file read that
///      would risk session demotion.
///  12. Construct a `planner::DeterministicPlanner` (or, when
///      `CAPRUN_PLANNER=llm`, `planner::LlmPlanner`). Seed an opaque handle
///      bag (`HashMap<String, ValueId>`) from ProvideIntent + claim locals
///      (email/file keys: `intent`, optional `derived_recipient`/`body`,
///      `trusted_subject`, `trusted_body`; coding keys: `named_handles` bag
///      keys — PLAN-03 handles only). Then run the sequential plan-stream
///      loop (Phase 48 / STREAM-01/02):
///        a. `PlanStreamContext { intent, step_index, handles: bag, task_instruction }`
///        b. `planner.plan_next(&ctx)` → `None` breaks the loop
///        c. `BrokerRequest::SubmitPlanNode { plan_node }` (no session_id —
///           HARD-03; sequential N× only — no batch authorize)
///        d. On `Allowed`: if `output_value_id` is `Some`, insert under
///           `out_{step}` for **any** sink (F-01 — process.exec / git.commit
///           / http.request mints, not process.exec-only); step += 1; continue
///        e. On `BlockedPendingConfirmation` (CONFIRM-01 / CLI-02):
///           - `SafeCodingWorkflow`: emit `caprun-stream: BLOCKED …`, stay
///             connected, read parent `PROCEED`/`ABORT` on stdin. PROCEED
///             advances `step_index` **without** re-submitting the blocked
///             node and **without** ProvideIntent remint. ABORT → exit 2.
///           - email/file (single-node): emit BLOCKED, exit 3 (blocked-
///             incomplete — not deny). No multi-node hold wait.
///           Silent continue-past-Block is impossible: Block never returns
///           success without an explicit PROCEED token.
///        f. On `Denied` / `NotImplemented`: emit `caprun-stream: DENIED
///           code=…`, abort remaining, exit 2 (CLI-02 denied/aborted bucket;
///           `policy_deny` distinguished via `code=` field, not a separate exit).
///      Empty stream (`submitted == 0`) fails closed (DESIGN §8.2) — no
///      STREAM_DONE, infra/non-zero. ProvideIntent runs exactly once before
///      any RequestFd — the loop never re-sends it. No reconnect-remint,
///      no dual-Session stitch (DESIGN §3.3).
///  13. On natural stream end with `submitted ≥ 1`: emit
///      `caprun-stream: STREAM_DONE submitted=N` and return Ok (exit 0).
///
/// # Cross-Platform Notes
///
/// The tokio `connect` call with the `\0` prefix compiles on macOS but fails at
/// runtime (abstract sockets are Linux-only). The e2e test is `#[cfg(target_os =
/// "linux")]` so this binary is never invoked on macOS; it only needs to COMPILE.
///
/// # EXTRACT-01 confined half (Phase 15, 15-04)
///
/// Multi-fragment extraction + the concat transform run ENTIRELY inside this
/// confined worker, over the hostile bytes it already read via the passed fd —
/// never re-read, never resolved from a broker `ValueId` back to a literal.
/// The worker transforms its OWN extracted fragment strings BEFORE any mint
/// (DESIGN-confirm-binding.md "Post-Transformation Bytes", D-08), then obtains
/// a FRESH derived handle from the broker (`ReportDerivedClaim` →
/// `DerivedClaimReceived`) before ever using it as a plan-node arg. Only typed
/// fragment tokens and the transformed literal cross the IPC boundary — the
/// raw hostile sentence is discarded worker-side (lossy guarantee, T-15-15).

mod planner;
mod stream_hold;

use anyhow::Context;
use crate::planner::{PlanStreamContext, Planner};
use crate::stream_hold::{format_line, parse_hold_resume, HoldResume, StreamLine};
use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind, WorkerClaim};
use brokerd::quarantine::{concat_doc_fragments, extract_doc_fragments, extract_relative_path_claims};
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::ValueId;
use runtime_core::ExecutorDecision;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let broker_sock = std::env::var("BROKER_SOCK").context("BROKER_SOCK")?;
    let workspace_file = std::env::var("WORKSPACE_FILE").context("WORKSPACE_FILE")?;

    // Deserialise the typed intent from the INTENT env var set by caprun main.
    // Fail closed on missing or malformed values (unknown variant → serde Err).
    let intent_json = std::env::var("INTENT").context("INTENT")?;
    let intent: CaprunIntent =
        serde_json::from_str(&intent_json).context("parse INTENT (unknown intent variant?)")?;

    // M7 (WG-1): the PER-LITERAL file-derived provenance of the PRIMARY intent
    // literal (recipient/path), forwarded by caprun main via PRIMARY_SEED_FILE_DERIVED
    // ("1" iff `--seed-from-file` was present). Threaded onto ProvideIntent so the
    // broker mints a file-derived primary literal via `mint_from_read` (TAINTED),
    // never `mint_from_intent` (trusted). Absent/any-other value → false (an
    // operator-typed literal stays trusted) — fail-safe default is the trusted-arg
    // behavior, but M7's SECURITY property is the opposite direction (a file-derived
    // literal Blocks), so a MISSING var only ever under-taints an operator run, never
    // laundering a file-derived one (caprun main always sets it explicitly).
    let primary_file_derived = std::env::var("PRIMARY_SEED_FILE_DERIVED").as_deref() == Ok("1");

    // Connect to the broker's abstract-namespace UDS.
    //
    // The broker binds this socket in a sibling task after only a best-effort
    // `yield_now()` in caprun main, and this worker is a freshly-spawned PROCESS
    // that connects at startup. Under CPU oversubscription the broker's `bind()`
    // can lose the race to this `connect()`, surfacing a transient ECONNREFUSED
    // (connecting to an as-yet-unbound abstract address). Retry on transient
    // "not bound yet" errors within a bounded budget so a scheduling hiccup does
    // not fail the run; a genuinely-absent broker still fails fast once the budget
    // is exhausted. This runs BEFORE self-confinement, so connect syscalls are
    // still permitted (ordering invariant preserved).
    let sock_path = format!("\0{broker_sock}");
    let stream = {
        use std::time::{Duration, Instant};
        const CONNECT_BUDGET: Duration = Duration::from_secs(2);
        const RETRY_DELAY: Duration = Duration::from_millis(25);
        let deadline = Instant::now() + CONNECT_BUDGET;
        loop {
            match tokio::net::UnixStream::connect(&sock_path).await {
                Ok(s) => break s,
                // Transient: broker task has not reached bind() yet. Retry until the
                // budget runs out, then fall through to the hard error below.
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                    ) && Instant::now() < deadline =>
                {
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                // Non-transient, or budget exhausted: fail fast (do not mask a
                // genuinely-absent broker behind an unbounded retry loop).
                Err(e) => return Err(e).context("connect to broker abstract UDS"),
            }
        }
    };

    // Convert to a blocking std UnixStream for all subsequent I/O.
    let std_stream = stream.into_std().context("into_std")?;
    std_stream
        .set_nonblocking(false)
        .context("set_nonblocking")?;

    let sock_fd = std_stream.as_raw_fd();

    // ── Self-confine AFTER connecting (self-confinement model) ───────────────
    sandbox::apply_confinement().map_err(|e| anyhow::anyhow!("apply_confinement: {e}"))?;

    // ── Send BrokerRequest::ProvideIntent (AFTER confinement) ────────────────
    // Ordering invariant: connect → set_nonblocking → apply_confinement →
    // ProvideIntent → RequestFd (Pitfall 6). Sending AFTER confinement means
    // the broker is the sole trust boundary for minting the intent value;
    // the worker cannot forge a ValueRecord, only supply the typed intent literal
    // it received from the trusted orchestrator env var.
    send_framed(
        &std_stream,
        &BrokerRequest::ProvideIntent {
            intent: intent.clone(),
            primary_file_derived,
        },
    )?;

    // ── Receive opaque UserTrusted ValueId handles for the intent ────────────
    // `subject_value_id`/`body_value_id` are additive (Phase 15 finding #6):
    // `SendEmailSummary` mints three DISTINCT UserTrusted handles; other
    // intents return `None` for both. Fall back to `intent_value_id` when
    // absent so a caller that doesn't need distinct subject/body handles
    // (e.g. `CreateFileFromReport`) never has to synthesize a placeholder.
    // `named_handles` is additive (Phase 49 / CODE-02): coding fills bag keys;
    // email/file return empty.
    let (intent_value_id, subject_value_id, body_value_id, named_handles) =
        match recv_framed::<BrokerResponse>(&std_stream)? {
            BrokerResponse::IntentAccepted {
                value_id,
                subject_value_id,
                body_value_id,
                named_handles,
            } => (value_id, subject_value_id, body_value_id, named_handles),
            other => anyhow::bail!("unexpected response to ProvideIntent: {other:?}"),
        };
    let trusted_subject_handle = subject_value_id.unwrap_or_else(|| intent_value_id.clone());
    let trusted_body_handle = body_value_id.unwrap_or_else(|| intent_value_id.clone());

    // ── Seed opaque handle bag + optional claim path (by intent kind) ────────
    // Worker-local routing table of opaque ValueIds only — never literals,
    // never taint, never ValueRecord. ProvideIntent already ran exactly once
    // above; the stream loop never re-sends it (DESIGN §2.3).
    //
    // Coding (SafeCodingWorkflow): seed from named_handles only — skip
    // RequestFd / ReportClaims / ReportDerivedClaim demotion path (CODE-02;
    // DESIGN §4 trusted-intent success path). Email/file: unchanged
    // RequestFd + claims path.
    let (mut bag, task_instruction): (HashMap<String, ValueId>, Option<String>) = match &intent {
        CaprunIntent::SafeCodingWorkflow { .. } => {
            // CODE-02 residual hygiene (49-02): coding success path never
            // RequestFd + claim-extract. Multi-file untrusted demotion is not
            // required before irreversible sinks — bag is seeded solely from
            // ProvideIntent named_handles (UserTrusted). Connect → confine →
            // ProvideIntent order is unchanged; RequestFd is simply omitted
            // for this intent kind (not reordered after ProvideIntent).
            let mut bag: HashMap<String, ValueId> = HashMap::new();
            bag.insert("intent".into(), intent_value_id.clone());
            // Primary value_id is write_path; also seed under write_path so a
            // partial named_handles wire still has the primary slot.
            bag.insert("write_path".into(), intent_value_id);
            for (name, vid) in named_handles {
                bag.insert(name, vid);
            }
            (bag, None)
        }
        CaprunIntent::SendEmailSummary { .. } | CaprunIntent::CreateFileFromReport { .. } => {
            // ── Send BrokerRequest::RequestFd ────────────────────────────────
            send_framed(
                &std_stream,
                &BrokerRequest::RequestFd {
                    path: workspace_file,
                },
            )?;

            // ── Receive file fd via SCM_RIGHTS (out-of-band) ─────────────────
            let file_fd = adapter_fs::recv_fd(sock_fd)
                .map_err(|e| anyhow::anyhow!("recv_fd: {e}"))?;

            // ── Consume BrokerResponse::FdGranted JSON ───────────────────────
            let _granted: BrokerResponse = recv_framed(&std_stream)?;

            // ── Read workspace file via passed fd (NOT via open()) ───────────
            // SAFETY: file_fd is a valid fd received from recv_fd (postcondition).
            let raw_bytes: Vec<u8> = {
                let mut file = unsafe { std::fs::File::from_raw_fd(file_fd) };
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).context("read via passed fd")?;
                buf
            };
            let raw_str = String::from_utf8_lossy(&raw_bytes);

            // ── Extract typed claims + (for email) derive the recipient ──────
            // The raw hostile sentence is discarded here — only the extracted
            // typed value (and, for email, the worker-side-transformed derived
            // literal) crosses the IPC boundary (ASM-03 / T-05-08 / EXTRACT-01).
            let (derived_recipient, body, task_instruction): (
                Option<ValueId>,
                Option<ValueId>,
                Option<String>,
            ) = match &intent {
                CaprunIntent::SendEmailSummary { .. } => {
                    let doc_fragments = extract_doc_fragments(&raw_str);
                    let body_fragment = extract_body_fragment(&raw_str);
                    let instruction_fragment = extract_instruction_fragment(&raw_str);

                    let mut fragment_claims: Vec<WorkerClaim> = doc_fragments
                        .iter()
                        .map(|c| WorkerClaim::DocFragment(c.value.clone()))
                        .collect();
                    if let Some(b) = &body_fragment {
                        fragment_claims.push(WorkerClaim::DocFragment(b.clone()));
                    }
                    if let Some(instr) = &instruction_fragment {
                        fragment_claims.push(WorkerClaim::DocFragment(instr.clone()));
                    }
                    send_framed(
                        &std_stream,
                        &BrokerRequest::ReportClaims {
                            claims: fragment_claims,
                        },
                    )?;
                    let fragment_value_ids = match recv_framed::<BrokerResponse>(&std_stream)? {
                        BrokerResponse::ClaimsReceived { value_ids } => value_ids,
                        other => {
                            anyhow::bail!("unexpected response to ReportClaims: {other:?}")
                        }
                    };

                    let derived_recipient = if doc_fragments.len() == 2 {
                        let transformed_literal = concat_doc_fragments(
                            &doc_fragments[0].value,
                            &doc_fragments[1].value,
                        );
                        send_framed(
                            &std_stream,
                            &BrokerRequest::ReportDerivedClaim {
                                transformed_literal,
                                transform: TransformKind::Concat,
                                input_value_ids: vec![
                                    fragment_value_ids[0].clone(),
                                    fragment_value_ids[1].clone(),
                                ],
                            },
                        )?;
                        match recv_framed::<BrokerResponse>(&std_stream)? {
                            BrokerResponse::DerivedClaimReceived { value_id } => Some(value_id),
                            other => anyhow::bail!(
                                "unexpected response to ReportDerivedClaim: {other:?}"
                            ),
                        }
                    } else {
                        None
                    };

                    let body = if body_fragment.is_some() {
                        Some(fragment_value_ids[doc_fragments.len()].clone())
                    } else {
                        None
                    };

                    (derived_recipient, body, instruction_fragment)
                }
                CaprunIntent::CreateFileFromReport { .. } => {
                    let claims: Vec<WorkerClaim> = extract_relative_path_claims(&raw_str)
                        .into_iter()
                        .map(|c| WorkerClaim::RelativePath(c.value))
                        .collect();
                    send_framed(&std_stream, &BrokerRequest::ReportClaims { claims })?;
                    let value_ids = match recv_framed::<BrokerResponse>(&std_stream)? {
                        BrokerResponse::ClaimsReceived { value_ids } => value_ids,
                        other => {
                            anyhow::bail!("unexpected response to ReportClaims: {other:?}")
                        }
                    };
                    (value_ids.into_iter().next(), None, None)
                }
                CaprunIntent::SafeCodingWorkflow { .. } => unreachable!(
                    "SafeCodingWorkflow handled in outer match — no claim path"
                ),
            };

            let mut bag: HashMap<String, ValueId> = HashMap::new();
            bag.insert("intent".into(), intent_value_id);
            if let Some(derived) = derived_recipient {
                bag.insert("derived_recipient".into(), derived);
            }
            if let Some(body_handle) = body {
                bag.insert("body".into(), body_handle);
            }
            bag.insert("trusted_subject".into(), trusted_subject_handle);
            bag.insert("trusted_body".into(), trusted_body_handle);
            (bag, task_instruction)
        }
    };

    // ── Planner selection (Phase 21 / PLANNER-03): CAPRUN_PLANNER selects ────
    // the concrete `Planner` behind the seam (PLANNER-01). Both implementors
    // receive only opaque ValueId handles — never the literal, never taint,
    // never a ValueRecord (PLAN-03, type-enforced by the trait method's own
    // signature) — so this selection cannot widen what either planner sees.
    // There is NO early-exit here anymore (finding #4): a benign
    // (fragment-free) SendEmailSummary still submits an all-UserTrusted node
    // → Allowed, preserving CONTROL-01's live clean-send-allowed path.
    //
    // Default (CAPRUN_PLANNER unset or any value other than "llm") stays
    // `DeterministicPlanner` — byte-for-byte the prior behavior, no
    // regression to any existing test. When "llm", constructs `LlmPlanner`
    // reading `PLANNER_SOCK` from env (set by caprun main ONLY when
    // CAPRUN_PLANNER=llm, see main.rs).
    //
    // ORDERING NOTE: `LlmPlanner::plan()`'s sidecar connect happens HERE,
    // i.e. AFTER `sandbox::apply_confinement()` above — this is legal because
    // the worker's seccomp filter permits AF_UNIX socket()/connect() (only
    // AF_INET/AF_INET6 and execve are denied, see
    // crates/sandbox/src/seccomp.rs); it is the SAME self-confinement-then-
    // connect pattern this worker already uses for its own broker connection,
    // just via a blocking std UnixStream instead of tokio (LlmPlanner::plan()
    // is a synchronous trait method).
    let planner: Box<dyn Planner> = match (
        std::env::var("CAPRUN_PLANNER").as_deref() == Ok("llm"),
        std::env::var("CAPRUN_CODING_I2_PROOF").as_deref() == Ok("1"),
        matches!(&intent, CaprunIntent::SafeCodingWorkflow { .. }),
    ) {
        (true, _, true) => anyhow::bail!("LLM planner does not support SafeCodingWorkflow"),
        (true, _, false) => {
            let planner_sock = std::env::var("PLANNER_SOCK")
                .context("PLANNER_SOCK required when CAPRUN_PLANNER=llm")?;
            Box::new(crate::planner::LlmPlanner::new(planner_sock))
        }
        (false, true, true) => {
            // Allowed nodes already store output_value_id as out_{step}; this
            // proof planner only places out_1 and does not change bag minting.
            Box::new(crate::planner::CodingI2ProofPlanner)
        }
        _ => Box::new(crate::planner::DeterministicPlanner),
    };

    // ── Sequential plan-stream loop (STREAM-01; N× SubmitPlanNode) ───────────
    // Each iteration: plan_next → SubmitPlanNode → PlanNodeDecision → branch.
    // No batch authorize. No mid-stream ProvideIntent. No re-submit on Block.
    let mut step_index: usize = 0;
    let mut submitted: usize = 0;
    loop {
        let ctx = PlanStreamContext {
            intent: intent.clone(),
            step_index,
            handles: bag.clone(),
            task_instruction: task_instruction.clone(),
        };
        let Some(plan_node) = planner.plan_next(&ctx) else {
            break;
        };

        // Capture sink id before move into SubmitPlanNode (machine lines need it).
        let sink_id = plan_node.sink.0.clone();

        // Submit for I2 evaluation (no session_id field — HARD-03).
        send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;

        // Receive the block/allow decision.
        //
        // `output_value_id` (32-05 + F-01): Some(handle) on an Allowed
        // decision that mints intermediate output — process.exec, git.commit,
        // and http.request (not process.exec-only; stale comments fixed).
        // The opaque handle is never the raw captured bytes (I1). Stored in
        // the bag under `out_{step}` for ANY sink when Some (DESIGN §2.2 F-01).
        let (decision, output_value_id) = match recv_framed::<BrokerResponse>(&std_stream)? {
            BrokerResponse::PlanNodeDecision {
                decision,
                output_value_id,
            } => (decision, output_value_id),
            other => anyhow::bail!("unexpected response to SubmitPlanNode: {other:?}"),
        };
        submitted += 1;

        // Decision branch table (DESIGN §6; Phase 50 CONFIRM-01 hold + CLI-02).
        // Bug found and fixed during Plan 21-04: originally only
        // BlockedPendingConfirmation exited non-zero, silently treating
        // Denied/NotImplemented as success; LlmPlanner can produce Denied.
        match decision {
            ExecutorDecision::Allowed => {
                // F-01: store any Some(output_value_id) regardless of sink id.
                if let Some(id) = output_value_id {
                    bag.insert(format!("out_{step_index}"), id);
                }
                // Optional progress line for parent orchestration (CLI-02).
                println!(
                    "{}",
                    format_line(&StreamLine::NodeAllowed {
                        step: step_index,
                        sink: sink_id,
                    })
                );
                step_index += 1;
                continue;
            }
            ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                // Fail-closed without anchors (always-confirm push always
                // supplies them; empty is an invariant violation).
                let first = anchors.first().ok_or_else(|| {
                    anyhow::anyhow!(
                        "BlockedPendingConfirmation without anchors — fail closed"
                    )
                })?;
                let effect_id = first.anchor.effect_id.to_string();
                let block_sink = first.anchor.sink.0.clone();

                // Machine-readable hold signal (parent protocol).
                println!(
                    "{}",
                    format_line(&StreamLine::Blocked {
                        effect_id: effect_id.clone(),
                        sink: block_sink.clone(),
                    })
                );

                // Coding multi-node: stay-connected Block-and-Hold (CONFIRM-01).
                // Email/file single-node: stop-on-Block mapped to exit 3.
                if matches!(intent, CaprunIntent::SafeCodingWorkflow { .. }) {
                    eprintln!(
                        "[worker] BLOCKED pending confirmation effect_id={effect_id} \
                         sink={block_sink}: holding stream (no re-submit, no remint); \
                         waiting for parent PROCEED/ABORT"
                    );
                    let mut line = String::new();
                    std::io::stdin()
                        .read_line(&mut line)
                        .context("read hold resume token from parent stdin")?;
                    match parse_hold_resume(&line) {
                        Ok(HoldResume::Proceed) => {
                            // Advance past the blocked step WITHOUT re-issuing
                            // SubmitPlanNode for it (confirm already ran the
                            // sink from the durable snapshot). MUST NOT
                            // ProvideIntent again.
                            step_index += 1;
                            continue;
                        }
                        Ok(HoldResume::Abort) => {
                            eprintln!(
                                "[worker] hold ABORT for effect_id={effect_id} \
                                 sink={block_sink} — exiting 2 (denied/aborted)"
                            );
                            std::process::exit(2);
                        }
                        Err(e) => {
                            // Unknown token: fail-closed infra — never Proceed.
                            eprintln!(
                                "[worker] unknown hold resume token — fail closed: {e}"
                            );
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!(
                        "[worker] BLOCKED pending confirmation effect_id={effect_id} \
                         sink={block_sink}: single-node stop (no multi-node hold) \
                         — exiting 3 (blocked/incomplete)"
                    );
                    std::process::exit(3);
                }
            }
            ExecutorDecision::Denied { reason } => {
                let code = reason.code().to_string();
                println!(
                    "{}",
                    format_line(&StreamLine::Denied {
                        code: code.clone(),
                        sink: sink_id,
                    })
                );
                eprintln!(
                    "[worker] DENIED code={code} ({reason}): aborting remaining \
                     plan nodes — exiting 2"
                );
                std::process::exit(2);
            }
            ExecutorDecision::NotImplemented => {
                let code = "not_implemented";
                println!(
                    "{}",
                    format_line(&StreamLine::Denied {
                        code: code.into(),
                        sink: sink_id,
                    })
                );
                eprintln!(
                    "[worker] NOT IMPLEMENTED: aborting remaining plan nodes \
                     — exiting 2"
                );
                std::process::exit(2);
            }
        }
    }

    // Empty multi-node stream fails closed (DESIGN §8.2) — not exit 0, no
    // STREAM_DONE (CLI-02 infra bucket via anyhow → non-zero).
    if submitted == 0 {
        anyhow::bail!(
            "empty plan stream: plan_next returned no nodes before any \
             SubmitPlanNode — fail closed (DESIGN §8.2)"
        );
    }

    // Full Allowed stream (or Allowed after hold-release with remaining
    // Allowed): machine-readable success terminal.
    println!(
        "{}",
        format_line(&StreamLine::StreamDone { submitted })
    );
    Ok(())
}

/// Extract the `Body:` marker-anchored line's content from raw untrusted bytes.
///
/// Hand-rolled, dependency-free (mirrors `extract_doc_fragments`'s marker-
/// anchored, lossy-extraction shape) — runs CONFINED worker-side, over the
/// bytes already read via the passed fd; never broker-side (EXTRACT-01).
/// Returns everything after the `Body:` marker up to end-of-line, trimmed;
/// `None` if the marker is absent or the remainder is empty. Only this
/// extracted token (never the surrounding sentence) is reported to the broker.
fn extract_body_fragment(raw: &str) -> Option<String> {
    let marker = "Body:";
    let idx = raw.find(marker)?;
    let after = &raw[idx + marker.len()..];
    let line_end = after.find('\n').unwrap_or(after.len());
    let value = after[..line_end].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Extract the `Instruction:` marker-anchored line's content from raw
/// untrusted bytes — the genuinely-tainted (mint_from_read-rooted) injection
/// instruction GATE-01 threads to the LLM planner as task framing (Phase 22 /
/// T-22-03).
///
/// Hand-rolled, dependency-free, mirrors `extract_body_fragment`'s
/// marker-anchored, lossy-extraction shape exactly — runs CONFINED
/// worker-side, over the bytes already read via the passed fd; never
/// broker-side (EXTRACT-01). Returns everything after the `Instruction:`
/// marker up to end-of-line, trimmed; `None` if the marker is absent or the
/// remainder is empty. Only this extracted token (never the surrounding
/// prose) is reported to the broker and kept worker-side as task framing.
///
/// Uses a marker DISTINCT from `Reply-To:`/`Domain:`/`Body:` so the
/// two-handle recipient offering (`build_planner_request`, keyed solely on
/// `derived_recipient` being `Some`) can be exercised WITHOUT an injection
/// present — a document carrying the recipient markers but no `Instruction:`
/// marker still offers both handles, with `task_instruction = None`. This is
/// the structural guarantee Plan 22-02's control leg depends on.
fn extract_instruction_fragment(raw: &str) -> Option<String> {
    let marker = "Instruction:";
    let idx = raw.find(marker)?;
    let after = &raw[idx + marker.len()..];
    let line_end = after.find('\n').unwrap_or(after.len());
    let value = after[..line_end].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Write a framed message (4-byte LE length prefix + JSON body) to `stream`.
fn send_framed(stream: &std::os::unix::net::UnixStream, msg: &impl serde::Serialize) -> anyhow::Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    (&*stream).write_all(&len)?;
    (&*stream).write_all(&body)?;
    Ok(())
}

/// Read a framed message (4-byte LE length prefix + JSON body) from `stream`.
fn recv_framed<T: serde::de::DeserializeOwned>(
    stream: &std::os::unix::net::UnixStream,
) -> anyhow::Result<T> {
    let mut len_buf = [0u8; 4];
    (&*stream).read_exact(&mut len_buf)?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    (&*stream).read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
