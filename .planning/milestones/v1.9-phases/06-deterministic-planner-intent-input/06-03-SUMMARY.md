---
phase: 06-deterministic-planner-intent-input
plan: 03
subsystem: brokerd
tags: [mint, ipc, taint, clean-path, audit-dag, value-store]
status: complete

requires: [06-01]
provides: [mint_from_intent, ProvideIntent-IPC, clean-path-Allowed]
affects: [brokerd, executor]

tech-stack:
  added: []
  patterns:
    - mint_from_intent mirrors mint_from_read (append event then mint record in one call)
    - exhaustive match on CaprunIntent in dispatch arm (no wildcard — compile error on new variants)
    - per-connection ValueStore scope enforcement via ProvideIntent IPC round-trip

key-files:
  created: []
  modified:
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/s9_acceptance.rs

decisions:
  - "mint_from_intent uses taint: [UserTrusted] not [] to make HARD-02 predicate explicit (Pitfall 2)"
  - "event.parent_id = None for intent_received, deferred to Phase 7 (consistent with mint_from_read)"
  - "placeholder dispatch arm added in Task 2 to maintain green build between tasks"

metrics:
  duration: "5m"
  completed: "2026-07-01"
  tasks_completed: 3
  files_modified: 5

requirements: [PLAN-04]
---

# Phase 06 Plan 03: mint_from_intent + ProvideIntent IPC Round-Trip Summary

**One-liner:** `mint_from_intent` mints a `[UserTrusted]`-tainted ValueRecord anchored to a genuine `intent_received` audit event, driven by a new `ProvideIntent`/`IntentAccepted` IPC round-trip inside the per-connection ValueStore.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (TDD RED) | Failing tests for mint_from_intent | b48ec6f | quarantine.rs |
| 1 (TDD GREEN) | Implement mint_from_intent | 2f7ef91 | quarantine.rs |
| 2 | ProvideIntent/IntentAccepted proto variants + placeholder arm | 56d0d1c | proto.rs, server.rs |
| 3 | ProvideIntent dispatch arm + IPC and clean-path tests | 4a50eca | server.rs, proto_claims.rs, s9_acceptance.rs |

## What Was Built

### `mint_from_intent` (quarantine.rs)
- Sibling of `mint_from_read`: appends `intent_received` audit event then mints ValueRecord in one call
- Record taint: `[TaintLabel::UserTrusted]` (explicit positive provenance — NOT empty vec)
- Event taint: `[]` (the event itself carries no taint, unlike `mint_from_read`)
- `provenance_chain[0] == intent_event_id` (genuine-provenance anchor, T-06-04)
- `event.parent_id = None` for now; Phase 7 wires parent chain (same precedent as `mint_from_read`)

### `ProvideIntent`/`IntentAccepted` proto variants (proto.rs)
- `BrokerRequest::ProvideIntent { intent: CaprunIntent }` — sent BEFORE `RequestFd`
- `BrokerResponse::IntentAccepted { value_id: ValueId }` — singular mirror of `ClaimsReceived`
- Security contract doc-comments: broker mints authoritatively, worker never constructs ValueRecord

### `ProvideIntent` dispatch arm (server.rs)
- Replaces placeholder with real implementation: extracts literal from `CaprunIntent::SendEmailSummary`
- Calls `mint_from_intent` inside `handle_connection`'s per-connection `ValueStore` (Pitfall 1 enforced)
- Advances `*last_event_id` / `*last_event_hash` after successful mint (causal chain threading)
- Exhaustive `match &intent` — adding a new `CaprunIntent` variant = compile error

### Tests
**quarantine.rs inline tests (3 new):**
- `mint_from_intent_anchor_identity`: provenance_chain[0] == intent_event_id
- `mint_from_intent_taint_on_record_empty_on_event`: record [UserTrusted], event taint []
- `mint_from_intent_literal_flows_through`: literal passes through unchanged

**proto_claims.rs (3 new):**
- `provide_intent_request_round_trips`: ProvideIntent serde round-trip
- `intent_accepted_response_round_trips`: IntentAccepted serde round-trip
- `provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle`: end-to-end IPC dispatch

**s9_acceptance.rs (1 new):**
- `clean_path_intent_value_evaluates_to_allowed`: mint_from_intent → email.send PlanNode → Allowed; DAG has intent_received + plan_node_evaluated; NO sink_blocked

## Verification

- `cargo test -p brokerd mint_from_intent` — 3 passed (RED: compile fail → GREEN: all pass)
- `cargo test -p brokerd --test proto_claims` — 6 passed (3 existing + 3 new)
- `cargo test -p brokerd --test s9_acceptance` — 2 passed (existing §9 + new clean-path)
- `cargo test --workspace --no-fail-fast` — all green (0 failed across all crates)
- `./scripts/check-invariants.sh` — PASS (Gate 1: no EffectRequest token; Gate 2: runtime-core pure)

## Deviations from Plan

### Auto-added: Placeholder dispatch arm in Task 2

**Found during:** Task 2 (proto.rs changes)
**Issue:** Adding `ProvideIntent` to `BrokerRequest` made `server.rs` fail to compile (exhaustive match). Task 2's acceptance criteria required `cargo build -p brokerd` to pass.
**Fix:** Added a temporary placeholder arm returning `Error` to maintain green build; Task 3 replaced it with the real implementation.
**Files modified:** `crates/brokerd/src/server.rs`
**Rule:** Rule 3 (auto-fix blocking build issue)

No other deviations — plan executed as specified.

## Known Stubs

None. The `mint_from_intent` function and dispatch arm are fully functional. The `plan_node_evaluated` event in the clean-path test is appended manually in the test itself (the real path goes through `server.rs` `SubmitPlanNode` arm which is already wired). No UI rendering, no placeholder data.

## Self-Check

### Files exist
- [x] `crates/brokerd/src/quarantine.rs` — contains `mint_from_intent`
- [x] `crates/brokerd/src/proto.rs` — contains `ProvideIntent` and `IntentAccepted`
- [x] `crates/brokerd/src/server.rs` — contains real `ProvideIntent` dispatch arm
- [x] `crates/brokerd/tests/proto_claims.rs` — contains 6 tests
- [x] `crates/brokerd/tests/s9_acceptance.rs` — contains 2 tests

### Commits exist
- [x] b48ec6f: test(06-03): add failing tests for mint_from_intent anchor identity
- [x] 2f7ef91: feat(06-03): implement mint_from_intent — UserTrusted anchor sibling of mint_from_read
- [x] 56d0d1c: feat(06-03): add ProvideIntent/IntentAccepted proto variants + placeholder dispatch arm
- [x] 4a50eca: feat(06-03): ProvideIntent dispatch arm + IPC and clean-path acceptance tests

## Self-Check: PASSED
