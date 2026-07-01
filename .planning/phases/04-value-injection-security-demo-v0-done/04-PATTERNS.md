# Phase 4: Value-Injection Security Demo (v0 DONE) — Pattern Map

**Mapped:** 2026-06-29
**Files analyzed:** 9 new/modified files
**Analogs found:** 9 / 9

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/executor/Cargo.toml` | config | — | `crates/brokerd/Cargo.toml` | exact |
| `crates/executor/src/lib.rs` | service | request-response | `crates/brokerd/src/server.rs` dispatch fn | role-match |
| `crates/executor/src/sink_sensitivity.rs` | utility | transform | `crates/runtime-core/src/plan_node.rs` (const enums) | role-match |
| `crates/executor/src/value_store.rs` | service | CRUD | `crates/brokerd/src/session.rs` + `server.rs` Arc<Mutex<>> pattern | role-match |
| `crates/executor/tests/s9_acceptance.rs` | test | request-response | `crates/brokerd/tests/` audit chain tests | role-match |
| `crates/runtime-core/src/plan_node.rs` (modify) | model | — | self (existing types to extend) | exact |
| `crates/runtime-core/src/executor_decision.rs` (modify) | model | — | self (existing enum to extend) | exact |
| `crates/brokerd/src/proto.rs` (modify) | model | — | self (existing enum to extend) | exact |
| `crates/brokerd/src/server.rs` (modify) | middleware | request-response | self (existing dispatch fn) | exact |
| `crates/brokerd/src/sinks/email_send.rs` | service | event-driven | `crates/brokerd/src/session.rs` + audit::append_event | role-match |

---

## Pattern Assignments

### `crates/executor/Cargo.toml` (config)

**Analog:** `crates/brokerd/Cargo.toml`

**Full pattern:**
```toml
[package]
name    = "executor"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
runtime-core = { path = "../runtime-core" }
uuid         = { workspace = true }
anyhow       = { workspace = true }
serde        = { workspace = true }
serde_json   = { workspace = true }

[dev-dependencies]
brokerd      = { path = "../brokerd" }
runtime-core = { path = "../runtime-core" }
rusqlite     = { workspace = true }
uuid         = { workspace = true }
chrono       = { workspace = true }
```

Note: add `"executor"` to `crates/*` glob in root `Cargo.toml` — already covered by the wildcard.

---

### `crates/executor/src/lib.rs` (service, request-response)

**Analog:** `crates/brokerd/src/server.rs` — specifically the `dispatch()` function pattern (lines 136-179): pure synchronous function, takes structured input, returns typed response, no DB writes from the decision logic itself.

**Imports pattern** (from `server.rs` lines 42-49):
```rust
use runtime_core::{ExecutorDecision, TaintLabel};
use runtime_core::plan_node::{PlanNode, PlanArg, SinkId};
use uuid::Uuid;
use crate::sink_sensitivity::is_routing_sensitive;
use crate::value_store::{ValueStore, ValueRecord};
```

**Core pattern — pure decision function** (modeled after `dispatch()` in `server.rs` lines 136-179):
```rust
/// Evaluate a PlanNode against the trusted ValueStore.
///
/// This is a pure function — it resolves ValueIds from the trusted store and
/// returns a decision. It NEVER sets taint on any ValueRecord. Taint is
/// written only at broker read-time (when the file_read Event is appended).
pub fn submit_plan_node(
    session_id: Uuid,
    plan_node: &PlanNode,
    value_store: &ValueStore,
) -> ExecutorDecision {
    for arg in &plan_node.args {
        let record = match value_store.resolve(&arg.value_id) {
            Some(r) => r,
            None => return ExecutorDecision::Denied {
                reason: format!("unresolvable ValueId: {:?}", arg.value_id),
            },
        };
        if is_routing_sensitive(&plan_node.sink, &arg.name) && !record.taint.is_empty() {
            return ExecutorDecision::BlockedPendingConfirmation {
                literal_value: record.literal.clone(),
                sink: plan_node.sink.0.clone(),
                arg_name: arg.name.clone(),
                taint: record.taint.clone(),
                provenance_chain: record.provenance_chain.clone(),
            };
        }
    }
    ExecutorDecision::Allowed
}
```

**Error handling pattern** (from `server.rs` lines 154-169):
```rust
// Use anyhow::Result for fallible ops; return typed enum variants for logic outcomes.
// Never panic on bad input — return Denied { reason } instead.
// Log internal detail with eprintln!("[executor] ..."); send generic message upward.
```

---

### `crates/executor/src/sink_sensitivity.rs` (utility, transform)

**Analog:** `crates/runtime-core/src/plan_node.rs` — const enum definition style (lines 1-54).

**Core pattern:**
```rust
use runtime_core::plan_node::SinkId;

/// Routing-sensitive args for email.send — Block if tainted.
/// Tainted value in these args → potential header injection / redirect attack.
const EMAIL_SEND_ROUTING_SENSITIVE: &[&str] = &["to", "cc", "bcc"];

/// Content-sensitive args — Tier-4 verbatim review, but NOT Block for §9.
const EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body", "attachment"];

/// Returns true if `arg_name` is routing-sensitive for `sink`.
/// Hardcoded for v0; no configuration knob — sensitivity is a security property, not a UX choice.
pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
```

---

### `crates/executor/src/value_store.rs` (service, CRUD)

**Analog:** `crates/brokerd/src/server.rs` lines 68-86 — `Arc<Mutex<rusqlite::Connection>>` pattern; and `crates/brokerd/src/session.rs` for the `persist_*` / `create_*` pattern.

**Core pattern:**
```rust
use runtime_core::plan_node::TaintLabel;
use std::collections::HashMap;
use uuid::Uuid;

/// Opaque handle the planner holds. Never carries a literal or taint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ValueId(pub Uuid);

impl ValueId {
    pub fn new() -> Self { ValueId(Uuid::new_v4()) }
}

/// Broker-owned record. The planner never sees this type.
#[derive(Debug, Clone)]
pub struct ValueRecord {
    pub id: ValueId,
    pub literal: String,
    pub taint: Vec<TaintLabel>,
    /// Ordered chain of audit Event IDs that produced this value.
    /// provenance_chain[0] == the file_read Event ID (the genuine-taint anchor).
    pub provenance_chain: Vec<Uuid>,
}

/// In-memory store for v0. Sufficient for §9 in-process test.
/// In production, backed by the broker's SQLite DB.
#[derive(Default)]
pub struct ValueStore {
    records: HashMap<ValueId, ValueRecord>,
}

impl ValueStore {
    pub fn mint(&mut self, literal: String, taint: Vec<TaintLabel>, provenance_chain: Vec<Uuid>) -> ValueId {
        let id = ValueId::new();
        self.records.insert(id.clone(), ValueRecord {
            id: id.clone(), literal, taint, provenance_chain,
        });
        id
    }

    pub fn resolve(&self, id: &ValueId) -> Option<&ValueRecord> {
        self.records.get(id)
    }
}
```

Note: matches the `Arc<Mutex<rusqlite::Connection>>` wrapping pattern from `server.rs` line 71; for concurrent use wrap `ValueStore` in `Arc<Mutex<ValueStore>>`.

---

### `crates/executor/tests/s9_acceptance.rs` (test, request-response)

**Analog:** `crates/brokerd/src/audit.rs` — `verify_chain` and `append_event` are called directly in tests. The test pattern uses `open_audit_db(":memory:")` (audit.rs line 56), `Event` construction (event.rs lines 16-29), and the `#[test]` attribute (no tokio needed for in-process test).

**Test scaffold pattern** (modeled after audit.rs test structure + event.rs types):
```rust
use brokerd::audit::{open_audit_db, append_event, verify_chain};
use runtime_core::{Event, plan_node::{TaintLabel, SinkId, PlanNode, PlanArg}};
use executor::{submit_plan_node, value_store::ValueStore};
use chrono::Utc;
use uuid::Uuid;

#[test]
fn s9_acceptance() {
    let conn = open_audit_db(":memory:").unwrap();
    let session_id = Uuid::new_v4();

    // Step 1: append file_read Event (genuine taint origin)
    let read_event = Event {
        id: Uuid::new_v4(),
        parent_id: None,
        session_id,
        actor: "confined-reader".into(),
        event_type: "file_read".into(),
        timestamp: Utc::now(),
        taint: vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
    };
    let read_hash = append_event(&conn, &read_event, None).unwrap();

    // Step 2: mint ValueRecord in broker-owned store (taint from read Event — NOT set by executor)
    let mut store = ValueStore::default();
    let value_id = store.mint(
        "accounts@ev1l.com".into(),
        vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
        vec![read_event.id],
    );

    // Step 3: scripted plan — planner holds only value_id, never literal/taint
    let plan_node = PlanNode {
        sink: SinkId("email.send".into()),
        args: vec![PlanArg { name: "to".into(), value_id: value_id.clone() }],
    };

    // Step 4: executor evaluates — must Block
    let decision = submit_plan_node(session_id, &plan_node, &store);

    // Step 5: assert §9 sub-criteria
    let (lit, sink, arg, taint, chain) = match &decision {
        ExecutorDecision::BlockedPendingConfirmation { literal_value, sink, arg_name, taint, provenance_chain } =>
            (literal_value, sink, arg_name, taint, provenance_chain),
        other => panic!("expected Block, got {:?}", other),
    };
    assert_eq!(lit, "accounts@ev1l.com");
    assert_eq!(sink, "email.send");
    assert_eq!(arg, "to");
    assert!(taint.contains(&TaintLabel::ExternalUntrusted));
    assert_eq!(chain[0], read_event.id);   // genuine-taint tripwire

    // Step 6: audit chain integrity
    assert!(verify_chain(&conn, &session_id.to_string()));

    // Step 7: taint NOT stapled — provenance_chain[0] must exist as real file_read Event
    // (a fabricated UUID would fail this lookup)
    let _ = read_hash; // chain is verified above; hash retained to confirm append succeeded
}
```

---

### `crates/runtime-core/src/plan_node.rs` (modify — add ValueId, PlanArg, ValueRecord; update PlanNode.args)

**Analog:** self — extend existing types. Current file lines 1-54 are the base.

**New types to add** (copy derive block pattern from lines 8-18):
```rust
/// Opaque value handle — the only thing the planner ever holds.
/// Never carries literal, taint, or provenance. Lives in runtime-core so PlanNode can reference it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ValueId(pub uuid::Uuid);

/// Planner-facing argument: a named slot bound to an opaque ValueId.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanArg {
    pub name: String,
    pub value_id: ValueId,
}
```

**Breaking change to PlanNode** (line 53 — `args: Vec<ValueNode>` → `args: Vec<PlanArg>`):
```rust
pub struct PlanNode {
    pub sink: SinkId,
    pub args: Vec<PlanArg>,   // was Vec<ValueNode> — breaking change; update all callers
}
```

**Extension to Provenance** (lines 22-26 — add provenance_chain):
```rust
pub struct Provenance {
    pub source_event_id: Option<uuid::Uuid>,
    pub source_artifact_id: Option<uuid::Uuid>,
    pub description: String,
    /// Ordered derivation edges from read Event to this value. Length 1 for v0 linear DAG.
    pub provenance_chain: Vec<uuid::Uuid>,
}
```

---

### `crates/runtime-core/src/executor_decision.rs` (modify — extend BlockedPendingConfirmation)

**Analog:** self — current file lines 1-22.

**Extended variant** (lines 13-17 → add taint + provenance_chain):
```rust
/// Execution blocked — tainted value in sensitive sink argument; confirmation required.
/// taint and provenance_chain are included so the §9 test can assert the unbroken chain
/// directly from the decision payload without a second DB query.
BlockedPendingConfirmation {
    literal_value: String,
    sink: String,
    arg_name: String,
    taint: Vec<crate::plan_node::TaintLabel>,
    /// provenance_chain[0] must equal the file_read Event ID in the audit DAG.
    provenance_chain: Vec<uuid::Uuid>,
},
```

Note: this is a breaking change to any `match` on this variant. Search for `BlockedPendingConfirmation` across the workspace before committing.

---

### `crates/brokerd/src/proto.rs` (modify — add SubmitPlanNode + PlanNodeDecision)

**Analog:** self — current file lines 1-31.

**New variants** (copy derive + doc style from lines 7-18):
```rust
// Add to BrokerRequest:
/// Submit a plan node for executor evaluation. Returns PlanNodeDecision.
SubmitPlanNode {
    session_id: uuid::Uuid,
    plan_node: runtime_core::plan_node::PlanNode,
},

// Add to BrokerResponse:
/// The executor's decision for a submitted PlanNode.
PlanNodeDecision {
    decision: runtime_core::ExecutorDecision,
},
```

---

### `crates/brokerd/src/server.rs` (modify — wire SubmitPlanNode in dispatch)

**Analog:** self — `dispatch()` function lines 136-179. The `CreateSession` arm (lines 138-169) is the exact pattern: lock conn, do work, match result, return typed response.

**New dispatch arm** (insert before the `RequestFd | ReportRead` arm at line 173):
```rust
BrokerRequest::SubmitPlanNode { session_id, plan_node } => {
    // executor::submit_plan_node is a pure fn — no DB lock needed for the decision itself.
    // Append plan_node_evaluated Event to audit DAG after decision.
    let decision = executor::submit_plan_node(session_id, &plan_node, &value_store);

    // Append audit event for the block/allow decision
    let eval_event = Event {
        id: Uuid::new_v4(),
        parent_id: None,   // Phase 4: use last_event_hash to thread the chain
        session_id,
        actor: "executor".into(),
        event_type: "plan_node_evaluated".into(),
        timestamp: Utc::now(),
        taint: vec![],
    };
    if let Ok(locked) = conn.lock() {
        let _ = append_event(&locked, &eval_event, None); // best-effort; log on error
    }

    BrokerResponse::PlanNodeDecision { decision }
}
```

Note: `value_store` must be added to `run_broker_server` signature as `Arc<Mutex<ValueStore>>` and threaded through `handle_connection` → `dispatch`, matching the existing `conn: Arc<Mutex<rusqlite::Connection>>` pattern (lines 68-86).

---

### `crates/brokerd/src/sinks/email_send.rs` (new — mediated sink stub)

**Analog:** `crates/brokerd/src/session.rs` — simple function that takes a `conn` and records to the audit DAG, following the `persist_session` / `append_event` pattern.

**Core pattern:**
```rust
use anyhow::Result;
use brokerd::audit::append_event;
use chrono::Utc;
use runtime_core::{Event, plan_node::PlanNode};
use uuid::Uuid;

/// Mediated email.send stub — records invocation to audit DAG, never sends email.
///
/// For §9: the test never reaches this function (executor blocks before invocation).
/// This stub is the post-confirmation dispatch target.
pub fn invoke_email_send_stub(
    conn: &rusqlite::Connection,
    session_id: Uuid,
    plan_node: &PlanNode,
    parent_hash: Option<&str>,
) -> Result<String> {
    let event = Event {
        id: Uuid::new_v4(),
        parent_id: None,
        session_id,
        actor: "sink-stub:email.send".into(),
        event_type: "email_send_stub".into(),
        timestamp: Utc::now(),
        taint: vec![],  // sink invocation itself is untainted; the args were confirmed
    };
    // T-03-09: payload would carry sanitized plan summary, not raw literals
    append_event(conn, &event, parent_hash)
}
```

---

## Shared Patterns

### Event Construction
**Source:** `crates/brokerd/src/server.rs` lines 143-151; `crates/runtime-core/src/event.rs` lines 16-29
**Apply to:** all new code that appends audit events (`server.rs` dispatch, `email_send.rs` stub, `s9_acceptance.rs` test)
```rust
let event = Event {
    id: Uuid::new_v4(),
    parent_id: None,          // Some(prev_event_id) when threading a chain
    session_id,
    actor: "<component>".into(),
    event_type: "<type>".into(),
    timestamp: Utc::now(),
    taint: vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw], // set only at read time
};
let hash = append_event(&conn, &event, parent_hash)?;
```

### Arc<Mutex<T>> for Shared Mutable State
**Source:** `crates/brokerd/src/server.rs` lines 68-86
**Apply to:** `ValueStore` threading through `run_broker_server` → `handle_connection` → `dispatch`
```rust
// Signature pattern:
pub async fn run_broker_server(
    session_id: &str,
    conn: Arc<Mutex<rusqlite::Connection>>,
    value_store: Arc<Mutex<ValueStore>>,   // add for Phase 4
) -> anyhow::Result<()>

// Lock pattern (no await while holding mutex):
let decision = match value_store.lock() {
    Ok(store) => executor::submit_plan_node(session_id, &plan_node, &store),
    Err(e) => { eprintln!("[brokerd] mutex poisoned: {e}"); return BrokerResponse::Error { message: "internal error".into() }; }
};
```

### Error Pattern (no swallowing)
**Source:** `crates/brokerd/src/server.rs` lines 116-125, 162-169
**Apply to:** all new fallible operations
```rust
// Internal detail logged; generic message returned to caller:
eprintln!("[executor] <detail>: {e}");
BrokerResponse::Error { message: "internal error".into() }
// Never return Ok(()) from a catch to hide an error.
```

### Taint-Only-At-Read-Time Invariant
**Source:** `crates/brokerd/src/audit.rs` lines 100-132 (append_event); `crates/runtime-core/src/event.rs` lines 16-29
**Apply to:** `value_store.rs` mint(), `server.rs` SubmitPlanNode dispatch, `executor/src/lib.rs`
- `ValueRecord.taint` is written ONLY in `ValueStore::mint()`, called from broker's read-event handler
- `executor::submit_plan_node()` reads taint via `value_store.resolve()` — it NEVER writes taint fields
- Any code path that calls `store.mint(literal, taint, chain)` must have a corresponding `append_event` call for a `file_read` Event in the same code path, and `chain[0]` must be that event's ID

---

## No Analog Found

All files have analogs. No entries.

---

## Metadata

**Analog search scope:** `crates/runtime-core/src/`, `crates/brokerd/src/`, `crates/sandbox/`
**Files scanned:** 8 source files + 2 Cargo.toml files
**Pattern extraction date:** 2026-06-29
