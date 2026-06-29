/// fd_pass — SCM_RIGHTS fd-passing integration tests
///
/// Proves REQ-adapters-fs end-to-end: the broker opens a file and passes
/// the fd to the worker via SCM_RIGHTS over a socketpair. The worker reads
/// the file content through the received fd without ever calling open() on
/// the path. FD_CLOEXEC is verified to be set on the received fd.
///
/// SCM_RIGHTS is cross-platform — both tests run on macOS (dev) and
/// Linux (CI) without a cfg gate. No Linux-specific features are exercised here;
/// the Landlock confinement that makes the fd the *only* channel is tested in
/// crates/sandbox/tests/confinement_integration.rs (Linux-only, Plan 02).

use adapter_fs::{pass_fd, recv_fd};
use nix::sys::socket::{socketpair, AddressFamily, SockType, SockFlag};
use std::io::Read;
use std::os::unix::io::{AsRawFd, FromRawFd};

/// Round-trip test: broker passes an open fd; worker reads via the fd only.
///
/// Proves the core REQ-adapters-fs invariant: the worker receives a usable
/// file descriptor and reads the expected content through it without opening
/// the path directly (the path is not passed to the worker at all).
#[test]
fn round_trip() {
    // Create a connected pair: broker_sock (left) and worker_sock (right)
    let (broker_sock, worker_sock) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .expect("socketpair failed");

    // Write known content to a temp file (broker's workspace file)
    let mut tmp_path = std::env::temp_dir();
    tmp_path.push("adapter_fs_round_trip_test.txt");
    let content = b"broker opened this; worker reads via fd only -- never via open()";
    std::fs::write(&tmp_path, content).expect("write temp file");

    // Broker side: open the file and pass the fd via SCM_RIGHTS
    let file = std::fs::File::open(&tmp_path).expect("broker: open temp file");
    let file_raw = file.as_raw_fd();
    pass_fd(broker_sock.as_raw_fd(), file_raw).expect("pass_fd failed");

    // Worker side: receive the fd -- this is the ONLY way to access the file
    let received_fd = recv_fd(worker_sock.as_raw_fd()).expect("recv_fd failed");

    // Worker reads via the received fd (NOT via re-opening tmp_path)
    let mut received_file = unsafe { std::fs::File::from_raw_fd(received_fd) };
    let mut buf = Vec::new();
    received_file
        .read_to_end(&mut buf)
        .expect("read via received fd failed");

    assert_eq!(
        buf, content,
        "worker must read identical content through the passed fd, not via open()"
    );

    // received_file drops here (closes received_fd); file drops (closes file_raw)
    std::fs::remove_file(&tmp_path).ok();
}

/// O_CLOEXEC test: the received fd must have FD_CLOEXEC set immediately.
///
/// Proves RESEARCH.md Pitfall 6 / threat T-03-11 mitigation: recv_fd sets
/// FD_CLOEXEC on the received fd before returning, so the fd cannot leak
/// into any grandchild process the worker might exec (seccomp blocks exec
/// anyway, but defence-in-depth requires the flag to be set here).
#[test]
fn fd_cloexec() {
    let (broker_sock, worker_sock) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .expect("socketpair failed");

    let mut tmp_path = std::env::temp_dir();
    tmp_path.push("adapter_fs_cloexec_test.txt");
    std::fs::write(&tmp_path, b"cloexec test file content").expect("write temp file");

    let file = std::fs::File::open(&tmp_path).expect("open temp file");
    pass_fd(broker_sock.as_raw_fd(), file.as_raw_fd()).expect("pass_fd failed");

    let received_fd = recv_fd(worker_sock.as_raw_fd()).expect("recv_fd failed");

    // Query the file-descriptor flags -- FD_CLOEXEC must be set
    let flags = nix::fcntl::fcntl(
        // SAFETY: received_fd is valid -- it was just returned from recv_fd
        unsafe { std::os::fd::BorrowedFd::borrow_raw(received_fd) },
        nix::fcntl::FcntlArg::F_GETFD,
    )
    .expect("fcntl F_GETFD failed");

    let fd_flags = nix::fcntl::FdFlag::from_bits_truncate(flags);
    assert!(
        fd_flags.contains(nix::fcntl::FdFlag::FD_CLOEXEC),
        "FD_CLOEXEC must be set on received fd immediately after recv_fd \
         (RESEARCH.md Pitfall 6 / T-03-11 mitigation); flags={flags:#x}"
    );

    // Close the received fd manually (it was not wrapped in a File)
    nix::unistd::close(received_fd).ok();
    std::fs::remove_file(&tmp_path).ok();
}
