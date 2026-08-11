# Phase 50 — API / Integration Coverage

**Phase:** 50-cli-multi-node-driver-mid-loop-confirm-continuity  
**Claim:** **No external API integration.** Phase 50 productizes the **CLI multi-node coding driver** and **mid-loop Block-and-Hold** over existing broker/worker/planner substrate. No new external service surface, no public SDK, no third-party client under test, **zero new packages**.

## External API / SDK

| Surface | Status |
|---------|--------|
| External HTTP/REST API | N/A — not in scope |
| Public client SDK | N/A — not in scope |
| Third-party SaaS integration | N/A — uses existing broker sinks only |
| New crates / packages | **Zero** (HYG-02) |
| Internal worker ↔ broker UDS | Existing IPC; Phase 50 adds parent↔worker **stdio hold lines** only (no new broker Wait verb) |

## What is covered instead

- Host unit: `cli/caprun/tests/stream_hold.rs` — parent↔worker protocol + CLI-02 exit taxonomy
- Host unit: `cli/caprun/tests/stream_substrate.rs` — HoldContinue (no re-submit) + HoldAbort
- Host unit: `cli/caprun/tests/coding_cli.rs` — CLI-01 argv → SafeCodingWorkflow JSON; unknown kind; seed-from-file reject; `--policy` accepted
- Product path: `cli/caprun/src/main.rs` — `safe-coding-workflow` arm; piped hold orchestration; mid-loop confirm/deny; grant pointer; exit 0/2/3/1
- Worker hold: `cli/caprun/src/worker.rs` (Plan 01) — stay-connected PROCEED/ABORT
- Regression: e2e / confirm / grant / planner / stream_substrate
- Architectural gates: `./scripts/check-invariants.sh` Gates 1–6

## Assumption-delta: CaprunIntent identity

**Delta:** **no-change** this phase.

| Option | Disposition | Rationale |
|--------|-------------|-----------|
| **CaprunIntent closed-enum identity unchanged** (chosen) | Accepted | Phase 49 already added `SafeCodingWorkflow` add-alongside. Phase 50 only **consumes** that variant via CLI JSON argv — no new variant, no promotion of email/file, no free-form map |
| Add new CaprunIntent variant in Phase 50 | Rejected | Would re-open identity; not needed |
| Free-form effect path / open tool map | Rejected | Gate 1 / HYG-02 / PLAN.md effect path locked |
| Dual-Session or reconnect-remint resume | Rejected | Occupancy + ProvideIntent once (CONFIRM-01) |

**Implications:** Phase 51 LIVE-07/08 composes the same closed intent + plan_next + hold spine driven by the real `caprun` binary.

## Framing honesty

- Phase 50 proves **product CLI multi-node orchestration** and **same-Session mid-loop confirm continuity**.
- Phase 50 does **not** claim LIVE-07/08 non-hybrid multi-node SUCCESS on real Linux git.push + github.pr — that is **Phase 51**.
- Host coding_cli tests prove argv + fail-closed contracts; full confined five-node LIVE is out of scope for this phase's DONE.

## Exemption rationale

Coverage gates that expect HTTP/SDK client contracts do not apply. Phase 50 is internal CLI orchestration over existing UDS + audit DB surfaces only.
