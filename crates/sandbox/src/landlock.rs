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
/// this ruleset grants exactly two carve-outs so an arbitrary target binary
/// can actually load and run:
///   - `ReadFile + Execute` on an enumerated, hardcoded system-path allow-list
///     (loading + running the target binary and its shared libraries).
///   - `ReadFile + WriteFile` on `workspace_root` ONLY — no `Execute` there,
///     so a worker-planted binary inside the workspace can never be run.
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

    // System paths: ReadFile + Execute only (loading + running the target
    // binary and its shared libs). Exact literal path list is an Open Item
    // (DESIGN §8 item 1) — resolved against the verification container's
    // real layout at 32-06.
    let system_paths = ["/usr", "/bin", "/lib", "/lib64"];
    let system_access = AccessFs::ReadFile | AccessFs::Execute;

    // Workspace: ReadFile + WriteFile only — no Execute (never run a
    // worker-planted binary), matching "narrowest that works."
    let workspace_access = AccessFs::ReadFile | AccessFs::WriteFile;

    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .create()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .add_rules(path_beneath_rules(system_paths.iter(), system_access))
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
