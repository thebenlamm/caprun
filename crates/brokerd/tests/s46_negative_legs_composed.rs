//! Phase 46 (LIVE-06) COMPOSED NEGATIVE-LEG acceptance test — the v1.9 DONE-gate
//! proof that the five negative mechanisms are each INDEPENDENTLY ATTRIBUTABLE,
//! run in ONE composed sequence over ONE shared persisted `audit.db`.
//!
//! # Why composed + independently attributable (RESEARCH § "The 5 negative legs")
//!
//! LIVE-06 requires each defensive mechanism be SEPARATELY attributable with a
//! DISTINCT machine-checkable tag asserted SEPARATELY — proving policy narrows
//! WHICH sinks are callable but NEVER disables I2 (POLICY-02, locked), and that
//! the destination pin + credential custody hold live. A single "it blocked"
//! assertion cannot tell a policy-deny from an I2-Block from a transport refusal;
//! this test pins each to its own tag.
//!
//! # The five negative legs, each with its DISTINCT tag
//!
//!   * LEG 1 — tainted push `remote` (I2): a genuinely-tainted git.push routing arg
//!     (real `mint_from_http` provenance) under a policy that EXPLICITLY PERMITS
//!     git.push → `BlockedPendingConfirmation` on the tainted arg + a durable
//!     `sink_blocked` EVENT. TAG: a `sink_blocked` event whose anchor names the arg.
//!   * LEG 2 — tainted POST `body` (I2): a genuinely-tainted `http.request.write`
//!     body under a policy that EXPLICITLY PERMITS http.request.write → Block on
//!     `body` + a `sink_blocked` event + NO `http_write_*` terminal (never writes).
//!     TAG: a `sink_blocked` event; the write is never attempted.
//!   * LEG 3 — policy-deny (NOT I2): a sink OMITTED from the session policy (while
//!     git.push + http.request.write ARE permitted) → `Denied{PolicyDeny}` with
//!     `reason.code()=="policy_deny"`, recorded as the GENERIC `plan_node_evaluated`
//!     event with NO `sink_blocked`. TAG: `code()=="policy_deny"` + no sink_blocked.
//!     Asserted SEPARATELY from the I2 legs' `sink_blocked` tag — proving policy-deny
//!     and I2-Block are provably-distinct mechanisms (POLICY-02, locked decision #3).
//!   * LEG 4 — destination-pin negative: a push whose `/redirect/*` remote makes the
//!     mock 302 the info/refs GET; the frozen redirect-none client REFUSES it (never
//!     followed) → `ConfirmedButSinkFailed` + exactly one terminal `git_push_failed`.
//!     TAG: `ConfirmedButSinkFailed` + one git_push_failed, zero git_push_succeeded.
//!   * LEG 5 — credential absence, TWO sub-assertions on TWO distinct pushes:
//!       - 5a (value store + audit chain): after a REAL clean confirmed 200 push,
//!         the sentinel token + the remote URL are absent from every event payload
//!         AND the actor columns. TAG: absence across the audit DB.
//!       - 5b (broker log): on the ERROR-PATH push (the LEG-4 redirect-refused push,
//!         where `scrub_secrets`→`eprintln!` at git_push.rs:784 ACTUALLY fires — the
//!         clean 200 push takes the Ok arm and emits NO log, so a log check on it is
//!         VACUOUS), the FD-2-captured broker stderr contains NEITHER the sentinel
//!         token NOR the raw remote host/URL. TAG: absence in the captured stderr,
//!         with the eprintln marker present (non-vacuous).
//!
//! # Gating (CLAUDE.md — all v0 security claims are Linux-only)
//!
//! The composed run is ONE sequential `#[tokio::test]` fn under
//! `#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]`. Although legs 2
//! and 3 are host-portable in ISOLATION (and are already proven host-portable in
//! `s43_http_write_differential.rs` / the `policy_gate` unit tests), the COMPOSED
//! single-fn run is gated at the UNION because legs 1/4/5 exercise the confined
//! `git` children (block-time oid freeze) and the pinned mock git-receive-pack
//! socket — a gate that needs the `mock-egress-ca` trust anchor + mock host. A
//! single sequential fn is mandatory (G6): leg 5 mutates the process-global
//! `CAPRUN_GIT_PUSH_TOKEN` env var and leg 5b dup2's the process-global FD 2, so
//! the legs must NEVER run in parallel with anything reading either. On the macOS
//! dev box the composed fn is cfg-excluded (0 run — EXPECTED, not a gap); the
//! host-portable guard below keeps `cargo test -p brokerd` meaningful there. The
//! authoritative gate is `scripts/compose-verify.sh --features brokerd/mock-egress-ca`.

#![cfg(test)]

// ─────────────────────────────────────────────────────────────────────────────
// Host-portable guard: keeps `cargo test -p brokerd --test
// s46_negative_legs_composed` meaningful on the macOS dev box, where the Linux +
// mock-egress-ca composed body is cfg-excluded (0 tests reported — expected, not a
// gap). Mirrors the v1.8 composed guard (`live_acceptance_v1_8_composed.rs:961`).
// ─────────────────────────────────────────────────────────────────────────────

/// The composed negative-leg proof is Linux + mock-egress-ca only (see the module
/// doc). This host guard asserts the test binary is wired into the build so a
/// green macOS `cargo test` is a real signal, not a silently-empty target.
#[test]
fn s46_negative_legs_composed_guard_present() {
    // A trivially-true assertion whose PURPOSE is to make this a non-empty test
    // target on every host — the real negative-leg proof runs on the Linux
    // compose-verify gate.
    assert!(
        cfg!(test),
        "the s46 negative-leg composed target must be built on every host"
    );
}

#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]
mod composed {
    use brokerd::audit::{append_event, current_chain_head, open_audit_db, verify_chain};
    use brokerd::quarantine::{mint_from_http, mint_from_intent};
    use brokerd::session::{create_session, persist_session};
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::plan_node::{PlanArg, SinkId, ValueId};
    use runtime_core::{
        DenyReason, Event, ExecutorDecision, PlanNode, SeedProvenance, SessionPolicy, SessionStatus,
    };
    use std::os::unix::io::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// A distinctive broker-env push credential — read ONLY from the broker's
    /// process env, set ONLY on the receive-pack POST (Basic x-access-token),
    /// NEVER a plan arg / ValueNode / audit literal. The sentinel makes the
    /// credential-absence assertions (5a value store/audit, 5b broker log)
    /// unambiguous.
    const TOKEN_SENTINEL: &str = "SENTINEL-caprun-s46-push-token-do-not-leak";
    /// The clean-push (LEG 5a) remote — the `/accept/*` path the mock
    /// git-receive-pack serves + accepts (a genuine 200 delivery).
    const REMOTE: &str = "https://github-mock.caprun.test/accept/repo.git";
    /// The redirect (LEG 4 / LEG 5b) remote — the `/redirect/*` path makes the mock
    /// 302 the info/refs GET, so the frozen redirect-none client refuses it.
    const REDIRECT_REMOTE: &str = "https://github-mock.caprun.test/redirect/repo.git";
    /// The raw remote host — asserted ABSENT from the LEG-5b captured broker log.
    const MOCK_HOST: &str = "github-mock.caprun.test";
    /// A plain non-force `<src>:<dst>` push refspec (held clean across every leg).
    const REFSPEC: &str = "refs/heads/main:refs/heads/main";
    /// The `http.request.write` URL (LEG 2, held identical to the s43 differential).
    const WRITE_URL: &str = "https://mock-write.caprun.test/ingest";
    /// The `http.request.write` verb (LEG 2).
    const WRITE_METHOD: &str = "POST";
    /// The request-body literal minted UNTRUSTED for LEG 2 (canonical exfil shape).
    const BODY_LITERAL: &str = "{\"summary\":\"quarterly figures for review\"}";

    // ── shared-DB / key seeding (mirror live_acceptance_v1_8_composed.rs) ──────

    /// Mint/read the shared HMAC key as a SIBLING `<db>.key` file so every
    /// in-process `append_event`/`verify_chain` MACs against the SAME key. Seeded
    /// ONCE, before any leg appends. F1-safe: the key file is a sibling of the
    /// audit.db, never a child of a WorkspaceRoot.
    fn seed_test_key(db_path: &Path) -> Vec<u8> {
        let key_path = PathBuf::from(format!("{}.key", db_path.to_str().unwrap()));
        if let Ok(bytes) = std::fs::read(&key_path) {
            return bytes;
        }
        let mut key = Uuid::new_v4().as_bytes().to_vec();
        key.extend_from_slice(Uuid::new_v4().as_bytes());
        std::fs::write(&key_path, &key).expect("write test MAC key file");
        key
    }

    /// Session discovery safe for a multi-session shared DB (mirror the v1.7/v1.8
    /// `all_session_ids` — never the unqualified no-`ORDER BY` `LIMIT 1`
    /// anti-pattern).
    fn all_session_ids(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT id FROM sessions ORDER BY rowid")
            .expect("prepare all_session_ids query");
        stmt.query_map([], |row| row.get(0))
            .expect("query all_session_ids")
            .filter_map(Result::ok)
            .collect()
    }

    /// Persist a fresh Active session on `conn` and seed its `session_created`
    /// causal root. Returns `(session_id, root_event_id, root_hash)`. Every leg
    /// seeds its OWN session on the shared DB so each mechanism is independently
    /// attributable (its own session, its own chain).
    fn seed_session(conn: &rusqlite::Connection, key: &[u8]) -> (Uuid, Uuid, String) {
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
        let root_hash = append_event(conn, key, &root, None).unwrap();
        (session.id, root.id, root_hash)
    }

    /// Mint a CLEAN (`UserTrusted`) value through the REAL broker UserTrusted mint
    /// path (`mint_from_intent`), threading the causal chain head forward.
    fn mint_clean(
        conn: &rusqlite::Connection,
        key: &[u8],
        store: &mut ValueStore,
        session_id: Uuid,
        literal: &str,
        parent_id: Uuid,
        parent_hash: &str,
    ) -> (ValueId, Uuid, String) {
        let (event_id, hash, value_id) = mint_from_intent(
            conn,
            key,
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

    /// Mint a genuinely-TAINTED value through the REAL broker http-taint mint path
    /// (`mint_from_http`) — `provenance_chain[0]` is a genuine
    /// `http_response_received` event (NON-STAPLED). Returns the value handle plus
    /// the new chain head (the LAST appended `session_demoted` event). NOTE: this
    /// demotes the persisted session to Draft (I1); the block legs pass
    /// `SessionStatus::Active` EXPLICITLY so the Block under test is TAINT-driven
    /// (I2), not a draft-session gate.
    fn mint_tainted(
        conn: &rusqlite::Connection,
        key: &[u8],
        store: &mut ValueStore,
        session_id: Uuid,
        literal: &str,
        parent_id: Uuid,
        parent_hash: &str,
    ) -> (ValueId, Uuid, String) {
        let (_event_id, _event_hash, value_id, chain_head_id, chain_head_hash) = mint_from_http(
            conn,
            key,
            store,
            session_id,
            literal.to_string(),
            Some(parent_id),
            Some(parent_hash),
        )
        .expect("mint_from_http (untrusted value) must succeed");
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
                "`{needle}` must NEVER appear in any hashed event payload (opaque audit)"
            );
        }
    }

    /// Build a temp workspace that IS a git repo with one commit on branch `main`,
    /// so the confined `git rev-parse main^{{commit}}` (block-time freeze) + `git
    /// pack-objects` resolve a real oid + pack. Runs the SETUP git UNCONFINED in
    /// the test process (mirror `s44_git_push_differential.rs::setup_git_push_repo`).
    fn setup_git_push_repo(tag: &str) -> (PathBuf, Arc<adapter_fs::workspace::WorkspaceRoot>) {
        use std::process::Command;
        let git = |dir: &Path, args: &[&str]| -> bool {
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
        root.push(format!("caprun_s46_gitpush_{tag}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(git(&root, &["init", "-q"]), "git init");
        std::fs::write(root.join("f.txt"), b"hello\n").unwrap();
        assert!(git(&root, &["add", "f.txt"]), "git add");
        assert!(git(&root, &["commit", "-q", "-m", "init"]), "git commit");
        assert!(git(&root, &["branch", "-M", "main"]), "git branch -M main");
        let ws = Arc::new(adapter_fs::workspace::WorkspaceRoot::open(&root).unwrap());
        (root, ws)
    }

    /// Drive a clean git.push (the SAME literal remote/refspec) through the REAL
    /// always-confirm-gate + confirm-release Step-7 dispatch, over the SHARED
    /// persisted `audit_db`. Seeds its OWN session (returned + tracked by the
    /// caller for the final sweep). Returns `(session_id, confirm outcome, repo)`.
    /// The connection is dropped before return so the shared DB is flushed for the
    /// next leg + the final sweep.
    async fn evaluate_and_confirm(
        audit_db_str: &str,
        key: &[u8],
        remote: &str,
    ) -> (Uuid, brokerd::confirmation::ConfirmOutcome, PathBuf) {
        let (repo, ws) = setup_git_push_repo("dispatch");

        let conn = open_audit_db(audit_db_str).expect("open shared audit db (push leg)");
        let (session_id, root_id, root_hash) = seed_session(&conn, key);
        let session_id_str = session_id.to_string();

        // Clean args minted through the REAL UserTrusted path (no demotion →
        // session stays Active → executor Allows → the broker always-confirm-gate
        // re-gates it to BlockedPendingConfirmation + freezes the new-oid).
        let mut store = ValueStore::default();
        let (remote_vid, head_id, head_hash) =
            mint_clean(&conn, key, &mut store, session_id, remote, root_id, &root_hash);
        let (refspec_vid, head_id, head_hash) =
            mint_clean(&conn, key, &mut store, session_id, REFSPEC, head_id, &head_hash);
        let node = push_node(&remote_vid, &refspec_vid);

        let conn = Arc::new(Mutex::new(conn));
        let mut last_event_id = head_id;
        let mut last_event_hash = head_hash;

        let (decision, output_value_id, _demoted) =
            brokerd::server::evaluate_plan_node_and_record_for_test(
                &node,
                &conn,
                key,
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

        // Fetch the effect_id for THIS session's pending confirmation. The query
        // is session-SCOPED — the shared DB may hold other legs' pending rows
        // (e.g. LEG 1's tainted-block frozen-oid row), so an unqualified SELECT
        // would be ambiguous.
        let effect_id: String = {
            let locked = conn.lock().unwrap();
            locked
                .query_row(
                    "SELECT effect_id FROM pending_confirmations WHERE session_id = ?1",
                    rusqlite::params![session_id_str],
                    |r| r.get(0),
                )
                .expect("one pending git.push row for this session")
        };

        let mut conn_owned = Arc::try_unwrap(conn)
            .expect("sole Arc owner after evaluate returned")
            .into_inner()
            .expect("mutex not poisoned");

        let outcome = brokerd::confirmation::confirm(&mut conn_owned, key, &effect_id, &ws)
            .await
            .expect("confirm completes (not a transport-level Err)");

        assert!(
            verify_chain(&conn_owned, &session_id_str, key),
            "verify_chain must hold across the confirm-release dispatch"
        );

        drop(conn_owned);
        (session_id, outcome, repo)
    }

    /// The composed v1.9 negative-leg proof (LIVE-06) — the milestone DONE gate's
    /// negative clause. All five legs run sequentially in ONE test fn over ONE
    /// shared persisted `audit.db` (single-threaded → env + FD-2 mutation and the
    /// shared DB path are race-free), each leg its own session; the final sweep
    /// asserts EXACTLY the five negative-leg sessions exist and every `verify_chain`
    /// is independently true.
    #[tokio::test]
    async fn s46_negative_legs_composed_all_legs() {
        let run_id = Uuid::new_v4();
        let tmp = std::env::temp_dir().join(format!("caprun_s46_neg_{run_id}"));
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        // F1-safe: audit.db is a SIBLING of any workspace/git-repo roots (each leg
        // makes its own repo under std::env::temp_dir()), never a child of a
        // WorkspaceRoot.
        let audit_db = tmp.join("audit.db"); // ONE shared path — NEVER :memory:
        let audit_db_str = audit_db.to_str().unwrap();

        // Mint/persist the shared MAC key ONCE, before any leg appends.
        let key = seed_test_key(&audit_db);

        // Track every negative-leg session so the final sweep asserts the exact set.
        let mut expected_sessions: Vec<Uuid> = Vec::new();

        // ── LEG 1 — tainted push `remote` I2-Blocks under a git.push-PERMITTING
        //    policy → a durable `sink_blocked` event (anchor names the tainted
        //    arg). Taint is GENUINE (real mint_from_http provenance), not stapled.
        let leg1_session = {
            let (repo, ws) = setup_git_push_repo("leg1");
            let conn = open_audit_db(audit_db_str).expect("open shared db (leg1)");
            let (session_id, root_id, root_hash) = seed_session(&conn, &key);
            expected_sessions.push(session_id);
            let session_id_str = session_id.to_string();

            let mut store = ValueStore::default();
            // A CLEAN refspec (byte-identical route) + a genuinely-TAINTED remote —
            // taint is the SOLE reason the Block fires (mirrors the s44 B1 shape).
            let (refspec_vid, head_id, head_hash) =
                mint_clean(&conn, &key, &mut store, session_id, REFSPEC, root_id, &root_hash);
            let (remote_tainted, _hid, _hh) =
                mint_tainted(&conn, &key, &mut store, session_id, REMOTE, head_id, &head_hash);
            assert!(
                store
                    .resolve(&remote_tainted)
                    .unwrap()
                    .taint
                    .iter()
                    .any(|t| t.is_untrusted()),
                "LEG 1 remote must be genuinely untrusted (why it Blocks)"
            );
            let node = push_node(&remote_tainted, &refspec_vid);

            // Re-read the chain head AFTER the tainted mint's session_demoted
            // appends, so the dispatch threads onto the true head.
            let (head_id, head_hash) = current_chain_head(&conn, &session_id_str)
                .expect("chain head query")
                .expect("chain head after mints");

            let conn = Arc::new(Mutex::new(conn));
            let mut last_event_id = head_id;
            let mut last_event_hash = head_hash;

            let (decision, ovid, _demoted) =
                brokerd::server::evaluate_plan_node_and_record_for_test(
                    &node,
                    &conn,
                    &key,
                    session_id,
                    &mut store,
                    &ws,
                    // Active: isolate the Block as TAINT-driven (I2), not a draft gate.
                    &SessionStatus::Active,
                    // broker_default() EXPLICITLY PERMITS git.push — so policy is
                    // provably NOT what blocks this leg (POLICY-02 distinctness).
                    &SessionPolicy::broker_default(),
                    &mut last_event_id,
                    &mut last_event_hash,
                )
                .await
                .expect("a Blocked decision is a normal Ok outcome");

            match &decision {
                ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                    assert_eq!(
                        anchors.len(),
                        1,
                        "only the tainted remote Blocks — the refspec is clean; got {anchors:?}"
                    );
                    assert_eq!(anchors[0].anchor.arg, "remote", "anchor names the tainted arg");
                    assert_eq!(anchors[0].anchor.sink.0, "git.push");
                    assert!(
                        !anchors[0].anchor.provenance_chain.is_empty(),
                        "the Block anchor must carry a genuine (non-empty) provenance chain"
                    );
                    assert_eq!(
                        anchors[0].anchor.read_event_id, anchors[0].anchor.provenance_chain[0],
                        "anchor.read_event_id must equal provenance_chain[0] (genuine, non-stapled)"
                    );
                }
                other => panic!("LEG 1 tainted remote must Block on `remote` — got {other:?}"),
            }
            assert!(ovid.is_none(), "a Blocked git.push mints nothing");

            let locked = conn.lock().unwrap();
            assert_eq!(
                count_events(&locked, session_id, "sink_blocked"),
                1,
                "LEG 1 records exactly one sink_blocked (I2 Block) under a git.push-PERMITTING policy"
            );
            assert_eq!(
                count_events(&locked, session_id, "git_push_succeeded"),
                0,
                "a Blocked push never dispatches — no git_push_succeeded"
            );
            assert_eq!(
                count_events(&locked, session_id, "git_push_failed"),
                0,
                "a Blocked push never dispatches — no git_push_failed"
            );
            assert!(
                verify_chain(&locked, &session_id_str, &key),
                "LEG 1 verify_chain must hold across the mint + block"
            );
            drop(locked);
            std::fs::remove_dir_all(&repo).ok();
            session_id
        };

        // ── LEG 2 — tainted POST `body` I2-Blocks under an http.request.write-
        //    PERMITTING policy → a `sink_blocked` event + NO http_write_* terminal
        //    (the write is NEVER attempted). Reuses the s43 dispatch-leg shape.
        let leg2_session = {
            let conn = open_audit_db(audit_db_str).expect("open shared db (leg2)");
            let (session_id, root_id, root_hash) = seed_session(&conn, &key);
            expected_sessions.push(session_id);
            let session_id_str = session_id.to_string();

            let mut store = ValueStore::default();
            let (url, head_id, head_hash) =
                mint_clean(&conn, &key, &mut store, session_id, WRITE_URL, root_id, &root_hash);
            let (method, head_id, head_hash) =
                mint_clean(&conn, &key, &mut store, session_id, WRITE_METHOD, head_id, &head_hash);
            let (body_tainted, _hid, _hh) =
                mint_tainted(&conn, &key, &mut store, session_id, BODY_LITERAL, head_id, &head_hash);
            assert!(
                store
                    .resolve(&body_tainted)
                    .unwrap()
                    .taint
                    .iter()
                    .any(|t| t.is_untrusted()),
                "LEG 2 body must be genuinely untrusted (why it Blocks)"
            );
            let node = write_node(&url, &method, &body_tainted);

            let (head_id, head_hash) = current_chain_head(&conn, &session_id_str)
                .expect("chain head query")
                .expect("chain head after mints");

            let conn = Arc::new(Mutex::new(conn));
            let workspace = Arc::new(
                adapter_fs::workspace::WorkspaceRoot::open(&std::env::temp_dir())
                    .expect("open temp workspace root"),
            );
            let mut last_event_id = head_id;
            let mut last_event_hash = head_hash;

            let (decision, ovid, _demoted) =
                brokerd::server::evaluate_plan_node_and_record_for_test(
                    &node,
                    &conn,
                    &key,
                    session_id,
                    &mut store,
                    &workspace,
                    &SessionStatus::Active,
                    &SessionPolicy::broker_default(),
                    &mut last_event_id,
                    &mut last_event_hash,
                )
                .await
                .expect("a Blocked decision is a normal Ok outcome — the write arm is never entered");

            match &decision {
                ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                    assert_eq!(anchors.len(), 1, "only the tainted body Blocks; got {anchors:?}");
                    assert_eq!(anchors[0].anchor.arg, "body", "anchor names the tainted `body`");
                    assert_eq!(anchors[0].anchor.sink.0, "http.request.write");
                    assert!(
                        !anchors[0].anchor.provenance_chain.is_empty(),
                        "genuine (non-empty) provenance chain"
                    );
                    assert_eq!(
                        anchors[0].anchor.read_event_id, anchors[0].anchor.provenance_chain[0],
                        "genuine anchor (read_event_id == provenance_chain[0])"
                    );
                }
                other => panic!("LEG 2 tainted body must Block on `body` — got {other:?}"),
            }
            assert!(ovid.is_none(), "a Blocked http.request.write mints nothing");

            let locked = conn.lock().unwrap();
            assert_eq!(
                count_events(&locked, session_id, "sink_blocked"),
                1,
                "LEG 2 records exactly one sink_blocked (I2 Block) under a write-PERMITTING policy"
            );
            assert_eq!(
                count_events(&locked, session_id, "http_write_failed"),
                0,
                "the tainted body NEVER reaches the write — no http_write_failed"
            );
            assert_eq!(
                count_events(&locked, session_id, "http_write_succeeded"),
                0,
                "the tainted body NEVER reaches the write — no http_write_succeeded"
            );
            assert!(
                verify_chain(&locked, &session_id_str, &key),
                "LEG 2 verify_chain must hold across the mint + block"
            );
            drop(locked);
            session_id
        };

        // ── LEG 3 — policy-deny (NOT an I2 Block). A session policy that EXPLICITLY
        //    PERMITS the leg-1/2 sinks (git.push + http.request.write) but OMITS
        //    email.send → submitting email.send yields `Denied{PolicyDeny}` with
        //    `code()=="policy_deny"`, recorded as the GENERIC `plan_node_evaluated`
        //    event with NO `sink_blocked`. Decision-level (NO new TCB).
        let leg3_session = {
            // A custom policy built via serde (the trusted-JSON binder shape): the
            // leg-1/2 sinks PERMITTED, email.send OMITTED. Proves the omitted sink
            // is policy-denied while the I2 legs' sinks are policy-permitted.
            let policy: SessionPolicy = serde_json::from_str(
                r#"{"allowed_sinks":["git.push","http.request.write"],"arg_constraints":{}}"#,
            )
            .expect("build policy permitting git.push + http.request.write, omitting email.send");
            assert!(
                policy.permits_sink(&SinkId("git.push".into())),
                "the policy must EXPLICITLY PERMIT git.push (the leg-1 sink)"
            );
            assert!(
                policy.permits_sink(&SinkId("http.request.write".into())),
                "the policy must EXPLICITLY PERMIT http.request.write (the leg-2 sink)"
            );
            assert!(
                !policy.permits_sink(&SinkId("email.send".into())),
                "the policy must OMIT email.send (the leg-3 policy-deny target)"
            );

            let conn = open_audit_db(audit_db_str).expect("open shared db (leg3)");
            let (session_id, root_id, root_hash) = seed_session(&conn, &key);
            expected_sessions.push(session_id);
            let session_id_str = session_id.to_string();

            // A schema-valid email.send node (email.send allows `to`, requires
            // none). The arg is CLEAN — policy-deny fires at the pre-I2 gate BEFORE
            // any taint check, so the deny is attributable to POLICY, not taint.
            let mut store = ValueStore::default();
            let (to_vid, _hid, _hh) =
                mint_clean(&conn, &key, &mut store, session_id, "ops@example.com", root_id, &root_hash);
            let node = PlanNode {
                sink: SinkId("email.send".into()),
                args: vec![PlanArg { name: "to".into(), value_id: to_vid }],
            };

            let (head_id, head_hash) = current_chain_head(&conn, &session_id_str)
                .expect("chain head query")
                .expect("chain head after mint");

            let conn = Arc::new(Mutex::new(conn));
            let workspace = Arc::new(
                adapter_fs::workspace::WorkspaceRoot::open(&std::env::temp_dir())
                    .expect("open temp workspace root"),
            );
            let mut last_event_id = head_id;
            let mut last_event_hash = head_hash;

            let (decision, ovid, _demoted) =
                brokerd::server::evaluate_plan_node_and_record_for_test(
                    &node,
                    &conn,
                    &key,
                    session_id,
                    &mut store,
                    &workspace,
                    &SessionStatus::Active,
                    &policy,
                    &mut last_event_id,
                    &mut last_event_hash,
                )
                .await
                .expect("a policy Denied decision is a normal Ok outcome (recorded, never dispatched)");

            match &decision {
                ExecutorDecision::Denied {
                    reason: reason @ DenyReason::PolicyDeny { sink, arg, constraint },
                } => {
                    assert_eq!(
                        reason.code(),
                        "policy_deny",
                        "LEG 3 must carry the DISTINCT policy_deny machine-checkable tag"
                    );
                    assert_eq!(sink, "email.send", "the deny names the omitted sink");
                    assert_eq!(*arg, None, "a sink-level deny (email.send absent), arg = None");
                    assert_eq!(
                        constraint, "sink-not-allowed",
                        "the constraint tag names the deny-by-default sink gate"
                    );
                }
                other => panic!(
                    "LEG 3 (omitted sink) must Deny PolicyDeny (code()==\"policy_deny\"), \
                     NEVER a BlockedPendingConfirmation — got {other:?}"
                ),
            }
            assert!(ovid.is_none(), "a policy-denied node mints nothing");

            let locked = conn.lock().unwrap();
            // The policy-deny leg records the GENERIC plan_node_evaluated event and
            // NO sink_blocked — the two tags are structurally distinct in the DAG.
            assert_eq!(
                count_events(&locked, session_id, "sink_blocked"),
                0,
                "LEG 3 policy-deny must record NO sink_blocked (distinct from an I2 Block)"
            );
            assert_eq!(
                count_events(&locked, session_id, "plan_node_evaluated"),
                1,
                "LEG 3 policy-deny is recorded as exactly one GENERIC plan_node_evaluated event"
            );
            assert!(
                verify_chain(&locked, &session_id_str, &key),
                "LEG 3 verify_chain must hold across the mint + policy-deny record"
            );
            drop(locked);
            session_id
        };

        // ── DISTINCT TAGS, ASSERTED SEPARATELY (locked decision #3) ────────────
        //    Re-open the shared DB and assert the two machine-checkable tags side
        //    by side, so policy-deny and I2-Block are provably-distinct mechanisms:
        //      * I2-Block legs (1, 2): each has a `sink_blocked` event.
        //      * policy-deny leg (3): NO `sink_blocked`, a generic
        //        `plan_node_evaluated` — the tag is `code()=="policy_deny"` (proven
        //        at evaluate time above), never a `sink_blocked`.
        {
            let conn = open_audit_db(audit_db_str).expect("open shared DB (tag separation)");
            assert_eq!(
                count_events(&conn, leg1_session, "sink_blocked"),
                1,
                "TAG(I2): leg 1 (tainted remote) carries a sink_blocked event"
            );
            assert_eq!(
                count_events(&conn, leg2_session, "sink_blocked"),
                1,
                "TAG(I2): leg 2 (tainted body) carries a sink_blocked event"
            );
            assert_eq!(
                count_events(&conn, leg3_session, "sink_blocked"),
                0,
                "TAG(policy_deny): leg 3 carries NO sink_blocked — distinct from an I2 Block"
            );
            assert_eq!(
                count_events(&conn, leg3_session, "plan_node_evaluated"),
                1,
                "TAG(policy_deny): leg 3 is a generic plan_node_evaluated (policy narrowed, I2 not run)"
            );
        }

        // Legs 4-5 are added in Task 3.

        // ── END-OF-RUN SWEEP — open the shared audit_db ONCE; every negative-leg
        //    session must exist with verify_chain INDEPENDENTLY true. ───────────
        {
            let conn = open_audit_db(audit_db_str).expect("open shared audit DB (sweep)");
            let sids = all_session_ids(&conn);
            for sid in &sids {
                assert!(
                    verify_chain(&conn, sid, &key),
                    "verify_chain must be true for session {sid} (ORDER BY rowid, never LIMIT 1)"
                );
            }
            for sid in &expected_sessions {
                assert!(
                    sids.contains(&sid.to_string()),
                    "session {sid} must be among the enumerated sessions in the final sweep"
                );
            }
            assert_eq!(
                sids.len(),
                expected_sessions.len(),
                "exactly the five negative-leg sessions must exist in the shared audit.db"
            );
        }

        std::fs::remove_dir_all(&tmp).ok();
    }
}
