---
phase: 01-substrate-foundation
plan: "01"
subsystem: runtime-core
status: complete
tags: [workspace, rust, domain-types, taint, purity]
completed_date: "2026-06-29"
duration_minutes: 6
tasks_completed: 3
tasks_total: 3
files_created:
  - Cargo.toml
  - crates/runtime-core/Cargo.toml
  - crates/runtime-core/src/lib.rs
  - crates/runtime-core/src/plan_node.rs
  - crates/runtime-core/src/effect.rs
  - crates/runtime-core/src/executor_decision.rs
  - crates/runtime-core/src/intent.rs
  - crates/runtime-core/src/session.rs
  - crates/runtime-core/src/artifact.rs
  - crates/runtime-core/src/event.rs
  - crates/runtime-core/tests/task2_types.rs
  - crates/runtime-core/tests/types_compile.rs
  - cli/caprun/Cargo.toml
  - cli/caprun/src/main.rs
files_modified: []
dependency_graph:
  requires: []
  provides:
    - runtime-core crate (Intent, Session, Effect, Artifact, Event, PlanNode, ValueNode, ExecutorDecision)
    - Cargo virtual workspace with resolver=3 and pinned workspace.dependencies
  affects:
    - "01-02-PLAN.md: brokerd stub depends on runtime-core for PlanNode/ExecutorDecision"
tech_stack:
  added:
    - serde 1.0.228 (derive)
    - serde_json 1.0.150
    - uuid 1.23.4 (v4 + serde)
    - thiserror 2.0.18
    - chrono 0.4.45 (serde)
    - anyhow 1.0.103
  patterns:
    - Virtual Cargo workspace (resolver=3, workspace.dependencies)
    - Pure-types crate with zero runtime I/O (enforced by negative grep gate)
    - TDD (RED/GREEN) for all type implementations
    - Workspace dep inheritance via { workspace = true }
key_decisions:
  - "ValueNode carries literal+provenance+taint from first commit — removing taint would be a breaking change (DEC-architectural-lock-plan-nodes, Success Criterion 3)"
  - "Effect has exactly 3 top-level variants (Observe/MutateReversible/CommitIrreversible) — grow by adding sub-enum variants, never new top-level classes (CON-effect-classes)"
  - "ExecutorDecision::NotImplemented is a typed enum variant, not todo!()/panic — callers can match on it (Phase 1 stub pattern)"
  - "Event.taint reuses plan_node::TaintLabel — no duplicate TaintLabel definition anywhere in runtime-core"
  - "serde_json::Value is acceptable in runtime-core for ValueNode.literal — serde_json has no runtime I/O (Pitfall 5 resolved)"
  - "cli/caprun included as stub workspace member in Phase 1 so cargo build --workspace never fails on a missing member (Open Question 2 resolved)"
requirements_satisfied:
  - REQ-runtime-core
---

# Phase 01 Plan 01: Virtual Workspace + runtime-core Domain Types Summary

**One-liner:** Cargo virtual workspace with resolver=3 and six pinned deps; runtime-core crate of pure domain types including ValueNode with literal+provenance+taint triple from first commit, 3-class Effect enum, and ExecutorDecision stub.

## What Was Built

The Cargo virtual workspace and the `runtime-core` crate of pure domain types. This is the foundation every later crate depends on — zero I/O, zero async, verifiably clean via negative grep gate.

**Workspace root (`Cargo.toml`):**
- Pure virtual manifest (no `[package]`) with `resolver = "3"` and `[workspace.dependencies]` pinning all six crate versions
- Members: `crates/*` glob + explicit `cli/caprun`

**`crates/runtime-core`** (13 source files):
- `plan_node.rs`: `TaintLabel` enum (7 labels), `Provenance`, `ValueNode` (literal+provenance+taint), `SinkId`, `PlanNode`
- `effect.rs`: `Effect` 3-class enum with `ObserveEffect`, `ReversibleEffect`, `IrreversibleEffect` sub-enums
- `executor_decision.rs`: `ExecutorDecision` with Allowed, BlockedPendingConfirmation, Denied, NotImplemented
- `intent.rs`: `Intent` + `IntentStatus` (6 variants)
- `session.rs`: `Session` + `SessionStatus` (5 variants; public name `Session`, not `ExecutionContext`)
- `artifact.rs`: `Artifact` + `ArtifactRef`
- `event.rs`: `Event` with causal DAG `parent_id` + `taint: Vec<plan_node::TaintLabel>` (shared type, no duplicate)
- `lib.rs`: declares all 7 modules; re-exports all 18 public types

**`cli/caprun`**: minimal `fn main() {}` stub so the explicit workspace member resolves.

**Tests (13 total, all green):**
- `tests/task2_types.rs`: 6 tests — ValueNode field presence, serde round-trip, Effect variants, ExecutorDecision variants
- `tests/types_compile.rs`: 7 tests — all domain types construct and serde round-trip; taint survives serialization; composed PlanNode smoke test

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build --workspace` | Exit 0 |
| `cargo test -p runtime-core` | 13/13 pass |
| `grep -rE "std::io\|std::fs\|std::net\|tokio\|async fn" crates/runtime-core/src/` | Exit 1 (no matches) |
| `grep -c '^\[package\]' Cargo.toml` | 0 (pure virtual manifest) |
| Workspace members | runtime-core + caprun |

## Deviations from Plan

None — plan executed exactly as written. All types implemented verbatim per RESEARCH patterns. TDD RED/GREEN cycle followed for Tasks 2 and 3.

## Known Stubs

None that affect plan goals. `cli/caprun/src/main.rs` has `fn main() {}` but this is intentional — it is explicitly a Phase 3 stub whose purpose (workspace member resolution) is fully satisfied.

## Threat Flags

None. No new network endpoints, file access patterns, or auth paths were introduced. The threat register items from the plan were addressed:

- **T-01-02** (ValueNode taint): mitigated — `taint: Vec<TaintLabel>` is present from this first commit; verified by serde round-trip test
- **T-01-03** (runtime-core purity): mitigated — negative grep gate passes; zero I/O tokens in `crates/runtime-core/src/`
- **T-01-SC** (supply chain): mitigated — all six packages were pre-verified in RESEARCH legitimacy audit (all OK); versions pinned in workspace.dependencies; Cargo.lock will be committed on first push

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 (RED n/a) | 6ffc0ec | chore(01-01): virtual workspace skeleton + caprun stub |
| Task 2 RED | a5869c3 | test(01-01): add failing tests for plan-node, effect, decision types |
| Task 2 GREEN | ea20166 | feat(01-01): implement locked plan-node, taint, effect, and decision types |
| Task 3 RED | 01e2244 | test(01-01): add failing types_compile integration tests |
| Task 3 GREEN | cce8be8 | feat(01-01): remaining domain types, lib re-exports, purity gate confirmed |

## Self-Check: PASSED

- [x] Cargo.toml exists and is a pure virtual manifest
- [x] crates/runtime-core/src/plan_node.rs — ValueNode has taint field
- [x] crates/runtime-core/src/effect.rs — Effect has 3 variants
- [x] crates/runtime-core/src/executor_decision.rs — NotImplemented variant present
- [x] crates/runtime-core/src/intent.rs, session.rs, artifact.rs, event.rs — all created
- [x] crates/runtime-core/tests/types_compile.rs — 7 tests, all green
- [x] All 5 task commits verified in git log
- [x] Purity gate: grep exits 1 (no I/O tokens)
- [x] REQ-runtime-core satisfied
