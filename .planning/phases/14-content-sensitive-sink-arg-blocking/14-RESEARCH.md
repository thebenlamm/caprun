# Phase 14: Content-Sensitive Sink-Arg Blocking - Research

**Researched:** 2026-07-07
**Domain:** Rust TCB security-decision code (`crates/executor`, `crates/runtime-core`, `crates/brokerd`) — no external packages, no new dependencies
**Confidence:** HIGH — every claim below is either a direct read of live source at the cited file/line, or a direct quote of the APPROVED, hash-pinned DESIGN docs that gate this phase's code.

## Summary

Phase 14 has **no discovery risk** — it is fully DESIGN-gated. `planning-docs/DESIGN-content-adapter-mediation.md` (APPROVED, hash `ca6294c3…`, `DESIGN-GATE-RECORD-v1.3.md`) already specifies CONTENT-01/02 down to the exact file/line and the exact target code shape. The research task here is not "what's the standard stack" (there is none — this is bespoke TCB code) but "map the design doc's MUSTs onto the live source precisely enough that the planner writes surgical tasks, not exploratory ones."

**The single most important finding:** CONTENT-01 is NOT just "flip Step 3 from no-op to Block." The design doc is explicit and repeated: doing that naively reintroduces the v1.2 B1 bug (`DESIGN-REVIEW-v1.2-round1.md`) in a new form — a plan node with BOTH a tainted `to` and a tainted `body` would Block on `to` only (first-match-wins early return in the per-arg loop) and the body would ship unconfirmed, riding the recipient's confirmation. Making CONTENT-01 *safe* therefore requires implementing **Collect-then-Block (D-14)**: the per-arg loop must scan every arg, collect ALL routing-sensitive-OR-content-sensitive tainted args into one set, and Block once with the whole set. This changes `ExecutorDecision::BlockedPendingConfirmation` and `SinkBlockedAnchor` from singular to **plural** (`Vec`-shaped), which ripples into `runtime-core::Event` (the `anchor` field), and into every `crates/brokerd` consumer that pattern-matches the singular shape today (`confirmation.rs`, `server.rs`, `audit.rs`) plus 6 existing test files. This is Phase 14's real engineering surface, not the one-line content-sensitivity flip.

**Primary recommendation:** Plan Phase 14 as two waves: **Wave 1** — `crates/executor` + `crates/runtime-core` (the plural decision/anchor type, the collect-then-Block loop, the content-sensitive Block consequence, the `attachment` descope). **Wave 2** (depends on Wave 1) — `crates/brokerd` consumer updates (`confirmation.rs`, `server.rs`, `audit.rs`, `event.rs` golden-byte fixture) so the workspace still builds and the existing single-arg-block behavior is preserved byte-for-byte for the recipient-only case, plus updated/new tests. Do NOT implement the CONFIRM-03 combined-digest binding or CONFIRM-04 full narration UX in this phase — those are Phase 16's explicit scope (`combined_digest`, per-arg display polish); Phase 14 only needs the plural *data shape* to exist so Phase 16 can add the digest field on top of it.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Content-sensitivity classification (is `body`/`subject` content-sensitive) | TCB / Executor (`crates/executor`) | — | Already implemented (`sink_sensitivity.rs:93-98`); hardcoded in Rust per `CON-i2-non-bypassable` — never a config/policy layer |
| Collect-then-Block decision logic | TCB / Executor (`crates/executor::submit_plan_node`) | — | Pure function over broker-owned `ValueStore`; no I/O, no async (Gate 2 purity) |
| Plural decision/anchor type definitions | Pure types (`crates/runtime-core`) | — | `ExecutorDecision`/`SinkBlockedAnchor` are pure data — Gate 2 forbids I/O here |
| Durable persistence of the plural anchor into the hash-chained audit DAG | Broker / control plane (`crates/brokerd::audit.rs`, `Event::sink_blocked`) | — | Broker owns the SQLite audit DAG and hash chain; executor never persists |
| Block-time snapshot (`PendingConfirmation.resolved_args`) | Broker / control plane (`crates/brokerd::server.rs`, `confirmation.rs`) | — | Already resolves the FULL arg set (not just the blocked one) — no change needed here for Phase 14 |
| Schema gate (`attachment` removal) | TCB / Executor (`crates/executor::sink_schema.rs`) | — | Fail-closed arg allow-list, hardcoded, evaluated before any resolve/sensitivity work |

## Package Legitimacy Audit

**Not applicable.** Phase 14 adds zero external dependencies. All work is internal Rust refactoring across `crates/executor`, `crates/runtime-core`, `crates/brokerd` using crates already present and pinned in the workspace (`sha2`, `hex`, `uuid`, `serde` — all `{ workspace = true }`; confirmed via `crates/executor/Cargo.toml`, `crates/runtime-core/Cargo.toml`). No `Cargo.toml` changes are expected. `lettre = "0.11.22"` (Phase 13's SMTP adapter dependency) is untouched by this phase.

## Standard Stack

Not applicable in the conventional sense — there is no "standard library for I2 executors." The relevant "stack" is this project's own locked TCB pattern, already implemented and cited below. Do not introduce any third-party validation/classification/policy library — `CON-i2-non-bypassable` and CONTENT-02 explicitly forbid a config-driven or framework-shaped solution.

## Architecture Patterns

### System Architecture Diagram

```
PlanNode { sink: "email.send", args: [to, body] }
        │
        ▼
┌────────────────────────────────────────────────────────────┐
│ crates/executor::submit_plan_node   (pure fn, TCB)          │
│                                                               │
│  Step 0  sink_schema::validate_schema()                     │
│           └─ attachment no longer in allowed set (D-23)     │
│              → UnknownArg if submitted                       │
│                                                               │
│  Step 1..3 (loop over EVERY arg — Collect-then-Block, D-14) │
│    for arg in plan_node.args:                               │
│      resolve(arg.value_id) → record                         │
│      is_routing_sensitive(sink,arg) && tainted?  ─┐          │
│      is_content_sensitive(sink,arg) && tainted?  ─┼─ collect │
│                                                     ▼          │
│                                          blocked: Vec<BlockedArg>
│                                                               │
│  after loop:                                                 │
│    if !blocked.is_empty()                                   │
│         → ExecutorDecision::BlockedPendingConfirmation       │
│              { anchors: Vec<SinkBlockedAnchor>, literals }   │
│    else → Step 0.5 (Draft-only class deny, UNCHANGED         │
│           ordering — D-15) → Allowed                         │
└────────────────────────────────────────────────────────────┘
        │
        ▼ (decision)
┌────────────────────────────────────────────────────────────┐
│ crates/brokerd::server.rs (control plane)                   │
│  - BlockedPendingConfirmation{anchors,..} → Event::sink_blocked
│    (anchor field becomes Vec-shaped; hash-chained payload)   │
│  - resolved_args: Vec<ResolvedArg> ALREADY covers every arg  │
│    (server.rs:458-473) — no change needed here               │
│  - PendingConfirmation persisted (pending_confirmations table)│
└────────────────────────────────────────────────────────────┘
        │
        ▼ (later, separate OS process — out of scope this phase)
   caprun confirm/deny  (Phase 16: CONFIRM-03/04 combined digest + narration)
```

### Recommended Wave Structure (not a folder layout — this phase edits existing files only)

**Wave 1 — `crates/executor` + `crates/runtime-core` (TCB decision/type layer):**
- `crates/runtime-core/src/executor_decision.rs`: `SinkBlockedAnchor.arg: String` → the anchor becomes one-element-per-blocked-arg (keep `SinkBlockedAnchor` singular-per-arg, per the design doc's illustrative shape — `arg`, `value_id`, `literal_sha256`, `taint`, `provenance_chain`, `read_event_id` stay as-is per element); `ExecutorDecision::BlockedPendingConfirmation` becomes `{ anchors: Vec<SinkBlockedAnchor>, literals: Vec<String> }` (or a single `Vec<(SinkBlockedAnchor, String)>` / a named `BlockedArg{anchor, literal}` collection — planner's choice of exact Rust shape, but it MUST be a `Vec`, per D-14, and MUST preserve one-literal-per-anchor pairing, matching today's `{anchor, literal}` pairing 1:1).
- `crates/executor/src/lib.rs`: replace the Step 2 early-return + Step 3 no-op comment with a single collect-loop (see design doc's illustrative Rust snippet, `DESIGN-content-adapter-mediation.md` "Collect-then-Block (D-14)" section) that checks BOTH `is_routing_sensitive` and `is_content_sensitive` per arg, accumulates, and returns ONE plural `BlockedPendingConfirmation` after the loop if the collection is non-empty. Step 0.5 (Draft-only class deny) stays AFTER the loop, unchanged in position (D-15 — do not reorder).
- `crates/executor/src/sink_sensitivity.rs`: `EMAIL_SEND_CONTENT_SENSITIVE` → `&["subject", "body"]` (drop `attachment`, D-23).
- `crates/executor/src/sink_schema.rs`: `email.send`'s `allowed` set → `&["to", "cc", "bcc", "subject", "body"]` (drop `attachment`, D-23) — so a plan node carrying `attachment` is `Denied(UnknownArg)` at Step 0, before any sensitivity check.

**Wave 2 — `crates/brokerd` + `crates/runtime-core::event.rs` consumers (depends on Wave 1's new type shape):**
- `crates/runtime-core/src/event.rs`: `Event.anchor: Option<SinkBlockedAnchor>` → `Event.anchors: Vec<SinkBlockedAnchor>` (or equivalent), `Event::sink_blocked(...)` constructor signature updated to take the plural collection; `#[serde(skip_serializing_if)]` discipline preserved (empty vec for non-block events) so pre-existing non-`sink_blocked` events stay byte-identical (the doc-comment at `event.rs:34-36` currently promises this for `None` — re-verify/re-establish the equivalent promise for `vec![]`, and re-run the golden-byte-fixture test at `event.rs:100` against the new shape).
- `crates/brokerd/src/server.rs:419-485`: the `match &decision { BlockedPendingConfirmation { anchor, literal } => ... }` arms become plural-aware; `Event::sink_blocked(..., anchor.clone())` → `Event::sink_blocked(..., anchors)`. `PendingConfirmation.effect_id = anchor.effect_id` still works (every element of the plural anchor carries the SAME `effect_id` — one Block, one `effect_id`, N blocked args).
- `crates/brokerd/src/audit.rs:204`: `event.anchor.is_none()` (the sink_blocked-must-carry-an-anchor fail-closed check) → `event.anchors.is_empty()`.
- `crates/brokerd/src/confirmation.rs`: `render_block_display` (currently selects ONE display arg, `confirmation.rs:274-291`) does not need the full CONFIRM-04 per-arg narration rewrite in Phase 14 (that's Phase 16's scope) — but it MUST NOT silently regress to displaying only one of N blocked args without at least a passing note; minimum bar for Phase 14 is "does not crash / does not lose data" — verify against a CONTROL-02-shaped fixture (body tainted, recipient trusted) where exactly one blocked arg exists, so the existing single-arg display path is still exercised correctly. A multi-blocked-arg display upgrade is explicitly Phase 16 (CONFIRM-04) — do not implement it here; do not leave a TODO that silently drops data either — assert/panic (fail-closed) rather than truncate is preferable if a genuinely-plural block reaches `render_block_display` before Phase 16 lands the real narration.

### Existing Pattern Being Extended (cite exactly — do not re-derive)

```rust
// Source: crates/executor/src/lib.rs:99-139 (current, singular, first-match-wins)
if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
    && record.taint.iter().any(|t| t.is_untrusted())
{
    let literal_sha256 = { /* sha2::Sha256 over record.literal, hex::encode */ };
    let read_event_id = record.provenance_chain[0];
    let anchor = SinkBlockedAnchor { effect_id, sink: plan_node.sink.clone(), arg: arg.name.clone(),
        value_id: arg.value_id.clone(), literal_sha256, taint: record.taint.clone(),
        provenance_chain: record.provenance_chain.clone(), read_event_id };
    return ExecutorDecision::BlockedPendingConfirmation { anchor, literal: record.literal.clone() };
}
// Step 3 (line 141-142): content-sensitive tainted args do NOT Block — comment-only no-op.
```

```rust
// Target shape per DESIGN-content-adapter-mediation.md "Collect-then-Block (D-14)"
// (illustrative in the design doc — planner turns this into real code):
let mut blocked: Vec<BlockedArg> = Vec::new();
for arg in &plan_node.args {
    let record = /* Steps 1/1a/1b unchanged: resolve, empty-taint guard, empty-provenance guard */;
    let sensitive = sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
        || sink_sensitivity::is_content_sensitive(&plan_node.sink, &arg.name); // CONTENT-01/02
    if sensitive && record.taint.iter().any(|t| t.is_untrusted()) {
        blocked.push(BlockedArg::from_record(&arg.name, &arg.value_id, &record)); // verbatim clone, T-04-03
    }
}
if !blocked.is_empty() {
    return ExecutorDecision::BlockedPendingConfirmation { anchors: blocked };
}
// fall through to Step 0.5 unchanged (D-15 — ordering is load-bearing)
```

### Anti-Patterns to Avoid

- **Adding a new `is_content_sensitive` implementation.** It already exists (`sink_sensitivity.rs:93-98`) and already returns `true` for `email.send`'s `subject`/`body`/`attachment`. Phase 14's work is Step 3's *consequence*, not new classification (D-21). A plan that proposes writing a fresh classifier duplicates existing code — the design doc explicitly flags this as a correction target.
- **Flipping Step 3 to Block WITHOUT also implementing Collect-then-Block.** This passes a naive single-arg test (tainted body alone blocks) but silently reintroduces B1: a plan node with both `to` and `body` tainted blocks on `to` only, and the body ships unconfirmed. This is not a hypothetical — `DESIGN-content-adapter-mediation.md`'s own "Problem Being Solved" section names this as the exact failure this document exists to close.
- **Reordering Step 0.5 (Draft-only class deny) relative to the per-arg loop.** D-15 is explicit: the collect-all loop MUST complete with NO Block found before Step 0.5 runs, exactly as today. Reordering reintroduces a *variant* of B1 (a Draft session with a tainted body gets Denied by the class-deny before the body's Block is ever collected, making confirm unreachable).
- **Building CONFIRM-03's combined_digest or CONFIRM-04's full per-arg narration in this phase.** Those requirements map to Phase 16 in `.planning/REQUIREMENTS.md`'s traceability table. Phase 14 only needs the `Vec`-shaped decision/anchor type to exist; the digest field and narration polish are explicitly out of this phase's scope, and `DESIGN-confirm-binding.md` is the design doc that governs them.
- **Re-adding `attachment`.** D-23 explicitly forbids treating the `attachment` descope as a temporary stub — it requires its own future DESIGN analysis of the Content-Disposition CRLF surface, out of v1.3 entirely.
- **Introducing a general content-classification taxonomy/framework.** CONTENT-02 is a hard MUST NOT — sensitivity for content args stays a single hardcoded match arm scoped to `email.send` only, mirroring `is_routing_sensitive`'s existing shape. Do not generalize to other sinks.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Content-sensitivity classification | A new classifier/config/policy layer | The existing `is_content_sensitive` (`sink_sensitivity.rs:93-98`), narrowed per D-23 | Already implemented, already reviewed at v0; CONTENT-02 forbids a generalized framework |
| Literal digesting for the audit anchor | A new hash scheme | The existing `Sha256`/`hex::encode` pattern (`lib.rs:112-116`), applied per blocked-arg element | Matches the existing tamper-evident/redactable pattern; Phase 16 layers the combined digest ON TOP of these per-element digests (per `DESIGN-confirm-binding.md`) — do not invent a competing digest shape now |
| Multi-arg Block collection | A `HashMap`/dedup-by-sink-shaped structure | A simple ordered `Vec<BlockedArg>` in `plan_node.args` iteration order | The design doc pins the ordering requirement (stable `plan_node.args` order) because Phase 16's combined digest is computed over this exact order — any reordering here breaks a downstream contract |

**Key insight:** This phase is bounded almost entirely by a document that has already been adversarially reviewed three times (`DESIGN-GATE-RECORD-v1.3.md`). The main risk is not "wrong design" — it's "implementer deviates from the pinned shape and reintroduces a bug the review process already found and fixed once." Cite the design doc's illustrative Rust shapes directly in task descriptions rather than re-deriving the approach.

## Common Pitfalls

### Pitfall 1: Implementing CONTENT-01 as a pure content-classification flip, skipping D-14
**What goes wrong:** Tests pass for the single-tainted-body case (CONTROL-02-shaped: body tainted, recipient trusted). The B1-reincarnation case (both `to` and `body` tainted) silently regresses — the body ships unconfirmed.
**Why it happens:** The roadmap's Phase 14 success criteria (ROADMAP.md lines 121-124) read as if a one-line flip suffices; only the DESIGN doc states the full requirement.
**How to avoid:** Treat D-14 (Collect-then-Block, plural anchor) as IN SCOPE for Phase 14, not deferred. Write a test with BOTH `to` and `body` tainted on the same plan node and assert BOTH appear in the returned collection.
**Warning signs:** A plan or PLAN.md that describes only "change Step 3 from comment to Block" without touching `ExecutorDecision`'s shape.

### Pitfall 2: Breaking the `event.rs` golden-byte-fixture test
**What goes wrong:** `Event.anchor: Option<SinkBlockedAnchor>` changing to a `Vec` changes JSON serialization shape, which can silently change the hashed payload bytes for EVERY future event (not just `sink_blocked` ones, if the `skip_serializing_if` discipline isn't preserved for the empty case).
**Why it happens:** The existing test (`event.rs:100`, referenced but not fully read in this research pass — planner/executor should read it directly before touching `event.rs`) asserts a specific byte shape for a `None`-anchor event; a naive `Vec` migration could add a `"anchors":[]` field to every non-block event, changing its hash.
**How to avoid:** Preserve `#[serde(default, skip_serializing_if = "...is_empty")]` on the new `anchors: Vec<...>` field so a non-block event serializes identically to today (empty vec omitted, matching today's `None` omitted). Re-run/update the golden-byte fixture explicitly as part of this phase's verification, not as an afterthought.
**Warning signs:** Any existing audit-DAG hash-chain verification test (`crates/brokerd/tests/audit_dag.rs`, `s9_acceptance.rs`) failing after this change, for events unrelated to `email.send`.

### Pitfall 3: Missing the `attachment` schema-gate interaction
**What goes wrong:** Removing `attachment` from `EMAIL_SEND_CONTENT_SENSITIVE` alone (without also removing it from `sink_schema.rs`'s `allowed` set) leaves `attachment` schema-ACCEPTED but no-longer-content-sensitive — a tainted attachment would then silently Allow (fail-open), the opposite of the design intent.
**Why it happens:** D-23 requires editing TWO separate hardcoded lists in two different files; it's easy to edit one and forget the other.
**How to avoid:** A single task/commit should change both `sink_sensitivity.rs:71` (`EMAIL_SEND_CONTENT_SENSITIVE`) AND `sink_schema.rs`'s `email.send` `allowed` array together, with a test asserting a plan node carrying `attachment` is `Denied(UnknownArg("attachment"))`.
**Warning signs:** `sink_schema.rs`'s existing test `email_send_exact_args_ok` (line 165-177) still lists `subject`/`body` only — good, it already doesn't include `attachment`, so this test won't catch a missed edit; a NEW explicit test is needed.

### Pitfall 4: Stale test `tainted_body_and_attachment_allow_in_v0`
**What goes wrong:** `crates/executor/tests/executor_decision.rs:229` currently asserts tainted `body`/`attachment` → `Allowed` (the v0 no-op behavior). This test will fail — correctly — once Step 3 becomes a real Block. If left unaddressed, `cargo test --workspace` reports a regression that is actually the intended behavior change, and the planner/executor may waste time "fixing" a test that should instead be rewritten to assert `BlockedPendingConfirmation`.
**Why it happens:** The test name and docstring (`"body" and "attachment" are content-sensitive — tainted values must NOT Block`) describe the OLD, now-superseded behavior.
**How to avoid:** Rewrite this test (and its docstring) to assert the NEW behavior: tainted `body` → Block; a plan node carrying `attachment` at all → `Denied(UnknownArg)` (schema gate, since attachment is now unregistered).
**Warning signs:** `cargo test -p executor` failing on this specific test after the Step 3 change — expected, must be fixed as part of this phase, not deferred.

### Pitfall 5: Confusing this phase's scope with Phase 16's CONFIRM-03/04
**What goes wrong:** Scope creep into implementing the combined SHA-256 digest (`combined_digest`, fixed-width per-element hashing) or the full every-arg block narration UX, both of which belong to `DESIGN-confirm-binding.md` and are traced to Phase 16 in `.planning/REQUIREMENTS.md`.
**Why it happens:** Both DESIGN docs were authored and approved together (same gate), and `DESIGN-confirm-binding.md` repeatedly cross-references the plural anchor this phase introduces, making it tempting to "finish the job."
**How to avoid:** Phase 14 delivers the `Vec`-shaped decision/anchor type and the collect-then-Block loop. It does NOT add a `combined_digest` field to `PendingConfirmation`, does NOT change `render_block_display`'s narration format beyond what's needed to not silently drop data, and does NOT touch `caprun confirm`/`deny`'s CLI surface. Leave a code comment at the plural type definition pointing to `DESIGN-confirm-binding.md` for Phase 16's follow-on, exactly as the DESIGN doc's own structure implies.
**Warning signs:** A PLAN.md task mentioning "combined digest" or "CONFIRM-03" — flag for descoping to Phase 16.

## Code Examples

### The exact per-arg loop location to modify
```rust
// Source: crates/executor/src/lib.rs:62-143 (submit_plan_node's per-arg loop)
// Step 1/1a/1b (resolve, empty-taint guard, empty-provenance guard) are UNCHANGED.
// Step 2 (routing-sensitive check + early return) and Step 3 (content-sensitive
// no-op) are the two blocks that collapse into ONE collect-then-check per D-14.
```

### The existing digest pattern to reuse per blocked-arg element
```rust
// Source: crates/executor/src/lib.rs:112-116 — reuse this EXACT pattern per element
// of the new Vec<BlockedArg>, do not invent a new hash scheme.
let literal_sha256 = {
    let mut hasher = Sha256::new();
    hasher.update(record.literal.as_bytes());
    hex::encode(hasher.finalize())
};
```

### The anti-stapling invariant to preserve per element
```rust
// crates/executor/src/lib.rs:1-10 doc comment — MUST hold for every element of
// the new plural collection, not just the whole decision:
// "The executor reads taint ONLY through value_store.resolve(). It NEVER mints
//  a ValueRecord and NEVER sets a taint field."
// Negative-grep acceptance criteria (unchanged, still enforced post-Phase-14):
//   grep -v '^[[:space:]]*//' crates/executor/src/lib.rs | grep -c 'ValueStore::mint'  → 0
//   grep -v '^[[:space:]]*//' crates/executor/src/lib.rs | grep -c 'ValueRecord {'     → 0
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| Content-sensitive tainted args are marked but never Block (v0/v1.2) | Content-sensitive tainted args Block, same as routing-sensitive | Phase 14 (this phase), gated by `DESIGN-GATE-RECORD-v1.3.md` APPROVED 2026-07-07 | Closes CONTENT-01, the gap deferred at v1.2 scoping |
| `ExecutorDecision::BlockedPendingConfirmation` / `SinkBlockedAnchor` are singular (one blocked arg per decision) | Plural (`Vec`-shaped) — a decision carries every sensitive+tainted arg found in one pass | Phase 14 (this phase) | Enables CONTENT-01 to compose safely with the existing routing block (D-02/D-14); is the load-bearing prerequisite for Phase 16's combined-digest confirm |
| `attachment` is schema-accepted and content-sensitive | `attachment` removed from both the schema `allowed` set and the content-sensitive set | Phase 14 (this phase), per D-23 | A plan node carrying `attachment` is `Denied(UnknownArg)` before any sensitivity check — descoped, not silently permissive |

**Deprecated/outdated:**
- The `tainted_body_and_attachment_allow_in_v0` test name/assertion (`crates/executor/tests/executor_decision.rs:229`) describes v0/v1.2 behavior this phase intentionally supersedes.
- The `lib.rs:35-36` module doc comment ("Content-sensitive tainted args do not Block in v0 — marked for Tier-4 verbatim review, not yet surfaced") is stale as of this phase and should be updated alongside the code change.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The exact Rust type name for the new plural collection (e.g. `Vec<BlockedArg>` vs `Vec<(SinkBlockedAnchor, String)>` vs a new named struct) is left to the planner/implementer — the DESIGN doc gives an "illustrative shape, not literal code to paste" | Architecture Patterns, Code Examples | Low — the DESIGN doc explicitly disclaims literal-code-to-paste status; any shape satisfying "Vec-shaped, one-per-blocked-arg, preserves anti-stapling per element, preserves stable `plan_node.args` order" satisfies D-14 |
| A2 | `crates/brokerd/src/confirmation.rs::render_block_display`'s minimum bar for Phase 14 (assert/panic rather than silently truncate on a genuinely-plural block, pending Phase 16's real narration) is a reasonable interim contract, not explicitly spelled out in the DESIGN doc for Phase 14's boundary specifically | Architecture Patterns, Wave 2 | Medium — if the planner instead chooses to do a MINIMAL Phase-16-style narration upgrade now, that is not wrong per the DESIGN doc, just extra scope; conversely leaving `render_block_display` silently showing only one of N blocked args without any interim safeguard would understate risk to a human confirming — recommend the fail-closed assert as the safer default until Phase 16 lands |

**If this table is empty:** N/A — two items above need no user confirmation beyond acknowledging the recommended interim behavior for `render_block_display`, since the DESIGN doc's Phase-16 boundary is a phase-traceability inference (from `.planning/REQUIREMENTS.md`), not a doc-stated Phase 14 boundary in `DESIGN-content-adapter-mediation.md` itself.

## Open Questions

1. **Exact `Vec` element type name for the plural collection.**
   - What we know: it must be Vec-shaped, one element per blocked arg, each element carrying the same fields `SinkBlockedAnchor` carries today plus its paired literal.
   - What's unclear: whether to keep `SinkBlockedAnchor` as the per-element type (paired with a separate `Vec<String>` of literals in matching order) or introduce a new combined `BlockedArg { anchor: SinkBlockedAnchor, literal: String }` struct.
   - Recommendation: prefer the combined `BlockedArg { anchor, literal }` struct — it removes the risk of the two parallel `Vec`s (anchors, literals) silently drifting out of index-alignment, which is exactly the kind of bug this design gate exists to prevent.

2. **How much of `render_block_display` Phase 14 must touch.**
   - What we know: Phase 16 (CONFIRM-04) owns the full every-arg narration UX per `.planning/REQUIREMENTS.md`'s traceability table.
   - What's unclear: whether leaving `render_block_display` operating on only the "first untrusted arg" (today's behavior, `confirmation.rs:275-279`) is acceptable for Phase 14's exit, given the underlying `PendingConfirmation.resolved_args` already contains every arg (unchanged by this phase) — it is CONFIRM-03/04 (Phase 16) that binds/narrates the BLOCKED subset specifically.
   - Recommendation: verify Phase 14's exit criteria only require the EXECUTOR to correctly Block on/collect the right set (CONTENT-01/02, ROADMAP success criteria 1-2) — leave `render_block_display` functionally unchanged (it will still show a coherent single confirmable literal for the CONTROL-02 single-blocked-arg case that Phase 16 tests), and explicitly note in the plan that full multi-arg narration is deferred to Phase 16 by design.

## Environment Availability

Skipped — this phase makes no external tool/service/runtime dependency. All work is `cargo build`/`cargo test` against the existing workspace; no new crates, no Docker/Colima/Mailpit dependency (those belong to Phase 13's SMTP work and Phase 17's live acceptance, not this phase's unit/integration-test surface).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust's built-in `#[test]` / `cargo test` (no external test framework) |
| Config file | none — workspace `Cargo.toml` (`resolver = "3"`, edition 2021) |
| Quick run command | `cargo test -p executor` (fastest relevant surface); `cargo test -p runtime-core` for the `Event`/`SinkBlockedAnchor` type-shape tests |
| Full suite command | `cargo test --workspace --no-fail-fast` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONTENT-01 | A plan node with a tainted value in the email sink's `body` arg is Blocked, same UX class as routing block | unit | `cargo test -p executor tainted_body` | ❌ Wave 1 — rewrite of `crates/executor/tests/executor_decision.rs:229` |
| CONTENT-01 (composition) | A plan node with BOTH tainted `to` AND tainted `body` surfaces BOTH as Blocked in ONE decision (D-02/D-14) | unit | `cargo test -p executor collect_then_block` (new test name) | ❌ Wave 1 — new test |
| CONTENT-01 (CONTROL-02 precursor) | Tainted `body` + TRUSTED `to` still Blocks (proves body dimension isn't dead code) | unit | `cargo test -p executor body_tainted_recipient_trusted_blocks` (new test name) | ❌ Wave 1 — new test; note full CONTROL-02 acceptance framing is Phase 16, but the underlying executor behavior this phase must establish is identical and should be tested here too |
| CONTENT-02 | `attachment` is schema-rejected (`UnknownArg`), not silently content-sensitive-but-allowed | unit | `cargo test -p executor attachment_unknown_arg` (new test name) | ❌ Wave 1 — new test |
| CONTENT-02 | `is_content_sensitive` scope stays a single hardcoded match arm for `email.send` only — no regression to other sinks | unit | existing `crates/executor/src/sink_sensitivity.rs` tests (`unknown_sink_not_routing_sensitive` pattern) + one new `unknown_sink_not_content_sensitive` test | Partial — pattern exists (line 148), content-sensitive equivalent does not |
| D-14 (plural type shape, cross-crate) | `Event::sink_blocked`'s golden-byte serialization for non-block events is UNCHANGED after the `anchor` → `anchors`/plural migration | unit | `cargo test -p runtime-core` (the existing golden-byte-fixture test at `event.rs:100`, re-verified) | ✅ exists — must be re-verified, not newly written |
| D-14 (audit-DAG fail-closed check) | `audit.rs`'s "sink_blocked event must carry an anchor" check still fails closed with the plural shape | unit/integration | `cargo test -p brokerd` (existing `audit.rs:204`-adjacent tests, e.g. in `audit_dag.rs`/`durable_anchor.rs`) | ✅ exists — must be updated for `anchors.is_empty()`, not newly written |

### Sampling Rate
- **Per task commit:** `cargo test -p executor` (and `-p runtime-core` for Wave 1 type-shape tasks; `-p brokerd` for Wave 2 consumer tasks)
- **Per wave merge:** `cargo test --workspace --no-fail-fast`
- **Phase gate:** Full suite green before `/gsd-verify-work`; also re-run `./scripts/check-invariants.sh` (Gate 1 `EffectRequest` token absence, Gate 2 `runtime-core` purity) — this phase touches `runtime-core::event.rs` and `runtime-core::executor_decision.rs`, both purity-gated files, so Gate 2 is directly relevant (no `std::io`/`std::fs`/`std::net`/`tokio`/`async fn` may be introduced there).

### Wave 0 Gaps
- [ ] Rewrite `crates/executor/tests/executor_decision.rs::tainted_body_and_attachment_allow_in_v0` (line 229) — its name and assertion describe pre-Phase-14 behavior and MUST be replaced, not merely updated in place, so the git history is legible about the behavior change.
- [ ] New test: both `to` and `body` tainted on one `email.send` plan node → one `BlockedPendingConfirmation` containing BOTH (D-02/D-14 composition proof) — this is the single most important new test in this phase; it is what proves the B1-reincarnation risk the DESIGN doc names is actually closed.
- [ ] New test: `attachment` arg on `email.send` → `Denied(UnknownArg("attachment"))` (D-23 schema-gate proof).
- [ ] Re-verify (not necessarily rewrite) the `event.rs:100` golden-byte-fixture test against the new `anchors` field shape.
- [ ] Re-verify `crates/brokerd/src/audit.rs`'s existing "sink_blocked must carry an anchor" fail-closed test against `anchors.is_empty()`.
- [ ] Every existing test file that pattern-matches `ExecutorDecision::BlockedPendingConfirmation { anchor, literal }` (singular) must be mechanically updated to the plural shape — enumerated exhaustively below in "Runtime State Inventory" (this phase is a refactor of a locked shared type, so the blast radius is a discovery item in itself, not a runtime-state item, but the same "did you find everything" discipline applies).

**Files requiring mechanical updates for the type-shape change (grep-verified, exhaustive as of this research pass):**
`cli/caprun/src/worker.rs:204`, `cli/caprun/tests/confirm.rs:78`, `crates/brokerd/src/confirmation.rs:653` (test fixture), `crates/brokerd/src/server.rs:420,453`, `crates/brokerd/tests/durable_anchor.rs:201,384,421,455`, `crates/brokerd/tests/phase5_dispatch.rs:120`, `crates/brokerd/tests/s9_acceptance.rs:128,176,435`, `crates/executor/tests/executor_decision.rs` (multiple `matches!`/destructure sites, lines 75,107,221,300,401), `crates/runtime-core/src/event.rs:38,80,90` (+golden-byte test), `crates/runtime-core/tests/task2_types.rs:70,93,111`. Re-run `grep -rln "BlockedPendingConfirmation\|SinkBlockedAnchor" --include="*.rs"` at plan time to catch any drift since this research pass.

## Security Domain

This project's threat model is not conventional web-app ASVS — it is the I0/I1/I2 taint-propagation model defined in `planning-docs/DESIGN-taint-model.md` and enforced by this exact crate. Mapping the closest-fit ASVS-style categories:

| Category (adapted) | Applies | Standard Control |
|---------------------|---------|-------------------|
| Value-injection into sensitive sink args (this project's I2, ~ASVS V5 Input Validation analog) | yes | Hardcoded TCB match-arm classification (`sink_sensitivity.rs`) + collect-then-Block loop (`submit_plan_node`) — never a config/policy-file mechanism (`CON-i2-non-bypassable`) |
| Instruction/context injection (I1) | no change this phase | Governed by session draft-only demotion (Step 0.5) — this phase preserves its ordering (D-15) but does not modify its logic |
| Tamper-evidence of the audit trail (~ASVS V9 analog) | yes | SHA-256 hash-chained `events` table; the plural anchor rides inside the hashed `payload`, so any post-hoc edit to a blocked-arg's literal digest breaks the chain — unchanged mechanism, extended to N elements |
| Fail-closed on malformed/incomplete decision data | yes | Existing `DanglingHandle`/`EmptyTaintInvariantViolation`/`MissingProvenanceAnchor`/`UnknownArg` `DenyReason` variants — Phase 14 adds no new variant (attachment removal surfaces the EXISTING `UnknownArg` variant, doesn't need a new one) |

### Known Threat Patterns for this stack

| Pattern | STRIDE (adapted) | Standard Mitigation |
|---------|--------|---------------------|
| First-match-wins Block silently drops a second sensitive tainted arg (B1-reincarnation) | Tampering / Information Disclosure (a human confirms less than they think they're confirming) | Collect-then-Block (D-14) — this phase's core deliverable |
| Content-sensitive arg accepted by schema but silently exempted from sensitivity (the `attachment` failure mode this phase actively prevents by REMOVING it, not by leaving it half-classified) | Elevation of Privilege (fail-open) | D-23 — remove from BOTH the schema `allowed` set and the sensitivity set together, in one atomic change |
| Golden-byte / hash-chain drift from an unrelated type-shape change | Tampering (breaks tamper-evidence guarantees for unrelated event types) | Preserve `skip_serializing_if` empty-collection discipline; re-verify the golden-byte fixture explicitly |

## Sources

### Primary (HIGH confidence — direct source reads this session)
- `crates/executor/src/lib.rs` (full file read) — the per-arg loop, Step 0.5 ordering, anti-stapling doc comments
- `crates/executor/src/sink_sensitivity.rs` (full file read) — `is_content_sensitive`, `EMAIL_SEND_CONTENT_SENSITIVE`, existing tests
- `crates/executor/src/sink_schema.rs` (full file read) — `KNOWN_SINKS`, `validate_schema`, existing tests
- `crates/executor/src/value_store.rs` (full file read) — `ValueStore::mint`/`resolve`, anti-stapling invariant
- `crates/runtime-core/src/executor_decision.rs` (full file read) — `DenyReason`, `SinkBlockedAnchor`, `ExecutorDecision`
- `crates/runtime-core/src/event.rs` (partial read, lines 1-100) — `Event.anchor`, `Event::sink_blocked`, golden-byte fixture test presence
- `crates/brokerd/src/confirmation.rs` (partial read, lines 1-140, 250-370) — `PendingConfirmation`, `ResolvedArg`, `render_block_display`, `confirm()`
- `crates/brokerd/src/server.rs` (partial read, lines 395-495) — decision handling, `Event::sink_blocked` construction, `PendingConfirmation` assembly
- `scripts/check-invariants.sh` (full file read) — Gate 1 (`EffectRequest` token ban), Gate 2 (`runtime-core` purity)
- `grep -rln "BlockedPendingConfirmation\|SinkBlockedAnchor"` across the workspace — exhaustive consumer enumeration
- `planning-docs/DESIGN-content-adapter-mediation.md` (full file read, APPROVED, hash `ca6294c3…`) — CONTENT-01/02, D-01/D-02/D-14/D-15/D-16/D-17/D-18/D-21/D-23
- `planning-docs/DESIGN-confirm-binding.md` (full file read, APPROVED, hash `fab14ec9…`) — the Phase-16-scoped combined-digest/narration doc, read to establish the exact scope boundary Phase 14 must NOT cross
- `planning-docs/DESIGN-GATE-RECORD-v1.3.md` (full file read) — 3-round adversarial review history, gate status UNBLOCKED, final approved hashes

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — traceability table mapping CONFIRM-03/04 to Phase 16 (this is the basis for A2's scope-boundary inference, since the DESIGN doc itself doesn't state a per-phase boundary)
- `.planning/ROADMAP.md` (Phase 14 section, lines 116-126) — the phase's stated success criteria, narrower on their face than the DESIGN doc's actual requirement (D-14) — this gap is the key finding of this research pass

### Tertiary (LOW confidence)
None — every claim in this document traces to a direct source or DESIGN-doc read performed this session; no WebSearch or external documentation was needed (this phase has zero external dependencies).

## Metadata

**Confidence breakdown:**
- Standard stack: N/A — no external stack; internal TCB pattern extension only
- Architecture: HIGH — every claim cites an exact file/line in either live source or the APPROVED design doc
- Pitfalls: HIGH — each pitfall is either a direct quote of a design-doc-recorded review finding (B1-reincarnation, partition-blindness precedent) or a directly observed stale test/comment in live source

**Research date:** 2026-07-07
**Valid until:** Stable until the next `crates/executor`/`crates/runtime-core` structural change (no natural expiry — re-verify file/line citations if Phase 15/16 lands first and touches these same files, since line numbers will drift)
