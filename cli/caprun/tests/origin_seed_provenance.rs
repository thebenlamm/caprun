//! origin_seed_provenance — ORIGIN-01/02: the CLI decides seed-provenance, the
//! broker (create_session) sets the resulting Draft/Active status.
//!
//! These assertions run the REAL `caprun` binary (`env!("CARGO_BIN_EXE_caprun")`,
//! same pattern as `e2e.rs` / `s9_live_block.rs`) but stop short of requiring the
//! full confined worker to succeed: `create_session` + `persist_session` +
//! the `session_created` event append all happen in `main()` BEFORE the broker
//! task binds its (Linux-only) abstract socket and BEFORE the worker is spawned
//! (DESIGN §3). So the `sessions` row's `status` column is durable and assertable
//! from any process exit status, on any platform — this is the macOS-runnable
//! slice of ORIGIN-01/02 this test proves; the full live confined run (worker
//! spawn + Landlock/seccomp) remains Linux-only per the existing e2e convention.

use runtime_core::SessionStatus;

/// Spawn the real `caprun` binary with the given args, returning
/// `(exit_success, stdout, stderr)`. Never panics on a non-zero exit — several
/// of these runs are EXPECTED to fail past the session-creation point (no live
/// Linux confinement stack on the dev/CI box), which is fine: session creation
/// happens before any of that.
fn run_caprun(args: &[&str]) -> (bool, String, String) {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = std::process::Command::new(caprun_bin)
        .args(args)
        .output()
        .expect("spawn caprun");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Read the sole `sessions` row's `status` column from the audit DB at `path`
/// and deserialize it back into a `SessionStatus` (persist_session encodes it
/// via `serde_json::to_string`, e.g. the literal text `"Draft"`).
fn read_session_status(audit_db_path: &std::path::Path) -> SessionStatus {
    let conn = brokerd::audit::open_audit_db(audit_db_path.to_str().unwrap())
        .expect("open audit DB");
    let status_json: String = conn
        .query_row("SELECT status FROM sessions LIMIT 1", [], |row| row.get(0))
        .expect("exactly one sessions row must exist");
    serde_json::from_str(&status_json).expect("status column must deserialize to SessionStatus")
}

fn setup_tmp(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let run_id = uuid::Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_origin_seed_{tag}_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let workspace_file = tmp.join("workspace.txt");
    let audit_db_path = tmp.join("audit.db");
    std::fs::write(&workspace_file, b"benign workspace content, no path tokens")
        .expect("write workspace file");
    (tmp, workspace_file, audit_db_path)
}

/// ORIGIN-02: a `--seed-from-file` run starts the session `Draft` — the I0
/// creation rule. Runs on macOS: session_created/persist_session complete
/// before any Linux-only confinement is touched.
#[test]
fn file_derived_seed_starts_draft() {
    let (tmp, workspace_file, audit_db_path) = setup_tmp("file_derived");
    let seed_file = tmp.join("seed.txt");
    std::fs::write(&seed_file, "recipient@example.test").expect("write seed file");

    let (_success, stdout, stderr) = run_caprun(&[
        "--seed-from-file",
        seed_file.to_str().unwrap(),
        "send-email-summary",
        workspace_file.to_str().unwrap(),
        audit_db_path.to_str().unwrap(),
    ]);
    eprintln!("caprun stdout:\n{stdout}\ncaprun stderr:\n{stderr}");

    assert_eq!(
        read_session_status(&audit_db_path),
        SessionStatus::Draft,
        "a --seed-from-file session MUST start Draft (ORIGIN-02, I0 creation rule)"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Regression guard: without `--seed-from-file`, today's trusted-arg behavior
/// is unchanged — the session starts `Active`.
#[test]
fn trusted_arg_seed_starts_active() {
    let (tmp, workspace_file, audit_db_path) = setup_tmp("trusted_arg");

    let (_success, stdout, stderr) = run_caprun(&[
        "send-email-summary",
        "recipient@example.test",
        workspace_file.to_str().unwrap(),
        audit_db_path.to_str().unwrap(),
    ]);
    eprintln!("caprun stdout:\n{stdout}\ncaprun stderr:\n{stderr}");

    assert_eq!(
        read_session_status(&audit_db_path),
        SessionStatus::Active,
        "a trusted-arg session must start Active (today's existing behavior, unchanged)"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// V5 fail-closed: a `--seed-from-file` path that does not exist is a hard
/// error — caprun MUST exit non-zero and MUST NOT create any session row (never
/// a silent fallback to trusted-arg / Active).
#[test]
fn missing_seed_file_fails_closed() {
    let (tmp, workspace_file, audit_db_path) = setup_tmp("missing_seed");
    let missing_seed = tmp.join("does-not-exist.txt");
    assert!(!missing_seed.exists(), "sanity: seed path must not exist");

    let (success, stdout, stderr) = run_caprun(&[
        "--seed-from-file",
        missing_seed.to_str().unwrap(),
        "send-email-summary",
        workspace_file.to_str().unwrap(),
        audit_db_path.to_str().unwrap(),
    ]);
    eprintln!("caprun stdout:\n{stdout}\ncaprun stderr:\n{stderr}");

    assert!(
        !success,
        "caprun MUST exit non-zero when --seed-from-file points at a missing path"
    );
    assert!(
        stderr.contains("--seed-from-file"),
        "the fail-closed error must name --seed-from-file; got stderr: {stderr}"
    );

    // Fail-closed proof: the missing-file error happens before `open_audit_db`
    // is ever called in main() (before the audit-db-path arg is even parsed),
    // so no sessions row — let alone an Active one — was ever created. Opening
    // it here (fresh, since the file never existed) must find zero rows, never
    // a silent trusted-arg Active session.
    let conn = brokerd::audit::open_audit_db(audit_db_path.to_str().unwrap())
        .expect("open (fresh) audit DB");
    let session_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("query sessions count");
    assert_eq!(
        session_count, 0,
        "no session row may exist after a fail-closed missing-seed-file error"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
