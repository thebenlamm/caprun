---
phase: 30
slug: regression-live-proof
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-17
---

# Phase 30 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from 30-RESEARCH.md "Validation Architecture". This is a proof/regression
> phase: 4 of 5 criteria already have shipped passing tests (cite + re-run); criterion 4
> is the one genuine gap (a self-skipping test under the bare recipe — DESIGN §j).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust libtest), workspace-wide (`resolver = "3"`, edition 2021) |
| **Config file** | root `Cargo.toml` (workspace members); no separate test-framework config |
| **Quick run command** | `cargo test -p <crate> --test <target> <test_name>` (macOS-runnable for x-platform tests) |
| **Full suite command** | `bash scripts/mailpit-verify.sh` (bare, real Linux via Colima) |
| **Criterion-4 command** | `bash scripts/verify-harden04-featureless.sh` (NEW — scoped featureless build+test, bypasses `--workspace` feature unification) |
| **Estimated runtime** | ~90s quick unit; ~several min full Linux suite |

---

## Sampling Rate

- **After every task commit:** targeted `cargo test -p <crate> <test_name>` for whatever the task touched; `cargo build --workspace` for wiring.
- **After every plan wave:** `cargo build --workspace && ./scripts/check-invariants.sh` (macOS sufficient for wiring; Linux-gated behavior deferred to phase gate).
- **Before `/gsd-verify-work`:** BOTH bare `bash scripts/mailpit-verify.sh` (criteria 1/2/3/5, full regression) AND `bash scripts/verify-harden04-featureless.sh` (criterion 4) green on real Linux. Capture TRUE exit before any pipe; assert on named tests + counts, never on `exit 0` through a pipe.
- **Max feedback latency:** ~90s (unit) / full-suite at the phase gate.

> ⚠️ **cfg(linux) test-blindness:** a green macOS `cargo test` proves NOTHING about the Linux-gated tests (replay_cas, s9_live_block, harden04). The bare recipe on real Linux is the only authoritative gate.
> ⚠️ **false-assurance guard:** criterion-4's existing test self-skips under `cargo test --workspace` (feature unification pulls `brokerd`'s `test-fixtures` in) — the scoped script is what makes the assertion actually execute.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 30-01-01 | 01 | 1 | HARDEN-06 (crit 4) | compile-exclusion | Forced-Active `CreateSession` arm absent from a featureless build; the scoped test's assertion actually EXECUTES (not self-skipped) | integration, Linux-gated, SCOPED | `bash scripts/verify-harden04-featureless.sh` | ❌ W0 (new script) | ⬜ pending |
| 30-01-02 | 01 | 1 | HARDEN-06 (audit) | regression-integrity | No Phase 27/28/29 test is `#[ignore]`'d, weakened, or self-skipping since its own verification pass | audit (doc) | manual sweep → `30-REGRESSION-AUDIT.md` | ❌ W0 (new doc) | ⬜ pending |
| 30-02-01 | 02 | 2 | HARDEN-06 (crit 1) | regression | Full workspace regression green on real Linux; fresh baseline recorded (supersedes stale 309/0) | e2e (whole suite) | `bash scripts/mailpit-verify.sh` | ✅ (script exists) | ⬜ pending |
| 30-02-02 | 02 | 2 | HARDEN-06 (crit 2) | chain-forge | `verify_chain` rejects a self-consistent forged chain AND a truncated tail AND a keyless legacy DB | unit (x-platform) | `cargo test -p brokerd --lib self_consistent_forgery_without_key_is_rejected tail_truncation_detected_via_anchor_mismatch legacy_db_without_anchor_fails_closed` | ✅ `audit.rs:1522,1673,1779` | ⬜ pending |
| 30-02-03 | 02 | 2 | HARDEN-06 (crit 3) | replay | Replayed Allowed `email.send` delivers exactly once | integration, Linux-gated | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test replay_cas allowed_email_send_replay_delivers_once' bash scripts/mailpit-verify.sh` | ✅ `replay_cas.rs` | ⬜ pending |
| 30-02-04 | 02 | 2 | HARDEN-06 (crit 5) | fd-release demotion | `RequestFd` on untrusted path demotes to Draft; trusted + CONTROL-01 clean path stay unaffected | integration | `cargo test -p brokerd --test harden01_session_integrity` + `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_control_ab_taint_driven' bash scripts/mailpit-verify.sh` | ✅ `harden01_session_integrity.rs`, `s9_live_block.rs:813` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `scripts/verify-harden04-featureless.sh` — NEW. Formalizes the DESIGN §j featureless-build + scoped-test proof for criterion 4 (runs `cargo build --workspace --release` as the featureless production artifact + the scoped `cargo test -p caprun --test harden04_featureless_create_session` so the assertion actually runs, not self-skipped). The ONE required new executable artifact this phase.
- [ ] `.planning/phases/30-regression-live-proof/30-REGRESSION-AUDIT.md` — NEW. Sweep all Phase 27/28/29 test files for `#[ignore]`, weakened assertions, or stale skip conditions (mirrors Phase 25's regression audit).

*(No Wave 0 gap for criteria 1, 2, 3, 5 — their tests already exist and already pass; no new test scaffolding needed, only a fresh run + citation.)*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Final milestone-closure sign-off (record fresh Linux baseline pass/fail tally as the new v1.6 baseline) | HARDEN-06 | The full-suite count is an evidentiary baseline recorded by the orchestrator running the bare recipe directly (non-laundered), true exit captured before any pipe | Run `bash scripts/mailpit-verify.sh`; capture `$?` before piping; record suite count + 0 failures into `30-VERIFICATION.md` / SUMMARY |

*All behavioral assertions are automated; the only "manual" item is recording the evidentiary baseline.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (criterion-4 script + regression audit)
- [x] No watch-mode flags
- [x] Feedback latency < 90s (unit tier)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-17 (derived from 30-RESEARCH.md HIGH-confidence per-criterion map; 4/5 EXISTS, criterion-4 gap has a pinned DESIGN §j fix)
