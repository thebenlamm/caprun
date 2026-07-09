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

use brokerd::audit::{
    find_event_by_type, get_blocked_literal, open_audit_db, redact_blocked_literal, verify_chain,
};
use brokerd::proto::BrokerRequest;
use brokerd::quarantine::{mint_from_read, Claim};
use brokerd::server::dispatch_request;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId, TaintLabel};
use runtime_core::SessionStatus;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Lowercase-hex SHA-256 of a literal — mirrors the digest the executor writes
/// into `SinkBlockedAnchor.literal_sha256`.
fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

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
    let (read_event_id, _read_hash, path_value_id, demoted_event_id, demoted_hash) = {
        let locked = conn.lock().unwrap();
        mint_from_read(&locked, &mut store, session_id, &claim, None, None).expect("mint_from_read")
    };

    // Chain onto the session_demoted event (the LAST event mint_from_read
    // appended) — not the file_read event (`read_event_id`, still returned
    // below as the genuine-taint anchor identity) — to avoid forking the
    // causal DAG (see mint_from_read's doc comment).
    let mut last_event_id = demoted_event_id;
    let mut last_event_hash = demoted_hash;
    // mint_from_read above already demoted this session to Draft in the DB
    // (TAINT-01); this harness thread mirrors the broker's in-memory local
    // starting from an Active seed and preserves this test's existing
    // Active-session semantics (a block on I2 fires regardless of session_status).
    let mut session_status = SessionStatus::Active;

    // `path` is FIRST so the executor blocks on the tainted routing-sensitive arg
    // before it ever resolves `contents` for the ALLOW/DENY decision itself (a
    // block short-circuits the executor's own decision). Both args are present so
    // `validate_schema` (file.create requires {path, contents}) passes. `contents`
    // IS minted (trusted) into the store because the block arm (10-02) now builds a
    // full-arg-set `PendingConfirmation` snapshot at Block time, resolving EVERY
    // plan_node arg — not only the one the executor blocked on.
    let contents_value_id = store
        .mint(
            "hostile block harness contents".into(),
            vec![TaintLabel::UserTrusted],
            vec![read_event_id],
        )
        .expect("mint contents value");
    let plan_node = PlanNode {
        sink: SinkId("file.create".into()),
        args: vec![
            PlanArg {
                name: "path".into(),
                value_id: path_value_id,
            },
            PlanArg {
                name: "contents".into(),
                value_id: contents_value_id,
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
        &mut session_status,
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
        .anchors
        .first()
        .expect("persisted sink_blocked event MUST carry at least one anchor (Defect B guard)");

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
    // The hashed anchor carries only the DIGEST of the literal (redactable-at-rest
    // design). The digest must match the byte-exact hostile path...
    assert_eq!(
        anchor.literal_sha256,
        sha256_hex(HOSTILE_PATH),
        "anchor.literal_sha256 must be sha256(hostile path) — tamper-evident digest"
    );
    // ...and the byte-exact literal itself is recoverable from the redactable side
    // table (data at rest lives OUTSIDE the hashed chain), keyed by the event id.
    let side_literal = get_blocked_literal(&reopened, &blocked.id.to_string())
        .expect("query blocked_literals")
        .expect("a blocked-literal side-table row must exist for the sink_blocked event");
    assert_eq!(
        side_literal, HOSTILE_PATH,
        "blocked_literals row must hold the byte-exact hostile path (redactable data at rest)"
    );
    // Tamper cross-check: the side-table literal must hash to the anchor digest.
    assert_eq!(
        sha256_hex(&side_literal),
        anchor.literal_sha256,
        "sha256(side-table literal) must equal anchor.literal_sha256"
    );
    // CORE PROPERTY (the reason this fix exists): the raw literal must NOT be in the
    // hashed `payload` column — only its digest — so it stays redactable. This is
    // the falsification guard: reintroducing `literal` into the anchor would pass
    // the digest checks above but fail HERE.
    let raw_payload: String = reopened
        .query_row(
            "SELECT payload FROM events WHERE id = ?1",
            rusqlite::params![blocked.id.to_string()],
            |row| row.get(0),
        )
        .expect("query raw payload");
    assert!(
        !raw_payload.contains(HOSTILE_PATH),
        "the raw literal MUST NOT appear in the hashed payload (redactability requires \
         only the digest is chained)"
    );
    assert!(
        raw_payload.contains(&anchor.literal_sha256),
        "the payload must contain the literal digest (tamper-evident anchor)"
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

    // Tamper: mutate the REAL `payload` column — swap the anchor's literal DIGEST
    // inside the serialized Event payload. The digest is part of the hashed payload,
    // so this MUST invalidate the recomputed hash. (The raw literal is no longer in
    // the payload — only its sha256 digest is — so we tamper the digest.)
    {
        let conn = open_audit_db(db_path.to_str().unwrap()).expect("reopen for tamper");
        let real_digest = sha256_hex(HOSTILE_PATH);
        let forged_digest = sha256_hex("reports/harmless.txt");
        let changed = conn
            .execute(
                "UPDATE events SET payload = REPLACE(payload, ?1, ?2) \
                 WHERE event_type = 'sink_blocked'",
                rusqlite::params![real_digest, forged_digest],
            )
            .expect("tamper UPDATE must execute");
        assert_eq!(
            changed, 1,
            "exactly one sink_blocked row must be tampered (the digest must be present in the payload)"
        );
    }

    // Reopen and assert the tamper is caught.
    {
        let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen after tamper");
        assert!(
            !verify_chain(&reopened, &sid),
            "verify_chain MUST return FALSE after the anchor digest was mutated in the \
             payload column — the durable anchor is tamper-evident (rides in the hashed payload)"
        );
    }

    cleanup_db(&db_path);
}

/// Redactability (the tamper-evidence ↔ redactability reconciliation): the raw
/// blocked literal lives in the `blocked_literals` side table, NOT the hashed
/// chain. Deleting that row (redaction) removes the attacker content / PII while
/// leaving `verify_chain` TRUE and the anchor digest intact as proof-of-existence.
#[tokio::test]
async fn redacting_side_table_literal_preserves_verify_chain_and_digest() {
    let (db_path, session_id, _read_event_id) = build_hostile_block_db("redact").await;
    let sid = session_id.to_string();

    let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen audit DB");
    let blocked = find_event_by_type(&reopened, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist");
    let anchor = blocked
        .anchors
        .first()
        .expect("sink_blocked must carry an anchor");
    let event_id = blocked.id.to_string();

    // Pre-redaction: chain verifies, literal present, digest matches.
    assert!(verify_chain(&reopened, &sid), "chain must verify before redaction");
    let before = get_blocked_literal(&reopened, &event_id)
        .expect("query side table")
        .expect("literal present before redaction");
    assert_eq!(before, HOSTILE_PATH, "side-table literal must be the hostile path");
    assert_eq!(
        sha256_hex(&before),
        anchor.literal_sha256,
        "digest must match the literal before redaction"
    );
    let digest_before = anchor.literal_sha256.clone();

    // Redact: delete the side-table row.
    let removed = redact_blocked_literal(&reopened, &event_id).expect("redact");
    assert_eq!(removed, 1, "exactly one side-table row must be redacted");

    // Post-redaction: literal is GONE, but the chain still verifies and the digest
    // in the (unmodified) hashed anchor remains as proof content of that hash existed.
    assert!(
        get_blocked_literal(&reopened, &event_id)
            .expect("query side table")
            .is_none(),
        "the raw literal must be gone after redaction"
    );
    assert!(
        verify_chain(&reopened, &sid),
        "verify_chain MUST stay TRUE after redaction — the hashed chain was untouched"
    );
    let blocked_after = find_event_by_type(&reopened, &sid, "sink_blocked")
        .expect("re-query sink_blocked")
        .expect("sink_blocked still present");
    assert_eq!(
        blocked_after
            .anchors
            .first()
            .expect("anchor still present")
            .literal_sha256,
        digest_before,
        "the anchor digest must survive redaction as proof-of-existence"
    );

    // Redaction is idempotent.
    assert_eq!(
        redact_blocked_literal(&reopened, &event_id).expect("re-redact"),
        0,
        "redacting an already-absent literal must remove 0 rows (idempotent)"
    );

    drop(reopened);
    cleanup_db(&db_path);
}

/// 10-02 (Task 2): a `BlockedPendingConfirmation` durably persists a full-snapshot
/// `PendingConfirmation` row ATOMICALLY with its `sink_blocked` event, keyed by the
/// same `effect_id` — reconstructed from the reopened DB alone (no in-memory state).
#[tokio::test]
async fn pending_confirmation_persisted_atomically_with_block() {
    use brokerd::confirmation::{find_pending_confirmation, PendingConfirmationState};

    let (db_path, session_id, _read_event_id) = build_hostile_block_db("pending_conf").await;
    let sid = session_id.to_string();

    let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen audit DB");

    let blocked = find_event_by_type(&reopened, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist");
    let anchor = blocked
        .anchors
        .first()
        .expect("sink_blocked must carry an anchor");

    let pc = find_pending_confirmation(&reopened, &anchor.effect_id.to_string())
        .expect("find_pending_confirmation")
        .expect("a pending_confirmations row must exist keyed by anchor.effect_id");

    assert_eq!(
        pc.effect_id, anchor.effect_id,
        "PendingConfirmation.effect_id must equal the sink_blocked anchor's effect_id"
    );
    assert_eq!(
        pc.blocked_event_id, blocked.id,
        "PendingConfirmation.blocked_event_id must equal the sink_blocked event's id"
    );
    assert_eq!(pc.session_id, session_id);
    assert_eq!(pc.sink.0, "file.create");
    assert_eq!(pc.state, PendingConfirmationState::Pending);
    assert_eq!(
        pc.workspace_root_path,
        ws_root().root_path().to_string_lossy(),
        "workspace_root_path must equal the workspace root the broker opened"
    );

    // The FULL arg set is captured — both `path` (the blocked, tainted arg) and
    // `contents` (never resolved by the executor's own decision, but frozen here).
    assert_eq!(
        pc.resolved_args.len(),
        2,
        "resolved_args must contain one entry per plan_node.args entry"
    );
    let path_arg = pc
        .resolved_args
        .iter()
        .find(|a| a.name == "path")
        .expect("path arg present in snapshot");
    assert_eq!(path_arg.literal, HOSTILE_PATH);
    let contents_arg = pc
        .resolved_args
        .iter()
        .find(|a| a.name == "contents")
        .expect("contents arg present in snapshot");
    assert_eq!(contents_arg.literal, "hostile block harness contents");

    drop(reopened);
    cleanup_db(&db_path);
}

/// Phase 16, Task 2 (CONFIRM-03, DESIGN-confirm-binding.md Round-6): a genuine
/// block over a plan node with a TRUSTED arg (`contents`) and a TAINTED arg
/// (`path`) durably records ONE combined digest over the FULL `resolved_args`
/// set — not just the blocked subset — identically in the hash-chained
/// `sink_blocked` Event payload and the mirrored `PendingConfirmation` row,
/// reconstructed from the reopened DB alone.
#[tokio::test]
async fn combined_digest_covers_full_set_and_matches_between_event_and_pending_confirmation() {
    use brokerd::confirmation::{combined_digest, find_pending_confirmation};

    let (db_path, session_id, _read_event_id) = build_hostile_block_db("combined_digest").await;
    let sid = session_id.to_string();

    let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen audit DB");

    let blocked = find_event_by_type(&reopened, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist");
    let anchor = blocked
        .anchors
        .first()
        .expect("sink_blocked must carry an anchor");

    let pc = find_pending_confirmation(&reopened, &anchor.effect_id.to_string())
        .expect("find_pending_confirmation")
        .expect("a pending_confirmations row must exist");

    // (a) The Event and the PendingConfirmation carry IDENTICAL combined_digest
    // + blocked_arg_names — one computation, mirrored into both under the
    // same Block-time write (no drift, no second computation).
    let event_digest = blocked
        .combined_digest
        .as_ref()
        .expect("sink_blocked Event must carry a combined_digest (Phase 16)");
    assert_eq!(
        event_digest, &pc.combined_digest,
        "Event.combined_digest must equal PendingConfirmation.combined_digest"
    );
    assert_eq!(
        blocked.blocked_arg_names, pc.blocked_arg_names,
        "Event.blocked_arg_names must equal PendingConfirmation.blocked_arg_names"
    );

    // (b) The digest equals combined_digest recomputed independently over
    // ALL of resolved_args' (arg_name, literal) pairs — the FULL set (both
    // `path` and `contents`), NOT the blocked subset — proving the digest's
    // domain is every current resolved_args element (BLOCKER-2 widening).
    assert_eq!(
        pc.resolved_args.len(),
        2,
        "sanity: this fixture has exactly two resolved_args (path, contents)"
    );
    let full_set_pairs: Vec<(&str, &str)> = pc
        .resolved_args
        .iter()
        .map(|a| (a.name.as_str(), a.literal.as_str()))
        .collect();
    let recomputed_full_set = combined_digest(&full_set_pairs);
    assert_eq!(
        &recomputed_full_set, event_digest,
        "combined_digest recomputed over the FULL resolved_args set must equal \
         the persisted digest — the domain is every current element, not just \
         the blocked subset"
    );

    // Falsification: a digest computed over ONLY the blocked subset (`path`)
    // must NOT equal the persisted full-set digest — proving the persisted
    // value genuinely covers the trusted `contents` arg too, not merely the
    // blocked one (this is the exact BLOCKER-2 hole this widening closes).
    let blocked_subset_only: Vec<(&str, &str)> = pc
        .resolved_args
        .iter()
        .filter(|a| a.name == "path")
        .map(|a| (a.name.as_str(), a.literal.as_str()))
        .collect();
    let blocked_subset_digest = combined_digest(&blocked_subset_only);
    assert_ne!(
        &blocked_subset_digest, event_digest,
        "a digest over the blocked subset ONLY must differ from the persisted \
         full-set digest — otherwise a side-table rewrite of the trusted \
         `contents` arg would go undetected (BLOCKER-2)"
    );

    // (c) blocked_arg_names is the ordered BLOCKED subset ONLY (display-
    // marking metadata) — `path` (tainted, routing-sensitive), never
    // `contents` (trusted, untainted).
    assert_eq!(
        pc.blocked_arg_names,
        vec!["path".to_string()],
        "blocked_arg_names must be exactly the blocked subset (`path`), not the \
         full resolved_args name set"
    );

    drop(reopened);
    cleanup_db(&db_path);
}
