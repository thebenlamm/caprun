//! sinks/github_pr — the broker-resident `github.pr` create-PR sink (GITHUB-01,
//! DESIGN-git-github-http-sinks.md §4.1/§4.2/§4.6).
//!
//! # Security role (Pattern A, broker-resident — NEVER the confined worker)
//!
//! This module performs ONE authenticated REST `POST
//! /repos/{owner}/{repo}/pulls` via the reused, SSRF-pinned `http_request` POST
//! egress (`invoke_pinned_post`). It is broker-resident and INVOKED only from
//! broker-resident code, exactly like `email_smtp.rs`'s SMTP call — the confined
//! worker cannot reach it (kernel default-deny net + broker-only call sites).
//!
//! # Credential custody (D-04, T-38-09)
//!
//! The bearer token is read from `CAPRUN_GITHUB_TOKEN` in the broker's LOCAL
//! process env ONLY (`github_token()`, mirroring `email_smtp::smtp_host`). It is
//! NEVER a `ValueNode`, a plan-node arg, an audit-DAG literal, the confined
//! worker, or the planner sidecar. An absent token fails closed (Err) before any
//! socket.
//!
//! # Destination pinning (MAJOR-4, T-38-08)
//!
//! The API base is FIXED broker-owned trusted config (`api_base()` →
//! `https://api.github.com`), overridable via `CAPRUN_GITHUB_API_BASE` ONLY for
//! the Phase-40 mock harness (mirroring `CAPRUN_SMTP_HOST`) — NEVER derived from
//! owner/repo or any tainted arg. Even when overridden it still rides the SAME
//! §3.6 `validate_url` → allowlist → resolve-and-pin path (`invoke_pinned_post`),
//! so `owner`/`repo` bind only the URL PATH and can never redirect the POST host.
//! `owner`/`repo` are additionally percent-encoded as path segments
//! (defense-in-depth atop the I2 routing-Block, T-38-10).
//!
//! # Opaque audit (T-38-07)
//!
//! `github_pr_succeeded` / `github_pr_failed` events carry NO token and NO raw
//! API response text in their hashed payload — only `effect_id` (in the `actor`
//! field, `sink:github.pr:<effect_id>`) and a static `event_type` marker. Raw
//! status/response text is routed to this codebase's `eprintln!` logging
//! convention only (mirror `email_smtp`'s `record_send_failed`).
//!
//! # NO mint here (Gate 3 unchanged)
//!
//! `github.pr` CONSUMES; a created-PR response is never re-fetched as a GET, so
//! this module performs NO `ValueStore::mint` / `mint_from_*` and introduces no
//! new mint site — `check-invariants.sh` Gate 3's mint-site allow-list stays
//! byte-identical.
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode};
use uuid::Uuid;

use crate::audit::append_event;
use crate::confirmation::ResolvedArg;
use crate::sinks::http_request;

/// The FIXED broker-owned API base (DESIGN §4.1). `api_base()` returns this
/// unless `CAPRUN_GITHUB_API_BASE` overrides it for the Phase-40 mock harness.
const DEFAULT_API_BASE: &str = "https://api.github.com";

/// The exact six-arg set for `github.pr` (executor sink schema, Plan 38-01).
const PR_ARGS: [&str; 6] = ["owner", "repo", "base", "head", "title", "body"];

/// Read the GitHub bearer token from the broker's LOCAL process env ONLY (D-04,
/// mirror `email_smtp::smtp_host`). Fails closed (Err) if unset. NEVER read from
/// a `ValueNode`, a plan-node arg, the audit DB, or `PendingConfirmation`.
fn github_token() -> Result<String> {
    std::env::var("CAPRUN_GITHUB_TOKEN").map_err(|_| {
        anyhow::anyhow!(
            "github.pr: CAPRUN_GITHUB_TOKEN is not set in the broker-local env (fail-closed)"
        )
    })
}

/// The API base URL: FIXED broker-owned trusted config (`https://api.github.com`),
/// overridable via `CAPRUN_GITHUB_API_BASE` ONLY for the Phase-40 mock harness
/// (like `CAPRUN_SMTP_HOST`). NEVER derived from a resolved/tainted arg. Even an
/// override still rides `invoke_pinned_post`'s validate_url + allowlist + pin.
fn api_base() -> String {
    std::env::var("CAPRUN_GITHUB_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.to_string())
}

/// The frozen, validated PRE-POST inputs for a `github.pr` dispatch (mirrors
/// `process_exec::PreparedExec`). Owned so a caller can validate independently.
pub(crate) struct PreparedPr {
    /// `{api_base}/repos/{owner}/{repo}/pulls` (owner/repo percent-encoded).
    url: String,
    /// The serialized `{title,body,head,base}` JSON request body.
    body: String,
}

/// Look up a required named literal from a frozen `ResolvedArg` snapshot,
/// fail-closed if missing OR empty (the "present + non-empty" precheck).
fn required_literal<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Result<&'a str> {
    let literal = resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
        .ok_or_else(|| anyhow::anyhow!("github.pr: missing required `{name}` arg (fail-closed)"))?;
    if literal.trim().is_empty() {
        bail!("github.pr: required `{name}` arg is empty (fail-closed)");
    }
    Ok(literal)
}

/// Build `{api_base}/repos/{owner}/{repo}/pulls` with `owner`/`repo`
/// PERCENT-ENCODED as path segments (defense-in-depth, T-38-10): a `/` inside a
/// segment is encoded to `%2F`, so a tainted-but-influenced owner/repo can never
/// inject extra path or redirect the host. The host is whatever `api_base()`
/// resolves to — FIXED, never derived from owner/repo (MAJOR-4).
fn build_pr_url(owner: &str, repo: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(&api_base())
        .context("github.pr: api_base is not a valid URL")?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("github.pr: api_base cannot be a base URL"))?
        .pop_if_empty()
        .extend(["repos", owner, repo, "pulls"]);
    Ok(url.to_string())
}

/// Build the `{title,body,head,base}` create-PR JSON body via serde_json (the
/// reqwest `json` feature is deliberately NOT enabled — serialize at the caller).
fn build_pr_body(title: &str, body: &str, head: &str, base: &str) -> Result<String> {
    let json = serde_json::json!({
        "title": title,
        "body": body,
        "head": head,
        "base": base,
    });
    serde_json::to_string(&json).context("github.pr: failed to serialize create-PR JSON body")
}

/// The fallible PRE-POST preparation shared by BOTH invoke paths AND the 38-05
/// confirm-release precheck: validate all six args are present + non-empty and
/// that the URL/body are constructible. Opens NO socket and appends NO event —
/// pure/read-only, so precheck (fail-closed-RECOVERABLE) and dispatch validate
/// IDENTICALLY and cannot drift (the P33/P34 audit-gap discipline;
/// mirror `process_exec::prepare_process_exec`).
pub(crate) fn prepare_github_pr(resolved_args: &[ResolvedArg]) -> Result<PreparedPr> {
    let owner = required_literal(resolved_args, "owner")?;
    let repo = required_literal(resolved_args, "repo")?;
    let base = required_literal(resolved_args, "base")?;
    let head = required_literal(resolved_args, "head")?;
    let title = required_literal(resolved_args, "title")?;
    let body = required_literal(resolved_args, "body")?;

    let url = build_pr_url(owner, repo)?;
    let json_body = build_pr_body(title, body, head, base)?;
    Ok(PreparedPr { url, body: json_body })
}

/// The network leg: read the broker-env token (fail-closed if unset) and POST via
/// the SSRF-pinned egress. Conn-free so no caller holds an audit lock across the
/// `.await` (mirror `email_smtp`/`git_commit`'s lock discipline).
async fn post_pr(prepared: &PreparedPr) -> Result<(u16, String)> {
    let token = github_token()?;
    http_request::invoke_pinned_post(&prepared.url, &token, &prepared.body).await
}

/// Fold a POST outcome into the OPAQUE two-phase audit, shared by both invoke
/// paths. On `Ok(2xx)`: append `github_pr_succeeded`. On `Ok(non-2xx)` OR `Err`:
/// route the raw status/response text to `eprintln!` (the ONLY place it may
/// appear), append an OPAQUE `github_pr_failed` event FIRST (terminal EVENT
/// before any terminal disposition — P33/P34), then propagate a non-swallowed
/// `Err`. NO retry. Payloads carry NO token and NO response text (T-38-07).
fn append_pr_outcome(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    parent_id: Uuid,
    parent_hash: &str,
    post_result: Result<(u16, String)>,
) -> Result<(Uuid, String)> {
    match post_result {
        Ok((status, _body)) if (200..300).contains(&status) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:github.pr:{effect_id}"),
                "github_pr_succeeded".into(),
                Utc::now(),
                vec![],
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append github_pr_succeeded")?;
            Ok((event.id, hash))
        }
        outcome => {
            // Ok(non-2xx) or a transport/build Err — both are audited-abort paths.
            // Raw status/response text goes ONLY to eprintln, never the payload.
            let err = match outcome {
                Ok((status, body)) => {
                    eprintln!(
                        "[brokerd] github.pr failed (effect_id={effect_id}): HTTP {status}: {body}"
                    );
                    anyhow::anyhow!("github.pr: GitHub API returned non-success status {status}")
                }
                Err(e) => {
                    eprintln!("[brokerd] github.pr failed (effect_id={effect_id}): {e}");
                    e.context("github.pr POST failed")
                }
            };
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:github.pr:{effect_id}"),
                "github_pr_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append github_pr_failed")?;
            Err(err)
        }
    }
}

/// Resolve the six `github.pr` args from the broker-owned `ValueStore` into a
/// frozen `ResolvedArg` snapshot. `validate_schema` (Step 0 of `submit_plan_node`)
/// already guaranteed each is present + known; a missing/dangling handle here is
/// a broker-internal invariant violation → fail closed (mirror `git_commit`'s
/// `resolve_arg`). Reusing this snapshot with `prepare_github_pr` means the
/// Allowed path, the confirm-release dispatch, and the precheck all validate
/// url/body IDENTICALLY.
fn resolve_all_args(store: &ValueStore, plan_node: &PlanNode) -> Result<Vec<ResolvedArg>> {
    let mut out = Vec::with_capacity(PR_ARGS.len());
    for name in PR_ARGS {
        let arg = plan_node
            .args
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| anyhow::anyhow!("github.pr plan node missing `{name}` arg"))?;
        let record = store
            .resolve(&arg.value_id)
            .ok_or_else(|| anyhow::anyhow!("github.pr `{name}` handle did not resolve"))?;
        out.push(ResolvedArg {
            name: name.to_string(),
            value_id: arg.value_id.clone(),
            literal: record.literal.clone(),
            taint: record.taint.clone(),
            provenance_chain: record.provenance_chain.clone(),
        });
    }
    Ok(out)
}

/// Invoke the live `github.pr` create-PR sink for an `Allowed` plan node.
///
/// Resolves the six args from the broker-owned `ValueStore`, builds the pinned
/// POST URL + JSON body, reads the broker-env bearer token, POSTs via the reused
/// SSRF-pinned egress, and records the OPAQUE two-phase audit. Returns
/// `(github_pr_succeeded event_id, hash)` on a 2xx. Does NOT mint (Gate 3).
///
/// A pre-POST build failure is FOLDED into the post result so it ALSO appends an
/// opaque `github_pr_failed` FIRST (never a bare `?` with no terminal event).
/// `conn` is the shared mutex-guarded audit connection; the lock is held ONLY
/// for the final synchronous append, never across the `.await`ed POST.
///
/// Wired by Plan 38-04 (server.rs Allowed-decision dispatch).
#[allow(clippy::too_many_arguments)]
pub async fn invoke_github_pr(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    key: &[u8],
    value_store: &ValueStore,
    session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let resolved = resolve_all_args(value_store, plan_node)?;
    let post_result = match prepare_github_pr(&resolved) {
        Ok(prepared) => post_pr(&prepared).await,
        Err(e) => Err(e),
    };
    let locked = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
    append_pr_outcome(
        &locked,
        key,
        session_id,
        effect_id,
        parent_id,
        parent_hash,
        post_result,
    )
}

/// Invoke the live `github.pr` sink from a FROZEN `ResolvedArg` snapshot
/// (confirm-release path, mirror `invoke_process_exec_from_resolved`). Prepares
/// (validates url/body via the SAME `prepare_github_pr` the 38-05 precheck uses)
/// and POSTs, folding ANY pre-POST failure into the SAME post result so EVERY
/// failure — pre-POST OR transport — appends an opaque `github_pr_failed` FIRST
/// then propagates (never a burned confirmation with no terminal event; the
/// P33/P34 MAJOR-1 audit-gap class). Wired by Plan 38-05 (confirmation.rs).
#[allow(dead_code)] // wired by Plan 38-05 (confirm-release dispatch)
#[allow(clippy::too_many_arguments)]
pub async fn invoke_github_pr_from_resolved(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let post_result = match prepare_github_pr(resolved_args) {
        Ok(prepared) => post_pr(&prepared).await,
        Err(e) => Err(e),
    };
    append_pr_outcome(
        conn,
        key,
        session_id,
        effect_id,
        parent_id,
        parent_hash,
        post_result,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{append_event, find_event_by_type, open_audit_db};
    use runtime_core::plan_node::{TaintLabel, ValueId};

    /// Fixed, non-secret test MAC key (mirrors `email_smtp.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"github-pr-rs-unit-test-key-not-secret";

    /// Serializes tests in THIS module that mutate the process-global
    /// `CAPRUN_GITHUB_*` env vars — the multi-threaded test runner would
    /// otherwise let two race on the same process-wide environment.
    static GITHUB_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn arg(name: &str, literal: &str) -> ResolvedArg {
        ResolvedArg {
            name: name.to_string(),
            value_id: ValueId::new(),
            literal: literal.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        }
    }

    fn well_formed_args() -> Vec<ResolvedArg> {
        vec![
            arg("owner", "octocat"),
            arg("repo", "hello-world"),
            arg("base", "main"),
            arg("head", "feature"),
            arg("title", "My PR"),
            arg("body", "PR description"),
        ]
    }

    // ── api_base / token: env-only sourcing (D-04) ──

    #[test]
    fn api_base_defaults_to_fixed_github_when_env_unset() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");
        assert_eq!(api_base(), "https://api.github.com");
    }

    #[test]
    fn github_token_errs_when_unset() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        std::env::remove_var("CAPRUN_GITHUB_TOKEN");
        assert!(
            github_token().is_err(),
            "an absent CAPRUN_GITHUB_TOKEN must fail closed"
        );
    }

    // ── build_pr_url: fixed host + percent-encoded path segments (MAJOR-4/T-38-10) ──

    #[test]
    fn build_pr_url_constructs_repos_pulls_path() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");
        assert_eq!(
            build_pr_url("octocat", "hello-world").unwrap(),
            "https://api.github.com/repos/octocat/hello-world/pulls"
        );
    }

    #[test]
    fn build_pr_url_host_is_fixed_not_derived_from_owner_repo() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");
        // An owner/repo containing path-traversal / host-injection bytes must NOT
        // move the host off api.github.com — a `/` inside a segment is encoded.
        let url = build_pr_url("evil.com/..", "repo/../../etc").unwrap();
        let parsed = reqwest::Url::parse(&url).unwrap();
        assert_eq!(
            parsed.host_str(),
            Some("api.github.com"),
            "owner/repo can never redirect the POST host (MAJOR-4)"
        );
        assert!(
            !url.contains("evil.com/.."),
            "the raw `/` in owner must be percent-encoded, never left as a path break"
        );
    }

    #[tokio::test]
    async fn overridden_api_base_still_rides_validate_url_and_allowlist() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        // Even a mock-harness override rides the SAME §3.6 pin: a non-allowlisted
        // base host is rejected by invoke_pinned_post's allowlist gate (MAJOR-4).
        std::env::set_var("CAPRUN_GITHUB_API_BASE", "https://evil.invalid");
        let url = build_pr_url("octocat", "hello-world").unwrap();
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");
        let r = http_request::invoke_pinned_post(&url, "tok", "{}").await;
        assert!(
            r.is_err(),
            "an overridden non-allowlisted base must still be rejected by the pin path"
        );
    }

    // ── prepare_github_pr: present + non-empty precheck, socket-free ──

    #[test]
    fn prepare_github_pr_ok_for_well_formed_six_args() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");
        assert!(prepare_github_pr(&well_formed_args()).is_ok());
    }

    #[test]
    fn prepare_github_pr_errs_on_missing_required_arg() {
        let mut args = well_formed_args();
        args.retain(|a| a.name != "title");
        assert!(
            prepare_github_pr(&args).is_err(),
            "a missing required arg must fail closed"
        );
    }

    #[test]
    fn prepare_github_pr_errs_on_empty_arg() {
        let mut args = well_formed_args();
        for a in args.iter_mut() {
            if a.name == "head" {
                a.literal = "   ".to_string();
            }
        }
        assert!(
            prepare_github_pr(&args).is_err(),
            "an empty/whitespace required arg must fail closed"
        );
    }

    // ── opaque audit: the token literal NEVER enters the hashed payload (T-38-07) ──

    #[tokio::test]
    async fn opaque_audit_token_literal_absent_from_appended_event() {
        let _guard = GITHUB_ENV_LOCK.lock().unwrap();
        const TOKEN: &str = "ghp_SUPERSECRETTOKENVALUE1234567890";
        std::env::set_var("CAPRUN_GITHUB_TOKEN", TOKEN);
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");

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

        let effect_id = Uuid::new_v4();
        // On macOS the live POST stubs out (Err) -> github_pr_failed is appended,
        // then Err propagates. On Linux (no mock endpoint) the connect fails the
        // same way. Either path exercises the OPAQUE failure event.
        let result = invoke_github_pr_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &well_formed_args(),
            root.id,
            &root_hash,
        )
        .await;

        std::env::remove_var("CAPRUN_GITHUB_TOKEN");

        assert!(
            result.is_err(),
            "the macOS stub / no-mock POST must propagate Err, never be swallowed"
        );

        // The github_pr_failed event MUST exist and MUST NOT be a success.
        let failed = find_event_by_type(&conn, &session_id.to_string(), "github_pr_failed")
            .unwrap()
            .expect("github_pr_failed event must be durably appended");
        assert_eq!(failed.actor, format!("sink:github.pr:{effect_id}"));
        assert_eq!(failed.parent_id, Some(root.id));
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "github_pr_succeeded")
                .unwrap()
                .is_none(),
            "no github_pr_succeeded event on the failure path"
        );

        // Grep the RAW persisted payload (the hashed content) for the token
        // literal and a raw-response marker — both MUST be absent (opaque).
        let payload: String = conn
            .query_row(
                "SELECT payload FROM events WHERE event_type = 'github_pr_failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            !payload.contains(TOKEN),
            "the bearer token literal must NEVER appear in the hashed event payload"
        );
        assert!(
            !payload.contains("ghp_"),
            "no token prefix may leak into the hashed payload"
        );
        // The actor carries only effect_id, never the token.
        assert!(!failed.actor.contains(TOKEN));
        assert!(
            failed.taint.is_empty(),
            "the opaque failure event carries empty taint"
        );
    }
}
