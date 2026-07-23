# Phase 6: Deterministic Planner & Intent Input — Pattern Map

**Mapped:** 2026-06-30
**Files analyzed:** 11 (8 edits, 1 new file, 2 test extensions)
**Analogs found:** 11 / 11

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/runtime-core/src/intent.rs` | model/types | transform | `crates/runtime-core/src/plan_node.rs` (TaintLabel, ValueId) | role-match |
| `crates/brokerd/src/quarantine.rs` (extend) | service | CRUD | itself — `mint_from_read` is the template | exact |
| `crates/brokerd/src/proto.rs` (extend) | model/wire-types | request-response | existing `WorkerClaim`/`ReportClaims`/`ClaimsReceived` pattern | exact |
| `crates/brokerd/src/server.rs` (extend) | controller/dispatch | request-response | existing `ReportClaims` dispatch arm (lines 289–319) | exact |
| `crates/executor/src/lib.rs` (edit) | service/enforcer | request-response | itself — predicate at line 62–63 is the edit site | exact |
| `cli/caprun/src/planner.rs` (NEW) | service/planner | transform | `cli/caprun/src/worker.rs` scripted planner block (lines 110–116) | role-match |
| `cli/caprun/src/main.rs` (extend) | controller/orchestrator | request-response | itself — arg parse block (lines 38–42) | exact |
| `cli/caprun/src/worker.rs` (extend) | controller/worker | request-response | itself — existing send/recv protocol block (lines 66–133) | exact |
| `crates/executor/tests/executor_decision.rs` (extend) | test | CRUD | itself (existing HARD-02 placeholder) | exact |
| `crates/brokerd/tests/quarantine.rs` / `s9_acceptance.rs` (extend) | test | CRUD | `crates/brokerd/src/quarantine.rs` test block (lines 161–331) | exact |
| `cli/caprun/tests/s9_live_block.rs` (extend) | test | request-response | itself — `run_caprun_on` helper + `#[cfg(target_os = "linux")]` pattern | exact |

---

## Pattern Assignments

### `crates/runtime-core/src/intent.rs` (model, transform)

**Analog:** `crates/runtime-core/src/plan_node.rs`

**Imports pattern** (plan_node.rs lines 1–21 style — no I/O, no async):
```rust
// No imports beyond serde derives — this is a pure-type file.
// Mirror the same derive set used on TaintLabel and PlanNode.
```

**Core pattern — enum definition** (plan_node.rs lines 12–21 as template):
```rust
// EXISTING (plan_node.rs lines 12–21):
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaintLabel {
    UserTrusted,
    LocalWorkspace,
    ExternalUntrusted,
    EmailRaw,
    PdfRaw,
    LlmGenerated,
    WorkerExtracted,
}

// NEW CaprunIntent — same derive set, add serde tag for struct variants:
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum CaprunIntent {
    SendEmailSummary { recipient: String },
}
```

**Core pattern — `TaintLabel::is_untrusted()` method** (add to plan_node.rs after enum):
```rust
// Use EXPLICIT match (no wildcard) so new variants force a compile error.
// SECURITY: a wildcard `_ => false` would silently treat a new untrusted label
// as trusted (false-allow is worse than false-block — Pitfall 5 in RESEARCH.md).
impl TaintLabel {
    pub fn is_untrusted(&self) -> bool {
        match self {
            TaintLabel::ExternalUntrusted
            | TaintLabel::EmailRaw
            | TaintLabel::PdfRaw
            | TaintLabel::LlmGenerated
            | TaintLabel::WorkerExtracted => true,
            TaintLabel::UserTrusted | TaintLabel::LocalWorkspace => false,
        }
    }
}
```

**Error handling:** None — pure types, no fallible operations.

**Testing pattern** — round-trip serde test in `crates/runtime-core/tests/`:
```rust
// Mirror the quarantine test style: assert specific field values, no helpers needed.
#[test]
fn caprun_intent_serde_round_trip() {
    let intent = CaprunIntent::SendEmailSummary { recipient: "boss@company.com".into() };
    let json = serde_json::to_string(&intent).unwrap();
    let back: CaprunIntent = serde_json::from_str(&json).unwrap();
    assert_eq!(intent, back);
}

#[test]
fn taint_label_is_untrusted_user_trusted_returns_false() {
    assert!(!TaintLabel::UserTrusted.is_untrusted());
}

#[test]
fn taint_label_is_untrusted_external_untrusted_returns_true() {
    assert!(TaintLabel::ExternalUntrusted.is_untrusted());
}
```

---

### `crates/brokerd/src/quarantine.rs` — ADD `mint_from_intent` (service, CRUD)

**Analog:** `mint_from_read` in same file (lines 124–159)

**Imports pattern** (quarantine.rs lines 16–22 — already present, no new imports needed):
```rust
use anyhow::Result;
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{plan_node::TaintLabel, Event};
use uuid::Uuid;
use crate::audit::append_event;
```

**Core pattern — `mint_from_intent`** (mirror of `mint_from_read`, lines 124–159):
```rust
// mint_from_read (lines 124–159) is the exact template. Differences:
//   - taint: [UserTrusted] instead of [ExternalUntrusted, EmailRaw]
//   - event_type: "intent_received" instead of "file_read"
//   - actor: "user-intent" instead of "confined-reader"
//   - argument: `literal: String` instead of `claim: &Claim`
//   - event taint: vec![] (the event itself carries no taint; only the ValueRecord does)

pub fn mint_from_intent(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    literal: String,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId)> {
    let taint = vec![TaintLabel::UserTrusted];
    let event_id = Uuid::new_v4();
    let event = Event {
        id: event_id,
        parent_id: None,      // Phase 7 wires parent_id linkage (same as mint_from_read)
        session_id,
        actor: "user-intent".into(),
        event_type: "intent_received".into(),
        timestamp: Utc::now(),
        taint: vec![],        // event itself carries no taint (differs from mint_from_read)
    };
    let intent_hash = append_event(conn, &event, parent_hash)?;
    let value_id = store.mint(literal, taint, vec![event_id]);
    Ok((event_id, intent_hash, value_id))
}
```

**Error handling pattern** (same as `mint_from_read`): return `Result<(Uuid, String, ValueId)>`, propagate `?` from `append_event`.

**Testing pattern** (mirror quarantine.rs lines 219–330):
```rust
// Add to quarantine.rs #[cfg(test)] block, or extend s9_acceptance.rs.
// Key assertions (mirroring mint_from_read_anchor_identity, lines 222–251):
//   record.provenance_chain[0] == returned intent_event_id
//   find_event_by_type("intent_received").id == intent_event_id
//   record.taint == [TaintLabel::UserTrusted]
//   record.literal == the literal passed in
//   DAG event does NOT carry taint (event.taint is empty)
```

---

### `crates/brokerd/src/proto.rs` — ADD `ProvideIntent` / `IntentAccepted` (model, request-response)

**Analog:** `WorkerClaim` enum + `ReportClaims`/`ClaimsReceived` (proto.rs lines 19–83)

**Core pattern — tagged enum and new variants** (proto.rs lines 19–83):
```rust
// EXISTING WorkerClaim (lines 19–26) — shows the serde tag convention:
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim { EmailAddress(String) }

// EXISTING BrokerRequest (lines 29–59) — ADD one variant:
// ProvideIntent { intent: runtime_core::intent::CaprunIntent }
// Place BEFORE RequestFd (matches the wire ordering — worker sends ProvideIntent first).

// EXISTING BrokerResponse (lines 62–83) — ADD one variant:
// IntentAccepted { value_id: runtime_core::plan_node::ValueId }
// Mirrors ClaimsReceived { value_ids } (lines 73–76) but singular.
```

**Security contract comment** (copy the HARD-03 comment style from SubmitPlanNode, lines 51–56):
```rust
/// Worker declares the user's typed intent. Broker calls mint_from_intent and
/// returns an opaque UserTrusted ValueId handle.
/// Sent BEFORE RequestFd. The literal flows from the trusted orchestrator env var;
/// the broker mints authoritatively — the worker never constructs the ValueRecord.
ProvideIntent { intent: runtime_core::intent::CaprunIntent },

/// Acknowledgement for ProvideIntent: opaque handle for the minted UserTrusted
/// ValueRecord. Mirrors ClaimsReceived but singular (one intent value per session).
IntentAccepted { value_id: runtime_core::plan_node::ValueId },
```

---

### `crates/brokerd/src/server.rs` — ADD `ProvideIntent` dispatch arm (controller, request-response)

**Analog:** `BrokerRequest::ReportClaims` arm (server.rs lines 289–319)

**Imports to add** (server.rs lines 41–53 — extend existing):
```rust
// Add to existing imports:
use crate::quarantine::mint_from_intent;
use runtime_core::intent::CaprunIntent;
```

**Core dispatch pattern** (mirror ReportClaims arm, lines 289–319):
```rust
// ReportClaims arm structure to mirror:
//   1. Lock conn mutex
//   2. Call mint_from_read (→ mint_from_intent for this arm)
//   3. Advance *last_event_id and *last_event_hash
//   4. send_response(stream, &BrokerResponse::ClaimsReceived { value_ids }) (→ IntentAccepted)

BrokerRequest::ProvideIntent { intent } => {
    let literal = match &intent {
        CaprunIntent::SendEmailSummary { recipient } => recipient.clone(),
    };
    let (intent_event_id, intent_hash, value_id) = {
        let locked = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        mint_from_intent(&locked, value_store, session_id, literal, Some(last_event_hash))?
    };
    *last_event_id = intent_event_id;
    *last_event_hash = intent_hash;
    send_response(stream, &BrokerResponse::IntentAccepted { value_id }).await?;
}
```

**Placement:** Insert as the FIRST arm in the `match request` block (before `RequestFd`), matching the wire ordering.

**Error handling:** Same as all other arms — `?` propagation, mutex poisoned error via `anyhow::anyhow!`.

---

### `crates/executor/src/lib.rs` — UPDATE blocking predicate (service/enforcer, request-response)

**Analog:** itself — edit line 63

**Edit site** (lib.rs lines 59–72):
```rust
// BEFORE (line 63):
        if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
            && !record.taint.is_empty()
        {

// AFTER (HARD-02):
        if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
            && record.taint.iter().any(|t| t.is_untrusted())
        {
```

**Surgical change:** ONE line changed. The surrounding function signature, the `Denied` arm, the `Allowed` return, and all comments are untouched.

**Testing pattern** (extend `crates/executor/tests/executor_decision.rs`):
```rust
// HARD-02 case: UserTrusted-only record in routing-sensitive arg → Allowed.
// Mirror the existing test structure — construct a ValueStore, mint a record with
// taint: vec![TaintLabel::UserTrusted], build a PlanNode for "email.send" / "to",
// call submit_plan_node, assert ExecutorDecision::Allowed.

// Also add: ExternalUntrusted record → still BlockedPendingConfirmation (regression guard).
```

---

### `cli/caprun/src/planner.rs` (NEW file — service/planner, transform)

**Analog:** worker.rs scripted planner block (lines 108–116)

**Imports pattern** (worker.rs lines 35–42 as reference for crate names):
```rust
use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};
```

**Core pattern** (worker.rs lines 108–116 as template, extracted to a pure function):
```rust
// EXISTING inline planner in worker.rs (lines 108–116) — the template:
//   let plan_node = PlanNode {
//       sink: SinkId("email.send".into()),
//       args: vec![PlanArg { name: "to".into(), value_id: value_ids[0].clone() }],
//   };

// NEW pure function (no I/O, no async, no ValueRecord access):
pub fn plan_from_intent(
    intent: &CaprunIntent,
    intent_value_id: ValueId,
    _file_value_ids: &[ValueId],
) -> PlanNode {
    match intent {
        CaprunIntent::SendEmailSummary { .. } => PlanNode {
            sink: SinkId("email.send".into()),
            args: vec![PlanArg {
                name: "to".into(),
                value_id: intent_value_id,
            }],
        },
    }
}
```

**Security invariant:** The planner receives only opaque `ValueId` handles — never a `ValueRecord`, never a literal, never taint. The `..` in the match arm intentionally ignores `recipient` (the literal is already in the broker's ValueStore, accessed only via the returned handle).

**Error handling:** None — pure function, infallible.

**Testing:**
```rust
// Unit test: plan_from_intent(SendEmailSummary { recipient: _ }, intent_vid, &[])
//   → PlanNode { sink: "email.send", args: [PlanArg { name: "to", value_id: intent_vid }] }
// Compile-time PLAN-03: function signature takes ValueId, not ValueRecord — enforced by types.
```

---

### `cli/caprun/src/main.rs` — UPDATE arg parsing (controller/orchestrator, request-response)

**Analog:** itself — arg parse block (lines 38–42)

**Edit site** (main.rs lines 38–42):
```rust
// BEFORE:
    let mut args = std::env::args().skip(1);
    let workspace_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: caprun <workspace-file> [audit-db-path]"))?;
    let audit_path = args.next().unwrap_or_else(|| ":memory:".to_string());

// AFTER (PLAN-01 — two positional args before workspace):
    let mut args = std::env::args().skip(1);
    let intent_kind = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]"))?;
    let intent_param = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]"))?;
    let workspace_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]"))?;
    let audit_path = args.next().unwrap_or_else(|| ":memory:".to_string());

    let intent = match intent_kind.as_str() {
        "send-email-summary" => runtime_core::intent::CaprunIntent::SendEmailSummary {
            recipient: intent_param,
        },
        _ => anyhow::bail!("unknown intent kind: {intent_kind}"),
    };
```

**Worker spawn env var** (main.rs lines 97–103 — add INTENT):
```rust
    let mut child = std::process::Command::new(&worker_binary)
        .env("BROKER_SOCK", format!("/agentos/{session_id}"))
        .env("SESSION_ID", session_id.to_string())
        .env("WORKSPACE_FILE", &workspace_path)
        .env("INTENT", serde_json::to_string(&intent)?)   // ADD THIS LINE
        .spawn()
        .context("spawn caprun-worker")?;
```

**Update the usage comment** at the top of main.rs: `/// Usage: caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]`

---

### `cli/caprun/src/worker.rs` — ADD ProvideIntent send/recv (controller/worker, request-response)

**Analog:** itself — the existing RequestFd / ReportClaims / SubmitPlanNode protocol blocks (lines 44–133)

**Env var parse** (mirror `WORKSPACE_FILE` pattern, lines 45–46):
```rust
// Add after existing env var reads:
let intent_json = std::env::var("INTENT").context("INTENT")?;
let intent: runtime_core::intent::CaprunIntent =
    serde_json::from_str(&intent_json).context("parse INTENT")?;
```

**Protocol insertion** (insert after `apply_confinement()` at line 63, BEFORE `RequestFd` at line 66):
```rust
// ── Send BrokerRequest::ProvideIntent ─────────────────────────────────────
// Sent AFTER self-confinement (ordering invariant: connect → set_nonblocking →
// apply_confinement → ProvideIntent → RequestFd).
send_framed(&std_stream, &BrokerRequest::ProvideIntent { intent: intent.clone() })?;

// ── Receive opaque UserTrusted ValueId handle ─────────────────────────────
let intent_value_id = match recv_framed::<BrokerResponse>(&std_stream)? {
    BrokerResponse::IntentAccepted { value_id } => value_id,
    other => anyhow::bail!("unexpected response to ProvideIntent: {other:?}"),
};
```

**Planner call** (replace inline planner at lines 108–116 with call to `plan_from_intent`):
```rust
// BEFORE (lines 108–116) — inline scripted planner:
    let plan_node = PlanNode {
        sink: SinkId("email.send".into()),
        args: vec![PlanArg { name: "to".into(), value_id: value_ids[0].clone() }],
    };

// AFTER — delegate to planner module:
    let plan_node = crate::planner::plan_from_intent(&intent, intent_value_id, &value_ids);
```

**Benign-content guard** (keep existing guard at lines 103–106 — no change needed):
```rust
    if value_ids.is_empty() {
        eprintln!("[worker] no claims extracted — benign content, exiting 0");
        return Ok(());
    }
```

**Imports to add** (worker.rs lines 35–42 — extend):
```rust
// Add: runtime_core::intent::CaprunIntent (for INTENT env var parse)
// BrokerRequest::ProvideIntent and BrokerResponse::IntentAccepted are part of existing
// proto imports — they just need to exist in proto.rs.
```

---

### `cli/caprun/tests/s9_live_block.rs` — ADD clean allow-path e2e test (test, request-response)

**Analog:** itself — `run_caprun_on` helper (lines 39–60) and `s9_live_caprun_exits_nonzero` test (lines 64–72)

**Key pattern differences for the clean-path test:**
```rust
// CLEAN content: no hostile email, just a user-provided intent recipient.
#[cfg(target_os = "linux")]
const CLEAN_CONTENT: &[u8] = b"Q3 summary ready for distribution.";  // no email addresses

// run_caprun_on must accept intent args (update the helper or add a new one):
#[cfg(target_os = "linux")]
fn run_caprun_intent_on(
    intent_kind: &str,
    intent_param: &str,
    content: &[u8],
    tag: &str,
) -> (bool, std::path::PathBuf) {
    // ... same tmp dir pattern as run_caprun_on ...
    let output = std::process::Command::new(caprun_bin)
        .arg(intent_kind)      // new positional arg 1
        .arg(intent_param)     // new positional arg 2
        .arg(workspace_file.to_str().unwrap())
        .arg(audit_db_path.to_str().unwrap())
        .output()
        .expect("spawn caprun");
    (output.status.success(), audit_db_path)
}

// Assertions for the clean-path test:
//   caprun exits SUCCESS (exit 0)
//   audit DB contains an `intent_received` event
//   audit DB contains a `plan_node_evaluated` event (not `sink_blocked`)
//   NO `sink_blocked` event
//   intent_received event does NOT carry taint (taint: [])
```

**`#[cfg(target_os = "linux")]` gate:** Apply to all new test bodies, same as existing tests. The cross-platform guard test at line 131 stays ungated.

---

## Shared Patterns

### IPC Framing (send_framed / recv_framed)
**Source:** `cli/caprun/src/worker.rs` lines 136–155
**Apply to:** All new worker IPC sends/recvs
```rust
// 4-byte LE length prefix + JSON body (serde_json). Do NOT re-implement.
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

### Mutex-guarded SQLite access
**Source:** `crates/brokerd/src/server.rs` lines 301–312
**Apply to:** `ProvideIntent` dispatch arm in server.rs
```rust
let locked = conn
    .lock()
    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
// call mint_from_intent / append_event with &locked
```

### Causal chain advancement
**Source:** `crates/brokerd/src/server.rs` lines 313–315 (after ReportClaims), lines 285–286 (after RequestFd)
**Apply to:** `ProvideIntent` dispatch arm — advance AFTER successful mint:
```rust
*last_event_id = intent_event_id;
*last_event_hash = intent_hash;
```

### Audit event construction (no-taint events)
**Source:** `crates/brokerd/src/server.rs` lines 254–264 (fd_granted event)
**Apply to:** `intent_received` event in `mint_from_intent` — same field pattern, `taint: vec![]`, `parent_id: None` (Phase 7 deferred).

### `#[cfg(target_os = "linux")]` test gate
**Source:** `cli/caprun/tests/s9_live_block.rs` lines 33, 38, 64, 77
**Apply to:** All new live e2e test bodies in s9_live_block.rs. Always-compiled guard test (last test in file) stays ungated.

---

## No Analog Found

None. All files have strong analogs in the existing codebase.

---

## Key Security Invariants (carry into every plan)

1. **Anti-stapling:** `mint_from_intent` MUST append the `intent_received` event AND call `store.mint` in one function, so `provenance_chain[0] == intent_event_id`. Never fabricate an event ID.
2. **No wildcard in `is_untrusted()`:** Use explicit `match self` with no `_ =>` arm. A missed variant = compile error, not a silent false-allow.
3. **Planner holds only ValueId:** `plan_from_intent` must never call `ValueStore::mint` and must never see a `ValueRecord`. Types enforce this if the function signature is `(intent, ValueId, &[ValueId]) -> PlanNode`.
4. **ProvideIntent sent AFTER apply_confinement:** The ordering in worker.rs (connect → set_nonblocking → apply_confinement → ProvideIntent → RequestFd) is load-bearing.
5. **Per-connection ValueStore:** `mint_from_intent` is called ONLY from the `ProvideIntent` dispatch arm inside `handle_connection` — never from main before the broker server is running.

---

## Metadata

**Analog search scope:** `crates/brokerd/src/`, `crates/executor/src/`, `crates/runtime-core/src/`, `cli/caprun/src/`, `cli/caprun/tests/`
**Files scanned:** 8 source files read directly
**Pattern extraction date:** 2026-06-30
