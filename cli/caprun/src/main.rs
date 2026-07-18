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

// Cross-process MAC-key custody + F1 fail-closed startup refusal (HARDEN-02).
// Plan 03 wires the `caprun run` path (below, step 1c) AND — as a necessary
// consequence of threading `key: &[u8]` through `confirm()`/`deny()`'s
// signatures (so `cli/caprun/tests/confirm.rs`'s existing macOS-run
// cross-process suite keeps passing under the keyed chain) — a MINIMAL
// key-load in `run_confirm_or_deny` for both verbs too. Plan 05 (v1.6 Phase
// 28, X-02) completed the REMAINING confirm/deny wiring on top of this: the
// `pending_confirmations` whole-row MAC fold, the MAC-verify-before-
// terminal-state gate, and deny()'s NEW `verify_chain` gate — `deny()` now
// carries the SAME integrity gates `confirm()` has.
mod key;

/// Trusted default `subject`/`body` for a `send-email-summary` intent (Phase
/// 15 finding #6). Deliberately NOT a new CLI flag (this plan's DEFERRED
/// note) — always user-trusted by construction (`SeedProvenance::TrustedArg`
/// path), never doc/file-derived.
const DEFAULT_EMAIL_SUBJECT: &str = "Workspace Summary";
const DEFAULT_EMAIL_BODY: &str = "Please see the attached workspace summary.";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    // ── confirm/deny/review dispatch — VERY FIRST branch, before ANYTHING else ──
    // (RESEARCH Pitfall 6): `caprun confirm <effect_id> [audit-db-path]` /
    // `caprun deny <effect_id> [audit-db-path]` / `caprun review <effect_id>
    // [audit-db-path]` (MAJOR-8 read-only pre-decision surface) have a
    // completely different arg shape than `<intent-kind> <intent-param>
    // <workspace-file> [audit-db-path]`, so this MUST be checked before the
    // `--seed-from-file` pre-parse below and before the intent-kind match.
    // Handled and the process exits explicitly here — it never falls through
    // to the intent-kind parse.
    if let Some(verb) = raw_args.first().map(String::as_str) {
        if verb == "confirm" || verb == "deny" || verb == "review" {
            let usage = format!("usage: caprun {verb} <effect_id> [audit-db-path]");
            let effect_id = match raw_args.get(1) {
                Some(id) => id.as_str(),
                None => {
                    eprintln!("{usage}");
                    std::process::exit(1);
                }
            };
            // Fail-closed: a malformed UUID is a usage error (exit 1) — never a
            // silent pass-through into find_pending_confirmation (which would
            // instead report the weaker "unknown effect_id" outcome).
            if uuid::Uuid::parse_str(effect_id).is_err() {
                eprintln!("error: <effect_id> is not a valid UUID: {effect_id}");
                std::process::exit(1);
            }
            // Defaults to ":memory:" like the existing audit_path convention —
            // against :memory: no persisted row can exist, so this fails closed
            // as UnknownEffect (safe; never a silent no-op success).
            let audit_path = raw_args
                .get(2)
                .cloned()
                .unwrap_or_else(|| ":memory:".to_string());
            let code = match run_confirm_or_deny(verb, effect_id, &audit_path).await {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("error: {e}");
                    1
                }
            };
            std::process::exit(code);
        }

        // ── grant dispatch (v1.8 Phase 38, GITHUB-02) ──────────────────────
        // `caprun grant <session_id> [audit-db-path]` records the durable,
        // session-scoped github.pr auth-grant. It is a DISTINCT human action
        // from confirm/deny (a bearer token's authority exceeds one PR —
        // DESIGN §4.3, FORK 3), so it gets its OWN verb and its OWN helper
        // (`run_grant`), never folded into `run_confirm_or_deny`. Arg is a
        // SESSION id (not an effect_id) — the capability is session-scoped.
        // Handled here, before the intent-kind parse, and exits explicitly.
        if verb == "grant" {
            let usage = "usage: caprun grant <session_id> [audit-db-path]";
            let session_id = match raw_args.get(1) {
                Some(s) => s.as_str(),
                None => {
                    eprintln!("{usage}");
                    std::process::exit(1);
                }
            };
            // Fail-closed: a malformed session id is a usage error (exit 1),
            // never a silent pass-through into record_github_grant (which
            // would otherwise persist a grant keyed by a non-UUID string).
            if uuid::Uuid::parse_str(session_id).is_err() {
                eprintln!("error: <session_id> is not a valid UUID: {session_id}");
                std::process::exit(1);
            }
            let audit_path = raw_args
                .get(2)
                .cloned()
                .unwrap_or_else(|| ":memory:".to_string());
            let code = match run_grant(session_id, &audit_path) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("error: {e}");
                    1
                }
            };
            std::process::exit(code);
        }
    }

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
    //
    // `subject`/`body` (Phase 15 finding #6) have no CLI surface of their own —
    // deliberately, per this plan's DEFERRED note (no new CLI flags) — so they
    // are always these trusted default constants, never file/doc-derived.
    // They are still minted as their OWN DISTINCT UserTrusted ValueRecords by
    // the broker's ProvideIntent arm (see server.rs), never degenerately
    // reusing the recipient's handle.
    let intent = match intent_kind.as_str() {
        "send-email-summary" => CaprunIntent::SendEmailSummary {
            recipient: intent_param,
            subject: DEFAULT_EMAIL_SUBJECT.to_string(),
            body: DEFAULT_EMAIL_BODY.to_string(),
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
    // v1.6 Phase 27 (HARDEN-01, DESIGN-security-hardening.md §a): the ONE
    // trusted-path identity threaded into the broker so its `RequestFd`
    // arm's fstat inode-identity compare has a trusted target — an OWNED
    // `PathBuf` derived from the SAME CLI-designated `<workspace-file>` arg
    // that anchors `workspace_root_dir`/`workspace_rel` above. This is the
    // second forwarding hop the DESIGN doc's blast-radius note requires
    // (the first, existing hop is `.env("WORKSPACE_FILE", workspace_rel)`
    // below, which the worker consumes — this one reaches the broker).
    let trusted_workspace_path: std::path::PathBuf = ws_path.to_path_buf();

    // ── 1c. Load (or create) the broker-owned MAC key (v1.6 Phase 28, HARDEN-02) ─
    // AFTER workspace_root_dir is derived (F1 needs a workspace root to check
    // containment against) and BEFORE the broker task is spawned (every
    // broker-internal append_event/verify_chain needs this key in scope from
    // its very first call). `load_or_create_key` is F1-checked (fail-closed
    // refusal if the audit DB or its `.key` sibling resolves beneath the
    // workspace root — the confined worker's PRIMARY reach) and idempotent
    // (a later, separate `caprun confirm`/`deny` process reads the SAME
    // persisted key back). Converted to a fixed `[u8; 32]` array — the DESIGN
    // doc's pinned key length — then wrapped in an `Arc` so it can be cloned
    // per-connection exactly like `conn` (server.rs's accepted Step-C
    // fallback: threaded as a sibling of the `conn` Arc, never bundled onto
    // the connection handle, since `conn`'s locked guard is consumed as a
    // bare `&rusqlite::Connection` by many non-audit call sites).
    let mac_key_bytes = key::load_or_create_key(&audit_path, workspace_root_dir)
        .context("load_or_create_key (F1 fail-closed MAC-key custody)")?;
    let mac_key: std::sync::Arc<[u8; 32]> = std::sync::Arc::new(
        mac_key_bytes
            .try_into()
            .map_err(|v: Vec<u8>| anyhow::anyhow!("MAC key must be 32 bytes, got {}", v.len()))?,
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
        append_event(&locked, &mac_key[..], &e_session, None).context("append session_created")?
    };

    // ── 3. Spawn the unified broker server ───────────────────────────────────
    // run_broker_server owns the abstract-socket bind AND the accept loop (the
    // single dispatch authority — ASM-01). It binds `\0/agentos/{session_id}`
    // synchronously at the top of the task, before the worker process can connect.
    let conn_clone = conn.clone();
    let ws_root_for_broker = workspace_root.clone();
    let mac_key_for_broker = mac_key.clone();
    let broker_task = tokio::spawn(async move {
        brokerd::server::run_broker_server(
            &session_id.to_string(),
            conn_clone,
            session_id,
            session_created_id,
            session_created_hash,
            initial_session_status,
            ws_root_for_broker,
            trusted_workspace_path,
            mac_key_for_broker,
        )
        .await
    });
    // Let the broker task reach its synchronous bind() before we spawn the worker.
    // (Process spawn latency alone is far larger than the time to bind, but this
    // makes the ordering explicit.)
    tokio::task::yield_now().await;

    // ── 3b. Spawn the LLM planner sidecar when CAPRUN_PLANNER=llm (Phase 21) ─
    // Only when explicitly selected — CAPRUN_PLANNER unset (or any other
    // value) spawns NO sidecar and passes NOTHING new into the worker's env,
    // so the default deterministic path is byte-for-byte unchanged (no
    // regression). Resolved via current_exe().parent() exactly like the
    // worker binary below (step 4) — caprun-planner lives alongside
    // caprun/caprun-worker. Spawned BEFORE the worker (step 4) so the sidecar
    // has a head start on its own bind(); the worker-side `LlmPlanner` proxy
    // still carries its own bounded connect-retry (cli/caprun/src/planner.rs)
    // to cover any residual scheduling race. `OPENAI_API_KEY` is forwarded to
    // the sidecar ONLY — the worker's env never receives it (T-21-10). This
    // holds by construction: the worker spawn below (step 4) `env_clear()`s and
    // passes only an explicit non-secret allowlist, so the worker inherits
    // NEITHER the explicitly-set key NOR any ambient broker secret.
    let mut planner_sidecar: Option<std::process::Child> = None;
    let mut worker_planner_env: Vec<(&'static str, String)> = Vec::new();
    if std::env::var("CAPRUN_PLANNER").as_deref() == Ok("llm") {
        let planner_sock = format!("/agentos/planner/{session_id}");
        let planner_binary = std::env::current_exe()
            .context("current_exe")?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("caprun has no parent dir"))?
            .join("caprun-planner");
        let mut cmd = std::process::Command::new(&planner_binary);
        cmd.env("PLANNER_SOCK", &planner_sock);
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            cmd.env("OPENAI_API_KEY", key);
        }
        if let Ok(model) = std::env::var("CAPRUN_PLANNER_MODEL") {
            cmd.env("CAPRUN_PLANNER_MODEL", model);
        }
        let child = cmd.spawn().context("spawn caprun-planner sidecar")?;
        planner_sidecar = Some(child);
        worker_planner_env.push(("PLANNER_SOCK", planner_sock));
        worker_planner_env.push(("CAPRUN_PLANNER", "llm".to_string()));
    }

    // ── 4. Spawn caprun-worker (NORMAL spawn — worker self-confines after connecting)
    let worker_binary = std::env::current_exe()
        .context("current_exe")?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("caprun has no parent dir"))?
        .join("caprun-worker");
    let mut child = std::process::Command::new(&worker_binary)
        // SECURITY (Phase 34 gap-closure, env_clear adversarial-review follow-up):
        // clear the inherited environment so the confined worker receives NONE of
        // the unconfined caprun process's env — notably OPENAI_API_KEY and
        // CAPRUN_SMTP_*. The worker processes untrusted content by design (I1
        // dynamic taint), so a prompt-injected worker could otherwise `getenv` a
        // secret (a pure memory read seccomp/Landlock cannot block) and embed it
        // into a draft-email body or an artifact write. Pass ONLY the explicit,
        // non-secret vars the worker actually reads (BROKER_SOCK, SESSION_ID,
        // WORKSPACE_FILE, INTENT, + the optional planner seam) plus a minimal PATH.
        // This makes the T-21-10 guarantee ("the worker's env never receives
        // OPENAI_API_KEY") TRUE by construction — it was previously false via
        // inheritance. The worker never `execve`s (seccomp-denied), so PATH is
        // belt-and-suspenders, not required.
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin")
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
        // Propagates PLANNER_SOCK + CAPRUN_PLANNER=llm ONLY when the sidecar
        // was spawned above (step 3b) — empty otherwise, so the default path
        // sees this call site add nothing (no regression).
        .envs(worker_planner_env)
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

    // ── 5b. Tear down the planner sidecar (mirrors broker_task.abort() below) ─
    // The worker has exited; the sidecar (if spawned) is no longer needed.
    // spawn_blocking so the blocking kill()/wait() doesn't stall the reactor.
    if let Some(mut planner_child) = planner_sidecar {
        let _ = tokio::task::spawn_blocking(move || {
            let _ = planner_child.kill();
            let _ = planner_child.wait();
        })
        .await;
    }

    // ── 6. Stop the broker accept loop ───────────────────────────────────────
    // run_broker_server loops forever accepting connections; the worker is done,
    // so abort the task. All audit writes are already durable (see step 5).
    broker_task.abort();

    // ── 7. Print audit DAG to stdout + verify the hash chain ─────────────────
    {
        let locked = conn.lock().unwrap();
        print_audit_dag(&locked, &session_id.to_string())?;
        let chain_ok = verify_chain(&locked, &session_id.to_string(), &mac_key[..]);
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

/// Parse args, open the SAME persistent audit DB the original `caprun` run
/// used, dispatch into `brokerd::confirmation`'s decision logic, and map the
/// outcome to the DESIGN Exit-code contract's distinct exit code. Extracted so
/// `main` stays readable and the logic is unit-reachable — mirrors the "parse
/// args, open DB, call into brokerd, map result" role `main` already plays for
/// the intent-kind flow.
///
/// `confirm` needs the workspace root the block was resolved against
/// (`PendingConfirmation.workspace_root_path`); `deny` needs no workspace root
/// at all, since it never invokes any sink (CONFIRM-03). `review` (MAJOR-8)
/// likewise never opens the workspace root — it is a read-only pre-decision
/// display and MUST NOT invoke any sink either.
async fn run_confirm_or_deny(verb: &str, effect_id: &str, audit_path: &str) -> anyhow::Result<i32> {
    use brokerd::confirmation::{confirm, deny, find_pending_confirmation, review, ConfirmOutcome};

    let mut conn = open_audit_db(audit_path).context("open_audit_db")?;

    let outcome = match verb {
        "confirm" => {
            // find_pending_confirmation itself returns None → confirm()'s
            // UnknownEffect for an absent row — this is what makes an omitted
            // audit-db-path (defaulting to :memory:) fail closed rather than
            // panic: an in-memory DB simply has no row, ever.
            match find_pending_confirmation(&conn, effect_id)? {
                None => ConfirmOutcome::UnknownEffect,
                Some(pc) => {
                    let ws = adapter_fs::workspace::WorkspaceRoot::open(Path::new(
                        &pc.workspace_root_path,
                    ))
                    .context("open workspace root for confirm")?;
                    // v1.6 Phase 28 (HARDEN-02): load the SAME F1-checked,
                    // cross-process-stable broker key the original `caprun
                    // run` process persisted (Plan 02's `load_or_create_key`)
                    // — required so `confirm()`'s keyed `verify_chain` gate
                    // (Step 4.5a) verifies true against a chain appended by a
                    // DIFFERENT OS process. This is the minimal key-load half
                    // of Plan 05 Task 2's "run_confirm_or_deny key wiring";
                    // Plan 05 still owns the pending_confirmations MAC-verify
                    // gate that runs BEFORE this point.
                    let key = key::load_or_create_key(
                        audit_path,
                        Path::new(&pc.workspace_root_path),
                    )
                    .context("load_or_create_key (F1 fail-closed MAC-key custody, confirm)")?;
                    confirm(&mut conn, &key, effect_id, &ws).await?
                }
            }
        }
        "deny" => {
            // deny() itself needs no workspace root for its OWN logic
            // (CONFIRM-03 — it never invokes a sink), but v1.6 Phase 28
            // (HARDEN-02) needs `pc.workspace_root_path` to run the SAME F1
            // key-load `confirm` runs above, so a `caprun deny` process
            // agrees with the original `caprun run` process's broker key.
            // An unknown effect_id short-circuits here (no row to derive a
            // workspace root from) rather than falling through to deny()'s
            // own (now key-requiring) internal lookup.
            match find_pending_confirmation(&conn, effect_id)? {
                None => ConfirmOutcome::UnknownEffect,
                Some(pc) => {
                    let key = key::load_or_create_key(
                        audit_path,
                        Path::new(&pc.workspace_root_path),
                    )
                    .context("load_or_create_key (F1 fail-closed MAC-key custody, deny)")?;
                    deny(&conn, &key, effect_id)?
                }
            }
        }
        // review (MAJOR-8): read-only — never opens the workspace root, never
        // invokes any sink, never transitions state.
        "review" => review(&conn, effect_id)?,
        other => anyhow::bail!("run_confirm_or_deny: unreachable verb `{other}`"),
    };

    // Exit-code contract (DESIGN "caprun confirm CLI Contract" — each outcome
    // distinguishable by code alone, no stdout parsing required).
    let (code, message): (i32, Option<&str>) = match outcome {
        ConfirmOutcome::Released => (0, None),
        ConfirmOutcome::Denied => (2, Some("denied")),
        ConfirmOutcome::ConfirmedButSinkFailed => {
            (3, Some("confirmed, but the sink invocation failed"))
        }
        ConfirmOutcome::UnknownEffect => (4, Some("unknown effect_id")),
        ConfirmOutcome::AlreadyTerminal => (5, Some("effect_id is already terminal")),
        ConfirmOutcome::BlockedLiteralRedacted => {
            (6, Some("blocked literal was redacted; refusing to release"))
        }
        ConfirmOutcome::EmailSendFailed => {
            (7, Some("email send failed after confirm; recorded, not retried"))
        }
        ConfirmOutcome::Reviewed => (0, None),
        ConfirmOutcome::DigestMismatch => (
            8,
            Some(
                "integrity check failed (broken audit chain or digest mismatch); \
                 refusing to release",
            ),
        ),
    };
    if let Some(msg) = message {
        eprintln!("caprun {verb}: {msg}");
    }
    Ok(code)
}

/// Record a durable, session-scoped github.pr auth-grant (v1.8 Phase 38,
/// GITHUB-02, DESIGN §4.3, FORK 3) — the WRITE side of the NEW capability
/// model both github.pr dispatch paths (Plans 38-04/38-05) gate on.
///
/// Mirrors `run_confirm_or_deny`'s "open DB, load the F1 fail-closed
/// cross-process MAC key, call into brokerd, map to an exit code" shape, but
/// for the grant capability rather than a per-effect confirm. Loads the SAME
/// broker key `caprun run` persisted at `<audit_path>.key` (via the shared
/// custody helper) so the appended `github_grant_authorized` event chains
/// consistently onto the session's existing keyed chain, then prints the
/// minimal-PAT operator notice and exits 0.
fn run_grant(session_id: &str, audit_path: &str) -> anyhow::Result<i32> {
    let conn = open_audit_db(audit_path).context("open_audit_db")?;
    let key = load_grant_key(audit_path)?;

    brokerd::audit::record_github_grant(&conn, &key, session_id)
        .context("record_github_grant")?;

    println!("caprun grant: github.pr auth-grant recorded for session {session_id}");
    println!(
        "OPERATOR NOTICE: the PAT in CAPRUN_GITHUB_TOKEN MUST be minimal-scope — a \
         fine-grained token with 'Pull requests: write' + 'Contents: read' ONLY \
         (DESIGN-git-github-http-sinks.md §4.4/§11). A broader token grants \
         arbitrary egress to a credential-bearing child."
    );
    Ok(0)
}

/// Load the cross-process broker MAC key for a `caprun grant`, via the SAME F1
/// fail-closed custody helper `run_confirm_or_deny` uses (`key::
/// load_or_create_key`) — never a hand-rolled key reader.
///
/// `caprun grant` has no workspace file of its own, so it has no
/// `PendingConfirmation.workspace_root_path` to feed the F1 containment check
/// (unlike confirm/deny). For a persistent DB we therefore hand F1 a dedicated
/// throwaway workspace root that is a SIBLING of the audit DB
/// (`<audit_path>.grant-ws`, never an ANCESTOR of it), which satisfies F1 by
/// construction — the audit DB and its `.key` sibling are NOT beneath it — so
/// the helper reads back the EXISTING key (the load-bearing cross-process
/// stability: the grant event must chain under the same key as the rest of
/// the session). The dir is created only for the `canonicalize()`-based
/// containment check and removed immediately after. For `:memory:` the helper
/// returns an ephemeral key before any F1 check (no persisted chain to stay
/// consistent with), so the workspace root is irrelevant.
fn load_grant_key(audit_path: &str) -> anyhow::Result<Vec<u8>> {
    if audit_path == ":memory:" {
        return key::load_or_create_key(audit_path, Path::new(".")).context(
            "load_or_create_key (F1 fail-closed MAC-key custody, grant, :memory:)",
        );
    }
    let grant_ws = std::path::PathBuf::from(format!("{audit_path}.grant-ws"));
    std::fs::create_dir_all(&grant_ws)
        .with_context(|| format!("create grant workspace dir {}", grant_ws.display()))?;
    let key = key::load_or_create_key(audit_path, &grant_ws)
        .context("load_or_create_key (F1 fail-closed MAC-key custody, grant)");
    std::fs::remove_dir_all(&grant_ws).ok();
    key
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

#[cfg(test)]
mod tests {
    use super::planner_sidecar_allowlist_env;

    /// Serializes the tests in this module that mutate the process-global
    /// environment — the multi-threaded test runner would otherwise race two
    /// tests on the same process-wide env (mirror `github_pr.rs`'s
    /// `GITHUB_ENV_LOCK` / `email_smtp.rs`'s `SMTP_ENV_LOCK`).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Snapshots the env keys a test touches, clears them to a known baseline,
    /// and restores the originals on drop — so a test never leaks state into a
    /// sibling and a pre-existing shell env (e.g. a real HTTPS_PROXY) can't skew
    /// the "absent" assertions.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }
    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            let saved = keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
            for k in keys {
                std::env::remove_var(k);
            }
            EnvGuard { saved }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.saved {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    /// All keys any test in this module reads or writes — the guard baselines
    /// every one so each test sees a clean slate regardless of the ambient env.
    const TOUCHED: &[&str] = &[
        "CAPRUN_PLANNER_MODEL",
        "HTTPS_PROXY",
        "NO_PROXY",
        "CAPRUN_SMTP_HOST",
        "CAPRUN_SMTP_PORT",
        "OPENAI_API_KEY",
    ];

    fn get<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
        env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    #[test]
    fn planner_sidecar_allowlist_returns_planner_sock() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _env = EnvGuard::new(TOUCHED);
        let out = planner_sidecar_allowlist_env("/agentos/planner/abc-123");
        assert_eq!(get(&out, "PLANNER_SOCK"), Some("/agentos/planner/abc-123"));
    }

    #[test]
    fn planner_sidecar_allowlist_always_sets_minimal_path() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _env = EnvGuard::new(TOUCHED);
        let out = planner_sidecar_allowlist_env("/sock");
        assert_eq!(get(&out, "PATH"), Some("/usr/bin:/bin:/usr/local/bin"));
    }

    #[test]
    fn planner_sidecar_allowlist_forwards_model_when_present_omits_when_absent() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _env = EnvGuard::new(TOUCHED);

        // Absent → omitted.
        let out = planner_sidecar_allowlist_env("/sock");
        assert_eq!(get(&out, "CAPRUN_PLANNER_MODEL"), None);

        // Present → forwarded verbatim.
        std::env::set_var("CAPRUN_PLANNER_MODEL", "gpt-4o-mini");
        let out = planner_sidecar_allowlist_env("/sock");
        assert_eq!(get(&out, "CAPRUN_PLANNER_MODEL"), Some("gpt-4o-mini"));

        // Present-but-empty → omitted (never forwarded as an empty string;
        // mirrors the mailpit-verify.sh empty-model bug note).
        std::env::set_var("CAPRUN_PLANNER_MODEL", "");
        let out = planner_sidecar_allowlist_env("/sock");
        assert_eq!(get(&out, "CAPRUN_PLANNER_MODEL"), None);
    }

    #[test]
    fn planner_sidecar_allowlist_forwards_proxy_when_present_omits_when_absent() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _env = EnvGuard::new(TOUCHED);

        // Absent → both omitted.
        let out = planner_sidecar_allowlist_env("/sock");
        assert_eq!(get(&out, "HTTPS_PROXY"), None);
        assert_eq!(get(&out, "NO_PROXY"), None);

        // Present → forwarded.
        std::env::set_var("HTTPS_PROXY", "http://proxy.example:8080");
        std::env::set_var("NO_PROXY", "localhost,127.0.0.1");
        let out = planner_sidecar_allowlist_env("/sock");
        assert_eq!(get(&out, "HTTPS_PROXY"), Some("http://proxy.example:8080"));
        assert_eq!(get(&out, "NO_PROXY"), Some("localhost,127.0.0.1"));
    }

    #[test]
    fn planner_sidecar_allowlist_never_leaks_smtp_or_secret() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _env = EnvGuard::new(TOUCHED);

        // Ambient broker secrets present in the PARENT env...
        std::env::set_var("CAPRUN_SMTP_HOST", "10.0.0.5");
        std::env::set_var("CAPRUN_SMTP_PORT", "1025");
        std::env::set_var("OPENAI_API_KEY", "sk-super-secret");

        let out = planner_sidecar_allowlist_env("/sock");

        // ...must NEVER appear in the non-secret allowlist builder.
        assert_eq!(get(&out, "CAPRUN_SMTP_HOST"), None);
        assert_eq!(get(&out, "CAPRUN_SMTP_PORT"), None);
        // OPENAI_API_KEY is the ONLY secret and is set on the Command separately,
        // never enumerated by this non-secret builder.
        assert_eq!(get(&out, "OPENAI_API_KEY"), None);

        // The surviving keys are EXACTLY the documented allowlist (no ambient var
        // outside it survives).
        let mut keys: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["PATH", "PLANNER_SOCK"]);
    }
}
