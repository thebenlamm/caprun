---
phase: 46
status: passed
verified_by: orchestrator
date: 2026-07-18
---

# Phase 46 Verification — Composed Live Proof (LIVE-05, LIVE-06) — the v1.9 DONE gate

**Verdict: PASSED — the v1.9 DONE gate is MET.** Proven green on real Linux via an INDEPENDENT
orchestrator re-run of the DEFAULT `compose-verify.sh` recipe (full workspace + `brokerd/mock-egress-ca`
+ mock GitHub/git-receive-pack/`/ingest` + Mailpit): **COMPOSE_VERIFY_EXIT=0, 696 passed / 0 failed**,
"Composed Linux verification suite PASSED". The fresh non-self Fable-5 adversarial trace returned
**APPROVE — the acceptance proof is GENUINE** (real broker arms, non-stapled taint, non-vacuous
credential-absence, distinct policy-vs-I2 attribution, honest framing), conditional ONLY on the
LIVE-05 success test actually executing — which this authoritative run confirms it did.

## Goal-backward check

Phase 46's goal: the full authorized-write loop runs and is inspected on real Linux, with every
adversarial/negative leg independently attributable — the v1.9 DONE gate.

| Requirement | Evidence | Status |
|-------------|----------|--------|
| LIVE-05 (composed success, driven + inspected via CLI/viewer) | `cli/caprun/tests/live_acceptance_v1_9_composed.rs` chains process.exec → filesystem edit → git.commit → git.push(confirm-release) → github.pr → http.request.write POST over ONE shared persisted `audit.db`, each leg its own session, through the REAL broker arms (`evaluate_plan_node_and_record_for_test` — a verbatim delegate to the live `evaluate_plan_node_and_record` — + `confirmation::confirm`). Taint is GENUINE, not stapled (`provenance_chain[0]` == the real `process_exited`/`http_response_received` event id). git.push's always-confirm-gate is correctly composed (Block-pending-confirmation → confirm → dispatch → `git_push_succeeded`). INSPECTED via a genuine `caprun audit` subprocess per session (`Chain verification: PASSED`) + a genuine `caprun run` I2-Block leg in the shared db; the final exact-set sweep asserts the 7-session set, all `verify_chain` true. **RAN + PASSED on real Linux: `linux::live_acceptance_v1_9_composed_success_chain ... ok`** (the http POST leg genuinely delivers to the mock `/ingest`→201 → `http_write_succeeded`). | ✅ Complete |
| LIVE-06 (5 independently-attributable negative legs + regression) | `crates/brokerd/tests/s46_negative_legs_composed.rs`: (1) tainted push remote/refspec → I2 `sink_blocked`; (2) tainted POST body → I2 `sink_blocked`; (3) POLICY-DENY → `DenyReason::code()=="policy_deny"` (generic `plan_node_evaluated`, zero `sink_blocked`) WHILE legs 1&2 run `broker_default()` which EXPLICITLY PERMITS git.push + http.request.write — the two tags asserted SEPARATELY, side-by-side, so policy-deny and I2-Block are provably distinct mechanisms; (4) DESTINATION-PIN negative → a `/redirect/*` push's info/refs 302 is refused by the redirect-none pinned client → `ConfirmedButSinkFailed` + one `git_push_failed`, zero succeeded (proves the pin HOLDS, downstream of confirm); (5) CREDENTIAL-ABSENCE split non-vacuously — 5a (value store + audit chain) on the clean 200 push; **5b (broker LOG) on the ERROR-PATH push** (redirect-refused, where `scrub_secrets`→`eprintln!` actually fires), captured via a re-exec'd `--nocapture` subprocess dup2'ing real FD 2, asserting the `"git.push failed"` marker IS present (logger ran) AND the token/remote/URL are ABSENT. **RAN + PASSED: `s46_negative_legs_composed_all_legs ... ok` + `leg5b_error_path_push_worker ... ok`.** Full-workspace regression green, zero v1.0–v1.8 regression. | ✅ Complete |

## Hard-constraint checks

- **No new `EffectRequest`** (Gate 1), **no new mint site** (Gate 3 — test/harness only), **no new
  crate** (Gate 5 — `libc` is a pre-existing workspace dev-dep, not C-crypto; HYG-01 aws-lc-rs /
  openssl-sys absence holds), Gate 4/4b (`test-fixtures`/`mock-egress-ca` never default).
  `check-invariants.sh` all gates PASS. ✅
- **Genuine, not stubbed** (the DONE-gate #1 concern): decisions asserted from the real arm's return
  value; terminals counted from the durable DB, never hand-set; taint roots on real events;
  `verify_chain` against the real keyed HMAC chain (32-byte key persisted before any append; the
  viewer uses load-only fail-closed key custody). ✅
- **Framing honesty (v1.3 DOC-01 discipline):** the test module doc + `46-MILESTONE-RECORD.md` state
  bluntly what is `caprun run`-driven (exactly ONE confined Block leg) vs composed-in-crate-through
  -the-real-arms vs `caprun audit`-inspected — explicitly NOT claiming `caprun run` drove the whole
  chain; machine-checked by a grep with a negative overclaim guard. ✅
- **Linux (authoritative, independent orchestrator re-run):** default `compose-verify.sh`,
  **696 passed / 0 failed, exit 0**, LIVE-05 success chain + all 5 LIVE-06 legs RAN and PASSED, all
  prior composed proofs (v1.3/v1.4/v1.7/v1.8) green. ✅

## Adversarial code-trace (standing v1.9 per-phase discipline)

A fresh non-self, orchestrator-owned Fable-5 trace of the full Phase-46 diff (`d0cc7b8..HEAD`, ~2,150
lines) traced all 7 self-fooling vectors against the actual test code + the real broker arms and
returned **APPROVE — the proof is genuine**: no stubbing (real delegate arm, DB-counted terminals,
non-stapled taint); leg-5b non-vacuous (the W1 fix held — error-path push, FD-2 capture, requires the
failure marker before asserting absence); no attribution collapse (legs 1&2 under a permitting policy
resolve to `BlockedPendingConfirmation`; leg 3 to `PolicyDeny`; tags asserted separately); honest
framing; real `verify_chain`; sound `/ingest` mock (201 → genuine `http_write_succeeded`); the pin
leg proves refusal. The ONE load-bearing condition — "the LIVE-05 success test had only been
compile-verified (`--no-run`); do not close until it actually runs" — is **CLOSED by this
authoritative run** (`live_acceptance_v1_9_composed_success_chain ... ok`). Risk direction was safe
(all-positive assertions → loud FAIL, never false PASS).

**Non-blocking findings (recorded, not acted on — optional hardening):**
- **MINOR** — leg-5b's 302-refusal error string carries no URL/host/token even unscrubbed, so it
  exercises the log-absence assertion against the real leak vector (all broker stderr in the window)
  but not `scrub_secrets`' replacement branch end-to-end (a transport-level reqwest error, which
  embeds the URL, would). The assertion is still falsifiable; `scrub_secrets` is unit-covered with
  secret-bearing strings. Optional future hardening: a second error-path push failing at the
  receive-pack POST transport level. Filed as a deferred hardening item.
- **NIT** — the framing-honesty grep rejects only the verbatim overclaim literal (a reworded
  overclaim wouldn't trip it); the actual text is honest. **NIT** — the success test's push-leg
  `pending_confirmations` SELECT is unqualified (correct today; the s46 twin session-scopes it).

## Notes

- **git.push safety-valve NOT triggered:** git.push SHIPPED in Phase 44, so the git.push leg is IN
  the composed proof — no auto-descope, no disclosed-gap sign-off needed for a deferral (there was
  none).
- **10MB pack-cap deferred item:** the composed proof pushes a small one-commit mock repo, so the
  cap is non-blocking here; it remains a recorded deferred item to revisit for large-repo pushes.
- **Human milestone-close sign-off** (`46-MILESTONE-RECORD.md` §8) remains the user's at
  `/gsd-complete-milestone` — this VERIFICATION records that the DONE-gate EVIDENCE is complete and
  authoritatively re-verified; it does not itself constitute the human sign-off.
