/// api_spike — Wave 0 spike: prove seccompiler 0.5.0 deny-rule API on Linux
///
/// VERIFIED seccompiler 0.5.0 API (verified by reading crate source 2026-06-29):
///
/// SeccompFilter::new(
///     rules: impl IntoIterator<Item = (i64, Vec<SeccompRule>)>,
///     mismatch_action: SeccompAction,  // action for syscalls NOT matching any rule
///     match_action: SeccompAction,     // action for syscalls matching a rule
///     target_arch: TargetArch,         // via std::env::consts::ARCH.try_into().unwrap()
/// ) -> Result<SeccompFilter, Error>
///
/// IMPORTANT: apply_filter() calls prctl(PR_SET_NO_NEW_PRIVS, 1) INTERNALLY.
/// Do NOT call nix::prctl separately before apply_filter — it is redundant.
/// (nix 0.31 prctl would be: nix::sys::prctl::prctl(
///   nix::sys::prctl::PrctlOption::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) but
///   seccompiler calls libc::prctl directly, making this unnecessary.)
///
/// Deny-rule construction:
///   - (libc::SYS_execve, vec![]) = deny execve unconditionally (empty vec = always match)
///   - (libc::SYS_socket, vec![rule_af_inet, rule_af_inet6]) = deny specific families
///
/// SeccompRule::new(vec![condition1, condition2, ...])  // AND-bound conditions
/// SeccompCondition::new(arg_index, arg_len, op, value)
///   arg_index: 0-5 (which syscall argument)
///   arg_len:   SeccompCmpArgLen::Dword (32-bit) or Qword (64-bit)
///   op:        SeccompCmpOp::Eq, Ne, Lt, Le, Gt, Ge, MaskedEq
///   value:     u64
///
/// BpfProgram conversion: let program: BpfProgram = filter.try_into().unwrap();
/// Apply: seccompiler::apply_filter(&program)  // this also sets NO_NEW_PRIVS
///
/// See: crates/sandbox/src/seccomp.rs for the Wave 2 implementation target.

#[cfg(target_os = "linux")]
mod spike {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule,
    };
    use std::convert::TryInto;

    /// Prove the seccompiler 0.5.0 deny-rule API compiles and loads correctly.
    ///
    /// This test MUST NOT run in the parent test process if it wants to verify
    /// that the filter actually blocks execve — the filter is process-wide and
    /// would prevent cargo-test from exec'ing reporters. Instead, the spike
    /// proves compilation and BpfProgram conversion; actual enforcement is
    /// tested via confine-probe in confinement_integration.rs (Wave 2).
    #[test]
    fn seccompiler_deny_rule_compiles_and_converts() {
        // Build a filter that:
        //   - denies execve (unconditionally, empty rule vec)
        //   - denies execveat (unconditionally)
        //   - denies socket(AF_INET, ...) and socket(AF_INET6, ...)
        //   - allows everything else (mismatch_action = Allow)
        let filter_result = SeccompFilter::new(
            vec![
                // execve: empty vec = always match → Errno(EPERM)
                (libc::SYS_execve, vec![]),
                // execveat: empty vec = always match → Errno(EPERM)
                (libc::SYS_execveat, vec![]),
                // socket: deny AF_INET and AF_INET6 (arg 0 = family, 32-bit)
                (
                    libc::SYS_socket,
                    vec![
                        SeccompRule::new(vec![SeccompCondition::new(
                            0,
                            SeccompCmpArgLen::Dword,
                            SeccompCmpOp::Eq,
                            libc::AF_INET as u64,
                        )
                        .unwrap()])
                        .unwrap(),
                        SeccompRule::new(vec![SeccompCondition::new(
                            0,
                            SeccompCmpArgLen::Dword,
                            SeccompCmpOp::Eq,
                            libc::AF_INET6 as u64,
                        )
                        .unwrap()])
                        .unwrap(),
                    ],
                ),
            ]
            .into_iter()
            .collect(),
            SeccompAction::Allow,                          // mismatch: allow other syscalls
            SeccompAction::Errno(libc::EPERM as u32),      // match: deny with EPERM
            std::env::consts::ARCH.try_into().unwrap(),
        );

        // Compilation to BpfProgram must succeed (proves API is correct)
        let filter = filter_result.expect("SeccompFilter::new failed");
        let program: BpfProgram = filter.try_into().expect("BpfProgram conversion failed");

        // Program must be non-empty (proves it has real BPF instructions)
        assert!(!program.is_empty(), "BPF program must not be empty");

        // Note: We do NOT call seccompiler::apply_filter(&program) here because
        // that would restrict the test process itself. Actual enforcement is
        // tested in confinement_integration.rs via the confine-probe binary
        // (Wave 2 Plan 02) which runs in a subprocess.
        //
        // VERIFIED: The filter compiles, converts, and produces a non-empty BpfProgram.
        // The seccompiler 0.5.0 API is confirmed working as documented above.
    }
}
