# Phase 51 — API / Integration Coverage

**Phase:** 51-non-hybrid-live-proof-v1-10-done  
**Claim:** **No new external API integration.** Phase 51 is **composition + acceptance proof** over the Phase 50 CLI multi-node coding driver, existing broker sinks (`file.write`, `process.exec`, `git.commit`, `git.push`, `github.pr`), and the **already-shipped** `scripts/compose-verify.sh` mock-egress-ca / mock-github / Mailpit stack. No new external service surface, no public SDK, no third-party client under test, **zero new packages**.

## External API / SDK

| Surface | Status |
|---------|--------|
| External HTTP/REST API | N/A — not in scope |
| Public client SDK | N/A — not in scope |
| Third-party SaaS integration | N/A — mock GitHub/push already in compose-verify; no new live SaaS |
| New crates / packages | **Zero** (HYG-02 continues) |
| New mint sites / EffectRequest | **Forbidden** (Gate 1/3) |
| Internal worker ↔ broker UDS | Existing IPC; proof only composes plan-node path |

## What is covered instead

- Linux e2e: `cli/caprun/tests/live_acceptance_v1_10_cli.rs` — LIVE-07 CLI multi-node one Session SUCCESS + framing honesty
- Linux e2e: same module — LIVE-08 sibling CLI mid-loop I2 Block with genuine taint (after Plan 02 product path)
- Product (default-off): `CodingI2ProofPlanner` + `CAPRUN_CODING_I2_PROOF=1` worker selection + main env allowlist forward
- Host regression: `coding_cli`, `stream_hold`, `stream_substrate`, `planner` (including success-path anti-launder)
- Regression-only hybrid: `live_acceptance_v1_9_composed` (not LIVE-07 DONE claim)
- Architectural gates: `./scripts/check-invariants.sh` Gates 1–6
- Authoritative Linux gate: `scripts/compose-verify.sh` with `brokerd/mock-egress-ca`

## Assumption-delta: CaprunIntent identity

**Delta:** **no-change** this phase.

| Option | Disposition | Rationale |
|--------|-------------|-----------|
| **CaprunIntent closed-enum identity unchanged** (chosen) | Accepted | Phase 49 added `SafeCodingWorkflow`; Phase 50 CLI consumes it; Phase 51 only drives that intent via real binary + optional proof planner selection |
| New CaprunIntent variant for LIVE-08 | Rejected | Env-gated planner is thinner (RESEARCH A1) |
| Free-form effect path / open tool map | Rejected | Gate 1 / HYG-02 / PLAN.md effect path locked |
| Dual-Session or reconnect-remint resume | Rejected | CONFIRM-01 occupancy + ProvideIntent once |

**Implications:** Phase 52 packaging co-locates the same three sibling binaries proven by LIVE; no intent identity change.

## Framing honesty

- **LIVE-07:** multi-node SUCCESS is real `caprun run safe-coding-workflow` **one Session** — not hybrid multi-leg `evaluate_plan_node_and_record_for_test` composition (closes v1.9 LIVE-05 honesty gap).
- **LIVE-08:** mid-loop I2 Block is CLI-driven sibling Session with default-off proof planner; unit expressibility alone is not DONE.
- **v1.9 composed / stream_substrate bag taint:** remain **regression / substrate** only.
- LIVE Complete claims require **compose-verify green** on a Docker-capable host; host guard alone is not LIVE DONE.

## Exemption rationale

Coverage gates that expect new HTTP/SDK client contracts do not apply. Phase 51 is internal composition of existing UDS + audit DB + mock-egress-ca surfaces only.
