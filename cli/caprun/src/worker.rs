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
///   5. Receive `BrokerResponse::IntentAccepted { value_id }` → `intent_value_id`.
///   6. Send `BrokerRequest::RequestFd { path }` (4-byte LE prefix + JSON).
///   7. Call `adapter_fs::recv_fd` to receive the file fd via SCM_RIGHTS out-of-band.
///      The broker sends the fd's 1-byte sendmsg payload BEFORE the JSON response,
///      so recvmsg here consumes exactly that 1 byte, leaving the JSON intact.
///   8. Read the `BrokerResponse::FdGranted` JSON response.
///   9. Read the workspace file via the received fd (NOT via open() — Landlock
///      deny-all blocks open on Linux; the passed fd is the only legal path).
///  10. Extract typed email claims LOCALLY (lossy guarantee — the raw sentence is
///      discarded here; only the address crosses the IPC boundary). Send
///      `BrokerRequest::ReportClaims { claims }`.
///  11. Receive `BrokerResponse::ClaimsReceived { value_ids }` (opaque handles).
///      If no claims were extracted (benign content), exit 0 WITHOUT submitting a
///      plan node.
///  12. Call `planner::plan_from_intent(&intent, intent_value_id, &value_ids)` — the
///      planner holds ONLY opaque ValueId handles, never literals or taint (PLAN-03).
///      Send `BrokerRequest::SubmitPlanNode { plan_node }` (no session_id — HARD-03).
///  13. Receive `BrokerResponse::PlanNodeDecision { decision }`. If it is
///      `BlockedPendingConfirmation`, exit 1 (non-success BEFORE any effect runs).
///  14. Otherwise exit 0.
///
/// # Cross-Platform Notes
///
/// The tokio `connect` call with the `\0` prefix compiles on macOS but fails at
/// runtime (abstract sockets are Linux-only). The e2e test is `#[cfg(target_os =
/// "linux")]` so this binary is never invoked on macOS; it only needs to COMPILE.

mod planner;

use anyhow::Context;
use brokerd::proto::{BrokerRequest, BrokerResponse, WorkerClaim};
use brokerd::quarantine::extract_email_claims;
use runtime_core::intent::CaprunIntent;
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
    let sock_path = format!("\0{broker_sock}");
    let stream = tokio::net::UnixStream::connect(&sock_path)
        .await
        .context("connect to broker abstract UDS")?;

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

    // ── Receive opaque UserTrusted ValueId handle for the intent ─────────────
    let intent_value_id = match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::IntentAccepted { value_id } => value_id,
        other => anyhow::bail!("unexpected response to ProvideIntent: {other:?}"),
    };

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

    // ── Extract typed claims LOCALLY (lossy guarantee) ───────────────────────
    // The raw hostile sentence is discarded here — only the extracted address
    // crosses the IPC boundary (ASM-03 / T-05-08). Reuses the proven extractor.
    let claims: Vec<WorkerClaim> = extract_email_claims(&raw_str)
        .into_iter()
        .map(|c| WorkerClaim::EmailAddress(c.value))
        .collect();

    // ── Send BrokerRequest::ReportClaims (typed; no raw bytes) ───────────────
    send_framed(&std_stream, &BrokerRequest::ReportClaims { claims })?;

    // ── Receive opaque ValueId handles ───────────────────────────────────────
    let value_ids = match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::ClaimsReceived { value_ids } => value_ids,
        other => anyhow::bail!("unexpected response to ReportClaims: {other:?}"),
    };

    // ── Benign content: no claims → exit success without submitting a plan ────
    if value_ids.is_empty() {
        eprintln!("[worker] no claims extracted — benign content, exiting 0");
        return Ok(());
    }

    // ── Deterministic planner: map intent + handles → PlanNode (PLAN-02) ─────
    // `plan_from_intent` receives only opaque ValueId handles — never the literal,
    // never taint, never a ValueRecord (PLAN-03, type-enforced by the signature).
    // The planner uses `intent_value_id` (UserTrusted) for the `to` arg on the
    // clean allow-path. `value_ids` (tainted file handles) are passed as
    // `_file_value_ids` and ignored by this variant — available for future paths.
    let plan_node = crate::planner::plan_from_intent(&intent, intent_value_id, &value_ids);

    // ── Submit for I2 evaluation (no session_id field — HARD-03) ─────────────
    send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;

    // ── Receive the block/allow decision ─────────────────────────────────────
    let decision = match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::PlanNodeDecision { decision } => decision,
        other => anyhow::bail!("unexpected response to SubmitPlanNode: {other:?}"),
    };

    // ── Exit non-success if blocked (durable audit event already recorded) ───
    if matches!(decision, ExecutorDecision::BlockedPendingConfirmation { .. }) {
        eprintln!("[worker] BLOCKED: value-injection defense triggered — exiting 1");
        std::process::exit(1);
    }

    Ok(())
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
