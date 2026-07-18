/// landlock — Landlock LSM filesystem restriction
///
/// Restricts ALL filesystem access for the confined worker. No allow-rules
/// means everything is denied. Abstract-namespace UDS sockets are unaffected
/// (they do not touch the filesystem namespace).
///
/// Linux-only. On macOS this module is compiled as a no-op stub.

/// Deny all filesystem access via Landlock.
///
/// Uses ABI::V3 (Linux 5.19+); the `landlock` crate negotiates gracefully
/// with older kernels (ABI::V1 requires Linux ≥ 5.13).
/// No allow-rules are added → everything is denied.
///
/// Returns `std::io::Result<()>` for `Command::pre_exec` compatibility.
#[cfg(target_os = "linux")]
pub fn deny_all_filesystem() -> std::io::Result<()> {
    use landlock::{Access, AccessFs, ABI, Ruleset, RulesetAttr, RulesetCreatedAttr};

    let abi = ABI::V3;
    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .create()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        // No rules added → everything denied
        .restrict_self()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    eprintln!("[sandbox] Landlock status: {:?}", status.ruleset);
    Ok(())
}

/// No-op stub on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn deny_all_filesystem() -> std::io::Result<()> {
    Ok(())
}

/// Narrow-allow-list Landlock ruleset for a broker-spawned `process.exec`
/// child (the `caprun-exec-launcher`, applied to itself post-fork, pre-exec —
/// DESIGN §1.3 Option B).
///
/// Unlike `deny_all_filesystem()` (zero allow-rules — everything denied),
/// this ruleset grants exactly three carve-outs so an arbitrary target binary
/// can actually load and run:
///   - `ReadFile + Execute` on an enumerated, hardcoded system-path allow-list
///     (loading + running the target binary and its shared libraries).
///   - `ReadFile + WriteFile + …` (the fuller create/list/remove/rename set) on
///     `workspace_root` ONLY — no `Execute` there, so a worker-planted binary
///     inside the workspace can never be run.
///   - `ReadFile + WriteFile` on the single device file `/dev/null` (the
///     canonical bit-bucket) — see the git.commit neutralization note below.
///
/// The system path list (`/usr`, `/bin`, `/lib`, `/lib64`) is an enumerated,
/// explicitly-hardcoded allow-list, NEVER a PATH walk or directory scan — per
/// the "no dynamic registry" discipline. DESIGN §8 item 1 defers the EXACT
/// strings to in-container verification (32-06 confirms via `ldd`/`which`
/// against the real verification container layout); this list is the
/// starting point, not a final deployment constant.
///
/// Do NOT reuse `deny_all_filesystem()` here — its zero-allow-rule ruleset
/// would block the target binary from loading (EACCES/ENOEXEC). This is a
/// deliberately distinct, narrower-than-open ruleset (T-32-08).
#[cfg(target_os = "linux")]
pub fn exec_child_ruleset(workspace_root: &std::path::Path) -> std::io::Result<()> {
    use landlock::{
        path_beneath_rules, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
    };

    let abi = ABI::V3;

    // System paths: ReadFile + ReadDir + Execute (loading + running the
    // target binary and its shared libs). Exact literal path list is an Open
    // Item (DESIGN §8 item 1) — resolved against the verification
    // container's real layout at 32-06.
    //
    // ReadDir (32-06 fix): the original ReadFile+Execute-only grant compiled
    // fine but FAILED at runtime for any target more complex than a trivial
    // single-binary (e.g. `/bin/echo`) — empirically discovered running
    // `/usr/bin/python3` through the real launcher in the mandatory Linux
    // container: CPython's own stdlib import bootstrap (`Fatal Python
    // error: Failed to import encodings module`) needs to enumerate
    // directory entries under its stdlib tree (`getdents`/`readdir`, a
    // DISTINCT Landlock right from `ReadFile`, which only gates `open()` for
    // reading a file's CONTENTS) while resolving `sys.path` module/package
    // candidates. Without `ReadDir`, the OS-level directory-listing syscalls
    // Landlock intercepts return EACCES, well before any individual
    // `encodings/*.py` file is opened. `/bin/echo` never hit this because it
    // needs no runtime directory enumeration — only a handful of shared libs
    // resolved by their EXACT dlopen path (no scan). `ReadDir` grants
    // directory-listing only — it does NOT grant write/create/delete rights
    // on these system paths (still absent here), so this remains
    // read+list+execute-only, never writable.
    let system_paths = ["/usr", "/bin", "/lib", "/lib64"];
    let system_access = AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::Execute;

    // Device carve-outs (Phase 36 git.commit fix) — the ONLY paths outside the
    // system-lib + workspace allow-list. Each is a SINGLE, universally-safe
    // device FILE (never the `/dev` directory, never `ReadDir`, never `MakeReg`/
    // `Execute`), so nothing can be created, listed, or run under `/dev` — no
    // escape surface. process.exec targets that never touch these are
    // unaffected (a granted-but-unused path changes nothing). Both grants were
    // empirically required, discovered by running `git commit` through the real
    // launcher in the mandatory Linux container (cfg-linux-test-blindness — this
    // path never ran on Mac):
    //
    //   * `/dev/null` (r/w): the git.commit neutralization env
    //     (`GIT_CONFIG_GLOBAL=/dev/null` — the documented way to strip global
    //     config; `-c core.hooksPath=/dev/null`) makes `git` OPEN `/dev/null`
    //     O_RDWR. Without it: `fatal: could not open '/dev/null' for reading and
    //     writing: Permission denied` (EACCES), before any commit.
    //   * `/dev/urandom` + `/dev/random` (read-only): `git`'s CSPRNG reads an
    //     entropy device to generate the random temp-object filename
    //     (`.git/objects/tmp_obj_*`) before renaming it into place. seccomp
    //     ALLOWS the `getrandom(2)` syscall (mismatch-Allow), so this is purely
    //     the Landlock `/dev/urandom` open being denied. Without it: `error:
    //     unable to get random bytes for temporary file: Permission denied` ->
    //     `insufficient permission for adding an object to repository database
    //     .git/objects`. Read-only entropy sources: no write, no escape.
    let devnull_paths = ["/dev/null"];
    let devnull_access = AccessFs::ReadFile | AccessFs::WriteFile;
    let devrandom_paths = ["/dev/urandom", "/dev/random"];
    let devrandom_access = AccessFs::ReadFile;

    // Workspace: the fuller write/list/create/remove/rename right set — still
    // NO Execute (never run a worker-planted binary), still scoped to
    // `workspace_root` ONLY. The worker already may freely mutate its own
    // workspace, so granting the full non-exec filesystem right set WITHIN it
    // widens nothing about the security boundary (no escape, no execute, net
    // stays default-deny via the separate seccomp filter).
    //
    // MakeReg (32-06 fix): `WriteFile` alone governs opening/truncating an
    // EXISTING file; Landlock gates CREATING a brand-new file via the
    // DISTINCT `MakeReg` right on the PARENT directory. Without it, any
    // target that writes a NEW file under the workspace (the common case —
    // `open(path, 'w')` on a path that doesn't yet exist) fails with EACCES
    // even though `WriteFile` is granted — empirically discovered running a
    // benign in-workspace write through the real launcher in the mandatory
    // Linux container.
    //
    // ReadDir + MakeDir + RemoveFile + RemoveDir + Refer + Truncate (Phase 36
    // git.commit fix): `git commit` writing into `.git/` under the workspace
    // needs more than create-a-regular-file. It ENUMERATES object/ref
    // directories (`ReadDir`), CREATES new fan-out object dirs like
    // `.git/objects/xx/` and the `.git/logs/` reflog tree (`MakeDir`), writes
    // and then REMOVES lock files (`index.lock`, `*.lock`; `RemoveFile`), and
    // atomically RENAMES those locks over their targets (`Refer` for any
    // cross-directory reparent; a same-directory rename needs no `Refer`, so
    // the index/ref lock renames work even on the ABI floor). `RemoveDir` +
    // `Truncate` are included to complete the fuller write set (git tooling may
    // prune empty dirs / truncate in place). ABI note: `Refer` is Landlock ABI
    // V2 (kernel ≥5.19) and `Truncate` is ABI V3 (≥5.19); the `landlock`
    // crate's best-effort compatibility (default for `Ruleset::default()`)
    // strips the unsupported bits on a ≥5.13 / <5.19 kernel WITHOUT failing, so
    // the 5.13 floor is preserved — and because git's lock renames are
    // same-directory, a commit still succeeds there without `Refer`.
    let workspace_access = AccessFs::ReadFile
        | AccessFs::WriteFile
        | AccessFs::ReadDir
        | AccessFs::MakeReg
        | AccessFs::MakeDir
        | AccessFs::RemoveFile
        | AccessFs::RemoveDir
        | AccessFs::Refer
        | AccessFs::Truncate;

    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .create()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .add_rules(path_beneath_rules(system_paths.iter(), system_access))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        // `/dev/null` r/w + `/dev/urandom`,`/dev/random` read — the only paths
        // outside the system-lib + workspace allow-list (git.commit opens
        // `/dev/null` for config neutralization and reads an entropy device for
        // temp-object filenames). Single device files, no MakeReg/Execute/ReadDir
        // → no escape surface.
        .add_rules(path_beneath_rules(devnull_paths.iter(), devnull_access))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .add_rules(path_beneath_rules(devrandom_paths.iter(), devrandom_access))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        // `path_beneath_rules` takes path-LIKE items (`P: AsRef<Path>`) and
        // resolves each to a `PathFd` INTERNALLY (crates.io landlock 0.4.5,
        // `fs.rs`) — it does NOT accept an already-constructed `PathFd`
        // (`PathFd` itself does not implement `AsRef<Path>`). Passing
        // `workspace_root` (a `&Path`) directly, rather than pre-resolving it
        // via `PathFd::new(..)`, is the correct call shape; this bug never
        // compiled before this Linux container run (`#[cfg(target_os =
        // "linux")]` — Mac only exercises the no-op stub below, per
        // cfg-linux-test-blindness) — genuine E0277 `AsRef<Path>` compile
        // error, fixed here (32-06 Task 3, out-of-scope-file Rule 1 fix).
        .add_rules(path_beneath_rules(
            std::iter::once(workspace_root),
            workspace_access,
        ))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .restrict_self()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    eprintln!(
        "[caprun-exec-launcher] Landlock exec_child_ruleset status: {:?}",
        status.ruleset
    );
    Ok(())
}

/// No-op stub on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn exec_child_ruleset(_workspace_root: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}
