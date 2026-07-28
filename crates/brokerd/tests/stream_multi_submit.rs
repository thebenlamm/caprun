//! stream_multi_submit — STREAM-01 broker multi-submit + verify_chain proof
//!
//! Proves, against a REAL `run_broker_server` instance on one connection /
//! Session:
//!
//!   1. N≥2 sequential **DIFFERENT** `SubmitPlanNode` calls (not identical
//!      CAS replay) each receive an independent `PlanNodeDecision`.
//!   2. Both evaluations are attributed to the same `session_id`.
//!   3. After multi-submit, `verify_chain(conn, session_id, key)` is true.
//!   4. At least two plan-node evaluation / sink-related durable events exist
//!      on that session.
//!   5. A mid-stream second `ProvideIntent` is rejected with the existing
//!      once-before-RequestFd fail-closed message (no second UserTrusted mint).
//!
//! # Sink choice (STREAM-01 isolation)
//!
//! Two sequential Allowed nodes on **different sinks** (`file.create` then
//! `file.write`) sharing one UserTrusted path handle from a single
//! `CreateFileFromReport` ProvideIntent. Slot-type binding requires the
//! path role on both sinks (email subject/body roles are rejected on
//! `file.create/path`). Avoids SMTP while exercising real multi-submit +
//! audit chain. STREAM-02 bag threading is proven in planner unit tests +
//! Phase 48 Plan 02 — this file isolates STREAM-01 chain-head continuity.
//!
//! # Linux-only
//!
//! Abstract-namespace UDS is Linux-only (CLAUDE.md). On macOS this file
//! compiles to 0 tests — expected, not a gap. Prefer the project Linux
//! harness for authority consistency with Phase 16+:
//!
//! ```text
//! MAILPIT_VERIFY_CMD='cargo test -p brokerd --test stream_multi_submit -- --nocapture' \
//!   bash scripts/mailpit-verify.sh
//! ```
//!
//! (file.create does not touch SMTP; mailpit-verify is preferred for the
//! shared Linux authority environment, not because this test sends mail.)
//!
//! Frame as STREAM substrate proof — **not** LIVE-07 CLI multi-step DONE.

#![cfg(target_os = "linux")]

use brokerd::audit::{append_event, open_audit_db, verify_chain};
use brokerd::proto::{BrokerRequest, BrokerResponse};
use brokerd::server::run_broker_server;
use brokerd::session::persist_session;
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::{PlanArg, SinkId};
use runtime_core::{Event, ExecutorDecision, PlanNode, Session, SessionStatus};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Fixed test MAC key — must match the key passed to `run_broker_server`
/// in `spawn_fresh_broker` (`Arc::new([0u8; 32])`).
const TEST_MAC_KEY: [u8; 32] = [0u8; 32];

/// Exact broker reject text for a second ProvideIntent (server.rs guard).
const PROVIDE_INTENT_ONCE_MSG: &str =
    "ProvideIntent rejected: must arrive exactly once, before any RequestFd (fail-closed)";

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

/// Spawn a brand-new `run_broker_server` on a unique abstract socket with an
/// in-memory audit DB, seeded Active session, and a **real** `session_created`
/// root event (so `verify_chain` can walk from `parent_id IS NULL` — mirrors
/// `cli/caprun/src/main.rs` production seeding, not the phantom genesis
/// shortcut some replay-only harnesses use).
async fn spawn_fresh_broker() -> (String, Arc<Mutex<Connection>>, Uuid, tokio::task::JoinHandle<()>) {
    let conn: Arc<Mutex<Connection>> =
        Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));

    let session_id_uuid = Uuid::new_v4();
    let (session_created_id, session_created_hash) = {
        let locked = conn.lock().expect("mutex");
        let seeded = Session {
            id: session_id_uuid,
            intent_id: Uuid::new_v4(),
            status: SessionStatus::Active,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        persist_session(&locked, &seeded).expect("seed sessions row");

        // Real causal root so verify_chain has a parent_id IS NULL walk start
        // (production main.rs seeds session_created the same way).
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id_uuid,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash =
            append_event(&locked, &TEST_MAC_KEY, &root, None).expect("append session_created");
        (root.id, root_hash)
    };

    let ws_dir = std::env::temp_dir().join(format!(
        "caprun-stream-multi-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
    let ws_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(ws_dir.as_path()).expect("open ws root"),
    );

    let server_session_name = format!("caprun-stream-multi-{}", Uuid::new_v4());
    let sock_path = format!("\0/agentos/{server_session_name}");

    let conn_clone = conn.clone();
    let name_clone = server_session_name.clone();
    let server_handle = tokio::spawn(async move {
        let _ = run_broker_server(
            &name_clone,
            conn_clone,
            session_id_uuid,
            session_created_id,
            session_created_hash,
            SessionStatus::Active,
            ws_root,
            std::env::temp_dir().join("__stream_multi_no_trusted_path__"),
            Arc::new(TEST_MAC_KEY),
            runtime_core::SessionPolicy::allow_all(),
        )
        .await;
    });

    tokio::task::yield_now().await;

    (sock_path, conn, session_id_uuid, server_handle)
}

/// STREAM-01: N≥2 sequential DIFFERENT SubmitPlanNode on one connection,
/// same session_id, verify_chain true, ≥2 plan-node evaluation events.
#[tokio::test]
async fn multi_submit_different_nodes_same_session_verify_chain() {
    let (sock_path, audit_conn, session_id, server_handle) = spawn_fresh_broker().await;

    let mut stream = tokio::net::UnixStream::connect(&sock_path)
        .await
        .expect("connect");

    // One UserTrusted path handle (role "path") from CreateFileFromReport —
    // slot-type binding accepts this on both file.create and file.write.
    let path = format!("stream-multi-{}.txt", Uuid::new_v4());

    send_req(
        &mut stream,
        &BrokerRequest::ProvideIntent {
            intent: CaprunIntent::CreateFileFromReport {
                path: path.clone(),
            },
            primary_file_derived: false,
        },
    )
    .await;
    let intent_resp = read_resp(&mut stream).await;
    let value_id = match intent_resp {
        BrokerResponse::IntentAccepted { value_id, .. } => value_id,
        other => panic!("expected IntentAccepted, got {other:?}"),
    };

    // Node 1: file.create (new exclusive file)
    let node_1 = PlanNode {
        sink: SinkId("file.create".into()),
        args: vec![
            PlanArg {
                name: "path".into(),
                value_id: value_id.clone(),
            },
            PlanArg {
                name: "contents".into(),
                value_id: value_id.clone(),
            },
        ],
    };
    // Node 2: DIFFERENT plan node — file.write on the same trusted path handle
    // (distinct sink → not identical CAS replay; both Allowed + independent I2).
    let node_2 = PlanNode {
        sink: SinkId("file.write".into()),
        args: vec![
            PlanArg {
                name: "path".into(),
                value_id: value_id.clone(),
            },
            PlanArg {
                name: "contents".into(),
                value_id: value_id.clone(),
            },
        ],
    };
    assert_ne!(
        node_1, node_2,
        "STREAM-01 requires DIFFERENT plan nodes (not identical CAS replay)"
    );

    send_req(
        &mut stream,
        &BrokerRequest::SubmitPlanNode {
            plan_node: node_1,
        },
    )
    .await;
    let first = match read_resp(&mut stream).await {
        BrokerResponse::PlanNodeDecision {
            decision,
            output_value_id: _,
        } => decision,
        other => panic!("first SubmitPlanNode: expected PlanNodeDecision, got {other:?}"),
    };
    assert_eq!(
        first,
        ExecutorDecision::Allowed,
        "first trusted file.create must Allow"
    );

    send_req(
        &mut stream,
        &BrokerRequest::SubmitPlanNode {
            plan_node: node_2,
        },
    )
    .await;
    let second = match read_resp(&mut stream).await {
        BrokerResponse::PlanNodeDecision {
            decision,
            output_value_id: _,
        } => decision,
        other => panic!("second SubmitPlanNode: expected PlanNodeDecision, got {other:?}"),
    };
    assert_eq!(
        second,
        ExecutorDecision::Allowed,
        "second DIFFERENT trusted file.write must Allow independently"
    );

    drop(stream);
    server_handle.abort();

    let session_id_str = session_id.to_string();

    // Chain-head continuity: verify_chain true for this Session after N submits.
    {
        let locked = audit_conn.lock().expect("mutex");
        assert!(
            verify_chain(&locked, &session_id_str, &TEST_MAC_KEY),
            "verify_chain must be true after N sequential DIFFERENT SubmitPlanNode \
             on the same session_id (STREAM-01 chain-head continuity)"
        );
    }

    // At least two plan-node evaluation events on that session_id.
    let plan_eval_count: i64 = {
        let locked = audit_conn.lock().expect("mutex");
        locked
            .query_row(
                "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2",
                rusqlite::params![session_id_str, "plan_node_evaluated"],
                |row| row.get(0),
            )
            .expect("count plan_node_evaluated")
    };
    assert!(
        plan_eval_count >= 2,
        "expected ≥2 plan_node_evaluated events on session, got {plan_eval_count}"
    );

    // Optional sink-related events (file.create Allowed dispatch).
    let sink_related: i64 = {
        let locked = audit_conn.lock().expect("mutex");
        locked
            .query_row(
                "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND \
                 (event_type = 'plan_node_evaluated' OR event_type = 'sink_executed' \
                  OR event_type LIKE 'file_create%')",
                rusqlite::params![session_id_str],
                |row| row.get(0),
            )
            .expect("count sink-related events")
    };
    assert!(
        sink_related >= 2,
        "expected ≥2 plan-node/sink-related durable events, got {sink_related}"
    );
}

/// T4: mid-stream second ProvideIntent is rejected; no second UserTrusted mint.
#[tokio::test]
async fn mid_stream_second_provide_intent_rejected() {
    let (sock_path, audit_conn, session_id, server_handle) = spawn_fresh_broker().await;

    let mut stream = tokio::net::UnixStream::connect(&sock_path)
        .await
        .expect("connect");

    // First ProvideIntent succeeds.
    send_req(
        &mut stream,
        &BrokerRequest::ProvideIntent {
            intent: CaprunIntent::CreateFileFromReport {
                path: format!("once-{}.txt", Uuid::new_v4()),
            },
            primary_file_derived: false,
        },
    )
    .await;
    match read_resp(&mut stream).await {
        BrokerResponse::IntentAccepted { .. } => {}
        other => panic!("first ProvideIntent must IntentAccepted, got {other:?}"),
    }

    // Optional first submit (still multi-submit context) — Allowed file.create.
    // We only need intent handles; for CreateFileFromReport we get one path handle.
    // Skip submit — second ProvideIntent is the load-bearing assert; optional
    // first submit after IntentAccepted still leaves intent_provided=true.

    // Mid-stream second ProvideIntent must fail closed.
    send_req(
        &mut stream,
        &BrokerRequest::ProvideIntent {
            intent: CaprunIntent::CreateFileFromReport {
                path: "launder-attempt.txt".into(),
            },
            primary_file_derived: false,
        },
    )
    .await;
    match read_resp(&mut stream).await {
        BrokerResponse::Error { message } => {
            assert!(
                message.contains("ProvideIntent rejected")
                    && message.contains("exactly once"),
                "second ProvideIntent must use existing once-before-RequestFd \
                 reject message, got: {message}"
            );
            // Pin the known production string so drift is visible.
            assert_eq!(
                message, PROVIDE_INTENT_ONCE_MSG,
                "reject message must match broker production text"
            );
        }
        other => panic!(
            "second ProvideIntent must return BrokerResponse::Error, got {other:?}"
        ),
    }

    drop(stream);
    server_handle.abort();

    // Primary proof is the Error response above (no second UserTrusted mint
    // success). Chain continuity still holds on the session_created + first
    // ProvideIntent mint events already recorded.
    let session_id_str = session_id.to_string();
    let locked = audit_conn.lock().expect("mutex");
    assert!(
        verify_chain(&locked, &session_id_str, &TEST_MAC_KEY),
        "verify_chain must hold after rejected second ProvideIntent \
         (session_created root + first ProvideIntent mint only)"
    );
}
