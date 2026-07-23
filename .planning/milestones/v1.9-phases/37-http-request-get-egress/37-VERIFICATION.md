---
phase: 37-http-request-get-egress
verified: 2026-07-18T08:00:00Z
status: passed
score: 4/4 success criteria verified (host-portable + fresh adversarial trace); live-HTTPS behavior deferred to Phase 40 (LIVE/ENV-01)
behavior_unverified: 1
verified_by: orchestrator (autonomous) — plan-checker PASS + fresh non-self Fable-5 adversarial code-trace (core mint mechanism CLEAN) + 6 finding-fixes applied & re-verified + Linux compile-check exit 0
gaps: []
---

# Phase 37 Verification — `http.request` GET Egress (HTTP-01/02/03)

## Success criteria (goal-backward)
1. ✅ **Allowlisted read-only GET, Observe, url I2-gated, non-allowlisted → Deny.** Verified:
   executor rows (Observe class, url routing+content-sensitive) + host allowlist + validate_url;
   23 http_request + 11 ssrf host-portable tests pass; a tainted url Blocks (executor collects
   I2 Block before the Observe fall-through — confirmed in the adversarial trace).
2. ✅ **Response minted untrusted via new `mint_from_http` rooted on a real `http_response_received`
   Event; session demotes to draft-only (I1).** Verified: adversarial trace confirmed the
   event-first→mint→atomic-demote ordering matches mint_from_read exactly (provenance_chain[0]==event id),
   demotion reaches BOTH the durable ledger AND the in-memory Arc<Mutex<SessionStatus>> cell (both call
   sites), verify_chain intact. Host-portable anti-staple + demotion integration tests pass.
3. ✅ **Fetched value → sensitive sink arg Blocks on a genuinely-propagated (non-stapled) chain.**
   Verified: the s37 anti-staple test proves a fetched value routed into git.commit `message`
   BlockedPendingConfirmation on a DAG-rooted chain (§9 genuineness).
4. ✅ **SSRF resolve-and-pin: loopback/RFC1918/link-local/metadata/userinfo@/redirects denied.**
   Verified: pure host-portable ssrf_check classifier + validate_url; resolve-and-pin connects to the
   SAME checked IP (TOCTOU closed — adversarial trace confirmed); redirects off; port pinned to 443
   (fix). Extended ranges (NAT64/6to4/Teredo/v4-compat + 224/4/240/4/198.18/192.0.0) added (fix).

## Adversarial gate (orchestrator-owned fresh Fable-5 trace on the diff)
Core mint mechanism traced CLEAN (non-stapling, dual-path demotion, TOCTOU, Gate-3, 3-tuple signal).
Found + FIXED (commits ba9c398/edf28f0/08f30c4/e493ddd/1d3a49b/263fe77):
- **MAJOR-1** aws-lc-rs C crypto in the workspace graph (resolver-3 feature unification via caprun-planner's
  reqwest) → both reqwest users switched to rustls-no-provider + ring + webpki-roots; new check-invariants
  **Gate 5** asserts `cargo tree --workspace -i aws-lc-rs` finds nothing (verified absent; ring wired for both).
- MINOR: URL port pin (443 only); connect/total timeout + fail-closed body byte cap; `http_request_failed`
  terminal audit event on a failed GET (opaque payload, P34-style); extended SSRF ranges; NIT comment fixes.

## Deferred to Phase 40 (correct scope)
- Live-HTTPS real GET→mint→demote on Linux + `env_clear()` webpki-roots hermetic-cert validation (ENV-01) —
  needs a real HTTPS run; the only place the TLS-env regression manifests. All Phase-37 mechanism verification
  is host-portable + Linux-compile-clean (exit 0).

## Invariants
No raw EffectRequest; sole mint site is mint_from_http (Gate 3 green); sink sensitivity hardcoded;
reqwest/ring/webpki-roots broker-side (kernel net-deny is the worker boundary); check-invariants exit 0 (5 gates).
