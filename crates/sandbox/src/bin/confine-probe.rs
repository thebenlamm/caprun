//! confine-probe — self-confinement negative-assertion probe
//!
//! Applies `sandbox::apply_confinement()` to itself, then attempts a forbidden
//! operation. The integration tests in `crates/sandbox/tests/confinement_integration.rs`
//! spawn this binary and assert the exit code.
//!
//! # Usage
//!
//! ```text
//! confine-probe <fs|net|exec>
//! confine-probe smtp <host> <port>
//! ```
//!
//! # Exit codes
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | Operation was **correctly blocked** (EACCES or EPERM) |
//! | 1    | Operation **unexpectedly succeeded** — confinement failure |
//! | 2    | Unexpected error (wrong errno, missing argument, etc.) |
//! | 100  | Non-Linux: confinement is a no-op (sentinel, not a failure) |
//!
//! # Design
//!
//! The probe applies confinement to itself (self-confinement model) then
//! performs the forbidden operation using raw libc calls. Spawning a fresh
//! single-purpose binary (rather than fork()-inside-multithreaded-test) avoids
//! async-signal-safety hazards in the libtest process.

fn main() {
    // Non-Linux: confinement is a no-op; exit with sentinel 100
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("[confine-probe] non-Linux target — confinement is a no-op (sentinel 100)");
        std::process::exit(100);
    }

    #[cfg(target_os = "linux")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() < 2 {
            eprintln!("Usage: confine-probe <fs|net|exec|uring|netlink|truncate|tiocsti|threaded> | confine-probe unix <path> | confine-probe smtp <host> <port>");
            std::process::exit(2);
        }
        run_linux(&args);
    }
}

/// Linux implementation: apply confinement, then attempt the forbidden op.
///
/// `args` is the full argv (args[0] is the binary path, args[1] is the op).
/// The `smtp` op additionally consumes args[2] (host) and args[3] (port);
/// all other ops (`fs`/`net`/`exec`) take no extra arguments, unchanged.
#[cfg(target_os = "linux")]
fn run_linux(args: &[String]) {
    if args[1] == "threaded" {
        probe_threaded();
    }
    if let Err(e) = sandbox::apply_confinement() {
        eprintln!("[confine-probe] apply_confinement failed: {e}");
        std::process::exit(2);
    }
    let op = args[1].as_str();
    eprintln!("[confine-probe] confinement applied, attempting op: {op}");

    match op {
        "fs" => probe_fs(),
        "net" => probe_net(),
        "exec" => probe_exec(),
        "uring" => probe_uring(),
        "netlink" => probe_netlink(),
        "truncate" => probe_truncate(),
        "tiocsti" => probe_tiocsti(),
        "unix" => {
            if args.len() < 3 {
                eprintln!("Usage: confine-probe unix <socket-path>");
                std::process::exit(2);
            }
            probe_unix(&args[2]);
        }
        "smtp" => {
            if args.len() < 4 {
                eprintln!("Usage: confine-probe smtp <host> <port>");
                std::process::exit(2);
            }
            probe_smtp(&args[2], &args[3]);
        }
        other => {
            eprintln!("[confine-probe] unknown op: {other}");
            std::process::exit(2);
        }
    }
}

/// fs probe: attempt to open $HOME/.ssh/id_rsa.
///
/// With Landlock deny-all, filesystem path resolution is blocked before the
/// kernel checks whether the file exists — EACCES is returned even for
/// non-existent paths because the directory traversal itself is denied.
#[cfg(target_os = "linux")]
fn probe_fs() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let path = format!("{home}/.ssh/id_rsa");

    match std::fs::File::open(&path) {
        Err(e) => {
            let errno = e.raw_os_error().unwrap_or(0);
            if errno == libc::EACCES || errno == libc::EPERM {
                // Correctly blocked by Landlock
                eprintln!("[confine-probe] fs: correctly blocked (errno={errno}, path={path})");
                std::process::exit(0);
            } else if errno == libc::ENOENT {
                // Landlock not working: should have returned EACCES for the
                // directory traversal before the kernel reached ENOENT.
                // Fall back to /etc/passwd which is guaranteed to exist.
                probe_fs_etc_passwd();
            } else {
                eprintln!("[confine-probe] fs: unexpected error on {path}: {e} (errno={errno})");
                std::process::exit(2);
            }
        }
        Ok(_) => {
            eprintln!("[confine-probe] fs: open succeeded — Landlock not enforced!");
            std::process::exit(1);
        }
    }
}

/// Fallback fs probe: open /etc/passwd (guaranteed to exist on any Linux system).
/// Called when ~/.ssh/id_rsa was ENOENT instead of EACCES (which would indicate
/// Landlock is not enforced at the directory level).
#[cfg(target_os = "linux")]
fn probe_fs_etc_passwd() {
    match std::fs::File::open("/etc/passwd") {
        Err(e) => {
            let errno = e.raw_os_error().unwrap_or(0);
            if errno == libc::EACCES || errno == libc::EPERM {
                eprintln!("[confine-probe] fs: correctly blocked on /etc/passwd (errno={errno})");
                std::process::exit(0);
            } else {
                eprintln!(
                    "[confine-probe] fs: unexpected error on /etc/passwd: {e} (errno={errno})"
                );
                std::process::exit(2);
            }
        }
        Ok(_) => {
            eprintln!("[confine-probe] fs: open /etc/passwd succeeded — Landlock not enforced!");
            std::process::exit(1);
        }
    }
}

/// net probe: attempt socket(AF_INET, SOCK_STREAM, 0).
///
/// With seccomp deny for AF_INET/AF_INET6, this returns EPERM. Landlock does
/// not restrict socket creation (Landlock governs filesystem, not the socket
/// namespace), so only seccomp can produce EPERM here.
#[cfg(target_os = "linux")]
fn probe_net() {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };

    if fd == -1 {
        let errno = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(0);
        if errno == libc::EPERM || errno == libc::EACCES {
            eprintln!("[confine-probe] net: correctly blocked (errno={errno})");
            std::process::exit(0);
        } else {
            eprintln!(
                "[confine-probe] net: unexpected error: {} (errno={errno})",
                std::io::Error::last_os_error()
            );
            std::process::exit(2);
        }
    } else {
        // Close the socket we unexpectedly created
        unsafe { libc::close(fd) };
        eprintln!("[confine-probe] net: socket(AF_INET) succeeded — seccomp not enforced!");
        std::process::exit(1);
    }
}

/// smtp probe: attempt an actual TCP `connect()` to `host:port` (Pitfall 5
/// option (b) — explicit decision, see 13-RESEARCH.md).
///
/// `apply_confinement()` has already run when this is called (see
/// `run_linux`), so the seccomp filter installed by
/// `sandbox::seccomp::apply_worker_filter()` already denies
/// `socket(AF_INET, ...)`/`socket(AF_INET6, ...)` with `EPERM` — the same
/// mechanism `probe_net()` exercises. `TcpStream::connect` must call
/// `socket()` before it can call `connect()`, so the syscall-level denial
/// fires before any connection attempt reaches `host:port`. This op exists
/// as defense-in-depth: if the seccomp filter is ever loosened to allow
/// `socket()` but still deny outbound `connect()`, this probe would still
/// correctly detect and report the denial (or the regression) at the
/// `connect()` boundary instead of stopping at `socket()`.
///
/// Exit codes mirror the other probes: 0 = correctly blocked, 1 = confinement
/// regressed (connection unexpectedly succeeded), 2 = unexpected error.
#[cfg(target_os = "linux")]
fn probe_smtp(host: &str, port: &str) {
    let addr = format!("{host}:{port}");
    eprintln!("[confine-probe] smtp: attempting connect() to {addr}");

    match std::net::TcpStream::connect(&addr) {
        Err(e) => {
            let errno = e.raw_os_error().unwrap_or(0);
            if errno == libc::EPERM || errno == libc::EACCES {
                eprintln!("[confine-probe] smtp: correctly blocked (errno={errno}, addr={addr})");
                std::process::exit(0);
            } else {
                // Any other connect()-stage failure (e.g. ECONNREFUSED because
                // no listener is up) is NOT a proof of kernel-enforced denial —
                // it means socket() unexpectedly succeeded and only the
                // downstream connect() failed for an unrelated reason. Report
                // as an unexpected error rather than silently treating it as
                // "blocked" (that would mask a seccomp regression).
                eprintln!(
                    "[confine-probe] smtp: unexpected error connecting to {addr}: {e} (errno={errno})"
                );
                std::process::exit(2);
            }
        }
        Ok(stream) => {
            drop(stream);
            eprintln!(
                "[confine-probe] smtp: connect() to {addr} succeeded — seccomp not enforced!"
            );
            std::process::exit(1);
        }
    }
}

/// exec probe: attempt execve("/bin/true").
///
/// With seccomp deny for execve/execveat, this returns EPERM.
/// With Landlock deny-all, it may also return EACCES (file read blocked during
/// binary loading). Either blockage proves confinement is working.
///
/// If execve succeeds (it should not), the process image is replaced and this
/// code never reaches the lines after the call.
#[cfg(target_os = "linux")]
fn probe_exec() {
    use std::ffi::CString;

    let path = CString::new("/bin/true").expect("CString::new failed");
    // argv = ["/bin/true", NULL], envp = [NULL]
    let argv: [*const libc::c_char; 2] = [path.as_ptr(), std::ptr::null()];
    let envp: [*const libc::c_char; 1] = [std::ptr::null()];

    let ret = unsafe { libc::execve(path.as_ptr(), argv.as_ptr(), envp.as_ptr()) };

    // execve only returns if it failed
    debug_assert_eq!(ret, -1, "execve returned non-(-1)");

    let errno = std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(0);

    if errno == libc::EPERM || errno == libc::EACCES {
        // Correctly blocked by seccomp (EPERM) or Landlock (EACCES)
        eprintln!("[confine-probe] exec: correctly blocked (errno={errno})");
        std::process::exit(0);
    } else {
        eprintln!(
            "[confine-probe] exec: unexpected error: {} (errno={errno})",
            std::io::Error::last_os_error()
        );
        std::process::exit(2);
    }
}

/// Exit 0 iff `errno` is a confinement denial (EPERM/EACCES), else exit 2.
#[cfg(target_os = "linux")]
fn exit_on_denial(op: &str, errno: i32) -> ! {
    if errno == libc::EPERM || errno == libc::EACCES {
        eprintln!("[confine-probe] {op}: correctly blocked (errno={errno})");
        std::process::exit(0);
    }
    eprintln!("[confine-probe] {op}: unexpected error (errno={errno})");
    std::process::exit(2);
}

/// io_uring must be unavailable: IORING_OP_SOCKET/IORING_OP_CONNECT create and
/// use sockets without the `socket`/`connect` syscalls, so a seccomp rule on
/// `socket` alone is bypassable (DESIGN-confinement-egress-hardening D1).
#[cfg(target_os = "linux")]
fn probe_uring() {
    // struct io_uring_params is 120 bytes; all-zero is a valid request.
    let mut params = [0u8; 120];
    let fd = unsafe { libc::syscall(libc::SYS_io_uring_setup, 1u32, params.as_mut_ptr()) };
    if fd >= 0 {
        unsafe { libc::close(fd as i32) };
        eprintln!("[confine-probe] uring: io_uring_setup succeeded — socket filter bypassable!");
        std::process::exit(1);
    }
    exit_on_denial("uring", std::io::Error::last_os_error().raw_os_error().unwrap_or(0));
}

/// Non-INET socket families (here AF_NETLINK) must be denied (D3).
#[cfg(target_os = "linux")]
fn probe_netlink() {
    let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_RAW, libc::NETLINK_ROUTE) };
    if fd >= 0 {
        unsafe { libc::close(fd) };
        eprintln!("[confine-probe] netlink: socket(AF_NETLINK) succeeded — not confined!");
        std::process::exit(1);
    }
    exit_on_denial("netlink", std::io::Error::last_os_error().raw_os_error().unwrap_or(0));
}

/// A confined worker must not reach a pathname UDS outside its broker
/// connection (D2) — e.g. D-Bus, ssh-agent, docker.sock.
#[cfg(target_os = "linux")]
fn probe_unix(path: &str) {
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => {
            eprintln!("[confine-probe] unix: connect({path}) succeeded — not confined!");
            std::process::exit(1);
        }
        Err(e) => exit_on_denial("unix", e.raw_os_error().unwrap_or(0)),
    }
}

/// `truncate(path)` must be denied by seccomp (Landlock's Truncate right needs
/// kernel ≥6.2). A NONEXISTENT path isolates the seccomp layer: without the
/// rule the kernel answers ENOENT (exit 1); with it, EPERM (exit 0).
#[cfg(target_os = "linux")]
fn probe_truncate() {
    let path = std::ffi::CString::new("/nonexistent-caprun-truncate-probe").unwrap();
    if unsafe { libc::truncate(path.as_ptr(), 0) } == 0 {
        eprintln!("[confine-probe] truncate: succeeded — not confined!");
        std::process::exit(1);
    }
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    if errno == libc::EPERM {
        eprintln!("[confine-probe] truncate: correctly blocked by seccomp (EPERM)");
        std::process::exit(0);
    }
    eprintln!("[confine-probe] truncate: reached the kernel (errno={errno}) — not filtered!");
    std::process::exit(1);
}

/// `ioctl(_, TIOCSTI)` must be denied by seccomp. Issued on a pipe so the
/// kernel's own answer is ENOTTY (exit 1); the seccomp rule answers EPERM
/// (exit 0) before the fd type is examined.
#[cfg(target_os = "linux")]
fn probe_tiocsti() {
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        eprintln!("[confine-probe] tiocsti: pipe() failed");
        std::process::exit(2);
    }
    let c: libc::c_char = b'x' as libc::c_char;
    let r = unsafe { libc::ioctl(fds[0], libc::TIOCSTI, &c) };
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    if r == -1 && errno == libc::EPERM {
        eprintln!("[confine-probe] tiocsti: correctly blocked by seccomp (EPERM)");
        std::process::exit(0);
    }
    eprintln!("[confine-probe] tiocsti: reached the kernel (r={r}, errno={errno}) — not filtered!");
    std::process::exit(1);
}

/// With a second thread alive, `apply_confinement()` must REFUSE (Err): the
/// other thread would stay unconfined. Exit 0 = refused, 1 = confined anyway.
#[cfg(target_os = "linux")]
fn probe_threaded() -> ! {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let _t = std::thread::spawn(move || {
        let _ = rx.recv();
    });
    match sandbox::apply_confinement() {
        Err(e) => {
            eprintln!("[confine-probe] threaded: correctly refused ({e})");
            drop(tx);
            std::process::exit(0);
        }
        Ok(()) => {
            eprintln!("[confine-probe] threaded: confined with a live second thread — that thread is unconfined!");
            drop(tx);
            std::process::exit(1);
        }
    }
}
