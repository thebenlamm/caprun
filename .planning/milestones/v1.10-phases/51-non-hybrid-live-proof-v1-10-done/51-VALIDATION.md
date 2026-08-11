---
phase: 51
slug: non-hybrid-live-proof-v1-10-done
status: validated
nyquist_compliant: true
wave_0_complete: true
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
| **Scoped LIVE command** | `COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca -- --test-threads=1' bash scripts/compose-verify.sh` |
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
| 51-01-T* | 51-01 | 1 | LIVE-07 | T-51-01 hybrid overclaim | Real CLI multi-node SUCCESS one Session; not hybrid | e2e Linux+mock-egress-ca | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-01-T* | 51-01 | 1 | LIVE-07 | T-51-01 | `caprun audit` Chain verification PASSED; verify_chain | e2e | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-01-T* | 51-01 | 1 | LIVE-07 | T-51-01 | Framing: CLI-driven not hybrid | unit/e2e | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-01-T* | 51-01 | 1 | LIVE-07 | — | Host guard binary present | unit host | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-02-T* | 51-02 | 2 | LIVE-08 | T-51-02 launder / T-51-03 vacuity | Mid-loop I2 Block via genuine bag taint under permitted sink | e2e Linux+mock-egress-ca | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-02-T* | 51-02 | 2 | LIVE-08 | T-51-03 | policy_deny is not what fired; no effect of blocked node | e2e | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-02-T* | 51-02 | 2 | LIVE-08 | T-51-04 stapled | process_exited precedes Block; verify_chain true after Block | e2e | scoped compose log `dc57e49b…` | ✅ exists | ✅ green |
| 51-*-reg | 51-02 | 2 | LIVE-07/08 | — | No v1.0–v1.9 regression | full workspace | full compose log `4bcb275b…` | ✅ harness | ✅ green |
| 51-*-hyg | * | * | HYG | Gate 1/3 | check-invariants Gates 1–6 | script | post-compose invariant output | ✅ | ✅ green |
| 51-*-p50 | * | * | regression | — | Phase 50 coding argv / hold / planner | unit | full compose log `4bcb275b…` | ✅ | ✅ green |
| 51-*-v19 | * | * | regression | T-51-01 | v1.9 hybrid still green (not DONE claim) | e2e Linux | full compose log `4bcb275b…` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `cli/caprun/tests/live_acceptance_v1_10_cli.rs` — LIVE-07 SUCCESS + LIVE-08 I2 + framing + host guard
- [x] Shared helpers: F1 layout, git repo fixture, intent/policy fixtures, external confirm/grant sidecar
- [x] Product delta: `CodingI2ProofPlanner` in `planner.rs` + worker selection + main env forward (`CAPRUN_CODING_I2_PROOF=1`, default off)
- [x] Confirm `git` available inside compose-verify `rust:1` container (scoped and full compose runs passed)
- [x] Framework install: **none** — cargo test already present

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

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s for host; LIVE via compose-verify documented
- [x] `nyquist_compliant: true` set in frontmatter after Wave 0 tests landed and executed

**Approval:** authoritative scoped and full real-Linux Docker compose gates passed on 2026-08-08. Evidence is retained in `51-LIVE-EVIDENCE.md`, `51-LIVE-SCOPED.log`, and `51-LIVE-FULL.log`.

## Validation Audit 2026-08-11

| Metric | Count |
|--------|-------|
| Requirements audited | 2 |
| Covered | 2 |
| Partial | 0 |
| Missing | 0 |
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

The retained authoritative real-Linux logs remain byte-identical to their evidence record:
`51-LIVE-SCOPED.log` SHA-256 is `dc57e49b2d75ec0d040da69a780f13485507148b5efe19d0457640d41916b98d`
and `51-LIVE-FULL.log` SHA-256 is `4bcb275b98dde637d7ac644a60227d33cc5ec47acf65e8898e2f1a4d4b34ee3e`.
The scoped log records LIVE-07 and LIVE-08 green (4/4), and the full log ends with the
composed Linux verification success marker. A current host-safe rerun of the order-independent
LIVE-08 attribution oracle passed (1/1), followed by invariant Gates 1–6.
