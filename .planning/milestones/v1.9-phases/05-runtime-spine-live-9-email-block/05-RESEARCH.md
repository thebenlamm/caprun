# Phase 5: Runtime Spine & Live §9 Email Block — Research

**Researched:** 2026-06-30
**Domain:** Rust IPC protocol refactor, broker dispatch unification, session-scoped value store, durable audit events
**Confidence:** HIGH — all claims derived from reading the actual codebase files in this session

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ASM-01 | Single `brokerd::server` dispatch path for RequestFd, read reporting, mint, evaluate, audit, sink invocation | `main.rs::handle_worker_connection` (dual dispatch) must be deleted; `brokerd::server` must absorb RequestFd + ReportClaims; see §Dual-Dispatch Reality |
| ASM-02 | `"SubmitPlanNode not wired until Plan 04"` stub removed; executor runs through live broker path | Stub at `cli/caprun/src/main.rs:278`; `brokerd::server::dispatch` already handles SubmitPlanNode correctly — fixing this is a consequence of collapsing dual dispatch |
| ASM-03 | Confined worker emits typed `ReportClaims` IPC with `EmailAddress` variant; fail-closed on unknown; raw bytes never cross broker | No `ReportClaims` type exists; `brokerd::quarantine::extract_email_claims` exists but only called from tests; worker sends only `ReportRead { bytes_read }` today |
| ASM-04 | `mint_from_read` produces authoritative `ValueId`s anchored to real `file_read` event in SQLite audit DAG | `mint_from_read` exists in `brokerd::quarantine` and is proven correct — must be wired into `ReportClaims` broker handler on the live path |
| HARD-03 | `ValueRecord`s session-scoped; cross-session handle denied; request-supplied `session_id` never trusted | Current `ValueStore` is a process-global `Arc<Mutex<ValueStore>>`; `SubmitPlanNode` carries `session_id` in the IPC message (HARD-03 violation); see §Session-Scoping |
| ACC-02 | Real `caprun` invocation on hostile input produces durable causal `sink_blocked` event; append-failure fails closed; block durable before CLI returns | Current dispatch appends `plan_node_evaluated` best-effort with `parent_id: None`; no `sink_blocked` event type; append errors are silently swallowed; see §Durable Sink-Blocked |
</phase_requirements>

---

## Summary

Phase 4 proved the value-injection defense in an in-process §9 test (51/51 passing). The
security logic is sound. Phase 5 wires that logic into the real `caprun` CLI — meaning a
developer who runs `caprun hostile_email.txt` gets a non-success exit code and a durable
`sink_blocked` audit event, not just a test assertion.

The dominant technical theme is **dual dispatch elimination**. The live `caprun` binary has two
separate IPC dispatch implementations: `main.rs::handle_worker_connection` handles
`RequestFd`/`ReportRead` (wired), and `brokerd::server::dispatch` handles
`CreateSession`/`SubmitPlanNode` (wired). These are entirely separate code paths. The broker
started by `caprun` uses `main.rs`'s dispatch, so `SubmitPlanNode` hits a stub error at
runtime. Collapsing these into a single `brokerd::server` dispatch path resolves ASM-01 and
ASM-02 together.

The second theme is **typed IPC claims**. The worker currently sends `ReportRead { bytes_read:
u64 }` with no semantic content. Phase 5 replaces this with a `ReportClaims { claims:
Vec<WorkerClaim> }` message carrying a bounded `EmailAddress` variant. The broker's
`ReportClaims` handler calls `mint_from_read` to anchor taint in the audit DAG and return
opaque `ValueId` handles — keeping raw content inside the confined worker.

The third theme is **durability and session scoping**. The current `plan_node_evaluated` event
is appended best-effort with `parent_id: None`. Phase 5 introduces a `sink_blocked` event type
appended durably (fail closed) with causal parent set, and scopes the `ValueStore` to each
connection so a handle from one session cannot resolve in another.

**Primary recommendation:** The cleanest implementation path is (1) add `ReportClaims` /
`WorkerClaim` / `ClaimsReceived` to `proto.rs`, (2) refactor `brokerd::server` to carry
per-connection state (`session_id`, `last_event_id`, `last_event_hash`, own `ValueStore`), (3)
wire `RequestFd` and `ReportClaims` into that stateful dispatch, (4) delete
`main.rs::handle_worker_connection` and use `brokerd::server`, (5) update `worker.rs` to do
extraction and send typed claims.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| IPC framing (length-prefix + JSON) | `brokerd::server` + `cli/caprun/src/worker.rs` | — | Both sides of UDS connection implement same framing |
| fd-pass (SCM_RIGHTS) | `brokerd::server` (broker side) | `adapter-fs` + `worker.rs` | Broker opens file and passes fd; worker receives via `adapter_fs::recv_fd` |
| Typed claim extraction | `cli/caprun/src/worker.rs` (confined, pre-IPC) | `brokerd::quarantine` (reused function) | Extraction happens inside confined worker; raw bytes never cross IPC boundary |
| ValueRecord minting (genuine taint anchor) | `brokerd::server` via `quarantine::mint_from_read` | — | Must happen on broker side with access to audit DB connection |
| Executor evaluation (I2 block) | `executor::submit_plan_node` via `brokerd::server::dispatch` | — | Pure function; called from stateful dispatch with per-connection ValueStore |
| Durable `sink_blocked` event | `brokerd::server::dispatch` | `brokerd::audit` | Broker appends event; fail-closed before returning any response |
| Session scoping of value handles | `brokerd::server::handle_connection` | — | Per-connection ValueStore; session_id from socket name, never from IPC message |
| CLI exit code propagation | `cli/caprun/src/main.rs` | `cli/caprun/src/worker.rs` | Worker exits non-success on BlockedPendingConfirmation; main.rs propagates it |

---

## Standard Stack

No new external libraries are required for Phase 5. All work uses the existing workspace
dependencies already proven in Phases 1-4.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio` | 1.52.3 | Async IPC server, spawn_blocking for fd-pass | Already in workspace; abstract UDS pattern proven |
| `rusqlite` (bundled) | 0.32 | Durable audit DAG append | Already in workspace; `append_event` proven |
| `serde` / `serde_json` | 1.0 | IPC framing serialization | Already in workspace; length-prefix + JSON pattern established |
| `uuid` | 1.23.4 | ValueId, EventId, SessionId | Already in workspace |
| `anyhow` | 1.0 | Error propagation | Already in workspace |

**No new dependencies.** Adding a dependency for Phase 5 would be a red flag.

---

## Package Legitimacy Audit

> SKIPPED — Phase 5 adds no new external packages. All dependencies are existing workspace
> members proven in Phases 1-4.

---

## Architecture Patterns

### System Architecture Diagram

```
 caprun (CLI)
   │ 1. open audit DB, create session, append session_created → session_id
   │ 2. bind \0/agentos/{session_id} (abstract UDS)
   │ 3. spawn caprun-worker env: BROKER_SOCK, WORKSPACE_FILE
   │
   ├──── brokerd::server::run_broker_server(session_id, conn, session_id_uuid, event_id, hash)
   │      │  accept loop → handle_connection(stream, conn, session_id, last_event_id, last_event_hash)
   │      │    per-connection ValueStore::default()
   │      │    mutable chain: last_event_id, last_event_hash
   │      │
   │      │ [RequestFd]──────────────────────────────────────────────────────────────────────┐
   │      │   broker opens file (ambient fs)                                                 │
   │      │   append fd_granted event (causal parent = last)                                 │
   │      │   spawn_blocking: pass_fd(sock_fd, file_fd)                                      │
   │      │   send FdGranted response                                                        │
   │      │   advance last_event_id, last_event_hash                                        │
   │      │                                                                                  │
   │      │ [ReportClaims { claims: [EmailAddress("accounts@ev1l.com")] }]                   │
   │      │   for each EmailAddress claim:                                                   │
   │      │     mint_from_read(conn, store, session_id, claim, last_hash)                    │
   │      │       → appends file_read event (taint=[ExternalUntrusted, EmailRaw])            │
   │      │       → mints ValueRecord { provenance_chain[0] = file_read event id }          │
   │      │       → returns (read_event_id, read_hash, value_id)                            │
   │      │   advance last_event_id, last_event_hash                                        │
   │      │   send ClaimsReceived { value_ids: [value_id] }                                 │
   │      │                                                                                  │
   │      │ [SubmitPlanNode { plan_node: {sink: email.send, args: [to=value_id]} }]          │
   │      │   executor::submit_plan_node(session_id, plan_node, &per-conn-store)            │
   │      │     → BlockedPendingConfirmation { literal, taint, provenance_chain }           │
   │      │   append sink_blocked event (DURABLE, causal parent = last, FAIL CLOSED)        │
   │      │   advance last_event_id, last_event_hash                                        │
   │      │   send PlanNodeDecision { BlockedPendingConfirmation }                          │
   │      │                                                                                  │
   └──── caprun-worker (confined)
          1. connect to broker socket
          2. apply_confinement() (Landlock deny-all, seccomp, rlimits)
          3. send RequestFd { path: workspace_file }
          4. recv_fd (SCM_RIGHTS) → file_fd
          5. recv FdGranted response
          6. read file_fd → raw_bytes (Landlock blocks open(); fd-only path)
          7. extract_email_claims(raw_str) → [Claim { "email_address", "accounts@ev1l.com" }]
          8. send ReportClaims { claims: [WorkerClaim::EmailAddress("accounts@ev1l.com")] }
          9. recv ClaimsReceived { value_ids: [value_id] }
         10. construct PlanNode { sink: "email.send", args: [PlanArg { "to", value_id }] }
         11. send SubmitPlanNode { plan_node }   (NO session_id field — connection-bound)
         12. recv PlanNodeDecision { BlockedPendingConfirmation }
         13. exit 1 (non-success — block detected)

 caprun (CLI) continues:
   wait child → status != 0
   print audit DAG + verify_chain
   exit 1 (non-success propagated)
```

### Recommended Project Structure

No structural changes to crate layout. All changes are within existing files:

```
crates/
├── brokerd/src/
│   ├── proto.rs          [MODIFY] add WorkerClaim enum, ReportClaims, ClaimsReceived; remove session_id from SubmitPlanNode
│   └── server.rs         [MODIFY] per-connection state, wire RequestFd + ReportClaims, durable sink_blocked
├── executor/             [NO CHANGE — pure logic unchanged]
├── runtime-core/         [NO CHANGE — types unchanged]
cli/caprun/src/
├── main.rs               [MODIFY] remove handle_worker_connection; use brokerd::server
└── worker.rs             [MODIFY] ReportClaims IPC; SubmitPlanNode; exit non-success on block
```

### Pattern 1: Per-Connection Stateful Dispatch

**What:** `handle_connection` maintains mutable state across multiple IPC messages from the same worker, threading the audit DAG chain and session-scoped ValueStore.

**When to use:** Any IPC connection that requires causal chaining (parent events reference prior events) or session-scoped resources.

**Example:**
```rust
// Source: pattern derived from cli/caprun/src/main.rs::handle_worker_connection (lines 154-284)
// and brokerd::server::handle_connection — unified form for Phase 5

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,                  // from socket context; never from IPC message
    mut last_event_id: Uuid,           // mutable chain state
    mut last_event_hash: String,       // mutable chain state
) -> anyhow::Result<()> {
    // Per-connection ValueStore — scoped to this session/connection.
    // A ValueId minted here does not exist in any other connection's store.
    let mut value_store = ValueStore::default();
    // ...dispatch loop reads messages and updates last_event_id/last_event_hash
}
```

### Pattern 2: WorkerClaim Bounded Enum (Fail-Closed on Unknown)

**What:** A bounded tagged enum for claims crossing the worker→broker IPC boundary. Unknown variants from future protocol versions fail closed at the broker.

**When to use:** Any typed extract that must never pass raw bytes to the planner, and where the set of claim kinds is intentionally limited.

**Example:**
```rust
// Source: pattern from REQUIREMENTS.md ASM-03 + DESIGN-taint-model.md §Worker Output Contract

/// Typed, lossy claim emitted by a confined worker.
/// Phase 5 ships EmailAddress only. Phase 7 adds RelativePath.
/// Unknown variants (future/unknown) never appear here — serde deny_unknown_fields
/// or explicit exhaustive matching causes an error response, not a Proceed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim {
    EmailAddress(String),
    // RelativePath(String),  // Phase 7
}

// In BrokerRequest:
ReportClaims { claims: Vec<WorkerClaim> },
// In BrokerResponse:
ClaimsReceived { value_ids: Vec<runtime_core::plan_node::ValueId> },
```

### Pattern 3: Durable Fail-Closed Audit Append

**What:** Unlike the current best-effort audit append in `SubmitPlanNode` dispatch, the `sink_blocked` event append must fail closed — if the append fails, the broker returns an error and the worker exits non-success. The block is not reported to the caller until it is durable.

**When to use:** Any audit event whose absence would allow a blocked effect to appear unrecorded (i.e., any event in the critical security path).

**Example:**
```rust
// Source: contrast with current server.rs lines 219-223 (best-effort swallowing)
// Phase 5 durable pattern:

let blocked_event = Event {
    id: Uuid::new_v4(),
    parent_id: Some(last_event_id),   // causal parent preserved (not None)
    session_id,
    actor: "executor".into(),
    event_type: "sink_blocked".into(),
    timestamp: Utc::now(),
    taint: vec![],
};
let blocked_hash = match conn.lock() {
    Ok(locked) => append_event(&locked, &blocked_event, Some(&last_event_hash))
        .map_err(|e| {
            eprintln!("[brokerd] sink_blocked audit append FAILED (fail-closed): {e}");
            // Return error to worker — block is not durable, cannot proceed
            anyhow::anyhow!("audit append failed")
        })?,
    Err(e) => return Err(anyhow::anyhow!("mutex poisoned: {e}")),
};
// Only send PlanNodeDecision AFTER durable append:
last_event_id = blocked_event.id;
last_event_hash = blocked_hash;
send_response(&mut stream, &BrokerResponse::PlanNodeDecision { decision }).await?;
```

### Pattern 4: SCM_RIGHTS fd-pass in Stateful Dispatch

**What:** Moving the fd-pass logic from `main.rs::handle_worker_connection` into `brokerd::server::handle_connection` requires the raw fd of the tokio stream. The existing `spawn_blocking` pattern is unchanged.

**When to use:** Any `RequestFd` dispatch arm in an async handler.

**Example:**
```rust
// Source: cli/caprun/src/main.rs lines 198-239 — move verbatim into brokerd::server
BrokerRequest::RequestFd { path } => {
    let file = std::fs::File::open(&path)...;
    let file_fd = file.as_raw_fd();
    // append fd_granted event ...
    let sock_fd = stream.as_raw_fd();
    tokio::task::spawn_blocking(move || {
        let result = pass_fd(sock_fd, file_fd).map_err(...);
        drop(file);  // close broker copy after sendmsg
        result
    }).await??;
    send_response(&mut stream, &BrokerResponse::FdGranted).await?;
    // advance chain...
}
```

### Anti-Patterns to Avoid

- **Best-effort audit append on security path:** Swallowing an `append_event` error on `sink_blocked` violates ACC-02. Any error in the durable-audit path must propagate as an error response to the worker.
- **`parent_id: None` on security events:** Events in the causal chain must carry their parent. Setting `parent_id: None` on `sink_blocked` breaks chain verification and the causal audit requirement.
- **Trusting request-supplied `session_id`:** `BrokerRequest::SubmitPlanNode` currently carries a `session_id` field. The broker MUST use the session_id established at connection time (from the socket name), not the value in the message. Remove the field from the protocol or ignore it entirely.
- **Taint stapling in ReportClaims handler:** The broker's `ReportClaims` handler must call `mint_from_read` — not mint a ValueRecord directly with a fabricated provenance_chain. `mint_from_read` appends the `file_read` event AND mints the record in one call, keeping provenance_chain[0] == real file_read event id.
- **Raw bytes in `ReportClaims` message:** The `WorkerClaim::EmailAddress(String)` carries only the extracted address, never the surrounding hostile sentence. The worker calls `extract_email_claims` locally before building the `ReportClaims` message.
- **Shared ValueStore across connections:** The current `Arc<Mutex<ValueStore>>` passed to all connections allows cross-session handle resolution. Phase 5 creates a fresh `ValueStore::default()` per accepted connection inside `handle_connection`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| fd-pass across process boundary | Custom sendmsg wrapper | `adapter_fs::pass_fd` + `adapter_fs::recv_fd` (existing, proven) | SCM_RIGHTS has subtle ordering requirements; already correct in adapter-fs |
| Email claim extraction | New regex/NLP | `brokerd::quarantine::extract_email_claims` (existing, tested) | Already covers the §9 hostile content scenario; 3 passing tests |
| Genuine-taint minting | Custom `ValueStore::mint` call with fabricated chain | `brokerd::quarantine::mint_from_read` (existing) | mint_from_read is the sole canonical taint-mint site; splitting this breaks the anti-stapling invariant |
| Audit hash chaining | Custom hash logic | `brokerd::audit::append_event` + `verify_chain` | SHA-256 chain with parent_hash threading is proven across 51 tests |
| IPC framing | New wire format | Existing 4-byte LE prefix + JSON serde pattern | Established, tested in uds_ipc.rs + uds_abstract_spike.rs |
| Session ID binding | Per-message authentication | Socket name binding (`\0/agentos/{session_id}`) | Abstract UDS name IS the session; worker connects to its session's socket by construction |

**Key insight:** Phase 5 is pure assembly — wiring proven components (mint_from_read, submit_plan_node, append_event, extract_email_claims, pass_fd) into a single dispatch path. No new logic is invented.

---

## Dual-Dispatch Reality (ASM-01 / ASM-02)

### The Two Separate Dispatch Loops

**Loop A — `cli/caprun/src/main.rs::handle_worker_connection` (lines 154-284):**
- Handles: `RequestFd` (WIRED), `ReportRead` (WIRED), `CreateSession` (error), `SubmitPlanNode` (STUB: "not wired until Plan 04" — line 278)
- Uses: stateful chain state (last_event_id, last_event_hash tracked across messages)
- This is what the real `caprun` binary actually runs

**Loop B — `crates/brokerd/src/server.rs::dispatch` (lines 147-227):**
- Handles: `CreateSession` (WIRED), `SubmitPlanNode` (WIRED), `RequestFd`/`ReportRead` (STUB: "not wired until Plan 05" — line 182)
- Uses: stateless dispatch (no per-connection chain state; `parent_id: None` on all events)
- This is brokerd::server::run_broker_server — NOT used by the real caprun binary

**Consequence:** When `caprun` runs today and a worker sends `SubmitPlanNode`, it hits Loop A's stub and gets `Error { message: "SubmitPlanNode not wired until Plan 04" }`. The executor is never invoked on the live path.

### What Unification Requires

Phase 5 must produce ONE dispatch path:
1. `brokerd::server::handle_connection` absorbs all of Loop A's logic (RequestFd with SCM_RIGHTS, chain state tracking)
2. `brokerd::server::handle_connection` adds `ReportClaims` dispatch (new)
3. `brokerd::server::dispatch` gets `RequestFd` wired (removing the "not wired until Plan 05" stub)
4. `cli/caprun/src/main.rs::handle_worker_connection` is **deleted entirely**
5. `cli/caprun/src/main.rs` calls `brokerd::server::run_broker_server` instead of its own accept loop

### Signature Change for `run_broker_server`

Current:
```rust
pub async fn run_broker_server(
    session_id: &str,
    conn: Arc<Mutex<rusqlite::Connection>>,
    value_store: Arc<Mutex<ValueStore>>,    // global shared — HARD-03 violation
) -> anyhow::Result<()>
```

Phase 5:
```rust
pub async fn run_broker_server(
    session_id: &str,             // socket name (string form)
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id_uuid: Uuid,        // connection-established session identity
    initial_last_event_id: Uuid,  // from session_created event (minted by main)
    initial_last_event_hash: String,
) -> anyhow::Result<()>
// ValueStore is no longer a parameter — each handle_connection creates its own
```

The `value_store: Arc<Mutex<ValueStore>>` is removed from `run_broker_server`; each accepted connection creates `ValueStore::default()` in `handle_connection`.

---

## `ReportClaims` IPC + `EmailAddress` Claim (ASM-03)

### Current State

`crates/brokerd/src/proto.rs` has no `ReportClaims` variant. The worker sends:
```rust
BrokerRequest::ReportRead { bytes_read: u64 }
```
This reports only a byte count. The broker appends a `file_read` event with `taint: []` (no taint on the workspace file in the Phase 3 substrate demo). The extraction code exists in `brokerd::quarantine::extract_email_claims` but is never called on the live path.

### New Protocol Types

Add to `crates/brokerd/src/proto.rs`:

```rust
/// Typed, lossy claim from a confined worker. The raw source bytes that produced
/// this claim never appear in the IPC message — only the extracted typed value.
/// Phase 5 ships EmailAddress; Phase 7 adds RelativePath (no second IPC revision).
///
/// Unknown variants (forward-compat from future versions) fail closed:
/// the broker returns an Error response; it never Proceeds on an unrecognized claim kind.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim {
    EmailAddress(String),
    // RelativePath(String),  // Phase 7
}
```

Add to `BrokerRequest`:
```rust
/// Worker read a file and extracted typed claims. The raw file bytes are NOT included.
/// Broker mints a ValueRecord per claim via mint_from_read and returns opaque handles.
ReportClaims { claims: Vec<WorkerClaim> },
```

Add to `BrokerResponse`:
```rust
/// Acknowledgement for ReportClaims: opaque ValueId handles for each minted claim,
/// in the same order as the claims in the ReportClaims message.
ClaimsReceived { value_ids: Vec<runtime_core::plan_node::ValueId> },
```

### Fail-Closed for Unknown Claim Kinds

Serde's `#[serde(tag = "kind")]` will fail to deserialize an unknown `kind` value because the enum is exhaustive. The broker's existing deserialization error path already returns `BrokerResponse::Error { message: "invalid request" }` (server.rs lines 121-131). This path already fails closed — no code change needed for the error handling. Just make sure there is no wildcard arm that treats unknown WorkerClaim variants as EmailAddress.

### Worker Changes (worker.rs)

Replace steps 3-9 in the current worker protocol (lines 64-97) with:

```rust
// Steps 3-5: RequestFd → recv_fd → FdGranted (unchanged)
send_framed(&std_stream, &BrokerRequest::RequestFd { path: workspace_file })?;
let file_fd = adapter_fs::recv_fd(sock_fd)?;
let _granted: BrokerResponse = recv_framed(&std_stream)?;

// Step 6: Read file bytes via passed fd (unchanged — Landlock blocks open())
let raw_bytes: Vec<u8> = { let mut f = unsafe { File::from_raw_fd(file_fd) }; ... };
let raw_str = String::from_utf8_lossy(&raw_bytes);

// Step 7 (NEW): Extract typed claims inside confined worker
// Raw sentence is discarded; only extracted addresses cross the IPC boundary.
use brokerd::quarantine::{extract_email_claims, Claim};
let claims: Vec<brokerd::proto::WorkerClaim> = extract_email_claims(&raw_str)
    .into_iter()
    .map(|c| brokerd::proto::WorkerClaim::EmailAddress(c.value))
    .collect();

// Step 8 (NEW): Send ReportClaims (typed; no raw bytes)
send_framed(&std_stream, &BrokerRequest::ReportClaims { claims })?;

// Step 9 (NEW): Receive ValueId handles from broker
let BrokerResponse::ClaimsReceived { value_ids } = recv_framed(&std_stream)? else {
    anyhow::bail!("unexpected response to ReportClaims");
};

// Step 10 (NEW): Scripted planner constructs PlanNode (holds only opaque handles)
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId};
let plan_node = PlanNode {
    sink: SinkId("email.send".into()),
    args: vec![PlanArg { name: "to".into(), value_id: value_ids[0].clone() }],
};

// Step 11 (NEW): Submit for I2 evaluation through live broker path
send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;

// Step 12 (NEW): Receive block decision
let BrokerResponse::PlanNodeDecision { decision } = recv_framed(&std_stream)? else {
    anyhow::bail!("unexpected response to SubmitPlanNode");
};

// Step 13 (NEW): Exit non-success if blocked (before any effect executes)
use runtime_core::ExecutorDecision;
if matches!(decision, ExecutorDecision::BlockedPendingConfirmation { .. }) {
    eprintln!("[worker] BLOCKED: value-injection defense triggered — exiting 1");
    std::process::exit(1);
}
```

---

## `mint_from_read` + Audit DAG Anchoring (ASM-04)

### Current State (Proven in Phase 4)

`brokerd::quarantine::mint_from_read` (lines 124-158) is already correct:
- Builds a `file_read` Event with `taint: [ExternalUntrusted, EmailRaw]`
- Calls `append_event(conn, &event, parent_hash)` → returns `read_hash`
- Calls `store.mint(claim.value, taint, vec![event_id])` → returns `ValueId`
- Returns `(event_id, read_hash, value_id)` as a triplet
- `provenance_chain[0] == event_id` is guaranteed by construction

### Wiring into ReportClaims Handler

The `ReportClaims` broker handler in Phase 5 calls `mint_from_read` for each `EmailAddress` claim:

```rust
BrokerRequest::ReportClaims { claims } => {
    let mut value_ids = Vec::new();
    for claim in claims {
        match claim {
            WorkerClaim::EmailAddress(addr) => {
                let quarantine_claim = brokerd::quarantine::Claim {
                    claim_type: "email_address".into(),
                    value: addr,
                };
                let (read_event_id, read_hash, value_id) = {
                    let mut store = value_store.lock()...;
                    let locked_conn = conn.lock()...;
                    mint_from_read(&locked_conn, &mut store, session_id,
                                   &quarantine_claim, Some(&last_event_hash))?
                };
                last_event_id = read_event_id;
                last_event_hash = read_hash;
                value_ids.push(value_id);
            }
            // future claim kinds that aren't yet handled:
            // _ => return error (fail-closed)
        }
    }
    send_response(&mut stream, &BrokerResponse::ClaimsReceived { value_ids }).await?;
}
```

**Critical:** `mint_from_read` receives `Some(&last_event_hash)` so the `file_read` event has a proper causal parent (the `fd_granted` event). This threads the causal chain correctly.

### session_id Anchoring

`mint_from_read` takes `session_id: Uuid` which comes from the connection context, not from any IPC message. This satisfies HARD-03: the audit event's `session_id` is broker-authoritative.

---

## Session-Scoping (HARD-03)

### Current HARD-03 Violation

1. **Shared ValueStore**: `brokerd::server::run_broker_server` takes `value_store: Arc<Mutex<ValueStore>>` and passes it to EVERY `handle_connection`. A `ValueId` minted in one worker's session would resolve in another worker's session.

2. **Request-supplied session_id**: `BrokerRequest::SubmitPlanNode { session_id, plan_node }` carries a `session_id` from the worker. The current dispatch uses this value directly: `executor::submit_plan_node(session_id, &plan_node, &store)`. An attacker-controlled worker could supply any session_id.

### Phase 5 Fix

**Per-connection ValueStore:** Remove `value_store` from `run_broker_server` signature. In `handle_connection`, create `let mut value_store = ValueStore::default()` at the top. This store exists only for the lifetime of this connection. A handle minted in session A's connection does not exist in session B's store — cross-session resolution returns `None` → `Denied`.

**Remove session_id from SubmitPlanNode:** The broker knows the session from the connection context (passed as `session_id: Uuid` into `handle_connection`). Remove the `session_id` field from `BrokerRequest::SubmitPlanNode`:

```rust
// Before (HARD-03 violation):
BrokerRequest::SubmitPlanNode { session_id: uuid::Uuid, plan_node: PlanNode }

// After (HARD-03 compliant):
BrokerRequest::SubmitPlanNode { plan_node: PlanNode }
// Broker uses connection-established session_id (from run_broker_server parameter)
```

**Breaking change scope:** `BrokerRequest::SubmitPlanNode` field removal affects:
- `crates/brokerd/src/proto.rs` (definition)
- `crates/brokerd/src/server.rs` (dispatch arm)
- `cli/caprun/src/worker.rs` (construction — no field to set)
- `crates/brokerd/tests/s9_acceptance.rs` does NOT use IPC; calls `executor::submit_plan_node` directly — unaffected
- `crates/executor/tests/executor_decision.rs` does NOT use IPC — unaffected
- `crates/brokerd/tests/uds_ipc.rs` — check if it constructs SubmitPlanNode via IPC

---

## Durable Causal `sink_blocked` Event (ACC-02)

### Current Problem (server.rs lines 209-223)

```rust
// CURRENT — three violations:
let eval_event = Event {
    parent_id: None,           // VIOLATION 1: no causal parent
    event_type: "plan_node_evaluated".into(),  // wrong type for a block
    ...
};
if let Ok(locked) = conn.lock() {
    if let Err(e) = append_event(&locked, &eval_event, None) {
        eprintln!("...");     // VIOLATION 2: swallowed error
    }
}
// VIOLATION 3: send PlanNodeDecision regardless of whether append succeeded
BrokerResponse::PlanNodeDecision { decision }
```

### Phase 5 Fix

When executor returns `BlockedPendingConfirmation`:

```rust
BrokerRequest::SubmitPlanNode { plan_node } => {
    let decision = {
        let store = value_store.lock().map_err(|e| anyhow::anyhow!("mutex: {e}"))?;
        executor::submit_plan_node(session_id, &plan_node, &store)
    };

    // Durably record the block outcome BEFORE returning any response.
    // append-failure → error response to worker → worker exits non-success
    // The block is not reported to the caller until it is in SQLite.
    let event_type = match &decision {
        ExecutorDecision::BlockedPendingConfirmation { .. } => "sink_blocked",
        ExecutorDecision::Allowed => "sink_allowed",  // for completeness; not tested in Phase 5
        _ => "plan_node_evaluated",
    };
    let audit_event = Event {
        id: Uuid::new_v4(),
        parent_id: Some(last_event_id),   // causal parent — REQUIRED
        session_id,
        actor: "executor".into(),
        event_type: event_type.into(),
        timestamp: Utc::now(),
        taint: vec![],
    };
    let new_hash = {
        let locked = conn.lock().map_err(|e| anyhow::anyhow!("mutex: {e}"))?;
        append_event(&locked, &audit_event, Some(&last_event_hash))
            .map_err(|e| {
                eprintln!("[brokerd] {event_type} audit append FAILED (fail-closed): {e}");
                anyhow::anyhow!("audit append failed: {e}")
            })?
    };
    // Advance chain only after successful append
    last_event_id = audit_event.id;
    last_event_hash = new_hash;

    // Only now send the decision — block is durable at this point
    send_response(&mut stream, &BrokerResponse::PlanNodeDecision { decision }).await?;
}
```

**What changes from current:**
1. `parent_id: None` → `parent_id: Some(last_event_id)` (causal parent preserved)
2. `event_type: "plan_node_evaluated"` → `"sink_blocked"` when decision is Block
3. Best-effort `if let Err(e)` swallow → fail-closed `?` propagation
4. `BrokerResponse` sent ONLY after successful audit append

**CLI exit code propagation:** `main.rs` already has:
```rust
if !child_status.success() {
    anyhow::bail!("caprun-worker exited with status: {child_status}");
}
```
When the worker receives `BlockedPendingConfirmation` and calls `std::process::exit(1)`, `child.wait()` returns non-success, and `caprun` exits non-zero. No change needed in main.rs for this behavior.

---

## Common Pitfalls

### Pitfall 1: Taint Stapling in ReportClaims Handler

**What goes wrong:** Developer manually calls `value_store.mint(addr, vec![ExternalUntrusted, EmailRaw], vec![fabricated_uuid])` instead of `mint_from_read`.

**Why it happens:** `mint` looks simpler. But without calling `append_event` first, there's no real `file_read` event in the audit DAG. `provenance_chain[0]` points to a UUID that doesn't exist in the DAG.

**How to avoid:** Use ONLY `mint_from_read` in the `ReportClaims` handler. Never call `ValueStore::mint` directly with a non-empty taint vector on the live broker path.

**Warning signs:** The §9 test's anti-stapling assertion (`find_event_by_type` returning None for the UUID in provenance_chain) would fail. Also, `check-invariants.sh` Gate 1 doesn't catch this — only test coverage does.

### Pitfall 2: SCM_RIGHTS fd-pass Stalling Tokio Reactor

**What goes wrong:** Calling `pass_fd` (sendmsg) directly on a tokio task without `spawn_blocking` stalls the entire tokio reactor thread.

**Why it happens:** `sendmsg` is a blocking syscall. Tokio's reactor is single-threaded by default.

**How to avoid:** The existing `main.rs` code (lines 225-232) already uses `spawn_blocking`. Move this pattern verbatim into `brokerd::server`. Do not simplify by removing `spawn_blocking`.

**Warning signs:** Integration tests hang instead of completing. This only surfaces on Linux with a real socket.

### Pitfall 3: Shared ValueStore Across Connections

**What goes wrong:** Passing `Arc<Mutex<ValueStore>>` through to all connections (current pattern). A ValueId minted in connection A resolves in connection B.

**Why it happens:** The existing `run_broker_server` signature takes a shared value_store. Easy to keep it.

**How to avoid:** Remove `value_store` from `run_broker_server`. Create fresh `ValueStore::default()` in each `handle_connection` invocation.

**Warning signs:** The HARD-03 cross-session denial test fails (handle from session A resolves in session B's dispatch).

### Pitfall 4: Trusting session_id from SubmitPlanNode Message

**What goes wrong:** Keeping `session_id: Uuid` in `BrokerRequest::SubmitPlanNode` and using the message value in the dispatch.

**Why it happens:** Current code does exactly this. It's a one-line fix to use the connection-established session_id instead.

**How to avoid:** Remove the field entirely or ignore it. The broker already knows the session_id from `handle_connection`'s parameter.

**Warning signs:** No unit test will catch this directly — need a test that asserts cross-session handle denial.

### Pitfall 5: Breaking the §9 Test

**What goes wrong:** Renaming or restructuring `brokerd::quarantine`, `mint_from_read`, `extract_email_claims`, or changing `ValueRecord` / `ValueId` types.

**Why it happens:** These types are used directly in `crates/brokerd/tests/s9_acceptance.rs`.

**How to avoid:** Phase 5 does NOT change the executor logic or the quarantine module. Protocol changes (proto.rs) are additive (new variants) + one removal (session_id from SubmitPlanNode). Check that `s9_acceptance.rs` doesn't use SubmitPlanNode IPC.

**Warning signs:** `cargo test -p brokerd --test s9_acceptance` fails.

### Pitfall 6: dispatch() Becoming Untestable After Stateful Refactor

**What goes wrong:** Inlining all state directly into `handle_connection` makes it impossible to unit-test individual dispatch arms.

**How to avoid:** Extract per-request dispatch logic into a `dispatch_request(request, &mut ConnectionState, &Arc<Mutex<Connection>>) -> BrokerResponse` function that takes the connection state by mutable reference. Tests can construct `ConnectionState` directly without a real socket.

---

## Validation Architecture

> Nyquist validation is enabled (not explicitly false in .planning/config.json assumed absent).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` |
| Config file | `Cargo.toml` workspace; no separate test config |
| Quick run command | `cargo test --workspace --no-fail-fast` |
| Full suite command | `cargo test --workspace --no-fail-fast` (same; no separate integration harness) |
| Linux-only enforcement | `#[cfg(target_os = "linux")]` gate on any test spawning a real confined worker |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | Platform | File Exists? |
|--------|----------|-----------|-------------------|----------|--------------|
| ASM-01 | No second dispatch loop; single brokerd::server path | structural (grep) | `grep -rn "handle_worker_connection" cli/caprun/src/main.rs` → 0 hits | any | ❌ Wave 0 |
| ASM-01 | RequestFd routes through brokerd::server dispatch | integration (in-process) | `cargo test -p brokerd -- requestfd_wired` | any | ❌ Wave 0 |
| ASM-02 | SubmitPlanNode not wired stub gone | structural (grep) | `grep -rn "not wired until Plan" crates/ cli/` → 0 hits | any | ❌ Wave 0 |
| ASM-03 | ReportClaims serializes/deserializes correctly | unit | `cargo test -p brokerd -- proto_report_claims` | any | ❌ Wave 0 |
| ASM-03 | Unknown WorkerClaim kind fails closed | unit | `cargo test -p brokerd -- worker_claim_unknown_fails_closed` | any | ❌ Wave 0 |
| ASM-03 | Raw bytes not in WorkerClaim::EmailAddress | unit (type-system) | compile-time (no String field named "raw_sentence") | any | implicit |
| ASM-04 | mint_from_read called from ReportClaims handler on live path | integration | `cargo test -p brokerd -- report_claims_mints_value_record` | any | ❌ Wave 0 |
| HARD-03 | Handle from session A denied in session B store | unit | `cargo test -p brokerd -- cross_session_handle_denied` | any | ❌ Wave 0 |
| HARD-03 | session_id not trusted from SubmitPlanNode message | unit | `cargo test -p brokerd -- submit_plan_node_ignores_request_session` | any | ❌ Wave 0 |
| ACC-02 | sink_blocked event appended durably before response | unit | `cargo test -p brokerd -- sink_blocked_durable_before_response` | any | ❌ Wave 0 |
| ACC-02 | sink_blocked has causal parent_id (not None) | unit | `cargo test -p brokerd -- sink_blocked_has_causal_parent` | any | ❌ Wave 0 |
| ACC-02 | append failure returns error to worker (fail-closed) | unit | `cargo test -p brokerd -- sink_blocked_append_failure_fail_closed` | any | ❌ Wave 0 |
| ACC-02 | CLI exits non-success on block | e2e (linux-only) | `cargo test -p caprun --test s9_live_block -- s9_live_caprun_exits_nonzero` | linux | ❌ Wave 0 |
| ACC-02 | sink_blocked event exists in audit DAG after live run | e2e (linux-only) | `cargo test -p caprun --test s9_live_block -- s9_live_sink_blocked_in_dag` | linux | ❌ Wave 0 |
| §9 guard | Existing §9 acceptance test still passes | regression | `cargo test -p brokerd --test s9_acceptance` | any | ✅ exists |

### macOS vs Linux Test Split

Tests marked "any" run on the dev box (macOS). Tests marked "linux" require Colima+Docker:
```bash
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 cargo test --workspace --no-fail-fast
```

The live e2e test (`s9_live_block`) spawns a real `caprun-worker` binary that calls `apply_confinement()`. On Linux, this is a real confined process. On macOS it would fail at the Landlock syscall. Gate with `#[cfg(target_os = "linux")]`.

### Sampling Rate

- **Per task commit:** `cargo test --workspace --no-fail-fast` (all cross-platform tests)
- **Per wave merge:** same + grep invariant checks
- **Phase gate:** `cargo test -p brokerd --test s9_acceptance` must still pass; full workspace suite green; Linux e2e test green on Colima

### Wave 0 Gaps (Tests to Create Before Implementation)

- [ ] `crates/brokerd/tests/phase5_dispatch.rs` — covers ASM-01, ASM-02, ASM-03, ASM-04, HARD-03, ACC-02 unit tests
- [ ] `cli/caprun/tests/s9_live_block.rs` — `#[cfg(target_os = "linux")]` e2e tests for ACC-02

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | `WorkerClaim` bounded enum (serde exhaustive match); unknown kinds error response |
| V4 Access Control | yes | Per-connection ValueStore (session scope); connection-established session_id |
| V6 Cryptography | yes | SHA-256 audit chain (`append_event` with `parent_hash`) — must not be bypassed by best-effort logic |
| V2 Authentication | no | No user authentication in scope for Phase 5 |
| V3 Session Management | yes (partial) | Session-connection binding via abstract socket name; no credential theft vector in scope |

### Known Threat Patterns for This Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-session handle forgery | Elevation of Privilege | Per-connection ValueStore; cross-session ValueId resolves None → Denied |
| Session spoofing via request-supplied session_id | Spoofing | Remove field from SubmitPlanNode; broker uses socket-established session_id |
| Taint stapling in ReportClaims handler | Tampering | Use only mint_from_read (appends real file_read event + mints record atomically) |
| Audit evasion via best-effort append | Repudiation | Fail-closed append in sink_blocked handler; error returned before decision propagated |
| Raw bytes smuggled through WorkerClaim | Information Disclosure | WorkerClaim::EmailAddress(String) carries only the extracted address; lossy guarantee enforced at extract_email_claims call site in worker |
| Worker connects to wrong session socket | Spoofing | Abstract socket name includes session_id; worker spawned with BROKER_SOCK env matching the session |

---

## Runtime State Inventory

> Phase 5 is not a rename/refactor phase — this section is omitted per the "rename/refactor phases only" instruction.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | Build | ✓ | (existing workspace) | — |
| rusqlite (bundled) | Audit DB | ✓ | 0.32, bundled | — |
| tokio | Async UDS server | ✓ | 1.52.3 | — |
| Colima + Docker | Linux e2e tests | check before e2e | — | Skip e2e; run unit/integration only on macOS |

**Missing dependencies with no fallback:** None for unit/integration tests.
**Missing dependencies with fallback:** Colima+Docker — unit tests run on macOS; Linux e2e gates run in CI or Colima only.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `brokerd/tests/uds_ipc.rs` does not construct `BrokerRequest::SubmitPlanNode` via IPC (only direct function calls) | Session-Scoping | If it does, removing the `session_id` field breaks that test — must update |
| A2 | `WorkerClaim` serde with `#[serde(tag = "kind")]` will reject unknown `kind` values (exhaustive) | ReportClaims | If serde allows unknown tags, need explicit validation in the broker handler |
| A3 | The `caprun` e2e test only needs `CARGO_BIN_EXE_caprun-worker` (already used in test harness) | Validation Architecture | No additional test infrastructure needed |

**Verify A1 before implementing:**
```bash
grep -n "SubmitPlanNode" /Users/benlamm/Workspace/AgentOS/crates/brokerd/tests/uds_ipc.rs
```

**Verify A2:** Serde's internally-tagged enum (`#[serde(tag)]`) returns a deserialization error for unknown tag values by default when `deny_unknown_fields` is not needed for this behavior — the enum itself being exhaustive is sufficient.

---

## Open Questions (RESOLVED)

1. **Should `ReportRead` be removed from proto.rs in Phase 5?**
   - What we know: Phase 5 replaces `ReportRead { bytes_read }` with `ReportClaims`; `ReportRead` is currently wired in `main.rs::handle_worker_connection` (the code being deleted)
   - What's unclear: whether any test still sends `ReportRead` via IPC
   - Recommendation: Keep `ReportRead` in the enum but have the new unified dispatch return an error for it ("use ReportClaims"), OR remove it if no live test uses it. Check `uds_ipc.rs`.
   - **RESOLVED: kept as deprecated — Plan 02 Task 2 keeps the `ReportRead` variant and has the unified dispatch return an Error for it (removal is cosmetic, not a requirement; avoids multi-plan file churn).**

2. **Should `plan_node_evaluated` event remain for the Allowed path in Phase 5?**
   - What we know: Phase 5 only proves the blocked path (hostile input always blocks); the Allowed path is tested in Phase 7 (file.create)
   - What's unclear: whether appending `sink_allowed` or `plan_node_evaluated` for the Allowed path is needed for Phase 5 tests
   - Recommendation: For Phase 5, only the blocked path matters for ACC-02. Keep `plan_node_evaluated` as the event type for Allowed (or just don't append on Allowed path in Phase 5). The causal chain for Allowed is Phase 7's concern.
   - **RESOLVED: kept; block path uses the new fail-closed `sink_blocked` append — Plan 02 Task 2. Allowed-path causal chain is Phase 7's concern (SC7).**

3. **Where does `approval.rs::build_confirmation_prompt` get called on the live path?**
   - What we know: It exists and works; the §9 test calls it. In Phase 5 the CLI exits immediately on block, so no UX confirmation is needed.
   - What's unclear: ACC-02 says "block durable before CLI returns" but doesn't require a human approval prompt in Phase 5. The confirmation UX is v2.
   - Recommendation: Phase 5 does NOT need to wire `build_confirmation_prompt` into the live dispatch. The `sink_blocked` event IS the block record. Human confirmation UX is explicitly deferred.
   - **RESOLVED: not wired in Phase 5; the `sink_blocked` event is the block record. Human confirmation UX deferred to v2.**

---

## Sources

### Primary (HIGH confidence)

All findings are derived from direct file reads in this session. No training data was relied upon for codebase-specific claims.

- `cli/caprun/src/main.rs` (lines 1-340) — dual dispatch reality; RequestFd/ReportRead wired; SubmitPlanNode stub at line 278
- `crates/brokerd/src/server.rs` (lines 1-241) — SubmitPlanNode wired; RequestFd/ReportRead stubbed at line 182; best-effort audit append at lines 219-223
- `crates/brokerd/src/proto.rs` (lines 1-47) — current BrokerRequest/BrokerResponse variants; SubmitPlanNode carries session_id
- `crates/brokerd/src/quarantine.rs` (lines 1-331) — mint_from_read implementation; extract_email_claims; anti-stapling invariant comments
- `crates/executor/src/lib.rs` (lines 1-79) — submit_plan_node pure function; anti-stapling comment
- `crates/executor/src/value_store.rs` (lines 1-107) — ValueStore; mint; resolve; no session key
- `crates/brokerd/tests/s9_acceptance.rs` — §9 test structure; what must keep passing
- `planning-docs/DESIGN-taint-model.md` — I0/I1/I2 invariants; genuine-taint requirement
- `planning-docs/DESIGN-plan-executor.md` — ValueRecord/ValueId model; executor decision logic; sink sensitivity map
- `.planning/REQUIREMENTS.md` — ASM-01..04, HARD-03, ACC-02 exact wording
- `.planning/STATE.md` — locked decisions; blocked-path audit primitive in Phase 5
- `.planning/phases/04-value-injection-security-demo-v0-done/04-05-SUMMARY.md` — Phase 4 completion state; 51 tests passing
- `cli/caprun/src/worker.rs` (lines 1-121) — current worker protocol; what must change

### Secondary (MEDIUM confidence)

- `planning-docs/PLAN.md` — §9 acceptance test spec; architectural lock; v0 build order
- `.planning/phases/04-value-injection-security-demo-v0-done/04-PATTERNS.md` — Phase 4 implementation patterns

---

## Metadata

**Confidence breakdown:**
- ASM-01 dual dispatch analysis: HIGH — traced exact file:line locations of both dispatch loops
- ASM-02 stub location: HIGH — confirmed at main.rs:278 with exact string
- ASM-03 protocol changes: HIGH — no ReportClaims type exists anywhere in codebase (confirmed by grep)
- ASM-04 mint_from_read wiring: HIGH — function exists and is proven; wiring path is straightforward
- HARD-03 session scoping: HIGH — confirmed via ValueStore inspection (no session key; shared Arc)
- ACC-02 durable event: HIGH — confirmed best-effort swallow at server.rs:219-223
- Test architecture: HIGH — existing test patterns are clear and established

**Research date:** 2026-06-30
**Valid until:** 2026-07-30 (stable codebase; no fast-moving external dependencies)
