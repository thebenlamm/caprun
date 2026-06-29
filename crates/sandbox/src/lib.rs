/// sandbox — kernel confinement primitives
///
/// All confinement is Linux-only. On non-Linux targets every public function
/// is a no-op stub that logs a warning and returns Ok(()).
/// Apply via `apply_confinement()` inside `Command::pre_exec` ONLY — never
/// in the parent process.
///
/// Threat model: T-03-01 — macOS no-op stubs must never give a false sense
/// of enforcement. All confinement code is `#[cfg(target_os = "linux")]`;
/// the macOS path eprintln-warns and returns Ok(()).

pub mod landlock;
pub mod seccomp;
pub mod rlimits;

pub use landlock::deny_all_filesystem;
pub use seccomp::apply_worker_filter;
pub use rlimits::apply_rlimits;

/// Apply all confinement primitives in order: rlimits → Landlock → seccomp.
///
/// MUST be called inside `Command::pre_exec` in the forked child, NEVER in
/// the parent caprun process. The order is mandatory:
/// 1. prctl(NO_NEW_PRIVS) is called inside seccomp::apply_worker_filter.
/// 2. rlimits first (unprivileged, always safe).
/// 3. Landlock next (filesystem restriction; abstract UDS unaffected).
/// 4. seccomp last (requires NO_NEW_PRIVS to precede it).
#[cfg(target_os = "linux")]
pub fn apply_confinement() -> std::io::Result<()> {
    rlimits::apply_rlimits()?;
    landlock::deny_all_filesystem()?;
    seccomp::apply_worker_filter()?;
    Ok(())
}

/// macOS / non-Linux no-op stub. Logs a warning; returns Ok(()).
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
        // return Ok(()) without panicking. On Linux CI this is cfg-gated away.
        #[cfg(not(target_os = "linux"))]
        assert!(apply_confinement().is_ok());
    }
}
