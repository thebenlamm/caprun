/// rlimits — resource limits for the confined worker
///
/// Applies unprivileged rlimits before Landlock and seccomp:
///   - RLIMIT_AS:  512 MiB virtual address space
///   - RLIMIT_CPU: 30 CPU seconds (wall-clock unlimited; CPU-time bounded)
///
/// Linux-only. On macOS this module is compiled as a no-op stub.

/// Apply resource limits via setrlimit(2).
///
/// Returns `std::io::Result<()>` for `Command::pre_exec` compatibility.
#[cfg(target_os = "linux")]
pub fn apply_rlimits() -> std::io::Result<()> {
    use nix::sys::resource::{setrlimit, Resource};

    setrlimit(
        Resource::RLIMIT_AS,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    setrlimit(Resource::RLIMIT_CPU, 30, 30)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

/// No-op stub on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn apply_rlimits() -> std::io::Result<()> {
    Ok(())
}
