---
phase: 40-cli-compose-env-clear-live-proof
verified: 2026-07-18T11:30:00Z
status: passed
score: 4/4 success criteria verified on real Linux (executor run + orchestrator independent re-run + closing adversarial APPROVE)
behavior_unverified: 0
verified_by: orchestrator (autonomous) — independent compose-verify.sh re-runs (TRUE_RC=0 captured before pipe) + closing fresh Fable-5 adversarial code-trace (VERDICT APPROVE)
gaps: []
---

# Phase 40 Verification — CLI Compose, Sidecar env_clear() & Composed Live Proof (v1.8 DONE)

## Success criteria (goal-backward), all TRUE on real Linux
1. ✅ **ENV-01 — sidecar env_clear hermetic.** `caprun-planner` spawn `env_clear()`'d + minimal allowlist
   (OPENAI_API_KEY explicit; PLANNER_SOCK + minimal PATH always; CAPRUN_PLANNER_MODEL/HTTPS_PROXY/NO_PROXY
   only-when-present). CAPRUN_SMTP_*/CAPRUN_GITHUB_TOKEN cannot survive (`never_leaks_smtp_or_secret`).
   Proven hermetic live: a post-env_clear GET to real https://api.github.com with SSL_CERT_* removed minted
   HttpRaw (webpki-roots compiled-in). **The live-OpenAI sidecar leg RAN and PASSED** (OPENAI_API_KEY present) —
   not structurally-only. Dead worker-spawn SESSION_ID entry dropped.
2. ✅ **LIVE-03 composed workflow.** `test linux::live_acceptance_v1_8_composed_all_legs ... ok`: process.exec →
   file.write edit → git.commit (real confined commit) → github.pr POST to the mock (201; bearer token absent
   from all payloads) → http.request GET — every step gated/tainted/audit-DAG-chained, per-session verify_chain
   true over one shared audit.db. (git.push deferred to v1.9; the mock accepts the PR head — see
   DECISION-git-push-deferral-v1.8.md.)
3. ✅ **LIVE-04 adversarial legs, each deterministically Blocked, verify_chain true:** tainted github.pr
   title/body Blocked (CONTENT-01); tainted http.request url Blocked + SSRF ranges/non-allowlisted host denied
   at the pin layer before any socket; tainted git.commit message Blocked — anchors rooted on the real
   http_response_received id (non-stapled). Post-env_clear live HTTPS call succeeds.
4. ✅ **Regression.** Full-workspace green on real Linux, no v1.0–v1.7 regression.

## Gates (independent, per coordinator-gate discipline)
- **Executor compose-verify.sh run:** TRUE_RC=0, 498 passed / 0 failed / 60 binaries; named composed test ok;
  2 harness/cert bugs found + fixed (mock cert needed CA:FALSE+serverAuth EKU for rustls; Mailpit IP collision)
  — sink + test code unchanged.
- **Orchestrator INDEPENDENT re-runs (true-exit-before-pipe):** (a) full default workspace via compose-verify —
  TRUE_RC=0, 0 failed; (b) feature-ON composed suite — TRUE_RC=0, `live_acceptance_v1_8_composed_all_legs ... ok`,
  all prior composed tests (v1.3/v1.4/v1.7) still ok, 0 FAILED/panicked anywhere, harness "Composed Linux
  verification suite PASSED".
- **Closing fresh non-self Fable-5 adversarial code-trace: VERDICT APPROVE** — the release/default build's egress
  trust is provably UNCHANGED (webpki-roots-only + allowlist api.github.com-only): every `mock-egress-ca`
  touchpoint cfg-gated, no transitive enablement (cargo metadata), Gate 4b + host-portable guards lock it, the
  mock rides the UNMODIFIED egress path (ssrf_check/validate_url/resolve-and-pin byte-identical), env_clear
  strictly narrows. 4 non-blocking LOW/INFO items noted for v1.9 hygiene.
- **check-invariants.sh exit 0** (Gate 4b: mock-egress-ca can never be a default feature).

## Honesty note (milestone record)
v1.8 proves the Safe Coding Agent for edit → commit → open-PR (mock GitHub endpoint) + an authorized HTTP fetch.
The real `git.push` step is DEFERRED to v1.9 (git.push's fully-unprivileged destination-pinned egress needs its
own design-gate — DESIGN BLOCKER-1). The mock stands in for the pushed-branch precondition.
