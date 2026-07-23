# Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance - Pattern Map

**Mapped:** 2026-06-30
**Files analyzed:** ~10 new artifacts (Stream 1 + Stream 2)
**Analogs found:** 10/10 (all grounded in read source)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `SinkBlockedAnchor` struct + `DenyReason` enum (`runtime-core`) | model | transform | `plan_node.rs` `TaintLabel`/`ValueNode`/`Provenance` structs | exact (same module, same derive contract) |
| `Event::new(...)` + `anchor: Option<SinkBlockedAnchor>` field | model | transform | `event.rs` `Event` struct + its ~13 literal-construction call sites | exact |
| `crates/brokerd/src/sinks/file_create.rs` | service | file-I/O | `crates/brokerd/src/sinks/email_send.rs` | role-match (shape to follow; note it's dead code) |
| `WorkerClaim::RelativePath(String)` + broker dispatch arm + mint | model/service | event-driven | `WorkerClaim::EmailAddress` + `extract_email_claims`/`mint_from_read` in `quarantine.rs` | exact |
| `adapter_fs::workspace::WorkspaceRoot` (+ `read_within`/`create_exclusive_within`) | utility | file-I/O | `crates/sandbox/src/landlock.rs` cfg-gated real/stub pattern | role-match (cfg-gate idiom), partial (different domain) |
| `crates/executor/src/sink_schema.rs` (`validate_schema`) | service | request-response | `crates/executor/src/sink_sensitivity.rs` (`is_routing_sensitive`) | exact |
| `crates/brokerd/tests/durable_anchor.rs` | test | event-driven | `crates/brokerd/tests/s9_acceptance.rs` | exact |
| `cli/caprun/tests/s9_file_create_live.rs` (or extend `s9_live_block.rs`) | test | event-driven | `cli/caprun/tests/s9_live_block.rs` (`run_caprun_intent_on`) | exact |
| `crates/brokerd/tests/hard06_crash_indeterminate.rs` | test | event-driven | `crates/brokerd/tests/phase5_dispatch.rs` | role-match |

## Pattern Assignments

### 1. `SinkBlockedAnchor` struct + `DenyReason` enum (`runtime-core`)

**Analog:** `crates/runtime-core/src/plan_node.rs:11-95` (`TaintLabel`, `Provenance`, `ValueNode`)

**Derive/doc convention to mirror** (`plan_node.rs:11-21`):
```rust
/// Labels indicating the trust/taint level of a value's provenance chain.
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
```
**Struct-with-provenance shape to mirror** (`plan_node.rs:70-95`, `Provenance`/`ValueNode`):
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    pub source_event_id: Option<uuid::Uuid>,
    pub source_artifact_id: Option<uuid::Uuid>,
    pub description: String,
    pub provenance_chain: Vec<uuid::Uuid>,
}
```
**Exhaustive-match discipline to mirror for `DenyReason` consumers** (`plan_node.rs:31-46`, `is_untrusted`): use an explicit `match` with no wildcard arm — this is the same doc-comment convention the executor module-doc requires ("Adding a new variant without updating this match is a compile error, not a silent false-allow").

**What to change:** Place `SinkBlockedAnchor` and `DenyReason` in `plan_node.rs` (or a sibling `runtime-core` module, per DESIGN §4) as new pub structs/enums with `Debug, Clone, PartialEq, Serialize, Deserialize` derives — identical derive list to every existing type in this file. `DenyReason` variants (`DanglingHandle`, `EmptyTaintInvariantViolation`, `MissingProvenanceAnchor`, later extended with `UnknownSink`/`MissingArg`/`DuplicateArg`/`UnknownArg`) should follow the same fieldless-variant-first style as `TaintLabel`.

---

### 2. `Event::new(...)` + `anchor: Option<SinkBlockedAnchor>` field

**Analog:** `crates/runtime-core/src/event.rs:16-29` (the struct itself) + call sites at `crates/brokerd/src/quarantine.rs:143`, `:214`, `crates/brokerd/src/server.rs:209`, `:256`, `:334`, `crates/brokerd/src/sinks/email_send.rs` (`Event { ... }` literal), plus test literals in `audit.rs:283`, `s9_acceptance.rs:295`, `audit_dag.rs:21/31/41/98`.

**Current struct** (`event.rs:16-29`):
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub session_id: Uuid,
    pub actor: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub taint: Vec<crate::plan_node::TaintLabel>,
}
```
**Representative literal-construction site to migrate** (`crates/brokerd/src/quarantine.rs:143-151`, inside `mint_from_read`):
```rust
let event = Event {
    id: event_id,
    parent_id: None,
    session_id,
    actor: "confined-reader".into(),
    event_type: "file_read".into(),
    timestamp: Utc::now(),
    taint: taint.clone(),
};
```
**What to change:** Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub anchor: Option<SinkBlockedAnchor>` to the struct (per DESIGN §5 — rides in `payload`, no DDL). Add `Event::new(id, parent_id, session_id, actor, event_type, timestamp, taint) -> Self` (sets `anchor: None`) as an `impl Event` block in `event.rs`, mirroring the field order of the struct. Migrate all ~13 struct-literal sites (`quarantine.rs` x2, `server.rs` x3, `sinks/email_send.rs` x1, plus test files) to call `Event::new(...)` instead of the literal — **except** the one broker-owned anchor-setting constructor for the `sink_blocked` path (per DESIGN §6), which sets `anchor: Some(...)` directly. Add the golden byte-fixture test proving existing (non-anchor) events serialize byte-identical before/after — model it on any existing `#[cfg(test)] mod tests` block in `audit.rs`.

---

### 3. `file.create` sink (arg schema + dispatch + `openat2` write)

**Analog — arg schema & sensitivity:** `crates/executor/src/sink_sensitivity.rs:1-45`
```rust
/// Args of email.send that determine WHERE the effect is delivered.
pub const EMAIL_SEND_ROUTING_SENSITIVE: &[&str] = &["to", "cc", "bcc"];
pub const EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body", "attachment"];

pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
```
**What to change:** Add `"file.create" => &["path"]` as a routing-sensitive arm (SINK-02) and create the sibling `sink_schema.rs` (per RESEARCH Q5) with `KNOWN_SINKS`, `FILE_CREATE_ARGS = &["path", "contents"]`, and `validate_schema(sink, args) -> Result<(), DenyReason>` returning `DenyReason::UnknownSink`/`UnknownArg`/`DuplicateArg`/(missing-arg) — called as literal first statement of `submit_plan_node` (`crates/executor/src/lib.rs:40`, before the existing `for arg in &plan_node.args` loop at line 41).

**Analog — dispatch/invocation shape (dead-code stub, follow shape not wiring):** `crates/brokerd/src/sinks/email_send.rs` (full file read):
```rust
pub fn invoke_email_send_stub(
    conn: &rusqlite::Connection,
    session_id: Uuid,
    plan_node: &PlanNode,
    parent_hash: Option<&str>,
) -> Result<String> {
    let _ = plan_node; // Opaque handles only — not embedded in the event payload.
    let event = Event { id: Uuid::new_v4(), parent_id: None, session_id,
        actor: "sink-stub:email.send".to_string(), event_type: "email_send_stub".to_string(),
        timestamp: Utc::now(), taint: vec![] };
    audit::append_event(conn, &event, parent_hash)
}
```
**What to change:** `sinks/file_create.rs::invoke_file_create(workspace_root, conn, session_id, plan_node, value_store, effect_id, parent_hash)` follows this exact shape but (a) actually performs the `openat2` write via `WorkspaceRoot::create_exclusive_within`, (b) resolves `path`/`contents` literals from `value_store` (unlike the stub, which discards `plan_node`), (c) appends `sink_executed`/`sink_execution_failed` — **and unlike the stub, IS wired into `dispatch_request`'s `SubmitPlanNode` arm** on `Allowed` (Pitfall 1 — the stub is never called live; Phase 7's is the first live sink invocation).

**Dispatch call-site to extend** (`crates/brokerd/src/server.rs:319-355`, `SubmitPlanNode` arm) — `event_type` selection and fail-closed append pattern to mirror verbatim:
```rust
let decision = executor::submit_plan_node(session_id, &plan_node, value_store);
let event_type = match &decision {
    runtime_core::ExecutorDecision::BlockedPendingConfirmation { .. } => "sink_blocked",
    _ => "plan_node_evaluated",
};
let audit_event = Event { id: Uuid::new_v4(), parent_id: Some(*last_event_id), session_id,
    actor: "executor".into(), event_type: event_type.into(), timestamp: Utc::now(), taint: vec![] };
let new_hash = { let locked = conn.lock()...; append_event(&locked, &audit_event, Some(last_event_hash))
    .map_err(|e| { eprintln!("[brokerd] {event_type} audit append FAILED (fail-closed): {e}"); anyhow::anyhow!(...) })? };
*last_event_id = audit_event.id;
*last_event_hash = new_hash;
send_response(stream, &BrokerResponse::PlanNodeDecision { decision }).await?;
```
Insert the invocation + result-audit-append (stages 5-6, RESEARCH Q5 table) **between** the authorization append and `send_response`, only when `decision == Allowed && plan_node.sink.0 == "file.create"`.

---

### 4. `RelativePath` claim variant

**Analog — wire type:** `crates/brokerd/src/proto.rs:16-24` (`WorkerClaim` enum, extension point already commented):
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WorkerClaim {
    EmailAddress(String),
    // RelativePath(String),  // Phase 7
}
```
**Analog — extraction:** `crates/brokerd/src/quarantine.rs` `extract_email_claims` (full function read, ~lines 47-99) — hand-rolled, deterministic, whitespace-split scanner returning `Claim { claim_type, value }`.

**Analog — dispatch arm (exhaustive match, fail-closed by compile error):** `crates/brokerd/src/server.rs:296-317` (`ReportClaims` arm):
```rust
for claim in claims {
    match claim {
        WorkerClaim::EmailAddress(addr) => {
            let quarantine_claim = Claim { claim_type: "email_address".into(), value: addr };
            let (read_event_id, read_hash, value_id) = {
                let locked = conn.lock()...;
                mint_from_read(&locked, value_store, session_id, &quarantine_claim, Some(last_event_hash))?
            };
            *last_event_id = read_event_id;
            *last_event_hash = read_hash;
            value_ids.push(value_id);
        } // Exhaustive enum: any future variant fails closed at deserialize.
    }
}
```
**Analog — mint site (sole taint-mint):** `crates/brokerd/src/quarantine.rs:129-164` (`mint_from_read`, full function already excerpted above in RESEARCH — reuse verbatim, only the taint vector and `claim_type` differ):
```rust
let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw];
...
let value_id = store.mint(claim.value.clone(), taint, vec![event_id]);
```
**What to change:** Uncomment `RelativePath(String)` in `proto.rs`. Add `extract_relative_path_claims(raw: &str) -> Vec<Claim>` in `quarantine.rs` next to `extract_email_claims`. Add a `WorkerClaim::RelativePath(path) => { ... }` arm in `server.rs`'s `ReportClaims` match calling `mint_from_read` with `taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::PathRaw]` (new label — add to `plan_node.rs:13-21` enum + the exhaustive `is_untrusted` match at `:38-45`). Per CONTEXT.md caveat: **never** `LocalWorkspace`.

---

### 5. Workspace-root capability / dirfd (`adapter-fs::workspace::WorkspaceRoot`)

**Analog — cfg-gated real/stub idiom:** `crates/sandbox/src/landlock.rs:16-38`:
```rust
#[cfg(target_os = "linux")]
pub fn deny_all_filesystem() -> std::io::Result<()> { /* real landlock ruleset */ }

#[cfg(not(target_os = "linux"))]
pub fn deny_all_filesystem() -> std::io::Result<()> { Ok(()) }
```
**Analog — current unguarded fs-open to replace (the HARD-04 gap):** `crates/brokerd/src/server.rs:248-251` (`RequestFd` arm):
```rust
BrokerRequest::RequestFd { path } => {
    let file = std::fs::File::open(&path)
        .with_context(|| format!("broker: open {path}"))?;
```
**Analog — SCM_RIGHTS fd-passing (different concern, do not conflate):** `adapter_fs::pass_fd` (used at `server.rs` inside the `RequestFd` arm via `spawn_blocking`) — the dirfd itself never crosses this boundary; only fds for worker-performed reads do.

**What to change:** New `crates/adapter-fs/src/workspace.rs`: `pub struct WorkspaceRoot(OwnedFd)` opened once via plain `nix::fcntl::open(root, O_DIRECTORY|O_RDONLY, Mode::empty())` in `main.rs`. `read_within`/`create_exclusive_within` follow the landlock cfg-gate pattern exactly — real `#[cfg(target_os = "linux")]` impl using `nix::fcntl::openat2` with `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`, `#[cfg(not(target_os = "linux"))]` fallback doing a plain join+open (no security claim, matches CLAUDE.md). Thread `Arc<WorkspaceRoot>` through `run_broker_server` → `handle_connection` → `dispatch_request` exactly like the existing `conn: Arc<Mutex<Connection>>` parameter is threaded (`dispatch_request` signature, `server.rs:194-201`).

---

### 6. New tests (`durable_anchor.rs`, live e2e, golden fixture)

**Analog — in-process §9-style DB-alone acceptance:** `crates/brokerd/tests/s9_acceptance.rs` (structure: build session, drive `dispatch_request`-equivalent calls, assert against SQLite rows via `find_event_by_type`/`verify_chain`; `Event { ... }` literal at line 295 to migrate to `Event::new`).

**Analog — dispatch-ordering test file to extend:** `crates/brokerd/tests/phase5_dispatch.rs` — contains the assertion to **delete** at line ~190:
```rust
assert_eq!(blocked.parent_id, Some(read_event_id),
    "sink_blocked must be causally parented onto the prior (file_read) event");
```
(delete per CONTEXT.md — two-graph model; do not conflate causal DAG with value-lineage).

**Analog — live e2e harness:** `cli/caprun/tests/s9_live_block.rs` — `run_caprun_intent_on(intent_kind, intent_param, content, tag) -> (exit_success, audit_db_path)`, spawns real `caprun` binary via `env!("CARGO_BIN_EXE_caprun")`, writes a workspace file, returns DB path for post-exit assertions. Both `s9_live_block.rs` tests are `#[cfg(target_os = "linux")]`-gated except the cross-platform binary-presence check.

**What to change:** `durable_anchor.rs` (new, cross-platform, no Linux gate per CONTEXT.md — "All ACC-07 anchor work is cross-platform") — file-backed DB, drive hostile block through `dispatch_request`, **drop + reopen connection**, `verify_chain` first then trust anchor, assert tamper-evidence via raw `UPDATE` on `payload`. `s9_file_create_live.rs` (new, `#[cfg(target_os = "linux")]`-gated) reuses `run_caprun_intent_on`-style helper for both hostile-block and clean-allow paths through `file.create`.

## Shared Patterns

### Fail-closed durable audit append (applies to ALL new dispatch code)
**Source:** `crates/brokerd/src/server.rs:319-355` (excerpted above)
**Apply to:** `file_create.rs` invocation wiring, any new event append in `dispatch_request`.
Pattern: append BEFORE side-effect / BEFORE response; every append uses `?` propagation; `parent_id` is always `Some(*last_event_id)` inside `dispatch_request` (never `None` — reserve `None` for session-root events only, per `CreateSession`/`mint_from_intent` precedent).

### Exhaustive match, no wildcard (applies to `TaintLabel`, `WorkerClaim`, `DenyReason` consumers)
**Source:** `crates/runtime-core/src/plan_node.rs:31-46` (`is_untrusted`) + `crates/brokerd/src/server.rs:301-316` (`ReportClaims` match comment: "Exhaustive enum: any future variant fails closed at deserialize")
**Apply to:** any new match over `TaintLabel`, `WorkerClaim`, or the extended `DenyReason`.

### Sole taint-mint site (`mint_from_read`) — never construct a `ValueRecord` elsewhere
**Source:** `crates/brokerd/src/quarantine.rs:129-164`
**Apply to:** `RelativePath` claim mint arm (reuse the function; do not add a second mint path). Anti-stapling doc-comment at top of `quarantine.rs:1-19` states this invariant explicitly.

### cfg-gated real/Linux vs stub/other-OS
**Source:** `crates/sandbox/src/landlock.rs:16-38`
**Apply to:** `adapter_fs::workspace::WorkspaceRoot::{read_within,create_exclusive_within}`.

## No Analog Found

None — every Phase 7 artifact has a direct or role-match analog already in the codebase.

## Naming/Convention Notes

- **Module layout:** new sink logic lives in `crates/brokerd/src/sinks/<sink_name>.rs` (mirrors `sinks/email_send.rs`); new executor-side pure logic lives as a sibling module in `crates/executor/src/` (mirrors `sink_sensitivity.rs` next to `lib.rs`/`value_store.rs`); new fs-capability code lives in `crates/adapter-fs/src/workspace.rs` (sibling to existing `lib.rs`).
- **Derive convention:** every wire/model type in `runtime-core` and `proto.rs` derives exactly `Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize` — no `Eq`/`Hash` unless it keys a map (`ValueId` is the sole exception, for `HashMap` keys).
- **cfg-gate idiom:** `#[cfg(target_os = "linux")]` real impl paired with `#[cfg(not(target_os = "linux"))]` stub that returns `Ok(())`/functional-but-unhardened — never a compile error on macOS. Exactly two functions, same signature, same doc comment above both.
- **Error-type convention:** typed enums over `anyhow::Result`/`Context` at crate boundaries — `executor` returns `ExecutorDecision`/`DenyReason` (typed), `brokerd` uses `anyhow::Result` + `.context(...)`/`.with_context(...)` internally. Do not introduce a second parallel error type for schema violations — extend `DenyReason`.
- **Test module convention:** `#[cfg(test)] mod tests { use super::*; ... }` inline for unit tests (e.g., `sink_sensitivity.rs:48+`); separate files under `tests/` for integration/e2e (`crates/brokerd/tests/*.rs`, `cli/caprun/tests/*.rs`), Linux-gated at the `#[test]` fn level, not file level, except where the whole file is Linux-only.

## Metadata

**Analog search scope:** `crates/runtime-core/src`, `crates/brokerd/src` (+`tests`), `crates/executor/src`, `crates/adapter-fs/src`, `crates/sandbox/src`, `cli/caprun/src` (+`tests`)
**Files scanned:** ~15 read directly (event.rs, plan_node.rs, proto.rs, quarantine.rs, server.rs, lib.rs [executor], sink_sensitivity.rs, sinks/email_send.rs) + grep across all `Event {` literal sites
**Pattern extraction date:** 2026-06-30
</content>
