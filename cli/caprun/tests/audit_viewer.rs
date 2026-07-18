/// audit_viewer — integration tests for the READ-ONLY `caprun audit` verb
/// (VIEW-01 / U1, Plan 45-03).
///
/// These drive the REAL compiled `caprun` binary as a separate OS process
/// against a PERSISTENT SQLite audit DB seeded directly via brokerd's public
/// `append_event` API (mirroring `tests/confirm.rs`'s seeding convention) — a
/// genuine keyed hash chain, no confined worker required. None of these legs
/// need a Linux worker to produce a block: the viewer is a pure read, so the
/// fixture chain can be built host-side on macOS, and ALL four legs are
/// host-portable (not `#[cfg(target_os = "linux")]`-gated).
///
/// Legs (Plan 45-03 Task 3 <done>):
///   1. RENDER — a genuine keyed chain renders events + `Chain verification:
///      PASSED`, exit 0.
///   2. FAIL-CLOSED-ON-ABSENT-KEY — with the `<db>.key` sibling removed, the
///      viewer hard-errors (non-zero exit) and prints NO `Chain verification:`
///      verdict (never PASSED/FAILED against a fresh key) — U1 M2.
///   3. MEMORY-REFUSED — `caprun audit <session> :memory:` exits non-zero with
///      no verdict — U1 M2.
///   4. TAINTED-LITERAL-NEUTRALIZED — an event whose `actor` embeds a `\x1b[2K`
///      (ESC CSI) sequence renders with the ESC escaped to a visible `\x1b`
///      form, with NO raw ESC byte in stdout — U1 M3 (WG-2).
use brokerd::audit::{append_event, open_audit_db};
use chrono::Utc;
use runtime_core::Event;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

/// Test-local mirror of `cli/caprun/src/key.rs`'s cross-process custody (that
/// module is `pub(crate)` inside the bin-only `caprun` crate, so this external
/// integration-test crate cannot import it). Writes the MAC key at
/// `<db_path>.key` BEFORE the seeding `append_event` calls — the SAME bytes the
/// `caprun audit` subprocess's OWN `key::load_existing_key` reads back. The
/// layout is F1-safe by construction (`audit.db` and any workspace dir are
/// SIBLINGS under a unique tmp dir, never nested).
fn seed_test_key(db_path: &Path) -> Vec<u8> {
    let key_path = PathBuf::from(format!("{}.key", db_path.to_str().unwrap()));
    if let Ok(bytes) = std::fs::read(&key_path) {
        return bytes;
    }
    // Uniqueness (not cryptographic strength) is all this test-local key needs:
    // every test uses its own fresh `run_id`-suffixed tmp dir.
    let mut key = Uuid::new_v4().as_bytes().to_vec();
    key.extend_from_slice(Uuid::new_v4().as_bytes());
    std::fs::write(&key_path, &key).expect("write test MAC key file");
    key
}

/// Seed a genuine two-event keyed chain (a `session_created` root + one child
/// event whose `actor` is `child_actor`) against the persistent DB at
/// `db_path`, and return the `session_id`. `verify_chain` verifies TRUE against
/// the seeded `.key`. The seeding connection drops within this function, fully
/// releasing (and checkpointing) the persistent file before any `caprun audit`
/// subprocess opens its own READ-ONLY connection.
fn seed_chain(db_path: &Path, key: &[u8], child_actor: &str) -> Uuid {
    let conn = open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db for seeding");

    let session_id = Uuid::new_v4();

    let root = Event::new(
        Uuid::new_v4(),
        None,
        session_id,
        "broker".into(),
        "session_created".into(),
        Utc::now(),
        vec![],
    );
    let root_hash = append_event(&conn, key, &root, None).expect("append session_created");

    let child = Event::new(
        Uuid::new_v4(),
        Some(root.id),
        session_id,
        child_actor.into(),
        "file_read".into(),
        Utc::now(),
        vec![],
    );
    append_event(&conn, key, &child, Some(&root_hash)).expect("append child event");

    // `conn` drops here — the persistent file is released + WAL checkpointed
    // before the read-only `caprun audit` open.
    session_id
}

/// Run `caprun audit <session_id> <audit-db-path>` as a REAL separate OS
/// process. Returns `(exit_code, stdout_bytes, stderr_string)`. stdout is
/// returned as raw bytes so the neutralization test can assert on the ABSENCE
/// of a raw ESC (0x1b) byte.
fn run_caprun_audit(session_id: &str, audit_path: &str) -> (i32, Vec<u8>, String) {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = Command::new(caprun_bin)
        .arg("audit")
        .arg(session_id)
        .arg(audit_path)
        .output()
        .unwrap_or_else(|e| panic!("spawn caprun audit: {e}"));
    (
        output.status.code().expect("process must exit with a code"),
        output.stdout,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Leg 1 — RENDER: a genuine keyed chain renders the events + a
/// `Chain verification: PASSED` verdict and exits 0.
#[test]
fn render_genuine_chain_verifies_passed() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_audit_render_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let db_path = tmp.join("audit.db");
    let key = seed_test_key(&db_path);
    let session_id = seed_chain(&db_path, &key, "worker");

    let (code, stdout, stderr) = run_caprun_audit(&session_id.to_string(), db_path.to_str().unwrap());
    let stdout = String::from_utf8_lossy(&stdout).into_owned();

    assert_eq!(
        code, 0,
        "a genuine keyed chain must render + verify PASSED (exit 0); stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Chain verification: PASSED"),
        "stdout must show a PASSED verdict; got:\n{stdout}"
    );
    assert!(
        stdout.contains("session_created") && stdout.contains("file_read"),
        "stdout must render the seeded events; got:\n{stdout}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Leg 2 — FAIL-CLOSED-ON-ABSENT-KEY: with the `<db>.key` sibling removed, the
/// viewer hard-errors (non-zero exit) and prints NO `Chain verification:`
/// verdict — it never verifies against a fresh/meaningless key (U1 M2).
#[test]
fn absent_key_fails_closed_with_no_verdict() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_audit_absent_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let db_path = tmp.join("audit.db");
    let key = seed_test_key(&db_path);
    let session_id = seed_chain(&db_path, &key, "worker");

    // Remove the `.key` sibling — the viewer must now fail closed.
    let key_path = PathBuf::from(format!("{}.key", db_path.to_str().unwrap()));
    std::fs::remove_file(&key_path).expect("remove key file");

    let (code, stdout, _stderr) =
        run_caprun_audit(&session_id.to_string(), db_path.to_str().unwrap());
    let stdout = String::from_utf8_lossy(&stdout).into_owned();

    assert_ne!(
        code, 0,
        "an absent MAC key must fail closed (non-zero exit); got exit 0 with stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("Chain verification:"),
        "the viewer must print NO verdict on an absent key (never PASSED/FAILED against a \
         fresh key); got:\n{stdout}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Leg 3 — MEMORY-REFUSED: `caprun audit <session> :memory:` exits non-zero
/// with no verdict — a `:memory:` DB has no persisted chain (U1 M2).
#[test]
fn memory_db_is_refused_with_no_verdict() {
    let session_id = Uuid::new_v4();

    let (code, stdout, _stderr) = run_caprun_audit(&session_id.to_string(), ":memory:");
    let stdout = String::from_utf8_lossy(&stdout).into_owned();

    assert_ne!(
        code, 0,
        ":memory: must be refused (non-zero exit); got exit 0 with stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("Chain verification:"),
        "a refused :memory: DB must render NO verdict; got:\n{stdout}"
    );
}

/// Leg 4 — TAINTED-LITERAL-NEUTRALIZED: an event whose `actor` embeds a
/// `\x1b[2K` (ESC CSI "erase line") sequence renders with the ESC escaped to a
/// visible `\x1b` form, and NO raw ESC (0x1b) byte survives to stdout — the
/// audit-line-spoofing surface is closed (U1 M3 / WG-2).
#[test]
fn tainted_actor_literal_is_neutralized() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_audit_taint_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let db_path = tmp.join("audit.db");
    let key = seed_test_key(&db_path);
    // An actor carrying a raw ESC CSI "erase line" — a terminal would clear the
    // line if it reached stdout raw.
    let tainted_actor = "worker\x1b[2Kspoofed";
    let session_id = seed_chain(&db_path, &key, tainted_actor);

    let (code, stdout_bytes, stderr) =
        run_caprun_audit(&session_id.to_string(), db_path.to_str().unwrap());
    let stdout = String::from_utf8_lossy(&stdout_bytes).into_owned();

    assert_eq!(
        code, 0,
        "a genuine (if tainted-literal) chain must still render + verify PASSED; stderr:\n{stderr}"
    );
    // The ESC byte must be escaped to a visible `\x1b` literal...
    assert!(
        stdout.contains("worker\\x1b[2Kspoofed"),
        "the tainted actor must render with the ESC escaped to a visible \\x1b; got:\n{stdout}"
    );
    // ...and NO raw ESC (0x1b) byte may survive to stdout.
    assert!(
        !stdout_bytes.contains(&0x1b_u8),
        "no raw ESC (0x1b) byte may reach the terminal (U1 M3 audit-line-spoofing surface)"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
