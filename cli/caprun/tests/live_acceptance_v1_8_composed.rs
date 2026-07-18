//! live_acceptance_v1_8_composed — the v1.8 milestone's FINAL HARD GATE
//! (LIVE-03 / LIVE-04 / ENV-01, Linux-gated).
//!
//! Composed multi-leg live proof over ONE shared, persisted `audit.db` (never
//! `:memory:`, never a fresh per-leg path — the project's standing composed-run
//! pattern from `live_acceptance_v1_7_composed.rs`: one shared file, every
//! session's `verify_chain` independently true, never a single cross-session
//! `parent_id` chain). It extends the v1.7 composed shape to the three v1.8
//! sinks (`git.commit`, `github.pr`, `http.request`) plus the ENV-01 live-HTTPS
//! proof and the three adversarial Block legs.
//!
//! # SUCCESS workflow (LIVE-03) — five legs, one per session
//!
//!   - Leg exec: a trusted `process.exec` Allows; the REAL confined
//!     `caprun-exec-launcher` runs `/bin/echo`, and its captured output is
//!     GENUINELY taint-minted via `mint_from_exec` rooted on the REAL
//!     `process_exited` event (a DB re-check confirms that event is durable in
//!     the DAG BEFORE minting — the anti-staple backstop).
//!   - Leg fs: a trusted-arg `file.write` Allows and overwrites an existing file
//!     within the workspace; `sink_executed` is durably audited.
//!   - Leg git.commit: a trusted-`message` `git.commit` Allows and runs the REAL
//!     confined launcher against a real staged git repo — a genuine commit whose
//!     `process_exited` event roots the same `mint_from_exec` taint edge.
//!   - Leg github.pr SUCCESS: with a recorded session grant + a reserved CAS key,
//!     `invoke_github_pr_from_resolved` POSTs trusted title/body/owner/repo/base/
//!     head to the MOCK (`CAPRUN_GITHUB_API_BASE=https://github-mock.caprun.test`)
//!     and gets a 201; a durable opaque `github_pr_succeeded` event is appended
//!     and the bearer-token literal appears in NO event payload (GITHUB-01).
//!   - Leg http GET (ENV-01 live proof): `invoke_http_get` to a real allowlisted
//!     `api.github.com` endpoint succeeds WITHOUT any `SSL_CERT_*` env set (they
//!     are explicitly removed for the call), and `mint_from_http` mints
//!     `[ExternalUntrusted, HttpRaw]` rooted on the real `http_response_received`
//!     event — proving `env_clear()` + `webpki-roots` is hermetic (no cert env).
//!
//! # Adversarial legs (LIVE-04) — three deterministic Blocks/Denies
//!
//!   - adv-a github.pr: a GENUINELY-tainted value (http-response-derived) routed
//!     into the `title` arg → `BlockedPendingConfirmation`; NO PR POST occurs.
//!   - adv-b http.request: (i) a tainted value in the `url` arg →
//!     `BlockedPendingConfirmation` (routing+content sensitive); (ii) an
//!     untainted non-allowlisted / SSRF-range url → `invoke_http_get` /
//!     `ssrf_check` Errs BEFORE any socket (the pin-layer Deny).
//!   - adv-c git.commit: a genuinely-tainted value routed into the `message` arg
//!     → `BlockedPendingConfirmation`; NO commit runs.
//!   Each adversarial leg's taint is genuine (provenance rooted on a real minted
//!   `http_response_received` event, mirroring `s37_http_request.rs`), never
//!   stapled at the sink; `verify_chain` holds for every session.
//!
//! # RESCOPING (v1.8 honesty — CONTEXT / DECISION-git-push-deferral-v1.8.md)
//!
//! There is NO `git.push` leg — it is DEFERRED to v1.9. The `github.pr` POST is a
//! MOCK; a real push is NOT proven here. The mock's 201 stands in for the
//! pushed-branch precondition. The full live-OpenAI SIDECAR run (ENV-01 second
//! half) is the harness's conditional-on-key run — this test NEVER asserts it
//! passed; the broker-side live HTTPS GET above is the webpki-roots-hermetic
//! proof. If `OPENAI_API_KEY` is absent the sidecar env_clear is
//! structurally-verified only and MUST NOT be claimed as a passed live path.
//!
//! # Linux-only
//!
//! The success legs spawn the REAL kernel-confined `caprun-exec-launcher`
//! (`process.exec` / `git.commit`) and open real sockets (`http.request` /
//! `github.pr`) — a macOS run would prove nothing (its confinement primitives +
//! socket legs are no-op stubs). This file's body is `#[cfg(target_os = "linux")]`;
//! `cargo test -p caprun` on macOS compiles it and runs only the always-on guard
//! test below (0 Linux tests — expected, not a gap, per CLAUDE.md's "Linux-only
//! security tests" / cfg-linux-test-blindness).
//!
//! # Run (Linux) — via the composed harness ONLY, which does `cargo build
//! --workspace` FIRST (so the sibling `caprun-exec-launcher`/`caprun-worker`
//! binaries exist at `current_exe`-resolution time —
//! cargo-test-workspace-missing-sibling-binary) and enables the NON-DEFAULT
//! `brokerd/mock-egress-ca` feature (so the mock cert is trusted):
//!
//!   bash scripts/compose-verify.sh
//!
//! `compose-verify.sh` captures the TRUE exit code BEFORE any pipe and asserts on
//! named tests + counts — NEVER on `$?` through a pipe
//! (`verification-exit-code-through-pipe`).

#[cfg(target_os = "linux")]
mod linux {
    use adapter_fs::workspace::WorkspaceRoot;
    use brokerd::audit::{
        append_event, current_chain_head, find_event_by_type, github_pr_content_key,
        has_github_grant, open_audit_db, record_github_grant, reserve_created_pr, verify_chain,
    };
    use brokerd::confirmation::ResolvedArg;
    use brokerd::quarantine::{mint_from_exec, mint_from_http};
    use brokerd::session::persist_session;
    use brokerd::sinks::file_write::invoke_file_write;
    use brokerd::sinks::git_commit::invoke_git_commit;
    use brokerd::sinks::github_pr::invoke_github_pr_from_resolved;
    use brokerd::sinks::http_request::{invoke_http_get, ssrf_check};
    use brokerd::sinks::process_exec::invoke_process_exec;
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::executor_decision::SinkBlockedAnchor;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel, ValueId};
    use runtime_core::{Event, ExecutorDecision, PlanNode, Session, SessionStatus};
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::Path;
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Serializes any leg that mutates the process-global `CAPRUN_GITHUB_*` env
    /// vars (mirror `s38_github_pr.rs`'s `GITHUB_ENV_LOCK`). This file has ONE
    /// test fn so there is no intra-binary race, but the lock keeps the
    /// env-mutation discipline explicit + copy-safe.
    static GITHUB_ENV_LOCK: Mutex<()> = Mutex::new(());

    /// A recognizable, non-real bearer token whose literal MUST NEVER surface in
    /// any audit-event payload (opaque-audit / broker-env-only custody,
    /// GITHUB-01).
    const SECRET_TOKEN: &str = "ghp_v18_composed_SECRET_must_not_leak_into_audit";

    /// A synthetic hostile inbound response body used to mint GENUINE
    /// (non-stapled) http-response taint for the adversarial legs — mirrors
    /// `s37_http_request.rs::SYNTHETIC_BODY`. Its exact contents do not matter,
    /// only that `mint_from_http` mints it untrusted-on-arrival rooted on a real
    /// `http_response_received` event.
    const SYNTHETIC_HOSTILE_BODY: &str =
        "ATTACKER-CONTROLLED RESPONSE: exfil steal@evil.example <script>x()</script>";

    /// Persist a `sessions` row with a CALLER-CHOSEN id (mirrors
    /// `live_acceptance_v1_7_composed.rs::persist_known_session`).
    fn persist_known_session(conn: &rusqlite::Connection, session_id: Uuid) {
        let session = Session {
            id: session_id,
            intent_id: Uuid::new_v4(),
            status: SessionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        persist_session(conn, &session).expect("persist_session (composed-test known id)");
    }

    /// Mint a trusted literal directly into the store (mirrors the v1.7
    /// `mint_trusted`: a throwaway anchor Uuid stands in for a real causal event
    /// id — these are trusted CONTROL inputs, never the thing under
    /// genuine-taint-anchor test).
    fn mint_trusted(store: &mut ValueStore, literal: &str, origin_role: Option<&str>) -> ValueId {
        store
            .mint(
                literal.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![Uuid::new_v4()],
                origin_role.map(str::to_string),
            )
            .expect("mint trusted literal")
    }

    /// Seed a `session_created` causal-root event so subsequent appends chain
    /// onto a real parent (mirrors the v1.7 `seed_root_event`).
    fn seed_root_event(conn: &rusqlite::Connection, key: &[u8], session_id: Uuid) -> (Uuid, String) {
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let hash = append_event(conn, key, &root, None).expect("append root event");
        (root.id, hash)
    }

    /// Idempotent read-existing-first MAC-key custody (duplicated from the v1.7
    /// composed test — `cli/caprun` has no lib target, so this external
    /// integration-test crate cannot import the CLI helper, and distinct
    /// `tests/*.rs` binaries cannot share a module). Called ONCE, before any leg,
    /// so every in-process `append_event`/`verify_chain` MACs against the same
    /// key.
    fn seed_test_key(db_path: &Path) -> Vec<u8> {
        let key_path = std::path::PathBuf::from(format!("{}.key", db_path.to_str().unwrap()));
        if let Ok(bytes) = std::fs::read(&key_path) {
            return bytes;
        }
        let mut key = Uuid::new_v4().as_bytes().to_vec();
        key.extend_from_slice(Uuid::new_v4().as_bytes());
        std::fs::write(&key_path, &key).expect("write test MAC key file");
        key
    }

    /// Session discovery safe for a multi-session shared DB (mirrors the v1.7
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

    /// Run the system `git` (in the TEST process, UNCONFINED) inside `dir` for
    /// SETUP + assertions only — never the committed-under-test operation, which
    /// the confined child performs with the broker's own trusted `-c` identity
    /// (mirrors `git_commit_spawn.rs::git_in`).
    fn git_in(dir: &Path, args: &[&str]) -> (bool, String) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("failed to spawn system git");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        )
    }

    /// A trusted six-arg `github.pr` `ResolvedArg` snapshot (owner/repo/base/head/
    /// title/body), all UserTrusted (untainted) — what the server.rs Allowed arm
    /// would resolve from its ValueStore (mirror `s38_github_pr.rs`).
    fn trusted_pr_args() -> Vec<ResolvedArg> {
        let mk = |name: &str, literal: &str| ResolvedArg {
            name: name.to_string(),
            value_id: ValueId::new(),
            literal: literal.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        };
        vec![
            mk("owner", "octocat"),
            mk("repo", "hello-world"),
            mk("base", "main"),
            mk("head", "feature-v18"),
            mk("title", "A v1.8 composed live-proof PR"),
            mk("body", "PR body from live_acceptance_v1_8_composed"),
        ]
    }

    /// Look up a required literal from a frozen `ResolvedArg` snapshot.
    fn lit<'a>(resolved: &'a [ResolvedArg], name: &str) -> &'a str {
        resolved
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.literal.as_str())
            .unwrap_or_else(|| panic!("test arg `{name}` missing"))
    }

    /// Build a `github.pr` PlanNode over UserTrusted handles for the SUCCESS
    /// leg's executor gate — proving the node Allows before the arm dispatches.
    fn trusted_pr_plan_node(store: &mut ValueStore) -> PlanNode {
        let args = ["owner", "repo", "base", "head", "title", "body"]
            .iter()
            .map(|name| {
                let vid = mint_trusted(store, &format!("v18-{name}"), None);
                PlanArg { name: (*name).to_string(), value_id: vid }
            })
            .collect();
        PlanNode { sink: SinkId("github.pr".into()), args }
    }

    /// The composed v1.8 live-acceptance scenario — the milestone's FINAL HARD
    /// GATE (LIVE-03 success + LIVE-04 adversarial + ENV-01 live HTTPS). All legs
    /// run sequentially in ONE test fn over ONE shared persisted `audit.db`
    /// (single-threaded → env mutation + the shared DB path are race-free), each
    /// leg its own session; the final sweep asserts exactly the composed sessions
    /// exist and every `verify_chain` is independently true.
    #[tokio::test]
    async fn live_acceptance_v1_8_composed_all_legs() {
        let run_id = Uuid::new_v4();
        let tmp = std::env::temp_dir().join(format!("caprun_live_v18_{run_id}"));
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        // F1-safe layout: workspace + git-repo roots under their own
        // subdirectories; audit.db a SIBLING of them (never a child of a
        // WorkspaceRoot).
        let ws_dir = tmp.join("workspace");
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        let audit_db = tmp.join("audit.db"); // ONE shared path — NEVER :memory:
        let audit_db_str = audit_db.to_str().unwrap();

        // Mint/persist the shared MAC key ONCE, before any leg.
        let key = seed_test_key(&audit_db);

        // Track every session id we persist so the final sweep can assert the
        // exact set (never LIMIT 1).
        let mut expected_sessions: Vec<Uuid> = Vec::new();

        // ── LEG exec (SUCCESS) — trusted process.exec Allows; output GENUINELY
        //    minted, rooted on the REAL process_exited event (anti-staple). ─────
        let exec_session_id = Uuid::new_v4();
        expected_sessions.push(exec_session_id);
        {
            let conn = Arc::new(Mutex::new(
                open_audit_db(audit_db_str).expect("open audit db (exec)"),
            ));
            let (root_id, root_hash) = {
                let locked = conn.lock().expect("lock conn");
                persist_known_session(&locked, exec_session_id);
                seed_root_event(&locked, &key, exec_session_id)
            };

            let mut store = ValueStore::default();
            let command_vid = mint_trusted(&mut store, "/bin/echo", None);
            let args_json = serde_json::to_string(&vec!["v18-exec-marker"]).expect("serialize args");
            let args_vid = mint_trusted(&mut store, &args_json, None);
            let plan_node = PlanNode {
                sink: SinkId("process.exec".into()),
                args: vec![
                    PlanArg { name: "command".into(), value_id: command_vid },
                    PlanArg { name: "args".into(), value_id: args_vid },
                ],
            };
            let effect_id = Uuid::new_v4();
            let decision = executor::submit_plan_node(
                exec_session_id,
                effect_id,
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            assert_eq!(
                decision,
                ExecutorDecision::Allowed,
                "leg exec: a trusted-input process.exec must Allow"
            );

            let ws_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root (exec)");
            let (exec_event_id, _hash, combined_output) = invoke_process_exec(
                &conn,
                &key,
                &store,
                exec_session_id,
                effect_id,
                &plan_node,
                &ws_root,
                root_id,
                &root_hash,
            )
            .await
            .expect("invoke_process_exec must succeed for trusted /bin/echo");
            assert!(
                combined_output.contains("v18-exec-marker"),
                "captured output must contain the echoed marker, got: {combined_output:?}"
            );

            // Anti-staple DB re-check: the process_exited event is durable in the
            // DAG BEFORE we mint from it.
            {
                let locked = conn.lock().expect("lock conn");
                let dag_event =
                    find_event_by_type(&locked, &exec_session_id.to_string(), "process_exited")
                        .expect("query process_exited")
                        .expect("process_exited event must exist in the audit DAG");
                assert_eq!(dag_event.id, exec_event_id);
            }

            let value_id =
                mint_from_exec(&mut store, exec_session_id, combined_output, exec_event_id)
                    .expect("mint_from_exec must succeed");
            let minted = store.resolve(&value_id).expect("minted value must resolve");
            assert_eq!(
                minted.provenance_chain,
                vec![exec_event_id],
                "GENUINE-TAINT: provenance_chain must be EXACTLY [process_exited id] (non-stapled)"
            );
            assert!(minted.taint.contains(&TaintLabel::ExternalUntrusted));
            assert!(minted.taint.contains(&TaintLabel::ExecRaw));

            let locked = conn.lock().expect("lock conn");
            assert!(
                verify_chain(&locked, &exec_session_id.to_string(), &key),
                "leg exec: verify_chain must be true"
            );
        }

        // ── LEG fs (SUCCESS) — trusted path/contents file.write Allows and
        //    overwrites an existing file within WorkspaceRoot, durably audited. ─
        let fs_session_id = Uuid::new_v4();
        expected_sessions.push(fs_session_id);
        {
            // write_within requires the target to ALREADY exist (O_TRUNC).
            std::fs::write(ws_dir.join("v18-fs-existing.txt"), b"original")
                .expect("pre-create fs target");

            let conn = open_audit_db(audit_db_str).expect("open audit db (fs)");
            persist_known_session(&conn, fs_session_id);
            let (root_id, root_hash) = seed_root_event(&conn, &key, fs_session_id);

            let mut store = ValueStore::default();
            let path_vid = mint_trusted(&mut store, "v18-fs-existing.txt", Some("path"));
            let contents_vid =
                mint_trusted(&mut store, "written by live_acceptance_v1_8_composed", Some("path"));
            let plan_node = PlanNode {
                sink: SinkId("file.write".into()),
                args: vec![
                    PlanArg { name: "path".into(), value_id: path_vid },
                    PlanArg { name: "contents".into(), value_id: contents_vid },
                ],
            };
            let effect_id = Uuid::new_v4();
            let decision = executor::submit_plan_node(
                fs_session_id,
                effect_id,
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            assert_eq!(
                decision,
                ExecutorDecision::Allowed,
                "leg fs: a trusted path/contents file.write must Allow"
            );

            let ws_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root (fs)");
            let (evt_id, _hash) = invoke_file_write(
                &conn,
                &key,
                &store,
                fs_session_id,
                effect_id,
                &plan_node,
                &ws_root,
                root_id,
                &root_hash,
            )
            .expect("invoke_file_write must succeed on an existing target");

            let on_disk =
                std::fs::read_to_string(ws_dir.join("v18-fs-existing.txt")).expect("read back");
            assert_eq!(on_disk, "written by live_acceptance_v1_8_composed");

            let evt = find_event_by_type(&conn, &fs_session_id.to_string(), "sink_executed")
                .expect("query sink_executed")
                .expect("leg fs: a durable sink_executed event must exist");
            assert_eq!(evt.id, evt_id);
            assert!(
                verify_chain(&conn, &fs_session_id.to_string(), &key),
                "leg fs: verify_chain must be true"
            );
        }

        // ── LEG git.commit (SUCCESS) — trusted message Allows; the REAL confined
        //    launcher makes a genuine commit in a real staged repo. ────────────
        let git_session_id = Uuid::new_v4();
        expected_sessions.push(git_session_id);
        {
            // A real git repo AT its own workspace root with one staged file, so
            // the launcher's Landlock allow-rule covers `.git`.
            let repo_dir = tmp.join("gitrepo");
            std::fs::create_dir_all(&repo_dir).expect("create gitrepo dir");
            assert!(git_in(&repo_dir, &["init", "-q"]).0, "git init failed");
            std::fs::write(repo_dir.join("tracked.txt"), b"initial\n").expect("write tracked file");
            assert!(git_in(&repo_dir, &["add", "tracked.txt"]).0, "git add failed");
            let ws_root = WorkspaceRoot::open(&repo_dir).expect("open workspace root (git)");

            let conn = Arc::new(Mutex::new(
                open_audit_db(audit_db_str).expect("open audit db (git)"),
            ));
            let (root_id, root_hash) = {
                let locked = conn.lock().expect("lock conn");
                persist_known_session(&locked, git_session_id);
                seed_root_event(&locked, &key, git_session_id)
            };

            let mut store = ValueStore::default();
            let message_vid = mint_trusted(&mut store, "genuine v1.8 composed commit", None);
            let plan_node = PlanNode {
                sink: SinkId("git.commit".into()),
                args: vec![PlanArg { name: "message".into(), value_id: message_vid }],
            };
            let effect_id = Uuid::new_v4();
            let decision = executor::submit_plan_node(
                git_session_id,
                effect_id,
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            assert_eq!(
                decision,
                ExecutorDecision::Allowed,
                "leg git.commit: a trusted-message git.commit must Allow"
            );

            let (evt_id, _hash, combined_output) = invoke_git_commit(
                &conn,
                &key,
                &store,
                git_session_id,
                effect_id,
                &plan_node,
                &ws_root,
                root_id,
                &root_hash,
            )
            .await
            .expect("git.commit must succeed via the confined launcher");

            // A real commit now exists (HEAD resolves) with the trusted subject.
            let (head_ok, head) = git_in(&repo_dir, &["rev-parse", "--verify", "HEAD"]);
            assert!(head_ok && !head.is_empty(), "HEAD did not advance to a real commit");
            let (_ok, subject) = git_in(&repo_dir, &["log", "-1", "--format=%s"]);
            assert_eq!(subject, "genuine v1.8 composed commit");

            // The commit output roots the same mint_from_exec taint edge.
            let value_id = mint_from_exec(&mut store, git_session_id, combined_output, evt_id)
                .expect("mint_from_exec must succeed");
            let minted = store.resolve(&value_id).expect("minted value must resolve");
            assert_eq!(minted.provenance_chain[0], evt_id, "non-stapled commit-output taint");

            let locked = conn.lock().expect("lock conn");
            let evt = find_event_by_type(&locked, &git_session_id.to_string(), "process_exited")
                .expect("query process_exited")
                .expect("leg git.commit: a durable process_exited event must exist");
            assert_eq!(evt.id, evt_id);
            assert!(
                verify_chain(&locked, &git_session_id.to_string(), &key),
                "leg git.commit: verify_chain must be true"
            );
        }

        // ── LEG github.pr SUCCESS — grant gate -> CAS reserve -> live POST to the
        //    MOCK (201) -> opaque github_pr_succeeded; token never in a payload. ─
        let pr_session_id = Uuid::new_v4();
        expected_sessions.push(pr_session_id);
        {
            let _env = GITHUB_ENV_LOCK.lock().unwrap();
            // Broker-local credential custody (D-04) + mock destination override.
            std::env::set_var("CAPRUN_GITHUB_TOKEN", SECRET_TOKEN);
            std::env::set_var("CAPRUN_GITHUB_API_BASE", "https://github-mock.caprun.test");

            let conn = open_audit_db(audit_db_str).expect("open audit db (github.pr)");
            persist_known_session(&conn, pr_session_id);
            let _ = seed_root_event(&conn, &key, pr_session_id);
            let sid = pr_session_id.to_string();

            // Gate: the executor Allows a fully-trusted github.pr node (the ONLY
            // thing that can deny is the arm's grant gate — proven next).
            let mut store = ValueStore::default();
            let plan_node = trusted_pr_plan_node(&mut store);
            let allow = executor::submit_plan_node(
                pr_session_id,
                Uuid::new_v4(),
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            assert_eq!(
                allow,
                ExecutorDecision::Allowed,
                "leg github.pr: a fully-trusted github.pr node must Allow"
            );

            // Arm ordering (mirror s38): grant gate FIRST, then CAS reserve, then
            // the POST on the fresh branch.
            record_github_grant(&conn, &key, &sid).expect("record_github_grant");
            assert!(has_github_grant(&conn, &sid), "the session must hold a live github.pr grant");

            let resolved = trusted_pr_args();
            let content_key = github_pr_content_key(
                lit(&resolved, "owner"),
                lit(&resolved, "repo"),
                lit(&resolved, "base"),
                lit(&resolved, "head"),
                lit(&resolved, "title"),
                lit(&resolved, "body"),
            );
            let effect_id = Uuid::new_v4();
            let fresh = reserve_created_pr(&conn, &content_key, &effect_id.to_string(), &sid)
                .expect("reserve_created_pr");
            assert!(fresh, "a first submit must reserve the CAS (fresh)");

            // Append the github_pr_attempted marker onto the current head, then
            // POST from that marker.
            let (head_id, head_hash) = current_chain_head(&conn, &sid)
                .expect("current_chain_head query")
                .expect("a chain head must exist");
            let marker = Event::new(
                Uuid::new_v4(),
                Some(head_id),
                pr_session_id,
                format!("sink:github.pr:{effect_id}"),
                "github_pr_attempted".into(),
                Utc::now(),
                vec![],
            );
            let marker_hash =
                append_event(&conn, &key, &marker, Some(&head_hash)).expect("append marker");

            let (succeeded_id, _hash) = invoke_github_pr_from_resolved(
                &conn,
                &key,
                pr_session_id,
                effect_id,
                &resolved,
                marker.id,
                &marker_hash,
            )
            .await
            .expect("github.pr POST to the mock must return 201 -> github_pr_succeeded");

            std::env::remove_var("CAPRUN_GITHUB_TOKEN");

            let succeeded =
                find_event_by_type(&conn, &sid, "github_pr_succeeded")
                    .expect("query github_pr_succeeded")
                    .expect("a durable github_pr_succeeded event must exist (mock 201)");
            assert_eq!(succeeded.id, succeeded_id);
            assert!(
                find_event_by_type(&conn, &sid, "github_pr_failed")
                    .expect("query github_pr_failed")
                    .is_none(),
                "no github_pr_failed on the mock-201 success path"
            );

            // GITHUB-01 opaque audit: the token literal is in NO event payload.
            let mut stmt = conn
                .prepare("SELECT actor, event_type, payload FROM events WHERE session_id = ?1")
                .expect("prepare events scan");
            let rows = stmt
                .query_map([sid.as_str()], |row| {
                    let a: String = row.get(0)?;
                    let t: String = row.get(1)?;
                    let p: String = row.get(2)?;
                    Ok(format!("{a}|{t}|{p}"))
                })
                .expect("query events");
            for row in rows {
                let combined = row.expect("read event row");
                assert!(
                    !combined.contains(SECRET_TOKEN) && !combined.contains("ghp_"),
                    "the bearer token literal must NEVER appear in any audit payload; found: {combined}"
                );
            }

            assert!(
                verify_chain(&conn, &sid, &key),
                "leg github.pr: verify_chain must be true across grant -> CAS -> succeeded"
            );
        }

        // ── LEG http GET (ENV-01 live proof) — a real allowlisted api.github.com
        //    GET succeeds WITH SSL_CERT_* removed, and mints HttpRaw. ───────────
        let http_session_id = Uuid::new_v4();
        expected_sessions.push(http_session_id);
        {
            // Prove webpki-roots hermeticity: remove any ambient cert env for the
            // call. The ring/webpki-roots egress client needs no SSL_CERT_*.
            std::env::remove_var("SSL_CERT_FILE");
            std::env::remove_var("SSL_CERT_DIR");

            let body = invoke_http_get("https://api.github.com/zen")
                .await
                .expect(
                    "ENV-01: a live GET to real api.github.com must succeed with no SSL_CERT_* \
                     (webpki-roots hermetic)",
                );
            assert!(!body.is_empty(), "api.github.com/zen must return a non-empty body");

            let conn = open_audit_db(audit_db_str).expect("open audit db (http)");
            persist_known_session(&conn, http_session_id);
            let (root_id, root_hash) = seed_root_event(&conn, &key, http_session_id);

            let mut store = ValueStore::default();
            let (event_id, _event_hash, value_id, _demoted_id, _demoted_hash) = mint_from_http(
                &conn,
                &key,
                &mut store,
                http_session_id,
                body,
                Some(root_id),
                Some(&root_hash),
            )
            .expect("mint_from_http must succeed on the live GET body");

            let minted = store.resolve(&value_id).expect("minted value must resolve");
            assert_eq!(
                minted.provenance_chain[0], event_id,
                "GENUINE-TAINT: http body provenance_chain[0] == http_response_received id"
            );
            assert!(minted.taint.contains(&TaintLabel::ExternalUntrusted));
            assert!(minted.taint.contains(&TaintLabel::HttpRaw));

            let evt = find_event_by_type(&conn, &http_session_id.to_string(), "http_response_received")
                .expect("query http_response_received")
                .expect("http_response_received event must exist in the DAG");
            assert_eq!(evt.id, event_id);
            assert!(
                verify_chain(&conn, &http_session_id.to_string(), &key),
                "leg http GET: verify_chain must be true"
            );
        }

        // ═══ ADVERSARIAL LEGS (LIVE-04) ═══════════════════════════════════════
        //
        // Each mints GENUINE http-response taint (mint_from_http on a synthetic
        // hostile body — appends a real http_response_received event, roots the
        // ValueRecord's provenance_chain[0] on it; mirrors s37_http_request.rs),
        // routes it into a SENSITIVE sink arg, and asserts a deterministic
        // BlockedPendingConfirmation anchored on that arg with the anchor's
        // provenance root EQUAL to the real event id (non-stapled backstop). No
        // sink effect occurs; verify_chain holds. `submit_plan_node` is passed
        // `Active` deliberately (mirror s37): the Block is TAINT-driven (I2), not
        // a draft-only (I1) session gate — isolating that the untrusted-on-arrival
        // taint ALONE forces the Block, even though mint_from_http demoted the
        // persisted session to Draft.

        /// Mint a genuine http-response-derived tainted value on a fresh session,
        /// returning `(conn, http_response_received id, chain-head id/hash after
        /// the mint's demote append, tainted value_id, ValueStore)`. The
        /// chain-head is the session_demoted event (the LAST appended) — a
        /// subsequent sink_blocked MUST thread onto THAT, not the response id
        /// (threading onto the response id forks the DAG, breaking verify_chain).
        fn seed_genuine_http_taint(
            audit_db_str: &str,
            key: &[u8],
            session_id: Uuid,
        ) -> (rusqlite::Connection, Uuid, Uuid, String, ValueId, ValueStore) {
            let conn = open_audit_db(audit_db_str).expect("open audit db (adversarial mint)");
            persist_known_session(&conn, session_id);
            let (root_id, root_hash) = seed_root_event(&conn, key, session_id);
            let mut store = ValueStore::default();
            let (event_id, _event_hash, value_id, demoted_id, demoted_hash) = mint_from_http(
                &conn,
                key,
                &mut store,
                session_id,
                SYNTHETIC_HOSTILE_BODY.to_string(),
                Some(root_id),
                Some(&root_hash),
            )
            .expect("mint_from_http must succeed");
            (conn, event_id, demoted_id, demoted_hash, value_id, store)
        }

        /// Extract the single blocked anchor from a BlockedPendingConfirmation
        /// decision, asserting the blocked arg + sink, and that the anchor's
        /// provenance root is the REAL minted event id (non-stapled backstop).
        fn assert_blocked_on(
            decision: ExecutorDecision,
            expect_sink: &str,
            expect_arg: &str,
            expect_root_event: Uuid,
        ) -> SinkBlockedAnchor {
            match decision {
                ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                    assert_eq!(anchors.len(), 1, "exactly one blocked arg ({expect_arg})");
                    let anchor = anchors.into_iter().next().expect("one anchor").anchor;
                    assert_eq!(anchor.arg, expect_arg, "blocked arg must be {expect_arg}");
                    assert_eq!(anchor.sink.0, expect_sink, "blocked sink must be {expect_sink}");
                    assert_eq!(
                        anchor.provenance_chain[0], expect_root_event,
                        "GENUINE-TAINT BACKSTOP: anchor.provenance_chain[0] must equal the \
                         http_response_received event id (non-stapled)"
                    );
                    assert_eq!(anchor.read_event_id, expect_root_event);
                    anchor
                }
                other => panic!(
                    "expected BlockedPendingConfirmation on a tainted {expect_sink} `{expect_arg}` \
                     arg, got {other:?}"
                ),
            }
        }

        // ── LEG adv-a (github.pr): a tainted value in `title` Blocks; NO POST. ──
        let adv_a_session_id = Uuid::new_v4();
        expected_sessions.push(adv_a_session_id);
        {
            let (conn, resp_event_id, demoted_id, demoted_hash, tainted_vid, mut store) =
                seed_genuine_http_taint(audit_db_str, &key, adv_a_session_id);

            // Five trusted routing/other args + the tainted title (content-sensitive).
            let mk_trusted = |store: &mut ValueStore, name: &str| PlanArg {
                name: name.to_string(),
                value_id: mint_trusted(store, &format!("adv-a-{name}"), None),
            };
            let plan_node = PlanNode {
                sink: SinkId("github.pr".into()),
                args: vec![
                    mk_trusted(&mut store, "owner"),
                    mk_trusted(&mut store, "repo"),
                    mk_trusted(&mut store, "base"),
                    mk_trusted(&mut store, "head"),
                    PlanArg { name: "title".into(), value_id: tainted_vid },
                    mk_trusted(&mut store, "body"),
                ],
            };
            let decision = executor::submit_plan_node(
                adv_a_session_id,
                Uuid::new_v4(),
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            let anchor =
                assert_blocked_on(decision, "github.pr", "title", resp_event_id);

            let block = Event::sink_blocked(
                Uuid::new_v4(),
                Some(demoted_id),
                adv_a_session_id,
                Utc::now(),
                vec![anchor],
                None,
                vec!["title".to_string()],
            );
            append_event(&conn, &key, &block, Some(&demoted_hash)).expect("append sink_blocked");

            assert!(
                find_event_by_type(&conn, &adv_a_session_id.to_string(), "github_pr_succeeded")
                    .expect("query")
                    .is_none()
                    && find_event_by_type(&conn, &adv_a_session_id.to_string(), "github_pr_attempted")
                        .expect("query")
                        .is_none(),
                "adv-a: no PR POST may occur on a blocked github.pr"
            );
            assert!(
                verify_chain(&conn, &adv_a_session_id.to_string(), &key),
                "adv-a: verify_chain must be true"
            );
        }

        // ── LEG adv-b (http.request): (i) tainted `url` Blocks; (ii) a
        //    non-allowlisted / SSRF-range url Denies at the pin layer. ──────────
        let adv_b_session_id = Uuid::new_v4();
        expected_sessions.push(adv_b_session_id);
        {
            let (conn, resp_event_id, demoted_id, demoted_hash, tainted_vid, store) =
                seed_genuine_http_taint(audit_db_str, &key, adv_b_session_id);

            // (i) executor Block: a tainted url (routing + content sensitive).
            let plan_node = PlanNode {
                sink: SinkId("http.request".into()),
                args: vec![PlanArg { name: "url".into(), value_id: tainted_vid }],
            };
            let decision = executor::submit_plan_node(
                adv_b_session_id,
                Uuid::new_v4(),
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            let anchor =
                assert_blocked_on(decision, "http.request", "url", resp_event_id);
            let block = Event::sink_blocked(
                Uuid::new_v4(),
                Some(demoted_id),
                adv_b_session_id,
                Utc::now(),
                vec![anchor],
                None,
                vec!["url".to_string()],
            );
            append_event(&conn, &key, &block, Some(&demoted_hash)).expect("append sink_blocked");
            assert!(
                verify_chain(&conn, &adv_b_session_id.to_string(), &key),
                "adv-b: verify_chain must be true after the tainted-url Block"
            );

            // (ii) pin-layer Deny — BEFORE any socket, host-portable:
            //   - a non-allowlisted host Errs at the allowlist gate;
            //   - the SSRF classifier Errs on a cloud-metadata / RFC1918 IP.
            assert!(
                invoke_http_get("https://evil.invalid/x").await.is_err(),
                "adv-b: a non-allowlisted host must Err at the allowlist gate (no socket)"
            );
            assert!(
                ssrf_check(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))).is_err(),
                "adv-b: the cloud-metadata IP must be denied by ssrf_check (SSRF range)"
            );
            assert!(
                ssrf_check(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))).is_err(),
                "adv-b: an RFC1918 IP must be denied by ssrf_check (SSRF range)"
            );
        }

        // ── LEG adv-c (git.commit): a tainted `message` Blocks; NO commit. ─────
        let adv_c_session_id = Uuid::new_v4();
        expected_sessions.push(adv_c_session_id);
        {
            let (conn, resp_event_id, demoted_id, demoted_hash, tainted_vid, store) =
                seed_genuine_http_taint(audit_db_str, &key, adv_c_session_id);

            let plan_node = PlanNode {
                sink: SinkId("git.commit".into()),
                args: vec![PlanArg { name: "message".into(), value_id: tainted_vid }],
            };
            let decision = executor::submit_plan_node(
                adv_c_session_id,
                Uuid::new_v4(),
                &plan_node,
                &store,
                &SessionStatus::Active,
            );
            let anchor =
                assert_blocked_on(decision, "git.commit", "message", resp_event_id);
            let block = Event::sink_blocked(
                Uuid::new_v4(),
                Some(demoted_id),
                adv_c_session_id,
                Utc::now(),
                vec![anchor],
                None,
                vec!["message".to_string()],
            );
            append_event(&conn, &key, &block, Some(&demoted_hash)).expect("append sink_blocked");

            assert!(
                find_event_by_type(&conn, &adv_c_session_id.to_string(), "process_exited")
                    .expect("query")
                    .is_none(),
                "adv-c: no commit (process_exited) may occur on a blocked git.commit"
            );
            assert!(
                verify_chain(&conn, &adv_c_session_id.to_string(), &key),
                "adv-c: verify_chain must be true"
            );
        }

        // ── END-OF-RUN SWEEP — open the shared audit_db ONCE; every composed
        //    session (5 SUCCESS + 3 adversarial = 8) must exist with verify_chain
        //    independently true. ────────────────────────────────────────────────
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
                "exactly the composed sessions must exist in the shared audit.db"
            );
        }

        std::fs::remove_dir_all(&tmp).ok();
    }
}

/// Cross-platform guard: keeps `cargo test -p caprun` meaningful on the macOS dev
/// box (where the Linux body above is cfg-excluded, 0 tests reported — expected,
/// not a gap). Confirms the caprun binary is wired into the test build (mirrors
/// `live_acceptance_v1_7_composed.rs`'s guard).
#[test]
fn live_acceptance_v1_8_composed_guard_binary_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(
        !caprun_bin.is_empty(),
        "CARGO_BIN_EXE_caprun must resolve — caprun must be built for the live test"
    );
}
