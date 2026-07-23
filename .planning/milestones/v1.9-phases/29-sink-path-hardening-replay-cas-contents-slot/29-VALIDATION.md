---
phase: 29
slug: sink-path-hardening-replay-cas-contents-slot
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-17
---

# Phase 29 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in harness) + `scripts/mailpit-verify.sh` for Linux-gated security tests |
| **Config file** | none — plain per-crate `Cargo.toml` test targets; `crates/brokerd/tests/*.rs` integration tests |
| **Quick run command** | `cargo test -p executor sink_sensitivity` (HARDEN-05 unit — runs on macOS, no cfg-linux gate) |
| **Full suite command** | `bash scripts/mailpit-verify.sh` (Linux-only; required for HARDEN-03's SMTP-touching test + any `#[cfg(target_os="linux")]` test) |
| **Estimated runtime** | ~90s quick unit; ~several min full Linux suite via Colima/Docker |

---

## Sampling Rate

- **After every task commit:** `cargo test -p executor sink_sensitivity` (HARDEN-05 edits, macOS-runnable); `cargo build --workspace` for HARDEN-03 edits (macOS can compile-check but cannot run the Linux-gated integration test).
- **After every plan wave:** `bash scripts/mailpit-verify.sh` — the ONLY way to actually exercise the HARDEN-03 CAS and the `s9_live_file_create_clean_allow` regression canary.
- **Before `/gsd-verify-work`:** Full `scripts/mailpit-verify.sh` green on real Linux.
- **Max feedback latency:** ~90s (unit) / full-suite at wave boundaries.

> ⚠️ **cfg(linux) test-blindness:** a green macOS `cargo test` run proves NOTHING about HARDEN-03's CAS test or the HARDEN-05 regression canary — both are `#[cfg(target_os="linux")]`. Enumerate Linux-gated call sites via `cargo build --tests --keep-going` in the Linux gate; never trust a green Mac build as coverage.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 29-01-xx | 01 | 1 | HARDEN-03 | replay | Content-derived idempotency-key derivation is order-invariant / sink+arg-scoped (mirrors `combined_digest`) | unit (x-platform) | `cargo test -p brokerd plan_node_idempotency_key` | ❌ W0 | ⬜ pending |
| 29-01-xx | 01 | 1 | HARDEN-03 | replay | `sent_plan_nodes` migration is idempotent on re-run | unit (x-platform) | `cargo test -p brokerd sent_plan_nodes_migration` | ❌ W0 | ⬜ pending |
| 29-01-xx | 01 | 1 | HARDEN-03 | replay | Replayed Allowed `email.send` sends at most once (1 Mailpit delivery, 1 `sent_plan_nodes` row, 1 `email_send_attempted`) | integration, Linux-only | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test <replay_test>' bash scripts/mailpit-verify.sh` | ❌ W0 | ⬜ pending |
| 29-02-xx | 02 | 2 | HARDEN-05 | value-injection | `file.create` `contents` is content-sensitive + role-checked to `Some(["path"])` | unit (x-platform) | `cargo test -p executor sink_sensitivity` | ✅ (inverts existing assertion) | ⬜ pending |
| 29-02-xx | 02 | 2 | HARDEN-05 | over-widening | `file.create` `path` arg is NOT content-sensitive after the change (defense-in-depth) | unit (x-platform) | `cargo test -p executor sink_sensitivity` | ❌ W0 (new) | ⬜ pending |
| 29-02-xx | 02 | 2 | HARDEN-05 | regression | `s9_live_file_create_clean_allow` still Allows after the role-list change (no false-positive block) | integration, Linux-only | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_live_file_create_clean_allow' bash scripts/mailpit-verify.sh` | ✅ existing canary | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky · Task IDs finalized by the planner.*

---

## Wave 0 Requirements

- [ ] New unit tests for the idempotency-key derivation fn (order-invariance, sink-scoping, distinct-args→distinct-key) — mirror `combined_digest`'s existing unit suite shape in `confirmation.rs`.
- [ ] New Linux-only integration test: submit an identical `SubmitPlanNode` twice on a trusted (never-blocked) `email.send` path; assert exactly one Mailpit delivery AND one `sent_plan_nodes` row AND one `email_send_attempted` event.
- [ ] New unit test asserting `file.create`'s `path` arg stays non-content-sensitive after HARDEN-05 (guards the over-widening failure mode).
- [ ] `sent_plan_nodes` schema-migration idempotency test, mirroring `pending_confirmations`/`chain_anchor` presence-check migration tests.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | All phase behaviors have automated verification. | — |

*All phase behaviors have automated verification (Phase 30 owns the composed live re-run + per-residual negative tests).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s (unit tier)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-17 (plan-checker VERIFICATION PASSED — every task carries an automated verify, no watch-mode flags, no 3-consecutive-unverified gap)
