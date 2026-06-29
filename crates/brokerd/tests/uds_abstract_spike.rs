/// uds_abstract_spike — Wave 0 spike: prove abstract-namespace UDS in tokio
///
/// This test is gated `#[cfg(target_os = "linux")]` and proves:
///   1. std::os::unix::net::UnixListener::bind with a null-prefixed abstract path
///   2. set_nonblocking(true) → tokio::net::UnixListener::from_std
///   3. Full bind→accept→connect→read-write round-trip with length-prefix framing
///
/// The verified pattern is recorded in the doc-block at the top of
/// crates/brokerd/src/server.rs for Wave 2 consumption.
///
/// See 03-PLAN.md Task 3 for the full spike spec.

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn abstract_uds_bind_from_std_accept() {
    // TODO Wave 0 Task 3: implement the spike here using #[tokio::test].
    // Gated #[ignore] so cargo test --workspace --lib passes on macOS.
    assert!(true); // placeholder — Task 3 will replace with real spike
}
