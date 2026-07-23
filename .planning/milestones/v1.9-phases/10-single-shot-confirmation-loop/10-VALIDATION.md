---
phase: 10
slug: single-shot-confirmation-loop
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-06
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `cargo test` (no external test framework; no config file) |
| **Config file** | none — `Cargo.toml` `[[test]]` targets per crate (see `cli/caprun/Cargo.toml`'s existing `e2e`/`planner` targets) |
| **Quick run command** | `cargo test -p brokerd confirmation` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` |
| **Estimated runtime** | ~30 seconds (quick), ~2-4 minutes (full workspace) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p brokerd confirmation` (or the relevant crate's fast subset)
- **After every plan wave:** Run `cargo test --workspace --no-fail-fast`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 10-01-xx | 01 | 0 | CONFIRM-01 | — | `caprun confirm <effect_id>` displays verbatim literal + provenance | unit + integration | `cargo test -p brokerd confirmation` / `cargo test -p caprun --test confirm` | ❌ W0 | ⬜ pending |
| 10-01-xx | 01 | 0 | CONFIRM-02 | — | Confirm releases exactly one `(sink, arg, literal-digest)` triple; no standing policy created | unit | `cargo test -p brokerd confirmation` | ❌ W0 | ⬜ pending |
| 10-01-xx | 01 | 0 | CONFIRM-03 | — | Deny is durable; same `effect_id` cannot later be confirmed | integration (cross-process, real binary) | `cargo test -p caprun --test confirm` | ❌ W0 | ⬜ pending |
| 10-01-xx | 01 | 0 | CONFIRM-04 | — | confirm/deny audited, anchored to `effect_id` via `parent_id`, executes in TCB | unit | `cargo test -p brokerd confirmation` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/brokerd/src/confirmation.rs` — new module: `PendingConfirmation`, `ResolvedArg`, `PendingConfirmationState`, `insert_pending_confirmation`, `find_pending_confirmation`, `transition_state`, `confirm`, `deny`
- [ ] `crates/brokerd/src/audit.rs` — `pending_confirmations` DDL addition (extend `SCHEMA_DDL`)
- [ ] `crates/brokerd/src/sinks/file_create.rs` — `invoke_file_create_from_resolved` (frozen-snapshot re-invocation)
- [ ] `cli/caprun/tests/confirm.rs` — new integration test target driving the real binary as a subprocess (cross-process durability for CONFIRM-03 cannot be honestly tested within one process)
- [ ] `cli/caprun/Cargo.toml` — add `[[test]]` target entry for `confirm.rs`

---

## Manual-Only Verifications

*All phase behaviors have automated verification. Linux-only enforcement/e2e tests (Landlock/seccomp) remain deferred to the live acceptance run per this project's existing convention (matches Phase 9's split) — those are exercised via the Colima+Docker recipe, not manual inspection.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
