//! Committed regression oracle for CR-01 grant/audit atomicity. This test must
//! never be hidden behind a target-OS or feature gate: the grant path is
//! host-portable and involves no confined worker, Landlock, or seccomp. The
//! writer, never `has_github_grant` or `verify_chain`, may change to make it pass.

use brokerd::audit::{
    has_github_grant, open_audit_db, record_github_grant, verify_chain, SCHEMA_DDL,
};
use uuid::Uuid;

/// Fixed, non-secret 32-byte test MAC key.
const TEST_KEY: &[u8] = b"caprun-grant-atomicity-test-key-01!";

fn grant_row_count(conn: &rusqlite::Connection, session_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM session_grants WHERE session_id = ?1",
        rusqlite::params![session_id],
        |row| row.get(0),
    )
    .expect("count session grant rows")
}

fn count_events(conn: &rusqlite::Connection, session_id: &str, event_type: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2",
        rusqlite::params![session_id, event_type],
        |row| row.get(0),
    )
    .expect("count session events by type")
}

#[test]
fn append_failure_after_grant_insert_rolls_back_and_retry_yields_exactly_one() {
    let conn = open_audit_db(":memory:").expect("open in-memory audit database");
    let session_id = Uuid::new_v4().to_string();

    conn.execute_batch("DROP TABLE events")
        .expect("inject deterministic append failure");
    let result = record_github_grant(&conn, TEST_KEY, &session_id);
    assert!(
        result.is_err(),
        "an append failure must propagate rather than be swallowed"
    );

    let observed_grants = grant_row_count(&conn, &session_id);
    assert_eq!(
        observed_grants, 0,
        "observed {observed_grants} surviving session_grants rows after append failure"
    );
    assert!(
        !has_github_grant(&conn, &session_id),
        "failed append must not leave the grant active"
    );

    conn.execute_batch(SCHEMA_DDL)
        .expect("repair idempotent audit schema");
    record_github_grant(&conn, TEST_KEY, &session_id).expect("retry github grant");

    let observed_grants = grant_row_count(&conn, &session_id);
    assert_eq!(observed_grants, 1, "observed {observed_grants} grant rows");
    let observed_events = count_events(&conn, &session_id, "github_grant_authorized");
    assert_eq!(
        observed_events, 1,
        "retry must yield exactly one authorization event; observed {observed_events}"
    );
    assert!(
        has_github_grant(&conn, &session_id),
        "successful retry must activate the grant"
    );
    assert!(
        verify_chain(&conn, &session_id, TEST_KEY),
        "root authorization event chain must verify"
    );
}

/// This guard passes before and after the fix. It pins replay suppression across
/// the transaction change; it is not the fault-injection oracle.
#[test]
fn fresh_grant_and_replay_yield_exactly_one_row_and_one_authorization_event() {
    let conn = open_audit_db(":memory:").expect("open in-memory audit database");
    let session_id = Uuid::new_v4().to_string();

    record_github_grant(&conn, TEST_KEY, &session_id).expect("record fresh github grant");
    assert_eq!(grant_row_count(&conn, &session_id), 1);
    assert_eq!(
        count_events(&conn, &session_id, "github_grant_authorized"),
        1
    );
    assert!(verify_chain(&conn, &session_id, TEST_KEY));

    record_github_grant(&conn, TEST_KEY, &session_id).expect("replay github grant");
    assert_eq!(grant_row_count(&conn, &session_id), 1);
    assert_eq!(
        count_events(&conn, &session_id, "github_grant_authorized"),
        1
    );
    assert!(verify_chain(&conn, &session_id, TEST_KEY));
}
