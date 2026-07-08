---
phase: 14
slug: content-sensitive-sink-arg-blocking
status: draft
nyquist_compliant: true
wave_0_complete: true
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
| 14-01-T1 | 14-01 | 1 | CONTENT-01 | T-14-03 | Plural block decision (BlockedArg / Vec anchors), anti-stapling per element | unit | `cargo test -p runtime-core` | ✅ (task2_types.rs) | ⬜ pending |
| 14-01-T2 | 14-01 | 1 | CONTENT-01 / CONTENT-02 | T-14-01, T-14-02, T-14-04 | Collect-then-Block loop + attachment descope; Step 0.5 ordering preserved | unit | `cargo test -p executor` | ✅ (executor_decision.rs) | ⬜ pending |
| 14-01-T3 | 14-01 | 1 | CONTENT-01 / CONTENT-02 | T-14-01, T-14-02 | Proof tests: both-args block, body+trusted-recipient still blocks, attachment UnknownArg, unknown-sink-not-content-sensitive | unit | `cargo test -p executor` | ❌ new tests | ⬜ pending |
| 14-02-T1 | 14-02 | 2 | CONTENT-01 | T-14-05 | Event.anchors plural; golden-byte fixture preserved | unit | `cargo test -p runtime-core` | ✅ (event.rs golden test) | ⬜ pending |
| 14-02-T2 | 14-02 | 2 | CONTENT-01 | T-14-06, T-14-07, T-14-08 | Broker plural decision handling; fail-closed empty-anchors guard; render safeguard | unit/integration | `cargo test -p brokerd` | ✅ (durable_anchor.rs) | ⬜ pending |
| 14-02-T3 | 14-02 | 2 | CONTENT-01 | — | cli consumer migration; workspace green + check-invariants | integration | `cargo test --workspace --no-fail-fast` | ✅ (confirm.rs) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing test infrastructure covers all phase requirements — no scaffolding-only Wave 0 plan is needed. The new proof tests are added inline in 14-01-T3 (executor) and the type-shape tests are re-verified/updated in place. Specifically:

- `crates/executor/tests/executor_decision.rs` already exists — the stale `tainted_body_and_attachment_allow_in_v0` test is REWRITTEN (renamed) in 14-01-T2; new tests are added in 14-01-T3.
- `crates/runtime-core/src/event.rs` golden-byte fixture already exists — re-verified (not newly written) in 14-02-T1.
- `crates/brokerd/src/audit.rs` Defect-B guard test already exists — updated for `anchors.is_empty()` in 14-02-T2.
- No MISSING `<automated>` references: every task has a runnable `cargo test` command; no 3 consecutive tasks lack automated verify.

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

**Approval:** approved (planner, 2026-07-07) — all tasks have `<automated>` verify; sampling continuity holds (no 3 consecutive tasks without automated verify); Wave 0 covered by existing infra; no watch-mode flags; feedback latency < 60s (`cargo test -p executor`).
