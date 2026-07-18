//! sinks/http_write — the broker-resident `http.request.write` (POST/PUT) egress
//! sink (HTTP-W-01, DESIGN-v1.9-egress-policy §2).
//!
//! # Security role (Pattern A, broker-resident — NEVER the confined worker)
//!
//! This module performs ONE authenticated (or unauthenticated) `POST`/`PUT` via
//! the DISTINCT, SSRF-pinned, write-allowlisted `http_request` WRITE egress
//! (`http_request::invoke_http_write`). It is broker-resident and INVOKED only
//! from broker-resident code, exactly like `github_pr.rs` / `email_smtp.rs` —
//! the confined worker cannot reach it (kernel default-deny net + broker-only
//! call sites).
//!
//! # Credential custody (§2.4, D-04)
//!
//! Any write bearer is read from `CAPRUN_HTTP_WRITE_TOKEN` in the broker's LOCAL
//! process env ONLY (`write_bearer()`, mirroring `github_pr::github_token` but
//! OPTIONAL — a write may legitimately need no auth, so an UNSET token yields
//! `None`, NOT a fail-closed Err). It is NEVER a `ValueNode`, a plan-node arg,
//! an audit-DAG literal, the confined worker, or the planner sidecar.
//!
//! # Distinct write allowlist + SSRF pin (§2.1/§2.3)
//!
//! The destination host MUST be on the DISTINCT `WRITE_HOST_ALLOWLIST`
//! (`http_request.rs`) — a GET-readable host is NOT implicitly writable. The
//! write rides the SAME `validate_url` → write-allowlist → resolve-and-pin →
//! `ssrf_check` → redirect-none defense-in-depth as the GET (no classifier
//! re-implemented); `invoke_http_write` owns that path.
//!
//! # Opaque audit (§2.4, T-43-07)
//!
//! `http_write_succeeded` / `http_write_failed` events carry NO url, NO body,
//! and NO credential in their hashed payload — only `effect_id` (in the `actor`
//! field, `sink:http.request.write:<effect_id>`) and a static `event_type`
//! marker. Raw status/response text is routed to this codebase's `eprintln!`
//! logging convention ONLY, and never carries the url/body/token on the write
//! leg (MINOR-4). The failure event is appended FIRST (terminal EVENT before any
//! terminal disposition — the P33/P34 confirm-release audit-gap discipline),
//! then a non-swallowed Err propagates.
//!
//! # NO mint here (Gate 3 unchanged)
//!
//! The WRITE response is CONSUMED, never re-minted into the value store: this
//! module performs NO `ValueStore::mint` / `mint_from_*` and introduces no new
//! mint site — `check-invariants.sh` Gate 3's mint-site allow-list stays
//! byte-identical.
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, PlanNode};
use rusqlite::Connection;
use uuid::Uuid;

use crate::audit::append_event;
use crate::confirmation::ResolvedArg;
use crate::sinks::http_request;

/// The broker-local env var carrying the OPTIONAL write bearer (§2.4). Read ONLY
/// here, ONLY from the broker's own process env — never a plan arg / `ValueNode`
/// / audit literal.
const WRITE_TOKEN_ENV: &str = "CAPRUN_HTTP_WRITE_TOKEN";

/// The exact three-arg set for `http.request.write` (executor sink schema,
/// Plan 43-01).
const WRITE_ARGS: [&str; 3] = ["url", "method", "body"];

/// Serializes any test that mutates the process-global `CAPRUN_HTTP_WRITE_TOKEN`
/// env var — SHARED across this module's tests AND (Plan 43-03) the
/// confirmation.rs http.request.write confirm-release tests, so both cannot race
/// on the same process-wide environment (mirrors `github_pr::GITHUB_ENV_LOCK`).
#[cfg(test)]
pub(crate) static HTTP_WRITE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Read the OPTIONAL write bearer from the broker's LOCAL process env ONLY
/// (§2.4, mirror `github_pr::github_token` but OPTIONAL). Returns `None` when
/// unset — a write may legitimately need no credential, so this does NOT fail
/// closed. NEVER read from a `ValueNode`, a plan-node arg, the audit DB, or
/// `PendingConfirmation`.
fn write_bearer() -> Option<String> {
    std::env::var(WRITE_TOKEN_ENV).ok()
}

/// The frozen, validated PRE-WRITE inputs for an `http.request.write` dispatch
/// (mirrors `github_pr::PreparedPr`). Owned so a caller can validate
/// independently of dispatch.
pub(crate) struct PreparedWrite {
    /// The write URL literal (full https/allowlist/SSRF vet happens in the
    /// egress invoke; here it is only checked present + constructible).
    url: String,
    /// The already-validated write verb (`POST` or `PUT`).
    method: String,
    /// The request body literal (sent verbatim; content-sensitivity is enforced
    /// upstream by the executor I2 Block on a tainted body).
    body: String,
}

/// Look up a required named literal from a frozen `ResolvedArg` snapshot,
/// fail-closed if missing OR empty (the "present + non-empty" precheck).
/// Mirrors `github_pr::required_literal`.
fn required_literal<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Result<&'a str> {
    let literal = resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("http.request.write: missing required `{name}` arg (fail-closed)")
        })?;
    if literal.trim().is_empty() {
        bail!("http.request.write: required `{name}` arg is empty (fail-closed)");
    }
    Ok(literal)
}

/// The fallible PRE-WRITE preparation shared by BOTH invoke paths AND the
/// Plan 43-03 confirm-release precheck: validate all three args are present +
/// non-empty, the method is the SAME `{POST,PUT}` enum the egress enforces
/// (`http_request::validate_write_method` — single source of truth, no drift),
/// and the URL is constructible. Opens NO socket and appends NO event —
/// pure/read-only, so the precheck (fail-closed-RECOVERABLE) and the dispatch
/// validate IDENTICALLY and cannot drift (the P33/P34 audit-gap discipline;
/// mirror `github_pr::prepare_github_pr` / `process_exec::prepare_process_exec`).
pub(crate) fn prepare_http_write(resolved_args: &[ResolvedArg]) -> Result<PreparedWrite> {
    let url = required_literal(resolved_args, "url")?;
    let method = required_literal(resolved_args, "method")?;
    let body = required_literal(resolved_args, "body")?;

    // The SAME method-enum gate the egress applies — precheck and dispatch cannot
    // drift on the accepted verb set.
    http_request::validate_write_method(method)?;

    // URL must be constructible here (socket-free). The full https-only +
    // write-allowlist + SSRF-pin vetting happens in `invoke_http_write`.
    reqwest::Url::parse(url).context("http.request.write: url is not a valid URL")?;

    Ok(PreparedWrite {
        url: url.to_string(),
        method: method.to_string(),
        body: body.to_string(),
    })
}

/// The network leg: read the OPTIONAL broker-env bearer and POST/PUT via the
/// DISTINCT SSRF-pinned write egress. Conn-free so no caller holds an audit lock
/// across the `.await` (mirror `github_pr::post_pr`'s lock discipline).
async fn post_write(prepared: &PreparedWrite) -> Result<(u16, String)> {
    let bearer = write_bearer();
    http_request::invoke_http_write(
        &prepared.url,
        &prepared.method,
        &prepared.body,
        bearer.as_deref(),
    )
    .await
}

/// Fold a WRITE outcome into the OPAQUE two-phase audit, shared by both invoke
/// paths. On `Ok(2xx)`: append `http_write_succeeded`. On `Ok(non-2xx)` OR `Err`:
/// route the raw status/response text to `eprintln!` (the ONLY place it may
/// appear — NEVER the url/body/token, MINOR-4), append an OPAQUE
/// `http_write_failed` event FIRST (terminal EVENT before any terminal
/// disposition — P33/P34), then propagate a non-swallowed `Err`. NO retry.
/// Payloads carry NO url, NO body, and NO credential (T-43-07).
fn append_write_outcome(
    conn: &Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    parent_id: Uuid,
    parent_hash: &str,
    write_result: Result<(u16, String)>,
) -> Result<(Uuid, String)> {
    match write_result {
        Ok((status, _body)) if (200..300).contains(&status) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:http.request.write:{effect_id}"),
                "http_write_succeeded".into(),
                Utc::now(),
                vec![],
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append http_write_succeeded")?;
            Ok((event.id, hash))
        }
        outcome => {
            // Ok(non-2xx) or a transport/gate/build Err — both are audited-abort
            // paths. Raw status/response text goes ONLY to eprintln (status +
            // effect_id only), NEVER the url/body/token, NEVER the payload.
            let err = match outcome {
                Ok((status, body)) => {
                    eprintln!(
                        "[brokerd] http.request.write failed (effect_id={effect_id}): HTTP {status}: {body}"
                    );
                    anyhow::anyhow!(
                        "http.request.write: endpoint returned non-success status {status}"
                    )
                }
                Err(e) => {
                    eprintln!(
                        "[brokerd] http.request.write failed (effect_id={effect_id}): {e}"
                    );
                    e.context("http.request.write POST/PUT failed")
                }
            };
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:http.request.write:{effect_id}"),
                "http_write_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append http_write_failed")?;
            Err(err)
        }
    }
}

/// Resolve the three `http.request.write` args from the broker-owned
/// `ValueStore` into a frozen `ResolvedArg` snapshot. `validate_schema` (Step 0
/// of `submit_plan_node`) already guaranteed each is present + known; a
/// missing/dangling handle here is a broker-internal invariant violation → fail
/// closed (mirror `github_pr::resolve_all_args`). Reusing this snapshot with
/// `prepare_http_write` means the Allowed path, the confirm-release dispatch,
/// and the precheck all validate url/method/body IDENTICALLY.
fn resolve_all_args(store: &ValueStore, plan_node: &PlanNode) -> Result<Vec<ResolvedArg>> {
    let mut out = Vec::with_capacity(WRITE_ARGS.len());
    for name in WRITE_ARGS {
        let arg = plan_node
            .args
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| {
                anyhow::anyhow!("http.request.write plan node missing `{name}` arg")
            })?;
        let record = store
            .resolve(&arg.value_id)
            .ok_or_else(|| anyhow::anyhow!("http.request.write `{name}` handle did not resolve"))?;
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

/// Invoke the live `http.request.write` sink for an `Allowed` plan node.
///
/// Resolves the three args from the broker-owned `ValueStore`, validates them via
/// the SAME socket-free `prepare_http_write` the confirm precheck uses, reads the
/// OPTIONAL broker-env bearer, POSTs/PUTs via the DISTINCT SSRF-pinned write
/// egress, and records the OPAQUE two-phase audit. Returns
/// `(http_write_succeeded event_id, hash)` on a 2xx. Does NOT mint (Gate 3).
///
/// A pre-write build/gate failure is FOLDED into the write result so it ALSO
/// appends an opaque `http_write_failed` FIRST (never a bare `?` with no
/// terminal event). `conn` is the shared mutex-guarded audit connection; the
/// lock is held ONLY for the final synchronous append, never across the
/// `.await`ed write. Wired by Plan 43-03 (server.rs Allowed-decision dispatch).
#[allow(dead_code)] // wired by Plan 43-03 (server.rs Allowed dispatch)
#[allow(clippy::too_many_arguments)]
pub async fn invoke_http_write_sink(
    conn: &Arc<Mutex<Connection>>,
    key: &[u8],
    value_store: &ValueStore,
    session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let resolved = resolve_all_args(value_store, plan_node)?;
    let write_result = match prepare_http_write(&resolved) {
        Ok(prepared) => post_write(&prepared).await,
        Err(e) => Err(e),
    };
    let locked = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
    append_write_outcome(
        &locked,
        key,
        session_id,
        effect_id,
        parent_id,
        parent_hash,
        write_result,
    )
}

/// Invoke the live `http.request.write` sink from a FROZEN `ResolvedArg`
/// snapshot (confirm-release path, mirror
/// `github_pr::invoke_github_pr_from_resolved`). Prepares (validates
/// url/method/body via the SAME `prepare_http_write` the Plan 43-03 precheck
/// uses) and writes, folding ANY pre-write failure into the SAME write result so
/// EVERY failure — pre-write OR transport — appends an opaque `http_write_failed`
/// FIRST then propagates (never a burned confirmation with no terminal event;
/// the P33/P34 MAJOR-1 audit-gap class). `conn` is pre-locked. Wired by Plan
/// 43-03 (confirmation.rs).
#[allow(dead_code)] // wired by Plan 43-03 (confirm-release dispatch)
#[allow(clippy::too_many_arguments)]
pub async fn invoke_http_write_from_resolved(
    conn: &Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let write_result = match prepare_http_write(resolved_args) {
        Ok(prepared) => post_write(&prepared).await,
        Err(e) => Err(e),
    };
    append_write_outcome(
        conn,
        key,
        session_id,
        effect_id,
        parent_id,
        parent_hash,
        write_result,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{append_event, find_event_by_type, open_audit_db};
    use runtime_core::plan_node::{TaintLabel, ValueId};

    /// Fixed, non-secret test MAC key (mirrors `github_pr.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"http-write-rs-unit-test-key-not-secret";

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
            arg("url", "https://github-mock.caprun.test/x"),
            arg("method", "POST"),
            arg("body", "{\"k\":\"v\"}"),
        ]
    }

    // ── write_bearer: OPTIONAL broker-env-only sourcing (§2.4) ──

    #[test]
    fn write_bearer_is_none_when_unset() {
        let _guard = HTTP_WRITE_ENV_LOCK.lock().unwrap();
        std::env::remove_var(WRITE_TOKEN_ENV);
        assert!(
            write_bearer().is_none(),
            "an absent write token is None (OPTIONAL), NOT a fail-closed Err"
        );
    }

    #[test]
    fn write_bearer_reads_broker_env_when_set() {
        let _guard = HTTP_WRITE_ENV_LOCK.lock().unwrap();
        std::env::set_var(WRITE_TOKEN_ENV, "tok-abc");
        let got = write_bearer();
        std::env::remove_var(WRITE_TOKEN_ENV);
        assert_eq!(got.as_deref(), Some("tok-abc"));
    }

    // ── prepare_http_write: present + non-empty + method-enum + constructible,
    // socket-free (shared precheck) ──

    #[test]
    fn prepare_http_write_ok_for_well_formed_args() {
        assert!(prepare_http_write(&well_formed_args()).is_ok());
    }

    #[test]
    fn prepare_http_write_errs_on_missing_required_arg() {
        for missing in ["url", "method", "body"] {
            let mut args = well_formed_args();
            args.retain(|a| a.name != missing);
            assert!(
                prepare_http_write(&args).is_err(),
                "a missing `{missing}` arg must fail closed"
            );
        }
    }

    #[test]
    fn prepare_http_write_errs_on_empty_arg() {
        let mut args = well_formed_args();
        for a in args.iter_mut() {
            if a.name == "body" {
                a.literal = "   ".to_string();
            }
        }
        assert!(
            prepare_http_write(&args).is_err(),
            "an empty/whitespace required arg must fail closed"
        );
    }

    #[test]
    fn prepare_http_write_errs_on_non_write_method() {
        // The SAME {POST,PUT} enum gate the egress applies — a non-write verb is
        // rejected identically in the precheck (no drift).
        let mut args = well_formed_args();
        for a in args.iter_mut() {
            if a.name == "method" {
                a.literal = "DELETE".to_string();
            }
        }
        assert!(prepare_http_write(&args).is_err());
    }

    #[test]
    fn prepare_http_write_errs_on_unconstructible_url() {
        let mut args = well_formed_args();
        for a in args.iter_mut() {
            if a.name == "url" {
                a.literal = "not a url".to_string();
            }
        }
        assert!(prepare_http_write(&args).is_err());
    }

    // ── opaque two-phase audit: no url/body/token in the hashed payload, and the
    // failed event is appended FIRST (terminal event before terminal state,
    // P33/P34), then Err propagates (T-43-07, mirror github_pr's opaque test) ──

    #[tokio::test]
    async fn opaque_audit_no_url_body_or_token_in_appended_event() {
        let _guard = HTTP_WRITE_ENV_LOCK.lock().unwrap();
        const TOKEN: &str = "wtok_SUPERSECRETVALUE1234567890";
        const URL: &str = "https://github-mock.caprun.test/secret-path";
        const BODY: &str = "SECRET-BODY-CONTENT-9876543210";
        std::env::set_var(WRITE_TOKEN_ENV, TOKEN);

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
        let args = vec![arg("url", URL), arg("method", "POST"), arg("body", BODY)];

        // On macOS the live write stubs out (Err); in the default build the empty
        // WRITE_HOST_ALLOWLIST also Errs at the gate. Either path exercises the
        // OPAQUE failure event, appended FIRST, then Err propagated.
        let result = invoke_http_write_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &args,
            root.id,
            &root_hash,
        )
        .await;

        std::env::remove_var(WRITE_TOKEN_ENV);

        assert!(
            result.is_err(),
            "the macOS stub / empty-allowlist / no-mock write must propagate Err, never be swallowed"
        );

        // The http_write_failed event MUST exist and MUST NOT be a success.
        let failed = find_event_by_type(&conn, &session_id.to_string(), "http_write_failed")
            .unwrap()
            .expect("http_write_failed event must be durably appended");
        assert_eq!(failed.actor, format!("sink:http.request.write:{effect_id}"));
        assert_eq!(failed.parent_id, Some(root.id));
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "http_write_succeeded")
                .unwrap()
                .is_none(),
            "no http_write_succeeded event on the failure path"
        );

        // Grep the RAW persisted payload (the hashed content) for the token, url,
        // and body literals — ALL MUST be absent (opaque, T-43-07).
        let payload: String = conn
            .query_row(
                "SELECT payload FROM events WHERE event_type = 'http_write_failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            !payload.contains(TOKEN),
            "the bearer token literal must NEVER appear in the hashed event payload"
        );
        assert!(
            !payload.contains("wtok_"),
            "no token prefix may leak into the hashed payload"
        );
        assert!(
            !payload.contains("secret-path"),
            "the url must NEVER appear in the hashed event payload"
        );
        assert!(
            !payload.contains(BODY),
            "the request body must NEVER appear in the hashed event payload"
        );
        // The actor carries only effect_id, never the token/url/body.
        assert!(!failed.actor.contains(TOKEN));
        assert!(!failed.actor.contains("secret-path"));
        assert!(
            failed.taint.is_empty(),
            "the opaque failure event carries empty taint"
        );
    }
}
