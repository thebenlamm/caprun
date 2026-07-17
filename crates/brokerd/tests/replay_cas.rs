//! replay_cas — HARDEN-03 behavioral proof (Linux-only, Mailpit-backed)
//!
//! Proves, against a REAL local capture SMTP (Mailpit) and a REAL
//! `run_broker_server` instance, that a replayed IDENTICAL `SubmitPlanNode`
//! on the Allowed (trusted, never-blocked) `email.send` path sends AT MOST
//! ONCE: exactly one Mailpit delivery, one `sent_plan_nodes` row, and one
//! `email_send_attempted` event across two identical submits.
//!
//! Harness style mirrors `email_smtp_acceptance.rs` (Mailpit HTTP polling)
//! and `two_connection_intent_bypass.rs` (a real `run_broker_server`
//! instance driven over a real Unix socket, `ProvideIntent` ->
//! `SubmitPlanNode`) — this test drives the SAME `SubmitPlanNode` message
//! TWICE on one connection/session, rather than across two connections.
//!
//! Linux-only: abstract-namespace UDS + a live SMTP send are both
//! Linux-only project conventions (CLAUDE.md). `cargo test -p brokerd` on
//! macOS compiles this file to 0 tests — expected, not a gap. Run under the
//! Mailpit harness (a benign Allowed `email.send` performs a LIVE SMTP send
//! since Phase 16):
//!
//!   MAILPIT_VERIFY_CMD='cargo test -p brokerd --test replay_cas \
//!     allowed_email_send_replay_delivers_once' bash scripts/mailpit-verify.sh

#![cfg(target_os = "linux")]

use brokerd::audit::open_audit_db;
use brokerd::proto::{BrokerRequest, BrokerResponse};
use brokerd::server::run_broker_server;
use brokerd::session::persist_session;
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::{PlanArg, SinkId};
use runtime_core::{ExecutorDecision, PlanNode, Session, SessionStatus};
use rusqlite::Connection;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Read the Mailpit host — the SAME env var the broker/confirm process reads
/// for the SMTP connection itself (`CAPRUN_SMTP_HOST`, D-04 endpoint
/// sourcing). Defaults to `127.0.0.1` for a locally-running Mailpit instance
/// outside the sidecar's Docker network.
fn mailpit_host() -> String {
    std::env::var("CAPRUN_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Mailpit's HTTP API port is FIXED at 8025 by Mailpit's own convention —
/// distinct from the SMTP port read via `CAPRUN_SMTP_PORT` (1025).
const MAILPIT_HTTP_PORT: u16 = 8025;

/// Minimal blocking HTTP GET, mirroring `email_smtp_acceptance.rs`'s own
/// helper — this is a SEPARATE integration-test binary with no access to
/// that file's private helpers, so it is duplicated rather than shared.
fn http_request(method: &str, host: &str, port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect((host, port))
        .unwrap_or_else(|e| panic!("failed to connect to Mailpit HTTP API at {host}:{port}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request =
        format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .unwrap_or_else(|e| panic!("failed reading Mailpit HTTP API response: {e}"));

    let sep_pos = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| {
            panic!(
                "malformed HTTP response from Mailpit (no header/body separator): {}",
                String::from_utf8_lossy(&raw)
            )
        });
    let headers = String::from_utf8_lossy(&raw[..sep_pos]).to_lowercase();
    let body_bytes = &raw[sep_pos + 4..];

    let body = if headers.contains("transfer-encoding: chunked") {
        decode_chunked(body_bytes)
    } else {
        body_bytes.to_vec()
    };
    String::from_utf8_lossy(&body).into_owned()
}

/// Decode an HTTP/1.1 chunked-transfer-encoded body into its unwrapped
/// bytes. Byte-level only — never a lossy `str` split.
fn decode_chunked(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let nl = match body.windows(2).position(|w| w == b"\r\n") {
            Some(p) => p,
            None => break,
        };
        let size_str = std::str::from_utf8(&body[..nl]).unwrap_or("0").trim();
        let size = usize::from_str_radix(size_str, 16).unwrap_or(0);
        let data_start = nl + 2;
        if size == 0 || data_start + size > body.len() {
            break;
        }
        out.extend_from_slice(&body[data_start..data_start + size]);
        let after_data = data_start + size;
        let next_start = if body.get(after_data..after_data + 2) == Some(b"\r\n") {
            after_data + 2
        } else {
            after_data
        };
        body = &body[next_start..];
    }
    out
}

/// GET a path from Mailpit's HTTP API, parsed as JSON.
fn http_get_json(host: &str, port: u16, path: &str) -> serde_json::Value {
    let body = http_request("GET", host, port, path);
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("failed to parse Mailpit JSON response: {e}\nbody: {body}"))
}

/// Extract every `Address` string from a `To`/`Cc`/`Bcc` array field (empty
/// vec if the field is missing, null, or not an array).
fn addresses(detail: &serde_json::Value, field: &str) -> Vec<String> {
    detail[field]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry["Address"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch a single message's DETAIL via Mailpit's HTTP API — the endpoint
/// whose `To`/`Cc`/`Bcc` fields are always arrays.
fn fetch_message_detail(host: &str, id: &str) -> serde_json::Value {
    http_get_json(host, MAILPIT_HTTP_PORT, &format!("/api/v1/message/{id}"))
}

/// Poll Mailpit's LIST endpoint (bounded wait) and return every message ID
/// addressed to `recipient` currently sitting in the shared inbox. Phase 16
/// (16-04, BLOCKER-3 3.5): NEVER purge-all / NEVER assert a global count —
/// parallel `cargo test --workspace` binaries share this ONE external
/// Mailpit inbox, so every assertion here isolates by a UNIQUE per-test
/// recipient address instead.
fn wait_and_collect_messages_for_recipient(host: &str, recipient: &str, min_count: usize) -> Vec<String> {
    for _ in 0..50 {
        let list = http_get_json(host, MAILPIT_HTTP_PORT, "/api/v1/messages?limit=250");
        let messages = list["messages"].as_array().cloned().unwrap_or_default();
        let mut matched = Vec::new();
        for m in &messages {
            if let Some(id) = m["ID"].as_str() {
                let detail = fetch_message_detail(host, id);
                if addresses(&detail, "To").contains(&recipient.to_string()) {
                    matched.push(id.to_string());
                }
            }
        }
        if matched.len() >= min_count {
            return matched;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!(
        "timed out waiting for >= {min_count} message(s) addressed to {recipient} to appear in Mailpit"
    );
}

/// Send a framed BrokerRequest (4-byte LE length prefix + JSON body).
async fn send_req(stream: &mut tokio::net::UnixStream, req: &BrokerRequest) {
    let body = serde_json::to_vec(req).expect("serialize request");
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await.expect("write length");
    stream.write_all(&body).await.expect("write body");
}

/// Read one framed BrokerResponse (4-byte LE length prefix + JSON body).
async fn read_resp(stream: &mut tokio::net::UnixStream) -> BrokerResponse {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.expect("read length");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_body = vec![0u8; msg_len];
    stream.read_exact(&mut resp_body).await.expect("read body");
    serde_json::from_slice(&resp_body).expect("deserialize response")
}

/// Spawn a brand-new `run_broker_server` instance on its own unique abstract
/// socket, with its own in-memory audit DB (kept alive via the returned
/// `Arc` for post-test `sent_plan_nodes`/`events` assertions) and a seeded
/// `sessions` row (status Active) — mirrors
/// `two_connection_intent_bypass.rs::spawn_fresh_broker`, minus the
/// workspace/RequestFd setup this test never needs (it never calls
/// RequestFd — only `ProvideIntent` -> `SubmitPlanNode`).
async fn spawn_fresh_broker() -> (String, Arc<Mutex<Connection>>, Uuid, tokio::task::JoinHandle<()>) {
    let conn: Arc<Mutex<Connection>> =
        Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));

    let session_id_uuid = Uuid::new_v4();
    {
        let locked = conn.lock().expect("mutex");
        let seeded = Session {
            id: session_id_uuid,
            intent_id: Uuid::new_v4(),
            status: SessionStatus::Active,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        persist_session(&locked, &seeded).expect("seed sessions row");
    }

    let ws_dir = std::env::temp_dir().join(format!(
        "caprun-replay-cas-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
    let ws_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(ws_dir.as_path()).expect("open ws root"),
    );

    let server_session_name = format!("caprun-replay-cas-{}", Uuid::new_v4());
    let sock_path = format!("\0/agentos/{server_session_name}");

    let conn_clone = conn.clone();
    let name_clone = server_session_name.clone();
    let server_handle = tokio::spawn(async move {
        let _ = run_broker_server(
            &name_clone,
            conn_clone,
            session_id_uuid,
            Uuid::new_v4(),        // initial_last_event_id (genesis anchor)
            "genesis-hash".into(), // initial_last_event_hash
            SessionStatus::Active,
            ws_root,
            std::env::temp_dir().join("__replay_cas_no_trusted_path__"),
            Arc::new([0u8; 32]), // HARDEN-02 broker MAC key (test)
        )
        .await;
    });

    // Let the server task reach accept().await.
    tokio::task::yield_now().await;

    (sock_path, conn, session_id_uuid, server_handle)
}

/// (HARDEN-03) A replayed IDENTICAL `SubmitPlanNode` on the Allowed
/// `email.send` path sends AT MOST ONCE: exactly one Mailpit delivery, one
/// `sent_plan_nodes` row, and one `email_send_attempted` event across two
/// identical submits.
#[tokio::test]
async fn allowed_email_send_replay_delivers_once() {
    let host = mailpit_host();
    let recipient = format!("replay-cas-{}@example.test", Uuid::new_v4());

    let (sock_path, audit_conn, session_id, server_handle) = spawn_fresh_broker().await;

    let mut stream = tokio::net::UnixStream::connect(&sock_path)
        .await
        .expect("connect");

    // A single ProvideIntent (no RequestFd first, so guard(a) never trips)
    // mints the recipient/subject/body literals UserTrusted — a benign,
    // never-blocked send.
    send_req(
        &mut stream,
        &BrokerRequest::ProvideIntent {
            intent: CaprunIntent::SendEmailSummary {
                recipient: recipient.clone(),
                subject: "replay-cas test".into(),
                body: "hello from replay_cas.rs".into(),
            },
        },
    )
    .await;
    let intent_resp = read_resp(&mut stream).await;
    let (value_id, subject_value_id, body_value_id) = match intent_resp {
        BrokerResponse::IntentAccepted {
            value_id,
            subject_value_id,
            body_value_id,
        } => (value_id, subject_value_id, body_value_id),
        other => panic!("expected IntentAccepted, got {other:?}"),
    };

    let mut args = vec![PlanArg {
        name: "to".into(),
        value_id: value_id.clone(),
    }];
    if let Some(sid) = subject_value_id {
        args.push(PlanArg {
            name: "subject".into(),
            value_id: sid,
        });
    }
    if let Some(bid) = body_value_id {
        args.push(PlanArg {
            name: "body".into(),
            value_id: bid,
        });
    }
    let plan_node = PlanNode {
        sink: SinkId("email.send".into()),
        args,
    };

    // Submit the IDENTICAL SubmitPlanNode TWICE on the SAME connection /
    // session / audit.db.
    send_req(
        &mut stream,
        &BrokerRequest::SubmitPlanNode {
            plan_node: plan_node.clone(),
        },
    )
    .await;
    let first_resp = read_resp(&mut stream).await;
    let first_decision = match first_resp {
        BrokerResponse::PlanNodeDecision { decision } => decision,
        other => panic!("first SubmitPlanNode: expected PlanNodeDecision, got {other:?}"),
    };
    assert_eq!(
        first_decision,
        ExecutorDecision::Allowed,
        "a benign, never-blocked email.send must Allow (control)"
    );

    send_req(
        &mut stream,
        &BrokerRequest::SubmitPlanNode {
            plan_node: plan_node.clone(),
        },
    )
    .await;
    let second_resp = read_resp(&mut stream).await;
    let second_decision = match second_resp {
        BrokerResponse::PlanNodeDecision { decision } => decision,
        other => panic!("replayed SubmitPlanNode: expected PlanNodeDecision, got {other:?}"),
    };
    assert_eq!(
        second_decision,
        ExecutorDecision::Allowed,
        "the replayed IDENTICAL plan node must still evaluate Allowed — the \
         CAS suppresses the SMTP send, it does not change the executor's \
         decision"
    );

    drop(stream);
    server_handle.abort();

    // ── Assertion 1: exactly ONE Mailpit delivery ──────────────────────
    let delivered = wait_and_collect_messages_for_recipient(&host, &recipient, 1);
    // Give Mailpit's async ingest a brief settle window in case a (buggy)
    // second send is still in flight, then re-poll once more before
    // asserting the exact count — avoids a false PASS from checking too
    // early.
    std::thread::sleep(Duration::from_millis(500));
    let list = http_get_json(&host, MAILPIT_HTTP_PORT, "/api/v1/messages?limit=250");
    let messages = list["messages"].as_array().cloned().unwrap_or_default();
    let mut final_count = 0usize;
    for m in &messages {
        if let Some(id) = m["ID"].as_str() {
            let detail = fetch_message_detail(&host, id);
            if addresses(&detail, "To").contains(&recipient) {
                final_count += 1;
            }
        }
    }
    assert_eq!(
        final_count, 1,
        "a replayed identical Allowed email.send must deliver EXACTLY ONCE \
         via Mailpit, got {final_count} deliveries: {delivered:?}"
    );

    // ── Assertion 2: exactly ONE sent_plan_nodes row for this plan node ──
    let sent_plan_nodes_count: i64 = {
        let locked = audit_conn.lock().expect("mutex");
        locked
            .query_row(
                "SELECT COUNT(*) FROM sent_plan_nodes WHERE session_id = ?1",
                rusqlite::params![session_id.to_string()],
                |row| row.get(0),
            )
            .expect("count sent_plan_nodes rows")
    };
    assert_eq!(
        sent_plan_nodes_count, 1,
        "exactly ONE sent_plan_nodes row must exist for this plan node after \
         two identical submits"
    );

    // ── Assertion 3: exactly ONE email_send_attempted event ─────────────
    let attempted_event_count: i64 = {
        let locked = audit_conn.lock().expect("mutex");
        locked
            .query_row(
                "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2",
                rusqlite::params![session_id.to_string(), "email_send_attempted"],
                |row| row.get(0),
            )
            .expect("count email_send_attempted events")
    };
    assert_eq!(
        attempted_event_count, 1,
        "exactly ONE email_send_attempted event must be recorded across two \
         identical submits — the replay's attempt is durably recorded, but \
         it must not duplicate the event, mirroring the single \
         sent_plan_nodes row"
    );
}
