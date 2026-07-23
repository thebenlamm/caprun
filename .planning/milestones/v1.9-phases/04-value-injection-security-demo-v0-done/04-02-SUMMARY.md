---
phase: 04-value-injection-security-demo-v0-done
plan: "02"
subsystem: executor
tags: [i2-enforcement, value-injection, taint, rust, tdd]
status: complete

dependency_graph:
  requires: ["04-01"]
  provides: ["crates/executor", "ValueStore", "submit_plan_node", "is_routing_sensitive"]
  affects: ["04-03", "04-04", "04-05"]

tech_stack:
  added:
    - "executor crate (crates/executor) — deterministic non-LLM I2 decision engine"
  patterns:
    - "Sole-taint-writer invariant: ValueStore::mint is the ONLY taint writer in the crate"
    - "Anti-stapling: submit_plan_node reads via resolve only — 0 mint refs, 0 ValueRecord{ refs in lib.rs"
    - "Handle model: ValueId/ValueRecord imported from runtime-core, never redefined"
    - "Hardcoded sensitivity map: is_routing_sensitive — no Cedar, no schema, no runtime config"
    - "TDD: RED (unimplemented!() stubs) → GREEN (implementation) cycle"

key_files:
  created:
    - crates/executor/Cargo.toml
    - crates/executor/src/lib.rs
    - crates/executor/src/value_store.rs
    - crates/executor/src/sink_sensitivity.rs
    - crates/executor/tests/executor_decision.rs
  modified: []

decisions:
  - "ValueId/ValueRecord imported from runtime-core (not redefined) — single-authority invariant"
  - "submit_plan_node in lib.rs (not a separate module) — per PATTERNS.md and DESIGN doc"
  - "is_content_sensitive exported alongside is_routing_sensitive for future approval-hook plan (04-04)"
  - "Integration tests include cc/bcc Block and body/attachment Allowed beyond the four mandated cases"

metrics:
  duration_min: 25
  completed_date: "2026-06-30"
  tasks_completed: 2
  files_created: 5
  files_modified: 0
  tests_added: 11
---

# Phase 04 Plan 02: Executor Crate — I2 Decision Logic Summary

One-liner: deterministic Rust TCB executor with sole-taint-writer ValueStore and hardcoded email.send sensitivity map enforcing I2.

## What Was Built

The `crates/executor` crate delivers the deterministic, non-LLM I2 executor specified in DESIGN-plan-executor.md. Three components:

**ValueStore** (`src/value_store.rs`): In-memory `HashMap<ValueId, ValueRecord>` wrapper. `mint` is the sole taint writer — the only code path in the crate that sets a record's taint field. `resolve` is read-only. Both `ValueId` and `ValueRecord` are imported from `runtime_core`, not redefined.

**Sink sensitivity map** (`src/sink_sensitivity.rs`): Hardcoded email.send sensitivity constants and `is_routing_sensitive(sink, arg_name)`. Routing-sensitive args: `["to", "cc", "bcc"]`. Content-sensitive args: `["subject", "body", "attachment"]`. No Cedar, no schema, no runtime config — sensitivity is a security property (CON-i2-non-bypassable).

**Decision function** (`src/lib.rs`): `submit_plan_node(session_id, plan_node, value_store) -> ExecutorDecision`. For each `PlanArg`, resolves the `ValueId` from the trusted store; `None` → `Denied`; routing-sensitive with non-empty taint → `BlockedPendingConfirmation` populated verbatim from the resolved record; otherwise → `Allowed`.

## Test Coverage

11 tests across two suites:

| Suite | Tests | Result |
|-------|-------|--------|
| `value_store` (unit) | 2 | PASS |
| `sink_sensitivity` (unit) | 3 | PASS |
| `executor_decision` (integration) | 6 | PASS |

Integration test cases:
1. Tainted "to" → `BlockedPendingConfirmation` with verbatim literal/taint/provenance_chain
2. Untainted "to" → `Allowed`
3. Dangling handle → `Denied` (never Allowed — T-04-02)
4. Tainted "subject" (content-sensitive) → `Allowed` in v0
5. Tainted "cc"/"bcc" → Block (routing-sensitive coverage)
6. Tainted "body"/"attachment" → Allowed (content-sensitive coverage)

## Security Properties Verified

**T-04-01 (Tampering — routing-sensitive Block):** Integration test 1 confirms tainted "to" returns `BlockedPendingConfirmation`. Payload verified verbatim from `mint` inputs — executor does not synthesize taint.

**T-04-02 (Spoofing — dangling handle → Denied):** Integration test 3 confirms `ValueId::new()` (never minted) returns `Denied`, not `Allowed`. An injected planner cannot reference a non-existent value to bypass.

**T-04-03 (Spoofing — anti-stapling):** Verified by negative grep:
- `grep -v '^[[:space:]]*//' crates/executor/src/lib.rs | grep -c 'ValueStore::mint'` → 0
- `grep -v '^[[:space:]]*//' crates/executor/src/lib.rs | grep -c 'ValueRecord {'` → 0

The executor only reads taint through `resolve`; it never writes it.

## TDD Gate Compliance

| Gate | Commit | Notes |
|------|--------|-------|
| RED | `4efe67d` | `test(04-02): add failing tests for ValueStore mint/resolve (RED)` — 2 tests failed via `unimplemented!()` stubs |
| GREEN (T1) | `f7871ff` | `feat(04-02): implement ValueStore mint and resolve (GREEN)` — 2 tests pass |
| GREEN (T2) | `fd8e691` | `feat(04-02): add executor_decision integration tests; complete I2 decision logic` |

Minor TDD deviation: `submit_plan_node` implementation was included in `lib.rs` during the Task 1 commits (the module structure required the full function signature and implementation for the crate to compile meaningfully). The Task 2 RED phase was therefore not strictly separated from the Task 1 implementation. All final tests pass; the behavioral invariants are fully exercised.

## Deviations from Plan

### Auto-included additional tests (Rule 2 — completeness)

**Found during:** Task 2

**Issue:** The plan specified four test cases for the integration tests. Adding coverage for `cc`/`bcc` (also routing-sensitive) and `body`/`attachment` (content-sensitive) provides complete coverage of the sensitivity map without adding complexity.

**Fix:** Added two extra test functions in `executor_decision.rs` (tainted_cc_and_bcc_also_block, tainted_body_and_attachment_allow_in_v0).

**Files modified:** `crates/executor/tests/executor_decision.rs`

### submit_plan_node included in Task 1 commits

**Found during:** Task 1

**Issue:** `lib.rs` was written with the complete `submit_plan_node` implementation to satisfy Rust's crate compilation requirements. This means the Task 2 integration tests ran GREEN on first execution (no separate RED phase for Task 2).

**Impact:** Non-strict TDD ordering between tasks. All security properties are verified; the deviation is in the TDD ceremony, not the implementation correctness.

## Stub Tracking

No stubs in production paths. All public functions are fully implemented and tested. The `is_content_sensitive` function in `sink_sensitivity.rs` is implemented but not yet called by `submit_plan_node` (v0 does not Block content-sensitive args — deferred to the approval-hook plan, 04-04). This is intentional per the DESIGN.

## Threat Flags

No new security surface beyond what the plan's threat model specifies. The executor crate has no network endpoints, no I/O, no async, no file access — it is a pure decision function over an in-memory store.

## Self-Check: PASSED

All 5 created files confirmed on disk. All 3 task commits (4efe67d, f7871ff, fd8e691) found in git log.
