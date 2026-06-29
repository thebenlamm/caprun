/// uds_ipc — broker UDS IPC server integration tests
///
/// Tests that the broker UDS server accepts connections, routes framed messages,
/// creates sessions correctly, and records them in the audit DAG.
///
/// Abstract-namespace UDS is Linux-only; all tests in this module are gated
/// with `#[cfg(target_os = "linux")]`. On macOS, `cargo test -p brokerd` exits 0
/// (zero tests compiled from this file).
///
/// Gate rationale: abstract sockets (`\0/agentos/...`) bypass Landlock filesystem
/// restrictions — they live in the kernel's abstract namespace, not the filesystem.
/// This makes them the correct IPC channel for confined workers, but the feature
/// is a Linux kernel extension not available on Darwin/macOS.

#[cfg(target_os = "linux")]
mod linux_tests {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::{BrokerRequest, BrokerResponse};
    use brokerd::server::run_broker_server;
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use uuid::Uuid;

    /// Helper: send a framed BrokerRequest and receive a BrokerResponse.
    ///
    /// Framing: 4-byte LE length prefix, then JSON body.
    async fn round_trip(
        stream: &mut tokio::net::UnixStream,
        req: &BrokerRequest,
    ) -> BrokerResponse {
        let body = serde_json::to_vec(req).expect("serialize request");
        let len = (body.len() as u32).to_le_bytes();
        stream.write_all(&len).await.expect("write length");
        stream.write_all(&body).await.expect("write body");

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.expect("read length");
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        let mut resp_body = vec![0u8; msg_len];
        stream.read_exact(&mut resp_body).await.expect("read body");
        serde_json::from_slice(&resp_body).expect("deserialize response")
    }

    /// Start a broker server, connect a client, round-trip a CreateSession request,
    /// and assert the server returns SessionCreated.
    #[tokio::test]
    async fn server_accept() {
        let conn: Arc<Mutex<Connection>> =
            Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));

        // Use PID to avoid socket name collisions across parallel test runs
        let pid = std::process::id();
        let server_session_id = format!("ipc-accept-{pid}");
        let sock_path = format!("\0/agentos/{server_session_id}");

        let conn_clone = conn.clone();
        let session_id_clone = server_session_id.clone();
        let server_handle = tokio::spawn(async move {
            let _ = run_broker_server(&session_id_clone, conn_clone).await;
        });

        // Yield once so the server task executes through bind() and into accept().await
        tokio::task::yield_now().await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path)
            .await
            .expect("connect to broker abstract socket");

        let intent_id = Uuid::new_v4();
        let resp = round_trip(&mut stream, &BrokerRequest::CreateSession { intent_id }).await;

        assert!(
            matches!(resp, BrokerResponse::SessionCreated { .. }),
            "expected SessionCreated, got {:?}",
            resp
        );

        server_handle.abort();
    }

    /// Send CreateSession to the broker and assert all three postconditions:
    /// 1. SessionCreated response with a valid UUID.
    /// 2. A `sessions` row exists in the SQLite DB for the returned session_id.
    /// 3. A `session_created` Event exists in the audit DAG for that session.
    #[tokio::test]
    async fn create_session_round_trip() {
        let conn: Arc<Mutex<Connection>> =
            Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));

        let pid = std::process::id();
        let server_session_id = format!("ipc-create-{pid}");
        let sock_path = format!("\0/agentos/{server_session_id}");

        let conn_clone = conn.clone();
        let session_id_clone = server_session_id.clone();
        let server_handle = tokio::spawn(async move {
            let _ = run_broker_server(&session_id_clone, conn_clone).await;
        });

        tokio::task::yield_now().await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path)
            .await
            .expect("connect to broker abstract socket");

        let intent_id = Uuid::new_v4();
        let resp =
            round_trip(&mut stream, &BrokerRequest::CreateSession { intent_id }).await;

        // 1. Assert SessionCreated with a valid UUID
        let returned_session_id = match resp {
            BrokerResponse::SessionCreated { session_id } => session_id,
            other => panic!("expected SessionCreated, got {:?}", other),
        };

        // Yield to allow server to finish writing to DB before we query
        tokio::task::yield_now().await;

        let locked = conn.lock().expect("mutex poisoned");

        // 2. sessions row exists
        let session_count: i64 = locked
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                rusqlite::params![returned_session_id.to_string()],
                |row| row.get(0),
            )
            .expect("query sessions");
        assert_eq!(session_count, 1, "sessions row must exist for returned session_id");

        // 3. session_created Event exists in the audit DAG
        let event_count: i64 = locked
            .query_row(
                "SELECT COUNT(*) FROM events \
                 WHERE session_id = ?1 AND event_type = 'session_created'",
                rusqlite::params![returned_session_id.to_string()],
                |row| row.get(0),
            )
            .expect("query events");
        assert_eq!(
            event_count, 1,
            "session_created event must be in the audit DAG"
        );

        drop(locked);
        server_handle.abort();
    }
}
