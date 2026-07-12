---
phase: 28
slug: authenticated-audit-chain
status: draft
nyquist_compliant: false
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
| TBD | — | — | HARDEN-02 | §b | Self-consistent-rewrite forgery → `verify_chain` returns false | unit | `cargo test -p brokerd` | ❌ W0 | ⬜ pending |
| TBD | — | — | HARDEN-02 | §b (D-04) | Tail-truncation → anchor/`event_count` mismatch → false | unit | `cargo test -p brokerd` | ❌ W0 | ⬜ pending |
| TBD | — | — | HARDEN-02 | §b (F1) | Audit DB/key under workspace root → broker refuses to start | unit | `cargo test -p brokerd` | ❌ W0 | ⬜ pending |
| TBD | — | — | HARDEN-02 | §b (A2) | Cross-process `confirm`/`deny` still verifies untampered chain (no false positive) | integration | `bash scripts/mailpit-verify.sh` | ❌ W0 | ⬜ pending |

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
