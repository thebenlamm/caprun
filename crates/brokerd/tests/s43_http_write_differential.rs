//! Phase 43 (HTTP-W-01) DIFFERENTIAL acceptance test — the requirement proof for
//! the `http.request.write` (POST/PUT) WRITE egress.
//!
//! # Why differential, not "not blocked" (DESIGN-v1.9-egress-policy §2.5, `[rev: M4]`)
//!
//! Acceptance for HTTP-W-01 is DIFFERENTIAL: with host, url, method, and policy
//! held BYTE-IDENTICAL, a TAINTED request `body` MUST Block under I2 while a CLEAN
//! `body` MUST be Allowed AND reach the live write dispatch. Taint is the SOLE
//! variable. This is the anti-stapling / anti-regression property:
//!
//!   * a block-everything I2 regression fails the clean (LEG C) leg — the clean
//!     body must Allow, not Block;
//!   * a blanket-allow regression fails the tainted (LEG B) leg — the tainted body
//!     must Block, not Allow;
//!   * so a "not blocked" test alone (which a block-everything regression passes
//!     vacuously) is insufficient, and this test is not that (T-43-12).
//!
//! The tainted body is minted through the REAL broker mint path (`mint_from_http`,
//! a genuine `http_response_received`-rooted provenance chain) — NEVER a hand-set
//! taint field — so the Block rides a genuine audit-DAG edge, not a tag stapled at
//! the sink (T-43-13, §9 anti-stapling).
//!
//! # Legs (Task 1, decision-level — HOST-PORTABLE)
//!
//!   * LEG A (I0): a draft / untrusted-seeded session submitting `http.request.write`
//!     is Denied `DraftOnlySessionDeniesCommitIrreversible` — proving the distinct
//!     WRITE id is `CommitIrreversible` (the MAJOR-1 I0-escape fix; the GET id is
//!     Observe and would fall through to Allowed even in a draft session).
//!   * LEG B (tainted body Blocks): Active session, url/method/policy fixed, a body
//!     minted UNTRUSTED → `BlockedPendingConfirmation` whose anchor names the `body`
//!     arg specifically (not a blanket sink block).
//!   * LEG C (clean body Allowed): the SAME Active session shape, SAME
//!     url/method/policy, a body minted `UserTrusted` — the ONLY difference from
//!     LEG B is the body's taint → Allowed.
//!   * LEG D (method enum): the same clean-body node but with a `method` literal
//!     outside `{POST,PUT}` → Denied `InvalidMethod` fail-closed.
//!
//! These decision-level legs drive the REAL executor (`executor::submit_plan_node`)
//! over the REAL broker value_store + `SessionPolicy::broker_default()` (the new
//! sink is policy-permitted). They are host-portable by construction — pure
//! decision function over an in-memory audit db + ValueStore, no socket. The
//! dispatch-level differential (clean leg REACHES egress, tainted leg does not) is
//! Task 2 below, Linux-gated where it touches a socket.

#![cfg(test)]

use brokerd::audit::{append_event, open_audit_db, verify_chain};
use brokerd::quarantine::{mint_from_http, mint_from_intent};
use brokerd::session::{create_session, persist_session};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, SinkId, ValueId};
use runtime_core::{
    Event, ExecutorDecision, PlanNode, SeedProvenance, SessionPolicy, SessionStatus,
};
use uuid::Uuid;

/// Fixed, non-secret test MAC key (mirrors the sibling broker integration tests).
const TEST_KEY: &[u8] = b"s43-http-write-differential-test-key-not-secret";

/// The write URL held IDENTICAL across the tainted (B) and clean (C) legs — taint,
/// not host/url, is the differentiator.
const WRITE_URL: &str = "https://mock-write.caprun.test/ingest";
/// The write verb held IDENTICAL across B and C.
const WRITE_METHOD: &str = "POST";
/// The request-body literal held IDENTICAL across B and C — only its TAINT differs
/// (minted untrusted for B via `mint_from_http`, minted `UserTrusted` for C via
/// `mint_from_intent`), making taint the byte-provable sole variable.
const BODY_LITERAL: &str = "{\"summary\":\"quarterly figures for review\"}";

/// Open an in-memory audit db, persist a fresh Active session, and seed a
/// `session_created` causal root so subsequent mints thread onto a real chain head
/// (letting `verify_chain` walk an unbroken linear chain afterward). Mirrors the
/// s37 setup shape.
fn setup() -> (
    rusqlite::Connection,
    ValueStore,
    Uuid,   // session id
    Uuid,   // chain head event id
    String, // chain head hash
) {
    let conn = open_audit_db(":memory:").unwrap();
    let store = ValueStore::default();
    let session = create_session(Uuid::new_v4(), SeedProvenance::TrustedArg);
    persist_session(&conn, &session).unwrap();
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
    let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();
    (conn, store, session.id, root.id, root_hash)
}

/// Mint a CLEAN (`UserTrusted`) value through the REAL broker UserTrusted mint path
/// (`mint_from_intent`), threading the causal chain head forward. Returns the value
/// handle plus the new chain head (id, hash). `origin_role` is `None`:
/// `http.request.write`'s url/body/method slots are all role-unconstrained
/// (`expected_role == None`), so the Step-1c role gate is a no-op here.
fn mint_clean(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    literal: &str,
    parent_id: Uuid,
    parent_hash: &str,
) -> (ValueId, Uuid, String) {
    let (event_id, hash, value_id) = mint_from_intent(
        conn,
        TEST_KEY,
        store,
        session_id,
        literal.to_string(),
        Some(parent_id),
        Some(parent_hash),
        None,
    )
    .expect("mint_from_intent (clean UserTrusted) must succeed");
    (value_id, event_id, hash)
}

/// Mint a genuinely-TAINTED body through the REAL broker http-taint mint path
/// (`mint_from_http`) — an untrusted-on-arrival response body re-POSTed is the
/// canonical exfil shape. `provenance_chain[0]` is a genuine
/// `http_response_received` event (NON-STAPLED). Returns the value handle plus the
/// new chain head (the LAST appended `session_demoted` event, per the mint's
/// documented parent-forking contract). NOTE: this demotes the persisted session to
/// Draft (I1) — the decision-level legs pass `SessionStatus::Active` EXPLICITLY to
/// isolate the Block as TAINT-driven (I2), not a draft-session gate.
fn mint_tainted_body(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    literal: &str,
    parent_id: Uuid,
    parent_hash: &str,
) -> (ValueId, Uuid, String) {
    let (_event_id, _event_hash, value_id, chain_head_id, chain_head_hash) = mint_from_http(
        conn,
        TEST_KEY,
        store,
        session_id,
        literal.to_string(),
        Some(parent_id),
        Some(parent_hash),
    )
    .expect("mint_from_http (untrusted body) must succeed");
    (value_id, chain_head_id, chain_head_hash)
}

/// Build an `http.request.write` plan node from the three arg handles.
fn write_node(url: &ValueId, method: &ValueId, body: &ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("http.request.write".into()),
        args: vec![
            PlanArg { name: "url".into(), value_id: url.clone() },
            PlanArg { name: "method".into(), value_id: method.clone() },
            PlanArg { name: "body".into(), value_id: body.clone() },
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LEG A (I0): a draft / untrusted-seeded session I0-denies the WRITE.
// ─────────────────────────────────────────────────────────────────────────────

/// A `http.request.write` submitted while the session is `Draft` — with a CLEAN
/// body, a valid POST method, and no tainted arg (so no I2 Block pre-empts) —
/// Denies `DraftOnlySessionDeniesCommitIrreversible`. This proves the DISTINCT
/// WRITE id is classed `CommitIrreversible` (the MAJOR-1 I0-escape fix): the GET
/// `http.request` id is `Observe` and would fall THROUGH to Allowed in a draft
/// session; the WRITE id must not.
#[test]
fn leg_a_i0_draft_session_denies_write_commit_irreversible() {
    let (conn, mut store, session_id, head_id, head_hash) = setup();

    let (url, head_id, head_hash) = mint_clean(&conn, &mut store, session_id, WRITE_URL, head_id, &head_hash);
    let (method, head_id, head_hash) =
        mint_clean(&conn, &mut store, session_id, WRITE_METHOD, head_id, &head_hash);
    let (body, _head_id, _head_hash) =
        mint_clean(&conn, &mut store, session_id, BODY_LITERAL, head_id, &head_hash);

    let node = write_node(&url, &method, &body);
    let decision = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node,
        &store,
        // Draft: the I0 class-level deny under test. Clean args + valid method mean
        // no I2 Block and no method Deny pre-empt it — the Draft+CommitIrreversible
        // gate is the sole reason.
        &SessionStatus::Draft,
        &SessionPolicy::broker_default(),
    );

    match decision {
        ExecutorDecision::Denied {
            reason: runtime_core::DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink },
        } => {
            assert_eq!(
                sink.0, "http.request.write",
                "the I0 deny must name the distinct WRITE sink id (CommitIrreversible)"
            );
        }
        other => panic!(
            "a draft-session http.request.write must Deny \
             DraftOnlySessionDeniesCommitIrreversible (I0) — got {other:?}"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LEGS B + C (the differential core): taint is the SOLE variable.
// ─────────────────────────────────────────────────────────────────────────────

/// The anti-stapling / anti-regression differential (§2.5, M4, T-43-12/T-43-13):
/// url, method, and policy are held BYTE-IDENTICAL across LEG B and LEG C (the
/// SAME url/method value handles are reused in both plan nodes; the SAME
/// `broker_default()` policy and `Active` status drive both). The ONLY difference
/// is the body's TAINT — tainted for B (minted via the real `mint_from_http`
/// path), clean for C (minted `UserTrusted`). LEG B Blocks on the `body` arg
/// specifically; LEG C is Allowed. A block-everything I2 regression would fail
/// LEG C; a blanket-allow regression would fail LEG B.
#[test]
fn legs_b_and_c_taint_is_the_sole_variable() {
    let (conn, mut store, session_id, head_id, head_hash) = setup();

    // Shared, IDENTICAL routing across both legs: one url handle, one method handle.
    let (url, head_id, head_hash) = mint_clean(&conn, &mut store, session_id, WRITE_URL, head_id, &head_hash);
    let (method, head_id, head_hash) =
        mint_clean(&conn, &mut store, session_id, WRITE_METHOD, head_id, &head_hash);

    // LEG B body: TAINTED, minted through the REAL http-taint path (genuine,
    // non-stapled `http_response_received`-rooted provenance).
    let (body_tainted, head_id, head_hash) =
        mint_tainted_body(&conn, &mut store, session_id, BODY_LITERAL, head_id, &head_hash);
    // LEG C body: CLEAN UserTrusted, SAME literal — only the taint differs.
    let (body_clean, _head_id, _head_hash) =
        mint_clean(&conn, &mut store, session_id, BODY_LITERAL, head_id, &head_hash);

    let node_b = write_node(&url, &method, &body_tainted);
    let node_c = write_node(&url, &method, &body_clean);

    // ── The "taint is the sole variable" property, asserted LITERALLY ──
    // A future change that diverges the url/method across the two legs fails here.
    let b_url = node_b.args.iter().find(|a| a.name == "url").unwrap();
    let c_url = node_c.args.iter().find(|a| a.name == "url").unwrap();
    let b_method = node_b.args.iter().find(|a| a.name == "method").unwrap();
    let c_method = node_c.args.iter().find(|a| a.name == "method").unwrap();
    assert_eq!(
        b_url.value_id, c_url.value_id,
        "url handle must be BYTE-IDENTICAL across the tainted and clean legs"
    );
    assert_eq!(
        b_method.value_id, c_method.value_id,
        "method handle must be BYTE-IDENTICAL across the tainted and clean legs"
    );
    // And the resolved literals are equal (defense in depth over the handle check).
    assert_eq!(
        store.resolve(&b_url.value_id).unwrap().literal,
        store.resolve(&c_url.value_id).unwrap().literal,
        "url literal must be identical across legs"
    );
    assert_eq!(store.resolve(&b_url.value_id).unwrap().literal, WRITE_URL);
    assert_eq!(
        store.resolve(&b_method.value_id).unwrap().literal,
        store.resolve(&c_method.value_id).unwrap().literal,
        "method literal must be identical across legs"
    );
    assert_eq!(store.resolve(&b_method.value_id).unwrap().literal, WRITE_METHOD);
    // The body handles DIFFER, and taint is the difference: B untrusted, C not.
    let b_body = node_b.args.iter().find(|a| a.name == "body").unwrap();
    let c_body = node_c.args.iter().find(|a| a.name == "body").unwrap();
    assert_ne!(
        b_body.value_id, c_body.value_id,
        "the body handle is the SOLE differing arg between the two legs"
    );
    assert_eq!(
        store.resolve(&b_body.value_id).unwrap().literal,
        store.resolve(&c_body.value_id).unwrap().literal,
        "body LITERAL is identical across legs — only its taint differs"
    );
    assert!(
        store
            .resolve(&b_body.value_id)
            .unwrap()
            .taint
            .iter()
            .any(|t| t.is_untrusted()),
        "LEG B body must be genuinely untrusted (why it Blocks)"
    );
    assert!(
        !store
            .resolve(&c_body.value_id)
            .unwrap()
            .taint
            .iter()
            .any(|t| t.is_untrusted()),
        "LEG C body must be clean (UserTrusted) — the sole reason it Allows"
    );

    // ── ONE policy, ONE status — held identical across both submissions ──
    let policy = SessionPolicy::broker_default();
    let status = SessionStatus::Active;

    // LEG B: tainted body → BlockedPendingConfirmation, anchor names `body`.
    let decision_b = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node_b,
        &store,
        &status,
        &policy,
    );
    match &decision_b {
        ExecutorDecision::BlockedPendingConfirmation { anchors } => {
            assert_eq!(
                anchors.len(),
                1,
                "only the tainted body should Block — url/method are clean; got {anchors:?}"
            );
            assert_eq!(
                anchors[0].anchor.arg, "body",
                "the Block anchor must name the `body` arg specifically (attributable to \
                 the body, not a blanket sink block)"
            );
            assert_eq!(
                anchors[0].anchor.sink.0, "http.request.write",
                "the Block anchor must name the WRITE sink id"
            );
            // The anchor rides a GENUINE provenance chain (non-stapled): its root is
            // the http_response_received event minted by mint_from_http.
            assert!(
                !anchors[0].anchor.provenance_chain.is_empty(),
                "the Block anchor must carry a genuine (non-empty) provenance chain"
            );
            assert_eq!(
                anchors[0].anchor.read_event_id, anchors[0].anchor.provenance_chain[0],
                "anchor.read_event_id must equal provenance_chain[0] (genuine anchor)"
            );
        }
        other => panic!(
            "LEG B (tainted body, identical url/method/policy) must \
             BlockPendingConfirmation on `body` — got {other:?}"
        ),
    }

    // LEG C: clean body, SAME url/method/policy/status → Allowed.
    let decision_c = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node_c,
        &store,
        &status,
        &policy,
    );
    assert_eq!(
        decision_c,
        ExecutorDecision::Allowed,
        "LEG C (clean body, IDENTICAL url/method/policy to LEG B) must be Allowed — \
         taint is the SOLE variable that flips the outcome"
    );

    // The differential holds AND the audit chain is intact (genuine DAG, not stapled).
    assert!(
        verify_chain(&conn, &session_id.to_string(), TEST_KEY),
        "the audit chain must remain intact (verify_chain) across the mints"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// LEG D (method enum): a method outside {POST,PUT} Denies fail-closed.
// ─────────────────────────────────────────────────────────────────────────────

/// The same clean-body node shape but with a `method` literal NOT in `{POST,PUT}`
/// (`DELETE`) Denies `InvalidMethod` at the fail-closed method-enum gate — BEFORE
/// the node can reach Allowed. Structural fail-closed (like `SlotTypeMismatch`),
/// never a confirmable Block: the write verb must never be a free literal.
#[test]
fn leg_d_method_outside_enum_denies_fail_closed() {
    let (conn, mut store, session_id, head_id, head_hash) = setup();

    let (url, head_id, head_hash) = mint_clean(&conn, &mut store, session_id, WRITE_URL, head_id, &head_hash);
    // A structurally-clean but out-of-enum method.
    let (method, head_id, head_hash) =
        mint_clean(&conn, &mut store, session_id, "DELETE", head_id, &head_hash);
    let (body, _head_id, _head_hash) =
        mint_clean(&conn, &mut store, session_id, BODY_LITERAL, head_id, &head_hash);

    let node = write_node(&url, &method, &body);
    let decision = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node,
        &store,
        &SessionStatus::Active,
        &SessionPolicy::broker_default(),
    );

    match decision {
        ExecutorDecision::Denied {
            reason: runtime_core::DenyReason::InvalidMethod { sink, method },
        } => {
            assert_eq!(sink, "http.request.write");
            assert_eq!(method, "DELETE", "the deny must name the rejected verb");
        }
        other => panic!(
            "an out-of-enum method must Deny InvalidMethod fail-closed (never a \
             confirmable Block) — got {other:?}"
        ),
    }
}
