---
phase: 38-github-pr-sink
verified: 2026-07-18T09:30:00Z
status: passed
score: 4/4 success criteria verified (plan-checker PASS + fresh non-self adversarial APPROVE + host tests + Linux compile-clean); live GitHub POST behavior deferred to Phase 40 mock
behavior_unverified: 1
verified_by: orchestrator (autonomous) — gsd-plan-checker PASS + fresh Fable-5 adversarial code-trace (VERDICT APPROVE) + finding-#2 fix + Linux compile exit 0
gaps: []
---

# Phase 38 Verification — `github.pr` Sink (GITHUB-01/02/03/04)

## Success criteria (goal-backward)
1. ✅ **PR via broker-held bearer token, token never in worker/planner/ValueNode/audit-literal, CommitIrreversible.**
   Adversarial trace confirmed: `CAPRUN_GITHUB_TOKEN` read ONLY in github_pr.rs, passed only to `.bearer_auth()`;
   opaque audit verified (a test greps appended events for the token + `ghp_` — absent); CommitIrreversible arm.
2. ✅ **Explicit human auth-grant, distinct from confirm; a PR cannot be created on a bare confirm.**
   Two independent gates confirmed at BOTH dispatch loci (server Allowed arm + confirm-release), fail-closed,
   session-scoped (`session_grants` PRIMARY KEY; A cannot authorize B — tested); grant event opaque, replay = no-op.
   NEW real-function integration test (`github_pr_without_grant_denies_via_real_dispatch`) drives the production
   `evaluate_plan_node_and_record` and asserts no-grant → `github_pr_denied`, no CAS row, no POST.
3. ✅ **Tainted title/body deterministically Blocked (CONTENT-01); verbatim provenance shown at confirm.**
   title/body content-sensitive, owner/repo/base/head routing-sensitive; the shown-at-confirm value == the POSTed
   frozen value; `github_pr_tainted_title_blocks_and_shows_verbatim` passes.
4. ✅ **Replay → at-most-one PR (content-derived CAS before the API call).** Key = combined_digest over the 6
   resolved literals (partition-safe, never effect_id/PlanNode-keyed); `reserve_created_pr` committed before the
   socket; replay under a fresh effect_id suppressed (tested).

## Gates run
- **gsd-plan-checker: PASS** (5 plans, credential-boundary checks confirmed against real code) + 1 warning → the
  required 4th regression test `github_pr_confirm_malformed_precheck_does_not_burn` was added (P33/P34 pre-burn leg).
- **Fresh non-self Fable-5 adversarial code-trace: VERDICT APPROVE.** All 8 vectors (token custody, two-gate
  integrity, confirm-release P33/P34, CAS, base-URL SSRF pin, tainted title/body, no-mint, concurrent-wave
  consistency) traced clean. 3 non-blocking observations:
  - #2 (MINOR, test used a mirror not the real arm) → FIXED (real-function test added, commit 59e7939).
  - #1 (NIT, confirm-release reserve+event non-atomic) → accepted residual: security-equivalent under the
    documented MAJOR-5 crash-window (at-most-once; lost PR never a second PR), identical exposure to the shipped
    process.exec confirm arm. Do NOT add a clear-key-on-failure path.
  - #3 (NIT, `caprun grant` accepts operator input) → correct by design (the deliberate human credential grant).
- **check-invariants exit 0** (5 gates); github.pr does NOT mint (Gate 3 byte-identical); no raw EffectRequest.
- **Linux compile-check exit 0.** brokerd 178 unit + s38 3/3 + all binaries green (host).

## Deferred to Phase 40 (correct scope)
- Live GitHub POST behavioral proof (grant→confirm→real POST→opaque success event) via a MOCK GitHub endpoint
  (base overridable via CAPRUN_GITHUB_API_BASE, still riding validate_url/allowlist/resolve-and-pin). No real
  GitHub creds/repo needed. This drives the server Allowed arm's POST/CAS/success leg end-to-end.
