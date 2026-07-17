//! s9_process_exec_block — EXEC-02/EXEC-03 per-requirement acceptance
//! (Linux-gated): a genuine (non-stapled) exec-output taint chain, deterministically
//! Blocked when routed into a sensitive sink arg.
//!
//! Phase 32's four success criteria are proven per-requirement across several
//! test files this phase (see 32-06-PLAN.md); THIS file proves EXEC-02/EXEC-03:
//! `process.exec`'s captured output is genuinely taint-minted (rooted on the SAME
//! `process_exited` audit event `invoke_process_exec` already appended — never a
//! fresh/fabricated root), and a plan node that routes that handle into a
//! sensitive sink arg (`process.exec`/`command` of a SECOND plan node)
//! deterministically Blocks with an
//! unbroken audit-DAG edge and `verify_chain` true.
//!
//! # Harness shape
//!
//! This mirrors `s9_live_block.rs`'s "stand up a broker session, drive plan
//! nodes, assert on the audit DAG + verify_chain" spirit, but does NOT drive
//! the full `caprun` CLI subprocess (there is no `process.exec` intent kind on
//! the intent-first CLI yet — that composed live acceptance is Phase 34's
//! LIVE-01). Instead this exercises the SAME production functions directly,
//! in-process: `executor::submit_plan_node` (the I2 decision), `mint_from_read`/
//! `mint_from_exec` (the SOLE broker taint-mint sites, `brokerd::quarantine`),
//! and `invoke_process_exec` (the SOLE `process.exec` spawn+capture+audit sink,
//! `brokerd::sinks::process_exec`). The one piece of production orchestration
//! this test inlines rather than calls is `evaluate_plan_node_and_record`'s
//! block-recording (that function is private to `server.rs`) — mirrors the
//! established `s9_acceptance.rs` precedent (which does the identical inline
//! `Event::sink_blocked` + `append_event` construction for the SAME reason).
//!
//! # Linux-only
//!
//! `invoke_process_exec` spawns the REAL `caprun-exec-launcher` binary via
//! `tokio::process::Command` — on macOS the launcher's own confinement
//! primitives are no-op stubs (it would `execve` the target UNCONFINED), so a
//! Mac run would prove nothing about the confined spawn path. This file's
//! bodies are `#[cfg(target_os = "linux")]`; `cargo test -p caprun` on Mac
//! compiles this file and runs only the always-on guard test below (0 of the
//! Linux tests run — expected, not a gap, per project CLAUDE.md's "Linux-only
//! security tests" section / cfg-linux-test-blindness).
//!
//! Run the Linux assertions under Colima/Docker:
//!
//!   docker run --rm --security-opt seccomp=unconfined \
//!     -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
//!     rust:1 bash -c "cargo build --workspace && cargo test -p caprun --test s9_process_exec_block"

#[cfg(target_os = "linux")]
mod linux {
    use adapter_fs::workspace::WorkspaceRoot;
    use brokerd::audit::{append_event, find_event_by_type, open_audit_db, verify_chain};
    use brokerd::quarantine::{mint_from_exec, mint_from_read, Claim};
    use brokerd::sinks::process_exec::invoke_process_exec;
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel, ValueId};
    use runtime_core::{Event, ExecutorDecision, PlanNode, SessionStatus};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Fixed, non-secret test MAC key (mirrors `process_exec_spawn.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"s9-process-exec-block-rs-integration-test-key";

    /// Mint a trusted literal directly into the store — mirrors
    /// `crates/brokerd/tests/process_exec_spawn.rs::setup()`'s own precedent
    /// for seeding a plan node's `command`/`args` inputs: a throwaway anchor
    /// Uuid stands in for a real causal event id. This is acceptable here
    /// because these are the FIRST plan node's OWN trusted inputs — never the
    /// thing under genuine-taint-anchor test (that is the exec-OUTPUT mint,
    /// asserted against the real `process_exited` event id below).
    fn mint_trusted(store: &mut ValueStore, literal: &str) -> ValueId {
        store
            .mint(
                literal.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![Uuid::new_v4()],
                None,
            )
            .expect("mint trusted literal")
    }

    /// Seed a `session_created` causal-root event so subsequent appends chain
    /// onto a real parent (mirrors `process_exec_spawn.rs::setup()`).
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

    /// A fresh, real temp workspace directory the launcher's Landlock
    /// exec-child ruleset resolves as its `EXEC_WORKSPACE_ROOT`.
    fn fresh_workspace(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("caprun_s9_exec_{tag}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create workspace dir");
        dir
    }

    /// (a) EXEC-03 acceptance — genuine (non-stapled) taint -> deterministic
    /// I2 Block, with an unbroken audit-DAG edge and `verify_chain` true.
    ///
    /// 1. A TRUSTED `process.exec` (command=`/bin/echo`, a marker arg) Allows.
    /// 2. `invoke_process_exec` spawns the real, kernel-confined launcher,
    ///    captures the echoed marker, and durably appends `process_exited`.
    /// 3. `mint_from_exec` mints the captured output rooted on THAT SAME event
    ///    id — asserted directly against the DB row, not just the in-memory
    ///    return value (the anti-stapling check).
    /// 4. A SECOND plan node routes the exec-output handle into a NEW
    ///    `process.exec` plan node's `command` arg (routing- AND
    ///    content-sensitive, and deliberately role-UNCONSTRAINED —
    ///    `sink_sensitivity::expected_role` returns `None` for
    ///    `process.exec`/`command`, since no legitimate exec command has an
    ///    `origin_role`-producing mint site; `email.send`/`body` and
    ///    `file.create`/`path`/`contents` are ALL role-checked and reject
    ///    `mint_from_exec`'s `origin_role = Some("exec_output")` with a
    ///    `SlotTypeMismatch` `Denied` BEFORE the taint check ever runs — this
    ///    was empirically discovered running this test's first draft against
    ///    `email.send`/`body` in the Linux container) -> a durable
    ///    `BlockedPendingConfirmation`, whose anchor's
    ///    `provenance_chain[0]`/`read_event_id` equal the `process_exited`
    ///    event id (the held-out genuine-taint backstop, T-04-03's exec analog).
    /// 5. The block is durably persisted (`sink_blocked`) and `verify_chain`
    ///    holds over the whole session (`session_created` -> `process_exited`
    ///    -> `sink_blocked`, one unbroken causal chain).
    #[tokio::test]
    async fn s9_process_exec_genuine_taint_block() {
        let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
        let session_id = Uuid::new_v4();
        let (root_id, root_hash) = {
            let locked = conn.lock().expect("lock conn");
            seed_root_event(&locked, session_id)
        };

        let mut store = ValueStore::default();
        let command_vid = mint_trusted(&mut store, "/bin/echo");
        let args_json = serde_json::to_string(&vec!["s9-exec-marker"]).expect("serialize args");
        let args_vid = mint_trusted(&mut store, &args_json);

        let plan_node1 = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![
                PlanArg {
                    name: "command".into(),
                    value_id: command_vid,
                },
                PlanArg {
                    name: "args".into(),
                    value_id: args_vid,
                },
            ],
        };
        let effect_id1 = Uuid::new_v4();
        let decision1 = executor::submit_plan_node(
            session_id,
            effect_id1,
            &plan_node1,
            &store,
            &SessionStatus::Active,
        );
        assert_eq!(
            decision1,
            ExecutorDecision::Allowed,
            "CLEAN ALLOW control: a trusted command/args process.exec must Allow"
        );

        let ws_dir = fresh_workspace("block");
        let workspace_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root");

        let (exec_event_id, exec_hash, combined_output) = invoke_process_exec(
            &conn,
            TEST_KEY,
            &store,
            session_id,
            effect_id1,
            &plan_node1,
            &workspace_root,
            root_id,
            &root_hash,
        )
        .await
        .expect("invoke_process_exec must succeed for a trusted /bin/echo command");

        assert!(
            combined_output.contains("s9-exec-marker"),
            "captured combined_output must contain the echoed marker, got: {combined_output:?}"
        );

        // The process_exited event is durably in the audit DAG (not merely
        // returned in-memory) BEFORE we mint from it — confirmed by a fresh DB
        // lookup, mirroring s9_acceptance.rs's anti-stapling DB re-check.
        {
            let locked = conn.lock().expect("lock conn");
            let dag_event = find_event_by_type(&locked, &session_id.to_string(), "process_exited")
                .expect("query process_exited")
                .expect("process_exited event must exist in the audit DAG");
            assert_eq!(
                dag_event.id, exec_event_id,
                "the process_exited DAG event id must equal invoke_process_exec's returned id"
            );
        }

        let output_value_id = mint_from_exec(&mut store, session_id, combined_output, exec_event_id)
            .expect("mint_from_exec must succeed");

        // Genuine, non-stapled taint anchor: provenance_chain is EXACTLY
        // [process_exited event id] — never a fresh/fabricated root.
        let minted = store
            .resolve(&output_value_id)
            .expect("output_value_id must resolve")
            .clone();
        assert_eq!(
            minted.provenance_chain,
            vec![exec_event_id],
            "mint_from_exec's provenance_chain must be EXACTLY [process_exited event id]"
        );
        assert!(minted.taint.contains(&TaintLabel::ExternalUntrusted));
        assert!(minted.taint.contains(&TaintLabel::ExecRaw));

        // Route the exec-output handle into a SECOND process.exec plan node's
        // `command` arg — routing- AND content-sensitive, and role-
        // UNCONSTRAINED (unlike email.send/body or file.create/path/contents,
        // which are role-checked and would Deny `origin_role =
        // Some("exec_output")` with SlotTypeMismatch before the taint check
        // ever runs — see the doc comment above this test).
        let plan_node2 = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![PlanArg {
                name: "command".into(),
                value_id: output_value_id,
            }],
        };
        let effect_id2 = Uuid::new_v4();
        let decision2 = executor::submit_plan_node(
            session_id,
            effect_id2,
            &plan_node2,
            &store,
            &SessionStatus::Active,
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
                "expected BlockedPendingConfirmation for a tainted exec-output routed to \
                 a second process.exec's command arg, got {other:?}"
            ),
        };

        // HELD-OUT GENUINE-TAINT BACKSTOP (EXEC-03's T-04-03 analog): the
        // anchor's provenance root is the SAME process_exited event id
        // invoke_process_exec appended — not a fabricated UUID, not a
        // different event. A stapled-taint implementation would fail here.
        assert_eq!(
            anchor.provenance_chain[0], exec_event_id,
            "GENUINE-TAINT BACKSTOP: anchor.provenance_chain[0] must equal the \
             process_exited event id (non-stapled)"
        );
        assert_eq!(
            anchor.read_event_id, exec_event_id,
            "anchor.read_event_id must equal the process_exited event id"
        );

        // Durably persist the block. `evaluate_plan_node_and_record` (the
        // production block-recording orchestration) is private to server.rs;
        // this inlines the SAME `Event::sink_blocked` + `append_event` call
        // shape s9_acceptance.rs already establishes as the sanctioned
        // in-process proof pattern for this exact constraint.
        let block_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(exec_event_id),
            session_id,
            Utc::now(),
            vec![anchor],
            None,
            vec!["command".to_string()],
        );
        {
            let locked = conn.lock().expect("lock conn");
            append_event(&locked, TEST_KEY, &block_event, Some(&exec_hash))
                .expect("append sink_blocked");
        }

        let locked = conn.lock().expect("lock conn");
        let persisted_block = find_event_by_type(&locked, &session_id.to_string(), "sink_blocked")
            .expect("query sink_blocked")
            .expect("a durable sink_blocked event must exist");
        assert_eq!(persisted_block.id, block_event.id);
        assert!(
            find_event_by_type(&locked, &session_id.to_string(), "sink_executed")
                .expect("query sink_executed")
                .is_none(),
            "no sink_executed event may exist — the block prevented any effect"
        );
        assert!(
            verify_chain(&locked, &session_id.to_string(), TEST_KEY),
            "verify_chain must be true — ONE unbroken causal chain: \
             session_created -> process_exited -> sink_blocked"
        );
    }

    /// (b) CLEAN ALLOW control + non-sensitive-context framing note.
    ///
    /// A benign `process.exec` (trusted command/args) Allows, and its captured
    /// output — though unconditionally tainted `[ExternalUntrusted, ExecRaw]`
    /// by `mint_from_exec` regardless of the target's own benign behavior — is
    /// never routed into ANY sink arg, so no Block occurs.
    ///
    /// HONEST SCOPE NOTE: every arg on every sink currently registered in
    /// `sink_sensitivity.rs` (`email.send`: to/cc/bcc/subject/body;
    /// `file.create`: path/contents; `process.exec`: command/args/cwd) is
    /// EITHER routing- OR content-sensitive — there is no registered
    /// sink/arg pair that is genuinely non-sensitive to route a tainted value
    /// into (by design: I2 must Block everywhere a tainted value could
    /// redirect or exfiltrate). "Routed to a non-sensitive context" is
    /// therefore represented here by the exec-output handle simply never
    /// being submitted in any `PlanNode` at all — the only context that is
    /// unconditionally non-sensitive is no context. This is a deliberate,
    /// documented scope decision (never a fabricated sink) contrasting with
    /// scenario (a) above, which explicitly routes the SAME kind of handle
    /// into a real sensitive arg and Blocks.
    #[tokio::test]
    async fn s9_process_exec_clean_allow_unrouted_output_causes_no_block() {
        let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
        let session_id = Uuid::new_v4();
        let (root_id, root_hash) = {
            let locked = conn.lock().expect("lock conn");
            seed_root_event(&locked, session_id)
        };

        let mut store = ValueStore::default();
        let command_vid = mint_trusted(&mut store, "/bin/echo");
        let args_json =
            serde_json::to_string(&vec!["clean-control-marker"]).expect("serialize args");
        let args_vid = mint_trusted(&mut store, &args_json);
        let plan_node = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![
                PlanArg {
                    name: "command".into(),
                    value_id: command_vid,
                },
                PlanArg {
                    name: "args".into(),
                    value_id: args_vid,
                },
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
            "CLEAN-ALLOW CONTROL: a trusted command/args process.exec must Allow"
        );

        let ws_dir = fresh_workspace("clean");
        let workspace_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root");
        let (exec_event_id, _hash, combined_output) = invoke_process_exec(
            &conn,
            TEST_KEY,
            &store,
            session_id,
            effect_id,
            &plan_node,
            &workspace_root,
            root_id,
            &root_hash,
        )
        .await
        .expect("invoke_process_exec must succeed");

        let output_value_id = mint_from_exec(&mut store, session_id, combined_output, exec_event_id)
            .expect("mint_from_exec must succeed");
        let minted = store
            .resolve(&output_value_id)
            .expect("output_value_id must resolve");
        assert!(
            minted.taint.iter().any(|t| t.is_untrusted()),
            "exec output is unconditionally tainted untrusted, regardless of the \
             target's own benign behavior"
        );

        // The positive control: no sink_blocked event exists for this session
        // — the clean exec Allowed, and the output, never routed anywhere,
        // causes no Block.
        let locked = conn.lock().expect("lock conn");
        assert!(
            find_event_by_type(&locked, &session_id.to_string(), "sink_blocked")
                .expect("query sink_blocked")
                .is_none(),
            "no sink_blocked event may exist on the clean allow-path when the exec \
             output is never routed anywhere"
        );
    }

    /// (c) TAINTED-COMMAND negative — a `process.exec` whose `command` value
    /// is itself untrusted-tainted Blocks BEFORE any spawn: no `process_exited`
    /// event is ever appended (the Block happens at the command-arg
    /// sensitivity check, mirroring `server.rs`'s dispatch condition: only an
    /// `Allowed` decision ever calls `invoke_process_exec`).
    #[tokio::test]
    async fn s9_process_exec_tainted_command_blocks_before_spawn() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let mut store = ValueStore::default();

        // Tainted "command" via the SAME production genuine-taint mint site
        // (mint_from_read) the file.create/email.send hostile paths use —
        // never a hand-set taint field.
        let claim = Claim {
            claim_type: "relative_path".into(),
            value: "malicious/binary".into(),
        };
        let (read_event_id, _read_hash, tainted_command_vid, _demoted_id, _demoted_hash) =
            mint_from_read(&conn, TEST_KEY, &mut store, session_id, &claim, None, None)
                .expect("mint_from_read");

        let args_vid = mint_trusted(&mut store, "[]");

        let plan_node = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![
                PlanArg {
                    name: "command".into(),
                    value_id: tainted_command_vid,
                },
                PlanArg {
                    name: "args".into(),
                    value_id: args_vid,
                },
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

        match decision {
            ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                assert_eq!(
                    anchors.len(),
                    1,
                    "exactly the tainted `command` arg blocks (args is trusted)"
                );
                assert_eq!(anchors[0].anchor.arg, "command");
                assert_eq!(anchors[0].anchor.provenance_chain[0], read_event_id);
            }
            other => panic!(
                "expected BlockedPendingConfirmation for a tainted process.exec command, \
                 got {other:?}"
            ),
        }

        // Never spawned: since the decision was NOT Allowed, invoke_process_exec
        // is never called here (mirrors server.rs's Allowed-only dispatch
        // condition) — no process_exited event exists in the audit DAG.
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "process_exited")
                .expect("query process_exited")
                .is_none(),
            "a tainted command must Block BEFORE any spawn — no process_exited event \
             may exist"
        );
    }
}

/// Cross-platform guard: keeps `cargo test -p caprun` meaningful on the macOS
/// dev box (where the Linux bodies above are cfg-excluded, 0 tests reported —
/// expected, not a gap). Confirms the caprun binary is wired into the test
/// build (mirrors `s9_live_block.rs`'s guard).
#[test]
fn s9_process_exec_block_guard_binary_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(
        !caprun_bin.is_empty(),
        "CARGO_BIN_EXE_caprun must resolve — caprun must be built for the live test"
    );
}
