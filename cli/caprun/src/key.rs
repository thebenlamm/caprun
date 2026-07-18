/// key — cross-process MAC-key custody + F1 fail-closed startup refusal
/// (HARDEN-02, `planning-docs/DESIGN-security-hardening.md` §b post-F1-amendment).
///
/// `caprun` is single-shot-per-session: `caprun run` and the later, separate
/// `caprun confirm`/`deny` OS process must agree on the SAME secret MAC key
/// with no persistent daemon in between (DESIGN §b KEY-CUSTODY-ACROSS-PROCESSES,
/// RESEARCH Assumption A2, the #1 named risk of this phase). This module is the
/// single custody + refusal choke point both call sites (Plan 03's `caprun run`
/// path, Plan 05's `caprun confirm`/`deny` path) will consume — a per-process
/// fresh key would silently break every legitimate `confirm()`'s `verify_chain`
/// gate (Pitfall 2).
///
/// # F1 (DESIGN-GATE-RECORD-v1.6 Round 1 BLOCKER, corrected pin)
///
/// The audit DB path is a free-form CLI argument, wholly independent of the
/// workspace root. If an operator co-locates the audit DB (and therefore its
/// `.key` sibling) beneath the workspace root, the confined worker — caprun's
/// PRIMARY adversary — can `RequestFd` the key file via the SAME
/// `WorkspaceRoot::read_within` (`RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`) reach
/// the broker already grants for legitimate reads, receive the fd via
/// `SCM_RIGHTS`, read the MAC key, and forge/truncate the chain. `F1` mirrors
/// that SAME containment check at key-custody time: canonicalize the workspace
/// root and BOTH candidate paths (audit DB, `.key` sibling) to absolute forms
/// and refuse — hard `Err`, no key returned, no file written — if either is
/// equal to or a descendant of the workspace root. An unresolvable/absent path
/// is fail-closed (refuse), never fail-open.
///
/// This module defines the helper + its unit tests ONLY — it is NOT yet wired
/// into the runtime `open_audit_db` flow (deferred to Plan 03/05 per this
/// plan's `<objective>`).
use anyhow::Context;
use std::path::{Path, PathBuf};

/// MAC key length in bytes (HMAC-SHA256 wants a 256-bit key for full-strength
/// security — `Hmac::new_from_slice` accepts any length, but 32 bytes is the
/// DESIGN doc's pinned target, RESEARCH's rejection of reusing `Uuid::new_v4`'s
/// 122 bits of entropy).
const KEY_LEN: usize = 32;

/// Load the existing cross-process MAC key for `audit_path`, or create it on
/// first call.
///
/// - `audit_path == ":memory:"`: returns a fresh ephemeral `getrandom`-backed
///   key, writes no file, runs no F1 containment check (an in-memory DB has no
///   persisted row for any later process to verify against anyway — mirrors
///   the existing `main.rs` ":memory: fails closed" rationale).
/// - Otherwise: the key path is `<audit_path>.key`. F1 refusal runs FIRST — if
///   the canonical resolved form of `audit_path` OR the canonical resolved
///   form of the key path is equal to or beneath the canonical `workspace_root`,
///   this returns `Err` and writes nothing.
/// - If the key file already exists, its bytes are read back and returned
///   (idempotent — the load-bearing cross-process-stability guarantee: a
///   later, separate `caprun confirm`/`deny` process must derive the SAME key).
/// - Otherwise a fresh 32-byte key is generated via `getrandom::fill` (a
///   vetted CSPRNG, never a custom PRNG) and persisted with `0600` permissions
///   before being returned.
///
/// The key bytes are never logged and never written into any audit payload —
/// callers must keep them out of `Event`/`ValueNode` construction.
pub(crate) fn load_or_create_key(
    audit_path: &str,
    workspace_root: &Path,
) -> anyhow::Result<Vec<u8>> {
    if audit_path == ":memory:" {
        return Ok(generate_key()?.to_vec());
    }

    let audit_path_buf = PathBuf::from(audit_path);
    let key_path_buf = PathBuf::from(format!("{audit_path}.key"));

    // F1 fail-closed refusal — runs BEFORE any key is generated, read, or
    // returned. Delegates to the ONE shared containment predicate in adapter-fs
    // (gate-record MAJOR-2: the check is lifted there so the broker policy
    // binder shares the exact same implementation). Both candidate paths (the
    // audit DB and its `.key` sibling) are refused if they resolve at or beneath
    // the workspace root; an unresolvable root/candidate is itself a refusal.
    for candidate in [&audit_path_buf, &key_path_buf] {
        adapter_fs::containment::refuse_if_beneath_workspace(candidate, workspace_root)?;
    }

    // Idempotent read-first: a later, separate `caprun confirm`/`deny` process
    // MUST derive the identical key (Pitfall 2) — never regenerate if present.
    if key_path_buf.exists() {
        let bytes = std::fs::read(&key_path_buf).with_context(|| {
            format!("failed to read existing key file {}", key_path_buf.display())
        })?;
        return Ok(bytes);
    }

    let key = generate_key()?;
    write_key_file(&key_path_buf, &key)?;
    Ok(key.to_vec())
}

/// Generate a fresh 32-byte key via the vetted `getrandom` CSPRNG. Never a
/// custom PRNG (DESIGN §b pin, RESEARCH's explicit rejection of `SystemTime`-
/// seeded alternatives).
fn generate_key() -> anyhow::Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    getrandom::fill(&mut key).map_err(|e| anyhow::anyhow!("OS CSPRNG unavailable: {e}"))?;
    Ok(key)
}

/// Write `key` to `key_path` with `0600` permissions, using create-new
/// semantics so a concurrent create (two processes racing to seed the key)
/// fails safely rather than silently overwriting — the loser reads the
/// winner's bytes back instead (RESEARCH "How to avoid" — cheap, unlikely in
/// practice since `caprun run` always precedes `confirm`/`deny`, but guarded).
#[cfg(unix)]
fn write_key_file(key_path: &Path, key: &[u8; KEY_LEN]) -> anyhow::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true).mode(0o600);

    match opts.open(key_path) {
        Ok(mut f) => {
            f.write_all(key)
                .with_context(|| format!("failed to write key file {}", key_path.display()))?;
            f.sync_all().ok();
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // A concurrent creator won the race — this is still correct
            // custody (the winner's bytes are what both processes must
            // agree on); the caller's `key` is discarded, not persisted.
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("failed to create key file {}", key_path.display())),
    }
}

/// Non-Unix stub — no security claim (dev-only compilation fallback; all v0
/// security claims are Linux-only per CLAUDE.md, mirroring
/// `adapter_fs::workspace::WorkspaceRoot`'s existing cfg-gated stub pattern).
#[cfg(not(unix))]
fn write_key_file(key_path: &Path, key: &[u8; KEY_LEN]) -> anyhow::Result<()> {
    std::fs::write(key_path, key)
        .with_context(|| format!("failed to write key file {}", key_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Create a uniquely-named temp subdir (no `tempfile` dev-dep in this
    /// crate — mirrors `crates/adapter-fs/src/workspace.rs`'s
    /// `unique_tmp_root` convention, but portable/not Linux-gated since these
    /// custody/F1 tests run on macOS host-side too).
    fn unique_tmp_root(tag: &str) -> PathBuf {
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!("caprun_key_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&d).expect("create tmp root");
        d
    }

    /// F1: an audit DB placed INSIDE the workspace root is refused — `Err`,
    /// and no `.key` file is left on disk (fail-closed leaves no artifact).
    #[test]
    fn f1_refusal_when_audit_under_workspace_root() {
        let ws_root = unique_tmp_root("f1_vuln");
        let audit_path = ws_root.join("audit.db");
        let key_path = PathBuf::from(format!("{}.key", audit_path.display()));

        let result = load_or_create_key(audit_path.to_str().unwrap(), &ws_root);

        assert!(
            result.is_err(),
            "audit DB beneath the workspace root must be refused (F1)"
        );
        assert!(
            !key_path.exists(),
            "F1 refusal must write no key file (fail-closed leaves no artifact)"
        );

        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// F1-safe layout, two independent loads: byte-identical keys prove
    /// cross-process stability (Pitfall 2 — a per-process fresh key would
    /// silently break every legitimate `confirm()`'s `verify_chain` gate).
    #[test]
    fn key_file_reused_across_calls() {
        let tmp = unique_tmp_root("f1_safe");
        let ws_root = tmp.join("workspace");
        std::fs::create_dir_all(&ws_root).expect("create workspace subdir");
        let audit_path = tmp.join("audit.db"); // sibling of ws_root, NOT beneath it

        let first = load_or_create_key(audit_path.to_str().unwrap(), &ws_root)
            .expect("first load must succeed");
        let second = load_or_create_key(audit_path.to_str().unwrap(), &ws_root)
            .expect("second load must succeed");

        assert_eq!(first.len(), KEY_LEN, "key must be 32 bytes");
        assert_eq!(
            first, second,
            "two independent loads for the same audit path must return identical bytes"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// `:memory:` returns an ephemeral 32-byte key and writes no `.key` file.
    #[test]
    fn memory_audit_path_ephemeral() {
        let ws_root = unique_tmp_root("memory_ephemeral");

        let key = load_or_create_key(":memory:", &ws_root).expect(":memory: load must succeed");

        assert_eq!(key.len(), KEY_LEN, "ephemeral key must be 32 bytes");
        assert!(
            !Path::new(":memory:.key").exists(),
            ":memory: must never write a key file"
        );

        std::fs::remove_dir_all(&ws_root).ok();
    }
}
