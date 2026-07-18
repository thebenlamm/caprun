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
    // F1-safe layout: workspace file under its own subdirectory, audit.db a
    // sibling of that subdirectory (never a direct child of the workspace root).
    let ws_dir = tmp.join("workspace");
    std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
    let workspace_file = ws_dir.join("workspace.txt");
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
/// session_created → policy_bound → intent_received(recipient) →
/// intent_received(subject) → intent_received(body) → fd_granted →
/// plan_node_evaluated → email_send_attempted → email_send_succeeded (the
/// 9-event benign chain, UPDATED Phase 42 POLICY-03 which inserts policy_bound
/// after session_created; prior UPDATE Phase 16 / 16-04, CONTROL-01/BLOCKER-3 —
/// see BLOCKER note below).
/// EMPIRICALLY VERIFIED under Colima/Docker (Linux) at 15-04 time for the
/// first 6 events — this is not a Mac-side inference. The trailing two
/// events are new as of Phase 16's email.send Allowed-dispatch and require
/// `scripts/mailpit-verify.sh` (a live Mailpit listener) to observe
/// email_send_succeeded rather than email_send_failed.
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
/// THREE `intent_received` events, not one.
///
/// FURTHER (Phase 16 / 16-04, CONTROL-01/BLOCKER-3): the benign send here is
/// an all-UserTrusted `SendEmailSummary` — an Allowed decision — which now
/// reaches the new email.send Allowed-dispatch branch in `server.rs`. That
/// branch appends a durable `email_send_attempted` event (MAJOR-4) BEFORE
/// invoking the real SMTP adapter, then (under a live Mailpit listener, i.e.
/// `scripts/mailpit-verify.sh`) an `email_send_succeeded` event.
///
/// FINALLY (Phase 42 / POLICY-03): `caprun run` now records the bound session
/// policy's identity as a genuine `policy_bound` audit event, hash-chained onto
/// `session_created` and installed as the broker's seed chain head — so the
/// first broker event (`intent_received` recipient) parents onto `policy_bound`,
/// not `session_created`. The full benign chain is now session_created →
/// policy_bound → intent_received(recipient) → intent_received(subject) →
/// intent_received(body) → fd_granted → plan_node_evaluated →
/// email_send_attempted → email_send_succeeded (9 events total). The §9 BLOCK
/// path is exercised by
/// `crates/brokerd/tests/s9_acceptance.rs` and (live) by
/// `s9_live_block.rs::s9_live_email_hostile_block`.
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
    // F1-safe layout: workspace file under its own subdirectory, audit.db a
    // sibling of that subdirectory (never a direct child of the workspace root).
    let ws_dir = tmp.join("workspace");
    std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
    let workspace_file = ws_dir.join("workspace.txt");
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
    // v1.6 Phase 28 (HARDEN-02): read back the SAME broker-owned MAC key the
    // `caprun run` subprocess persisted via `key::load_or_create_key` at
    // `<audit_db_path>.key` — `verify_chain` is now keyed, so a fresh/
    // unrelated key would spuriously fail this assertion.
    let mac_key = std::fs::read(format!("{}.key", audit_db_path.display()))
        .expect("read persisted MAC key file written by the caprun run subprocess");
    assert!(
        verify_chain(&conn, &session_id, &mac_key),
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
        9,
        "audit DAG must contain exactly 9 events (session_created, policy_bound \
         — Phase 42 POLICY-03, chained onto session_created as the broker's seed \
         head — THREE intent_received — recipient/subject/body, Phase 15 finding \
         #6 — fd_granted, plan_node_evaluated, email_send_attempted, \
         email_send_succeeded — Phase 16 CONTROL-01); got {}: {:?}",
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
    let (e5_type, e5_parent, e5_hash) = &events[5];
    let (e6_type, e6_parent, e6_hash) = &events[6];
    let (e7_type, e7_parent, e7_hash) = &events[7];
    let (e8_type, e8_parent, _e8_hash) = &events[8];

    assert_eq!(e0_type, "session_created", "event[0] must be session_created");
    assert!(
        e0_parent.is_none(),
        "session_created must have no parent_hash; got {e0_parent:?}"
    );

    // Phase 42 (POLICY-03): the bound session policy's identity is recorded as a
    // genuine `policy_bound` audit event, chained onto session_created and made
    // the broker's seed chain head — so the first broker event (intent_received)
    // now parents onto policy_bound, NOT session_created.
    assert_eq!(e1_type, "policy_bound", "event[1] must be policy_bound (Phase 42 POLICY-03)");
    assert_eq!(
        e1_parent.as_deref(),
        Some(e0_hash.as_str()),
        "policy_bound.parent_hash must equal session_created.hash"
    );

    assert_eq!(e2_type, "intent_received", "event[2] must be intent_received (recipient)");
    assert_eq!(
        e2_parent.as_deref(),
        Some(e1_hash.as_str()),
        "intent_received(recipient).parent_hash must equal policy_bound.hash"
    );

    assert_eq!(e3_type, "intent_received", "event[3] must be intent_received (subject)");
    assert_eq!(
        e3_parent.as_deref(),
        Some(e2_hash.as_str()),
        "intent_received(subject).parent_hash must equal intent_received(recipient).hash"
    );

    assert_eq!(e4_type, "intent_received", "event[4] must be intent_received (body)");
    assert_eq!(
        e4_parent.as_deref(),
        Some(e3_hash.as_str()),
        "intent_received(body).parent_hash must equal intent_received(subject).hash"
    );

    assert_eq!(e5_type, "fd_granted", "event[5] must be fd_granted");
    assert_eq!(
        e5_parent.as_deref(),
        Some(e4_hash.as_str()),
        "fd_granted.parent_hash must equal intent_received(body).hash"
    );

    assert_eq!(
        e6_type, "plan_node_evaluated",
        "event[6] must be plan_node_evaluated (Phase 15 / 15-04 — the benign send \
         now always submits an all-UserTrusted plan node, which the executor Allows)"
    );
    assert_eq!(
        e6_parent.as_deref(),
        Some(e5_hash.as_str()),
        "plan_node_evaluated.parent_hash must equal fd_granted.hash \
         (no file_read event intervenes — zero doc-fragment claims for this benign content)"
    );

    // Phase 16 (CONTROL-01/BLOCKER-3): the Allowed decision now reaches the
    // email.send Allowed-dispatch branch, which appends email_send_attempted
    // (MAJOR-4) BEFORE invoking the adapter, then — under a live Mailpit
    // listener (scripts/mailpit-verify.sh) — email_send_succeeded.
    assert_eq!(
        e7_type, "email_send_attempted",
        "event[7] must be email_send_attempted (MAJOR-4 durable attempt ledger, \
         appended BEFORE the SMTP socket ever opens)"
    );
    assert_eq!(
        e7_parent.as_deref(),
        Some(e6_hash.as_str()),
        "email_send_attempted.parent_hash must equal plan_node_evaluated.hash"
    );

    assert_eq!(
        e8_type, "email_send_succeeded",
        "event[8] must be email_send_succeeded — this test MUST run under \
         scripts/mailpit-verify.sh (a live Mailpit listener); under the bare \
         rust:1 recipe (no listener) this would instead be email_send_failed"
    );
    assert_eq!(
        e8_parent.as_deref(),
        Some(e7_hash.as_str()),
        "email_send_succeeded.parent_hash must equal email_send_attempted.hash"
    );

    // ── Cleanup ──────────────────────────────────────────────────────────────
    std::fs::remove_dir_all(&tmp).ok();
}
