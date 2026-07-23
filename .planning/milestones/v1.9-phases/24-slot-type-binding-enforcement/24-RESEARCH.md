# Phase 24: Slot-Type Binding Enforcement - Research

**Researched:** 2026-07-12
**Domain:** Rust TCB security enforcement — mechanical realization of a locked design (origin-role tagging, exhaustive enum extension, executor per-arg deny gate)
**Confidence:** HIGH (every claim below is a direct code read of this repo, taken 2026-07-12, independently re-verified against `planning-docs/DESIGN-slot-type-binding.md` and Phase 23's research — code is unchanged since Phase 23, confirmed via `git log --oneline -5 -- crates/ cli/` and `git status --porcelain crates/ cli/` both showing no TCB code touched since the design gate cleared)

## Summary

Phase 24 is a **mechanical realization** of `planning-docs/DESIGN-slot-type-binding.md` (Phase 23's cleared, adversarially-reviewed design gate). Nothing in this phase is a design decision — every ruling (tag shape, role vocabulary, expected-role table contents, `DenyReason` variant shape, ordering, fail-closed default) is already pinned in the DESIGN doc and MUST be followed verbatim, not re-derived. This research's job is to (a) independently re-confirm every file:line citation the DESIGN doc relies on still holds (it does — code is byte-identical to what Phase 23 read), and (b) surface implementation-mechanics the DESIGN doc states at the "what" level but not the exhaustive "every call site" level, because T2-02's mandate ("additive... signature change... call sites") has a much larger blast radius than the DESIGN doc's illustrative examples show.

**The one thing the DESIGN doc does not spell out explicitly, verified here:** `ValueRecord` has exactly ONE production constructor — `ValueStore::mint()` (`crates/executor/src/value_store.rs:61-82`). None of the three `mint_from_*` functions constructs a `ValueRecord` directly; each delegates to `store.mint(literal, taint, provenance_chain)`. Therefore `origin_role` must be threaded as a **fourth parameter into `ValueStore::mint` itself**, not merely into the three `mint_from_*` wrapper signatures — otherwise the field has no path onto the record. This is a legal, sanctioned edit (`value_store.rs` is one of Gate 3's three allowed loci for the `.mint(` token) but it means the blast radius quoted in the DESIGN doc's §9 ("three mint sites") undersells the actual signature-threading work: it is three wrapper signatures **plus** `ValueStore::mint`'s signature **plus** every direct call site of all four.

**Primary recommendation:** Implement exactly what `DESIGN-slot-type-binding.md` §1–§7 pins, in this order: (1) add `origin_role: Option<String>` to `ValueRecord` with `#[serde(default)]` (F6) and to `ValueStore::mint`'s parameter list; (2) thread the role string through all three `mint_from_*` functions and their ~9 production call sites (5 in `server.rs`, plus the 3 function bodies' own `.mint()` calls) using the exact vocabulary in DESIGN §2's table, selecting `"recipient"` vs `"path"` **inside** the existing `server.rs:1294-1300` intent-variant match (per Round-1 finding F3 — never hardcode at the shared `:1317` call site); (3) update all ~63 test call sites across 8 files (mechanical, `None` is a legal `origin_role` value for any test that doesn't care about the new check, but tests exercising role-checked slots need an explicit `Some(...)`); (4) add `expected_role()` to `sink_sensitivity.rs` mirroring `is_routing_sensitive`'s shape exactly, with DESIGN §3's pinned table; (5) add `DenyReason::SlotTypeMismatch { sink: String, arg: String, expected: Vec<String>, found: Option<String> }` and update the two (independently re-confirmed) exhaustive matches in `executor_decision.rs`; (6) insert Step 1c in `submit_plan_node` between the existing Step 1b (empty-provenance) and Step 2/3 (sensitivity collect), returning hard `Denied` on first mismatch — zero reordering of the existing I0/I2 precedence.

## Architectural Responsibility Map

Single-tier phase — Rust TCB backend enforcement only (no browser/frontend/CDN involvement), identical framing to Phase 23's research.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Origin-role tag threading at mint time | API/Backend (`crates/brokerd` dispatch + `crates/executor::value_store`) | — | Broker dispatch (`server.rs`) selects the role string; `ValueStore::mint` (executor crate) is the sole place it lands on the record. |
| Expected-role table lookup | API/Backend (`crates/executor::sink_sensitivity`) | — | Hardcoded Rust TCB, mirrors `is_routing_sensitive`/`is_content_sensitive`. |
| `DenyReason` variant + 2 exhaustive matches | API/Backend (`crates/runtime-core::executor_decision`) | — | Sole owner of the type; both match sites co-located with the definition. |
| Step 1c enforcement | API/Backend (`crates/executor::submit_plan_node`) | — | Same function that already enforces I0 (Step 0.5) and I2 (Steps 2/3). |
| Test-fixture threading | API/Backend (test-only, `#[cfg(test)]`/`tests/*.rs`, all Gate-3-exempt) | — | ~63 mechanical call-site updates; no new production logic. |

## Project Constraints (from CLAUDE.md)

- TCB is Rust; the executor/`sink_sensitivity.rs`-style hardcoded match is mandatory — no config file, no pluggable framework (explicitly out of scope per REQUIREMENTS.md and DESIGN §0).
- Terminology locked: `Intent`/`Session`/`Planner`/`Worker`/`Broker`/`Adapter`/`Effect`/`Artifact`/`Event`. `ExecutionContext` never in public API. Not implicated by this phase's additive field.
- Effect path is locked to `PlanNode { sink, args: Vec<ValueNode> }` (in practice `Vec<PlanArg>` resolving through `ValueStore`); `check-invariants.sh` Gate 1 forbids the token `EffectRequest` anywhere under `crates/` — not implicated here (no new effect-dispatch shape is introduced; `expected_role()` returns a lookup value, never an args-map).
- `check-invariants.sh` Gate 3 restricts `mint_from_read(`, `mint_from_derivation(`, and `.mint(` call-site TOKENS to `crates/brokerd/src/quarantine.rs`, `crates/brokerd/src/server.rs`, and `crates/executor/src/value_store.rs` (production code only — files under any `tests/` directory and any `#[cfg(test)]` module in a source file are exempt). **Independently re-verified below (see Existing-Code Map §4): this remains exactly true today, no drift since Phase 23.** Signature changes at these three loci are safe; a NEW call site anywhere else fails the build.
- Design-gate discipline already satisfied: `planning-docs/DESIGN-slot-type-binding.md` cleared a fresh non-self adversarial review (`planning-docs/DESIGN-GATE-RECORD-v1.5.md`, status CLEARED) on 2026-07-11 — Phase 24 is UNBLOCKED and MUST implement its pinned rulings verbatim, not re-open them.
- Out of scope for this milestone (v1.5): any I0/I1 trust-classification change, a general/pluggable role framework, sinks beyond `email.send`/`file.create`, T2-06/07/08 (held-out acceptance test, regression audit, final `mailpit-verify.sh` re-run — all Phase 25).
- Source of truth on any doc conflict: `planning-docs/PLAN.md`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| T2-02 | Each minted value carries a semantic origin-role tag, populated at the three mint call sites via an additive signature change; I0/I1 trust classification unaffected | Existing-Code Map §1–§3 (exact current signatures, ALL production + test call sites enumerated); DESIGN §1/§2/§4 (tag mechanism + vocabulary, already locked) |
| T2-03 | A hardcoded per-sink-arg expected-role table exists in `crates/executor`, mirroring `sink_sensitivity.rs` CONTENT-01/02, scoped to `email.send`/`file.create` | Existing-Code Map §5 (full current `sink_sensitivity.rs` content to mirror); DESIGN §3 (exact table pinned) |
| T2-04 | A new exhaustive `DenyReason` variant with no wildcard arm; every existing exhaustive match over `DenyReason` across the workspace updated | Existing-Code Map §4 (independently re-run grep, confirms exactly 2 match sites, both in `executor_decision.rs`); DESIGN §5 (variant shape, `Vec<String>` not `&'static [&'static str]` per Round-1 F1) |
| T2-05 | `submit_plan_node` denies a plan node when a resolved value's origin role doesn't match its slot's expected role, evaluated per-arg in the same pass as routing/content-sensitivity, without weakening/reordering I0/I2 precedence | Existing-Code Map §6 (full current `submit_plan_node` step order, byte-identical to Phase 23's citations); DESIGN §6/§7 (Step 1c placement + fail-closed contract, already locked) |
</phase_requirements>

## Existing-Code Map

All citations below were read directly from the repository on 2026-07-12, after confirming `git status --porcelain crates/ cli/` is empty and the most recent commit touching `crates/`/`cli/` predates the Phase 23 design-gate close — i.e. this is the exact code state the DESIGN doc was authored against.

### 1. `ValueRecord` — the field to add

`crates/runtime-core/src/value_record.rs:20-31`:
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueRecord {
    pub id: ValueId,
    pub literal: String,
    pub taint: Vec<TaintLabel>,
    pub provenance_chain: Vec<uuid::Uuid>,
}
```
No `#[derive(Default)]`. **Add** `#[serde(default)] pub origin_role: Option<String>` as a fifth field (per DESIGN §1 and Round-1 finding F6). Because there is no `Default` impl, **every direct struct-literal construction of `ValueRecord` anywhere in the workspace will fail to compile until updated** — confirmed there are exactly 3 such direct-literal sites (all test-only, none production):
- `crates/executor/src/value_store.rs:74-79` — inside `ValueStore::mint` itself (the ONE production constructor).
- `crates/brokerd/src/quarantine.rs:1589-1594` and `:1595-1600` — inside `quarantine.rs`'s own `#[cfg(test)] mod tests` (hand-constructed `mint_from_derivation` input fixtures, deliberately bypassing `mint_from_read` to control provenance-chain overlap for a dedup test).
- `crates/runtime-core/tests/types_compile.rs:42-47` — a serde/shape compile-check integration test.

`ValueRecord` has no other struct-literal construction site in the workspace (verified via `grep -rn "ValueRecord {" crates/ cli/`).

### 2. The three mint wrapper functions — exact current signatures

All in `crates/brokerd/src/quarantine.rs`:

| Function | Current signature | Line |
|---|---|---|
| `mint_from_read` | `(conn: &rusqlite::Connection, store: &mut ValueStore, session_id: Uuid, claim: &Claim, parent_id: Option<Uuid>, parent_hash: Option<&str>) -> Result<(Uuid, String, ValueId, Uuid, String)>` | `:284-297` |
| `mint_from_intent` | `(conn: &rusqlite::Connection, store: &mut ValueStore, session_id: Uuid, literal: String, parent_id: Option<Uuid>, parent_hash: Option<&str>) -> Result<(Uuid, String, ValueId)>` | `:435-442` |
| `mint_from_derivation` | `(conn: &rusqlite::Connection, store: &mut ValueStore, session_id: Uuid, transformed_literal: String, inputs: &[&ValueRecord], transform_kind: &str, parent_id: Option<Uuid>, parent_hash: Option<&str>) -> Result<(Uuid, String, ValueId)>` (`#[allow(clippy::too_many_arguments)]` already present) | `:574-583` |

Each delegates its actual record-construction to `store.mint(literal, taint, provenance_chain)`:
- `mint_from_read` → `:365`, `mint_from_intent` → `:470`, `mint_from_derivation` → `:703`.

**`ValueStore::mint`'s current signature** (`crates/executor/src/value_store.rs:61-66`):
```rust
pub fn mint(
    &mut self,
    literal: String,
    taint: Vec<TaintLabel>,
    provenance_chain: Vec<uuid::Uuid>,
) -> Result<ValueId, MintInvariantError>
```
This is the actual record-writer (constructs the `ValueRecord {}` literal at `:74-79`). **This signature must also gain the `origin_role: Option<String>` parameter** — it is not one of the DESIGN doc's named "three mint sites" but it is the only production code path that ever builds a `ValueRecord`, so the tag cannot reach the record without passing through here. This edit is Gate-3-legal (`value_store.rs` is a sanctioned `.mint(` locus).

### 3. Every call site of the three `mint_from_*` functions and of `.mint(` directly

Independently re-run 2026-07-12 (`grep -rn "mint_from_read(\|mint_from_intent(\|mint_from_derivation(" crates/ cli/ --include="*.rs" | grep -v /target/`):

| File | Call-site count | Kind |
|---|---|---|
| `crates/brokerd/src/quarantine.rs` | 47 (incl. the 3 function definitions themselves) | 3 defs + ~44 test call sites inside its own `#[cfg(test)] mod tests` |
| `crates/brokerd/src/server.rs` | 5 | PRODUCTION — the sole dispatch call sites (see §7 below) |
| `crates/brokerd/tests/extract_provenance_threading.rs` | 5 | test (Cargo integration binary, Gate-3-exempt) |
| `crates/brokerd/tests/s9_acceptance.rs` | 3 | test |
| `crates/brokerd/tests/durable_anchor.rs` | 1 | test |
| `crates/brokerd/tests/phase5_dispatch.rs` | 4 | test |
| `cli/caprun/tests/live_acceptance_v1_3.rs` | 1 | test |

Direct `.mint(` calls (excluding the `mint_from_*` internal delegations already counted above), independently re-run (`grep -rn "\.mint(" crates/ cli/ | grep -v mint_from`):
- `crates/brokerd/src/quarantine.rs:365,470,703` — inside the 3 `mint_from_*` function bodies (production; these gain the new parameter as part of threading through).
- `crates/brokerd/tests/s9_acceptance.rs:433`, `crates/brokerd/tests/durable_anchor.rs:128` — direct `ValueStore::mint` calls from tests (Gate-3-exempt, under `tests/`).
- `crates/brokerd/src/sinks/file_create.rs:255,258` — **inside that file's own `#[cfg(test)] mod tests` block** (starts `:223`), building a fixture `PlanNode` for `invoke_file_create` unit tests; Gate-3-exempt, not a 4th production mint locus.
- `crates/executor/src/value_store.rs:111,146,158,179` — inside `ValueStore`'s own `#[cfg(test)] mod tests`.
- `crates/executor/tests/executor_decision.rs` — 16 separate `.mint(` call sites (lines 72,133,184,215,222,250,286,314,340,347,415,441,448,494,501,559,566) — the single largest test-fixture blast radius outside `quarantine.rs`, all Gate-3-exempt (`tests/` dir).

**Total: this phase's mechanical threading touches ~9 production call sites (5 in `server.rs` + 3 `mint_from_*` bodies' `.mint()` delegation + `ValueStore::mint`'s own construction) and roughly 63 test call sites across 8 files.** None of the test-file counts are a design question — every one is `None` (no role asserted) or a specific `Some("...")` literal, entirely mechanical. Flag for planning: `crates/executor/tests/executor_decision.rs` (16 sites) and `crates/brokerd/src/quarantine.rs`'s own test module (~44 sites) are the two highest-effort files and should each get a dedicated task/verification step rather than being folded into the "wire everything" task silently.

### 4. `DenyReason` — independently re-confirmed exhaustive-match blast radius

`crates/runtime-core/src/executor_decision.rs:14-60`. Current 9 variants: `DanglingHandle`, `EmptyTaintInvariantViolation`, `MissingProvenanceAnchor`, `UnknownSink(String)`, `UnknownArg(String)`, `DuplicateArg(String)`, `MissingArg(String)`, `DraftOnlySessionDeniesCommitIrreversible { sink }`, `NonLiveSessionDeniesCommitIrreversible { sink }`.

Re-ran `grep -rn "DenyReason" crates/ cli/ --include="*.rs" | grep -v /target/` fresh (2026-07-12, not reusing Phase 23's grep output) — **confirms exactly 2 exhaustive matches, unchanged from Phase 23's finding:**

| # | File:line | Match | Update needed |
|---|---|---|---|
| 1 | `executor_decision.rs:64-80` | `impl DenyReason { pub fn code(&self) -> &'static str { match self {...} } }` | add `DenyReason::SlotTypeMismatch { .. } => "slot_type_mismatch"` |
| 2 | `executor_decision.rs:83-112` | `impl std::fmt::Display for DenyReason { fn fmt(...) { match self {...} } }` | add a human-readable arm |

Every other reference is a construction site, not a match (`sink_schema.rs:113-136`, `executor/src/lib.rs:85/98/107/180/205` — Step 1c's new construction goes here — `brokerd/src/lib.rs:80-91` test, `runtime-core/tests/task2_types.rs:85` test, `executor/tests/executor_decision.rs:371/580/637` test). `cli/caprun/src/worker.rs:381` uses `matches!(decision, ExecutorDecision::Allowed)` (boolean, on the OUTER enum) + `eprintln!("{decision:?}")` (Debug, auto-derived) — confirmed, no update needed. **Also independently re-confirmed A3 (server.rs:650-683's `match &decision { BlockedPendingConfirmation {..} => ..., _ => ... }` at line 671 has a wildcard `_` arm)** — reusing `ExecutorDecision::Denied { reason }` (no new outer-enum variant) needs zero update there. No CLI rendering match, no audit-serialization match (`#[derive(Serialize)]`, not hand-written) found anywhere in the workspace.

### 5. `sink_sensitivity.rs` — exact shape to mirror

`crates/executor/src/sink_sensitivity.rs`, full file read. Relevant pattern to copy verbatim:
```rust
pub const EMAIL_SEND_ROUTING_SENSITIVE: &[&str] = &["to", "cc", "bcc"];   // :61
pub const FILE_CREATE_ROUTING_SENSITIVE: &[&str] = &["path"];             // :66
pub const EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body"];   // :78

pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {      // :86-93
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
```
Add, same file, same discipline (per DESIGN §3):
```rust
pub fn expected_role(sink: &SinkId, arg_name: &str) -> Option<&'static [&'static str]> {
    match sink.0.as_str() {
        "email.send" => match arg_name {
            "to" | "cc" | "bcc" => Some(&["recipient", "email_address"]),
            "subject" => Some(&["subject"]),
            "body" => Some(&["body"]),
            _ => None,
        },
        "file.create" => match arg_name {
            "path" => Some(&["path", "relative_path"]),
            "contents" => None, // unconstrained for v1.5 — Assumption A2, documented not accidental
            _ => None,
        },
        _ => None, // any other sink: unconstrained, out of v1.5 scope
    }
}
```
**Contract (load-bearing, DESIGN §3/§7):** `None` = unconstrained/no-op for this arg (fall through to Step 2/3 as today); `Some(&[])` must never be constructed; do NOT implement as `.unwrap_or(&[])` anywhere (would collapse the two states). `email.send`'s schema (`sink_schema.rs:50`) allows exactly `["to","cc","bcc","subject","body"]`, `file.create`'s allows exactly `["path","contents"]` (`sink_schema.rs:55`) — confirmed these match the table's arg-name universe exactly, no orphan arg names.

### 6. `submit_plan_node` — full current step order (byte-identical to Phase 23's citation)

`crates/executor/src/lib.rs:54-216`, full function re-read line-by-line 2026-07-12:

| Step | Lines | Behavior |
|---|---|---|
| 0 — schema gate | `:66-68` | `sink_schema::validate_schema`; hard `Denied`, immediate return |
| **per-arg loop** | `:78-158` | for each `PlanArg`: |
| 1 — resolve handle | `:81-88` | `None` → `Denied { DanglingHandle }`, return |
| 1a — empty-taint | `:96-100` | `Denied { EmptyTaintInvariantViolation }`, return |
| 1b — empty-provenance | `:105-109` | `Denied { MissingProvenanceAnchor }`, return |
| **1c — role check (NEW, this phase)** | insert between `:109` and `:117` | mismatch → `Denied { SlotTypeMismatch }`, return |
| 2/3 — sensitivity | `:117-157` | `is_routing_sensitive \|\| is_content_sensitive` AND untrusted → collect into `blocked: Vec<BlockedArg>` (does NOT return) |
| after loop | `:162-164` | `blocked` non-empty → `BlockedPendingConfirmation { anchors: blocked }`, return |
| 0.5 — I0 class-deny | `:174-213` | exhaustive `SessionStatus` match (6 variants, no wildcard, `:176`); Draft/non-live + `CommitIrreversible` → `Denied` |
| Allowed | `:215` | only if nothing above returned |

**Insertion point, precisely:** after the `if record.provenance_chain.is_empty() { ... }` block closes at `:109`, before the `let sensitive = ...` line at `:117`. Step 1c reads `record.origin_role` (already resolved as `record` from Step 1) and calls the new `sink_sensitivity::expected_role(&plan_node.sink, &arg.name)`; on `Some(list)` with `record.origin_role` either `None` or `Some(s) if !list.contains(&s.as_str())`, construct and return `Denied { reason: DenyReason::SlotTypeMismatch { sink: plan_node.sink.0.clone(), arg: arg.name.clone(), expected: list.iter().map(|s| s.to_string()).collect(), found: record.origin_role.clone() } }`. On `None` (unconstrained slot) or a matching role, fall through unchanged to the existing `let sensitive = ...` line.

**Precedence preserved (T2-05's own success criterion):** Step 1c is per-arg and returns before that arg reaches Steps 2/3, so I2 Block still fires exactly as before for every arg that PASSES the role check. Step 0.5 (I0) is untouched — still gated on an empty `blocked` set from Steps 2/3, unaffected by Step 1c's insertion between 1b and 2/3.

### 7. `server.rs` — the 5 production call sites, exact context

- `:1084-1094` — `ReportClaims` handler; mints via `mint_from_read(&locked, value_store, session_id, &quarantine_claim, Some(*last_event_id), Some(last_event_hash))`. `quarantine_claim.claim_type` is already resolved just above (`:1061-1079`, one of `"email_address"`/`"relative_path"`/`"doc_fragment"`) — this IS the role string per DESIGN §2 (verbatim reuse, no new lookup needed).
- `:1217-1228` — `ReportDerivedClaim` handler; mints via `mint_from_derivation(&locked, value_store, session_id, transformed_literal, &input_refs, transform.as_mint_tag(), ...)`. `transform.as_mint_tag()` returns `"concat"` (the only `TransformKind` variant, `proto.rs:57-72`) — role is hardcoded `"recipient"` inside `mint_from_derivation`'s own `match transform_kind { "concat" => ... }` block (`quarantine.rs:670-692`), per DESIGN §4, NOT passed in from this call site.
- `:1294-1300` — the `ProvideIntent` handler's intent-variant match, producing `(primary_literal, subject_literal, body_literal)`:
  ```rust
  let (primary_literal, subject_literal, body_literal) = match &intent {
      CaprunIntent::SendEmailSummary { recipient, subject, body } =>
          (recipient.clone(), Some(subject.clone()), Some(body.clone())),
      CaprunIntent::CreateFileFromReport { path } => (path.clone(), None, None),
  };
  ```
  **This is the exact match Round-1 finding F3 requires the role selection to happen inside** — `primary_literal` is `recipient` for `SendEmailSummary` but `path` for `CreateFileFromReport`, and both flow to the SAME `mint_from_intent` call at `:1317`. Implementation must produce a `primary_role: &'static str` (`"recipient"` or `"path"`) inside this same match, threaded alongside `primary_literal`, and pass it into the `:1317` call. Hardcoding `"recipient"` at `:1317` would mistag every `file.create` path (a fail-closed regression per DESIGN's own F3 discussion, not a security hole, but a real correctness bug that must not ship).
- `:1317-1324` — `mint_from_intent(&locked, value_store, session_id, primary_literal, Some(*last_event_id), Some(last_event_hash))` — gains the `primary_role` param from the match above.
- `:1330-1337` — `mint_from_intent(..., subject, ...)` inside the `Some(subject) =>` arm — role is the literal `"subject"` (only reachable for `SendEmailSummary`, no ambiguity).
- `:1347-1354` — `mint_from_intent(..., body, ...)` inside the `Some(body) =>` arm — role is the literal `"body"` (same, no ambiguity).

### 8. Role vocabulary — exact strings pinned by DESIGN §2 (do not invent new spellings)

| Origin | Source | `origin_role` value |
|---|---|---|
| `mint_from_read` | `claim.claim_type` (verbatim, already resolved inside the function at `quarantine.rs:315-341`) | `"email_address"` / `"relative_path"` / `"doc_fragment"` |
| `mint_from_intent`, `SendEmailSummary` recipient | `server.rs:1296` match arm | `"recipient"` |
| `mint_from_intent`, subject | `server.rs:1330` | `"subject"` |
| `mint_from_intent`, body | `server.rs:1347` | `"body"` |
| `mint_from_intent`, `CreateFileFromReport` path | `server.rs:1299` match arm | `"path"` |
| `mint_from_derivation`, `"concat"` | hardcoded inside `quarantine.rs`'s `match transform_kind` at `:670-692` | `"recipient"` |

Any other `claim_type` is already a fail-closed mint error (`quarantine.rs:336-340`) — no untrusted value can carry an unrecognized role string.

### 9. `TransformKind` / `Concat` arity — Round-1 finding F2 (must be honored)

`crates/brokerd/src/proto.rs:57-72`. `TransformKind` has exactly one variant, `Concat`. `mint_from_derivation`'s `"concat"` byte-verify (`quarantine.rs:670-685`) joins `inputs.iter().map(|r| r.literal).join("@")` — this works for ANY input count ≥1 (0 inputs already fails-closed at `:588-593`), but the `local@domain` **email shape** only holds for exactly 2 inputs. Per DESIGN §4's Round-1 amendment, Phase 24 must either (a) add an explicit `inputs.len() == 2` check before assigning `origin_role = Some("recipient".into())` in the `"concat"` arm (preferred — tighter guarantee), or (b) rely on the fact that the derived value's taint is unconditionally untrusted (`WorkerExtracted` forced in at `:611-613`) so I2's per-arg Block already fires at any routing/content-sensitive slot regardless of role, for any arity. **(a) is the DESIGN doc's stated preference — implement the length check, do not skip it and lean solely on (b).**

### 10. `TaintLabel` — independently re-confirmed 8 variants (Round-1 finding F5's correction)

`crates/runtime-core/src/plan_node.rs:12-24`: `UserTrusted`, `LocalWorkspace`, `ExternalUntrusted`, `EmailRaw`, `PdfRaw`, `LlmGenerated`, `WorkerExtracted`, `PathRaw` — exactly 8, matches the DESIGN doc's corrected count. `is_untrusted()` (`:40-50`) is a no-wildcard exhaustive match — **unaffected by this phase** (origin_role is a parallel field, never a `TaintLabel` variant, per DESIGN §1's explicit rejection of that alternative).

## Standard Stack

Not applicable in the traditional sense — this phase adds zero new Cargo dependencies. All new code uses only `Option<String>`, `Vec<String>`, `&'static [&'static str]`, and the `serde::Serialize`/`Deserialize` derives already present throughout `runtime-core`. `#[serde(default)]` (F6) is a standard serde attribute, already used elsewhere in this codebase's serde-derived structs — no new crate needed.

## Package Legitimacy Audit

Not applicable — no new external crate dependencies are introduced by this phase (confirmed: the recommended shape uses only stdlib/already-vendored types). `Cargo.toml` at the workspace root and every touched crate's own `Cargo.toml` require no edits.

## Architecture Patterns

### System Architecture Diagram

```
Worker (kernel-confined)
  │  ReportClaims / ReportDerivedClaim / ProvideIntent  (IPC over UDS)
  ▼
brokerd::server::dispatch_request  ──────────────────────────────────┐
  │  (5 production mint call sites, §7 above)                        │
  │                                                                   │
  ├─ ReportClaims ─────► quarantine::mint_from_read                  │
  │                        role = claim.claim_type (verbatim)         │
  │                                                                   │
  ├─ ReportDerivedClaim ► quarantine::mint_from_derivation            │
  │                        role = hardcoded "recipient" iff           │
  │                              transform_kind == "concat"           │
  │                              AND inputs.len() == 2 (NEW guard)    │
  │                                                                   │
  └─ ProvideIntent ─────► [intent-variant match, server.rs:1294-1300] │
                            selects role "recipient"|"path" HERE ─────┤
                            then 1-3x quarantine::mint_from_intent     │
                              (role = "recipient"/"subject"/"body"/   │
                               "path", selected by caller)             │
                                                                       │
  Every mint_from_* delegates to ─────────────────────────────────────┘
  executor::value_store::ValueStore::mint(literal, taint,
                                            provenance_chain,
                                            origin_role)  ◄── NEW param
      constructs the ONE ValueRecord{} in production code
      (id, literal, taint, provenance_chain, origin_role: Option<String>)

                    ... later, on a SubmitPlanNode request ...

brokerd::server ──► executor::submit_plan_node(plan_node, value_store, session_status)
  Step 0   schema gate                       (unchanged)
  Step 1   resolve handle                    (unchanged)
  Step 1a  empty-taint guard                 (unchanged)
  Step 1b  empty-provenance guard            (unchanged)
  Step 1c  role check (NEW) ─────► sink_sensitivity::expected_role(sink, arg_name)
              None            → no-op, fall through
              Some(list):
                role ∉ list   → Denied{SlotTypeMismatch}  [RETURN]
                role ∈ list   → fall through
  Step 2/3 sensitivity collect-then-Block    (unchanged, runs only for
                                               args that passed Step 1c)
  Step 0.5 I0 class-deny                     (unchanged, runs only if
                                               nothing collected above)
  Allowed                                    (unchanged, terminal)
```

### Recommended Task Sequencing (informative — planner's call on exact wave/task split)

1. **Type + storage layer:** `ValueRecord` field, `ValueStore::mint` signature, the 3 test-only direct-`ValueRecord{}`-literal fixes (§1). Compiles standalone (everything downstream is `None` until threaded).
2. **Threading through `mint_from_*` + `server.rs`:** the 3 wrapper signatures, the intent-variant-match role selection (§7, F3), the `"concat"`/2-input guard (§9, F2). This is where ALL ~9 production call sites + ~63 test call sites get touched — expect this to be the largest task by file count.
3. **Executor:** `expected_role()` in `sink_sensitivity.rs` (§5), `DenyReason::SlotTypeMismatch` + the 2 exhaustive-match updates (§4), Step 1c in `submit_plan_node` (§6).
4. **Verification:** `cargo build --workspace` after each step; `cargo test --workspace --no-fail-fast` at the end of the phase (Linux-only live paths need `scripts/mailpit-verify.sh`, but the NEW Step 1c `Denied` path itself is workspace-buildable and NOT Linux-gated — see Validation Architecture below).

### Anti-Patterns to Avoid

- **Hardcoding `"recipient"` at the shared `server.rs:1317` call site instead of selecting inside the `:1294-1300` match.** Mistags every `file.create` path (F3).
- **`.unwrap_or(&[])` in `expected_role()`'s lookup or its call site.** Collapses the `None`-vs-`Some(&[])` fail-closed contract (§5, §7 of the DESIGN doc).
- **Reading `inputs[*].origin_role` inside `mint_from_derivation`'s role assignment.** Role is a function of `transform_kind` ONLY — anti-laundering (DESIGN §4).
- **Folding Step 1c into the `blocked`/collect-then-Block loop.** A role mismatch is a hard structural `Denied`, never `BlockedPendingConfirmation` — DESIGN §6 forbids it explicitly (no sound human-confirmable response to a slot-type error).
- **Adding a new `ExecutorDecision` variant for `SlotTypeMismatch`.** The reused `Denied { reason }` carrier needs no outer-enum change (A3, independently re-confirmed at `server.rs:671`'s wildcard arm).
- **Skipping the `inputs.len() == 2` guard in the `"concat"` arm** on the theory that I2 already covers degenerate arities. DESIGN's Round-1 amendment states (a) is preferred; implement it.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-sink-arg role constraint lookup | A general pluggable role/policy framework, a config file, a rule engine | A hardcoded `match sink.0.as_str() { ... }` function in `sink_sensitivity.rs`, identical shape to `is_routing_sensitive` | Locked by `CLAUDE.md`/`PLAN.md` (CON-i2-non-bypassable); explicitly out of scope this milestone (REQUIREMENTS.md Out-of-Scope table) |
| Role vocabulary | A new enum unifying `claim_type` and role tags | Reuse `claim_type`'s existing `String` values directly (DESIGN §2) | DESIGN-08 requires unification, not a parallel taxonomy — a new enum would BE the forbidden parallel taxonomy |
| ValueRecord construction with the new field | A `Default` derive + `..Default::default()` shortcut at test call sites (silently defaulting `origin_role` to `None` everywhere, masking which tests actually exercise a role) | Explicit `origin_role: None` or `origin_role: Some("...")` at each of the 3 direct-literal test sites | Explicit is auditable; a blanket `Default` would let a test silently mean "unconstrained" when the author intended "trusted-recipient" |

**Key insight:** This phase's entire scope is deliberately narrow and ALREADY DESIGNED — the only genuine engineering risk is under-counting the mechanical blast radius (test call sites) or mis-locating the role-selection logic (F3's shared call site trap), not any remaining open design question.

## Common Pitfalls

### Pitfall 1: Undercounting the signature-threading blast radius
**What goes wrong:** Implementer treats "three mint sites" (DESIGN §9's phrasing) as "three call sites to update," missing that (a) `ValueStore::mint` itself needs the new parameter too, and (b) ~63 test call sites across 8 files reference these functions and will fail to compile the moment the signatures change.
**Why it happens:** DESIGN §9's table is illustrative ("Mint sites — additive origin_role threading"), not an exhaustive call-site inventory — that inventory is this research's job, not the design gate's.
**How to avoid:** Use Existing-Code Map §3's file-by-file count as the task checklist; compile after each signature change to surface every break immediately (`cargo build --workspace` then `cargo test --workspace --no-run` to catch test-compile errors without running the Linux-only suites).
**Warning signs:** `cargo build --workspace` succeeding but `cargo test --workspace --no-run` failing on dozens of "missing field `origin_role`" / "this function takes N arguments but M were supplied" errors discovered late.

### Pitfall 2: Hardcoding the recipient/path role at the shared mint_from_intent call site
**What goes wrong:** `server.rs:1317`'s `mint_from_intent(..., primary_literal, ...)` call is reached by BOTH `SendEmailSummary` (role should be `"recipient"`) and `CreateFileFromReport` (role should be `"path"`) — hardcoding either string there silently mistags the other intent variant.
**Why it happens:** The call site itself has no visibility into which `CaprunIntent` variant produced `primary_literal` — that information only exists in the match at `:1294-1300`, several lines earlier.
**How to avoid:** Select `primary_role: &'static str` inside the SAME match arm that produces `primary_literal`, and thread it as a tuple element alongside `primary_literal`/`subject_literal`/`body_literal` into the shared call.
**Warning signs:** A `file.create` live-acceptance test starts failing at Step 1c with `expected: ["path", "relative_path"], found: Some("recipient")` — silent, but a real regression, not a security hole (DESIGN's own F3 characterization).

### Pitfall 3: Treating the Concat-derived role as inherited/unioned
**What goes wrong:** Implementer computes `mint_from_derivation`'s `origin_role` from `inputs[0].origin_role` (or a union), "for consistency with how taint IS unioned" (`quarantine.rs:603-613`).
**Why it happens:** The taint-union code sits right above where role assignment must go, and pattern-matching on adjacent code is a natural mistake.
**How to avoid:** Role is a function of `transform_kind` ONLY, hardcoded `"recipient"` inside the `"concat"` match arm — never read from `inputs`.
**Warning signs:** Any code path where `origin_role` is derived from `inputs` inside `mint_from_derivation`.

### Pitfall 4: `.unwrap_or(&[])` in the expected-role lookup
**What goes wrong:** `expected_role()`'s call site (inside Step 1c) is implemented as `expected_role(sink, arg).unwrap_or(&[])`, collapsing "unconstrained" (`None`) and "role-checked-but-value-has-no-matching-role" into indistinguishable states.
**Why it happens:** `is_routing_sensitive`'s existing precedent returns a bare `bool`, not an `Option`, so an implementer copy-pasting that shape may reach for a similar "just give me something iterable" pattern here.
**How to avoid:** Match on `Option<&[&str]>` explicitly: `None => continue (no-op)`, `Some(list) if role check fails => Denied`, `Some(list) if role check passes => continue`.
**Warning signs:** A `.unwrap_or(` anywhere in the Step 1c implementation or in `expected_role`'s definition.

### Pitfall 5: Forgetting the `"concat"` 2-input arity guard
**What goes wrong:** `mint_from_derivation`'s `"concat"` arm assigns `origin_role = Some("recipient".into())` unconditionally, even when `inputs.len() != 2` (a degenerate single-input or 3+-input concat), producing a mistagged role for a non-`local@domain`-shaped output.
**Why it happens:** The existing byte-verify guard already accepts any input count ≥1 (only 0 is rejected), so it's easy to assume "concat" always implies exactly 2.
**How to avoid:** Add an explicit `if inputs.len() == 2 { Some("recipient".to_string()) } else { None }` (or an equivalent explicit branch) inside the `"concat"` match arm, per DESIGN §4's Round-1 amendment (option (a), the preferred fix).
**Warning signs:** A test asserting a 1-input or 3-input concat derivation's `origin_role` without checking DESIGN's arity caveat first.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 (carried from DESIGN §10) | `"recipient"` is the correct hardcoded role for 2-input `Concat`-derived values | Existing-Code Map §9 | If wrong, legitimate concat-derived recipients would be denied at `to` — DESIGN doc states this was cross-checked against the confirm-binding scenario during authoring; Phase 24 should not re-litigate, only implement. |
| A2 (carried from DESIGN §10) | `file.create`'s `contents` arg stays role-unconstrained for v1.5 | Existing-Code Map §5 | Low risk — documented intentional scope-out, not a design gap Phase 24 can unilaterally close (would require its own design pass for a `"file_body"` role). |
| A3 (carried from DESIGN §10, independently re-confirmed this session) | No exhaustive match over the OUTER `ExecutorDecision` enum needs a new arm | Existing-Code Map §4 | Re-confirmed via direct read of `server.rs:650-683` — the match has an explicit `_ =>` wildcard arm. Low remaining risk. |
| A4 (new, this research) | `ValueStore::mint`'s signature is the correct place to add the fourth `origin_role` parameter (rather than, e.g., a `mint_with_role` sibling function) | Existing-Code Map §1/§2 | If a sibling function were preferred instead, the existing `mint` call sites (test-only) would need a decision about which to call — adding a parameter to the existing function (with test call sites updated) is the minimal-surface-area choice and is Gate-3-legal; flagging as assumed rather than DESIGN-pinned because the DESIGN doc does not mention `ValueStore::mint` by name. |

**All other claims in this research are `[VERIFIED: direct code read, 2026-07-12]`** — every file:line citation was independently re-read this session (not merely copied from Phase 23's research), and cross-checked against `planning-docs/DESIGN-slot-type-binding.md`'s own citations for drift (none found).

## Open Questions

1. **Exact task/wave split for the ~63 test call-site updates.**
   - What we know: the work is entirely mechanical (add `None` or a specific `Some("...")` argument per call), concentrated in `quarantine.rs`'s own test module (~44 sites) and `executor/tests/executor_decision.rs` (16 sites).
   - What's unclear: whether the planner splits this into one giant "make it compile" task or several smaller per-file tasks with individual verification gates.
   - Recommendation: given the file-count concentration, a per-crate split (brokerd test threading / executor test threading / cli test threading) with a `cargo build --workspace && cargo test --workspace --no-run` gate after each is likely to catch mistakes earlier than one monolithic task — but this is a planning-granularity choice, not a research gap.

2. **Whether existing tests that assert `UserTrusted`-in-any-slot Allowed behavior will now fail at Step 1c, and if so which ones.**
   - What we know: T2-07 (regression audit of exactly this) is explicitly scoped to Phase 25, not Phase 24.
   - What's unclear: Phase 24's own test threading (assigning `origin_role` values to existing fixtures) may incidentally cause some existing tests to newly fail at Step 1c if their fixture's assigned role doesn't match its slot — this is NOT the T2-06/07 acceptance test (that's a NEW held-out test) but could be a side effect of threading roles through pre-existing fixtures naively.
   - Recommendation: when threading `origin_role` through existing test fixtures in Phase 24, assign roles that make each fixture's EXISTING assertions continue to hold (e.g. a fixture asserting `to`-slot `Allowed` with a `UserTrusted` value should get `origin_role: Some("recipient")`, not `None` or a mismatched role) — this is mechanical but requires reading each fixture's intent, not just its plan-node shape. Flag any fixture where the "correct" role assignment is ambiguous for a `checkpoint:human-verify`-style pause rather than guessing silently.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust's built-in `cargo test` (no external test framework) |
| Config file | none — workspace `Cargo.toml` at repo root (`resolver = "3"`, edition 2021) |
| Quick run command | `cargo build --workspace` (compile-check after each signature change), then `cargo test -p executor executor_decision` (targeted, workspace-buildable on Mac) |
| Full suite command | `cargo test --workspace --no-fail-fast` (Mac; Linux-only security tests show "0 passed" as expected per `CLAUDE.md`) plus `bash scripts/mailpit-verify.sh` for the Linux-only live paths |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| T2-02 | Every minted value carries `origin_role`; existing round-trip tests still pass with the new field threaded | unit | `cargo test -p executor value_store` and `cargo test -p brokerd quarantine` | ✅ existing files, ❌ new assertions on `origin_role` — add to `value_store.rs`'s `mint_then_resolve_round_trip` and `quarantine.rs`'s existing mint tests |
| T2-03 | `expected_role()` returns the exact pinned table for both live sinks | unit | `cargo test -p executor sink_sensitivity` | ❌ Wave 0 gap — new test module additions mirroring `is_routing_sensitive`'s existing test shape |
| T2-04 | New `DenyReason::SlotTypeMismatch` variant; `.code()` and `Display` both cover it; no wildcard arm anywhere | unit + compile-time | `cargo build --workspace` (no-wildcard discipline is a compile error, not a runtime test) + `cargo test -p runtime-core` | ❌ Wave 0 gap — add a `.code()`/`Display` assertion for the new variant mirroring `DraftOnlySessionDeniesCommitIrreversible`'s existing test shape |
| T2-05 | `submit_plan_node` Denies on a role mismatch, per-arg, without disturbing I0/I2 precedence | unit + integration | `cargo test -p executor executor_decision` | ❌ Wave 0 gap — add cases mirroring `executor_decision.rs:371`/`:580`'s existing `Denied`-path test shape: (a) mismatched role → Denied{SlotTypeMismatch}; (b) matching role + tainted → still Blocks (I2 precedence preserved); (c) unconstrained slot (`file.create` `contents`) → falls through unaffected |

### Sampling Rate
- **Per task commit:** `cargo build --workspace` (catches every signature-threading break immediately, cheap).
- **Per wave merge:** `cargo test --workspace --no-fail-fast` (Mac-buildable subset; Linux security tests report 0 passed as expected).
- **Phase gate:** Full suite green before `/gsd-verify-work`; the NEW Step 1c `Denied` path itself is NOT Linux-gated (pure Rust logic, no kernel confinement dependency) — `cargo test -p executor executor_decision` alone is sufficient to prove T2-05 on the Mac dev machine. `scripts/mailpit-verify.sh` is only needed to confirm the pre-existing live-dispatch paths still work end-to-end after the signature threading (regression, not new-feature verification) — full T2-08's independent Linux re-run is explicitly Phase 25's job, not this phase's.

### Wave 0 Gaps
- [ ] `crates/executor/src/sink_sensitivity.rs` — new `#[cfg(test)] mod tests` cases for `expected_role()` covering every table row (both matching-role and non-matching-role cases for `to`/`cc`/`bcc`/`subject`/`body`/`path`, plus the `contents`-is-`None` unconstrained case).
- [ ] `crates/runtime-core/src/executor_decision.rs` — a test asserting `.code()` and `Display` both cover `SlotTypeMismatch` without panicking (mirrors existing coverage pattern for other variants, though note: this file's own `#[cfg(test)]` module currently has no test module at all — check before assuming one exists).
- [ ] `crates/executor/tests/executor_decision.rs` — 3 new test cases per the T2-05 row above (mismatch-denies, matching-role-tainted-still-blocks, unconstrained-slot-unaffected).
- [ ] `crates/executor/src/value_store.rs`'s `mint_then_resolve_round_trip` and `crates/brokerd/src/quarantine.rs`'s existing mint tests — extend existing assertions to also check `record.origin_role` round-trips correctly (not a new file, an extension of the ~63 already-being-touched call sites).

*(No framework install needed — `cargo test` is already fully configured; this is additive test-case authorship inside already-existing test modules/files, not new test infrastructure.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V5 Input Validation | Yes | This phase IS the enforcement mechanism — Step 1c is a new structural validation gate, hardcoded in Rust TCB, no bypass path. |
| V4 Access Control | Yes (indirectly) | `expected_role()` is a capability-scoped check over which value-shapes may occupy which sink-arg slots — same class of control as `is_routing_sensitive`/`is_content_sensitive`, already treated as a security-critical hardcoded map in this codebase. |
| V6 Cryptography | No | Not implicated — no new crypto. |
| V2/V3 Authentication/Session | No new surface | `SessionStatus` handling (Step 0.5) is explicitly UNTOUCHED — Step 1c is inserted strictly before it, per DESIGN §6/§8 item 6. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Slot-type confusion (T2, the whole subject of this phase): a `UserTrusted` value minted for one semantic field submitted into a different slot | Spoofing / Tampering | Origin-role tag + expected-role table + fail-closed `Denied` — this phase's entire deliverable. |
| Role-laundering via derivation (an attacker smuggling a `"recipient"`-tagged doc_fragment input to make a derived output inherit an unearned role) | Tampering | Role assigned from `transform_kind`'s own verified output shape (`Concat`'s byte-verify), NEVER read from `inputs[*].origin_role` — Pitfall 3 above. |
| Fail-open via ambiguous "no constraint" representation (`.unwrap_or(&[])` collapsing `None` and `Some(&[])`) | Tampering / Elevation of Privilege | `Option<&[&str]>` explicit two-state contract, never collapsed — Pitfall 4 above. |
| Untrusted-side role tags being worker-influenced (a hostile worker can mint an arbitrary string tagged `"email_address"`/`"relative_path"`, since `mint_from_read` shape-validates only `doc_fragment`) | Spoofing | Table-construction invariant (DESIGN §3 Round-1 F4): an untrusted-origin role may appear in a slot's expected-role list ONLY if that slot is also I2-sensitive — so I2's per-arg Block already fires there regardless of role. **Phase 24 must preserve this invariant when writing the table** — do not add an untrusted-origin role string to a slot that is neither `is_routing_sensitive` nor `is_content_sensitive` for that sink. |

## Sources

### Primary (HIGH confidence — direct code reads, this session, 2026-07-12)
- `planning-docs/DESIGN-slot-type-binding.md` (full, 518 lines, post-Round-1-amendment) — the authoritative, cleared design ruling this phase mechanically realizes.
- `planning-docs/DESIGN-GATE-RECORD-v1.5.md` (referenced via 23-02-SUMMARY.md) — confirms gate CLEARED, all findings resolved.
- `crates/runtime-core/src/value_record.rs` (full) — `ValueRecord` struct, no `Default` derive.
- `crates/runtime-core/src/executor_decision.rs` (full) — `DenyReason`, both exhaustive matches, `ExecutorDecision`.
- `crates/runtime-core/src/plan_node.rs:1-120` — `TaintLabel` (8 variants), `PlanArg`, `SinkId`.
- `crates/executor/src/value_store.rs` (full) — `ValueStore::mint`, the sole production `ValueRecord` constructor.
- `crates/executor/src/sink_sensitivity.rs` (full) — `is_routing_sensitive`/`is_content_sensitive` pattern to mirror.
- `crates/executor/src/lib.rs` (full) — `submit_plan_node`, full step order, byte-identical to Phase 23's citations.
- `crates/executor/src/sink_schema.rs` (partial) — `KNOWN_SINKS`, `allowed` arg sets for both live sinks.
- `crates/brokerd/src/quarantine.rs` (full, 1699 lines) — all 3 mint function bodies, claim_type taxonomy, mint-ordering, both direct `ValueRecord{}` test-literal sites.
- `crates/brokerd/src/server.rs:1060-1365` — all 5 production mint call sites, the intent-variant match (F3's exact location), `ReportDerivedClaim` handler.
- `crates/brokerd/src/server.rs:640-690` — independently re-confirmed the `_ =>` wildcard arm discharging A3.
- `crates/brokerd/src/sinks/file_create.rs:220-290` — confirmed the file's `.mint(` calls are test-only, Gate-3-exempt.
- `crates/brokerd/src/proto.rs:48-72` — `TransformKind` (1 variant, `Concat`), `as_mint_tag()`.
- `crates/runtime-core/tests/types_compile.rs:30-50` — the third direct `ValueRecord{}` test-literal site.
- `cli/caprun/src/worker.rs:370-390` — confirmed `matches!`/Debug rendering, no exhaustive `DenyReason` match.
- `scripts/check-invariants.sh` (full) — Gate 1/2/3 exact text and exemption rules, independently re-verified.
- Workspace-wide `grep -rn "mint_from_read(\|mint_from_intent(\|mint_from_derivation(\|\.mint(" crates/ cli/` and `grep -rn "DenyReason" crates/ cli/` and `grep -rn "ValueRecord {" crates/ cli/` — all re-run fresh this session, not reused from Phase 23.
- `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/phases/23-slot-type-binding-design-gate/{23-RESEARCH.md,23-01-SUMMARY.md,23-02-SUMMARY.md}` — phase requirements, prior research, design-gate outcome.
- `git log --oneline -5 -- crates/ cli/` and `git status --porcelain crates/ cli/` — confirmed code state is unchanged since Phase 23's design-gate close (no TCB drift).

### Secondary (MEDIUM confidence)
- None — this phase required no external library/documentation lookups; it is pure internal-codebase mechanical implementation of an already-locked design.

### Tertiary
- None.

## Metadata

**Confidence breakdown:**
- Existing-code map (§1–§10): HIGH — every citation independently re-read this session (not copied from Phase 23), cross-checked against the DESIGN doc for drift (none found), and workspace-wide greps re-run fresh.
- Mechanical blast-radius counts (test call sites, direct `ValueRecord{}` literals): HIGH — exact grep counts, not estimates.
- Design decisions themselves (role vocabulary, table contents, ordering, fail-closed default): HIGH — these are NOT this phase's decisions; they are DESIGN-slot-type-binding.md's pinned, adversarially-reviewed rulings, cited not re-derived.
- The `ValueStore::mint` fourth-parameter requirement (A4): MEDIUM-HIGH — logically forced by the code (it's the only constructor), but not named explicitly in the DESIGN doc, so flagged as this research's own inference rather than a doc-cited fact.

**Research date:** 2026-07-12
**Valid until:** No expiry driver — internal-codebase research, valid until `quarantine.rs`/`server.rs`/`executor_decision.rs`/`lib.rs`/`sink_sensitivity.rs`/`value_store.rs`/`value_record.rs`/`plan_node.rs` change. Re-verify file:line citations if Phase 24 execution begins more than a few commits after this research (unlikely — no other TCB work is scheduled between now and Phase 24 execution per `.planning/STATE.md`).

## RESEARCH COMPLETE
