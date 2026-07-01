# DESIGN — Durable Sink-Blocked Anchor (Phase-7 ACC-07 spec) + Mint Invariants

> ✅ **SHIPPED — Phase 7 complete 2026-07-01.** Implemented in 07-01 (mint invariant + typed `DenyReason`), 07-02 (durable anchor, broker-minted `effect_id`), 07-05 (live §9). **v0-DONE genuine-taint chain proven on real Linux** — `provenance_chain[0] == read_event_id`, not stapled (s9_live_block 4/4, durable_anchor 2/2, check-invariants PASS). Load-bearing live fix: commit `9324450` threaded `parent_id` through both mint sites (were `None` → `verify_chain` false → ACC-05 silently unreachable); the two graphs stayed separate. The review status below is retained for the record. **One item carried forward:** `LocalWorkspace`=Trusted remains UNREVIEWED by a threat lane (safe by default — nothing mints it).

**Status:** REV.2 — **RESCOPED**. This is **not** a standalone TCB change and produces **no `crates/**` code from the review window**. Defect A (predicate over untrusted labels) was **already shipped by Phase 6** while this review ran; this doc is now (1) the **implementation spec for Phase-7 ACC-07** (the durable anchor, Defect B) and (2) two **routed flags** — a mint invariant for Phase 6, and a threat caveat for the roadmap.
**Date:** 2026-06-30 · rev.1 2026-07-01 (board) · **rev.2 2026-07-01 (rescoped, owner-approved Option A)**
**Board:** `#caprun-tcb-review` — `AoS-codex` (serde/hash), `matt` (scope). **`grok` (threat lane) never ran → this design was NOT adversarially threat-reviewed.** codex + matt (non-specialists) grep-verified the facts and cleared the conservative label classifications as *ship-with-caveat*, not threat validation (see §3, §11). codex read REV.2-as-written and approved it (§11); both reviewers converged and yielded.
**Supersedes:** REV.1 §3 "SPEC 1 — TrustClass/trust_class" is **withdrawn** (see §2). PLAN.md wins on any conflict.

---

## 0. Governing principle — two graphs, distinct edges, shared node ids

(Board finding codex #1; wording corrected per codex #2 + matt.)

Two graphs. They **share node identity** (the value-lineage references causal-DAG `Event.id`s) but have **distinct edge semantics**, and their edges are **never equated**:

- **Causal DAG** — edges are `Event.parent_id` / `parent_hash`, threaded on the connection chain head (`last_event_id`/`last_event_hash`). This is the hash chain `verify_chain` walks.
- **Value-lineage** — edges are `SinkBlockedAnchor.provenance_chain` / `read_event_id`, which *reference* `Event.id`s but describe value derivation, not causal ordering. Lives inside the anchor.

**Never assert `sink_blocked.parent_id == read_event_id`** (it holds only on the one-claim path; forcing it breaks `verify_chain` on multi-claim). **Keep both genuine-taint backstops** (they are *node-reference* lookups, not edge-conflation):
- anchor-internal: `anchor.read_event_id == anchor.provenance_chain[0]`.
- value↔DAG: the DAG contains a `file_read` Event with `id == anchor.provenance_chain[0]` whose taint is untrusted (`is_untrusted()`).

## 1. Status of the two defects

**Defect A — predicate over the label model — DONE in HEAD (Phase 6).** Verified against live code:
- `TaintLabel::is_untrusted()` — exhaustive `match`, no wildcard; `UserTrusted`/`LocalWorkspace` → false, other 5 → true (`crates/runtime-core/src/plan_node.rs:37-46`, commit `246c4d8`).
- Predicate is now `is_routing_sensitive(sink,arg) && record.taint.iter().any(|t| t.is_untrusted())` (`crates/executor/src/lib.rs:65-66`, commit `8fe5e7a`), with a 7-label truth table + `UserTrusted`→Allowed test (`crates/runtime-core/tests/intent_taint.rs`). HARD-02.
- **Consequence:** REV.1's `TrustClass`/`trust_class()` is redundant *and* harmful (a second partition API). **Withdrawn — reuse `is_untrusted()`.** See §2.

**Defect B — durable genuine-taint anchor — NOT done; = Phase-7 ACC-07.** The persisted `sink_blocked` event is still a bare marker (`taint: vec![]`, no `value_id`/`literal`/provenance; `crates/brokerd/src/server.rs:333-341`), and `ExecutorDecision::BlockedPendingConfirmation` is still flat (no `value_id`/`effect_id`/anchor; `executor/src/lib.rs:68-74`). This doc §4–§8 is the spec for closing it **in Phase 7**.

> **Integrity note to owner (matt).** git log still reads "v0 DONE gate green" (Phase 4). The durable genuine-taint edge does **not** exist yet — Phase 7 is what makes it true (consistent with PROGRESS-REVIEW R1/R2). Do not assume it behind you.

## 2. One partition source of truth — reuse `is_untrusted()`, do not add a second

The executor decides with the **live** `TaintLabel::is_untrusted()`; the durable anchor stores **raw labels** verbatim; a DB reader **re-derives** untrusted-ness by calling `is_untrusted()` on `anchor.taint`. **No precomputed boolean is persisted.** T-04-03 (anti-stapling) holds: the executor copies record fields verbatim; the partition only reads labels. **If anyone ever prefers a `TrustClass` enum, it must REPLACE `is_untrusted()` with a full test/doc migration — never sit alongside it.** Two partitions is worse than either.

## 3. Mint invariants — GLOBAL, enforced at the mint site (⚑ ROUTED TO PHASE 6)

Both reviewers ruled non-empty **taint** and non-empty **provenance** are **global `ValueRecord` mint invariants**, not merely block-time needs — every value already satisfies them (`mint_from_read` → `[ExternalUntrusted, EmailRaw]` + `[event_id]`; planned `mint_from_intent` → `[UserTrusted]` + `[intent_received_id]`).

**Enforce at the source (matt's hardening on codex #4/#5) — makes the invariant TRUE, not merely asserted:**
- `ValueStore::mint` **rejects** empty `taint` and empty `provenance_chain` (returns `Result`/errors). Today it accepts `vec![]` (`crates/executor/src/value_store.rs`), and tests pass `vec![]`.
- **Executor guard stays as defense-in-depth**, and moves **UP** — evaluated right after `resolve`, **before** the sensitivity/trust check — so a `[UserTrusted]` record with empty provenance **Denies** instead of reaching Allowed (the hole codex #5 found):
  ```
  record = resolve(value_id)                    // None => Denied(DanglingHandle)
  if record.taint.is_empty(): Denied(EmptyTaintInvariantViolation)
  if record.provenance_chain.is_empty(): Denied(MissingProvenanceAnchor)   // moved up — global
  if is_routing_sensitive(sink,arg) && record.taint.iter().any(is_untrusted): Block{anchor}
  else Allowed
  ```
- **Wording (codex #4):** empty taint is a **"mint-invariant violation / missing provenance label,"** NOT proof a "mint site was bypassed."
- **Typed `DenyReason`** (both reviewers, over reason-strings): `Denied { reason: DenyReason }`, `enum DenyReason { DanglingHandle, EmptyTaintInvariantViolation, MissingProvenanceAnchor }` (derive Debug/Clone/PartialEq/Serialize/Deserialize; `Display`/`code()` for text). Tests match the variant.

> **⚑ ROUTING FLAG — this belongs in Phase 6, now, not Phase 7.** Phase 6 is *authoring a new mint site this moment* (`mint_from_intent`, 06-03 in flight). Enforcing non-empty taint+provenance at `ValueStore::mint` is cheapest while mint sites are being written, so the new site is born correct. Route to the Phase-6 window. (The executor defense-in-depth guard + typed `DenyReason` can ride with either phase.)

**Threat-lane note — NOT a specialist review (grok absent; matt #2).** codex + matt are non-specialists here; they grep-verified the *facts* and judged the *conservative direction* safe, which is a ship-with-caveat, not adversarial threat validation. Facts: untrusted-tagging can never fail-open; `LlmGenerated`/`WorkerExtracted` = Untrusted are fine even though **dormant** (no live mint site tags them). empty ⇒ Denied is **fail-closed** (worst case DoS, observable via typed `DenyReason`); out-of-executor sink reach is the real fail-open, already locked by the plan-node architecture (Gate 1).

> **⚑ TRACKED CAVEAT for the roadmap — `LocalWorkspace` = Trusted is the one fail-open-capable seam. Status: UNREVIEWED BY THE THREAT LANE — do NOT record as "cleared" (matt #2).** Workspace files can carry attacker-injected content. Latent now (nothing mints `LocalWorkspace`), but the moment a mint site tags a workspace read `LocalWorkspace`→Trusted, an injected routing value reaches a sink with no confirmation. **Directly tensions HARD-02.** Practical rule for Phase 6/7: **mint workspace-derived values as `ExternalUntrusted`, not `LocalWorkspace`**, until a threat specialist rules on workspace-content trust.

## 4. Durable anchor (Defect B / Phase-7 ACC-07)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]  // codex #3
pub struct SinkBlockedAnchor {
    pub effect_id:        Uuid,            // BROKER-minted (codex #9), passed into executor
    pub sink:             SinkId,
    pub arg:              String,          // decision: String, no ArgName newtype
    pub value_id:         ValueId,
    pub literal:          String,          // byte-exact; DATA AT REST, never executed
    pub taint:            Vec<TaintLabel>, // verbatim clone from ValueRecord
    pub provenance_chain: Vec<Uuid>,       // verbatim; [0] is the root read Event id
    pub read_event_id:    Uuid,            // == provenance_chain[0]
}
```

**Rules:**
1. **Authority map (codex #4 — `ValueRecord` has only `{id,literal,taint,provenance_chain}`):** `sink ← plan_node.sink` · `arg ← PlanArg.name` · `value_id ← record.id` · `literal/taint/provenance_chain ← record` (clones) · `read_event_id ← provenance_chain[0]` · `effect_id ← broker-minted param`. T-04-03 preserved (exact clones; nothing constructed).
2. **`effect_id` minted by the BROKER, passed into `submit_plan_node` (codex #9)** — keeps the executor a pure function: `submit_plan_node(session_id, effect_id, plan_node, store)`.
3. **Two graphs (§0):** on a block, `dispatch_request` appends `sink_blocked` with `parent_id`/`parent_hash` on the **causal head** (unchanged) and sets `event.anchor = Some(anchor)`. `read_event_id` lives only in the anchor. Remove the `parent_id == read_event_id` assertion (incl. `phase5_dispatch.rs:190-194`).
4. **Persistence = serialized anchor in `payload` (not typed columns)** — §5.
5. **Hash coverage:** anchor rides in `payload`, which is in `compute_event_hash` (`audit.rs:69-85`) → tamper-evident, no DDL.
6. **Taint consistency (codex #5):** set the event's `taint` column to `anchor.taint` (not `[]`); assert `Event.taint == anchor.taint == record.taint` (order+dups). DB readers re-derive trust from `anchor.taint`.
7. **`sink_blocked ⇒ anchor=Some` enforced in the TCB (codex #8):** `append_event` rejects `event_type=="sink_blocked" && anchor.is_none()`; the block path uses one broker-owned constructor. Defect B becomes **non-persistable through the TCB** (Option still *represents* it in Rust — codex #6 precision).
8. **No effect executed** on the block path.

`build_confirmation_prompt` (`approval.rs:57`) still surfaces the byte-exact literal; in-memory decision == durable anchor.

## 5. Persistence — `Option<SinkBlockedAnchor>` on `Event`, not typed columns

Audit schema (`audit.rs`): `events` STRICT; `payload` = serialized `Event` (`:105`) and **is hashed**; `compute_event_hash` = `parent_hash ‖ id ‖ session_id ‖ event_type ‖ payload ‖ taint` (`:69-85`); `verify_chain` recomputes from **raw stored strings** (`:238-261`).

**Decision:** `#[serde(default, skip_serializing_if = "Option::is_none")] pub anchor: Option<SinkBlockedAnchor>` on `Event`. Hashed for free; `skip_serializing_if` keeps existing events **byte-identical** (and codex #2: `verify_chain` uses raw strings, so old rows survive even a future reformat); round-trips via the existing `find_event_by_type` deserialize; tamper-evident. **Rejected typed columns** (would edit `compute_event_hash` + STRICT ALTER + sparse NULLs).

**No DB migration — but a SOURCE migration is required (codex #7):** adding `Event.anchor` breaks every `Event { ... }` literal (~13 sites). Add `Event::new(...)` (sets `anchor: None`), migrate literals to it; the block path uses the anchor-setting constructor (rule 7). Add a **golden byte-fixture test** (codex #2). "No migration" is DB-only — say so in Phase 7's record.

## 6. Ratified decisions (post-board)

1. `BlockedPendingConfirmation { anchor: SinkBlockedAnchor }` (reshape) — unanimous. Breaks `s9_acceptance.rs`/`executor_decision.rs` destructuring (RED churn).
2. **Typed `DenyReason` enum** — unanimous (flipped from REV.1's reason-string).
3. **Non-empty provenance is GLOBAL**, guard moved up (§3) — unanimous.
4. `arg` stays `String` — unanimous.

## 7. Acceptance criteria (RED-first, when Phase 7/6 implement)

**Mint invariants (Phase-6-adjacent):**
- [ ] `ValueStore::mint` rejects empty taint and empty provenance (`Result`/error).
- [ ] executor: empty-taint arg → `Denied(EmptyTaintInvariantViolation)`; empty-provenance arg → `Denied(MissingProvenanceAnchor)` (**before** sensitivity/trust — so `[UserTrusted]`+empty-provenance Denies, not Allows).
- [ ] `[UserTrusted]` in `to` → **Allowed** *(already green in HEAD via Phase 6; keep as regression — predicate-unit only, `store.mint`-synthetic, NOT live e2e until `mint_from_intent`)*.

**Durable anchor (Phase-7 ACC-07):**
- [ ] block appends `sink_blocked` whose anchor has `value_id`, `literal=="accounts@ev1l.com"`, verbatim `taint`, `provenance_chain[0]==read_event_id`.
- [ ] anchor-internal `read_event_id == provenance_chain[0]`; taint consistency `Event.taint == anchor.taint == record.taint`.
- [ ] **AFTER-EXIT, DB alone:** file-backed DB, drop+reopen; `verify_chain` passes FIRST, THEN read anchor; DAG has `file_read` with `id == anchor.read_event_id` and untrusted taint (via `is_untrusted()`); `provenance_chain[0] == that id`.
- [ ] **tamper-evidence:** `UPDATE` the real `payload` column (change literal), reopen → `verify_chain` **false** (codex #6: mutate the DB, not memory).
- [ ] `append_event` of a `sink_blocked` with `anchor=None` → `Err` (codex #8).
- [ ] no `email_send_stub` event on the block path.
- [ ] NOT asserted anywhere: `sink_blocked.parent_id == read_event_id`.
- [ ] golden serde byte-fixture (anchor=None) round-trips byte-identical.

**Authoritative §9:** the dispatch-level after-exit durable test is canonical; in-process `s9_acceptance.rs` is the faster backstop (updated for the reshape).

## 8. Affected files (for Phase 7 / the routed Phase-6 item — NOT this window)

**Phase-6 routed item (mint invariant):** `crates/executor/src/value_store.rs` (reject empty at `mint`), and callers/tests updating to the `Result`.
**Phase-7 (durable anchor):** runtime-core `plan_node.rs` (reuse `is_untrusted` — no change), new `SinkBlockedAnchor` + `DenyReason`, `event.rs` (`anchor` field + `Event::new`), `executor_decision.rs` (reshape + `DenyReason`), `lib.rs` (exports); executor `lib.rs` (guards up, `effect_id` param, verbatim copy); brokerd `server.rs` (mint `effect_id`, anchor constructor), `audit.rs` (append guard — no DDL); the ~13 `Event`-literal migration; tests (`executor_decision.rs`, `phase5_dispatch.rs` drop parent==read, new `durable_anchor.rs`, `s9_acceptance.rs` reshape, golden fixture). All cross-platform (no Linux gate).

## 9. Routing & disposition (Option A — no code/gate from this window)

- **This window writes no `crates/**`** and creates **no `DESIGN-GATE-RECORD.md` entry** — there is no TCB change to gate here. When Phase 7 implements Defect B, its own GSD gate applies, consuming this doc as the spec.
- **⚑ Route to Phase 6 now:** the §3 mint invariant (`ValueStore::mint` rejects empty taint+provenance), while `mint_from_intent` is being written.
- **⚑ Route to roadmap:** the §3 `LocalWorkspace`=Trusted / HARD-02 caveat (mint workspace values as `ExternalUntrusted`) — **record it as UNREVIEWED by the threat lane, NOT "cleared"** (matt #2). This whole design was never seen by a threat specialist (grok absent).
- **REV.1 Spec-1 (`TrustClass`) withdrawn** — Phase 6 shipped `is_untrusted()`.

## 10. Out of scope (unchanged)

Cedar/Biscuit/policy engine; LLM planner; SMTP/real send; standing policy/auto-confirm; cross-host delegation; plugin system; complex approval UI; I0 seed-state; `EffectSchema` generalization; a pinned-root defense for `verify_chain` (pre-existing gap, explicitly not this work).

## 11. Board disposition log (`#caprun-tcb-review`)

| # | Reviewer | Finding | Disposition |
|---|----------|---------|-------------|
| 1 | codex | first pass: `parent_id==read_event_id` breaks `verify_chain` on multi-claim | Accepted → §0 two-graph |
| 2–9 | codex | serde byte-compat / derives / authority-map / taint-consistency / verify-then-trust / source-migration / append-guard / broker `effect_id` | All accepted → §4–§8 |
| — | matt | typed `DenyReason`; denorm guard; scope clean; integrity flag ("gate green" overclaims) | Accepted → §3/§6/§1 |
| — | matt | grok-lane swing: classifications safe; `LocalWorkspace`=Trusted seam; empty⇒Denied fail-closed | → §3 caveat |
| **1** | **codex (2nd pass)** | **REV.1 STALE: Defect A already fixed in HEAD (`246c4d8`,`8fe5e7a`); `TrustClass` is a duplicate partition** | **ACCEPTED (blocking) → Spec-1 withdrawn §2** |
| 2 | codex (2nd) | §0 absolute "share no field" false — graphs share node identity, not edges | Accepted → §0 wording |
| 4 | codex (2nd) | "bypassed mint site" wrong; harden at `ValueStore::mint` | Accepted → §3 |
| 5 | codex (2nd) | `MissingProvenanceAnchor` only in block branch; make provenance global, guard up | Accepted (blocking) → §3 |
| 6 | codex (2nd) | "non-representable" → "non-persistable through TCB" | Accepted → §4 rule 7 |
| — | matt (2nd) | owned same stale-read miss; concurs: drop Spec-1, globalize guard, enforce at mint | Accepted → REV.2 |
| — | claude | verified HEAD moved `668477b`→`b48ec6f`; rescoped to Phase-7 spec + Phase-6 mint flag (owner-approved Option A) | This doc |
| — | **codex (final)** | **read REV.2-as-written; APPROVED as the Phase-7 ACC-07 spec + Option A; every 1st/2nd-pass finding closed; no design blockers** | Approval logged; boundary: approves DESIGN+routing, NOT implementation (Phase 7 must pass §7) nor the v0-DONE claim |
| 1 | matt (post-close) | board ratified a *direction*; REV.2-as-written wasn't re-read | **Resolved** — codex's final message read the artifact and confirmed clean |
| 2 | matt (post-close) | "threat lane covered" is too strong; grok never ran; non-specialist clearance | **ACCEPTED** — header/§3/§9 now say NOT threat-reviewed; `LocalWorkspace` tracked UNREVIEWED-by-threat-lane, not cleared |
