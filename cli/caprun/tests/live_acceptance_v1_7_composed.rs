//! live_acceptance_v1_7_composed — the v1.7 milestone's FINAL HARD GATE
//! (LIVE-01/LIVE-02, Linux-gated).
//!
//! Composed multi-leg live proof over ONE shared, persisted `audit.db`
//! (never `:memory:`, never a fresh per-leg path — the project's standing
//! composed-run pattern from `live_acceptance_v1_3.rs`/
//! `live_acceptance_v1_4_composed.rs`: one shared file, every session's
//! `verify_chain` independently true, never a single cross-session
//! `parent_id` chain). Four legs (D-12):
//!
//!   - Leg (a) BLOCK: a trusted `process.exec` Allows and its captured
//!     output is genuinely taint-minted (rooted on the REAL `process_exited`
//!     event `invoke_process_exec` appends — never a fabricated root). That
//!     tainted handle is routed into a SECOND `process.exec` plan node's
//!     `command` arg and deterministically Blocks (I2), with an unbroken
//!     audit-DAG edge (exec Event -> ValueNode -> sink arg -> block) and
//!     `verify_chain` true for the session.
//!   - Leg (b) ALLOW: a clean `process.exec` (trusted command/args) Allows
//!     and its output, never routed anywhere, causes no Block.
//!   - Leg (c) FS WRITE: a `file.write` with a trusted path/contents pair
//!     Allows, overwrites an EXISTING file within `WorkspaceRoot`, and is
//!     durably audited (`sink_executed`).
//!   - Leg (d) EXEC-05 RELEASE: a Blocked `process.exec` (seeded directly,
//!     mirrors `s9_process_exec_block.rs`'s confirm-release leg) is released
//!     via a REAL `caprun confirm` subprocess — runs EXACTLY ONCE, its
//!     `process_exited` event chains onto the `confirm_granted` head (the
//!     real head — never a fabricated root, D-04), and `verify_chain` holds.
//!
//! # Why in-process for legs (a)/(b)/(c), a real subprocess for leg (d)
//!
//! There is no `process.exec`/`file.write` intent kind on the intent-first
//! `caprun` CLI (mirrors `s9_process_exec_block.rs`'s own scoping note) — so
//! legs (a)-(c) drive the SAME production functions directly, in-process,
//! against the shared persisted `audit.db`:
//! `executor::submit_plan_node` (the I2 decision), `brokerd::quarantine`'s
//! sole mint sites (`mint_from_exec`), and the sole live sinks
//! (`brokerd::sinks::process_exec::invoke_process_exec`,
//! `brokerd::sinks::file_write::invoke_file_write`). Leg (d) exercises
//! `caprun confirm` as a REAL, separate OS process (mirrors
//! `s9_process_exec_block.rs`'s own EXEC-05 leg and
//! `tests/confirm.rs::run_caprun_verb`) — the confirm-release path is a
//! cross-process design (a human runs `caprun confirm` later against a
//! persisted DB), so it can only be proven live via a real subprocess.
//!
//! A single MAC key is shared across every leg (D-13/D-14 consistency): the
//! key is minted/persisted ONCE via `seed_test_key` (idempotent
//! read-existing-first, mirrors `cli/caprun/src/key.rs`'s custody discipline
//! and `s9_process_exec_block.rs`'s own test-local mirror of it) BEFORE any
//! event is appended, so every in-process `append_event`/`verify_chain` call
//! AND the leg (d) `caprun confirm` subprocess (which reads the SAME
//! `<audit_db>.key` file from disk) MAC against the identical key.
//!
//! # Linux-only
//!
//! `invoke_process_exec` spawns the REAL, kernel-confined `caprun-exec-launcher`
//! binary — a macOS run would prove nothing about the confined spawn path (its
//! confinement primitives are no-op stubs there). This file's bodies are
//! `#[cfg(target_os = "linux")]`; `cargo test -p caprun` on macOS compiles this
//! file and runs only the always-on guard test below (0 of the Linux tests run
//! — expected, not a gap, per CLAUDE.md's "Linux-only security tests" /
//! cfg-linux-test-blindness).
//!
//! # Run (Linux, via the project's standing Mailpit-aware recipe, D-13) —
//! scoped to this test target, `cargo build --workspace` FIRST so the
//! `caprun-exec-launcher`/`caprun-worker` sibling binaries exist at
//! `current_exe`-resolution time (cargo-test-workspace-missing-sibling-binary):
//!
//!   MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun \
//!     --test live_acceptance_v1_7_composed' bash scripts/mailpit-verify.sh
//!
//! Capture the TRUE exit code BEFORE any pipe (`set +e; ...; rc=$?; set -e`)
//! and assert on named tests + counts from the captured log — NEVER on `$?`
//! through a pipe (D-13/D-14, `verification-exit-code-through-pipe`).

#[cfg(target_os = "linux")]
mod linux {
    use adapter_fs::workspace::WorkspaceRoot;
    use brokerd::audit::{
        append_event, find_event_by_type, insert_blocked_literal, open_audit_db, verify_chain,
    };
    use brokerd::confirmation::{
        combined_digest, insert_pending_confirmation, PendingConfirmation,
        PendingConfirmationState, ResolvedArg,
    };
    use brokerd::quarantine::mint_from_exec;
    use brokerd::session::persist_session;
    use brokerd::sinks::file_write::invoke_file_write;
    use brokerd::sinks::process_exec::invoke_process_exec;
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::executor_decision::SinkBlockedAnchor;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel, ValueId};
    use runtime_core::{Event, ExecutorDecision, PlanNode, Session, SessionStatus};
    use sha2::{Digest, Sha256};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Persist a `sessions` table row with a CALLER-CHOSEN `session_id`
    /// (`session::create_session` mints its own fresh id, which this
    /// composed test cannot use — each leg needs a known, pre-determined
    /// session id to thread through its own event-seeding calls). Mirrors
    /// `session::persist_session`'s exact row shape; only the id-selection
    /// step differs.
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

    /// Mint a trusted literal directly into the store — mirrors
    /// `s9_process_exec_block.rs`/`s9_file_write_block.rs`'s own
    /// `mint_trusted` precedent: a throwaway anchor Uuid stands in for a real
    /// causal event id (these are trusted control inputs — never the thing
    /// under genuine-taint-anchor test, which is the exec-output mint in leg
    /// (a) below). `origin_role` mirrors the real planner's live flow
    /// (`file.write`'s `path`/`contents` slots reuse the SAME trusted
    /// `"path"`-role literal per DESIGN §4.3); `process.exec`'s `command`/
    /// `args` slots are role-unconstrained (`None`).
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
    /// onto a real parent (mirrors `s9_process_exec_block.rs::seed_root_event`).
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

    /// Test-local mirror of `cli/caprun/src/key.rs`'s idempotent
    /// read-existing-first custody (duplicated verbatim from
    /// `s9_process_exec_block.rs::seed_test_key` — `cli/caprun` has no lib
    /// target, so this external integration-test crate cannot import it
    /// directly, and distinct `tests/*.rs` binaries cannot share a module in
    /// this workspace). Called ONCE, before any leg runs, so every
    /// in-process `append_event`/`verify_chain` call in this file AND the
    /// leg (d) `caprun confirm` subprocess (which independently reads this
    /// SAME `<db_path>.key` file) MAC against the identical key.
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

    /// Run `caprun confirm <effect_id> <db_path>` as a REAL separate OS
    /// process (mirrors `s9_process_exec_block.rs::run_caprun_confirm` /
    /// `tests/confirm.rs::run_caprun_verb`). Returns `(exit_code, stdout)`.
    fn run_caprun_confirm(effect_id: Uuid, db_path: &Path) -> (i32, String) {
        let caprun_bin = env!("CARGO_BIN_EXE_caprun");
        let output = Command::new(caprun_bin)
            .arg("confirm")
            .arg(effect_id.to_string())
            .arg(db_path.to_str().unwrap())
            .output()
            .unwrap_or_else(|e| panic!("spawn caprun confirm: {e}"));
        (
            output.status.code().expect("process must exit with a code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    /// Seed a Pending `process.exec` block directly via brokerd's API against
    /// the SHARED persistent `db_path` (duplicated from
    /// `s9_process_exec_block.rs::seed_pending_process_exec_block`, adapted
    /// to accept the shared `key` as a parameter instead of a file-local
    /// `TEST_KEY` const, since this file's key is minted once via
    /// `seed_test_key` above and threaded through every leg). The blocked
    /// arg is `command`; `args` is a trusted JSON array carrying the
    /// marker-file path. Returns `(effect_id, session_id, blocked_event_id)`.
    fn seed_pending_process_exec_block(
        db_path: &Path,
        key: &[u8],
        command: &str,
        args_json: &str,
        workspace_root: &Path,
    ) -> (Uuid, Uuid, Uuid) {
        let conn = open_audit_db(db_path.to_str().unwrap()).expect("open_audit_db for seeding");

        let session_id = Uuid::new_v4();
        persist_known_session(&conn, session_id);
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, key, &root, None).expect("append session_created");

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(command.as_bytes());
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("process.exec".into()),
            arg: "command".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExecRaw],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };

        let resolved_args = vec![
            ResolvedArg {
                name: "command".into(),
                value_id: ValueId::new(),
                literal: command.to_string(),
                taint: vec![TaintLabel::ExecRaw],
                provenance_chain: vec![read_event_id],
            },
            ResolvedArg {
                name: "args".into(),
                value_id: ValueId::new(),
                literal: args_json.to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![],
            },
        ];
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["command".to_string()];

        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(&conn, key, &blocked_event, Some(&root_hash)).expect("append sink_blocked");
        insert_blocked_literal(&conn, &blocked_event_id.to_string(), "command", command)
            .expect("insert_blocked_literal");

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("process.exec".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: workspace_root.to_string_lossy().into_owned(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(&conn, key, &pc).expect("insert_pending_confirmation");

        (effect_id, session_id, blocked_event_id)
    }

    /// Session/effect discovery safe for a multi-session shared DB (mirrors
    /// `live_acceptance_v1_4_composed.rs::all_session_ids` — never the
    /// unqualified, no-`ORDER BY` `LIMIT 1` anti-pattern).
    fn all_session_ids(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT id FROM sessions ORDER BY rowid")
            .expect("prepare all_session_ids query");
        stmt.query_map([], |row| row.get(0))
            .expect("query all_session_ids")
            .filter_map(Result::ok)
            .collect()
    }

    /// The composed FOUR-leg live acceptance scenario — the v1.7 milestone's
    /// FINAL HARD GATE (LIVE-01, D-12).
    #[tokio::test]
    async fn live_acceptance_v1_7_composed_four_legs() {
        let run_id = Uuid::new_v4();
        let tmp = std::env::temp_dir().join(format!("caprun_live_v17_{run_id}"));
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        // F1-safe layout: shared workspace root under its own subdirectory,
        // audit.db a sibling of that subdirectory (never a direct child of it).
        let ws_dir = tmp.join("workspace");
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        let audit_db = tmp.join("audit.db"); // ONE shared path for ALL FOUR legs — NEVER :memory:
        let audit_db_str = audit_db.to_str().unwrap();

        // Mint/persist the shared MAC key ONCE, before any leg runs (see
        // module header — every leg, including leg (d)'s real subprocess,
        // MACs against this same key).
        let key = seed_test_key(&audit_db);

        // ── LEG (a) BLOCK — genuine (non-stapled) exec-output taint -> I2 ──
        let leg_a_session_id = Uuid::new_v4();
        let leg_a_exec_event_id = {
            let conn = Arc::new(Mutex::new(
                open_audit_db(audit_db_str).expect("open audit db (leg a)"),
            ));
            let (root_id, root_hash) = {
                let locked = conn.lock().expect("lock conn");
                persist_known_session(&locked, leg_a_session_id);
                seed_root_event(&locked, &key, leg_a_session_id)
            };

            let mut store = ValueStore::default();
            let command_vid = mint_trusted(&mut store, "/bin/echo", None);
            let args_json =
                serde_json::to_string(&vec!["v17-leg-a-marker"]).expect("serialize args");
            let args_vid = mint_trusted(&mut store, &args_json, None);
            let plan_node1 = PlanNode {
                sink: SinkId("process.exec".into()),
                args: vec![
                    PlanArg { name: "command".into(), value_id: command_vid },
                    PlanArg { name: "args".into(), value_id: args_vid },
                ],
            };
            let effect_id1 = Uuid::new_v4();
            let decision1 = executor::submit_plan_node(
                leg_a_session_id,
                effect_id1,
                &plan_node1,
                &store,
                &SessionStatus::Active,
                &runtime_core::SessionPolicy::allow_all(),
            );
            assert_eq!(
                decision1,
                ExecutorDecision::Allowed,
                "leg (a): the first (trusted-input) process.exec must Allow"
            );

            let ws_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root (leg a)");
            let (exec_event_id, exec_hash, combined_output) = invoke_process_exec(
                &conn,
                &key,
                &store,
                leg_a_session_id,
                effect_id1,
                &plan_node1,
                &ws_root,
                root_id,
                &root_hash,
            )
            .await
            .expect("invoke_process_exec must succeed for a trusted /bin/echo command");
            assert!(
                combined_output.contains("v17-leg-a-marker"),
                "captured combined_output must contain the echoed marker, got: {combined_output:?}"
            );

            // Anti-stapling DB re-check: the process_exited event is durably
            // in the audit DAG BEFORE we mint from it.
            {
                let locked = conn.lock().expect("lock conn");
                let dag_event =
                    find_event_by_type(&locked, &leg_a_session_id.to_string(), "process_exited")
                        .expect("query process_exited")
                        .expect("process_exited event must exist in the audit DAG");
                assert_eq!(
                    dag_event.id, exec_event_id,
                    "the process_exited DAG event id must equal invoke_process_exec's returned id"
                );
            }

            let output_value_id =
                mint_from_exec(&mut store, leg_a_session_id, combined_output, exec_event_id)
                    .expect("mint_from_exec must succeed");
            let minted = store
                .resolve(&output_value_id)
                .expect("output_value_id must resolve")
                .clone();
            assert_eq!(
                minted.provenance_chain,
                vec![exec_event_id],
                "mint_from_exec's provenance_chain must be EXACTLY [process_exited event id] \
                 (genuine, non-stapled root)"
            );
            assert!(minted.taint.contains(&TaintLabel::ExternalUntrusted));
            assert!(minted.taint.contains(&TaintLabel::ExecRaw));

            // Route the exec-output handle into a SECOND process.exec plan
            // node's `command` arg (routing- AND content-sensitive, role-
            // UNCONSTRAINED — mirrors s9_process_exec_block.rs's own genuine-
            // taint Block leg).
            let plan_node2 = PlanNode {
                sink: SinkId("process.exec".into()),
                args: vec![PlanArg { name: "command".into(), value_id: output_value_id }],
            };
            let effect_id2 = Uuid::new_v4();
            let decision2 = executor::submit_plan_node(
                leg_a_session_id,
                effect_id2,
                &plan_node2,
                &store,
                &SessionStatus::Active,
                &runtime_core::SessionPolicy::allow_all(),
            );
            let anchor = match decision2 {
                ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                    assert_eq!(anchors.len(), 1, "exactly one blocked arg (command)");
                    let blocked = anchors.into_iter().next().expect("one anchor");
                    assert_eq!(blocked.anchor.arg, "command");
                    assert_eq!(blocked.anchor.sink.0, "process.exec");
                    blocked.anchor
                }
                other => panic!(
                    "leg (a): expected BlockedPendingConfirmation for a tainted exec-output \
                     routed to a second process.exec's command arg, got {other:?}"
                ),
            };

            // HELD-OUT GENUINE-TAINT BACKSTOP: the anchor's provenance root is
            // the SAME process_exited event id invoke_process_exec appended —
            // not a fabricated UUID, not a different event.
            assert_eq!(
                anchor.provenance_chain[0], exec_event_id,
                "GENUINE-TAINT BACKSTOP: anchor.provenance_chain[0] must equal the \
                 process_exited event id (non-stapled)"
            );
            assert_eq!(anchor.read_event_id, exec_event_id);

            let block_event = Event::sink_blocked(
                Uuid::new_v4(),
                Some(exec_event_id),
                leg_a_session_id,
                Utc::now(),
                vec![anchor],
                None,
                vec!["command".to_string()],
            );
            {
                let locked = conn.lock().expect("lock conn");
                append_event(&locked, &key, &block_event, Some(&exec_hash))
                    .expect("append sink_blocked");
                let persisted_block =
                    find_event_by_type(&locked, &leg_a_session_id.to_string(), "sink_blocked")
                        .expect("query sink_blocked")
                        .expect("a durable sink_blocked event must exist");
                assert_eq!(persisted_block.id, block_event.id);
                assert!(
                    find_event_by_type(&locked, &leg_a_session_id.to_string(), "sink_executed")
                        .expect("query sink_executed")
                        .is_none(),
                    "leg (a): no sink_executed event may exist — the block prevented any effect"
                );
                assert!(
                    verify_chain(&locked, &leg_a_session_id.to_string(), &key),
                    "leg (a): verify_chain must be true — session_created -> process_exited -> \
                     sink_blocked, one unbroken causal chain"
                );
            }
            exec_event_id
        };
        let _ = leg_a_exec_event_id;

        // ── LEG (b) ALLOW — clean exec, output never routed, no Block ──────
        let leg_b_session_id = Uuid::new_v4();
        {
            let conn = Arc::new(Mutex::new(
                open_audit_db(audit_db_str).expect("open audit db (leg b)"),
            ));
            let (root_id, root_hash) = {
                let locked = conn.lock().expect("lock conn");
                persist_known_session(&locked, leg_b_session_id);
                seed_root_event(&locked, &key, leg_b_session_id)
            };

            let mut store = ValueStore::default();
            let command_vid = mint_trusted(&mut store, "/bin/echo", None);
            let args_json =
                serde_json::to_string(&vec!["v17-leg-b-clean-marker"]).expect("serialize args");
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
                leg_b_session_id,
                effect_id,
                &plan_node,
                &store,
                &SessionStatus::Active,
                &runtime_core::SessionPolicy::allow_all(),
            );
            assert_eq!(
                decision,
                ExecutorDecision::Allowed,
                "leg (b): CLEAN-ALLOW CONTROL — trusted command/args process.exec must Allow"
            );

            let ws_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root (leg b)");
            let (exec_event_id, _hash, combined_output) = invoke_process_exec(
                &conn,
                &key,
                &store,
                leg_b_session_id,
                effect_id,
                &plan_node,
                &ws_root,
                root_id,
                &root_hash,
            )
            .await
            .expect("invoke_process_exec must succeed");
            assert!(combined_output.contains("v17-leg-b-clean-marker"));

            let output_value_id =
                mint_from_exec(&mut store, leg_b_session_id, combined_output, exec_event_id)
                    .expect("mint_from_exec must succeed");
            let minted = store
                .resolve(&output_value_id)
                .expect("output_value_id must resolve");
            assert!(minted.taint.iter().any(|t| t.is_untrusted()));

            // Never routed anywhere -> no Block.
            let locked = conn.lock().expect("lock conn");
            assert!(
                find_event_by_type(&locked, &leg_b_session_id.to_string(), "sink_blocked")
                    .expect("query sink_blocked")
                    .is_none(),
                "leg (b): no sink_blocked event may exist — the clean exec Allowed and its \
                 output was never routed anywhere"
            );
            assert!(
                verify_chain(&locked, &leg_b_session_id.to_string(), &key),
                "leg (b): verify_chain must be true"
            );
        }

        // ── LEG (c) FS WRITE — trusted path/contents Allows, file within ───
        // WorkspaceRoot is overwritten, durably audited.
        let leg_c_session_id = Uuid::new_v4();
        {
            // write_within requires the target to ALREADY exist (O_TRUNC,
            // never O_CREAT) — pre-create it under the shared workspace.
            std::fs::write(ws_dir.join("v17-leg-c-existing.txt"), b"original")
                .expect("pre-create leg (c) target file");

            let conn = open_audit_db(audit_db_str).expect("open audit db (leg c)");
            persist_known_session(&conn, leg_c_session_id);
            let (root_id, root_hash) = seed_root_event(&conn, &key, leg_c_session_id);

            let mut store = ValueStore::default();
            let path_vid = mint_trusted(&mut store, "v17-leg-c-existing.txt", Some("path"));
            let contents_vid =
                mint_trusted(&mut store, "hello from live_acceptance_v1_7_composed leg (c)", Some("path"));
            let plan_node = PlanNode {
                sink: SinkId("file.write".into()),
                args: vec![
                    PlanArg { name: "path".into(), value_id: path_vid },
                    PlanArg { name: "contents".into(), value_id: contents_vid },
                ],
            };
            let effect_id = Uuid::new_v4();
            let decision = executor::submit_plan_node(
                leg_c_session_id,
                effect_id,
                &plan_node,
                &store,
                &SessionStatus::Active,
                &runtime_core::SessionPolicy::allow_all(),
            );
            assert_eq!(
                decision,
                ExecutorDecision::Allowed,
                "leg (c): a trusted path/contents file.write must Allow"
            );

            let ws_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root (leg c)");
            let (evt_id, _hash) = invoke_file_write(
                &conn,
                &key,
                &store,
                leg_c_session_id,
                effect_id,
                &plan_node,
                &ws_root,
                root_id,
                &root_hash,
            )
            .expect("invoke_file_write must succeed on an existing target");

            let on_disk =
                std::fs::read_to_string(ws_dir.join("v17-leg-c-existing.txt")).expect("read back");
            assert_eq!(on_disk, "hello from live_acceptance_v1_7_composed leg (c)");

            let evt = find_event_by_type(&conn, &leg_c_session_id.to_string(), "sink_executed")
                .expect("query sink_executed")
                .expect("leg (c): a durable sink_executed event must exist");
            assert_eq!(evt.id, evt_id);
            assert!(
                verify_chain(&conn, &leg_c_session_id.to_string(), &key),
                "leg (c): verify_chain must be true"
            );
        }

        // ── LEG (d) EXEC-05 RELEASE — a Blocked process.exec is released ───
        // via a REAL `caprun confirm` subprocess: runs exactly once, chains
        // onto confirm_granted, verify_chain true (D-04/D-06/D-11).
        let leg_d_marker = ws_dir.join("v17-leg-d-released.marker");
        let leg_d_args_json =
            serde_json::to_string(&vec![leg_d_marker.to_string_lossy().into_owned()])
                .expect("serialize leg (d) args");
        let (leg_d_effect_id, leg_d_session_id, _leg_d_blocked_event_id) =
            seed_pending_process_exec_block(
                &audit_db,
                &key,
                "/usr/bin/touch",
                &leg_d_args_json,
                &ws_dir,
            );

        let (code, stdout) = run_caprun_confirm(leg_d_effect_id, &audit_db);
        assert_eq!(
            code, 0,
            "leg (d): first confirm on a Pending process.exec block must exit 0 (Released)"
        );
        assert!(
            stdout.contains("Taint:"),
            "leg (d): stdout must show the CONFIRM-01 block display; got:\n{stdout}"
        );
        assert!(
            leg_d_marker.exists(),
            "leg (d): the released command must have run exactly once, creating the marker file"
        );

        {
            let reconn = open_audit_db(audit_db_str).expect("reopen persisted audit DB (leg d)");
            let (granted_id, granted_actor): (String, String) = reconn
                .query_row(
                    "SELECT id, actor FROM events WHERE session_id = ?1 AND event_type = 'confirm_granted'",
                    [leg_d_session_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("confirm_granted event must exist");
            assert!(granted_actor.contains(&leg_d_effect_id.to_string()));

            let exited_parent: Option<String> = reconn
                .query_row(
                    "SELECT parent_id FROM events WHERE session_id = ?1 AND event_type = 'process_exited'",
                    [leg_d_session_id.to_string()],
                    |row| row.get(0),
                )
                .expect("process_exited event must exist");
            assert_eq!(
                exited_parent.as_deref(),
                Some(granted_id.as_str()),
                "leg (d): process_exited must chain directly onto confirm_granted — the real \
                 head (D-04), never a fabricated root"
            );

            assert!(
                verify_chain(&reconn, &leg_d_session_id.to_string(), &key),
                "leg (d): verify_chain must be true — session_created -> sink_blocked -> \
                 confirm_granted -> process_exited, one unbroken causal chain"
            );
        }

        // Second confirm on the same effect_id: AlreadyTerminal, no double-spawn.
        let (code2, _stdout2) = run_caprun_confirm(leg_d_effect_id, &audit_db);
        assert_eq!(
            code2, 5,
            "leg (d): a second confirm on an already-Confirmed effect_id must exit 5 \
             (AlreadyTerminal)"
        );
        {
            let reconn = open_audit_db(audit_db_str).expect("reopen persisted audit DB (leg d 2)");
            let exited_count: i64 = reconn
                .query_row(
                    "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = 'process_exited'",
                    [leg_d_session_id.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                exited_count, 1,
                "leg (d): no double-spawn — exactly ONE process_exited event must exist after \
                 both confirms"
            );
        }

        // ── END-OF-RUN SWEEP — open the shared audit_db ONCE, verify every ──
        // session independently (composed-run semantics: one shared audit.db
        // file, every session's verify_chain independently true — never a
        // single cross-session parent_id chain).
        let conn = open_audit_db(audit_db_str).expect("open shared audit DB (final sweep)");
        let sids = all_session_ids(&conn);
        assert_eq!(
            sids.len(),
            4,
            "exactly the four composed sessions (a/b/c/d) must exist in the shared audit.db"
        );
        for sid in &sids {
            assert!(
                verify_chain(&conn, sid, &key),
                "verify_chain must be true for session {sid} (per-session, enumerated via \
                 ORDER BY rowid, never LIMIT 1)"
            );
        }
        for sid in [
            &leg_a_session_id,
            &leg_b_session_id,
            &leg_c_session_id,
            &leg_d_session_id,
        ] {
            assert!(
                sids.contains(&sid.to_string()),
                "session {sid} must be among the four enumerated sessions in the final sweep"
            );
        }

        std::fs::remove_dir_all(&tmp).ok();
    }
}

/// Cross-platform guard: keeps `cargo test -p caprun` meaningful on the macOS
/// dev box (where the Linux body above is cfg-excluded, 0 tests reported —
/// expected, not a gap). Confirms the caprun binary is wired into the test
/// build (mirrors `s9_process_exec_block.rs`'s / `live_acceptance_v1_4_composed.rs`'s
/// own guard).
#[test]
fn live_acceptance_v1_7_composed_guard_binary_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(
        !caprun_bin.is_empty(),
        "CARGO_BIN_EXE_caprun must resolve — caprun must be built for the live test"
    );
}
