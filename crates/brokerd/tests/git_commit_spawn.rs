//! git_commit_spawn — GIT-01 integration coverage for
//! `sinks::git_commit::invoke_git_commit`: a genuine broker-mediated
//! `git commit` via the confined `caprun-exec-launcher`, its non-stapled
//! `process_exited`-rooted taint edge (minted via the SAME `mint_from_exec`
//! server.rs uses), the P2 RCE neutralization (a planted `.git/hooks/pre-commit`
//! + config alias does NOT execute), and the exec-child env-clear (the git child
//! inherits none of the broker env).
//!
//! Linux-only (CLAUDE.md "Linux-only security tests" / cfg-linux-test-blindness):
//! these tests genuinely spawn the real `caprun-exec-launcher` (32-03) + the
//! system `git` binary. The launcher only actually self-confines under
//! `#[cfg(target_os = "linux")]` — on macOS the confinement primitives are
//! no-op stubs, so a Mac run would prove nothing about the confined spawn path.
//! On Mac this file COMPILES (`cargo test -p brokerd --no-run`) and reports 0
//! tests — EXPECTED, never a gap. Real verification is the Phase 40 / container
//! step (`scripts/mailpit-verify.sh` with `cargo build --workspace` first so the
//! sibling `caprun-exec-launcher` binary is placed — Pitfall 3).

#[cfg(target_os = "linux")]
mod linux {
    use adapter_fs::workspace::WorkspaceRoot;
    use brokerd::audit::{append_event, find_event_by_type, open_audit_db, verify_chain};
    use brokerd::quarantine::mint_from_exec;
    use brokerd::sinks::git_commit::invoke_git_commit;
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel};
    use runtime_core::{Event, PlanNode};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Fixed, non-secret test MAC key (mirrors `process_exec_spawn.rs`).
    const TEST_KEY: &[u8] = b"git-commit-spawn-rs-integration-test-key";

    /// Run the system `git` (in the TEST process, unconfined) inside `dir` for
    /// setup + assertions. Returns (success, stdout-trimmed).
    fn git_in(dir: &std::path::Path, args: &[&str]) -> (bool, String) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            // Deterministic identity for the SETUP-side `git init`/`git add`
            // (never for the committed-under-test operation, which the confined
            // child performs with the broker's own trusted `-c` identity).
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("failed to spawn system git");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        )
    }

    /// Create a temp workspace that IS a git repo with one staged file. Returns
    /// (repo_path, WorkspaceRoot). The repo lives AT the workspace root so the
    /// launcher's Landlock allow-rule (resolved from EXEC_WORKSPACE_ROOT) covers
    /// `.git`.
    fn setup_repo(tag: &str) -> (std::path::PathBuf, WorkspaceRoot) {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_gc_{tag}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(git_in(&root, &["init", "-q"]).0, "git init failed");
        std::fs::write(root.join("tracked.txt"), b"initial content\n").unwrap();
        assert!(git_in(&root, &["add", "tracked.txt"]).0, "git add failed");
        let ws = WorkspaceRoot::open(&root).unwrap();
        (root, ws)
    }

    /// Build a `git.commit` plan node whose `message` resolves to the given
    /// trusted literal, plus a seeded causal-root event. Returns
    /// (store, plan_node, conn, session_id, root_id, root_hash).
    fn setup_node(
        message: &str,
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

        // The Allowed path: a UserTrusted `message` (a tainted message would
        // Block in the executor — covered host-portably by Plan 36-01).
        let ev = Uuid::new_v4();
        let message_vid = store
            .mint(
                message.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![ev],
                None,
            )
            .unwrap();

        let plan_node = PlanNode {
            sink: SinkId("git.commit".into()),
            args: vec![PlanArg {
                name: "message".into(),
                value_id: message_vid,
            }],
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

    /// (1) genuine-commit + unbroken-audit-DAG-edge (ROADMAP criterion 2 / GIT-01).
    /// An Allowed git.commit with a UserTrusted message produces a REAL commit
    /// (HEAD resolves), appends a chained `process_exited` event, and — minted
    /// via the SAME `mint_from_exec` server.rs uses — the resulting ValueRecord's
    /// `provenance_chain[0]` EQUALS that event id (anti-staple: the taint is
    /// genuinely rooted on the spawn event, not re-minted clean). verify_chain
    /// stays intact.
    #[tokio::test]
    async fn git_commit_produces_real_commit_with_process_exited_rooted_taint() {
        let (repo, ws) = setup_repo("ok");
        let (mut store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup_node("genuine caprun commit");
        let effect_id = Uuid::new_v4();

        let (evt_id, hash, combined_output) = invoke_git_commit(
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
        .expect("git.commit must succeed via the confined launcher");
        assert!(!hash.is_empty());

        // A real commit now exists (HEAD resolves to a commit object).
        let (head_ok, head) = git_in(&repo, &["rev-parse", "--verify", "HEAD"]);
        assert!(head_ok && !head.is_empty(), "HEAD did not advance to a real commit");
        let (_ok, subject) = git_in(&repo, &["log", "-1", "--format=%s"]);
        assert_eq!(subject, "genuine caprun commit", "commit subject mismatch");

        // Anti-staple: mint via the SAME server.rs mint (mint_from_exec) rooted
        // on the returned process_exited event id — provenance_chain[0] MUST be
        // that exact event id, proving the taint edge is genuine, not stapled.
        let value_id = mint_from_exec(&mut store, session_id, combined_output, evt_id)
            .expect("mint_from_exec must succeed");
        let record = store.resolve(&value_id).expect("minted record must resolve");
        assert_eq!(
            record.provenance_chain[0], evt_id,
            "provenance_chain[0] must equal the process_exited event id (non-stapled)"
        );
        assert_eq!(
            record.taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw],
            "minted git output must carry the untrusted-origin exec taint pair"
        );

        let locked = conn.lock().unwrap();
        let evt = find_event_by_type(&locked, &session_id.to_string(), "process_exited")
            .unwrap()
            .expect("process_exited event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.parent_id, Some(parent_id));
        assert_eq!(evt.actor, format!("sink:git.commit:{effect_id}"));
        assert_eq!(
            evt.taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw]
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

        std::fs::remove_dir_all(&repo).ok();
    }

    /// (2) neutralization — a planted malicious pre-commit hook + config alias
    /// does NOT execute (ROADMAP criterion 3 / GIT-01, DESIGN §1.5, T-36-04).
    /// The `-c core.hooksPath=/dev/null` argv flag (highest git config
    /// precedence, overriding the repo-local hooksPath) makes the planted hook
    /// inert; the commit still succeeds. The sentinel lives INSIDE the workspace
    /// root, so a firing hook COULD write it (Landlock would not block it) — its
    /// absence therefore proves the hook did not fire, not that the write was
    /// merely denied.
    #[tokio::test]
    async fn planted_hook_and_alias_do_not_execute_and_commit_succeeds() {
        let (repo, ws) = setup_repo("hook");

        // Plant a malicious executable pre-commit hook that writes a sentinel
        // INSIDE the repo (a write the confined child is otherwise permitted).
        let hooks_dir = repo.join(".git/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("pre-commit");
        // Use pure shell redirection (`: > file`) rather than an external
        // `touch`/`git` — git runs hooks with cwd at the repo root, so this
        // creates the sentinel there using ONLY /bin/sh. If the hook fires at
        // all, the sentinel WILL appear (no dependency on another binary being
        // reachable under confinement) — so its ABSENCE unambiguously means the
        // hook never fired, never that a subordinate write was merely blocked.
        std::fs::write(&hook_path, "#!/bin/sh\n: > HOOK_FIRED_SENTINEL\nexit 0\n").unwrap();
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Plant a repo-local alias that shells out (never invoked — the broker
        // constructs the exact `commit` argv itself).
        assert!(
            git_in(&repo, &["config", "alias.evil", "!touch ALIAS_FIRED_SENTINEL"]).0,
            "planting the alias failed"
        );

        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup_node("commit despite planted hook");
        let effect_id = Uuid::new_v4();

        let (_evt_id, _hash, _out) = invoke_git_commit(
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
        .expect("git.commit must succeed even with a planted hook (hook must be inert)");

        assert!(
            !repo.join("HOOK_FIRED_SENTINEL").exists(),
            "the planted pre-commit hook FIRED — core.hooksPath=/dev/null neutralization failed (P2 RCE)"
        );
        assert!(
            !repo.join("ALIAS_FIRED_SENTINEL").exists(),
            "a planted alias executed — the broker did not construct the exact commit argv"
        );
        // The commit still succeeded (HEAD advanced) — neutralization did not
        // break the legitimate operation.
        let (head_ok, head) = git_in(&repo, &["rev-parse", "--verify", "HEAD"]);
        assert!(head_ok && !head.is_empty(), "commit did not succeed after hook neutralization");

        let locked = conn.lock().unwrap();
        assert!(verify_chain(&locked, &session_id.to_string(), TEST_KEY));
        drop(locked);

        std::fs::remove_dir_all(&repo).ok();
    }

    /// (3) exec-child env-clear (DESIGN §1.5, T-36-05). GIT_AUTHOR_NAME /
    /// GIT_COMMITTER_NAME set in the BROKER (this test process) env take
    /// precedence over `-c user.name` IF a child inherits them. Because
    /// `run_launcher` env_clear()s the child, the git commit's author/committer
    /// is the broker-supplied TRUSTED `caprun` identity, NOT the sentinel — a
    /// genuine, git-specific proof that the broker env does not leak into the
    /// confined child (adapts run_launcher_env_clear_prevents_broker_secret_leak).
    #[tokio::test]
    async fn exec_child_does_not_inherit_broker_env() {
        let (repo, ws) = setup_repo("envclear");

        // Sentinel identity in the broker (test) env. If it leaked into the git
        // child, GIT_AUTHOR_NAME would override -c user.name=caprun.
        let sentinel = "LEAKED_BROKER_IDENTITY_36GAP";
        std::env::set_var("GIT_AUTHOR_NAME", sentinel);
        std::env::set_var("GIT_COMMITTER_NAME", sentinel);

        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup_node("env-clear proof commit");
        let effect_id = Uuid::new_v4();

        let result = invoke_git_commit(
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

        std::env::remove_var("GIT_AUTHOR_NAME");
        std::env::remove_var("GIT_COMMITTER_NAME");
        let (_evt_id, _hash, _out) = result.expect("git.commit must succeed in the confined child");

        let (_ok, author) = git_in(&repo, &["log", "-1", "--format=%an"]);
        let (_ok2, committer) = git_in(&repo, &["log", "-1", "--format=%cn"]);
        assert_eq!(
            author, "caprun",
            "author was `{author}` — the git child inherited the broker GIT_AUTHOR_NAME (env_clear failed)"
        );
        assert_eq!(
            committer, "caprun",
            "committer was `{committer}` — the git child inherited the broker GIT_COMMITTER_NAME (env_clear failed)"
        );
        assert_ne!(author, sentinel, "broker identity leaked into the commit author");

        std::fs::remove_dir_all(&repo).ok();
    }
}

/// On non-Linux (this dev machine is a Mac), the tests above are compiled out
/// entirely. Expected, not a gap: the launcher's real confinement (and thus the
/// confined git spawn this file asserts) only applies on Linux. `cargo test -p
/// brokerd --no-run` still confirms this file COMPILES on Mac; the Linux
/// container step actually RUNS it (cfg-linux-test-blindness).
#[cfg(not(target_os = "linux"))]
mod not_linux {}
