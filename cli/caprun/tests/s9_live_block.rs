//! s9_live_block — live §9 allow-path proof, end to end (Linux-gated)
//!
//! Phase 6 live complement to the in-process `s9_acceptance.rs` gate. Runs the
//! REAL `caprun` binary with the new intent-first CLI:
//!
//!   caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]
//!
//! and proves, through the resulting SQLite audit DB, that the CLEAN ALLOW-PATH
//! works end to end (SC1 + SC4):
//!
//!   * caprun exits 0 (success) — the UserTrusted intent recipient is ALLOWED.
//!   * a durable `intent_received` event exists (PLAN-04: mint_from_intent anchor).
//!   * a durable `plan_node_evaluated` event exists (not `sink_blocked`) — the
//!     executor returned Allowed for the UserTrusted `email.send / to` argument.
//!   * no `sink_blocked` event — HARD-02: is_untrusted() returns false for UserTrusted.
//!   * the `intent_received` event carries empty taint (the audit DAG node has
//!     taint: []; the UserTrusted label lives on the ValueRecord in the ValueStore).
//!
//! Note on workspace content: the file contains an email address so the worker
//! reaches the plan-submission step (the `value_ids.is_empty()` early-exit guard
//! requires at least one claim to proceed). Critically, `plan_from_intent` routes
//! `intent_value_id` (UserTrusted, from the CLI arg) — NOT the ExternalUntrusted
//! file-extracted handle — to the `email.send / to` argument. The executor sees
//! only the UserTrusted value and returns Allowed. The presence of a file-extracted
//! email does not influence the decision; it only triggers the plan-node submission
//! code path. This is what "clean allow-path" means: the DECISION outcome is Allowed,
//! not the absence of any extractable email claim.
//!
//! The live hostile block (ExternalUntrusted → sink_blocked) is no longer reachable
//! from the intent-driven CLI in Phase 6: `plan_from_intent` always routes
//! `intent_value_id` to `email.send / to`. The live hostile-block proof moves to
//! Phase 7 (file.create path). The in-process hostile proof remains intact in
//! `crates/brokerd/tests/s9_acceptance.rs`.
//!
//! The live assertions are `#[cfg(target_os = "linux")]` because the confinement
//! stack (abstract UDS + Landlock + seccomp) is Linux-only. On macOS the bodies
//! are cfg-excluded and only the cross-platform guard test runs. Run the live
//! assertions under Colima/Docker:
//!
//!   docker run --rm --security-opt seccomp=unconfined \
//!     -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
//!     rust:1 cargo test -p caprun --test s9_live_block

/// Workspace content for the clean allow-path test. Contains an email address so
/// the worker reaches the plan-node submission step. The planner routes the
/// UserTrusted `intent_value_id` (CLI arg) to `email.send / to` — NOT this
/// file-extracted ExternalUntrusted value — so the executor returns Allowed.
#[cfg(target_os = "linux")]
const CLEAN_PATH_CONTENT: &[u8] =
    b"Q3 financial summary prepared. Route distribution to reports@internal.example as instructed.";

/// Run the real caprun binary with the new intent-first positional-arg signature.
/// Returns `(exit_success, audit_db_path)`.
#[cfg(target_os = "linux")]
fn run_caprun_intent_on(
    intent_kind: &str,
    intent_param: &str,
    content: &[u8],
    tag: &str,
) -> (bool, std::path::PathBuf) {
    let run_id = uuid::Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_s9_{tag}_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let workspace_file = tmp.join("workspace.txt");
    let audit_db_path = tmp.join("audit.db");
    std::fs::write(&workspace_file, content).expect("write workspace file");

    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = std::process::Command::new(caprun_bin)
        .arg(intent_kind)
        .arg(intent_param)
        .arg(workspace_file.to_str().unwrap())
        .arg(audit_db_path.to_str().unwrap())
        .output()
        .expect("spawn caprun");

    eprintln!("caprun stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    eprintln!("caprun stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    (output.status.success(), audit_db_path)
}

/// SC1 / SC4: a real caprun run on the clean allow-path exits 0, with a durable
/// intent_received → plan_node_evaluated audit chain and no sink_blocked event.
///
/// Proves that HARD-02 (UserTrusted → Allowed) + PLAN-04 (mint_from_intent) +
/// PLAN-01 (intent CLI) compose into a reachable allow-path through the real
/// confined-worker + broker + executor stack.
#[cfg(target_os = "linux")]
#[test]
fn s9_live_clean_allow_path() {
    use brokerd::audit::{find_event_by_type, open_audit_db};

    let (success, audit_db) = run_caprun_intent_on(
        "send-email-summary",
        "analyst@example.test",
        CLEAN_PATH_CONTENT,
        "clean",
    );
    assert!(
        success,
        "caprun MUST exit 0 on the clean allow-path — UserTrusted recipient must be allowed"
    );

    let conn = open_audit_db(audit_db.to_str().unwrap()).expect("open audit DB");
    let session_id: String = conn
        .query_row("SELECT id FROM sessions LIMIT 1", [], |row| row.get(0))
        .expect("one session row must exist");

    // (a) PLAN-04: intent_received event anchors the UserTrusted value in the DAG.
    let intent_received = find_event_by_type(&conn, &session_id, "intent_received")
        .expect("query intent_received")
        .expect("an intent_received event must exist for any intent-CLI run");

    // (b) HARD-02: the intent_received event carries NO taint — the UserTrusted
    //     label lives on the ValueRecord in the ValueStore, not on the DAG node.
    assert!(
        intent_received.taint.is_empty(),
        "intent_received event must carry empty taint (UserTrusted label is on the ValueRecord, not the DAG event)"
    );

    // (c) SC4 / HARD-02: executor returned Allowed — a plan_node_evaluated event
    //     (not sink_blocked) must exist. plan_from_intent routed intent_value_id
    //     (UserTrusted) to email.send / to; is_untrusted() returns false for UserTrusted.
    let plan_node_evaluated = find_event_by_type(&conn, &session_id, "plan_node_evaluated")
        .expect("query plan_node_evaluated")
        .expect("a plan_node_evaluated event must exist — UserTrusted recipient must be allowed, not blocked");

    // (d) The Allowed decision implies no sink_blocked event was recorded.
    let sink_blocked = find_event_by_type(&conn, &session_id, "sink_blocked")
        .expect("query sink_blocked");
    assert!(
        sink_blocked.is_none(),
        "no sink_blocked event may exist on the clean allow-path (plan_node_evaluated id={})",
        plan_node_evaluated.id
    );
}

/// Cross-platform guard: this always-compiled test keeps `cargo test -p caprun`
/// meaningful on the macOS dev box (where the live bodies above are cfg-excluded).
/// It proves the caprun binary is wired into the test build; the real live
/// assertions run under Colima/Docker on Linux (see the module header command).
#[test]
fn s9_live_block_guard_binary_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(
        !caprun_bin.is_empty(),
        "CARGO_BIN_EXE_caprun must resolve — caprun must be built for the live test"
    );
}
