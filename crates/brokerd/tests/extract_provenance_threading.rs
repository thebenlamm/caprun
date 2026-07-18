//! extract_provenance_threading -- the phase's HARD-GATE proof (EXTRACT-02/03).
//!
//! A programmatic, DB-alone audit-DAG query verifying genuine taint
//! propagation -- including through a concatenation transform -- for EVERY
//! blocked arg in a multi-anchor Block (Phase 12/14 collect-then-Block),
//! paired with an anti-staple negative control that rejects a
//! fabricated/re-anchored chain. "Taint stapled at the sink proves nothing"
//! (CLAUDE.md #1) -- a passing proof on genuine data is worthless without a
//! paired FAILING proof on fabricated data.
//!
//! Modeled directly on `durable_anchor.rs`'s after-exit, DB-alone pattern: a
//! FILE-BACKED audit DB is built end-to-end through the real
//! `dispatch_request` `SubmitPlanNode` arm, the write connection is DROPPED
//! (simulating process exit), then REOPENED so every assertion below is
//! reconstructed from the persisted DB alone -- never from in-memory state.
//!
//! Two graphs, never equated (DESIGN §0): the causal DAG (`parent_id`/
//! `parent_hash`) and the value-lineage graph (`provenance_chain` /
//! `derivation` event payloads) share node ids but have distinct edge
//! semantics. EVERY `provenance_chain` element MUST resolve to a real
//! `file_read` event -- a `derivation` event appearing as a chain element is
//! a fail-closed error, never walked recursively as a chain element (finding
//! #10). The genuine-derivation edge is a SEPARATE predicate over the
//! `derivation` event's HASHED PAYLOAD (finding #2).

use brokerd::audit::{event_hash_by_id, find_event_by_type, open_audit_db, verify_chain};
use brokerd::proto::BrokerRequest;
use brokerd::provenance_proof::{assert_unbroken_edge, genuine_derivation_binds};
use brokerd::quarantine::{concat_doc_fragments, extract_doc_fragments, mint_from_derivation, mint_from_read, Claim};
use brokerd::server::dispatch_request;
use executor::value_store::ValueStore;
use runtime_core::{
    plan_node::{PlanArg, PlanNode, SinkId},
    SessionStatus, ValueRecord,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Fixed, non-secret test MAC key (v1.6 Phase 28, HARDEN-02).
const TEST_KEY: &[u8] = b"extract-provenance-threading-test-key";

/// The CONFIRM-02 hostile-doc fixture: a realistic vendor-reconciliation
/// template that embeds (a) a send-redirection injection attempt the
/// deterministic extractor does NOT obey (there is no LLM in the loop -- the
/// extractor only ever looks for its own marker-anchored fields, never
/// free-form instructions), and (b) the multi-fragment recipient structure
/// on INDEPENDENTLY PLAUSIBLE lines (finding #9): a `Reply-To:` line (the
/// local-part half) and a separate `Domain:` line (the domain half) inside
/// an "Email Routing Configuration" block -- not two adjacent
/// whitespace-delimited tokens that would look extractor-rigged. The concat
/// transform joins them with a literal `@` into the attacker recipient
/// (`accounts@ev1l.com`). A `Body:` marker line carries the tainted email
/// body content.
const HOSTILE_DOC: &str = include_str!("fixtures/hostile_doc.txt");

/// Extract the `Body:` marker-anchored line's content (test-harness-only
/// helper -- NOT part of the confined worker's production extractor set,
/// which this plan does not modify; mirrors `extract_doc_fragments`' marker-
/// anchored, lossy-extraction shape). Returns everything after the `Body:`
/// marker up to end-of-line, trimmed.
fn extract_body_fragment(raw: &str) -> Option<String> {
    let marker = "Body:";
    let idx = raw.find(marker)?;
    let after = &raw[idx + marker.len()..];
    let line_end = after.find('\n').unwrap_or(after.len());
    let value = after[..line_end].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// A unique file-backed audit-DB path (brokerd has no tempfile dev-dep) --
/// mirrors `durable_anchor.rs::temp_db_path`.
fn temp_db_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("caprun_extract_{tag}_{}.db", Uuid::new_v4()))
}

/// Remove a file-backed audit DB and its WAL/SHM sidecars -- mirrors
/// `durable_anchor.rs::cleanup_db`.
fn cleanup_db(db_path: &std::path::Path) {
    let _ = std::fs::remove_file(db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
}

/// A workspace-root anchor for the dispatch call. The BLOCK path never
/// invokes the sink, so any valid directory suffices (mirrors
/// `durable_anchor.rs::ws_root`).
fn ws_root() -> Arc<adapter_fs::workspace::WorkspaceRoot> {
    Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
            .expect("open ws root"),
    )
}

/// Everything Task 3's EXTRACT-02/03 walk needs, reconstructed from the
/// reopened, after-exit, DB-alone connection.
struct TwoAnchorFixture {
    /// The REOPENED connection -- the only source of truth from this point on.
    conn: rusqlite::Connection,
    session_id: Uuid,
    db_path: std::path::PathBuf,
    /// The file_read event id the `Reply-To:` half (local-part) minted.
    reply_to_read_id: Uuid,
    /// The file_read event id the `Domain:` half minted.
    domain_read_id: Uuid,
    /// The file_read event id the tainted body fragment minted.
    body_read_id: Uuid,
}

/// Build a FILE-BACKED audit DB containing a genuine TWO-anchor
/// `email.send` block -- a concatenation-derived tainted `to` (threaded via
/// `mint_from_derivation` over the two recipient-half reads) AND a tainted
/// `body` (via a plain `mint_from_read` doc_fragment mint) -- driven END TO
/// END through the real `dispatch_request` `SubmitPlanNode` arm, then DROP
/// the write connection (simulating process exit) and REOPEN from the path
/// alone.
///
/// Threads `last_event_id`/`last_event_hash` across every mint so the causal
/// DAG stays ONE linear chain (`mint_from_read` returns its
/// `session_demoted` chain head -- never the `file_read` id -- as the next
/// append's `parent_id`, per its own doc warning; `mint_from_derivation`
/// returns its own `derivation` event as the new chain head).
///
/// NOTE for the SUMMARY (finding #13): the two recipient-half `file_read`
/// events are SYNTHETIC (one physical read of the fixture produced both,
/// plus the body read) -- each `mint_from_read` also appends its own
/// `session_demoted` event, so this 3-fragment doc yields 3 redundant
/// demotion events in the persisted DAG in addition to the 3 `file_read` +
/// 1 `derivation` + 1 `sink_blocked` events.
async fn build_two_anchor_block_db(tag: &str) -> TwoAnchorFixture {
    let db_path = temp_db_path(tag);
    let session_id = Uuid::new_v4();

    // FILE-BACKED DB (NOT ":memory:") -- the after-exit proof requires
    // durability across a connection drop + reopen.
    let conn = Arc::new(Mutex::new(
        open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db"),
    ));
    let mut store = ValueStore::default();

    // Extract the two recipient-half fragments (Reply-To:/Domain:) and the
    // tainted body fragment from the hostile doc -- simulating what the
    // CONFINED worker already did over the hostile bytes (extraction never
    // happens broker-side; the broker only mints what the worker already
    // extracted).
    let doc_fragments = extract_doc_fragments(HOSTILE_DOC);
    assert_eq!(
        doc_fragments.len(),
        2,
        "expected exactly two doc_fragment claims (Reply-To: local-part, Domain: domain-half)"
    );
    let reply_to_claim = doc_fragments[0].clone();
    let domain_claim = doc_fragments[1].clone();
    assert_eq!(reply_to_claim.value, "accounts");
    assert_eq!(domain_claim.value, "ev1l.com");

    let body_value = extract_body_fragment(HOSTILE_DOC).expect("Body: marker must be present in fixture");
    let body_claim = Claim {
        claim_type: "doc_fragment".into(),
        value: body_value,
    };

    // Mint the Reply-To: half -- the causal chain ROOT (parent_hash = None).
    let (reply_to_read_id, _reply_to_hash, reply_to_value_id, demoted1_id, demoted1_hash) = {
        let locked = conn.lock().unwrap();
        mint_from_read(&locked, TEST_KEY, &mut store, session_id, &reply_to_claim, None, None)
            .expect("mint_from_read reply_to")
    };

    // Mint the Domain: half, chained onto the Reply-To: session_demoted head
    // (NOT the file_read id -- forking the DAG breaks verify_chain, per
    // mint_from_read's own doc warning).
    let (domain_read_id, _domain_hash, domain_value_id, demoted2_id, demoted2_hash) = {
        let locked = conn.lock().unwrap();
        mint_from_read(
            &locked,
            TEST_KEY,
            &mut store,
            session_id,
            &domain_claim,
            Some(demoted1_id),
            Some(&demoted1_hash),
        )
        .expect("mint_from_read domain")
    };

    // Mint the tainted body fragment, chained onto the Domain: session_demoted head.
    let (body_read_id, _body_hash, body_value_id, demoted3_id, demoted3_hash) = {
        let locked = conn.lock().unwrap();
        mint_from_read(&locked, TEST_KEY, &mut store, session_id, &body_claim, Some(demoted2_id), Some(&demoted2_hash))
            .expect("mint_from_read body")
    };

    // Resolve OWNED clones of the two recipient-half records BEFORE calling
    // mint_from_derivation -- the caller resolves ValueIds to records itself;
    // mint_from_derivation never re-resolves from `store` (avoids a
    // simultaneous mutable+immutable borrow, per its own doc note).
    let reply_to_record: ValueRecord = store
        .resolve(&reply_to_value_id)
        .expect("reply_to_value_id resolves")
        .clone();
    let domain_record: ValueRecord = store
        .resolve(&domain_value_id)
        .expect("domain_value_id resolves")
        .clone();

    // join(input_literals, '@') -- inputs[0] (reply_to) first, inputs[1]
    // (domain) second -- must byte-match mint_from_derivation's own
    // byte-verify guard (MAJOR-1).
    let transformed_literal = concat_doc_fragments(&reply_to_record.literal, &domain_record.literal);
    assert_eq!(transformed_literal, "accounts@ev1l.com");

    // Mint the concatenation-derived recipient, chained onto the body
    // mint's session_demoted head. This becomes the new chain head.
    let (derivation_event_id, derivation_hash, to_value_id) = {
        let locked = conn.lock().unwrap();
        mint_from_derivation(
            &locked,
            TEST_KEY,
            &mut store,
            session_id,
            transformed_literal,
            &[&reply_to_record, &domain_record],
            "concat",
            Some(demoted3_id),
            Some(&demoted3_hash),
        )
        .expect("mint_from_derivation")
    };

    let mut last_event_id = derivation_event_id;
    let mut last_event_hash = derivation_hash;
    // Mirrors durable_anchor.rs: this harness thread starts from an Active
    // seed (a block on I2 fires regardless of session_status); the mints
    // above already demoted the session's DB row to Draft via TAINT-01.
    // v1.6 Phase 27 (X-04/F3): dispatch_request now takes the shared
    // Arc<Mutex<SessionStatus>> shape — a fresh test-local cell here.
    let session_status = Arc::new(Mutex::new(SessionStatus::Active));
    // Trusted-inode placeholder (HARDEN-01, review Fix 2) — this harness
    // never drives RequestFd, so the fstat identity compare is never reached.
    let trusted_inode: Option<(u64, u64)> = None;

    // Both `to` (routing-sensitive, derived-recipient) and `body`
    // (content-sensitive) are present -- present so the collect-then-Block
    // loop (Phase 12/14) surfaces BOTH in one Block, not just one.
    let plan_node = PlanNode {
        sink: SinkId("email.send".into()),
        args: vec![
            PlanArg {
                name: "to".into(),
                value_id: to_value_id,
            },
            PlanArg {
                name: "body".into(),
                value_id: body_value_id,
            },
        ],
    };

    let (mut server_end, _client_end) = tokio::net::UnixStream::pair().expect("UnixStream::pair");
    // Phase 16 (BLOCKER-1 guard a): dispatch_request gained two new
    // per-connection `&mut bool` ordering-guard params; this harness never
    // drives ProvideIntent/RequestFd, so fresh `false` locals are correct.
    let mut intent_provided = false;
    let mut fd_requested = false;
    let mut fd_request_count: u32 = 0;
    dispatch_request(
        BrokerRequest::SubmitPlanNode { plan_node },
        &mut server_end,
        &conn,
        TEST_KEY,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root(),
        &session_status,
        &runtime_core::SessionPolicy::allow_all(),
        trusted_inode,
        &mut intent_provided,
        &mut fd_requested,
        &mut fd_request_count,
    )
    .await
    .expect("dispatch_request must succeed once the block append is durable");

    // Simulate process exit: DROP the only connection handle so SQLite
    // closes it (WAL checkpointed) before anything reopens from the path
    // alone.
    drop(conn);

    // REOPEN from the path alone -- the persisted DB is now the ONLY source
    // of truth for every assertion from here on.
    let reopened = open_audit_db(db_path.to_str().unwrap()).expect("reopen audit DB");

    TwoAnchorFixture {
        conn: reopened,
        session_id,
        db_path,
        reply_to_read_id,
        domain_read_id,
        body_read_id,
    }
}

/// Sanity check consumed by Task 3: the fixture builder produces a
/// persisted, reopenable, DB-alone TWO-anchor block whose anchors carry
/// DISTINCT arg names -- the collect-then-Block multi-arg Block Phase
/// 12/14 mandate. `len == 2` alone would be satisfied by the same
/// `BlockedArg` pushed twice (finding #13 nit); asserting distinct arg
/// names `{"to", "body"}` catches a Phase-14 regression that would slip
/// past a bare length check.
#[tokio::test]
async fn builds_two_anchor_block() {
    let fixture = build_two_anchor_block_db("two_anchor").await;
    let sid = fixture.session_id.to_string();

    assert!(
        verify_chain(&fixture.conn, &sid, TEST_KEY),
        "verify_chain must pass on the REOPENED DB before anything else is trusted \
         (the causal hash chain must survive process exit)"
    );

    let blocked = find_event_by_type(&fixture.conn, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist in the reopened DAG");

    assert_eq!(
        blocked.anchors.len(),
        2,
        "collect-then-Block must surface BOTH the tainted derived recipient AND the tainted body \
         in ONE Block (Phase 12/14, D-14) -- not just one"
    );

    let mut arg_names: Vec<&str> = blocked.anchors.iter().map(|a| a.arg.as_str()).collect();
    arg_names.sort();
    assert_eq!(
        arg_names,
        vec!["body", "to"],
        "the two anchors must carry DISTINCT arg names {{\"to\", \"body\"}} (finding #13) -- \
         len == 2 alone is satisfied by the same BlockedArg pushed twice, which would slip a \
         Phase-14 regression"
    );

    cleanup_db(&fixture.db_path);
}

// -----------------------------------------------------------------------
// Task 3: EXTRACT-02 per-anchor unbroken-edge proof + paired anti-staple
// control + EXTRACT-03 block-survival.
//
// `assert_unbroken_edge`, `genuine_derivation_binds`, and their helper
// `union_provenance_chains` are now imported from the promoted, PUBLIC
// `brokerd::provenance_proof` module (Phase 17, Plan 01) rather than defined
// locally here -- so this test and Phase 17's new live composed test (in a
// different crate, cli/caprun) exercise the SAME implementation, with no
// forked copy that could drift from the HARD-GATE check (COORD-T2). See
// `crates/brokerd/src/provenance_proof.rs` for the full doc comments
// (findings #2/#10/#12) that previously lived on these definitions here.
// -----------------------------------------------------------------------

/// EXTRACT-02 POSITIVE proof + EXTRACT-03 (survival through the transform):
/// iterates EVERY anchor in the persisted two-anchor block (both the
/// derived `to` and the tainted `body`), asserting the per-anchor unbroken
/// edge with IDENTITY-PINNED root-vector equality (finding #12) for BOTH,
/// plus the payload-bound genuine-derivation predicate (finding #2) for the
/// derived `to` anchor specifically. The test FAILS if ANY anchor has any
/// missing/unresolvable/non-file_read element, any root-vector mismatch, or
/// (for the derived anchor) no payload-binding derivation event -- a
/// two-anchor block with only ONE edge proven is a partial pass, i.e. a
/// FAIL.
#[tokio::test]
async fn extract_02_and_03_positive_proof_both_anchors() {
    let fixture = build_two_anchor_block_db("extract02_positive").await;
    let sid = fixture.session_id.to_string();

    assert!(
        verify_chain(&fixture.conn, &sid, TEST_KEY),
        "verify_chain must pass on the REOPENED DB before the anchors are trusted"
    );

    let blocked = find_event_by_type(&fixture.conn, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist in the reopened DAG");
    assert_eq!(blocked.anchors.len(), 2, "sanity: exactly two anchors");

    let to_anchor = blocked
        .anchors
        .iter()
        .find(|a| a.arg == "to")
        .expect("a `to` anchor must be present");
    let body_anchor = blocked
        .anchors
        .iter()
        .find(|a| a.arg == "body")
        .expect("a `body` anchor must be present");

    // ── BOTH anchors: per-element unbroken edge + identity-pinned root vector. ──
    let expected_roots_to = vec![fixture.reply_to_read_id, fixture.domain_read_id];
    assert_unbroken_edge(&fixture.conn, &sid, &to_anchor.provenance_chain, &expected_roots_to).expect(
        "the derived `to` anchor's provenance_chain must be a fully-proven unbroken edge, \
         identity-pinned to EXACTLY [reply_to_read_id, domain_read_id]",
    );

    let expected_roots_body = vec![fixture.body_read_id];
    assert_unbroken_edge(&fixture.conn, &sid, &body_anchor.provenance_chain, &expected_roots_body)
        .expect(
            "the `body` anchor's provenance_chain must be a fully-proven unbroken edge, \
             identity-pinned to EXACTLY [body_read_id]",
        );

    // ── Derived `to` anchor ADDITIONALLY: the finding #2 payload-bound ──
    // genuine-derivation predicate, scanning ALL session derivation events.
    assert!(
        genuine_derivation_binds(&fixture.conn, &sid, &to_anchor.value_id, &to_anchor.provenance_chain),
        "a `derivation` event must exist whose HASHED PAYLOAD binds \
         derived_value_id == anchor.value_id AND \
         ∪input_provenance_chains == anchor.provenance_chain (finding #2) -- found by \
         scanning ALL session derivation events, never find_event_by_type's LIMIT 1"
    );

    // ── EXTRACT-03: the concatenation-derived recipient STILL carries ──
    // untrusted taint AND is STILL in the persisted Block (taint and
    // provenance survived the transform, not just a copy).
    assert!(
        to_anchor.taint.iter().any(|t| t.is_untrusted()),
        "EXTRACT-03: the transform-derived recipient must still carry untrusted taint \
         after the concat transform"
    );
    assert_eq!(
        blocked.anchors.iter().filter(|a| a.arg == "to").count(),
        1,
        "EXTRACT-03: the derived recipient is present in the persisted sink_blocked Block \
         (submit_plan_node still returned BlockedPendingConfirmation for it)"
    );

    cleanup_db(&fixture.db_path);
}

/// NEGATIVE CONTROL A (finding #4/Pitfall 4): a hand-constructed chain
/// containing a `Uuid::new_v4()` NEVER appended to the DAG must be
/// REJECTED -- `find_event_by_id` returns `None`, so the edge is unproven.
/// This proves the check is not merely a non-empty-chain test.
#[tokio::test]
async fn extract_02_anti_staple_control_a_fabricated_root_is_rejected() {
    let fixture = build_two_anchor_block_db("extract02_control_a").await;
    let sid = fixture.session_id.to_string();
    assert!(verify_chain(&fixture.conn, &sid, TEST_KEY), "baseline chain must verify");

    let fabricated_root = Uuid::new_v4();
    let fabricated_chain = vec![fabricated_root];

    let result = assert_unbroken_edge(&fixture.conn, &sid, &fabricated_chain, &fabricated_chain);
    assert!(
        result.is_err(),
        "a provenance_chain rooted at a uuid NEVER appended to the DAG must be REJECTED"
    );
    assert!(
        result.unwrap_err().contains("does not resolve"),
        "the rejection must be because the fabricated root does not resolve to any real event"
    );

    cleanup_db(&fixture.db_path);
}

/// NEGATIVE CONTROL B (finding #11, the anti-staple's exact teeth
/// requirement): a NAIVE extractor's defect is simulated by minting the
/// ALREADY-CONCATENATED recipient literal via a PLAIN `mint_from_read` call
/// INTO THE SAME session/DB as the genuine block -- a fresh, REAL file_read
/// root, in the SAME session, but with NO threaded ancestry to the two
/// original recipient-half reads. Because the concatenated literal contains
/// `@` (rejected by `looks_like_doc_fragment`'s guard), this control mints
/// it via the `"email_address"` claim shape instead -- a DIFFERENT
/// claim_type, still a genuine broker mint, faithfully modeling what a
/// naive (non-provenance-threading) extractor implementation would produce.
///
/// The genuine-derivation predicate MUST reject this re-anchored value
/// specifically because NO derivation event's payload binds its value_id to
/// the recipient-half input chains -- NEVER via a session-wide "this
/// session has a derivation event" query (which would be VACUOUSLY true,
/// since the genuine block's own derivation event lives in this same
/// session). The test asserts BOTH: (a) a derivation event DOES exist in
/// this session (sanity -- proving the check isn't merely "no derivation
/// events exist"), and (b) the payload-bound predicate still rejects.
#[tokio::test]
async fn extract_02_anti_staple_control_b_reanchored_staple_is_rejected() {
    let fixture = build_two_anchor_block_db("extract02_control_b").await;
    let sid = fixture.session_id.to_string();
    assert!(verify_chain(&fixture.conn, &sid, TEST_KEY), "baseline chain must verify");

    let blocked = find_event_by_type(&fixture.conn, &sid, "sink_blocked")
        .expect("query sink_blocked")
        .expect("a durable sink_blocked event must exist");
    let to_anchor = blocked
        .anchors
        .iter()
        .find(|a| a.arg == "to")
        .expect("a `to` anchor must be present");

    // Sanity (the vacuous-check trap, finding #11): a genuine derivation
    // event DOES exist in this session -- the block's own.
    let session_has_a_derivation_event = find_event_by_type(&fixture.conn, &sid, "derivation")
        .expect("query derivation")
        .is_some();
    assert!(
        session_has_a_derivation_event,
        "sanity: this session DOES have a derivation event (the genuine block's own) -- \
         a session-wide existence query would be VACUOUSLY satisfied, which is exactly why \
         the real predicate must be payload-bound, not existence-based"
    );

    // Chain the naive re-mint onto the sink_blocked event -- keeps ONE
    // linear causal chain (never fork a second parent_id=None root in the
    // same session, which would break verify_chain's single-chain walk).
    let blocked_hash = event_hash_by_id(&fixture.conn, &blocked.id.to_string())
        .expect("event_hash_by_id")
        .expect("sink_blocked event must have a stored hash");

    let mut scratch_store = ValueStore::default();
    let naive_claim = Claim {
        // A DIFFERENT claim_type shape than `doc_fragment` (whose
        // looks_like_doc_fragment guard rejects any '@'-containing token,
        // per finding #1a) -- `email_address` accepts the already-assembled
        // recipient literal directly, modeling a naive extractor that skips
        // provenance-threading entirely.
        claim_type: "email_address".into(),
        value: "accounts@ev1l.com".into(),
    };
    let (naive_read_id, _naive_hash, naive_value_id, _demoted_id, _demoted_hash) = mint_from_read(
        &fixture.conn,
        TEST_KEY,
        &mut scratch_store,
        fixture.session_id,
        &naive_claim,
        Some(blocked.id),
        Some(&blocked_hash),
    )
    .expect("mint_from_read (naive re-anchor) must succeed -- a REAL, same-session mint");

    let naive_provenance_chain = vec![naive_read_id];

    // Root-vector mismatch: the naive chain is NOT the identity-pinned
    // expected roots for `to` (finding #12).
    assert_ne!(
        naive_provenance_chain, to_anchor.provenance_chain,
        "the naive re-anchored chain must NOT equal the genuine derived `to` anchor's \
         identity-pinned provenance_chain"
    );

    // THE TEETH: rejected specifically on the payload-binding predicate,
    // never on session-wide existence (asserted true above).
    assert!(
        !genuine_derivation_binds(&fixture.conn, &sid, &naive_value_id, &naive_provenance_chain),
        "control B (finding #11): NO derivation event's payload may bind the naive \
         re-anchored value_id to its (nonexistent) input chains -- the check must reject \
         here even though a derivation event EXISTS in this session (asserted above), because \
         the predicate is payload-bound, not existence-based"
    );

    // The mutation preserves a single linear chain (append-only, chained
    // onto the sink_blocked event -- never a second root).
    assert!(
        verify_chain(&fixture.conn, &sid, TEST_KEY),
        "verify_chain must still hold after appending the naive re-mint (single linear \
         chain preserved, not forked)"
    );

    cleanup_db(&fixture.db_path);
}
