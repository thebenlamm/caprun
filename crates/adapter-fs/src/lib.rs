/// adapter-fs — filesystem adapter via SCM_RIGHTS fd-passing
///
/// The broker opens a workspace file and passes the open fd to the worker
/// via sendmsg(SCM_RIGHTS) over the existing UDS connection. The worker
/// reads via the received fd without ever having ambient filesystem access
/// (REQ-adapters-fs).
///
/// SCM_RIGHTS fd-passing is available on both Linux and macOS — this
/// crate compiles and the fd-pass round-trip test runs on the dev machine.
/// In production, the worker is confined by Landlock (Plan 02) so the
/// broker-passed fd is the only way to touch any file.
///
/// Correctness invariants (RESEARCH.md pitfalls):
/// - Exactly one `ControlMessage::ScmRights` slice per sendmsg (nix #464)
/// - cmsg buffer sized via `cmsg_space!([RawFd; 1])` (Pitfall 3)
/// - FD_CLOEXEC set immediately after recvmsg via fcntl (Pitfall 6)
/// - At least one iov byte sent alongside cmsg (some kernels require >=1 byte)

pub mod protocol;

use std::os::fd::RawFd;

/// Send an open file descriptor to a peer over a Unix socket.
///
/// Sends `file_raw_fd` via `SCM_RIGHTS` ancillary data over the connected
/// socket `socket_raw_fd`. Exactly one fd is sent in a single
/// `ControlMessage::ScmRights` slice to avoid platform-dependent behavior
/// (see nix issue #464).
///
/// A 1-byte iov payload is included because some kernels require at least
/// one payload byte for cmsg delivery.
///
/// # Errors
/// Returns a `nix::errno::Errno` on `sendmsg` failure.
///
/// # Note on async contexts
/// `sendmsg` is a blocking syscall. When called from a tokio async task,
/// wrap in `tokio::task::spawn_blocking` to avoid blocking the runtime thread.
pub fn pass_fd(_socket_raw_fd: RawFd, _file_raw_fd: RawFd) -> nix::Result<()> {
    // TODO Wave 2 Plan 04: implement via nix::sys::socket::sendmsg SCM_RIGHTS
    // Pattern: RESEARCH.md Pattern 6
    Err(nix::errno::Errno::ENOSYS)
}

/// Receive a file descriptor from a peer over a Unix socket.
///
/// Reads one fd from `SCM_RIGHTS` ancillary data arriving on
/// `socket_raw_fd`. The cmsg buffer is sized using `cmsg_space!([RawFd; 1])`
/// (RESEARCH.md Pitfall 3). `FD_CLOEXEC` is set on the received fd
/// immediately after receipt (RESEARCH.md Pitfall 6) to prevent accidental
/// fd leakage into worker-exec'd grandchildren.
///
/// # Errors
/// - Returns `Err(Errno::ENODATA)` if no `SCM_RIGHTS` cmsg arrived.
/// - Returns the underlying `recvmsg`/`fcntl` errno on failure.
///
/// # Note on async contexts
/// `recvmsg` is a blocking syscall. When called from a tokio async task,
/// wrap in `tokio::task::spawn_blocking` to avoid blocking the runtime thread.
pub fn recv_fd(_socket_raw_fd: RawFd) -> nix::Result<RawFd> {
    // TODO Wave 2 Plan 04: implement via nix::sys::socket::recvmsg SCM_RIGHTS
    // Pattern: RESEARCH.md Pattern 6
    Err(nix::errno::Errno::ENOSYS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::socket::{socketpair, AddressFamily, SockType, SockFlag};
    use std::io::Read;
    use std::os::unix::io::{AsRawFd, FromRawFd};

    /// TDD RED: unit-level pass_fd + recv_fd round-trip via a socketpair.
    ///
    /// This test FAILS on the stub implementation (returns ENOSYS) and passes
    /// after the real SCM_RIGHTS implementation is in place (Task 1 GREEN).
    /// SCM_RIGHTS works on both Linux and macOS — no cfg gate needed.
    #[test]
    fn pass_recv_fd_roundtrip() {
        let (broker_sock, worker_sock) = socketpair(
            AddressFamily::Unix,
            SockType::Stream,
            None,
            SockFlag::empty(),
        )
        .expect("socketpair failed");

        let mut tmp_path = std::env::temp_dir();
        tmp_path.push("adapter_fs_lib_roundtrip.txt");
        let content = b"unit-level fd-pass test content";
        std::fs::write(&tmp_path, content).expect("write temp file");

        // Open the file on the "broker" side
        let file = std::fs::File::open(&tmp_path).expect("open temp file");
        let file_raw = file.as_raw_fd();

        // Broker side: send the open fd over the socket
        pass_fd(broker_sock.as_raw_fd(), file_raw).expect("pass_fd failed");

        // Worker side: receive the fd and read via it — NOT via path re-open
        let received_fd = recv_fd(worker_sock.as_raw_fd()).expect("recv_fd failed");
        let mut received = unsafe { std::fs::File::from_raw_fd(received_fd) };
        let mut buf = Vec::new();
        received.read_to_end(&mut buf).expect("read via received fd");

        assert_eq!(buf, content, "worker must read identical content via passed fd");

        // Cleanup: received drops here (closes received_fd), file drops (closes file_raw)
        std::fs::remove_file(&tmp_path).ok();
    }
}
