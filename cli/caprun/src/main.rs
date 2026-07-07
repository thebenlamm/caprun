/// caprun — confined-worker orchestrator
///
/// The no-LLM complete-mediation proof: caprun opens the audit DB, creates a
/// Session, spawns the SINGLE unified broker dispatch (`brokerd::server::
/// run_broker_server` — it owns the abstract-socket bind + accept loop), then
/// spawns `caprun-worker` (which self-confines AFTER connecting). Every effect
/// is logged to the SQLite audit DAG with an unbroken SHA-256 hash chain.
///
/// Usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]
///
/// Intent kinds:
///   send-email-summary <recipient>  — send a workspace summary to the recipient
///   create-file-from-report <path>  — create <path> under the workspace root
///                                     (clean path → Allow; a hostile workspace-read
///                                      path → Block, per §9)
///
/// # Single dispatch authority (ASM-01)
///
/// There is exactly ONE broker dispatch path: `brokerd::server`. caprun no longer
/// contains a second worker-connection loop — the prior local dispatch handler
/// has been deleted. The worker's RequestFd / ReportClaims / SubmitPlanNode
/// protocol is handled entirely by `run_broker_server::dispatch_request`.
///
/// # Self-Confinement Order
///
/// caprun spawns the worker as a NORMAL subprocess (no confinement in pre_exec).
/// The worker connects to the broker, THEN calls sandbox::apply_confinement()
/// on itself. This self-confinement model is required because Landlock deny-all +
/// seccomp deny-execve cannot be applied before exec without preventing the
/// worker binary from loading.

use anyhow::Context;
use brokerd::{
    audit::{append_event, open_audit_db, verify_chain},
    session::{create_session, persist_session},
};
use chrono::Utc;
use runtime_core::{intent::CaprunIntent, Event, SeedProvenance};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let mut idx = 0usize;

    // ── Parse optional --seed-from-file flag BEFORE positional args (ORIGIN-01/02) ──
    // The caprun CLI is the ONLY place that decides seed-provenance (DESIGN §3):
    // presence of this flag means the intent parameter is read from external file
    // content (untrusted source) => SeedProvenance::FileDerived; absence means
    // today's trusted-argv behavior is unchanged => SeedProvenance::TrustedArg.
    // The broker (create_session) — not the CLI — turns that provenance into the
    // session's initial Draft/Active status; the CLI only forwards it.
    let seed_from_file_path: Option<String> =
        if raw_args.get(idx).map(String::as_str) == Some("--seed-from-file") {
            idx += 1;
            let path = raw_args.get(idx).cloned().ok_or_else(|| {
                anyhow::anyhow!(
                    "usage: caprun --seed-from-file <path> <intent-kind> <workspace-file> [audit-db-path]"
                )
            })?;
            idx += 1;
            Some(path)
        } else {
            None
        };

    let mut args = raw_args[idx..].iter().cloned();

    let usage = if seed_from_file_path.is_some() {
        "usage: caprun --seed-from-file <path> <intent-kind> <workspace-file> [audit-db-path]"
    } else {
        "usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]"
    };

    // ── Parse typed intent from positional args (PLAN-01) ────────────────────
    let intent_kind = args.next().ok_or_else(|| anyhow::anyhow!(usage))?;

    // The file-derived intent parameter REPLACES the positional <intent-param>
    // slot entirely (RESEARCH Pitfall 4 / A2) — no redundant positional value is
    // consumed when --seed-from-file is present. Fail-closed (V5): a missing or
    // unreadable seed file is a hard error, NEVER a silent fallback to trusted-arg.
    let (seed_provenance, intent_param) = match &seed_from_file_path {
        Some(path) => {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("--seed-from-file: failed to read {path}"))?;
            (SeedProvenance::FileDerived, content)
        }
        None => {
            let param = args.next().ok_or_else(|| anyhow::anyhow!(usage))?;
            (SeedProvenance::TrustedArg, param)
        }
    };

    let workspace_path = args.next().ok_or_else(|| anyhow::anyhow!(usage))?;
    let audit_path = args.next().unwrap_or_else(|| ":memory:".to_string());

    // Map intent kind → typed enum. Fail closed on unknown kinds (V5).
    let intent = match intent_kind.as_str() {
        "send-email-summary" => CaprunIntent::SendEmailSummary {
            recipient: intent_param,
        },
        "create-file-from-report" => CaprunIntent::CreateFileFromReport {
            path: intent_param,
        },
        _ => anyhow::bail!("unknown intent kind: {intent_kind}"),
    };

    // ── 1. Open audit DB ────────────────────────────────────────────────────
    let conn = Arc::new(Mutex::new(
        open_audit_db(&audit_path).context("open_audit_db")?,
    ));

    // ── 1b. Open the workspace-root dirfd capability ONCE (HARD-04) ──────────
    // RESEARCH Q2 Option (a): derive the workspace ROOT from the workspace-file
    // parent and hand the worker a root-RELATIVE basename — zero new CLI surface.
    // The broker resolves every RequestFd read BENEATH this dirfd via
    // openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS); the broker never again opens
    // a worker-supplied path via ambient std::fs::File::open.
    let ws_path = Path::new(&workspace_path);
    let workspace_root_dir = match ws_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let workspace_rel = ws_path.file_name().ok_or_else(|| {
        anyhow::anyhow!("workspace-file has no file name: {workspace_path}")
    })?;
    let workspace_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(workspace_root_dir)
            .context("open workspace root")?,
    );

    // ── 2. Create session + persist + append session_created event ──────────
    // The CLI decides seed_provenance (above); create_session (broker-side,
    // Plan 03) is the ONLY place that turns provenance into the session's
    // initial SessionStatus (Draft for FileDerived, Active for TrustedArg) —
    // the CLI never self-declares status (DESIGN §3).
    let intent_id = Uuid::new_v4();
    let session = create_session(intent_id, seed_provenance.clone());
    let session_id = session.id;
    let initial_session_status = session.status.clone();

    // ORIGIN-01: record the seed-provenance determination in the
    // session_created Event's actor field — Event carries no free-form
    // metadata field (its serialized form IS the hashed audit payload), so the
    // provenance tag rides in `actor`, exhaustively matched (mirrors the
    // broker's own in-process CreateSession IPC arm in server.rs).
    let session_created_id = Uuid::new_v4();
    let actor = match seed_provenance {
        SeedProvenance::TrustedArg => "broker:seed_provenance=trusted_arg",
        SeedProvenance::FileDerived => "broker:seed_provenance=file_derived",
    };
    let e_session = Event::new(
        session_created_id,
        None,
        session_id,
        actor.into(),
        "session_created".into(),
        Utc::now(),
        vec![],
    );

    let session_created_hash = {
        let locked = conn.lock().unwrap();
        persist_session(&locked, &session).context("persist_session")?;
        append_event(&locked, &e_session, None).context("append session_created")?
    };

    // ── 3. Spawn the unified broker server ───────────────────────────────────
    // run_broker_server owns the abstract-socket bind AND the accept loop (the
    // single dispatch authority — ASM-01). It binds `\0/agentos/{session_id}`
    // synchronously at the top of the task, before the worker process can connect.
    let conn_clone = conn.clone();
    let ws_root_for_broker = workspace_root.clone();
    let broker_task = tokio::spawn(async move {
        brokerd::server::run_broker_server(
            &session_id.to_string(),
            conn_clone,
            session_id,
            session_created_id,
            session_created_hash,
            initial_session_status,
            ws_root_for_broker,
        )
        .await
    });
    // Let the broker task reach its synchronous bind() before we spawn the worker.
    // (Process spawn latency alone is far larger than the time to bind, but this
    // makes the ordering explicit.)
    tokio::task::yield_now().await;

    // ── 4. Spawn caprun-worker (NORMAL spawn — worker self-confines after connecting)
    let worker_binary = std::env::current_exe()
        .context("current_exe")?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("caprun has no parent dir"))?
        .join("caprun-worker");
    let mut child = std::process::Command::new(&worker_binary)
        // Abstract socket name WITHOUT the leading NUL (worker prepends it)
        .env("BROKER_SOCK", format!("/agentos/{session_id}"))
        .env("SESSION_ID", session_id.to_string())
        // Root-RELATIVE basename: the worker echoes this verbatim into
        // RequestFd { path }, which the broker resolves BENEATH the workspace
        // dirfd (HARD-04). Sending the full path would defeat RESOLVE_BENEATH.
        .env("WORKSPACE_FILE", workspace_rel)
        // Serialised CaprunIntent — worker deserialises this and sends ProvideIntent
        // to the broker, which mints it authoritatively in the per-connection
        // ValueStore. Never passed raw bytes here; always the typed intent enum.
        .env("INTENT", serde_json::to_string(&intent).context("serialise intent")?)
        .spawn()
        .context("spawn caprun-worker")?;

    // ── 5. Wait for caprun-worker process exit ───────────────────────────────
    // spawn_blocking so child.wait() (blocking) doesn't stall the tokio reactor.
    // All audit writes complete before the worker exits (the broker writes each
    // event and sends its response before the worker proceeds), so by the time
    // wait() returns the DAG is fully durable.
    let child_status = tokio::task::spawn_blocking(move || child.wait())
        .await
        .context("spawn_blocking child.wait")?
        .context("child.wait")?;

    // ── 6. Stop the broker accept loop ───────────────────────────────────────
    // run_broker_server loops forever accepting connections; the worker is done,
    // so abort the task. All audit writes are already durable (see step 5).
    broker_task.abort();

    // ── 7. Print audit DAG to stdout + verify the hash chain ─────────────────
    {
        let locked = conn.lock().unwrap();
        print_audit_dag(&locked, &session_id.to_string())?;
        let chain_ok = verify_chain(&locked, &session_id.to_string());
        println!(
            "\nChain verification: {}",
            if chain_ok { "PASSED" } else { "FAILED" }
        );
    }

    // Non-success propagation: a §9 block makes the worker exit non-zero, which
    // must surface as a non-zero caprun exit (BEFORE any effect runs).
    if !child_status.success() {
        anyhow::bail!("caprun-worker exited with status: {child_status}");
    }

    Ok(())
}

/// Print the audit DAG for `session_id` in causal order (depth-first CTE walk).
fn print_audit_dag(conn: &rusqlite::Connection, session_id: &str) -> anyhow::Result<()> {
    println!("\n=== Audit DAG (session {session_id}) ===");
    let mut stmt = conn.prepare(
        "WITH RECURSIVE chain(id, event_type, actor, hash, parent_hash, depth) AS (
             SELECT id, event_type, actor, hash, parent_hash, 0
             FROM events
             WHERE session_id = ?1 AND parent_id IS NULL
           UNION ALL
             SELECT e.id, e.event_type, e.actor, e.hash, e.parent_hash, c.depth + 1
             FROM events e
             JOIN chain c ON e.parent_id = c.id
             WHERE e.session_id = ?1
         )
         SELECT depth, event_type, actor, hash, parent_hash
         FROM chain
         ORDER BY depth",
    )?;

    let rows = stmt.query_map([session_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for row in rows {
        let (depth, event_type, actor, hash, parent_hash): (i64, String, String, String, Option<String>) = row?;
        let indent = "  ".repeat(depth as usize);
        let parent_str = parent_hash.as_deref().map(|h| &h[..8]).unwrap_or("(root)");
        println!(
            "{indent}[{depth}] {event_type} (actor={actor})\n\
             {indent}    hash={} parent={parent_str}",
            &hash[..8]
        );
    }
    Ok(())
}
