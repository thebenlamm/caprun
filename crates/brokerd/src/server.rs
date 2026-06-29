/// server — tokio async UDS IPC server
///
/// ─────────────────────────────────────────────────────────────────────────────
/// VERIFIED abstract-UDS-in-tokio pattern — recorded by Wave-0 Task 3 spike
/// (Update this block when Task 3 confirms or corrects the assumed pattern.)
/// ─────────────────────────────────────────────────────────────────────────────
/// Current status: [ASSUMED] — see 03-RESEARCH.md §Assumptions Log A2.
/// Wave 0 Task 3 (crates/brokerd/tests/uds_abstract_spike.rs) will prove
/// the exact bind→from_std→accept pattern on Linux and update this doc-block.
///
/// Assumed bind pattern (to be confirmed by Task 3):
///   let addr = format!("\0/agentos/{session_id}");
///   let std_listener = std::os::unix::net::UnixListener::bind(&addr)?;
///   std_listener.set_nonblocking(true)?;
///   let listener = tokio::net::UnixListener::from_std(std_listener)?;
///
/// Fallback if abstract bind returns EINVAL (documented in RESEARCH.md Q1):
///   Use a temp-dir path-based socket and add a Landlock read/connect
///   exception for that path in sandbox::landlock::deny_all_filesystem.
///
/// Phase 3 Wave 0: stub compiles. Wave 2 Plan 03 implements the full loop.

/// Start the broker IPC server on an abstract-namespace UDS socket.
///
/// The socket path is `\0/agentos/{session_id}` (abstract namespace — no
/// filesystem entry; survives Landlock deny-all on the worker side).
///
/// Returns when the server loop exits (or immediately in this stub).
pub async fn run_broker_server(_session_id: &str) -> anyhow::Result<()> {
    // TODO Wave 2 Plan 03: bind abstract UDS, accept connections, dispatch
    // BrokerRequest messages from workers.
    // Pattern: RESEARCH.md Pattern 4 + verified in uds_abstract_spike.rs.
    Ok(())
}
