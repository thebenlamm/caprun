//! phase5_dispatch — cross-platform tests for the unified broker dispatch.
//!
//! These tests exercise the Phase 5 security-critical core WITHOUT any
//! Linux-only confinement primitives (no abstract sockets, no apply_confinement),
//! so they run on the macOS dev box as well as Linux. Where a live socket is
//! needed, `tokio::net::UnixStream::pair()` provides a connected pair that works
//! cross-platform. Where the public dispatch surface is awkward to drive, the
//! tests call the SAME production functions the handler calls (`mint_from_read`,
//! `executor::submit_plan_node`, `append_event`) — exercising the real code path
//! without re-implementing taint logic.
//!
//! Coverage:
//!   1. ASM-04  — mint anchors provenance_chain[0] to the real file_read event.
//!   2. HARD-03 — a ValueId minted in one store resolves None (→ Denied) in another.
//!   3. HARD-03 — SubmitPlanNode carries no session_id; evaluation uses the
//!                connection-established identity / per-connection store.
//!   4. ACC-02  — a Block produces a durable sink_blocked event causally parented
//!                onto the prior (file_read) event, queryable in the DAG.
//!   5. ACC-02  — if the audit append fails, the handler returns Err (the block
//!                is NEVER reported as completed) — fail-closed.

use brokerd::audit::{find_event_by_type, open_audit_db};
use brokerd::proto::{BrokerRequest, WorkerClaim};
use brokerd::quarantine::{mint_from_read, Claim};
use brokerd::server::dispatch_request;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId};
use runtime_core::ExecutorDecision;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const HOSTILE_ADDR: &str = "accounts@ev1l.com";

/// Build an Arc<Mutex<Connection>> backed by an in-memory audit DB with schema.
fn shared_db() -> Arc<Mutex<rusqlite::Connection>> {
    Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")))
}

/// A scripted email.send PlanNode routing `to` through the given handle.
fn email_plan(value_id: runtime_core::plan_node::ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("email.send".into()),
        args: vec![PlanArg {
            name: "to".into(),
            value_id,
        }],
    }
}

// ---------------------------------------------------------------------------
// 1. ASM-04 — mint anchors provenance_chain[0] to the real file_read event.
// ---------------------------------------------------------------------------

#[test]
fn mint_anchors_provenance_to_file_read_event() {
    let conn = open_audit_db(":memory:").expect("open_audit_db");
    let mut store = ValueStore::default();
    let session_id = Uuid::new_v4();
    let claim = Claim {
        claim_type: "email_address".into(),
        value: HOSTILE_ADDR.into(),
    };

    let (read_event_id, _hash, value_id) =
        mint_from_read(&conn, &mut store, session_id, &claim, None).expect("mint_from_read");

    // The resolved record's provenance chain anchors to the returned read event id.
    let record = store.resolve(&value_id).expect("value_id resolves");
    assert_eq!(
        record.provenance_chain[0], read_event_id,
        "provenance_chain[0] must equal the minted file_read event id (no stapling)"
    );

    // And that id is a real file_read event in the audit DAG.
    let evt = find_event_by_type(&conn, &session_id.to_string(), "file_read")
        .expect("find_event_by_type")
        .expect("file_read event present in DAG");
    assert_eq!(
        evt.id, read_event_id,
        "audit DAG file_read id must equal the anchor id"
    );
}

// ---------------------------------------------------------------------------
// 2. HARD-03 — a handle minted in store A resolves None (→ Denied) in store B.
// ---------------------------------------------------------------------------

#[test]
fn handle_from_other_connection_store_is_denied() {
    let conn = open_audit_db(":memory:").expect("open_audit_db");
    let session_id = Uuid::new_v4();

    // Connection A mints a tainted value into its own store.
    let mut store_a = ValueStore::default();
    let claim = Claim {
        claim_type: "email_address".into(),
        value: HOSTILE_ADDR.into(),
    };
    let (_id, _hash, value_id) =
        mint_from_read(&conn, &mut store_a, session_id, &claim, None).expect("mint_from_read");

    // Sanity: in store A the same plan blocks (the value is tainted + routing-sensitive).
    let decision_a = executor::submit_plan_node(session_id, &email_plan(value_id.clone()), &store_a);
    assert!(
        matches!(decision_a, ExecutorDecision::BlockedPendingConfirmation { .. }),
        "store A must block the tainted recipient, got {decision_a:?}"
    );

    // Connection B has a DISTINCT empty store — the same handle does not resolve.
    let store_b = ValueStore::default();
    let decision_b = executor::submit_plan_node(session_id, &email_plan(value_id), &store_b);
    assert!(
        matches!(decision_b, ExecutorDecision::Denied { .. }),
        "cross-connection handle must resolve None → Denied, got {decision_b:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. HARD-03 — SubmitPlanNode carries no session_id (field removed).
// ---------------------------------------------------------------------------

#[test]
fn submit_plan_node_has_no_session_id_field() {
    // Compile-time + serde proof: the variant is constructible with ONLY plan_node,
    // and its serialized form contains no "session_id" key. The broker therefore
    // cannot receive (or trust) a message-supplied session identity.
    let req = BrokerRequest::SubmitPlanNode {
        plan_node: email_plan(runtime_core::plan_node::ValueId::new()),
    };
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(
        !json.contains("session_id"),
        "SubmitPlanNode wire form must not carry a session_id (HARD-03): {json}"
    );

    // Round-trips back to an equal value (no dropped/renamed fields).
    let back: BrokerRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back, "SubmitPlanNode must serde round-trip");
}

// ---------------------------------------------------------------------------
// 4. ACC-02 — a Block yields a durable sink_blocked event causally parented
//             onto the prior (file_read) event, queryable in the DAG.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn block_appends_durable_causal_sink_blocked() {
    let conn = shared_db();
    let session_id = Uuid::new_v4();
    let mut store = ValueStore::default();

    // Mint a tainted value through the real path; capture the read event as the
    // expected causal parent of the sink_blocked event.
    let claim = Claim {
        claim_type: "email_address".into(),
        value: HOSTILE_ADDR.into(),
    };
    let (read_event_id, read_hash, value_id) = {
        let locked = conn.lock().unwrap();
        mint_from_read(&locked, &mut store, session_id, &claim, None).expect("mint_from_read")
    };

    let mut last_event_id = read_event_id;
    let mut last_event_hash = read_hash;

    // Drive the real dispatch_request SubmitPlanNode arm over a UnixStream pair.
    let (mut server_end, _client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");
    dispatch_request(
        BrokerRequest::SubmitPlanNode {
            plan_node: email_plan(value_id),
        },
        &mut server_end,
        &conn,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
    )
    .await
    .expect("dispatch_request must succeed once the append is durable");

    // The sink_blocked event is durably recorded and parented onto the file_read event.
    let blocked = {
        let locked = conn.lock().unwrap();
        find_event_by_type(&locked, &session_id.to_string(), "sink_blocked")
            .expect("find_event_by_type")
            .expect("sink_blocked event must be durable in the DAG")
    };
    assert_eq!(
        blocked.parent_id,
        Some(read_event_id),
        "sink_blocked must be causally parented onto the prior (file_read) event"
    );
    // The chain head advanced to the sink_blocked event.
    assert_eq!(
        last_event_id, blocked.id,
        "dispatch must advance the chain head to the sink_blocked event"
    );
}

// ---------------------------------------------------------------------------
// 5. ACC-02 — if the audit append fails, the handler returns Err (fail-closed):
//             the block is NEVER reported to the worker as completed.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn append_failure_is_fail_closed() {
    let conn = shared_db();
    let session_id = Uuid::new_v4();
    let mut store = ValueStore::default();

    // Mint a tainted value (file_read appends fine while the table exists).
    let claim = Claim {
        claim_type: "email_address".into(),
        value: HOSTILE_ADDR.into(),
    };
    let (read_event_id, read_hash, value_id) = {
        let locked = conn.lock().unwrap();
        mint_from_read(&locked, &mut store, session_id, &claim, None).expect("mint_from_read")
    };

    // Sanity: the file_read anchor exists before we break the DAG.
    {
        let locked = conn.lock().unwrap();
        let exists = find_event_by_type(&locked, &session_id.to_string(), "file_read")
            .unwrap()
            .is_some();
        assert!(exists, "file_read must exist before inducing the append failure");
    }

    // Induce an append failure for the sink_blocked write: drop the events table.
    {
        let locked = conn.lock().unwrap();
        locked.execute("DROP TABLE events", []).expect("drop events table");
    }

    let mut last_event_id = read_event_id;
    let mut last_event_hash = read_hash;

    let (mut server_end, _client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");
    let result = dispatch_request(
        BrokerRequest::SubmitPlanNode {
            plan_node: email_plan(value_id),
        },
        &mut server_end,
        &conn,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
    )
    .await;

    // Fail-closed: the audit append failed, so the handler returns Err and the
    // block is NOT reported as completed. The chain head must NOT have advanced.
    assert!(
        result.is_err(),
        "audit append failure must propagate as Err (fail-closed) — block not reported durable"
    );
    assert_eq!(
        last_event_id, read_event_id,
        "chain head must NOT advance when the durable append failed"
    );
}

// Ensure the ReportClaims wire type compiles against the public proto surface
// (the worker constructs exactly this variant in Plan 03).
#[test]
fn report_claims_variant_constructs() {
    let req = BrokerRequest::ReportClaims {
        claims: vec![WorkerClaim::EmailAddress(HOSTILE_ADDR.into())],
    };
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(json.contains(HOSTILE_ADDR), "claim address must serialize");
}
