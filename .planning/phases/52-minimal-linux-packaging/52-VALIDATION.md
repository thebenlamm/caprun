---
phase: 52
slug: minimal-linux-packaging
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: validated
nyquist_compliant: true
wave_0_complete: true
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

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 52-01-T1/T2 | 01 | 1 | PKG-01 | T-52-01..07 | Release build installs exactly three executable sibling binaries; destination/help/error paths fail or succeed explicitly | integration/smoke | `bash -n scripts/install-linux.sh && INSTALL_DEST="$(mktemp -d)" bash scripts/install-linux.sh` plus exact-directory and CLI assertions | ✅ exists | ✅ green |
| 52-01-T3 | 01 | 1 | PKG-01 | T-52-08 | Linux install walkthrough gives the manual equivalent and warns that `cargo install --path cli/caprun` is insufficient | doc behavior | `grep` assertions from `52-01-PLAN.md` | ✅ exists | ✅ green |
| 52-02-T1/T2 | 02 | 1 | PKG-01 | credential disclosure | `CAPRUN_*`, policy precedence, and GitHub grant/credential checklist match source names | doc/source integration | bidirectional source/document checks from `52-02-PLAN.md` | ✅ exists | ✅ green |
| 52-03-T1/T2 | 03 | 2 | PKG-01 | framing drift | README points to the installer and names the three-binary layout; cross-doc and invariant gates remain green | doc/source integration | `grep` cross-document checks plus `./scripts/check-invariants.sh` | ✅ exists | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `scripts/install-linux.sh` — primary artifact exists and was executed behaviorally
- [x] No test framework install needed — `bash -n`, `cargo`, and `check-invariants.sh` already exist

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Install path works on a clean design-partner Linux host | PKG-01 | Requires a host that is not the dev box; a clean-machine install cannot be simulated in-repo | Provision a fresh Ubuntu 24.04 host (kernel ≥ 5.13 for Landlock), clone the repo, run the documented install path, confirm all three binaries land as siblings and `caprun --help` runs |
| Documentation accuracy for a first-time reader | PKG-01 | Prose clarity is not machine-checkable | Have a reader unfamiliar with the repo follow the install doc top-to-bottom without consulting source |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s on a warm release build
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** PKG-01's install artifact, exact sibling layout, operator-facing failure paths, documentation links/checklist, and invariant gates are green. The clean-host and first-time-reader checks remain explicitly manual-only and were accepted during phase verification.

## Validation Audit 2026-08-11

| Metric | Count |
|--------|-------|
| Requirements audited | 1 |
| Covered | 1 |
| Partial | 0 |
| Missing | 0 |
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

A current behavioral run passed shell syntax, a real warm release build, and installation
into a fresh temporary destination containing exactly `caprun`, `caprun-worker`, and
`caprun-exec-launcher`, all executable. `--help`, bogus-flag failure, install/configuration
documentation assertions, README linkage, and invariant Gates 1–6 also passed. The first
combined audit command initially redirected only stdout for `--help`; because the script
writes usage to stderr, the audit harness was corrected and rerun without weakening any
product assertion.
