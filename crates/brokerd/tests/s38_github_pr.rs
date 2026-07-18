//! Phase 38 (GITHUB-01/02/04) integration tests — the two independent gates
//! that stand ahead of any `github.pr` POST on the Allowed (untainted,
//! never-blocked) dispatch path:
//!
//!   * GITHUB-02 (§4.3/§8): a bare Allowed decision CANNOT create a PR. Absent a
//!     live session auth-grant, the dispatch Denies (opaque `github_pr_denied`)
//!     and NEVER reaches the content CAS or the socket.
//!   * GITHUB-04 (§4.5): a replayed identical submission creates AT MOST ONE PR.
//!     The content-derived `created_prs` CAS is reserved (INSERT-before-POST)
//!     BEFORE any socket opens; the second identical submit is suppressed
//!     (`github_pr_replay_suppressed`), leaving exactly one CAS row + one attempt.
//!   * GITHUB-01 (opaque audit): the bearer token literal NEVER appears in any
//!     audit-event payload (token custody is broker-env-only).
//!
//! HOST-PORTABLE by construction (CLAUDE.md): these drive the broker-side
//! primitives the server.rs Allowed-`github.pr` arm composes
//! (`has_github_grant` gate -> `reserve_created_pr` content CAS ->
//! `invoke_github_pr_from_resolved` POST) DIRECTLY against an in-memory audit db
//! + a frozen `ResolvedArg` snapshot — NO abstract-namespace UDS (Linux-only)
//! and NO live GitHub. Both gates run BEFORE the POST, and the live POST is a
//! macOS no-op stub (`do_pinned_post` bails `Err`, appending the durable opaque
//! `github_pr_failed` terminal event), so every assertion here — grant gate,
//! CAS at-most-once, opaque audit, `verify_chain` — holds on ANY platform. The
//! real-socket end-to-end create-PR proof is the Phase-40 mock / composed live
//! step (mirrors `s37_http_request.rs`, which likewise drives `mint_from_http`
//! directly rather than over the Linux-only socket).
//!
//! `dispatch_github_pr_like_arm` MIRRORS the ordering of the server.rs
//! `evaluate_plan_node_and_record` github.pr arm (grant gate FIRST, then the CAS
//! + divergent attempt/replay marker committed before the POST, then the POST on
//! the fresh branch only). The arm itself lives inside a crate-private async fn
//! whose only live driver needs the Linux UDS server; this helper exercises the
//! identical PUBLIC primitives in the identical order so the load-bearing
//! properties (gate, at-most-once, opacity) are proven host-portably.

#![cfg(test)]

use brokerd::audit::{
    append_event, current_chain_head, find_event_by_type, github_pr_content_key, has_github_grant,
    open_audit_db, record_github_grant, reserve_created_pr, verify_chain,
};
use brokerd::confirmation::ResolvedArg;
use brokerd::session::{create_session, persist_session};
use brokerd::sinks::github_pr::invoke_github_pr_from_resolved;
use chrono::Utc;
use runtime_core::plan_node::{TaintLabel, ValueId};
use runtime_core::{Event, SeedProvenance};
use rusqlite::Connection;
use uuid::Uuid;

/// Fixed, non-secret test MAC key (mirrors the audit-layer test key style).
const TEST_KEY: &[u8] = b"s38-github-pr-integration-test-key-not-secret";

/// A recognizable, non-real bearer token whose literal MUST NEVER surface in
/// any audit-event payload (opaque-audit / broker-env-only custody, GITHUB-01).
const SECRET_TOKEN: &str = "ghp_s38_SECRET_TOKEN_must_not_leak_into_audit";

/// Serializes tests in THIS binary that mutate the process-global
/// `CAPRUN_GITHUB_*` env vars — the multi-threaded test runner would otherwise
/// let two race on the same process-wide environment (mirror
/// `github_pr.rs`'s `GITHUB_ENV_LOCK`).
static GITHUB_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One UserTrusted (untainted) resolved arg — represents the never-blocked,
/// Allowed-decision input the server.rs arm would resolve from its ValueStore.
fn arg(name: &str, literal: &str) -> ResolvedArg {
    ResolvedArg {
        name: name.to_string(),
        value_id: ValueId::new(),
        literal: literal.to_string(),
        taint: vec![TaintLabel::UserTrusted],
        provenance_chain: vec![],
    }
}

/// The six-arg untainted github.pr snapshot (owner/repo/base/head/title/body).
fn well_formed_args() -> Vec<ResolvedArg> {
    vec![
        arg("owner", "octocat"),
        arg("repo", "hello-world"),
        arg("base", "main"),
        arg("head", "feature-branch"),
        arg("title", "A github.pr integration-test PR"),
        arg("body", "PR body from s38_github_pr.rs"),
    ]
}

/// Look up a required literal from a frozen snapshot (test-side helper).
fn lit<'a>(resolved: &'a [ResolvedArg], name: &str) -> &'a str {
    resolved
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
        .unwrap_or_else(|| panic!("test arg `{name}` missing"))
}

/// Open an in-memory audit db, persist a fresh Active session, and seed a
/// `session_created` causal root so `verify_chain` can walk an unbroken linear
/// chain afterward (mirror `s37_http_request.rs::setup`).
fn setup() -> (Connection, Uuid) {
    let conn = open_audit_db(":memory:").expect("open_audit_db");
    let session = create_session(Uuid::new_v4(), SeedProvenance::TrustedArg);
    persist_session(&conn, &session).expect("persist session");

    let root = Event::new(
        Uuid::new_v4(),
        None,
        session.id,
        "broker".into(),
        "session_created".into(),
        Utc::now(),
        vec![],
    );
    append_event(&conn, TEST_KEY, &root, None).expect("append session_created root");

    (conn, session.id)
}

/// Mirror the server.rs Allowed-`github.pr` arm's ordering against the PUBLIC
/// primitives (grant gate FIRST; then the content CAS + divergent
/// attempt/replay marker BEFORE the socket; then the POST on the fresh branch
/// only). NOTE (Phase-38 adversarial finding #2): this is a HAND-ROLLED MIRROR
/// — the no-grant Deny leg it models is ALSO covered against the REAL
/// production `evaluate_plan_node_and_record` arm by
/// `github_pr_without_grant_denies_via_real_dispatch` below, so drift between
/// this mirror and the real grant gate is caught. The replay/CAS leg's
/// real-arm behavioral coverage is the Phase-40 composed live-proof (its POST
/// leg needs the Linux UDS + a live/mock GitHub). Reads the current chain head
/// from the db each call so it threads
/// linearly onto whatever came before (including `record_github_grant`'s own
/// `github_grant_authorized` event). Returns nothing — every property is
/// asserted from the durable audit afterward.
async fn dispatch_github_pr_like_arm(conn: &Connection, session_id: Uuid, resolved: &[ResolvedArg]) {
    let sid = session_id.to_string();
    let (head_id, head_hash) = current_chain_head(conn, &sid)
        .expect("current_chain_head query")
        .expect("a chain head must exist (session_created root was seeded)");
    let effect_id = Uuid::new_v4();

    // (1) GRANT GATE FIRST — absent a live grant, append an OPAQUE terminal
    //     github_pr_denied and STOP (no content key, no CAS, no POST).
    if !has_github_grant(conn, &sid) {
        let denied = Event::new(
            Uuid::new_v4(),
            Some(head_id),
            session_id,
            format!("sink:github.pr:{effect_id}"),
            "github_pr_denied".into(),
            Utc::now(),
            vec![],
        );
        append_event(conn, TEST_KEY, &denied, Some(&head_hash)).expect("append github_pr_denied");
        return;
    }

    // (2) content-derived CAS key from the resolved literals.
    let content_key = github_pr_content_key(
        lit(resolved, "owner"),
        lit(resolved, "repo"),
        lit(resolved, "base"),
        lit(resolved, "head"),
        lit(resolved, "title"),
        lit(resolved, "body"),
    );

    // (3) CAS BEFORE EFFECT — reserve, then append the divergent
    //     attempt/replay-suppressed marker (in the real arm these commit in one
    //     transaction; the test is single-threaded so atomicity is not the
    //     property under test — the at-most-once CAS is).
    let fresh = reserve_created_pr(conn, &content_key, &effect_id.to_string(), &sid)
        .expect("reserve_created_pr");
    let event_type = if fresh {
        "github_pr_attempted"
    } else {
        "github_pr_replay_suppressed"
    };
    let marker = Event::new(
        Uuid::new_v4(),
        Some(head_id),
        session_id,
        format!("sink:github.pr:{effect_id}"),
        event_type.into(),
        Utc::now(),
        vec![],
    );
    let marker_hash =
        append_event(conn, TEST_KEY, &marker, Some(&head_hash)).expect("append marker event");

    // (4) FRESH branch only opens the socket. The live POST is a macOS no-op
    //     stub (bails Err after a durable opaque github_pr_failed); on any
    //     platform without a live endpoint it Errs. The durable terminal event
    //     is what the tests assert — swallow the Err (it is expected here).
    if fresh {
        let _ = invoke_github_pr_from_resolved(
            conn,
            TEST_KEY,
            session_id,
            effect_id,
            resolved,
            marker.id,
            &marker_hash,
        )
        .await;
    }
}

/// Count events of a given type for a session.
fn count_events(conn: &Connection, session_id: Uuid, event_type: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2",
        rusqlite::params![session_id.to_string(), event_type],
        |row| row.get(0),
    )
    .expect("count events")
}

/// Count created_prs rows for a content key.
fn count_created_prs(conn: &Connection, content_key: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM created_prs WHERE idempotency_key = ?1",
        rusqlite::params![content_key],
        |row| row.get(0),
    )
    .expect("count created_prs")
}

/// Assert a substring appears in NO event's actor/event_type/payload for a
/// session (opaque-audit discipline).
fn assert_absent_from_all_event_payloads(conn: &Connection, session_id: Uuid, needle: &str) {
    let mut stmt = conn
        .prepare("SELECT actor, event_type, payload FROM events WHERE session_id = ?1")
        .expect("prepare events scan");
    let rows = stmt
        .query_map(rusqlite::params![session_id.to_string()], |row| {
            let actor: String = row.get(0)?;
            let event_type: String = row.get(1)?;
            let payload: String = row.get(2)?;
            Ok(format!("{actor}|{event_type}|{payload}"))
        })
        .expect("query events");
    for row in rows {
        let combined = row.expect("read event row");
        assert!(
            !combined.contains(needle),
            "the bearer token literal must NEVER appear in any audit-event \
             actor/event_type/payload (opaque audit, GITHUB-01); found in: {combined}"
        );
    }
}

/// GITHUB-02 (§4.3/§8): an untainted `github.pr` with NO live grant Denies —
/// an opaque `github_pr_denied` terminal event is recorded, NO
/// `github_pr_attempted` is ever recorded, and NO `created_prs` CAS row is
/// created. Proves a bare Allowed/confirm decision cannot create a PR
/// independent of the executor's decision.
#[tokio::test]
async fn github_pr_without_grant_denies_no_attempt() {
    let (conn, session_id) = setup();
    let resolved = well_formed_args();

    // NO record_github_grant — the session holds no github.pr grant.
    assert!(
        !has_github_grant(&conn, &session_id.to_string()),
        "precondition: an ungranted session must report no github.pr grant"
    );

    dispatch_github_pr_like_arm(&conn, session_id, &resolved).await;

    // A terminal denied event exists...
    assert_eq!(
        count_events(&conn, session_id, "github_pr_denied"),
        1,
        "an ungranted github.pr must record exactly one opaque github_pr_denied event"
    );
    // ...and NO attempt / no PR was ever created.
    assert_eq!(
        count_events(&conn, session_id, "github_pr_attempted"),
        0,
        "an ungranted github.pr must NEVER record a github_pr_attempted (no CAS, no POST)"
    );
    let content_key = github_pr_content_key(
        lit(&resolved, "owner"),
        lit(&resolved, "repo"),
        lit(&resolved, "base"),
        lit(&resolved, "head"),
        lit(&resolved, "title"),
        lit(&resolved, "body"),
    );
    assert_eq!(
        count_created_prs(&conn, &content_key),
        0,
        "an ungranted github.pr must create NO created_prs CAS row"
    );
    assert!(
        verify_chain(&conn, &session_id.to_string(), TEST_KEY),
        "the audit chain must remain intact after the denied dispatch"
    );
}

/// GITHUB-04 (§4.5): with a live grant recorded, two IDENTICAL untainted
/// `github.pr` submits create AT MOST ONE PR — EXACTLY ONE `github_pr_attempted`,
/// exactly ONE `github_pr_replay_suppressed`, and exactly ONE `created_prs` row
/// for the content key. The chain verifies, and the bearer token literal never
/// enters any audit payload (GITHUB-01).
#[tokio::test]
async fn github_pr_replay_creates_at_most_one() {
    let _env = GITHUB_ENV_LOCK.lock().unwrap();
    // Broker-local bearer token (D-04): set for the POST leg. Even though the
    // macOS POST is a no-op stub, the token is read (github_token()) before the
    // stub bails — this exercises that path while asserting the token stays out
    // of the audit. Leave CAPRUN_GITHUB_API_BASE at its fixed default.
    std::env::set_var("CAPRUN_GITHUB_TOKEN", SECRET_TOKEN);
    std::env::remove_var("CAPRUN_GITHUB_API_BASE");

    let (conn, session_id) = setup();
    let resolved = well_formed_args();

    // Record a live github.pr grant for this session (GITHUB-02 satisfied).
    record_github_grant(&conn, TEST_KEY, &session_id.to_string()).expect("record_github_grant");
    assert!(
        has_github_grant(&conn, &session_id.to_string()),
        "precondition: the granted session must report a live github.pr grant"
    );

    // Two IDENTICAL submits.
    dispatch_github_pr_like_arm(&conn, session_id, &resolved).await;
    dispatch_github_pr_like_arm(&conn, session_id, &resolved).await;

    std::env::remove_var("CAPRUN_GITHUB_TOKEN");

    // Exactly one attempt + one replay-suppression across the two submits.
    assert_eq!(
        count_events(&conn, session_id, "github_pr_attempted"),
        1,
        "two identical submits must yield EXACTLY ONE github_pr_attempted (at-most-once)"
    );
    assert_eq!(
        count_events(&conn, session_id, "github_pr_replay_suppressed"),
        1,
        "the second identical submit must be suppressed (one github_pr_replay_suppressed)"
    );

    // Exactly one created_prs CAS row for the content key.
    let content_key = github_pr_content_key(
        lit(&resolved, "owner"),
        lit(&resolved, "repo"),
        lit(&resolved, "base"),
        lit(&resolved, "head"),
        lit(&resolved, "title"),
        lit(&resolved, "body"),
    );
    assert_eq!(
        count_created_prs(&conn, &content_key),
        1,
        "the content-derived CAS must hold EXACTLY ONE row after two identical submits"
    );

    // The first (fresh) submit's POST failed on the no-op stub -> a durable
    // opaque github_pr_failed terminal event was recorded (audit completeness).
    assert!(
        find_event_by_type(&conn, &session_id.to_string(), "github_pr_failed")
            .expect("find github_pr_failed")
            .is_some(),
        "the fresh-branch POST (macOS no-op stub) must leave a durable opaque \
         github_pr_failed terminal event"
    );

    // Chain intact + token opaque.
    assert!(
        verify_chain(&conn, &session_id.to_string(), TEST_KEY),
        "the audit chain must remain intact across two submits + the failed POST"
    );
    assert_absent_from_all_event_payloads(&conn, session_id, SECRET_TOKEN);
}

/// GITHUB-02 via the REAL production dispatch path (Phase-38 adversarial
/// finding #2): drive `brokerd::server::evaluate_plan_node_and_record_for_test`
/// — the `test-fixtures`-gated VERBATIM delegate to the ACTUAL server.rs
/// `evaluate_plan_node_and_record` `github.pr` arm — for the no-grant Deny
/// case. This path DENIES at the grant gate BEFORE any content key, CAS row,
/// or GitHub socket, so it is fully HOST-PORTABLE (no live GitHub, no
/// Linux-only UDS). Unlike `dispatch_github_pr_like_arm` (a hand-rolled
/// mirror), any drift between the mirror's grant-gate ordering and the real
/// arm's is caught HERE: the executor first Allows the untainted node
/// (never-blocked), and ONLY the real arm's own grant gate turns that Allowed
/// decision into an opaque `github_pr_denied` — proving a bare Allowed
/// decision cannot create a PR (§4.3/§8) through the production code itself.
#[tokio::test]
async fn github_pr_without_grant_denies_via_real_dispatch() {
    use executor::value_store::ValueStore;
    use runtime_core::{ExecutorDecision, PlanArg, PlanNode, SessionStatus, SinkId};
    use std::sync::{Arc, Mutex};

    let (conn, session_id) = setup();
    let session_id_str = session_id.to_string();

    // Thread the dispatch onto the seeded `session_created` root — the real
    // arm appends `github_pr_denied` with `Some(*last_event_id)` as its causal
    // parent and `Some(last_event_hash)` as the chain link.
    let (head_id, head_hash) = current_chain_head(&conn, &session_id_str)
        .expect("current_chain_head query")
        .expect("a chain head must exist (session_created root was seeded)");

    // Build a per-connection ValueStore holding six UserTrusted (untainted)
    // args and a `github.pr` PlanNode of opaque handles to them. All-UserTrusted
    // + role-unconstrained github.pr slots => the executor returns Allowed
    // (never-blocked), so the ONLY thing that can deny is the arm's grant gate.
    let resolved = well_formed_args();
    let mut value_store = ValueStore::default();
    let provenance_anchor = Uuid::new_v4(); // non-empty provenance (mint invariant)
    let mut args = Vec::with_capacity(resolved.len());
    for r in &resolved {
        let vid = value_store
            .mint(
                r.literal.clone(),
                vec![TaintLabel::UserTrusted],
                vec![provenance_anchor],
                None,
            )
            .expect("mint UserTrusted arg");
        args.push(PlanArg {
            name: r.name.clone(),
            value_id: vid,
        });
    }
    let plan_node = PlanNode {
        sink: SinkId("github.pr".to_string()),
        args,
    };

    // Precondition: NO github.pr grant recorded for this session.
    assert!(
        !has_github_grant(&conn, &session_id_str),
        "precondition: an ungranted session must report no github.pr grant"
    );

    // Wrap for the real function's `&Arc<Mutex<Connection>>` contract. The
    // no-grant path never touches the workspace fd, but the signature requires
    // one — open a throwaway root at the OS temp dir (host-portable).
    let conn = Arc::new(Mutex::new(conn));
    let workspace = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(&std::env::temp_dir())
            .expect("open temp workspace root"),
    );
    let mut last_event_id = head_id;
    let mut last_event_hash = head_hash;

    let (decision, output_value_id, session_demoted) =
        brokerd::server::evaluate_plan_node_and_record_for_test(
            &plan_node,
            &conn,
            TEST_KEY,
            session_id,
            &mut value_store,
            &workspace,
            &SessionStatus::Active,
            // v1.9 Phase 42: policy-agnostic — allow_all() permits github.pr so
            // this exercises the arm's grant gate, not a policy deny.
            &runtime_core::SessionPolicy::allow_all(),
            &mut last_event_id,
            &mut last_event_hash,
        )
        .await
        .expect("real github.pr dispatch must not error on the no-grant deny path");

    // The executor ALLOWED the untainted node — the Deny is the arm's grant
    // gate, NOT an executor block (proves the gate stands independent of the
    // executor decision).
    assert!(
        matches!(decision, ExecutorDecision::Allowed),
        "an untainted github.pr node must be Allowed by the executor; the arm's \
         grant gate — not a block — is what denies the PR"
    );
    assert!(
        output_value_id.is_none(),
        "github.pr CONSUMES — it mints nothing, so no output ValueId"
    );
    assert!(
        !session_demoted,
        "the github.pr deny path performs no I1 session demotion"
    );

    // Durable audit assertions against the REAL arm's writes.
    let locked = conn.lock().expect("lock conn");

    assert_eq!(
        count_events(&locked, session_id, "github_pr_denied"),
        1,
        "the REAL arm must record exactly one opaque github_pr_denied for an \
         ungranted github.pr"
    );
    assert_eq!(
        count_events(&locked, session_id, "github_pr_attempted"),
        0,
        "the REAL arm must record NO github_pr_attempted (denied before the CAS)"
    );

    let content_key = github_pr_content_key(
        lit(&resolved, "owner"),
        lit(&resolved, "repo"),
        lit(&resolved, "base"),
        lit(&resolved, "head"),
        lit(&resolved, "title"),
        lit(&resolved, "body"),
    );
    assert_eq!(
        count_created_prs(&locked, &content_key),
        0,
        "the REAL arm must create NO created_prs CAS row when denied (no POST \
         was ever attempted)"
    );

    assert!(
        verify_chain(&locked, &session_id_str, TEST_KEY),
        "the audit chain must remain intact after the real-arm denied dispatch"
    );
}
