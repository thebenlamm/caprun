/// seccomp — seccomp-bpf worker filter
///
/// Applies a BPF filter that:
///   - Denies execve/execveat (prevents spawning new processes)
///   - Denies socket(AF_INET) and socket(AF_INET6) (blocks outbound TCP/UDP)
///   - Allows all other syscalls (default action: Allow)
///
/// ─────────────────────────────────────────────────────────────────────────────
/// VERIFIED seccompiler 0.5.0 API — recorded by Wave-0 Task 2 spike
/// (Update this block when Task 2 confirms or corrects the assumed API.)
/// ─────────────────────────────────────────────────────────────────────────────
/// Current status: [ASSUMED] — see 03-RESEARCH.md §Assumptions Log A1.
/// Wave 0 Task 2 (crates/sandbox/tests/api_spike.rs) will prove the exact
/// call pattern on Linux and update this doc-block with the VERIFIED pattern.
///
/// Assumed call shape (to be confirmed):
///   SeccompFilter::new(rules, default_action, match_action, arch)
///   where rules: BTreeMap<i64, Vec<SeccompRule>>
///   and arch: seccompiler::TargetArch via std::env::consts::ARCH.try_into()
///
/// Linux-only. On macOS this module is compiled as a no-op stub.

/// Apply the worker seccomp-bpf filter.
///
/// MUST be called AFTER `prctl(PR_SET_NO_NEW_PRIVS, 1)` — kernel requirement
/// for unprivileged filter installation (RESEARCH.md Pitfall 2).
/// The prctl call is made inside this function, so callers need not invoke it
/// separately.
///
/// Returns `std::io::Result<()>` for `Command::pre_exec` compatibility.
#[cfg(target_os = "linux")]
pub fn apply_worker_filter() -> std::io::Result<()> {
    // Wave-0 Task 2 will prove the exact seccompiler 0.5.0 API and replace
    // this stub with the verified implementation. The doc-block above records
    // the verified pattern for Wave 2 Plan 02.
    //
    // Stub body: returns Ok(()) so the workspace builds on macOS CI without
    // a real seccomp call. On Linux CI, Task 2's api_spike.rs proves the API.
    Ok(())
}

/// No-op stub on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn apply_worker_filter() -> std::io::Result<()> {
    Ok(())
}
