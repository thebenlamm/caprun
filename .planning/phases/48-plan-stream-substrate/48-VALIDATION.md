---
phase: 48
slug: plan-stream-substrate
status: validated
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-28
updated: 2026-08-11
---

# Phase 48 — Validation Strategy

> Per-phase validation contract for sequential plan-stream substrate (STREAM-01/02).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` / `#[tokio::test]` via cargo |
| **Config file** | per-crate `Cargo.toml` |
| **Quick run command** | `./scripts/check-invariants.sh && cargo test -p caprun --test planner -- --nocapture && cargo test -p caprun --test stream_substrate -- --nocapture` |
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
| 48-01-T1 | 01 | 1 | STREAM-01/02 | T-48-01 | plan_next + bag + sequential worker loop | unit | `cargo test -p caprun --test planner` | ✅ | ✅ green |
| 48-01-T2 | 01 | 1 | STREAM-01 | T-48-batch | N sequential SubmitPlanNode; verify_chain; ProvideIntent-once | integration | `cargo test -p brokerd --test stream_multi_submit` | ✅ | ✅ green (Linux) |
| 48-02-T1 | 02 | 2 | STREAM-01 | T-48-09, T-48-04 | Deny abort remaining; Block no re-submit; F-01 bag any Some | unit | `cargo test -p caprun --test stream_substrate` | ✅ | ✅ green |
| 48-02-T2 | 02 | 2 | STREAM-02 | T-48-01 | Genuine taint via bag → process.exec/command Block + verify_chain | hybrid Linux | `cargo test -p caprun --test stream_substrate` (cfg linux) | ✅ | ✅ present (host: compile-away; Linux: mailpit-verify) |
| 48-* | * | * | HYG | T-48-06 | Gate 1/3; no new mint sites | script | `./scripts/check-invariants.sh` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] Multi-submit + `verify_chain` test (brokerd or caprun integration) — `crates/brokerd/tests/stream_multi_submit.rs`
- [x] Handle bag unit tests (any `Some(output_value_id)`; opaque ValueId only) — `planner.rs` + `stream_substrate.rs` F-01
- [x] Sequential planner surface + one-shot adapter regression — `cli/caprun/tests/planner.rs`
- [x] Deny/abort remaining + ProvideIntent-once under multi-submit — `stream_substrate` deny/block + `stream_multi_submit` ProvideIntent-once
- [x] Optional Linux taint-via-bag leg via mailpit-verify — `stream_substrate::linux::taint_via_bag_exec_output_blocks_with_genuine_provenance`
- [x] Framework install: **none**

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Phase 50 Block-and-Hold product path not required this phase | STREAM-01/CONFIRM-01 split | Product confirm UX is Phase 50 | Confirm Phase 48 stops fail-closed on Block without re-submit; no dual-Session |

---

## Linux run recipes

```bash
# stream_substrate (host-safe + Linux taint leg)
MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test stream_substrate -- --nocapture' \
  bash scripts/mailpit-verify.sh

# Plan 01 broker multi-submit regression
MAILPIT_VERIFY_CMD='cargo test -p brokerd --test stream_multi_submit -- --nocapture' \
  bash scripts/mailpit-verify.sh
```

When Docker/mailpit is unavailable, host runs `cargo test -p caprun --test stream_substrate` for non-Linux-gated cases; Linux bodies are `#[cfg(target_os = "linux")]` (0 passed on non-Linux is expected per CLAUDE.md).

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s focused
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** wave-0 complete after 48-01 + 48-02 automated STREAM tests exist and host-safe legs pass. Production-worker coverage is additionally established by Phase 51's retained, hash-verified real-Linux CLI evidence.

## Validation Audit 2026-08-11

| Metric | Count |
|--------|-------|
| Requirements audited | 2 |
| Covered | 2 |
| Partial | 0 |
| Missing | 0 |
| Gaps found | 0 |
| Resolved by retained authoritative evidence | 1 |
| Escalated | 0 |

Current focused evidence passed: planner 25/25, stream substrate 12/12, broker same-connection multi-submit 2/2, and invariant Gates 1–6. The phase-local deny/block/empty cases use a test-only loop mirror, so the production-worker backstop is Phase 51's retained real-Linux acceptance execution: its scoped log hash matches `51-LIVE-EVIDENCE.md`, records LIVE-07/LIVE-08 green (4/4), and exercises the real CLI and worker through sequential submissions, opaque handle forwarding, stop/hold behavior, and a valid audit chain. A direct rerun on this managed host reached environment-only abstract-UDS/DNS/mock-egress failures; Docker is unavailable. Per explicit user approval, the retained Linux evidence is authoritative for this production-path coverage.
