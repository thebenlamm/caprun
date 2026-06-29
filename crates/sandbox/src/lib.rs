/// sandbox — kernel confinement primitives
///
/// Implements kernel-enforced confinement for confined worker processes.
///
/// # Self-Confinement Model
///
/// `apply_confinement()` is designed to be called by a process on **ITSELF**
/// after startup, NOT inside `Command::pre_exec` before an exec.
///
/// Rationale (planner decision, Phase 3 Plan 02):
/// - Landlock deny-all restricts the Execute access right and blocks reading
///   the target binary — applying it in `pre_exec` would prevent the worker
///   binary from ever being loaded by exec.
/// - seccomp deny-execve blocks the exec syscall itself — applying it in
///   `pre_exec` would prevent the exec that launches the worker.
///
/// Therefore the worker process confines itself at startup, after its binary
/// has been loaded and after it has (optionally) connected to the broker.
///
/// # Mandatory Ordering
///
/// The confinement primitives are applied in this strict order:
/// 1. `rlimits::apply_rlimits()` — RLIMIT_AS + RLIMIT_CPU (unprivileged)
/// 2. `landlock::deny_all_filesystem()` — deny all filesystem access
/// 3. `seccomp::apply_worker_filter()` — deny execve/execveat + socket(AF_INET/6);
///    `seccompiler::apply_filter()` calls `prctl(PR_SET_NO_NEW_PRIVS, 1)`
///    internally before installing the filter. This satisfies the kernel
///    requirement that NO_NEW_PRIVS precede any unprivileged seccomp filter
///    without a separate nix::prctl call.
///
/// # Platform Notes
///
/// All confinement code is `#[cfg(target_os = "linux")]`. On macOS (and other
/// non-Linux targets), every public function is a no-op stub that returns
/// `Ok(())` without panicking. This is intentional — confinement is a
/// Linux-only security claim (see REQUIREMENTS.md).

pub mod landlock;
pub mod rlimits;
pub mod seccomp;

pub use landlock::deny_all_filesystem;
pub use rlimits::apply_rlimits;
pub use seccomp::apply_worker_filter;

/// Apply all confinement primitives to the calling process.
///
/// Call this at worker startup after connecting to the broker.
/// See module doc-block for the mandatory ordering rationale.
///
/// Confinement applied (Linux):
/// 1. `RLIMIT_AS` = 512 MiB, `RLIMIT_CPU` = 30 s
/// 2. Landlock deny-all filesystem (abstract UDS unaffected)
/// 3. seccomp deny execve/execveat + socket(AF_INET/AF_INET6); sets NO_NEW_PRIVS internally
#[cfg(target_os = "linux")]
pub fn apply_confinement() -> std::io::Result<()> {
    rlimits::apply_rlimits()?;
    landlock::deny_all_filesystem()?;
    seccomp::apply_worker_filter()?;
    Ok(())
}

/// macOS / non-Linux no-op stub. Returns `Ok(())` without panicking.
#[cfg(not(target_os = "linux"))]
pub fn apply_confinement() -> std::io::Result<()> {
    eprintln!("[sandbox] WARNING: confinement is a no-op on non-Linux targets");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_on_macos_does_not_panic() {
        // This test runs on macOS (and any non-Linux). apply_confinement must
        // return Ok(()) without panicking. On Linux CI this path is cfg-gated away.
        #[cfg(not(target_os = "linux"))]
        assert!(apply_confinement().is_ok());
    }
}
