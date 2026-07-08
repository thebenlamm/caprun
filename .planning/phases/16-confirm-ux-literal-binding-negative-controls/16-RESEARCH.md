# Phase 16: Confirm UX, Literal Binding & Negative Controls - Research

**Researched:** 2026-07-08
**Domain:** Rust TCB extension — combined-digest confirm-binding over a plural blocked-arg set, multi-arg block narration, and Allowed-decision sink-invocation wiring for a negative control
**Confidence:** HIGH (current source read directly, file:line cited throughout)

No `.planning/phases/16-*/16-CONTEXT.md` exists (this phase has not been through `/gsd-discuss-phase` yet). There is therefore no `## User Constraints` section — the planner's binding constraints for this phase are `CLAUDE.md`, `.planning/REQUIREMENTS.md` (CONFIRM-01/03/04, CONTROL-01/02), and `planning-docs/DESIGN-confirm-binding.md` (the APPROVED v1.3 design doc that is this phase's primary contract).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CONFIRM-01 | `caprun confirm`/`deny` displays verbatim recipient AND body + provenance for a doc-derived send blocked at I2+CONTENT-01 | "Current `render_block_display`" + "Block Narration for Every Arg" sections below — the display currently shows exactly ONE arg and panics on a genuine 2-arg block; CONFIRM-01/04 replace this with real per-arg narration |
| CONFIRM-03 | `caprun confirm` binds to ONE combined hash over the FULL SET of blocked args' resolved literals; no drift; no truncation; no partial confirm | "Combined-Digest Binding" section — `combined_digest` does not exist anywhere in the codebase today; this is fully greenfield, built on the existing `PendingConfirmation`/`ResolvedArg` store |
| CONFIRM-04 | The BLOCK moment narrates provenance for EVERY blocked arg, not "Error: blocked" | Same as CONFIRM-01 — one narration function serves both `confirm`/`deny` (pre-decision display) and the Block moment itself |
| CONTROL-01 | Fully-trusted send proceeds with NO block/confirm gate, same run as hostile block | "CONTROL-01's Live-Send Question" section — Allowed-decision sink invocation is wired ONLY for `file.create` today; `email.send` has no such wiring. Recommends staying at the Allowed-decision proof level (already built in Phase 15) rather than adding new sink-invocation code this phase |
| CONTROL-02 | Body TAINTED + recipient TRUSTED → still blocks | "CONTROL-02's Test Shape" section — an executor-level unit fixture already exists (`body_tainted_recipient_trusted_blocks`, Phase 14); this phase needs a live/wire-level analog, not new executor logic |
</phase_requirements>

## Summary

Phase 16's real engineering content is narrower than "build a confirm UX" and has one architectural trap the design doc does not fully resolve on its own: **the design doc's illustrative shape treats the digest as riding "inside the hashed `sink_blocked` anchor payload,"** but Phase 14 already made the anchor PLURAL (`Vec<BlockedArg>`, one `SinkBlockedAnchor` per element) — there is no single "the anchor" left to embed a whole-set digest inside. The correct, codebase-consistent placement (confirmed by reading `crates/runtime-core/src/event.rs`'s existing `derived_value_id`/`input_value_ids`/`transform_kind` fields, added in Phase 15 for exactly this kind of block-level-not-element-level payload) is a **new field on `Event` itself** (e.g. `combined_digest: Option<String>`), `#[serde(default, skip_serializing_if = "Option::is_none")]`, populated only for `sink_blocked` events. Since `crates/brokerd/src/audit.rs:229` serializes the **whole** `Event` struct into the hashed `payload` column (`serde_json::to_string(event)`), adding this field automatically makes it tamper-evident with zero new plumbing beyond the field itself and its population site in `crates/brokerd/src/server.rs`'s `SubmitPlanNode` handler.

The second major finding is that **`render_block_display`'s "fail-closed panic path" (T-14-08) is a literal Rust `assert!` macro** (`crates/brokerd/src/confirmation.rs:299-304`), not a `Result::Err`. It fires whenever the re-derived blocked-arg count (recomputing the executor's own `is_routing_sensitive || is_content_sensitive` predicate over `pc.resolved_args`) exceeds 1. It is currently **completely untested** — no test in the codebase constructs a genuinely-plural (2+ blocked arg) `PendingConfirmation` and calls `confirm()`/`deny()`/`render_block_display()` against it. Both existing seed helpers (`seed_pending_file_create_block`, `seed_pending_email_send_block`) construct exactly ONE `SinkBlockedAnchor`/blocked arg. Per the mandate from Phase 14/caprun-opus-77, this phase MUST add a test proving the panic fires correctly BEFORE (or as part of) replacing the guard with CONFIRM-04's real multi-arg narration — not just delete the assert and lose the coverage opportunity.

The third major finding, directly answering the coordinator's flagged CONTROL-01 question: **the confirm-less "no gate at all" path does NOT currently reach the real SMTP adapter for `email.send`.** `crates/brokerd/src/server.rs`'s `SubmitPlanNode` handler invokes a live sink on an `Allowed` decision ONLY when `plan_node.sink.0 == "file.create"` (line 576); there is no equivalent branch for `email.send`. `invoke_email_smtp_from_resolved` (`crates/brokerd/src/sinks/email_smtp.rs:204`) is called from exactly one call site — `confirmation.rs::confirm()`'s `email.send` match arm — which only runs on a CONFIRM decision, never an Allowed one. Phase 15's `s9_live_clean_allow_path` test (already proven, Linux-verified) asserts ONLY the `plan_node_evaluated` (Allowed) decision and the absence of a `sink_blocked` event — it does NOT assert an actual SMTP send. Given the roadmap's own phase boundary (Phase 17/ACCEPT-01 owns "confirm sends exactly once via the real adapter... composes CONTROL-01 (clean send, ungated) alongside the hostile block"), the recommended scope for Phase 16 is to compose the ALREADY-PROVEN Allowed-decision proof with the hostile-block proof into one test/run (satisfying "NO block and NO confirm gate... in the same acceptance run as the hostile block" at the decision level), and defer actual Mailpit-delivery wiring for the trusted path to Phase 17. Building that wiring now would be scope creep into ACCEPT-01's job and would require touching `check-invariants.sh` Gate 3's mint-call-site allowlist and the Allowed-decision dispatch arm — a change with its own review surface this phase's design doc does not gate.

**Primary recommendation:** (1) Add `combined_digest: Option<String>` to `Event` (mirroring the `derived_value_id` pattern) and `blocked_arg_names: Vec<String>` + `combined_digest: String` to `PendingConfirmation`, computed ONCE at Block time in `server.rs` using SHA-256-over-fixed-width-per-element-`literal_sha256`-in-order (never plain concatenation), recompute-and-compared before any send in `confirm()`. (2) Replace `render_block_display`'s single-arg-or-panic logic with a real per-arg loop over the blocked subset (selected by `PendingConfirmation.blocked_arg_names`), FIRST adding a regression test that drives the CURRENT panic path from a genuinely-2-anchor fixture (proving T-14-08's guard fires today), so the replacement is a verified behavior change, not a silent deletion. (3) For CONTROL-01, extend the ALREADY-EXISTING `s9_live_clean_allow_path` + `s9_live_email_hostile_block` proofs (both in `cli/caprun/tests/s9_live_block.rs`, currently separate `#[test]` functions/processes) into one composed test or a shared test module run in the same `cargo test` invocation — do not add new Allowed-path sink-invocation wiring this phase. (4) For CONTROL-02, add a live/wire-level analog of the existing executor-unit-level `body_tainted_recipient_trusted_blocks` fixture (Phase 14), driven through the real confined-worker + broker + executor stack the way `s9_live_email_hostile_block` already is.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Combined-digest computation (SHA-256 over fixed-width per-element digests, in blocked-arg order) | Broker (`crates/brokerd/src/server.rs`, at Block-time write) | — | Must be computed ONCE, at the same atomic write as `resolved_args`/`sink_blocked` — never re-derived from a live `ValueStore` (which does not survive process exit); this is a broker-owned durability concern, not an executor decision |
| Combined-digest recompute-and-compare before send | Broker confirm-path process (`crates/brokerd/src/confirmation.rs::confirm()`) | — | The confirm process is a SEPARATE, LATER OS process; it reads the frozen snapshot and must verify integrity before invoking any sink — same tier that already owns `confirm()`'s dispatch logic |
| Verbatim, no-truncation display of every blocked arg | CLI (`cli/caprun/src/main.rs` → `crates/brokerd/src/confirmation.rs::render_block_display`) | — | Human-facing text rendering; brokerd already owns this function, CLI only prints its return value |
| Per-arg Block-moment narration (CONFIRM-04) | Broker (`confirmation.rs::render_block_display`, called from BOTH `confirm()` and `deny()`) | — | Same function, same tier — both verbs show identical evidence before acting (existing discipline, unchanged) |
| Sink invocation for a CONFIRMED plural block | Broker confirm-path process (`confirmation.rs::confirm()` dispatch, `sinks::email_smtp`/`sinks::file_create`) | — | Unchanged from Phase 13/14 — sinks are invoked from the FULL `resolved_args` (not just the blocked subset), which already exists |
| Trusted-send Allowed-decision proof (CONTROL-01) | CLI/integration test (`cli/caprun/tests/s9_live_block.rs`) | Broker (decision only, no sink invocation) | Recommendation: stay at the decision level this phase (see Summary) — actual SMTP delivery for the trusted path is Phase 17/ACCEPT-01 scope |
| Body-tainted/recipient-trusted Block proof (CONTROL-02) | Executor (unit, already exists) + confined-worker/broker/executor stack (live, new this phase) | — | The classification logic is unchanged; only a new live fixture is needed |

## Standard Stack

This phase adds **no new Cargo dependency**. It is a pure extension of existing internal types (`Event`, `PendingConfirmation`, `BlockedArg`) using primitives already vendored and already used for the identical single-arg `literal_sha256` pattern.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sha2` | workspace-pinned (`Cargo.toml` `[workspace.dependencies]`) | SHA-256 over each blocked arg's fixed-width `literal_sha256`, in blocked-arg order, to produce `combined_digest` | Already the project's sole hash primitive (`crates/executor/src/lib.rs:130-134`, `crates/brokerd/src/confirmation.rs:720-724`); DESIGN-confirm-binding.md explicitly forbids introducing HMAC/keyed-hash/non-SHA-256 |
| `hex` | workspace-pinned | Lowercase-hex encoding of the digest bytes | Same reuse rationale — `hex::encode(hasher.finalize())` is the existing verbatim pattern |
| `serde_json` | workspace-pinned | `PendingConfirmation.resolved_args`/new `blocked_arg_names` field JSON round-trip in the `pending_confirmations` side table | Already used for this exact column (`crates/brokerd/src/confirmation.rs:137,179`) |

**Version verification:** No new package to verify — `sha2`/`hex`/`serde_json` are already resolved in `Cargo.lock` and in active use by the exact files this phase modifies `[VERIFIED: local Cargo.toml/lockfile read, this session]`.

### Supporting
None new.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| SHA-256 over fixed-width per-element `literal_sha256`s, in order | Plain literal concatenation then one SHA-256 | REJECTED by the design doc itself (partition-blind — `H("a"‖"bc") == H("ab"‖"c")`, admits a to/body boundary-shift bypass, DESIGN-confirm-binding.md finding #4). Do not implement. |
| SHA-256 over fixed-width per-element digests | Length-prefixed encoding (`len(name)‖name‖len(literal)‖literal`) | Explicitly RETRACTED by the design doc's own round-3 tightening note — folds in names/raw literals rather than fixed-width digests, risking producer/verifier encoding divergence. Do not implement. |
| A new `Event.combined_digest: Option<String>` field | Threading the combined digest through `SinkBlockedAnchor` (per-element) instead | REJECTED — the digest is a WHOLE-SET property, not a per-element one; duplicating an identical value into every `BlockedArg`'s anchor would be redundant and would not match "one combined digest for the whole set" (CONFIRM-03's literal wording) |

## Package Legitimacy Audit

No external packages are added or removed by this phase — all dependencies used (`sha2`, `hex`, `serde_json`, `lettre` transitively via existing `email_smtp.rs`) are already approved and audited in Phase 13/14/15's research. No new audit required.

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Current Source — Read Directly This Session (ground truth for planning)

### 1. `render_block_display`'s current shape (`crates/brokerd/src/confirmation.rs:274-363`)

- Selects a SINGLE display arg: `pc.resolved_args.iter().find(|a| a.taint.iter().any(TaintLabel::is_untrusted)).or_else(|| pc.resolved_args.first())` (lines 306-310).
- **The T-14-08 fail-closed guard is a literal `assert!` macro** (lines 289-304), not `Result::Err`:
  ```rust
  let blocked_count = pc.resolved_args.iter().filter(|a| {
      (executor::sink_sensitivity::is_routing_sensitive(&pc.sink, &a.name)
          || executor::sink_sensitivity::is_content_sensitive(&pc.sink, &a.name))
          && a.taint.iter().any(TaintLabel::is_untrusted)
  }).count();
  assert!(blocked_count <= 1, "render_block_display: genuinely-plural block ...");
  ```
  This re-derives the executor's OWN blocking predicate (`is_routing_sensitive || is_content_sensitive` AND untrusted) from `pc.resolved_args`/`pc.sink` — it does NOT read a stored count. **A Rust `assert!` panics the whole process** (unwinds, and since `confirm`/`deny` run inside a fresh CLI process per invocation — `cli/caprun/src/main.rs`'s `run_confirm_or_deny` — a panic here means the `caprun confirm`/`caprun deny` process aborts with a non-zero/signal exit rather than returning a typed error). `[VERIFIED: direct source read]`
- **Untested today**: no test in `crates/brokerd/src/confirmation.rs`'s `#[cfg(test)] mod tests` (lines 555-1204) or anywhere else in the workspace constructs a `PendingConfirmation` with 2+ genuinely blocked args and calls `render_block_display`/`confirm`/`deny` on it. Both seed helpers (`seed_pending_file_create_block` line 699, `seed_pending_email_send_block` line 934) build exactly ONE `SinkBlockedAnchor` and one matching blocked `ResolvedArg`. 14-02-SUMMARY.md's own coverage table (D5) explicitly flags this as `human_judgment: true`, unexercised. `[VERIFIED: direct source read, cross-checked against 14-02-SUMMARY.md D5]`
- Output format is a `format!` string with fixed fields: `Effect ID`, `Sink`, `Arg`, `Literal value` (quoted, no truncation already), `Taint`, `Source`, `Provenance chain`, plus a fixed trailer telling the human to run `caprun confirm`/`caprun deny`. This exact format is a reasonable template to repeat per-arg for CONFIRM-04, plus a Draft/untrusted-posture statement (DESIGN "MUST (Draft/untrusted-seeded posture is stated at confirm time, D-20 legibility)") which does NOT exist in the current string at all — a genuinely NEW addition, not present even in the single-arg case today.

### 2. `PendingConfirmation`/`ResolvedArg` — `combined_digest` does not exist yet (fully greenfield)

`crates/brokerd/src/confirmation.rs:100-124` (current struct, verbatim):
```rust
pub struct PendingConfirmation {
    pub effect_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub blocked_event_id: uuid::Uuid,
    pub sink: runtime_core::plan_node::SinkId,
    pub resolved_args: Vec<ResolvedArg>,   // the FULL arg set, not just blocked
    pub workspace_root_path: String,
    pub state: PendingConfirmationState,
}
```
No `combined_digest`, no `blocked_arg_names`, no equivalent field anywhere in this struct, in `BlockedArg` (`runtime-core/src/executor_decision.rs:147-158` — deliberately scoped OUT by 14-01, per its own doc comment: "Phase 16 ... layers a `combined_digest` ... binding on top of this collection — that field does NOT live here"), or in `Event` (`event.rs:19-77`, which has `anchors`/`derived_value_id`/`input_value_ids`/`input_provenance_chains`/`transform_kind` but nothing digest-related beyond the existing per-anchor `literal_sha256` inside each `SinkBlockedAnchor`). **CONFIRM-03 is fully greenfield for this phase.** `[VERIFIED: direct source read]`

### 3. Where the combined digest must live — the exact schema-extension question the design doc leaves ambiguous

`DESIGN-confirm-binding.md` (lines 164-172) says the digest MUST be "persist[ed]... INSIDE the hashed `sink_blocked` anchor payload... so the DAG's own hash chain covers the digest." This wording predates the Phase-14 pluralization and reads as if there is ONE anchor to embed it inside — there is not; `SinkBlockedAnchor` is now per-element (`Vec<BlockedArg>`, one anchor per blocked arg). The codebase's own precedent for a WHOLE-EVENT (not per-anchor) payload addition already exists: Phase 15 added `derived_value_id: Option<ValueId>`, `input_value_ids: Vec<ValueId>`, `input_provenance_chains: Vec<Vec<Uuid>>`, `transform_kind: Option<String>` directly to `Event` (not to any per-element sub-struct), each `#[serde(default, skip_serializing_if = ...)]`-gated so non-`derivation` events serialize byte-identically (`event.rs:47-77`). Because `crates/brokerd/src/audit.rs:229` does `let payload = serde_json::to_string(event)?` — serializing the WHOLE `Event` struct, not just `anchors` — **any new field added directly to `Event` automatically rides inside the hashed `payload` column**, satisfying the design doc's tamper-evidence MUST with zero additional plumbing. **Recommendation: add `combined_digest: Option<String>` (and, if the verifier needs it independent of `PendingConfirmation`, `blocked_arg_names: Vec<String>`) directly to `Event`, following the exact `derived_value_id` pattern**, populated only in a `sink_blocked` construction path, mirrored into `PendingConfirmation` for the confirm process to read (matching the design doc's own "mirrored into `PendingConfirmation`... the DAG anchor copy is the tamper-evident source of truth" instruction). This is a plan-level architectural decision this research surfaces explicitly because the design doc's illustrative code does not resolve it. `[VERIFIED: direct source read, event.rs + audit.rs; ASSUMED: this specific field-placement is the correct resolution — the design doc does not name it explicitly, so this is this research's recommendation, not a directly-cited MUST]`

### 4. `server.rs`'s Block-time write — where `combined_digest` must be computed (`crates/brokerd/src/server.rs:452-563`)

The existing flow: on `ExecutorDecision::BlockedPendingConfirmation { anchors }`, `server.rs` builds `Event::sink_blocked(..., anchors.iter().map(|b| b.anchor.clone()).collect())` (line 459), collects `blocked_literals` (arg name + literal pairs, line 461-464), resolves the FULL arg set into `resolved_args: Vec<ResolvedArg>` (lines 493-508, unchanged, already covers every arg not just blocked ones), and constructs `PendingConfirmation` (lines 509-519) — all under ONE mutex-locked write (lines 523-563) that also inserts every blocked literal into the redactable side table and the `pending_confirmations` row atomically. **This is the exact site to compute `combined_digest`**: after the `anchors` collection is known (it already carries each element's `literal` via `BlockedArg.literal` and `anchor.literal_sha256`), compute `SHA-256(anchors[0].anchor.literal_sha256 ++ anchors[1].anchor.literal_sha256 ++ ...)` in `anchors` order (which is already the stable `plan_node.args` iteration order per the executor's collect-then-Block loop), then thread it into BOTH the `Event` (if added per finding #3) and `PendingConfirmation`. No new mutex/lock discipline is needed — this slots into the EXISTING single locked write. `[VERIFIED: direct source read]`

### 5. Single-shot atomicity — current `confirm()`'s handling of a plural set (`crates/brokerd/src/confirmation.rs:374-507`)

`confirm()` already operates on the WHOLE `pc.resolved_args` snapshot atomically — there is no per-arg confirm mechanism, no CLI flag for a subset, and the dispatch to `invoke_file_create_from_resolved`/`invoke_email_smtp_from_resolved` (lines 444-501) always passes the FULL `&pc.resolved_args`, never a filtered subset. **Single-shot-over-the-whole-set (D-17/D-19) is ALREADY satisfied by the existing code shape** — there is no partial-confirm gap to close in the confirm/deny DISPATCH logic itself. The gap this phase closes is narrower: (a) the DISPLAY only shows one arg (finding #1), and (b) there is no digest binding the shown set to what gets sent (finding #2). The `transition_state` SQL guard (`AND state = 'pending'`, line 209) already makes re-confirm/re-deny atomic and terminal — unaffected by pluralization. `[VERIFIED: direct source read]`

### 6. CONTROL-01's live-send question — Allowed decisions do NOT invoke `email.send`'s sink today (`crates/brokerd/src/server.rs:575-595`)

```rust
if matches!(decision, runtime_core::ExecutorDecision::Allowed)
    && plan_node.sink.0 == "file.create"
{
    // ... invoke_file_create(...) ...
}
```
This is the ONLY Allowed-decision sink-invocation site in `SubmitPlanNode`'s handler. There is no `else if plan_node.sink.0 == "email.send"` branch — an Allowed `email.send` plan node currently produces ONLY a `plan_node_evaluated` audit event and NO SMTP send. `invoke_email_smtp_from_resolved` (`crates/brokerd/src/sinks/email_smtp.rs:204`) has exactly ONE call site in the whole workspace — `confirmation.rs::confirm()`'s `"email.send"` match arm (line 486) — which only executes on a CONFIRM decision (i.e., only reachable from a PRIOR Block). **There is no code path today by which a trusted, never-blocked `email.send` plan node reaches the real SMTP adapter.** Phase 15's `s9_live_clean_allow_path` (`cli/caprun/tests/s9_live_block.rs:120-166`) proves ONLY the Allowed DECISION (asserts a `plan_node_evaluated` event exists and NO `sink_blocked` event exists) — it does not invoke or assert any sink execution, and 15-04-SUMMARY.md's own "Framing honesty" section explicitly states this ("`s9_live_clean_allow_path`: asserts the Allowed DECISION only... NOT `sink_executed`/actual Mailpit delivery. 'Actually sends to Mailpit' is a Phase 17 claim"). `[VERIFIED: direct source read + cross-checked against 15-04-SUMMARY.md and 15-VERIFICATION.md]`

**Scope recommendation:** Building new Allowed-path email-sink-invocation wiring this phase would (a) require a new dispatch branch in `server.rs` mirroring the `file.create` one, resolving `plan_node.args` directly (there is no `PendingConfirmation` for an Allowed decision — a NEW resolve-loop, structurally identical to the one already used for the Blocked path at lines 493-508, would be needed), (b) touch `check-invariants.sh` Gate 3's mint-call-site allowlist analysis (a new call site invoking the SMTP sink), and (c) most importantly is explicitly ROADMAP-assigned to Phase 17 ("confirm sends exactly once via the real adapter... composes CONTROL-01 (clean send, ungated) alongside the hostile block" — Phase 17 success criterion 1-2). **Recommendation: Phase 16's CONTROL-01 test should compose the ALREADY-PROVEN Allowed-decision proof (`s9_live_clean_allow_path`) with the hostile-block proof (`s9_live_email_hostile_block`) into ONE test module/run** (satisfying "in the SAME acceptance run" at the decision-proof level, matching Phase 16's own ROADMAP success criterion 4 wording, which says "proceeds with NO block and NO confirm gate" — not "reaches Mailpit"), and defer actual trusted-path SMTP delivery wiring to Phase 17/ACCEPT-01, where the full live A/B against a real Mailpit target is the explicit deliverable. This is presented as a recommendation, not a locked decision — see Open Questions.

### 7. CONTROL-02's existing fixture (`crates/executor/tests/executor_decision.rs#body_tainted_recipient_trusted_blocks`, Phase 14)

Already exists at the EXECUTOR UNIT level (14-01-SUMMARY.md D3: "A tainted body with a trusted `to` still Blocks — CONTROL-02 precursor"). This is a pure `submit_plan_node` unit test — no confined worker, no broker, no live process. Phase 16's CONTROL-02 requirement ("Runs... proving the body dimension isn't dead code") reads most naturally as needing a LIVE analog through the real confined-worker+broker+executor stack, mirroring how `s9_live_email_hostile_block` (Phase 15) is the live analog of the executor-unit-level `collect_then_block_both_to_and_body` test. **Recommendation: add a live fixture** — a doc that carries a genuine multi-fragment recipient-taint-absent path (no `Reply-To:`/`Domain:` markers, so `to` routes to the trusted CLI-arg intent value per `s9_live_clean_allow_path`'s own mechanism) PLUS a `Body:` marker (tainting `body` only), asserting a `sink_blocked` event with EXACTLY ONE anchor (`body`), not `to`. This is a NEW fixture; no existing test drives this exact combination live. `[VERIFIED: direct source read of executor_decision.rs test names via 14-01-SUMMARY.md coverage table; ASSUMED: the live analog is the right shape for this phase's "wire-level" reading — REQUIREMENTS.md's CONTROL-02 text does not explicitly mandate "live," only that it "runs" as a negative control, so an executor-unit-only satisfaction is also textually defensible — flagged as an Open Question]`

### 8. Existing fixture reuse — `hostile_doc.txt` vs. `s9_live_block.rs`'s own constants

`crates/brokerd/tests/fixtures/hostile_doc.txt` (CONFIRM-02's fixture, Phase 15) is a STANDALONE FILE used by `crates/brokerd/tests/extract_provenance_threading.rs`'s DB-alone dispatch-harness tests. `cli/caprun/tests/s9_live_block.rs`'s LIVE tests (`s9_live_clean_allow_path`, `s9_live_email_hostile_block`) instead use INLINE byte-string constants (`CLEAN_PATH_CONTENT`, `HOSTILE_EMAIL_CONTENT`) written to a temp workspace file at test-run time — they do NOT read `hostile_doc.txt` from disk, though `HOSTILE_EMAIL_CONTENT`'s content deliberately MIRRORS `hostile_doc.txt`'s `Reply-To:`/`Domain:`/`Body:` marker shape (per `s9_live_block.rs`'s own doc comment, line 65-66: "mirroring `crates/brokerd/tests/fixtures/hostile_doc.txt`'s CONFIRM-02 shape"). **For Phase 16's CONFIRM-01/04 display test, the natural, lowest-friction choice is to reuse `HOSTILE_EMAIL_CONTENT`'s live-test convention (an inline constant, not a disk fixture)** — it already produces a genuine 2-anchor (`to`+`body`) live Block via `s9_live_email_hostile_block`'s exact run, which is the EXACT scenario CONFIRM-01/04 need to display/narrate. No new doc fixture is needed; the display/narration test can drive `caprun confirm`/`caprun deny` against the SAME `audit.db` `s9_live_email_hostile_block` already produces, or a near-identical sibling test reusing `HOSTILE_EMAIL_CONTENT`. `[VERIFIED: direct source read of both files]`

## Architecture Patterns

### System Architecture Diagram

```
                    ┌───────────────────────────────────────────────┐
                    │   BROKER — SubmitPlanNode handler (server.rs)  │
                    │                                                 │
  PlanNode ────────▶│  executor::submit_plan_node(...)               │
  (to, subject,      │        │                                       │
   body args)        │        ├─ Allowed ──────────────┐              │
                    │        │                          │              │
                    │        └─ BlockedPendingConf. ─┐  │              │
                    │           { anchors: Vec<Blk> } │  │              │
                    │                                  │  │              │
                    │  [NEW] compute combined_digest   │  │              │
                    │  = SHA256(anchors[0].literal_sha256  │              │
                    │           ++ anchors[1].literal_sha256 ++ ...)     │
                    │  in anchors order                │  │              │
                    │                                  │  │              │
                    │  Event::sink_blocked(             │  │              │
                    │    anchors, combined_digest[NEW]) │  │  if sink ==  │
                    │        │                          │  │  "file.create"│
                    │  PendingConfirmation {             │  │  invoke_file_ │
                    │    resolved_args (FULL set),       │  │  create()     │
                    │    blocked_arg_names[NEW],          │  │  (Allowed only)│
                    │    combined_digest[NEW] }           │  │              │
                    │        │                          │  │  [email.send: │
                    │  atomic write (existing lock)      │  │   NO sink call │
                    │        ▼                          │  │   today — see  │
                    └────────┼──────────────────────────┴──┤   finding #6]  │
                             │                                 └──────────────┘
                             ▼
                ┌──────────────────────────────────────┐
                │  SEPARATE, LATER OS process:           │
                │  caprun confirm <effect_id>  /  deny    │
                │  (cli/caprun/src/main.rs)               │
                │                                          │
                │  find_pending_confirmation()             │
                │       │                                  │
                │  [REPLACE] render_block_display()         │
                │    OLD: single-arg-or-panic (T-14-08)      │
                │    NEW: loop over blocked_arg_names,        │
                │         narrate EACH (recipient/body →       │
                │         doc → these bytes → this sink arg)   │
                │       │                                       │
                │  confirm(): [NEW] recompute combined_digest    │
                │    from frozen blocked-subset literals,         │
                │    compare to stored combined_digest,            │
                │    fail-closed on mismatch (never re-resolve      │
                │    a live ValueId)                                 │
                │       │                                              │
                │  dispatch: invoke_{file_create,email_smtp}            │
                │    _from_resolved(&pc.resolved_args) — UNCHANGED,      │
                │    already sends the FULL set (not just blocked)        │
                └──────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

No new files. All changes land in existing modules:
```
crates/runtime-core/src/
├── event.rs                    # + combined_digest: Option<String> field, following derived_value_id pattern
crates/brokerd/src/
├── server.rs                   # + combined_digest computation at Block-time write
├── confirmation.rs             # + blocked_arg_names/combined_digest on PendingConfirmation;
│                                #   render_block_display rewritten for per-arg narration;
│                                #   confirm() gains recompute-and-compare before dispatch
cli/caprun/tests/
├── s9_live_block.rs             # CONTROL-01 composition (existing tests, run together);
│                                 # + CONTROL-02 live fixture
crates/brokerd/tests/            # + new plural-block confirm/deny integration test
```

### Pattern 1: Fixed-Width Per-Element Digest (the mandated combined-digest scheme)
**What:** SHA-256 over the CONCATENATION of each blocked arg's own 64-hex-char `literal_sha256`, taken in blocked-arg order (the stable `plan_node.args`/`anchors` iteration order) — never plain literal concatenation.
**When to use:** The ONLY normative encoding for `combined_digest` per `DESIGN-confirm-binding.md` (plain concatenation is explicitly rejected as partition-blind; length-prefixing is explicitly retracted).
**Example:**
```rust
// Source: pattern mandated by planning-docs/DESIGN-confirm-binding.md
// "Combined-Digest Binding" section, reusing the existing per-element
// literal_sha256 primitive already computed by executor::submit_plan_node
// (crates/executor/src/lib.rs:130-134).
let combined_digest = {
    let mut hasher = Sha256::new();
    for blocked in &anchors {                 // anchors: &[BlockedArg], in order
        hasher.update(blocked.anchor.literal_sha256.as_bytes()); // fixed-width 64-hex-char
    }
    hex::encode(hasher.finalize())
};
```

### Pattern 2: Whole-Event Payload Field Addition (how to add `combined_digest` without breaking golden-byte tests)
**What:** Add a new `#[serde(default, skip_serializing_if = "Option::is_none")]` field directly to `Event`, following the exact precedent Phase 15 set for `derived_value_id`/`input_value_ids`/`input_provenance_chains`/`transform_kind`.
**When to use:** Any time a NEW whole-decision (not per-anchor) payload needs to ride inside the hashed audit chain without a DB migration.
**Example:**
```rust
// Source: crates/runtime-core/src/event.rs:47-59 (existing derived_value_id
// pattern, read directly this session) — combined_digest follows identically.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub combined_digest: Option<String>,
```
Then re-verify the EXISTING golden-byte test (`event.rs`'s `anchors_empty_event_serializes_byte_identical_and_round_trips`, per 14-02-SUMMARY.md) still passes with the new field omitted for non-`sink_blocked` events — mirroring exactly how the Phase 15 fields were verified.

### Pattern 3: Per-Arg Block Narration Loop (replaces the single-arg-or-panic display)
**What:** Iterate `blocked_arg_names` (the ordered subset), looking up each name in `pc.resolved_args`, rendering the SAME per-arg fields the current single-arg format already uses (Effect ID once, then per-arg: Arg name, Literal value verbatim/untruncated, Taint, Source, Provenance chain), plus a NEW Draft/untrusted-posture statement (DESIGN D-20).
**When to use:** Both `confirm()` and `deny()` (both already call `render_block_display` before acting — unchanged discipline).
**Example:**
```rust
// Illustrative — not literal code to paste. Adapts the existing single-arg
// format! block (confirmation.rs:346-362) into a per-arg loop driven by
// pc.blocked_arg_names, closing over pc.resolved_args by name lookup.
for name in &pc.blocked_arg_names {
    let arg = pc.resolved_args.iter().find(|a| &a.name == name)
        .expect("blocked_arg_names must be a subset of resolved_args");
    // render arg.literal (verbatim, NO truncation), arg.taint, arg.provenance_chain
}
// Then append the Draft-posture statement + confirm/deny instructions once.
```

### Anti-Patterns to Avoid
- **Digest over plain concatenation:** `H(to ++ body)` is partition-blind — a boundary-shift attack (`to="a"+body="bc"` vs `to="ab"+body="c"`) produces the SAME hash. Explicitly rejected by the design doc (finding #4). Always hash the FIXED-WIDTH per-element `literal_sha256`s, never raw literals.
- **Recomputing the digest from a live `ValueId`:** The confirm process's `ValueStore` does not exist (separate OS process). ALWAYS recompute from the FROZEN `PendingConfirmation.resolved_args` snapshot, never re-resolve a handle.
- **Re-reading the DB row between compare and send (TOCTOU):** DESIGN-confirm-binding.md's finding #5 — read the frozen snapshot ONCE, compare, then hand the SAME in-memory value to the sink invocation. Do not re-query `pending_confirmations` after the compare.
- **Deleting the T-14-08 `assert!` without first testing it:** The coordinator's explicit mandate — write a test that drives the CURRENT panic path from a genuine 2-anchor fixture FIRST (proving it fires), THEN replace the guard body with real narration, so the change is a verified behavior replacement, not a silent deletion of untested code.
- **Building new Allowed-path sink-invocation wiring for `email.send` to satisfy CONTROL-01:** This is Phase 17/ACCEPT-01's explicit job per ROADMAP.md. Doing it in Phase 16 risks scope creep and touches `check-invariants.sh` Gate 3's mint-call-site allowlist without that gate's own review pass. See Open Question 1.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Combined digest over a set of literals | A custom multi-value hash scheme (e.g. HMAC, keyed hash, or a bespoke serialization format) | SHA-256 over fixed-width per-element `literal_sha256`s, in order (Pattern 1 above) | DESIGN-confirm-binding.md explicitly mandates reusing the EXISTING `sha2`/`hex` primitive with NO new hash primitive — this is a locked design decision, not a style preference |
| Tamper-evidence for the digest | A separate signature/HMAC scheme, or storing the digest in a NON-hashed column | Add the field directly to `Event` so it rides inside the existing `serde_json::to_string(event)` → SHA-256 hash-chain payload (Pattern 2) | The audit DAG's hash chain ALREADY provides tamper-evidence for anything inside `Event`'s serialized payload — building a parallel integrity mechanism would duplicate existing, already-reviewed infrastructure |
| Multi-arg display formatting | A templating library or generic pretty-printer | The existing `format!` string convention (per-field labels, `\"{literal}\"` quoting, `short_evt`/`taint_label_display` helpers already in `confirmation.rs`) | This is a small, fixed, human-facing CLI output — the project has no other CLI templating dependency, and introducing one for ~10 lines of repeated-per-arg text is unwarranted complexity |

**Key insight:** Every artifact this phase needs (hash primitive, tamper-evidence mechanism, display formatting convention) already exists in the codebase for the single-arg case. The work is generalizing existing patterns to N elements, not introducing new infrastructure.

## Common Pitfalls

### Pitfall 1: Treating the design doc's "inside the hashed anchor payload" wording as meaning "inside `SinkBlockedAnchor`"
**What goes wrong:** A plan that tries to add `combined_digest` as a field on `SinkBlockedAnchor` (duplicated per element) rather than once on `Event`.
**Why it happens:** The design doc's illustrative code (written before/without full awareness of Phase 14's pluralization) speaks of "the anchor" singular.
**How to avoid:** Add the field to `Event` directly (finding #3/Pattern 2), matching the Phase-15 precedent for whole-decision payload fields.
**Warning signs:** A diff that touches `SinkBlockedAnchor`'s struct definition to add a digest field, or that computes N identical copies of the same digest.

### Pitfall 2: Deleting the T-14-08 `assert!` without a regression test first
**What goes wrong:** `render_block_display`'s panic-on-plural-block guard is silently replaced by narration code with no verification that the OLD guard ever actually fired correctly — losing the one coverage opportunity 14-02 explicitly deferred.
**Why it happens:** It's tempting to treat "replace the placeholder" as a pure rewrite, skipping the "prove the thing I'm replacing worked" step.
**How to avoid:** Add a test using a genuinely-2-anchor `PendingConfirmation` fixture (neither existing seed helper has one) that calls `render_block_display` (or `confirm`/`deny`) against the CURRENT code and asserts a panic (e.g. via `std::panic::catch_unwind` or `#[should_panic]`), THEN in a subsequent commit replace the guard body with real narration and update that same test to assert the new narrated output instead.
**Warning signs:** A plan step that says "replace `render_block_display`" with no preceding step that exercises the current panic path.

### Pitfall 3: Recomputing the combined digest from a live `ValueStore`/re-resolved `ValueId`
**What goes wrong:** `confirm()` runs in a separate, later OS process — the `ValueStore` that resolved the original handles does not exist there. Any code path that tries to `value_store.resolve(...)` inside `confirm()` will not compile (no `ValueStore` is threaded into `confirm()`'s signature today) or, worse, would require re-plumbing a live store into the confirm CLI path, reopening the exact problem `DESIGN-confirmation-release.md` already closed for `resolved_args`.
**Why it happens:** "Recompute and compare" sounds like it implies re-deriving from the original source.
**How to avoid:** Recompute ONLY from `pc.resolved_args` (the frozen snapshot already read from SQLite) — the same data `render_block_display` already reads.
**Warning signs:** A new parameter threading a `ValueStore` or `ValueId` resolution into `confirm()`'s signature.

### Pitfall 4: Assuming `s9_live_clean_allow_path` already proves CONTROL-01's "send" claim
**What goes wrong:** A plan or test that asserts CONTROL-01 is "already done" because Phase 15 built a passing test with that name, without checking that the test only asserts the Allowed DECISION, not an actual SMTP send.
**Why it happens:** The test name ("clean_allow_path") sounds send-adjacent, and 15-04-SUMMARY.md is easy to skim past the explicit "Framing honesty" section that says otherwise.
**How to avoid:** Read `s9_live_clean_allow_path`'s actual assertions (finding #6 above) — it checks `intent_received`/`plan_node_evaluated`/absence of `sink_blocked`, nothing about `sink_executed` or an SMTP transcript.
**Warning signs:** A plan task description that says "CONTROL-01 is already proven, just add the confirm/deny gate assertions" without separately confirming whether actual delivery is in scope.

### Pitfall 5: CONTROL-02's live fixture accidentally re-taints `to` too
**What goes wrong:** A doc content string meant to taint ONLY `body` accidentally also contains a `Reply-To:`/`Domain:` pair (even coincidentally), causing the worker's extractor to derive a tainted `to` as well — turning the intended one-anchor (`body`-only) Block into a two-anchor Block, which would not prove CONTROL-02's claim (recipient must be TRUSTED, not merely absent-from-blocking).
**Why it happens:** Copy-pasting from `HOSTILE_EMAIL_CONTENT` (which deliberately has BOTH markers) without removing the `Reply-To:`/`Domain:` lines.
**How to avoid:** Model the CONTROL-02 fixture on `CLEAN_PATH_CONTENT`'s recipient side (no markers, so `to` falls through to the trusted CLI-arg intent value) PLUS a `Body:` marker only (mirroring `HOSTILE_EMAIL_CONTENT`'s body-marker mechanism). Assert the resulting `sink_blocked` event's `anchors` contains EXACTLY `["body"]`, not `["body","to"]`.
**Warning signs:** A CONTROL-02 test whose `sink_blocked.anchors` assertion accepts (or doesn't check) more than one arg name.

## Code Examples

### Existing single-arg `literal_sha256` pattern to reuse verbatim (per-element digest input)
```rust
// Source: crates/executor/src/lib.rs:130-134 (existing code, read directly)
let literal_sha256 = {
    let mut hasher = Sha256::new();
    hasher.update(record.literal.as_bytes());
    hex::encode(hasher.finalize())
};
```

### Existing whole-Event additive-field pattern (the template for `combined_digest`)
```rust
// Source: crates/runtime-core/src/event.rs:58-59 (existing code, read directly)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub derived_value_id: Option<ValueId>,
```

### Existing Block-time atomic write to mirror (where combined_digest computation slots in)
```rust
// Source: crates/brokerd/src/server.rs:523-563 (existing code, structure only —
// read the file directly for exact variable names before editing)
let new_hash = {
    let locked = conn.lock()...;
    let hash = append_event(&locked, &audit_event, Some(last_event_hash))...;
    for (arg, literal) in &blocked_literals {
        crate::audit::insert_blocked_literal(&locked, &audit_event.id.to_string(), arg, literal)...;
    }
    if let Some(pc) = &pending_confirmation {
        crate::confirmation::insert_pending_confirmation(&locked, pc)...;
    }
    hash
};
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `render_block_display` shows exactly ONE arg, panics on plural | Per-arg narration loop over `blocked_arg_names` | This phase (CONFIRM-04) | The Block moment becomes the demo's climax per ROADMAP framing — every blocked arg's provenance is shown, not just one |
| No confirm-binding digest at all | `combined_digest` field, recompute-and-compared before every confirmed send | This phase (CONFIRM-03) | Closes the "human confirms recipient, body ships unconfirmed" B1-reincarnation risk at the binding layer |
| `email.send` sink only invoked from the confirm path | (Unchanged this phase — see Open Question 1) | N/A this phase; Phase 17 owns this | CONTROL-01's "actual send" claim, if desired, is deferred |

**Deprecated/outdated:** The T-14-08 `assert!` guard is deliberately temporary scaffolding (14-02-SUMMARY.md calls it "the seam Phase 16 must replace") — it should not survive this phase in its current panic form, but MUST be tested before removal (Pitfall 2).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `combined_digest`/`blocked_arg_names` belong on `Event` directly (mirroring `derived_value_id`), not on `SinkBlockedAnchor` or as a wrapper struct | "Current Source" finding #3, Pattern 2 | If the planner instead threads the digest through `SinkBlockedAnchor` (duplicated N times), the code still satisfies the design doc's literal MUST ("inside the hashed anchor payload") but with redundant per-element copies — a style/efficiency cost, not a correctness one, but worth confirming before implementation since it affects the `Event::sink_blocked` constructor's signature |
| A2 | Phase 16's CONTROL-01 should stay at the Allowed-decision-proof level (composing existing tests) rather than adding new Allowed-path SMTP-invocation wiring | "Current Source" finding #6 | If the coordinator/planner instead wants ACTUAL Mailpit delivery proven in Phase 16 (not deferred to Phase 17), new dispatch wiring in `server.rs` + a `check-invariants.sh` Gate 3 review is required — a materially larger scope than this research assumes. This is the single highest-leverage open question in this research. |
| A3 | CONTROL-02 needs a NEW LIVE (confined-worker+broker+executor) fixture, not merely reuse of the existing executor-unit-level `body_tainted_recipient_trusted_blocks` test | "Current Source" finding #7 | REQUIREMENTS.md's CONTROL-02 wording doesn't explicitly say "live" — if the unit-level test is judged sufficient, this phase's CONTROL-02 work shrinks to a documentation/traceability update rather than new test code |

**If this table is empty:** N/A — see rows above; all three merit explicit confirmation before planning locks in scope.

## Open Questions

1. **Does Phase 16's CONTROL-01 need to prove an actual completed SMTP send, or is the Allowed-decision proof sufficient?**
   - What we know: The current codebase has NO wiring to invoke `email.send`'s sink on an Allowed decision (finding #6). Phase 15 already built and Linux-verified the Allowed-decision proof (`s9_live_clean_allow_path`). ROADMAP.md's Phase 17 (ACCEPT-01) explicitly claims "confirm sends exactly once via the real adapter... composes CONTROL-01 (clean send, ungated) alongside the hostile block" — suggesting the FULL live send-and-deliver composition is Phase 17's job, while Phase 16's own success criterion 4 wording ("proceeds with NO block and NO confirm gate... in the same acceptance run as the hostile block") is satisfiable at the decision level.
   - What's unclear: Whether "same acceptance run" in Phase 16's own ROADMAP wording is satisfied by two tests in the same `cargo test` invocation/module, or requires literally the SAME `caprun` process run (one workspace, one audit.db, one session) proving BOTH the hostile block and the clean allow in a single execution.
   - Recommendation: Default to composing the two EXISTING proofs (decision-level) in Phase 16, explicitly deferring "one process, one session, both outcomes, real Mailpit delivery" to Phase 17/ACCEPT-01 — this matches the phases' respective ROADMAP requirement lists (CONTROL-01 → Phase 16; ACCEPT-01, which literally names "composes... in the SAME run," → Phase 17) and avoids Phase 16 doing Phase 17's SMTP-wiring work twice. Flag this explicitly for `/gsd-discuss-phase` or plan-review sign-off before implementation, since it is the single highest-uncertainty scope call in this phase.

2. **Where exactly does `blocked_arg_names` come from, and must it be independently persisted, or can it be re-derived?**
   - What we know: `render_block_display`'s CURRENT plurality guard already re-derives the blocked set from `pc.resolved_args`/`pc.sink` via `is_routing_sensitive || is_content_sensitive` + untrusted-taint, with ZERO new persisted field (14-02's own decision, made explicitly to avoid a schema/struct ripple). The design doc's illustrative `PendingConfirmation` shape (lines 138-155) suggests `blocked_arg_names: Vec<String>` as an explicit NEW persisted field, for the verifier to filter `resolved_args` by "recorded arg-name+order."
   - What's unclear: Whether re-deriving the blocked set (same predicate, no new field, matching 14-02's existing philosophy) is sufficient for CONFIRM-03's verifier-reproducibility MUST, or whether the design doc's explicit persisted-field recommendation should be followed literally for auditability (a human/tool re-deriving the digest independently, without re-running the executor's sensitivity predicate, needs the ORDERED NAMES persisted somewhere).
   - Recommendation: Persist `blocked_arg_names: Vec<String>` explicitly (following the design doc literally) rather than re-deriving via the predicate a second time — CONFIRM-03's own text says "PendingConfirmation MUST therefore persist enough to identify that subset deterministically," which reads as a MUST for an explicit field, not an implicit re-derivation. This also sidesteps a subtle re-derivation risk: if `sink_sensitivity.rs`'s classification rules ever change in a LATER phase, re-deriving the blocked set from an OLD `PendingConfirmation` row using the NEW predicate could silently disagree with what was actually blocked at write time.

## Environment Availability

No new external dependency or service. `sha2`/`hex`/`serde_json` are already vendored and in active use; no new CLI tool, database, or runtime is introduced by this phase. Skipping this section's tabular form — nothing to audit beyond what Phase 13/14/15's research already covered.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (built-in), no external test framework |
| Config file | none — standard `#[test]`/`#[cfg(test)]` conventions, workspace `Cargo.toml` |
| Quick run command | `cargo test -p brokerd confirmation` (unit-level, macOS-runnable) |
| Full suite command | `cargo test --workspace --no-fail-fast` (macOS: Linux-gated tests report 0 passed, expected); Linux verification via `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast`, or `scripts/mailpit-verify.sh` for anything touching `email_smtp.rs` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONFIRM-01 | Verbatim recipient+body+provenance display for a doc-derived 2-arg block | integration (macOS-runnable, brokerd unit-level using DB-alone fixtures) | `cargo test -p brokerd --lib confirmation` | ❌ Wave — new test needed (no existing 2-anchor `PendingConfirmation` fixture) |
| CONFIRM-03 | Combined digest binds the whole blocked set; recompute-and-compare fails closed on tamper | unit | `cargo test -p brokerd --lib confirmation` | ❌ Wave — new test needed |
| CONFIRM-04 | Block-moment narration for every blocked arg | unit + regression-first (T-14-08 panic proof) | `cargo test -p brokerd --lib confirmation` | ❌ Wave — new test needed (regression test for the CURRENT panic path FIRST, per Pitfall 2) |
| CONTROL-01 | Trusted send Allowed, no block/confirm, same run as hostile block | e2e (Linux-gated) | `cargo test -p caprun --test s9_live_block` (existing tests composed/co-run) | ✅ mostly — `s9_live_clean_allow_path`/`s9_live_email_hostile_block` both exist; composition into "same run" is new |
| CONTROL-02 | Body tainted + recipient trusted still blocks | unit (exists) + e2e (new, live analog) | unit: `cargo test -p executor body_tainted_recipient_trusted_blocks`; e2e: `cargo test -p caprun --test s9_live_block` (new fn) | Unit ✅ exists; e2e ❌ Wave — new test needed |

### Sampling Rate
- **Per task commit:** `cargo test -p brokerd --lib confirmation` (fast, macOS-runnable, covers CONFIRM-01/03/04's new unit tests)
- **Per wave merge:** `cargo test --workspace --no-fail-fast` on macOS, THEN the Colima/Docker Linux run (or `scripts/mailpit-verify.sh` if `email_smtp.rs` was touched) for anything touching `s9_live_block.rs`/CONTROL-01/CONTROL-02's live fixtures
- **Phase gate:** Full Linux-verified suite green before `/gsd-verify-work`, per this project's standing CLAUDE.md discipline

### Wave 0 Gaps
- [ ] A genuinely-2-anchor `PendingConfirmation` seed helper in `crates/brokerd/src/confirmation.rs`'s test module (neither existing seed helper — `seed_pending_file_create_block`, `seed_pending_email_send_block` — constructs more than one blocked arg)
- [ ] A regression test proving the CURRENT `render_block_display` `assert!` panics on a genuine 2-anchor input, added BEFORE the guard is replaced (Pitfall 2)
- [ ] A live CONTROL-02 fixture in `cli/caprun/tests/s9_live_block.rs` (body-tainted-only, recipient-trusted) — does not exist today

*(No framework install needed — `cargo test` infrastructure is fully in place.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | No | This project has no user-authentication surface — `caprun confirm <effect_id>` authorization IS the security model (possession of a valid, unconsumed `effect_id` + local process access), not a login system |
| V3 Session Management | Partially | `SessionStatus`/`PendingConfirmationState` are the project's own session/state-machine primitives (`Pending → Confirmed`/`Denied`, one-way, SQL-enforced terminal check) — already implemented, unchanged by this phase |
| V4 Access Control | Yes | The `AND state = 'pending'` SQL guard (`transition_state`) is this project's access-control primitive for "can this effect still be confirmed" — unchanged by this phase; this phase's NEW recompute-and-compare is an ADDITIONAL integrity check layered on top, not a replacement |
| V5 Input Validation | Yes | `lettre::Address`'s typed parser (existing, `email_smtp.rs:113-118`) is this project's CRLF/header-injection defense — unchanged by this phase. The NEW combined-digest recompute-and-compare is itself a form of input (snapshot) validation before an irreversible effect |
| V6 Cryptography | Yes | SHA-256 (via `sha2`) is the project's sole hash primitive, used here for INTEGRITY (tamper-evidence), not confidentiality or authentication — reuse of the existing primitive is mandated by the design doc, never introduce a new one (see Don't Hand-Roll) |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|-----------------------|
| Partial confirm — human confirms recipient, unconfirmed body ships anyway (B1-reincarnation) | Tampering / Elevation of Privilege | Combined digest over the WHOLE blocked-arg set, single-shot atomic confirm/deny (this phase's core deliverable) |
| Boundary-shift bypass — shifting bytes between `to`/`body` produces the same plain-concatenation hash | Tampering | Fixed-width per-element `literal_sha256` digest input, never plain concatenation (Pattern 1, DESIGN finding #4) |
| TOCTOU — literals differ between "what was compared" and "what is sent" | Tampering | Same in-memory snapshot read for compare AND send, no DB re-read in between (DESIGN finding #5, Pitfall 3) |
| Truncated display — human confirms bytes they never actually saw | Repudiation | Verbatim, no-truncation display of every blocked arg's literal (CONFIRM-01/03's explicit MUST) |
| CRLF/header injection via a confirmed body | Tampering | UNCHANGED from Phase 13 — `lettre::Address` typed parsing, already in place, this phase does not touch it |

## Sources

### Primary (HIGH confidence — direct source read this session)
- `crates/brokerd/src/confirmation.rs` — `render_block_display`, `PendingConfirmation`/`ResolvedArg`, `confirm()`/`deny()`, existing test module (lines 1-1204, read in full)
- `crates/brokerd/src/server.rs` — `SubmitPlanNode` handler (lines 400-600, read directly), `ProvideIntent` handler (lines 700-760)
- `crates/runtime-core/src/executor_decision.rs` — `BlockedArg`, `SinkBlockedAnchor`, `ExecutorDecision` (full file read)
- `crates/runtime-core/src/event.rs` — `Event` struct, `sink_blocked`/`derivation` constructors (lines 1-145 read directly)
- `crates/executor/src/lib.rs` — `submit_plan_node`'s collect-then-Block loop (full file read)
- `crates/executor/src/sink_sensitivity.rs` — routing/content sensitivity classification (full file read)
- `crates/brokerd/src/sinks/email_smtp.rs` — `invoke_email_smtp_from_resolved` and module doc comment (lines 1-120 read directly, call-site grep confirmed single call site)
- `cli/caprun/src/main.rs` — `run_confirm_or_deny` (lines 300-405 read directly)
- `cli/caprun/tests/s9_live_block.rs` — `s9_live_clean_allow_path`, `s9_live_email_hostile_block` (lines 1-260 read directly)
- `crates/brokerd/tests/fixtures/hostile_doc.txt` — CONFIRM-02 fixture (read in full)
- `planning-docs/DESIGN-confirm-binding.md` — full document read
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md` — full documents read
- `.planning/phases/14-*/14-01-SUMMARY.md`, `14-02-SUMMARY.md`, `14-PATTERNS.md` — read in full
- `.planning/phases/15-*/15-01-SUMMARY.md`, `15-04-SUMMARY.md`, `15-VERIFICATION.md` — read in full

### Secondary (MEDIUM confidence)
- None — all findings in this document trace to a direct source read or a directly-quoted design-doc MUST; no WebSearch or external documentation was needed (this is a pure internal-codebase extension, no new library).

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependency; existing `sha2`/`hex`/`serde_json` usage confirmed by direct source read
- Architecture: HIGH — every claim about current code shape (render_block_display's assert, PendingConfirmation's fields, server.rs's Allowed-decision dispatch) is a direct file:line citation from this session's reads
- Pitfalls: HIGH — each pitfall is grounded in a specific, cited current-code behavior (the T-14-08 assert, the single email.send call site, the existing atomic-write structure), not speculation
- Open Questions (CONTROL-01 scope, blocked_arg_names persistence): MEDIUM — these are genuine, unresolved scope calls the design doc and ROADMAP leave ambiguous; this research states a recommendation but flags both explicitly for planner/discuss-phase confirmation before locking in

**Research date:** 2026-07-08
**Valid until:** No external time pressure (internal codebase only, no third-party API/library drift risk) — valid until the next phase (17) or a design-doc revision changes the `PendingConfirmation`/`Event` schema this research is grounded in.
