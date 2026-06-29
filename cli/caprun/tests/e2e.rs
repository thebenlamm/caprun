/// e2e — substrate demo end-to-end tests (Linux-gated)
///
/// Tests the full Phase 3 mediation proof: `caprun` orchestrates a broker +
/// confined worker. The worker reads a workspace file ONLY via a broker-passed
/// fd (SCM_RIGHTS), and the read lands in the SQLite audit DAG as a `file_read`
/// Event with an unbroken SHA-256 hash chain.
///
/// Both tests are `#[cfg(target_os = "linux")]` because:
/// - Abstract-namespace UDS (abstract socket confinement) is Linux-only.
/// - `sandbox::apply_confinement()` (Landlock + seccomp) is Linux-only.
/// - On macOS: `cargo test -p caprun` exits 0 (e2e cfg-gated out).
///   `cargo build -p caprun --bins` exits 0 (compile proof only).
///
/// NOTE: These tests spawn the real `caprun` and `caprun-worker` binaries using
/// `env!("CARGO_BIN_EXE_caprun")` — Cargo resolves these paths at compile time
/// from the integration-test build context.

/// substrate_demo — the no-LLM complete-mediation proof.
///
/// Writes a known byte string to a temp workspace file, runs `caprun` pointing
/// at that file and a temp audit DB, then asserts:
///  1. caprun exits 0 (entire flow succeeded, no errors).
///  2. A `file_read` Event exists in the audit DAG for the session.
///  3. The `file_read` actor encodes the correct byte count (matching the known
///     string length), proving the worker read via the passed fd and reported
///     the exact size — complete mediation.
#[cfg(target_os = "linux")]
#[test]
fn substrate_demo() {
    use brokerd::audit::open_audit_db;

    // ── Setup: temp workspace file + audit DB path ───────────────────────────
    let run_id = uuid::Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_e2e_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let workspace_file = tmp.join("workspace.txt");
    let audit_db_path = tmp.join("audit.db");

    let known_content = b"caprun substrate demo: no-LLM complete mediation proof 2026";
    std::fs::write(&workspace_file, known_content).expect("write workspace file");

    // ── Run caprun ───────────────────────────────────────────────────────────
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = std::process::Command::new(caprun_bin)
        .arg(workspace_file.to_str().unwrap())
        .arg(audit_db_path.to_str().unwrap())
        .output()
        .expect("spawn caprun");

    // Print stdout/stderr on failure for diagnosis
    if !output.status.success() {
        eprintln!(
            "caprun stdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!(
            "caprun stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        output.status.success(),
        "caprun must exit 0; got: {}",
        output.status
    );

    // ── Verify audit DB: file_read Event exists with correct bytes_read ──────
    let conn = open_audit_db(audit_db_path.to_str().unwrap()).expect("open audit DB");

    // Get the single session's id
    let session_id: String = conn
        .query_row("SELECT id FROM sessions LIMIT 1", [], |row| row.get(0))
        .expect("query session_id");

    // Find the file_read event for this session
    let actor: String = conn
        .query_row(
            "SELECT actor FROM events \
             WHERE session_id = ?1 AND event_type = 'file_read' \
             LIMIT 1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("file_read event must exist in audit DAG");

    // The broker encodes bytes_read in the actor field as "worker:{bytes_read}"
    // (caprun/src/main.rs handle_worker_connection ReportRead branch)
    let bytes_reported: u64 = actor
        .strip_prefix("worker:")
        .unwrap_or_else(|| panic!("unexpected actor format: {actor}"))
        .parse()
        .unwrap_or_else(|e| panic!("parse bytes_read from actor '{actor}': {e}"));

    assert_eq!(
        bytes_reported,
        known_content.len() as u64,
        "worker must report the exact byte count of the known workspace content; \
         actor={actor}"
    );

    // ── Cleanup ──────────────────────────────────────────────────────────────
    std::fs::remove_dir_all(&tmp).ok();
}

/// dag_chain_integrity — verifies the unbroken hash chain: session_created →
/// fd_granted → file_read.
///
/// Runs `caprun` independently of `substrate_demo` (no shared state) and then:
///  1. Calls `brokerd::audit::verify_chain` — asserts the SHA-256 chain is
///     mathematically unbroken (no hash mismatches, no gaps).
///  2. Walks the events in causal depth order and asserts exactly the three
///     expected event types appear in the correct order with linked parent_hashes.
///  3. A broken or gapped chain (e.g., missing fd_granted between session_created
///     and file_read) MUST fail this test.
#[cfg(target_os = "linux")]
#[test]
fn dag_chain_integrity() {
    use brokerd::audit::{open_audit_db, verify_chain};

    // ── Setup ────────────────────────────────────────────────────────────────
    let run_id = uuid::Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_e2e_dag_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let workspace_file = tmp.join("workspace.txt");
    let audit_db_path = tmp.join("audit.db");

    std::fs::write(&workspace_file, b"dag chain integrity test content")
        .expect("write workspace file");

    // ── Run caprun ───────────────────────────────────────────────────────────
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let status = std::process::Command::new(caprun_bin)
        .arg(workspace_file.to_str().unwrap())
        .arg(audit_db_path.to_str().unwrap())
        .status()
        .expect("spawn caprun");
    assert!(status.success(), "caprun must exit 0 for chain test");

    // ── Open audit DB ────────────────────────────────────────────────────────
    let conn = open_audit_db(audit_db_path.to_str().unwrap()).expect("open audit DB");

    // Get session_id
    let session_id: String = conn
        .query_row("SELECT id FROM sessions LIMIT 1", [], |row| row.get(0))
        .expect("sessions table must have one row");

    // ── Assert 1: cryptographic hash chain is unbroken ───────────────────────
    assert!(
        verify_chain(&conn, &session_id),
        "audit DAG hash chain must be unbroken for session {session_id}"
    );

    // ── Assert 2: exactly 3 events in causal order ───────────────────────────
    // Walk the chain via recursive CTE (same traversal as verify_chain).
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE chain(id, event_type, parent_hash, hash, depth) AS (
                 SELECT id, event_type, parent_hash, hash, 0
                 FROM events
                 WHERE session_id = ?1 AND parent_id IS NULL
               UNION ALL
                 SELECT e.id, e.event_type, e.parent_hash, e.hash, c.depth + 1
                 FROM events e
                 JOIN chain c ON e.parent_id = c.id
                 WHERE e.session_id = ?1
             )
             SELECT event_type, parent_hash, hash
             FROM chain
             ORDER BY depth",
        )
        .expect("prepare chain CTE");

    let events: Vec<(String, Option<String>, String)> = stmt
        .query_map([&session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("query chain")
        .map(|r| r.expect("row"))
        .collect();

    assert_eq!(
        events.len(),
        3,
        "audit DAG must contain exactly 3 events (session_created, fd_granted, file_read); \
         got {}: {:?}",
        events.len(),
        events.iter().map(|(et, _, _)| et.as_str()).collect::<Vec<_>>()
    );

    // Verify causal order and parent_hash linkage
    let (e0_type, e0_parent, e0_hash) = &events[0];
    let (e1_type, e1_parent, e1_hash) = &events[1];
    let (e2_type, e2_parent, _e2_hash) = &events[2];

    assert_eq!(e0_type, "session_created", "event[0] must be session_created");
    assert!(
        e0_parent.is_none(),
        "session_created must have no parent_hash; got {e0_parent:?}"
    );

    assert_eq!(e1_type, "fd_granted", "event[1] must be fd_granted");
    assert_eq!(
        e1_parent.as_deref(),
        Some(e0_hash.as_str()),
        "fd_granted.parent_hash must equal session_created.hash"
    );

    assert_eq!(e2_type, "file_read", "event[2] must be file_read");
    assert_eq!(
        e2_parent.as_deref(),
        Some(e1_hash.as_str()),
        "file_read.parent_hash must equal fd_granted.hash"
    );

    // ── Cleanup ──────────────────────────────────────────────────────────────
    std::fs::remove_dir_all(&tmp).ok();
}
