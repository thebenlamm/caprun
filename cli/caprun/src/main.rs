/// caprun — confined-worker orchestrator (Phase 3 substrate demo)
///
/// The no-LLM complete-mediation proof: caprun starts the broker, creates a
/// Session, spawns `caprun-worker` (which self-confines AFTER connecting), and
/// handles the `RequestFd` → fd-pass → `ReportRead` protocol. Every effect
/// is logged to the SQLite audit DAG with an unbroken SHA-256 hash chain.
///
/// Usage: caprun <workspace-file> [audit-db-path]
///
/// # Self-Confinement Order
///
/// caprun spawns the worker as a NORMAL subprocess (no confinement in pre_exec).
/// The worker connects to the broker, THEN calls sandbox::apply_confinement()
/// on itself. This is the self-confinement model required because Landlock
/// deny-all + seccomp deny-execve cannot be applied before exec without
/// preventing the worker binary from loading (Plan 02, self-confinement decision).
///
/// # Broker Interaction (RequestFd protocol)
///
/// The fd is sent via SCM_RIGHTS (sendmsg, 1-byte payload + cmsg) BEFORE the
/// JSON `FdGranted` response. This ensures the worker's `recv_fd` (recvmsg)
/// consumes exactly the 1-byte sendmsg payload, leaving the JSON response
/// intact for the subsequent framed read.
///
/// Pass fd inside `tokio::task::spawn_blocking` — sendmsg is a blocking syscall
/// that would stall the tokio reactor thread (RESEARCH.md Pitfall 4).

use adapter_fs::pass_fd;
use anyhow::Context;
use brokerd::{
    audit::{append_event, open_audit_db, verify_chain},
    proto::{BrokerRequest, BrokerResponse},
    session::{create_session, persist_session},
};
use chrono::Utc;
use runtime_core::Event;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Maximum IPC message size (64 KiB) — T-03-08 DoS mitigate.
const MAX_MSG_SIZE: usize = 64 * 1024;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let workspace_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: caprun <workspace-file> [audit-db-path]"))?;
    let audit_path = args.next().unwrap_or_else(|| ":memory:".to_string());

    // ── 1. Open audit DB ────────────────────────────────────────────────────
    let conn = Arc::new(Mutex::new(
        open_audit_db(&audit_path).context("open_audit_db")?,
    ));

    // ── 2. Create session + persist + append session_created event ──────────
    let intent_id = Uuid::new_v4();
    let session = create_session(intent_id);
    let session_id = session.id;

    let session_created_id = Uuid::new_v4();
    let e_session = Event {
        id: session_created_id,
        parent_id: None,
        session_id,
        actor: "broker".into(),
        event_type: "session_created".into(),
        timestamp: Utc::now(),
        taint: vec![],
    };

    let session_created_hash = {
        let locked = conn.lock().unwrap();
        persist_session(&locked, &session).context("persist_session")?;
        append_event(&locked, &e_session, None).context("append session_created")?
    };

    // ── 3. Bind abstract-namespace UDS socket ────────────────────────────────
    // Approach A (verified in Plan 03): tokio detects the leading NUL and calls
    // from_abstract_name internally — abstract socket bypasses Landlock fs rules.
    let sock_path = format!("\0/agentos/{session_id}");
    let listener = tokio::net::UnixListener::bind(&sock_path).context("bind abstract UDS")?;

    // ── 4. Spawn caprun-worker (NORMAL spawn — worker self-confines after connecting)
    // Resolve caprun-worker from next to the caprun binary (both built to the same
    // target directory). Tests use env!("CARGO_BIN_EXE_caprun-worker") directly.
    let worker_binary = std::env::current_exe()
        .context("current_exe")?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("caprun has no parent dir"))?
        .join("caprun-worker");
    let mut child = std::process::Command::new(&worker_binary)
        // Abstract socket name WITHOUT the leading NUL (worker prepends it)
        .env("BROKER_SOCK", format!("/agentos/{session_id}"))
        .env("SESSION_ID", session_id.to_string())
        .env("WORKSPACE_FILE", &workspace_path)
        .spawn()
        .context("spawn caprun-worker")?;

    // ── 5. Handle broker connection in a task ────────────────────────────────
    let conn_clone = conn.clone();
    let broker_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.context("accept")?;
        handle_worker_connection(
            stream,
            conn_clone,
            session_id,
            session_created_id,
            session_created_hash,
        )
        .await
    });

    // ── 6. Wait for caprun-worker process exit ───────────────────────────────
    // spawn_blocking so child.wait() (blocking) doesn't stall the tokio reactor.
    let child_status = tokio::task::spawn_blocking(move || child.wait())
        .await
        .context("spawn_blocking child.wait")?
        .context("child.wait")?;

    // ── 7. Wait for broker handler to finish ─────────────────────────────────
    broker_task.await.context("broker_task join")??;

    // ── 8. Print audit DAG to stdout ─────────────────────────────────────────
    {
        let locked = conn.lock().unwrap();
        print_audit_dag(&locked, &session_id.to_string())?;
        let chain_ok = verify_chain(&locked, &session_id.to_string());
        println!(
            "\nChain verification: {}",
            if chain_ok { "PASSED" } else { "FAILED" }
        );
    }

    if !child_status.success() {
        anyhow::bail!("caprun-worker exited with status: {child_status}");
    }

    Ok(())
}

/// Handle one worker connection: loop reading framed BrokerRequests, dispatch
/// RequestFd and ReportRead, updating the audit DAG chain on each.
///
/// Protocol for RequestFd:
///   1. Broker opens the file (ambient fs access).
///   2. Broker appends `fd_granted` Event (parent = session_created).
///   3. Broker passes the fd via SCM_RIGHTS inside spawn_blocking (sendmsg is
///      blocking — RESEARCH.md Pitfall 4).  The 1-byte sendmsg payload arrives
///      at the worker BEFORE the JSON FdGranted response.
///   4. Broker sends `FdGranted` JSON response.
async fn handle_worker_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    mut last_event_id: Uuid,
    mut last_event_hash: String,
) -> anyhow::Result<()> {
    loop {
        // Read 4-byte LE length prefix
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let msg_len = u32::from_le_bytes(len_buf) as usize;

        // T-03-08: guard before allocation
        if msg_len > MAX_MSG_SIZE {
            let resp = BrokerResponse::Error {
                message: "message too large".into(),
            };
            send_response(&mut stream, &resp).await?;
            break;
        }

        let mut body = vec![0u8; msg_len];
        stream.read_exact(&mut body).await?;

        let request = match serde_json::from_slice::<BrokerRequest>(&body) {
            Ok(r) => r,
            Err(e) => {
                // T-03-09: internal detail logged, generic message to worker
                eprintln!("[caprun broker] deserialize error: {e}");
                let resp = BrokerResponse::Error {
                    message: "invalid request".into(),
                };
                send_response(&mut stream, &resp).await?;
                break;
            }
        };

        match request {
            BrokerRequest::RequestFd { path } => {
                // Broker opens the file — broker has ambient fs access
                let file = std::fs::File::open(&path)
                    .with_context(|| format!("broker: open {path}"))?;
                let file_fd = file.as_raw_fd();

                // Append fd_granted Event to the audit DAG
                let fd_event_id = Uuid::new_v4();
                let fd_event = Event {
                    id: fd_event_id,
                    parent_id: Some(last_event_id),
                    session_id,
                    actor: "broker".into(),
                    event_type: "fd_granted".into(),
                    timestamp: Utc::now(),
                    taint: vec![],
                };
                let fd_hash = {
                    let locked = conn.lock().unwrap();
                    append_event(&locked, &fd_event, Some(&last_event_hash))
                        .context("append fd_granted")?
                };

                // Pass fd via SCM_RIGHTS inside spawn_blocking (sendmsg is blocking).
                // Move `file` into the closure so the fd stays open during sendmsg.
                // The 1-byte iov payload sent by pass_fd lands in the worker's recv
                // buffer BEFORE the JSON FdGranted response below (protocol ordering).
                let sock_fd = stream.as_raw_fd();
                tokio::task::spawn_blocking(move || {
                    let result = pass_fd(sock_fd, file_fd)
                        .map_err(|e| anyhow::anyhow!("pass_fd: {e}"));
                    drop(file); // close broker's copy after sendmsg completes
                    result
                })
                .await
                .context("spawn_blocking pass_fd")??;

                // Send FdGranted JSON response AFTER the fd has been passed
                send_response(&mut stream, &BrokerResponse::FdGranted).await?;

                // Advance the chain to fd_granted
                last_event_id = fd_event_id;
                last_event_hash = fd_hash;
            }

            BrokerRequest::ReportRead { bytes_read } => {
                // Append file_read Event.
                // taint=[] — clean workspace file (Phase 4 adds genuine taint stapling).
                // actor encodes bytes_read for e2e test verification.
                let read_event = Event {
                    id: Uuid::new_v4(),
                    parent_id: Some(last_event_id),
                    session_id,
                    actor: format!("worker:{bytes_read}"),
                    event_type: "file_read".into(),
                    timestamp: Utc::now(),
                    taint: vec![], // clean workspace file — NOT stapled
                };
                {
                    let locked = conn.lock().unwrap();
                    append_event(&locked, &read_event, Some(&last_event_hash))
                        .context("append file_read")?;
                }

                eprintln!("[caprun broker] file_read: bytes_read={bytes_read}");
                send_response(&mut stream, &BrokerResponse::Ack).await?;
                // ReportRead is the last message in the demo flow
                break;
            }

            BrokerRequest::CreateSession { .. } => {
                // Not used in caprun flow — session is pre-created by caprun itself
                send_response(&mut stream, &BrokerResponse::Error {
                    message: "CreateSession not used in caprun flow".into(),
                })
                .await?;
            }

            // SubmitPlanNode dispatch is wired in Plan 04 (executor integration).
            BrokerRequest::SubmitPlanNode { .. } => {
                send_response(&mut stream, &BrokerResponse::Error {
                    message: "SubmitPlanNode not wired until Plan 04".into(),
                })
                .await?;
            }

            // ReportClaims dispatch is wired in Plan 05 (server rewrite).
            // Stubbed here to keep the match exhaustive after the additive
            // proto.rs change in Plan 01.
            BrokerRequest::ReportClaims { .. } => {
                send_response(&mut stream, &BrokerResponse::Error {
                    message: "ReportClaims not wired until Plan 05".into(),
                })
                .await?;
            }
        }
    }
    Ok(())
}

/// Send a framed BrokerResponse (4-byte LE length prefix + JSON body).
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

/// Print the audit DAG for `session_id` in causal order (depth-first CTE walk).
fn print_audit_dag(conn: &rusqlite::Connection, session_id: &str) -> anyhow::Result<()> {
    println!("\n=== Audit DAG (session {session_id}) ===");
    let mut stmt = conn.prepare(
        "WITH RECURSIVE chain(id, event_type, actor, hash, parent_hash, depth) AS (
             SELECT id, event_type, actor, hash, parent_hash, 0
             FROM events
             WHERE session_id = ?1 AND parent_id IS NULL
           UNION ALL
             SELECT e.id, e.event_type, e.actor, e.hash, e.parent_hash, c.depth + 1
             FROM events e
             JOIN chain c ON e.parent_id = c.id
             WHERE e.session_id = ?1
         )
         SELECT depth, event_type, actor, hash, parent_hash
         FROM chain
         ORDER BY depth",
    )?;

    let rows = stmt.query_map([session_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for row in rows {
        let (depth, event_type, actor, hash, parent_hash): (i64, String, String, String, Option<String>) = row?;
        let indent = "  ".repeat(depth as usize);
        let parent_str = parent_hash.as_deref().map(|h| &h[..8]).unwrap_or("(root)");
        println!(
            "{indent}[{depth}] {event_type} (actor={actor})\n\
             {indent}    hash={} parent={parent_str}",
            &hash[..8]
        );
    }
    Ok(())
}
