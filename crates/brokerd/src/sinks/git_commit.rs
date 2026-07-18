/// sinks/git_commit — mediated `git.commit` sink (broker-spawned confined child).
///
/// Pattern B (DESIGN-git-github-http-sinks.md §1.1/§1.4/§1.5, CONTEXT decisions
/// 1, 4, 5, GIT-01): `git.commit` is realized as an EXEC under the shipped v1.7
/// `caprun-exec-launcher` — this module builds a broker-constructed, TRUSTED
/// `git commit` argv + a neutralized git environment, then reuses
/// `process_exec::run_launcher` verbatim (same confinement stack: rlimits ->
/// Landlock exec-child ruleset -> seccomp exec-child filter -> `execve`, same
/// `env_clear()`, wall-clock timeout, combined-output byte cap, `kill_on_drop`).
/// It never spawns `git` directly and never mints the captured output.
///
/// # Config/hook neutralization — the P2 RCE mitigation (DESIGN §1.5, T-36-04)
///
/// A `git.commit` runs inside an attacker-writable workspace repo whose
/// `.git/config` and `.git/hooks/*` are untrusted input. Three layers make any
/// planted alias or hook inert:
///   * `-c core.hooksPath=/dev/null` on the argv — a COMMAND-LINE `-c` is the
///     HIGHEST git config precedence, so it overrides even a repo-local
///     `core.hooksPath`; a planted `.git/hooks/pre-commit` therefore never fires.
///   * `GIT_CONFIG_NOSYSTEM=1` + `GIT_CONFIG_GLOBAL=/dev/null` — strips the
///     system + global config so no ambient alias/hook config is read.
///   * The broker constructs the EXACT `commit` argv itself (never an
///     arg-derived subcommand), so no repo alias is ever invoked.
/// Because `GIT_CONFIG_GLOBAL=/dev/null` + `GIT_CONFIG_NOSYSTEM=1` also strip
/// any ambient committer identity, the broker supplies a TRUSTED
/// `-c user.name`/`-c user.email` on the argv (never from args) so the commit
/// still has an author. The child is `env_clear()`ed by `run_launcher`, so it
/// inherits no `OPENAI_API_KEY` / `CAPRUN_SMTP_*` (T-36-05).
///
/// # Two-phase durable audit (identical to `process_exec`)
///
/// The authorizing `plan_node_evaluated` event is persisted by the caller
/// BEFORE this function runs. This function performs the effect and records its
/// outcome durably:
///   * success -> a `process_exited` event tainted `[ExternalUntrusted,
///     ExecRaw]` (the mint-root `mint_from_exec` chains `provenance_chain[0]`
///     onto — one event, both roles, no stapling; DESIGN §1.4).
///   * failure -> a `process_spawn_failed` event (untainted) FIRST, then the
///     original error is propagated. NO automatic retry.
/// The `effect_id` is carried in the `actor` field as `sink:git.commit:<id>`.
///
/// # Gate 3 — this module NEVER mints
///
/// `invoke_git_commit` returns `(process_exited event_id, hash, combined_output)`
/// on success — it does NOT mint. Because git IS an exec under Pattern B, its
/// output correctly reuses the EXISTING `mint_from_exec` (no new mint site, no
/// new `TaintLabel`), and that mint call is pinned to `server.rs` by
/// `check-invariants.sh` Gate 3's sanctioned-loci allow-list (server.rs +
/// quarantine.rs only). A mint call here would fail Gate 3.
///
/// # git.commit is NOT on the confirm-release path (Phase 36 scoped decision)
///
/// Unlike `process.exec`, there is deliberately NO `invoke_git_commit_from_resolved`
/// and no Step-4.75 confirm-release arm. `git.commit` is a LOCAL, reversible
/// (`MutateReversible`) op; DESIGN §9 scopes the P33/P34 confirm-release
/// audit-gap discipline to `git.push`/`github.pr` (CommitIrreversible) only. A
/// tainted `message` therefore Blocks deterministically and cannot be released
/// this phase — closed, with no confirm-release audit-gap surface.
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode, TaintLabel};
use uuid::Uuid;

use adapter_fs::workspace::WorkspaceRoot;

use crate::audit::append_event;
use crate::sinks::process_exec::{resolve_launcher_path, run_launcher};

/// Invoke the live `git.commit` sink for an `Allowed` plan node.
///
/// Resolves the `message` arg from the broker-owned `ValueStore`, builds the
/// broker-constructed neutralized `git commit` argv + env, spawns
/// `caprun-exec-launcher` (reusing `process_exec::run_launcher`), and records
/// the two-phase durable audit event. Returns `(process_exited event_id, hash,
/// combined_output)` on success — the mint of `combined_output` via the
/// existing `mint_from_exec` is `server.rs`'s job (DESIGN §1.4 / Gate 3), NEVER
/// here.
///
/// # Arguments
/// Mirror `invoke_process_exec` exactly (`conn` is the shared mutex-guarded
/// audit connection; the lock is held ONLY for the final synchronous
/// `append_event`, never across the `.await`ed spawn).
///
/// # Errors
/// On any spawn/exec/timeout/cap failure a `process_spawn_failed` event is
/// durably appended FIRST, then the original error is propagated (no retry).
#[allow(clippy::too_many_arguments)]
pub async fn invoke_git_commit(
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
    // Resolve the `message` arg from the broker-owned store. validate_schema
    // (Step 0 of submit_plan_node) already guaranteed `message` is present and
    // known (git.commit's exact-match schema requires it) — a missing/dangling
    // handle here is a broker-internal invariant violation, so it fails closed.
    let message = resolve_arg(value_store, plan_node, "message")?;

    // Broker-constructed, TRUSTED argv. The global `-c` flags MUST precede the
    // `commit` subcommand (git requires config overrides before the command).
    //   -c core.hooksPath=/dev/null : highest-precedence hook disable (overrides
    //       even a repo-local core.hooksPath) — planted repo hooks cannot fire.
    //   -c user.name/user.email     : trusted committer identity, required
    //       because GIT_CONFIG_GLOBAL=/dev/null + NOSYSTEM strip any ambient one.
    // The `message` is the ONLY attacker-influenced value, and it flows solely
    // as the `-m` argument (never as a flag or subcommand), so it can inject no
    // git behavior.
    let argv: Vec<String> = vec![
        "-c".into(),
        "core.hooksPath=/dev/null".into(),
        "-c".into(),
        "user.name=caprun".into(),
        "-c".into(),
        "user.email=caprun@localhost".into(),
        "commit".into(),
        "-m".into(),
        message,
    ];
    // ONE PlanArg-style JSON blob, matching run_launcher's EXEC_ARGS_JSON
    // contract (the launcher decodes it and execve's `git <argv...>`).
    let args_json = serde_json::to_string(&argv)
        .context("git.commit: failed to JSON-serialize the constructed argv")?;

    // Run git inside the workspace repo (already-staged changes are committed).
    let cwd = workspace_root.root_path().to_string_lossy().into_owned();

    // Env-only neutralization (no `-c` equivalent). NON-SECRET constants — the
    // child stays env_clear()ed otherwise (inherits no broker secrets).
    let extra_env: [(&str, &str); 3] = [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_TERMINAL_PROMPT", "0"),
    ];

    // `git` as a BARE name — the launcher's execve resolves it via
    // SAFE_EXEC_PATH (matching process.exec's bare-name support). The seccomp
    // net-deny exec-child filter is reused verbatim (a local commit needs no
    // network) — sandbox is untouched.
    let launcher_path = resolve_launcher_path()?;

    let spawn_result = run_launcher(
        &launcher_path,
        "git",
        &args_json,
        Some(cwd.as_str()),
        workspace_root,
        &argv,
        &extra_env,
    )
    .await;

    // Outcome triage. A non-zero `git` exit is a SINK FAILURE — a commit was
    // NOT made — and MUST NOT be reported as success. This diverges
    // DELIBERATELY from `process.exec`, where a non-zero exit is a normal,
    // successful `process_exited` (the command ran; its output is the effect).
    // For `git.commit` the effect IS the commit: if `git` exits non-zero the
    // effect did not occur, so we treat it like any other spawn failure. The
    // previous code bound `_exit_status` and appended `process_exited`
    // unconditionally, falsely claiming success (and minting exec taint) on a
    // failed commit — the Phase 36 exit-code bug this fixes.
    //
    // Both failure shapes (spawn `Err`, or `Ok` with a non-zero exit) fold into
    // ONE failure path that appends a terminal `process_spawn_failed` event
    // FIRST, then propagates the error — never a terminal STATE ahead of the
    // terminal EVENT that justifies it (P33/P34 audit-gap discipline). NO
    // automatic retry.
    let combined_output = match spawn_result {
        Ok((exit_status, output)) if exit_status.success() => output,
        outcome => {
            let err = match outcome {
                Ok((exit_status, output)) => anyhow::anyhow!(
                    "git.commit: git exited with non-zero status {exit_status}; no commit was \
                     made. git output:\n{output}"
                ),
                Err(e) => e.context("git.commit invoke_git_commit failed"),
            };
            // Terminal EVENT before any terminal disposition (P33/P34).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:git.commit:{effect_id}"),
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
            return Err(err);
        }
    };

    // Success (git exited 0 — the commit was made): append `process_exited`,
    // tainted untrusted-origin — this event id is the mint-root server.rs's
    // `mint_from_exec` chains its `provenance_chain[0]` onto (one event, both
    // roles; DESIGN §1.4).
    let event = Event::new(
        Uuid::new_v4(),
        Some(parent_id),
        session_id,
        format!("sink:git.commit:{effect_id}"),
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

/// Resolve a required named plan-node arg to its broker-owned literal
/// (mirrors `process_exec.rs`'s private `resolve_arg` — each sink keeps its own
/// copy rather than sharing an abstraction).
fn resolve_arg(store: &ValueStore, plan_node: &PlanNode, name: &str) -> Result<String> {
    let arg = plan_node
        .args
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| anyhow::anyhow!("git.commit plan node missing `{name}` arg"))?;
    let record = store
        .resolve(&arg.value_id)
        .ok_or_else(|| anyhow::anyhow!("git.commit `{name}` handle did not resolve"))?;
    Ok(record.literal.clone())
}
