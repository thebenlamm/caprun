---
phase: 24
slug: slot-type-binding-enforcement
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-11
---

# Phase 24 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust, single Cargo workspace, edition 2021) |
| **Config file** | none — workspace `Cargo.toml` at repo root |
| **Quick run command** | `cargo test -p executor` / `cargo test -p brokerd` (crate-scoped) |
| **Full suite command** | `cargo test --workspace --no-fail-fast` (Mac: Linux-only security tests show 0-passed by design) |
| **Estimated runtime** | ~60–120 seconds (Mac workspace build+test) |

> **Linux enforcement note:** the new `Denied` path itself is workspace-buildable and unit-testable
> on the Mac (per DESIGN §9). The held-out swapped-handle acceptance test + full Linux re-verify via
> `bash scripts/mailpit-verify.sh` are **Phase 25** (T2-06/T2-08), NOT this phase. Phase 24 must
> still leave the Mac-runnable workspace green (`cargo build --workspace && cargo test --workspace`).

---

## Sampling Rate

- **After every task commit:** Run the crate-scoped `cargo test -p <crate>` for the touched crate
- **After every plan wave:** Run `cargo build --workspace && cargo test --workspace --no-fail-fast`
- **Before `/gsd-verify-work`:** Full Mac workspace must be green (0 failures; Linux-only tests 0-passed expected)
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

> Populated by the planner from PLAN.md tasks. Each task's `<verify>` must map to an automated
> `cargo test`/`cargo build` command or `./scripts/check-invariants.sh` assertion.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 24-01-01 | 01 | 1 | T2-02 | — | minted value carries origin-role tag; trust classification unchanged | unit | `cargo test -p executor` | ❌ W0 | ⬜ pending |
| 24-01-02 | 01 | 1 | T2-04 | — | new exhaustive `DenyReason::SlotTypeMismatch` arm; no wildcard | unit | `cargo test --workspace` | ❌ W0 | ⬜ pending |
| 24-02-01 | 02 | 2 | T2-03 | — | hardcoded expected-role table for `email.send` + `file.create` | unit | `cargo test -p executor` | ❌ W0 | ⬜ pending |
| 24-02-02 | 02 | 2 | T2-05 | — | `submit_plan_node` Denies on role↔slot mismatch, per-arg, precedence intact | unit | `cargo test -p brokerd` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky · IDs illustrative — planner finalizes.*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements — `cargo test` is already wired; no new
framework install. New unit tests are added inline in each crate's existing `#[cfg(test)]` modules
(`crates/executor/tests/`, `crates/executor/src/*.rs`, `crates/brokerd/`).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Linux kernel-enforced `Denied` dispatch end-to-end | T2-06 (Phase 25) | Landlock/seccomp Linux-only; not this phase | Deferred to Phase 25 `scripts/mailpit-verify.sh` |

*All Phase-24-scoped behaviors (T2-02..05) have automated `cargo test` verification on the Mac.*

---

## Validation Sign-Off

- [ ] All tasks have `<verify>` automated commands or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (none — infra exists)
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter (set by planner once map is finalized)

**Approval:** pending
