---
phase: 33
slug: filesystem-read-write-breadth
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-17
---

# Phase 33 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **TCB implementation phase.** Real Rust code lands in
> `crates/{adapter-fs,brokerd,executor}`. Enforcement is **Linux-only**
> (`#[cfg(target_os="linux")]`); the dev machine is a Mac. **Mac `cargo test`
> compiles ZERO Linux test targets** — a Mac-green build can hide broken Linux
> sites (the project's `cfg-linux-test-blindness` lesson: 279/279 macOS-green
> once hid 8 broken sites). Therefore this phase's validation has a MANDATORY
> Linux compile-check step (`cargo build --tests --keep-going` in the rust:1
> Colima container), and the full live acceptance is Phase 34.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust workspace, `resolver = "3"`) |
| **Config file** | none — workspace `Cargo.toml` at repo root |
| **Quick run command** | `cargo test -p adapter-fs -p executor --no-fail-fast` (macOS-fast, no Linux gates) |
| **Full suite command** | `bash scripts/mailpit-verify.sh` (real Linux via Colima/Docker; `cargo test --workspace --no-fail-fast`) |
| **Linux compile-check** | `MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going' bash scripts/mailpit-verify.sh` (enumerate `#[cfg(target_os="linux")]` callers) |
| **Estimated runtime** | ~30s macOS quick · ~6–10 min full Linux gate |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <touched crate> --no-fail-fast` (macOS quick)
- **After every plan wave:** Run the macOS quick suite; TCB-signature-changing waves ALSO run the Linux compile-check
- **Before Phase 34 live proof:** Full `scripts/mailpit-verify.sh` must be green on real Linux
- **Max feedback latency:** ~30s macOS · full Linux gate reserved for wave boundaries + Phase 34

---

## Per-Task Verification Map

*Populated by the planner from the RESEARCH.md `## Validation Architecture` section — one row per task, each FS-01/02/03 requirement mapped to a named test target. Provisional test files: `crates/adapter-fs/src/workspace.rs` unit tests (new `O_WRONLY|O_TRUNC` negative set) and `crates/brokerd/tests/file_write_*.rs` (write/edit-under-I2 + read-breadth coverage). A dedicated negative test per new sink is required by LIVE-02, not optional.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| _TBD by planner_ | | | FS-01/02/03 | | | unit / integration | | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] New `O_WRONLY|O_TRUNC` negative-assertion tests in `crates/adapter-fs/src/workspace.rs` — absolute-path rejected, `..`-traversal rejected, symlink-escape rejected, PLUS `ENOENT`-on-missing-target (NOT inherited from existing read/create negative tests — DESIGN §3.2).
- [ ] `crates/brokerd/tests/file_write_*.rs` — write/edit-under-I2 Block (tainted path/contents) + fs read-breadth `RequestFd` count-limiter Deny.
- [ ] Dedicated negative test per new sink (LIVE-02).

*Existing `cargo test` infrastructure covers the harness; the above are the new stubs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Genuine (non-stapled) taint-chain Block end-to-end on real Linux | FS-03 | Requires the confined worker + broker live path; deferred to Phase 34 composed acceptance | Phase 34 `scripts/mailpit-verify.sh` or exec-scoped equivalent, true-exit-before-pipe |

*Per-task automated coverage exists for FS-01/02/03; the full live composed proof is Phase 34 by design (mirrors the milestone's design-gate → implement → live-proof split).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (new `O_WRONLY|O_TRUNC` negative set)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s (macOS quick)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
