/// adapter-fs — filesystem adapter via SCM_RIGHTS fd-passing
///
/// The broker opens a workspace file and passes the open fd to the worker
/// via sendmsg(SCM_RIGHTS) over the existing UDS connection. The worker
/// reads via the received fd without ever having ambient filesystem access
/// (REQ-adapters-fs).
///
/// Phase 3 Wave 0: stubs compile on macOS. Wave 2 Plan 04 implements the
/// real sendmsg/recvmsg SCM_RIGHTS transfer (RESEARCH.md Pattern 6).

pub mod protocol;

use std::os::fd::RawFd;

/// Send an open file descriptor to a peer over a Unix socket.
///
/// The caller is responsible for opening the file; this function transfers
/// the fd via SCM_RIGHTS. At least one payload byte is sent alongside the
/// ancillary data (required by some kernel versions).
///
/// # Arguments
/// * `socket_raw_fd` — the connected UDS socket fd (blocking)
/// * `file_raw_fd`   — the open file fd to transfer
///
/// Returns `Ok(())` on success; `Err(nix::errno::Errno::ENOSYS)` in the
/// Phase 3 Wave 0 stub (Wave 2 Plan 04 replaces with real sendmsg call).
pub fn pass_fd(_socket_raw_fd: RawFd, _file_raw_fd: RawFd) -> nix::Result<()> {
    // TODO Wave 2 Plan 04: implement via nix::sys::socket::sendmsg SCM_RIGHTS
    // Pattern: RESEARCH.md Pattern 6
    Err(nix::errno::Errno::ENOSYS)
}

/// Receive a file descriptor from a peer over a Unix socket.
///
/// Reads the SCM_RIGHTS ancillary data and returns the received fd.
/// Sets O_CLOEXEC immediately after receipt (RESEARCH.md Pitfall 6).
///
/// Returns `Ok(RawFd)` on success; `Err(nix::errno::Errno::ENOSYS)` in the
/// Phase 3 Wave 0 stub (Wave 2 Plan 04 replaces with real recvmsg call).
pub fn recv_fd(_socket_raw_fd: RawFd) -> nix::Result<RawFd> {
    // TODO Wave 2 Plan 04: implement via nix::sys::socket::recvmsg SCM_RIGHTS
    // Pattern: RESEARCH.md Pattern 6
    Err(nix::errno::Errno::ENOSYS)
}
