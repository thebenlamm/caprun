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
    /// Root path — used by the non-Linux stub to reconstruct the join, and
    /// exposed via `root_path()` so a `BlockedPendingConfirmation` snapshot can
    /// persist the workspace root a later `caprun confirm` process must reopen
    /// (RESEARCH Assumption A2).
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

    /// The workspace-root directory path the broker opened.
    ///
    /// Platform-independent: `root_path` is populated unconditionally in
    /// `open` on both Linux and non-Linux paths. Persisted at Block time into
    /// `PendingConfirmation.workspace_root_path` so a later, separate
    /// `caprun confirm` process can re-`open` the same root and re-invoke the
    /// sink (RESEARCH Assumption A2).
    pub fn root_path(&self) -> &Path {
        &self.root_path
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

    /// Exclusively CREATE a file resolved BENEATH the workspace-root anchor and
    /// write `contents` into it (Linux; write side of SINK-03/SINK-04).
    ///
    /// Resolves and creates `rel_path` in a single `openat2` syscall with
    /// `O_CREAT | O_EXCL | O_WRONLY` and `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`:
    /// - `O_EXCL` — never overwrites; a create on an EXISTING path fails with
    ///   `EEXIST` (SINK-03: no clobber). This is mandatory.
    /// - `RESOLVE_BENEATH` — rejects absolute paths and `..` escape (`EXDEV`).
    /// - `RESOLVE_NO_SYMLINKS` — rejects ALL symlink traversal (`RESOLVE_BENEATH`
    ///   alone does not; RESEARCH Pitfall 2).
    ///
    /// Resolution + exclusive create happen in ONE syscall — there is no
    /// validate-then-create window (SINK-04, TOCTOU-safe, CWE-367). After the fd
    /// is obtained the bytes are written and `fsync`'d before close.
    ///
    /// # Errors
    /// Returns an `std::io::Error`: `EEXIST` if the path already exists, `EXDEV`
    /// (or other raw OS error) for a `RESOLVE_*` violation, or a write error.
    #[cfg(target_os = "linux")]
    pub fn create_exclusive_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
        use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
        use nix::sys::stat::Mode;
        use std::io::Write;
        use std::os::fd::AsFd;

        let how = OpenHow::new()
            .flags(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY)
            .mode(Mode::S_IRUSR | Mode::S_IWUSR) // 0o600
            .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);

        let fd = openat2(self.dirfd.as_fd(), rel_path, how)
            .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

        let mut file = std::fs::File::from(fd);
        file.write_all(contents)?;
        file.sync_all()?;
        Ok(())
    }

    /// Non-Linux stub — NO security claim (dev-machine compilation only).
    ///
    /// Mirrors `read_within`'s stub: an ordinary `create_new` open + write with
    /// none of the `openat2` RESOLVE_* guarantees, so the crate builds on macOS.
    /// `create_new(true)` keeps the no-overwrite semantic locally.
    #[cfg(not(target_os = "linux"))]
    pub fn create_exclusive_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root_path.join(rel_path))?;
        file.write_all(contents)?;
        Ok(())
    }

    /// Write/edit an EXISTING file resolved BENEATH the workspace-root anchor
    /// (Linux; write side of FS-02 — the `file.write` sink's existing-file-only
    /// sibling to `create_exclusive_within`'s new-file-only authority).
    ///
    /// Resolves and opens `rel_path` in a single `openat2` syscall with
    /// `O_WRONLY | O_TRUNC` and `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`:
    /// - `O_WRONLY | O_TRUNC`, deliberately **no `O_CREAT`, no `O_EXCL`** — a
    ///   missing target path fails closed with `ENOENT` rather than silently
    ///   creating the file. This is the semantic split from
    ///   `create_exclusive_within`: that method is new-file-only (`EEXIST` on
    ///   an existing path); this method is existing-file-only (`ENOENT` on a
    ///   missing path). The two authorities are deliberately non-overlapping
    ///   (DESIGN §3.2) — `write_within` MUST NOT gain create authority.
    /// - `RESOLVE_BENEATH` — rejects absolute paths and `..` escape (`EXDEV`).
    /// - `RESOLVE_NO_SYMLINKS` — rejects ALL symlink traversal (`RESOLVE_BENEATH`
    ///   alone does not; RESEARCH Pitfall 2).
    ///
    /// Resolution + open happen in ONE syscall — there is no
    /// validate-then-open window (TOCTOU-safe, CWE-367). After the fd is
    /// obtained the bytes are written and `fsync`'d before close.
    ///
    /// # Errors
    /// Returns an `std::io::Error`: `ENOENT` if the target does not exist,
    /// `EXDEV` (or other raw OS error) for a `RESOLVE_*` violation, or a
    /// write error.
    #[cfg(target_os = "linux")]
    pub fn write_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
        use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
        use std::io::Write;
        use std::os::fd::AsFd;

        let how = OpenHow::new()
            .flags(OFlag::O_WRONLY | OFlag::O_TRUNC)
            .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);

        let fd = openat2(self.dirfd.as_fd(), rel_path, how)
            .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

        let mut file = std::fs::File::from(fd);
        file.write_all(contents)?;
        file.sync_all()?;
        Ok(())
    }

    /// Non-Linux stub — NO security claim (dev-machine compilation only).
    ///
    /// Mirrors `create_exclusive_within`'s stub shape, but deliberately omits
    /// `.create(true)`/`.create_new(true)` so a missing target still errors on
    /// macOS too (ENOENT-contract parity with the Linux impl; this stub
    /// carries no security claim — Linux is the only enforced path).
    #[cfg(not(target_os = "linux"))]
    pub fn write_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.root_path.join(rel_path))?;
        file.write_all(contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;

    /// `root_path()` returns the exact path passed to `open` — platform-
    /// independent (runs on macOS too), since `root_path` is populated
    /// unconditionally on both the Linux and non-Linux paths.
    #[test]
    fn root_path_returns_the_opened_root() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_ws_root_path_{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create tmp root");

        let ws = super::WorkspaceRoot::open(&root).expect("open workspace root");
        assert_eq!(ws.root_path(), root.as_path());

        std::fs::remove_dir_all(&root).ok();
    }

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

    // ── create_exclusive_within (write side, SINK-03/SINK-04) ────────────────

    /// A legit in-root exclusive create writes the file with the expected bytes.
    #[cfg(target_os = "linux")]
    #[test]
    fn create_exclusive_writes_file() {
        let root = unique_tmp_root("create_ok");
        let contents = b"exclusive create via openat2";

        let ws = WorkspaceRoot::open(&root).unwrap();
        ws.create_exclusive_within("out.txt", contents)
            .expect("exclusive create under root must succeed");

        let on_disk = std::fs::read(root.join("out.txt")).unwrap();
        assert_eq!(on_disk, contents, "written bytes must match");

        std::fs::remove_dir_all(&root).ok();
    }

    /// A create on an EXISTING path fails (O_EXCL → EEXIST) — never overwrites.
    #[cfg(target_os = "linux")]
    #[test]
    fn create_exclusive_existing_path_rejected() {
        let root = unique_tmp_root("create_excl");
        std::fs::write(root.join("dup.txt"), b"original").unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let err = ws
            .create_exclusive_within("dup.txt", b"clobber")
            .expect_err("O_EXCL must reject an existing path");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::EEXIST),
            "existing path must fail with EEXIST (no overwrite)"
        );
        // Original bytes untouched.
        assert_eq!(std::fs::read(root.join("dup.txt")).unwrap(), b"original");

        std::fs::remove_dir_all(&root).ok();
    }

    /// An absolute path arg is rejected by RESOLVE_BENEATH (EXDEV).
    #[cfg(target_os = "linux")]
    #[test]
    fn create_exclusive_absolute_path_rejected() {
        let root = unique_tmp_root("create_abs");
        let ws = WorkspaceRoot::open(&root).unwrap();

        let err = ws
            .create_exclusive_within("/tmp/caprun_should_not_exist", b"x")
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
    fn create_exclusive_parent_traversal_rejected() {
        let root = unique_tmp_root("create_dotdot");
        let target_name = format!("caprun_create_escape_{}.txt", std::process::id());

        let ws = WorkspaceRoot::open(&root).unwrap();
        let err = ws
            .create_exclusive_within(&format!("../{target_name}"), b"x")
            .expect_err("`..` traversal must be rejected");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::EXDEV),
            "RESOLVE_BENEATH must reject `..` escape with EXDEV"
        );

        // Ensure nothing was written to the parent.
        let parent = root.parent().unwrap();
        assert!(!parent.join(&target_name).exists(), "no file must escape the root");

        std::fs::remove_dir_all(&root).ok();
    }

    /// An in-root symlink to a DIRECTORY outside the root must not let a create
    /// escape — RESOLVE_NO_SYMLINKS rejects symlink traversal at resolution.
    #[cfg(target_os = "linux")]
    #[test]
    fn create_exclusive_symlink_escape_rejected() {
        let root = unique_tmp_root("create_symlink");
        let parent = root.parent().expect("temp root has a parent");
        let outside_dir_name = format!("caprun_create_outdir_{}", std::process::id());
        let outside_dir = parent.join(&outside_dir_name);
        std::fs::create_dir_all(&outside_dir).unwrap();

        // In-root symlink → outside directory; try to create a file "through" it.
        std::os::unix::fs::symlink(&outside_dir, root.join("escape_dir")).unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let res = ws.create_exclusive_within("escape_dir/planted.txt", b"x");
        assert!(
            res.is_err(),
            "RESOLVE_NO_SYMLINKS must reject symlink traversal at resolution"
        );
        assert!(
            !outside_dir.join("planted.txt").exists(),
            "no file must be planted outside the root via a symlink"
        );

        std::fs::remove_dir_all(&outside_dir).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    // ── write_within (write side, FS-02) ─────────────────────────────────
    //
    // NOT-inherited negative test set for the O_WRONLY|O_TRUNC flag
    // combination (DESIGN §3.2: equivalent negative tests are NOT assumed
    // inherited from read_within's O_RDONLY or create_exclusive_within's
    // O_CREAT|O_EXCL|O_WRONLY coverage).

    /// A legit in-root write overwrites an EXISTING file's bytes exactly
    /// (truncation semantics — no leftover trailing bytes from the original).
    #[cfg(target_os = "linux")]
    #[test]
    fn write_within_overwrites_existing() {
        let root = unique_tmp_root("write_ok");
        std::fs::write(root.join("edit.txt"), b"original longer content").unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        ws.write_within("edit.txt", b"new")
            .expect("write to an existing in-root file must succeed");

        let on_disk = std::fs::read(root.join("edit.txt")).unwrap();
        assert_eq!(on_disk, b"new", "O_TRUNC must replace, not append/leave trailing bytes");

        std::fs::remove_dir_all(&root).ok();
    }

    /// write_within on a rel_path that does NOT exist fails closed with
    /// ENOENT — proving no O_CREAT path exists (the existing-file-only
    /// contract). This is the genuinely-new test with no analog.
    #[cfg(target_os = "linux")]
    #[test]
    fn write_within_missing_target_enoent() {
        let root = unique_tmp_root("write_enoent");
        let ws = WorkspaceRoot::open(&root).unwrap();

        let err = ws
            .write_within("does_not_exist.txt", b"x")
            .expect_err("write to a missing target must fail closed");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::ENOENT),
            "write_within must never silently create a missing target — ENOENT proves no O_CREAT path"
        );
        assert!(
            !root.join("does_not_exist.txt").exists(),
            "no file must be created as a side effect of the failed write"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// An absolute path arg is rejected by RESOLVE_BENEATH (EXDEV).
    #[cfg(target_os = "linux")]
    #[test]
    fn write_within_absolute_path_rejected() {
        let root = unique_tmp_root("write_abs");
        let ws = WorkspaceRoot::open(&root).unwrap();

        let err = ws
            .write_within("/tmp/caprun_should_not_exist", b"x")
            .expect_err("absolute path must be rejected");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::EXDEV),
            "RESOLVE_BENEATH must reject absolute paths with EXDEV"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `../` traversal escaping the root is rejected by RESOLVE_BENEATH,
    /// even when the target actually exists outside the root.
    #[cfg(target_os = "linux")]
    #[test]
    fn write_within_parent_traversal_rejected() {
        let root = unique_tmp_root("write_dotdot");
        let parent = root.parent().expect("temp root has a parent");
        let target_name = format!("caprun_write_escape_{}.txt", std::process::id());
        let target = parent.join(&target_name);
        std::fs::write(&target, b"outside the root").unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let err = ws
            .write_within(&format!("../{target_name}"), b"clobber")
            .expect_err("`..` traversal must be rejected");
        assert_eq!(
            err.raw_os_error(),
            Some(nix::libc::EXDEV),
            "RESOLVE_BENEATH must reject `..` escape with EXDEV"
        );
        // Original bytes untouched.
        assert_eq!(std::fs::read(&target).unwrap(), b"outside the root");

        std::fs::remove_file(&target).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    /// An in-root symlink pointing OUTSIDE the root is rejected at resolution
    /// (RESOLVE_NO_SYMLINKS) — the write must fail, not follow-then-write.
    #[cfg(target_os = "linux")]
    #[test]
    fn write_within_symlink_escape_rejected() {
        let root = unique_tmp_root("write_symlink");
        let parent = root.parent().expect("temp root has a parent");
        let target_name = format!("caprun_write_symtarget_{}.txt", std::process::id());
        let target = parent.join(&target_name);
        std::fs::write(&target, b"sensitive outside file").unwrap();

        // In-root symlink → outside target.
        std::os::unix::fs::symlink(&target, root.join("escape")).unwrap();

        let ws = WorkspaceRoot::open(&root).unwrap();
        let res = ws.write_within("escape", b"clobber");
        assert!(
            res.is_err(),
            "RESOLVE_NO_SYMLINKS must reject symlink traversal at resolution"
        );
        // Original bytes untouched.
        assert_eq!(std::fs::read(&target).unwrap(), b"sensitive outside file");

        std::fs::remove_file(&target).ok();
        std::fs::remove_dir_all(&root).ok();
    }
}
