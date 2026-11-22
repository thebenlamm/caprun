/// seccomp — seccomp-bpf worker filter
///
/// Applies a BPF filter that:
///   - Denies execve/execveat (worker only — prevents spawning new processes)
///   - Denies socket() for ALL families, io_uring_*, truncate(path), and
///     ioctl TIOCSTI/TIOCLINUX (worker and exec child — see `install_filter`)
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
    // execve/execveat: the worker never spawns anything itself.
    install_filter(&[libc::SYS_execve, libc::SYS_execveat])
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
/// under the workspace) and this same persistent socket()/io_uring
/// deny, which survives execve unchanged (kernel semantics: seccomp filters
/// are inherited across execve — DESIGN §9 A5, the single load-bearing
/// kernel-semantics assumption here, empirically confirmed by the 32-06
/// confinement negative test).
///
/// Shares `install_filter` with `apply_worker_filter()` so the two cannot
/// drift; the only difference is the worker's extra execve/execveat deny.
#[cfg(target_os = "linux")]
pub fn exec_child_filter() -> std::io::Result<()> {
    // NO execve/execveat deny here — the launcher's own upcoming execve()
    // must succeed (DESIGN §1.4 B1 resolution: no seccomp recursion-deny is
    // realizable with a stateless BPF program; grandchild bound is the
    // Landlock Execute allow-list + this same persistent egress deny).
    install_filter(&[])
}

/// Build and install the shared egress-deny filter
/// (DESIGN-confinement-egress-hardening.md §3), plus `extra_unconditional`
/// syscalls. Every match returns EPERM; everything else is allowed.
///
/// Denied for BOTH the worker and the exec child:
///   - `socket` (all families). The worker's only sockets are those it holds
///     at confinement time; the exec child gets none. Denying only
///     AF_INET/AF_INET6 left AF_UNIX open, which reaches the user D-Bus
///     session bus → `systemd-run --user` → an unconfined process (full
///     escape, reproduced 2026-11-22), and left AF_NETLINK etc. open.
///     `socketpair` stays allowed: it creates only an unnamed connected pair.
///   - `io_uring_setup`/`io_uring_enter`/`io_uring_register`. IORING_OP_SOCKET
///     creates a socket without the `socket` syscall, so a `socket` rule alone
///     is bypassable (reproduced 2026-11-22).
///   - `truncate(path)`. Landlock's Truncate right needs ABI V3 (kernel ≥6.2);
///     on older kernels Landlock is only PartiallyEnforced and path truncation
///     of any user-writable file is unchecked. `ftruncate` on an fd the process
///     legitimately holds stays allowed.
///   - `ioctl(_, TIOCSTI | TIOCLINUX, ..)`: terminal input injection into the
///     inherited controlling terminal (kernels < 6.2 or legacy_tiocsti=1).
#[cfg(target_os = "linux")]
fn install_filter(extra_unconditional: &[i64]) -> std::io::Result<()> {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule,
    };
    use std::collections::BTreeMap;
    use std::convert::TryInto;

    fn io_err(e: impl std::fmt::Display) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))
    }
    let ioctl_cmd_rule = |cmd: u64| -> std::io::Result<SeccompRule> {
        SeccompRule::new(vec![SeccompCondition::new(
            1,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Eq,
            cmd,
        )
        .map_err(io_err)?])
        .map_err(io_err)
    };

    // The ioctl entry below is argument-conditional; an unconditional extra
    // for the same syscall would be silently overwritten (BTreeMap key).
    if extra_unconditional.contains(&libc::SYS_ioctl) {
        return Err(io_err("SYS_ioctl cannot be an unconditional extra deny"));
    }

    // Empty rule vec = match unconditionally.
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for nr in [
        libc::SYS_socket,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_truncate,
    ]
    .into_iter()
    .chain(extra_unconditional.iter().copied())
    {
        rules.insert(nr, vec![]);
    }
    rules.insert(
        libc::SYS_ioctl,
        vec![
            ioctl_cmd_rule(libc::TIOCSTI as u64)?,
            ioctl_cmd_rule(libc::TIOCLINUX as u64)?,
        ],
    );

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,                     // mismatch: allow all other syscalls
        SeccompAction::Errno(libc::EPERM as u32), // match: deny with EPERM
        std::env::consts::ARCH.try_into().map_err(io_err)?,
    )
    .map_err(io_err)?;

    let program: BpfProgram = filter.try_into().map_err(io_err)?;

    // apply_filter sets PR_SET_NO_NEW_PRIVS internally before installing the filter.
    seccompiler::apply_filter(&program).map_err(io_err)
}

/// No-op stub on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn exec_child_filter() -> std::io::Result<()> {
    Ok(())
}
