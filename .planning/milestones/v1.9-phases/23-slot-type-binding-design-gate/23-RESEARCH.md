# Phase 23: Slot-Type Binding Design Gate - Research

**Researched:** 2026-07-11
**Domain:** Rust TCB security enforcement — origin-role tagging for value provenance, exhaustive enum extension, executor deny/block ordering
**Confidence:** HIGH (all findings grounded in direct code reads of this repo; no external library research needed — this phase is pure internal-architecture design)

## Summary

Phase 23 is a DESIGN-doc gate: no code changes, only research to ground `planning-docs/DESIGN-slot-type-binding.md`. This phase closes the v1.4 T2 residual — a `UserTrusted` value can occupy any plan-node slot regardless of its semantic origin (e.g. a value minted as an email body could be routed into the `to` field). The fix path (Phase 24) adds an additive origin-role tag at the three mint sites, a hardcoded per-sink-arg expected-role table in the executor, and a new exhaustive `DenyReason` variant. This research maps every code site Phase 24 will touch and gives grounded rulings for the four open design questions.

**Primary recommendation:** Add one additive field `origin_role: Option<String>` to `ValueRecord` (`crates/runtime-core/src/value_record.rs:21`), threaded through all three mint sites with a caller-supplied string tag; reuse `claim_type`'s existing string values as the untrusted-side role vocabulary (unifying, not duplicating, DESIGN-08); hardcode `mint_from_derivation`'s role as `"recipient"` for the `"concat"` transform (the only transform in scope, and the only shape it ever produces); and enforce the new check as an early per-arg fail-closed guard (a new "Step 1c") in `submit_plan_node`, positioned after the existing empty-taint/empty-provenance guards and before the sensitivity/collect-then-Block loop — returning a hard `Denied { reason: DenyReason::SlotTypeMismatch { .. } }` on first mismatch, never joining `BlockedPendingConfirmation`. This preserves I2-Block-before-I0-class-deny precedence exactly, requires zero new `BlockedArg`/anchor shapes, and mirrors the existing DanglingHandle/EmptyTaint immediate-return pattern already in the function.

## Architectural Responsibility Map

This phase is single-tier (Rust TCB backend enforcement only — no browser/frontend/CDN involvement). All capabilities below live in the Broker/Executor tier of `crates/*`.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Origin-role tagging at mint time | API/Backend (`crates/brokerd`) | — | Broker is sole mint authority (T-04-03); role must be assigned atomically with taint/provenance, same as today's claim_type-derived taint. |
| Expected-role table lookup | API/Backend (`crates/executor`) | — | Mirrors `sink_sensitivity.rs`'s existing hardcoded-in-Rust-TCB pattern; policy may never be a config file (CON-i2-non-bypassable). |
| DenyReason variant + exhaustive matches | API/Backend (`crates/runtime-core`, `crates/executor`) | — | `runtime-core` owns the type; `executor`/`cli` are the only other match sites found (see blast-radius table below). |
| Fail-closed default enforcement | API/Backend (`crates/executor::submit_plan_node`) | — | Single enforcement point, same function that already enforces I0/I2. |

## Project Constraints (from CLAUDE.md)

- TCB is Rust; Python not applicable here. No custom expected-role framework — hardcoded per-sink-arg only, mirroring `sink_sensitivity.rs`.
- Terminology locked: `Intent`/`Session`/`Planner`/`Worker`/`Broker`/`Adapter`/`Effect`/`Artifact`/`Event`. `ExecutionContext` never in public API.
- Effect path is locked to plan nodes; `check-invariants.sh` Gate 1 forbids the token `EffectRequest` anywhere under `crates/` (no exemption needed here — this phase adds no new effect path).
- `check-invariants.sh` Gate 3 restricts `mint_from_read(`, `mint_from_derivation(`, and `.mint(` call-site tokens to `crates/brokerd/src/quarantine.rs` (definitions), `crates/brokerd/src/server.rs` (dispatch), and `crates/executor/src/value_store.rs` (`.mint(` only). Adding an `origin_role` parameter to these functions does NOT violate Gate 3 (it is a signature change, not a new call site) — but Phase 24 must not introduce a NEW call site elsewhere.
- Design-gate discipline: no `crates/executor` or `crates/brokerd` mint-site/TCB code may be written until this design doc clears a fresh (non-self) adversarial review (mirrors v1.0 Phase 2 / v1.2 Phase 8 / v1.3 Phase 12 / v1.4 Phase 18).
- Out of scope for this milestone (v1.5): any I0/I1 trust-classification change, a general/pluggable role framework, sinks beyond `email.send`/`file.create`.
- Source of truth on any doc conflict: `planning-docs/PLAN.md`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DESIGN-07 | Design doc specifies origin-role tagging mechanism, new `DenyReason` variant shape + full exhaustive-match blast radius, and the ordering ruling (collect-then-Block vs hard Denied) | See "Existing-Code Map" §4 (complete blast-radius inventory: only 2 match sites) and "Design Decision Space" (d) |
| DESIGN-08 | Design doc unifies with existing `claim_type` taxonomy in `quarantine.rs`, defines analogous role tags from scratch for `ProvideIntent`-minted `UserTrusted` values | See "Existing-Code Map" §1 (complete claim_type inventory) and "Design Decision Space" (b) |
| DESIGN-09 | Design doc resolves role propagation through `mint_from_derivation`'s `Concat` transform | See "Existing-Code Map" §3 and "Design Decision Space" (c) |
| DESIGN-10 | Design doc pins fail-closed default: no-role or unrecognized-role at a role-checked slot is Deny, never silent-pass | See "Design Decision Space" (d) and "Common Pitfalls" |
</phase_requirements>

## Existing-Code Map

### 1. `claim_type` taxonomy — `crates/brokerd/src/quarantine.rs`

`Claim { claim_type: String, value: String }` (`quarantine.rs:47-53`) — a lossy, typed extract. THREE known values, each set at exactly one extractor:

| `claim_type` value | Set at | Consumed at (taint mapping) |
|---|---|---|
| `"email_address"` | `extract_email_claims` (`quarantine.rs:79`) | `mint_from_read` (`quarantine.rs:316`) → `[ExternalUntrusted, EmailRaw]` |
| `"relative_path"` | `extract_relative_path_claims` (`quarantine.rs:110`) | `mint_from_read` (`quarantine.rs:317`) → `[ExternalUntrusted, PathRaw]` |
| `"doc_fragment"` | `extract_doc_fragments` (`quarantine.rs:176`) | `mint_from_read` (`quarantine.rs:318-335`) → `[ExternalUntrusted]` (plus mint-time guard rejecting `@`-containing tokens, finding #1a anti-laundering) |

Any other `claim_type` string is a fail-closed mint error (`quarantine.rs:336-340`, "unknown claim_type ... fail-closed, never default-tagged"). Once consumed, `claim_type` itself is **discarded** — it never enters `ValueRecord` (confirmed: `ValueRecord` has no `claim_type` field, `value_record.rs:21-31`). This is exactly the gap DESIGN-08 closes: today the semantic type information dies at mint time; Phase 24 needs it to survive on the record.

### 2. The three mint sites — signatures + `ValueRecord` field to add

`ValueRecord` (`crates/runtime-core/src/value_record.rs:21-31`): `{ id: ValueId, literal: String, taint: Vec<TaintLabel>, provenance_chain: Vec<Uuid> }`. **Add** `pub origin_role: Option<String>` here — additive, `Option` so `None` is representable (required for DESIGN-10's fail-closed default) and no existing `Vec`/enum shape needs to change.

| Mint site | Signature (current) | Caller knows target semantic field? |
|---|---|---|
| `mint_from_read` | `(conn, store, session_id, claim: &Claim, parent_id, parent_hash)` (`quarantine.rs:284-297`) | Yes — via `claim.claim_type`, already resolved inside the function (see §1 table above; role = claim_type verbatim, no new taxonomy needed). |
| `mint_from_intent` | `(conn, store, session_id, literal: String, parent_id, parent_hash)` (`quarantine.rs:435-442`) | Yes, but only at the CALLER (`server.rs`) — the function itself has no field-name context; the three call sites in `server.rs:1317` (recipient), `server.rs:1330` (subject), `server.rs:1347` (body) each already know which literal they're minting. Confirmed by `server.rs:1294-1300`'s `match &intent { SendEmailSummary { recipient, subject, body } => ..., CreateFileFromReport { path } => ... }`. |
| `mint_from_derivation` | `(conn, store, session_id, transformed_literal: String, inputs: &[&ValueRecord], transform_kind: &str, parent_id, parent_hash)` (`quarantine.rs:574-583`) | No explicit field name at the call site (`server.rs:1217-1228`, dispatched from a `ReportDerivedClaim` request that carries no target-slot hint) — role must be derived from `transform_kind` itself (see §3). |

**Known ordering gotcha (confirmed by reading, not just told):** `mint_from_read` appends the `file_read` Event FIRST (`quarantine.rs:342-354`), then calls `store.mint` (`quarantine.rs:364-366`) — event exists before the value. `mint_from_derivation` does the REVERSE: it calls `store.mint` FIRST (`quarantine.rs:702-704`, explicit comment "Mint the derived ValueRecord FIRST: the derivation Event's hashed payload embeds `derived_value_id`"), then appends the `derivation` Event (`quarantine.rs:713-729`) which references the just-minted `value_id`. Any Phase 24 code touching both call shapes must NOT assume a uniform mint-then-event or event-then-mint order.

### 3. `mint_from_derivation` / `ReportDerivedClaim` / `Concat` — role propagation (DESIGN-09)

`TransformKind` (`crates/brokerd/src/proto.rs:57-61`) has exactly ONE variant today: `Concat` — "a fixed `'@'`-join over doc fragments" (`proto.rs:48`), mapped via `as_mint_tag()` (`proto.rs:68-72`) to the `&str` `"concat"` that `mint_from_derivation`'s `match transform_kind` consumes (`quarantine.rs:670-692`, itself fail-closed on any other tag). The transform's byte-verify guard proves the output literal is exactly `join(input_literals, '@')` (`quarantine.rs:672-684`) — i.e. **the only shape `Concat` can ever produce is `local@domain`, a syntactic email address.** In the live product flow this is the Reply-To/Domain doc-fragment pair being assembled into a recipient candidate (this is literally the v1.4 T2 vulnerability scenario this whole milestone closes). Taint on the output is unconditionally untrusted (`WorkerExtracted` forced in, `UserTrusted` dropped — `quarantine.rs:595-624`), so I2's existing per-arg Block already fires whenever this derived value lands in any routing/content-sensitive slot, independent of role.

### 4. `DenyReason` enum — COMPLETE exhaustive-match blast-radius inventory (DESIGN-07b)

Defined `crates/runtime-core/src/executor_decision.rs:15-60`. Current variants (8): `DanglingHandle`, `EmptyTaintInvariantViolation`, `MissingProvenanceAnchor`, `UnknownSink(String)`, `UnknownArg(String)`, `DuplicateArg(String)`, `MissingArg(String)`, `DraftOnlySessionDeniesCommitIrreversible { sink }`, `NonLiveSessionDeniesCommitIrreversible { sink }`.

Workspace-wide grep (`grep -rn "DenyReason" crates/ cli/`, all non-`/target/` hits reviewed) found **exactly two exhaustive matches over the enum**, both in the SAME file as the definition:

| # | File:line | Match kind | Update required for new variant |
|---|---|---|---|
| 1 | `crates/runtime-core/src/executor_decision.rs:64-80` | `impl DenyReason { pub fn code(&self) -> &'static str { match self { ... } } }` | Yes — add `"slot_type_mismatch"` (or similar) arm |
| 2 | `crates/runtime-core/src/executor_decision.rs:83-112` | `impl std::fmt::Display for DenyReason { fn fmt(...) { match self { ... } } }` | Yes — add a human-readable arm |

Every OTHER `DenyReason` reference in the workspace is a *construction* site (`Err(DenyReason::X(...))` or `reason: DenyReason::X { .. }`), not a match — these do not need updating for a new variant, only NEW construction call sites are added by Phase 24:
- `crates/executor/src/sink_schema.rs:113-136` — constructs `UnknownSink`/`UnknownArg`/`DuplicateArg`/`MissingArg` (Step 0 schema gate). Not exhaustive; no update needed.
- `crates/executor/src/lib.rs:85,98,107,180,205` — `submit_plan_node` constructs `DanglingHandle`/`EmptyTaintInvariantViolation`/`MissingProvenanceAnchor`/`DraftOnlySessionDeniesCommitIrreversible`/`NonLiveSessionDeniesCommitIrreversible`. Not exhaustive; this is where Phase 24 ADDS the new construction site (Step 1c).
- `crates/brokerd/src/lib.rs:80-91` — test-only construction of `UnknownSink`. Not exhaustive.
- `crates/runtime-core/tests/task2_types.rs:85`, `crates/executor/tests/executor_decision.rs:371,580,637` — test-only constructions of specific variants. Not exhaustive.
- `cli/caprun/src/worker.rs:381` — renders the decision via `matches!(decision, ExecutorDecision::Allowed)` (a boolean check on the OUTER `ExecutorDecision` enum, wildcard-else) and `eprintln!("{decision:?}")` (Debug format, auto-derived, needs no manual update). **Confirmed: no exhaustive match over `DenyReason` in the CLI.**
- `crates/executor/src/sink_sensitivity.rs:174-210` doc-comments reference "DenyReason taxonomy" but contain no match over it.

**Conclusion: the DESIGN-07b blast radius is exactly 2 match sites, both co-located with the enum definition.** No CLI rendering match, no audit-serialization match (audit persistence uses `#[derive(Serialize)]`, not a hand-written match), no test-harness exhaustive match. Phase 24's task list can be scoped tightly: add the variant + update `code()` + update `Display::fmt` + add ONE new construction site in `submit_plan_node`.

### 5. `sink_sensitivity.rs` — the shape the expected-role table must mirror

`crates/executor/src/sink_sensitivity.rs`. Hardcoded `&str`-keyed membership tests, no config file, doc-commented "Sensitivity is a security property, not a configuration knob. CON-i2-non-bypassable" (`sink_sensitivity.rs:5`):
- `is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool` (`:86-93`) — `match sink.0.as_str() { "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name), "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name), _ => false }`.
- `is_content_sensitive(...)` (`:102-107`) — same shape, email.send only (`EMAIL_SEND_CONTENT_SENSITIVE = ["subject", "body"]`, `:78`).
- Const arrays: `EMAIL_SEND_ROUTING_SENSITIVE = ["to", "cc", "bcc"]` (`:61`), `FILE_CREATE_ROUTING_SENSITIVE = ["path"]` (`:66`).

**Recommended expected-role table shape (mirrors this exactly):** a new function `expected_role(sink: &SinkId, arg_name: &str) -> Option<&'static [&'static str]>` (or `&[]` for "no constraint" vs a sentinel for "unconstrained slot" — see Design Decision Space (a) below for the exact contract), same `match sink.0.as_str() { "email.send" => match arg_name { "to"|"cc"|"bcc" => Some(&["recipient"]), "subject" => Some(&["subject"]), "body" => Some(&["body"]), _ => None }, "file.create" => match arg_name { "path" => Some(&["path"]), "contents" => None /* unconstrained for v1.5 */, _ => None }, _ => None }` shape — same file, same crate, same hardcoded-Rust-TCB discipline.

### 6. `submit_plan_node` — full per-arg pass, ordering (DESIGN-07c)

Full function read: `crates/executor/src/lib.rs:54-216`. Confirmed step order:

1. **Step 0** (`:66-68`) — `sink_schema::validate_schema` (schema gate). Hard `Denied`, immediate return, runs before ANY resolve/taint/sensitivity work.
2. **Per-arg loop** (`:78-158`), for each `PlanArg`:
   - **Step 1** (`:81-88`) — resolve handle; `None` → hard `Denied { DanglingHandle }`, immediate return.
   - **Step 1a** (`:96-100`) — empty-taint guard → hard `Denied`, immediate return.
   - **Step 1b** (`:105-109`) — empty-provenance guard → hard `Denied`, immediate return.
   - **Step 2/3** (`:117-157`) — sensitivity check (`is_routing_sensitive || is_content_sensitive`) AND untrusted taint → **collect** (not return) into `blocked: Vec<BlockedArg>` (Collect-then-Block, D-14: loop always finishes scanning every arg before deciding).
3. **After the loop** (`:162-164`) — if `blocked` non-empty → `BlockedPendingConfirmation { anchors: blocked }`, return.
4. **Step 0.5** (`:174-213`) — exhaustive match over all 6 `SessionStatus` variants (no wildcard, `:176`); Draft/non-live + `CommitIrreversible` sink → hard `Denied`. Only reached when `blocked` was empty — I2 Block always pre-empts this I0 class-deny (explicit comment `:169-172`: "this placement is load-bearing... the per-arg I2 Block always takes precedence over this I1/I0 class-level deny").
5. **`Allowed`** (`:215`) — only if nothing above returned.

**Load-bearing existing precedence: schema (Step 0) > per-arg fail-closed structural guards (Steps 1/1a/1b) > I2 sensitivity Block (collect-then-Block) > I0 class-deny (Step 0.5) > Allowed.** Phase 24's mandate ("without weakening or reordering the existing I0/I2 precedence") is satisfied by inserting the new check as a Step 1c (a per-arg structural guard, same tier as 1/1a/1b) — see Design Decision Space (d).

### 7. Slot/arg identity — how the executor names args

Confirmed via `PlanArg { name: String, value_id: ValueId }` and `SinkId(pub String)` (`crates/runtime-core/src/plan_node.rs:108-115`) — `arg.name` is a plain `String`, matched by `&str` value throughout `sink_sensitivity.rs` (`"to"`, `"cc"`, `"bcc"`, `"subject"`, `"body"`, `"path"`, `"contents"`). These are the exact keys the expected-role table must use — no new identity concept needed.

### 8. House style (skimmed `DESIGN-session-trust-coherence.md`, `DESIGN-taint-model.md`)

Both use numbered `## §N. Title` sections, an explicit `## Acceptance Predicate (Done When)` numbered checklist near the end, and a `## Accepted Residual Risks` section. `DESIGN-session-trust-coherence.md` additionally shows the "amendment" convention: findings from a review round are folded back into the relevant §, tagged inline (e.g. "revised after DESIGN-GATE-RECORD-v1.4.md Round 1, finding F1"), never as a separate changelog. The design doc for this phase should follow the same shape: numbered §sections, explicit Done-When predicate, Accepted Residual Risks, round-tagged amendments after the fresh adversarial review.

## Design Decision Space

### (a) Origin-role tag representation + where it lives

**Options:** (i) new field on `ValueRecord`; (ii) side-table keyed by `ValueId` in the broker; (iii) encode role as a new `TaintLabel` variant.

**Recommended: (i)** — `pub origin_role: Option<String>` added to `ValueRecord` (`crates/runtime-core/src/value_record.rs:21-31`), mirroring `claim_type`'s bare-`String` style rather than inventing a new enum. `Option<String>` (not a bare `String`) is required so "no role assigned" is representable distinctly from any valid role tag — this is the exact bit DESIGN-10's fail-closed default keys off (`None` never matches a slot's expected-role list).

**Why not (ii) a side-table:** breaks the existing "ValueRecord is the sole broker-owned truth, planner never constructs it" invariant's symmetry — `resolve()` already returns everything the executor needs in one call (`value_store.rs:89-91`); a side-table adds a second lookup with its own can-go-stale risk, for no benefit since the field is small and always co-resident with taint/provenance.

**Why not (iii) a new `TaintLabel` variant:** `TaintLabel` is the I0/I1 trust-classification vocabulary (`ExternalUntrusted`, `EmailRaw`, `PathRaw`, `WorkerExtracted`, `UserTrusted`, `LocalWorkspace` per the taint model). Folding origin-role into it would conflate "is this value trustworthy" (I0/I1) with "is this value semantically the right shape for this slot" (the new T2 check) — Phase 24's own mandate is explicit that I0/I1 classification must be UNAFFECTED (ROADMAP.md Phase 24 success criterion 1). A parallel field keeps the two concerns orthogonal by construction.

### (b) `claim_type`-vs-new-`UserTrusted`-role unification (DESIGN-08)

**Recommended:** ONE string-tag vocabulary, reused (not duplicated) across both origins:
- Untrusted (`mint_from_read`) values: `origin_role` = the SAME string already in `claim.claim_type` — `"email_address"`, `"relative_path"`, `"doc_fragment"`. No new taxonomy; the existing extractor output is reused verbatim as the role tag (this is what "unifies with the existing taxonomy" concretely means — literally the same `String`, not a parallel enum that happens to have matching variant names).
- `UserTrusted` (`mint_from_intent`) values, defined from scratch per DESIGN-08's mandate, keyed by which of the three `server.rs` call sites mints them: `"recipient"` (`server.rs:1317`), `"subject"` (`server.rs:1330`), `"body"` (`server.rs:1347`), `"path"` (the `CreateFileFromReport` arm, `server.rs:1299`).
- `mint_from_derivation` output: `"recipient"` (see (c) below) — reuses the SAME tag as the `UserTrusted` recipient role, since both represent "a value shaped like/intended as an email recipient," which is exactly what makes the expected-role table's `"to"`/`"cc"`/`"bcc"` entries permit BOTH a human-typed recipient and a legitimately-derived one (while still catching a MISROUTED one).

**Tradeoff:** reusing `claim_type` strings as role tags means the untrusted-side vocabulary (`"email_address"`, `"relative_path"`, `"doc_fragment"`) and the trusted-side vocabulary (`"recipient"`, `"subject"`, `"body"`, `"path"`) are not perfectly symmetric names (e.g. `"email_address"` vs `"recipient"` both mean "goes in the `to`/`cc`/`bcc` slot" but spell it differently). The expected-role table's `Some(&["recipient", "email_address"])`-style lists must enumerate BOTH spellings per slot. This is explicit and auditable (a flat array Phase 24 can grep), not a hidden mapping function — preferred over inventing a single canonical vocabulary and translating `claim_type` into it, which would add a translation layer with its own correctness burden for zero behavioral gain.

### (c) Derivation propagation (DESIGN-09)

**Recommended:** `mint_from_derivation`'s role is a deterministic function of `transform_kind`, computed in the SAME `match transform_kind { "concat" => ..., other => fail-closed }` block that already exists (`quarantine.rs:670-692`) — for `"concat"`, hardcode `origin_role = Some("recipient".to_string())`. Grounded in §3 above: `Concat` is the ONLY transform in scope, its byte-verify guard proves the output is always `local@domain` shape, and the live product scenario it serves (Reply-To/Domain fragment join) IS a recipient-address construction. Tagging it `None` (the conservative-looking alternative) would be WRONG, not merely cautious: it would make every legitimately-derived recipient unconditionally fail-closed-Deny at the `to` slot even when everything else is correct, breaking the exact feature (concat-derived recipients) this milestone's own acceptance flow exercises. Explicit, not implicit (DESIGN-09's requirement) — a future SECOND `TransformKind` variant forces a compile-time decision at this same match (no wildcard), consistent with the codebase's existing exhaustive-match discipline.

**Adversarial framing this closes:** does a derived value "launder" its role by inheriting whatever the concat inputs' roles were? No — the recommendation does NOT inherit or union the inputs' roles (the two doc-fragment inputs have `origin_role = "doc_fragment"` each); it assigns a NEW role from the transform's own known output shape. This avoids a laundering path where an attacker-controlled fragment pair could smuggle a `"recipient"`-tagged doc_fragment input in to make the output inherit an unearned role.

### (d) Ordering: hard-Deny vs collect-then-Block (DESIGN-07c) + fail-closed default (DESIGN-10)

**Options:** (A) new early per-arg guard ("Step 1c"), immediate hard `Denied` on first mismatch — same tier as the existing DanglingHandle/EmptyTaint/EmptyProvenance guards (Steps 1/1a/1b). (B) fold into the existing sensitivity loop, collecting mismatches into `blocked` alongside tainted-sensitive args, surfacing as `BlockedPendingConfirmation`.

**Recommended: (A).** A role mismatch is a STRUCTURAL/type error, not a judgment call a human confirmation can legitimately resolve — unlike a tainted-but-plausible recipient (where a human confirming "yes, send to this externally-sourced address" is a meaningful, sound action), there is no sound human response to "this value's role doesn't match its slot" other than fixing the plan node and resubmitting. `BlockedPendingConfirmation` exists specifically for the confirmable case (D-14's own framing); reusing it for a non-confirmable structural violation would be a category error and would require a new `BlockedArg`/`SinkBlockedAnchor` shape or a role field bolted onto the existing one. Option (A) requires zero new anchor shapes — only a new `DenyReason` variant (already scoped in §4) — and slots naturally between Step 1b and Step 2/3 (same per-arg loop, same tier as the other three fail-closed structural guards, satisfies Phase 24 success criterion 4's "evaluated per-arg in the same pass").

**Precedence preserved:** inserting Step 1c between 1b and 2/3 means: a role mismatch on arg N is caught BEFORE that arg is even considered for the sensitivity/Block collection — so I2 Block still fires exactly as before for every arg that PASSES the role check (no reordering of I2 vs I0, since Step 0.5 is untouched and still runs only after the per-arg loop completes with an empty `blocked` set). New total order: schema (0) > structural per-arg guards incl. NEW role check (1/1a/1b/1c) > I2 sensitivity Block (2/3, collect-then-Block) > I0 class-deny (0.5) > Allowed.

**Fail-closed default (DESIGN-10):** the expected-role lookup returns `Option<&[&str]>` per (a)'s recommended shape. Two failure shapes, BOTH must Deny (never fall through):
1. The VALUE has no role (`record.origin_role == None`) AND the slot IS role-checked (lookup returns `Some(list)`) → `Denied`.
2. The value HAS a role, but it's not in the slot's `Some(list)` → `Denied`.
3. (Non-failure, explicit) — the slot is UNCONSTRAINED for v1.5 scope (lookup returns `None`, e.g. `file.create`'s `contents` arg, deliberately left unconstrained per §5) → role check is a no-op for that arg, falls through to Step 2/3 as today. This is NOT a fail-open bug — it's a scoped-out slot, and the design doc must say so explicitly (never leave it to be inferred) to satisfy DESIGN-10's "never silent pass-through" language at the DOCUMENT level, not just the code level.

## Adversarial-Review Angles

The design doc must preempt these — a fresh non-self reviewer will probe:

1. **Can a derived value launder its role?** Addressed in (c): role is assigned from `transform_kind`'s known output shape, never inherited/unioned from input roles. The doc must show this explicitly, not just assert it — cite that `mint_from_derivation`'s ONLY transform (`"concat"`) has a byte-verified, structurally-fixed output shape (`local@domain`), so the role assignment is provably tied to a verified property of the output, not a claim about the inputs.
2. **Does fail-closed truly fail closed for an unassigned role?** The doc must show BOTH failure shapes from (d) — `None` role at a role-checked slot, AND a role not in the slot's list — reach `Denied`, with no third path. A reviewer will specifically try: what if `expected_role()` is accidentally implemented as `.unwrap_or(&[])` with an empty-slice-means-"allow anything" bug, rather than `Option::None`-means-unconstrained vs `Some(&[])`-means-nothing-is-allowed? The doc must pin the exact return-type contract (recommend: `None` = unconstrained/no-check, `Some(&[])` should never be constructed — a slot with zero valid roles is a design bug, not a runtime state) to close this ambiguity.
3. **Does the check reorder/weaken I0/I2?** Addressed in (d)'s precedence table — the doc must reproduce `submit_plan_node`'s FULL existing step list (not summarize it) so a reviewer can diff old-vs-new ordering line by line, and must explicitly state Step 0.5 (I0) is untouched and still gated on an empty `blocked` set from Steps 2/3 (I2), unaffected by the new Step 1c.
4. **Is the match blast-radius actually complete?** The doc must include the full grep output (or an equivalent inventory) from §4 above, not just claim "2 sites" — a reviewer must be able to re-run `grep -rn "DenyReason" crates/ cli/` and verify the count independently. Flag explicitly that `cli/caprun/src/worker.rs` uses `matches!`/Debug-format (not an exhaustive match) so it needs NO update — a reviewer unfamiliar with the CLI code might assume it does.
5. **Does the `claim_type`-vs-role dual-vocabulary (b) create a bypass?** An attacker-controlled `doc_fragment` claim_type value could theoretically be crafted to look like it should carry a `"recipient"`-adjacent role if the expected-role table's per-slot lists are too permissive. The doc must show the exact `Some(&[...])` list contents per slot (not "TBD") so a reviewer can check for over-broad membership.
6. **Session-status interaction:** does a Draft session's role-mismatch still Deny correctly, or could a Draft-only code path skip Step 1c? Step 1c lives inside the per-arg loop that runs unconditionally BEFORE Step 0.5's session-status match — the doc should state this explicitly since it's a one-line but load-bearing ordering fact.

## Landmines

- **Mint-ordering reversal:** `mint_from_read` appends its Event before minting; `mint_from_derivation` mints before appending its Event (`quarantine.rs:342-366` vs `:702-729`). Any Phase 24 code that threads `origin_role` through both functions must not assume a uniform "mint after event" pattern — a design doc that gets this backwards will produce a wrong sequencing diagram.
- **`check-invariants.sh` Gate 3** restricts `mint_from_read(`, `mint_from_derivation(`, `.mint(` call-site TOKENS to three files (`quarantine.rs`, `server.rs`, `value_store.rs`). Adding an `origin_role` parameter is a signature change at existing call sites — safe. Phase 24 must NOT introduce any NEW call site to these functions outside those three files, or the gate fails the build.
- **`check-invariants.sh` Gate 1** forbids `EffectRequest` anywhere under `crates/` (unless annotated `<!-- planner-discipline-allow: EffectRequest -->`). Not directly implicated by this phase, but the design doc should not introduce any raw args-map-to-sink path when describing the expected-role table — keep it a lookup over `(SinkId, arg_name) -> Option<&[&str]>`, never a new effect-dispatch shape.
- **`#[cfg(target_os = "linux")]` split:** per `CLAUDE.md`, ALL live security-enforcement tests (including any future acceptance test proving the swapped-recipient-subject scenario denies, per Phase 25) are Linux-only. The design doc itself needs no `cfg` gating (it's a doc, not code), but the doc should flag for Phase 25 that its held-out acceptance test must live behind `#[cfg(target_os = "linux")]` and be run via `scripts/mailpit-verify.sh`, not bare `cargo test` on the Mac dev machine.
- **`SessionStatus` exhaustive match (`executor/src/lib.rs:176`, `_no wildcard arm_`)** is a SEPARATE exhaustive match from `DenyReason`'s two sites — do not conflate them when scoping "the blast radius." A reviewer might ask about `SessionStatus`'s match too; the doc should note it is UNCHANGED by this phase (Step 1c doesn't touch session status) to preempt confusion.
- **Empty-vs-None ambiguity in the expected-role table return type** (see Adversarial-Review Angle 2) — pin the exact contract in the doc's type signature, don't leave it to Phase 24's implementer to guess.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-sink-arg role constraint lookup | A general pluggable role/policy framework, a config file, a Cedar-style rule engine | A hardcoded `match sink.0.as_str() { ... }` function, same file/pattern as `sink_sensitivity.rs` | `PLAN.md`/`CLAUDE.md` lock policy as hardcoded-in-Rust-TCB (CON-i2-non-bypassable); a general framework is explicitly out of scope for this milestone and would itself need a design gate. |
| Role vocabulary | A new enum type unifying `claim_type` and role tags | Reuse `claim_type`'s existing `String` values directly as role tags (see Design Decision (b)) | DESIGN-08 explicitly requires unification, not a parallel taxonomy; a new enum would be exactly the "parallel taxonomy" the requirement forbids. |

**Key insight:** This phase's entire scope is deliberately narrow (2 live sinks, additive tag, hardcoded table) — any solution that looks more general than `sink_sensitivity.rs`'s existing precedent is over-scoped for v1.5 and should be flagged in the adversarial review, not built.

## Package Legitimacy Audit

Not applicable — this phase produces a design document only; no `crates/executor` or `crates/brokerd` code, and no external crate dependencies are introduced. Phase 24 (enforcement) is likewise expected to add zero new Cargo dependencies (the recommended shape uses only `Option<String>`, `&'static str` arrays, and existing `serde` derives already in use throughout `runtime-core`).

## Common Pitfalls

### Pitfall 1: Treating role-mismatch as confirmable
**What goes wrong:** Implementer folds the new check into the existing `BlockedPendingConfirmation` collect loop because it's the "obvious" place sensitivity-adjacent checks go.
**Why it happens:** `BlockedPendingConfirmation` and the sensitivity loop are the most recently-touched, most visible part of `submit_plan_node` (Phase 14 D-14); a role check "feels" similar.
**How to avoid:** Design doc must state explicitly (per Decision (d)) that role mismatch is NEVER confirmable and must return `Denied`, with the precedence table reproduced verbatim.
**Warning signs:** A `BlockedArg`/`SinkBlockedAnchor` shape gaining a new optional `role` field instead of a dedicated `DenyReason` variant.

### Pitfall 2: Empty-list-vs-None ambiguity in the expected-role table
**What goes wrong:** `expected_role()` implemented so that "no entry for this slot" and "this slot allows nothing" both return an empty collection, collapsing the fail-closed-vs-unconstrained distinction DESIGN-10 depends on.
**Why it happens:** Rust's `Option<&[&str]>` vs bare `&[&str]` is an easy detail to blur when copy-pasting `sink_sensitivity.rs`'s `bool`-returning precedent (which has no such distinction to make).
**How to avoid:** Pin the exact return type and its two-state contract in the design doc (see Decision (d) fail-closed default and Adversarial Angle 2).
**Warning signs:** A `.unwrap_or(&[])` anywhere in the Phase 24 implementation of the lookup function.

### Pitfall 3: Inheriting/unioning role across `mint_from_derivation` inputs
**What goes wrong:** Implementer computes the derived value's role as `inputs[0].origin_role` or a union of input roles, "for consistency with how taint is unioned."
**Why it happens:** `mint_from_derivation`'s taint IS unioned across inputs (`quarantine.rs:603-613`) — an implementer pattern-matching on that code will naturally reach for the same shape for role.
**How to avoid:** Design doc must state role is a function of `transform_kind` ONLY, never of input roles (Decision (c), Adversarial Angle 1).
**Warning signs:** Any code path where `origin_role` is read from `inputs` inside `mint_from_derivation`.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `"recipient"` is the correct hardcoded role for `Concat`-derived values (rather than, e.g., a more generic `"derived_address"` tag) | Design Decision (c) | If wrong, legitimate concat-derived recipients could be denied at the `to` slot, breaking the existing live email flow — must be confirmed against `planning-docs/DESIGN-confirm-binding.md`'s framing of this exact scenario during doc authoring, not left to Phase 24 to discover. |
| A2 | `file.create`'s `contents` arg should remain role-unconstrained for v1.5 (no known-safe role vocabulary for arbitrary file content) | Design Decision (d), item 3 | If a reviewer disagrees, `contents` may need its own role tag (e.g. `"file_body"`) mirroring `body`'s treatment — low risk, but changes the expected-role table's completeness claim. |
| A3 | No other exhaustive match over `ExecutorDecision` (the outer enum, not `DenyReason`) exists that would also need a new arm for a `SlotTypeMismatch` case — since the recommendation reuses the EXISTING `Denied { reason }` variant rather than adding a new `ExecutorDecision` variant, this should be moot, but was not independently re-verified with a second grep pass scoped to `ExecutorDecision::` (only `DenyReason` was exhaustively re-verified). | Existing-Code Map §4 | Low — if a new `ExecutorDecision` variant were needed (it isn't, per the recommendation), the blast radius would be larger; worth a final grep during doc authoring as a sanity check. |

**All other claims in this research are `[VERIFIED: direct code read]`** — every file:line citation above was read directly from the repository during this research session (no external library/documentation lookups were needed; this phase is pure internal-architecture research).

## Open Questions

1. **Exact literal string for the new `DenyReason` variant's public code (`.code()` return value)**
   - What we know: existing convention is `snake_case`, matching the variant name (e.g. `DraftOnlySessionDeniesCommitIrreversible` → `"draft_only_session_denies_commit_irreversible"`).
   - What's unclear: whether the design doc should name the variant `SlotTypeMismatch` (matches ROADMAP.md's own phrasing) or something more specific like `RoleSlotMismatch` / `OriginRoleMismatch`.
   - Recommendation: name it `SlotTypeMismatch` to match `ROADMAP.md`'s and `REQUIREMENTS.md`'s own vocabulary exactly (reduces doc-to-requirement traceability friction); confirm during doc authoring, not a blocking question for research.

2. **Whether `mint_from_derivation`'s hardcoded `"recipient"` role should be validated against a FUTURE second `TransformKind`**
   - What we know: only `Concat` exists today; adding a variant is a compile-time-forced decision at the same match (§3/(c)).
   - What's unclear: nothing blocking — this is inherently forward-looking and the existing fail-closed-on-unknown-transform_kind pattern already handles it structurally.
   - Recommendation: design doc should state this as a residual note (mirrors `DESIGN-session-trust-coherence.md`'s "named v2 obligation" convention) rather than attempt to design for transforms that don't exist yet.

## Validation Architecture

This phase's deliverable is a Markdown design document, not executable code — `workflow.nyquist_validation` test-mapping does not apply to the artifact itself. The actual verification gate for Phase 23 (per `ROADMAP.md` success criterion 5) is a **fresh, non-self adversarial review** of the design doc, with every raised finding resolved — analogous in spirit to a test suite but executed as a review pass, not `cargo test`. No code-level Wave 0 gaps exist because no code is written this phase.

For traceability, Phase 24/25 (which DO write code) will need to extend:
- `crates/executor/tests/executor_decision.rs` — add cases for the new `SlotTypeMismatch` `Denied` path (mirrors existing `DraftOnlySessionDeniesCommitIrreversible`/`NonLiveSessionDeniesCommitIrreversible` test shapes at `:371`, `:580`).
- `crates/brokerd/tests/extract_provenance_threading.rs` / `crates/brokerd/tests/s9_acceptance.rs` — existing fixtures that call `mint_from_intent`/`mint_from_read`/`mint_from_derivation` will need an `origin_role` argument threaded through once the signature changes (a mechanical, additive update, not a design question).
Quick run: `cargo test -p executor executor_decision` (workspace-buildable on Mac; the NEW Deny path itself is not Linux-gated — only the live-Mailpit-dispatch tests are). Full suite: `cargo test --workspace --no-fail-fast` plus `scripts/mailpit-verify.sh` for Linux-only paths per `CLAUDE.md`.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V5 Input Validation | Yes | This IS the enforcement mechanism — a new structural validation gate (`Denied` on role mismatch) in `submit_plan_node`, hardcoded in Rust TCB. |
| V4 Access Control | Yes (indirectly) | The expected-role table is a form of capability-scoped access control over which value-shapes may occupy which sink-arg slots. |
| V6 Cryptography | No | Not implicated — no new crypto in this phase. |
| V2/V3 Authentication/Session | No new surface | `SessionStatus` handling (Step 0.5) is explicitly UNTOUCHED by this design (see Landmines). |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Slot-type confusion (T2): a `UserTrusted` value minted for one semantic field (e.g. `body`) submitted into a different slot (e.g. `to`) by a compromised/adaptive planner | Spoofing / Tampering | This phase's entire subject — origin-role tag + expected-role table + fail-closed `Denied` (Decisions (a)-(d)). |
| Role-laundering via derivation | Tampering | Decision (c): role assigned from `transform_kind`'s verified output shape, never inherited from inputs (Adversarial Angle 1). |
| Fail-open via ambiguous "no constraint" representation | Tampering / Elevation of Privilege | Decision (d) fail-closed default + Pitfall 2's explicit `Option`-vs-empty-slice contract. |

## Sources

### Primary (HIGH confidence — direct code reads, this session)
- `crates/brokerd/src/quarantine.rs` (full file, 1699 lines — claim_type taxonomy, all three mint sites, mint-ordering)
- `crates/runtime-core/src/value_record.rs` — `ValueRecord` struct
- `crates/runtime-core/src/executor_decision.rs` (full file) — `DenyReason`, `ExecutorDecision`, `code()`, `Display`
- `crates/executor/src/value_store.rs` (partial) — `ValueStore::mint`
- `crates/executor/src/sink_sensitivity.rs` (full file) — `is_routing_sensitive`/`is_content_sensitive` pattern
- `crates/executor/src/lib.rs` (full file) — `submit_plan_node`, full step ordering
- `crates/runtime-core/src/plan_node.rs` (partial) — `PlanArg`/`SinkId`/`PlanNode`
- `crates/brokerd/src/proto.rs` (partial) — `TransformKind`/`as_mint_tag`
- `crates/brokerd/src/server.rs` (partial, lines ~1180-1364) — mint-site call sites, recipient/subject/body/path field-name context
- `cli/caprun/src/worker.rs` (partial) — confirms no exhaustive `DenyReason` match in CLI
- `scripts/check-invariants.sh` (partial) — Gate 1/2/3 text
- Workspace-wide `grep -rn "DenyReason" crates/ cli/` (excluding `/target/`) — DESIGN-07b blast-radius inventory
- `.planning/ROADMAP.md` Phase 23/24/25 entries, `.planning/REQUIREMENTS.md` DESIGN-07..10 / T2-02..08

### Secondary (MEDIUM confidence)
- `planning-docs/DESIGN-session-trust-coherence.md`, `planning-docs/DESIGN-taint-model.md` — skimmed for house-style section headers only, not for their substantive content (which is not this phase's subject).

### Tertiary
- None — no web search or external library research was required; this phase is pure internal-codebase architecture research.

## Metadata

**Confidence breakdown:**
- Existing-code map (§1-8, including the full DenyReason blast-radius inventory): HIGH — every claim is a direct file:line read this session, cross-checked with a workspace-wide grep.
- Design decision recommendations (a-d): HIGH for the grounding facts (mint signatures, match sites, transform shape); MEDIUM for the specific role-tag STRINGS chosen (`"recipient"`/`"subject"`/`"body"`/`"path"`), which are reasonable but not yet confirmed against `DESIGN-confirm-binding.md`'s exact prior vocabulary (see Assumption A1) — flag for the design-doc author to cross-check during authoring.
- Pitfalls/adversarial angles: HIGH — derived directly from reading the actual precedent code (D-14 collect-then-Block, the taint-union pattern) that a naive implementation would likely copy incorrectly.

**Research date:** 2026-07-11
**Valid until:** No expiry driver — this is internal-codebase research, not third-party API/library research; valid until the underlying code (`quarantine.rs`, `executor_decision.rs`, `lib.rs`, `sink_sensitivity.rs`) changes. Re-verify file:line citations if any Phase 24 work begins more than a few commits after this research.
