/// e2e — substrate demo end-to-end integration tests (Linux-only)
///
/// Tests the full caprun demo: confined worker reads file via broker-passed
/// fd; read Event appears in the SQLite audit DAG with an unbroken hash chain.
/// Wave 2 Plan 05 implements the test bodies.

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn substrate_demo() {
    // TODO Wave 2 Plan 05: run the full caprun demo:
    //   1. Start brokerd UDS server
    //   2. Spawn caprun-worker with apply_confinement() in pre_exec
    //   3. Worker requests fd, reads file, reports read
    //   4. Assert file_read Event appears in audit DAG
    assert!(true); // placeholder — Wave 2
}

#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn dag_chain_integrity() {
    // TODO Wave 2 Plan 05: after substrate_demo, verify the DAG hash chain
    // is unbroken end-to-end (session_created → fd_granted → file_read).
    assert!(true); // placeholder — Wave 2
}
