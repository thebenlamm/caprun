//! planner_capability_split — proof of the per-connection capability split
//! (Phase 20, PLANNER-02/PLANNER-04, `DESIGN-session-trust-coherence.md` §3).
//!
//! A planner-role connection must NEVER be able to invoke any of the four
//! trust-minting verbs (`ProvideIntent`, `ReportClaims`, `ReportDerivedClaim`,
//! `CreateSession`) or either raw-bytes verb (`RequestFd`, `ReportRead`) —
//! fail-closed, default-deny, decided once at connection establishment and
//! never re-derived per message.
//!
//! Coverage:
//!   1. `planner_role_permits_only_submit` (Task 1) — pure, macOS-runnable:
//!      `ConnectionRole::Planner.permits` denies every non-`SubmitPlanNode`
//!      verb (including a mid-stream `DeclarePlannerRole` re-handshake);
//!      `ConnectionRole::Worker.permits` permits all of them.
//!   2. `planner_role_gate_denies_every_mint_and_fd_verb` (Task 2) — pure,
//!      macOS-runnable: the SAME pure gate, framed explicitly around "what a
//!      planner connection's pre-dispatch gate in `handle_connection` will
//!      reject" — the wire-level proof that the gate actually fires inside a
//!      running connection is the Linux-gated integration test below (Task 3),
//!      per this plan's own fallback: `handle_connection` is a private fn not
//!      directly callable from an integration-test binary, so the pure-gate
//!      assertion plus the Linux wire-level test together constitute the full
//!      proof.
//!   3. `linux_tests::planner_second_connection_accepted_and_capability_restricted`
//!      (Task 3, Linux-only) — a real `run_broker_server` instance: conn#1
//!      (worker) established via `RequestFd`; conn#2 declares planner role
//!      and is Ack'd; every mint verb + `RequestFd` sent on conn#2 is
//!      rejected with `Error`; a 3rd connection declaring planner role after
//!      the planner slot is taken is rejected; a plain 2nd-worker-style
//!      connection (no handshake) is rejected exactly as Phase 19 rejects it.

use brokerd::proto::{BrokerRequest, TransformKind, WorkerClaim};
use brokerd::server::ConnectionRole;
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::{PlanNode, SinkId, ValueId};
use uuid::Uuid;

/// Every non-`SubmitPlanNode` `BrokerRequest` verb, named, that a planner
/// connection must never be permitted to invoke — the four mint verbs
/// (`ProvideIntent`, `ReportClaims`, `ReportDerivedClaim`, `CreateSession`),
/// the two raw-bytes verbs (`RequestFd`, `ReportRead`), and a mid-stream
/// `DeclarePlannerRole` re-handshake.
fn denied_planner_requests() -> Vec<(&'static str, BrokerRequest)> {
    vec![
        (
            "ProvideIntent",
            BrokerRequest::ProvideIntent {
                intent: CaprunIntent::CreateFileFromReport {
                    path: "report.txt".into(),
                },
            },
        ),
        (
            "ReportClaims",
            BrokerRequest::ReportClaims {
                claims: vec![WorkerClaim::EmailAddress("attacker@evil.com".into())],
            },
        ),
        (
            "ReportDerivedClaim",
            BrokerRequest::ReportDerivedClaim {
                transformed_literal: "accounts@evil.com".into(),
                transform: TransformKind::Concat,
                input_value_ids: vec![ValueId::new(), ValueId::new()],
            },
        ),
        (
            "CreateSession",
            BrokerRequest::CreateSession {
                intent_id: Uuid::new_v4(),
            },
        ),
        (
            "RequestFd",
            BrokerRequest::RequestFd {
                path: "workspace.txt".into(),
            },
        ),
        (
            "ReportRead",
            BrokerRequest::ReportRead { bytes_read: 42 },
        ),
        ("DeclarePlannerRole", BrokerRequest::DeclarePlannerRole),
    ]
}

fn sample_submit_plan_node() -> BrokerRequest {
    BrokerRequest::SubmitPlanNode {
        plan_node: PlanNode {
            sink: SinkId("noop.test".into()),
            args: vec![],
        },
    }
}

/// Task 1: `ConnectionRole::Planner.permits` is `false` for every verb in
/// `denied_planner_requests` and `true` for `SubmitPlanNode`;
/// `ConnectionRole::Worker.permits` is `true` for all of them (no worker
/// behavior change). Pure function — not Linux-gated, passes on macOS.
#[test]
fn planner_role_permits_only_submit() {
    for (name, req) in denied_planner_requests() {
        assert!(
            !ConnectionRole::Planner.permits(&req),
            "ConnectionRole::Planner must NOT permit {name}"
        );
        assert!(
            ConnectionRole::Worker.permits(&req),
            "ConnectionRole::Worker must permit {name} (no worker behavior change)"
        );
    }

    let submit = sample_submit_plan_node();
    assert!(
        ConnectionRole::Planner.permits(&submit),
        "ConnectionRole::Planner must permit SubmitPlanNode"
    );
    assert!(
        ConnectionRole::Worker.permits(&submit),
        "ConnectionRole::Worker must permit SubmitPlanNode"
    );
}

/// Task 2: the same pure gate, framed as "what `handle_connection`'s
/// pre-dispatch gate rejects for a planner-role connection" — every mint
/// verb and every raw-bytes verb is denied, mints nothing (the gate never
/// reaches `dispatch_request`, so no mint site is ever called). The
/// wire-level proof that the gate is actually wired into a live connection's
/// request loop is `linux_tests::planner_second_connection_accepted_and_capability_restricted`
/// below.
#[test]
fn planner_role_gate_denies_every_mint_and_fd_verb() {
    for (name, req) in denied_planner_requests() {
        assert!(
            !ConnectionRole::Planner.permits(&req),
            "the pre-dispatch gate must deny {name} for a planner-role connection \
             (fail-closed, default-deny — PLANNER-02/PLANNER-04)"
        );
    }
}

// ===========================================================================
// Task 3 — Linux-only accept-loop integration proof.
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_tests {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::BrokerRequest;
    use brokerd::proto::BrokerResponse;
    use brokerd::server::run_broker_server;
    use brokerd::session::persist_session;
    use runtime_core::intent::CaprunIntent;
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
    /// `two_connection_intent_bypass.rs::request_fd`): the broker passes the
    /// fd via `sendmsg(SCM_RIGHTS)` BEFORE the framed `FdGranted` response, so
    /// a plain 1-byte read drains that mandatory iov payload first (some
    /// kernels require >=1 payload byte for cmsg delivery). This test only
    /// needs to prove conn#1 is a functioning worker connection, not to
    /// actually read via the granted fd.
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
    /// own in-memory audit DB, a seeded `sessions` row, and a workspace
    /// containing `attacker_doc.txt` — mirrors
    /// `two_connection_intent_bypass.rs::spawn_fresh_broker` (each test gets
    /// its own broker so its one-way occupancy latches start unset).
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
        let ws_dir = std::env::temp_dir().join(format!("caprun-planner-cap-{pid}-{variant}"));
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        std::fs::write(ws_dir.join("attacker_doc.txt"), b"attacker@evil.com")
            .expect("write attacker doc");
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(ws_dir.as_path()).expect("open ws root"),
        );

        let server_session_name = format!("caprun-planner-cap-{pid}-{variant}");
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
                std::env::temp_dir().join("__planner_capability_split_no_trusted_path__"),
                Arc::new([0u8; 32]), // HARDEN-02 broker MAC key (test)
                runtime_core::SessionPolicy::allow_all(), // POLICY-03 (policy-agnostic test)
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

    /// Task 3's full accept-loop proof: conn#1 (worker) established via
    /// `RequestFd`; conn#2 declares planner role and is Ack'd (accepted as
    /// the ONE additional, capability-restricted connection); every mint verb
    /// + `RequestFd` sent on conn#2 is rejected with `Error` (mints nothing,
    /// grants no fd); conn#3 declaring planner role AFTER the planner slot is
    /// already taken is rejected; conn#4 sending `ProvideIntent` with NO
    /// handshake (a plain 2nd-worker-style connection) is rejected exactly as
    /// Phase 19 rejects it.
    #[tokio::test]
    async fn planner_second_connection_accepted_and_capability_restricted() {
        let broker = spawn_fresh_broker("cap-split").await;

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
            "a declared planner connection must be Ack'd as the session's single \
             additional connection, got {declare_resp:?}"
        );

        // Every mint verb + RequestFd sent on conn#2 must be rejected.
        let denied_on_conn2: Vec<(&str, BrokerRequest)> = vec![
            (
                "ProvideIntent",
                BrokerRequest::ProvideIntent {
                    intent: CaprunIntent::SendEmailSummary {
                        recipient: "attacker@evil.com".into(),
                        subject: "x".into(),
                        body: "y".into(),
                    },
                },
            ),
            (
                "ReportClaims",
                BrokerRequest::ReportClaims { claims: vec![] },
            ),
            (
                "ReportDerivedClaim",
                BrokerRequest::ReportDerivedClaim {
                    transformed_literal: "accounts@evil.com".into(),
                    transform: brokerd::proto::TransformKind::Concat,
                    input_value_ids: vec![],
                },
            ),
            (
                "CreateSession",
                BrokerRequest::CreateSession {
                    intent_id: Uuid::new_v4(),
                },
            ),
            (
                "RequestFd",
                BrokerRequest::RequestFd {
                    path: "attacker_doc.txt".into(),
                },
            ),
        ];
        for (name, req) in denied_on_conn2 {
            send_req(&mut conn2, &req).await;
            let resp = read_resp(&mut conn2).await;
            assert!(
                matches!(resp, BrokerResponse::Error { .. }),
                "{name} must be rejected fail-closed on a planner-role connection, got {resp:?}"
            );
        }

        // conn#3 — declares planner role AFTER the planner slot is already
        // taken by conn#2; must be rejected (the planner slot is a one-way
        // latch, exactly like the worker slot).
        let mut conn3 = tokio::net::UnixStream::connect(&broker.sock_path)
            .await
            .expect("connect conn3");
        send_req(&mut conn3, &BrokerRequest::DeclarePlannerRole).await;
        let conn3_resp = read_resp(&mut conn3).await;
        assert!(
            matches!(conn3_resp, BrokerResponse::Error { .. }),
            "a 2nd planner-role declaration must be rejected once the planner slot \
             is taken, got {conn3_resp:?}"
        );

        // conn#4 — a plain 2nd-worker-style connection with NO handshake
        // (sends ProvideIntent as its first frame) must be rejected exactly
        // as Phase 19 rejects it.
        let mut conn4 = tokio::net::UnixStream::connect(&broker.sock_path)
            .await
            .expect("connect conn4");
        send_req(
            &mut conn4,
            &BrokerRequest::ProvideIntent {
                intent: CaprunIntent::CreateFileFromReport {
                    path: "z".into(),
                },
            },
        )
        .await;
        let conn4_resp = read_resp(&mut conn4).await;
        assert!(
            matches!(conn4_resp, BrokerResponse::Error { .. }),
            "an un-declared 2nd connection must still be rejected exactly as \
             Phase 19 rejects it, got {conn4_resp:?}"
        );

        drop(conn1);
        drop(conn2);
        drop(conn3);
        drop(conn4);
        broker.teardown();
    }
}
