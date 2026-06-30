/// server — tokio async UDS IPC server
///
/// ─────────────────────────────────────────────────────────────────────────────
/// VERIFIED abstract-namespace UDS pattern — confirmed by reading tokio-1.52.3
/// source (src/net/unix/listener.rs + stream.rs) 2026-06-29, and by
/// crates/brokerd/tests/uds_abstract_spike.rs (bind → accept → round-trip)
/// ─────────────────────────────────────────────────────────────────────────────
///
/// APPROACH A — PRIMARY (verified simpler in tokio 1.52.3):
///   tokio 1.52.3 handles abstract paths natively in UnixListener::bind and
///   UnixStream::connect. Simply pass the path with a leading NUL byte:
///
///   let listener = tokio::net::UnixListener::bind("\0/agentos/<session_id>")?;
///   // On Linux, tokio strips the \0 and calls:
///   // StdSocketAddr::from_abstract_name(&os_str_bytes[1..])
///
/// APPROACH B — ALTERNATIVE (if Approach A fails — more explicit):
///   use std::os::linux::net::SocketAddrExt;
///   let addr = std::os::unix::net::SocketAddr::from_abstract_name(b"/agentos/<session_id>")?;
///   let std_listener = std::os::unix::net::UnixListener::bind_addr(&addr)?;
///   std_listener.set_nonblocking(true)?;
///   let listener = tokio::net::UnixListener::from_std(std_listener)?;
///
///   NOTE: std::os::unix::net::UnixListener::bind(path_with_null) does NOT work —
///   CString rejects embedded NUL bytes. Always use bind_addr for std approach.
///
/// KEY PROPERTY: Abstract sockets bypass Landlock filesystem restrictions.
/// After Landlock deny-all-filesystem is applied in pre_exec, the worker can
/// still connect to the broker's abstract socket. This is why abstract UDS is
/// the correct choice (over path-based /tmp/agentos.sock which Landlock would block).
///
/// FALLBACK (per RESEARCH.md Q1): If abstract bind returns EINVAL on an older kernel,
/// use a temp-dir path-based socket and add a Landlock read/connect exception
/// for that path in sandbox::landlock::deny_all_filesystem.
///
/// IPC framing: 4-byte LE length prefix + JSON body (serde_json).
/// Max message size: 64 KiB (ASVS V5 input validation / T-03-08 DoS mitigate).
///
/// Wave 2 Plan 03: full accept loop implementation.
/// Plan 05 (caprun) wires RequestFd and ReportRead end-to-end.

use crate::audit::append_event;
use crate::proto::{BrokerRequest, BrokerResponse};
use crate::session::{create_session, persist_session};
use chrono::Utc;
use runtime_core::Event;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Maximum IPC message size (64 KiB).
///
/// Any message claiming a length beyond this limit is rejected before allocation
/// (T-03-08 DoS mitigate: guard before vec allocation, not after).
const MAX_MSG_SIZE: usize = 64 * 1024;

/// Start the broker IPC server on an abstract-namespace UDS socket.
///
/// Binds `\0/agentos/{session_id}` using tokio's native abstract-path support
/// (Approach A — verified in uds_abstract_spike.rs). Accepts connections in a
/// loop and spawns a task per connection.
///
/// # Arguments
/// * `session_id` — unique string used as the socket name suffix.
/// * `conn` — shared, mutex-protected SQLite connection for session + audit writes.
///
/// Returns when the accept loop encounters an unrecoverable error.
pub async fn run_broker_server(
    session_id: &str,
    conn: Arc<Mutex<rusqlite::Connection>>,
) -> anyhow::Result<()> {
    // Approach A: tokio detects the leading NUL and calls from_abstract_name internally.
    // Verified against tokio-1.52.3 source and confirmed by uds_abstract_spike.rs.
    let sock_path = format!("\0/agentos/{session_id}");
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    loop {
        let (stream, _addr) = listener.accept().await?;
        let conn_clone = conn.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, conn_clone).await {
                eprintln!("[brokerd] connection error: {e}");
            }
        });
    }
}

/// Handle one IPC connection: read a framed request, dispatch it, send the response.
///
/// Framing: 4-byte LE length prefix + JSON body.
/// Guard: length > 64 KiB → reject with Error response, never allocate.
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
) -> anyhow::Result<()> {
    // Read 4-byte LE length prefix
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;

    // T-03-08: guard before allocation — reject oversized messages
    if msg_len > MAX_MSG_SIZE {
        let resp = BrokerResponse::Error {
            message: "message too large".into(),
        };
        send_response(&mut stream, &resp).await?;
        return Ok(());
    }

    let mut body = vec![0u8; msg_len];
    stream.read_exact(&mut body).await?;

    // Deserialize — serde errors → Error response, never panic (T-03-08)
    let request = match serde_json::from_slice::<BrokerRequest>(&body) {
        Ok(req) => req,
        Err(e) => {
            // T-03-09: log detail internally, send generic message to caller
            eprintln!("[brokerd] deserialize error: {e}");
            let resp = BrokerResponse::Error {
                message: "invalid request".into(),
            };
            send_response(&mut stream, &resp).await?;
            return Ok(());
        }
    };

    let response = dispatch(request, &conn);
    send_response(&mut stream, &response).await?;
    Ok(())
}

/// Dispatch a BrokerRequest and return the appropriate BrokerResponse.
///
/// DB writes are wrapped in a synchronous lock block (no await while holding
/// the mutex — safe to use std::sync::Mutex in async context).
fn dispatch(request: BrokerRequest, conn: &Arc<Mutex<rusqlite::Connection>>) -> BrokerResponse {
    match request {
        BrokerRequest::CreateSession { intent_id } => {
            let session = create_session(intent_id);
            let session_id = session.id;

            // Build the session_created audit event
            let event = Event {
                id: Uuid::new_v4(),
                parent_id: None,
                session_id,
                actor: "broker".into(),
                event_type: "session_created".into(),
                timestamp: Utc::now(),
                taint: vec![],
            };

            // Lock, persist, append — lock released before response is sent
            let result: anyhow::Result<()> = match conn.lock() {
                Ok(locked) => persist_session(&locked, &session)
                    .and_then(|_| append_event(&locked, &event, None).map(|_| ())),
                Err(e) => Err(anyhow::anyhow!("mutex poisoned: {e}")),
            };

            match result {
                Ok(_) => BrokerResponse::SessionCreated { session_id },
                Err(e) => {
                    // T-03-09: internal detail logged, generic message to worker
                    eprintln!("[brokerd] CreateSession error: {e}");
                    BrokerResponse::Error {
                        message: "internal error".into(),
                    }
                }
            }
        }

        // RequestFd and ReportRead are not wired until Plan 05 (caprun end-to-end).
        BrokerRequest::RequestFd { .. } | BrokerRequest::ReportRead { .. } => {
            BrokerResponse::Error {
                message: "not wired until Plan 05".into(),
            }
        }

        // SubmitPlanNode dispatch is wired in Plan 04 (executor integration).
        // The variant is declared here (closing RESEARCH Gap 3) but the taint
        // evaluation + ConfirmationPrompt path is connected in the next plan.
        BrokerRequest::SubmitPlanNode { .. } => BrokerResponse::Error {
            message: "SubmitPlanNode not wired until Plan 04".into(),
        },
    }
}

/// Write a framed BrokerResponse to the stream (4-byte LE length + JSON body).
async fn send_response(
    stream: &mut tokio::net::UnixStream,
    response: &BrokerResponse,
) -> anyhow::Result<()> {
    let body = serde_json::to_vec(response)?;
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    Ok(())
}
