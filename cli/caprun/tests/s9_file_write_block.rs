//! s9_file_write_block — FS-03 per-requirement acceptance (cross-platform, NOT
//! Linux-gated): a genuine (non-stapled) taint chain, deterministically
//! Blocked when routed into `file.write`'s sensitive `path` or `contents` arg.
//!
//! # Scope
//!
//! Mirrors `s9_process_exec_block.rs`'s NON-live half: mint a tainted value
//! via the SOLE broker taint-mint site (`brokerd::quarantine::mint_from_read`),
//! route it into a `file.write` `PlanNode`, call the UNMODIFIED
//! `executor::submit_plan_node`, and assert `BlockedPendingConfirmation` with
//! an unbroken `provenance_chain[0]` anchor back to the originating `file_read`
//! event.
//!
//! Unlike `s9_process_exec_block.rs`, this file does NOT spawn a real process
//! or drive `invoke_file_write` (the live sink is a single in-broker
//! `openat2` with no child process, no launcher, no confinement machinery
//! to prove) — it exercises ONLY the executor's I2 decision, which is
//! identical on macOS and Linux (no `#[cfg(target_os = "linux")]` gate is
//! needed or wanted here; the standing project convention gates ONLY code
//! paths that differ between macOS stubs and real Linux enforcement, e.g.
//! `write_within`'s kernel syscall path — the executor's pure
//! `submit_plan_node` decision function has no such split).
//!
//! # What this proves (T-33-11)
//!
//! 1. A tainted, routing-sensitive `path` value → `BlockedPendingConfirmation`.
//! 2. A tainted, content-sensitive `contents` value → `BlockedPendingConfirmation`.
//! 3. Both blocks are genuine (non-stapled): the blocked anchor's
//!    `provenance_chain[0]`/`read_event_id` equal the REAL `file_read` audit
//!    event id `mint_from_read` durably appended — never a fresh/fabricated
//!    root (mirrors `s9_process_exec_block.rs`'s held-out genuine-taint
//!    backstop).
//! 4. A clean (`UserTrusted`) path+contents pair is NOT blocked (positive
//!    control) — the same trusted-value shape a real planner would submit.

use brokerd::audit::{append_event, find_event_by_type, open_audit_db, verify_chain};
use brokerd::quarantine::{mint_from_read, Claim};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel};
use runtime_core::{Event, ExecutorDecision, PlanNode, SessionStatus};
use uuid::Uuid;

/// Fixed, non-secret test MAC key (mirrors `s9_process_exec_block.rs`'s `TEST_KEY`).
const TEST_KEY: &[u8] = b"s9-file-write-block-rs-integration-test-key";

/// Mint a trusted literal directly into the store with the given `origin_role`
/// — mirrors the real planner's live `file.create`/`file.write` flow (DESIGN
/// §4.3), which reuses the SAME trusted `"path"`-role literal in both the
/// `path` and `contents` slots. A throwaway anchor Uuid stands in for a real
/// causal event id (these are the CLEAN-ALLOW control's own trusted inputs —
/// never the thing under genuine-taint-anchor test).
fn mint_trusted(store: &mut ValueStore, literal: &str, origin_role: &str) -> runtime_core::plan_node::ValueId {
    store
        .mint(
            literal.to_string(),
            vec![TaintLabel::UserTrusted],
            vec![Uuid::new_v4()],
            Some(origin_role.to_string()),
        )
        .expect("mint trusted literal")
}

/// Seed a `session_created` causal-root event so subsequent appends chain
/// onto a real parent (mirrors `s9_process_exec_block.rs::seed_root_event`).
fn seed_root_event(conn: &rusqlite::Connection, session_id: Uuid) -> (Uuid, String) {
    let root = Event::new(
        Uuid::new_v4(),
        None,
        session_id,
        "broker".into(),
        "session_created".into(),
        Utc::now(),
        vec![],
    );
    let hash = append_event(conn, TEST_KEY, &root, None).expect("append root event");
    (root.id, hash)
}

/// (a) FS-03 acceptance — a tainted `path` (routing-sensitive) Blocks via the
/// UNMODIFIED `submit_plan_node`, with a genuine (non-stapled) provenance
/// anchor back to the real `file_read` event `mint_from_read` durably
/// appended.
#[test]
fn s9_file_write_tainted_path_blocks_with_genuine_anchor() {
    let conn = open_audit_db(":memory:").expect("open_audit_db");
    let session_id = Uuid::new_v4();
    let (root_id, root_hash) = seed_root_event(&conn, session_id);

    let mut store = ValueStore::default();

    // Tainted `path` via the SOLE production genuine-taint mint site
    // (mint_from_read) — never a hand-set taint field. `claim_type:
    // "relative_path"` yields `origin_role = Some("relative_path")`, which
    // file.write's `path` slot admits (`expected_role` ==
    // `["path", "relative_path"]`).
    let claim = Claim {
        claim_type: "relative_path".into(),
        value: "../../etc/passwd".into(),
    };
    let (read_event_id, _read_hash, tainted_path_vid, demoted_id, demoted_hash) =
        mint_from_read(&conn, TEST_KEY, &mut store, session_id, &claim, Some(root_id), Some(&root_hash))
            .expect("mint_from_read");

    // The file_read event is durably in the audit DAG (not merely returned
    // in-memory) BEFORE we route it into a sensitive sink — mirrors
    // s9_process_exec_block.rs's anti-stapling DB re-check.
    {
        let dag_event = find_event_by_type(&conn, &session_id.to_string(), "file_read")
            .expect("query file_read")
            .expect("file_read event must exist in the audit DAG");
        assert_eq!(
            dag_event.id, read_event_id,
            "the file_read DAG event id must equal mint_from_read's returned id"
        );
    }

    // contents stays trusted here — this case isolates the tainted-`path` arg.
    let contents_vid = mint_trusted(&mut store, "irrelevant contents", "path");

    let plan_node = PlanNode {
        sink: SinkId("file.write".into()),
        args: vec![
            PlanArg { name: "path".into(), value_id: tainted_path_vid },
            PlanArg { name: "contents".into(), value_id: contents_vid },
        ],
    };
    let effect_id = Uuid::new_v4();
    let decision = executor::submit_plan_node(
        session_id,
        effect_id,
        &plan_node,
        &store,
        &SessionStatus::Active,
    );

    let anchor = match decision {
        ExecutorDecision::BlockedPendingConfirmation { anchors } => {
            assert_eq!(anchors.len(), 1, "exactly one blocked arg (path)");
            let blocked = anchors.into_iter().next().expect("one anchor");
            assert_eq!(blocked.anchor.arg, "path");
            assert_eq!(blocked.anchor.sink.0, "file.write");
            blocked.anchor
        }
        other => panic!(
            "expected BlockedPendingConfirmation for a tainted file.write path, got {other:?}"
        ),
    };

    // GENUINE-TAINT BACKSTOP (T-33-11): the anchor's provenance root is the
    // SAME file_read event id mint_from_read appended — not a fabricated
    // UUID, not a different event. A stapled-taint implementation would fail
    // here.
    assert_eq!(
        anchor.provenance_chain[0], read_event_id,
        "GENUINE-TAINT BACKSTOP: anchor.provenance_chain[0] must equal the \
         file_read event id (non-stapled)"
    );
    assert_eq!(
        anchor.read_event_id, read_event_id,
        "anchor.read_event_id must equal the file_read event id"
    );

    // Durably persist the block. `evaluate_plan_node_and_record` (the
    // production block-recording orchestration) is private to server.rs;
    // this inlines the SAME `Event::sink_blocked` + `append_event` call
    // shape s9_acceptance.rs/s9_process_exec_block.rs already establish as
    // the sanctioned in-process proof pattern for this exact constraint.
    //
    // Chain onto `demoted_id`/`demoted_hash` (mint_from_read's `chain_head_id`
    // /`chain_head_hash`) — NOT `read_event_id` — per mint_from_read's own doc
    // warning: using `read_event_id` here would make `sink_blocked` a SIBLING
    // of `session_demoted` (both children of `file_read`), forking the DAG and
    // breaking `verify_chain`'s single-linear-chain walk. The
    // genuine-taint-anchor assertions above (`provenance_chain[0]`/
    // `read_event_id`) are a SEPARATE value-lineage graph from this
    // causal-chain `parent_id` — never conflated.
    let block_event = Event::sink_blocked(
        Uuid::new_v4(),
        Some(demoted_id),
        session_id,
        Utc::now(),
        vec![anchor],
        None,
        vec!["path".to_string()],
    );
    append_event(&conn, TEST_KEY, &block_event, Some(&demoted_hash)).expect("append sink_blocked");

    let persisted_block = find_event_by_type(&conn, &session_id.to_string(), "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist");
    assert_eq!(persisted_block.id, block_event.id);
    assert!(
        find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .expect("query sink_executed")
            .is_none(),
        "no sink_executed event may exist — the block prevented any effect"
    );
    assert!(
        verify_chain(&conn, &session_id.to_string(), TEST_KEY),
        "verify_chain must be true — ONE unbroken causal chain: \
         session_created -> file_read -> session_demoted -> sink_blocked"
    );
}

/// (b) FS-03 acceptance — a tainted `contents` (content-sensitive) Blocks via
/// the UNMODIFIED `submit_plan_node`, with the same genuine-anchor guarantee.
#[test]
fn s9_file_write_tainted_contents_blocks_with_genuine_anchor() {
    let conn = open_audit_db(":memory:").expect("open_audit_db");
    let session_id = Uuid::new_v4();
    let (root_id, root_hash) = seed_root_event(&conn, session_id);

    let mut store = ValueStore::default();

    // path stays trusted here — this case isolates the tainted-`contents` arg.
    let path_vid = mint_trusted(&mut store, "notes/summary.txt", "path");

    // Tainted `contents` via the SOLE production genuine-taint mint site
    // (mint_from_read). `claim_type: "doc_fragment"` yields
    // `origin_role = Some("doc_fragment")`, which file.write's `contents`
    // slot admits (`expected_role` == `["path", "exec_output", "doc_fragment"]`).
    let claim = Claim {
        claim_type: "doc_fragment".into(),
        value: "hostile-exfil-payload".into(),
    };
    let (read_event_id, _read_hash, tainted_contents_vid, demoted_id, demoted_hash) =
        mint_from_read(&conn, TEST_KEY, &mut store, session_id, &claim, Some(root_id), Some(&root_hash))
            .expect("mint_from_read");

    {
        let dag_event = find_event_by_type(&conn, &session_id.to_string(), "file_read")
            .expect("query file_read")
            .expect("file_read event must exist in the audit DAG");
        assert_eq!(
            dag_event.id, read_event_id,
            "the file_read DAG event id must equal mint_from_read's returned id"
        );
    }

    let plan_node = PlanNode {
        sink: SinkId("file.write".into()),
        args: vec![
            PlanArg { name: "path".into(), value_id: path_vid },
            PlanArg { name: "contents".into(), value_id: tainted_contents_vid },
        ],
    };
    let effect_id = Uuid::new_v4();
    let decision = executor::submit_plan_node(
        session_id,
        effect_id,
        &plan_node,
        &store,
        &SessionStatus::Active,
    );

    let anchor = match decision {
        ExecutorDecision::BlockedPendingConfirmation { anchors } => {
            assert_eq!(anchors.len(), 1, "exactly one blocked arg (contents)");
            let blocked = anchors.into_iter().next().expect("one anchor");
            assert_eq!(blocked.anchor.arg, "contents");
            assert_eq!(blocked.anchor.sink.0, "file.write");
            blocked.anchor
        }
        other => panic!(
            "expected BlockedPendingConfirmation for tainted file.write contents, got {other:?}"
        ),
    };

    // GENUINE-TAINT BACKSTOP (T-33-11): non-stapled, real anchor.
    assert_eq!(
        anchor.provenance_chain[0], read_event_id,
        "GENUINE-TAINT BACKSTOP: anchor.provenance_chain[0] must equal the \
         file_read event id (non-stapled)"
    );
    assert_eq!(
        anchor.read_event_id, read_event_id,
        "anchor.read_event_id must equal the file_read event id"
    );

    // Chain onto `demoted_id`/`demoted_hash` (see case (a)'s comment above for
    // the full rationale) — NOT `read_event_id`.
    let block_event = Event::sink_blocked(
        Uuid::new_v4(),
        Some(demoted_id),
        session_id,
        Utc::now(),
        vec![anchor],
        None,
        vec!["contents".to_string()],
    );
    append_event(&conn, TEST_KEY, &block_event, Some(&demoted_hash)).expect("append sink_blocked");

    let persisted_block = find_event_by_type(&conn, &session_id.to_string(), "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist");
    assert_eq!(persisted_block.id, block_event.id);
    assert!(
        find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .expect("query sink_executed")
            .is_none(),
        "no sink_executed event may exist — the block prevented any effect"
    );
    assert!(
        verify_chain(&conn, &session_id.to_string(), TEST_KEY),
        "verify_chain must be true — ONE unbroken causal chain: \
         session_created -> file_read -> session_demoted -> sink_blocked"
    );
}

/// (c) CLEAN-ALLOW control — a fully trusted `path`+`contents` pair (mirroring
/// the real planner's `file.write` flow, which reuses the SAME trusted
/// `"path"`-role literal in both slots per DESIGN §4.3) is NOT blocked.
#[test]
fn s9_file_write_clean_trusted_pair_is_allowed() {
    let session_id = Uuid::new_v4();
    let mut store = ValueStore::default();

    let path_vid = mint_trusted(&mut store, "notes/summary.txt", "path");
    let contents_vid = mint_trusted(&mut store, "clean, trusted body text", "path");

    let plan_node = PlanNode {
        sink: SinkId("file.write".into()),
        args: vec![
            PlanArg { name: "path".into(), value_id: path_vid },
            PlanArg { name: "contents".into(), value_id: contents_vid },
        ],
    };
    let effect_id = Uuid::new_v4();
    let decision = executor::submit_plan_node(
        session_id,
        effect_id,
        &plan_node,
        &store,
        &SessionStatus::Active,
    );

    assert_eq!(
        decision,
        ExecutorDecision::Allowed,
        "CLEAN-ALLOW CONTROL: a trusted path/contents file.write must Allow"
    );
}
