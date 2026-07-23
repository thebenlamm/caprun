---
phase: 30-regression-live-proof
status: passed
requirements: [HARDEN-06]
verified: 2026-07-17
verifier: orchestrator (non-laundered live-Linux gate run)
baseline: "331 passed / 0 failed across 49 suites (real Linux, Colima kernel 6.8.0)"
---

# Phase 30 — Regression & Live Proof — VERIFICATION

> HARDEN-06 milestone-close proof. All 5 ROADMAP success criteria proven green on **real
> Linux** (Colima/Docker, kernel 6.8.0 — Landlock-capable). This is the v1.6 DONE gate.

## Standing disciplines applied (restated)

1. **Capture `$?` before any pipe.** Every gate's TRUE exit was written to a `.exit` file
   immediately after the command, BEFORE any `| grep`/`| tail`. Assertions are on the PASSED
   sentinel + named-test counts — **never** on exit-0-through-a-pipe (documented Phase-15 near-miss).
2. **cfg(linux) test-blindness.** A green macOS `cargo test` proves NOTHING about the Linux-gated
   tests (`replay_cas`, `s9_live_block`, `harden04_featureless_create_session`). Only the bare
   recipe on real Linux is authoritative. macOS was used ONLY for the genuinely cross-platform
   criterion-2 unit tier.

## Per-criterion evidence

| # | Criterion | Proving file:test | Automated command | Result (real Linux) |
|---|-----------|-------------------|-------------------|---------------------|
| 1 | Full workspace regression green on real Linux via the BARE recipe; no regression to v1.1–v1.5; fresh baseline | whole suite (49 targets) | `bash scripts/mailpit-verify.sh` | ✅ **true exit 0; 331 passed / 0 failed across 49 suites** — fresh v1.6 baseline (supersedes stale 309/0 @ 46 suites) |
| 2 | Forged/tampered audit chain rejected by `verify_chain` (forge AND truncation AND keyless-legacy) | `crates/brokerd/src/audit.rs`: `self_consistent_forgery_without_key_is_rejected`, `tail_truncation_detected_via_anchor_mismatch`, `legacy_db_without_anchor_fails_closed` | host: `cargo test -p brokerd --lib -- <3 names>` + under bare recipe | ✅ **3 passed / 0 failed** (host, true exit 0) AND all 3 `... ok` under the bare Linux recipe |
| 3 | Replayed Allowed `email.send` delivers exactly once | `crates/brokerd/tests/replay_cas.rs::allowed_email_send_replay_delivers_once` (`#![cfg(target_os="linux")]`) | under bare `scripts/mailpit-verify.sh` (Linux-gated) | ✅ `allowed_email_send_replay_delivers_once ... ok` on real Linux |
| 4 | Forced-Active `CreateSession` arm absent from the featureless production build (compile-exclusion, not a runtime flag) | `cli/caprun/tests/harden04_featureless_create_session.rs::featureless_create_session_denied_even_with_flag_set` | `bash scripts/verify-harden04-featureless.sh` | ✅ **true exit 0; assertion ACTUALLY EXECUTED (no self-skip) and passed** — script's own false-assurance guard confirmed the skip-sentinel did NOT fire; `featureless_create_session_denied_even_with_flag_set ... ok` |
| 5 | `RequestFd` demotes the session; CONTROL-01 clean path still succeeds | `crates/brokerd/tests/harden01_session_integrity.rs::fd_grant_on_untrusted_path_demotes_without_report_claims` + `::fd_grant_on_trusted_path_stays_active` + `cli/caprun/tests/s9_live_block.rs::s9_control_ab_taint_driven` | `cargo test -p brokerd --test harden01_session_integrity` + `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_control_ab_taint_driven' bash scripts/mailpit-verify.sh` | ✅ both fd-grant demotion tests `... ok` AND `s9_control_ab_taint_driven ... ok` on real Linux (dual citation per RESEARCH Pitfall 3 — not called done on one alone) |

## Gate run record (true exits, captured before any pipe)

- **Gate A** (`bash scripts/mailpit-verify.sh`, bare): captured exit **0**; 49 `test result: ok` suites, 331 total passed, 0 FAILED suites, 0 panics/compile errors. Named criteria tests confirmed `... ok` in the log (lines 477/481/483 crit 2; 669 crit 3; 611/612 + 840 crit 5).
- **Gate B** (`bash scripts/verify-harden04-featureless.sh`): captured exit **0**; scoped featureless `--release` build + `-p caprun`-scoped test; the D-10 assertion ran (no self-skip sentinel) and reported `ok`; script emitted its `PASSED — HARDEN-04 criterion 4 proven` sentinel.
- **Criterion-2 host tier**: `cargo test -p brokerd --lib -- <3 names>` exit **0**, `3 passed; 0 failed`.

## Honest framing (no overclaim)

- Criteria **2, 3, 5** were already covered by tests shipped in their origin phases (28, 29, 27) — Phase 30 CITES and RE-RUNS them under the bare recipe; it did not rewrite them.
- Criterion **4** was the one genuine gap: the existing `harden04` test SELF-SKIPPED under `cargo test --workspace` (feature unification pulled `brokerd::test-fixtures` in). Phase 30's NEW `scripts/verify-harden04-featureless.sh` (DESIGN §j) makes the assertion actually execute and FAILS on the self-skip — closing the false-assurance gap.
- Criterion **1** is a fresh full-suite baseline run.
- The `30-REGRESSION-AUDIT.md` sweep (Plan 30-01) found 0 `#[ignore]`, 0 `assert!(true)`, 0 TODO/FIXME across all Phase 27/28/29 test files.

## Minor deviation noted

- 30-02-PLAN Task 1's `<automated>` command listed the 3 criterion-2 tests as bare positionals; `cargo test` accepts only one positional filter, so the multi-filter form requires a `--` separator: `cargo test -p brokerd --lib -- <name1> <name2> <name3>`. Run with the corrected form (3 passed / 0 failed). Non-blocking; recorded for accuracy.

## Human / orchestrator sign-off

- [x] **DONE — all 5 HARDEN-06 criteria verified green on real Linux, non-laundered, true-exit-before-pipe. v1.6 hardening milestone proof complete.** — recorded by the orchestrator 2026-07-17, BEFORE the final plan flips complete (record-sign-off-before-last-plan discipline; `phase.complete` then reconciles HARDEN-06 in REQUIREMENTS.md).
