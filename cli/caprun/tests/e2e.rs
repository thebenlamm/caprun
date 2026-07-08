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
///
/// TEST ISOLATION (dag_chain_integrity flake): the two tests below each spawn a
/// full `caprun` → broker + worker process tree. The broker binds a per-run-unique
/// abstract UDS (`\0/agentos/{session_id}`, a fresh UUID), so this is NOT a
/// socket-name collision. The intermittent failure under parallel runs is a
/// spawn/accept ordering race: caprun (`#[tokio::main]`, multi-threaded) spawns the
/// broker task with only a best-effort `yield_now()` before spawning the worker,
/// and the worker connects to the abstract socket exactly once (no retry). Under
/// CPU oversubscription (default cargo test runs these two tests on parallel
/// threads) the broker's `bind()` can lose the race to the worker's single-shot
/// `connect()`. Serializing the two tests removes that in-binary contention so each
/// caprun process tree comes up uncontended. This is a test-scoped isolation fix;
/// it does not touch the broker/worker wiring.
#[cfg(target_os = "linux")]
static E2E_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// substrate_demo — the no-LLM complete-mediation proof.
///
/// Writes benign workspace content (no email address → zero claims), runs
/// `caprun` pointing at that file and a temp audit DB, then asserts:
///  1. caprun exits 0 (entire flow succeeded; benign content is not blocked).
///  2. A `fd_granted` Event exists in the audit DAG — the worker obtained the
///     file ONLY through a broker-passed fd (complete mediation still holds).
///
/// NOTE: Under the Phase 5 protocol the worker extracts typed claims locally and
/// sends `ReportClaims` (not the old `ReportRead` byte-count). Benign content
/// yields zero claims, so NO `file_read` event is minted — mediation is now
/// proven by the `fd_granted` event rather than the old byte-count actor encoding.
///
/// UPDATED (Phase 15 / 15-04, finding #4): there is no early-exit anymore — a
/// benign `SendEmailSummary` still submits an all-UserTrusted `to`+`subject`+
/// `body` plan node and is Allowed (a `plan_node_evaluated` event now always
/// appears; see `dag_chain_integrity` below for the exact updated event count).
/// This test itself only asserts `fd_granted_count == 1` and exit 0, so it is
/// unaffected by the extra event.
#[cfg(target_os = "linux")]
#[test]
fn substrate_demo() {
    use brokerd::audit::open_audit_db;

    // Serialize against dag_chain_integrity (same binary) — see TEST ISOLATION note.
    // Poison-safe: a panic in the peer test must not wedge this one.
    let _serial = E2E_SERIAL.lock().unwrap_or_else(|p| p.into_inner());

    // ── Setup: temp workspace file + audit DB path ───────────────────────────
    let run_id = uuid::Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_e2e_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let workspace_file = tmp.join("workspace.txt");
    let audit_db_path = tmp.join("audit.db");

    // Benign content — no Reply-To:/Domain:/Body: markers, so the confined
    // worker derives no recipient/body (Phase 15 finding #8) and the plan
    // node it submits is all-UserTrusted → Allowed (there is no early-exit
    // anymore, finding #4).
    let known_content = b"caprun substrate demo: no-LLM complete mediation proof 2026";
    std::fs::write(&workspace_file, known_content).expect("write workspace file");

    // ── Run caprun ───────────────────────────────────────────────────────────
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = std::process::Command::new(caprun_bin)
        .arg("send-email-summary")
        .arg("demo@example.test")
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
        "caprun must exit 0 for benign content; got: {}",
        output.status
    );

    // ── Verify audit DB: fd_granted Event exists (broker-mediated fd) ─────────
    let conn = open_audit_db(audit_db_path.to_str().unwrap()).expect("open audit DB");

    // Get the single session's id
    let session_id: String = conn
        .query_row("SELECT id FROM sessions LIMIT 1", [], |row| row.get(0))
        .expect("query session_id");

    // The worker obtained the file ONLY through a broker-passed fd — proven by a
    // fd_granted event for this session (complete mediation).
    let fd_granted_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events \
             WHERE session_id = ?1 AND event_type = 'fd_granted'",
            [&session_id],
            |row| row.get(0),
        )
        .expect("query fd_granted events");
    assert_eq!(
        fd_granted_count, 1,
        "exactly one fd_granted event must exist — the broker mediated the fd pass"
    );

    // ── Cleanup ──────────────────────────────────────────────────────────────
    std::fs::remove_dir_all(&tmp).ok();
}

/// dag_chain_integrity — verifies the unbroken hash chain:
/// session_created → intent_received(recipient) → intent_received(subject) →
/// intent_received(body) → fd_granted → plan_node_evaluated (the 6-event
/// benign chain, UPDATED Phase 15 / 15-04 — see BLOCKER note below).
/// EMPIRICALLY VERIFIED under Colima/Docker (Linux) at 15-04 time — this is
/// not a Mac-side inference.
///
/// Runs `caprun` independently of `substrate_demo` (no shared state) and then:
///  1. Calls `brokerd::audit::verify_chain` — asserts the SHA-256 chain is
///     mathematically unbroken (no hash mismatches, no gaps).
///  2. Walks the events in causal depth order and asserts exactly the six
///     expected event types appear in the correct order with linked parent_hashes.
///  3. A broken or gapped chain (e.g., a missing fd_granted) MUST fail this test.
///
/// BLOCKER (Phase 15 / 15-04, Mac-invisible Linux-gated casualty of removing
/// the worker's email early-exit, finding #4): benign content (no
/// `Reply-To:`/`Domain:`/`Body:` markers) yields zero doc-fragment claims, so
/// no `file_read` event is minted for THIS content — but the worker no longer
/// early-exits before submitting a plan node. A benign `SendEmailSummary` now
/// ALWAYS submits an all-UserTrusted `to`+`subject`+`body` plan node, which
/// the executor Allows, appending a `plan_node_evaluated` event chained onto
/// `fd_granted` (no `file_read` event intervenes, since zero claims were
/// reported).
///
/// ADDITIONALLY (Phase 15 / 15-04, finding #6, discovered while empirically
/// verifying the above under Docker — NOT anticipated by the plan's own
/// 3-event → 4-event framing): `ProvideIntent`'s three sequential
/// `mint_from_intent` calls for `SendEmailSummary` (recipient, subject, body)
/// each append their OWN `intent_received` event, chained onto the previous
/// one (`Some(*last_event_id)` threading in `server.rs`) — so there are
/// THREE `intent_received` events, not one. The full benign chain is
/// session_created → intent_received(recipient) → intent_received(subject) →
/// intent_received(body) → fd_granted → plan_node_evaluated (6 events total).
/// The §9 BLOCK path is exercised by `crates/brokerd/tests/s9_acceptance.rs`
/// and (live) by `s9_live_block.rs::s9_live_email_hostile_block`.
#[cfg(target_os = "linux")]
#[test]
fn dag_chain_integrity() {
    use brokerd::audit::{open_audit_db, verify_chain};

    // Serialize against substrate_demo (same binary) — see TEST ISOLATION note.
    // Poison-safe: a panic in the peer test must not wedge this one.
    let _serial = E2E_SERIAL.lock().unwrap_or_else(|p| p.into_inner());

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
        .arg("send-email-summary")
        .arg("demo@example.test")
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

    // ── Assert 2: exactly 4 events in causal order (Phase 15 / 15-04) ────────
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
        6,
        "audit DAG must contain exactly 6 events (session_created, THREE \
         intent_received — recipient/subject/body, Phase 15 finding #6 — \
         fd_granted, plan_node_evaluated); got {}: {:?}",
        events.len(),
        events.iter().map(|(et, _, _)| et.as_str()).collect::<Vec<_>>()
    );

    // Verify causal order and parent_hash linkage. Empirically confirmed
    // under Colima/Docker (Linux) at 15-04 time: `ProvideIntent`'s three
    // sequential `mint_from_intent` calls (recipient, subject, body — Phase
    // 15 finding #6) EACH thread `parent_id = Some(previous chain head)`, so
    // all three `intent_received` events are causally chained in a single
    // linear walk (contradicting the STALE pre-15-04 comment this replaced,
    // which claimed `intent_received.parent_id == None`); `fd_granted` chains
    // onto the THIRD (body) `intent_received`; the 4th event
    // (`plan_node_evaluated`, Phase 15 / 15-04 finding #4) chains onto
    // `fd_granted` directly — no `file_read` event intervenes, since this
    // benign content yields zero doc-fragment claims.
    let (e0_type, e0_parent, e0_hash) = &events[0];
    let (e1_type, e1_parent, e1_hash) = &events[1];
    let (e2_type, e2_parent, e2_hash) = &events[2];
    let (e3_type, e3_parent, e3_hash) = &events[3];
    let (e4_type, e4_parent, e4_hash) = &events[4];
    let (e5_type, e5_parent, _e5_hash) = &events[5];

    assert_eq!(e0_type, "session_created", "event[0] must be session_created");
    assert!(
        e0_parent.is_none(),
        "session_created must have no parent_hash; got {e0_parent:?}"
    );

    assert_eq!(e1_type, "intent_received", "event[1] must be intent_received (recipient)");
    assert_eq!(
        e1_parent.as_deref(),
        Some(e0_hash.as_str()),
        "intent_received(recipient).parent_hash must equal session_created.hash"
    );

    assert_eq!(e2_type, "intent_received", "event[2] must be intent_received (subject)");
    assert_eq!(
        e2_parent.as_deref(),
        Some(e1_hash.as_str()),
        "intent_received(subject).parent_hash must equal intent_received(recipient).hash"
    );

    assert_eq!(e3_type, "intent_received", "event[3] must be intent_received (body)");
    assert_eq!(
        e3_parent.as_deref(),
        Some(e2_hash.as_str()),
        "intent_received(body).parent_hash must equal intent_received(subject).hash"
    );

    assert_eq!(e4_type, "fd_granted", "event[4] must be fd_granted");
    assert_eq!(
        e4_parent.as_deref(),
        Some(e3_hash.as_str()),
        "fd_granted.parent_hash must equal intent_received(body).hash"
    );

    assert_eq!(
        e5_type, "plan_node_evaluated",
        "event[5] must be plan_node_evaluated (Phase 15 / 15-04 — the benign send \
         now always submits an all-UserTrusted plan node, which the executor Allows)"
    );
    assert_eq!(
        e5_parent.as_deref(),
        Some(e4_hash.as_str()),
        "plan_node_evaluated.parent_hash must equal fd_granted.hash \
         (no file_read event intervenes — zero doc-fragment claims for this benign content)"
    );

    // ── Cleanup ──────────────────────────────────────────────────────────────
    std::fs::remove_dir_all(&tmp).ok();
}
