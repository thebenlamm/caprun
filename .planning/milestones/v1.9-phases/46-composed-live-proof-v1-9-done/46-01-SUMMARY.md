---
phase: 46-composed-live-proof-v1-9-done
plan: 01
subsystem: mock-harness
tags: [http-write, mock-github, live-proof, LIVE-05]
requires: []
provides:
  - "POST /ingest -> 201 http.request.write mock endpoint"
  - "_WRITE_RECEIPTS ledger (distinct from git-push _RECEIPTS)"
affects:
  - "46-02 composed POST leg (http_write_succeeded delivery)"
tech-stack:
  added: []
  patterns: ["stdlib-only test double", "additive first-match routing", "distinct receipt ledger per delivery surface"]
key-files:
  created: []
  modified:
    - scripts/mock-github/server.py
decisions:
  - "Distinct _WRITE_RECEIPTS ledger (not shared with git-push _RECEIPTS) so the two delivery surfaces stay independently observable in sidecar output."
  - "Auth header recorded (authenticated) but not required — mirrors the receive-pack mock (T-46-01-01 accept disposition)."
  - "/ingest routed as first-match case; all pre-existing branches left byte-for-byte intact (T-46-01-02 mitigation)."
metrics:
  duration: "~6 min"
  completed: 2026-07-18
status: complete
---

# Phase 46 Plan 01: Mock http-write /ingest Endpoint Summary

Added a stdlib-only `POST /ingest` -> 201 endpoint to the mock GitHub server so a clean composed `http.request.write` POST leg gets a genuine 201 (`http_write_succeeded`) instead of the prior 404 (`http_write_failed`) — closing wiring gap G3 for LIVE-05.

## What was built

`scripts/mock-github/server.py`:
- `_is_ingest_path(path)` — pure predicate, true only for exactly `/ingest` (query string stripped), mirroring `_is_pulls_path`.
- `_WRITE_RECEIPTS = []` — a ledger distinct from the git-push `_RECEIPTS`, keeping the two delivery surfaces independently observable.
- `_handle_ingest(body)` — records a receipt `{path, body_bytes, authenticated}`, writes a `mock-http-write: RECEIPT <json>` stderr line, and replies `201` with `{"id": <n>, "received": true}`.
- `do_POST` routes `_is_ingest_path` FIRST (additive `if`/`elif` — the former `if _is_pulls_path` became `elif`), ahead of the pulls / receive-pack branches. No existing branch reordered or weakened.
- Module docstring extended with a "v1.9 Phase 46 (G3): http.request.write POST /ingest" section documenting the endpoint contract (mirrors the WG-9 receive-pack section).

Stdlib only — no third-party import, no package-manager install. Runs unmodified on `python:3-slim`.

## What /ingest returns

`POST /ingest` (query string ignored) → HTTP `201`, body `{"id": <1-based receipt index>, "received": true}`, `Content-Type: application/json`. A Basic-auth `Authorization` header is recorded in the receipt's `authenticated` field but is NOT required.

## Verification

- Plan automated verify: `py_compile` + routing assertions → `ingest-routing-ok`.
- Functional smoke test (in-process Handler, no TLS): `/ingest` (auth) → 201 + receipt (`body_bytes=11`, `authenticated=true`); `/ingest?x=1` (no auth) → 201 + `authenticated=false`; distinct `_WRITE_RECEIPTS` vs `_RECEIPTS`.
- Existing routes confirmed intact: `/repos/o/r/pulls` → 201; `/accept/repo.git/git-receive-pack` → 200 report-status + separate git ledger entry; `/owner/repo.git/git-receive-pack` → 404; unknown path → 404.
- `./scripts/check-invariants.sh` → all gates PASS (incl. Gate 4b: `mock-egress-ca` not a default feature — feature-OFF release write-allowlist provably unchanged).

Genuine live 201 / `http_write_succeeded` delivery is proven downstream by 46-02's composed POST leg under `scripts/compose-verify.sh` (mock-egress-ca).

## Existing endpoints status

All pre-existing mock endpoints still work unchanged: `/repos/*/pulls` (201), `/accept/*` git-receive-pack (200 + report-status), `/redirect/*` (302), and both 404 fallbacks. The diff only ADDS the ingest branch, helper, ledger, and docstring section.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. `/ingest` is a deliberate test double (records but never acts); this is the intended behaviour per the threat model (T-46-01-01, accept disposition), not an unwired stub.

## Threat Flags

None. No new security-relevant surface beyond the plan's `<threat_model>` — the endpoint lives only in the Python sidecar (never compiled into `caprun`), on an RFC-6761 non-resolvable TLD behind a cert trusted only under the non-default `mock-egress-ca` feature.

## Commits

- `bc99998`: feat(46-01): add POST /ingest -> 201 http-write mock endpoint (LIVE-05)

## Self-Check: PASSED

- FOUND: scripts/mock-github/server.py (modified, committed)
- FOUND: commit bc99998 in git log
- FOUND: .planning/phases/46-composed-live-proof-v1-9-done/46-01-SUMMARY.md
