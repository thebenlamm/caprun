---
phase: 37-http-request-get-egress
plan: 02
subsystem: brokerd / net-egress
tags: [http-egress, ssrf, tls, reqwest, rustls, ring, supply-chain]
requires:
  - "runtime-core TaintLabel::HttpRaw (Plan 37-01)"
provides:
  - "brokerd::sinks::http_request::invoke_http_get — broker-side SSRF-defended GET egress"
  - "brokerd::sinks::http_request::ssrf_check — pure IpAddr SSRF classifier (host-portable)"
  - "brokerd reqwest(rustls-no-provider)+ring+webpki-roots+url deps (broker-only)"
affects:
  - "Plan 37-03 (server dispatch: mint_from_http + I1 demotion will call invoke_http_get)"
  - "Phase 38 github.pr (reuses this GET egress + resolve-and-pin infra)"
tech-stack:
  added:
    - "reqwest =0.13.4 (default-features=false, features=[rustls-no-provider])"
    - "rustls 0.23 (default-features=false, features=[ring,std,tls12])"
    - "webpki-roots 1.0.8"
    - "url 2.5"
  patterns:
    - "Pattern A broker-resident egress (mirrors email_smtp.rs); worker stays net-denied"
    - "SSRF resolve-and-pin: pure classifier + fail-closed vetting + reqwest .resolve() pin"
    - "Linux-gated real socket leg, host-portable pure classifiers (CLAUDE.md cfg pattern)"
key-files:
  created:
    - crates/brokerd/src/sinks/http_request.rs
  modified:
    - crates/brokerd/Cargo.toml
    - crates/brokerd/src/sinks.rs
    - Cargo.lock
decisions:
  - "reqwest's own `rustls` feature pulls the aws-lc-rs C provider; selected `rustls-no-provider` + explicit ring to honor DESIGN §5.1 lean-ring"
  - "webpki-roots supplied explicitly via use_preconfigured_tls (not rustls-platform-verifier) for env_clear hermeticity (DESIGN §5.2)"
  - "validate_url rejects IP-encoded hosts via url::Host typed determination (same parser reqwest uses) rather than re-parsing host_str"
  - "vet_resolved is fail-closed: any denied resolved IP denies the whole request (mixed DNS answer defense)"
metrics:
  tasks_completed: 2
  files_changed: 4
  commits: 3
  tests_added: 23
  duration: "~35m"
  completed: 2026-07-18
status: complete
---

# Phase 37 Plan 02: brokerd http.request GET egress + SSRF defense Summary

Broker-side, read-only `http.request` GET egress (Pattern A) with a pure, host-portable SSRF resolve-and-pin classifier: reqwest pinned to the SSRF-vetted resolved IP with redirects off, backed by a lean pure-Rust ring + compiled-in webpki-roots TLS stack — no minting or session demotion (Plan 03 owns those). Covers HTTP-01 (allowlisted GET egress) and HTTP-03 (SSRF resolve-and-pin).

## What was built

**Task 1 — brokerd net deps (broker-only).** Added `reqwest =0.13.4` + `rustls 0.23` (ring) + `webpki-roots 1.0.8` (and, in Task 2, `url 2.5`) to `crates/brokerd/Cargo.toml` only. These are never a dependency of the confined worker.

**Task 2 (TDD) — `crates/brokerd/src/sinks/http_request.rs`:**
- `HOST_ALLOWLIST: &[&str] = ["api.github.com"]` — a hardcoded broker-owned trusted-config constant (NOT a policy file), documented as a security property never runtime-configurable from a plan node / ValueNode / audit DB (D-04 discipline).
- `validate_url` — rejects `userinfo@`, any non-`https` scheme, and IP-encoding tricks (decimal/octal/hex-packed and plain IP literals, v4 and v6) by requiring `url::Host::Domain`. Pure, host-portable.
- `ssrf_check(IpAddr)` — the load-bearing pure classifier: denies loopback, RFC1918, link-local (incl. cloud-metadata 169.254.169.254), CGNAT (100.64/10), ULA (fc00::/7), IPv6-mapped-IPv4 (::ffff:0:0/96, embedded v4 re-checked), and unspecified. Host-portable.
- `vet_resolved` — fail-closed vetting of the resolved set (empty → Err; any denied IP → Err; else pin the first).
- `build_pinned_client` / `ring_webpki_tls_config` — redirect-free (`Policy::none()`), IP-pinned (`.resolve(host, pinned)`) reqwest client with a preconfigured ring + webpki-roots rustls `ClientConfig`. Host-portable (type-checked + runtime-built on macOS by a unit test; no socket).
- `invoke_http_get` — validate_url → allowlist gate (Err BEFORE any resolve) → [Linux] resolve → `vet_resolved` → pin → GET → body text. macOS no-op stub returns Err (live GET deferred to Phase 40). Performs NO mint / NO audit event / NO session demotion.

## TLS-backend evidence (crates.io + cargo-tree)

- `cargo add reqwest@0.13.4 --dry-run --features rustls` → enables `__rustls-aws-lc-rs` (the aws-lc-rs C crypto provider). To honor DESIGN §5.1 "lean ring / minimize untrusted C in the TCB", selected `rustls-no-provider` (reqwest bundles NO provider + NO default roots) and supplied both explicitly: the pure-Rust `ring` provider (via the `rustls` crate's `ring` feature) and compiled-in `webpki-roots` anchors.
- `cargo tree -p brokerd -i openssl-sys` → **"nothing to print"** (openssl-sys absent from brokerd's graph).
- `cargo tree -p brokerd -i aws-lc-rs` → **"did not match any packages"** (aws-lc-rs absent).
- `cargo tree -p brokerd -i ring` → `ring v0.17.14` present, pulled by `rustls v0.23.41` (used by brokerd directly + by hyper-rustls/tokio-rustls/reqwest). `webpki-roots v1.0.8` present.
- `native-tls v0.2.18` IS in brokerd's graph but is pulled ONLY by `lettre v0.11.22` (pre-existing Phase 13 SMTP dep) — NOT by reqwest. reqwest's TLS path is rustls/ring only.
- Supply-chain: reqwest already vetted in-workspace at cli/caprun-planner (seanmonstar/hyper, not yanked, ~573M downloads); ring (briansmith) + webpki-roots (rustls project) are the Phase-35 DESIGN-gate-selected §5.1/§5.2 crates. Pinned `=0.13.4` identical to the planner.

## Verification (real results)

- `cargo build -p brokerd` — Finished, **0 warnings, 0 errors**.
- `cargo test -p brokerd http_request` — **23 passed; 0 failed** (RED gate first showed 23 failing via `todo!()`, then GREEN).
- `cargo test -p brokerd ssrf` — **11 passed; 0 failed** (the ssrf_* subset).
- `cargo test -p brokerd --lib` — **140 passed; 0 failed** (whole brokerd lib, no regression).
- `bash ./scripts/check-invariants.sh` — **all 4 gates PASS, exit 0** (Gate 3 unaffected — this module has no mint token).
- Live-HTTPS resolve/connect behavior intentionally NOT verified here (macOS dev host); deferred to Phase 40's live Linux proof, per plan.

## must_haves coverage

- Non-allowlisted host rejected before any resolve — `invoke_http_get_rejects_non_allowlisted_host_without_network` (host-portable).
- SSRF classifier denies every §3.6 range — 11 host-portable ssrf_* tests (loopback v4/v6, RFC1918, link-local, metadata, CGNAT + boundary, ULA, IPv6-mapped, unspecified, public-allow).
- url validation rejects userinfo@ / non-https / IP-encoding — 3 validate_url_rejects_* tests + a plain-domain accept.
- reqwest disables redirects + pins the SSRF-vetted IP — `build_pinned_client` (Policy::none() + .resolve()); the checked IP == connected IP because `vet_resolved` returns the exact addr passed to `.resolve()` (no re-resolve).

## Deviations from Plan

### Auto-fixed / clarifications

**1. [Rule 3 - Blocking] Added `url 2.5` as a direct dep.** `validate_url` needs `url::Host` typed matching to robustly reject IP-encoded hosts using the same parser reqwest uses. `url` was already a reqwest transitive; adding it as a direct dep is a zero-cost, more-correct SSRF choice than re-parsing `host_str`. Committed in the GREEN commit.

**2. [Rule 2 - Hardening] `ssrf_check` also denies unspecified (0.0.0.0 / ::) and IPv4 broadcast.** Not in the plan's explicit enumeration but clearly SSRF-relevant fail-closed ranges; added with dedicated test coverage.

**3. Feature-selection deviation (recorded per plan instruction).** Used `rustls-no-provider` + explicit ring rather than reqwest's `rustls` feature (which the planner uses) because the latter pulls aws-lc-rs — see TLS-backend evidence above. This is the plan's own contingency ("if a clean ring selection is not expressible in reqwest features... lean ring first and record the exact choice").

## Accepted residuals

- `rustls-platform-verifier v0.7.0` is compiled into brokerd's graph as an unconditional reqwest 0.13.4 transitive dependency, but it is **never invoked**: `build_pinned_client` uses `use_preconfigured_tls` with an explicit webpki-roots root store, so the system-store verifier code path is dead. `env_clear()` hermeticity (DESIGN §5.2) is preserved — validated live in Phase 40.

## Parallel-execution note

Ran in the same working tree concurrently with Plan 37-01 (which owns runtime-core `TaintLabel::HttpRaw` + executor sink schema/sensitivity). One transient build hit a mid-commit state of 37-01's HttpRaw rollout; rebuilding after 37-01's `feat(37-01)` commit (`0041ab7`) landed was clean. All three of this plan's commits staged only this plan's files (Cargo.toml, sinks.rs, http_request.rs, Cargo.lock) — zero cross-contamination with 37-01's files.

## Commits

- `8aa0d99` chore(37-02): add brokerd reqwest(rustls-no-provider)+ring+webpki-roots deps
- `16bbc43` test(37-02): add failing http_request SSRF/url/allowlist tests (RED)
- `d00d52d` feat(37-02): implement http_request SSRF-defended GET egress (GREEN)

## Self-Check: PASSED

All created files present (http_request.rs, 37-02-SUMMARY.md); all three commits (8aa0d99, 16bbc43, d00d52d) exist in the log.
