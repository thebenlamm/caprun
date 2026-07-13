/// confirm — cross-process integration test for the single-shot confirmation
/// loop (CONFIRM-01..04, DESIGN-confirmation-release.md).
///
/// `caprun confirm`/`caprun deny` are ALWAYS separate, LATER OS processes from
/// the one that created the block ("The Problem Being Solved" in the DESIGN
/// doc) — CONFIRM-03's cross-process durability claim cannot be honestly tested
/// within one process (RESEARCH Pitfall 5). These tests spawn the REAL compiled
/// `caprun` binary as separate subprocesses against a shared, PERSISTENT SQLite
/// audit DB (never `:memory:` — an in-memory DB has nothing to resume from once
/// the seeding process exits).
///
/// Unlike `tests/e2e.rs` (whose two tests are `#[cfg(target_os = "linux")]`-gated
/// because they spawn the broker+worker process tree over an abstract-namespace
/// UDS, Linux-only), the confirm/deny decision logic + persistence +
/// `adapter_fs::workspace::WorkspaceRoot`'s non-Linux
/// `create_exclusive_within` stub all run on macOS. These tests therefore seed
/// the Pending block DIRECTLY via brokerd's public API (mirroring
/// `crates/brokerd/src/server.rs`'s `SubmitPlanNode` block-time write) rather
/// than driving a real confined-worker run — this exercises the CLI + confirm/
/// deny across real OS processes without needing a Linux worker to produce the
/// block. A genuine live §9 block → confirm/deny run (real worker, real
/// Landlock/seccomp) is exercised in Phase 11's live acceptance via the
/// Colima+Docker recipe (ACC-01/02/03). None of these tests are blanket-gated
/// behind `#[cfg(target_os = "linux")]`.
use brokerd::audit::{append_event, insert_blocked_literal, open_audit_db};
use brokerd::confirmation::{
    combined_digest, insert_pending_confirmation, PendingConfirmation, PendingConfirmationState,
    ResolvedArg,
};
use chrono::Utc;
use runtime_core::executor_decision::SinkBlockedAnchor;
use runtime_core::plan_node::{SinkId, TaintLabel, ValueId};
use runtime_core::Event;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

/// Test-local mirror of `cli/caprun/src/key.rs`'s idempotent
/// read-existing-first custody (that module is `pub(crate)` inside the
/// bin-only `caprun` crate — `cli/caprun` has no lib target, so this
/// external integration-test crate cannot import it directly). This
/// duplicates ONLY the read-or-create-and-persist behavior (no F1 check —
/// this test fully controls its own tmpdir layout, which is always F1-safe:
/// `audit.db` and `workspace/` are created as SIBLINGS under the same unique
/// `tmp` dir, never nested) so the SAME key bytes land on disk at
/// `<db_path>.key` BEFORE the seeding `append_event` calls below — matching
/// what the `caprun confirm`/`deny` subprocess's OWN
/// `key::load_or_create_key` call reads back (v1.6 Phase 28, HARDEN-02).
fn seed_test_key(db_path: &Path) -> Vec<u8> {
    let key_path = PathBuf::from(format!("{}.key", db_path.to_str().unwrap()));
    if let Ok(bytes) = std::fs::read(&key_path) {
        return bytes;
    }
    // Uniqueness (not cryptographic strength) is all this test-local key
    // needs: every test uses its own fresh `run_id`-suffixed tmp dir, so
    // there is no cross-test collision risk.
    let mut key = Uuid::new_v4().as_bytes().to_vec();
    key.extend_from_slice(Uuid::new_v4().as_bytes());
    std::fs::write(&key_path, &key).expect("write test MAC key file");
    key
}

/// Seed a Pending file.create block directly via brokerd's API against the
/// persistent DB at `db_path`: a causal-root event, a `sink_blocked` event
/// carrying a genuine `SinkBlockedAnchor`, its `blocked_literals` row, and a
/// matching `PendingConfirmation` — mirroring server.rs's `SubmitPlanNode`
/// block-time write (minus the live `plan_node`/`ValueStore`, which this
/// integration test has no need to construct).
///
/// The seeding connection is opened and dropped within this function, so the
/// persistent file is fully released before any `caprun confirm`/`deny`
/// subprocess opens its own connection to the same path.
///
/// Returns `(effect_id, session_id, blocked_event_id)`.
fn seed_pending_file_create_block(
    db_path: &Path,
    key: &[u8],
    path: &str,
    contents: &str,
    workspace_root: &Path,
) -> (Uuid, Uuid, Uuid) {
    let conn = open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db for seeding");

    let session_id = Uuid::new_v4();
    let effect_id = Uuid::new_v4();
    let read_event_id = Uuid::new_v4();

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

    let literal_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        hex::encode(hasher.finalize())
    };
    let anchor = SinkBlockedAnchor {
        effect_id,
        sink: SinkId("file.create".into()),
        arg: "path".into(),
        value_id: ValueId::new(),
        literal_sha256,
        taint: vec![TaintLabel::PathRaw],
        provenance_chain: vec![read_event_id],
        read_event_id,
    };

    let resolved_args = vec![
        ResolvedArg {
            name: "path".into(),
            value_id: ValueId::new(),
            literal: path.to_string(),
            taint: vec![TaintLabel::PathRaw],
            provenance_chain: vec![read_event_id],
        },
        ResolvedArg {
            name: "contents".into(),
            value_id: ValueId::new(),
            literal: contents.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        },
    ];
    // CONFIRM-03 (Round-6): computed once over the FULL resolved_args set,
    // threaded into BOTH the sink_blocked Event and the PendingConfirmation
    // below — mirrors server.rs's Block-time write.
    let digest = combined_digest(
        &resolved_args
            .iter()
            .map(|a| (a.name.as_str(), a.literal.as_str()))
            .collect::<Vec<_>>(),
    );
    let blocked_arg_names = vec!["path".to_string()];

    let blocked_event = Event::sink_blocked(
        Uuid::new_v4(),
        Some(root.id),
        session_id,
        Utc::now(),
        vec![anchor],
        Some(digest.clone()),
        blocked_arg_names.clone(),
    );
    let blocked_event_id = blocked_event.id;
    append_event(&conn, key, &blocked_event, Some(&root_hash)).expect("append sink_blocked");
    insert_blocked_literal(&conn, &blocked_event_id.to_string(), "path", path)
        .expect("insert_blocked_literal");

    let pc = PendingConfirmation {
        effect_id,
        session_id,
        blocked_event_id,
        sink: SinkId("file.create".into()),
        resolved_args,
        blocked_arg_names,
        combined_digest: digest,
        workspace_root_path: workspace_root.to_string_lossy().into_owned(),
        state: PendingConfirmationState::Pending,
        mac: String::new(),
    };
    insert_pending_confirmation(&conn, key, &pc).expect("insert_pending_confirmation");

    // `conn` drops here — the persistent file is fully released before any
    // `caprun confirm`/`deny` subprocess opens its own connection.
    (effect_id, session_id, blocked_event_id)
}

/// Run `caprun confirm <effect_id> <db_path>` or `caprun deny <effect_id>
/// <db_path>` as a REAL separate OS process. Returns `(exit_code, stdout)`.
fn run_caprun_verb(verb: &str, effect_id: Uuid, db_path: &Path) -> (i32, String) {
    run_caprun_verb_with_env(verb, effect_id, db_path, &[])
}

/// Same as `run_caprun_verb`, but with additional environment variables set
/// on the child process (used to point the confirm-path `email.send` adapter
/// at a specific `CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` for Phase 13's
/// exit-code-7 acceptance test).
fn run_caprun_verb_with_env(
    verb: &str,
    effect_id: Uuid,
    db_path: &Path,
    envs: &[(&str, &str)],
) -> (i32, String) {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let mut cmd = Command::new(caprun_bin);
    cmd.arg(verb)
        .arg(effect_id.to_string())
        .arg(db_path.to_str().unwrap());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawn caprun {verb}: {e}"));
    (
        output.status.code().expect("process must exit with a code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// Assert a raw `events` row of `event_type` exists whose `actor` contains
/// `effect_id` and whose `parent_id` equals `expected_parent_id` (CONFIRM-04:
/// confirm/deny events anchored to the same effect_id as the sink_blocked
/// event, preserving one unbroken causal chain).
fn assert_anchored_event(
    db_path: &Path,
    event_type: &str,
    effect_id: Uuid,
    expected_parent_id: Uuid,
) {
    let conn = open_audit_db(db_path.to_str().unwrap()).expect("reopen persisted audit DB");
    let (actor, parent_id): (String, Option<String>) = conn
        .query_row(
            "SELECT actor, parent_id FROM events WHERE event_type = ?1",
            [event_type],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or_else(|e| panic!("{event_type} event must exist in the persisted DB: {e}"));
    assert!(
        actor.contains(&effect_id.to_string()),
        "{event_type} actor must carry the effect_id; got {actor}"
    );
    assert_eq!(
        parent_id.as_deref(),
        Some(expected_parent_id.to_string().as_str()),
        "{event_type}.parent_id must equal the sink_blocked event id (CONFIRM-04)"
    );
}

/// CONFIRM-01 + CONFIRM-02 + CONFIRM-03 (release half): a first `caprun
/// confirm` on a Pending file.create block prints the verbatim literal +
/// taint, exits 0, and creates the file exactly once; a SECOND `caprun
/// confirm` on the SAME effect_id (a distinct process) exits 5
/// (AlreadyTerminal) and creates no additional file. CONFIRM-04: a
/// confirm_granted event exists anchored to the sink_blocked event.
#[test]
fn confirm_releases_once_and_second_confirm_is_already_terminal() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_confirm_release_{run_id}"));
    let workspace = tmp.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace dir");
    let db_path = tmp.join("audit.db");
    let key = seed_test_key(&db_path);

    let (effect_id, _session_id, blocked_event_id) =
        seed_pending_file_create_block(&db_path, &key, "released.txt", "hello world", &workspace);

    // First confirm: releases exactly once (CONFIRM-01, CONFIRM-02).
    let (code, stdout) = run_caprun_verb("confirm", effect_id, &db_path);
    assert_eq!(code, 0, "first confirm on a Pending block must exit 0 (Released)");
    assert!(
        stdout.contains("\"released.txt\""),
        "stdout must show the verbatim blocked literal in quotes (CONFIRM-01); got:\n{stdout}"
    );
    assert!(
        stdout.contains("Taint:"),
        "stdout must show a Taint: line (CONFIRM-01); got:\n{stdout}"
    );
    let created = workspace.join("released.txt");
    assert!(created.exists(), "confirm must create the target file exactly once");
    assert_eq!(
        std::fs::read_to_string(&created).unwrap(),
        "hello world",
        "created file must carry the frozen contents literal"
    );

    // Second confirm: a DISTINCT process, same effect_id — durable single-shot
    // release (CONFIRM-03).
    let (code2, _stdout2) = run_caprun_verb("confirm", effect_id, &db_path);
    assert_eq!(
        code2, 5,
        "a second confirm on an already-Confirmed effect_id must exit 5 (AlreadyTerminal)"
    );
    let entries: Vec<_> = std::fs::read_dir(&workspace).unwrap().collect();
    assert_eq!(
        entries.len(),
        1,
        "a second confirm must not create any additional file"
    );

    // CONFIRM-04: confirm_granted anchored to the sink_blocked event.
    assert_anchored_event(&db_path, "confirm_granted", effect_id, blocked_event_id);

    std::fs::remove_dir_all(&tmp).ok();
}

/// CONFIRM-03 (deny durability half): `caprun deny` on a fresh Pending block
/// exits 2; a following `caprun confirm` on that SAME effect_id (a distinct
/// process) exits 5 (AlreadyTerminal); the target file never exists.
/// CONFIRM-04: a confirm_denied event exists anchored to the sink_blocked
/// event.
#[test]
fn deny_is_durable_and_confirm_after_deny_is_already_terminal() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_confirm_deny_{run_id}"));
    let workspace = tmp.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace dir");
    let db_path = tmp.join("audit.db");
    let key = seed_test_key(&db_path);

    let (effect_id, _session_id, blocked_event_id) = seed_pending_file_create_block(
        &db_path,
        &key,
        "denied.txt",
        "should never land",
        &workspace,
    );

    let (code, _stdout) = run_caprun_verb("deny", effect_id, &db_path);
    assert_eq!(code, 2, "deny on a Pending block must exit 2 (Denied)");

    let (code2, _stdout2) = run_caprun_verb("confirm", effect_id, &db_path);
    assert_eq!(
        code2, 5,
        "confirm on an already-Denied effect_id must exit 5 (AlreadyTerminal), never released"
    );

    assert!(
        !workspace.join("denied.txt").exists(),
        "the target file must NEVER exist once the effect_id has been denied"
    );

    // CONFIRM-04: confirm_denied anchored to the sink_blocked event.
    assert_anchored_event(&db_path, "confirm_denied", effect_id, blocked_event_id);

    std::fs::remove_dir_all(&tmp).ok();
}

/// An unknown `effect_id` (no `PendingConfirmation` row was ever persisted for
/// it) fails closed on both verbs — exit 4 (T-10-03: a forged/unknown
/// effect_id can never be released or denied into existing).
#[test]
fn confirm_and_deny_on_unknown_effect_id_exit_4() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_confirm_unknown_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    let db_path = tmp.join("audit.db");

    // Create the schema (so the DB file exists) but seed nothing.
    { open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db"); }

    let unknown = Uuid::new_v4();
    let (code, _stdout) = run_caprun_verb("confirm", unknown, &db_path);
    assert_eq!(code, 4, "confirm on an unknown effect_id must exit 4 (UnknownEffect)");

    let (code2, _stdout2) = run_caprun_verb("deny", unknown, &db_path);
    assert_eq!(code2, 4, "deny on an unknown effect_id must exit 4 (UnknownEffect)");

    std::fs::remove_dir_all(&tmp).ok();
}

/// Seed a Pending email.send block directly via brokerd's API against the
/// persistent DB at `db_path` — mirrors `seed_pending_file_create_block` but
/// for the `email.send` sink (Phase 13 Plan 02, SEND-01/SEND-02).
///
/// `workspace_root` MUST be a real, existing directory even though
/// `confirm()`'s `email.send` arm never reads it: `run_confirm_or_deny`
/// (`cli/caprun/src/main.rs`) unconditionally opens
/// `PendingConfirmation.workspace_root_path` via `WorkspaceRoot::open` BEFORE
/// dispatching to `confirm()`, regardless of sink — a pre-existing CLI
/// behavior, out of this phase's scope to change.
///
/// Returns `(effect_id, session_id, blocked_event_id)`.
fn seed_pending_email_send_block(
    db_path: &Path,
    key: &[u8],
    workspace_root: &Path,
    to: &str,
    subject: &str,
    body: &str,
) -> (Uuid, Uuid, Uuid) {
    let conn = open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db for seeding");

    let session_id = Uuid::new_v4();
    let effect_id = Uuid::new_v4();
    let read_event_id = Uuid::new_v4();

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

    let literal_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(to.as_bytes());
        hex::encode(hasher.finalize())
    };
    let anchor = SinkBlockedAnchor {
        effect_id,
        sink: SinkId("email.send".into()),
        arg: "to".into(),
        value_id: ValueId::new(),
        literal_sha256,
        taint: vec![TaintLabel::ExternalUntrusted],
        provenance_chain: vec![read_event_id],
        read_event_id,
    };

    let resolved_args = vec![
        ResolvedArg {
            name: "to".into(),
            value_id: ValueId::new(),
            literal: to.to_string(),
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![read_event_id],
        },
        ResolvedArg {
            name: "subject".into(),
            value_id: ValueId::new(),
            literal: subject.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        },
        ResolvedArg {
            name: "body".into(),
            value_id: ValueId::new(),
            literal: body.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        },
    ];
    // CONFIRM-03 (Round-6): computed once over the FULL resolved_args set,
    // threaded into BOTH the sink_blocked Event and the PendingConfirmation
    // below — mirrors server.rs's Block-time write.
    let digest = combined_digest(
        &resolved_args
            .iter()
            .map(|a| (a.name.as_str(), a.literal.as_str()))
            .collect::<Vec<_>>(),
    );
    let blocked_arg_names = vec!["to".to_string()];

    let blocked_event = Event::sink_blocked(
        Uuid::new_v4(),
        Some(root.id),
        session_id,
        Utc::now(),
        vec![anchor],
        Some(digest.clone()),
        blocked_arg_names.clone(),
    );
    let blocked_event_id = blocked_event.id;
    append_event(&conn, key, &blocked_event, Some(&root_hash)).expect("append sink_blocked");
    insert_blocked_literal(&conn, &blocked_event_id.to_string(), "to", to)
        .expect("insert_blocked_literal");

    let pc = PendingConfirmation {
        effect_id,
        session_id,
        blocked_event_id,
        sink: SinkId("email.send".into()),
        resolved_args,
        blocked_arg_names,
        combined_digest: digest,
        workspace_root_path: workspace_root.to_string_lossy().into_owned(),
        state: PendingConfirmationState::Pending,
        mac: String::new(),
    };
    insert_pending_confirmation(&conn, key, &pc).expect("insert_pending_confirmation");

    (effect_id, session_id, blocked_event_id)
}

/// SEND-02, real cross-process exit-code contract: a `caprun confirm` on a
/// Pending `email.send` block, run as a genuine separate OS process against a
/// closed SMTP port (`CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` pointed at an
/// ephemeral port with nothing listening), exits 7 — distinct from
/// denied (2) / confirmed-but-sink-failed (3) / unknown (4) / already-terminal
/// (5) / redacted (6). The CAS + `email_send_attempted` transaction still
/// committed (durable, atomic, before the socket ever opened); a re-confirm
/// on the same effect_id is refused (5), proving no auto-retry.
#[test]
fn confirm_email_send_adapter_failure_exits_7() {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_confirm_email_fail_{run_id}"));
    let workspace = tmp.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace dir");
    let db_path = tmp.join("audit.db");
    let key = seed_test_key(&db_path);

    // Bind an ephemeral port then drop the listener immediately — nothing is
    // listening on it, so the adapter's real send fails fast (ECONNREFUSED).
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (effect_id, session_id, _blocked_event_id) = seed_pending_email_send_block(
        &db_path,
        &key,
        &workspace,
        "recipient@example.com",
        "hello",
        "hi there",
    );

    let port_str = port.to_string();
    let envs = [("CAPRUN_SMTP_HOST", "127.0.0.1"), ("CAPRUN_SMTP_PORT", port_str.as_str())];
    let (code, _stdout) = run_caprun_verb_with_env("confirm", effect_id, &db_path, &envs);
    assert_eq!(
        code, 7,
        "email.send adapter failure after confirm must exit 7, distinct from 2/3/4/5/6"
    );

    // The CAS + attempt-append committed atomically before the failed send.
    let conn = open_audit_db(db_path.to_str().unwrap()).expect("reopen persisted audit DB");
    let attempted_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = 'email_send_attempted'",
            [session_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(attempted_count, 1, "exactly ONE email_send_attempted event must be durable");

    // No auto-retry: a re-confirm on the same effect_id refuses.
    let (code2, _stdout2) = run_caprun_verb_with_env("confirm", effect_id, &db_path, &envs);
    assert_eq!(
        code2, 5,
        "a re-confirm after a send failure must be refused (AlreadyTerminal), never retried"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
