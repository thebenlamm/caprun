---
phase: 1
slug: substrate-foundation
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-06-29
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | none — workspace Cargo.toml installs |
| **Quick run command** | `cargo test -p runtime-core` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds (cold build dominates; incremental test run < 3s) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p runtime-core`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd-verify-work`:** Full suite must be green + `bash scripts/check-invariants.sh` exits 0
- **Max feedback latency:** ~15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | REQ-runtime-core | T-01-SC | Pinned + slopchecked crates.io deps; root is virtual manifest only | compile + grep | `cargo build --workspace 2>&1 \| tail -5; grep -c '^\[package\]' Cargo.toml` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | REQ-runtime-core | T-01-02 | ValueNode carries literal+provenance+taint from first commit (compile-enforced) | compile | `cargo build -p runtime-core 2>&1 \| tail -5` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | REQ-runtime-core | T-01-03 | runtime-core is I/O-free (negative-grep purity gate) | unit + grep | `cargo test -p runtime-core 2>&1 \| tail -8 && ! grep -rE "std::io\|std::fs\|std::net\|tokio\|async fn" crates/runtime-core/src/` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 2 | REQ-api-stub-plan-node | T-01-01 | submit_plan_node returns typed NotImplemented (no panic/todo!) | unit | `cargo test -p brokerd -- submit_plan_node_returns_not_implemented 2>&1 \| tail -8` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 2 | REQ-api-stub-plan-node | T-01-01 / T-01-04 | No raw effect-request-to-sink path anywhere under crates/ (grep gate) | grep + unit | `bash scripts/check-invariants.sh && cargo test --workspace 2>&1 \| tail -10` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

This is a greenfield Rust workspace — all test infrastructure is created within Phase 1 itself (not a separate Wave 0):

- [ ] `Cargo.toml` — workspace virtual manifest (Plan 01-01 Task 1)
- [ ] `crates/runtime-core/tests/types_compile.rs` — field-presence + serde round-trip coverage for all domain types (Plan 01-01 Task 3)
- [ ] `crates/brokerd/src/lib.rs` `#[cfg(test)]` — `submit_plan_node_returns_not_implemented` unit test (Plan 01-02 Task 1)
- [ ] `scripts/check-invariants.sh` — re-runnable negative-grep gate for the two architectural invariants (Plan 01-02 Task 2)

Framework: `cargo test` is built into the Rust toolchain (1.92.0 confirmed available) — no framework install needed.

---

## Manual-Only Verifications

All phase behaviors have automated verification (compile checks, unit tests, and negative-grep gates). No manual-only verifications.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (greenfield — created in-phase)
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** ready
