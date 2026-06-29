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
