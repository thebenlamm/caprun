//! exec_child_confinement — negative-assertion + positive-capability tests for
//! a broker-spawned `process.exec` confined child (Linux-only, EXEC-01/EXEC-04,
//! DESIGN-effect-breadth-exec.md §1.4/§7).
//!
//! Drives the REAL `caprun-exec-launcher` binary (32-03) — never a stub —
//! with `EXEC_*` env vars pointed at a temp `WorkspaceRoot`, exactly as the
//! broker's `process.exec` sink does (`crates/brokerd/src/sinks/process_exec.rs`).
//! The launcher self-confines (rlimits -> Landlock exec-child ruleset ->
//! seccomp exec-child filter) THEN self-replaces via `execve` into the target
//! command — this test proves that confinement holds in the TARGET's own
//! post-`execve` process image, without the target doing anything itself to
//! confine itself (the target here is a plain, unconfined `/usr/bin/python3`
//! probe script — proving the confinement is INHERITED via kernel semantics
//! across `execve`, not self-applied by the target).
//!
//! Per DESIGN §1.4, three properties are asserted:
//!   (a) a target that attempts to WRITE a path OUTSIDE the workspace root
//!       (and outside the system-path allow-list) is denied (Landlock).
//!   (b) a target that attempts an outbound `socket(AF_INET)` fails with
//!       EPERM — proving the seccomp net-deny PERSISTS across the launcher's
//!       OWN `execve` (A6, the single load-bearing kernel-semantics
//!       assumption this test empirically confirms — DESIGN §9 A5/A6,
//!       T-32-25).
//!   (c) a benign in-WorkspaceRoot write+read AND the legitimate target
//!       `execve` itself both SUCCEED — proving the ruleset is a
//!       narrow-allow, not a deny-all (contrast with `confinement_integration.rs`'s
//!       WORKER deny-all assertions).
//!
//! # Probe target: `/usr/bin/python3`
//!
//! The system-path allow-list (`/usr`, `/bin`, `/lib`, `/lib64`) grants
//! `ReadFile + Execute` — sufficient to load and run `/usr/bin/python3` and
//! its stdlib (empirically confirmed in the 32-06 Linux container: the
//! `rust:1` base image is Debian trixie, `/bin`/`/lib` are `usr-merge`
//! symlinks into `/usr`, and `/usr/bin/python3` is present). A plain Python
//! `-c` snippet gives unambiguous, distinguishable exit codes (0/1/3) without
//! any shell-quoting fragility or a dependency on `/dev/tcp` (bash-only,
//! unreliable across minimal images) or `nc` (not installed in `rust:1`).
//!
//! # Fresh-process-per-op (mirrors `confinement_integration.rs`)
//!
//! Each op spawns a fresh `caprun-exec-launcher` process (never a fork()
//! inside the multithreaded libtest process) — the same async-signal-safety
//! rationale `confinement_integration.rs`'s module doc comment gives.
//!
//! # Linux-only
//!
//! All bodies are `#[cfg(target_os = "linux")]` — Landlock/seccomp are
//! Linux-only; on macOS `cargo test -p sandbox` compiles this file and shows
//! 0 of these tests run (expected, not a gap, per project CLAUDE.md's
//! "Linux-only security tests" section). Run under Colima/Docker:
//!
//!   docker run --rm --security-opt seccomp=unconfined \
//!     -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
//!     rust:1 bash -c "cargo build --workspace && cargo test -p sandbox --test exec_child_confinement"

#[cfg(target_os = "linux")]
mod linux {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    /// Resolve the `caprun-exec-launcher` sibling binary's path relative to
    /// the CURRENTLY RUNNING process image.
    ///
    /// `caprun-exec-launcher` lives in a SEPARATE Cargo package
    /// (`cli/caprun-exec-launcher`) from `sandbox` — Cargo's
    /// `CARGO_BIN_EXE_<name>` environment variable is only set for binary
    /// targets WITHIN the package under test, so it does not resolve
    /// cross-package here (unlike `confine-probe`, a `[[bin]]` of THIS SAME
    /// `sandbox` package, which every other test in this directory correctly
    /// resolves via `env!("CARGO_BIN_EXE_confine-probe")`). This walks up a
    /// small, bounded number of ancestor directories from `current_exe()`'s
    /// parent — covering both a `cargo test` integration-test binary
    /// (`target/{debug,release}/deps/<name>-<hash>`, one level deeper) and,
    /// for parity, a directly-run binary (`target/{debug,release}/<name>`) —
    /// mirroring the identical fix applied to
    /// `crates/brokerd/src/sinks/process_exec.rs::resolve_launcher_path` in
    /// this same plan (32-06), after that exact resolution gap was
    /// empirically caught by the mandatory Linux container run.
    fn resolve_launcher_path() -> PathBuf {
        let current_exe = std::env::current_exe().expect("resolve current_exe");
        let mut dir = current_exe.parent().map(|p| p.to_path_buf());
        for _ in 0..3 {
            let Some(candidate_dir) = dir else { break };
            let candidate = candidate_dir.join("caprun-exec-launcher");
            if candidate.is_file() {
                return candidate;
            }
            dir = candidate_dir.parent().map(|p| p.to_path_buf());
        }
        panic!(
            "could not locate sibling binary `caprun-exec-launcher` near current_exe() \
             {current_exe:?} (checked current_exe()'s parent and up to 2 ancestor \
             directories) — run `cargo build --workspace` first \
             (cargo-test-workspace-missing-sibling-binary)"
        );
    }

    /// A fresh, real temp workspace directory for `EXEC_WORKSPACE_ROOT`.
    fn fresh_workspace(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "caprun-exec-child-confinement-{tag}-{}-{}",
            std::process::id(),
            uuid_like_suffix(),
        ));
        std::fs::create_dir_all(&dir).expect("create workspace dir");
        dir
    }

    /// A short, unique-enough suffix without pulling in a `uuid` dev-dependency
    /// — this crate has none today; a nanosecond timestamp is sufficient
    /// entropy for a same-process, same-run temp-directory name collision
    /// avoidance (not a security-relevant identifier).
    fn uuid_like_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos()
    }

    /// Spawn `caprun-exec-launcher` with `EXEC_COMMAND=/usr/bin/python3`,
    /// `EXEC_ARGS_JSON=["-c", <python_code>]`, and `EXEC_WORKSPACE_ROOT=
    /// workspace_root`. Returns the process `Output` — since the launcher's
    /// `execve` REPLACES its own process image (same PID), this `Output`'s
    /// exit status and captured stdout/stderr are the TARGET python3
    /// process's own, post-`execve` (fd inheritance across `execve` is
    /// standard POSIX semantics, mirrored in `caprun-exec-launcher/src/main.rs`'s
    /// doc comment).
    fn run_exec_child(workspace_root: &Path, python_code: &str) -> Output {
        let launcher = resolve_launcher_path();
        let args_json = serde_json::to_string(&vec!["-c".to_string(), python_code.to_string()])
            .expect("serialize EXEC_ARGS_JSON");
        Command::new(&launcher)
            .env("EXEC_COMMAND", "/usr/bin/python3")
            .env("EXEC_ARGS_JSON", args_json)
            .env(
                "EXEC_WORKSPACE_ROOT",
                workspace_root.to_str().expect("workspace_root is valid UTF-8"),
            )
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn caprun-exec-launcher: {e}"))
    }

    /// (a) fs-escape denied: a target that attempts to WRITE a path OUTSIDE
    /// the workspace root (and outside the system-path allow-list) fails —
    /// Landlock denies it, the file is never created.
    #[test]
    fn exec_child_fs_escape_denied() {
        let ws = fresh_workspace("fs-escape");
        let escape_target = std::env::temp_dir().join(format!(
            "caprun_exec_escape_probe_{}",
            uuid_like_suffix()
        ));
        // Pre-condition: the escape target must not already exist (so its
        // absence afterward is meaningful, not a stale leftover).
        let _ = std::fs::remove_file(&escape_target);

        let python_code = format!(
            "import sys\n\
             try:\n\
             \x20   open({path:?}, 'w').write('pwned')\n\
             \x20   sys.exit(1)  # UNEXPECTED: write outside the workspace succeeded\n\
             except PermissionError:\n\
             \x20   sys.exit(0)  # EXPECTED: Landlock denied the write\n\
             except OSError:\n\
             \x20   sys.exit(3)  # unexpected other errno\n",
            path = escape_target.to_str().expect("valid UTF-8 path")
        );

        let output = run_exec_child(&ws, &python_code);
        eprintln!(
            "[exec_child_fs_escape_denied] status={:?} stdout={:?} stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0 (write outside the workspace correctly DENIED by Landlock); \
             got {:?}. Exit codes: 0=denied(expected), 1=succeeded(confinement FAILURE), \
             3=unexpected-error",
            output.status.code()
        );
        assert!(
            !escape_target.exists(),
            "the escape-target file must NEVER be created on disk (no effect on the \
             denied path)"
        );

        std::fs::remove_dir_all(&ws).ok();
    }

    /// (b) outbound socket denied — A6 empirically confirmed: the seccomp
    /// net-deny (`socket(AF_INET, ...)` -> EPERM) PERSISTS across the
    /// launcher's OWN `execve` into the target. The target here does NOTHING
    /// to confine itself — this proves inheritance via kernel semantics, not
    /// self-application (contrast with `confine-probe`, which self-applies
    /// `sandbox::apply_confinement()` before probing — see
    /// `confinement_integration.rs`).
    #[test]
    fn exec_child_net_deny_persists_across_execve() {
        let ws = fresh_workspace("net-deny");
        let python_code = "import socket, sys\n\
             try:\n\
             \x20   socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n\
             \x20   sys.exit(1)  # UNEXPECTED: socket() succeeded — net-deny did NOT persist\n\
             except PermissionError:\n\
             \x20   sys.exit(0)  # EXPECTED: seccomp denied socket() with EPERM (A6 confirmed)\n\
             except OSError:\n\
             \x20   sys.exit(3)  # unexpected other errno\n"
            .to_string();

        let output = run_exec_child(&ws, &python_code);
        eprintln!(
            "[exec_child_net_deny_persists_across_execve] status={:?} stdout={:?} stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0 (socket(AF_INET) correctly DENIED with EPERM, proving A6 — \
             the net-deny PERSISTS across the launcher's own execve into an unconfined \
             target); got {:?}. Exit codes: 0=denied(expected, A6 confirmed), \
             1=succeeded(regression), 3=unexpected-error",
            output.status.code()
        );

        std::fs::remove_dir_all(&ws).ok();
    }

    /// (c) narrow-allow, not deny-all: a benign in-WorkspaceRoot write+read
    /// AND the legitimate target `execve` itself both SUCCEED — the launcher
    /// correctly loaded and ran `/usr/bin/python3` (Landlock `ReadFile +
    /// Execute` on the system-path allow-list), and the target could write
    /// (and read back) a file under the workspace root (Landlock `ReadFile +
    /// WriteFile` there).
    #[test]
    fn exec_child_benign_workspace_write_and_legitimate_execve_succeed() {
        let ws = fresh_workspace("benign");
        let target_file = ws.join("caprun_exec_child_probe.txt");
        let python_code = format!(
            "import sys\n\
             p = {path:?}\n\
             with open(p, 'w') as f:\n\
             \x20   f.write('confined-child-benign-write-ok')\n\
             with open(p) as f:\n\
             \x20   content = f.read()\n\
             sys.exit(0 if content == 'confined-child-benign-write-ok' else 1)\n",
            path = target_file.to_str().expect("valid UTF-8 path")
        );

        let output = run_exec_child(&ws, &python_code);
        eprintln!(
            "[exec_child_benign_workspace_write_and_legitimate_execve_succeed] status={:?} \
             stdout={:?} stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "the legitimate execve of /usr/bin/python3 AND a benign in-workspace \
             write+read must BOTH succeed (narrow-allow ruleset, not deny-all); got {:?}",
            output.status.code()
        );
        assert!(
            target_file.exists(),
            "the benign in-workspace file must exist on disk after a successful write"
        );
        let on_disk = std::fs::read_to_string(&target_file).expect("read created file");
        assert_eq!(
            on_disk, "confined-child-benign-write-ok",
            "the on-disk content must match what the confined child wrote"
        );

        std::fs::remove_dir_all(&ws).ok();
    }
}

/// Cross-platform guard: keeps `cargo test -p sandbox` meaningful on the
/// macOS dev box (where the Linux bodies above are cfg-excluded, 0 tests
/// reported — expected, not a gap). Confirms `confine-probe` (this crate's
/// OWN `[[bin]]`) is wired into the test build, mirroring
/// `confinement_integration.rs`'s existing precedent.
#[test]
fn exec_child_confinement_guard_binary_present() {
    let probe_bin = env!("CARGO_BIN_EXE_confine-probe");
    assert!(
        !probe_bin.is_empty(),
        "CARGO_BIN_EXE_confine-probe must resolve — sandbox's bins must be built for \
         the live tests in this crate"
    );
}
