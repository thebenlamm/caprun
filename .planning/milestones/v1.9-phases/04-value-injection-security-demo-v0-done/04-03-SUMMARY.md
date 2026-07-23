---
phase: 04-value-injection-security-demo-v0-done
plan: 03
subsystem: brokerd
tags: [broker, proto, audit, sinks, approval, taint, tdd]
status: complete
dependencies:
  requires: [04-01]
  provides: [SubmitPlanNode-proto, PlanNodeDecision-proto, audit-query-helpers, email-send-stub, confirmation-prompt-builder]
  affects: [crates/brokerd, cli/caprun]
tech_stack:
  added: []
  patterns: [hash-linked-audit-dag, tdd-red-green, opaque-value-handles, no-executor-dep]
key_files:
  created:
    - crates/brokerd/src/sinks.rs
    - crates/brokerd/src/sinks/email_send.rs
    - crates/brokerd/src/approval.rs
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/lib.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/main.rs
decisions:
  - SubmitPlanNode arm in caprun stubbed at Plan 04 boundary — same policy as server.rs
  - ConfirmationPrompt uses rfind('@') for domain extraction to handle @ in display names
  - Stub implementation drops plan_node to avoid embedding opaque handles in audit payload
metrics:
  duration_minutes: 5
  completed_date: "2026-06-30"
  tasks_completed: 2
  files_changed: 7
---

# Phase 04 Plan 03: Broker-side Sink Stub and Approval Prompt Summary

Broker-side email.send stub and literal-value confirmation prompt builder with SubmitPlanNode/PlanNodeDecision proto variants and audit-DAG query helpers — satisfying REQ-mediated-sink-stub and REQ-approval-hook without executor crate dependency.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Proto variants + audit-DAG query helpers | 7e81369 | proto.rs, audit.rs, server.rs |
| 2 (RED) | Failing tests for sink stub + prompt builder | 32a03e6 | sinks.rs, sinks/email_send.rs, approval.rs, lib.rs |
| 2 (GREEN) | Implement email_send stub + prompt builder | 7e77c00 | sinks/email_send.rs, approval.rs, caprun/main.rs |

## What Was Built

**Proto variants** (`crates/brokerd/src/proto.rs`):
- `BrokerRequest::SubmitPlanNode { session_id, plan_node }` — closes RESEARCH Gap 3 data surface
- `BrokerResponse::PlanNodeDecision { decision }` — returns `ExecutorDecision` to caller
- Stub dispatch arm in both `brokerd/src/server.rs` and `cli/caprun/src/main.rs` ("wired in Plan 04")

**Audit query helpers** (`crates/brokerd/src/audit.rs`):
- `query_events_by_session(conn, session_id)` — returns all Events ordered by rowid, taint intact
- `find_event_by_type(conn, session_id, event_type)` — returns first typed Event with payload deserialized
- 3 unit tests: taint round-trip (ExternalUntrusted + EmailRaw), missing-type None, session enumeration

**Mediated email.send sink stub** (`crates/brokerd/src/sinks/email_send.rs`):
- `invoke_email_send_stub` appends an `email_send_stub` Event (actor: `sink-stub:email.send`, empty taint)
- No SMTP or network call; PlanNode dropped so no opaque handles appear in audit payload
- Unit test: asserts event findable via `find_event_by_type` with correct actor/type/taint

**Literal-value confirmation prompt builder** (`crates/brokerd/src/approval.rs`):
- `ConfirmationPrompt { raw_recipient, canonical_address, domain, known_contact, source_event_id, taint }`
- `build_confirmation_prompt`: trims ASCII whitespace for v0 canonicalisation; raw == canonical for simple case
- 3 unit tests: literal fidelity ("accounts@ev1l.com"), whitespace trim, malformed address empty domain

## Verification Results

```
cargo test -p brokerd     → 8/8 passed (all unit tests green)
cargo build --workspace   → Finished (clean)
No executor crate dependency added to brokerd (confirmed via Cargo.toml grep)
```

## TDD Gate Compliance

- RED commit: `32a03e6` — test(04-03): 3 tests failing as expected before implementation
- GREEN commit: `7e77c00` — feat(04-03): all 8 tests passing after implementation
- No REFACTOR needed (code is clean as written)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added SubmitPlanNode stub arm to `cli/caprun/src/main.rs`**
- **Found during:** Task 2 GREEN — `cargo build --workspace` failed
- **Issue:** New `BrokerRequest::SubmitPlanNode` variant made the match in `caprun/src/main.rs:196` non-exhaustive
- **Fix:** Added stub arm returning `BrokerResponse::Error { message: "SubmitPlanNode not wired until Plan 04" }` — same pattern as `server.rs`
- **Files modified:** `cli/caprun/src/main.rs`
- **Commit:** 7e77c00

## Known Stubs

| Stub | File | Line | Reason |
|------|------|------|--------|
| `SubmitPlanNode` dispatch in server.rs | crates/brokerd/src/server.rs | ~178 | Executor integration lands in Plan 04 |
| `SubmitPlanNode` dispatch in caprun | cli/caprun/src/main.rs | ~267 | Same — Plan 04 wires executor |
| `known_contact: false` | crates/brokerd/src/approval.rs | ~95 | v0 stub; user contact store lookup is post-v0 |

## Threat Surface

All mitigations from the threat register are implemented:

| Threat | Mitigation | Status |
|--------|-----------|--------|
| T-04-04 (Info Disclosure — prompt) | raw_recipient == canonical_address; domain extracted; known_contact false | Implemented |
| T-04-05 (Repudiation — sink) | Every invocation appends email_send_stub Event to audit DAG | Implemented |
| T-04-SC (Tampering — deps) | No new external crates; only existing workspace deps used | Confirmed |

## Self-Check: PASSED

Files verified:
- crates/brokerd/src/sinks.rs — FOUND
- crates/brokerd/src/sinks/email_send.rs — FOUND
- crates/brokerd/src/approval.rs — FOUND

Commits verified:
- 7e81369 (proto + audit helpers) — in log
- 32a03e6 (TDD RED) — in log
- 7e77c00 (TDD GREEN) — in log
