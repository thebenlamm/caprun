/// uds_abstract_spike — Wave 0 spike: prove abstract-namespace UDS in tokio
///
/// VERIFIED abstract-namespace UDS pattern — confirmed by reading tokio-1.52.3
/// source (src/net/unix/listener.rs + stream.rs) 2026-06-29:
///
/// APPROACH A (VERIFIED — PRIMARY): tokio 1.52.3 handles abstract paths natively.
///   tokio::net::UnixListener::bind("\0/agentos/<name>")
///   tokio::net::UnixStream::connect("\0/agentos/<name>").await
///
///   tokio detects the leading NUL byte and calls:
///   `StdSocketAddr::from_abstract_name(&os_str_bytes[1..])`
///   This works in tokio 1.52.3 on Linux (and Android). No from_std needed.
///
/// APPROACH B (VERIFIED — ALTERNATIVE): std bind_addr + from_std
///   use std::os::linux::net::SocketAddrExt;
///   let addr = std::os::unix::net::SocketAddr::from_abstract_name(b"/agentos/<name>")?;
///   let std_listener = std::os::unix::net::UnixListener::bind_addr(&addr)?;
///   std_listener.set_nonblocking(true)?;
///   let listener = tokio::net::UnixListener::from_std(std_listener)?;
///
///   NOTE: std::os::unix::net::UnixListener::bind(path_with_null_byte) DOES NOT work —
///   std converts to CString which rejects embedded NUL bytes. Use bind_addr instead.
///
/// IMPORTANT: Abstract sockets bypass Landlock filesystem restrictions — the
/// broker's abstract socket remains accessible to the worker even after
/// Landlock deny-all-filesystem is applied. This is the key property that
/// makes abstract UDS the correct choice for the broker IPC channel.
///
/// Wave 2 Plan 03 uses Approach A (simpler) in run_broker_server.
/// See crates/brokerd/src/server.rs for the verified doc-block.

#[cfg(target_os = "linux")]
mod spike {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Prove abstract-namespace UDS bind → accept → connect → framed-message round-trip.
    ///
    /// Uses tokio 1.52.3's native abstract-path support (Approach A from doc above).
    #[tokio::test]
    async fn abstract_uds_framed_round_trip() {
        let pid = std::process::id();
        // Abstract path: "\0" prefix + name. Tokio strips the NUL and calls
        // from_abstract_name with the remainder.
        let sock_path = format!("\0/agentos/spike-{pid}");

        // Server: bind abstract socket (tokio handles NUL-prefix natively)
        let listener =
            tokio::net::UnixListener::bind(&sock_path).expect("abstract UDS bind failed");

        let sock_path_clone = sock_path.clone();
        // Client task: connect and send a 4-byte LE length-prefixed JSON body
        let client_handle = tokio::spawn(async move {
            let mut stream = tokio::net::UnixStream::connect(&sock_path_clone)
                .await
                .expect("abstract UDS connect failed");

            let body = b"{\"msg\":\"hello from spike\"}";
            let len: u32 = body.len() as u32;
            stream
                .write_all(&len.to_le_bytes())
                .await
                .expect("write length failed");
            stream
                .write_all(body)
                .await
                .expect("write body failed");
        });

        // Server: accept and read the framed message
        let (mut stream, _addr) = listener.accept().await.expect("accept failed");

        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .expect("read length failed");
        let msg_len = u32::from_le_bytes(len_buf) as usize;

        let mut body = vec![0u8; msg_len];
        stream
            .read_exact(&mut body)
            .await
            .expect("read body failed");

        client_handle.await.expect("client task panicked");

        // Assert round-trip equality
        assert_eq!(
            &body,
            b"{\"msg\":\"hello from spike\"}",
            "round-trip body mismatch"
        );
        assert_eq!(
            msg_len,
            b"{\"msg\":\"hello from spike\"}".len(),
            "round-trip length mismatch"
        );

        // VERIFIED: Abstract-namespace UDS bind → accept → connect → framed message
        // round-trip succeeds in tokio 1.52.3 on Linux. The pattern is:
        //   tokio::net::UnixListener::bind("\0/name") — direct, no from_std needed.
    }
}
