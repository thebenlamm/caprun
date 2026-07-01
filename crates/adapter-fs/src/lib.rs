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
pub mod workspace;

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
pub fn pass_fd(socket_raw_fd: RawFd, file_raw_fd: RawFd) -> nix::Result<()> {
    use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
    use std::io::IoSlice;

    // At least one iov byte required by some kernel versions for cmsg delivery.
    let iov = [IoSlice::new(b"\x00")];
    let fds = [file_raw_fd];
    // CRITICAL: exactly one ControlMessage::ScmRights slice (never multiple per
    // sendmsg — nix issue #464 documents platform-dependent behaviour otherwise).
    let cmsg = ControlMessage::ScmRights(&fds);
    sendmsg::<()>(socket_raw_fd, &iov, &[cmsg], MsgFlags::empty(), None)?;
    Ok(())
}

/// Receive a file descriptor from a peer over a Unix socket.
///
/// Reads one fd from `SCM_RIGHTS` ancillary data arriving on
/// `socket_raw_fd`. The cmsg buffer is sized using `cmsg_space!([RawFd; 1])`
/// (RESEARCH.md Pitfall 3). `FD_CLOEXEC` is set on the received fd
/// immediately after receipt (RESEARCH.md Pitfall 6) to prevent accidental
/// fd leakage into worker-exec'd grandchildren (mitigates T-03-11).
///
/// # Errors
/// - Returns `Err(Errno::ENODATA)` if no `SCM_RIGHTS` cmsg arrived (mitigates
///   T-03-12 — bogus fd is never returned if ancillary data was lost).
/// - Returns the underlying `recvmsg`/`fcntl` errno on failure.
///
/// # Note on async contexts
/// `recvmsg` is a blocking syscall. When called from a tokio async task,
/// wrap in `tokio::task::spawn_blocking` to avoid blocking the runtime thread.
pub fn recv_fd(socket_raw_fd: RawFd) -> nix::Result<RawFd> {
    use nix::sys::socket::{recvmsg, ControlMessageOwned, MsgFlags};
    use std::io::IoSliceMut;
    use std::os::fd::BorrowedFd;

    let mut buf = [0u8; 1];
    let mut iov = [IoSliceMut::new(&mut buf)];
    // Buffer MUST be at least cmsg_space!([RawFd; 1]) bytes — Pitfall 3.
    // The macro ensures correct platform-specific alignment and sizing.
    let mut cmsgspace = nix::cmsg_space!([RawFd; 1]);

    let msg = recvmsg::<()>(
        socket_raw_fd,
        &mut iov,
        Some(&mut cmsgspace),
        MsgFlags::empty(),
    )?;

    for cmsg in msg.cmsgs()? {
        if let ControlMessageOwned::ScmRights(fds) = cmsg {
            if let Some(&fd) = fds.first() {
                // Set FD_CLOEXEC immediately — received fds are NOT cloexec by
                // default after recvmsg. This closes the race window before any
                // exec (seccomp blocks exec anyway, but defence-in-depth
                // requires setting it here — see T-03-11 in threat register).
                //
                // SAFETY: `fd` is a valid file descriptor received from recvmsg.
                // BorrowedFd does not take ownership; the fd will be managed
                // by the caller after this function returns Ok(fd).
                let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
                nix::fcntl::fcntl(
                    borrowed,
                    nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
                )?;
                return Ok(fd);
            }
        }
    }
    // No SCM_RIGHTS cmsg found — return ENODATA rather than a bogus fd.
    // This surfaces dropped/truncated ancillary data rather than silently
    // handing the worker an invalid fd (mitigates T-03-12).
    Err(nix::errno::Errno::ENODATA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::socket::{socketpair, AddressFamily, SockType, SockFlag};
    use std::io::Read;
    use std::os::unix::io::{AsRawFd, FromRawFd};

    /// TDD GREEN: unit-level pass_fd + recv_fd round-trip via a socketpair.
    ///
    /// Proves that the broker can open a file and hand the fd to the worker
    /// via SCM_RIGHTS, and the worker can read the file content through the
    /// received fd without ever calling open() on the path.
    ///
    /// SCM_RIGHTS is cross-platform — this test runs on macOS (dev) and Linux
    /// (CI) without a cfg gate.
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
