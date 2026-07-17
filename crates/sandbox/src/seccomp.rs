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

/// seccomp-bpf filter for a broker-spawned `process.exec` child (the
/// `caprun-exec-launcher`, applied to itself post-fork, pre-exec — DESIGN
/// §1.3 Option B).
///
/// Identical to `apply_worker_filter()` EXCEPT it drops the two
/// `SYS_execve`/`SYS_execveat` deny entries: the launcher's own upcoming
/// `execve` of the target binary must succeed. Denying execve here would
/// prevent the launcher from ever running the target.
///
/// WHY execve is not denied: a stateless BPF filter cannot express
/// "allow exactly one future execve, then deny all subsequent ones" — there
/// is no recursion-deny realizable with seccomp-bpf alone (DESIGN §1.4 B1
/// resolution). The bound on what a grandchild (spawned by the target
/// binary itself) can do is NOT a seccomp execve-deny — it is the
/// combination of Landlock's Execute allow-list (`exec_child_ruleset`, which
/// only permits executing enumerated system-path binaries, never anything
/// under the workspace) and this same persistent socket(AF_INET/AF_INET6)
/// deny, which survives execve unchanged (kernel semantics: seccomp filters
/// are inherited across execve — DESIGN §9 A5, the single load-bearing
/// kernel-semantics assumption here, empirically confirmed by the 32-06
/// confinement negative test).
///
/// The socket(AF_INET/AF_INET6) deny block below is copied byte-for-byte
/// from `apply_worker_filter()` — same rules, same match/mismatch actions,
/// same BpfProgram conversion, same `seccompiler::apply_filter` call (which
/// still sets NO_NEW_PRIVS internally).
#[cfg(target_os = "linux")]
pub fn exec_child_filter() -> std::io::Result<()> {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule,
    };
    use std::convert::TryInto;

    let filter = SeccompFilter::new(
        vec![
            // NO execve/execveat deny here — the launcher's own upcoming
            // execve() must succeed (DESIGN §1.4 B1 resolution: no seccomp
            // recursion-deny is realizable with a stateless BPF program;
            // grandchild bound is Landlock Execute allow-list + this same
            // persistent net-deny, not a seccomp execve-deny).
            //
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
        SeccompAction::Allow, // mismatch: allow all other syscalls (including execve)
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
pub fn exec_child_filter() -> std::io::Result<()> {
    Ok(())
}
