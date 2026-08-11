---
phase: 52
slug: minimal-linux-packaging
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-11
---

# Phase 52 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `bash -n` (shell syntax) + `cargo test` (Rust workspace) + `scripts/check-invariants.sh` (architectural gate) |
| **Config file** | none — existing workspace `Cargo.toml` + `scripts/` cover this phase |
| **Quick run command** | `bash -n scripts/install-linux.sh && ./scripts/check-invariants.sh` |
| **Full suite command** | `cargo build --workspace --release && ./scripts/check-invariants.sh` |
| **Estimated runtime** | ~5 seconds (quick) / ~2–4 minutes (release build) |

**Platform note (from CLAUDE.md):** the Linux security tests (Landlock/seccomp/e2e)
require Docker + EC2 and **cannot** run on this dev box. Everything listed above
runs locally without Docker. This phase is docs + a thin shell script, so its own
deliverables are fully validatable locally; no EC2 provisioning is required to
verify Phase 52.

---

## Sampling Rate

- **After every task commit:** Run `bash -n scripts/install-linux.sh` (once the script exists) and `./scripts/check-invariants.sh`
- **After every plan wave:** Run `./scripts/check-invariants.sh` plus a dry-run install into a temp prefix
- **Before `/gsd-verify-work`:** Release build green and install script verified to co-locate all three binaries
- **Max feedback latency:** 10 seconds for the quick command

---

## Per-Task Verification Map

*Populated by `/gsd-validate-phase` after plans are written. Seeded here as draft.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | PKG-01 | TBD | TBD | integration | `bash -n scripts/install-linux.sh` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `scripts/install-linux.sh` — the install script itself is the primary artifact under test
- [ ] No test framework install needed — `bash -n`, `cargo`, and `check-invariants.sh` already exist

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Install path works on a clean design-partner Linux host | PKG-01 | Requires a host that is not the dev box; a clean-machine install cannot be simulated in-repo | Provision a fresh Ubuntu 24.04 host (kernel ≥ 5.13 for Landlock), clone the repo, run the documented install path, confirm all three binaries land as siblings and `caprun --help` runs |
| Documentation accuracy for a first-time reader | PKG-01 | Prose clarity is not machine-checkable | Have a reader unfamiliar with the repo follow the install doc top-to-bottom without consulting source |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
