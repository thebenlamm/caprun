//! `caprun-planner` — the out-of-process LLM sidecar (PLANNER-03/04).
//!
//! Reads `PLANNER_SOCK` / `OPENAI_API_KEY` / `CAPRUN_PLANNER_MODEL` from the
//! environment, binds an abstract-namespace UDS at `\0` + `PLANNER_SOCK`
//! using tokio's native abstract-path support (the exact pattern
//! `brokerd::server::run_broker_server` uses — see that module's doc
//! comment for the verification trail), and accepts connections in a loop.
//!
//! Each connection: read exactly ONE framed `PlannerRequest` (4-byte
//! little-endian length prefix + JSON body — byte-identical to the framing
//! `cli/caprun/src/worker.rs`'s `send_framed`/`recv_framed` and
//! `crates/brokerd/src/server.rs`'s `read_one_frame`/`send_response` use),
//! call `openai::call_openai`, and write back exactly one framed
//! [`SidecarReply`] — `Ok { response }` on success, `Error { message }` on
//! ANY failure (transport error, non-2xx status, missing/malformed tool
//! call, or `parse_planner_response` rejection). There is no retry
//! framework, no provider abstraction, no backoff — a single request per
//! connection, matching the plan's explicit "keep the loop simple"
//! instruction.
//!
//! This process is handed NOTHING beyond the `PLANNER_SOCK` listener and
//! the OpenAI endpoint it calls out to (PLANNER-04, T-21-07): no workspace
//! fd, no broker socket, no audit DB. It is never confined (unlike
//! `caprun-worker`) because it holds no filesystem or broker capability to
//! confine away from.
//!
//! # Cross-platform notes
//!
//! The abstract-namespace `UnixListener::bind` call compiles on macOS (the
//! dev machine) but only actually resolves an abstract socket path at
//! runtime on Linux — mirroring `cli/caprun/src/worker.rs`'s own note on its
//! abstract-socket `connect`. This binary only needs to COMPILE here; it
//! only ever RUNS on Linux, alongside the confined `caprun-worker` it serves.

use caprun_planner::openai;
use llm_planner::{PlannerRequest, PlannerResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The sidecar's one framed reply shape. Tagged by `status` so the
/// worker-side proxy (Plan 21-03) can distinguish success from failure
/// without guessing from field presence:
///
/// ```json
/// {"status":"ok","response":{"sink":"...","args":[...]}}
/// {"status":"error","message":"..."}
/// ```
///
/// An `Error` reply is the sidecar's fail-closed signal — the worker-side
/// proxy MUST treat it as "no usable plan node", never attempt to parse a
/// partial `response` out of it (there is none).
#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SidecarReply {
    Ok { response: PlannerResponse },
    Error { message: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let planner_sock = std::env::var("PLANNER_SOCK").map_err(|_| {
        anyhow::anyhow!("PLANNER_SOCK env var is required (unset or not valid UTF-8)")
    })?;
    let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
        anyhow::anyhow!("OPENAI_API_KEY env var is required (unset or not valid UTF-8)")
    })?;
    let model =
        std::env::var("CAPRUN_PLANNER_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    // Abstract-namespace UDS bind — tokio detects the leading NUL and calls
    // from_abstract_name internally, exactly as
    // `brokerd::server::run_broker_server` does for the broker's own socket.
    let sock_path = format!("\0{planner_sock}");
    let listener = tokio::net::UnixListener::bind(&sock_path)?;
    eprintln!(
        "[caprun-planner] listening on abstract socket {planner_sock:?} (model={model})"
    );

    loop {
        let (stream, _addr) = listener.accept().await?;
        let model = model.clone();
        let api_key = api_key.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, &model, &api_key).await {
                eprintln!("[caprun-planner] connection error: {e}");
            }
        });
    }
}

/// Service exactly one connection: read one framed `PlannerRequest`, call
/// `openai::call_openai`, write back one framed `SidecarReply`. Returns
/// `Ok(())` on a clean EOF before any request arrives (nothing to reply to)
/// as well as after a successful reply — I/O errors on read/write propagate
/// as `Err` and are logged by the caller, never silently dropped.
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    model: &str,
    api_key: &str,
) -> anyhow::Result<()> {
    let request = match read_framed_request(&mut stream).await? {
        Some(request) => request,
        None => return Ok(()),
    };

    let reply = match openai::call_openai(&request, model, api_key).await {
        Ok(response) => SidecarReply::Ok { response },
        Err(e) => SidecarReply::Error {
            message: e.to_string(),
        },
    };

    write_framed_reply(&mut stream, &reply).await
}

/// Read one framed `PlannerRequest` (4-byte LE length prefix + JSON body),
/// or `Ok(None)` on a clean EOF before any bytes arrive. Byte-identical
/// framing to `crates/brokerd/src/server.rs`'s `read_one_frame` and
/// `cli/caprun/src/worker.rs`'s `recv_framed`.
async fn read_framed_request(
    stream: &mut tokio::net::UnixStream,
) -> anyhow::Result<Option<PlannerRequest>> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    stream.read_exact(&mut body).await?;
    let request: PlannerRequest = serde_json::from_slice(&body)?;
    Ok(Some(request))
}

/// Write one framed `SidecarReply` (4-byte LE length prefix + JSON body) —
/// the same wire framing every other transport in this workspace uses.
async fn write_framed_reply(
    stream: &mut tokio::net::UnixStream,
    reply: &SidecarReply,
) -> anyhow::Result<()> {
    let body = serde_json::to_vec(reply)?;
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    Ok(())
}
