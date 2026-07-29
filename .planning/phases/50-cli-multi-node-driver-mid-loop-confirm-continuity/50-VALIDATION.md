---
phase: 50
slug: cli-multi-node-driver-mid-loop-confirm-continuity
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-29
---

# Phase 50 — Validation Strategy

> Per-phase validation contract for CLI multi-node driver & mid-loop confirm continuity (CLI-01, CLI-02, CONFIRM-01).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` / `#[tokio::test]` via cargo |
| **Config file** | none — Cargo workspace defaults |
| **Quick run command** | `./scripts/check-invariants.sh && cargo test -p caprun --test stream_hold --test stream_substrate --test planner -- --test-threads=1` |
| **Full suite command** | `./scripts/check-invariants.sh && cargo test --workspace --no-fail-fast` (Linux SMTP: `bash scripts/mailpit-verify.sh`) |
| **Linux authority** | Host unit tests for hold protocol / exit map / argv; full push/PR LIVE is Phase 51 |
| **Estimated runtime** | quick ~30–120s; full workspace longer |

---

## Sampling Rate

- **After every task commit:** `./scripts/check-invariants.sh` + targeted `cargo test -p caprun --test stream_hold --test stream_substrate --test planner` (+ coding_cli / e2e / confirm / grant when added)
- **After every plan wave:** above + `cargo test -p caprun --test e2e` / confirm / grant as applicable + invariants
- **Before `/gsd-verify-work`:** invariants green; hold/exit/argv tests green; email/file + confirm/grant regression green; **no LIVE-07 SUCCESS claim**
- **Max feedback latency:** ~120s for focused runs

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 50-01-T1 | 50-01 | 1 | CONFIRM-01, CLI-02 | T-50-01, T-50-03 | Hold PROCEED no re-submit; protocol + exit map | unit | `cargo test -p caprun --test stream_hold --test stream_substrate -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 50-01-T2 | 50-01 | 1 | CLI-02, CONFIRM-01 | T-50-03 | HoldAbort; DENIED code=policy_deny; Deny exit 2 | unit | `cargo test -p caprun --test stream_hold --test stream_substrate --test planner -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 50-02-T1 | 50-02 | 2 | CLI-01, CONFIRM-01, CLI-02 | T-50-02, T-50-05, T-50-06 | safe-coding-workflow argv; piped hold; mid-loop confirm; grant pointer; exit 0/2/3/1 | unit/check | `./scripts/check-invariants.sh && cargo check -p caprun` | ❌ | ⬜ pending |
| 50-02-T2 | 50-02 | 2 | CLI-01, CLI-02 | T-50-07 | coding_cli + e2e/confirm/grant regression; COVERAGE honesty | integration | `cargo test -p caprun --test coding_cli --test e2e --test confirm --test grant --test stream_hold --test stream_substrate --test planner -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 50-*-email-reg | 50-02 | 2 | CLI-01 | — | Existing email/file run still works | integration | `cargo test -p caprun --test e2e` | ✅ | ⬜ pending |
| 50-*-confirm-surf | 50-02 | 2 | CLI-01 | — | review/confirm/deny/grant still dispatch | integration | `cargo test -p caprun --test confirm`; grant tests | ✅ | ⬜ pending |
| 50-*-no-remint | 50-01/02 | * | CONFIRM-01 | T-50-01 | No mid-stream ProvideIntent / no dual Session product path | script/grep | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |
| 50-*-hyg | 50-01/02 | * | HYG | T-50-07 / Gate 1 | Gate 1/3; zero new crates | script | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cli/caprun/tests/stream_hold.rs` — protocol contract + exit-map unit tests (50-01)
- [ ] Extend `cli/caprun/tests/stream_substrate.rs` — HoldContinue (no re-submit); HoldAbort (50-01)
- [ ] `cli/caprun/tests/coding_cli.rs` — CLI-01 argv → SafeCodingWorkflow JSON; unknown kind fail-closed; policy flag; seed-from-file reject (50-02)
- [ ] Email/file e2e + confirm/grant regression still green (50-02)
- [ ] Optional: Linux coding smoke only if environment allows — **never** claim LIVE-07 SUCCESS
- [ ] Framework install: **none** — use existing cargo test

---

## Edge / Prohibition Lift (spec-less probe)

| Requirement | Resolution | Verification |
|-------------|------------|--------------|
| CLI-01 | **covered** | coding_cli + main safe-coding-workflow path + e2e/confirm/grant regression |
| CLI-02 | **covered** | stream_hold exit map + DENIED/BLOCKED lines + main process exit 0/2/3/1 |
| CONFIRM-01 | **covered** | HoldContinue/HoldAbort + worker stay-connected + main mid-loop confirm PROCEED/ABORT |

**Prohibitions (descriptor-less project locks):** no free-form effect path under crates/; no dual-Session stitch; no reconnect-remint; no session-wide confirm waiver; no silent continue-past-Block; no LIVE-07 claim in Phase 50.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Interactive TTY confirm UX polish | CLI-01/CONFIRM-01 | Discretionary product UX | Optional after automated hold protocol is green |
| Non-hybrid LIVE multi-node success + mid-loop I2 Block on real Linux | LIVE-07/08 | Phase 51 scope | Do not claim LIVE-07 SUCCESS in Phase 50 |

*All Phase 50 in-scope product behaviors should have automated verification via Wave 0 + plan tasks.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 tests land

**Approval:** pending (plans authored; Wave 0 tests land during execute)
