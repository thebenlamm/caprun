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
            eprintln!("Usage: confine-probe <fs|net|exec>");
            std::process::exit(2);
        }
        run_linux(&args[1]);
    }
}

/// Linux implementation: apply confinement, then attempt the forbidden op.
#[cfg(target_os = "linux")]
fn run_linux(op: &str) {
    if let Err(e) = sandbox::apply_confinement() {
        eprintln!("[confine-probe] apply_confinement failed: {e}");
        std::process::exit(2);
    }
    eprintln!("[confine-probe] confinement applied, attempting op: {op}");

    match op {
        "fs" => probe_fs(),
        "net" => probe_net(),
        "exec" => probe_exec(),
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
