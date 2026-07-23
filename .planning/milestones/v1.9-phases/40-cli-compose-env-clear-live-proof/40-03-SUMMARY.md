---
phase: 40-cli-compose-env-clear-live-proof
plan: 03
subsystem: infra
tags: [verification-harness, docker, tls, mock-github, mailpit, ssrf, mock-egress-ca, live-proof]

# Dependency graph
requires:
  - phase: 40-cli-compose-env-clear-live-proof
    plan: 02
    provides: "non-default brokerd/mock-egress-ca feature + egress trust anchor (mock-egress-ca.der)"
  - phase: 38-github-pr-sink
    provides: "github.pr sink with CAPRUN_GITHUB_API_BASE override + invoke_pinned_post"
provides:
  - "scripts/mock-github/server.py — stdlib-only HTTPS mock GitHub endpoint (201 to POST /repos/*/pulls)"
  - "scripts/mock-github/certs/* — self-signed test cert+key matching the 40-02 DER trust anchor"
  - "scripts/compose-verify.sh — composed Linux verification harness (Mailpit + mock GitHub)"
  - "regenerated crates/brokerd/tests/fixtures/mock-egress-ca.der (matched to the checked-in mock cert)"
affects: [40-04, compose-live-proof, orchestrator-closing-gate]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Stdlib-only (http.server+ssl) mock over TLS — no pip/package-manager dependency"
    - "Self-signed CA:TRUE cert (no EKU) reused as BOTH the rustls trust anchor AND the server end-entity cert"
    - "PUBLIC-range docker subnet (203.0.113.0/24, RFC 5737) so a mock IP passes ssrf_check unmodified — no TCB bypass"
    - "Per-container --add-host mapping (only the mock host; api.github.com never remapped)"
    - "TRUE container exit code captured before any pipe (set +e; rc=$?; set -e)"

key-files:
  created:
    - scripts/mock-github/server.py
    - scripts/mock-github/certs/github-mock.caprun.test.pem
    - scripts/mock-github/certs/github-mock.caprun.test.key
    - scripts/mock-github/README.md
    - scripts/compose-verify.sh
  modified:
    - crates/brokerd/tests/fixtures/mock-egress-ca.der

key-decisions:
  - "Regenerated a matched cert+key+DER triple: 40-02 discarded its keypair, so an identical-fingerprint match to the OLD DER was impossible. The new PEM cert == the new DER anchor (same fingerprint 6A:39:...); 40-02's tests assert anchor COUNT, not fingerprint, so the swap is safe."
  - "Used 40-02's EXACT openssl command (CA:TRUE, SAN github-mock.caprun.test, no EKU) — no-EKU lets rustls/webpki accept the cert as both trust anchor and server cert."
  - "Followed the plan's literal default COMPOSE_VERIFY_CMD (plain leading `cargo build --workspace`): cargo feature unification rebuilds the caprun binary (which hosts the in-process broker egress) WITH the anchor during the feature-carrying `cargo test`, so no feature is needed on the leading build."

patterns-established:
  - "Composed verification harness as a SIBLING script (not an edit) so the stable single-test recipe is untouched."

requirements-completed: []

coverage:
  - id: T1
    description: "Stdlib-only mock GitHub HTTPS server answers 201 + PR JSON to POST /repos/*/pulls, 404 otherwise, presenting the github-mock.caprun.test cert the feature trusts."
    requirement: "LIVE-03"
    verification:
      - kind: other
        ref: "standalone TLS smoke: curl --cacert <test cert> POST .../pulls => 201 + {number,html_url}; GET / => 404; POST .../issues => 404"
        status: pass
      - kind: other
        ref: "python3 ast.parse(server.py) OK (stdlib only); cert SAN=github-mock.caprun.test; PEM fingerprint == DER anchor fingerprint; key modulus == cert modulus"
        status: pass
    human_judgment: false
  - id: T2
    description: "compose-verify.sh stands up Mailpit + mock GitHub on a 203.0.113.0/24 network, add-hosts only the mock host, runs the seccomp=unconfined rust:1 container with --features brokerd/mock-egress-ca and a leading cargo build, captures the true exit code before any pipe, cleans up both sidecars."
    requirement: "LIVE-03"
    verification:
      - kind: other
        ref: "bash -n + shellcheck clean; plan grep gate PASS (203.0.113 / add-host / mock-egress-ca / cargo build --workspace / set +e|rc=$? / seccomp=unconfined / !privileged)"
        status: pass
      - kind: other
        ref: "docker dry-run: created 203.0.113.0/24 net, mock sidecar at fixed IP 203.0.113.2, peer container reached it over TLS via --add-host trusting the test CA => 201 + PR JSON; trap teardown left zero stray container/network"
        status: pass
    human_judgment: false

# Metrics
duration: 20min
completed: 2026-07-18
status: complete
---

# Phase 40 Plan 03: Mock GitHub HTTPS endpoint + compose-verify.sh harness Summary

**A stdlib-only HTTPS mock GitHub endpoint plus a sibling `compose-verify.sh` that stands up Mailpit + the mock on ONE public-range docker network, add-hosts only the mock (api.github.com untouched), and runs the unprivileged rust:1 suite with the `mock-egress-ca` feature and a leading workspace build — true exit code captured before any pipe.**

## Performance
- **Duration:** ~20 min
- **Completed:** 2026-07-18
- **Tasks:** 2
- **Files:** 6 (5 created, 1 modified)

## Accomplishments
- **`scripts/mock-github/server.py`** — ~90-line stdlib-only (`http.server` + `ssl`, no dependency) HTTPS server: 201 + plausible created-PR JSON (`number`, `html_url`, `state`, `id`, `title`) to `POST /repos/<owner>/<repo>/pulls`, 404 to everything else; presents `github-mock.caprun.test`. Runs on `python:3-slim` with no `pip install`.
- **`scripts/mock-github/certs/github-mock.caprun.test.{pem,key}`** — a self-signed test cert (CA:TRUE, SAN `github-mock.caprun.test`, no EKU), reproduced via 40-02's exact offline openssl command. The SAME cert, DER-encoded, is now the broker's feature-gated trust anchor — so the mock's leaf cert IS the anchor.
- **`scripts/mock-github/README.md`** — documents the reproduction command, the cert⇄anchor relationship, and the test-only / no-production-trust posture.
- **`scripts/compose-verify.sh`** — the composed Linux harness (sibling of `mailpit-verify.sh`, not an edit): public-range network (203.0.113.0/24) → Mailpit + mock sidecars → resolve Mailpit IP → readiness-gate the mock → unprivileged rust:1 run with `--add-host github-mock.caprun.test:<mock IP>` (api.github.com NOT remapped), `CAPRUN_GITHUB_API_BASE=https://github-mock.caprun.test`, `--features brokerd/mock-egress-ca`, leading `cargo build --workspace`, libssl-dev/pkg-config install, true-exit-before-pipe, trap-cleanup both sidecars.

## Task Commits
1. **Task 1: mock server + matched cert fixtures + regenerated DER anchor** — `6646371` (feat)
2. **Task 2: compose-verify.sh composed harness** — `b6f050d` (feat)

## Real Verification Results
- **server.py:** parses under stdlib (`ast.parse`); standalone TLS smoke via `curl --cacert`: `POST /repos/octo/repo/pulls` → **HTTP 201** with `{number, html_url, state, id, title}`; `GET /` → **404**; `POST .../issues` → **404**.
- **Cert integrity:** SAN `github-mock.caprun.test`; PEM cert SHA-256 fingerprint (`6A:39:80:66:...`) == regenerated DER anchor fingerprint; cert modulus == key modulus.
- **compose-verify.sh:** `bash -n` OK, **shellcheck CLEAN**; plan grep gate **PASS** (`203.0.113`, `add-host`, `mock-egress-ca`, `cargo build --workspace`, `set +e`/`rc=$?`, `seccomp=unconfined`, and `! grep privileged`). `api.github.com` appears only in two explanatory comments — never in an `--add-host`.
- **Docker dry-run (Colima, real):** created the `203.0.113.0/24` network, started the mock sidecar at fixed IP `203.0.113.2` (readiness gate passed, `--ip` + `--network-alias` both honored), and a peer `python:3-slim` container reached the mock over TLS via `--add-host github-mock.caprun.test:203.0.113.2` trusting the checked-in test CA → **201 + PR JSON**. Trap teardown left **zero** stray containers/networks.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Regenerated the cert+key+DER as a matched triple (updated the 40-02 DER)**
- **Found during:** Task 1.
- **Issue:** The plan (and 40-02's README) assume 40-03 checks in a `.pem`/`.key` whose cert equals the EXISTING `mock-egress-ca.der`. But 40-02 generated a fresh keypair and checked in ONLY the DER — the matching private key was discarded and exists nowhere in the repo (verified via `git ls-files`). Producing a `.key` that matches the old DER's public key is therefore impossible. Without a key matching the trusted anchor, the mock cannot present a cert the broker validates → the whole composed live proof is unachievable.
- **Fix:** Regenerated a fresh cert+key using 40-02's EXACT openssl command, then re-derived `mock-egress-ca.der` from that SAME cert (`openssl x509 -outform DER`). The checked-in PEM cert, key, and the DER anchor now all share fingerprint `6A:39:...` and the key matches the cert.
- **Files modified:** `crates/brokerd/tests/fixtures/mock-egress-ca.der` (regenerated), plus the new `certs/*`.
- **Why safe:** 40-02's guard tests assert anchor COUNT (`webpki + 1`) and host-allowlist membership — never a specific fingerprint/byte content; its README explicitly states the DER "is not reproducible run-to-run … regenerating produces an equivalent, equally-valid test anchor." Swapping in an equivalent valid DER preserves every 40-02 invariant. `include_bytes!` path is unchanged; Gate 4b (never-default) is unaffected.
- **Commit:** `6646371`.

**Total deviations:** 1 auto-fixed (Rule 3 blocking). No architectural changes; no scope creep beyond the DER regeneration the fix strictly required.

## Known Stubs
None. `server.py` is a deliberate test mock (documented as such in its README and 40-CONTEXT decision 5), not a stub of unfinished product code.

## Threat Flags
None. The harness introduces no new product egress/auth surface: it only orchestrates docker sidecars for the Linux verification run. The mock cert/host reach the broker ONLY under the non-default `mock-egress-ca` feature (40-02); every release build is webpki-roots-only. `api.github.com` is never remapped (T-40-06). The run adds no elevated-privilege docker flag (T-40-07). The mock is stdlib-only — no new package-manager dependency (T-40-SC).

## What I Did NOT Do (per plan boundary)
- Did NOT write the composed acceptance TEST — that is Plan 40-04. This plan delivers only the harness + mock so 40-04's test can run.
- Did NOT edit `scripts/mailpit-verify.sh` (compose-verify.sh is a sibling).
- Did NOT touch `.planning/ROADMAP.md` or `.planning/STATE.md`.

## Next Phase Readiness
- 40-04 can author the Linux-gated composed acceptance test and run it under `bash scripts/compose-verify.sh` (or scope it via `COMPOSE_VERIFY_CMD='... cargo test -p caprun --test <name> --features brokerd/mock-egress-ca'`).
- The full live composed run is exercised only on real Linux (the mock TLS reach was dry-run-verified from a docker peer; the broker-side rustls validation of the mock is exercised by 40-04's test under the feature, and by the orchestrator's closing gate).

## Self-Check: PASSED
All created/modified files present on disk; both task commits (`6646371`, `b6f050d`) exist in git.
