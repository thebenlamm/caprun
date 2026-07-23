---
phase: 5
slug: runtime-spine-live-9-email-block
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-30
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust, edition 2021, workspace `resolver = "3"`) |
| **Config file** | `Cargo.toml` (workspace root) — none to install |
| **Quick run command** | `cargo test -p brokerd --no-fail-fast` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` |
| **Estimated runtime** | ~60–120 seconds (cross-platform logic tests); Linux enforcement suite via Colima adds ~2–4 min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <crate touched> --no-fail-fast`
- **After every plan wave:** Run `cargo test --workspace --no-fail-fast` + `./scripts/check-invariants.sh`
- **Before `/gsd-verify-work`:** Full suite must be green AND the §9 acceptance test (`crates/brokerd/tests/s9_acceptance.rs`) must still pass
- **Max feedback latency:** ~120 seconds (cross-platform); Linux-gated enforcement assertions are verified in the Colima/Docker recipe and are NOT part of the per-commit loop

---

## Per-Task Verification Map

> The planner fills exact task IDs/commands. Below are the per-criterion test anchors derived from RESEARCH.md `## Validation Architecture`. Most Phase 5 work is cross-platform Rust logic (dispatch wiring, session-scoped state, IPC enum, audit append) and is testable on the macOS dev box. Only confinement-dependent end-to-end assertions are Linux-gated.

| Criterion | Requirement | Secure Behavior | Test Type | Automated Command | Platform |
|-----------|-------------|-----------------|-----------|-------------------|----------|
| SC1 — single dispatch | ASM-01 | RequestFd/ReportRead/mint/evaluate/audit/sink all route through `brokerd::server` dispatch; no second loop | unit + grep invariant | `cargo test -p brokerd` + `./scripts/check-invariants.sh` | cross-platform |
| SC2 — placeholder gone | ASM-02 | `"SubmitPlanNode not wired"` string absent; `executor::submit_plan_node` runs live broker path | grep assertion + unit | `! grep -rn "SubmitPlanNode not wired" crates/ cli/` + `cargo test -p brokerd` | cross-platform |
| SC3 — typed `ReportClaims` | ASM-03 | `WorkerClaim::EmailAddress` round-trips; raw source bytes never cross planner boundary; unknown claim kinds fail closed | unit (serde round-trip + fail-closed) | `cargo test -p brokerd report_claims` | cross-platform |
| SC4 — `mint_from_read` anchoring | ASM-04 | minted `ValueId` provenance anchors to the real `file_read` event id in the audit DAG | unit (DAG edge assertion) | `cargo test -p brokerd mint_from_read` | cross-platform |
| SC5 — session-scoped handles | HARD-03 | handle minted in session A denied in session B; request-supplied `session_id` never trusted | unit (cross-session denial) | `cargo test -p brokerd session_scope` | cross-platform |
| SC6 — durable `sink_blocked` | ACC-02 | real `caprun` run on hostile input → `sink_blocked` with causal parent, append-failure fails closed, CLI exits non-success before any effect, durable before return | e2e (real binary run) | `cargo test -p caprun` live-block test (+ Linux confinement assertions in Colima recipe) | logic: cross-platform · confinement: Linux-gated |

*Status legend: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- Existing `cargo test` infrastructure covers all phase requirements — no framework install needed.
- New test files the planner is expected to scaffold (names indicative):
  - [ ] `crates/brokerd/tests/` — `ReportClaims` IPC round-trip + fail-closed-on-unknown-variant
  - [ ] `crates/brokerd/tests/` — session-scoped handle cross-session denial
  - [ ] `crates/brokerd/tests/` — `sink_blocked` durable-append + causal-parent + fail-closed-on-append-error
  - [ ] a live `caprun` CLI block test (hostile input → non-success exit, no effect executed)
- **Invariant:** `crates/brokerd/tests/s9_acceptance.rs` must remain green throughout (it calls `executor::submit_plan_node` / `mint_from_read` directly and is independent of the IPC protocol changes).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Full kernel-confined live block (worker self-confines, only egress is broker-mediated plan nodes) | ACC-02 / HARD-03 | Landlock + seccomp enforcement is Linux-only; macOS paths are `#[cfg(not(target_os="linux"))]` no-op stubs | Run the Colima/Docker recipe from CLAUDE.md (`docker run ... rust:1 cargo test --workspace --no-fail-fast`); confirm the live block test asserts non-success exit and an empty effect outcome under real confinement |

*Cross-platform logic for every criterion is automated; only the confinement-gated end-to-end assertion is verified in the Linux recipe.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-30 (plan-checker confirmed all sign-off conditions met)
