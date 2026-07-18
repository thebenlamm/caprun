---
phase: 37-http-request-get-egress
plan: 04
subsystem: brokerd
tags: [http, ssrf, supply-chain, tcb, dos, audit, adversarial-fixes]
status: complete
requires:
  - "37-01/02/03 (http.request GET egress: runtime-core rows, invoke_http_get, mint_from_http dispatch)"
provides:
  - "aws-lc-rs-free workspace build graph (ring-only crypto in the TCB) + check-invariants.sh Gate 5 enforcing it"
  - "http.request GET hardening: default-port pin, connect/total timeout, fail-closed body cap, http_request_failed terminal audit event"
  - "extended ssrf_check classifier (v4 multicast/reserved/benchmark/IETF-protocol; v6 NAT64-WKP/6to4/Teredo/IPv4-compatible)"
affects:
  - "Phase 38 (github.pr / http.request POST) inherits the ring-only TLS recipe + the hardened GET client"
  - "Phase 40 composed live proof (real GET) inherits timeout + body-cap + failed-event audit completeness"
tech-stack:
  added: []
  patterns:
    - "reqwest rustls-no-provider + explicit ring CryptoProvider via use_preconfigured_tls (shared brokerd + caprun-planner recipe)"
    - "fail-closed streaming body cap (resp.chunk() loop, mirrors process_exec read_capped)"
    - "opaque _failed terminal audit event on a failed effect (email_smtp convention)"
key-files:
  created:
    - .planning/phases/37-http-request-get-egress/37-04-SUMMARY.md
  modified:
    - cli/caprun-planner/Cargo.toml
    - cli/caprun-planner/src/openai.rs
    - crates/brokerd/Cargo.toml
    - crates/brokerd/src/sinks/http_request.rs
    - crates/brokerd/src/server.rs
    - scripts/check-invariants.sh
decisions:
  - "caprun-planner adopts brokerd's EXACT rustls-no-provider + ring + webpki-roots recipe (not just any provider) so the shared workspace rustls unit is aws-lc-rs-free under resolver-3 unification."
  - "Default-port pin implemented as parsed.port().is_none() — the WHATWG parser reports an explicit :443 as None, so the https default stays accepted while any non-default port is rejected."
  - "http_request_failed is audit-completeness only (terminal EVENT, no terminal STATE) — explicitly NOT the P33/P34 state-before-event class."
metrics:
  duration: ~1h
  completed: 2026-07-18
---

# Phase 37 Plan 04: Adversarial-Review Fixes Summary

Six Phase-37 adversarial-review findings fixed in blast-radius order — one MAJOR
(supply-chain / TCB integrity: aws-lc-rs C crypto removed from the workspace
graph), four MINOR (SSRF port pin, DoS timeout+body-cap, audit completeness,
SSRF classifier breadth), and two NIT comment corrections — each committed
atomically and re-verified on host (macOS) and in a Linux container.

## Fixes

### FIX 1 (MAJOR — supply-chain / TCB integrity): aws-lc-rs removed from the build graph
- **Commit:** `ba9c398`
- **Root cause:** `cli/caprun-planner`'s `reqwest { features=["rustls"] }` enables
  hyper-rustls' default provider + rustls-platform-verifier, which under
  resolver-3 feature unification activated the `aws-lc-rs` C provider on the
  SINGLE shared `rustls` build unit that `crates/brokerd` also links.
- **Fix:** switched caprun-planner to the exact brokerd recipe —
  `reqwest { default-features=false, features=["rustls-no-provider","json"] }`
  plus `rustls { features=["ring","std","tls12"] }` + `webpki-roots`, and built
  the OpenAI client via `reqwest::Client::builder().use_preconfigured_tls(ring_webpki_tls_config())`
  instead of `Client::new()`. Preserves working OpenAI HTTPS (client builds, crate compiles).
- **Gate:** `check-invariants.sh` Gate 5 asserts `cargo tree --workspace -i aws-lc-rs`
  finds nothing (workspace scope) and no `openssl-sys` reaches via a reqwest path.

### FIX 2 (MINOR — SSRF port pin)
- **Commit:** `edf28f0`
- `validate_url` now bails unless `parsed.port().is_none()`. reqwest's
  `.resolve(host, socket_addr)` pins only the IP and connects on the URL's port,
  so an unconstrained port meant "checked IP ≠ connected endpoint" in the port
  dimension. Test: `validate_url_rejects_non_default_port` (`:8080`/`:22` rejected, `:443` accepted).

### FIX 3 (MINOR — DoS: timeout + body cap)
- **Commit:** `08f30c4`
- `build_pinned_client`: `connect_timeout(10s)` + total `timeout(30s)`.
- `do_pinned_get`: streams the body via `resp.chunk()` with a fail-closed 10 MiB
  cap (`check_body_cap`), mirroring `process_exec::read_capped` — over the cap
  errors, never truncates. Host-portable tests: `body_cap_is_fail_closed_at_the_boundary`,
  `body_cap_trips_while_accumulating_synthetic_chunks`.

### FIX 4 (MINOR — audit completeness)
- **Commit:** `1d3a49b`
- server.rs Allowed http.request Err arm now appends an OPAQUE-payload
  `http_request_failed` terminal audit event (email_smtp `_failed` convention:
  NO url / NO error text in the hashed payload), chained on the current head and
  advancing it so `verify_chain` stays linear. Explicitly audit-completeness
  only — NO terminal STATE is written on failure (not the P33/P34 class).

### FIX 5 (MINOR — SSRF defense-depth)
- **Commit:** `e493ddd`
- Extended `ssrf_check`: v4 multicast (224/4), reserved (240/4), benchmark
  (198.18/15), IETF-protocol (192.0.0/24 incl. 192.0.0.170); v6 NAT64-WKP
  (64:ff9b::/96), 6to4 (2002::/16), Teredo (2001:0::/32) denied wholesale, and
  deprecated IPv4-compatible (`::a.b.c.d`) embedded-v4 re-checked. Boundary unit
  tests for each range (pure, host-portable).

### FIX 6 (NITs — comment accuracy)
- **Commit:** `263fe77`
- server.rs: corrected the claim that a 30x is an `Err` — with
  `redirect::Policy::none()` a 30x is an `Ok` response whose body is minted +
  demotes; the redirect is neutralized by not being followed.
- brokerd/Cargo.toml + http_request.rs module doc: reworded the "NEVER a
  dependency of the confined worker" claim — reqwest/rustls/ring ARE linked into
  the caprun-worker image via `cli/caprun` → `brokerd`; the real net-deny
  boundary is kernel-enforced (default-deny network sandbox), not the dependency graph.

## Verification (REAL results)

### Host (macOS)
- `cargo tree --workspace -i aws-lc-rs` → `error: package ID specification 'aws-lc-rs' did not match any packages` (EXIT 101 = ABSENT). **Before FIX 1:** `aws-lc-rs v1.17.1 ← rustls ← {brokerd/caprun, hyper-rustls ← reqwest ← brokerd + caprun-planner, rustls-platform-verifier, tokio-rustls}`.
- `cargo tree --workspace -i openssl-sys` → `warning: nothing to print` (no reqwest path; no openssl-sys at all on the host graph).
- `cargo tree --workspace -i ring` → `ring v0.17.14 ← rustls ← {brokerd, caprun-planner, hyper-rustls ← reqwest, ...}` (pure-Rust provider wired for both reqwest users).
- `cargo build --workspace` → clean.
- `cargo test -p brokerd` → 152→ (lib) all pass; full suite 0 failures on host.
- `cargo test -p brokerd --lib http_request` → 34 passed / 0 failed (all new port/body-cap/SSRF tests).
- `./scripts/check-invariants.sh` → all gates PASS (Gate 5: aws-lc-rs absent, no openssl-sys via reqwest).

### Linux container (`scripts/mailpit-verify.sh`, unprivileged rust:1)
- Whole workspace (incl. brokerd + caprun-planner) **compiles on Linux** — confirms the cfg(target_os="linux") `do_pinned_get` `resp.chunk()` streaming path builds under the `rustls-no-provider` feature set (cfg-linux-test-blindness cleared).
- `cargo test -p brokerd` lib: **155 passed / 0 failed** (includes both process_exec env_clear/spawn tests and all 34 http_request tests).
- `check-invariants.sh` in-container: **All invariant gates PASSED** — Gate 5 PASS (aws-lc-rs absent on the Linux target graph too).

## Deferred Issues

**Pre-existing (NOT caused by these fixes): 3 `tests/git_commit_spawn.rs` `linux::`
failures in-container.** `git_commit_produces_real_commit_...`,
`exec_child_does_not_inherit_broker_env`, `planted_hook_and_alias_...` fail with
"HEAD did not advance to a real commit" / empty git author. Falsified as a
regression: checking out the pre-fix commit `c4de35f` and running the same test
binary in the same container reproduces the **identical 3 failures**. These are
environmental (real-`git` execution / identity drift in the fresh rust:1
container vs. the v1.7 391/0 proof run), in the Phase 33/34 git.commit path,
which these P37 fixes do not touch (server.rs diff is confined to the
http.request arm; process_exec.rs / git_commit.rs untouched; the shared confined
launcher works — process_exec's own env_clear test passes). Out of scope for this
task; flagged for a separate look at the container git environment.

## Deviations from Plan

- **FIX 6 scope extension (accuracy):** in addition to the two named comments,
  the same inaccurate "deps are broker-only / boundary" claim in the
  `http_request.rs` module doc was corrected for consistency (same factual error).
- Did NOT touch `.planning/ROADMAP.md` or `STATE.md` per task constraint (no
  state advancement performed for this fix pass).

## Self-Check: PASSED
- `.planning/phases/37-http-request-get-egress/37-04-SUMMARY.md` — created (this file).
- Commits present in `git log`: `ba9c398`, `edf28f0`, `08f30c4`, `e493ddd`, `1d3a49b`, `263fe77`.
- Constraints held: no raw EffectRequest; sink sensitivity hardcoded; sole mint site `mint_from_http` (Gate 3 PASS); `check-invariants.sh` exits 0.
