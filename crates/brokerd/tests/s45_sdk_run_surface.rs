//! Phase 45 (SDK-01) M7 ANTI-LAUNDERING differential — the requirement proof
//! that file/stream/env content is minted TAINTED via the existing sole
//! broker taint-mint path, NEVER laundered through the trusted intent mint.
//!
//! # Why differential, and why through the REAL ProvideIntent arm (WG-1, T-45-01)
//!
//! The confirmed shipped laundering path: caprun's `--seed-from-file` content
//! REPLACES the positional intent-param → becomes the `recipient`
//! (`SendEmailSummary`) / `path` (`CreateFileFromReport`) sink arg; before this
//! plan the `server.rs` ProvideIntent arm minted it via `mint_from_intent` →
//! `UserTrusted` UNCONDITIONALLY, so the VALUE carried a FALSE `UserTrusted` and
//! I2 never fired on it. Only session-level I0 demotion protected it — but I0 is
//! per-session, and a per-literal file-derived value must Block even in an Active
//! session where an operator-typed literal in the SAME arg is Allowed.
//!
//! This test drives the ACTUAL production ProvideIntent dispatch arm
//! (`brokerd::server::dispatch_request`) for BOTH legs — NOT the mint helpers
//! directly — so it proves the ARM's per-literal ROUTING, not merely that the
//! mint functions produce the right taint:
//!
//!   * OPERATOR leg (`primary_file_derived = false`): the recipient is minted
//!     `UserTrusted` (via `mint_from_intent`) and, in an Active session, is
//!     Allowed in the routing-sensitive `to` arg.
//!   * FILE-DERIVED leg (`primary_file_derived = true`): the SAME recipient
//!     literal is minted `[ExternalUntrusted, EmailRaw]` (via `mint_from_read`
//!     in the arm) — a genuine `file_read` event + session-demote — and, in the
//!     SAME `to` arg with the SAME Active status and SAME policy, deterministically
//!     Blocks under I2. `primary_file_derived` is the SOLE variable.
//!
//! A block-everything I2 regression fails the operator leg (must Allow); a
//! blanket-allow / re-laundering regression fails the file-derived leg (must
//! Block). The file-derived value's `provenance_chain[0]` is asserted to root on
//! a REAL `file_read` event (genuine, non-stapled), and `verify_chain` holds.
//!
//! Host-portable by construction: decision-level over an in-memory audit DB +
//! ValueStore, no socket touched (the `UnixStream::pair` carries only the framed
//! IPC response, exactly as `proto_claims.rs`'s dispatch tests do on both macOS
//! and Linux) — so NO `#[cfg(target_os = "linux")]` gate is needed (a gate would
//! only make it a macOS no-op, reducing coverage).

#![cfg(test)]

use brokerd::audit::{append_event, find_event_by_id, open_audit_db, verify_chain};
use brokerd::proto::{BrokerRequest, BrokerResponse};
use brokerd::server::dispatch_request;
use brokerd::session::{create_session, persist_session};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel, ValueId};
use runtime_core::{Event, ExecutorDecision, PlanNode, SeedProvenance, SessionPolicy, SessionStatus};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

/// Fixed, non-secret test MAC key (mirrors the sibling broker integration tests).
const TEST_KEY: &[u8] = b"s45-sdk-run-surface-m7-differential-key";

/// The recipient literal held IDENTICAL across the operator and file-derived legs
/// — `primary_file_derived` (which mint path the arm routes to), not the bytes,
/// is the differentiator. `@`-bearing so the file-derived leg mints it as an
/// `email_address` claim (`[ExternalUntrusted, EmailRaw]`).
const RECIPIENT_LITERAL: &str = "attacker@evil.com";

/// Persist a fresh Active session and seed its `session_created` causal root so
/// subsequent mints thread onto a real chain head (letting `verify_chain` walk an
/// unbroken linear chain afterward). Returns `(session_id, root_event_id, root_hash)`.
fn seed_session(conn: &rusqlite::Connection) -> (Uuid, Uuid, String) {
    let session = create_session(Uuid::new_v4(), SeedProvenance::TrustedArg);
    persist_session(conn, &session).unwrap();
    assert_eq!(
        session.status,
        SessionStatus::Active,
        "sanity: session starts Active before any inbound taint"
    );
    let root = Event::new(
        Uuid::new_v4(),
        None,
        session.id,
        "broker".into(),
        "session_created".into(),
        Utc::now(),
        vec![],
    );
    let root_hash = append_event(conn, TEST_KEY, &root, None).unwrap();
    (session.id, root.id, root_hash)
}

/// Read one framed `BrokerResponse` from the client end of a `UnixStream::pair`.
async fn read_response(client_end: &mut tokio::net::UnixStream) -> BrokerResponse {
    let mut len_buf = [0u8; 4];
    client_end.read_exact(&mut len_buf).await.expect("read len");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    client_end.read_exact(&mut body).await.expect("read body");
    serde_json::from_slice(&body).expect("deserialize response")
}

/// Drive a single `ProvideIntent` through the REAL production dispatch arm and
/// return the primary (recipient) `ValueId` the arm minted. `primary_file_derived`
/// selects the arm's per-literal mint routing under test.
async fn provide_intent_via_arm(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    store: &mut ValueStore,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
    session_status: &Arc<Mutex<SessionStatus>>,
    ws_root: &Arc<adapter_fs::workspace::WorkspaceRoot>,
    intent: CaprunIntent,
    primary_file_derived: bool,
) -> ValueId {
    let (mut server_end, mut client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");
    let mut intent_provided = false;
    let mut fd_requested = false;
    let mut fd_request_count: u32 = 0;

    dispatch_request(
        BrokerRequest::ProvideIntent {
            intent,
            primary_file_derived,
        },
        &mut server_end,
        conn,
        TEST_KEY,
        session_id,
        last_event_id,
        last_event_hash,
        store,
        ws_root,
        session_status,
        // Policy-agnostic: allow_all permits every sink so the routing under test
        // (which MINT path the arm takes), not the policy gate, is isolated.
        &SessionPolicy::allow_all(),
        None,
        &mut intent_provided,
        &mut fd_requested,
        &mut fd_request_count,
    )
    .await
    .expect("ProvideIntent dispatch must succeed");

    match read_response(&mut client_end).await {
        BrokerResponse::IntentAccepted { value_id, .. } => value_id,
        other => panic!("expected IntentAccepted, got {other:?}"),
    }
}

/// A `SendEmailSummary` intent with the shared recipient literal.
fn email_intent() -> CaprunIntent {
    CaprunIntent::SendEmailSummary {
        recipient: RECIPIENT_LITERAL.to_string(),
        subject: "Workspace Summary".to_string(),
        body: "Please see the attached workspace summary.".to_string(),
    }
}

/// Build an `email.send` plan node routing `recipient` into the routing-sensitive
/// `to` arg — the SOLE arg, so the recipient's taint is the sole Block driver.
fn email_send_node(recipient: &ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("email.send".into()),
        args: vec![PlanArg {
            name: "to".into(),
            value_id: recipient.clone(),
        }],
    }
}

/// A fresh temp-dir workspace root (ProvideIntent never drives RequestFd, so any
/// valid dir anchors the root — mirrors `proto_claims.rs`).
fn ws_root() -> Arc<adapter_fs::workspace::WorkspaceRoot> {
    Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
            .expect("open ws root"),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// The M7 differential: `primary_file_derived` is the SOLE variable that flips a
// recipient from operator-trusted/Allowed to file-derived-tainted/I2-Blocked.
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn m7_operator_literal_is_trusted_and_allowed() {
    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let (session_id, root_id, root_hash) = {
        let locked = conn.lock().unwrap();
        seed_session(&locked)
    };
    let mut store = ValueStore::default();
    let mut last_event_id = root_id;
    let mut last_event_hash = root_hash;
    let session_status = Arc::new(Mutex::new(SessionStatus::Active));
    let ws = ws_root();

    // OPERATOR leg: primary_file_derived = false → mint_from_intent → UserTrusted.
    let recipient = provide_intent_via_arm(
        &conn,
        session_id,
        &mut store,
        &mut last_event_id,
        &mut last_event_hash,
        &session_status,
        &ws,
        email_intent(),
        false,
    )
    .await;

    // The minted recipient is genuinely trusted (NOT untrusted) — via the real arm.
    let rec = store.resolve(&recipient).expect("recipient resolves in store");
    assert_eq!(rec.literal, RECIPIENT_LITERAL);
    assert!(
        !rec.taint.iter().any(|t| t.is_untrusted()),
        "an operator-typed literal must be trusted (no untrusted label): {:?}",
        rec.taint
    );

    // In an Active session it is Allowed in the routing-sensitive `to` arg.
    let node = email_send_node(&recipient);
    let decision = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node,
        &store,
        &SessionStatus::Active,
        &SessionPolicy::broker_default(),
    );
    assert_eq!(
        decision,
        ExecutorDecision::Allowed,
        "an operator-typed recipient must be Allowed in the `to` arg (trusted)"
    );

    let locked = conn.lock().unwrap();
    assert!(
        verify_chain(&locked, &session_id.to_string(), TEST_KEY),
        "the audit chain must remain intact after the operator mint"
    );
}

#[tokio::test]
async fn m7_file_derived_literal_is_tainted_and_i2_blocks_with_genuine_anchor() {
    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let (session_id, root_id, root_hash) = {
        let locked = conn.lock().unwrap();
        seed_session(&locked)
    };
    let mut store = ValueStore::default();
    let mut last_event_id = root_id;
    let mut last_event_hash = root_hash;
    let session_status = Arc::new(Mutex::new(SessionStatus::Active));
    let ws = ws_root();

    // FILE-DERIVED leg: primary_file_derived = true → mint_from_read → TAINTED.
    let recipient = provide_intent_via_arm(
        &conn,
        session_id,
        &mut store,
        &mut last_event_id,
        &mut last_event_hash,
        &session_status,
        &ws,
        email_intent(),
        true,
    )
    .await;

    // The SAME recipient literal is now genuinely tainted — minted via the arm's
    // mint_from_read path as an email_address claim.
    let rec = store.resolve(&recipient).expect("recipient resolves in store");
    assert_eq!(
        rec.literal, RECIPIENT_LITERAL,
        "same literal as the operator leg — only its taint/provenance differs"
    );
    assert!(
        rec.taint.contains(&TaintLabel::ExternalUntrusted),
        "a file-derived literal must carry ExternalUntrusted: {:?}",
        rec.taint
    );
    assert!(
        rec.taint.contains(&TaintLabel::EmailRaw),
        "an @-bearing file-derived recipient must carry EmailRaw (email_address \
         claim_type): {:?}",
        rec.taint
    );

    // GENUINE, non-stapled anchor: provenance_chain[0] roots on a REAL file_read
    // event in the audit DAG (never a hand-set taint field).
    assert!(
        !rec.provenance_chain.is_empty(),
        "the file-derived value must carry a non-empty provenance chain"
    );
    let root_read_id = rec.provenance_chain[0];
    let root_event = {
        let locked = conn.lock().unwrap();
        find_event_by_id(&locked, &session_id.to_string(), &root_read_id.to_string())
            .expect("find_event_by_id")
            .expect("provenance_chain[0] must exist as a real event")
    };
    assert_eq!(
        root_event.event_type, "file_read",
        "the file-derived value's provenance must root on a genuine file_read event \
         (minted, not stapled)"
    );
    assert!(
        root_event.taint.contains(&TaintLabel::ExternalUntrusted),
        "the rooting file_read event itself must carry ExternalUntrusted taint"
    );

    // In the SAME Active session + SAME policy + SAME `to` arg, it deterministically
    // Blocks under I2 — proving it is minted TAINTED, not laundered (M7). Active
    // status isolates the Block as TAINT-driven (I2), not an I0 draft-session gate.
    let node = email_send_node(&recipient);
    let decision = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node,
        &store,
        &SessionStatus::Active,
        &SessionPolicy::broker_default(),
    );
    match &decision {
        ExecutorDecision::BlockedPendingConfirmation { anchors } => {
            assert_eq!(
                anchors.len(),
                1,
                "only the tainted recipient should Block; got {anchors:?}"
            );
            assert_eq!(
                anchors[0].anchor.arg, "to",
                "the Block anchor must name the routing-sensitive `to` arg"
            );
            assert_eq!(
                anchors[0].anchor.sink.0, "email.send",
                "the Block anchor must name the email.send sink"
            );
            // The anchor rides the SAME genuine provenance chain as the record.
            assert_eq!(
                anchors[0].anchor.read_event_id, anchors[0].anchor.provenance_chain[0],
                "anchor.read_event_id must equal provenance_chain[0] (genuine anchor)"
            );
            assert_eq!(
                anchors[0].anchor.provenance_chain[0], root_read_id,
                "the Block anchor must root on the same file_read event the record does"
            );
        }
        other => panic!(
            "a file-derived recipient must I2-Block in the `to` arg (minted TAINTED, \
             not laundered) — got {other:?}"
        ),
    }

    // The genuine audit DAG stays intact across the file_read + session_demoted mints.
    let locked = conn.lock().unwrap();
    assert!(
        verify_chain(&locked, &session_id.to_string(), TEST_KEY),
        "verify_chain must hold after the file-derived mint (unbroken taint chain)"
    );
}
