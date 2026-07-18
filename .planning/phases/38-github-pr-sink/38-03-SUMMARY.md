---
phase: 38-github-pr-sink
plan: 03
subsystem: brokerd-sinks
tags: [github, github-pr, ssrf, resolve-and-pin, opaque-audit, bearer-token, reqwest, rustls, pattern-a]
status: complete

# Dependency graph
requires:
  - phase: 37-http-request-sink
    provides: "§3.6 SSRF resolve-and-pin egress (validate_url, HOST_ALLOWLIST incl. api.github.com, ssrf_check, vet_resolved, build_pinned_client, do_pinned_get) + ring/webpki-roots TLS wiring"
  - phase: 16-confirm-binding
    provides: "opaque email_send_succeeded/_failed audit pattern + terminal-EVENT-before-disposition discipline (record_send_failed)"
  - phase: 34-process-exec
    provides: "prepare_process_exec precheck/dispatch single-source-of-truth pattern (P33/P34 audit-gap fix)"
  - phase: 38-01
    provides: "github.pr exact six-arg schema (owner/repo/base/head/title/body) + sensitivity/effect-class rows"
provides:
  - "http_request::invoke_pinned_post — pub(crate) SSRF-pinned POST egress reusing §3.6 resolve-and-pin verbatim"
  - "http_request::resolve_and_pin — shared Linux resolve→vet→pin core (GET+POST), no classifier re-impl"
  - "sinks/github_pr.rs — invoke_github_pr (Allowed), invoke_github_pr_from_resolved (confirm-release), prepare_github_pr (socket-free precheck)"
affects: [38-04, 38-05, github-pr-dispatch, github-pr-confirm, phase-40-mock]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Reuse the GET path's validate_url + allowlist gate + resolve-and-pin for a WRITE POST — one pin path, no duplicated ssrf_check"
    - "FIXED broker-owned API base (api_base()) env-overridable only for the mock harness; owner/repo bind only the URL path (MAJOR-4)"
    - "Bearer token from broker-local env ONLY (github_token(), D-04); opaque succeeded/failed audit (token + response text never hashed)"
    - "Single prepare_github_pr shared by both invoke paths + the 38-05 precheck so validation cannot drift (P33/P34)"

key-files:
  created:
    - crates/brokerd/src/sinks/github_pr.rs
  modified:
    - crates/brokerd/src/sinks/http_request.rs
    - crates/brokerd/src/sinks.rs

key-decisions:
  - "invoke_pinned_post reuses the SAME validate_url → is_host_allowlisted gate → resolve_and_pin path as the GET; the shared Linux resolve-and-pin core was factored out (resolve_and_pin) rather than duplicated, so ssrf_check and the range classifiers are never re-implemented."
  - "api_base() returns the FIXED https://api.github.com, overridable via CAPRUN_GITHUB_API_BASE ONLY for the Phase-40 mock; even when overridden the URL still rides invoke_pinned_post's validate_url + allowlist + pin, so a tainted-but-influenced owner/repo can never redirect the POST host (MAJOR-4). owner/repo are percent-encoded as path segments (T-38-10)."
  - "Bearer token read from CAPRUN_GITHUB_TOKEN broker-local env only (github_token(), mirrors email_smtp::smtp_host); never a ValueNode/plan-arg/audit-literal/worker/planner. Absent token fails closed before any socket."
  - "github_pr_succeeded/_failed events are OPAQUE: actor sink:github.pr:{effect_id}, empty payload/taint; raw status/response text only to eprintln, never the hash chain (T-38-07, mirror email_smtp)."
  - "Both invoke paths fold prepare_github_pr into the SAME post-result so ANY pre-POST build failure ALSO appends a terminal github_pr_failed FIRST — never a bare ? that could leave a burned confirmation with no terminal event (the P33/P34 MAJOR-1 audit-gap class)."
  - "github.pr does NOT mint (it consumes a created-PR response, never re-fetched as a GET). No mint_from_* / .mint() token in github_pr.rs; check-invariants Gate 3 mint-site allow-list stays byte-identical."
  - "reqwest json feature NOT added; body serialized via serde_json at the caller (build_pr_body), sent via .body(...)."

metrics:
  duration_min: 22
  completed: 2026-07-18
  tasks: 2
  files_created: 1
  files_modified: 2
  tests_added: 12
---

# Phase 38 Plan 03: github.pr Sink Adapter + Base-URL-Pinned POST Egress Summary

Implemented the `github.pr` create-PR sink (GITHUB-01) as a broker-resident Pattern A adapter that performs one authenticated `POST /repos/{owner}/{repo}/pulls` via the Phase-37 SSRF resolve-and-pin egress, with a broker-env bearer token and opaque audit — without minting.

## What was built

### Task 1 — pinned POST egress (`http_request.rs`)
- `invoke_pinned_post(url, bearer, json_body) -> Result<(u16, String)>` (`pub(crate)`): `validate_url` → `is_host_allowlisted` gate (Err before any resolve) → the SAME Linux-gated resolve→`vet_resolved`→`build_pinned_client` pin path the GET uses. Sets method POST, `.bearer_auth`, `Accept: application/vnd.github+json`, `User-Agent: caprun`, `X-GitHub-Api-Version: 2022-11-28`, `Content-Type: application/json`; sends the caller-serialized body via `.body(...)`. redirect(none), connect/total timeouts, and the fail-closed response-body byte cap are identical to the GET. Linux-gated real socket leg + macOS no-op stub ("github.pr live POST is Linux-only … deferred to Phase 40"), mirroring `do_pinned_get`.
- Factored `resolve_and_pin(host)` (Linux) as the shared resolve→vet→build-pinned-client core now used by BOTH `do_pinned_get` and `do_pinned_post` — `ssrf_check` and the range classifiers are invoked, never re-implemented.

### Task 2 — the sink adapter (`github_pr.rs`, new)
- `github_token()` (env-only, fail-closed), `api_base()` (fixed `https://api.github.com`, mock-override only), `build_pr_url` (percent-encoded path segments, fixed host), `build_pr_body` (serde_json `{title,body,head,base}`).
- `prepare_github_pr(&[ResolvedArg]) -> Result<PreparedPr>`: socket-free, event-free present+non-empty precheck; shared by both invoke paths and the 38-05 precheck (single source of validation).
- `invoke_github_pr(...)` (Allowed path, `&Arc<Mutex<Connection>>`, lock held only for the append) and `invoke_github_pr_from_resolved(...)` (confirm-release, `&Connection`, `#[allow(dead_code)]` until wired by 38-05). Both POST via `post_pr` (conn-free, off-lock) and route the outcome through the shared opaque `append_pr_outcome` (succeeded on 2xx; failed-event-FIRST then Err otherwise).
- Declared `pub mod github_pr;` in `sinks.rs`.

## Tests (host-portable; real counts)
- `cargo test -p brokerd --lib sinks::http_request::tests`: **37 passed / 0 failed** (3 new POST-gate tests: non-allowlisted, non-https, userinfo base all Err pre-resolve).
- `cargo test -p brokerd --lib sinks::github_pr::tests`: **9 passed / 0 failed** (api_base default; token errs unset; url path + percent-encoding + fixed-host MAJOR-4; overridden base still rides allowlist; prepare Ok/missing/empty; opaque-audit token-literal-absent grep of the raw hashed payload).
- `cargo test -p brokerd` (full lib + integration): **218 passed / 0 failed**.
- `cargo build --workspace`: clean (0 warnings — the transient dead-code warnings from Task 1 cleared once `github_pr.rs` wired `invoke_pinned_post`).
- `./scripts/check-invariants.sh`: **exit 0** — Gate 1 (no EffectRequest) PASS, Gate 3 (mint-site allow-list) unchanged/PASS, Gates 2/4/5 PASS.

Live socket POST behavior is Linux-gated (macOS stub) and deferred to the Phase-40 mock endpoint, per the project's Linux-only enforcement pattern.

## Deviations from Plan

None — plan executed as written. One clarification worth recording: `invoke_github_pr` (Allowed path) also folds its `prepare_github_pr` build step into the post-result routed through `append_pr_outcome`, not just the confirm-release path. This applies the strongest P33/P34 discipline uniformly (a pre-POST build failure appends `github_pr_failed` first) — strictly safer than a bare `?` and consistent with `process_exec`. The only pre-audit `?` is the store-handle resolution in the Allowed path (a broker-internal dangling-handle invariant, matching `git_commit::resolve_arg`).

## Known Stubs

`do_pinned_post`'s real socket leg is a macOS no-op stub by design (Linux-only enforcement, CLAUDE.md); live GitHub behavior is Phase 40 (mock endpoint). This is the same intentional cfg-split as `do_pinned_get` and is not a data-flow stub.

## Threat Flags

None — no security surface beyond the plan's `<threat_model>` (T-38-07..10) was introduced. The POST egress rides the existing §3.6 pin + allowlist; the only new host reachability is via the already-allowlisted `api.github.com`.

## Self-Check: PASSED
- `crates/brokerd/src/sinks/github_pr.rs` — FOUND (created)
- `crates/brokerd/src/sinks/http_request.rs`, `crates/brokerd/src/sinks.rs` — FOUND (modified)
- Commit `39c22df` (Task 1), `372bda9` (Task 2) — both in `git log`.
