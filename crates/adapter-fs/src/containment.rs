/// containment — the ONE shared at-or-beneath-workspace-root refusal predicate
/// (DESIGN-v1.9-egress-policy §5.3, gate-record MAJOR-2).
///
/// The confined worker — caprun's PRIMARY adversary — can `RequestFd` any file
/// at-or-beneath the workspace root via `WorkspaceRoot::read_within`. So any
/// custody artifact (the MAC-key `.key` file, F1) OR policy artifact (the
/// bound session policy, POLICY-03) placed there is worker-reachable and MUST
/// be refused. This predicate was originally inlined + `pub(crate)` in
/// `cli/caprun/src/key.rs`; gate-record MAJOR-2 proved that inlining made it
/// unreachable from the broker-side policy binder. It is lifted here — to
/// adapter-fs, a crate BOTH `cli/caprun` and `crates/brokerd` already depend on
/// and which already owns `WorkspaceRoot` containment — so both call sites
/// delegate to ONE implementation that cannot drift. (runtime-core cannot host
/// it: the invariant gate bans filesystem access there.)
///
/// The predicate logic is preserved EXACTLY from the F1 original — MAJOR-2
/// confirmed the only gap was factoring, not the (component-wise-correct) logic.
use anyhow::Context;
use std::path::{Path, PathBuf};

/// Refuse (`Err`) if `path` resolves to — or beneath — `workspace_root`.
///
/// Fail-closed on every unresolvable input:
/// - An absent/unresolvable `workspace_root` is itself a refusal (the
///   requires-root-exists behavior — a caller cannot contain against a root
///   that does not resolve).
/// - A candidate `path` whose own parent cannot be resolved is a refusal.
///
/// A nonexistent candidate whose parent DOES exist is resolved via
/// parent-then-rejoin (canonicalize the existing parent, rejoin the final
/// component) so the file itself need not exist yet — sufficient for a
/// containment comparison at first-run custody/binding time.
///
/// Containment is component-wise (`Path::starts_with`), so a sibling whose name
/// merely shares a textual prefix with the root (e.g. `/ws-foo` vs root `/ws`)
/// is correctly ACCEPTED — it is not beneath the root.
pub fn refuse_if_beneath_workspace(path: &Path, workspace_root: &Path) -> anyhow::Result<()> {
    // Fail-closed: an unresolvable workspace root is a refusal, never fail-open.
    let canonical_root = std::fs::canonicalize(workspace_root).with_context(|| {
        format!(
            "containment fail-closed refusal: cannot resolve workspace root {}",
            workspace_root.display()
        )
    })?;

    let canonical_candidate = canonicalize_existing_or_parent(path).with_context(|| {
        format!(
            "containment fail-closed refusal: cannot resolve {}",
            path.display()
        )
    })?;

    // containment-predicate
    if canonical_candidate.starts_with(&canonical_root) {
        anyhow::bail!(
            "containment refusal: {} resolves at or beneath the workspace root {} \
             — the confined worker could RequestFd it; refusing",
            canonical_candidate.display(),
            canonical_root.display()
        );
    }

    Ok(())
}

/// Canonicalize `path`. `std::fs::canonicalize` requires an existing path, but
/// the candidate (audit DB / key file / policy file) may not exist yet on a
/// first run — in that case, canonicalize the (existing) parent directory and
/// rejoin the file name, which is sufficient for the containment comparison
/// without requiring the file itself to exist.
fn canonicalize_existing_or_parent(path: &Path) -> std::io::Result<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name")
    })?;
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let canonical_parent = std::fs::canonicalize(parent)?;
    Ok(canonical_parent.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Create a uniquely-named temp subdir (no `tempfile` dev-dep in this
    /// crate — mirrors the `unique_tmp_root` convention in
    /// `crates/adapter-fs/src/workspace.rs`; portable / not Linux-gated since
    /// `canonicalize` works on the macOS host too).
    fn unique_tmp_root(tag: &str) -> PathBuf {
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!("caprun_containment_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&d).expect("create tmp root");
        d
    }

    /// A candidate resolving EQUAL TO the workspace root is refused.
    #[test]
    fn refuses_when_equal_to_root() {
        let ws_root = unique_tmp_root("equal");
        let result = refuse_if_beneath_workspace(&ws_root, &ws_root);
        assert!(result.is_err(), "a path equal to the root must be refused");
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// A candidate resolving to a DESCENDANT of the root is refused.
    #[test]
    fn refuses_when_descendant_of_root() {
        let ws_root = unique_tmp_root("descendant");
        let candidate = ws_root.join("audit.db"); // nonexistent, parent = ws_root
        let result = refuse_if_beneath_workspace(&candidate, &ws_root);
        assert!(
            result.is_err(),
            "a path beneath the root must be refused (parent-then-rejoin comparison)"
        );
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// A SIBLING whose name shares a textual prefix with the root (e.g.
    /// `/ws-foo` vs root `/ws`) is NOT beneath it — component-wise `starts_with`
    /// must ACCEPT it (the MAJOR-2 correctness assertion, no sibling-prefix bug).
    #[test]
    fn accepts_sibling_prefix_of_root() {
        let base = unique_tmp_root("sibling");
        let ws_root = base.join("ws");
        let sibling = base.join("ws-foo");
        std::fs::create_dir_all(&ws_root).expect("create ws");
        std::fs::create_dir_all(&sibling).expect("create ws-foo");

        // Candidate is a nonexistent file inside the sibling dir.
        let candidate = sibling.join("audit.db");
        let result = refuse_if_beneath_workspace(&candidate, &ws_root);
        assert!(
            result.is_ok(),
            "/ws-foo/... must NOT be refused against root /ws (component-wise starts_with)"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    /// An unresolvable/absent workspace root is a fail-closed refusal.
    #[test]
    fn refuses_when_root_unresolvable() {
        let base = unique_tmp_root("bad_root");
        let missing_root = base.join("does-not-exist");
        // A perfectly-fine candidate elsewhere; the root is the problem.
        let candidate = base.join("audit.db");
        let result = refuse_if_beneath_workspace(&candidate, &missing_root);
        assert!(
            result.is_err(),
            "an unresolvable workspace root must fail closed (refuse), never fail-open"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    /// A candidate whose own PARENT cannot be resolved is a fail-closed refusal.
    #[test]
    fn refuses_when_candidate_parent_unresolvable() {
        let ws_root = unique_tmp_root("bad_parent_root");
        // Candidate's parent directory does not exist.
        let candidate = ws_root.join("missing-dir").join("audit.db");
        let result = refuse_if_beneath_workspace(&candidate, &ws_root);
        assert!(
            result.is_err(),
            "a candidate whose parent is unresolvable must fail closed (refuse)"
        );
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// A nonexistent candidate whose parent EXISTS and lies OUTSIDE the root is
    /// accepted — parent-then-rejoin resolves it, the file need not exist.
    #[test]
    fn accepts_nonexistent_candidate_with_existing_parent_outside_root() {
        let base = unique_tmp_root("outside");
        let ws_root = base.join("workspace");
        std::fs::create_dir_all(&ws_root).expect("create workspace");
        // Sibling of ws_root, does not exist yet, parent (base) exists.
        let candidate = base.join("audit.db");
        let result = refuse_if_beneath_workspace(&candidate, &ws_root);
        assert!(
            result.is_ok(),
            "a nonexistent candidate outside the root (existing parent) must be accepted"
        );
        std::fs::remove_dir_all(&base).ok();
    }
}
