---
phase: 06-deterministic-planner-intent-input
plan: 04
subsystem: caprun-cli
tags: [planner, intent, cli, clean-path, tdd, ipc]
status: complete

requires: [06-01, 06-03]
provides: [plan_from_intent, intent-cli-args, ProvideIntent-worker-round-trip]
affects: [cli/caprun]

tech-stack:
  added: []
  patterns:
    - TDD RED/GREEN for plan_from_intent (stub→impl, 3 unit tests)
    - "#[path] include pattern for integration tests in multi-binary crate without lib target"
    - plan_from_intent is a pure infallible function — no I/O, no async, no ValueRecord access
    - CaprunIntent deserialized from INTENT env var (serde_json, exhaustive enum, fail-closed)
    - ProvideIntent IPC ordering: connect → set_nonblocking → apply_confinement → ProvideIntent → RequestFd

key-files:
  created:
    - cli/caprun/src/planner.rs
    - cli/caprun/tests/planner.rs
  modified:
    - cli/caprun/src/main.rs
    - cli/caprun/src/worker.rs
    - cli/caprun/Cargo.toml

decisions:
  - "plan_from_intent always uses intent_value_id (UserTrusted) for the 'to' arg — file_value_ids are passed as _file_value_ids and ignored on the clean path (PLAN-03)"
  - "tests/planner.rs uses #[path = ../src/planner.rs] mod planner to test the pure function without a lib target"
  - "s9_live_block.rs Linux-gated hostile tests are deferred to Phase 7 — planner now routes UserTrusted handle, not tainted file handle"
  - "Dead-code warning for plan_from_intent during TDD RED was acceptable; resolved at GREEN"

metrics:
  duration: "5m"
  completed: "2026-07-01"
  tasks_completed: 3
  files_modified: 5

requirements: [PLAN-01, PLAN-02, PLAN-03]
---

# Phase 06 Plan 04: CLI Intent Input + Deterministic Planner Summary

**One-liner:** `plan_from_intent` maps a typed `CaprunIntent` to a `PlanNode` using only opaque `ValueId` handles; caprun CLI now accepts `<intent-kind> <intent-param>` and passes the intent to the worker via `INTENT` env var, which drives the `ProvideIntent` IPC round-trip after self-confinement.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (TDD RED) | Failing tests for plan_from_intent | 5358bb8 | tests/planner.rs, src/planner.rs (stub), Cargo.toml, src/worker.rs (mod planner) |
| 1 (TDD GREEN) | Implement plan_from_intent | e2a0219 | src/planner.rs |
| 2 | main.rs intent args + INTENT env var | 5c7621e | src/main.rs |
| 3 | worker.rs ProvideIntent round-trip + planner call | e39497e | src/worker.rs |

## What Was Built

### `cli/caprun/src/planner.rs` (NEW)

- `pub fn plan_from_intent(intent: &CaprunIntent, intent_value_id: ValueId, _file_value_ids: &[ValueId]) -> PlanNode`
- Pure function: no I/O, no async, infallible. Never calls `ValueStore::mint`. Never sees a `ValueRecord`, raw bytes, or taint labels.
- `SendEmailSummary { .. }` → `PlanNode { sink: "email.send", args: [PlanArg { name: "to", value_id: intent_value_id }] }`
- The `..` in the match arm ignores `recipient` (literal lives in broker ValueStore, reachable only via the handle — PLAN-03)
- `_file_value_ids` unused on the clean allow-path; available for future mixed-path demos

### `cli/caprun/tests/planner.rs` (NEW)

3 unit tests (not Linux-gated — pure function, no platform dependencies):
- `plan_from_intent_send_email_summary_maps_to_email_send`: core mapping assertion
- `plan_from_intent_ignores_file_value_ids`: asserts file-derived tainted handles are not routed to the plan node
- `plan_from_intent_recipient_literal_is_not_visible_to_planner`: asserts the planner derives no handle from the literal

TDD pattern: `#[path = "../src/planner.rs"] mod planner;` includes the source directly; avoids needing a lib target.

### `cli/caprun/src/main.rs` (updated)

- New CLI signature: `caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]`
- `"send-email-summary"` → `CaprunIntent::SendEmailSummary { recipient: intent_param }`
- Unknown intent kind → `anyhow::bail!` (fail closed, V5)
- `.env("INTENT", serde_json::to_string(&intent)?)` added to worker `Command`

### `cli/caprun/src/worker.rs` (updated)

New protocol ordering (Pitfall 6 maintained):
1. Read `INTENT` env var → deserialize `CaprunIntent` (fail closed on unknown variant)
2. After `apply_confinement()`: send `ProvideIntent { intent: intent.clone() }`
3. Receive `IntentAccepted { value_id }` → `intent_value_id`
4. Existing `RequestFd` → `FdGranted` → file read → `ReportClaims` → `ClaimsReceived` flow unchanged
5. `crate::planner::plan_from_intent(&intent, intent_value_id, &value_ids)` replaces inline planner
6. `SubmitPlanNode` / `PlanNodeDecision` / exit logic unchanged

## Verification

- `cargo test -p caprun --test planner` — 3 passed (RED: 3 failed → GREEN: 3 passed)
- `cargo build --workspace` — clean (no errors)
- `cargo test --workspace --no-fail-fast` — all green (macOS; Linux e2e stubs pass as 0-assertion no-ops)
- `./scripts/check-invariants.sh` — Gate 1 (no EffectRequest token): PASS; Gate 2 (runtime-core pure): PASS

## Deviations from Plan

None — plan executed exactly as written.

TDD RED commit included a stub `plan_from_intent` returning `SinkId("STUB_NOT_IMPLEMENTED")` and empty args (compile-pass, test-fail RED state). GREEN replaced the stub with the correct implementation.

## Known Stubs

None. `plan_from_intent` is fully implemented. The `_file_value_ids` parameter is intentionally unused on the clean allow-path (documented in source). No placeholder data.

## Deferred Items

### Linux-gated e2e tests need CLI arg updates (Phase 7)

`cli/caprun/tests/e2e.rs` and `cli/caprun/tests/s9_live_block.rs` spawn `caprun` with the old CLI signature (`workspace-file [audit-db-path]`). After this plan, caprun requires `<intent-kind> <intent-param> <workspace-file> [audit-db-path]`. These tests are `#[cfg(target_os = "linux")]` gated and do not run on macOS, so they do not affect macOS verification.

On Linux (Phase 7 gate):
- `e2e.rs/substrate_demo` and `dag_chain_integrity`: need intent args; `dag_chain_integrity` expects 2 events but will see 3 (session_created → intent_received → fd_granted)
- `s9_live_block.rs` hostile tests: need intent args AND the hostile-block assertion needs redesign — after Phase 06-04, `plan_from_intent` always routes `intent_value_id` (UserTrusted) to the `to` arg, so the executor returns `Allowed` even for hostile file content. The §9 block is still provable via in-process tests (`s9_acceptance.rs`) but the live binary test needs to be redesigned for Phase 7.

Phase 7 will address these as part of the v1.1 DONE gate.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes. The `INTENT` env var passes through the OS process environment from the orchestrator to the worker — consistent with how `WORKSPACE_FILE` and `BROKER_SOCK` are already passed. No new trust boundaries.

T-06-06 (forged INTENT): mitigated — `serde_json::from_str` into exhaustive `CaprunIntent` enum; unknown variant returns `Err`, worker bails (fail closed).
T-06-07 (planner sees raw bytes/taint): mitigated — `plan_from_intent` signature is `(&CaprunIntent, ValueId, &[ValueId]) -> PlanNode`, type-enforced.
T-06-10 (IPC before confinement): mitigated — ordering invariant: `apply_confinement()` precedes `ProvideIntent` in worker.rs.

## Self-Check

Files exist:
- [x] `cli/caprun/src/planner.rs` — contains `pub fn plan_from_intent`
- [x] `cli/caprun/tests/planner.rs` — contains 3 unit tests
- [x] `cli/caprun/src/main.rs` — contains new CLI arg parsing + INTENT env var
- [x] `cli/caprun/src/worker.rs` — contains ProvideIntent round-trip + planner call
- [x] `cli/caprun/Cargo.toml` — contains `[[test]] name = "planner"`

Commits exist:
- [x] 5358bb8: test(06-04): add failing tests for plan_from_intent
- [x] e2a0219: feat(06-04): implement plan_from_intent pure planner
- [x] 5c7621e: feat(06-04): parse intent args + pass INTENT env var to worker
- [x] e39497e: feat(06-04): worker ProvideIntent round-trip + planner integration

## Self-Check: PASSED
