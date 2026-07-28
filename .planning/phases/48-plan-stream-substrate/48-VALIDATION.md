---
phase: 48
slug: plan-stream-substrate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-28
---

# Phase 48 — Validation Strategy

> Per-phase validation contract for sequential plan-stream substrate (STREAM-01/02).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` / `#[tokio::test]` via cargo |
| **Config file** | per-crate `Cargo.toml` |
| **Quick run command** | `./scripts/check-invariants.sh && cargo test -p caprun --test planner -- --nocapture` |
| **Full suite command** | `./scripts/check-invariants.sh && cargo test --workspace --no-fail-fast` |
| **Linux authority** | `bash scripts/mailpit-verify.sh` (scoped `MAILPIT_VERIFY_CMD` for stream tests) |
| **Estimated runtime** | quick ~30–90s; full workspace longer |

---

## Sampling Rate

- **After every task commit:** `./scripts/check-invariants.sh` + focused `cargo test` for touched crate/test
- **After every plan wave:** `cargo test --workspace --no-fail-fast` (host); Linux stream legs via mailpit-verify when available
- **Before `/gsd-verify-work`:** invariants green + STREAM-01/02 automated tests green
- **Max feedback latency:** ~120s for focused runs

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 48-01-* | 01 | 1 | STREAM-01 | T-48-batch | N sequential SubmitPlanNode; verify_chain | integration | `cargo test -p brokerd` stream multi-submit | ❌ W0 | ⬜ pending |
| 48-01-* | 01 | 1 | STREAM-02 | T-48-launder | Opaque bag; any Some(output_value_id) | unit | `cargo test -p caprun` handle bag | ❌ W0 | ⬜ pending |
| 48-* | * | * | HYG | T-48-mint | Gate 1/3; no new mint sites | script | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Multi-submit + `verify_chain` test (brokerd or caprun integration)
- [ ] Handle bag unit tests (any `Some(output_value_id)`; opaque ValueId only)
- [ ] Sequential planner surface + one-shot adapter regression
- [ ] Deny/abort remaining + ProvideIntent-once under multi-submit
- [ ] Optional Linux taint-via-bag leg via mailpit-verify
- [ ] Framework install: **none**

*Existing planner tests + check-invariants cover regressions; Wave 0 is stream-specific tests + implementation.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Phase 50 Block-and-Hold product path not required this phase | STREAM-01/CONFIRM-01 split | Product confirm UX is Phase 50 | Confirm Phase 48 stops fail-closed on Block without re-submit; no dual-Session |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s focused
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
