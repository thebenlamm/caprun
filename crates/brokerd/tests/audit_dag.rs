/// audit_dag — SQLite audit DAG hash-chain verification tests
///
/// Verifies that the audit DAG maintains a valid SHA-256 hash chain across
/// appended events, and that any row mutation breaks the chain (tamper-evidence).
///
/// These tests run on macOS and Linux (rusqlite bundled — no system SQLite).

use brokerd::audit::{append_event, open_audit_db, verify_chain};
use chrono::Utc;
use runtime_core::{Event, TaintLabel};
use uuid::Uuid;

/// Append three events (session_created → fd_granted → file_read) and assert
/// verify_chain returns true, parent_hash links are correct, and the chain is
/// contiguous and unbroken.
#[test]
fn audit_hash_chain() {
    let conn = open_audit_db(":memory:").expect("open_audit_db failed");
    let session_id = Uuid::new_v4();

    let e1 = Event {
        id: Uuid::new_v4(),
        parent_id: None,
        session_id,
        actor: "broker".into(),
        event_type: "session_created".into(),
        timestamp: Utc::now(),
        taint: vec![],
    };

    let e2 = Event {
        id: Uuid::new_v4(),
        parent_id: Some(e1.id),
        session_id,
        actor: "broker".into(),
        event_type: "fd_granted".into(),
        timestamp: Utc::now(),
        taint: vec![],
    };

    let e3 = Event {
        id: Uuid::new_v4(),
        parent_id: Some(e2.id),
        session_id,
        actor: "worker".into(),
        event_type: "file_read".into(),
        timestamp: Utc::now(),
        taint: vec![TaintLabel::LocalWorkspace],
    };

    let h1 = append_event(&conn, &e1, None).expect("append e1 failed");
    let h2 = append_event(&conn, &e2, Some(&h1)).expect("append e2 failed");
    let _h3 = append_event(&conn, &e3, Some(&h2)).expect("append e3 failed");

    assert!(
        verify_chain(&conn, &session_id.to_string()),
        "chain should be unbroken after 3 events"
    );

    // Verify e2's stored parent_hash equals h1 (parent_hash linkage)
    let stored_parent_hash: Option<String> = conn
        .query_row(
            "SELECT parent_hash FROM events WHERE id = ?1",
            rusqlite::params![e2.id.to_string()],
            |row| row.get(0),
        )
        .expect("query e2 parent_hash failed");
    assert_eq!(
        stored_parent_hash,
        Some(h1),
        "e2.parent_hash must equal the hash of e1"
    );

    // Root event has no parent_hash
    let root_parent_hash: Option<String> = conn
        .query_row(
            "SELECT parent_hash FROM events WHERE id = ?1",
            rusqlite::params![e1.id.to_string()],
            |row| row.get(0),
        )
        .expect("query e1 parent_hash failed");
    assert!(
        root_parent_hash.is_none(),
        "root event parent_hash must be NULL"
    );
}

/// Mutating a stored event's payload (via raw SQL UPDATE, test-only) must make
/// verify_chain return false — proving tamper-evidence.
///
/// Note: production brokerd code NEVER issues UPDATE or DELETE on the events
/// table. This test uses a raw UPDATE to simulate an out-of-band tamper.
#[test]
fn tamper_breaks_chain() {
    let conn = open_audit_db(":memory:").expect("open_audit_db failed");
    let session_id = Uuid::new_v4();

    let e1 = Event {
        id: Uuid::new_v4(),
        parent_id: None,
        session_id,
        actor: "broker".into(),
        event_type: "session_created".into(),
        timestamp: Utc::now(),
        taint: vec![],
    };

    let _ = append_event(&conn, &e1, None).expect("append e1 failed");

    // Tamper: directly mutate the payload via raw SQL (out-of-band, test-only)
    conn.execute(
        "UPDATE events SET payload = '{\"tampered\":true}' WHERE id = ?1",
        rusqlite::params![e1.id.to_string()],
    )
    .expect("tamper UPDATE failed");

    assert!(
        !verify_chain(&conn, &session_id.to_string()),
        "verify_chain must return false after payload tampering"
    );
}
