---
phase: 05-runtime-spine-live-9-email-block
plan: "01"
subsystem: brokerd/proto
tags: [ipc-protocol, serde, security, asm-03, tdd, additive]
status: complete

dependency_graph:
  requires: []
  provides:
    - brokerd::proto::WorkerClaim (enum, EmailAddress variant, internally-tagged serde)
    - brokerd::proto::BrokerRequest::ReportClaims
    - brokerd::proto::BrokerResponse::ClaimsReceived
  affects:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs (stub arm only)
    - cli/caprun/src/main.rs (stub arm only)
    - crates/brokerd/tests/proto_claims.rs (new)

tech_stack:
  added: []
  patterns:
    - internally-tagged serde enum (#[serde(tag, content)]) for fail-closed deserialization
    - exhaustive Rust enum as IPC security contract (no #[serde(other)] wildcard)
    - TDD RED/GREEN cycle with compile-error RED gate

key_files:
  created:
    - crates/brokerd/tests/proto_claims.rs
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/main.rs

decisions:
  - "WorkerClaim uses internally-tagged serde (#[serde(tag = \"kind\", content = \"value\")]) so unknown kind fields produce a hard deserialize Err — fail-closed by construction, no runtime check needed"
  - "Phase 5 ships exactly one variant (EmailAddress); RelativePath deferred to Phase 7 — placeholder comment only"
  - "Plan 01 is strictly additive: ReportRead, SubmitPlanNode.session_id, and existing match arms unchanged; stub ReportClaims arms added to server.rs and main.rs to keep build green"

metrics:
  duration_minutes: 7
  completed_date: "2026-06-30"
  tasks_completed: 2
  files_created: 1
  files_modified: 3
---

# Phase 05 Plan 01: Add WorkerClaim IPC Vocabulary Summary

Typed IPC vocabulary for worker→broker claims protocol: `WorkerClaim` enum with internally-tagged serde (fail-closed on unknown kinds), `ReportClaims` request variant, and `ClaimsReceived` response variant — all additive, workspace stays green.

## What Was Built

### WorkerClaim enum (`crates/brokerd/src/proto.rs`)

New public enum with `#[serde(tag = "kind", content = "value")]` attribute. Exhaustive with one variant for Phase 5: `EmailAddress(String)`. No wildcard arm — unknown `kind` values produce a hard deserialize `Err`. Doc comment explicitly states raw source bytes never appear in the payload (ASM-03).

### BrokerRequest::ReportClaims

New variant carrying `claims: Vec<WorkerClaim>`. Doc comment: raw bytes not included; broker mints a `ValueRecord` per claim and returns opaque handles.

### BrokerResponse::ClaimsReceived

New variant carrying `value_ids: Vec<runtime_core::plan_node::ValueId>`. Opaque handles in the same order as submitted claims.

### Test file (`crates/brokerd/tests/proto_claims.rs`)

Three `#[test]` functions, all cross-platform (no Linux-only cfg gates):

1. `report_claims_request_round_trips` — `BrokerRequest::ReportClaims` survives a `serde_json::to_value` / `from_value` round-trip
2. `claims_received_response_round_trips` — `BrokerResponse::ClaimsReceived` with a freshly-minted `ValueId` survives a `to_string` / `from_str` round-trip
3. `unknown_claim_kind_fails_closed` — hand-crafted JSON with `kind: "TotallyUnknownKind"` returns `Err`, asserted with `.is_err()`

## TDD Gate Compliance

- RED commit: `61fa1d5` — `test(05-01): add failing test for WorkerClaim serde contract (RED)` — failed with `E0432: unresolved import brokerd::proto::WorkerClaim`
- GREEN commit: `9cea4b8` — `feat(05-01): add WorkerClaim enum and IPC variants to proto.rs (GREEN)` — minimal test passed
- Task 2 commit: `5f7d2aa` — complete 3-test suite, all pass

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build -p brokerd` | green |
| `cargo test -p brokerd --test proto_claims` | 3/3 passed |
| `cargo test -p brokerd --test s9_acceptance` | 1/1 passed (no regression) |
| `cargo test --workspace --no-fail-fast` | all green (macOS; Linux-only tests show 0 passed as expected) |
| `grep -c "enum WorkerClaim" proto.rs` | 1 |
| `grep -c "ReportClaims" proto.rs` | 3 |
| `grep -c "ClaimsReceived" proto.rs` | 1 |
| `grep -c "serde(other)" proto.rs` | 0 |
| `grep -c "is_err" proto_claims.rs` | 1 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Non-exhaustive match in brokerd server.rs**
- **Found during:** Task 1 GREEN phase (`cargo build -p brokerd`)
- **Issue:** Adding `ReportClaims` to `BrokerRequest` made the `dispatch` match in `server.rs` non-exhaustive
- **Fix:** Added `BrokerRequest::ReportClaims { .. }` stub arm returning `BrokerResponse::Error { message: "not wired until Plan 05" }` — same pattern used by `RequestFd` / `ReportRead` stubs
- **Files modified:** `crates/brokerd/src/server.rs`
- **Commit:** `9cea4b8`

**2. [Rule 3 - Blocking] Non-exhaustive match in cli/caprun/src/main.rs**
- **Found during:** Task 2 workspace build verification
- **Issue:** `handle_worker_connection` in `main.rs` also matched on `BrokerRequest` exhaustively; `ReportClaims` was not covered
- **Fix:** Added `BrokerRequest::ReportClaims { .. }` stub arm with error response — same pattern as `SubmitPlanNode` stub already present
- **Files modified:** `cli/caprun/src/main.rs`
- **Commit:** `5f7d2aa`

### Out-of-Scope Pre-existing Issue (deferred)

`./scripts/check-invariants.sh` Gate 1 fails due to `EffectRequest` mention in `crates/brokerd/src/lib.rs` line 31 (doc comment, no `planner-discipline-allow` annotation). Present in base commit `9929ffa` — not caused by Plan 01 changes. Logged for follow-up: add `// planner-discipline-allow` annotation or reword the doc comment.

## Threat Surface Scan

No new network endpoints, auth paths, or file access patterns introduced. New serde surface (`WorkerClaim` deserialization) is specifically the validation surface for T-05-01; the fail-closed contract is proven by the `unknown_claim_kind_fails_closed` test. No threat flags beyond the plan's threat register.

## Known Stubs

- `server.rs` dispatch: `ReportClaims { .. }` arm returns `Error { "not wired until Plan 05" }` — intentional, Plan 02 wires the real handler
- `main.rs` `handle_worker_connection`: same stub — intentional, Plan 03 deletes this function and delegates to `brokerd::server`

These stubs do not prevent Plan 01's goal (establishing the wire types and proving the serde contract). Plans 02 and 03 resolve them.

## Self-Check: PASSED

- `crates/brokerd/src/proto.rs` — exists, contains `WorkerClaim`, `ReportClaims`, `ClaimsReceived`
- `crates/brokerd/tests/proto_claims.rs` — exists, 3 tests, all pass
- Commits `61fa1d5`, `9cea4b8`, `5f7d2aa` — all present in git log
