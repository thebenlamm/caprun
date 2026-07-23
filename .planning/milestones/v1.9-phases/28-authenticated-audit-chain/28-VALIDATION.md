---
phase: 28
slug: authenticated-audit-chain
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-12
---

# Phase 28 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust workspace) + Linux-only security tests via `scripts/mailpit-verify.sh` |
| **Config file** | none — workspace `Cargo.toml` |
| **Quick run command** | `cargo test -p brokerd` |
| **Full suite command** | `bash scripts/mailpit-verify.sh` (Linux enforcement + e2e) |
| **Estimated runtime** | ~30s quick (macOS unit) / ~5–8 min full (Colima+Docker Linux) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p brokerd` (macOS unit + host-side)
- **After every plan wave:** Run `MAILPIT_VERIFY_CMD='cargo test --workspace --no-fail-fast' bash scripts/mailpit-verify.sh`
- **Before `/gsd-verify-work`:** Full Linux suite must be green (all `#[cfg(target_os = "linux")]` enforcement + negative-assertion tests)
- **Max feedback latency:** ~30 seconds (macOS unit tests; Linux gates run at wave boundaries)

---

## Per-Task Verification Map

> Populated by the planner from RESEARCH.md `## Validation Architecture`. Each Phase 28 success
> criterion maps to at least one Linux-only or host-side test.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 | 28-01 | 1 | HARDEN-02 | §b (F1) | 7 fixtures relocated to F1-safe layout; macOS suite unchanged (no-op) | regression | `cargo test --workspace --no-fail-fast` | ❌ W0→green | ⬜ pending |
| 01-T2 | 28-01 | 1 | HARDEN-02 | T-28-SC | hmac 0.12.1 / getrandom 0.4 added; brokerd builds | build | `cargo build -p brokerd` | ❌ W0→green | ⬜ pending |
| 02-T2 | 28-02 | 2 | HARDEN-02 | §b (F1) | Audit DB/key under workspace root → `load_or_create_key` Err (fail-closed), no key written; :memory: ephemeral; cross-process key reuse idempotent | unit | `cargo test -p caprun --lib -- key::` | ❌ W0 | ⬜ pending |
| 03-T2 | 28-03 | 3 | HARDEN-02 (SC1) | §b | Self-consistent-rewrite forgery WITHOUT the key → `verify_chain` false | unit | `cargo test -p brokerd -- self_consistent_forgery_without_key_is_rejected` | ❌ W0 | ⬜ pending |
| 03-T2 | 28-03 | 3 | HARDEN-02 (SC2) | §b | Key-dependence: different keys → different MACs; wrong key → false on untampered chain | unit | `cargo test -p brokerd -- verify_chain_is_key_dependent` | ❌ W0 | ⬜ pending |
| 04-T2 | 28-04 | 4 | HARDEN-02 | §b (D-04) | Tail-truncation → anchor/`event_count` mismatch → false | unit | `cargo test -p brokerd -- tail_truncation_detected_via_anchor_mismatch` | ❌ W0 | ⬜ pending |
| 04-T2 | 28-04 | 4 | HARDEN-02 | §b (migration) | Legacy DB with no anchor row → `verify_chain` false (fail-closed) | unit | `cargo test -p brokerd -- legacy_db_without_anchor_fails_closed` | ❌ W0 | ⬜ pending |
| 05-T3 | 28-05 | 5 | HARDEN-02 | §f X-02 | Flip-back Denied→Pending via raw SQL caught by pending-row MAC; deny() gate fails closed | unit | `cargo test -p brokerd -- flip_back_denied_to_pending_caught_by_mac deny_fails_closed_on_tampered_state` | ❌ W0 | ⬜ pending |
| existing | 28-05 | 5 | HARDEN-02 (SC3/A2) | §b (A2) | Cross-process `confirm`/`deny` still verifies untampered chain (no false positive) — existing confirm.rs cross-process suite | integration | `bash scripts/mailpit-verify.sh` | ✅ existing | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Migrate existing live-test fixtures that co-locate `audit.db` under the workspace root to an out-of-root layout (7 of 8 fixtures per RESEARCH.md) — precondition for the F1 refusal test.
- [ ] `hmac`/`getrandom` deps added to `crates/brokerd/Cargo.toml` (compile-verified in RESEARCH).

*Otherwise: existing `cargo test` + `mailpit-verify.sh` infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | — | — |

*All Phase 28 behaviors have automated verification (unit + Linux integration).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (fixture migration, deps)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s (macOS unit)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
