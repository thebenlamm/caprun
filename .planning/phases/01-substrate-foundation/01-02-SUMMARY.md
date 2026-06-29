---
phase: 01-substrate-foundation
plan: "02"
subsystem: brokerd
status: complete
tags: [rust, brokerd, broker-api, architectural-lock, invariant-gate, tdd]
completed_date: "2026-06-29"
duration_minutes: 7
tasks_completed: 2
tasks_total: 2
files_created:
  - crates/brokerd/Cargo.toml
  - crates/brokerd/src/lib.rs
  - scripts/check-invariants.sh
files_modified:
  - crates/runtime-core/src/plan_node.rs
dependency_graph:
  requires:
    - "01-01: runtime-core crate (PlanNode, ExecutorDecision, SinkId, ValueNode)"
  provides:
    - brokerd crate with submit_plan_node(Uuid, PlanNode) -> ExecutorDecision
    - scripts/check-invariants.sh re-runnable Phase 1 invariant gate
  affects:
    - "Phase 4 executor — must call submit_plan_node, no other effect entry point"
tech_stack:
  added:
    - brokerd crate (path dep on runtime-core)
  patterns:
    - TDD RED/GREEN cycle (submit_plan_node stub)
    - Structural architectural lock via grep gate (not runtime test)
    - Re-runnable invariant script (CI-ready, exits non-zero on violation)
key_decisions:
  - "submit_plan_node returns ExecutorDecision::NotImplemented typed value — never todo!()/panic (Phase 1 stub pattern, enforced by test)"
  - "Invariant script uses grep structural gates, not runtime tests — enforcement holds even before code runs (DEC-architectural-lock-plan-nodes)"
  - "Plan 01-01 comment in plan_node.rs used the banned token 'EffectRequest' as a doc literal; rephrased to 'no raw effect-to-sink path' so Gate 1 stays clean (Rule 1 auto-fix)"
requirements_satisfied:
  - REQ-api-stub-plan-node
---

# Phase 01 Plan 02: brokerd Stub + Invariant Gate Summary

**One-liner:** brokerd crate exposing submit_plan_node(Uuid, PlanNode) -> ExecutorDecision::NotImplemented with a TDD unit test, plus a re-runnable scripts/check-invariants.sh encoding two grep-based structural gates for Phase 1 correctness.

## What Was Built

**`crates/brokerd`** — the broker effect API crate:
- `Cargo.toml`: inherits workspace package keys; path dep on `runtime-core`; workspace deps for `uuid` and `anyhow`
- `src/lib.rs`: single public function `submit_plan_node(_session_id: uuid::Uuid, _plan: runtime_core::PlanNode) -> runtime_core::ExecutorDecision` returning `ExecutorDecision::NotImplemented`. PlanNode and ExecutorDecision are imported from runtime-core — no shadow types. No raw effect-to-sink path, no convenience executor helper. Documented with no-bypass invariant using the phrase "no raw effect-to-sink path" (not the banned token literal) so Gate 1 stays clean.
- `#[cfg(test)]` unit test `submit_plan_node_returns_not_implemented`: constructs a minimal PlanNode (SinkId + empty args Vec), calls the function, asserts result == ExecutorDecision::NotImplemented.

**`scripts/check-invariants.sh`** — re-runnable Phase 1 invariant gate:
- Gate 1: `grep -r "EffectRequest" crates/` must return no matches (T-01-01, Success Criterion 2)
- Gate 2: `grep -rE "std::io|std::fs|std::net|tokio|async fn" crates/runtime-core/src/` must return no matches (T-01-03, runtime-core purity)
- Prints `PASS`/`FAIL` per gate; exits non-zero on any violation
- `set -euo pipefail` throughout

## Verification Results

| Check | Result |
|-------|--------|
| `bash scripts/check-invariants.sh` | Exit 0 — both gates PASS |
| `grep -r "EffectRequest" crates/` | No matches (CLEAN) |
| `cargo test -p brokerd -- submit_plan_node_returns_not_implemented` | 1/1 pass |
| `cargo test --workspace` | All tests pass (runtime-core: 13, brokerd: 1) |
| `cargo build --workspace` | Exit 0 |
| No todo!()/unimplemented!()/panic! in submit_plan_node | Confirmed |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Rephrased doc comment using banned token in plan_node.rs**
- **Found during:** Task 2 — first run of check-invariants.sh Gate 1 failed
- **Issue:** `crates/runtime-core/src/plan_node.rs` (from Plan 01-01) contained the comment `"Raw EffectRequest→sink is forbidden."` — the literal token "EffectRequest" caused Gate 1 to report FAIL
- **Fix:** Rephrased to `"No raw effect-to-sink path may exist."` — semantically identical, token-clean
- **Files modified:** `crates/runtime-core/src/plan_node.rs`
- **Commit:** `ce52b24` (included in Task 2 commit)

## Known Stubs

`submit_plan_node` always returns `ExecutorDecision::NotImplemented`. This is intentional — the Phase 1 contract. Phase 4 will implement real taint-enforcement logic (Allowed / BlockedPendingConfirmation / Denied).

## Threat Flags

None. No new network endpoints, file access patterns, or auth paths introduced.

Threat register items addressed:
- **T-01-01** (Tampering/Elevation): mitigated — Gate 1 in check-invariants.sh confirms no raw effect-to-sink type in crates/; confirmed `grep -r "EffectRequest" crates/` returns nothing
- **T-01-04** (DoS on stub): accepted — stub returns immediately with no allocation beyond moved PlanNode; no mitigation needed in Phase 1

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 RED | ae386ed | test(01-02): add failing test for brokerd submit_plan_node |
| Task 1 GREEN | 14a5fd0 | feat(01-02): implement brokerd submit_plan_node returning NotImplemented |
| Task 2 | ce52b24 | feat(01-02): add architectural invariant gate script + fix comment token |

## Self-Check: PASSED

- [x] `crates/brokerd/Cargo.toml` exists — path dep on runtime-core
- [x] `crates/brokerd/src/lib.rs` exists — submit_plan_node with NotImplemented body
- [x] `scripts/check-invariants.sh` exists and is executable
- [x] `bash scripts/check-invariants.sh` exits 0 (both gates PASS)
- [x] `grep -r "EffectRequest" crates/` returns nothing (CLEAN)
- [x] `cargo test --workspace` all pass
- [x] `cargo build --workspace` exits 0
- [x] All 3 task commits verified in git log
- [x] REQ-api-stub-plan-node satisfied
