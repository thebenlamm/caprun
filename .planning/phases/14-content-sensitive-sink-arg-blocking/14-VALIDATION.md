---
phase: 14
slug: content-sensitive-sink-arg-blocking
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-07
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust workspace) |
| **Config file** | none — workspace `Cargo.toml` at repo root |
| **Quick run command** | `cargo test -p executor` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` |
| **Estimated runtime** | ~60 seconds (macOS host); Linux enforcement tests run via Colima+Docker |

> **Linux-only note:** enforcement / negative-assertion tests are `#[cfg(target_os = "linux")]`.
> On the Mac host they show "0 passed" — expected. Run them with the Colima+Docker recipe
> (see CLAUDE.md "Linux-only security tests").

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p executor`
- **After every plan wave:** Run `cargo test --workspace --no-fail-fast`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| _(planner fills)_ | | | CONTENT-01 / CONTENT-02 | | | unit | `cargo test -p executor` | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*(planner fills — likely "Existing infrastructure covers all phase requirements"; `crates/executor/tests/executor_decision.rs` already exists.)*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| _(planner fills)_ | | | |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
