# Phase 6: Deterministic Planner & Intent Input — Research

**Researched:** 2026-06-30
**Domain:** Rust in-process planner logic, IPC protocol extension, executor predicate hardening
**Confidence:** HIGH — all findings are sourced directly from the existing codebase (confirmed by Read tool)

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PLAN-01 | `caprun` accepts an intent input alongside the workspace (not just a bare file path) | §CLI Change: one new positional arg before workspace-file; worker receives intent via env var |
| PLAN-02 | A deterministic non-LLM planner maps a small typed intent enum to `PlanNode{sink, args}`, emitting only `SinkId` + existing `ValueId` handles | §Typed Intent Enum + §Planner: pure Rust function; variant → PlanNode mapping |
| PLAN-03 | The planner never sees raw bytes or taint labels — handles only opaque `ValueId` handles | §Planner: reinforced by handle model (ValueId only); planner receives typed intent struct + ValueId handles, never ValueRecord |
| PLAN-04 | A broker-owned `mint_from_intent` mints trusted values for clean/user-provided inputs, anchored to an `intent_received` audit event, separate from `mint_from_read` | §mint_from_intent: new function in `brokerd::quarantine`; new `intent_received` event type; new IPC round-trip |
| HARD-02 | The executor's blocking predicate is defined over explicitly-untrusted taint labels; `UserTrusted`/`LocalWorkspace`-only provenance does NOT block | §Executor Predicate Fix: change `!record.taint.is_empty()` to `record.taint.iter().any(|t| t.is_untrusted())` |
</phase_requirements>

---

## Summary

Phase 5 delivered the unified broker dispatch path, session-scoped ValueStore, `mint_from_read` anchored to real `file_read` events, and a live §9 hostile block with durable `sink_blocked` audit evidence. Phase 6 builds ONLY on what Phase 5 delivered: it adds a clean allow-path by (1) extending the caprun CLI to accept a typed intent, (2) adding `mint_from_intent` as a broker-owned sibling of `mint_from_read` that mints a `UserTrusted` ValueRecord anchored to a new `intent_received` audit event, (3) adding a minimal typed intent enum + deterministic scripted planner, and (4) fixing the executor's blocking predicate to explicitly enumerate untrusted labels so `UserTrusted`-only provenance passes.

The scope is narrow: NO `file.create` sink, NO `RelativePath` claim, NO workspace-root capability model. The existing `email.send` stub is the target for the clean allow-path demo — it is already wired and the executor already returns `Allowed` for untainted args. The end-to-end proof is: caprun exits 0, a `plan_node_evaluated` (not `sink_blocked`) event appears in the DAG, and the minted ValueRecord's `provenance_chain[0]` points to the `intent_received` event.

**Primary recommendation:** Extend the broker IPC protocol with one new request/response pair (`ProvideIntent` / `IntentAccepted`), implement `mint_from_intent` in `brokerd::quarantine`, add a `TaintLabel::is_untrusted()` predicate, update the executor to use it, and update the worker to optionally send `ProvideIntent` + use the returned handle in the scripted planner.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CLI intent arg parsing | `cli/caprun/src/main.rs` | — | Orchestrator parses user input; passes intent kind to worker via env var |
| Typed intent enum (`CaprunIntent`) | `crates/runtime-core/src/intent.rs` | — | Pure type; zero I/O — runtime-core is the home for all locked domain types |
| Deterministic planner (intent → PlanNode) | `cli/caprun/src/worker.rs` (new `planner.rs` file) | — | Planner is part of the orchestrator boundary; it sees only typed intent + opaque ValueId handles |
| `mint_from_intent` (mints UserTrusted ValueRecord) | `crates/brokerd/src/quarantine.rs` | — | Broker owns the ValueStore and the audit DAG; `mint_from_intent` is a sibling of `mint_from_read` |
| `intent_received` audit event type | `crates/brokerd/src/quarantine.rs` | — | Same call that mints the ValueRecord also appends the audit event (genuine-provenance rule) |
| New IPC message (`ProvideIntent` / `IntentAccepted`) | `crates/brokerd/src/proto.rs` | `crates/brokerd/src/server.rs` | Proto owns the wire types; server dispatches them in `handle_connection` |
| Executor blocking predicate (`is_untrusted()`) | `crates/executor/src/lib.rs` + `crates/runtime-core/src/plan_node.rs` | — | Predicate lives on TaintLabel in runtime-core (pure types); executor calls it |
| Allow-path audit recording | `crates/brokerd/src/server.rs` (`SubmitPlanNode` arm) | — | Already records `plan_node_evaluated` on `Allowed`; no change needed |

---

## Standard Stack

### Core (existing — no new crates)

| Crate (workspace) | Purpose | Phase 6 Change |
|-------------------|---------|---------------|
| `runtime-core` | Pure domain types: `TaintLabel`, `ValueId`, `PlanNode`, `Event` | Add `CaprunIntent` enum; add `TaintLabel::is_untrusted()` |
| `crates/brokerd` | Broker: audit DAG, ValueStore minting, IPC dispatch | Add `mint_from_intent`; extend `proto.rs` + `server.rs` |
| `crates/executor` | Deterministic I2 enforcer (`submit_plan_node`) | Update blocking predicate to use `is_untrusted()` |
| `cli/caprun` | Orchestrator: CLI parsing, broker spawn, worker spawn | Add intent arg parsing; update worker to send `ProvideIntent` + use intent handle |

### No New External Crates

This phase requires zero new dependencies. All building blocks already exist:
- `uuid` (already used for ValueId, event IDs)
- `serde_json` (already used for IPC framing)
- `rusqlite` (already used for audit DAG)
- `tokio` (already used in broker server)
- `anyhow` (already used for error handling)

**Installation:** none needed. `cargo build --workspace` uses existing `Cargo.lock`.

---

## Package Legitimacy Audit

> No new external packages in this phase — pure workspace-internal changes.

| Package | Verdict | Disposition |
|---------|---------|-------------|
| (none) | N/A | No new packages installed |

---

## Architecture Patterns

### System Architecture Diagram

```
User CLI Input
   │
   ▼
caprun main (cli/caprun/src/main.rs)
   │  parse: CaprunIntent + workspace-file
   │  spawn broker task (brokerd::server::run_broker_server)
   │  spawn worker (caprun-worker, env: BROKER_SOCK, WORKSPACE_FILE, INTENT)
   │
   ├─► Broker Task (brokerd::server::handle_connection, per-connection)
   │      │  owns per-connection ValueStore
   │      │  threads last_event_id / last_event_hash (causal chain)
   │      │
   │      ├── ProvideIntent { intent }           [NEW - Phase 6]
   │      │      ▼ mint_from_intent()
   │      │      ▼ appends intent_received event → ValueId (UserTrusted, taint:[])
   │      │      ▼ BrokerResponse::IntentAccepted { value_id }
   │      │
   │      ├── RequestFd { path }
   │      │      ▼ broker opens file (ambient fs), pass_fd via SCM_RIGHTS
   │      │      ▼ appends fd_granted event
   │      │
   │      ├── ReportClaims { claims }
   │      │      ▼ mint_from_read() per claim
   │      │      ▼ appends file_read event (taint: [ExternalUntrusted, EmailRaw])
   │      │      ▼ BrokerResponse::ClaimsReceived { value_ids }
   │      │
   │      └── SubmitPlanNode { plan_node }
   │             ▼ executor::submit_plan_node()
   │             │  for each arg: resolve ValueId → ValueRecord
   │             │  if routing-sensitive AND record.taint.is_untrusted() → Block
   │             │  else → Allowed
   │             ▼ append sink_blocked OR plan_node_evaluated
   │             ▼ BrokerResponse::PlanNodeDecision { decision }
   │
   └─► Worker Process (cli/caprun/src/worker.rs)
          Self-confines after connecting
          Sends ProvideIntent → receives intent_value_id (UserTrusted)   [NEW]
          Sends RequestFd → receives file fd
          Reads file → extracts claims → ReportClaims → receives file_value_ids (tainted)
          Scripted planner: maps (CaprunIntent, intent_value_id, file_value_ids) → PlanNode
          Sends SubmitPlanNode
          Receives decision → exits 0 (Allowed) or exits 1 (Blocked)
```

### Recommended Project Structure (additive changes only)

```
crates/runtime-core/src/
├── intent.rs              # ADD CaprunIntent enum here (alongside existing Intent struct)
│                          # ADD TaintLabel::is_untrusted() method
└── plan_node.rs           # (unchanged — TaintLabel already has UserTrusted variant)

crates/brokerd/src/
├── proto.rs               # ADD ProvideIntent / IntentAccepted variants
├── server.rs              # ADD ProvideIntent dispatch arm in dispatch_request()
└── quarantine.rs          # ADD mint_from_intent() (sibling of mint_from_read)

crates/executor/src/
└── lib.rs                 # UPDATE blocking predicate: use record.taint.iter().any(|t| t.is_untrusted())

cli/caprun/src/
├── main.rs                # ADD intent arg parsing; pass INTENT env var to worker
└── worker.rs              # ADD ProvideIntent send + IntentAccepted recv;
                           # UPDATE scripted planner to use intent_value_id for clean path
```

### Pattern 1: `mint_from_intent` (sibling of `mint_from_read`)

**What:** Mints a `UserTrusted` `ValueRecord` anchored to a new `intent_received` audit event. The `taint` vec is `[TaintLabel::UserTrusted]` (positive provenance assertion). The `provenance_chain[0]` equals the `intent_received` event id.

**When to use:** When the value comes from the user's direct input on the CLI (trusted boundary), not from reading an external file.

**Why `[TaintLabel::UserTrusted]` not `[]`:** HARD-02 says "UserTrusted/LocalWorkspace-only provenance does NOT block" — this implies those labels ARE present and the predicate must explicitly allow them. Using an empty vec would satisfy the current predicate accidentally; using `[UserTrusted]` makes the provenance explicit and the HARD-02 predicate change meaningful.

**Example (mirrors `mint_from_read` signature exactly):**
```rust
// Source: crates/brokerd/src/quarantine.rs (EXISTING mint_from_read pattern)

/// Append an `intent_received` Event and mint a `UserTrusted` ValueRecord.
///
/// SOLE site in brokerd that mints a UserTrusted ValueRecord.
/// Symmetrical to mint_from_read: event appended + record minted in one call
/// so provenance_chain[0] == returned intent_event_id (genuine-provenance anchor).
pub fn mint_from_intent(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    literal: String,       // e.g. "boss@company.com"
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, ValueId)> {
    // Step 1: Build the intent_received audit Event (no taint on the EVENT itself —
    // taint lives on the ValueRecord, not the event type).
    let event_id = Uuid::new_v4();
    let event = Event {
        id: event_id,
        parent_id: None,       // session-root for now (Phase 7 wires parent chain)
        session_id,
        actor: "user-intent".into(),
        event_type: "intent_received".into(),
        timestamp: Utc::now(),
        taint: vec![],         // the event itself carries no taint
    };

    // Step 2: Append to audit DAG.
    let intent_hash = append_event(conn, &event, parent_hash)?;

    // Step 3: Mint the ValueRecord with UserTrusted label.
    // taint: [UserTrusted] — positive provenance; NOT ExternalUntrusted/EmailRaw.
    // provenance_chain[0] == event_id (genuine-provenance anchor, symmetrical with
    // mint_from_read's provenance_chain[0] == file_read event_id).
    let taint = vec![TaintLabel::UserTrusted];
    let value_id = store.mint(literal, taint, vec![event_id]);

    Ok((event_id, intent_hash, value_id))
}
```

### Pattern 2: `TaintLabel::is_untrusted()` and updated executor predicate (HARD-02)

**What:** A method on `TaintLabel` that returns `true` only for labels that signal hostile/external origin. `UserTrusted` and `LocalWorkspace` return `false`.

**Why:** The current predicate `!record.taint.is_empty()` would block a `[UserTrusted]`-only record, defeating the clean allow-path. HARD-02 requires the predicate to be explicitly about untrusted labels.

**Example:**
```rust
// Source: crates/runtime-core/src/plan_node.rs (ADD this method)

impl TaintLabel {
    /// Returns true for labels that signal hostile/external origin.
    /// UserTrusted and LocalWorkspace are TRUSTED provenance labels — they do NOT block.
    pub fn is_untrusted(&self) -> bool {
        matches!(
            self,
            TaintLabel::ExternalUntrusted
                | TaintLabel::EmailRaw
                | TaintLabel::PdfRaw
                | TaintLabel::LlmGenerated
                | TaintLabel::WorkerExtracted
        )
    }
}
```

```rust
// Source: crates/executor/src/lib.rs (UPDATE existing predicate)

// BEFORE (Phase 5):
if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
    && !record.taint.is_empty()       // WRONG: blocks UserTrusted-only records
{

// AFTER (Phase 6, HARD-02):
if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
    && record.taint.iter().any(|t| t.is_untrusted())   // explicit untrusted check
{
```

### Pattern 3: Typed Intent Enum + Deterministic Planner

**What:** A small typed enum in `runtime-core` that maps to a specific sink + arg set. The planner is a pure function that takes `(CaprunIntent, intent_value_id: ValueId, file_value_ids: Vec<ValueId>)` and returns `PlanNode`.

**When to use:** When the user specifies a specific action on the CLI.

**Example:**
```rust
// Source: crates/runtime-core/src/intent.rs (ADD alongside existing Intent struct)

/// A typed v0 caprun intent — the user's declared action.
/// Small, closed enum; every variant maps deterministically to exactly one PlanNode.
/// LLM-driven intent is v2 (PLAN-F1) — not implemented here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CaprunIntent {
    /// Send an email summary to a known, user-trusted recipient.
    /// `recipient` is a user-provided literal that will be minted as UserTrusted.
    SendEmailSummary { recipient: String },
}
```

```rust
// Source: cli/caprun/src/planner.rs (NEW file)

use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};

/// Map a typed CaprunIntent to a single PlanNode.
///
/// The planner holds ONLY opaque ValueId handles — never the literal or taint.
/// Taint lives in the broker-owned ValueStore; the planner is not aware of it.
///
/// # Arguments
/// * `intent`         — the typed user intent.
/// * `intent_value_id` — the UserTrusted ValueId minted by mint_from_intent
///                       (handle for the user-provided literal, e.g. recipient).
/// * `_file_value_ids` — tainted handles from mint_from_read (not used on the
///                       clean allow-path; available for future mixed-path demos).
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
                value_id: intent_value_id,   // UserTrusted → no block
            }],
        },
    }
}
```

### Pattern 4: New IPC Messages

**What:** `BrokerRequest::ProvideIntent` + `BrokerResponse::IntentAccepted` added to `brokerd::proto`.

**When to use:** Worker sends `ProvideIntent` immediately after self-confinement, before `RequestFd`. The broker mints the intent value and returns the handle.

**Example:**
```rust
// Source: crates/brokerd/src/proto.rs (ADD to existing enums)

// In BrokerRequest:
/// Worker declares the user's typed intent. Broker calls mint_from_intent and
/// returns an opaque UserTrusted ValueId handle. Sent BEFORE RequestFd.
ProvideIntent { intent: runtime_core::intent::CaprunIntent },

// In BrokerResponse:
/// Acknowledgement for ProvideIntent: opaque ValueId handle for the minted
/// UserTrusted ValueRecord (literal = intent field; taint = [UserTrusted]).
IntentAccepted { value_id: runtime_core::plan_node::ValueId },
```

```rust
// Source: crates/brokerd/src/server.rs (ADD to dispatch_request match)

BrokerRequest::ProvideIntent { intent } => {
    let literal = match &intent {
        CaprunIntent::SendEmailSummary { recipient } => recipient.clone(),
    };
    let (intent_event_id, intent_hash, value_id) = {
        let locked = conn.lock()...;
        mint_from_intent(&locked, value_store, session_id, literal, Some(last_event_hash))?
    };
    *last_event_id = intent_event_id;
    *last_event_hash = intent_hash;
    send_response(stream, &BrokerResponse::IntentAccepted { value_id }).await?;
}
```

### Pattern 5: CLI Arg Parsing Change (PLAN-01)

**Current signature:**
```
caprun <workspace-file> [audit-db-path]
```

**Phase 6 signature:**
```
caprun <intent-kind>:<intent-param> <workspace-file> [audit-db-path]
```

Or more ergonomically (simpler to parse, avoids shell quoting issues):
```
caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]
```

**Example parse in main.rs:**
```rust
// Source: cli/caprun/src/main.rs (UPDATE existing arg parsing)
let mut args = std::env::args().skip(1);
let intent_kind = args.next().ok_or_else(|| ...)?;  // e.g. "send-email-summary"
let intent_param = args.next().ok_or_else(|| ...)?; // e.g. "boss@company.com"
let workspace_path = args.next().ok_or_else(|| ...)?;
let audit_path = args.next().unwrap_or_else(|| ":memory:".to_string());

let intent = match intent_kind.as_str() {
    "send-email-summary" => CaprunIntent::SendEmailSummary { recipient: intent_param },
    _ => anyhow::bail!("unknown intent kind: {intent_kind}"),
};

// Pass intent to worker via env var (serialized JSON)
.env("INTENT", serde_json::to_string(&intent)?)
```

### Pattern 6: Worker's Clean Allow-Path (Updated worker.rs)

**The critical ordering:**
```
1. Connect to broker UDS
2. set_nonblocking(false)
3. sandbox::apply_confinement()          [self-confine AFTER connect]
4. send ProvideIntent { intent }         [NEW: before RequestFd]
5. recv IntentAccepted { intent_value_id }  [NEW]
6. send RequestFd { path }
7. recv_fd (file fd via SCM_RIGHTS)
8. recv FdGranted
9. Read file via passed fd
10. Extract claims → send ReportClaims
11. recv ClaimsReceived { file_value_ids }
12. Planner: plan_from_intent(&intent, intent_value_id, &file_value_ids) → plan_node
13. send SubmitPlanNode { plan_node }
14. recv PlanNodeDecision { decision }
15. exit 0 (Allowed) or exit 1 (Blocked)
```

**Security note:** The confined worker sends `ProvideIntent` with the intent value it received from the trusted orchestrator (caprun main) via environment variable. The broker trusts it because the intent comes through the broker's own IPC channel — the worker does not construct the ValueRecord, only the literal string from the env var flows to the broker which mints it authoritatively. The planner still holds only the opaque ValueId handle returned by the broker.

### Anti-Patterns to Avoid

- **Anti-pattern: planner constructs a ValueRecord directly.** The planner must NEVER call `ValueStore::mint` or construct a `ValueRecord`. It must only hold opaque `ValueId` handles. The handle model is the spine of I2 soundness (DESIGN-plan-executor.md).
- **Anti-pattern: mint_from_intent with `taint: []` (empty vec).** Using an empty taint vec would satisfy the CURRENT predicate accidentally but would make HARD-02 a no-op. Use `[TaintLabel::UserTrusted]` to make positive provenance explicit and the predicate fix meaningful.
- **Anti-pattern: new `EffectRequest { effect, args: Map }` path.** `check-invariants.sh` Gate 1 fails if `EffectRequest` appears under `crates/`. Ths token must never appear.
- **Anti-pattern: intent received as free-form text crossing the IPC boundary.** The intent must be a typed enum, not a raw string. `PLAN-03` requires the planner never to see raw bytes.
- **Anti-pattern: calling `mint_from_intent` from main before per-connection ValueStore is created.** The per-connection `ValueStore` is created inside `handle_connection`; minting before that connection is established puts the value in a different store that the executor never sees. Must go through the `ProvideIntent` IPC round-trip.
- **Anti-pattern: stapling provenance.** `mint_from_intent` must append the `intent_received` audit event AND mint the ValueRecord in the same call, so `provenance_chain[0]` equals the event that was just appended. Never fabricate an event ID.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Framed IPC messaging | Custom length-prefix framing | Existing `send_framed` / `recv_framed` in worker.rs | Already implemented and tested (Phase 5) |
| Audit event append | Custom SQL | `brokerd::audit::append_event` | Hash chain logic is already correct; re-implementing risks breaking the verify_chain invariant |
| ValueRecord storage | Custom HashMap | `executor::value_store::ValueStore::mint` + `::resolve` | The ValueStore is the anti-stapling invariant boundary; replacing it breaks the guarantee |
| Intent serialization | Custom format | `serde_json` (already a dep) + `#[derive(Serialize, Deserialize)]` on `CaprunIntent` | Consistent with the existing IPC framing used for all other messages |
| Audit DAG querying | Raw SQL in tests | `brokerd::audit::find_event_by_type` | Already implemented; returns Event with taint intact |

**Key insight:** This phase adds NO new capability surface. Every building block (UDS framing, ValueStore, audit DAG, executor decision logic, per-connection state) already exists. The work is: one new enum, three new functions, two new IPC variants, one predicate fix, and CLI arg parsing.

---

## Runtime State Inventory

> **SKIP** — not a rename/refactor/migration phase. Greenfield additions only.

---

## Common Pitfalls

### Pitfall 1: Minting Intent Value OUTSIDE the Per-Connection ValueStore

**What goes wrong:** `caprun main` calls `mint_from_intent` before spawning the broker task or before the per-connection `ValueStore` is created. The minted `ValueId` points into a different `ValueStore` that the broker's `handle_connection` never sees. When the worker later sends `SubmitPlanNode` with that `ValueId`, `value_store.resolve()` returns `None` → executor returns `Denied`.

**Why it happens:** The `ValueStore` is created INSIDE `handle_connection` (per-connection, HARD-03 isolation). Any minting done before or outside that scope is invisible to the executor.

**How to avoid:** Mint ONLY via the `ProvideIntent` IPC round-trip, so minting happens inside the per-connection handler after `handle_connection` creates its `ValueStore`.

**Warning signs:** Executor returns `Denied { reason: "unresolvable handle..." }` for the intent ValueId.

---

### Pitfall 2: Setting Taint to `[]` on `mint_from_intent` (HARD-02 becomes vacuous)

**What goes wrong:** `mint_from_intent` mints with `taint: vec![]`. The existing executor predicate `!record.taint.is_empty()` happens to allow it through already — HARD-02 is satisfied by accident, not by design. A future code change that mints `UserTrusted` as a positive label (e.g., for audit richness) would then BREAK the allow-path without anyone noticing the underlying issue.

**Why it happens:** The requirement says "UserTrusted/LocalWorkspace-only provenance does NOT block", which implies those labels ARE present, not that taint is empty.

**How to avoid:** Always mint intent values with `taint: vec![TaintLabel::UserTrusted]`. Update the executor predicate to `record.taint.iter().any(|t| t.is_untrusted())`. Both changes together make HARD-02 explicit and future-proof.

**Warning signs:** Test `executor::HARD02_usertrusted_only_allows` passes even without the predicate fix (because taint is empty).

---

### Pitfall 3: Forgetting `parent_hash` Chaining in `mint_from_intent`

**What goes wrong:** `mint_from_intent` appends the `intent_received` event with `parent_hash = None` instead of threading the causal chain. The `intent_received` event becomes a dangling root event disconnected from the session chain. `verify_chain` may still pass (it follows parent_id links, not strict linear ordering) but the causal audit trail is incomplete.

**Why it happens:** `mint_from_read` already has the same issue (per the Phase 5 test comment: "PROHIBITION (Phase 5): this test does NOT assert `verify_chain` over the full hostile chain. `mint_from_read` sets `file_read.parent_id = None`"). Phase 7 wires this linkage. Follow the same pattern for Phase 6: pass `parent_hash` correctly in the IPC handler but note that `event.parent_id` wiring is a Phase 7 concern.

**How to avoid:** In `server.rs` `ProvideIntent` arm, call `mint_from_intent(..., Some(last_event_hash))` and advance `*last_event_hash`. The `parent_hash` column in the DB is threaded correctly; the `event.parent_id` field on the in-memory Event struct is a separate concern deferred to Phase 7.

---

### Pitfall 4: CaprunIntent Serialization Mismatch Between Main and Worker

**What goes wrong:** `caprun main` serializes `CaprunIntent` to a JSON env var string; the worker deserializes it. If the enum tag format differs (e.g., internal vs. external tagging), deserialization fails and the worker exits with an error before sending `ProvideIntent`.

**Why it happens:** `serde_json` defaults differ by enum type (unit variants serialize as strings; struct variants serialize as objects). If the main doesn't use the same derive macros as the intent enum, the wire format diverges.

**How to avoid:** Add `#[serde(tag = "kind")]` or `#[serde(rename_all = "snake_case")]` consistently to `CaprunIntent`. Write a unit test that round-trips each variant through `serde_json::to_string` + `serde_json::from_str`. Keep the test in `crates/runtime-core/tests/`.

---

### Pitfall 5: `is_untrusted()` Missing a TaintLabel Variant

**What goes wrong:** A new `TaintLabel` variant is added to runtime-core but not added to the `is_untrusted()` match arm. Rust's exhaustive match would catch this IF `is_untrusted()` uses `match self { ... }` with a wildcard — but a `matches!()` macro (which has implicit `_ => false`) would silently fail closed (untrusted new label treated as trusted = allow it through). For security, a false-allow is worse than false-block.

**How to avoid:** Implement `is_untrusted()` using an EXPLICIT `match self` with NO wildcard arm, so adding a new variant forces a compile error:
```rust
pub fn is_untrusted(&self) -> bool {
    match self {
        TaintLabel::ExternalUntrusted | TaintLabel::EmailRaw | TaintLabel::PdfRaw
            | TaintLabel::LlmGenerated | TaintLabel::WorkerExtracted => true,
        TaintLabel::UserTrusted | TaintLabel::LocalWorkspace => false,
        // Compile error if a new variant is added — prevents silent false-allows.
    }
}
```

---

### Pitfall 6: Worker Sends `ProvideIntent` BEFORE `apply_confinement()`

**What goes wrong:** The intent IPC message is sent before self-confinement. While not a direct security flaw (the broker still mints authoritatively), the ordering violates the self-confinement invariant documented in worker.rs. Also: `ProvideIntent` MUST be sent after connecting and after `set_nonblocking(false)` but before `RequestFd`.

**How to avoid:** Keep the ordering: connect → set_nonblocking → apply_confinement → ProvideIntent → RequestFd → ... (matching the existing comment block in worker.rs).

---

## Code Examples

### Existing `mint_from_read` (the template to mirror)

```rust
// Source: crates/brokerd/src/quarantine.rs (Phase 5 — VERIFIED by Read tool)
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId)> {
    let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw];
    let event_id = Uuid::new_v4();
    let event = Event {
        id: event_id,
        parent_id: None,
        session_id,
        actor: "confined-reader".into(),
        event_type: "file_read".into(),
        timestamp: Utc::now(),
        taint: taint.clone(),
    };
    let read_hash = append_event(conn, &event, parent_hash)?;
    let value_id = store.mint(claim.value.clone(), taint, vec![event_id]);
    Ok((event_id, read_hash, value_id))
}
```

### Existing Executor Decision Logic (the predicate to update)

```rust
// Source: crates/executor/src/lib.rs (Phase 5 — VERIFIED by Read tool)
// Current predicate (blocks on ANY non-empty taint):
if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
    && !record.taint.is_empty()           // ← HARD-02: change this
{
    return ExecutorDecision::BlockedPendingConfirmation { ... };
}
// After Phase 6:
if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
    && record.taint.iter().any(|t| t.is_untrusted())   // explicit untrusted check
{
```

### Existing TaintLabel (add `is_untrusted()`)

```rust
// Source: crates/runtime-core/src/plan_node.rs (Phase 5 — VERIFIED by Read tool)
pub enum TaintLabel {
    UserTrusted,        // trusted provenance — must NOT block
    LocalWorkspace,     // trusted provenance — must NOT block
    ExternalUntrusted,  // untrusted — MUST block in routing-sensitive arg
    EmailRaw,           // untrusted — MUST block
    PdfRaw,             // untrusted — MUST block
    LlmGenerated,       // untrusted — MUST block
    WorkerExtracted,    // untrusted — MUST block
}
```

### Existing per-connection IPC dispatch (the arm to add to)

```rust
// Source: crates/brokerd/src/server.rs dispatch_request() (Phase 5 — VERIFIED by Read tool)
// Structure to ADD to the existing match:
match request {
    BrokerRequest::ProvideIntent { intent } => { /* NEW in Phase 6 */ }
    BrokerRequest::RequestFd { path } => { /* existing */ }
    BrokerRequest::ReportClaims { claims } => { /* existing */ }
    BrokerRequest::SubmitPlanNode { plan_node } => { /* existing */ }
    BrokerRequest::CreateSession { .. } => { /* existing */ }
    BrokerRequest::ReportRead { .. } => { /* existing (deprecated)  */ }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `!record.taint.is_empty()` (any taint = block) | `record.taint.iter().any(|t| t.is_untrusted())` (explicit label check) | Phase 6 | Allows UserTrusted-only records; makes HARD-02 explicit |
| No typed intent input (just workspace file) | `CaprunIntent` enum parsed from CLI | Phase 6 | Enables the clean allow-path demo end-to-end |
| No mint_from_intent | `mint_from_intent` in `brokerd::quarantine` | Phase 6 | Completes the two-sided minting model (hostile path + clean path) |

**Deprecated/outdated:**
- `BrokerRequest::ReportRead`: already deprecated in Phase 5 server.rs (returns Error, keep for wire compat).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `mint_from_intent` should mint with `taint: vec![TaintLabel::UserTrusted]` rather than `vec![]` | Pattern 1 | If empty vec is correct, the HARD-02 predicate change is still right but less meaningful; no security regression either way |
| A2 | `CaprunIntent` enum belongs in `runtime-core/src/intent.rs` alongside the existing `Intent` struct | Architecture Patterns | If kept in brokerd or cli/caprun, import paths change; no functional difference |
| A3 | `plan_from_intent` (planner function) lives in `cli/caprun/src/planner.rs` | Project Structure | Could live in `brokerd` as a public function; either works since the planner is just a pure mapping function |
| A4 | CLI interface uses two positional args: `<intent-kind> <intent-param>` before `<workspace-file>` | Pattern 5 | Could use flags (--intent, --recipient) instead; both satisfy PLAN-01 |
| A5 | The `ProvideIntent` → `IntentAccepted` IPC round-trip is the right architecture for minting intent values | Anti-Patterns + Pattern 4 | See Pitfall 1 for why the alternative (pre-seeding from main) doesn't work with per-connection ValueStore |

**If this table is reviewed:** A1 and A5 are the load-bearing design choices. All others are implementation detail decisions the planner can make without user confirmation.

---

## Open Questions

1. **Scope of `CaprunIntent` for Phase 6**
   - What we know: Only one variant is needed to prove the clean allow-path: `SendEmailSummary { recipient: String }`. It targets the existing `email.send` stub.
   - What's unclear: Should a second variant (e.g., `CreateFile { path: String, contents: String }`) be stubbed out now for Phase 7's benefit, or strictly minimal?
   - Recommendation: One variant only (`SendEmailSummary`). YAGNI — adding stubs for Phase 7 risks scope creep and the Phase 7 research will determine the right variant shape for `file.create`.

2. **Phase 7 linkage: `file_read.parent_id` chain**
   - What we know: Phase 5 note says "mint_from_read sets `file_read.parent_id = None`" and the Phase 5 live test explicitly defers `verify_chain` assertion over the full chain. Phase 7 SC7 wires this.
   - What's unclear: Should Phase 6 fix `parent_id` for `intent_received` events or defer to Phase 7?
   - Recommendation: Defer `event.parent_id` wiring to Phase 7 (consistent with Phase 5 pattern). Thread `parent_hash` correctly for the DB chain (already done by the server.rs causal threading), but leave `event.parent_id = None` in the Event struct for now.

---

## Environment Availability

> Step 2.6: All dependencies are workspace-internal. No external tools required beyond what Phase 5 already used.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo build --workspace` | All phase code | Yes (macOS dev) | Rust 1.x (existing) | — |
| Colima + Docker | Linux e2e tests | Yes (Phase 5 verified 29/29) | Existing | macOS cfg-excluded stubs |

---

## Validation Architecture

> `workflow.nyquist_validation` is absent from `.planning/config.json` → treat as enabled.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | `Cargo.toml` workspace members (no separate test config) |
| Quick run command | `cargo test --workspace --no-fail-fast` (macOS, Linux no-op stubs pass) |
| Full suite command | `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File |
|--------|----------|-----------|-------------------|------|
| PLAN-01 | caprun parses `<intent-kind> <intent-param> <workspace-file>` | unit | `cargo test -p caprun --test e2e` | ❌ Wave 0 |
| PLAN-02 | `plan_from_intent(SendEmailSummary, intent_vid, []) → PlanNode{email.send, to: intent_vid}` | unit | `cargo test -p caprun` (or new `tests/planner.rs`) | ❌ Wave 0 |
| PLAN-03 | Planner function receives only `CaprunIntent` + `ValueId` handles; compile-time check | compile | `cargo build --workspace` | compile-time |
| PLAN-04 | `mint_from_intent` mints UserTrusted record + appends `intent_received` event; `provenance_chain[0] == event_id` | unit | `cargo test -p brokerd quarantine` | ❌ Wave 0 |
| PLAN-04 | `ProvideIntent` IPC dispatch: broker receives intent, mints, returns ValueId | unit | `cargo test -p brokerd --test proto_claims` (extend) | ❌ Wave 0 |
| HARD-02 | `TaintLabel::UserTrusted.is_untrusted()` returns false | unit | `cargo test -p runtime-core` | ❌ Wave 0 |
| HARD-02 | `TaintLabel::ExternalUntrusted.is_untrusted()` returns true | unit | `cargo test -p runtime-core` | ❌ Wave 0 |
| HARD-02 | Executor: UserTrusted-only record in routing-sensitive arg → Allowed | unit | `cargo test -p executor --test executor_decision` | ❌ Wave 0 |
| HARD-02 | Executor: ExternalUntrusted record in routing-sensitive arg → still BlockedPendingConfirmation | unit | `cargo test -p executor --test executor_decision` (existing) | Extend existing |
| PLAN-04+HARD-02 | Clean allow-path in-process: mint_from_intent → PlanNode → Allowed | integration | `cargo test -p brokerd --test s9_acceptance` (extend) | ❌ Wave 0 |
| PLAN-01+PLAN-04+HARD-02 | Live caprun clean-path: caprun exits 0, `plan_node_evaluated` in DAG, `intent_received` in DAG | e2e (Linux-only `#[cfg]`) | `cargo test -p caprun --test s9_live_block` (extend) | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --workspace --no-fail-fast` (macOS — all Linux e2e stubs pass as 0-assertion no-ops, which is correct)
- **Per wave merge:** same
- **Phase gate:** Full suite green on Linux (Colima/Docker) before `/gsd-verify-work`

### Wave 0 Gaps (files to create before implementation)

- [ ] `crates/runtime-core/src/intent.rs` — ADD `CaprunIntent` enum + `TaintLabel::is_untrusted()` (extend existing file)
- [ ] `crates/brokerd/src/quarantine.rs` — ADD `mint_from_intent()` (extend existing file)
- [ ] `crates/brokerd/src/proto.rs` — ADD `ProvideIntent` + `IntentAccepted` (extend existing file)
- [ ] `crates/brokerd/src/server.rs` — ADD `ProvideIntent` dispatch arm (extend existing file)
- [ ] `crates/executor/src/lib.rs` — UPDATE predicate (edit existing file)
- [ ] `cli/caprun/src/planner.rs` — NEW file: `plan_from_intent()`
- [ ] `cli/caprun/src/main.rs` — UPDATE arg parsing (edit existing file)
- [ ] `cli/caprun/src/worker.rs` — ADD ProvideIntent send + intent_value_id use (edit existing file)
- [ ] `crates/executor/tests/executor_decision.rs` — ADD HARD-02 cases (extend existing file)
- [ ] `crates/brokerd/tests/quarantine.rs` (or extend `s9_acceptance.rs`) — ADD `mint_from_intent` anchor test
- [ ] `cli/caprun/tests/s9_live_block.rs` — ADD Linux-gated clean-path e2e test

---

## Security Domain

> `security_enforcement` is not set to false → required.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Session identity is connection-scoped (HARD-03, Phase 5) |
| V3 Session Management | No | Session lifecycle unchanged |
| V4 Access Control | Yes | Executor blocking predicate (HARD-02 is the control; `is_untrusted()` must be exhaustive match) |
| V5 Input Validation | Yes | `CaprunIntent` deserialization from env var (worker side); fail-closed on unknown variants |
| V6 Cryptography | No | SHA-256 hash chain is Phase 5; no changes to audit DAG hashing |

### Known Threat Patterns for This Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Worker claims a non-existent intent value_id is clean | Elevation of Privilege | Per-connection ValueStore; unknown ValueId → `Denied` (existing, unmodified) |
| `is_untrusted()` wildcards a new untrusted label as trusted | Elevation of Privilege | Exhaustive `match self` with no wildcard arm (see Pitfall 5) |
| Forged CaprunIntent JSON in INTENT env var | Tampering | Worker deserializes from env var; fail on unknown variants (serde exhaustive enum); broker mints authoritatively and the broker is the trust boundary |
| mint_from_intent called with same literal as hostile file content to launder taint | Tampering | Minting happens in broker; the literal and taint come from separate mint calls; the executor resolves each ValueId independently; a UserTrusted record for the same string as a hostile record is separate and legitimate (humans choose their own recipients) |

**I0 Note:** The `CaprunIntent` is provided by the USER on the CLI, not by an agent reading untrusted content. Therefore I0 (tainted-seed rule) does NOT apply to Phase 6 intent input — the session is not seeded from tainted content; the user directly provides the recipient. I0 becomes relevant in the full LLM-planner scenario (v2 / PLAN-F1).

---

## Sources

### Primary (HIGH confidence — VERIFIED by Read tool against codebase)

- `cli/caprun/src/main.rs` — current CLI arg parsing; how broker server is spawned; session/event flow
- `cli/caprun/src/worker.rs` — existing self-confinement order; existing scripted planner; IPC protocol usage
- `crates/brokerd/src/server.rs` — per-connection ValueStore; dispatch_request arms; causal chain threading
- `crates/brokerd/src/quarantine.rs` — `mint_from_read` exact signature and implementation; taint mint pattern
- `crates/executor/src/lib.rs` — current blocking predicate (`!record.taint.is_empty()`)
- `crates/executor/src/value_store.rs` — ValueStore::mint / resolve; anti-stapling invariant
- `crates/runtime-core/src/plan_node.rs` — TaintLabel enum (all 7 variants); ValueId; PlanNode; PlanArg
- `crates/runtime-core/src/value_record.rs` — ValueRecord fields; provenance_chain structure
- `crates/brokerd/src/proto.rs` — BrokerRequest / BrokerResponse wire types; WorkerClaim enum
- `crates/brokerd/src/audit.rs` — append_event; find_event_by_type; Event schema
- `crates/executor/tests/executor_decision.rs` — existing executor test patterns to extend
- `cli/caprun/tests/s9_live_block.rs` — Linux-gated e2e test pattern to follow
- `crates/brokerd/tests/s9_acceptance.rs` — in-process acceptance test pattern to follow
- `planning-docs/DESIGN-plan-executor.md` — ValueRecord model; PlanArg handle model; executor decision rule
- `planning-docs/DESIGN-taint-model.md` — UserTrusted semantics; taint monotonicity; I0/I2 invariants
- `.planning/REQUIREMENTS.md` — Phase 6 requirements (PLAN-01 through PLAN-04, HARD-02); traceability table
- `.planning/ROADMAP.md` — Phase 6 success criteria; Phase 7 boundary

### Secondary (MEDIUM confidence — authoritative internal docs)

- `CLAUDE.md` — hard constraints (TCB is Rust; no LLM planner; EffectRequest token banned; terminology locked)
- `.planning/STATE.md` — accumulated decisions; Phase 5 completion confirmation

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new crates; all findings from existing workspace
- Architecture: HIGH — all patterns verified directly from Phase 5 source
- Pitfalls: HIGH — most are traps visible directly in the existing code (per-connection ValueStore, empty-taint vs UserTrusted, exhaustive match)
- Executor predicate fix: HIGH — predicate text read verbatim from executor/src/lib.rs

**Research date:** 2026-06-30
**Valid until:** 2026-08-30 (stable Rust internals; no fast-moving ecosystem)
