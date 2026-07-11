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
///      body_value_id }` → `intent_value_id` (the recipient/path handle) plus the
///      trusted subject/body handles (Phase 15 finding #6 — `SendEmailSummary`
///      mints THREE distinct UserTrusted handles; `CreateFileFromReport` mints
///      only `value_id` and returns `None` for the other two).
///   6. Send `BrokerRequest::RequestFd { path }` (4-byte LE prefix + JSON).
///   7. Call `adapter_fs::recv_fd` to receive the file fd via SCM_RIGHTS out-of-band.
///      The broker sends the fd's 1-byte sendmsg payload BEFORE the JSON response,
///      so recvmsg here consumes exactly that 1 byte, leaving the JSON intact.
///   8. Read the `BrokerResponse::FdGranted` JSON response.
///   9. Read the workspace file via the received fd (NOT via open() — Landlock
///      deny-all blocks open on Linux; the passed fd is the only legal path).
///  10. Extract typed claims LOCALLY (lossy guarantee — the raw sentence is
///      discarded here; only the extracted typed value crosses the IPC boundary).
///      For `SendEmailSummary`: extract the recipient-half doc fragments
///      (`Reply-To:`/`Domain:` markers, EXTRACT-01/Phase 15) AND the tainted
///      `Body:` fragment, report them via `ReportClaims { claims }`, and — ONLY
///      when BOTH recipient-half fragments were found (finding #8's resolved
///      fork) — apply the concat transform to the worker's OWN already-
///      extracted fragment values (never a resolved broker literal) and report
///      the result via `ReportDerivedClaim` to obtain a FRESH derived handle.
///      For `CreateFileFromReport`: extract root-relative paths, unchanged.
///  11. Receive the opaque `ValueId` handles for each report.
///  12. Construct a `planner::DeterministicPlanner` and call its `Planner::plan(
///      &intent, intent_value_id, derived_recipient, body,
///      trusted_subject_handle, trusted_body_handle)` trait method (PLANNER-01
///      seam) — the planner holds ONLY opaque ValueId handles, never literals
///      or taint (PLAN-03). Send `BrokerRequest::SubmitPlanNode { plan_node }`
///      (no session_id — HARD-03). A benign (fragment-free) `SendEmailSummary`
///      STILL submits an all-UserTrusted plan node (finding #4 — CONTROL-01's
///      clean half survives; there is no early-exit here anymore).
///  13. Receive `BrokerResponse::PlanNodeDecision { decision }`. If it is
///      `BlockedPendingConfirmation`, exit 1 (non-success BEFORE any effect runs).
///  14. Otherwise exit 0.
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

use anyhow::Context;
use crate::planner::Planner;
use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind, WorkerClaim};
use brokerd::quarantine::{concat_doc_fragments, extract_doc_fragments, extract_relative_path_claims};
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::ValueId;
use runtime_core::ExecutorDecision;
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
        },
    )?;

    // ── Receive opaque UserTrusted ValueId handles for the intent ────────────
    // `subject_value_id`/`body_value_id` are additive (Phase 15 finding #6):
    // `SendEmailSummary` mints three DISTINCT UserTrusted handles; other
    // intents return `None` for both. Fall back to `intent_value_id` when
    // absent so a caller that doesn't need distinct subject/body handles
    // (e.g. `CreateFileFromReport`) never has to synthesize a placeholder.
    let (intent_value_id, subject_value_id, body_value_id) =
        match recv_framed::<BrokerResponse>(&std_stream)? {
            BrokerResponse::IntentAccepted {
                value_id,
                subject_value_id,
                body_value_id,
            } => (value_id, subject_value_id, body_value_id),
            other => anyhow::bail!("unexpected response to ProvideIntent: {other:?}"),
        };
    let trusted_subject_handle = subject_value_id.unwrap_or_else(|| intent_value_id.clone());
    let trusted_body_handle = body_value_id.unwrap_or_else(|| intent_value_id.clone());

    // ── Send BrokerRequest::RequestFd ────────────────────────────────────────
    send_framed(&std_stream, &BrokerRequest::RequestFd { path: workspace_file })?;

    // ── Receive file fd via SCM_RIGHTS (out-of-band) ─────────────────────────
    let file_fd = adapter_fs::recv_fd(sock_fd)
        .map_err(|e| anyhow::anyhow!("recv_fd: {e}"))?;

    // ── Consume BrokerResponse::FdGranted JSON ────────────────────────────────
    let _granted: BrokerResponse = recv_framed(&std_stream)?;

    // ── Read workspace file via passed fd (NOT via open()) ───────────────────
    // SAFETY: file_fd is a valid fd received from recv_fd (postcondition).
    let raw_bytes: Vec<u8> = {
        let mut file = unsafe { std::fs::File::from_raw_fd(file_fd) };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).context("read via passed fd")?;
        buf
    };
    let raw_str = String::from_utf8_lossy(&raw_bytes);

    // ── Extract typed claims + (for email) derive the recipient LOCALLY ──────
    // The raw hostile sentence is discarded here — only the extracted typed
    // value (and, for email, the worker-side-transformed derived literal)
    // crosses the IPC boundary (ASM-03 / T-05-08 / EXTRACT-01). The extractor
    // is chosen by INTENT KIND; the broker independently taints whatever the
    // worker emits (mint_from_read / mint_from_derivation) — the worker cannot
    // launder trust by choosing a variant.
    let (derived_recipient, body): (Option<ValueId>, Option<ValueId>) = match &intent {
        CaprunIntent::SendEmailSummary { .. } => {
            // Marker-anchored recipient-half fragments (Reply-To:/Domain:) —
            // extracted worker-side, never re-read, never resolved-then-
            // reused. `extract_body_fragment` (below) mirrors the same
            // marker-anchored, lossy-extraction shape for the `Body:` marker.
            let doc_fragments = extract_doc_fragments(&raw_str);
            let body_fragment = extract_body_fragment(&raw_str);

            // Report ALL raw fragments (recipient-halves + body, if present)
            // in one ReportClaims batch — the broker mints one genuinely-
            // tainted ValueRecord per claim via mint_from_read, in the same
            // order submitted.
            let mut fragment_claims: Vec<WorkerClaim> = doc_fragments
                .iter()
                .map(|c| WorkerClaim::DocFragment(c.value.clone()))
                .collect();
            if let Some(b) = &body_fragment {
                fragment_claims.push(WorkerClaim::DocFragment(b.clone()));
            }
            send_framed(&std_stream, &BrokerRequest::ReportClaims { claims: fragment_claims })?;
            let fragment_value_ids = match recv_framed::<BrokerResponse>(&std_stream)? {
                BrokerResponse::ClaimsReceived { value_ids } => value_ids,
                other => anyhow::bail!("unexpected response to ReportClaims: {other:?}"),
            };

            // RESOLVED FORK (finding #8): a derived recipient exists ONLY when
            // BOTH recipient-half fragments (Reply-To local-part + Domain
            // domain-half) were extracted — a lone fragment (or none) never
            // taints `to`; a benign doc that merely mentions an address is
            // NOT routed here (extract_doc_fragments only ever yields
            // marker-anchored halves, never a whole address).
            let derived_recipient = if doc_fragments.len() == 2 {
                // Apply the transform to the worker's OWN already-extracted
                // fragment VALUES — never resolve a broker ValueId back to a
                // literal and reuse it (DESIGN-confirm-binding.md
                // "Post-Transformation Bytes"). The transform runs BEFORE any
                // mint; the broker never re-applies it (it only byte-verifies).
                let transformed_literal =
                    concat_doc_fragments(&doc_fragments[0].value, &doc_fragments[1].value);
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
                    other => anyhow::bail!("unexpected response to ReportDerivedClaim: {other:?}"),
                }
            } else {
                None
            };

            // The body handle (if a `Body:` fragment was found) is the LAST
            // element reported — `doc_fragments.len()` fragments precede it
            // in `fragment_value_ids`, in every case (0, 1, or 2 recipient
            // halves).
            let body = if body_fragment.is_some() {
                Some(fragment_value_ids[doc_fragments.len()].clone())
            } else {
                None
            };

            (derived_recipient, body)
        }
        CaprunIntent::CreateFileFromReport { .. } => {
            let claims: Vec<WorkerClaim> = extract_relative_path_claims(&raw_str)
                .into_iter()
                .map(|c| WorkerClaim::RelativePath(c.value))
                .collect();
            send_framed(&std_stream, &BrokerRequest::ReportClaims { claims })?;
            let value_ids = match recv_framed::<BrokerResponse>(&std_stream)? {
                BrokerResponse::ClaimsReceived { value_ids } => value_ids,
                other => anyhow::bail!("unexpected response to ReportClaims: {other:?}"),
            };
            // Route the FIRST tainted path handle (if any) — mirrors the
            // pre-Phase-15 `file_value_ids.first()` behavior exactly, just
            // now threaded through the shared `derived_recipient` slot
            // (call-site convention, finding #7 — the planner never sees
            // provenance; it just places whichever handle the caller hands it).
            (value_ids.into_iter().next(), None)
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
    let planner: Box<dyn Planner> = match std::env::var("CAPRUN_PLANNER").as_deref() {
        Ok("llm") => {
            let planner_sock = std::env::var("PLANNER_SOCK")
                .context("PLANNER_SOCK required when CAPRUN_PLANNER=llm")?;
            Box::new(crate::planner::LlmPlanner::new(planner_sock))
        }
        _ => Box::new(crate::planner::DeterministicPlanner),
    };
    let plan_node = planner.plan(
        &intent,
        intent_value_id,
        derived_recipient,
        body,
        trusted_subject_handle,
        trusted_body_handle,
    );

    // ── Submit for I2 evaluation (no session_id field — HARD-03) ─────────────
    send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;

    // ── Receive the block/allow decision ─────────────────────────────────────
    let decision = match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::PlanNodeDecision { decision } => decision,
        other => anyhow::bail!("unexpected response to SubmitPlanNode: {other:?}"),
    };

    // ── Exit non-success unless the effect actually ran (durable audit event
    //    already recorded either way) ──────────────────────────────────────
    //
    // Bug found and fixed during Plan 21-04's live composed run: this
    // originally checked ONLY `BlockedPendingConfirmation`, silently falling
    // through to `Ok(())` (exit 0) for `ExecutorDecision::Denied { .. }` and
    // `NotImplemented` — a plan node the executor REJECTED before any effect
    // ran was indistinguishable, from the caller's exit code alone, from one
    // that actually succeeded. This was never exercised by the
    // `DeterministicPlanner` path (its hardcoded arg names always satisfy
    // `sink_schema::validate_schema`, so it never produces `Denied`), but the
    // `LlmPlanner` path CAN produce a schema-invalid plan node (e.g. the
    // model naming an arg something other than the sink's required name) —
    // empirically confirmed live on Linux (`scripts/mailpit-verify.sh`): a
    // real run reached `Denied` and exited 0 with NO email ever sent, before
    // this fix.
    if !matches!(decision, ExecutorDecision::Allowed) {
        eprintln!(
            "[worker] NOT ALLOWED ({decision:?}): no effect ran — exiting 1"
        );
        std::process::exit(1);
    }

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
