//! LIVE-07/08 non-hybrid CLI multi-node acceptance proof.
//!
//! SUCCESS here is driven by the real `caprun run safe-coding-workflow` CLI as
//! one Session. The v1.9 hybrid, in-crate multi-leg
//! `evaluate_plan_node_and_record_for_test` composition remains useful regression
//! coverage, but is not this DONE claim.
//!
//! Authoritative Linux recipe (the workspace build is required so the sibling
//! worker and exec-launcher binaries resolve beside `caprun`):
//!
//! ```text
//! COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca' bash scripts/compose-verify.sh
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

const LIVE_07_DRIVER: &str = "caprun run safe-coding-workflow (CLI multi-node, one Session)";
const LIVE_07_NOT: &str = "hybrid in-crate evaluate_plan_node_and_record_for_test composition";
const LIVE_08_DRIVER: &str =
    "caprun run safe-coding-workflow (CLI sibling Session, CAPRUN_CODING_I2_PROOF=1)";

#[test]
fn live_acceptance_v1_10_cli_guard_present() {
    assert!(!env!("CARGO_BIN_EXE_caprun").is_empty());
    assert!(!LIVE_07_NOT.is_empty());
}

#[cfg(all(
    target_os = "linux",
    feature = "mock-egress-ca",
    feature = "live-proof-fixtures"
))]
mod linux {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::process::{Child, Stdio};
    use std::thread;
    use std::time::Duration;
    use runtime_core::event::Event;

    const POLICY_JSON: &str = r#"{
      "allowed_sinks": ["email.send", "file.create", "file.write", "process.exec",
        "git.commit", "git.push", "github.pr", "http.request", "http.request.write"],
      "arg_constraints": {}
    }"#;

    const INTENT_JSON: &str = r#"{
      "kind": "SafeCodingWorkflow",
      "path": "src/hello.txt",
      "contents": "hello from LIVE-07 CLI multi-node proof\n",
      "test_command": "sh",
      "test_args_json": "[\"-c\",\"git add -A && true\"]",
      "commit_message": "caprun: LIVE-07 safe coding proof",
      "remote": "https://github-mock.caprun.test/accept/repo.git",
      "refspec": "HEAD:refs/heads/caprun-live-07",
      "owner": "caprun",
      "repo": "live-07",
      "base": "main",
      "head": "caprun-live-07",
      "pr_title": "LIVE-07 CLI multi-node proof",
      "pr_body": "Opened by one caprun Session"
    }"#;

    struct LiveCodingLayout {
        root: PathBuf,
        workspace_file: PathBuf,
        audit_db: PathBuf,
        intent_json: PathBuf,
        policy_path: PathBuf,
    }

    impl LiveCodingLayout {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("caprun_live_07_{}", Uuid::new_v4()));
            let workspace = root.join("workspace");
            std::fs::create_dir_all(workspace.join("src")).expect("create LIVE-07 workspace");
            std::fs::write(workspace.join("src/hello.txt"), b"before LIVE-07\n")
                .expect("pre-create O_TRUNC write target");
            let workspace_file = workspace.join("workspace.txt");
            std::fs::write(&workspace_file, b"LIVE-07 workspace marker\n")
                .expect("write workspace argv file");
            let audit_db = root.join("audit.db");
            let intent_json = root.join("coding-intent.json");
            let policy_path = root.join("policy.json");
            std::fs::write(&intent_json, INTENT_JSON).expect("write coding intent");
            std::fs::write(&policy_path, POLICY_JSON).expect("write trusted policy");
            git(&workspace, &["init", "-q"]);
            git(&workspace, &["config", "user.name", "caprun-live"]);
            git(&workspace, &["config", "user.email", "live@caprun.test"]);
            git(&workspace, &["add", "-A"]);
            git(&workspace, &["commit", "-q", "-m", "initial LIVE-07 fixture"]);
            Self {
                root,
                workspace_file,
                audit_db,
                intent_json,
                policy_path,
            }
        }
    }

    impl Drop for LiveCodingLayout {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("spawn fixture git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn caprun_command(proof_selector: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_caprun"));
        command.env_clear().env("PATH", "/usr/bin:/bin");
        for key in [
            "CAPRUN_SMTP_HOST",
            "CAPRUN_SMTP_PORT",
            "CAPRUN_GITHUB_API_BASE",
            "HTTPS_PROXY",
            "NO_PROXY",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        if proof_selector {
            command.env("CAPRUN_CODING_I2_PROOF", "1");
        }
        command
    }

    fn caprun(args: &[&str]) -> std::process::Output {
        caprun_command(false)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("spawn caprun {args:?}: {error}"))
    }

    fn run_sidecar_command(args: &[&str], action: &str) -> Result<(), String> {
        let output = caprun(args);
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{action} failed (status {:?}): stdout={} stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    fn grant_with_startup_retry(session_id: &str, audit_db: &str) -> Result<(), String> {
        let mut last = String::new();
        for _ in 0..25 {
            match run_sidecar_command(&["grant", session_id, audit_db], "github.pr grant") {
                Ok(()) => return Ok(()),
                Err(error) => last = error,
            }
            thread::sleep(Duration::from_millis(40));
        }
        Err(last)
    }

    /// Concurrent human-action simulation. It grants the surfaced coding Session
    /// exactly once, then confirms only the pending `git.push` effect. It never
    /// starts a second Session or invokes `run` itself.
    fn drive_external_confirm_and_grant(
        child: &mut Child,
        audit_db: PathBuf,
    ) -> thread::JoinHandle<Result<String, String>> {
        let stdout = child.stdout.take().expect("caprun run stdout must be piped");
        thread::spawn(move || {
            let audit = audit_db.to_string_lossy().into_owned();
            let mut transcript = String::new();
            let mut granted = false;
            let mut confirmed = false;
            for line in BufReader::new(stdout).lines() {
                let line = line.map_err(|error| format!("read caprun stdout: {error}"))?;
                transcript.push_str(&line);
                transcript.push('\n');
                if let Some(session_id) = line.trim().strip_prefix("session_id=") {
                    if granted {
                        return Err("more than one session_id surfaced by coding run".into());
                    }
                    grant_with_startup_retry(session_id, &audit)?;
                    granted = true;
                }
                if line.contains("sink=git.push") {
                    let effect_id = line
                        .split_whitespace()
                        .find_map(|field| field.strip_prefix("effect_id="))
                        .ok_or_else(|| format!("git.push pending line lacked effect_id: {line}"))?;
                    if confirmed {
                        return Err("more than one git.push confirmation requested".into());
                    }
                    run_sidecar_command(&["confirm", effect_id, &audit], "git.push confirm")?;
                    confirmed = true;
                }
            }
            if !granted || !confirmed {
                return Err(format!(
                    "sidecar incomplete: granted={granted} confirmed={confirmed}\n{transcript}"
                ));
            }
            Ok(transcript)
        })
    }

    fn count_events(db: &Path, session_id: &str, event_type: &str) -> i64 {
        let conn = brokerd::audit::open_audit_db(db.to_str().expect("audit DB path is UTF-8"))
            .expect("open LIVE-07 audit DB");
        conn.query_row(
            "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2",
            rusqlite::params![session_id, event_type],
            |row| row.get(0),
        )
        .expect("count durable LIVE-07 events")
    }

    fn events_of_type(db: &Path, session_id: &str, event_type: &str) -> Vec<Event> {
        let conn = brokerd::audit::open_audit_db(db.to_str().expect("audit DB path is UTF-8"))
            .expect("open LIVE audit DB");
        let mut statement = conn
            .prepare("SELECT payload FROM events WHERE session_id = ?1 AND event_type = ?2")
            .expect("prepare event payload query");
        statement
            .query_map(rusqlite::params![session_id, event_type], |row| row.get::<_, String>(0))
            .expect("query event payloads")
            .map(|row| {
                serde_json::from_str(&row.expect("read event payload"))
                    .expect("deserialize durable Event payload")
            })
            .collect()
    }

    fn assert_audit_passed(session_id: &str, audit_db: &Path) {
        let output = caprun(&["audit", session_id, audit_db.to_str().unwrap()]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "caprun audit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("Chain verification: PASSED"),
            "audit did not verify chain:\n{stdout}"
        );
    }

    #[test]
    fn live_07_cli_multi_node_one_session_verify_chain() {
        assert!(!LIVE_07_DRIVER.is_empty(), "LIVE-07 driver framing pin");
        let layout = LiveCodingLayout::new();
        let mut command = caprun_command(false);
        command
            .args([
                "run",
                "--policy",
                layout.policy_path.to_str().unwrap(),
                "safe-coding-workflow",
                layout.intent_json.to_str().unwrap(),
                layout.workspace_file.to_str().unwrap(),
                layout.audit_db.to_str().unwrap(),
            ])
            .env("CAPRUN_CONFIRM", "external")
            .env("CAPRUN_CONFIRM_TIMEOUT_SECS", "60")
            .env("CAPRUN_GIT_PUSH_TOKEN", "live-07-opaque-test-push-token")
            .env("CAPRUN_GITHUB_TOKEN", "live-07-opaque-test-github-token")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Bind the spawned Child. Configuring the Command and discarding
        // spawn()'s result would orphan the process and leave `child` a Command.
        let mut child = command.spawn().expect("spawn real caprun LIVE-07 driver");

        let stderr = child.stderr.take().expect("caprun run stderr must be piped");
        let stderr_reader = thread::spawn(move || {
            let mut text = String::new();
            BufReader::new(stderr)
                .read_to_string(&mut text)
                .expect("read caprun stderr");
            text
        });
        let sidecar = drive_external_confirm_and_grant(&mut child, layout.audit_db.clone());
        let status = child.wait().expect("wait for caprun run");
        let sidecar_result = sidecar.join();
        let stderr = stderr_reader.join().unwrap_or_else(|_| {
            "<stderr reader panicked>".to_string()
        });
        let stdout = match sidecar_result {
            Ok(Ok(stdout)) => stdout,
            Ok(Err(error)) => panic!("sidecar failed: {error}\nstderr:\n{stderr}"),
            Err(_) => panic!("sidecar thread panicked\nstderr:\n{stderr}"),
        };
        assert_eq!(
            status.code(),
            Some(0),
            "LIVE-07 driver failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );

        let session_ids: Vec<&str> = stdout
            .lines()
            .filter_map(|line| line.trim().strip_prefix("session_id="))
            .collect();
        assert_eq!(
            session_ids.len(),
            1,
            "exactly one coding Session must own the chain: {stdout}"
        );
        let session_id = session_ids[0];
        assert_audit_passed(session_id, &layout.audit_db);
        assert_eq!(
            count_events(&layout.audit_db, session_id, "git_push_succeeded"),
            1
        );
        assert_eq!(
            count_events(&layout.audit_db, session_id, "github_pr_succeeded"),
            1
        );
    }

    /// LIVE-08 is a sibling real-CLI run, not a hybrid composition. The
    /// policy explicitly permits github.pr; the genuine process.exec output
    /// reaches its body via product bag out_1 and must trip I2 sink_blocked.
    #[test]
    fn live_08_cli_mid_loop_i2_block_genuine_taint() {
        assert!(!LIVE_08_DRIVER.is_empty(), "LIVE-08 driver framing pin");
        assert!(POLICY_JSON.contains("\"github.pr\""));
        let layout = LiveCodingLayout::new();
        let mut command = caprun_command(true);
        command
            .args([
                "run",
                "--policy",
                layout.policy_path.to_str().unwrap(),
                "safe-coding-workflow",
                layout.intent_json.to_str().unwrap(),
                layout.workspace_file.to_str().unwrap(),
                layout.audit_db.to_str().unwrap(),
            ])
            .env("CAPRUN_CONFIRM", "external")
            .env("CAPRUN_CONFIRM_TIMEOUT_SECS", "60")
            .env("CAPRUN_GIT_PUSH_TOKEN", "live-08-opaque-test-push-token")
            .env("CAPRUN_GITHUB_TOKEN", "live-08-opaque-test-github-token")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Bind the spawned Child (see LIVE-07 note).
        let mut child = command.spawn().expect("spawn real caprun LIVE-08 driver");

        let stderr = child.stderr.take().expect("caprun run stderr must be piped");
        let stderr_reader = thread::spawn(move || {
            let mut text = String::new();
            BufReader::new(stderr)
                .read_to_string(&mut text)
                .expect("read caprun stderr");
            text
        });
        // This sidecar grants github.pr and confirms git.push only. It never
        // confirms the I2-blocked PR node, preserving the no-effect claim.
        let sidecar = drive_external_confirm_and_grant(&mut child, layout.audit_db.clone());
        let status = child.wait().expect("wait for caprun run");
        let sidecar_result = sidecar.join();
        let stderr = stderr_reader.join().unwrap_or_else(|_| {
            "<stderr reader panicked>".to_string()
        });
        let stdout = match sidecar_result {
            Ok(Ok(stdout)) => stdout,
            Ok(Err(error)) => panic!("sidecar failed: {error}\nstderr:\n{stderr}"),
            Err(_) => panic!("sidecar thread panicked\nstderr:\n{stderr}"),
        };
        assert!(
            matches!(status.code(), Some(2 | 3)),
            "LIVE-08 must stop denied/blocked, never success: {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            status.code()
        );
        assert!(!stdout.contains("DENIED code=policy_deny"));

        // The durable anchor assertions below are the attribution oracle; never
        // substitute a driver-stdout substring for this persisted provenance.
        let session_ids: Vec<&str> = stdout
            .lines()
            .filter_map(|line| line.trim().strip_prefix("session_id="))
            .collect();
        assert_eq!(session_ids.len(), 1, "LIVE-08 must use one sibling Session");
        let session_id = session_ids[0];
        assert_audit_passed(session_id, &layout.audit_db);
        assert_eq!(count_events(&layout.audit_db, session_id, "process_exited"), 1);
        assert_eq!(count_events(&layout.audit_db, session_id, "github_pr_succeeded"), 0);
        let process_events = events_of_type(&layout.audit_db, session_id, "process_exited");
        assert_eq!(process_events.len(), 1, "unique genuine process root required");
        let process_event_id = process_events[0].id;
        let matching: Vec<_> = events_of_type(&layout.audit_db, session_id, "sink_blocked")
            .into_iter()
            .flat_map(|event| event.anchors)
            .filter(|anchor| anchor.sink.0 == "github.pr" && anchor.arg == "body")
            .collect();
        assert_eq!(matching.len(), 1, "exactly one durable github.pr/body anchor required");
        let anchor = &matching[0];
        assert!(!anchor.taint.is_empty(), "blocked body must carry taint");
        assert!(anchor.taint.iter().any(|label| label.is_untrusted()));
        assert_eq!(anchor.provenance_chain.first(), Some(&process_event_id));
        assert_eq!(anchor.read_event_id, process_event_id);
        assert_eq!(anchor.read_event_id, anchor.provenance_chain[0]);
    }
}
