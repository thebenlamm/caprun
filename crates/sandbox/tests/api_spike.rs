/// api_spike — Wave 0 spike: prove seccompiler 0.5.0 deny-rule API on Linux
///
/// This test is gated `#[cfg(target_os = "linux")]` and proves:
///   1. nix::sys::prctl::prctl(PR_SET_NO_NEW_PRIVS, 1) call signature (A4)
///   2. seccompiler 0.5.0 SeccompFilter deny-execve + deny-socket(AF_INET)
///   3. apply_filter + in-process enforcement
///
/// The verified call pattern is recorded in the doc-block at the top of
/// crates/sandbox/src/seccomp.rs for Wave 2 consumption.
///
/// See 03-PLAN.md Task 2 for the full spike spec.

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn seccompiler_deny_rule_api() {
    // TODO Wave 0 Task 2: implement the spike inline here.
    // Gated #[ignore] so cargo test --workspace --lib passes on macOS.
    // On Linux CI: cargo test -p sandbox --test api_spike (no --ignored flag
    // needed — the spike impl will use #[test] without #[ignore]).
    assert!(true); // placeholder — Task 2 will replace this with the real spike
}
