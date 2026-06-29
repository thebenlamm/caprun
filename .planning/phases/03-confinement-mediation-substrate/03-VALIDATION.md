---
phase: 3
slug: confinement-mediation-substrate
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-06-29
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Test map ported from 03-RESEARCH.md §Validation Architecture and the
> 03-0N-PLAN.md task `<verify>` blocks.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test runner (`cargo test`) — ships with the toolchain, no install |
| **Config file** | `Cargo.toml` per crate (`[[test]]` sections) |
| **Quick run command** | `cargo test --workspace --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | quick ~5s (macOS) · full ~30s (Linux CI, incl. confinement integration) |

> **Dev/CI split:** macOS runs all non-Linux-gated tests; `#[cfg(target_os = "linux")]`
> confinement/negative-assertion/e2e tests run only on Linux CI (GitHub Actions
> ubuntu ≥ 22.04). See 03-RESEARCH.md §Environment Availability.

---

## Sampling Rate

- **After every task commit:** Run `cargo test --workspace --lib` (< 5s, green on macOS)
- **After every plan wave:** Run `cargo test --workspace` (Linux CI for confinement tests)
- **Before `/gsd-verify-work`:** Full suite must be green on Linux CI
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01a | 01 | 0/1 | REQ-sandbox, REQ-brokerd-core, REQ-adapters-fs, REQ-substrate-demo | T-03-SC | New cargo deps enter TCB only at pinned, audited versions; no forbidden raw-effect token | unit (manifest resolve) | `cargo metadata --no-deps --format-version 1 >/dev/null && bash scripts/check-invariants.sh` | ❌ W0 | ⬜ pending |
| 03-01-01b | 01 | 0/1 | REQ-sandbox, REQ-brokerd-core, REQ-adapters-fs, REQ-substrate-demo | T-03-01 | macOS stubs no-op without false enforcement assurance; Phase 1 brokerd stub preserved | unit | `cargo build --workspace && cargo test --workspace --lib` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 0/1 | REQ-sandbox | T-03-02 / T-03-06 | seccompiler 0.5.0 deny-execve + deny-socket(AF_INET) loads after NO_NEW_PRIVS and actually blocks (EPERM) | integration (Linux-only) | `cargo test -p sandbox --test api_spike` | ❌ W0 | ⬜ pending |
| 03-01-03 | 01 | 0/1 | REQ-brokerd-core | T-03-02 | abstract-UDS bind → from_std → accept framed round-trip works in tokio (or documented path fallback) | integration (Linux-only) | `cargo test -p brokerd --test uds_abstract_spike` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 2 | REQ-sandbox | T-03-03 / T-03-04 / T-03-05 / T-03-06 | rlimits + Landlock + seccomp confinement stack applies; confine-probe builds | unit + build | `cargo test -p sandbox --lib && cargo build -p sandbox --bin confine-probe` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 2 | REQ-sandbox | T-03-03 / T-03-04 / T-03-05 | Confined worker cannot read `~/.ssh/id_rsa`, cannot open TCP to 1.1.1.1:80, cannot exec `/bin/ls` | integration (Linux-only) | `cargo test -p sandbox --test confinement_integration` | ❌ W0 | ⬜ pending |
| 03-03-01 | 03 | 2 | REQ-brokerd-core | T-03-07 | Audit Event appended with valid hash-chain (parent_hash links); chain verification detects tampering | integration | `cargo test -p brokerd --test audit_dag` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 2 | REQ-brokerd-core | T-03-08 / T-03-09 / T-03-10 | `CreateSession` → Session row in SQLite; UDS server accepts + responds; bounded/non-predictable session | integration | `cargo test -p brokerd --test uds_ipc` | ❌ W0 | ⬜ pending |
| 03-04-01 | 04 | 2 | REQ-adapters-fs | T-03-12 / T-03-13 | pass_fd/recv_fd + protocol types compile and pass unit checks | unit + build | `cargo build -p adapter-fs && cargo test -p adapter-fs --lib` | ❌ W0 | ⬜ pending |
| 03-04-02 | 04 | 2 | REQ-adapters-fs | T-03-11 / T-03-12 | Broker passes fd; worker reads via fd not open(); recv_fd sets O_CLOEXEC | integration | `cargo test -p adapter-fs --test fd_pass` | ❌ W0 | ⬜ pending |
| 03-05-01 | 05 | 3 | REQ-substrate-demo | T-03-15 / T-03-17 | caprun + caprun-worker binaries build (self-confining reader + broker/fd-pass orchestrator) | build | `cargo build -p caprun --bins` | ❌ W0 | ⬜ pending |
| 03-05-02 | 05 | 3 | REQ-substrate-demo | T-03-14 / T-03-16 | End-to-end: confined worker reads file via fd, read Event in DAG; DAG hash chain unbroken | integration (Linux-only) | `cargo test -p caprun --test e2e` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*File Exists: ❌ W0 = file is created in Wave 0 (Plan 01); flips to ✅ once scaffolded.*

---

## Wave 0 Requirements

Wave 0 = Plan 01. It delivers the compiling skeleton, the two highest-risk API
spikes, and all downstream test scaffold files so Wave 2/3 build on verified facts.

- [ ] Workspace dependency pins wired + five Cargo.toml manifests resolve (Task 1a)
- [ ] `crates/sandbox/tests/confinement_integration.rs` — negative assertions (Linux-gated, `#[ignore]` placeholder)
- [ ] `crates/brokerd/tests/uds_ipc.rs` — UDS accept + CreateSession round-trip (`#[ignore]` placeholder)
- [ ] `crates/brokerd/tests/audit_dag.rs` — hash-chain verification (`#[ignore]` placeholder)
- [ ] `crates/adapter-fs/tests/fd_pass.rs` — SCM_RIGHTS round-trip (`#[ignore]` placeholder)
- [ ] `cli/caprun/tests/e2e.rs` — substrate demo end-to-end (Linux-gated, `#[ignore]` placeholder)
- [ ] `crates/sandbox/tests/api_spike.rs` — seccompiler 0.5.0 deny-rule + nix prctl spike PROVEN on Linux (Task 2)
- [ ] `crates/brokerd/tests/uds_abstract_spike.rs` — abstract-UDS bind/from_std/accept spike PROVEN on Linux (Task 3)
- [ ] Framework install: already available (`cargo test` ships with Rust)

*Wave 0 completes at runtime when Plan 01 executes; `wave_0_complete: false` until then.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | — | — |

*All phase behaviors have automated verification (cargo test, Linux-gated where confinement requires the kernel).*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (all test files created in Plan 01)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-29
