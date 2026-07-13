//! planner_reduced_signal — proof that a planner-role connection's
//! `SubmitPlanNode` response is reduced to a proceed/blocked signal, never
//! the full decision (Phase 20, PLANNER-04, `DESIGN-session-trust-coherence.md`
//! §7/§8 DESIGN-04 — closing the decision-oracle for the planner connection).
//!
//! Coverage:
//!   1. `plan_node_decision_reduced_round_trips_with_only_blocked_bool` — pure,
//!      macOS-runnable serde/shape assertion: `BrokerResponse::PlanNodeDecisionReduced`
//!      serializes to a JSON object whose payload is exactly `{ "blocked":
//!      <bool> }` — NO `anchors`, NO `literal_sha256`, NO `literal` key,
//!      structurally proving the oracle detail is not on the wire (the
//!      reduced variant simply has no fields to carry it).
//!   2. `linux_tests::planner_submit_plan_node_receives_reduced_never_full`
//!      (Linux-only, via `bash scripts/mailpit-verify.sh`) — a real
//!      `run_broker_server` instance: conn#1 (worker) established via
//!      `RequestFd`; conn#2 declares planner role and is Ack'd; conn#2 sends
//!      `SubmitPlanNode`; the response MUST match
//!      `BrokerResponse::PlanNodeDecisionReduced` and MUST NEVER match
//!      `BrokerResponse::PlanNodeDecision`.

use brokerd::proto::BrokerResponse;

/// Task 2 (non-Linux-gated): `PlanNodeDecisionReduced { blocked: bool }`
/// round-trips through serde_json, and its serialized form contains ONLY a
/// boolean `blocked` key — asserted on the parsed `serde_json::Value`'s key
/// set (not a raw string grep) so this proof is robust to formatting.
/// Confirms no `anchors`, `literal_sha256`, or `literal` key is present,
/// which the full `PlanNodeDecision { decision }` shape WOULD carry on a
/// `BlockedPendingConfirmation` decision (`executor_decision.rs`'s
/// `BlockedArg`/`SinkBlockedAnchor`).
#[test]
fn plan_node_decision_reduced_round_trips_with_only_blocked_bool() {
    for blocked in [true, false] {
        let resp = BrokerResponse::PlanNodeDecisionReduced { blocked };
        let json = serde_json::to_value(&resp).expect("serialize PlanNodeDecisionReduced");

        // Round-trips to an equal value.
        let recovered: BrokerResponse =
            serde_json::from_value(json.clone()).expect("deserialize PlanNodeDecisionReduced");
        assert_eq!(resp, recovered);

        // `BrokerResponse` is an externally-tagged enum (serde default), so
        // the serialized shape is `{ "PlanNodeDecisionReduced": { "blocked": bool } }`.
        let obj = json.as_object().expect("top-level JSON object");
        assert_eq!(
            obj.len(),
            1,
            "expected exactly one variant-tag key, got {obj:?}"
        );
        let payload = obj
            .get("PlanNodeDecisionReduced")
            .expect("PlanNodeDecisionReduced tag key present")
            .as_object()
            .expect("variant payload is a JSON object");

        assert_eq!(
            payload.len(),
            1,
            "PlanNodeDecisionReduced payload must carry EXACTLY one field (`blocked`), got {payload:?}"
        );
        assert_eq!(
            payload.get("blocked").and_then(|v| v.as_bool()),
            Some(blocked),
            "payload must carry `blocked: {blocked}`, got {payload:?}"
        );

        // Structural proof (DESIGN §7): none of the full-decision oracle
        // fields are present anywhere in the reduced variant's payload.
        for forbidden_key in ["anchors", "literal_sha256", "literal"] {
            assert!(
                !payload.contains_key(forbidden_key),
                "PlanNodeDecisionReduced must NEVER carry `{forbidden_key}` \
                 (DESIGN-session-trust-coherence.md §7 oracle reduction), \
                 got payload {payload:?}"
            );
        }
    }
}

// ===========================================================================
// Linux-only accept-loop integration proof.
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_tests {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::{BrokerRequest, BrokerResponse};
    use brokerd::server::run_broker_server;
    use brokerd::session::persist_session;
    use runtime_core::plan_node::{PlanNode, SinkId};
    use runtime_core::{Session, SessionStatus};
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::task::JoinHandle;
    use uuid::Uuid;

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

    /// Round-trip a `RequestFd` on a WORKER connection (mirrors
    /// `planner_capability_split.rs::request_fd` /
    /// `two_connection_intent_bypass.rs::request_fd`): the broker passes the
    /// fd via `sendmsg(SCM_RIGHTS)` BEFORE the framed `FdGranted` response, so
    /// a plain 1-byte read drains that mandatory iov payload first. This test
    /// only needs conn#1 to be a functioning worker connection occupying the
    /// worker slot, not to actually read via the granted fd.
    async fn request_fd(stream: &mut tokio::net::UnixStream, path: &str) -> BrokerResponse {
        send_req(
            stream,
            &BrokerRequest::RequestFd {
                path: path.to_string(),
            },
        )
        .await;
        let mut iov_byte = [0u8; 1];
        stream
            .read_exact(&mut iov_byte)
            .await
            .expect("read SCM_RIGHTS iov payload byte");
        read_resp(stream).await
    }

    /// A brand-new `run_broker_server` instance on its own unique socket, its
    /// own in-memory audit DB, and a seeded `sessions` row — mirrors
    /// `planner_capability_split.rs::spawn_fresh_broker` (each test gets its
    /// own broker so its one-way occupancy latches start unset).
    struct FreshBroker {
        sock_path: String,
        ws_dir: PathBuf,
        server_handle: JoinHandle<()>,
    }

    impl FreshBroker {
        fn teardown(self) {
            self.server_handle.abort();
            let _ = std::fs::remove_dir_all(&self.ws_dir);
        }
    }

    async fn spawn_fresh_broker(variant: &str) -> FreshBroker {
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

        let pid = std::process::id();
        let ws_dir = std::env::temp_dir().join(format!("caprun-planner-reduced-{pid}-{variant}"));
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        std::fs::write(ws_dir.join("attacker_doc.txt"), b"attacker@evil.com")
            .expect("write attacker doc");
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(ws_dir.as_path()).expect("open ws root"),
        );

        let server_session_name = format!("caprun-planner-reduced-{pid}-{variant}");
        let sock_path = format!("\0/agentos/{server_session_name}");

        let conn_clone = conn.clone();
        let name_clone = server_session_name.clone();
        let ws_root_clone = ws_root.clone();
        let server_handle = tokio::spawn(async move {
            let _ = run_broker_server(
                &name_clone,
                conn_clone,
                session_id_uuid,
                Uuid::new_v4(),
                "genesis-hash".into(),
                SessionStatus::Active,
                ws_root_clone,
                std::env::temp_dir().join("__planner_reduced_signal_no_trusted_path__"),
                Arc::new([0u8; 32]), // HARDEN-02 broker MAC key (test)
            )
            .await;
        });

        tokio::task::yield_now().await;

        FreshBroker {
            sock_path,
            ws_dir,
            server_handle,
        }
    }

    /// The full accept-loop proof: conn#1 (worker) established via
    /// `RequestFd`; conn#2 declares planner role and is Ack'd; conn#2 sends
    /// `SubmitPlanNode` (any plan node — under HARD-03 the planner's own
    /// `ValueStore` is empty, so an args-bearing node's handles would not
    /// resolve; this test uses an empty-args node so the point is purely the
    /// RESPONSE SHAPE, not a particular `ExecutorDecision` outcome). Asserts
    /// the response matches `BrokerResponse::PlanNodeDecisionReduced` and is
    /// NEVER `BrokerResponse::PlanNodeDecision` — because the reduced variant
    /// structurally has no `anchors`/`literal_sha256`/`literal` fields,
    /// matching it proves the oracle detail is not on the wire.
    #[tokio::test]
    async fn planner_submit_plan_node_receives_reduced_never_full() {
        let broker = spawn_fresh_broker("reduced-signal").await;

        // conn#1 — the worker's single connection, occupying the worker slot.
        let mut conn1 = tokio::net::UnixStream::connect(&broker.sock_path)
            .await
            .expect("connect conn1");
        let fd_resp1 = request_fd(&mut conn1, "attacker_doc.txt").await;
        assert!(
            matches!(fd_resp1, BrokerResponse::FdGranted),
            "conn#1 (worker) RequestFd should be granted, got {fd_resp1:?}"
        );

        // conn#2 — declares planner role as its FIRST framed message.
        let mut conn2 = tokio::net::UnixStream::connect(&broker.sock_path)
            .await
            .expect("connect conn2");
        send_req(&mut conn2, &BrokerRequest::DeclarePlannerRole).await;
        let declare_resp = read_resp(&mut conn2).await;
        assert!(
            matches!(declare_resp, BrokerResponse::Ack),
            "a declared planner connection must be Ack'd, got {declare_resp:?}"
        );

        // conn#2 submits a plan node — the RESPONSE SHAPE is what this test
        // proves, not a particular ExecutorDecision outcome.
        send_req(
            &mut conn2,
            &BrokerRequest::SubmitPlanNode {
                plan_node: PlanNode {
                    sink: SinkId("noop.test".into()),
                    args: vec![],
                },
            },
        )
        .await;
        let submit_resp = read_resp(&mut conn2).await;

        assert!(
            matches!(submit_resp, BrokerResponse::PlanNodeDecisionReduced { .. }),
            "a planner-role connection's SubmitPlanNode response must be \
             PlanNodeDecisionReduced, got {submit_resp:?}"
        );
        assert!(
            !matches!(submit_resp, BrokerResponse::PlanNodeDecision { .. }),
            "a planner-role connection must NEVER receive the full \
             PlanNodeDecision (anchors/literal_sha256/literal), got {submit_resp:?}"
        );

        drop(conn1);
        drop(conn2);
        broker.teardown();
    }
}
