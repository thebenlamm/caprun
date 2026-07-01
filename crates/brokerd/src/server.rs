/// server — tokio async UDS IPC server (the unified, session-scoped reference monitor)
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
///
/// KEY PROPERTY: Abstract sockets bypass Landlock filesystem restrictions.
/// After Landlock deny-all-filesystem is applied in the worker, it can still
/// connect to the broker's abstract socket.
///
/// IPC framing: 4-byte LE length prefix + JSON body (serde_json).
/// Max message size: 64 KiB (ASVS V5 input validation / T-03-08 DoS mitigate).
///
/// ─────────────────────────────────────────────────────────────────────────────
/// UNIFIED DISPATCH (Phase 5, Plan 02)
/// ─────────────────────────────────────────────────────────────────────────────
/// This is THE single broker dispatch path — there is no second loop in the CLI.
/// It is the live, session-scoped, fail-closed reference monitor:
///   * RequestFd    — broker opens the workspace file (ambient fs) and passes the
///                    fd via SCM_RIGHTS; appends a causal `fd_granted` event.
///   * ReportClaims — mints a genuinely-tainted ValueRecord per typed claim via
///                    `quarantine::mint_from_read` (the SOLE taint-mint site), so
///                    `provenance_chain[0]` anchors to the real file_read event.
///   * SubmitPlanNode — evaluates I2 via `executor::submit_plan_node` against the
///                    CONNECTION-established session_id (never a message-supplied
///                    one), durably appends `sink_blocked`/`plan_node_evaluated`
///                    BEFORE returning the decision (fail-closed, ACC-02).
///
/// Per-connection isolation (HARD-03): each accepted connection owns a fresh
/// `ValueStore::default()`, so a ValueId minted in one connection resolves to
/// None (→ Denied) in another. There is no shared cross-session store.

use crate::audit::append_event;
use crate::proto::{BrokerRequest, BrokerResponse, WorkerClaim};
use crate::quarantine::{mint_from_intent, mint_from_read, Claim};
use runtime_core::intent::CaprunIntent;
use crate::session::{create_session, persist_session};
use adapter_fs::pass_fd;
use anyhow::Context;
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::Event;
use std::os::unix::io::AsRawFd;
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
/// loop and spawns a per-connection task. Each connection owns its own
/// `ValueStore` and threads the causal chain forward from the supplied initial
/// session_created event state.
///
/// # Arguments
/// * `session_id`            — string form, used as the socket name suffix.
/// * `conn`                  — shared, mutex-protected SQLite connection.
/// * `session_id_uuid`       — broker-authoritative session identity (HARD-03).
/// * `initial_last_event_id` — id of the `session_created` event minted upstream.
/// * `initial_last_event_hash` — hash of that event row (chain anchor).
///
/// The `value_store` is NO LONGER a parameter — it is created fresh per
/// connection inside `handle_connection` to enforce session-scoped resolution.
///
/// Returns when the accept loop encounters an unrecoverable error.
pub async fn run_broker_server(
    session_id: &str,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id_uuid: Uuid,
    initial_last_event_id: Uuid,
    initial_last_event_hash: String,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
) -> anyhow::Result<()> {
    // Approach A: tokio detects the leading NUL and calls from_abstract_name internally.
    let sock_path = format!("\0/agentos/{session_id}");
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    loop {
        let (stream, _addr) = listener.accept().await?;
        let conn_clone = conn.clone();
        // Workspace-root capability — cloned per connection exactly like `conn`.
        let workspace_root_clone = workspace_root.clone();
        // Pass connection state by value — each connection owns its own chain state.
        let initial_hash = initial_last_event_hash.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                stream,
                conn_clone,
                session_id_uuid,
                initial_last_event_id,
                initial_hash,
                workspace_root_clone,
            )
            .await
            {
                eprintln!("[brokerd] connection error: {e}");
            }
        });
    }
}

/// Handle one IPC connection: loop reading framed requests and dispatching them.
///
/// Framing: 4-byte LE length prefix + JSON body.
/// Guard: length > 64 KiB → reject with Error response, never allocate.
///
/// Owns a per-connection `ValueStore` (HARD-03: session-scoped resolution) and
/// threads `last_event_id` / `last_event_hash` across every message so each
/// appended event chains causally onto the previous one.
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    mut last_event_id: Uuid,
    mut last_event_hash: String,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
) -> anyhow::Result<()> {
    // Per-connection ValueStore — scoped to this session ONLY (HARD-03 fix).
    let mut value_store = ValueStore::default();

    loop {
        // Read 4-byte LE length prefix; clean EOF ends the connection loop.
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let msg_len = u32::from_le_bytes(len_buf) as usize;

        // T-03-08: guard before allocation — reject oversized messages.
        if msg_len > MAX_MSG_SIZE {
            send_response(
                &mut stream,
                &BrokerResponse::Error {
                    message: "message too large".into(),
                },
            )
            .await?;
            break;
        }

        let mut body = vec![0u8; msg_len];
        stream.read_exact(&mut body).await?;

        // Deserialize — serde errors → generic Error response, never panic (T-03-08/09).
        let request = match serde_json::from_slice::<BrokerRequest>(&body) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("[brokerd] deserialize error: {e}");
                send_response(
                    &mut stream,
                    &BrokerResponse::Error {
                        message: "invalid request".into(),
                    },
                )
                .await?;
                break;
            }
        };

        dispatch_request(
            request,
            &mut stream,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut value_store,
            &workspace_root,
        )
        .await?;
    }
    Ok(())
}

/// Dispatch a single `BrokerRequest`, writing its response to `stream`.
///
/// Extracted from the accept loop (RESEARCH Pitfall 6) so individual arms are
/// unit-testable against a `UnixStream::pair()` without binding a real socket.
///
/// `last_event_id` / `last_event_hash` are advanced (after a successful append)
/// by every arm that records an event, preserving the causal chain.
pub async fn dispatch_request(
    request: BrokerRequest,
    stream: &mut tokio::net::UnixStream,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
    value_store: &mut ValueStore,
    workspace_root: &Arc<adapter_fs::workspace::WorkspaceRoot>,
) -> anyhow::Result<()> {
    match request {
        BrokerRequest::CreateSession { intent_id } => {
            // Session-independent: mints its own fresh session (parent_id None);
            // does NOT thread the connection chain.
            let session = create_session(intent_id);
            let new_session_id = session.id;

            let event = Event::new(
                Uuid::new_v4(),
                None,
                new_session_id,
                "broker".into(),
                "session_created".into(),
                Utc::now(),
                vec![],
            );

            let result: anyhow::Result<()> = match conn.lock() {
                Ok(locked) => persist_session(&locked, &session)
                    .and_then(|_| append_event(&locked, &event, None).map(|_| ())),
                Err(e) => Err(anyhow::anyhow!("mutex poisoned: {e}")),
            };

            match result {
                Ok(_) => {
                    send_response(
                        stream,
                        &BrokerResponse::SessionCreated {
                            session_id: new_session_id,
                        },
                    )
                    .await?;
                }
                Err(e) => {
                    eprintln!("[brokerd] CreateSession error: {e}");
                    send_response(
                        stream,
                        &BrokerResponse::Error {
                            message: "internal error".into(),
                        },
                    )
                    .await?;
                }
            }
        }

        BrokerRequest::RequestFd { path } => {
            // HARD-04: resolve the worker-supplied path BENEATH the workspace
            // dirfd anchor via a single openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)
            // syscall. Absolute paths, `..` traversal, and symlink escapes are
            // rejected at kernel resolution time (fail-closed) — the broker no
            // longer opens a worker-controlled path via ambient std::fs::File::open.
            let file = workspace_root
                .read_within(&path)
                .with_context(|| format!("broker: read_within {path}"))?;
            let file_fd = file.as_raw_fd();

            // Append fd_granted Event — causal parent is the prior event (not None).
            let fd_event_id = Uuid::new_v4();
            let fd_event = Event::new(
                fd_event_id,
                Some(*last_event_id),
                session_id,
                "broker".into(),
                "fd_granted".into(),
                Utc::now(),
                vec![],
            );
            let fd_hash = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                append_event(&locked, &fd_event, Some(last_event_hash))
                    .context("append fd_granted")?
            };

            // pass_fd is blocking (sendmsg) — MUST use spawn_blocking (RESEARCH Pitfall 2).
            let sock_fd = stream.as_raw_fd();
            tokio::task::spawn_blocking(move || {
                let result = pass_fd(sock_fd, file_fd).map_err(|e| anyhow::anyhow!("pass_fd: {e}"));
                drop(file); // close broker's copy after sendmsg completes
                result
            })
            .await
            .context("spawn_blocking pass_fd")??;

            send_response(stream, &BrokerResponse::FdGranted).await?;

            // Advance the chain ONLY after the append + fd-pass succeeded.
            *last_event_id = fd_event_id;
            *last_event_hash = fd_hash;
        }

        BrokerRequest::ReportClaims { claims } => {
            // Mint one genuinely-tainted ValueRecord per typed claim.
            // mint_from_read is the SOLE taint-mint site — NEVER call ValueStore::mint
            // directly on the live path (taint stapling fails §9, T-04-03/T-05-05).
            let mut value_ids = Vec::new();
            for claim in claims {
                match claim {
                    WorkerClaim::EmailAddress(addr) => {
                        let quarantine_claim = Claim {
                            claim_type: "email_address".into(),
                            value: addr,
                        };
                        let (read_event_id, read_hash, value_id) = {
                            let locked = conn
                                .lock()
                                .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                            mint_from_read(
                                &locked,
                                value_store,
                                session_id,
                                &quarantine_claim,
                                Some(last_event_hash),
                            )?
                        };
                        *last_event_id = read_event_id;
                        *last_event_hash = read_hash;
                        value_ids.push(value_id);
                    } // Exhaustive enum: any future variant fails closed at deserialize.
                }
            }
            send_response(stream, &BrokerResponse::ClaimsReceived { value_ids }).await?;
        }

        BrokerRequest::SubmitPlanNode { plan_node } => {
            // session_id comes from the connection — NEVER from the IPC message (HARD-03).
            let decision = executor::submit_plan_node(session_id, &plan_node, value_store);

            // Durably record the decision BEFORE returning any response (ACC-02).
            // Fail-closed: an append error propagates with `?`, so the block is
            // NEVER reported to the worker as having succeeded.
            let event_type = match &decision {
                runtime_core::ExecutorDecision::BlockedPendingConfirmation { .. } => "sink_blocked",
                _ => "plan_node_evaluated",
            };
            let audit_event = Event::new(
                Uuid::new_v4(),
                Some(*last_event_id), // causal parent preserved — not None
                session_id,
                "executor".into(),
                event_type.into(),
                Utc::now(),
                vec![],
            );
            let new_hash = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                append_event(&locked, &audit_event, Some(last_event_hash)).map_err(|e| {
                    eprintln!("[brokerd] {event_type} audit append FAILED (fail-closed): {e}");
                    anyhow::anyhow!("audit append failed: {e}")
                })?
            };
            *last_event_id = audit_event.id;
            *last_event_hash = new_hash;

            // Only send the decision AFTER the durable append succeeded.
            send_response(stream, &BrokerResponse::PlanNodeDecision { decision }).await?;
        }

        BrokerRequest::ProvideIntent { intent } => {
            // Extract the user-provided literal from the typed intent.
            // Exhaustive match: adding a new CaprunIntent variant causes a compile error here,
            // forcing the dispatcher to be updated (no silent unhandled variants).
            let literal = match &intent {
                CaprunIntent::SendEmailSummary { recipient } => recipient.clone(),
            };

            // Mint inside the per-connection ValueStore (Pitfall 1: minting outside
            // handle_connection would put the ValueId in an unreachable store → Denied).
            // mint_from_intent is the ONLY caller site of mint_from_intent (T-06-05).
            let (intent_event_id, intent_hash, value_id) = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                mint_from_intent(&locked, value_store, session_id, literal, Some(last_event_hash))?
            };

            // Advance the causal chain AFTER successful mint (same pattern as ReportClaims arm).
            *last_event_id = intent_event_id;
            *last_event_hash = intent_hash;

            send_response(stream, &BrokerResponse::IntentAccepted { value_id }).await?;
        }

        BrokerRequest::ReportRead { .. } => {
            // Deprecated: the live taint path is ReportClaims (typed extracts).
            // The variant remains in proto for wire compatibility but is no longer
            // a live broker path — direct callers to ReportClaims.
            send_response(
                stream,
                &BrokerResponse::Error {
                    message: "ReportRead is deprecated — use ReportClaims".into(),
                },
            )
            .await?;
        }
    }
    Ok(())
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
