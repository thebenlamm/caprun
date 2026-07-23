---
phase: 34
slug: regression-live-proof-v1-7-done
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 34 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `34-RESEARCH.md` § Validation Architecture. All security claims are
> **Linux-only** — the authoritative signal is `scripts/mailpit-verify.sh`
> (unprivileged rust:1 Colima container), true-exit-0 captured **before any pipe**.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust workspace, edition 2021) + `check-invariants.sh` grep gate |
| **Config file** | Cargo workspace at repo root (`resolver = "3"`) |
| **Quick run command** | `cargo build --workspace && ./scripts/check-invariants.sh` (macOS host — compiles no cfg(linux) targets) |
| **Full suite command** | `bash scripts/mailpit-verify.sh` (real Linux; default `MAILPIT_VERIFY_CMD='cargo test --workspace --no-fail-fast'`) |
| **Estimated runtime** | ~5–12 min (Linux container full suite) |

---

## Sampling Rate

- **After every task commit:** `cargo build --workspace` + `./scripts/check-invariants.sh` (host — Gate 1 no raw `EffectRequest`, Gate 3 mint-site list unchanged).
- **After the EXEC-05 TCB slice (before the live proof):** the MANDATORY Linux compile-check — `MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going' bash scripts/mailpit-verify.sh`, true-exit-0 before any pipe (guards cfg(linux) test-blindness).
- **After every plan wave:** scope the Linux run to the new/affected tests via `MAILPIT_VERIFY_CMD`.
- **Before milestone close:** full-workspace regression green on real Linux, asserted on **counts + named tests** (never `script | tail`).
- **Max feedback latency:** ~12 min (full Linux suite).

---

## Per-Task Verification Map

> Task IDs are provisional (filled by the planner). Requirement→test mapping is fixed.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 34-EXEC05-impl | EXEC-05 | 1 | EXEC-05 | I2 / confirm-release | Blocked `process.exec` released by `caprun confirm` runs exactly once; output taint-minted (non-stapled), audit chained onto `confirm_granted` head; `verify_chain` true | integration (cfg linux) | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_process_exec_block <exec_confirm_release_test>' bash scripts/mailpit-verify.sh` | ❌ W0 | ⬜ pending |
| 34-EXEC05-guard | EXEC-05 | 1 | EXEC-05 | fail-closed | Any still-un-dispatchable sink stays fail-closed-recoverable (P33 entry-guard not regressed OPEN) | integration (cfg linux) | (same suite, entry-guard leg) | ❌ W0 | ⬜ pending |
| 34-gate-compile | — | 2 | EXEC-05 | cfg(linux) blindness | `cargo build --tests --workspace --keep-going` compiles all cfg(linux) targets, true-exit-0 | compile gate | `MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going' bash scripts/mailpit-verify.sh` | ✅ | ⬜ pending |
| 34-gate-adversarial | — | 2 | EXEC-05 | TCB diff | Fresh non-self Fable-5 adversarial code-trace of the confirm-release diff → APPROVED or findings resolved (orchestrator-owned) | manual/agent | (orchestrator spawns Fable-5) | ✅ | ⬜ pending |
| 34-LIVE01 | LIVE-01 | 3 | LIVE-01 | I2 composed | Composed run: tainted exec-output→sink Blocked (verify_chain true), clean exec/fs Allowed, fs write/edit within WorkspaceRoot audited, EXEC-05 release exercised | integration (cfg linux) | `bash scripts/mailpit-verify.sh` (or exec-scoped `MAILPIT_VERIFY_CMD`) | ❌ W0 | ⬜ pending |
| 34-LIVE02 | LIVE-02 | 3 | LIVE-02 | regression | Full-workspace green, no regression to v1.0–v1.6, asserted on counts + named tests; dedicated negative test per new sink | integration (cfg linux) | `bash scripts/mailpit-verify.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] EXEC-05 acceptance test — recommend **extending** `crates/caprun/tests/s9_process_exec_block.rs` (per research open Q1) with a confirm-release leg + entry-guard fail-closed leg, mirroring P33's `s9_file_write_block` + confirm-release leg.
- [ ] Per-new-sink negative tests present (process.exec, fs write/edit) for LIVE-02.

*Existing infrastructure (`mailpit-verify.sh`, `s9_*` suites, `confirm.rs` cross-process harness, `live_acceptance_v1_4_composed.rs` shared-audit.db pattern) covers the rest.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Fresh Fable-5 adversarial code-trace of the confirm-release TCB diff | EXEC-05 | Standing project guardrail — an independent non-self review has caught real MAJORs a passing verifier + green gates missed; must be a fresh context | Orchestrator (not a gsd-executor) spawns a Fable-5 agent with ONLY the confirm-release diff; APPROVED or findings resolved before LIVE-01 is authorized |
| Human DONE sign-off | v1.7 close | Milestone-close policy (v1.5/v1.6 precedent) | Operator confirms DONE before the milestone is marked; not pushed unless explicitly requested |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 720s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
