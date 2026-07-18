//! harden01_session_integrity — HARDEN-01 (demote-at-RequestFd) + X-04/F3
//! (shared, monotonic `Arc<Mutex<SessionStatus>>`) proof (v1.6 Phase 27).
//!
//! Three tests, driving the REAL `dispatch_request`/`RequestFd` production
//! code (never stubbed):
//!
//!   A. `fd_grant_on_untrusted_path_demotes_without_report_claims` — the
//!      HARDEN-01 primary, negative case: an fd granted for a path whose
//!      inode identity differs from the trusted `<workspace-file>`, with NO
//!      subsequent `ReportClaims`, still demotes the session to `Draft` at
//!      GRANT time — proving the demotion is genuinely fd-grant-time, not
//!      worker-reported-read-time.
//!   B. `second_dispatch_call_after_demotion_observes_draft_not_stale_active`
//!      — the X-04/F3 fold: a SEPARATE `dispatch_request` call (its own
//!      fresh `ValueStore`, mirroring a genuinely distinct per-connection
//!      scope — HARD-03) sharing ONLY the same `Arc<Mutex<SessionStatus>>`
//!      handle as the first call observes the `Draft` the first call
//!      committed, not a stale `Active` snapshot.
//!   C. `fd_grant_on_trusted_path_stays_active` — the SC2 regression: the
//!      trusted `<workspace-file>` path (inode match) stays `Active`, so the
//!      CONTROL-01 clean-send path is not regressed.
//!
//! # Why no test here drives a real Planner (`ConnectionRole::Planner`)
//! wire connection for Test B
//!
//! `DeclarePlannerRole`/`ConnectionRole::Planner` is a genuinely-unused
//! forward-looking seam in this codebase today (Phase 20, PLANNER-02/04) —
//! grep confirms it is referenced only inside `crates/brokerd/src/` and
//! `crates/brokerd/tests/`, never from `cli/caprun`'s production code path.
//! Structurally, ANY `ConnectionRole::Planner` connection's own
//! `ValueStore` is ALWAYS empty (HARD-03: no mint verb is `permits()`-ed for
//! that role), so a plan node with resolvable args can never actually be
//! submitted over a literal second wire connection with today's shipped
//! sink registry (confirmed: `planner_reduced_signal.rs`'s own Linux-gated
//! accept-loop test uses an EMPTY-args node for exactly this reason, noting
//! "under HARD-03 the planner's own ValueStore is empty, so an args-bearing
//! node's handles would not resolve"). The X-04/F3 property under test here
//! — "a demotion committed via one `dispatch_request` call is visible to
//! ANY other holder of the SAME shared `Arc<Mutex<SessionStatus>>` handle,
//! never a stale per-connection copy" — is agnostic to which `ConnectionRole`
//! calls `dispatch_request`: `dispatch_request` re-reads the identical
//! shared cell under lock at the top of every call regardless of caller
//! (Task 1's fix). Test B therefore proves the mechanism directly with TWO
//! independent `dispatch_request` calls, each with its OWN fresh
//! `ValueStore`/chain-state locals (mirroring what two genuinely separate
//! connections hold) sharing only the ONE `Arc<Mutex<SessionStatus>>`
//! handle — the load-bearing shared-state boundary the fix closes — rather
//! than adding Linux-only abstract-socket plumbing that would not, in this
//! codebase's CURRENT sink registry, actually let a second connection's
//! `SubmitPlanNode` reach Step 0.5 with resolvable args at all.
//!
//! None of these three tests requires Linux confinement (Landlock/seccomp/
//! `openat2` `RESOLVE_BENEATH` enforcement) — they exercise the fstat
//! identity-compare + demotion LOGIC and the shared-cell-visibility LOGIC,
//! which run identically on `WorkspaceRoot`'s macOS stub (an ordinary
//! `std::fs::File::open`) and its Linux `openat2` path (mirrors the
//! existing, already-ungated `provide_intent_after_request_fd_is_rejected`
//! in-module test in `crates/brokerd/src/server.rs`, which also drives
//! `RequestFd`+`recv_fd` on macOS). All three therefore run — and prove
//! something real — on both platforms, not "0 passed by design."

use adapter_fs::recv_fd;
use brokerd::audit::{find_event_by_type, open_audit_db};
use brokerd::proto::{BrokerRequest, BrokerResponse};
use brokerd::server::dispatch_request;
use brokerd::session::persist_session;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId, TaintLabel};
use runtime_core::{DenyReason, ExecutorDecision, Session, SessionStatus};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Fixed, non-secret test MAC key (v1.6 Phase 28, HARDEN-02).
const TEST_KEY: &[u8] = b"harden01-session-integrity-test-key";

/// A fresh workspace dir containing TWO distinct files — `trusted.txt` (the
/// CLI-designated `<workspace-file>`) and `hostile.txt` (any other in-tree
/// path a worker might `RequestFd`, standing in for an attacker-controlled
/// document). Returns `(ws_dir, ws_root, trusted_path)`.
fn fresh_workspace(tag: &str) -> (std::path::PathBuf, Arc<adapter_fs::workspace::WorkspaceRoot>, std::path::PathBuf) {
    let mut ws_dir = std::env::temp_dir();
    ws_dir.push(format!("caprun_harden01_{tag}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&ws_dir).expect("create ws dir");
    std::fs::write(ws_dir.join("trusted.txt"), b"trusted workspace-file content")
        .expect("write trusted.txt");
    std::fs::write(ws_dir.join("hostile.txt"), b"hostile attacker-controlled content")
        .expect("write hostile.txt");
    let ws_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(&ws_dir).expect("open ws root"),
    );
    let trusted_path = ws_dir.join("trusted.txt");
    (ws_dir, ws_root, trusted_path)
}

/// fstat a path to the `(dev, ino)` pair `dispatch_request`'s `trusted_inode`
/// parameter now expects (v1.6 Phase 27 review Fix 2: the broker itself
/// freezes this identity exactly once, at `run_broker_server` entry, rather
/// than re-resolving the path on every `RequestFd`; these tests mirror that
/// same freeze-once-per-scope discipline by stat'ing `trusted_path` a single
/// time up front, not once per `RequestFd` call).
fn trusted_inode_of(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// Seed a `sessions` row for `session_id` so `update_session_status`'s
/// `UPDATE ... WHERE id = ?` has a real row to affect, and so the persisted
/// status is queryable afterward — mirrors `planner_reduced_signal.rs`'s
/// `spawn_fresh_broker` harness.
fn seed_session_row(conn: &rusqlite::Connection, session_id: Uuid) {
    let session = Session {
        id: session_id,
        intent_id: Uuid::new_v4(),
        status: SessionStatus::Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    persist_session(conn, &session).expect("seed sessions row");
}

/// Read the persisted `sessions.status` column for `session_id`.
fn persisted_status(conn: &rusqlite::Connection, session_id: Uuid) -> SessionStatus {
    let status_json: String = conn
        .query_row(
            "SELECT status FROM sessions WHERE id = ?1",
            rusqlite::params![session_id.to_string()],
            |row| row.get(0),
        )
        .expect("query persisted status");
    serde_json::from_str(&status_json).expect("deserialize persisted SessionStatus")
}

/// Drive a `RequestFd { path }` through the REAL `dispatch_request` and
/// return `(response, last_event_id, last_event_hash)`. Handles the
/// RequestFd arm's SCM_RIGHTS-before-framed-response ordering (mirrors
/// `server.rs`'s own in-module `provide_intent_after_request_fd_is_rejected`
/// test and `planner_reduced_signal.rs`'s `request_fd` helper).
#[allow(clippy::too_many_arguments)]
async fn request_fd_via_dispatch(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
    store: &mut ValueStore,
    ws_root: &Arc<adapter_fs::workspace::WorkspaceRoot>,
    session_status: &Arc<Mutex<SessionStatus>>,
    trusted_inode: Option<(u64, u64)>,
    path: &str,
) -> BrokerResponse {
    let mut intent_provided = false;
    let mut fd_requested = false;
    let mut fd_request_count: u32 = 0;
    let (mut server_end, mut client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");

    dispatch_request(
        BrokerRequest::RequestFd { path: path.to_string() },
        &mut server_end,
        conn,
        TEST_KEY,
        session_id,
        last_event_id,
        last_event_hash,
        store,
        ws_root,
        session_status,
        trusted_inode,
        &mut intent_provided,
        &mut fd_requested,
        &mut fd_request_count,
    )
    .await
    .expect("dispatch_request(RequestFd) must complete");

    // Drain the RequestFd arm's mandatory 1-byte SCM_RIGHTS sendmsg payload
    // BEFORE reading the framed FdGranted response (the arm sends the fd
    // ahead of the JSON response — see server.rs's own test/doc comments).
    let received_fd = recv_fd(client_end.as_raw_fd())
        .expect("recv_fd must consume the RequestFd arm's SCM_RIGHTS payload");
    drop(unsafe { std::fs::File::from_raw_fd(received_fd) });

    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    client_end.read_exact(&mut len_buf).await.expect("read length");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    client_end.read_exact(&mut body).await.expect("read body");
    serde_json::from_slice(&body).expect("deserialize BrokerResponse")
}

/// Test A (HARDEN-01 primary, negative): `RequestFd` on a path whose inode
/// identity differs from the trusted `<workspace-file>`, with NO subsequent
/// `ReportClaims`, still demotes the session to `Draft` AT GRANT TIME —
/// proving the demotion no longer depends on the worker choosing to
/// self-report the read.
#[tokio::test]
async fn fd_grant_on_untrusted_path_demotes_without_report_claims() {
    let (_ws_dir, ws_root, trusted_path) = fresh_workspace("test_a");
    let trusted_inode = trusted_inode_of(&trusted_path);
    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let session_id = Uuid::new_v4();
    seed_session_row(&conn.lock().unwrap(), session_id);

    let session_status = Arc::new(Mutex::new(SessionStatus::Active));
    let mut store = ValueStore::default();
    let mut last_event_id = Uuid::new_v4();
    let mut last_event_hash = "genesis-hash".to_string();

    // RequestFd the HOSTILE (untrusted) path — NOT the trusted inode.
    let resp = request_fd_via_dispatch(
        &conn,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root,
        &session_status,
        trusted_inode,
        "hostile.txt",
    )
    .await;
    assert!(
        matches!(resp, BrokerResponse::FdGranted),
        "expected FdGranted (demotion never blocks the read), got {resp:?}"
    );

    // The demotion happened at GRANT time — no ReportClaims was ever sent.
    assert_eq!(
        *session_status.lock().unwrap(),
        SessionStatus::Draft,
        "the shared in-memory cell must read Draft after an untrusted RequestFd, \
         with no ReportClaims ever sent"
    );
    assert_eq!(
        persisted_status(&conn.lock().unwrap(), session_id),
        SessionStatus::Draft,
        "the persisted sessions.status row must also read Draft (atomic pair, TAINT-04)"
    );

    // A genuine causal edge: session_demoted.parent_id == fd_granted.id.
    let locked = conn.lock().unwrap();
    let sid = session_id.to_string();
    let fd_granted = find_event_by_type(&locked, &sid, "fd_granted")
        .expect("query fd_granted")
        .expect("an fd_granted event must exist");
    let session_demoted = find_event_by_type(&locked, &sid, "session_demoted")
        .expect("query session_demoted")
        .expect("a session_demoted event must exist (fd-grant-time demotion)");
    assert_eq!(
        session_demoted.parent_id,
        Some(fd_granted.id),
        "session_demoted must be parented on fd_granted's id — a genuine causal \
         edge, not a stapled tag"
    );
}

/// Test B (X-04/F3): a demotion committed by one `dispatch_request` call
/// (its own `ValueStore`, mirroring a genuinely separate connection scope)
/// is observed by a SECOND, independent `dispatch_request` call that shares
/// ONLY the same `Arc<Mutex<SessionStatus>>` handle — never a stale `Active`
/// snapshot. This test MUST fail against a pre-Task-1 build: before the
/// X-04/F3 fix, `dispatch_request` took an OWNED `&mut SessionStatus` with
/// no shared cell at all, so a second, independent call could only ever
/// start from its own fresh `Active` seed and would reach `Allowed` here
/// instead of `Denied`.
#[tokio::test]
async fn second_dispatch_call_after_demotion_observes_draft_not_stale_active() {
    let (_ws_dir, ws_root, trusted_path) = fresh_workspace("test_b");
    let trusted_inode = trusted_inode_of(&trusted_path);
    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let session_id = Uuid::new_v4();
    seed_session_row(&conn.lock().unwrap(), session_id);

    let session_status = Arc::new(Mutex::new(SessionStatus::Active));

    // "Connection 1" (Worker): its OWN ValueStore + chain-state locals,
    // requests the HOSTILE (untrusted) fd — demotes the SHARED cell.
    let mut conn1_store = ValueStore::default();
    let mut conn1_last_event_id = Uuid::new_v4();
    let mut conn1_last_event_hash = "genesis-hash".to_string();
    let resp1 = request_fd_via_dispatch(
        &conn,
        session_id,
        &mut conn1_last_event_id,
        &mut conn1_last_event_hash,
        &mut conn1_store,
        &ws_root,
        &session_status,
        trusted_inode,
        "hostile.txt",
    )
    .await;
    assert!(matches!(resp1, BrokerResponse::FdGranted));
    assert_eq!(*session_status.lock().unwrap(), SessionStatus::Draft);

    // "Connection 2" (would-be Planner, or any second connection): a FRESH,
    // INDEPENDENT ValueStore and its OWN fresh chain-state locals (HARD-03
    // per-connection isolation — never conn1's store or chain head).
    // Directly mints an all-UserTrusted `file.create` plan node into THIS
    // store (legitimate test-only direct `ValueStore::mint` — this is a
    // /tests/ file, exempt from check-invariants.sh Gate 3's mint-call-site
    // restriction, and mirrors durable_anchor.rs/extract_provenance_
    // threading.rs's existing precedent of minting a UserTrusted `contents`
    // value directly in a test harness). `file.create` is CommitIrreversible
    // (sink_effect_class) and has NO content-sensitive args and only `path`
    // is routing-sensitive — an all-UserTrusted set therefore reaches Step
    // 0.5 cleanly (no per-arg I2 Block masks the I0 class-level check).
    let mut conn2_store = ValueStore::default();
    let path_value_id = conn2_store
        .mint(
            "report.txt".to_string(),
            vec![TaintLabel::UserTrusted],
            vec![Uuid::new_v4()],
            Some("path".to_string()),
        )
        .expect("mint UserTrusted path value");
    // HARDEN-05 (v1.6): `contents` is now role-checked to `Some(&["path"])`
    // — mint with the reused trusted `"path"` role (the only live production
    // shape) so this stays a clean, all-trusted fixture that reaches Step
    // 0.5 (not a SlotTypeMismatch).
    let contents_value_id = conn2_store
        .mint(
            "clean trusted contents".to_string(),
            vec![TaintLabel::UserTrusted],
            vec![Uuid::new_v4()],
            Some("path".to_string()),
        )
        .expect("mint UserTrusted contents value");
    let plan_node = PlanNode {
        sink: SinkId("file.create".into()),
        args: vec![
            PlanArg { name: "path".into(), value_id: path_value_id },
            PlanArg { name: "contents".into(), value_id: contents_value_id },
        ],
    };
    let mut conn2_last_event_id = Uuid::new_v4();
    let mut conn2_last_event_hash = "genesis-hash-conn2".to_string();
    let mut conn2_intent_provided = false;
    let mut conn2_fd_requested = false;
    let mut conn2_fd_request_count: u32 = 0;
    let (mut conn2_server_end, mut conn2_client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");

    dispatch_request(
        BrokerRequest::SubmitPlanNode { plan_node },
        &mut conn2_server_end,
        &conn,
        TEST_KEY,
        session_id,
        &mut conn2_last_event_id,
        &mut conn2_last_event_hash,
        &mut conn2_store,
        &ws_root,
        // THE SAME shared Arc<Mutex<SessionStatus>> handle conn1 used —
        // never a fresh clone of an owned SessionStatus.
        &session_status,
        trusted_inode,
        &mut conn2_intent_provided,
        &mut conn2_fd_requested,
        &mut conn2_fd_request_count,
    )
    .await
    .expect("dispatch_request(SubmitPlanNode) must complete");

    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    conn2_client_end.read_exact(&mut len_buf).await.expect("read length");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    conn2_client_end.read_exact(&mut body).await.expect("read body");
    let resp2: BrokerResponse = serde_json::from_slice(&body).expect("deserialize response");

    match resp2 {
        BrokerResponse::PlanNodeDecision {
            decision: ExecutorDecision::Denied {
                reason: DenyReason::DraftOnlySessionDeniesCommitIrreversible { .. },
            },
            ..
        } => {
            // Expected: connection 2 observed Draft via the shared cell.
        }
        other => panic!(
            "connection 2's dispatch_request call must observe the shared cell's \
             Draft status (X-04/F3) and Deny the CommitIrreversible file.create \
             plan node, got {other:?} — a stale Active snapshot would instead \
             yield PlanNodeDecision {{ decision: Allowed }}"
        ),
    }
}

/// Test C (SC2 regression): `RequestFd` on the TRUSTED `<workspace-file>`
/// path (inode match) stays `Active` — the benign clean path is not
/// regressed. Deliberately drives ONLY `RequestFd`, never `ReportClaims`:
/// `mint_from_read` (the pre-existing, UNRELATED demotion site) demotes
/// UNCONDITIONALLY on any reported claim regardless of the underlying
/// file's trust label, so including a `ReportClaims` call here would make
/// the "stays Active" assertion fail for a reason that has nothing to do
/// with HARDEN-01's fstat-identity mechanism — this test isolates the
/// RequestFd-time trust check from that separate, already-tested behavior.
#[tokio::test]
async fn fd_grant_on_trusted_path_stays_active() {
    let (_ws_dir, ws_root, trusted_path) = fresh_workspace("test_c");
    let trusted_inode = trusted_inode_of(&trusted_path);
    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let session_id = Uuid::new_v4();
    seed_session_row(&conn.lock().unwrap(), session_id);

    let session_status = Arc::new(Mutex::new(SessionStatus::Active));
    let mut store = ValueStore::default();
    let mut last_event_id = Uuid::new_v4();
    let mut last_event_hash = "genesis-hash".to_string();

    // RequestFd the TRUSTED path itself — "trusted.txt", the same file
    // `trusted_path` points to (inode-identical). `trusted_inode` is stat'd
    // ONCE, above, mirroring the broker's own freeze-at-startup pattern
    // (review Fix 2) — this is the Test-C stay-Active case: the inodes
    // match, so the fd-grant demotion arm's compare succeeds.
    let resp = request_fd_via_dispatch(
        &conn,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root,
        &session_status,
        trusted_inode,
        "trusted.txt",
    )
    .await;
    assert!(
        matches!(resp, BrokerResponse::FdGranted),
        "expected FdGranted, got {resp:?}"
    );

    assert_eq!(
        *session_status.lock().unwrap(),
        SessionStatus::Active,
        "the trusted, inode-matched clean path must stay Active — no demotion (SC2)"
    );
    assert_eq!(
        persisted_status(&conn.lock().unwrap(), session_id),
        SessionStatus::Active,
        "the persisted sessions.status row must also stay Active — no UPDATE fired"
    );

    // No session_demoted event should exist at all on the trusted path.
    let locked = conn.lock().unwrap();
    let sid = session_id.to_string();
    let session_demoted = find_event_by_type(&locked, &sid, "session_demoted")
        .expect("query session_demoted");
    assert!(
        session_demoted.is_none(),
        "no session_demoted event must exist on the trusted, inode-matched path, \
         got {session_demoted:?}"
    );
}
