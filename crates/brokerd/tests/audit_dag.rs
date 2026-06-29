/// audit_dag — SQLite audit DAG hash-chain verification tests
///
/// Tests that the audit DAG maintains a valid SHA-256 hash chain across
/// appended events. Wave 2 Plan 03 implements the test bodies.

#[test]
#[ignore]
fn hash_chain_is_unbroken() {
    // TODO Wave 2 Plan 03: open an in-memory rusqlite DB, append 3 events,
    // verify each row's hash is SHA-256(parent_hash || id || ... fields).
    assert!(true); // placeholder — Wave 2
}

#[test]
#[ignore]
fn session_created_event_has_null_parent() {
    // TODO Wave 2 Plan 03: the root event of a session has parent_id=NULL
    // and parent_hash=NULL; hash is computed over empty-string parent_hash.
    assert!(true); // placeholder — Wave 2
}
