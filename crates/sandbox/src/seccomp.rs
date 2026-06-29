/// seccomp — seccomp-bpf worker filter
///
/// Applies a BPF filter that:
///   - Denies execve/execveat (prevents spawning new processes)
///   - Denies socket(AF_INET) and socket(AF_INET6) (blocks outbound TCP/UDP)
///   - Allows all other syscalls (default mismatch action: Allow)
///
/// ─────────────────────────────────────────────────────────────────────────────
/// VERIFIED seccompiler 0.5.0 API — confirmed by reading crate source 2026-06-29
/// and by crates/sandbox/tests/api_spike.rs (compiles + BpfProgram conversion)
/// ─────────────────────────────────────────────────────────────────────────────
///
/// SeccompFilter::new(
///     rules: impl IntoIterator<Item = (i64, Vec<SeccompRule>)>,
///     mismatch_action: SeccompAction,  // action for syscalls NOT matching any rule
///     match_action: SeccompAction,     // action for syscalls matching a rule
///     target_arch: TargetArch,         // via std::env::consts::ARCH.try_into().unwrap()
/// ) -> Result<SeccompFilter, Error>
///
/// IMPORTANT — NO_NEW_PRIVS: seccompiler::apply_filter() calls
/// prctl(PR_SET_NO_NEW_PRIVS, 1) INTERNALLY via libc::prctl. Do NOT call
/// nix::prctl separately before apply_filter. The call order in apply_worker_filter
/// below is: build BpfProgram → seccompiler::apply_filter (which sets
/// NO_NEW_PRIVS then installs the filter). The sandbox::apply_confinement()
/// caller (pre_exec hook in caprun) does NOT need to set NO_NEW_PRIVS.
///
/// nix 0.31.x prctl signature (A4) — confirmed by crate inspection:
///   nix::sys::prctl::prctl(PrctlOption::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
///   → returns nix::Result<()>
/// This is NOT needed when using seccompiler::apply_filter.
///
/// Deny-rule construction (verified from api_spike.rs):
///   (libc::SYS_execve, vec![])   → deny execve unconditionally (empty = always match)
///   (libc::SYS_socket, vec![rule_af_inet, rule_af_inet6]) → deny specific families
///
/// SeccompRule::new(vec![cond1, cond2]).unwrap()  // AND-bound conditions
/// SeccompCondition::new(arg_index, arg_len, op, value).unwrap()
///   arg_len: SeccompCmpArgLen::Dword (32-bit) for socket family (int)
///   op:      SeccompCmpOp::Eq
///   value:   libc::AF_INET as u64 or libc::AF_INET6 as u64
///
/// BpfProgram: let prog: BpfProgram = filter.try_into().unwrap();
/// Apply:      seccompiler::apply_filter(&prog)
///
/// Linux-only. On macOS this module is compiled as a no-op stub.

/// Apply the worker seccomp-bpf filter.
///
/// Builds a BPF filter using the verified seccompiler 0.5.0 API, then installs
/// it. seccompiler::apply_filter sets PR_SET_NO_NEW_PRIVS internally — no
/// separate prctl call is needed.
///
/// Returns `std::io::Result<()>` for `Command::pre_exec` compatibility.
#[cfg(target_os = "linux")]
pub fn apply_worker_filter() -> std::io::Result<()> {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule,
    };
    use std::convert::TryInto;

    let filter = SeccompFilter::new(
        vec![
            // Deny execve unconditionally (empty vec = always match)
            (libc::SYS_execve, vec![]),
            (libc::SYS_execveat, vec![]),
            // Deny socket(AF_INET, ...) — blocks outbound TCP/UDP IPv4
            (
                libc::SYS_socket,
                vec![
                    SeccompRule::new(vec![SeccompCondition::new(
                        0,
                        SeccompCmpArgLen::Dword,
                        SeccompCmpOp::Eq,
                        libc::AF_INET as u64,
                    )
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?])
                    .map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))
                    })?,
                    // Deny socket(AF_INET6, ...) — blocks outbound TCP/UDP IPv6
                    SeccompRule::new(vec![SeccompCondition::new(
                        0,
                        SeccompCmpArgLen::Dword,
                        SeccompCmpOp::Eq,
                        libc::AF_INET6 as u64,
                    )
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?])
                    .map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))
                    })?,
                ],
            ),
        ]
        .into_iter()
        .collect(),
        SeccompAction::Allow, // mismatch: allow all other syscalls
        SeccompAction::Errno(libc::EPERM as u32), // match: deny with EPERM
        std::env::consts::ARCH
            .try_into()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    let program: BpfProgram = filter
        .try_into()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    // apply_filter sets PR_SET_NO_NEW_PRIVS internally before installing the filter.
    seccompiler::apply_filter(&program)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))
}

/// No-op stub on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn apply_worker_filter() -> std::io::Result<()> {
    Ok(())
}
