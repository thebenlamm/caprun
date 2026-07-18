/// sinks/process_exec — mediated `process.exec` sink (broker-spawned confined child).
///
/// Spawns `caprun-exec-launcher` (32-03) — never the worker — as an async,
/// cancellable child via `tokio::process::Command`. The launcher self-confines
/// (rlimits -> Landlock exec-child ruleset -> seccomp exec-child filter) in its
/// OWN address space, THEN self-replaces via `execve` into the target command
/// (Option B, DESIGN §1.3). This module never spawns the target directly and
/// never mints the captured output as a taint-tracked value — see below.
///
/// # Two-phase durable audit (mirrors `invoke_file_create`, `file_create.rs:1-116`)
///
/// The authorizing `plan_node_evaluated` event is persisted by the caller
/// BEFORE this function runs. This function then performs the effect (spawn +
/// capture + wait) and records its outcome durably:
///   * success (child spawned, ran, exited within the wall-clock timeout, and
///     combined output stayed within the byte cap) -> a `process_exited` event
///     tainted `[ExternalUntrusted, ExecRaw]` (this event's id is the mint-root
///     `mint_from_exec`, 32-05, chains its `provenance_chain[0]` onto — one
///     event, both roles, per DESIGN §2.1/locked-decision, no stapling).
///   * failure (spawn error, wall-clock timeout, byte-cap exceeded, or a
///     `wait()` OS error) -> a `process_spawn_failed` event (untainted — no
///     output was genuinely captured), THEN the original error is propagated.
///     There is NO automatic retry (mirrors `sink_execution_failed`, T-07-45).
///
/// The `effect_id` is carried in the `actor` field (`sink:process.exec:<effect_id>`)
/// for the same reason `file_create.rs` does this — `Event` has no `effect_id`
/// column (DESIGN §5, no DB migration).
///
/// # Gate 3 — this module NEVER mints
///
/// `invoke_process_exec` returns `(process_exited event_id, hash, combined_output)`
/// on success — it does NOT call the value-store mint entry point or the
/// exec-mint helper here. The mint call site is pinned to `server.rs` by
/// DESIGN §2.4 (Round-1 finding M1): this module is NOT in
/// `check-invariants.sh` Gate 3's sanctioned-loci allow-list, so a mint call
/// here would fail the very extension DESIGN §2.4 mandates for 32-05.
///
/// # Locking discipline — never hold the conn mutex across an `.await`
///
/// `std::sync::MutexGuard` is not `Send`, so the compiler forbids holding one
/// across an await point anyway — but the discipline is deliberate, not
/// accidental: the entire spawn/capture/timeout sequence runs lock-free, and
/// the mutex is acquired ONLY for the brief, synchronous `append_event` calls
/// at the very end of each path.
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode, TaintLabel};
use tokio::io::AsyncReadExt;
use tokio::process::Command as TokioCommand;
use uuid::Uuid;

use adapter_fs::workspace::WorkspaceRoot;

use crate::audit::append_event;
use crate::confirmation::ResolvedArg;

/// Combined stdout+stderr byte cap (DESIGN §1.4 Open Q5: "a sane default, on
/// the order of 10 MiB"). Exceeding this is fail-closed (T-32-13) — reading
/// stops immediately and the whole invocation is treated as a failure, never
/// a silent truncate-and-keep-going past the cap.
const MAX_COMBINED_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Wall-clock timeout bounding the whole spawn+wait (T-32-12). `RLIMIT_CPU`
/// (`crates/sandbox/src/rlimits.rs:5,23`, 30 CPU-seconds) bounds CPU-time
/// consumed, NOT wall-clock elapsed time — a child that blocks on I/O or
/// sleeps evades it entirely. This constant is a NEW, independent bound;
/// its value matches the existing `RLIMIT_CPU` ceiling (30s) as a documented
/// deployment constant, not a re-derivation of it.
const EXEC_WALL_CLOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Invoke the live `process.exec` sink for an `Allowed` plan node.
///
/// Resolves `command`/`args`/`cwd` from the broker-owned `ValueStore`, spawns
/// `caprun-exec-launcher` (resolved as a sibling of the running `caprun`
/// binary), captures its combined stdout+stderr within a wall-clock timeout
/// and byte cap, and records the two-phase durable audit event. Returns
/// `(process_exited event_id, hash, combined_output)` on success — the mint
/// of `combined_output` into a taint-tracked `ValueId` is 32-05's
/// `mint_from_exec`, called from `server.rs` (DESIGN §2.4), NOT from here.
///
/// # Arguments
/// * `conn`           — the shared, mutex-guarded broker audit-db connection
///   (concurrent broker tasks share this ONE connection; unlike
///   `invoke_file_create`'s pre-locked `&Connection`, this async function
///   must acquire the lock itself, and only for the final synchronous
///   `append_event` call — never across the `.await`ed spawn/capture below).
/// * `key`            — the broker-owned audit-chain MAC key.
/// * `value_store`    — the per-connection `ValueStore` (resolves arg handles).
/// * `session_id`     — the active broker session.
/// * `effect_id`      — the broker-minted effect identity (carried in the actor).
/// * `plan_node`      — the `process.exec` plan node (opaque-handle args only).
/// * `workspace_root` — the workspace root; the launcher independently
///   resolves its own Landlock allow-rule from `workspace_root.root_path()`
///   (it cannot share the broker's in-process dirfd — separate process).
/// * `parent_id`      — causal predecessor event id (`plan_node_evaluated`).
/// * `parent_hash`    — hash of that predecessor row (chain anchor).
///
/// # Returns
/// `(event_id, hash, combined_output)` of the appended `process_exited` event
/// on success.
///
/// # Errors
/// On any spawn/exec/timeout/cap failure a `process_spawn_failed` event is
/// durably appended FIRST, then the original error is propagated (no retry).
#[allow(clippy::too_many_arguments)]
pub async fn invoke_process_exec(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    key: &[u8],
    value_store: &ValueStore,
    session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String, String)> {
    // Resolve args from the broker-owned store. validate_schema (Step 0 of
    // submit_plan_node) already guaranteed `command` is present and known —
    // a missing/dangling handle here is a broker-internal invariant violation.
    let command = resolve_arg(value_store, plan_node, "command")?;
    // `args` is ONE PlanArg whose literal is a JSON-serialized `Vec<String>`
    // (locked decision 1) — decoded broker-side, AFTER I2 already cleared the
    // whole blob as one taint-tracked unit. A malformed JSON here is a
    // broker-internal invariant error (never attacker-controlled at this
    // point), so it fails closed via `?` rather than defaulting silently.
    let args_json =
        resolve_arg_optional(value_store, plan_node, "args")?.unwrap_or_else(|| "[]".to_string());
    let args: Vec<String> = serde_json::from_str(&args_json).with_context(|| {
        format!("process.exec: `args` literal `{args_json}` was not a valid JSON Vec<String>")
    })?;
    let cwd = resolve_arg_optional(value_store, plan_node, "cwd")?;

    // The launcher is a sibling binary of the running process image.
    let launcher_path = resolve_launcher_path()?;

    // Spawn + confine-handoff + capture + timeout, entirely lock-free — the
    // conn mutex is never held across this `.await`.
    let spawn_result = run_launcher(
        &launcher_path,
        &command,
        &args_json,
        cwd.as_deref(),
        workspace_root,
        &args,
    )
    .await;

    match spawn_result {
        Ok((_exit_status, combined_output)) => {
            // Success: append `process_exited`, tainted untrusted-origin
            // (mirrors `mint_from_read`'s file_read taint pairing) — this
            // event is the mint-root 32-05's `mint_from_exec` chains its
            // `provenance_chain[0]` onto (one event, both roles; DESIGN §2.1).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:process.exec:{effect_id}"),
                "process_exited".into(),
                Utc::now(),
                vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw],
            );
            let hash = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                append_event(&locked, key, &event, Some(parent_hash))
                    .context("append process_exited")?
            };
            Ok((event.id, hash, combined_output))
        }
        Err(e) => {
            // Two-phase durable audit: record an explicit failure outcome
            // FIRST, then propagate. NO automatic retry (mirrors
            // `invoke_file_create`'s `sink_execution_failed` arm).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:process.exec:{effect_id}"),
                "process_spawn_failed".into(),
                Utc::now(),
                vec![],
            );
            {
                let locked = conn
                    .lock()
                    .map_err(|e2| anyhow::anyhow!("mutex poisoned: {e2}"))?;
                append_event(&locked, key, &event, Some(parent_hash))
                    .context("append process_spawn_failed")?;
            }
            Err(e.context("process.exec invoke_process_exec failed"))
        }
    }
}

/// Re-invoke the live `process.exec` sink from a FROZEN `ResolvedArg` snapshot
/// (confirm-time re-invocation; EXEC-05, mirrors
/// `invoke_file_write_from_resolved`/`invoke_file_create_from_resolved`).
///
/// A `ValueStore`-free sibling of `invoke_process_exec`: this is called by a
/// later, separate `caprun confirm` process after a human has released
/// exactly one (sink, arg, literal-digest) triple. The literals are already
/// adjudicated and frozen at Block time
/// (`crate::confirmation::PendingConfirmation.resolved_args`) — this function
/// never constructs a `ValueStore`, never calls `store.resolve`, never calls
/// `store.mint`, and never calls `executor::submit_plan_node` (I2 is neither
/// re-run nor bypassable here). It re-applies the EXACT Allowed-path spawn
/// discipline of `invoke_process_exec` via the SAME conn-free `run_launcher`
/// helper — broker-spawned confined child (Landlock + seccomp + default-deny
/// net + rlimits), wall-clock timeout, byte cap on captured stdout/stderr.
///
/// This module still NEVER mints the captured output — the sole mint call
/// site for exec output stays in `confirmation.rs`'s Step-7 arm (D-10); this
/// function only spawns + audits and returns the raw `combined_output` for
/// the caller to mint.
///
/// # Arguments
/// * `conn`            — plain, unlocked rusqlite connection (broker-owned).
///   Confirm-time re-invocation is single-shot with no concurrent broker
///   tasks sharing this connection, unlike the Allowed-path's
///   `Arc<Mutex<rusqlite::Connection>>` (Pitfall 2).
/// * `key`             — the broker-owned audit-chain MAC key.
/// * `session_id`      — the Session the blocked plan node belonged to.
/// * `effect_id`       — the SAME `effect_id` as the original block's anchor.
/// * `resolved_args`   — the frozen `ResolvedArg` snapshot from
///   `PendingConfirmation` (`command` required, `args`/`cwd` optional).
/// * `workspace_root`  — the workspace root reopened at confirm time (same
///   root the broker opened at Block time).
/// * `parent_id`       — causal predecessor event id (the `confirm_granted`
///   head — NOT a fresh root; D-04).
/// * `parent_hash`     — hash of that predecessor row (chain anchor).
///
/// # Returns
/// `(event_id, hash, combined_output)` of the appended `process_exited` event
/// on success.
///
/// # Errors
/// On any spawn/exec/timeout/cap failure a `process_spawn_failed` event is
/// durably appended FIRST, chained onto `parent_id`/`parent_hash`, then the
/// original error is propagated (no retry — D-06).
#[allow(clippy::too_many_arguments)]
pub async fn invoke_process_exec_from_resolved(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String, String)> {
    // Look up the frozen literals directly — never re-resolve, never re-decide.
    let command = resolved_literal(resolved_args, "command")?;
    let args_json = resolved_literal_optional(resolved_args, "args").unwrap_or("[]");
    let args: Vec<String> = serde_json::from_str(args_json).with_context(|| {
        format!("process.exec: `args` literal `{args_json}` was not a valid JSON Vec<String>")
    })?;
    let cwd = resolved_literal_optional(resolved_args, "cwd");

    // The launcher is a sibling binary of the running process image.
    let launcher_path = resolve_launcher_path()?;

    // Spawn + confine-handoff + capture + timeout — same conn-free helper as
    // the Allowed path, reused unmodified.
    let spawn_result =
        run_launcher(&launcher_path, command, args_json, cwd, workspace_root, &args).await;

    match spawn_result {
        Ok((_exit_status, combined_output)) => {
            // Success: append `process_exited` chained onto the passed
            // parent (the real `confirm_granted` head, per D-04 — never a
            // fabricated root).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:process.exec:{effect_id}"),
                "process_exited".into(),
                Utc::now(),
                vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw],
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append process_exited")?;
            Ok((event.id, hash, combined_output))
        }
        Err(e) => {
            // Two-phase durable audit: record an explicit failure outcome
            // FIRST, chained onto the same parent, then propagate. NO
            // automatic retry (D-06 — exactly-once contract's failure leg).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:process.exec:{effect_id}"),
                "process_spawn_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append process_spawn_failed")?;
            Err(e.context("process.exec invoke_process_exec_from_resolved failed"))
        }
    }
}

/// Spawn `caprun-exec-launcher`, capture its combined stdout+stderr within
/// `EXEC_WALL_CLOCK_TIMEOUT` and `MAX_COMBINED_OUTPUT_BYTES`, and return the
/// exit status + combined output on success. `_args` is passed for
/// documentation/parity only — the launcher itself decodes `EXEC_ARGS_JSON`;
/// the broker never invokes the target directly.
async fn run_launcher(
    launcher_path: &std::path::Path,
    command: &str,
    args_json: &str,
    cwd: Option<&str>,
    workspace_root: &WorkspaceRoot,
    _args: &[String],
) -> Result<(std::process::ExitStatus, String)> {
    let mut cmd = TokioCommand::new(launcher_path);
    cmd.env("EXEC_COMMAND", command).env("EXEC_ARGS_JSON", args_json);
    // Only set EXEC_CWD when a cwd was actually supplied (32-06 fix): setting
    // it to an EMPTY STRING when `cwd` is `None` made the launcher call
    // `Command::current_dir("")`, which performs `chdir("")` — POSIX
    // `chdir("")` fails with ENOENT (empty path is not a valid path), so
    // EVERY process.exec invocation with no explicit `cwd` arg failed the
    // launcher's OWN subsequent `execve` with a misleading "No such file or
    // directory" error attributed to the TARGET command, not the empty cwd.
    // Reproduced and confirmed empirically in the 32-06 Linux container run
    // (cfg-linux-test-blindness: this path never compiled/ran on Mac). Leaving
    // the var UNSET when `cwd` is `None` makes the launcher's own
    // `std::env::var("EXEC_CWD").ok()` correctly resolve to `None`, matching
    // its doc comment's "EXEC_CWD (optional)" contract.
    if let Some(dir) = cwd {
        cmd.env("EXEC_CWD", dir);
    }
    cmd.env(
            "EXEC_WORKSPACE_ROOT",
            workspace_root.root_path().to_string_lossy().as_ref(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .context("process.exec: failed to spawn caprun-exec-launcher")?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("process.exec: launcher stdout was not piped"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("process.exec: launcher stderr was not piped"))?;

    // Concurrent capped reads of stdout+stderr, raced together with the
    // child's `wait()` — draining both pipes WHILE waiting avoids the classic
    // pipe-buffer deadlock (a child blocked writing to a full, undrained
    // pipe while the parent blocks on `wait()`). `total_read` is a SHARED
    // counter so the cap is enforced on the COMBINED byte total, not each
    // stream independently.
    let total_read = AtomicUsize::new(0);
    let run = async {
        tokio::join!(
            read_capped(stdout, MAX_COMBINED_OUTPUT_BYTES, &total_read),
            read_capped(stderr, MAX_COMBINED_OUTPUT_BYTES, &total_read),
            child.wait(),
        )
    };

    match tokio::time::timeout(EXEC_WALL_CLOCK_TIMEOUT, run).await {
        Ok((stdout_result, stderr_result, wait_result)) => {
            let exit_status = wait_result.context("process.exec: child wait() failed")?;
            let mut combined = stdout_result
                .context("process.exec: stdout capture failed (byte cap or read error)")?;
            let stderr_bytes = stderr_result
                .context("process.exec: stderr capture failed (byte cap or read error)")?;
            combined.extend_from_slice(&stderr_bytes);
            Ok((exit_status, String::from_utf8_lossy(&combined).into_owned()))
        }
        Err(_elapsed) => {
            // Wall-clock timeout: kill via the cancellable async `Child` — NOT
            // `wait_with_output()`, which consumes `self` and cannot be killed
            // once a timeout races it (Pitfall 5 / T-32-14 — a leaked
            // child/thread). `.kill_on_drop(true)` above is a second,
            // defense-in-depth backstop if this explicit kill is somehow
            // skipped (e.g. a future panic unwinding past this point).
            let _ = child.kill().await;
            let _ = child.wait().await; // reap, avoid a zombie
            Err(anyhow::anyhow!(
                "process.exec: wall-clock timeout ({EXEC_WALL_CLOCK_TIMEOUT:?}) exceeded, child killed"
            ))
        }
    }
}

/// Read a piped child stream to completion, enforcing `cap` as a COMBINED
/// (stdout+stderr) byte budget via the shared `total_read` counter.
///
/// Fail-closed (T-32-13): exceeding the cap immediately stops reading and
/// returns an error — this never silently truncates-and-continues past the
/// cap. Returning early drops `reader`, closing our end of the pipe; a child
/// that keeps writing past that point gets `EPIPE`/`SIGPIPE` rather than
/// blocking us indefinitely (the outer wall-clock timeout is the backstop if
/// the child ignores `SIGPIPE` and blocks on a subsequent write instead).
async fn read_capped<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    cap: usize,
    total_read: &AtomicUsize,
) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut chunk)
            .await
            .context("process.exec: reading piped output failed")?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        let running_total = total_read.fetch_add(n, Ordering::SeqCst) + n;
        if running_total > cap {
            anyhow::bail!(
                "process.exec: combined captured output exceeded the {cap}-byte cap (fail-closed)"
            );
        }
    }
    Ok(buf)
}

/// Resolve the `caprun-exec-launcher` sibling binary's path relative to the
/// CURRENTLY RUNNING process image.
///
/// # Why this is not a single fixed-depth `.parent()` hop (32-06 fix)
///
/// In the PRODUCTION path, `current_exe()` resolves to the real `caprun`
/// binary at `target/{debug,release}/caprun` — `caprun-exec-launcher` is a
/// direct sibling in that SAME directory (`main.rs` uses the identical
/// resolution for `caprun-worker`/`caprun-planner`).
///
/// But `invoke_process_exec` is ALSO exercised directly, in-process, by
/// integration tests (`crates/brokerd/tests/process_exec_spawn.rs`, and
/// `cli/caprun/tests/s9_process_exec_block.rs`) — and Cargo places a `cargo
/// test` integration-test BINARY one directory level deeper, at
/// `target/{debug,release}/deps/<test-name>-<hash>`, never at
/// `target/{debug,release}/` directly (empirically confirmed: `[[bin]]`
/// targets like `caprun-exec-launcher` are placed ONLY at the un-hashed
/// `target/{debug,release}/<name>` path, never mirrored into `deps/`). A
/// single fixed `.parent()` hop therefore resolves correctly in production
/// but NEVER finds the launcher when this function runs inside a test binary
/// (`current_exe().parent()` == `.../deps/`, which does not contain it) —
/// this was a genuine, reproducible bug, caught by 32-06's mandatory Linux
/// container run (a Mac build never compiles/runs this cfg(linux) path at
/// all — cfg-linux-test-blindness).
///
/// The fix: walk up a SMALL, bounded number of ancestor directories from
/// `current_exe()`'s parent, returning the first one that actually contains
/// `caprun-exec-launcher` — this covers BOTH the production layout (found at
/// depth 0) and the `cargo test` layout (found at depth 1, `deps/` ->
/// `debug/`), fails closed (a bounded, explicit search — never an unbounded
/// walk to the filesystem root) if neither is found.
fn resolve_launcher_path() -> Result<std::path::PathBuf> {
    let current_exe =
        std::env::current_exe().context("process.exec: could not resolve current_exe()")?;
    let mut dir = current_exe.parent().map(|p| p.to_path_buf());
    for _ in 0..3 {
        let Some(candidate_dir) = dir else { break };
        let candidate = candidate_dir.join("caprun-exec-launcher");
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = candidate_dir.parent().map(|p| p.to_path_buf());
    }
    Err(anyhow::anyhow!(
        "process.exec: could not locate sibling binary `caprun-exec-launcher` near \
         current_exe() {current_exe:?} (checked current_exe()'s parent and up to 2 \
         ancestor directories — covers both the production `caprun` binary layout and \
         a `cargo test` integration-test binary under target/{{debug,release}}/deps/; \
         run `cargo build --workspace` first if this is a fresh checkout — \
         cargo-test-workspace-missing-sibling-binary)"
    ))
}

/// Resolve a required named plan-node arg to its broker-owned literal.
fn resolve_arg(store: &ValueStore, plan_node: &PlanNode, name: &str) -> Result<String> {
    let arg = plan_node
        .args
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| anyhow::anyhow!("process.exec plan node missing `{name}` arg"))?;
    let record = store
        .resolve(&arg.value_id)
        .ok_or_else(|| anyhow::anyhow!("process.exec `{name}` handle did not resolve"))?;
    Ok(record.literal.clone())
}

/// Resolve an optional named plan-node arg. Returns `Ok(None)` if the arg is
/// simply absent (schema-allowed for `args`/`cwd`); still fails closed if the
/// arg IS present but its handle does not resolve (a broker-internal
/// invariant violation, same as `resolve_arg`).
fn resolve_arg_optional(
    store: &ValueStore,
    plan_node: &PlanNode,
    name: &str,
) -> Result<Option<String>> {
    let arg = match plan_node.args.iter().find(|a| a.name == name) {
        Some(a) => a,
        None => return Ok(None),
    };
    let record = store
        .resolve(&arg.value_id)
        .ok_or_else(|| anyhow::anyhow!("process.exec `{name}` handle did not resolve"))?;
    Ok(Some(record.literal.clone()))
}

/// Look up a required named literal directly from a frozen `ResolvedArg`
/// snapshot (mirrors `file_write.rs`'s private `resolved_literal` — not a
/// shared abstraction, each `_from_resolved` sibling keeps its own copy).
fn resolved_literal<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Result<&'a str> {
    resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
        .ok_or_else(|| anyhow::anyhow!("frozen resolved_args missing `{name}` arg"))
}

/// Look up an optional named literal directly from a frozen `ResolvedArg`
/// snapshot. Returns `None` if the arg is simply absent (schema-allowed for
/// `args`/`cwd`).
fn resolved_literal_optional<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Option<&'a str> {
    resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
}
