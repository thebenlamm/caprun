//! policy.rs — the broker's session-policy BINDER (POLICY-03, DESIGN §5.3).
//!
//! POLICY-03 is the milestone's converged BLOCKER: a policy loaded from a
//! worker-writable location lets the confined worker widen its own allowlist
//! (the F1-precedent attack — `.planning/REQUIREMENTS.md` Out of Scope "Policy
//! loaded from a session/worker-writable location"). This module binds the
//! session policy AT SESSION CREATION from a trusted source PROVABLY outside the
//! confined worker's reach, and hands back the bound value plus its SHA-256
//! identity so `caprun run` can record it as a genuine, hash-chained
//! `policy_bound` audit-DAG event.
//!
//! # The trust boundary (DESIGN §5.3)
//!
//! The confined worker — caprun's PRIMARY adversary — can `RequestFd` any file
//! at-or-beneath the workspace root via the SAME `WorkspaceRoot::read_within`
//! (`RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`) reach the broker grants for
//! legitimate reads. So a policy file placed there is worker-reachable and MUST
//! be refused. `bind_policy` runs the SAME shared containment predicate as
//! MAC-key custody (`adapter_fs::containment::refuse_if_beneath_workspace` —
//! the ONE extracted helper, gate-record MAJOR-2, check-invariants Gate 6): a
//! policy path resolving at-or-beneath the workspace root is a hard `Err` — NO
//! session runs. It does NOT re-inline the canonicalize/prefix-compare logic;
//! it delegates to the shared helper so both custody call sites cannot drift.
//!
//! # Immutability (DESIGN §5.3, T-42-12)
//!
//! `bind_policy` reads + parses the policy ONCE and returns it BY VALUE. The
//! bound `SessionPolicy` is captured at bind time and threaded immutably into
//! `run_broker_server`; nothing ever re-reads the policy file mid-session, so a
//! worker that rewrites the file cannot change the enforced allowlist (proven by
//! the immutability negative test below).
//!
//! # Fail-closed (DESIGN §4 policy-source row, T-42-14)
//!
//! A specified-but-unresolvable, at-or-beneath-workspace, or unparseable policy
//! path is a hard `Err` — no session. An ABSENT path (`None`) binds the
//! broker-constructed in-memory trusted default (`SessionPolicy::broker_default()`,
//! an EXPLICIT deny-by-default allowlist of the currently-callable production
//! sinks — never allow-everything), the ONLY non-on-disk trusted source DESIGN
//! §5.3 permits.

use std::path::Path;

use anyhow::Context;
use runtime_core::SessionPolicy;
use sha2::{Digest, Sha256};

/// Bind the session policy from a trusted source at session creation.
///
/// * `Some(path)` — FIRST refuse (hard `Err`, no session) if `path` resolves
///   at-or-beneath `workspace_root`, via the shared `refuse_if_beneath_workspace`
///   helper (the SAME predicate MAC-key custody uses; this is what makes the
///   Gate-6 anti-drift check cover the binder). Then read + parse the file into a
///   `SessionPolicy` (a read or parse failure is a fail-closed `Err`), and return
///   it with the SHA-256 hash of its canonical serialized form.
/// * `None` — return `(SessionPolicy::broker_default(), hash)` — the
///   broker-in-memory trusted source DESIGN §5.3 allows. Never a refusal, never
///   allow-everything.
///
/// The returned `String` is the lowercase-hex SHA-256 of the policy's canonical
/// serialized bytes: a stable, deterministic identity (the `BTreeSet`/`BTreeMap`
/// fields of `SessionPolicy` serialize in sorted order, so the same policy always
/// hashes to the same value regardless of the on-disk JSON's key order or
/// whitespace). `caprun run` records this hash into the `policy_bound` audit-DAG
/// event so the enforced policy is provable after the fact via `verify_chain`.
pub fn bind_policy(
    policy_path: Option<&Path>,
    workspace_root: &Path,
) -> anyhow::Result<(SessionPolicy, String)> {
    match policy_path {
        // Absent path → the broker-constructed in-memory trusted default. No
        // containment check (there is no worker-reachable path), never a
        // refusal, never allow-everything (DESIGN §5.3).
        None => {
            let policy = SessionPolicy::broker_default();
            let hash = policy_identity_hash(&policy)?;
            Ok((policy, hash))
        }
        // Specified path → the trusted on-disk source. Containment FIRST, then
        // read + parse, all fail-closed.
        Some(path) => {
            // The load-bearing POLICY-03 refusal: a policy path at-or-beneath the
            // workspace root is worker-reachable (F1-precedent attack) — refuse,
            // no session. Delegates to the ONE shared helper (never a re-inlined
            // copy) so the Gate-6 anti-drift check covers this binder.
            adapter_fs::containment::refuse_if_beneath_workspace(path, workspace_root)
                .with_context(|| {
                    format!(
                        "bind_policy: refusing to bind a policy at-or-beneath the workspace \
                         root (worker-reachable): {}",
                        path.display()
                    )
                })?;

            // Fail-closed: an unreadable path is a hard Err (no session), never a
            // silent fallback to a default.
            let bytes = std::fs::read(path).with_context(|| {
                format!("bind_policy: failed to read policy file {}", path.display())
            })?;

            // Fail-closed: an unparseable policy is a hard Err (no session).
            let policy: SessionPolicy = serde_json::from_slice(&bytes).with_context(|| {
                format!(
                    "bind_policy: failed to parse policy JSON at {} (fail-closed: no session)",
                    path.display()
                )
            })?;

            let hash = policy_identity_hash(&policy)?;
            Ok((policy, hash))
        }
    }
}

/// The stable SHA-256 identity of a bound policy: the lowercase-hex digest of its
/// CANONICAL serialized form. `SessionPolicy`'s `BTreeSet`/`BTreeMap` fields
/// serialize in deterministic sorted order, so equivalent policies (any on-disk
/// key order / whitespace) produce the SAME hash — a semantic identity, not a
/// byte-of-the-file identity.
fn policy_identity_hash(policy: &SessionPolicy) -> anyhow::Result<String> {
    let canonical =
        serde_json::to_vec(policy).context("bind_policy: serialize policy for identity hash")?;
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{append_event, open_audit_db, verify_chain};
    use chrono::Utc;
    use runtime_core::plan_node::SinkId;
    use runtime_core::{Event, PlanNode};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use uuid::Uuid;

    /// Create a uniquely-named temp subdir (no `tempfile` dev-dep — mirrors the
    /// `unique_tmp_root` convention in `key.rs`/`containment.rs`; portable /
    /// not Linux-gated since `canonicalize` works on the macOS host too).
    fn unique_tmp_root(tag: &str) -> PathBuf {
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        let mut d = std::env::temp_dir();
        d.push(format!("caprun_policy_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&d).expect("create tmp root");
        d
    }

    /// A minimal valid policy JSON that allows exactly `email.send`.
    const ALLOW_EMAIL_ONLY: &str = r#"{"allowed_sinks":["email.send"],"arg_constraints":{}}"#;

    fn git_commit_node() -> PlanNode {
        PlanNode {
            sink: SinkId("git.commit".to_string()),
            args: vec![],
        }
    }

    // ── Task 1: containment + fail-closed + broker_default + determinism ──────

    /// POLICY-03 load-bearing containment proof: a policy path BENEATH the
    /// workspace root is refused — hard `Err`, NO session (the F1-precedent
    /// refusal, via the shared helper). Host-portable, NOT Linux-gated.
    #[test]
    fn bind_policy_refuses_path_beneath_workspace_root() {
        let ws_root = unique_tmp_root("beneath");
        let policy_path = ws_root.join("policy.json");
        std::fs::write(&policy_path, ALLOW_EMAIL_ONLY).expect("write policy");

        let result = bind_policy(Some(&policy_path), &ws_root);
        assert!(
            result.is_err(),
            "a policy path beneath the workspace root must be refused (no session)"
        );
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// A policy path EQUAL to the workspace root is refused.
    #[test]
    fn bind_policy_refuses_path_equal_to_workspace_root() {
        let ws_root = unique_tmp_root("equal");
        let result = bind_policy(Some(&ws_root), &ws_root);
        assert!(
            result.is_err(),
            "a policy path equal to the workspace root must be refused"
        );
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// A valid policy at a SIBLING path (outside the workspace root) parses into
    /// a `SessionPolicy` and returns a stable non-empty SHA-256 hash.
    #[test]
    fn bind_policy_binds_valid_sibling_policy_with_hash() {
        let base = unique_tmp_root("sibling_valid");
        let ws_root = base.join("workspace");
        std::fs::create_dir_all(&ws_root).expect("create workspace");
        // Sibling of ws_root, NOT beneath it.
        let policy_path = base.join("policy.json");
        std::fs::write(&policy_path, ALLOW_EMAIL_ONLY).expect("write policy");

        let (policy, hash) =
            bind_policy(Some(&policy_path), &ws_root).expect("valid sibling policy must bind");

        // Parsed into the intended narrowing policy.
        assert!(policy.permits_sink(&SinkId("email.send".to_string())));
        assert!(!policy.permits_sink(&SinkId("git.commit".to_string())));
        // SHA-256 hex is 64 chars.
        assert_eq!(hash.len(), 64, "hash must be a 64-char SHA-256 hex digest");
        std::fs::remove_dir_all(&base).ok();
    }

    /// A specified-but-unresolvable path (parent does not exist) is a hard
    /// fail-closed `Err` — no session.
    #[test]
    fn bind_policy_fail_closed_on_unresolvable_path() {
        let ws_root = unique_tmp_root("unresolvable");
        // The candidate's parent directory does not exist → the shared helper
        // fails closed.
        let policy_path = ws_root.join("no-such-dir").join("policy.json");
        // Sanity: this must be outside the ws_root check on its own merits — the
        // parent-unresolvable branch fires first regardless.
        let result = bind_policy(Some(&policy_path), &ws_root);
        assert!(
            result.is_err(),
            "an unresolvable policy path must fail closed (no session)"
        );
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// A resolvable, outside-the-workspace path whose CONTENT is not valid policy
    /// JSON is a fail-closed `Err` — no session (never a silent default).
    #[test]
    fn bind_policy_fail_closed_on_unparseable_policy() {
        let base = unique_tmp_root("unparseable");
        let ws_root = base.join("workspace");
        std::fs::create_dir_all(&ws_root).expect("create workspace");
        let policy_path = base.join("policy.json");
        std::fs::write(&policy_path, b"this is not json").expect("write garbage");

        let result = bind_policy(Some(&policy_path), &ws_root);
        assert!(
            result.is_err(),
            "an unparseable policy must fail closed (no session)"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    /// `None` binds the broker-constructed default: the EXPLICIT deny-by-default
    /// allowlist of the nine production sinks (never allow-everything, never a
    /// refusal), with a stable hash.
    #[test]
    fn bind_policy_none_binds_broker_default() {
        let ws_root = unique_tmp_root("none_default");
        let (policy, hash) =
            bind_policy(None, &ws_root).expect("None must bind broker_default, never refuse");

        // Permits every currently-callable production sink...
        for s in [
            "email.send",
            "file.create",
            "file.write",
            "process.exec",
            "git.commit",
            "http.request",
            "http.request.write",
            "github.pr",
            "git.push",
        ] {
            assert!(
                policy.permits_sink(&SinkId(s.to_string())),
                "broker_default should permit {s}"
            );
        }
        // ...but is NOT allow-everything: a genuinely-unregistered sink is not
        // callable (git.push became a production sink in Phase 44-01, so it can no
        // longer serve as the unknown-sink example — mirrors the runtime-core twin).
        assert!(!policy.permits_sink(&SinkId("deploy.service".to_string())));
        assert_eq!(hash.len(), 64);
        std::fs::remove_dir_all(&ws_root).ok();
    }

    /// Deterministic identity: the same policy always hashes to the same value.
    #[test]
    fn bind_policy_hash_is_deterministic() {
        let base = unique_tmp_root("determinism");
        let ws_root = base.join("workspace");
        std::fs::create_dir_all(&ws_root).expect("create workspace");
        let policy_path = base.join("policy.json");
        std::fs::write(&policy_path, ALLOW_EMAIL_ONLY).expect("write policy");

        let (_p1, h1) = bind_policy(Some(&policy_path), &ws_root).expect("bind 1");
        let (_p2, h2) = bind_policy(Some(&policy_path), &ws_root).expect("bind 2");
        assert_eq!(h1, h2, "the same policy bytes must hash to the same value");

        // And a DIFFERENT policy hashes differently.
        let (_pd, hd) = bind_policy(None, &ws_root).expect("bind default");
        assert_ne!(h1, hd, "a different policy must hash differently");
        std::fs::remove_dir_all(&base).ok();
    }

    // ── Task 3: immutability negative leg + audit-DAG recording proof ─────────

    /// IMMUTABILITY (the negative leg, DESIGN §5.3 / T-42-12): bind a policy that
    /// DENIES `git.commit`, keep the bound value, then rewrite the SAME file to
    /// ALLOW `git.commit`. The bound value never re-reads the file, so evaluating
    /// a `git.commit` node against it STILL policy-denies — a mid-session rewrite
    /// has zero effect on enforcement.
    #[test]
    fn bound_policy_is_immutable_across_a_mid_session_file_rewrite() {
        let base = unique_tmp_root("immutable");
        let ws_root = base.join("workspace");
        std::fs::create_dir_all(&ws_root).expect("create workspace");
        let policy_path = base.join("policy.json");

        // Bind a policy that DENIES git.commit (allows only email.send).
        std::fs::write(&policy_path, ALLOW_EMAIL_ONLY).expect("write deny-git policy");
        let (bound_policy, _hash) =
            bind_policy(Some(&policy_path), &ws_root).expect("bind deny-git policy");

        // Sanity: the bound policy denies git.commit right now.
        let store = executor::value_store::ValueStore::default();
        assert!(
            executor::policy_gate::policy_gate(&bound_policy, &git_commit_node(), &store).is_err(),
            "the freshly-bound policy must deny git.commit"
        );

        // The worker (the adversary) rewrites the SAME file mid-session to ALLOW
        // git.commit.
        std::fs::write(
            &policy_path,
            r#"{"allowed_sinks":["email.send","git.commit"],"arg_constraints":{}}"#,
        )
        .expect("rewrite to allow-git policy");

        // The STILL-BOUND value never re-read the file → enforcement is unchanged:
        // git.commit is STILL policy-denied.
        assert!(
            executor::policy_gate::policy_gate(&bound_policy, &git_commit_node(), &store).is_err(),
            "a mid-session policy-file rewrite must NOT change the enforced allowlist \
             (the bound policy is immutable — captured by value, never re-read)"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    /// AUDIT RECORDING (DESIGN §5.3 / T-42-13): a `policy_bound` event appended
    /// after `session_created`, carrying the bind_policy hash in its actor field,
    /// is retrievable with that exact hash AND `verify_chain` passes — proving the
    /// policy identity is a GENUINE, tamper-evident, hash-chained audit-DAG event,
    /// not stapled on.
    #[test]
    fn policy_bound_event_is_genuinely_hash_chained() {
        let ws_root = unique_tmp_root("audit_chain");
        let (_policy, policy_hash) = bind_policy(None, &ws_root).expect("bind default");

        let conn = open_audit_db(":memory:").expect("open in-memory audit db");
        let key = [7u8; 32];
        let session_id = Uuid::new_v4();

        // session_created (chain root).
        let session_created_id = Uuid::new_v4();
        let e_session = Event::new(
            session_created_id,
            None,
            session_id,
            "broker:seed_provenance=trusted_arg".to_string(),
            "session_created".to_string(),
            Utc::now(),
            vec![],
        );
        let session_created_hash =
            append_event(&conn, &key, &e_session, None).expect("append session_created");

        // policy_bound (chained child of session_created) — the hash rides in the
        // actor field so it is hashed into the chain (genuine, not stapled),
        // mirroring how seed_provenance rides in the session_created actor.
        let policy_bound_id = Uuid::new_v4();
        let policy_actor = format!("broker:policy_bound sha256={policy_hash}");
        let e_policy_bound = Event::new(
            policy_bound_id,
            Some(session_created_id),
            session_id,
            policy_actor.clone(),
            "policy_bound".to_string(),
            Utc::now(),
            vec![],
        );
        append_event(&conn, &key, &e_policy_bound, Some(&session_created_hash))
            .expect("append policy_bound");

        // (a) the stored event's hash-carrying field equals the bind_policy hash.
        let stored_actor: String = conn
            .query_row(
                "SELECT actor FROM events WHERE id = ?1",
                [policy_bound_id.to_string()],
                |row| row.get(0),
            )
            .expect("query policy_bound actor");
        assert_eq!(
            stored_actor, policy_actor,
            "the stored policy_bound actor must carry the exact bind_policy hash"
        );
        assert!(
            stored_actor.contains(&policy_hash),
            "the recorded hash must equal the hash bind_policy returned"
        );

        // (b) verify_chain passes — the policy_bound event is genuinely chained.
        assert!(
            verify_chain(&conn, &session_id.to_string(), &key),
            "verify_chain must pass across the policy_bound event (genuine, not stapled)"
        );
        std::fs::remove_dir_all(&ws_root).ok();
    }
}
