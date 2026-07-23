---
phase: 42
status: passed
verified_by: orchestrator
date: 2026-07-18
---

# Phase 42 Verification — Policy Layer: Binding, Enforcement & the I2 Boundary

**Verdict: PASSED.** Proven green on real Linux (compose-verify, 535 passed / 0 failed).

## Goal-backward check

Phase 42's goal: a minimal per-session policy that narrows which sinks/args are callable, is bound
from a trusted source outside the confined worker's reach, and can NEVER override I2.

| Requirement | Evidence | Status |
|-------------|----------|--------|
| POLICY-01 | `SessionPolicy` hardcoded-schema type (runtime-core), deny-by-default; `DenyReason::PolicyDeny` distinct machine-checkable outcome; deny-only `policy_gate` early return in `submit_plan_node` (Step 0.25, before the I2 loop). Tests: `broker_default_permits_the_seven_production_sinks_and_denies_unlisted`, `coarse_arg_constraint_permits_matching_and_denies_non_matching`, `policy_deny_code_and_display`. | ✅ Complete |
| POLICY-02 | Holds **by construction** — `policy_gate() -> Result<(),DenyReason>` has no permit-carrying return; the I2 collect-then-Block loop + CommitIrreversible class gate are byte-unchanged and reference `policy` nowhere. Adversarial trace confirmed no Allow-and-skip path. Enforcement-order proof test present (permissive policy + tainted arg → `BlockedPendingConfirmation`). | ✅ Complete |
| POLICY-03 | `bind_policy` binds at session creation from `CAPRUN_POLICY` (unconfined parent, before worker spawn), refuses any path at-or-beneath the workspace root via the SHARED `adapter_fs::containment::refuse_if_beneath_workspace` (MAJOR-2 fix: extracted, both key-custody + binder call it, anti-drift Gate 6 enforces). Captured by value (immutable — negative test proves a mid-session worker rewrite has zero effect). `policy_bound` genuinely hash-chained after `session_created` (`verify_chain` true). `CAPRUN_POLICY` stripped from worker + planner-sidecar + exec-child spawns. | ✅ Complete |

## Hard-constraint checks

- **No raw EffectRequest** (Gate 1), **DenyReason exhaustive** (no wildcard, owned fields), **anti-drift Gate 6** (self-tested against false-PASS). check-invariants exit 0. ✅
- **Linux (authoritative):** compose-verify.sh (full workspace + `brokerd/mock-egress-ca` + mock GitHub) — **COMPOSE_EXIT=0, 535 passed / 0 failed**, no v1.0–v1.8 regression. The two blast-radius casualties now pass: `dag_chain_integrity ... ok` (e2e updated for the new `policy_bound` event), `live_acceptance_v1_8_composed_all_legs ... ok`. ✅

## Adversarial code-trace (standing v1.9 per-phase discipline)

A fresh non-self Fable-5 code-trace of the full Phase-42 TCB diff verified POLICY-02/POLICY-03 SOUND
against live code, and surfaced 1 MAJOR + 1 MINOR (both fixed, Round-1, orchestrator-re-verified):
- **MAJOR** — `ArgConstraint::permits` bare `starts_with` was bypassable (`api.example.com` permitted `api.example.com.evil.com`; `/ws/out/` permitted `/ws/out/../..`). Fixed to boundary-and-traversal-safe matching (pure, no new crate), + regression tests (`host_suffix_bypass_is_denied`, `path_traversal_escape_is_denied`) that fail pre-fix and pass on Linux.
- **MINOR** — Gate 6 anti-drift evadable by a method-form re-inline / a third site; hardened to a dynamic site list + broadened token, re-tested against its own adversarial negatives.
- The Linux gate independently caught the `policy_bound` event-count blast radius (cfg-linux-test-blindness) — e2e.rs updated, all other CLI tests confirmed robust (verify_chain / event-type-scoped / dynamic-chain-head).

Full catalogue: `42-ADVERSARIAL-REVIEW.md`.

## Notes

The load-bearing milestone invariant (policy narrows, never overrides I2) is enforced by construction
and proven live. Commits: 42-01 (`3828511`,`9ba3d47`,`23ce3df`), 42-02 (`efa9ea6`,`ad45401`,`680b465`,`a71abf5`),
42-03 (`bd6cb39`,`bbdad02`,`3c3037c`,`f431519`), 42-04 (`da27b66`,`cd1ccae`,`7bb891c`), 42-05 hardening
(`1a56deb`,`ae21e85`,`28f3a48`,`74bdd00`).
