---
phase: 04-value-injection-security-demo-v0-done
plan: "05"
subsystem: brokerd/acceptance
tags: [acceptance-test, taint-propagation, security, v0-done]
dependency_graph:
  requires: [04-01, 04-02, 04-03, 04-04]
  provides: [REQ-s9-acceptance-test]
  affects: []
tech_stack:
  added: []
  patterns: [genuine-taint-backstop, anti-stapling, audit-dag-verification]
key_files:
  created:
    - crates/brokerd/tests/s9_acceptance.rs
  modified: []
decisions:
  - "Test placed under crates/brokerd/tests/ (not crates/executor/tests/) to avoid a dev-dependency cycle: brokerd already depends on executor, so tests that exercise both crates belong in brokerd."
  - "executor::submit_plan_node called directly (not via brokerd::submit_plan_node) to match the plan spec and keep the import chain explicit."
  - "Sub-criterion 6 (no-send-cap) referenced via comment pointing to Phase 3 29/29 results rather than re-running confinement infrastructure in-process."
metrics:
  duration: "238s"
  completed: "2026-06-30"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 0
status: complete
---

# Phase 4 Plan 5: §9 Acceptance Test — v0 DONE Gate Summary

## One-liner

§9 end-to-end acceptance test with genuine-taint backstop: tainted email address blocked in email.send `to` arg, provenance_chain[0] proven equal to a real file_read Event, audit DAG hash-chain verified.

## What Was Built

Single file: `crates/brokerd/tests/s9_acceptance.rs` — the non-negotiable gate for v0 DONE.

The test drives real production code end-to-end with no LLM and no interactive input:

1. **Hostile content extraction** — `extract_email_claims` finds `accounts@ev1l.com` in the hostile sentence; asserts the raw sentence is NOT stored in the Claim (lossy guarantee).
2. **Genuine taint minting** — `mint_from_read` appends a `file_read` Event to the in-memory audit DAG and mints a `ValueRecord` with `provenance_chain[0] == that Event's id`. The test never calls `store.mint` or sets any taint field directly.
3. **Scripted plan** — `PlanNode { sink: SinkId("email.send"), args: [PlanArg { name: "to", value_id }] }` constructed with only the opaque handle (no literal, no taint on the planner side).
4. **Executor block** — `executor::submit_plan_node` returns `BlockedPendingConfirmation`; all fields are asserted: `literal_value == "accounts@ev1l.com"`, `sink == "email.send"`, `arg_name == "to"`, `taint contains ExternalUntrusted`.
5. **Held-out genuine-taint backstop** — `provenance_chain[0] == read_event_id` (9 occurrences in the file); a stapled-taint implementation MUST fail this assertion.
6. **Literal-value confirmation** — `build_confirmation_prompt` returns `raw_recipient == "accounts@ev1l.com"` (byte-exact).
7. **Audit DAG verification** — `find_event_by_type` returns the `file_read` event with both `ExternalUntrusted` and `EmailRaw` taint; its `id == provenance_chain[0]`; `verify_chain` returns `true`.

## Anti-Stapling Invariant (T-04-03)

Confirmed by grep:
- `provenance_chain[0]` appears 9 times (genuine-taint backstop is present).
- `store.mint` appears 0 times in non-comment lines (taint originates only from production `mint_from_read`).
- `ValueRecord {` appears 0 times in non-comment lines (no broker-owned record construction in the test).

## Test Results

```
cargo test -p brokerd --test s9_acceptance
test s9_acceptance ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Full workspace: all 51 tests pass, 0 failures.

## Deviations from Plan

None — plan executed exactly as written.

## Anti-Stapling Sanity (Manual Reasoning)

If `mint_from_read` were modified to return a fabricated UUID as `read_event_id` (stapled taint), the assertion `provenance_chain[0] == read_event_id` would pass trivially but the assertion `file_read_event.id == provenance_chain[0]` would fail: the fabricated UUID would not match any event in the audit DAG. The two-sided check (chain → DAG query) closes this loophole. This makes the test a genuine backstop, not a tautology.

## Self-Check: PASSED

- `crates/brokerd/tests/s9_acceptance.rs` exists: FOUND
- Commit `8771592` exists: FOUND
- `cargo test -p brokerd --test s9_acceptance`: PASSED
- `cargo test --workspace`: PASSED (all results ok)
- `grep -c "provenance_chain[0]"`: 9 (>= 1 required)
- `grep -c "store.mint"` (non-comment): 0 (required)
