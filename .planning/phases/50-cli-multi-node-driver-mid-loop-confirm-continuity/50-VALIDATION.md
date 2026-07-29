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
| **Quick run command** | `./scripts/check-invariants.sh && cargo test -p caprun --test stream_substrate --test planner -- --test-threads=1` |
| **Full suite command** | `./scripts/check-invariants.sh && cargo test --workspace --no-fail-fast` (Linux SMTP: `bash scripts/mailpit-verify.sh`) |
| **Linux authority** | Host unit tests for hold protocol / exit map / argv; full push/PR LIVE is Phase 51 |
| **Estimated runtime** | quick ~30–120s; full workspace longer |

---

## Sampling Rate

- **After every task commit:** `./scripts/check-invariants.sh` + targeted `cargo test -p caprun --test stream_substrate --test planner` (+ new coding_cli / hold tests when added)
- **After every plan wave:** above + `cargo test -p caprun --test e2e` / confirm / grant as applicable + invariants
- **Before `/gsd-verify-work`:** invariants green; hold/exit/argv tests green; email/file + confirm/grant regression green; **no LIVE-07 SUCCESS claim**
- **Max feedback latency:** ~120s for focused runs

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 50-*-argv | TBD | 0/1 | CLI-01 | T-50-policy | safe-coding-workflow argv → SafeCodingWorkflow + policy bind once | unit | `cargo test -p caprun --test coding_cli` (name may vary) | ❌ W0 | ⬜ pending |
| 50-*-email-reg | TBD | * | CLI-01 | — | Existing email/file run still works | integration | `cargo test -p caprun --test e2e` | ✅ | ⬜ pending |
| 50-*-confirm-surf | TBD | * | CLI-01 | — | review/confirm/deny/grant still dispatch | integration | `cargo test -p caprun --test confirm`; grant tests | ✅ | ⬜ pending |
| 50-*-block-surface | TBD | 0/1 | CLI-02 | T-50-silent | Block surfaces effect_id + review pointer; no silent continue | unit | stream_substrate + protocol tests | ❌ W0 | ⬜ pending |
| 50-*-deny-abort | TBD | 0/1 | CLI-02 | T-50-deny | Deny/policy_deny abort remaining; distinct code/label | unit | stream_substrate + exit-map | ✅ partial | ⬜ pending |
| 50-*-exit-map | TBD | 0/1 | CLI-02 | — | success=0, denied=2, blocked=3, infra=1 | unit | pure exit mapper tests | ❌ W0 | ⬜ pending |
| 50-*-hold-proceed | TBD | 0/1 | CONFIRM-01 | T-50-remint | PROCEED advances without re-submit of blocked node | unit | HoldContinue assertion | ❌ W0 | ⬜ pending |
| 50-*-hold-abort | TBD | 0/1 | CONFIRM-01 | — | ABORT after Block stops remaining nodes | unit | HoldAbort branch | ❌ W0 | ⬜ pending |
| 50-*-no-remint | TBD | * | CONFIRM-01 | T-50-remint | No mid-stream ProvideIntent / no dual Session product path | script/grep | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |
| 50-*-hyg | TBD | * | HYG | T-50-effect | Gate 1/3; zero new crates | script | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cli/caprun/tests/coding_cli.rs` (or e2e extension) — CLI-01 argv → INTENT JSON SafeCodingWorkflow; unknown kind fail-closed; policy flag accepted
- [ ] Extend `cli/caprun/tests/stream_substrate.rs` — HoldContinue (Block then PROCEED: no re-submit; step advances); HoldAbort
- [ ] Exit-code map tests — success=0, denied=2, blocked=3 (unit over pure mapper fn if extracted)
- [ ] Worker/main protocol contract tests — parse BLOCKED/DENIED/STREAM_DONE lines; PROCEED/ABORT tokens
- [ ] Optional: integration harness that drives real worker hold with in-process confirm against temp audit DB (host mock OK; full push/PR LIVE → Phase 51)
- [ ] Framework install: **none** — use existing cargo test

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
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
