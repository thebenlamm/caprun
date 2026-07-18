//! Phase 44 (GIT-02/GIT-03) DIFFERENTIAL acceptance test — the requirement proof
//! for the broker-performed, destination-pinned `git.push` egress.
//!
//! # Why differential, not "not blocked" (DESIGN-v1.9-egress-policy §1, RESEARCH §8)
//!
//! Acceptance for GIT-02/GIT-03 is DIFFERENTIAL (the Phase-43 M4 discipline): with
//! remote, refspec, policy, and session-status held BYTE-IDENTICAL, a TAINTED
//! routing arg (`remote` or `refspec`) MUST Block under I2 while the CLEAN args
//! MUST be Allowed AND — after a human confirm — REACH the pinned mock
//! `git-receive-pack` endpoint. Taint is the SOLE Block variable. This is the
//! anti-stapling / anti-regression property (T-44-20):
//!
//!   * a block-everything I2 regression fails the clean (LEG C) leg — the clean
//!     push must reach egress, not Block;
//!   * a blanket-allow regression fails the tainted (LEG B) leg — the tainted arg
//!     must Block, not Allow;
//!   * so a "not blocked" test alone (which a block-everything regression passes
//!     vacuously) is insufficient, and this test is not that.
//!
//! The tainted arg is minted through the REAL broker mint path (`mint_from_http`,
//! a genuine `http_response_received`-rooted provenance chain) — NEVER a hand-set
//! taint field — so the Block rides a genuine audit-DAG edge, not a tag stapled at
//! the sink (§9 anti-stapling).
//!
//! # Legs
//!
//!   * LEG A (I0, host-portable): a draft / untrusted-seeded session submitting
//!     `git.push` is Denied `DraftOnlySessionDeniesCommitIrreversible` — proving
//!     git.push is `CommitIrreversible` (T-44-01), not an Observe fall-through.
//!   * LEG B (tainted arg Blocks, host-portable): Active session, remote/refspec/
//!     policy fixed, a `remote` (and separately a `refspec`) minted UNTRUSTED →
//!     `BlockedPendingConfirmation` whose anchor names the tainted arg.
//!   * LEG C (clean Allowed at the executor → confirm-gated → confirmed →
//!     delivered): the SAME Active session shape / remote / refspec / policy, args
//!     minted `UserTrusted` — the executor Allows (taint is the SOLE difference
//!     from B), then the broker ALWAYS-confirm-gate re-gates even the clean push
//!     into `BlockedPendingConfirmation` (git.push is never auto-dispatched), and
//!     after a single-shot confirm the push dispatches through the frozen-IP
//!     client to the mock git-receive-pack (Linux; mock records receipt),
//!     appending an opaque `git_push_succeeded` terminal event.
//!   * LEG D (structural denial, host-portable): a `--force`/`+`-refspec and a
//!     `:delete` refspec are refused BY CONSTRUCTION — `validate_git_refspec` and
//!     `build_command_list` (the zero-new-oid delete) Err — unreachable even via a
//!     confirm (a human confirms a specific push, never a license to rewrite
//!     history), T-44-24.
//!   * LEG E (destination pin / redirect, Linux): a receive-pack 3xx is REFUSED
//!     (never followed) — the frozen redirect-none client surfaces it as a
//!     non-success status and the push fails closed (terminal git_push_failed).
//!
//! The decision-level legs (A, B, C-executor, D) drive the REAL executor
//! (`executor::submit_plan_node`) over the REAL broker value_store +
//! `SessionPolicy::broker_default()` (git.push is policy-permitted) and are
//! host-portable by construction (pure decision over an in-memory audit db +
//! ValueStore, no socket, no git). The DISPATCH-level legs (C's mock delivery +
//! credential absence, E's redirect refusal) touch a socket + a confined `git`
//! child and are `#[cfg(target_os = "linux")]` + `#[cfg(feature = "mock-egress-ca")]`
//! — proven on the compose-verify Linux gate with the git-receive-pack mock
//! sidecar reachable.
//!
//! # Scope (Phase 46 is the composed proof)
//!
//! The full composed multi-sink workflow (process.exec → edit → commit → push →
//! PR) driven via the CLI/viewer is Phase 46 (LIVE-05/06); THIS test proves the
//! git.push leg's differential + pinned mock-endpoint dispatch + credential
//! absence in isolation.

#![cfg(test)]

use brokerd::audit::{append_event, open_audit_db, verify_chain};
use brokerd::quarantine::{mint_from_http, mint_from_intent};
use brokerd::session::{create_session, persist_session};
use brokerd::sinks::git_push::{build_command_list, validate_git_refspec};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, SinkId, ValueId};
use runtime_core::{
    Event, ExecutorDecision, PlanNode, SeedProvenance, SessionPolicy, SessionStatus,
};
use uuid::Uuid;

/// Fixed, non-secret test MAC key (mirrors the sibling broker integration tests).
const TEST_KEY: &[u8] = b"s44-git-push-differential-test-key-not-secret";

/// The push remote held IDENTICAL across the tainted (B) and clean (C) legs —
/// taint, not remote, is the differentiator. The `/accept/` path is the one the
/// git-receive-pack mock serves + accepts (the LEG-C delivery target).
const REMOTE: &str = "https://github-mock.caprun.test/accept/repo.git";
/// The push refspec held IDENTICAL across B and C (a plain non-force
/// `<src>:<dst>`).
const REFSPEC: &str = "refs/heads/main:refs/heads/main";

/// Open an in-memory audit db, persist a fresh Active session, and seed a
/// `session_created` causal root so subsequent mints thread onto a real chain
/// head. Mirrors the s43 setup shape.
fn setup() -> (rusqlite::Connection, ValueStore, Uuid, Uuid, String) {
    let conn = open_audit_db(":memory:").unwrap();
    let store = ValueStore::default();
    let (session_id, root_id, root_hash) = seed_session(&conn);
    (conn, store, session_id, root_id, root_hash)
}

/// Persist a fresh Active session on `conn` and seed its `session_created` causal
/// root. Returns `(session_id, root_event_id, root_hash)`.
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

/// Mint a CLEAN (`UserTrusted`) value through the REAL broker UserTrusted mint
/// path (`mint_from_intent`), threading the causal chain head forward. git.push's
/// remote/refspec slots are role-unconstrained (`expected_role == None`), so the
/// Step-1c role gate is a no-op here (`origin_role = None`).
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

/// Mint a genuinely-TAINTED routing arg through the REAL broker http-taint mint
/// path (`mint_from_http`) — an untrusted-on-arrival value re-used as a push
/// destination is the canonical mis-route shape. `provenance_chain[0]` is a
/// genuine `http_response_received` event (NON-STAPLED). Returns the value handle
/// plus the new chain head (the LAST appended `session_demoted` event). NOTE: this
/// demotes the persisted session to Draft (I1) — the decision-level legs pass
/// `SessionStatus::Active` EXPLICITLY so the Block under test is TAINT-driven (I2),
/// not a draft-session gate.
fn mint_tainted(
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
    .expect("mint_from_http (untrusted routing arg) must succeed");
    (value_id, chain_head_id, chain_head_hash)
}

/// Build a `git.push` plan node from the two arg handles.
fn push_node(remote: &ValueId, refspec: &ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("git.push".into()),
        args: vec![
            PlanArg { name: "remote".into(), value_id: remote.clone() },
            PlanArg { name: "refspec".into(), value_id: refspec.clone() },
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LEG A (I0): a draft / untrusted-seeded session I0-denies the push.
// ─────────────────────────────────────────────────────────────────────────────

/// A `git.push` submitted while the session is `Draft` — with CLEAN remote +
/// refspec (so no I2 Block pre-empts) — Denies
/// `DraftOnlySessionDeniesCommitIrreversible`. This proves git.push is classed
/// `CommitIrreversible` (T-44-01): a draft / untrusted-seeded session cannot
/// auto-authorize a push (never an Observe fall-through).
#[test]
fn leg_a_i0_draft_session_denies_push_commit_irreversible() {
    let (conn, mut store, session_id, head_id, head_hash) = setup();

    let (remote, head_id, head_hash) =
        mint_clean(&conn, &mut store, session_id, REMOTE, head_id, &head_hash);
    let (refspec, _head_id, _head_hash) =
        mint_clean(&conn, &mut store, session_id, REFSPEC, head_id, &head_hash);

    let node = push_node(&remote, &refspec);
    let decision = executor::submit_plan_node(
        session_id,
        Uuid::new_v4(),
        &node,
        &store,
        // Draft: the I0 class-level deny under test. Clean args mean no I2 Block
        // pre-empts it — the Draft+CommitIrreversible gate is the sole reason.
        &SessionStatus::Draft,
        &SessionPolicy::broker_default(),
    );

    match decision {
        ExecutorDecision::Denied {
            reason: runtime_core::DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink },
        } => {
            assert_eq!(
                sink.0, "git.push",
                "the I0 deny must name the git.push sink id (CommitIrreversible)"
            );
        }
        other => panic!(
            "a draft-session git.push must Deny \
             DraftOnlySessionDeniesCommitIrreversible (I0) — got {other:?}"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LEGS B + C (the differential core): taint is the SOLE variable.
// ─────────────────────────────────────────────────────────────────────────────

/// The anti-stapling / anti-regression differential (§1, T-44-20): remote,
/// refspec, and policy are held BYTE-IDENTICAL across LEG B and LEG C. The ONLY
/// difference is a routing arg's TAINT. Two B sub-legs are proven — a tainted
/// `remote` (clean refspec) and a tainted `refspec` (clean remote) — each Blocking
/// on the NAMED arg specifically. LEG C (both args clean) is Allowed at the
/// executor. A block-everything I2 regression would fail LEG C; a blanket-allow
/// regression would fail either B sub-leg.
#[test]
fn legs_b_and_c_taint_is_the_sole_variable() {
    let (conn, mut store, session_id, head_id, head_hash) = setup();

    // ── B1: tainted REMOTE, clean refspec ──
    // A shared CLEAN refspec handle reused across B1 and C (byte-identical route).
    let (refspec_clean, head_id, head_hash) =
        mint_clean(&conn, &mut store, session_id, REFSPEC, head_id, &head_hash);
    // A shared CLEAN remote handle reused across B2 and C.
    let (remote_clean, head_id, head_hash) =
        mint_clean(&conn, &mut store, session_id, REMOTE, head_id, &head_hash);
    // The tainted remote (SAME literal as remote_clean; only taint differs).
    let (remote_tainted, head_id, head_hash) =
        mint_tainted(&conn, &mut store, session_id, REMOTE, head_id, &head_hash);
    // The tainted refspec (SAME literal as refspec_clean; only taint differs).
    let (refspec_tainted, _head_id, _head_hash) =
        mint_tainted(&conn, &mut store, session_id, REFSPEC, head_id, &head_hash);

    let node_b1 = push_node(&remote_tainted, &refspec_clean);
    let node_b2 = push_node(&remote_clean, &refspec_tainted);
    let node_c = push_node(&remote_clean, &refspec_clean);

    // ── The "taint is the sole variable" property, asserted LITERALLY ──
    // B1 vs C: the refspec handle is byte-identical; only the remote's taint
    // differs (same literal). B2 vs C: the remote handle is byte-identical; only
    // the refspec's taint differs.
    let b1_refspec = node_b1.args.iter().find(|a| a.name == "refspec").unwrap();
    let c_refspec = node_c.args.iter().find(|a| a.name == "refspec").unwrap();
    assert_eq!(
        b1_refspec.value_id, c_refspec.value_id,
        "B1 refspec handle must be BYTE-IDENTICAL to C (only the remote's taint differs)"
    );
    let b2_remote = node_b2.args.iter().find(|a| a.name == "remote").unwrap();
    let c_remote = node_c.args.iter().find(|a| a.name == "remote").unwrap();
    assert_eq!(
        b2_remote.value_id, c_remote.value_id,
        "B2 remote handle must be BYTE-IDENTICAL to C (only the refspec's taint differs)"
    );
    // Resolved literals are equal across the tainted and clean handles (only
    // taint differs) — defense in depth over the handle checks.
    let b1_remote = node_b1.args.iter().find(|a| a.name == "remote").unwrap();
    assert_eq!(
        store.resolve(&b1_remote.value_id).unwrap().literal,
        store.resolve(&c_remote.value_id).unwrap().literal,
        "the tainted-remote and clean-remote literals are identical — only taint differs"
    );
    assert_eq!(store.resolve(&c_remote.value_id).unwrap().literal, REMOTE);
    let b2_refspec = node_b2.args.iter().find(|a| a.name == "refspec").unwrap();
    assert_eq!(
        store.resolve(&b2_refspec.value_id).unwrap().literal,
        store.resolve(&c_refspec.value_id).unwrap().literal,
        "the tainted-refspec and clean-refspec literals are identical — only taint differs"
    );
    assert_eq!(store.resolve(&c_refspec.value_id).unwrap().literal, REFSPEC);
    // The tainted handles are genuinely untrusted; the clean handles are not.
    assert!(
        store.resolve(&remote_tainted).unwrap().taint.iter().any(|t| t.is_untrusted()),
        "B1 remote must be genuinely untrusted (why it Blocks)"
    );
    assert!(
        store.resolve(&refspec_tainted).unwrap().taint.iter().any(|t| t.is_untrusted()),
        "B2 refspec must be genuinely untrusted (why it Blocks)"
    );
    assert!(
        !store.resolve(&remote_clean).unwrap().taint.iter().any(|t| t.is_untrusted()),
        "C remote must be clean (UserTrusted)"
    );
    assert!(
        !store.resolve(&refspec_clean).unwrap().taint.iter().any(|t| t.is_untrusted()),
        "C refspec must be clean (UserTrusted)"
    );

    // ── ONE policy, ONE status — held identical across all submissions ──
    let policy = SessionPolicy::broker_default();
    let status = SessionStatus::Active;

    // LEG B1: tainted remote → Block, anchor names `remote`.
    assert_block_on_arg(
        &executor::submit_plan_node(
            session_id,
            Uuid::new_v4(),
            &node_b1,
            &store,
            &status,
            &policy,
        ),
        "remote",
    );

    // LEG B2: tainted refspec → Block, anchor names `refspec`.
    assert_block_on_arg(
        &executor::submit_plan_node(
            session_id,
            Uuid::new_v4(),
            &node_b2,
            &store,
            &status,
            &policy,
        ),
        "refspec",
    );

    // LEG C: clean remote + refspec, SAME policy/status → Allowed at the executor
    // (the broker always-confirm-gate then re-gates it — see the Linux dispatch
    // leg below; the executor decision itself is Allowed, taint being the SOLE
    // variable that flips B → Block).
    assert_eq!(
        executor::submit_plan_node(
            session_id,
            Uuid::new_v4(),
            &node_c,
            &store,
            &status,
            &policy,
        ),
        ExecutorDecision::Allowed,
        "LEG C (clean args, IDENTICAL remote/refspec/policy to LEG B) must be Allowed \
         at the executor — taint is the SOLE variable that flips the outcome"
    );

    // The differential holds AND the audit chain is intact (genuine DAG).
    assert!(
        verify_chain(&conn, &session_id.to_string(), TEST_KEY),
        "the audit chain must remain intact (verify_chain) across the mints"
    );
}

/// Assert `decision` is a `BlockedPendingConfirmation` with a single anchor naming
/// `arg`, riding a GENUINE (non-empty, non-stapled) provenance chain whose root
/// equals `read_event_id`.
fn assert_block_on_arg(decision: &ExecutorDecision, arg: &str) {
    match decision {
        ExecutorDecision::BlockedPendingConfirmation { anchors } => {
            assert_eq!(
                anchors.len(),
                1,
                "only the tainted {arg} should Block — the other arg is clean; got {anchors:?}"
            );
            assert_eq!(
                anchors[0].anchor.arg, arg,
                "the Block anchor must name the `{arg}` arg specifically"
            );
            assert_eq!(
                anchors[0].anchor.sink.0, "git.push",
                "the Block anchor must name the git.push sink id"
            );
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
            "a tainted `{arg}` git.push must BlockPendingConfirmation on `{arg}` — got {other:?}"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LEG D (structural denial): --force / :delete refused BY CONSTRUCTION.
// ─────────────────────────────────────────────────────────────────────────────

/// The force/deletion refusal is a VALUE-level structural denial (§1.3, RESEARCH
/// §5), unreachable even via a human confirm (T-44-24). Two defense-in-depth
/// layers, both proven here host-portable over the REAL shipped functions:
///   1. `validate_git_refspec` rejects a leading `+` (force), a `--force`/flag-
///      shaped token, and an empty `<src>` (`:dst` deletion);
///   2. `build_command_list` refuses to construct a line whose `<new-oid>` is the
///      zero-oid (a delete) for ANY input, while DISTINGUISHING + allowing a
///      CREATE (zero `<old-oid>`, non-zero `<new-oid>`).
/// The pushed argument-value can never express a force update or a deletion, so a
/// human who confirms a specific push never confirms a history rewrite.
#[test]
fn leg_d_force_and_delete_refused_by_construction() {
    // Layer 1: the refspec value-gate.
    assert!(
        validate_git_refspec("refs/heads/main:refs/heads/main").is_ok(),
        "a plain non-force <src>:<dst> refspec is accepted"
    );
    assert!(
        validate_git_refspec("+refs/heads/main:refs/heads/main").is_err(),
        "a leading '+' (force) refspec is refused by construction"
    );
    assert!(
        validate_git_refspec("--force").is_err(),
        "a --force flag-shaped refspec token is refused"
    );
    assert!(
        validate_git_refspec("--force-with-lease").is_err(),
        "a --force-with-lease flag-shaped refspec token is refused"
    );
    assert!(
        validate_git_refspec(":refs/heads/main").is_err(),
        "a deletion refspec (empty <src> / bare ':dst') is refused"
    );

    // Layer 2: the receive-pack command-list builder.
    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";
    const ZERO: &str = "0000000000000000000000000000000000000000";
    assert!(
        build_command_list(OLD, NEW, "refs/heads/main").is_ok(),
        "a plain update command line is constructible"
    );
    assert!(
        build_command_list(OLD, ZERO, "refs/heads/main").is_err(),
        "a zero <new-oid> (delete) command line is refused by construction"
    );
    // A CREATE (zero <old-oid>, non-zero <new-oid>) is DISTINGUISHED and allowed —
    // the refusal keys on <new-oid> == zero ONLY.
    assert!(
        build_command_list(ZERO, NEW, "refs/heads/brand-new").is_ok(),
        "a create (zero <old-oid>, non-zero <new-oid>) is allowed — refusal keys on new-oid only"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// DISPATCH-LEVEL legs (Linux + mock-egress-ca): the clean confirmed push REACHES
// the pinned mock git-receive-pack (LEG C delivery + credential absence), and a
// receive-pack redirect is REFUSED (LEG E).
//
// These drive the ACTUAL production always-confirm-gate + confirm-release Step-7
// dispatch via `evaluate_plan_node_and_record_for_test` (the `test-fixtures`-gated
// VERBATIM delegate to the real arm) + `confirmation::confirm`. The confined `git`
// children (rev-parse / pack-objects) self-confine only under
// `#[cfg(target_os = "linux")]`; the socket is a REAL TLS connection to the mock
// (`github-mock.caprun.test`, trusted only under the non-default `mock-egress-ca`
// feature). Real verification is the compose-verify Linux gate with the mock
// git-receive-pack sidecar reachable (`cargo build --workspace` first — the
// confined children resolve the `caprun-exec-launcher` sibling binary).
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]
mod dispatch {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Count durable events of `event_type` in `session_id`.
    fn count_events(conn: &rusqlite::Connection, session_id: Uuid, event_type: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2",
            rusqlite::params![session_id.to_string(), event_type],
            |row| row.get(0),
        )
        .unwrap()
    }

    /// Assert `needle` appears in NO hashed event payload for `session_id`.
    fn assert_absent_from_all_payloads(conn: &rusqlite::Connection, session_id: Uuid, needle: &str) {
        let mut stmt = conn
            .prepare("SELECT payload FROM events WHERE session_id = ?1")
            .unwrap();
        let rows = stmt
            .query_map(rusqlite::params![session_id.to_string()], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        for payload in rows {
            let payload = payload.unwrap();
            assert!(
                !payload.contains(needle),
                "`{needle}` must NEVER appear in any hashed event payload (opaque audit, T-44-10)"
            );
        }
    }

    /// Build a temp workspace that IS a git repo with one commit on branch
    /// `main`, so the confined `git rev-parse main^{{commit}}` (freeze) + `git
    /// pack-objects` resolve a real oid + pack. Runs the SETUP git UNCONFINED in
    /// the test process. Mirrors the server.rs `setup_git_push_repo` helper.
    fn setup_git_push_repo(tag: &str) -> (std::path::PathBuf, Arc<adapter_fs::workspace::WorkspaceRoot>) {
        use std::process::Command;
        let git = |dir: &std::path::Path, args: &[&str]| -> bool {
            Command::new("git")
                .args(args)
                .current_dir(dir)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .expect("spawn setup git")
                .status
                .success()
        };
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_s44_gitpush_{tag}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(git(&root, &["init", "-q"]), "git init");
        std::fs::write(root.join("f.txt"), b"hello\n").unwrap();
        assert!(git(&root, &["add", "f.txt"]), "git add");
        assert!(git(&root, &["commit", "-q", "-m", "init"]), "git commit");
        assert!(git(&root, &["branch", "-M", "main"]), "git branch -M main");
        let ws = Arc::new(adapter_fs::workspace::WorkspaceRoot::open(&root).unwrap());
        (root, ws)
    }

    /// Drive a clean git.push through the REAL always-confirm-gate + confirm-
    /// release, targeting a `remote`. Returns the owned `Connection` (for
    /// post-hoc assertions), the `session_id`, and the confirm `outcome`.
    async fn evaluate_and_confirm(
        remote: &str,
    ) -> (rusqlite::Connection, Uuid, brokerd::confirmation::ConfirmOutcome, std::path::PathBuf) {
        use runtime_core::SessionStatus;
        let (repo, ws) = setup_git_push_repo("dispatch");

        let conn = open_audit_db(":memory:").unwrap();
        let (session_id, root_id, root_hash) = seed_session(&conn);
        let session_id_str = session_id.to_string();

        // Clean args minted through the REAL UserTrusted path (no demotion →
        // session stays Active → executor Allows → the broker always-confirm-gate
        // re-gates it to BlockedPendingConfirmation + freezes the new-oid).
        let mut store = ValueStore::default();
        let (remote_vid, head_id, head_hash) =
            mint_clean(&conn, &mut store, session_id, remote, root_id, &root_hash);
        let (refspec_vid, head_id, head_hash) =
            mint_clean(&conn, &mut store, session_id, REFSPEC, head_id, &head_hash);
        let node = push_node(&remote_vid, &refspec_vid);

        let conn = Arc::new(Mutex::new(conn));
        let mut last_event_id = head_id;
        let mut last_event_hash = head_hash;

        let (decision, output_value_id, _demoted) =
            brokerd::server::evaluate_plan_node_and_record_for_test(
                &node,
                &conn,
                TEST_KEY,
                session_id,
                &mut store,
                &ws,
                &SessionStatus::Active,
                &SessionPolicy::broker_default(),
                &mut last_event_id,
                &mut last_event_hash,
            )
            .await
            .expect("evaluate clean git.push");

        // The clean push is NEVER auto-dispatched — the always-confirm-gate
        // re-gates it (there is no bare Allowed→dispatch arm for git.push).
        assert!(
            matches!(decision, ExecutorDecision::BlockedPendingConfirmation { .. }),
            "a clean git.push MUST be re-gated to BlockedPendingConfirmation \
             (always-confirm-gate), never auto-dispatched: got {decision:?}"
        );
        assert!(output_value_id.is_none(), "git.push mints nothing");

        // No socket opened yet — the gate freezes the oid but dispatches nothing.
        {
            let locked = conn.lock().unwrap();
            assert_eq!(
                count_events(&locked, session_id, "git_push_succeeded")
                    + count_events(&locked, session_id, "git_push_failed"),
                0,
                "the confirm-gate opens NO socket — no git_push_* terminal event yet"
            );
        }

        let effect_id: String = {
            let locked = conn.lock().unwrap();
            locked
                .query_row("SELECT effect_id FROM pending_confirmations", [], |r| r.get(0))
                .expect("one pending git.push row")
        };

        // Take sole ownership of the Connection so confirm() (which needs a
        // `&mut Connection`) can run against the SAME db.
        let mut conn_owned = Arc::try_unwrap(conn)
            .expect("sole Arc owner after evaluate returned")
            .into_inner()
            .expect("mutex not poisoned");

        let outcome = brokerd::confirmation::confirm(&mut conn_owned, TEST_KEY, &effect_id, &ws)
            .await
            .expect("confirm completes (not a transport-level Err)");

        // The chain stays unbroken across the whole gate → confirm → dispatch.
        assert!(
            verify_chain(&conn_owned, &session_id_str, TEST_KEY),
            "verify_chain must hold across the confirm-release dispatch"
        );

        (conn_owned, session_id, outcome, repo)
    }

    /// LEG C (delivery): a CLEAN confirmed push REACHES the pinned mock
    /// git-receive-pack (the `/accept/*` repo the mock serves + accepts), the
    /// mock records the receipt, and the broker appends an opaque
    /// `git_push_succeeded` terminal event (its `Released` outcome + the report-
    /// status success is delivery PROOF — not merely "not blocked"). Credential /
    /// remote-URL absence: after the real push, neither the broker-env push token
    /// nor the remote URL appears in any hashed audit payload (opaque audit,
    /// T-44-10 / DESIGN §1.4). `verify_chain` holds.
    #[tokio::test]
    async fn leg_c_clean_confirmed_push_reaches_mock_receive_pack() {
        // A distinctive broker-env push credential — read ONLY from the broker's
        // process env, set ONLY on the receive-pack POST (Basic x-access-token),
        // NEVER a plan arg / ValueNode / audit literal. The sentinel lets the
        // credential-absence assertion below be unambiguous.
        const TOKEN_SENTINEL: &str = "SENTINEL-caprun-git-push-token-do-not-leak";
        std::env::set_var("CAPRUN_GIT_PUSH_TOKEN", TOKEN_SENTINEL);

        let (conn, session_id, outcome, repo) = evaluate_and_confirm(REMOTE).await;

        std::env::remove_var("CAPRUN_GIT_PUSH_TOKEN");

        // Delivery: the confirm RELEASED and the push was accepted by the mock.
        assert_eq!(
            outcome,
            brokerd::confirmation::ConfirmOutcome::Released,
            "the clean confirmed push must be RELEASED — delivered to the mock \
             git-receive-pack with a clean report-status"
        );
        assert_eq!(
            count_events(&conn, session_id, "git_push_succeeded"),
            1,
            "the delivered push must append exactly one opaque git_push_succeeded \
             terminal event (the mock's report-status success = receipt)"
        );
        assert_eq!(
            count_events(&conn, session_id, "git_push_failed"),
            0,
            "a delivered push must NOT record a failure terminal"
        );
        // The clean leg did NOT Block at dispatch (a block-everything regression
        // would leave no git_push_succeeded — fails the differential, T-44-20).

        // Credential absence: the push token appears in NO hashed audit payload
        // (it is broker-env-only, never audited). The remote URL is OPAQUE in the
        // audit too (T-44-10) — it rides no hashed event payload.
        assert_absent_from_all_payloads(&conn, session_id, TOKEN_SENTINEL);
        assert_absent_from_all_payloads(&conn, session_id, REMOTE);
        // Belt-and-suspenders: the credential never appears anywhere in the audit
        // db text (payloads OR actor columns).
        let token_hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE payload LIKE ?1 OR actor LIKE ?1",
                rusqlite::params![format!("%{TOKEN_SENTINEL}%")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(token_hits, 0, "the push credential must never touch the audit db");

        std::fs::remove_dir_all(&repo).ok();
    }

    /// LEG E (destination pin / redirect-none): a receive-pack info/refs 3xx is
    /// REFUSED — the frozen redirect-none client surfaces the 302 as a non-success
    /// status and the push fails closed, folding into a terminal git_push_failed
    /// FIRST (never followed to the redirect target). The confirm still RELEASES
    /// to Step-7 (confirm_granted appended) — the refusal is a transport failure,
    /// not a pre-dispatch block.
    #[tokio::test]
    async fn leg_e_receive_pack_redirect_is_refused() {
        // The `/redirect/*` repo makes the mock 302 the info/refs GET.
        const REDIRECT_REMOTE: &str = "https://github-mock.caprun.test/redirect/repo.git";

        let (conn, session_id, outcome, repo) = evaluate_and_confirm(REDIRECT_REMOTE).await;

        assert_eq!(
            outcome,
            brokerd::confirmation::ConfirmOutcome::ConfirmedButSinkFailed,
            "a refused redirect is a transport failure — the confirm releases to \
             Step-7 but the sink fails (ConfirmedButSinkFailed)"
        );
        assert_eq!(
            count_events(&conn, session_id, "git_push_succeeded"),
            0,
            "a refused redirect must NEVER be followed to a success"
        );
        assert_eq!(
            count_events(&conn, session_id, "git_push_failed"),
            1,
            "the refused redirect folds into exactly one terminal git_push_failed"
        );
        // The confirm DID release to Step-7 (the redirect refusal is downstream of
        // the confirm gate, not a pre-dispatch block).
        assert_eq!(
            count_events(&conn, session_id, "confirm_granted"),
            1,
            "the confirm released to Step-7 before the transport-level refusal"
        );

        std::fs::remove_dir_all(&repo).ok();
    }
}
