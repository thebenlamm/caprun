# Phase 14: Content-Sensitive Sink-Arg Blocking - Pattern Map

**Mapped:** 2026-07-07
**Files analyzed:** 15 (5 Wave 1 core, ~10 Wave 2 mechanical consumers)
**Analogs found:** 15 / 15 (this phase is a refactor of existing locked types — every "new" file is really a same-file extension, so the analog for each file is its own current content)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/runtime-core/src/executor_decision.rs` (`SinkBlockedAnchor`, `ExecutorDecision::BlockedPendingConfirmation`) | model (pure type) | transform (singular→plural refactor) | itself, current singular shape (lines 108-153) | exact — same file, shape change only |
| `crates/executor/src/lib.rs` (`submit_plan_node` per-arg loop) | service (pure decision fn) | transform / request-response | itself, current Step 2 early-return block (lines ~99-139) + Step 3 no-op (lines 141-142) | exact — this IS the analog to mirror (routing-block path → body-block path) |
| `crates/executor/src/sink_sensitivity.rs` (`EMAIL_SEND_CONTENT_SENSITIVE`) | config (hardcoded const) | — | itself, line 71 | exact |
| `crates/executor/src/sink_schema.rs` (`email.send` `allowed` set) | config (hardcoded const) | — | itself, `KNOWN_SINKS` entry | exact |
| `crates/runtime-core/src/event.rs` (`Event.anchor`, `Event::sink_blocked`) | model (pure type) | event-driven (audit DAG node) | itself, lines 1-100 (current `Option<SinkBlockedAnchor>` shape + golden-byte test) | exact |
| `crates/brokerd/src/server.rs:419-485` | controller/service (IPC handler) | event-driven (decision → audit event → pending confirmation) | itself, the `match &decision { BlockedPendingConfirmation { anchor, literal } => ... }` block | exact |
| `crates/brokerd/src/audit.rs:204` | service (fail-closed guard) | CRUD (INSERT-only) | itself, `event.anchor.is_none()` Defect-B guard | exact |
| `crates/brokerd/src/confirmation.rs::render_block_display` (lines ~260-300) | service (display/UX) | transform | itself, first-untrusted-arg selection logic | exact (behavior unchanged this phase — see below) |
| `crates/executor/tests/executor_decision.rs` (multiple sites) | test | — | itself, `tainted_body_and_attachment_allow_in_v0` (line 229) as the test to rewrite; existing routing-block tests (lines 75,107,221,300,401) as the pattern for new plural-block tests | exact |
| `crates/runtime-core/tests/task2_types.rs` (lines 70,93,111) | test | — | itself | exact |
| `crates/brokerd/tests/durable_anchor.rs`, `phase5_dispatch.rs`, `s9_acceptance.rs`, `cli/caprun/tests/confirm.rs`, `cli/caprun/src/worker.rs:204` | test / consumer | — | itself, each site destructures `{ anchor, literal }` singular | mechanical — same pattern applied N times |

## Pattern Assignments

### `crates/runtime-core/src/executor_decision.rs` (model, transform)

**Analog:** itself — current singular shape, to be made plural.

**Current struct/enum shape** (lines 108-153, read verbatim this session):
```rust
pub struct SinkBlockedAnchor {
    pub effect_id: uuid::Uuid,
    pub sink: crate::plan_node::SinkId,
    pub arg: String,
    pub value_id: crate::plan_node::ValueId,
    pub literal_sha256: String,
    pub taint: Vec<crate::plan_node::TaintLabel>,
    pub provenance_chain: Vec<uuid::Uuid>,
    pub read_event_id: uuid::Uuid,
}

pub enum ExecutorDecision {
    Allowed,
    BlockedPendingConfirmation { anchor: SinkBlockedAnchor, literal: String },
    Denied { reason: DenyReason },
    NotImplemented,
}
```

**Target shape (per DESIGN D-14, and per RESEARCH Open Question 1's recommendation — prefer a combined struct over parallel Vecs):**
```rust
pub struct BlockedArg {
    pub anchor: SinkBlockedAnchor,   // SinkBlockedAnchor itself stays PER-ELEMENT/unchanged internally
    pub literal: String,
}

pub enum ExecutorDecision {
    Allowed,
    BlockedPendingConfirmation { anchors: Vec<BlockedArg> },
    Denied { reason: DenyReason },
    NotImplemented,
}
```
Keep every doc comment's anti-stapling language (T-04-03) — copy the existing `SinkBlockedAnchor` doc block verbatim as the per-element contract; add one new doc note pointing to `DESIGN-confirm-binding.md` for Phase 16's `combined_digest` follow-on (per RESEARCH's explicit instruction).

---

### `crates/executor/src/lib.rs` (service, transform / request-response)

**Analog:** itself — Step 2 (routing-sensitive early-return) is the pattern the new content-sensitive check must be UNIFIED with, not appended alongside.

**Current Step 2 + Step 3 (lines ~99-142, read verbatim this session):**
```rust
if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
    && record.taint.iter().any(|t| t.is_untrusted())
{
    let literal_sha256 = { /* Sha256::new().update(record.literal.as_bytes()); hex::encode(...) */ };
    let read_event_id = record.provenance_chain[0];
    debug_assert_eq!(read_event_id, record.provenance_chain[0], "...");
    let anchor = SinkBlockedAnchor { effect_id, sink: plan_node.sink.clone(), arg: arg.name.clone(),
        value_id: arg.value_id.clone(), literal_sha256, taint: record.taint.clone(),
        provenance_chain: record.provenance_chain.clone(), read_event_id };
    return ExecutorDecision::BlockedPendingConfirmation { anchor, literal: record.literal.clone() };
}
// Step 3: Content-sensitive tainted args (subject/body/attachment) do NOT
// Block in v0 — Tier-4 verbatim review is deferred to the approval-hook plan.
```

**Target collect-loop (DESIGN's illustrative shape, adapted to this file's existing variable names — `effect_id`, `record`, `arg`):**
```rust
let mut blocked: Vec<BlockedArg> = Vec::new();
for arg in &plan_node.args {
    let record = /* Steps 1/1a/1b UNCHANGED — resolve/empty-taint/empty-provenance guards, same as today */;
    let sensitive = sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
        || sink_sensitivity::is_content_sensitive(&plan_node.sink, &arg.name);
    if sensitive && record.taint.iter().any(|t| t.is_untrusted()) {
        let literal_sha256 = { /* SAME Sha256/hex::encode pattern, lib.rs:112-116, reused verbatim */ };
        let read_event_id = record.provenance_chain[0];
        let anchor = SinkBlockedAnchor { effect_id, sink: plan_node.sink.clone(), arg: arg.name.clone(),
            value_id: arg.value_id.clone(), literal_sha256, taint: record.taint.clone(),
            provenance_chain: record.provenance_chain.clone(), read_event_id };
        blocked.push(BlockedArg { anchor, literal: record.literal.clone() });
    }
}
if !blocked.is_empty() {
    return ExecutorDecision::BlockedPendingConfirmation { anchors: blocked };
}
// Step 0.5 (Draft-only class deny) UNCHANGED IN POSITION — runs only here, D-15.
```
Steps 1/1a/1b (`value_store.resolve`, empty-taint guard, empty-provenance guard) are copied verbatim, unchanged — do not touch their `Denied` early-returns; only Step 2/Step 3 collapse into the loop body above. Step 0.5 and the module doc comment at lines 35-36 ("Content-sensitive tainted args do not Block in v0...") must be updated to describe the new behavior (RESEARCH "State of the Art" deprecation note).

---

### `crates/executor/src/sink_sensitivity.rs` / `sink_schema.rs` (config)

**Analog:** itself. Single-line const edits, must be done together (Pitfall 3):
```rust
// sink_sensitivity.rs:71
EMAIL_SEND_CONTENT_SENSITIVE: &["subject", "body"]   // was ["subject","body","attachment"]

// sink_schema.rs, email.send allowed set
&["to", "cc", "bcc", "subject", "body"]   // was [..., "attachment"]
```
`is_content_sensitive` itself (lines 93-98) is NOT touched — already correct, D-21.

---

### `crates/runtime-core/src/event.rs` (model, event-driven)

**Analog:** itself — current `Option<SinkBlockedAnchor>` field/constructor.

**Current shape (lines 8-90, read verbatim this session):**
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub anchor: Option<SinkBlockedAnchor>,

pub fn sink_blocked(id: Uuid, parent_id: Option<Uuid>, session_id: Uuid, timestamp: DateTime<Utc>, anchor: SinkBlockedAnchor) -> Self {
    Event { id, parent_id, session_id, actor: "executor".into(), event_type: "sink_blocked".into(),
        timestamp, taint: anchor.taint.clone(), anchor: Some(anchor) }
}
```

**Target shape** — mirror the SAME `skip_serializing_if` discipline for a Vec:
```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub anchors: Vec<SinkBlockedAnchor>,   // or Vec<BlockedArg> if literal needs to ride here too — check confirmation.rs usage first

pub fn sink_blocked(..., anchors: Vec<SinkBlockedAnchor>) -> Self {
    Event { ..., taint: anchors.iter().flat_map(|a| a.taint.clone()).collect(), anchors, .. }
    // taint field must now merge every blocked arg's taint (was single anchor.taint.clone())
}
```
Golden-byte test at `event.rs` `mod tests` (`anchor_none_event_serializes_byte_identical_and_round_trips`) MUST be re-verified against `anchors: vec![]` omission — reuse the exact same test, rename/re-assert for the new field name.

---

### `crates/brokerd/src/server.rs:419-485` (controller, event-driven)

**Analog:** itself — the `match &decision { BlockedPendingConfirmation { anchor, literal } => (Event::sink_blocked(...), Some(literal.clone())), _ => ... }` arm (lines ~419-441 verbatim, read this session) and the `resolved_args` snapshot loop (lines ~451-480, UNCHANGED per RESEARCH — already resolves every arg, not just the blocked one).

**Pattern to mirror:** destructure `{ anchors }` (plural) in place of `{ anchor, literal }`; `Event::sink_blocked(..., anchors.iter().map(|b| b.anchor.clone()).collect())`; `PendingConfirmation.effect_id = anchors[0].anchor.effect_id` (every element shares one `effect_id` per RESEARCH's note). The `resolved_args` build loop (iterating `plan_node.args`, calling `value_store.resolve`, pushing `ResolvedArg`) is untouched — it already covers the full arg set.

---

### `crates/brokerd/src/audit.rs:204` (service, fail-closed guard)

**Analog:** itself.
```rust
// current (verbatim, read this session):
if event.event_type == "sink_blocked" && event.anchor.is_none() {
    return Err(anyhow::anyhow!("sink_blocked event requires an anchor (Defect B guard)"));
}
// target:
if event.event_type == "sink_blocked" && event.anchors.is_empty() {
    return Err(anyhow::anyhow!("sink_blocked event requires at least one anchor (Defect B guard)"));
}
```

---

### `crates/brokerd/src/confirmation.rs::render_block_display` (service, transform)

**Analog:** itself — the "find first untrusted arg, else `.first()`" selection (lines ~260-300, verbatim, read this session):
```rust
let display_arg = pc.resolved_args.iter()
    .find(|a| a.taint.iter().any(TaintLabel::is_untrusted))
    .or_else(|| pc.resolved_args.first());
```
**Phase 14 minimum bar (per RESEARCH A2):** leave this function's single-arg display path FUNCTIONALLY UNCHANGED for the single-blocked-arg case (it still operates on `pc.resolved_args`, unaffected by the anchor-type refactor). Do NOT implement multi-arg narration (Phase 16/CONFIRM-04). If a genuinely-plural block is detected before rendering, prefer a fail-closed assert/panic over silent truncation — this is a NEW safeguard this phase adds, not a rewrite of the existing selection logic.

---

## Shared Patterns

### Anti-stapling digest pattern (reuse verbatim per blocked-arg element)
**Source:** `crates/executor/src/lib.rs:112-116`
```rust
let literal_sha256 = {
    let mut hasher = Sha256::new();
    hasher.update(record.literal.as_bytes());
    hex::encode(hasher.finalize())
};
```
**Apply to:** every element pushed into the new `Vec<BlockedArg>` in `submit_plan_node` — do not invent a new hash scheme (Phase 16 layers `combined_digest` on top).

### `skip_serializing_if` empty-collection discipline (golden-byte preservation)
**Source:** `crates/runtime-core/src/event.rs` `anchor: Option<SinkBlockedAnchor>` + `#[serde(default, skip_serializing_if = "Option::is_none")]`
**Apply to:** the new `anchors: Vec<SinkBlockedAnchor>` field — use `skip_serializing_if = "Vec::is_empty"` so non-block events serialize byte-identically (empty vec omitted, matching today's `None` omitted).

### Exhaustive `match` over `SessionStatus` (no wildcard arm) — pattern to preserve, not touch
**Source:** `crates/executor/src/lib.rs` Step 0.5 block (`match *session_status { Draft => ..., Active => ..., WaitingApproval | Done | Failed | RolledBack => ... }`)
**Apply to:** nothing new this phase — cited only as the ordering anchor: the collect-loop MUST fully complete BEFORE this match runs (D-15); do not reorder.

### Fail-closed `Denied` typed taxonomy (no free-form String)
**Source:** `crates/runtime-core/src/executor_decision.rs` `DenyReason` enum + `.code()`/`Display` impls
**Apply to:** the `attachment` schema-rejection path — reuses the EXISTING `DenyReason::UnknownArg(String)` variant; no new variant needed (RESEARCH confirms this explicitly).

## No Analog Found

None — this phase is a pure refactor of existing, previously-reviewed types/files. Every file touched has itself as its own closest analog (no cross-codebase search was needed; RESEARCH.md already grep-verified the exhaustive consumer list).

## Metadata

**Analog search scope:** `crates/executor/src`, `crates/runtime-core/src`, `crates/brokerd/src`, plus their `tests/` dirs and `cli/caprun` consumers — all read directly (no Glob/Grep search needed beyond RESEARCH.md's exhaustive grep, re-verified: `grep -rln "BlockedPendingConfirmation\|SinkBlockedAnchor" --include="*.rs"` at plan time to catch drift).
**Files scanned:** `crates/executor/src/lib.rs`, `sink_sensitivity.rs`, `sink_schema.rs`; `crates/runtime-core/src/executor_decision.rs`, `event.rs`; `crates/brokerd/src/server.rs`, `audit.rs`, `confirmation.rs`.
**Pattern extraction date:** 2026-07-07
