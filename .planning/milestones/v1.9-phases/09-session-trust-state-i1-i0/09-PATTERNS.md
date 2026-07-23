# Phase 9: Session Trust State (I1 + I0) - Pattern Map

**Mapped:** 2026-07-06
**Files analyzed:** 9 (3 new/extended modules, 6 call-site update targets treated as one group)
**Analogs found:** 9 / 9 (all extensions of existing files — no new files/modules needed except a possible test-only sink fixture)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/runtime-core/src/session.rs` (add `Draft` variant) | model (domain enum) | CRUD (state field) | Same file, existing `SessionStatus` enum | exact (self-extension) |
| `crates/runtime-core/src/executor_decision.rs` (add `DenyReason` variant) | model (domain enum) | request-response | Same file, existing `DenyReason` enum + `TaintLabel::is_untrusted` exhaustive-match style in `plan_node.rs` | exact (self-extension) |
| `crates/brokerd/src/session.rs` (`create_session` conditional + new `update_session_status`) | service (session lifecycle) | CRUD | Same file's own `persist_session` (INSERT) as the analog for the new UPDATE fn | exact (self-extension) |
| `crates/brokerd/src/quarantine.rs` (`mint_from_read` demotion) | service (sole mint/trust-flip site) | event-driven | Same function, existing Steps 1-3 (event append + mint) | exact (self-extension) |
| `crates/brokerd/src/server.rs` (`session_status` threading + Step 0.5 call) | controller (per-connection dispatch) | request-response / event-driven | Same file's existing `last_event_id`/`last_event_hash` threading pattern | exact (same file, same mechanism) |
| `crates/executor/src/lib.rs` (`submit_plan_node` Step 0.5) | service (pure decision function) | request-response | Same function's existing Step 1/1a/1b/2 per-arg loop pattern | exact (self-extension) |
| `crates/executor/src/sink_sensitivity.rs` (new `sink_effect_class`) | utility (hardcoded classification) | transform | Same file's `is_routing_sensitive`/`is_content_sensitive` | exact |
| `cli/caprun/src/main.rs` (new `--seed-from-file` on-ramp) | controller (CLI orchestrator) | request-response / file-I/O | Same file's existing positional-arg parsing block (lines 45-75) | exact (self-extension) |
| 14 `submit_plan_node` call sites (tests + `brokerd::lib.rs` wrapper) | test / service wrapper | request-response | Existing call sites themselves (mechanical arg-add) | exact |

## Pattern Assignments

### `crates/runtime-core/src/session.rs` (model, CRUD)

**Analog:** same file, current enum (lines 12-18 per RESEARCH.md)

**Core pattern — insert variant, no reorder-sensitivity:**
```rust
// Current:
pub enum SessionStatus {
    Active,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
// New (design doc §1, insert Draft after Active):
pub enum SessionStatus {
    Active,
    Draft,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
```
serde derives are already `#[derive(..., Serialize, Deserialize)]` — tag-by-name, so insertion position is safe (confirmed by RESEARCH.md).

---

### `crates/runtime-core/src/executor_decision.rs` (model, request-response)

**Analog:** same file's `DenyReason` enum; exhaustive-match discipline precedent is `TaintLabel::is_untrusted()` in `crates/runtime-core/src/plan_node.rs`.

**Core pattern — append one variant, never a second taxonomy:**
```rust
pub enum DenyReason {
    DanglingHandle,
    EmptyTaintInvariantViolation,
    MissingProvenanceAnchor,
    UnknownSink(String),
    UnknownArg(String),
    DuplicateArg(String),
    MissingArg(String),
    // v1.2 addition:
    DraftOnlySessionDeniesCommitIrreversible { sink: SinkId },
}
```
Doc comment on the enum already states "the ONE base denial error enum ... never introduce a second denial error type" — preserve, don't duplicate.

---

### `crates/brokerd/src/session.rs` (service, CRUD)

**Analog:** its own `persist_session` (INSERT) is the closest existing analog for the new UPDATE path — same file, same connection/param conventions.

**Imports (lines 1-10, unchanged):**
```rust
use chrono::Utc;
use runtime_core::{Session, SessionStatus};
use uuid::Uuid;
```

**Current `create_session` (lines 18-27) — becomes conditional:**
```rust
pub fn create_session(intent_id: Uuid) -> Session {
    let now = Utc::now();
    Session {
        id: Uuid::new_v4(),
        intent_id,
        status: SessionStatus::Active,   // ORIGIN-02: must become conditional on seed-provenance
        created_at: now,
        updated_at: now,
    }
}
```

**Current `persist_session` (lines 37-49) — INSERT-only, the analog for the new UPDATE fn:**
```rust
pub fn persist_session(conn: &rusqlite::Connection, session: &Session) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, intent_id, status, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            session.id.to_string(),
            session.intent_id.to_string(),
            serde_json::to_string(&session.status)?,
            session.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}
// NEW function, mirroring this exact param/error-handling style:
// pub fn update_session_status(conn: &rusqlite::Connection, session_id: Uuid, status: &SessionStatus)
//     -> anyhow::Result<()> {
//     conn.execute(
//         "UPDATE sessions SET status = ?1 WHERE id = ?2",
//         rusqlite::params![serde_json::to_string(status)?, session_id.to_string()],
//     )?;
//     Ok(())
// }
```

**Error handling pattern:** `anyhow::Result<()>` returned directly from `conn.execute(...)?` — no custom error type, matches every other function in this file.

---

### `crates/brokerd/src/quarantine.rs` (service, event-driven — sole trust-flip site)

**Analog:** same function's own Steps 1-3 (file_read Event append + ValueRecord mint) — the new demotion write is Step 4, same style, same connection, same lock.

**Imports (top of file, unchanged):**
```rust
use crate::audit::append_event;
use chrono::Utc;
use runtime_core::{Event, TaintLabel};
use uuid::Uuid;
```

**Core pattern — current `mint_from_read` (lines 177-238), with the Step 4 extension point:**
```rust
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_id: Option<Uuid>,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId)> {
    // Step 1: build file_read Event, taint derived from claim_type (fail-closed on unknown type)
    let taint = match claim.claim_type.as_str() {
        "email_address" => vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
        "relative_path" => vec![TaintLabel::ExternalUntrusted, TaintLabel::PathRaw],
        other => return Err(anyhow::anyhow!("mint_from_read: unknown claim_type `{other}` (fail-closed)")),
    };
    let event_id = Uuid::new_v4();
    let event = Event::new(event_id, parent_id, session_id, "confined-reader".into(),
        "file_read".into(), Utc::now(), taint.clone());

    // Step 2: append to audit DAG
    let read_hash = append_event(conn, &event, parent_hash)?;

    // Step 3: mint ValueRecord, provenance_chain[0] == event_id (genuine-taint anchor)
    let value_id = store.mint(claim.value.clone(), taint, vec![event_id])
        .map_err(|e| anyhow::anyhow!("mint invariant: {e:?}"))?;

    // NEW Step 4 (TAINT-01/04): atomic demotion pair, SAME conn/lock as above —
    // never a second lock acquisition (Pitfall 5).
    // crate::session::update_session_status(conn, session_id, &SessionStatus::Draft)?;
    // let demoted_event = Event::new(Uuid::new_v4(), Some(event_id), session_id,
    //     "broker".into(), "session_demoted".into(), Utc::now(), vec![]);
    // append_event(conn, &demoted_event, Some(&read_hash))?;

    Ok((event_id, read_hash, value_id))
}
```

**Error handling pattern:** every step's error propagates with `?`; fail-closed on unknown claim_type via explicit `Err(anyhow::anyhow!(...))` — never a silent default. Apply the same to any new step.

---

### `crates/brokerd/src/server.rs` (controller, request-response / event-driven)

**Analog:** this file's own `last_event_id`/`last_event_hash` per-connection mutable-local threading — `session_status` must be threaded identically.

**Imports (lines 41-54, unchanged):**
```rust
use crate::audit::append_event;
use crate::proto::{BrokerRequest, BrokerResponse, WorkerClaim};
use crate::quarantine::{mint_from_intent, mint_from_read, Claim};
use runtime_core::intent::CaprunIntent;
use crate::session::{create_session, persist_session};
use executor::value_store::ValueStore;
use runtime_core::Event;
use uuid::Uuid;
```

**Auth/trust pattern (HARD-03 precedent — `session_id` sourced from connection, never IPC) — the SAME discipline `session_status` must follow:**
```rust
// SubmitPlanNode arm (line ~347, current):
BrokerRequest::SubmitPlanNode { plan_node } => {
    let effect_id = Uuid::new_v4();
    // session_id comes from the connection — NEVER from the IPC message (HARD-03).
    let decision =
        executor::submit_plan_node(session_id, effect_id, &plan_node, value_store);
    // NEW: session_status threaded the same way —
    // executor::submit_plan_node(session_id, effect_id, &plan_node, value_store, &session_status)
```

**Core threading pattern to mirror — `handle_connection`'s existing mutable locals (lines 125-135, 177-190):**
```rust
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    mut last_event_id: Uuid,
    mut last_event_hash: String,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
) -> anyhow::Result<()> {
    let mut value_store = ValueStore::default();
    // NEW: let mut session_status = initial_session_status; (new param, seeded from
    // run_broker_server, itself seeded from create_session's result in main.rs)
    loop {
        // ... read/deserialize framed request ...
        dispatch_request(
            request, &mut stream, &conn, session_id,
            &mut last_event_id, &mut last_event_hash, &mut value_store,
            &workspace_root,
            // NEW: &mut session_status,
        ).await?;
    }
    Ok(())
}
```

**Update-in-place pattern (mirrors `*last_event_id = ...` after every append) — apply the same after `mint_from_read` demotes:**
```rust
// ReportClaims arm (lines 302-345, current) — after mint_from_read returns:
*last_event_id = read_event_id;
*last_event_hash = read_hash;
// NEW: session_status = SessionStatus::Draft;  (update the threaded local in place,
// unconditionally after every mint_from_read call, since the transition is one-way/idempotent)
```

**Error handling pattern:** `conn.lock().map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?` used identically at every DB-touching arm — reuse verbatim for any new session-status DB read/write.

---

### `crates/executor/src/lib.rs` (service, request-response — pure decision fn)

**Analog:** the function's own Step 1/1a/1b/2 per-arg loop — Step 0.5 is appended after the loop, same file, same style.

**Imports (top of file, unchanged style):**
```rust
use runtime_core::{DenyReason, ExecutorDecision, PlanNode, SinkBlockedAnchor};
use sha2::{Digest, Sha256};
use uuid::Uuid;
```

**Core pattern — current signature + end of function (lines 43-142), extension point:**
```rust
pub fn submit_plan_node(
    _session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    value_store: &ValueStore,
    // NEW 5th param:
    // session_status: &SessionStatus,
) -> ExecutorDecision {
    if let Err(reason) = sink_schema::validate_schema(plan_node) {
        return ExecutorDecision::Denied { reason };
    }
    for arg in &plan_node.args {
        // Steps 1/1a/1b/2/3 — UNCHANGED, must complete or return Block BEFORE Step 0.5 runs
        // (design doc §8 B1 fix: I2 Block always takes precedence).
    }

    // NEW Step 0.5 — after the loop, before Allowed, exhaustive match (design doc §8):
    // match *session_status {
    //     SessionStatus::Draft => {
    //         if sink_sensitivity::sink_effect_class(&plan_node.sink) == EffectClass::CommitIrreversible {
    //             return ExecutorDecision::Denied {
    //                 reason: DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink: plan_node.sink.clone() },
    //             };
    //         }
    //     }
    //     SessionStatus::Active => {}
    //     SessionStatus::WaitingApproval | SessionStatus::Done | SessionStatus::Failed
    //         | SessionStatus::RolledBack => {}
    // }

    ExecutorDecision::Allowed
}
```

**Error handling / decision pattern:** every deny returns `ExecutorDecision::Denied { reason }` inline at the point of failure — never a collected-errors list. Follow exactly for Step 0.5.

---

### `crates/executor/src/sink_sensitivity.rs` (utility, transform)

**Analog:** same file's `is_routing_sensitive` — the exact hardcoded-`match` shape `sink_effect_class` must mirror.

**Core pattern (lines 32-39, current):**
```rust
pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
// NEW function, same module, same style, but fail-closed on the unknown branch
// (returns the MOST restrictive class, not the permissive `false` this fn uses):
// pub fn sink_effect_class(sink: &SinkId) -> EffectClass {
//     match sink.0.as_str() {
//         "email.send"  => EffectClass::CommitIrreversible,
//         "file.create" => EffectClass::CommitIrreversible,
//         #[cfg(test)]
//         "test.observe" => EffectClass::Observe,   // TAINT-03 fixture (Pitfall 3)
//         _ => EffectClass::CommitIrreversible,      // fail-closed unknown-sink
//     }
// }
```
Note: this `_ =>` wildcard is permitted here — it is an internal `&str` match, not a match over the `EffectClass`/`DenyReason` enum itself (design doc §10 draws this distinction explicitly).

---

### `cli/caprun/src/main.rs` (controller, request-response / file-I/O)

**Analog:** the same file's existing positional-arg parsing block (lines 45-75) — the new flag is parsed the same way, before the existing positional args.

**Core pattern — current parse block (lines 45-75), extension point:**
```rust
let mut args = std::env::args().skip(1);
// NEW: peek/consume an optional `--seed-from-file <path>` flag BEFORE the
// existing positional parsing, mirroring this file's existing `.next().ok_or_else(...)`
// error-message style:
// let mut seed_from_file: Option<String> = None;
// (parse-and-strip the flag out of the iterator, then fall through to the
//  unchanged positional parsing below)

let intent_kind = args.next().ok_or_else(|| {
    anyhow::anyhow!("usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]")
})?;
// ... unchanged ...

// ── 2. Create session ── (lines 101-104, current) — extension point:
let intent_id = Uuid::new_v4();
let session = create_session(intent_id);   // NEW: create_session(intent_id, seed_provenance)
let session_id = session.id;
```

**Error handling pattern:** `anyhow::anyhow!("usage: ...")` on missing arg, `anyhow::bail!("unknown intent kind: ...")` on invalid value — fail-closed, exact strings shown; extend this style for a missing/unreadable seed file (V5 requirement per RESEARCH.md).

---

### 14 `submit_plan_node` call sites (test / service wrapper, request-response)

**Analog:** the call sites themselves — mechanical signature update, no new pattern needed.

**Locations (verified):**
- `crates/brokerd/src/lib.rs` — 1 definition (thin wrapper) + 2 test calls (lines ~32-41, ~64, ~79)
- `crates/brokerd/src/server.rs` — 1 call (line 353, shown above)
- `crates/brokerd/tests/phase5_dispatch.rs` — 2 calls (lines 112, 126)
- `crates/brokerd/tests/s9_acceptance.rs` — 3 calls (lines 124, 316, 425)
- `crates/executor/tests/executor_decision.rs` — 8 calls (lines 72, 133, 155, 184, 219, 240, 273, 297)

**Pattern:** every existing call passes exactly 4 args (`session_id, effect_id, &plan_node, &store`); add `&SessionStatus::Active` as the 5th arg to preserve existing test semantics unless the test is deliberately about `Draft` (TAINT-02/03 new tests).

```rust
// Before (all 14 sites):
executor::submit_plan_node(session_id, effect_id, &plan_node, &store)
// After (default/preserve-semantics update):
executor::submit_plan_node(session_id, effect_id, &plan_node, &store, &SessionStatus::Active)
```

---

## Shared Patterns

### Exhaustive-match discipline (no wildcard, ever, for security enums)
**Source:** `crates/runtime-core/src/plan_node.rs` — `TaintLabel::is_untrusted()` doc comment: "This method uses an EXPLICIT `match self` with NO wildcard arm. Adding a new `TaintLabel` variant without updating this match is a compile error, not a silent false-allow."
**Apply to:** the new Step 0.5 match over `SessionStatus` in `executor/src/lib.rs`, and any future match over `EffectClass`/`DenyReason`.
```rust
match *session_status {
    SessionStatus::Draft => { /* ... */ }
    SessionStatus::Active => { /* ... */ }
    SessionStatus::WaitingApproval | SessionStatus::Done | SessionStatus::Failed
        | SessionStatus::RolledBack => { /* ... */ }
}
```

### Fail-closed on unknown/malformed input
**Source:** `crates/brokerd/src/quarantine.rs::mint_from_read`'s unknown-`claim_type` branch (`Err(anyhow::anyhow!(...))`, never a default taint), and `sink_effect_class`'s unknown-sink → `CommitIrreversible` (most restrictive, never permissive).
**Apply to:** the new CLI file-derived-seed path (missing/unreadable file → hard error, never silently fall back to trusted-arg), and any new match arm touching sink/session-status classification.

### Per-connection mutable-local threading (never a global/thread-local)
**Source:** `crates/brokerd/src/server.rs`'s `last_event_id`/`last_event_hash` threaded as `mut` locals through `handle_connection` and passed `&mut` into `dispatch_request`.
**Apply to:** the new `session_status: SessionStatus` local — same declaration site, same update-after-mutation-succeeds discipline, same "seeded once, updated in place" lifecycle. Do NOT re-derive from a DB query per call (Pitfall 2).

### Trusted-path-only sourcing (never from IPC/PlanNode/worker assertion)
**Source:** `session_id` in `server.rs`'s `SubmitPlanNode` arm: "session_id comes from the connection — NEVER from the IPC message (HARD-03)."
**Apply to:** `session_status` passed into `executor::submit_plan_node` — must be the broker's own per-connection local, never a field read off `plan_node` or the `BrokerRequest` wire type.

### Mutex/connection error handling
**Source:** `crates/brokerd/src/server.rs`, repeated at every DB touch point.
```rust
let locked = conn.lock().map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
```
**Apply to:** any new lock acquisition inside `mint_from_read`'s extension (Step 4) or `server.rs`'s new session-status resolution code — reuse this exact idiom, do not introduce a new poisoned-mutex handling style.

## No Analog Found

None — every file in scope is an extension of an existing module with a direct same-file or same-crate analog. The one genuinely novel piece (a `SeedProvenance`-like enum and its exact persistence mechanism — event-payload vs. new `sessions` column) has no existing analog because no prior phase persisted CLI-decided provenance; RESEARCH.md's Open Question 2 recommends recording it in the `session_created` Event's payload (lower risk, no schema migration) rather than a new column — planner should decide this explicitly, informed by the `Event` payload precedent already used for other event types in `crates/brokerd/src/audit.rs`.

## Metadata

**Analog search scope:** `crates/runtime-core/src`, `crates/brokerd/src`, `crates/brokerd/tests`, `crates/executor/src`, `crates/executor/tests`, `cli/caprun/src`
**Files scanned:** 9 source files read directly this session (session.rs x2, executor_decision.rs, quarantine.rs, server.rs, lib.rs x2, sink_sensitivity.rs, main.rs) plus grep across all 14 `submit_plan_node` call sites
**Pattern extraction date:** 2026-07-06
