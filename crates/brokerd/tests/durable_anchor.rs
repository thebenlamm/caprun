//! durable_anchor — the CANONICAL ACC-07 proof: an AFTER-EXIT, DB-ALONE
//! anti-stapling sentinel plus a tamper-evidence test.
//!
//! This is the authoritative §9 durable-anchor test (DESIGN-durable-anchor-and-
//! label-partition §7 "Authoritative §9"; the in-process `s9_acceptance.rs` is the
//! faster backstop). It drives a genuine `file.create` hostile block END TO END
//! through the real `dispatch_request` SubmitPlanNode arm against a FILE-BACKED
//! SQLite DB, then DROPS + REOPENS the connection (simulating process exit) and
//! reconstructs the genuine-taint proof FROM THE PERSISTED DB ALONE.
//!
//! Why after-exit / DB-alone matters (T-07-51): an in-memory-only assertion proves
//! nothing durable. The genuine-taint edge (raw file_read Event → ValueNode →
//! sensitive sink arg → deterministic block) must survive process exit and be
//! reconstructable from the audit DB by itself. An event-ORDER-only assertion is
//! explicitly INSUFFICIENT — the value-lineage backstops below (the file_read
//! `id == read_event_id == provenance_chain[0]`, untrusted taint) are the
//! anti-stapling proof: they fail if taint were stapled at the sink (T-07-53).
//!
//! Two graphs, never equated (DESIGN §0): the causal DAG (`parent_id`/`parent_hash`,
//! walked by `verify_chain`) and the value-lineage (`anchor.provenance_chain` /
//! `read_event_id`) SHARE node ids but have distinct edge semantics. We assert
//! `verify_chain` FIRST (trust the chain), THEN trust the anchor and its value-
//! lineage backstops. We do NOT assert `sink_blocked.parent_id == read_event_id`.
//!
//! Cross-platform: the BLOCK path performs NO file I/O (the tainted `path` is
//! rejected by the executor before any `create_exclusive_within` call), so these
//! tests pass on macOS and Linux — unlike the live `s9_live_block.rs` tests, which
//! need the Linux confinement stack.

use brokerd::audit::{find_event_by_type, open_audit_db, verify_chain};
use brokerd::proto::BrokerRequest;
use brokerd::quarantine::{mint_from_read, Claim};
use brokerd::server::dispatch_request;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId, TaintLabel, ValueId};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// The hostile, attacker-controlled root-relative path extracted from workspace
/// content. It is byte-exact `data at rest` in the durable anchor — never executed.
const HOSTILE_PATH: &str = "reports/pwned.txt";

/// A workspace-root anchor for the dispatch call. The BLOCK path never invokes the
/// sink (the tainted `path` is rejected upstream), so any valid directory suffices.
fn ws_root() -> Arc<adapter_fs::workspace::WorkspaceRoot> {
    Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
            .expect("open ws root"),
    )
}

/// A unique file-backed audit-DB path (brokerd has no tempfile dev-dep).
fn temp_db_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("caprun_durable_{tag}_{}.db", Uuid::new_v4()))
}

/// Remove a file-backed audit DB and its WAL/SHM sidecars.
fn cleanup_db(db_path: &std::path::Path) {
    let _ = std::fs::remove_file(db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
}

/// Build a FILE-BACKED audit DB containing a genuine `file.create` hostile block,
/// driven END TO END through `dispatch_request`, then DROP the connection to
/// simulate process exit. Returns `(db_path, session_id, read_event_id)`.
///
/// Harness mirrors `phase5_dispatch.rs`: the tainted value is minted via
/// `mint_from_read` (exactly what the `ReportClaims` arm calls — the SOLE broker
/// taint-mint site), with `parent_hash = None` so the `file_read` event is the
/// single causal ROOT of a clean linear chain. The BLOCK itself goes through the
/// real `dispatch_request` SubmitPlanNode arm.
async fn build_hostile_block_db(tag: &str) -> (std::path::PathBuf, Uuid, Uuid) {
    let db_path = temp_db_path(tag);
    let session_id = Uuid::new_v4();

    // FILE-BACKED DB (NOT ":memory:") — the after-exit proof requires durability
    // across a connection drop + reopen.
    let conn = Arc::new(Mutex::new(
        open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db"),
    ));

    // Mint the tainted RelativePath value via the production taint-mint path.
    // relative_path → [ExternalUntrusted, PathRaw]; provenance_chain[0] == the
    // appended file_read event id (genuine-taint anchor, no stapling).
    let mut store = ValueStore::default();
    let claim = Claim {
        claim_type: "relative_path".into(),
        value: HOSTILE_PATH.into(),
    };
    let (read_event_id, read_hash, path_value_id) = {
        let locked = conn.lock().unwrap();
        mint_from_read(&locked, &mut store, session_id, &claim, None).expect("mint_from_read")
    };

    let mut last_event_id = read_event_id;
    let mut last_event_hash = read_hash;

    // `path` is FIRST so the executor blocks on the tainted routing-sensitive arg
    // before it ever resolves `contents` (a block short-circuits — `contents` is
    // never resolved on this path, so its handle need not resolve). Both args are
    // present so `validate_schema` (file.create requires {path, contents}) passes.
    let plan_node = PlanNode {
        sink: SinkId("file.create".into()),
        args: vec![
            PlanArg {
                name: "path".into(),
                value_id: path_value_id,
            },
            PlanArg {
                name: "contents".into(),
                value_id: ValueId::new(),
            },
        ],
    };

    let (mut server_end, _client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");
    dispatch_request(
        BrokerRequest::SubmitPlanNode { plan_node },
        &mut server_end,
        &conn,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root(),
    )
    .await
    .expect("dispatch_request must succeed once the block append is durable");

    // Simulate process exit: DROP the only connection handle so SQLite closes it
    // (WAL checkpointed) before anything reopens from the path alone.
    drop(conn);

    (db_path, session_id, read_event_id)
}

/// ACC-07 canonical proof — after-exit, DB-alone, anti-stapling sentinel.
///
/// Drops + reopens a file-backed hostile-block DB and reconstructs the genuine-
/// taint proof from the persisted DB ALONE, asserting IN THIS ORDER:
///   1. `verify_chain` passes FIRST (trust the chain before the anchor).
///   2. the persisted `sink_blocked` event carries `Some(anchor)`.
///   3. genuine-taint backstops: a `file_read` DAG event with
///      `id == anchor.read_event_id == anchor.provenance_chain[0]`, untrusted taint.
///   4. taint consistency `Event.taint == anchor.taint` + byte-exact literal.
///   5. NO effect executed (no `sink_executed`, no `email_send_stub`).
#[tokio::test]
async fn after_exit_db_alone_anti_stapling_sentinel() {
    let (db_path, session_id, read_event_id) = build_hostile_block_db("sentinel").await;

    // REOPEN from the path alone — the persisted DB is now the ONLY source of truth.
    let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen audit DB");
    let sid = session_id.to_string();

    // ── (1) verify_chain FIRST — trust the chain BEFORE trusting the anchor. ──
    assert!(
        verify_chain(&reopened, &sid),
        "verify_chain must pass on the REOPENED DB before the anchor is trusted \
         (the causal hash chain must survive process exit)"
    );

    // ── (2) the persisted sink_blocked event carries an anchor. ──
    let blocked = find_event_by_type(&reopened, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist in the reopened DAG");
    let anchor = blocked
        .anchor
        .as_ref()
        .expect("persisted sink_blocked event MUST carry Some(anchor) (Defect B guard)");

    // ── (3) genuine-taint backstops (value-lineage ↔ causal DAG, DESIGN §0). ──
    assert!(
        !anchor.provenance_chain.is_empty(),
        "anchor.provenance_chain must be non-empty (genuine-taint chain required)"
    );
    // anchor-internal edge:
    assert_eq!(
        anchor.read_event_id, anchor.provenance_chain[0],
        "anchor.read_event_id must equal anchor.provenance_chain[0]"
    );
    // value↔DAG edge: the provenance root is a REAL file_read event in the DAG...
    let file_read = find_event_by_type(&reopened, &sid, "file_read")
        .expect("query file_read")
        .expect("a file_read event must exist in the reopened DAG");
    assert_eq!(
        file_read.id, anchor.read_event_id,
        "the DAG file_read event id must equal anchor.read_event_id \
         (the anchor references a real, auditable event — not a fabricated UUID)"
    );
    assert_eq!(
        anchor.provenance_chain[0], file_read.id,
        "anchor.provenance_chain[0] must reference the real file_read event id"
    );
    // ...and that file_read event carries UNTRUSTED taint (re-derived via is_untrusted()).
    assert!(
        file_read.taint.iter().any(|t| t.is_untrusted()),
        "the anchored file_read event must carry untrusted taint (is_untrusted()) — \
         a stapled-taint impl would fail here"
    );
    // Durability sanity: the anchor root is the file_read minted during ReportClaims.
    assert_eq!(
        anchor.read_event_id, read_event_id,
        "anchor root must be the file_read minted upstream, recovered from disk"
    );
    // PathRaw is the specific untrusted label a workspace-derived path carries.
    assert!(
        file_read.taint.contains(&TaintLabel::PathRaw),
        "the anchored file_read event must carry the PathRaw label (workspace path)"
    );

    // ── (4) taint consistency + byte-exact literal (DESIGN §4 rule 6). ──
    assert_eq!(
        blocked.taint, anchor.taint,
        "persisted Event.taint must equal anchor.taint (DESIGN §4 rule 6)"
    );
    assert!(
        anchor.taint.iter().any(|t| t.is_untrusted()),
        "anchor.taint must itself carry an untrusted label"
    );
    assert_eq!(
        anchor.literal, HOSTILE_PATH,
        "anchor.literal must be the byte-exact hostile path (data at rest)"
    );
    // The blocked arg is the routing-sensitive file.create `path`.
    assert_eq!(anchor.sink.0, "file.create", "anchor.sink must be file.create");
    assert_eq!(anchor.arg, "path", "anchor.arg must be the routing-sensitive path");

    // ── (5) NO effect executed on the block path (T-07-54). ──
    assert!(
        find_event_by_type(&reopened, &sid, "sink_executed")
            .expect("query sink_executed")
            .is_none(),
        "block path must NOT record a sink_executed event (no effect on block)"
    );
    assert!(
        find_event_by_type(&reopened, &sid, "email_send_stub")
            .expect("query email_send_stub")
            .is_none(),
        "block path must NOT record an email_send_stub event"
    );

    drop(reopened);
    cleanup_db(&db_path);
}

/// Tamper-evidence (T-07-52): the durable anchor rides inside the HASHED `payload`
/// column, so mutating the anchor literal with a raw `UPDATE` must break the
/// SHA-256 chain. `verify_chain` recomputes each row's hash from the raw stored
/// `payload`, so a post-hoc edit is detected on reopen (codex #6: mutate the DB,
/// not memory).
#[tokio::test]
async fn tamper_evidence_mutating_payload_breaks_verify_chain() {
    let (db_path, session_id, _read_event_id) = build_hostile_block_db("tamper").await;
    let sid = session_id.to_string();

    // Baseline: the freshly persisted chain verifies before any tampering.
    {
        let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen audit DB");
        assert!(
            verify_chain(&reopened, &sid),
            "verify_chain must be TRUE before tampering (durable baseline)"
        );
    }

    // Tamper: mutate the REAL `payload` column — change the anchor's literal inside
    // the serialized Event payload. The anchor is part of the hashed payload, so
    // this MUST invalidate the recomputed hash.
    {
        let conn = open_audit_db(db_path.to_str().unwrap()).expect("reopen for tamper");
        let changed = conn
            .execute(
                "UPDATE events SET payload = REPLACE(payload, ?1, ?2) \
                 WHERE event_type = 'sink_blocked'",
                rusqlite::params![HOSTILE_PATH, "reports/harmless.txt"],
            )
            .expect("tamper UPDATE must execute");
        assert_eq!(
            changed, 1,
            "exactly one sink_blocked row must be tampered (the literal must be present in the payload)"
        );
    }

    // Reopen and assert the tamper is caught.
    {
        let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen after tamper");
        assert!(
            !verify_chain(&reopened, &sid),
            "verify_chain MUST return FALSE after the anchor literal was mutated in the \
             payload column — the durable anchor is tamper-evident (rides in the hashed payload)"
        );
    }

    cleanup_db(&db_path);
}
