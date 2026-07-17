---
phase: 30-regression-live-proof
plan: 02
status: complete
requirements: [HARDEN-06]
---

# Plan 30-02 SUMMARY — Live Proof + Evidence

## What was done

Executed the HARDEN-06 milestone-close live proof and compiled `30-VERIFICATION.md`. Per the
plan's role split, Task 2's real-Linux gates were **run by the orchestrator directly**
(non-laundered, true exit captured before any pipe); the executor role was performed inline for
the doc scaffold + the cross-platform criterion-2 unit tier, since 30-02 is a doc-only checkpoint
plan whose Linux proof is intrinsically the orchestrator's.

## Evidence (all real Linux, Colima kernel 6.8.0)

- **Gate A — bare `scripts/mailpit-verify.sh`**: true exit 0, **331 passed / 0 failed across 49 suites**. Fresh v1.6 regression baseline (supersedes stale 309/0). Covers criteria 1, 2, 3, 5.
- **Gate B — `scripts/verify-harden04-featureless.sh`**: true exit 0. Criterion-4 assertion `featureless_create_session_denied_even_with_flag_set` ACTUALLY EXECUTED (no self-skip) and passed; script's false-assurance guard confirmed the skip-sentinel did not fire.
- **Criterion-2 host tier**: `cargo test -p brokerd --lib -- self_consistent_forgery_without_key_is_rejected tail_truncation_detected_via_anchor_mismatch legacy_db_without_anchor_fails_closed` → 3 passed / 0 failed, exit 0.

## Per-criterion verdict: all 5 GREEN

1. Full regression fresh baseline (331/0) ✅
2. Forged/truncated/keyless chain rejected by `verify_chain` ✅
3. Replayed Allowed `email.send` delivers exactly once ✅
4. Forced-Active `CreateSession` absent from featureless build (self-skip closed) ✅
5. `RequestFd` demotes + CONTROL-01 clean path succeeds (dual-cited) ✅

## Deviations

- **Plan crit-2 command corrected**: the plan listed 3 test filters as bare positionals; `cargo test` needs a `--` separator for multi-filter (libtest OR). Ran the corrected form. Non-blocking; noted in 30-VERIFICATION.md.

## Key files

- `.planning/phases/30-regression-live-proof/30-VERIFICATION.md` (created — the 5-criterion evidence table, fresh baseline, orchestrator DONE sign-off; `status: passed`).

## Self-Check: PASSED

All 5 HARDEN-06 criteria proven green on real Linux with true-exit-before-pipe + named-test-count discipline; sign-off recorded before the plan flips complete.
