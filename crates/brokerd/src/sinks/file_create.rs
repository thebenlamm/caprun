/// sinks/file_create — mediated `file.create` sink (LIVE side effect).
///
/// Unlike `email_send` (a stub that only records an audit event), `file.create`
/// performs a REAL filesystem side effect via 07-04a's
/// `WorkspaceRoot::create_exclusive_within` — a single
/// `openat2(O_CREAT|O_EXCL|O_WRONLY, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`
/// syscall that never overwrites, rejects absolute/`..`/symlink escapes at kernel
/// resolution time, and is TOCTOU-safe.
///
/// This sink is invoked ONLY after the executor returns `Allowed` for a
/// `file.create` plan node (server.rs `SubmitPlanNode` arm). A tainted
/// (routing-sensitive) `path` arg is blocked upstream by the executor and never
/// reaches here — so a genuine-tainted workspace path is never written.
///
/// # Two-phase durable audit (T-07-45 / ACC-01 / HARD-06)
///
/// The authorizing `plan_node_evaluated` event is persisted by the caller BEFORE
/// this function runs. This function then performs the effect and records its
/// outcome durably:
///   * success → a `sink_executed` event (carrying `effect_id` in the actor field).
///   * error   → a `sink_execution_failed` event (an explicit indeterminate
///     record), THEN the original error is propagated. There is NO automatic
///     retry — a mid-effect failure leaves a durable, explicit trace rather than
///     silently retrying an operation that may have partially applied.
///
/// The `effect_id` is carried in the `actor` field (`sink:file.create:<effect_id>`)
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

/// Invoke the live `file.create` sink for an `Allowed` plan node.
///
/// Resolves the `path` and `contents` args to their broker-owned literals via the
/// per-connection `ValueStore`, then creates the file exclusively beneath the
/// workspace root. Records a two-phase durable audit event and returns the new
/// event's `(id, hash)` so the caller can advance the causal chain (the event is
/// chained onto `parent_id`/`parent_hash`, keeping `verify_chain` intact).
///
/// # Arguments
/// * `conn`         — open rusqlite connection (broker-owned).
/// * `value_store`  — the per-connection ValueStore (resolves the arg handles).
/// * `session_id`   — the active broker session.
/// * `effect_id`    — the broker-minted effect identity (carried in the event actor).
/// * `plan_node`    — the `file.create` plan node (opaque-handle args only).
/// * `workspace_root` — the dirfd-anchored write capability (07-03/07-04a).
/// * `parent_id`    — causal predecessor event id (the `plan_node_evaluated` event).
/// * `parent_hash`  — hash of that predecessor row (chain anchor).
///
/// # Returns
/// `(event_id, hash)` of the appended `sink_executed` event on success.
///
/// # Errors
/// On a filesystem error a `sink_execution_failed` event is durably appended
/// FIRST, then the original error is propagated (no retry).
#[allow(clippy::too_many_arguments)]
pub fn invoke_file_create(
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

    // The single side-effecting syscall (07-04a). O_EXCL never overwrites;
    // RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS reject escapes at kernel resolution.
    match workspace_root.create_exclusive_within(&path, contents.as_bytes()) {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.create:{effect_id}"),
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
            // then propagate. NO automatic retry (T-07-45 / ACC-01 / HARD-06).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.create:{effect_id}"),
                "sink_execution_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_execution_failed")?;
            Err(anyhow::Error::new(e).context("file.create create_exclusive_within failed"))
        }
    }
}

/// Resolve a named plan-node arg to its broker-owned literal.
fn resolve_arg(store: &ValueStore, plan_node: &PlanNode, name: &str) -> Result<String> {
    let arg = plan_node
        .args
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| anyhow::anyhow!("file.create plan node missing `{name}` arg"))?;
    let record = store
        .resolve(&arg.value_id)
        .ok_or_else(|| anyhow::anyhow!("file.create `{name}` handle did not resolve"))?;
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

/// Re-invoke the live `file.create` sink from a FROZEN `ResolvedArg` snapshot
/// (confirm-time re-invocation; DESIGN-confirmation-release.md Step 4a.4).
///
/// A `ValueStore`-free sibling of `invoke_file_create`: this is called by a later,
/// separate `caprun confirm` process after a human has released exactly one
/// (sink, arg, literal-digest) triple. The literals are already adjudicated and
/// frozen at Block time (`crate::confirmation::PendingConfirmation.resolved_args`)
/// — this function never constructs a `ValueStore`, never calls `store.resolve`,
/// never calls `store.mint`, and never calls `executor::submit_plan_node` (I2 is
/// neither re-run nor bypassable here; T-10-05 / CON-i2-non-bypassable).
///
/// Copies `invoke_file_create`'s two-phase durable-audit structure verbatim,
/// changing ONLY the arg source (`resolved_args` lookup instead of
/// `ValueStore::resolve`). On a filesystem error this appends a
/// `sink_invocation_failed` event (NOT `sink_execution_failed` — that event type
/// is reserved for the allow-path's `invoke_file_create`, distinguishing a
/// confirm-time sink failure from an allow-time one per DESIGN Step 4a.5), THEN
/// propagates the original error (no retry).
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
/// On a filesystem error a `sink_invocation_failed` event is durably appended
/// FIRST, then the original error is propagated (no retry).
#[allow(clippy::too_many_arguments)]
pub fn invoke_file_create_from_resolved(
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

    match workspace_root.create_exclusive_within(path, contents.as_bytes()) {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:file.create:{effect_id}"),
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
                format!("sink:file.create:{effect_id}"),
                "sink_invocation_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append sink_invocation_failed")?;
            Err(anyhow::Error::new(e).context("file.create create_exclusive_within (from_resolved) failed"))
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
    const TEST_KEY: &[u8] = b"file-create-rs-unit-test-key-not-secret";

    /// Build a file.create plan node whose path+contents resolve to the given
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
            sink: SinkId("file.create".into()),
            args: vec![
                PlanArg { name: "path".into(), value_id: path_vid },
                PlanArg { name: "contents".into(), value_id: contents_vid },
            ],
        };
        (store, plan_node, conn, session_id, root.id, root_hash)
    }

    /// On success, invoke_file_create creates the file and records a chained
    /// `sink_executed` event carrying the effect_id in its actor.
    #[test]
    fn invoke_file_create_success_records_sink_executed() {
        // Unique temp workspace root (no tempfile dev-dep in brokerd).
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fc_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup("created.txt", "hello from file.create");
        let effect_id = Uuid::new_v4();

        let (evt_id, hash) = invoke_file_create(
            &conn, TEST_KEY, &store, session_id, effect_id, &plan_node, &ws, parent_id, &parent_hash,
        )
        .expect("file.create must succeed on a fresh path");

        assert!(!hash.is_empty());
        // File exists with the expected contents.
        let on_disk = std::fs::read_to_string(root.join("created.txt")).unwrap();
        assert_eq!(on_disk, "hello from file.create");

        // A sink_executed event exists carrying effect_id in the actor.
        let evt = find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .unwrap()
            .expect("sink_executed event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.actor, format!("sink:file.create:{effect_id}"));
        assert_eq!(evt.parent_id, Some(parent_id));

        std::fs::remove_dir_all(&root).ok();
    }

    /// On an existing path (O_EXCL → EEXIST), invoke_file_create records a
    /// `sink_execution_failed` event and propagates the error (no retry).
    #[test]
    fn invoke_file_create_failure_records_sink_execution_failed() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fc_err_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // Pre-create the target so the exclusive create fails.
        std::fs::write(root.join("dup.txt"), b"original").unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (store, plan_node, conn, session_id, parent_id, parent_hash) =
            setup("dup.txt", "clobber");
        let effect_id = Uuid::new_v4();

        let result = invoke_file_create(
            &conn, TEST_KEY, &store, session_id, effect_id, &plan_node, &ws, parent_id, &parent_hash,
        );
        assert!(result.is_err(), "exclusive create on an existing path must fail");

        // The original file is untouched (no overwrite).
        assert_eq!(std::fs::read_to_string(root.join("dup.txt")).unwrap(), "original");

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

    // ── invoke_file_create_from_resolved (10-02 Task 3, frozen-literal re-invocation) ──

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

    /// On success, invoke_file_create_from_resolved creates the file from frozen
    /// literals (never a ValueStore) and records a chained `sink_executed` event.
    #[test]
    fn invoke_file_create_from_resolved_success_records_sink_executed() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fc_resolved_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let resolved_args = resolved_args_for("created.txt", "hi");

        let (evt_id, hash) = invoke_file_create_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        )
        .expect("invoke_file_create_from_resolved must succeed on a fresh path");

        assert!(!hash.is_empty());
        let on_disk = std::fs::read_to_string(root.join("created.txt")).unwrap();
        assert_eq!(on_disk, "hi");

        let evt = find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .unwrap()
            .expect("sink_executed event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.actor, format!("sink:file.create:{effect_id}"));
        assert_eq!(evt.parent_id, Some(parent_id));

        std::fs::remove_dir_all(&root).ok();
    }

    /// On a pre-existing target path, invoke_file_create_from_resolved records a
    /// `sink_invocation_failed` event (NOT `sink_execution_failed`) and propagates
    /// the error; the original file is left untouched.
    #[test]
    fn invoke_file_create_from_resolved_failure_records_sink_invocation_failed() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_fc_resolved_err_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("dup.txt"), b"original").unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let resolved_args = resolved_args_for("dup.txt", "clobber");

        let result = invoke_file_create_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        );
        assert!(result.is_err(), "exclusive create on an existing path must fail");

        assert_eq!(std::fs::read_to_string(root.join("dup.txt")).unwrap(), "original");

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
