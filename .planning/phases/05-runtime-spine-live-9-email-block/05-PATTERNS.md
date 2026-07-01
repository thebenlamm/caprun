# Phase 5: Runtime Spine & Live §9 Email Block — Pattern Map

**Mapped:** 2026-06-30
**Files analyzed:** 4 modified files
**Analogs found:** 4 / 4

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/brokerd/src/proto.rs` | protocol/model | request-response | `crates/brokerd/src/proto.rs` (self — additive) | exact |
| `crates/brokerd/src/server.rs` | service/middleware | request-response + event-driven | `cli/caprun/src/main.rs::handle_worker_connection` (lines 154-284) | exact — this is the code being absorbed |
| `cli/caprun/src/main.rs` | orchestrator/controller | request-response | `cli/caprun/src/main.rs` (self — loop deletion) | exact |
| `cli/caprun/src/worker.rs` | worker/client | request-response | `cli/caprun/src/worker.rs` (self — protocol upgrade) | exact |

---

## Pattern Assignments

### `crates/brokerd/src/proto.rs` (protocol model, additive + one field removal)

**Analog:** `crates/brokerd/src/proto.rs` (existing file — additive changes)

**Current enum pattern** (lines 1-47 — copy this serde style):
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerRequest {
    CreateSession { intent_id: uuid::Uuid },
    RequestFd { path: String },
    ReportRead { bytes_read: u64 },
    SubmitPlanNode {
        session_id: uuid::Uuid,   // REMOVE this field — HARD-03 violation
        plan_node: runtime_core::PlanNode,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerResponse {
    SessionCreated { session_id: uuid::Uuid },
    FdGranted,
    Ack,
    Error { message: String },
    PlanNodeDecision { decision: runtime_core::ExecutorDecision },
}
```

**New types to add — WorkerClaim enum** (use internally-tagged serde so unknown `kind` values fail to deserialize, closing fail-fast on unknown variants):
```rust
/// Typed, lossy claim from a confined worker. Raw source bytes never appear in
/// the IPC message — only the extracted typed value crosses the boundary.
/// Unknown variants fail closed: serde returns a deserialize error (exhaustive enum).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim {
    EmailAddress(String),
    // RelativePath(String),  // Phase 7
}
```

**New BrokerRequest variant:**
```rust
/// Worker extracted typed claims from a file read. Raw bytes NOT included.
/// Broker mints a ValueRecord per claim via mint_from_read and returns handles.
ReportClaims { claims: Vec<WorkerClaim> },
```

**New BrokerResponse variant:**
```rust
/// Acknowledgement for ReportClaims: opaque ValueId handles per minted claim,
/// in the same order as the claims in the ReportClaims message.
ClaimsReceived { value_ids: Vec<runtime_core::plan_node::ValueId> },
```

**Field removal from SubmitPlanNode:**
```rust
// BEFORE (HARD-03 violation — broker must never trust request-supplied session_id):
SubmitPlanNode { session_id: uuid::Uuid, plan_node: runtime_core::PlanNode }

// AFTER (broker uses connection-established session_id from handle_connection param):
SubmitPlanNode { plan_node: runtime_core::PlanNode }
```

---

### `crates/brokerd/src/server.rs` (IPC server, per-connection stateful dispatch)

**Analog:** `cli/caprun/src/main.rs` lines 154-284 (`handle_worker_connection`) — this function moves verbatim into brokerd::server with additions.

**Current `run_broker_server` signature** (lines 71-75 — replace):
```rust
// CURRENT (HARD-03: shared ValueStore across connections):
pub async fn run_broker_server(
    session_id: &str,
    conn: Arc<Mutex<rusqlite::Connection>>,
    value_store: Arc<Mutex<ValueStore>>,   // REMOVE — creates cross-session handle resolution
) -> anyhow::Result<()>
```

**New `run_broker_server` signature:**
```rust
pub async fn run_broker_server(
    session_id: &str,               // socket name suffix (string form)
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id_uuid: Uuid,          // broker-authoritative session identity
    initial_last_event_id: Uuid,    // from session_created event minted by main.rs
    initial_last_event_hash: String,
) -> anyhow::Result<()>
// ValueStore is no longer a parameter — created fresh per-connection inside handle_connection
```

**Accept loop pattern** (lines 80-91 — keep structure, update call):
```rust
loop {
    let (stream, _addr) = listener.accept().await?;
    let conn_clone = conn.clone();
    // Pass connection state by value — each connection owns its own chain state
    tokio::spawn(async move {
        if let Err(e) = handle_connection(
            stream, conn_clone, session_id_uuid,
            initial_last_event_id, initial_last_event_hash.clone()
        ).await {
            eprintln!("[brokerd] connection error: {e}");
        }
    });
}
```

**`handle_connection` pattern** (merge of server.rs lines 97-136 + main.rs lines 154-284):
```rust
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,           // connection-established; never from IPC message
    mut last_event_id: Uuid,    // mutable chain state threaded across all messages
    mut last_event_hash: String,
) -> anyhow::Result<()> {
    // Per-connection ValueStore — scoped to this session ONLY (HARD-03 fix)
    let mut value_store = ValueStore::default();

    loop {
        // Framing: 4-byte LE length prefix (from server.rs lines 103-131)
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        if msg_len > MAX_MSG_SIZE {
            send_response(&mut stream, &BrokerResponse::Error {
                message: "message too large".into()
            }).await?;
            break;
        }
        let mut body = vec![0u8; msg_len];
        stream.read_exact(&mut body).await?;
        let request = match serde_json::from_slice::<BrokerRequest>(&body) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[brokerd] deserialize error: {e}");
                send_response(&mut stream, &BrokerResponse::Error {
                    message: "invalid request".into()
                }).await?;
                break;
            }
        };
        // Dispatch to per-request handler (see each arm below)
        dispatch_request(request, &mut stream, &conn, session_id,
                         &mut last_event_id, &mut last_event_hash,
                         &mut value_store).await?;
    }
    Ok(())
}
```

**Recommended: extract `dispatch_request` for testability** (per RESEARCH.md Pitfall 6):
```rust
// Signature that can be constructed in tests without a real socket:
async fn dispatch_request(
    request: BrokerRequest,
    stream: &mut tokio::net::UnixStream,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
    value_store: &mut ValueStore,
) -> anyhow::Result<()>
```

**RequestFd arm** — move verbatim from `main.rs` lines 197-239 into dispatch_request:
```rust
BrokerRequest::RequestFd { path } => {
    let file = std::fs::File::open(&path)
        .with_context(|| format!("broker: open {path}"))?;
    let file_fd = file.as_raw_fd();

    let fd_event_id = Uuid::new_v4();
    let fd_event = Event {
        id: fd_event_id,
        parent_id: Some(*last_event_id),   // causal parent — not None
        session_id,
        actor: "broker".into(),
        event_type: "fd_granted".into(),
        timestamp: Utc::now(),
        taint: vec![],
    };
    let fd_hash = {
        let locked = conn.lock().unwrap();
        append_event(&locked, &fd_event, Some(last_event_hash))
            .context("append fd_granted")?
    };

    // pass_fd is blocking (sendmsg) — MUST use spawn_blocking (RESEARCH.md Pitfall 2)
    let sock_fd = stream.as_raw_fd();
    tokio::task::spawn_blocking(move || {
        let result = pass_fd(sock_fd, file_fd)
            .map_err(|e| anyhow::anyhow!("pass_fd: {e}"));
        drop(file);  // close broker's copy after sendmsg
        result
    })
    .await
    .context("spawn_blocking pass_fd")??;

    send_response(stream, &BrokerResponse::FdGranted).await?;
    *last_event_id = fd_event_id;
    *last_event_hash = fd_hash;
}
```

**ReportClaims arm** — new; calls `mint_from_read` (the sole taint-mint site):
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
                // mint_from_read: appends file_read Event AND mints ValueRecord atomically.
                // NEVER call ValueStore::mint directly with a non-empty taint vector —
                // that is taint stapling and breaks the provenance_chain invariant (T-04-03).
                let (read_event_id, read_hash, value_id) = {
                    let locked = conn.lock()
                        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                    mint_from_read(&locked, value_store, session_id,
                                   &quarantine_claim, Some(last_event_hash))?
                };
                *last_event_id = read_event_id;
                *last_event_hash = read_hash;
                value_ids.push(value_id);
            }
            // Any future/unknown variant that somehow reaches here must fail closed.
            // With an exhaustive Rust enum this is a compile-time guarantee;
            // add a wildcard arm returning an error only if the enum gains a non-exhaustive attr.
        }
    }
    send_response(stream, &BrokerResponse::ClaimsReceived { value_ids }).await?;
}
```

**SubmitPlanNode arm — durable fail-closed audit** (replaces server.rs lines 189-226):
```rust
BrokerRequest::SubmitPlanNode { plan_node } => {
    // session_id comes from handle_connection parameter — NEVER from the IPC message (HARD-03)
    let decision = executor::submit_plan_node(session_id, &plan_node, value_store);

    // Durably record block/allow BEFORE returning any response (ACC-02).
    // Fail closed: append error → error response to worker → worker exits non-success.
    let event_type = match &decision {
        runtime_core::ExecutorDecision::BlockedPendingConfirmation { .. } => "sink_blocked",
        _ => "plan_node_evaluated",
    };
    let audit_event = Event {
        id: Uuid::new_v4(),
        parent_id: Some(*last_event_id),   // causal parent preserved — not None
        session_id,
        actor: "executor".into(),
        event_type: event_type.into(),
        timestamp: Utc::now(),
        taint: vec![],
    };
    let new_hash = {
        let locked = conn.lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        append_event(&locked, &audit_event, Some(last_event_hash))
            .map_err(|e| {
                eprintln!("[brokerd] {event_type} audit append FAILED (fail-closed): {e}");
                anyhow::anyhow!("audit append failed: {e}")
            })?   // ? propagates — error response returned; block NOT reported until durable
    };
    *last_event_id = audit_event.id;
    *last_event_hash = new_hash;

    // Only send decision AFTER successful durable append
    send_response(stream, &BrokerResponse::PlanNodeDecision { decision }).await?;
}
```

**`send_response` helper** (lines 230-240 — unchanged, already correct):
```rust
async fn send_response(
    stream: &mut tokio::net::UnixStream,
    response: &BrokerResponse,
) -> anyhow::Result<()> {
    let body = serde_json::to_vec(response)?;
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    Ok(())
}
```

---

### `cli/caprun/src/main.rs` (orchestrator — delete `handle_worker_connection`, call brokerd::server)

**Analog:** `cli/caprun/src/main.rs` (self — steps 1-4 and 6-8 unchanged; step 5 is the only change)

**Session + event setup pattern** (lines 53-78 — unchanged, keep verbatim):
```rust
// Steps 1-2: open DB, create session, persist, append session_created event
let conn = Arc::new(Mutex::new(open_audit_db(&audit_path).context("open_audit_db")?));
let session_created_id = Uuid::new_v4();
let e_session = Event { id: session_created_id, parent_id: None, session_id, ... };
let session_created_hash = {
    let locked = conn.lock().unwrap();
    persist_session(&locked, &session).context("persist_session")?;
    append_event(&locked, &e_session, None).context("append session_created")?
};
// Steps 3-4: bind abstract socket, spawn worker (unchanged)
```

**Step 5 — broker task: replace `handle_worker_connection` with `brokerd::server::run_broker_server`**:
```rust
// BEFORE (deleted):
let broker_task = tokio::spawn(async move {
    let (stream, _) = listener.accept().await.context("accept")?;
    handle_worker_connection(stream, conn_clone, session_id,
                             session_created_id, session_created_hash).await
});

// AFTER: use brokerd::server — unified single dispatch path
// run_broker_server takes the listener's socket path; it owns the accept loop.
// Pass the session chain state so the first accepted connection threads correctly.
let broker_task = tokio::spawn(async move {
    brokerd::server::run_broker_server(
        &session_id.to_string(),
        conn_clone,
        session_id,               // Uuid form
        session_created_id,
        session_created_hash,
    ).await
});
```

**Child exit propagation** (lines 137-139 — unchanged, already correct for block detection):
```rust
if !child_status.success() {
    anyhow::bail!("caprun-worker exited with status: {child_status}");
}
```

**Delete entirely:** `handle_worker_connection` function (lines 154-284) and local `send_response` (lines 287-297). Both move into `brokerd::server`.

---

### `cli/caprun/src/worker.rs` (worker client — protocol upgrade to ReportClaims)

**Analog:** `cli/caprun/src/worker.rs` (self — steps 1-6 unchanged, steps 7-13 replace step 7-9)

**Framing helpers** (lines 101-120 — unchanged, keep verbatim):
```rust
fn send_framed(stream: &std::os::unix::net::UnixStream, msg: &impl serde::Serialize) -> anyhow::Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    (&*stream).write_all(&len)?;
    (&*stream).write_all(&body)?;
    Ok(())
}

fn recv_framed<T: serde::de::DeserializeOwned>(stream: &std::os::unix::net::UnixStream) -> anyhow::Result<T> {
    let mut len_buf = [0u8; 4];
    (&*stream).read_exact(&mut len_buf)?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    (&*stream).read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
```

**Steps 1-6: connect, into_std, apply_confinement, RequestFd, recv_fd, FdGranted** (lines 34-75 — unchanged):
```rust
// Step 3: apply_confinement AFTER connecting (self-confinement model; socket fd survives)
sandbox::apply_confinement().map_err(|e| anyhow::anyhow!("apply_confinement: {e}"))?;
// Steps 4-6: RequestFd → recv_fd → consume FdGranted (unchanged protocol)
send_framed(&std_stream, &BrokerRequest::RequestFd { path: workspace_file })?;
let file_fd = adapter_fs::recv_fd(sock_fd).map_err(|e| anyhow::anyhow!("recv_fd: {e}"))?;
let _granted: BrokerResponse = recv_framed(&std_stream)?;
```

**Steps 7-13 replace steps 7-9** (after reading raw bytes via fd):
```rust
// Step 6: read file bytes via passed fd (unchanged — Landlock blocks open())
let raw_bytes = {
    let mut file = unsafe { std::fs::File::from_raw_fd(file_fd) };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).context("read via passed fd")?;
    buf
};
let raw_str = String::from_utf8_lossy(&raw_bytes);

// Step 7 (NEW): Extract typed claims inside the confined worker.
// Raw sentence is discarded here; only the address crosses the IPC boundary.
// Reuses the proven extract_email_claims function from brokerd::quarantine.
use brokerd::quarantine::extract_email_claims;
let claims: Vec<brokerd::proto::WorkerClaim> = extract_email_claims(&raw_str)
    .into_iter()
    .map(|c| brokerd::proto::WorkerClaim::EmailAddress(c.value))
    .collect();

// Step 8 (NEW): Send ReportClaims (typed; no raw bytes cross the boundary)
send_framed(&std_stream, &BrokerRequest::ReportClaims { claims })?;

// Step 9 (NEW): Receive opaque ValueId handles
let BrokerResponse::ClaimsReceived { value_ids } = recv_framed(&std_stream)? else {
    anyhow::bail!("unexpected response to ReportClaims");
};

// Step 10 (NEW): Scripted planner builds PlanNode from opaque handles only
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId};
let plan_node = PlanNode {
    sink: SinkId("email.send".into()),
    args: vec![PlanArg { name: "to".into(), value_id: value_ids[0].clone() }],
};

// Step 11 (NEW): Submit for I2 evaluation (no session_id field — HARD-03 compliant)
send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;

// Step 12 (NEW): Receive block decision
let BrokerResponse::PlanNodeDecision { decision } = recv_framed(&std_stream)? else {
    anyhow::bail!("unexpected response to SubmitPlanNode");
};

// Step 13 (NEW): Exit non-success if blocked (durable audit event was recorded by broker)
if matches!(decision, runtime_core::ExecutorDecision::BlockedPendingConfirmation { .. }) {
    eprintln!("[worker] BLOCKED: value-injection defense triggered — exiting 1");
    std::process::exit(1);
}
```

---

## Shared Patterns

### IPC Framing (4-byte LE length prefix + JSON)
**Source:** `crates/brokerd/src/server.rs` lines 103-131 (broker recv) + `cli/caprun/src/worker.rs` lines 101-120 (worker send/recv)
**Apply to:** All new IPC dispatch arms and the new test file `crates/brokerd/tests/phase5_dispatch.rs`

The framing is symmetric: same length-prefix format on both sides. The worker uses synchronous `std::os::unix::net::UnixStream`; the broker uses `tokio::net::UnixStream`. The `round_trip` helper in `crates/brokerd/tests/uds_ipc.rs` lines 29-44 is the test-side framing pattern to copy for `phase5_dispatch.rs`.

### Causal Chain Threading
**Source:** `cli/caprun/src/main.rs` lines 154-239 (`handle_worker_connection`)
**Apply to:** Every new dispatch arm in `server.rs`

Pattern: `mut last_event_id: Uuid` + `mut last_event_hash: String` passed by mutable reference through each arm. Every arm that appends an event advances both variables AFTER the append succeeds, BEFORE sending the response.

```rust
// canonical advance pattern (from main.rs lines 238-239):
last_event_id = fd_event_id;
last_event_hash = fd_hash;
```

### Durable Fail-Closed Audit Append
**Source:** `cli/caprun/src/main.rs` lines 214-218 (fd_granted append — the correct model); contrast with `crates/brokerd/src/server.rs` lines 219-223 (plan_node_evaluated — the broken model)
**Apply to:** `sink_blocked` event append in SubmitPlanNode arm

```rust
// CORRECT (fd_granted in main.rs — fail-closed, fail propagates):
let fd_hash = {
    let locked = conn.lock().unwrap();
    append_event(&locked, &fd_event, Some(&last_event_hash)).context("append fd_granted")?
};

// WRONG (plan_node_evaluated in server.rs — best-effort swallow — DO NOT COPY):
if let Ok(locked) = conn.lock() {
    if let Err(e) = append_event(&locked, &eval_event, None) {
        eprintln!("...");  // swallowed — block never confirmed durable
    }
}
```

### SCM_RIGHTS fd-pass (spawn_blocking)
**Source:** `cli/caprun/src/main.rs` lines 224-232
**Apply to:** `RequestFd` arm in `server.rs::dispatch_request`

```rust
// pass_fd is blocking (sendmsg syscall) — must use spawn_blocking or tokio reactor stalls
let sock_fd = stream.as_raw_fd();
tokio::task::spawn_blocking(move || {
    let result = pass_fd(sock_fd, file_fd).map_err(|e| anyhow::anyhow!("pass_fd: {e}"));
    drop(file);
    result
}).await.context("spawn_blocking pass_fd")??;
```

### Genuine-Taint Mint (mint_from_read)
**Source:** `crates/brokerd/src/quarantine.rs` lines 124-159
**Apply to:** `ReportClaims` arm in `server.rs::dispatch_request`

```rust
// mint_from_read is the ONLY broker call site for minting tainted ValueRecords.
// It atomically: (1) appends file_read Event, (2) mints ValueRecord with provenance_chain[0] == event_id.
// Never call ValueStore::mint directly with a non-empty taint vector (taint stapling = §9 failure).
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_hash: Option<&str>,  // Some(&last_event_hash) to thread causal chain
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId)>
```

---

## Assumptions Requiring Verification Before Implementation

**A1 — `uds_ipc.rs` uses `SubmitPlanNode` via IPC:**
```bash
grep -n "SubmitPlanNode" /Users/benlamm/Workspace/AgentOS/crates/brokerd/tests/uds_ipc.rs
```
If it constructs `SubmitPlanNode { session_id, plan_node }` directly, update that test to remove `session_id` when implementing the proto.rs field removal.

**A2 — `run_broker_server` signature change breaks `uds_ipc.rs`:**
The test at `uds_ipc.rs` line 62 calls `run_broker_server(&session_id_clone, conn_clone, value_store)` — this call site must be updated to the new 5-parameter signature.

---

## No Analog Found

None. All four files have exact analogs in the existing codebase.

---

## Metadata

**Analog search scope:** `crates/brokerd/src/`, `cli/caprun/src/`, `crates/brokerd/tests/`, `crates/executor/src/`
**Files read:** `proto.rs`, `server.rs`, `main.rs`, `worker.rs`, `quarantine.rs` (lines 1-159), `uds_ipc.rs` (lines 1-100), `s9_acceptance.rs` (lines 1-80)
**Pattern extraction date:** 2026-06-30
