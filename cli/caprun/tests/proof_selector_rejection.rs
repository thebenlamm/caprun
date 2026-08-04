use std::process::Command;

#[test]
fn proof_selector_is_rejected_before_effects() {
    let root = std::env::temp_dir().join(format!(
        "caprun_proof_selector_{}",
        uuid::Uuid::new_v4()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(workspace.join("src")).expect("create workspace");
    std::fs::write(workspace.join("src/hello.txt"), b"unchanged\n").expect("write target");
    let workspace_file = workspace.join("workspace.txt");
    std::fs::write(&workspace_file, b"marker\n").expect("write workspace marker");
    let intent = root.join("intent.json");
    std::fs::write(&intent, r#"{"kind":"SafeCodingWorkflow","path":"src/hello.txt","contents":"changed\n","test_command":"true","test_args_json":"[]","commit_message":"proof","remote":"origin","refspec":"HEAD:refs/heads/proof","owner":"o","repo":"r","base":"main","head":"proof","pr_title":"proof","pr_body":"proof"}"#)
        .expect("write intent");
    let policy = root.join("policy.json");
    std::fs::write(&policy, r#"{"allowed_sinks":["file.write","process.exec","git.commit","git.push","github.pr"],"arg_constraints":{}}"#)
        .expect("write policy");
    let audit = root.join("audit.db");
    let before = std::fs::read(workspace.join("src/hello.txt")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_caprun"))
        .args([
            "run", "--policy", policy.to_str().unwrap(), "safe-coding-workflow",
            intent.to_str().unwrap(), workspace_file.to_str().unwrap(), audit.to_str().unwrap(),
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CAPRUN_CODING_I2_PROOF", "1")
        .output()
        .expect("spawn ordinary caprun");
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("CAPRUN_CODING_I2_PROOF requires a caprun build with live-proof-fixtures"),
        "unexpected diagnostic: {text}"
    );
    assert!(!audit.exists(), "rejection must precede audit/session creation");
    assert!(!root.join("audit.db.key").exists(), "rejection must precede key creation");
    assert_eq!(std::fs::read(workspace.join("src/hello.txt")).unwrap(), before);
    std::fs::remove_dir_all(root).ok();
}
