//! confinement_integration — negative-assertion integration tests (Linux-only)
//!
//! These tests prove that a process confined by `sandbox::apply_confinement()` CANNOT:
//!   - read filesystem paths (REQ-sandbox / T-03-03 — `negative_fs`)
//!   - open outbound network sockets (REQ-sandbox / T-03-04 — `negative_net`)
//!   - exec new processes (REQ-sandbox / T-03-05 — `negative_exec`)
//!
//! Each test spawns the `confine-probe` binary (via `env!("CARGO_BIN_EXE_confine-probe")`)
//! with the corresponding argument (`fs`, `net`, `exec`). The probe applies
//! confinement to itself, attempts the forbidden operation, and exits 0 if the
//! operation was correctly blocked (EACCES or EPERM).
//!
//! Spawning a fresh single-purpose binary avoids async-signal-safety hazards
//! that arise from calling fork() inside the multithreaded libtest process.
//!
//! All tests are `#[cfg(target_os = "linux")]` — they are not compiled or run
//! on macOS dev machines. They run on Linux CI (ubuntu ≥ 22.04, Landlock-capable
//! kernel ≥ 5.13). `cargo test -p sandbox` exits 0 on macOS with 0 tests run.

/// Spawn `confine-probe <op>` and assert it exits 0 (operation correctly blocked).
#[cfg(target_os = "linux")]
fn assert_probe_blocked(op: &str) {
    let binary = env!("CARGO_BIN_EXE_confine-probe");
    let status = std::process::Command::new(binary)
        .arg(op)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn confine-probe {op}: {e}"));

    assert_eq!(
        status.code(),
        Some(0),
        "confine-probe {op}: expected exit 0 (correctly blocked) but got {:?}. \
         Exit codes: 0=blocked, 1=not-blocked, 2=unexpected-error, 100=non-Linux-sentinel",
        status.code()
    );
}

/// A confined process cannot open filesystem paths.
///
/// Proves T-03-03: Landlock deny-all blocks `open(~/.ssh/id_rsa)` → EACCES.
#[cfg(target_os = "linux")]
#[test]
fn negative_fs() {
    assert_probe_blocked("fs");
}

/// A confined process cannot open an outbound network socket.
///
/// Proves T-03-04: seccomp denies `socket(AF_INET, SOCK_STREAM, 0)` → EPERM.
#[cfg(target_os = "linux")]
#[test]
fn negative_net() {
    assert_probe_blocked("net");
}

/// A confined process cannot exec an un-allowlisted binary.
///
/// Proves T-03-05: seccomp denies `execve("/bin/true")` → EPERM (or Landlock
/// denies binary loading → EACCES; either blockage confirms confinement).
#[cfg(target_os = "linux")]
#[test]
fn negative_exec() {
    assert_probe_blocked("exec");
}
