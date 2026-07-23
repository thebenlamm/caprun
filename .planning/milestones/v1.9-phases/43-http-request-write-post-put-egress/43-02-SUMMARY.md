---
phase: 43-http-request-write-post-put-egress
plan: 02
subsystem: brokerd/sinks
status: complete
tags: [http-write, egress, ssrf, allowlist, opaque-audit, credential-custody]
requires: ["43-01"]
provides: ["invoke_http_write", "http_write module (invoke_http_write_sink + invoke_http_write_from_resolved + prepare_http_write)"]
affects: ["Plan 43-03 (server.rs Allowed-dispatch + confirmation.rs confirm-release wiring)"]
tech-stack:
  added: []          # ZERO new crates (HYG-01) — reuses shipped reqwest+rustls(ring)+webpki-roots
  patterns: ["reuse shipped SSRF resolve-and-pin verbatim", "opaque two-phase audit", "socket-free shared precheck", "broker-env-only credential custody"]
key-files:
  created:
    - crates/brokerd/src/sinks/http_write.rs
  modified:
    - crates/brokerd/src/sinks/http_request.rs
    - crates/brokerd/src/sinks.rs
decisions:
  - "WRITE_HOST_ALLOWLIST ships EMPTY (fail-closed): the release build is writable to nothing until an operator surfaces a target — the maximally fail-closed reading of DESIGN §2.1's 'operator-surfaced deployment constant'. Under mock-egress-ca the Phase-46 mock host (MOCK_EGRESS_HOST) is additionally admitted."
  - "validate_write_method made pub(crate) so prepare_http_write and the egress share ONE {POST,PUT} enum gate (no precheck/dispatch drift)."
  - "http.request.write reuses the same mock host as the GET (github-mock.caprun.test) so the write leg rides the existing mock CA cert in Phase 46."
metrics:
  tasks: 2
  commits: 2
  completed: 2026-07-18
---

# Phase 43 Plan 02: http.request.write WRITE (POST/PUT) egress path Summary

Built the broker-resident WRITE egress for `http.request.write`: a distinct write host-allowlist, a generic SSRF-pinned write-invoke reusing the shipped GET infra verbatim, broker-env-only OPTIONAL credential custody, an opaque non-minting two-phase audit, and a socket-free `prepare_http_write` precheck shared by the Allowed and confirm-release paths — ready for Plan 43-03 dispatch wiring.

## Tasks Completed

- **Task 1** (`1394420`) — `WRITE_HOST_ALLOWLIST` + generic `invoke_http_write` in `http_request.rs`.
  - Distinct `WRITE_HOST_ALLOWLIST` (empty base, fail-closed) separate from the GET `HOST_ALLOWLIST`; `is_write_host_allowlisted` mirrors `is_host_allowlisted`; mock-egress-ca admits `MOCK_EGRESS_HOST` only.
  - `validate_write_method` (pure `{POST,PUT}` enum, defense-in-depth §2.6).
  - `invoke_http_write(url, method, body, bearer: Option<&str>)` order: `validate_url` → method gate → distinct write-allowlist gate (Err BEFORE any resolve) → shared `resolve_and_pin`/`vet_resolved`/`ssrf_check`/`build_pinned_client` (redirect-none, body cap) → POST/PUT with bearer only when `Some`. Linux socket leg + macOS no-op stub. Does NOT reuse `invoke_pinned_post`; re-implements NO classifier.
- **Task 2** (`38a97cb`) — `http_write.rs` orchestration module + `pub mod http_write;` in `sinks.rs`.
  - `write_bearer()` (OPTIONAL `CAPRUN_HTTP_WRITE_TOKEN`, `None` when unset), `resolve_all_args {url,method,body}`, socket-free `prepare_http_write` (present+non-empty + shared method-enum re-check + url-constructible), `post_write`, opaque `append_write_outcome`, and the two entry points `invoke_http_write_sink` (Allowed, `Arc<Mutex>`) + `invoke_http_write_from_resolved` (confirm, pre-locked `&Connection`) — mirroring `github_pr` minus CAS/grant machinery.
  - Opaque two-phase audit: `http_write_succeeded` on 2xx; else raw status to `eprintln!` ONLY (no url/body/token), `http_write_failed` appended FIRST, then non-swallowed Err. Mints NOTHING.

## Verification

- `cargo build --workspace` — clean, no warnings.
- `cargo test -p brokerd --no-fail-fast` — **206 lib tests + all integration binaries green** (9 new http_request WRITE tests + 8 new http_write tests, 0 failures).
- `./scripts/check-invariants.sh` — **exit 0, all gates PASS**:
  - Gate 3 (mint-site allow-list) — byte-identical; `http_write.rs` contains no `mint_from_*(` / `.mint(` call token (only a doc-comment mention of `mint_from_*`).
  - Gate 5 (aws-lc-rs absent + no openssl-sys via reqwest) — still green (ZERO new crates, HYG-01).
- Linux socket-leg tests are `#[cfg(target_os="linux")]` and compile to no-ops on this macOS host (expected per CLAUDE.md); the host-portable pre-socket gate tests run and pass here. Full Linux socket-leg verification via `bash scripts/mailpit-verify.sh` with `MAILPIT_VERIFY_CMD='cargo test -p brokerd sinks::http_write:: sinks::http_request::'` is deferred to the phase Linux-verification step (no live write mock exists until Phase 46).

## Must-Haves Satisfied

- Write admitted ONLY to a host on the distinct `WRITE_HOST_ALLOWLIST` (GET-readable ≠ writable) — proven by the host-portable `write_vs_read_allowlist_split` test.
- WRITE path reuses shipped SSRF resolve-and-pin verbatim — no classifier re-implemented.
- Credential broker-env-only, OPTIONAL, never a plan arg/ValueNode/audit literal, never logged on the write leg.
- Response NOT minted (Gate 3 byte-identical); two-phase audit OPAQUE (payload-scrub test asserts no url/body/token in the hashed event).
- Tainted-body and clean-body writes validate IDENTICALLY through `prepare_http_write` (shared socket-free precheck; method gate is the single shared `validate_write_method`).

## Deviations from Plan

None — plan executed as written. One planned detail pinned by judgment: the plan says "seed `WRITE_HOST_ALLOWLIST` default membership per DESIGN §2.1"; §2.1 names no concrete production write host (unlike the GET `api.github.com`), so the base set ships **empty** (fail-closed, operator-surfaced) with `MOCK_EGRESS_HOST` admitted only under `mock-egress-ca`. Documented as a decision above, enforced by the `not(mock-egress-ca)` invariant test.

## Known Stubs

- `WRITE_HOST_ALLOWLIST = &[]` is an intentional fail-closed security posture (writable to nothing until an operator surfaces a target), NOT a placeholder stub. The Linux socket legs are `#[cfg(target_os="linux")]` no-ops on macOS by project convention. No functionality-blocking stubs.

## Self-Check: PASSED

- `crates/brokerd/src/sinks/http_write.rs` — FOUND
- `crates/brokerd/src/sinks/http_request.rs` (WRITE additions) — FOUND
- `crates/brokerd/src/sinks.rs` (`pub mod http_write;`) — FOUND
- Commit `1394420` (Task 1) — FOUND
- Commit `38a97cb` (Task 2) — FOUND
