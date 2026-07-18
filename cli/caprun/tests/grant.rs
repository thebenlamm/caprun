/// grant — cross-process integration test for the `caprun grant` session
/// auth-grant verb (v1.8 Phase 38, GITHUB-02, DESIGN §4.3, FORK 3).
///
/// `caprun grant <session_id> [audit-db-path]` is a DISTINCT human action from
/// confirm/deny: it records the durable, session-scoped github.pr auth-grant
/// that both github.pr dispatch paths (Plans 38-04/38-05) gate on. These tests
/// spawn the REAL compiled `caprun` binary as a separate OS process against a
/// PERSISTENT SQLite audit DB, then reopen the DB via brokerd's public API to
/// assert the grant landed — mirroring `tests/confirm.rs`'s cross-process
/// discipline. None of these tests are `#[cfg(target_os = "linux")]`-gated
/// (the grant path is host-portable — no confined worker, no Landlock/seccomp).
use brokerd::audit::{has_github_grant, open_audit_db};
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

/// Run `caprun grant <session_id> <db_path>` as a REAL separate OS process.
/// Returns the exit code.
fn run_caprun_grant(session_id: &str, db_path: &Path) -> i32 {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = Command::new(caprun_bin)
        .arg("grant")
        .arg(session_id)
        .arg(db_path.to_str().unwrap())
        .output()
        .expect("spawn caprun grant");
    output.status.code().expect("process must exit with a code")
}

/// GITHUB-02: `caprun grant <uuid> <db>` records a durable, session-scoped
/// grant (has_github_grant true afterward) and exits 0. Session-scoping is
/// asserted directly: an unrelated session is NOT granted by the same call.
#[test]
fn grant_records_session_scoped_capability_and_exits_0() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_grant_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let db_path = tmp.join("audit.db");
    // Create the schema (persistent DB on disk) before the grant subprocess.
    {
        open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db (seed schema)");
    }

    let session_id = Uuid::new_v4();
    let code = run_caprun_grant(&session_id.to_string(), &db_path);
    assert_eq!(code, 0, "caprun grant on a valid session id must exit 0");

    let conn = open_audit_db(db_path.to_str().unwrap()).expect("reopen persisted audit DB");
    assert!(
        has_github_grant(&conn, &session_id.to_string()),
        "has_github_grant must be true for the granted session after caprun grant"
    );
    assert!(
        !has_github_grant(&conn, &Uuid::new_v4().to_string()),
        "the grant must be session-scoped — an unrelated session stays ungranted"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Fail-closed: a malformed (non-UUID) session id exits non-zero and records
/// nothing (never a silent pass-through into record_github_grant).
#[test]
fn grant_with_malformed_session_id_exits_nonzero() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_grant_bad_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let db_path = tmp.join("audit.db");
    {
        open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db (seed schema)");
    }

    let code = run_caprun_grant("not-a-uuid", &db_path);
    assert_ne!(
        code, 0,
        "a malformed session id must fail closed with a non-zero exit"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
