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
        // process.exec adds NO effect-specific env — the child env is
        // byte-identical to before run_launcher grew the `extra_env` param
        // (git.commit is the sole caller that passes a non-empty slice).
        &[],
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
/// This module still NEVER mints the captured output. On the confirm-release
/// path the output is NOT minted at all (34-03 adversarial-review MINOR fix):
/// `confirmation.rs`'s Step-7 arm discards `combined_output` — unlike the
/// Allowed path (server.rs), the human-driven `caprun confirm` process has no
/// live `ValueStore` and no downstream plan node to receive a minted `ValueId`.
/// This function only spawns + audits — the durable, non-stapled taint anchor
/// is the `process_exited` Event it appends ({ExternalUntrusted, ExecRaw},
/// chained on `confirm_granted`) — and returns `combined_output` to its caller
/// unmodified. The sole exec-output mint site remains the Allowed path in
/// `server.rs` (Gate 3 allow-list: server.rs + quarantine.rs only).
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
    // Prepare all fallible PRE-SPAWN inputs (frozen `command`/`args`/`cwd`
    // literals + launcher-path resolution) up front, folding their errors into
    // the SAME `Result` as the spawn itself. A pre-spawn failure (malformed
    // frozen `args`, missing launcher) is normally caught by confirm()'s
    // Step-4.8 precheck BEFORE the one-shot confirmation is burned
    // (fail-closed-RECOVERABLE) — and that precheck calls this SAME
    // `prepare_process_exec`, so precheck and dispatch can never drift. Folding
    // it in here is defense-in-depth for the residual TOCTOU window (launcher
    // removed between precheck and here): EVERY failure — pre-spawn OR spawn —
    // now flows through the single `Err` branch below that appends a durable
    // `process_spawn_failed` FIRST, so a burned confirmation can NEVER be left
    // with a dangling `confirm_granted` and no terminal event (34-03
    // adversarial-review MAJOR fix; the exact P33 MAJOR-1 audit-gap class,
    // previously re-entered via the `?` legs here that propagated before any
    // event was appended).
    let spawn_result = match prepare_process_exec(resolved_args) {
        // Spawn + confine-handoff + capture + timeout — same conn-free helper as
        // the Allowed path, reused unmodified.
        Ok(prepared) => {
            run_launcher(
                &prepared.launcher_path,
                &prepared.command,
                &prepared.args_json,
                prepared.cwd.as_deref(),
                workspace_root,
                &prepared.args,
                // confirm-release process.exec adds NO effect-specific env.
                &[],
            )
            .await
        }
        Err(e) => Err(e),
    };

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

/// The frozen, validated PRE-SPAWN inputs for a confirm-release `process.exec`
/// re-invocation. Owned (not borrowed from `resolved_args`) so a caller can
/// validate independently of the borrow — see [`prepare_process_exec`].
pub(crate) struct PreparedExec {
    command: String,
    args_json: String,
    args: Vec<String>,
    cwd: Option<String>,
    launcher_path: std::path::PathBuf,
}

/// The fallible PRE-SPAWN preparation shared by the confirm-release sink
/// ([`invoke_process_exec_from_resolved`], Step 7) and confirm()'s Step-4.8
/// precheck: look up the frozen `command`/`args`/`cwd` literals, parse the
/// `args` JSON, and resolve the launcher sibling binary (`.is_file()`-checked,
/// so a missing launcher fails HERE, not mid-spawn). Every step that can fail
/// BEFORE a subprocess is spawned lives here. Pure/read-only — no events, no DB:
/// the caller decides the failure disposition (precheck → fail-closed-
/// RECOVERABLE, row stays Pending; sink → append a terminal
/// `process_spawn_failed` first). One function = precheck and dispatch validate
/// IDENTICALLY, so they cannot drift (34-03 adversarial-review MAJOR fix).
pub(crate) fn prepare_process_exec(resolved_args: &[ResolvedArg]) -> Result<PreparedExec> {
    // Look up the frozen literals directly — never re-resolve, never re-decide.
    let command = resolved_literal(resolved_args, "command")?.to_string();
    let args_json = resolved_literal_optional(resolved_args, "args")
        .unwrap_or("[]")
        .to_string();
    let args: Vec<String> = serde_json::from_str(&args_json).with_context(|| {
        format!("process.exec: `args` literal `{args_json}` was not a valid JSON Vec<String>")
    })?;
    let cwd = resolved_literal_optional(resolved_args, "cwd").map(str::to_string);
    // The launcher is a sibling binary of the running process image.
    let launcher_path = resolve_launcher_path()?;
    Ok(PreparedExec { command, args_json, args, cwd, launcher_path })
}

/// Spawn `caprun-exec-launcher`, capture its combined stdout+stderr within
/// `EXEC_WALL_CLOCK_TIMEOUT` and `MAX_COMBINED_OUTPUT_BYTES`, and return the
/// exit status + combined output on success. `_args` is passed for
/// documentation/parity only — the launcher itself decodes `EXEC_ARGS_JSON`;
/// the broker never invokes the target directly.
/// Minimal `PATH` handed to the confined exec child after `env_clear()` — enough
/// to resolve bare-name commands (`/usr/bin`, `/bin`, `/usr/local/bin`) without
/// re-introducing any of the broker's own environment. See `run_launcher`.
pub(crate) const SAFE_EXEC_PATH: &str = "/usr/bin:/bin:/usr/local/bin";

/// Spawn the confined launcher and capture its combined output.
///
/// `pub(crate)` so the `git_commit` sink (Pattern B, GIT-01) reuses the EXACT
/// same confined-spawn discipline instead of duplicating it — the launcher,
/// `env_clear()`, `SAFE_EXEC_PATH`, wall-clock timeout, byte cap, and
/// `kill_on_drop` are all shared verbatim.
///
/// `extra_env` layers NON-SECRET, effect-specific vars onto the `env_clear()`ed
/// child AFTER the `EXEC_*` metadata (e.g. git.commit's
/// `GIT_CONFIG_NOSYSTEM`/`GIT_CONFIG_GLOBAL`/`GIT_TERMINAL_PROMPT`
/// neutralization). `process.exec` passes `&[]`, so its child env is
/// byte-identical to before this parameter existed — never a behavior change,
/// and no secret can be reintroduced via this path (the caller controls the
/// slice; the broker only ever passes constant, non-secret git-neutralization
/// vars here).
pub(crate) async fn run_launcher(
    launcher_path: &std::path::Path,
    command: &str,
    args_json: &str,
    cwd: Option<&str>,
    workspace_root: &WorkspaceRoot,
    _args: &[String],
    extra_env: &[(&str, &str)],
) -> Result<(std::process::ExitStatus, String)> {
    let mut cmd = TokioCommand::new(launcher_path);
    // SECURITY (todo 2026-07-17-exec-child-env-clear / Phase 34 gap-closure):
    // clear the broker's environment so the confined child inherits NONE of the
    // unconfined `caprun` process's env — notably `OPENAI_API_KEY` and
    // `CAPRUN_SMTP_*`. Without this, a UserTrusted `env`/`printenv` target would
    // capture those secrets into `combined_output`, which is minted/audited and
    // shown verbatim in the confirmation UX. Pass ONLY a minimal safe `PATH` (so
    // the launcher's `execve` of a bare-name target command still resolves
    // standard binaries) plus the `EXEC_*` vars the launcher itself reads — those
    // are non-secret command metadata (command/args/cwd/workspace root), already
    // known to any human confirmer.
    cmd.env_clear();
    cmd.env("PATH", SAFE_EXEC_PATH);
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
    // Effect-specific neutralization env, layered onto the env_clear()ed child
    // AFTER the EXEC_* metadata. Empty for process.exec (child env unchanged);
    // git.commit passes the non-secret GIT_CONFIG_NOSYSTEM/GLOBAL +
    // GIT_TERMINAL_PROMPT triple (GIT-01, DESIGN §1.5).
    for (k, v) in extra_env {
        cmd.env(k, v);
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
pub(crate) fn resolve_launcher_path() -> Result<std::path::PathBuf> {
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

// ── invoke_process_exec_from_resolved (EXEC-05 confirm-release, frozen-literal
// re-invocation) ─────────────────────────────────────────────────────────
//
// Linux-only (mirrors `crates/brokerd/tests/process_exec_spawn.rs`'s
// discipline): this function genuinely spawns the real
// `caprun-exec-launcher` binary, whose confinement primitives (rlimits ->
// Landlock -> seccomp) are macOS no-op stubs — a Mac run would compile these
// tests but prove nothing about the confined spawn path. `cargo test -p
// brokerd` on macOS shows 0 tests from this module — expected
// (cfg-linux-test-blindness), never treated as a pass. Run for real via
// `scripts/mailpit-verify.sh` (Colima/Linux container).
#[cfg(test)]
#[cfg(target_os = "linux")]
mod from_resolved_tests {
    use super::*;
    use crate::audit::{find_event_by_type, open_audit_db, verify_chain};
    use runtime_core::plan_node::ValueId;

    /// Fixed, non-secret test MAC key (mirrors `process_exec_spawn.rs`'s
    /// `TEST_KEY`).
    const TEST_KEY: &[u8] = b"process-exec-from-resolved-rs-unit-test-key";

    /// A frozen `ResolvedArg` snapshot for {command, args} — mirrors what a
    /// `PendingConfirmation.resolved_args` payload would carry (args already
    /// JSON-serialized, matching DESIGN's locked decision 1).
    fn resolved_args_for(command: &str, args: &[&str]) -> Vec<ResolvedArg> {
        let ev = Uuid::new_v4();
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let args_json = serde_json::to_string(&args_vec).unwrap();
        vec![
            ResolvedArg {
                name: "command".into(),
                value_id: ValueId::new(),
                literal: command.to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![ev],
            },
            ResolvedArg {
                name: "args".into(),
                value_id: ValueId::new(),
                literal: args_json,
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![ev],
            },
        ]
    }

    /// Seed a causal-root event in a fresh in-memory DB (stands in for the
    /// `confirm_granted` head this function chains onto per D-04). Returns
    /// (conn, session_id, root_id, root_hash).
    fn seed_root() -> (rusqlite::Connection, Uuid, Uuid, String) {
        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "confirm_granted".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();
        (conn, session_id, root.id, root_hash)
    }

    /// A temp workspace root — `EXEC_WORKSPACE_ROOT` is required by the
    /// launcher even though these tests don't exercise file I/O under it.
    fn temp_workspace(tag: &str) -> (std::path::PathBuf, WorkspaceRoot) {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_pe_resolved_{tag}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();
        (root, ws)
    }

    /// On success, invoke_process_exec_from_resolved spawns the confined
    /// launcher from frozen literals (never a ValueStore), returns the
    /// captured combined output, and records a `process_exited` event
    /// chained onto the passed `parent_id`/`parent_hash` (the
    /// `confirm_granted` head, per D-04 — never a fresh root).
    #[tokio::test]
    async fn invoke_process_exec_from_resolved_success_appends_process_exited_chained_on_parent()
    {
        let (root, ws) = temp_workspace("ok");
        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let resolved_args = resolved_args_for("/bin/echo", &["hello-from-confirm"]);

        let (evt_id, hash, combined_output) = invoke_process_exec_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        )
        .await
        .expect("invoke_process_exec_from_resolved must succeed via the confined launcher");

        assert!(!hash.is_empty());
        assert!(
            combined_output.contains("hello-from-confirm"),
            "combined_output was: {combined_output:?}"
        );

        let evt = find_event_by_type(&conn, &session_id.to_string(), "process_exited")
            .unwrap()
            .expect("process_exited event must exist");
        assert_eq!(evt.id, evt_id);
        assert_eq!(evt.actor, format!("sink:process.exec:{effect_id}"));
        assert_eq!(
            evt.parent_id,
            Some(parent_id),
            "process_exited must chain onto the passed confirm_granted parent, not a fresh root"
        );
        assert_eq!(
            evt.taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw],
            "process_exited must carry the untrusted-origin exec taint pair"
        );
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "process_spawn_failed")
                .unwrap()
                .is_none(),
            "no process_spawn_failed on the success path"
        );
        assert!(
            verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "the causal chain must stay intact after appending process_exited"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// An output-flooding command (`/bin/yes`) trips the combined 10 MiB
    /// byte cap fail-closed inside `run_launcher` — the same genuine
    /// spawn-path failure `process_exec_spawn.rs`'s Allowed-path twin uses
    /// (a nonexistent target command does NOT surface as an `Err` here: the
    /// launcher's own `execve` failure is a normal nonzero-exit-code process
    /// exit captured as a successful `process_exited`, not a spawn failure —
    /// this test exercises a genuine `run_launcher` `Err` instead). Asserts
    /// a durably-appended `process_spawn_failed` event chained on the passed
    /// parent, and an `Err` return (no double-spawn, no retry — D-06).
    #[tokio::test]
    async fn invoke_process_exec_from_resolved_spawn_failure_appends_process_spawn_failed() {
        let (root, ws) = temp_workspace("cap");
        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let resolved_args = resolved_args_for("/bin/yes", &[]);

        let result = invoke_process_exec_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        )
        .await;

        assert!(
            result.is_err(),
            "/bin/yes's unbounded output must trip the byte cap and fail closed"
        );

        let evt = find_event_by_type(&conn, &session_id.to_string(), "process_spawn_failed")
            .unwrap()
            .expect("process_spawn_failed event must exist");
        assert_eq!(evt.actor, format!("sink:process.exec:{effect_id}"));
        assert_eq!(
            evt.parent_id,
            Some(parent_id),
            "process_spawn_failed must chain onto the passed confirm_granted parent"
        );
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "process_exited")
                .unwrap()
                .is_none(),
            "no process_exited on the byte-cap failure path"
        );
        assert!(
            verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "the causal chain must stay intact after appending process_spawn_failed"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// Phase 34 gap-closure (todo 2026-07-17-exec-child-env-clear): the confined
    /// exec child must NOT inherit the broker's environment. A secret set in the
    /// broker (this test process) — standing in for `OPENAI_API_KEY` /
    /// `CAPRUN_SMTP_*` — must be ABSENT from a UserTrusted `/usr/bin/env` target's
    /// captured output (which is minted, audited, and shown verbatim on confirm).
    /// `run_launcher`'s `env_clear()` + minimal `PATH` guarantees this. A sanity
    /// assertion confirms the clear did not leave an empty env (PATH survives).
    #[tokio::test]
    async fn run_launcher_env_clear_prevents_broker_secret_leak() {
        let (root, ws) = temp_workspace("envclear");
        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();

        // A unique sentinel "secret" in the broker's (this process's)
        // environment — mirrors OPENAI_API_KEY / CAPRUN_SMTP_* being present in
        // the unconfined caprun process. Unique name so it cannot collide with
        // any real var another concurrent test reads.
        let sentinel_key = "CAPRUN_ENV_LEAK_SENTINEL_34GAP";
        let sentinel_val = "s3cr3t-broker-value-must-not-leak-9x7q";
        std::env::set_var(sentinel_key, sentinel_val);

        // `/usr/bin/env` prints the child's full environment.
        let resolved_args = resolved_args_for("/usr/bin/env", &[]);
        let result = invoke_process_exec_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved_args,
            &ws,
            parent_id,
            &parent_hash,
        )
        .await;
        std::env::remove_var(sentinel_key);

        let (_evt_id, _hash, combined_output) =
            result.expect("/usr/bin/env must run in the confined child");

        assert!(
            !combined_output.contains(sentinel_key),
            "the confined child inherited the broker var NAME `{sentinel_key}` — env_clear() failed:\n{combined_output}"
        );
        assert!(
            !combined_output.contains(sentinel_val),
            "the confined child inherited the broker SECRET VALUE — env_clear() failed:\n{combined_output}"
        );
        assert!(
            !combined_output.contains("OPENAI_API_KEY"),
            "the confined child inherited OPENAI_API_KEY from the broker env — the exact leak this fix closes:\n{combined_output}"
        );
        // Sanity: env_clear() did not strip the minimal PATH the child needs.
        assert!(
            combined_output.contains("PATH="),
            "expected the minimal SAFE_EXEC_PATH in the child env, got:\n{combined_output}"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
