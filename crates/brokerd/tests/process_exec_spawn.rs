//! process_exec_spawn — EXEC-01/04 integration coverage for
//! `sinks::process_exec::invoke_process_exec`: spawn-via-launcher + capture,
//! wall-clock-timeout-kill, and combined-output byte-cap fail-closed, plus
//! the two-phase `process_exited`/`process_spawn_failed` durable audit
//! chained onto the causal head.
//!
//! Linux-only (CLAUDE.md "Linux-only security tests"): these tests exercise
//! the REAL `caprun-exec-launcher` binary (32-03), which only actually
//! self-confines under `#[cfg(target_os = "linux")]` — on macOS the
//! confinement primitives are no-op stubs, so a Mac run of this file would
//! prove nothing about the confined spawn path. On Mac this file compiles
//! (`cargo test -p brokerd --no-run`) and reports 0 tests — expected, not a
//! gap (this project's own standing precedent, cfg-linux-test-blindness).
//! The Linux compile-check + run happens in the 32-06 container step
//! (`scripts/mailpit-verify.sh`, per Pitfall 3: run `cargo build --workspace`
//! first so the sibling `caprun-exec-launcher` binary is placed).

#[cfg(target_os = "linux")]
mod linux {
    use adapter_fs::workspace::WorkspaceRoot;
    use brokerd::audit::{append_event, find_event_by_type, open_audit_db, verify_chain};
    use brokerd::sinks::process_exec::invoke_process_exec;
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel};
    use runtime_core::{Event, PlanNode};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    /// Fixed, non-secret test MAC key (mirrors `file_create.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"process-exec-spawn-rs-integration-test-key";

    /// Build a `process.exec` plan node whose `command`/`args` resolve to the
    /// given trusted literals in a fresh store (mirrors DESIGN's locked
    /// decision 1: `args` is ONE `PlanArg` whose literal is a JSON-serialized
    /// `Vec<String>`), plus a seeded causal-root event. Returns
    /// (store, plan_node, conn, session_id, root_id, root_hash).
    fn setup(
        command: &str,
        args: &[&str],
    ) -> (
        ValueStore,
        PlanNode,
        Arc<Mutex<rusqlite::Connection>>,
        Uuid,
        Uuid,
        String,
    ) {
        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let mut store = ValueStore::default();
        // Seed a causal-root event so the sink event can chain onto it and
        // verify_chain has an unbroken parent linkage.
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();

        // Mint command + args(JSON) as trusted values (this is the Allowed path).
        let ev = Uuid::new_v4();
        let command_vid = store
            .mint(
                command.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![ev],
                None,
            )
            .unwrap();
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let args_json = serde_json::to_string(&args_vec).unwrap();
        let args_vid = store
            .mint(args_json, vec![TaintLabel::UserTrusted], vec![ev], None)
            .unwrap();

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
        (
            store,
            plan_node,
            Arc::new(Mutex::new(conn)),
            session_id,
            root.id,
            root_hash,
        )
    }

    /// A temp workspace root — `EXEC_WORKSPACE_ROOT` is required by the
    /// launcher (it independently resolves its own Landlock allow-rule from
    /// this path, 32-03), even though these tests don't exercise file I/O
    /// under it.
    fn temp_workspace(tag: &str) -> (std::path::PathBuf, WorkspaceRoot) {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_pe_{tag}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();
        (root, ws)
    }

    /// (a) A benign command (`/bin/echo hello`) spawns via the launcher,
    /// captures "hello" in `combined_output`, and appends a chained
    /// `process_exited` event tainted `[ExternalUntrusted, ExecRaw]`.
    #[tokio::test]
    async fn process_exec_spawns_launcher_captures_output_and_chains_process_exited() {
        let (root, ws) = temp_workspace("ok");
        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup("/bin/echo", &["hello"]);
        let effect_id = Uuid::new_v4();

        let (evt_id, hash, combined_output) = invoke_process_exec(
            &conn,
            TEST_KEY,
            &store,
            session_id,
            effect_id,
            &plan_node,
            &ws,
            parent_id,
            &parent_hash,
        )
        .await
        .expect("process.exec of /bin/echo must succeed via the confined launcher");

        assert!(!hash.is_empty());
        assert!(
            combined_output.contains("hello"),
            "combined_output was: {combined_output:?}"
        );

        let locked = conn.lock().unwrap();
        let evt = find_event_by_type(&locked, &session_id.to_string(), "process_exited")
            .unwrap()
            .expect("process_exited event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.parent_id, Some(parent_id));
        assert_eq!(evt.actor, format!("sink:process.exec:{effect_id}"));
        assert_eq!(
            evt.taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw],
            "process_exited must carry the untrusted-origin exec taint pair"
        );
        assert!(
            find_event_by_type(&locked, &session_id.to_string(), "process_spawn_failed")
                .unwrap()
                .is_none(),
            "no process_spawn_failed on the success path"
        );
        assert!(
            verify_chain(&locked, &session_id.to_string(), TEST_KEY),
            "the causal chain must stay intact after appending process_exited"
        );
        drop(locked);

        std::fs::remove_dir_all(&root).ok();
    }

    /// (b) A long-sleeping command (`/bin/sleep 40`, exceeding the fixed 30s
    /// wall-clock timeout) is killed and appends `process_spawn_failed`. The
    /// timeout const is not env-tunable (a deployment constant, not a
    /// swappable policy knob), so this test asserts the elapsed-time bound
    /// instead of overriding it (plan-sanctioned alternative).
    #[tokio::test]
    async fn process_exec_wall_clock_timeout_kills_child_and_records_process_spawn_failed() {
        let (root, ws) = temp_workspace("timeout");
        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup("/bin/sleep", &["40"]);
        let effect_id = Uuid::new_v4();

        let started = Instant::now();
        let result = invoke_process_exec(
            &conn,
            TEST_KEY,
            &store,
            session_id,
            effect_id,
            &plan_node,
            &ws,
            parent_id,
            &parent_hash,
        )
        .await;
        let elapsed = started.elapsed();

        assert!(
            result.is_err(),
            "a 40s sleep must be killed by the 30s wall-clock timeout, not run to completion"
        );
        assert!(
            elapsed >= Duration::from_secs(28) && elapsed < Duration::from_secs(38),
            "expected elapsed time to be bounded by the ~30s wall-clock timeout, got {elapsed:?}"
        );

        let locked = conn.lock().unwrap();
        let evt = find_event_by_type(&locked, &session_id.to_string(), "process_spawn_failed")
            .unwrap()
            .expect("process_spawn_failed event must exist");
        assert_eq!(evt.parent_id, Some(parent_id));
        assert!(
            find_event_by_type(&locked, &session_id.to_string(), "process_exited")
                .unwrap()
                .is_none(),
            "no process_exited on the timeout-kill failure path"
        );
        assert!(
            verify_chain(&locked, &session_id.to_string(), TEST_KEY),
            "the causal chain must stay intact after appending process_spawn_failed"
        );
        drop(locked);

        std::fs::remove_dir_all(&root).ok();
    }

    /// (c) An output-flooding command (`/bin/yes`, an unbounded stdout write
    /// loop) trips the combined 10 MiB byte cap fail-closed: the invocation
    /// errors and a `process_spawn_failed` event is recorded — the captured
    /// output is NEVER allowed to grow unbounded past the cap.
    #[tokio::test]
    async fn process_exec_byte_cap_fail_closed_records_process_spawn_failed() {
        let (root, ws) = temp_workspace("cap");
        let (store, plan_node, conn, session_id, parent_id, parent_hash) = setup("/bin/yes", &[]);
        let effect_id = Uuid::new_v4();

        let result = invoke_process_exec(
            &conn,
            TEST_KEY,
            &store,
            session_id,
            effect_id,
            &plan_node,
            &ws,
            parent_id,
            &parent_hash,
        )
        .await;

        assert!(
            result.is_err(),
            "/bin/yes's unbounded output must trip the byte cap and fail closed, never fail open"
        );

        let locked = conn.lock().unwrap();
        let evt = find_event_by_type(&locked, &session_id.to_string(), "process_spawn_failed")
            .unwrap()
            .expect("process_spawn_failed event must exist");
        assert_eq!(evt.parent_id, Some(parent_id));
        assert!(
            find_event_by_type(&locked, &session_id.to_string(), "process_exited")
                .unwrap()
                .is_none(),
            "no process_exited on the byte-cap failure path"
        );
        assert!(
            verify_chain(&locked, &session_id.to_string(), TEST_KEY),
            "the causal chain must stay intact after appending process_spawn_failed"
        );
        drop(locked);

        std::fs::remove_dir_all(&root).ok();
    }
}

/// On non-Linux (this dev machine is a Mac), the tests above are compiled out
/// entirely — `#[cfg(target_os = "linux")]` gates the whole `linux` module.
/// This is expected, not a gap: the launcher's actual confinement (and thus
/// the real spawn/capture/timeout/byte-cap behavior this file asserts) only
/// applies on Linux. `cargo test -p brokerd --no-run` still confirms this
/// file COMPILES on Mac; the Linux container step actually RUNS it.
#[cfg(not(target_os = "linux"))]
mod not_linux {}
