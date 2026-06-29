/// confinement_integration — negative-assertion integration tests (Linux-only)
///
/// These tests prove that a confined worker CANNOT:
///   - read filesystem paths (REQ-sandbox negative_fs)
///   - open outbound network sockets (REQ-sandbox negative_net)
///   - exec new processes (REQ-sandbox negative_exec)
///
/// All tests are gated `#[cfg(target_os = "linux")]` — they do not run on
/// macOS dev machines; they run on Linux CI (ubuntu ≥ 22.04 with Landlock).
/// Wave 2 Plan 02 implements the test bodies.

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn negative_fs_access() {
    // TODO Wave 2 Plan 02: spawn confine-probe, apply_confinement(), assert
    // that open("/etc/passwd") returns EACCES or EPERM.
    assert!(true); // placeholder — Wave 2
}

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn negative_net_access() {
    // TODO Wave 2 Plan 02: after apply_confinement(), assert that
    // socket(AF_INET, SOCK_STREAM) returns EPERM (seccomp block).
    assert!(true); // placeholder — Wave 2
}

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn negative_exec() {
    // TODO Wave 2 Plan 02: after apply_confinement(), assert that
    // execve("/bin/true") returns EPERM (seccomp block).
    assert!(true); // placeholder — Wave 2
}
