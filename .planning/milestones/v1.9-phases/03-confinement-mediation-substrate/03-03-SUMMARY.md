---
phase: 03-confinement-mediation-substrate
plan: "03"
subsystem: brokerd-reference-monitor
tags: [brokerd, rusqlite, sha256, audit-dag, uds-ipc, session, tokio, hash-chain, tamper-evidence]
dependencies:
  requires:
    - 03-01-SUMMARY  # workspace skeleton + verified abstract-UDS pattern
    - 03-02-SUMMARY  # sandbox confinement (Wave 2)
  provides:
    - REQ-brokerd-core: Session create, SQLite audit-DAG hash-chain, abstract-UDS IPC server
    - append_event / verify_chain for Phase 4 taint-chain verification
    - persist_session for broker-owned session lifecycle
  affects:
    - 03-04-PLAN  # adapter-fs uses brokerd::audit for fd_granted events
    - 03-05-PLAN  # caprun wires RequestFd + ReportRead end-to-end
tech-stack:
  added: []
  patterns:
    - SHA-256 hash-chain with recursive CTE walk for tamper detection
    - Arc<Mutex<rusqlite::Connection>> for safe sharing between tokio tasks
    - std::sync::Mutex held only in sync blocks (never across await) in async context
    - Abstract-namespace UDS via tokio native Approach A (NUL-prefix → from_abstract_name)
    - serde errors → BrokerResponse::Error (never panic); internal detail eprintln-only (T-03-09)
    - 64 KiB guard before allocation (T-03-08 DoS mitigate)
key-files:
  created: []
  modified:
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/session.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/audit_dag.rs
    - crates/brokerd/tests/uds_ipc.rs
    - crates/brokerd/Cargo.toml
decisions:
  - "verify_chain uses recursive CTE (parent_id → id traversal) to walk the chain in depth order; simpler than in-memory topological sort and correct for linear chains used in Phase 3"
  - "Arc<Mutex<rusqlite::Connection>> chosen over per-task connections: in-memory DB cannot be shared across separate Connection objects (each :memory: connection is independent); Arc<Mutex> ensures test and server share the same in-memory DB"
  - "dispatch() is a sync fn called from async handle_connection: std::sync::Mutex held only while running persist_session + append_event (both synchronous); never held across an await point — safe pattern per tokio docs"
  - "RequestFd and ReportRead return Error('not wired until Plan 05'): fd-pass is Plan 04 (adapter-fs) and end-to-end wiring is Plan 05 (caprun)"
  - "Both UDS IPC tests gated #[cfg(target_os = 'linux')]: abstract-namespace UDS is a Linux kernel extension; cargo test -p brokerd exits 0 on macOS (0 tests from uds_ipc.rs)"
metrics:
  duration_minutes: 4
  completed_date: "2026-06-29"
  tasks_completed: 2
  files_created: 0
  files_modified: 6
status: complete
---

# Phase 3 Plan 3: brokerd Reference Monitor — Audit DAG + Session + UDS IPC Summary

Expanded brokerd from the Wave-0 stub into the reference monitor: a SHA-256 hash-chained
SQLite audit DAG (append-only, tamper-evident), Session creation persisted to SQLite,
and a tokio abstract-namespace UDS IPC server that accepts framed requests and dispatches
CreateSession. Delivers REQ-brokerd-core (Phase 3 Success Criterion 2).

## What Was Built

### Task 1 — Audit DAG: schema, hash-chained append, chain verification (commits 26b416f, 84af4ab)

**`crates/brokerd/src/audit.rs`** expanded with three new public functions:

- **`compute_event_hash(parent_hash, id, session_id, event_type, payload, taint) -> String`**
  SHA-256 over the ordered concatenation of six canonical fields; `hex::encode` of the
  finalized digest. Root events pass `parent_hash = None` → hash over empty string prefix.

- **`append_event(conn, &Event, parent_hash) -> anyhow::Result<String>`**
  Reuses `runtime_core::Event` directly. Serializes the full Event to JSON for `payload`;
  `event.taint` to JSON for `taint`. Calls `compute_event_hash`, then INSERTs the row and
  returns the new hash. **No UPDATE or DELETE anywhere in this function** (append-only invariant).

- **`verify_chain(conn, session_id) -> bool`**
  Recursive CTE walk from root (`parent_id IS NULL`) through each linked event (`parent_id = c.id`).
  For each row: (1) asserts `stored.parent_hash == prev_hash`; (2) recomputes hash from stored
  fields; (3) asserts recomputed == stored. Returns `false` on any mismatch or empty chain.

**`crates/brokerd/tests/audit_dag.rs`** (replaces scaffold):
- `audit_hash_chain`: appends session_created → fd_granted → file_read, asserts `verify_chain = true` and verifies parent_hash linkage of each row.
- `tamper_breaks_chain`: appends one event, raw-UPDATEs its payload (test-only violation), asserts `verify_chain = false`.

Both tests pass on macOS and Linux (rusqlite bundled).

### Task 2 — Session persistence + abstract-UDS IPC server (commits 67fd617, 662cbae)

**`crates/brokerd/src/session.rs`** gains `persist_session(conn, &Session)`:
INSERT into `sessions (id, intent_id, status, created_at)` — status JSON-serialized for clean round-trip.

**`crates/brokerd/src/server.rs`** fully implemented:

```
run_broker_server(session_id, Arc<Mutex<Connection>>)
  └─ tokio::net::UnixListener::bind("\0/agentos/{session_id}")  [Approach A, verified native]
  └─ accept loop → tokio::spawn(handle_connection)
       └─ read 4-byte LE length prefix
       └─ GUARD: msg_len > 64KiB → Error("message too large"), never allocate  [T-03-08]
       └─ read body, serde_json::from_slice → Error on deserialize failure  [T-03-08]
       └─ dispatch(request, &conn)  [sync fn, no await while holding mutex]
            ├─ CreateSession { intent_id }:
            │    lock → persist_session + append_event(session_created, parent_hash=None)
            │    unlock → reply SessionCreated { session_id }
            ├─ RequestFd / ReportRead → Error("not wired until Plan 05")
       └─ send_response: 4-byte LE length + JSON body
```

**`crates/brokerd/tests/uds_ipc.rs`** (replaces scaffold, all gated `#[cfg(target_os = "linux")]`):
- `server_accept`: bind → accept → CreateSession → SessionCreated round-trip.
- `create_session_round_trip`: SessionCreated + verifies `sessions` row + `session_created` event in audit DAG.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Arc<Mutex<Connection>> instead of bare Connection**
- **Found during:** Task 2 GREEN design — `tokio::spawn` requires `Send + 'static`; `rusqlite::Connection` is `Send` but owned by the server loop; tests need the same in-memory DB instance.
- **Fix:** Changed `run_broker_server` signature to take `Arc<Mutex<rusqlite::Connection>>`. std::sync::Mutex held only in sync blocks (never across await) — safe per tokio guidelines.
- **Files modified:** crates/brokerd/src/server.rs, crates/brokerd/tests/uds_ipc.rs

**2. [Rule 2 - Missing Critical] dev-dependencies added to brokerd/Cargo.toml**
- **Found during:** Task 1 RED — integration tests in `tests/` cannot access the crate's `[dependencies]` directly; they need `[dev-dependencies]` to import `runtime_core::Event`, `rusqlite::params!`, etc.
- **Fix:** Added `[dev-dependencies]` block: runtime-core, rusqlite, uuid, chrono, tokio, serde_json.
- **Files modified:** crates/brokerd/Cargo.toml

## Threat Surface Scan

All threat mitigations from the plan's STRIDE register are implemented:

| Threat | Mitigation in Code |
|--------|--------------------|
| T-03-07 Tampering | SHA-256 hash-chain; STRICT tables; append_event uses INSERT-only; verify_chain detects any payload mutation |
| T-03-08 DoS (huge message) | Guard: `if msg_len > MAX_MSG_SIZE` before `vec![0u8; msg_len]`; serde errors → Error response, never panic |
| T-03-09 Info disclosure | Error responses carry generic messages ("message too large", "invalid request", "internal error"); internal detail via `eprintln!` only |
| T-03-10 Session spoofing | Sessions use `Uuid::new_v4()` per create_session |

No new security-relevant surface introduced beyond the plan's threat register.

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `RequestFd` → Error("not wired until Plan 05") | crates/brokerd/src/server.rs | Plan 04 implements SCM_RIGHTS fd-pass in adapter-fs; Plan 05 wires end-to-end |
| `ReportRead` → Error("not wired until Plan 05") | crates/brokerd/src/server.rs | Same — end-to-end wiring is Plan 05 scope |
| uds_ipc tests (0 on macOS) | crates/brokerd/tests/uds_ipc.rs | Abstract-namespace UDS is Linux-only; tests correctly cfg-gated |

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| crates/brokerd/src/audit.rs (compute_event_hash + append_event + verify_chain) | FOUND |
| crates/brokerd/src/session.rs (persist_session) | FOUND |
| crates/brokerd/src/server.rs (run_broker_server full impl) | FOUND |
| crates/brokerd/tests/audit_dag.rs (audit_hash_chain + tamper_breaks_chain) | FOUND |
| crates/brokerd/tests/uds_ipc.rs (server_accept + create_session_round_trip, Linux-gated) | FOUND |
| Commit 26b416f (Task 1 RED) | FOUND |
| Commit 84af4ab (Task 1 GREEN) | FOUND |
| Commit 67fd617 (Task 2 RED) | FOUND |
| Commit 662cbae (Task 2 GREEN) | FOUND |
| cargo test -p brokerd --test audit_dag (2 passed) | PASSED |
| cargo build --workspace exits 0 | PASSED |
| runtime_core::Event / Session reused, not redefined | VERIFIED |
| No UPDATE/DELETE on events in non-test code | VERIFIED |
