---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 02
subsystem: runtime-core / executor / brokerd
tags: [ACC-07, ACC-01, ACC-06, HARD-06, durable-anchor, I2, genuine-taint]
requires:
  - "runtime_core::DenyReason + ExecutorDecision (07-01)"
  - "ValueStore::mint non-empty invariant → provenance_chain[0] safe (07-01)"
  - "Event + audit append_event / compute_event_hash (brokerd)"
provides:
  - "runtime_core::SinkBlockedAnchor (8-field durable genuine-taint anchor)"
  - "ExecutorDecision::BlockedPendingConfirmation { anchor } (reshaped)"
  - "Event.anchor: Option<SinkBlockedAnchor> + Event::new + Event::sink_blocked"
  - "append_event rejects sink_blocked with anchor==None (Defect B non-persistable)"
  - "submit_plan_node(session_id, effect_id, plan_node, store) — broker-minted effect_id"
affects:
  - "07-05 (after-exit DB-alone sentinel, tamper-evidence, live e2e consume the persisted anchor)"
tech-stack:
  added: []
  patterns:
    - "Durable anchor rides inside the hashed payload column — tamper-evident, no DB migration"
    - "serde skip_serializing_if keeps pre-anchor events byte-identical (golden fixture)"
    - "One broker-owned anchor-setting constructor; append guard makes a defect non-persistable"
    - "Two graphs (causal DAG vs value-lineage) share node ids, never equated edges"
key-files:
  created: []
  modified:
    - crates/runtime-core/src/executor_decision.rs
    - crates/runtime-core/src/event.rs
    - crates/runtime-core/src/lib.rs
    - crates/executor/src/lib.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/lib.rs
    - crates/brokerd/src/sinks/email_send.rs
    - crates/brokerd/src/quarantine.rs
    - cli/caprun/src/main.rs
    - crates/brokerd/tests/phase5_dispatch.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - crates/brokerd/tests/audit_dag.rs
    - crates/executor/tests/executor_decision.rs
    - crates/runtime-core/tests/task2_types.rs
    - crates/runtime-core/tests/types_compile.rs
decisions:
  - "Anchor persists inside the existing hashed payload column — no DDL, no typed columns (DESIGN §5)"
  - "Event.taint = anchor.taint set by the sole broker-owned Event::sink_blocked constructor (DESIGN §4 rule 6)"
  - "sink_blocked.parent_id stays the causal chain head, NEVER read_event_id (two graphs, DESIGN §0)"
  - "brokerd::submit_plan_node delegate mints effect_id broker-side (ripple of the executor signature change)"
metrics:
  duration: ~35min
  completed: 2026-07-01
  tasks: 6
  files: 16
status: complete
---

# Phase 7 Plan 02: Durable Genuine-Taint Anchor (ACC-07) Summary

Closed Defect B — the persisted `sink_blocked` event now carries a real `SinkBlockedAnchor` (value_id + byte-exact literal + verbatim taint + provenance chain, `provenance_chain[0] == read_event_id`) instead of the old bare marker (`taint: vec![]`). The anchor is cloned verbatim from the resolved `ValueRecord` by the executor (never constructed — T-04-03), persisted inside the existing hashed `payload` column (no DB migration), and a `sink_blocked` event with no anchor is now non-persistable through the TCB (`append_event` returns `Err`). The broker mints `effect_id` (HARD-06) and threads it into the pure executor.

## What Was Built

- **`SinkBlockedAnchor`** (`runtime-core/executor_decision.rs`): 8 fields (`effect_id`, `sink: SinkId`, `arg: String`, `value_id: ValueId`, `literal`, `taint`, `provenance_chain`, `read_event_id`) with `Debug/Clone/PartialEq/Serialize/Deserialize`; re-exported from `runtime_core`. `ExecutorDecision::BlockedPendingConfirmation` reshaped from 5 flat fields to `{ anchor }` (DESIGN §4, §6 decision 1).
- **`Event.anchor`** (`runtime-core/event.rs`): `#[serde(default, skip_serializing_if = "Option::is_none")] pub anchor: Option<SinkBlockedAnchor>` as the last field. `Event::new(..)` (sets `anchor: None`) and the sole broker-owned `Event::sink_blocked(..)` constructor (sets `event_type="sink_blocked"`, `actor="executor"`, `taint = anchor.taint.clone()`, `anchor = Some`).
- **Source migration**: every `Event { .. }` literal (~15 sites across server.rs, quarantine.rs, sinks/email_send.rs, audit.rs, main.rs, and 3 test files) migrated to `Event::new(..)` — no site constructs the field list anymore (except the two constructors).
- **Executor** (`executor/lib.rs`): `submit_plan_node` takes a broker-minted `effect_id: Uuid`; the block branch builds `SinkBlockedAnchor` by cloning `plan_node.sink`/`arg.name`/`arg.value_id`/`record.{literal,taint,provenance_chain}` and `read_event_id = provenance_chain[0]`. The executor mints no Uuid and sets no taint.
- **Broker** (`brokerd/server.rs`, `audit.rs`): the `SubmitPlanNode` arm mints `effect_id`, and on a block persists via `Event::sink_blocked(.., Some(*last_event_id), .., anchor.clone())` — causal parent = chain head, NOT `read_event_id`. `append_event` rejects `sink_blocked && anchor.is_none()` (Defect B guard). No DDL — the anchor rides in the hashed `payload`, covered by `compute_event_hash`.
- **Golden byte-fixture** (`event.rs` tests): an `anchor:None` Event serializes byte-identical to a hardcoded pre-anchor JSON (no `"anchor"` key) and round-trips — proving `skip_serializing_if` means no DB migration.

## Task Commits

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | SinkBlockedAnchor + BlockedPendingConfirmation reshape | 60bf5cd |
| 2 | Event.anchor field + Event::new/sink_blocked + literal migration | ec47157 |
| 3 | Executor builds anchor verbatim + effect_id param | 4757e63 |
| 4 | Broker mints effect_id, persists anchor, append_event guard | 01cad28 |
| 5 | Golden byte-fixture + delete parent==read assertion | 58bcaa7 |
| 6 | s9_acceptance + executor_decision anchor reshape | df2c559 |

## Verification

- `cargo test --workspace --no-fail-fast` — green on macOS. runtime-core 1+9+7+11 (incl. golden fixture), executor 8+8, brokerd 18 lib + phase5 6 + s9 2 + audit 2 + proto 6. Linux-only enforcement/e2e tests show "0 passed" as expected (cfg-gated; not a gap).
- `./scripts/check-invariants.sh` — both gates PASS (no `EffectRequest` under crates/; runtime-core stays I/O-free).
- Golden fixture asserts byte-exact serialization equality AND no `"anchor"` key AND round-trip.
- `append_event` guard verified indirectly (constructor path); the after-exit/tamper/live backstops are 07-05.
- s9_acceptance anti-stapling assertion reads `anchor.provenance_chain[0] == read_event_id` — unweakened; new in-process `Event.taint == anchor.taint` check added.
- phase5_dispatch no longer asserts `sink_blocked.parent_id == read_event_id`; chain-head-advanced assertion retained.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `brokerd::submit_plan_node` delegate broke on the executor signature change**
- **Found during:** Task 4 (workspace build)
- **Issue:** `crates/brokerd/src/lib.rs:37` calls `executor::submit_plan_node(session_id, &plan, store)` with 3 args; the executor now takes `effect_id`.
- **Fix:** The delegate mints `effect_id` broker-side (consistent with DESIGN §4 rule 2 — effect_id is broker-owned) and passes it through. Not in Task 4's file list but a direct ripple.
- **Files modified:** crates/brokerd/src/lib.rs — **Commit:** 01cad28

**2. [Rule 3 - Blocking] `task2_types.rs` used the old flat BlockedPendingConfirmation shape**
- **Found during:** Task 5 (runtime-core test build)
- **Issue:** `crates/runtime-core/tests/task2_types.rs` constructed/destructured the 5-field flat variant, which no longer compiles after the Task 1 reshape.
- **Fix:** Rebuilt both usages with `{ anchor: SinkBlockedAnchor { .. } }` and imported `SinkBlockedAnchor`; the round-trip test now asserts `anchor.taint`/`anchor.provenance_chain`/`anchor.read_event_id`.
- **Files modified:** crates/runtime-core/tests/task2_types.rs — **Commit:** 58bcaa7

**3. [Rule 3 - Blocking] `phase5_dispatch.rs` had two 3-arg executor calls**
- **Found during:** Task 5
- **Issue:** `handle_from_other_connection_store_is_denied` calls `executor::submit_plan_node` with 3 args.
- **Fix:** Threaded a test `Uuid::new_v4()` effect_id into both calls.
- **Files modified:** crates/brokerd/tests/phase5_dispatch.rs — **Commit:** 58bcaa7

Note: the ~15 `Event` literal migrations (email_send.rs, quarantine.rs, main.rs, audit_dag.rs, types_compile.rs) were explicitly anticipated by Task 2's spec (DESIGN §5 "SOURCE migration required"), so they are in-plan work, not deviations.

## Known Stubs

None. Every field of the anchor is wired from real resolved-record data; the block path persists a genuine chain. No placeholder values introduced.

## Threat Flags

None. No new network endpoints, auth paths, or trust-boundary schema introduced beyond the plan's threat model. T-07-21/22/23/24 are the addressed mitigations: anchor in hashed payload (tamper-evident), verbatim clone (no stapling), non-persistable defect (append guard), serde byte-compat (no DDL).

## Notes for Downstream

- **07-05** consumes the persisted anchor for the after-exit DB-alone sentinel (file-backed DB, drop+reopen, `verify_chain` THEN read anchor), tamper-evidence (`UPDATE payload` → `verify_chain` false), and the live e2e block. The in-process reshape, persistence, append guard, golden fixture, and source migration are done here.
- `Event::sink_blocked` is the ONLY sanctioned anchor-setting constructor — do not build a `sink_blocked` Event any other way, or the append guard/rule-6 taint consistency can drift.
- No `TrustClass` / second partition was introduced — DB readers re-derive trust via `TaintLabel::is_untrusted()` on `anchor.taint` (DESIGN §2).

## Self-Check: PASSED
- All 16 modified files exist and are committed.
- Commits 60bf5cd, ec47157, 4757e63, 01cad28, 58bcaa7, df2c559 present in git log.
