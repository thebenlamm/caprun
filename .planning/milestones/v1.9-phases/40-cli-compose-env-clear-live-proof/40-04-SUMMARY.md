---
phase: 40-cli-compose-env-clear-live-proof
plan: 04
subsystem: testing
tags: [live-acceptance, composed, github.pr, git.commit, http.request, env-clear, rustls, webpki-roots, mock-egress-ca, ssrf, taint, audit-dag, LIVE-03, LIVE-04, ENV-01]

# Dependency graph
requires:
  - phase: 40-cli-compose-env-clear-live-proof
    provides: "40-01 sidecar env_clear(); 40-02 non-default brokerd/mock-egress-ca feature + egress trust anchor; 40-03 mock GitHub HTTPS server + compose-verify.sh harness"
  - phase: 38-github-pr-sink
    provides: "invoke_github_pr_from_resolved + grant gate + created_prs CAS + opaque audit"
  - phase: 37-http-request-egress
    provides: "invoke_http_get + mint_from_http + validate_url/ssrf_check/resolve-and-pin + ring/webpki-roots TLS"
  - phase: 36-git-commit-sink
    provides: "invoke_git_commit (confined launcher) + mint_from_exec"
provides:
  - "cli/caprun/tests/live_acceptance_v1_8_composed.rs — the v1.8 DONE composed live-acceptance test (Linux-gated, macOS guard)"
  - "Composed proof: exec -> file.write -> git.commit (real confined) -> github.pr (mock 201) -> http GET (real api.github.com), one shared audit.db, per-session verify_chain"
  - "Three adversarial Block/Deny legs (tainted github.pr title, tainted/SSRF http url, tainted git.commit message)"
  - "ENV-01 live-HTTPS proof: post-env_clear GET to real api.github.com with SSL_CERT_* removed mints HttpRaw (webpki-roots hermetic)"
  - "Corrected mock-github TLS cert (CA:FALSE + serverAuth EKU) + Mailpit fixed-IP collision fix in compose-verify.sh"
affects: [v1.8-milestone-close, gsd-audit-milestone, v1.9-git-push]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composed live proof: one shared persisted audit.db, each leg its own session, per-session verify_chain (never :memory:, never LIMIT 1) — extended from the v1.7 shape to the 3 v1.8 sinks + ENV-01"
    - "Genuine (non-stapled) adversarial taint via mint_from_http on a synthetic hostile body, routed into a sensitive sink arg, asserting the anchor's provenance root == the real http_response_received event id (mirror s37)"
    - "Self-signed rustls server leaf that is ALSO its own trust anchor MUST be CA:FALSE + serverAuth EKU"

key-files:
  created:
    - "cli/caprun/tests/live_acceptance_v1_8_composed.rs"
  modified:
    - "scripts/compose-verify.sh (Mailpit fixed IP)"
    - "scripts/mock-github/certs/github-mock.caprun.test.{pem,key} (CA:FALSE + serverAuth EKU regen)"
    - "crates/brokerd/tests/fixtures/mock-egress-ca.der (matched DER anchor)"
    - "scripts/mock-github/README.md, crates/brokerd/tests/fixtures/README-mock-egress-ca.md (corrected recipe + rationale)"

key-decisions:
  - "One big #[tokio::test] fn with all 8 legs (5 success + 3 adversarial), single-threaded, so process-global env mutation (CAPRUN_GITHUB_*, SSL_CERT_* removal) and the shared audit.db path are race-free (mirror v1.7 single-fn shape)"
  - "Adversarial taint minted via mint_from_http (synthetic body), submit_plan_node passed Active to isolate the Block as I2 taint-driven (not draft-only I1) — mirror s37"
  - "The live-OpenAI sidecar leg is NOT asserted inside this test; it is the harness's llm_planner_clean_allow_delivers run, which DID run with a real key in the DONE-gate execution"

patterns-established:
  - "Composed v1.8 live proof mirrors v1.7: shared audit.db + per-session verify_chain sweep enumerated ORDER BY rowid"
  - "rustls-webpki self-signed-leaf-as-anchor cert discipline: CA:FALSE + serverAuth EKU (openssl s_client does NOT catch either omission)"

requirements-completed: [LIVE-03, LIVE-04, ENV-01]

coverage:
  - id: D1
    description: "Composed SUCCESS workflow (exec -> file.write -> git.commit real confined -> github.pr mock 201 -> http GET real api.github.com), every step gated/tainted/audit-DAG-chained, per-session verify_chain true over one shared audit.db (LIVE-03)"
    requirement: "LIVE-03"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/live_acceptance_v1_8_composed.rs#linux::live_acceptance_v1_8_composed_all_legs (via scripts/compose-verify.sh)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Three adversarial legs each deterministically Blocked/Denied with genuine non-stapled taint and verify_chain true: tainted github.pr title, tainted http.request url + SSRF/non-allowlisted pin-layer Deny, tainted git.commit message (LIVE-04)"
    requirement: "LIVE-04"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/live_acceptance_v1_8_composed.rs#linux::live_acceptance_v1_8_composed_all_legs (via scripts/compose-verify.sh)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Post-env_clear live HTTPS GET to real api.github.com succeeds with SSL_CERT_* removed and mints HttpRaw (webpki-roots hermetic) — the observable ENV-01 proof; the live-OpenAI sidecar leg (llm_planner_clean_allow_delivers) ran with a real key in this run and passed"
    requirement: "ENV-01"
    verification:
      - kind: e2e
        ref: "live_acceptance_v1_8_composed http GET leg + cli/caprun/tests/llm_planner_live_accept.rs#llm_planner_clean_allow_delivers (OPENAI_API_KEY present)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Full-workspace regression green on real Linux with no v1.0-v1.7 regression"
    verification:
      - kind: e2e
        ref: "bash scripts/compose-verify.sh (cargo test --workspace --no-fail-fast --features brokerd/mock-egress-ca) -> 498 passed / 0 failed, TRUE_RC=0"
        status: pass
    human_judgment: false

# Metrics
duration: ~75min
completed: 2026-07-18
status: complete
---

# Phase 40 Plan 04: Composed Live Proof (v1.8 DONE) Summary

**The v1.8 DONE gate passes on real Linux: a composed exec -> file.write -> git.commit (real confined) -> github.pr (mock 201) -> http GET (real api.github.com) workflow, three deterministic adversarial Blocks/Denies, and the ENV-01 hermetic-env_clear live-HTTPS proof — full workspace 498 passed / 0 failed, TRUE_RC=0.**

## Performance

- **Duration:** ~75 min (incl. multiple full Linux container verification cycles)
- **Completed:** 2026-07-18
- **Tasks:** 2 (both TDD-style test authoring)
- **Files modified:** 6 (1 created, 5 modified for the deviation fixes)

## Accomplishments
- Authored `cli/caprun/tests/live_acceptance_v1_8_composed.rs` — the v1.8 milestone's FINAL HARD GATE, mirroring the v1.7 composed shape (one shared persisted audit.db, per-session `verify_chain`, genuine non-stapled taint) extended to `git.commit`, `github.pr`, `http.request`.
- **LIVE-03 success (5 legs):** trusted `process.exec` (output minted, anti-staple DB re-check) → trusted `file.write` overwrite → trusted `git.commit` making a REAL commit via the confined launcher → `github.pr` live POST to the mock returning **201** with the bearer token absent from every audit payload → live GET to real `api.github.com`.
- **LIVE-04 adversarial (3 legs):** genuinely-tainted (http-response-rooted) value routed into `github.pr` title, `http.request` url, and `git.commit` message each `BlockedPendingConfirmation` with the anchor's provenance root == the real `http_response_received` id; plus a non-allowlisted host + cloud-metadata/RFC1918 IPs Denied at the pin layer (`allowlist gate` / `ssrf_check`) before any socket. No sink effect on any blocked leg.
- **ENV-01:** the post-env_clear GET to real `api.github.com` succeeds with `SSL_CERT_FILE`/`SSL_CERT_DIR` explicitly removed and mints `[ExternalUntrusted, HttpRaw]` — proving `env_clear()` + `webpki-roots` is hermetic for TLS.
- Ran the DONE gate on real Linux via `bash scripts/compose-verify.sh`: **TRUE_RC=0**, `live_acceptance_v1_8_composed_all_legs ... ok`, full workspace **498 passed / 0 failed**, no v1.0–v1.7 regression.

## Task Commits

1. **Task 1: Composed SUCCESS workflow + ENV-01 live-HTTPS proof** — `1ecb612` (test)
2. **Task 2: Three adversarial Block/Deny legs** — `16ee3a0` (test)
3. **Deviation fix: mock-cert rustls + Mailpit IP** — `5041b97` (fix)

## Files Created/Modified
- `cli/caprun/tests/live_acceptance_v1_8_composed.rs` — the Linux-gated composed acceptance test (8 legs in one tokio test) + macOS guard.
- `scripts/compose-verify.sh` — Mailpit pinned to a fixed IP (203.0.113.3) so it can't grab the mock's .2.
- `scripts/mock-github/certs/github-mock.caprun.test.{pem,key}` — regenerated as `CA:FALSE` + `keyUsage=digitalSignature` + `extendedKeyUsage=serverAuth`.
- `crates/brokerd/tests/fixtures/mock-egress-ca.der` — DER anchor re-derived to match the regenerated cert.
- `scripts/mock-github/README.md`, `crates/brokerd/tests/fixtures/README-mock-egress-ca.md` — corrected cert recipe + rationale.

## Decisions Made
- Single `#[tokio::test]` fn holding all 8 legs, run serially, so process-global env mutation and the shared `audit.db` path are race-free (mirrors the v1.7 single-fn composed shape).
- Adversarial taint minted via `mint_from_http` on a synthetic hostile body (genuine `http_response_received`-rooted, non-stapled), with `submit_plan_node` passed `Active` to isolate the Block as I2 taint-driven rather than a draft-only I1 gate (mirror `s37_http_request.rs`).
- No `git.push` leg (deferred to v1.9 per `DECISION-git-push-deferral-v1.8.md`); the mock's 201 stands in for the pushed-branch precondition. The test's module doc states this plainly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Mock-github TLS cert rejected by rustls (`CaUsedAsEndEntity` then `RequiredEkuNotFound`)**
- **Found during:** Task 1/2 DONE-gate run — the `github.pr` live POST failed with an opaque `error sending request`.
- **Issue:** The 40-02/40-03 mock cert was `CA:TRUE` with no EKU. rustls-webpki rejects such a cert when presented as a TLS server leaf (an end-entity must not be a CA, and must assert `serverAuth`). `openssl s_client` verified OK (it enforces neither), masking the defect — the prompt flagged this "no-EKU" leg to watch. Diagnosis required temporarily instrumenting the sink to walk the full `reqwest` error source chain (the sink flattens it via `{e}` for opaque audit), which surfaced `invalid peer certificate: Other(OtherError(CaUsedAsEndEntity))`.
- **Fix:** Regenerated the self-signed cert as `CA:FALSE` + `keyUsage=digitalSignature` + `extendedKeyUsage=serverAuth` (still valid as its own rustls trust anchor — `RootCertStore::add` trusts an explicitly-provided cert regardless of the CA bit), re-derived the matching DER anchor (fingerprints verified equal), and corrected both cert READMEs. Temp instrumentation fully reverted (zero diff on the sink + test vs their task commits).
- **Files modified:** `scripts/mock-github/certs/*`, `crates/brokerd/tests/fixtures/mock-egress-ca.der`, both READMEs.
- **Verification:** `bash scripts/compose-verify.sh` → github.pr POST returns 201, `github_pr_succeeded` durable, token absent from payloads; TRUE_RC=0.
- **Committed in:** `5041b97`.

**2. [Rule 3 - Blocking] Mailpit / mock-github fixed-IP collision in compose-verify.sh**
- **Found during:** first DONE-gate run — the mock GitHub sidecar failed to start (`Address already in use`).
- **Issue:** `compose-verify.sh` started Mailpit with no `--ip`, so docker's IPAM assigned it the lowest free address (`.2`) on a freshly-created network — which is the mock's fixed IP — so the mock could not bind. This would also break the orchestrator's closing re-run.
- **Fix:** Pinned Mailpit to a fixed `203.0.113.3` (its address is still resolved dynamically via `docker inspect`), leaving `.2` free for the mock's explicit `--ip`.
- **Files modified:** `scripts/compose-verify.sh`.
- **Verification:** both sidecars come up; `Resolved Mailpit sidecar IP` + mock readiness both succeed; TRUE_RC=0.
- **Committed in:** `5041b97`.

---

**Total deviations:** 2 auto-fixed (both Rule 3 - blocking harness/cert bugs in the 40-02/40-03 artifacts).
**Impact on plan:** Both were prerequisites for the DONE gate to genuinely pass; no scope creep. The test file (40-04's own artifact) executed as written. No `EffectRequest`, `check-invariants.sh` exits 0, `mock-egress-ca` stays non-default.

## Issues Encountered
- The `reqwest` error is intentionally flattened via `{e}` inside the sink (opaque audit), so the real rustls cause was invisible from the test. Resolved by a temporary, then-reverted, source-chain walk in the sink to obtain `CaUsedAsEndEntity` — the definitive diagnosis (rather than continuing to guess at the cert).

## Live-OpenAI SIDECAR leg status (explicit, per plan honesty requirement)
`OPENAI_API_KEY` was **present** in the DONE-gate run env, so the live-OpenAI sidecar leg (`llm_planner_live_accept::llm_planner_clean_allow_delivers`) **RAN and PASSED** — it was NOT skipped and is NOT structurally-verified-only. The env_clear'd sidecar therefore made a real OpenAI call over webpki-roots TLS successfully, corroborating the ENV-01 hermetic-env_clear proof from the live-LLM-sidecar side as well as the broker-side `api.github.com` GET.

## User Setup Required
None - no external service configuration required (the mock GitHub + Mailpit sidecars are stood up by `scripts/compose-verify.sh`; `OPENAI_API_KEY` is optional and only gates the live-LLM leg).

## Next Phase Readiness
- v1.8 DONE gate is genuinely green on real Linux (LIVE-03, LIVE-04, ENV-01 all proven). The milestone is ready for `/gsd-audit-milestone` / close.
- `git.push` and its live/adversarial-push legs remain deferred to v1.9 (per `DECISION-git-push-deferral-v1.8.md`) — the real push path is NOT proven by this phase.

## Self-Check: PASSED
- `cli/caprun/tests/live_acceptance_v1_8_composed.rs` — FOUND
- `.planning/phases/40-cli-compose-env-clear-live-proof/40-04-SUMMARY.md` — FOUND
- Commits `1ecb612`, `16ee3a0`, `5041b97` — all FOUND in git log

---
*Phase: 40-cli-compose-env-clear-live-proof*
*Completed: 2026-07-18*
