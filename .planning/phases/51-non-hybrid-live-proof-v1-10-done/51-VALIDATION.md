---
phase: 51
slug: non-hybrid-live-proof-v1-10-done
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-29
---

# Phase 51 — Validation Strategy

> Per-phase validation contract for non-hybrid LIVE proof (LIVE-07, LIVE-08) — v1.10 DONE gate.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (workspace), bin crate `caprun`, lib tests in `brokerd`/`executor` |
| **Config file** | workspace `Cargo.toml` (no jest/pytest) |
| **Quick run command** | `./scripts/check-invariants.sh && cargo test -p caprun --test coding_cli --test stream_hold --test stream_substrate --test planner -- --test-threads=1` |
| **Full suite command** | `./scripts/check-invariants.sh && COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test --workspace --no-fail-fast --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh` |
| **Scoped LIVE command** | `COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_10_cli --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh` |
| **Estimated runtime** | quick ~30–120s; scoped LIVE several minutes; full compose-verify longer |

---

## Sampling Rate

- **After every task commit:** `./scripts/check-invariants.sh` + host-safe caprun tests (`coding_cli` / `stream_hold` / `stream_substrate` / `planner`)
- **After every plan wave:** host-safe suite + (when Docker available) scoped `live_acceptance_v1_10_cli` via compose-verify
- **Before `/gsd-verify-work`:** full compose-verify workspace green + invariants + framing honesty review
- **Max feedback latency:** ~120s focused host; LIVE scoped runs several minutes

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 51-01-T* | 51-01 | 1 | LIVE-07 | T-51-01 hybrid overclaim | Real CLI multi-node SUCCESS one Session; not hybrid | e2e Linux+mock-egress-ca | `cargo test -p caprun --test live_acceptance_v1_10_cli live_07 --features mock-egress-ca` | ✅ exists | ⚠️ compose pending |
| 51-01-T* | 51-01 | 1 | LIVE-07 | T-51-01 | `caprun audit` Chain verification PASSED; verify_chain | e2e | same test asserts audit subprocess | ✅ exists | ⚠️ compose pending |
| 51-01-T* | 51-01 | 1 | LIVE-07 | T-51-01 | Framing: CLI-driven not hybrid | unit/e2e | framing asserts + module doc pin | ✅ exists | ⚠️ compose pending |
| 51-01-T* | 51-01 | 1 | LIVE-07 | — | Host guard binary present | unit host | `live_acceptance_v1_10_cli_guard_present` | ✅ exists | ⚠️ host link blocked |
| 51-02-T* | 51-02 | 2 | LIVE-08 | T-51-02 launder / T-51-03 vacuity | Mid-loop I2 Block via genuine bag taint under permitted sink | e2e Linux+mock-egress-ca | `… live_08 …` | ✅ exists | ⚠️ compose pending |
| 51-02-T* | 51-02 | 2 | LIVE-08 | T-51-03 | policy_deny is not what fired; no effect of blocked node | e2e | assert permitting policy + sink_blocked + no github_pr_succeeded | ✅ exists | ⚠️ compose pending |
| 51-02-T* | 51-02 | 2 | LIVE-08 | T-51-04 stapled | process_exited precedes Block; verify_chain true after Block | e2e | real CLI + audit subprocess | ✅ exists | ⚠️ compose pending |
| 51-*-reg | 51-02 | 2 | LIVE-07/08 | — | No v1.0–v1.9 regression | full workspace | compose-verify default full suite | ✅ harness | ⬜ pending |
| 51-*-hyg | * | * | HYG | Gate 1/3 | check-invariants Gates 1–6 | script | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |
| 51-*-p50 | * | * | regression | — | Phase 50 coding argv / hold / planner | unit | `cargo test -p caprun --test coding_cli --test stream_hold --test stream_substrate --test planner` | ✅ | ⬜ pending |
| 51-*-v19 | * | * | regression | T-51-01 | v1.9 hybrid still green (not DONE claim) | e2e Linux | `… --test live_acceptance_v1_9_composed …` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `cli/caprun/tests/live_acceptance_v1_10_cli.rs` — LIVE-07 SUCCESS + LIVE-08 I2 + framing + host guard
- [x] Shared helpers: F1 layout, git repo fixture, intent/policy fixtures, external confirm/grant sidecar
- [x] Product delta: `CodingI2ProofPlanner` in `planner.rs` + worker selection + main env forward (`CAPRUN_CODING_I2_PROOF=1`, default off)
- [ ] Confirm `git` available inside compose-verify `rust:1` container (install step if missing)
- [ ] Framework install: **none** — cargo test already present

*If Docker unavailable during implementation: complete Wave 0 code + host guards; do not mark LIVE-07/08 Complete until compose-verify green.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Design-partner interactive TTY confirm UX | LIVE-07 | Discretionary UX; CI uses CAPRUN_CONFIRM=external | Optional after automated external-confirm path is green |
| Milestone framing prose in close-out docs | LIVE-07/08 | Orchestrator verify-work / milestone record | Ensure no hybrid DONE language; cite CLI multi-node |

*All LIVE-07/08 acceptance behaviors must have automated verification via Wave 0 + plan tasks on real Linux through compose-verify.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s for host; LIVE via compose-verify documented
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 tests land

**Approval:** implementation complete; authoritative Docker compose execution pending. `nyquist_compliant` intentionally remains false until that gate is green.
