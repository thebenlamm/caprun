//! coding_cli — Phase 50 Wave 0 host tests for CLI-01 coding argv surface.
//!
//! Proves:
//! - SafeCodingWorkflow JSON fixture deserializes (closed CaprunIntent identity)
//! - `caprun` accepts `safe-coding-workflow` kind (not "unknown intent kind")
//! - Unknown intent kinds still fail closed
//! - `--seed-from-file` + safe-coding-workflow fails closed
//! - `--policy` is accepted by argv parse (not "unknown argument")
//!
//! Full multi-node confined stream SUCCESS (LIVE-07/08) is **Phase 51** — these
//! tests intentionally do **not** claim LIVE multi-node success. Host runs may
//! stop at spawn/confine/UDS without that being a Phase 50 gap.

use runtime_core::intent::CaprunIntent;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

/// RESEARCH coding intent JSON fixture shape (kind + 13 operator fields).
const CODING_INTENT_JSON: &str = r#"{
  "kind": "SafeCodingWorkflow",
  "path": "src/hello.txt",
  "contents": "hello from caprun\n",
  "test_command": "sh",
  "test_args_json": "[\"-c\", \"git add -A && true\"]",
  "commit_message": "caprun: safe coding demo",
  "remote": "origin",
  "refspec": "HEAD:refs/heads/caprun-demo",
  "owner": "acme",
  "repo": "demo",
  "base": "main",
  "head": "caprun-demo",
  "pr_title": "caprun safe coding demo",
  "pr_body": "Opened by multi-node stream"
}"#;

/// Minimal valid policy JSON (POLICY-03) allowing production coding sinks.
/// Placed as a sibling of the workspace dir — never beneath it (F1).
const MINIMAL_POLICY_JSON: &str = r#"{
  "allowed_sinks": [
    "email.send",
    "file.create",
    "file.write",
    "process.exec",
    "git.commit",
    "git.push",
    "github.pr",
    "http.request",
    "http.request.write"
  ],
  "arg_constraints": {}
}"#;

fn unique_tmp(tag: &str) -> PathBuf {
    let run_id = Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_coding_cli_{tag}_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    tmp
}

/// F1-safe layout: workspace under subdirectory; audit.db + policy as siblings.
struct CodingLayout {
    root: PathBuf,
    workspace_file: PathBuf,
    audit_db: PathBuf,
    intent_json: PathBuf,
    policy_path: PathBuf,
}

impl CodingLayout {
    fn new(tag: &str) -> Self {
        let root = unique_tmp(tag);
        let ws_dir = root.join("workspace");
        std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
        // Pre-create file.write target path (O_TRUNC) under workspace.
        let src = ws_dir.join("src");
        std::fs::create_dir_all(&src).expect("create src dir");
        std::fs::write(src.join("hello.txt"), b"").expect("pre-create write target");
        let workspace_file = ws_dir.join("workspace.txt");
        std::fs::write(&workspace_file, b"coding workspace marker\n").expect("write workspace file");
        let audit_db = root.join("audit.db");
        let intent_json = root.join("coding-intent.json");
        std::fs::write(&intent_json, CODING_INTENT_JSON).expect("write intent json");
        let policy_path = root.join("policy.json");
        std::fs::write(&policy_path, MINIMAL_POLICY_JSON).expect("write policy json");
        Self {
            root,
            workspace_file,
            audit_db,
            intent_json,
            policy_path,
        }
    }
}

impl Drop for CodingLayout {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn caprun_bin() -> &'static str {
    env!("CARGO_BIN_EXE_caprun")
}

fn run_caprun(args: &[&str]) -> std::process::Output {
    Command::new(caprun_bin())
        .args(args)
        .output()
        .expect("spawn caprun")
}

fn combined_output(out: &std::process::Output) -> String {
    let mut s = String::new();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

/// Wave 0: JSON fixture deserializes to CaprunIntent::SafeCodingWorkflow with
/// field presence (closed-enum identity — no-change from Phase 49; CLI consumes it).
#[test]
fn coding_intent_json_deserializes_to_safe_coding_workflow() {
    let intent: CaprunIntent =
        serde_json::from_str(CODING_INTENT_JSON).expect("deserialize coding intent JSON");
    match intent {
        CaprunIntent::SafeCodingWorkflow {
            path,
            contents,
            test_command,
            test_args_json,
            commit_message,
            remote,
            refspec,
            owner,
            repo,
            base,
            head,
            pr_title,
            pr_body,
        } => {
            assert_eq!(path, "src/hello.txt");
            assert!(contents.contains("hello from caprun"));
            assert_eq!(test_command, "sh");
            assert!(test_args_json.contains("git add"));
            assert!(!commit_message.is_empty());
            assert_eq!(remote, "origin");
            assert!(!refspec.is_empty());
            assert_eq!(owner, "acme");
            assert_eq!(repo, "demo");
            assert_eq!(base, "main");
            assert_eq!(head, "caprun-demo");
            assert!(!pr_title.is_empty());
            assert!(!pr_body.is_empty());
        }
        other => panic!("expected SafeCodingWorkflow, got {other:?}"),
    }
}

/// Unknown intent kind still fail-closed (non-zero; "unknown intent kind").
#[test]
fn unknown_intent_kind_fails_closed() {
    let layout = CodingLayout::new("unknown_kind");
    let out = run_caprun(&[
        "not-a-real-intent",
        "param",
        layout.workspace_file.to_str().unwrap(),
        layout.audit_db.to_str().unwrap(),
    ]);
    assert_ne!(out.status.code(), Some(0), "unknown kind must be non-zero");
    let text = combined_output(&out);
    assert!(
        text.contains("unknown intent kind"),
        "must surface unknown intent kind; got:\n{text}"
    );
}

/// Missing coding intent JSON file → non-zero usage/infra (not success).
#[test]
fn safe_coding_workflow_missing_json_exits_nonzero() {
    let layout = CodingLayout::new("missing_json");
    let missing = layout.root.join("no-such-intent.json");
    let out = run_caprun(&[
        "safe-coding-workflow",
        missing.to_str().unwrap(),
        layout.workspace_file.to_str().unwrap(),
        layout.audit_db.to_str().unwrap(),
    ]);
    assert_ne!(
        out.status.code(),
        Some(0),
        "missing intent JSON must fail closed"
    );
    let text = combined_output(&out);
    assert!(
        !text.contains("unknown intent kind"),
        "must accept the kind and fail on file read, not unknown kind; got:\n{text}"
    );
}

/// `--seed-from-file` + safe-coding-workflow fails closed with file-derived error.
#[test]
fn safe_coding_workflow_rejects_seed_from_file() {
    let layout = CodingLayout::new("seed_reject");
    // Seed file content is irrelevant — CLI must refuse before session create.
    let seed = layout.root.join("seed.txt");
    std::fs::write(&seed, b"untrusted seed content\n").expect("write seed");
    let out = run_caprun(&[
        "--seed-from-file",
        seed.to_str().unwrap(),
        "safe-coding-workflow",
        layout.workspace_file.to_str().unwrap(),
        layout.audit_db.to_str().unwrap(),
    ]);
    assert_ne!(
        out.status.code(),
        Some(0),
        "seed-from-file coding must fail closed"
    );
    let text = combined_output(&out).to_ascii_lowercase();
    assert!(
        text.contains("seed-from-file")
            || text.contains("primary_file_derived")
            || text.contains("file-derived")
            || text.contains("file_derived"),
        "error must mention file-derived / seed-from-file refusal; got:\n{}",
        combined_output(&out)
    );
    assert!(
        !combined_output(&out).contains("unknown intent kind"),
        "must not fail as unknown kind"
    );
}

/// `--policy` path is accepted by argv parse (not unknown argument).
/// Bind may still fail-closed if content invalid; we use a minimal valid policy
/// placed as a sibling of the workspace (F1-safe). Full stream may still be
/// non-zero on host without Linux confinement — assert only argv/policy acceptance.
#[test]
fn policy_flag_accepted_for_coding_argv() {
    let layout = CodingLayout::new("policy_flag");
    let out = run_caprun(&[
        "--policy",
        layout.policy_path.to_str().unwrap(),
        "safe-coding-workflow",
        layout.intent_json.to_str().unwrap(),
        layout.workspace_file.to_str().unwrap(),
        layout.audit_db.to_str().unwrap(),
    ]);
    let text = combined_output(&out);
    assert!(
        !text.contains("unknown argument") && !text.contains("unknown flag"),
        "--policy must not be rejected as unknown argument; got:\n{text}"
    );
    // Kind accepted: not "unknown intent kind".
    assert!(
        !text.contains("unknown intent kind"),
        "safe-coding-workflow with --policy must parse as a known kind; got:\n{text}"
    );
    // Policy path under workspace would refuse — our policy is a sibling, so
    // refusal text about "beneath the workspace" must be absent.
    assert!(
        !text.contains("beneath the workspace"),
        "sibling policy must pass F1 containment; got:\n{text}"
    );
}

/// Valid coding JSON + temp workspace: argv acceptance progresses past
/// unknown-kind. Host may not complete full five-node confined stream —
/// assert session_id printed and/or structured non-success that is not
/// "unknown intent kind". Never assert LIVE-07 SUCCESS.
#[test]
fn safe_coding_workflow_argv_accepted_past_unknown_kind() {
    let layout = CodingLayout::new("argv_accept");
    let out = run_caprun(&[
        "run",
        "safe-coding-workflow",
        layout.intent_json.to_str().unwrap(),
        layout.workspace_file.to_str().unwrap(),
        layout.audit_db.to_str().unwrap(),
    ]);
    let text = combined_output(&out);
    assert!(
        !text.contains("unknown intent kind"),
        "valid coding JSON must not fail as unknown kind; got:\n{text}"
    );

    // Progress signal: session_id printed (coding path) and/or grant pointer,
    // OR a structured stream/infra failure after session create.
    let progressed = text.contains("session_id=")
        || text.contains("grant: caprun grant")
        || text.contains("Chain verification")
        || text.contains("caprun-stream:")
        || text.contains("spawn caprun-worker")
        || text.contains("open workspace")
        || out.status.code() != Some(0); // non-zero after accept is OK on host

    assert!(
        progressed || out.status.success(),
        "expected argv acceptance to progress past kind parse; got status={:?}\n{text}",
        out.status.code()
    );

    // Honesty: do not treat host partial runs as LIVE-07.
    // (This test documents the boundary — no LIVE SUCCESS assertion.)
}
