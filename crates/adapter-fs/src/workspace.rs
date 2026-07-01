/// workspace — dirfd-anchored workspace-root capability (HARD-04 read side)
///
/// The broker must never open a worker-supplied path via ambient
/// `std::fs::File::open`: the `RequestFd { path }` string is fully controlled
/// by the confined worker, so a compromised/injected worker could otherwise
/// request any broker-openable absolute path (the HARD-04 vulnerability).
///
/// `WorkspaceRoot` holds an anchor directory fd (`OwnedFd`), opened ONCE by the
/// broker in `main()` against a root the broker legitimately has ambient access
/// to. Every subsequent read resolves BENEATH that anchor via a single
/// `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` syscall — resolution and
/// open are atomic (TOCTOU-safe, CWE-367), and absolute paths, `..` traversal,
/// and symlink escapes are rejected at kernel resolution time (not string
/// filtered).
///
/// Linux-only enforcement. On macOS (dev machine) `read_within` is a cfg-gated
/// stub with NO security claim, mirroring `sandbox::landlock::deny_all_filesystem`
/// so the crate still compiles (CLAUDE.md: all v0 security claims are Linux-only).
///
/// This module is the READ side (HARD-04). The write side
/// (`create_exclusive_within`, SINK-04) is deliberately out of scope here and is
/// added in a later plan.
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

/// A directory-fd anchor for the workspace root.
///
/// Constructed once by the broker; all `RequestFd` reads resolve beneath it.
pub struct WorkspaceRoot {
    /// Anchor dirfd — the resolution root for `openat2`. Used on Linux;
    /// held (and thus kept open) on all platforms.
    #[allow(dead_code)]
    dirfd: OwnedFd,
    /// Root path — used ONLY by the non-Linux stub to reconstruct the join.
    #[allow(dead_code)]
    root_path: PathBuf,
}

impl WorkspaceRoot {
    /// Open the workspace-root anchor dirfd.
    ///
    /// Uses a plain `open(O_DIRECTORY | O_RDONLY)` — NOT `openat2` — because the
    /// broker legitimately has ambient access to the root; this single call
    /// establishes the anchor that all subsequent `read_within` calls resolve
    /// beneath. The `nix` error is mapped to `std::io::Error`.
    ///
    /// # Errors
    /// Returns an `std::io::Error` if the root cannot be opened as a directory.
    pub fn open(root: &Path) -> std::io::Result<Self> {
        use nix::fcntl::{open, OFlag};
        use nix::sys::stat::Mode;

        let dirfd = open(root, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())
            .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

        Ok(Self {
            dirfd,
            root_path: root.to_path_buf(),
        })
    }

    /// Read a file resolved BENEATH the workspace-root anchor (Linux).
    ///
    /// Resolves and opens `rel_path` in a single `openat2` syscall with
    /// `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`. Both flags are required:
    /// `RESOLVE_BENEATH` rejects absolute paths and `..` traversal that would
    /// escape the tree (surfacing as `EXDEV`), while `RESOLVE_NO_SYMLINKS`
    /// disallows ALL symlink resolution — `RESOLVE_BENEATH` alone does not block
    /// in-tree symlink traversal (RESEARCH Q1 caveat / Pitfall 2). Resolution
    /// and open are atomic (no TOCTOU window).
    ///
    /// # Errors
    /// Returns an `std::io::Error` if resolution violates the `RESOLVE_*`
    /// constraints or the file cannot be opened. A path-escape violation
    /// surfaces as raw OS error `EXDEV`.
    #[cfg(target_os = "linux")]
    pub fn read_within(&self, rel_path: &str) -> std::io::Result<std::fs::File> {
        use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
        use std::os::fd::AsFd;

        let how = OpenHow::new()
            .flags(OFlag::O_RDONLY)
            .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);

        let fd = openat2(self.dirfd.as_fd(), rel_path, how)
            .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

        Ok(std::fs::File::from(fd))
    }

    /// Non-Linux stub — NO security claim (dev-machine compilation only).
    ///
    /// Mirrors `sandbox::landlock::deny_all_filesystem`'s cfg-gated stub: exists
    /// solely so the crate builds on macOS (CLAUDE.md). This performs an ordinary
    /// path join + open with none of the `openat2` RESOLVE_* guarantees.
    #[cfg(not(target_os = "linux"))]
    pub fn read_within(&self, rel_path: &str) -> std::io::Result<std::fs::File> {
        std::fs::File::open(self.root_path.join(rel_path))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;

    /// Create a uniquely-named temp subdir (no `tempfile` dev-dep in this crate).
    #[cfg(target_os = "linux")]
    fn unique_tmp_root(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!("caprun_ws_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&d).expect("create tmp root");
        d
    }

    /// A legit in-root relative read returns Ok and yields the file bytes.
    #[cfg(target_os = "linux")]
    #[test]
    fn legit_relative_read_ok() {
        use std::io::Read;
        let root = unique_tmp_root("legit");
        let content = b"in-root read via openat2";
        std::fs::write(root.join("hello.txt"), content).unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let mut f = ws.read_within("hello.txt").expect("legit relative read must succeed");
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, content, "read must yield the in-root file bytes");

        std::fs::remove_dir_all(&root).ok();
    }

    /// An absolute path arg is rejected by RESOLVE_BENEATH (raw OS error EXDEV).
    #[cfg(target_os = "linux")]
    #[test]
    fn absolute_path_rejected() {
        let root = unique_tmp_root("abs");
        let ws = WorkspaceRoot::open(&root).unwrap();

        let err = ws
            .read_within("/etc/hostname")
            .expect_err("absolute path must be rejected");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::EXDEV),
            "RESOLVE_BENEATH must reject absolute paths with EXDEV"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `../` traversal escaping the root is rejected by RESOLVE_BENEATH.
    #[cfg(target_os = "linux")]
    #[test]
    fn parent_traversal_rejected() {
        let root = unique_tmp_root("dotdot");
        // Place a file in the root's PARENT, then try to reach it via `../`.
        let parent = root.parent().expect("temp root has a parent");
        let secret_name = format!("caprun_secret_{}.txt", std::process::id());
        let secret = parent.join(&secret_name);
        std::fs::write(&secret, b"outside the root").unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let err = ws
            .read_within(&format!("../{secret_name}"))
            .expect_err("`..` traversal must be rejected");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::EXDEV),
            "RESOLVE_BENEATH must reject `..` escape with EXDEV"
        );

        std::fs::remove_file(&secret).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    /// An in-root symlink pointing OUTSIDE the root is rejected at resolution
    /// (RESOLVE_NO_SYMLINKS) — the read must fail, not follow-then-block.
    #[cfg(target_os = "linux")]
    #[test]
    fn symlink_escape_rejected() {
        let root = unique_tmp_root("symlink");
        // Target lives outside the root.
        let parent = root.parent().expect("temp root has a parent");
        let target_name = format!("caprun_symtarget_{}.txt", std::process::id());
        let target = parent.join(&target_name);
        std::fs::write(&target, b"sensitive outside file").unwrap();

        // In-root symlink → outside target.
        std::os::unix::fs::symlink(&target, root.join("escape")).unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let res = ws.read_within("escape");
        assert!(
            res.is_err(),
            "RESOLVE_NO_SYMLINKS must reject symlink traversal at resolution"
        );

        std::fs::remove_file(&target).ok();
        std::fs::remove_dir_all(&root).ok();
    }
}
