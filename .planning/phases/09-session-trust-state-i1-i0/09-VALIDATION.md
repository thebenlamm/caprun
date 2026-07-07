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
| TBD | TBD | TBD | TAINT-01 | — | Session demoted to Draft on mint_from_read | unit | `cargo test -p brokerd` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | TAINT-02 | — | CommitIrreversible denied against Draft session (after I2 loop) | unit | `cargo test -p executor` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | TAINT-03 | — | MutateReversible/Observe still succeeds against Draft session | unit | `cargo test -p executor` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | TAINT-04 | — | Demotion recorded as audit event with causal edge to triggering read | unit | `cargo test -p brokerd audit` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | ORIGIN-01 | — | caprun CLI decides seed-provenance for externally-derived intent | unit/e2e | `cargo test -p caprun` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | ORIGIN-02 | — | Session created with externally-derived seed starts Draft | unit | `cargo test -p brokerd` | ❌ W0 | ⬜ pending |

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
