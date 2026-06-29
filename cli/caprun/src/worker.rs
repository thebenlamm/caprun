/// caprun-worker — self-confining worker binary (Phase 3 substrate demo)
///
/// # Self-Confinement Order (load-bearing)
///
///   1. Connect to broker's abstract UDS (BROKER_SOCK env var, WITHOUT leading NUL).
///   2. Convert the tokio stream to a blocking std UnixStream for all subsequent I/O.
///   3. Call `sandbox::apply_confinement()` on self — AFTER connecting, so the
///      already-open broker socket fd survives Landlock deny-all.
///   4. Send `BrokerRequest::RequestFd { path }` (4-byte LE prefix + JSON).
///   5. Call `adapter_fs::recv_fd` to receive the file fd via SCM_RIGHTS out-of-band.
///      The broker sends the fd's 1-byte sendmsg payload BEFORE the JSON response,
///      so recvmsg here consumes exactly that 1 byte, leaving the JSON intact.
///   6. Read the `BrokerResponse::FdGranted` JSON response.
///   7. Read the workspace file via the received fd (NOT via open() — Landlock
///      deny-all blocks open on Linux; the passed fd is the only legal path).
///   8. Send `BrokerRequest::ReportRead { bytes_read }`.
///   9. Read `BrokerResponse::Ack` (drain the socket before closing to avoid EPIPE
///      in the broker's Ack write).
///  10. Exit 0.
///
/// # Cross-Platform Notes
///
/// The tokio `connect` call with the `\0` prefix compiles on macOS but fails at
/// runtime (abstract sockets are Linux-only). The e2e test is `#[cfg(target_os =
/// "linux")]` so this binary is never invoked on macOS; it only needs to COMPILE.

use anyhow::Context;
use brokerd::proto::{BrokerRequest, BrokerResponse};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let broker_sock = std::env::var("BROKER_SOCK").context("BROKER_SOCK")?;
    let workspace_file = std::env::var("WORKSPACE_FILE").context("WORKSPACE_FILE")?;

    // Connect to the broker's abstract-namespace UDS.
    // tokio detects the leading NUL and calls from_abstract_name internally
    // (Approach A, verified in Plan 03 uds_abstract_spike.rs).
    // On macOS this fails at runtime (abstract sockets are Linux-only) — the
    // e2e test is cfg-gated so the worker is never invoked on macOS.
    let sock_path = format!("\0{broker_sock}");
    let stream = tokio::net::UnixStream::connect(&sock_path)
        .await
        .context("connect to broker abstract UDS")?;

    // Convert to a blocking std UnixStream for all subsequent I/O.
    // This avoids mixing blocking recv_fd (recvmsg) with tokio's edge-triggered
    // epoll state on the same fd.
    let std_stream = stream.into_std().context("into_std")?;
    std_stream
        .set_nonblocking(false)
        .context("set_nonblocking")?;

    // Get raw fd for adapter_fs::recv_fd (SCM_RIGHTS recvmsg)
    let sock_fd = std_stream.as_raw_fd();

    // ── Self-confine AFTER connecting (self-confinement model) ───────────────
    // From this point on (Linux only): Landlock denies all filesystem access;
    // seccomp denies execve + socket(AF_INET/6); rlimits bound CPU/memory.
    // The already-open socket fd and any fds received via SCM_RIGHTS remain usable.
    sandbox::apply_confinement().map_err(|e| anyhow::anyhow!("apply_confinement: {e}"))?;

    // ── Send BrokerRequest::RequestFd ────────────────────────────────────────
    send_framed(&std_stream, &BrokerRequest::RequestFd { path: workspace_file })?;

    // ── Receive file fd via SCM_RIGHTS (out-of-band) ─────────────────────────
    // The broker sends the 1-byte sendmsg payload + fd cmsg BEFORE the JSON
    // FdGranted response. recv_fd (recvmsg) consumes exactly that 1 byte and
    // returns the received RawFd. The JSON FdGranted remains for the next read.
    let file_fd = adapter_fs::recv_fd(sock_fd)
        .map_err(|e| anyhow::anyhow!("recv_fd: {e}"))?;

    // ── Consume BrokerResponse::FdGranted JSON ────────────────────────────────
    let _granted: BrokerResponse = recv_framed(&std_stream)?;

    // ── Read workspace file via passed fd (NOT via open()) ───────────────────
    // On Linux, Landlock deny-all prevents open() — the passed fd is the only
    // way to read the file (complete mediation via the broker).
    // SAFETY: file_fd is a valid fd received from recv_fd (postcondition).
    let bytes_read: u64 = {
        let mut file = unsafe { std::fs::File::from_raw_fd(file_fd) };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).context("read via passed fd")?;
        // `file` drops here, closing file_fd; that's correct since the worker
        // holds its own dup'd copy (SCM_RIGHTS duplicates the fd).
        buf.len() as u64
    };

    // ── Send BrokerRequest::ReportRead ───────────────────────────────────────
    send_framed(&std_stream, &BrokerRequest::ReportRead { bytes_read })?;

    // ── Consume BrokerResponse::Ack ──────────────────────────────────────────
    // Drain the Ack before closing the socket to avoid EPIPE in the broker when
    // it writes the Ack to a closed peer socket.
    let _ack: BrokerResponse = recv_framed(&std_stream)?;

    Ok(())
}

/// Write a framed message (4-byte LE length prefix + JSON body) to `stream`.
fn send_framed(stream: &std::os::unix::net::UnixStream, msg: &impl serde::Serialize) -> anyhow::Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    (&*stream).write_all(&len)?;
    (&*stream).write_all(&body)?;
    Ok(())
}

/// Read a framed message (4-byte LE length prefix + JSON body) from `stream`.
fn recv_framed<T: serde::de::DeserializeOwned>(
    stream: &std::os::unix::net::UnixStream,
) -> anyhow::Result<T> {
    let mut len_buf = [0u8; 4];
    (&*stream).read_exact(&mut len_buf)?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    (&*stream).read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
