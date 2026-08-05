//! Committed RED oracle for D1: an out-of-band audit append must not fork a
//! broker connection's causal chain. This test must never be hidden behind a
//! target-OS or feature cfg. `verify_chain` is correct by design; the append
//! path, never the verifier, is what may change to make this test pass.

use brokerd::audit::{append_event, open_audit_db, record_github_grant, verify_chain};
use chrono::Utc;
use runtime_core::Event;
use uuid::Uuid;

/// Fixed, non-secret 32-byte test MAC key.
const TEST_KEY: &[u8] = b"caprun-chain-fork-test-key-0001!";

fn unique_audit_db(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("caprun_chainfork_{tag}_{}.db", Uuid::new_v4()))
}

fn leaf_count(conn: &rusqlite::Connection, session_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM events \
         WHERE session_id = ?1 AND id NOT IN ( \
             SELECT parent_id FROM events \
             WHERE session_id = ?1 AND parent_id IS NOT NULL \
         )",
        rusqlite::params![session_id],
        |row| row.get(0),
    )
    .expect("count chain leaves")
}

fn seed_session_root(conn: &rusqlite::Connection, session_id: Uuid) -> (Uuid, String) {
    let event = Event::new(
        Uuid::new_v4(),
        None,
        session_id,
        "broker".into(),
        "session_created".into(),
        Utc::now(),
        vec![],
    );
    let hash = append_event(conn, TEST_KEY, &event, None).expect("append session root");
    (event.id, hash)
}

#[test]
fn broker_append_after_external_grant_must_not_fork_chain() {
    let db_path = unique_audit_db("external_grant");
    let db_path_string = db_path.to_string_lossy().into_owned();
    let broker = open_audit_db(&db_path_string).expect("open broker audit connection");
    let session_id = Uuid::new_v4();
    let session_id_string = session_id.to_string();
    let (mut remembered_id, mut remembered_hash) = seed_session_root(&broker, session_id);

    let evaluated = Event::new(
        Uuid::new_v4(),
        Some(remembered_id),
        session_id,
        "broker".into(),
        "plan_node_evaluated".into(),
        Utc::now(),
        vec![],
    );
    remembered_hash = append_event(&broker, TEST_KEY, &evaluated, Some(&remembered_hash))
        .expect("append first broker event");
    remembered_id = evaluated.id;

    let external = open_audit_db(&db_path_string).expect("open external audit connection");
    record_github_grant(&external, TEST_KEY, &session_id_string)
        .expect("append out-of-band github grant");

    let resumed = Event::new(
        Uuid::new_v4(),
        Some(remembered_id),
        session_id,
        "broker".into(),
        "plan_node_evaluated".into(),
        Utc::now(),
        vec![],
    );
    append_event(&broker, TEST_KEY, &resumed, Some(&remembered_hash))
        .expect("append resumed broker event");

    let observed_leaves = leaf_count(&broker, &session_id_string);
    assert_eq!(
        observed_leaves, 1,
        "observed leaf count {observed_leaves}; remembered parent {remembered_id} acquired two children"
    );
    assert!(
        verify_chain(&broker, &session_id_string, TEST_KEY),
        "audit chain must verify after an external grant append"
    );

    drop(external);
    drop(broker);
    std::fs::remove_file(db_path).expect("remove successful test database");
}
