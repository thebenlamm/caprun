/// sinks/file_write — mediated `file.write` sink (LIVE side effect).
///
/// Unlike `file_create` (which never overwrites), `file.write` performs a REAL
/// filesystem side effect via Plan 01's `WorkspaceRoot::write_within` — a
/// single `openat2(O_WRONLY|O_TRUNC, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`
/// syscall that requires the target to ALREADY exist (ENOENT on a missing
/// target), rejects absolute/`..`/symlink escapes at kernel resolution time,
/// and is TOCTOU-safe.
///
/// This sink is invoked ONLY after the executor returns `Allowed` for a
/// `file.write` plan node (server.rs `SubmitPlanNode` arm). A tainted
/// (routing-sensitive) `path` arg is blocked upstream by the executor and never
/// reaches here — so a genuine-tainted workspace path is never written.
///
/// # Two-phase durable audit (T-33-09)
///
/// The authorizing `plan_node_evaluated` event is persisted by the caller BEFORE
/// this function runs. This function then performs the effect and records its
/// outcome durably:
///   * success → a `sink_executed` event (carrying `effect_id` in the actor field).
///   * on a *filesystem* error (from `write_within`) → a `sink_execution_failed`
///     event is appended FIRST (an explicit indeterminate record), THEN the
///     original error is propagated. There is NO automatic retry — a
///     mid-effect failure leaves a durable, explicit trace rather than
///     silently retrying an operation that may have partially applied.
///
/// (Phase 33 adversarial-review NIT-5: this "append FIRST" guarantee covers
/// only *filesystem* errors — a missing/dangling arg handle in `resolve_arg`
/// propagates BEFORE any filesystem call is attempted, pre-effect with zero
/// indeterminacy, so no `sink_execution_failed` event is appended on that
/// path; there is nothing indeterminate to record.)
///
/// The `effect_id` is carried in the `actor` field (`sink:file.write:<effect_id>`)
/// because `Event` has no `effect_id` column and adding one would break the
/// pre-anchor golden byte-fixture (DESIGN §5, no DB migration).
use anyhow::{Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode};
use uuid::Uuid;

use adapter_fs::workspace::WorkspaceRoot;

use crate::audit::append_event;
use crate::confirmation::ResolvedArg;

/// Invoke the live `file.write` sink for an `Allowed` plan node.
///
/// Resolves the `path` and `contents` args to their broker-owned literals via the
/// per-connection `ValueStore`, then overwrites the existing file beneath the
/// workspace root. Records a two-phase durable audit event and returns the new
/// event's `(id, hash)` so the caller can advance the causal chain (the event is
/// chained onto `parent_id`/`parent_hash`, keeping `verify_chain` intact).
///
/// # Arguments
/// * `conn`         — open rusqlite connection (broker-owned).
/// * `value_store`  — the per-connection ValueStore (resolves the arg handles).
/// * `session_id`   — the active broker session.
/// * `effect_id`    — the broker-minted effect identity (carried in the event actor).
/// * `plan_node`    — the `file.write` plan node (opaque-handle args only).
/// * `workspace_root` — the dirfd-anchored write capability (07-03/07-04a).
/// * `parent_id`    — causal predecessor event id (the `plan_node_evaluated` event).
/// * `parent_hash`  — hash of that predecessor row (chain anchor).
///
/// # Returns
/// `(event_id, hash)` of the appended `sink_executed` event on success.
///
/// # Errors
/// On a filesystem error (including ENOENT on a missing target) a
/// `sink_execution_failed` event is durably appended FIRST, then the original
/// error is propagated (no retry).
#[allow(clippy::too_many_arguments)]
pub fn invoke_file_write(
    conn: &rusqlite::Connection,
    key: &[u8],
    value_store: &ValueStore,
    session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    // Resolve the two args from the broker-owned store. validate_schema (07-04a,
    // Step 0 of submit_plan_node) already guaranteed both are present and known —
    // a missing/dangling handle here is a broker-internal invariant violation.
    let path = resolve_arg(value_store, plan_node, "path")?;
    let contents = resolve_arg(value_store, plan_node, "contents")?;

    // The single side-effecting syscall (Plan 01). O_TRUNC requires the target
    // to already exist (ENOENT otherwise); RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS
    // reject escapes at kernel resolution.
    match workspace_root.write_within(&path, contents.as_bytes()) {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.write:{effect_id}"),
                "sink_executed".into(),
                Utc::now(),
                vec![], // the executed effect carries no taint (path was UserTrusted)
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_executed")?;
            Ok((event.id, hash))
        }
        Err(e) => {
            // Two-phase durable audit: record an explicit indeterminate outcome,
            // then propagate. NO automatic retry (T-33-09).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.write:{effect_id}"),
                "sink_execution_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_execution_failed")?;
            Err(anyhow::Error::new(e).context("file.write write_within failed"))
        }
    }
}

/// Resolve a named plan-node arg to its broker-owned literal.
fn resolve_arg(store: &ValueStore, plan_node: &PlanNode, name: &str) -> Result<String> {
    let arg = plan_node
        .args
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| anyhow::anyhow!("file.write plan node missing `{name}` arg"))?;
    let record = store
        .resolve(&arg.value_id)
        .ok_or_else(|| anyhow::anyhow!("file.write `{name}` handle did not resolve"))?;
    Ok(record.literal.clone())
}

/// Look up a named literal directly from a frozen `ResolvedArg` snapshot.
fn resolved_literal<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Result<&'a str> {
    resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
        .ok_or_else(|| anyhow::anyhow!("frozen resolved_args missing `{name}` arg"))
}

/// Re-invoke the live `file.write` sink from a FROZEN `ResolvedArg` snapshot
/// (confirm-time re-invocation; DESIGN-confirmation-release.md Step 4a.4).
///
/// A `ValueStore`-free sibling of `invoke_file_write`: this is called by a later,
/// separate `caprun confirm` process after a human has released exactly one
/// (sink, arg, literal-digest) triple. The literals are already adjudicated and
/// frozen at Block time (`crate::confirmation::PendingConfirmation.resolved_args`)
/// — this function never constructs a `ValueStore`, never calls `store.resolve`,
/// never calls `store.mint`, and never calls `executor::submit_plan_node` (I2 is
/// neither re-run nor bypassable here; T-10-05 / CON-i2-non-bypassable).
///
/// Copies `invoke_file_create_from_resolved`'s two-phase durable-audit structure
/// verbatim (Phase 33 adversarial-review MAJOR-1 fix — a blocked `file.write` had
/// no confirm-release dispatch before this function existed), changing ONLY the
/// arg source (`resolved_args` lookup instead of `ValueStore::resolve`) and the
/// underlying syscall (`write_within` instead of `create_exclusive_within`). On a
/// filesystem error this appends a `sink_invocation_failed` event (NOT
/// `sink_execution_failed` — that event type is reserved for the allow-path's
/// `invoke_file_write`, distinguishing a confirm-time sink failure from an
/// allow-time one per DESIGN Step 4a.5), THEN propagates the original error (no
/// retry).
///
/// # Arguments
/// * `conn`            — open rusqlite connection (broker-owned).
/// * `session_id`      — the Session the blocked plan node belonged to.
/// * `effect_id`       — the SAME `effect_id` as the original block's anchor.
/// * `resolved_args`   — the frozen `ResolvedArg` snapshot from `PendingConfirmation`.
/// * `workspace_root`  — the workspace root reopened at confirm time (same root the
///   broker opened at Block time; `PendingConfirmation.workspace_root_path`).
/// * `parent_id`       — causal predecessor event id.
/// * `parent_hash`     — hash of that predecessor row (chain anchor).
///
/// # Returns
/// `(event_id, hash)` of the appended `sink_executed` event on success.
///
/// # Errors
/// On a filesystem error (including ENOENT on a missing target) a
/// `sink_invocation_failed` event is durably appended FIRST, then the original
/// error is propagated (no retry).
#[allow(clippy::too_many_arguments)]
pub fn invoke_file_write_from_resolved(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    // Look up the frozen literals directly — never re-resolve, never re-decide.
    let path = resolved_literal(resolved_args, "path")?;
    let contents = resolved_literal(resolved_args, "contents")?;

    match workspace_root.write_within(path, contents.as_bytes()) {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.write:{effect_id}"),
                "sink_executed".into(),
                Utc::now(),
                vec![], // the executed effect carries no taint (frozen literal was adjudicated)
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_executed")?;
            Ok((event.id, hash))
        }
        Err(e) => {
            // Two-phase durable audit: record an explicit indeterminate outcome,
            // then propagate. NO automatic retry. Distinct event type from the
            // allow-path's `sink_execution_failed` (DESIGN Step 4a.5).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.write:{effect_id}"),
                "sink_invocation_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_invocation_failed")?;
            Err(anyhow::Error::new(e)
                .context("file.write write_within (from_resolved) failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{find_event_by_type, open_audit_db};
    use executor::value_store::ValueStore;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel};

    /// Fixed, non-secret test MAC key (mirrors `audit.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"file-write-rs-unit-test-key-not-secret";

    /// Build a file.write plan node whose path+contents resolve to the given
    /// literals in a fresh store, plus a seeded causal-root event.
    /// Returns (store, plan_node, conn, session_id, parent_event_id, parent_hash).
    fn setup(
        path: &str,
        contents: &str,
    ) -> (ValueStore, PlanNode, rusqlite::Connection, Uuid, Uuid, String) {
        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let mut store = ValueStore::default();
        // Seed a causal-root event so the sink event can chain onto it and
        // verify_chain has an unbroken parent linkage.
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();
        // Mint path + contents as trusted values (this is the Allowed path).
        let ev = Uuid::new_v4();
        let path_vid = store
            .mint(
                path.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![ev],
                Some("path".to_string()),
            )
            .unwrap();
        let contents_vid = store
            .mint(
                contents.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![ev],
                None,
            )
            .unwrap();
        let plan_node = PlanNode {
            sink: SinkId("file.write".into()),
            args: vec![
                PlanArg { name: "path".into(), value_id: path_vid },
                PlanArg { name: "contents".into(), value_id: contents_vid },
            ],
        };
        (store, plan_node, conn, session_id, root.id, root_hash)
    }

    /// On success, invoke_file_write overwrites the pre-existing file and
    /// records a chained `sink_executed` event carrying the effect_id in its
    /// actor.
    #[test]
    fn invoke_file_write_success_records_sink_executed() {
        // Unique temp workspace root (no tempfile dev-dep in brokerd).
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fw_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // Pre-create the target — write_within requires it to already exist.
        std::fs::write(root.join("existing.txt"), b"original").unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup("existing.txt", "hello from file.write");
        let effect_id = Uuid::new_v4();

        let (evt_id, hash) = invoke_file_write(
            &conn, TEST_KEY, &store, session_id, effect_id, &plan_node, &ws, parent_id, &parent_hash,
        )
        .expect("file.write must succeed on an existing target");

        assert!(!hash.is_empty());
        // File content was overwritten (Linux path; the non-Linux stub also
        // requires the target to already exist and truncates on write).
        let on_disk = std::fs::read_to_string(root.join("existing.txt")).unwrap();
        assert_eq!(on_disk, "hello from file.write");

        // A sink_executed event exists carrying effect_id in the actor.
        let evt = find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .unwrap()
            .expect("sink_executed event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.actor, format!("sink:file.write:{effect_id}"));
        assert_eq!(evt.parent_id, Some(parent_id));

        std::fs::remove_dir_all(&root).ok();
    }

    /// On a missing target (ENOENT), invoke_file_write records a
    /// `sink_execution_failed` event and propagates the error (no retry).
    #[test]
    fn invoke_file_write_failure_records_sink_execution_failed() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fw_err_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // Deliberately do NOT create "missing.txt" — write_within requires
        // the target to already exist.
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup("missing.txt", "will not land");
        let effect_id = Uuid::new_v4();

        let result = invoke_file_write(
            &conn, TEST_KEY, &store, session_id, effect_id, &plan_node, &ws, parent_id, &parent_hash,
        );
        assert!(result.is_err(), "write to a missing target must fail (ENOENT)");

        // No file was created as a side effect of the failed write.
        assert!(!root.join("missing.txt").exists());

        // A durable sink_execution_failed event exists.
        let evt = find_event_by_type(&conn, &session_id.to_string(), "sink_execution_failed")
            .unwrap()
            .expect("sink_execution_failed event must exist");
        assert_eq!(evt.parent_id, Some(parent_id));
        // No sink_executed event on the failure path.
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
                .unwrap()
                .is_none(),
            "no sink_executed on the failure path"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // ── invoke_file_write_from_resolved (Phase 33 adversarial-review MAJOR-1
    // fix, frozen-literal re-invocation) ─────────────────────────────────

    /// A minimal 2-element ResolvedArg snapshot for {path, contents} — mirrors
    /// what a `PendingConfirmation.resolved_args` payload would carry.
    fn resolved_args_for(path: &str, contents: &str) -> Vec<ResolvedArg> {
        let ev = Uuid::new_v4();
        vec![
            ResolvedArg {
                name: "path".into(),
                value_id: runtime_core::plan_node::ValueId::new(),
                literal: path.to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![ev],
            },
            ResolvedArg {
                name: "contents".into(),
                value_id: runtime_core::plan_node::ValueId::new(),
                literal: contents.to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![ev],
            },
        ]
    }

    /// Seed a causal-root event in a fresh in-memory DB. Returns (conn, session_id, root_id, root_hash).
    fn seed_root() -> (rusqlite::Connection, Uuid, Uuid, String) {
        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();
        (conn, session_id, root.id, root_hash)
    }

    /// On success, invoke_file_write_from_resolved overwrites the pre-existing
    /// file from frozen literals (never a ValueStore) and records a chained
    /// `sink_executed` event.
    #[test]
    fn invoke_file_write_from_resolved_success_records_sink_executed() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fw_resolved_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("existing.txt"), b"original").unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let resolved_args = resolved_args_for("existing.txt", "hi from confirm");

        let (evt_id, hash) = invoke_file_write_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        )
        .expect("invoke_file_write_from_resolved must succeed on an existing target");

        assert!(!hash.is_empty());
        let on_disk = std::fs::read_to_string(root.join("existing.txt")).unwrap();
        assert_eq!(on_disk, "hi from confirm");

        let evt = find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .unwrap()
            .expect("sink_executed event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.actor, format!("sink:file.write:{effect_id}"));
        assert_eq!(evt.parent_id, Some(parent_id));

        std::fs::remove_dir_all(&root).ok();
    }

    /// On a missing target path, invoke_file_write_from_resolved records a
    /// `sink_invocation_failed` event (NOT `sink_execution_failed`) and
    /// propagates the error; no file is created.
    #[test]
    fn invoke_file_write_from_resolved_failure_records_sink_invocation_failed() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fw_resolved_err_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let resolved_args = resolved_args_for("missing.txt", "will not land");

        let result = invoke_file_write_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        );
        assert!(result.is_err(), "write to a missing target must fail (ENOENT)");

        assert!(!root.join("missing.txt").exists());

        let evt = find_event_by_type(&conn, &session_id.to_string(), "sink_invocation_failed")
            .unwrap()
            .expect("sink_invocation_failed event must exist");
        assert_eq!(evt.parent_id, Some(parent_id));
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
                .unwrap()
                .is_none(),
            "no sink_executed on the failure path"
        );
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "sink_execution_failed")
                .unwrap()
                .is_none(),
            "sink_invocation_failed must be distinct from the allow-path's sink_execution_failed"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
