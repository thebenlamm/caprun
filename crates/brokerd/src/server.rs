/// server — tokio async UDS IPC server
///
/// ─────────────────────────────────────────────────────────────────────────────
/// VERIFIED abstract-namespace UDS pattern — confirmed by reading tokio-1.52.3
/// source (src/net/unix/listener.rs + stream.rs) 2026-06-29, and by
/// crates/brokerd/tests/uds_abstract_spike.rs (bind → accept → round-trip)
/// ─────────────────────────────────────────────────────────────────────────────
///
/// APPROACH A — PRIMARY (verified simpler in tokio 1.52.3):
///   tokio 1.52.3 handles abstract paths natively in UnixListener::bind and
///   UnixStream::connect. Simply pass the path with a leading NUL byte:
///
///   let listener = tokio::net::UnixListener::bind("\0/agentos/<session_id>")?;
///   // On Linux, tokio strips the \0 and calls:
///   // StdSocketAddr::from_abstract_name(&os_str_bytes[1..])
///
/// APPROACH B — ALTERNATIVE (if Approach A fails — more explicit):
///   use std::os::linux::net::SocketAddrExt;
///   let addr = std::os::unix::net::SocketAddr::from_abstract_name(b"/agentos/<session_id>")?;
///   let std_listener = std::os::unix::net::UnixListener::bind_addr(&addr)?;
///   std_listener.set_nonblocking(true)?;
///   let listener = tokio::net::UnixListener::from_std(std_listener)?;
///
///   NOTE: std::os::unix::net::UnixListener::bind(path_with_null) does NOT work —
///   CString rejects embedded NUL bytes. Always use bind_addr for std approach.
///
/// KEY PROPERTY: Abstract sockets bypass Landlock filesystem restrictions.
/// After Landlock deny-all-filesystem is applied in pre_exec, the worker can
/// still connect to the broker's abstract socket. This is why abstract UDS is
/// the correct choice (over path-based /tmp/agentos.sock which Landlock would block).
///
/// FALLBACK (per RESEARCH.md Q1): If abstract bind returns EINVAL on an older kernel,
/// use a temp-dir path-based socket and add a Landlock read/connect exception
/// for that path in sandbox::landlock::deny_all_filesystem.
///
/// IPC framing: 4-byte LE length prefix + JSON body (serde_json).
/// Max message size: 64 KiB (ASVS V5 input validation).
///
/// Phase 3 Wave 0: stub compiles. Wave 2 Plan 03 implements the full loop.

const MAX_MSG_SIZE: usize = 64 * 1024;

/// Start the broker IPC server on an abstract-namespace UDS socket.
///
/// The socket path is `\0/agentos/{session_id}` (abstract namespace — no
/// filesystem entry; survives Landlock deny-all on the worker side).
///
/// Verified pattern (Approach A): `tokio::net::UnixListener::bind("\0/agentos/<id>")`
///
/// Returns when the server loop exits (or immediately in this stub).
pub async fn run_broker_server(_session_id: &str) -> anyhow::Result<()> {
    // TODO Wave 2 Plan 03: implement full broker loop using verified pattern:
    //
    //   let sock_path = format!("\0/agentos/{_session_id}");
    //   let listener = tokio::net::UnixListener::bind(&sock_path)?;
    //   loop {
    //       let (mut stream, _addr) = listener.accept().await?;
    //       tokio::spawn(async move { handle_connection(&mut stream).await });
    //   }
    //
    // Message handling: length-prefix (4-byte LE) + serde_json::from_slice.
    // Guard: reject messages > MAX_MSG_SIZE (64 KiB) before allocation.
    let _ = MAX_MSG_SIZE; // silence unused warning until Wave 2
    Ok(())
}
