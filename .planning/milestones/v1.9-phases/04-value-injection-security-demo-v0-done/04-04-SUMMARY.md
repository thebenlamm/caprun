---
phase: 04-value-injection-security-demo-v0-done
plan: "04"
subsystem: brokerd/quarantine
tags: [taint-propagation, genuine-taint, executor-integration, I2-enforcement, quarantine]
dependency_graph:
  requires: ["04-01", "04-02", "04-03"]
  provides: [quarantine-extractor, mint-from-read-anchor, submit-plan-node-delegation]
  affects: [brokerd, executor, s9-acceptance-test]
tech_stack:
  added: ["brokerd→executor path dependency", "quarantine.rs module"]
  patterns: ["TDD RED/GREEN", "Arc<Mutex<T>> threading", "genuine-taint anchor"]
key_files:
  created:
    - crates/brokerd/src/quarantine.rs
  modified:
    - crates/brokerd/Cargo.toml
    - crates/brokerd/src/lib.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/uds_ipc.rs
decisions:
  - "mint_from_read is the sole broker taint-mint site; executor::submit_plan_node is read-only"
  - "extract_email_claims uses hand-rolled word scanner — no new regex crate (T-04-SC accepted)"
  - "trailing dot trimmed by excluding '.' from trim_matches exempt set (fix for sentence-terminal punctuation)"
metrics:
  duration: "~12 minutes"
  completed: "2026-06-30"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 5
status: complete
requirements: [REQ-quarantined-reader]
---

# Phase 04 Plan 04: Quarantine Extractor + Genuine-Taint Anchor Summary

Quarantined reader extraction path + genuine-taint mint anchor (REQ-quarantined-reader): `extract_email_claims` discards the hostile sentence and returns only the typed email claim; `mint_from_read` appends the `file_read` Event and mints the `ValueRecord` with `provenance_chain=[read_event.id]` in a single broker code path — taint is genuine, not stapled.

## Tasks Completed

| # | Name | Type | Commit | Status |
|---|------|------|--------|--------|
| 1 | Quarantine extractor + mint_from_read genuine-taint anchor | TDD | d65af10, 1d0d443 | Complete |
| 2 | Wire SubmitPlanNode dispatch + brokerd::submit_plan_node delegation | auto | 2dcedab | Complete |

## What Was Built

### Task 1 — quarantine.rs (TDD)

**`Claim { claim_type: String, value: String }`** — typed, lossy extract struct. No raw sentence field.

**`extract_email_claims(raw: &str) -> Vec<Claim>`** — deterministic hand-rolled word scanner (no regex crate, no LLM). Splits on whitespace, trims edge punctuation (including sentence-terminal `.`), validates local@domain.tld shape, discards surrounding sentence. Returns one Claim per address or empty Vec.

**`mint_from_read(conn, store, session_id, claim, parent_hash) -> Result<(Uuid, String, ValueId)>`** — the genuine-taint anchor (T-04-03). Appends a `file_read` Event with taint `[ExternalUntrusted, EmailRaw]` to the audit DAG, then calls `ValueStore::mint` with `provenance_chain=[event.id]` in a single call. Returns `(read_event_id, read_hash, value_id)`. The only broker site that calls `ValueStore::mint` with a non-empty taint vector.

TDD gate:
- **RED commit** `d65af10`: 7 failing tests (todo! stubs)
- **GREEN commit** `1d0d443`: 7 passing tests

### Task 2 — server.rs + lib.rs wiring

**`run_broker_server` / `handle_connection` / `dispatch`** — threaded with `Arc<Mutex<executor::value_store::ValueStore>>` (matching existing `Arc<Mutex<rusqlite::Connection>>` pattern).

**SubmitPlanNode dispatch arm** — locks value store (std::sync::Mutex, no await), calls `executor::submit_plan_node` (pure fn), appends `plan_node_evaluated` audit Event (best-effort), returns `BrokerResponse::PlanNodeDecision { decision }`. Mutex poison → generic Error response (T-03-09: no detail leak).

**`brokerd::submit_plan_node`** — signature updated to accept `&executor::value_store::ValueStore`; body now delegates to `executor::submit_plan_node` (NotImplemented stub removed). In-crate test updated to construct a `ValueStore` and assert `Allowed` for empty-arg PlanNode.

## Verification

```
cargo test -p brokerd    → 17 passed (15 unit + 2 integration)
cargo build --workspace  → Finished dev (no cycle, brokerd→executor resolves)
```

Acceptance criteria:
- `executor` path dep in brokerd/Cargo.toml: ✓
- `grep -c "pub fn mint_from_read" crates/brokerd/src/quarantine.rs` → 1: ✓
- `grep -c "pub fn extract_email_claims" crates/brokerd/src/quarantine.rs` → 1: ✓
- `TaintLabel::ExternalUntrusted` in non-comment lines of quarantine.rs → 3: ✓
- `grep -c "SubmitPlanNode" crates/brokerd/src/server.rs` → 2 (variant match + call): ✓
- `grep -c "executor::submit_plan_node" crates/brokerd/src/lib.rs` → 2 (body + call): ✓
- `grep -n "NotImplemented" crates/brokerd/src/lib.rs` → NONE: ✓

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Sentence-terminal dot not stripped by trim_matches**
- **Found during:** Task 1, GREEN phase (2 of 7 tests failing after initial implementation)
- **Issue:** `extract_email_claims` excluded `.` from the `trim_matches` exempt set (kept `.` safe), so the trailing `.` in `"accounts@ev1l.com."` was not stripped. `looks_like_email("accounts@ev1l.com.")` correctly returned false (domain ends with `.`), causing 0 claims returned instead of 1.
- **Fix:** Removed `.` from the trim_matches exempt characters — `trim_matches` only strips from edges, so internal domain dots (e.g., `ev1l.com`) are preserved while the sentence-terminal dot is stripped.
- **Files modified:** `crates/brokerd/src/quarantine.rs`
- **Commit:** `1d0d443`

**2. [Rule 3 - Blocking] uds_ipc.rs test fails to compile on Linux with new run_broker_server signature**
- **Found during:** Task 2 (proactive check of test files)
- **Issue:** `crates/brokerd/tests/uds_ipc.rs` (Linux-gated) called `run_broker_server` with 2 args; new signature requires 3 (adds `Arc<Mutex<ValueStore>>`). Would cause compile failure on Linux.
- **Fix:** Added `executor` to brokerd `[dev-dependencies]`, imported `executor::value_store::ValueStore` in the test, passed `Arc::new(Mutex::new(ValueStore::default()))` to both `run_broker_server` calls.
- **Files modified:** `crates/brokerd/Cargo.toml`, `crates/brokerd/tests/uds_ipc.rs`
- **Commit:** `2dcedab`

## Genuine-Taint Invariant (T-04-03)

The sole taint-write site in brokerd is `quarantine::mint_from_read`. Verified:
- `grep -rn "\.mint(" crates/brokerd/src/` → only `quarantine.rs:156`
- The executor's `submit_plan_node` calls only `value_store.resolve()` — never `mint()`
- The anchor-identity test confirms `store.resolve(value_id).provenance_chain[0] == read_event_id` and `find_event_by_type("file_read").id == read_event_id`

## Known Stubs

None — all functions have real implementations. The SubmitPlanNode dispatch arm calls the real executor; mint_from_read performs real DAG append + store mint.

## Threat Flags

None — no new network endpoints, auth paths, or trust boundaries beyond those in the plan's threat model (T-04-03, T-04-01, T-04-04, T-04-SC all mitigated or accepted as planned).

## Self-Check: PASSED

All files verified present. All 3 task commits confirmed in git log.
