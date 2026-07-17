//! `caprun-exec-launcher` — DESIGN §1.3 Option B.
//!
//! A dedicated helper binary, spawned UNCONFINED by the broker
//! (`crates/brokerd/src/sinks/process_exec.rs`, Plan 32-04), which reads its
//! target command from env vars, applies kernel confinement to ITSELF (the
//! exec-child variants of the primitives `sandbox::apply_confinement()`
//! already applies to the worker, in the SAME mandatory order: rlimits ->
//! Landlock -> seccomp), and THEN self-replaces via `execve` into the target
//! command.
//!
//! # Why this exists (DESIGN §1.2/§1.3)
//!
//! The confined worker's own seccomp filter denies `execve`/`execveat`
//! unconditionally, so it can never run an external command itself. An
//! arbitrary target binary also has no IPC handshake with the broker to
//! self-confine after (unlike the worker). The only way to kernel-confine an
//! arbitrary child is to apply confinement in the launcher's OWN address
//! space, post-fork, BEFORE the target's `execve` — mirroring the proven
//! `apply_confinement()` self-confinement ordering, never inside a
//! `Command::pre_exec` closure (Option A, retired — DESIGN §2.5/§9: heap
//! allocation inside `landlock`/`seccompiler` setup is not async-signal-safe
//! between `fork()` and `execve()`).
//!
//! # Fail-closed contract
//!
//! Every confinement step below `.expect()`s on failure, aborting the process
//! (non-zero exit) BEFORE any `execve` of the target ever runs. No target ever
//! runs unconfined (DESIGN §5, §6 "Kernel-confined exec child").
//!
//! # argv discipline (DESIGN §1.5, Pitfall 7)
//!
//! `command` is executed directly — NEVER through `sh -c` or any shell
//! interpreter. `args` is JSON-decoded into `Vec<String>` and each element is
//! passed as a distinct `execve` argv slot via `Command::args`, never
//! shell-joined into one string.

use std::os::unix::process::CommandExt;
use std::path::Path;

fn main() -> ! {
    // --- Read EXEC_* env vars (DESIGN §1.5 arg schema) ---

    let command = std::env::var("EXEC_COMMAND").unwrap_or_else(|_| {
        eprintln!("[caprun-exec-launcher] EXEC_COMMAND env var required");
        std::process::exit(2);
    });

    let args: Vec<String> = match std::env::var("EXEC_ARGS_JSON") {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
            eprintln!("[caprun-exec-launcher] EXEC_ARGS_JSON is not valid JSON Vec<String>: {e}");
            std::process::exit(2);
        }),
        Err(_) => Vec::new(),
    };

    let cwd = std::env::var("EXEC_CWD").ok();

    let workspace_root = std::env::var("EXEC_WORKSPACE_ROOT").unwrap_or_else(|_| {
        eprintln!("[caprun-exec-launcher] EXEC_WORKSPACE_ROOT env var required");
        std::process::exit(2);
    });

    // --- Self-confine, in the mandatory order: rlimits -> Landlock -> seccomp ---
    //
    // Mirrors sandbox::apply_confinement()'s ordering (crates/sandbox/src/lib.rs)
    // exactly, applied to the exec-child variants. Each step aborts on failure
    // (fail-closed: no target ever runs unconfined) BEFORE the execve below.

    sandbox::apply_rlimits().expect("[caprun-exec-launcher] apply_rlimits failed");

    sandbox::exec_child_ruleset(Path::new(&workspace_root))
        .expect("[caprun-exec-launcher] exec_child_ruleset failed");

    sandbox::exec_child_filter().expect("[caprun-exec-launcher] exec_child_filter failed");

    // --- Self-replacing exec into the target (never sh -c, never a shell join) ---
    //
    // std::process::Command::exec() only returns on failure. stdout/stderr fds
    // set up by the broker's original Stdio::piped() spawn are inherited
    // across this exec unchanged (standard fd-inheritance semantics), so the
    // broker's existing pipe reader sees the TARGET's output with no extra
    // plumbing.

    let mut cmd = std::process::Command::new(&command);
    cmd.args(&args);
    if let Some(dir) = &cwd {
        cmd.current_dir(dir);
    }

    let err = cmd.exec();
    eprintln!("[caprun-exec-launcher] exec of '{command}' failed: {err}");
    std::process::exit(2);
}
