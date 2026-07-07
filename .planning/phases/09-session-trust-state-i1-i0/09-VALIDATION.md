---
phase: 9
slug: session-trust-state-i1-i0
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-06
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p brokerd -p executor --no-fail-fast` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p brokerd -p executor --no-fail-fast`
- **After every plan wave:** Run `cargo test --workspace --no-fail-fast`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-T2 | 09-03 | 3 | TAINT-01 | T-09-07 | Session demoted to Draft on mint_from_read | unit | `cargo test -p brokerd --lib quarantine` | ⬜ new | ⬜ pending |
| 02-T3 | 09-02 | 2 | TAINT-02 | T-09-02/03 | CommitIrreversible denied against Draft session (after I2 loop) | unit | `cargo test -p executor --test executor_decision` | ⬜ new | ⬜ pending |
| 02-T3 | 09-02 | 2 | TAINT-03 | — | MutateReversible/Observe still succeeds against Draft session (via `#[cfg(test)] test.observe` fixture) | unit | `cargo test -p executor --test executor_decision` | ⬜ new | ⬜ pending |
| 03-T2 | 09-03 | 3 | TAINT-04 | T-09-08 | Demotion recorded as audit event with causal edge (parent_id == file_read id) | unit | `cargo test -p brokerd --lib quarantine` | ⬜ new | ⬜ pending |
| 04-T2 | 09-04 | 4 | ORIGIN-01 | T-09-10 | caprun CLI decides seed-provenance for externally-derived intent | integration | `cargo test -p caprun` | ⬜ new | ⬜ pending |
| 03-T1 / 04-T2 | 09-03, 09-04 | 3, 4 | ORIGIN-02 | T-09-11 | Session created with externally-derived seed starts Draft | unit + integration | `cargo test -p brokerd --lib session` / `cargo test -p caprun` | ⬜ new | ⬜ pending |

*Populated with placeholder rows per phase requirement; the planner fills in exact Task ID / Plan / Wave / File Exists columns once PLAN.md exists.*

---

## Wave 0 Requirements

- [ ] Fixture sink for TAINT-03 — both registered sinks (`email.send`, `file.create`) are `CommitIrreversible`; no live sink exists to exercise the "MutateReversible/Observe still succeeds against Draft" path. Per 09-RESEARCH.md, a `#[cfg(test)]`-gated fixture sink is required, following existing platform-gating precedent.
- [ ] Test fixtures/helpers for constructing a `Draft`-status session and asserting `DenyReason` variant match.

---

## Manual-Only Verifications

*None — all phase behaviors have automated verification (Rust unit/integration tests; no UI, no Linux-only manual step beyond the existing Landlock/seccomp test suite already covered by CI/Colima convention).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
