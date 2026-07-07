---
phase: 11
slug: live-acceptance-tainted-session-human-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-07
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Cargo's built-in test harness (`cargo test`), Rust 1.92 |
| **Config file** | none — test targets are auto-discovered `.rs` files under `tests/` per crate (no `autotests = false` anywhere in the workspace) |
| **Quick run command** | `cargo test -p caprun --test live_acceptance_tainted_session` (macOS — only the cross-platform guard test runs; Linux bodies are `#[cfg(target_os = "linux")]`-excluded) |
| **Full suite command** | `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test live_acceptance_tainted_session` (Colima+Docker — the only way to exercise the real Linux-only assertions) |
| **Estimated runtime** | ~10-30s quick (macOS compile-check only); ~2-5 min full (Colima/Docker cold start + confinement stack) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p caprun --test live_acceptance_tainted_session` (macOS quick check — proves the file compiles and is wired in; does NOT exercise the live assertions)
- **After every plan wave:** Run the Colima+Docker full suite command — this is the ONLY way to observe the real Linux-only assertions, including whether Pitfall 1's stale `s9_live_block.rs` assertion needs fixing
- **Before `/gsd-verify-work`:** The Colima+Docker full suite must be green at least once — do not declare the phase done from a macOS-only compile check (see Pitfall 4 in RESEARCH.md)
- **Max feedback latency:** ~5 min (bounded by Colima/Docker cold start)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | ACC-01 | — | Deny path: hostile read → I1 demotion → I2 block → `caprun deny` → no effect proceeds, ever | integration (Linux-gated) | `cargo test -p caprun --test live_acceptance_tainted_session live_acceptance_deny_path` (under Colima/Docker) | ❌ W0 | ⬜ pending |
| 11-01-02 | 01 | 1 | ACC-02 | — | Confirm path: same scenario, `caprun confirm` → effect proceeds exactly once | integration (Linux-gated) | `cargo test -p caprun --test live_acceptance_tainted_session live_acceptance_confirm_path` (under Colima/Docker) | ❌ W0 | ⬜ pending |
| 11-01-03 | 01 | 1 | ACC-03 | — | Audit DAG unbroken causal chain (`fd_granted → file_read → session_demoted → sink_blocked → confirm_granted/confirm_denied`), proven via `parent_id` walk + `verify_chain()` | assertions within both tests above | Same commands as ACC-01/02 | ❌ W0 | ⬜ pending |
| 11-01-04 | 01 | 1 | ACC-03 (regression guard) | — | Fix `s9_live_block.rs`'s stale `blocked.parent_id == Some(file_read.id)` assertion to `Some(session_demoted.id)` (Pitfall 1) | integration (Linux-gated) | `cargo test -p caprun --test s9_live_block` (under Colima/Docker) | ✅ (existing file, one-line fix) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cli/caprun/tests/live_acceptance_tainted_session.rs` — new file, covers ACC-01/02/03 (two `#[test]` functions: deny path, confirm path). No `Cargo.toml` change needed — Cargo auto-discovers `.rs` files under `tests/`.
- [ ] No framework install needed — `cargo test` is already the project's test runner.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Actually running the Colima+Docker recipe at least once before declaring the phase done | ACC-01, ACC-02, ACC-03 | `#[cfg(target_os = "linux")]` bodies are compile-excluded on macOS — no automated CI/local signal proves they pass without an explicit Docker invocation (see Pitfall 1 and Pitfall 4: the existing `s9_live_block.rs` assertion has been silently stale since Phase 9 for exactly this reason) | Run `colima start` (if not running) then the full suite Docker command above; capture stdout/exit codes for the milestone acceptance record (D-06) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
