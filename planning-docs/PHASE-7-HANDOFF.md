# Phase-7 handoff — the ACC-07 durable anchor is already specified. Implement it; don't re-derive it.

> ✅ **SHIPPED — Phase 7 complete 2026-07-01** (6/6 plans, 14/14 must-haves, live §9 green on real Linux). This handoff was consumed by the Phase-7 plans and **every constraint below was honored** (verified against 07-01…07-05: two-graph split, broker `effect_id`, `append_event` anchor-guard, verify-then-trust, taint-consistency, typed `DenyReason`, `is_untrusted()` reused — no `TrustClass`, and workspace/path values minted `[ExternalUntrusted, PathRaw]` never `LocalWorkspace`). Retained for the record.

**To:** whoever plans/executes Phase 7 (file.create Sink, Enforcement Hardening & Full Acceptance).
**From:** the `#caprun-tcb-review` design gate (reviewers `AoS-codex` serde/hash + `matt` scope; owner-approved Option A). Board-reviewed and approved as **design**.
**Scope of THIS handoff:** the **ACC-07 durable genuine-taint anchor** + the **mint invariant** + one **label-tagging constraint**. It does **not** spec `file.create`/SINK-01..04 or the HARD-04 workspace-root capability — those remain Phase 7's own per ROADMAP (though HARD-04 interacts with the tagging constraint below).

---

## 1. READ FIRST (authoritative inputs — treat as the spec, not suggestions)

- **`planning-docs/DESIGN-durable-anchor-and-label-partition.md` (REV.2)** — the ACC-07 durable-anchor implementation spec. Every design question (persistence, hashing, the two-graph model, the anchor shape, the acceptance tests) is answered there and was adversarially reviewed. **Do not re-derive the anchor design — implement this one.**
- **`planning-docs/TASK-mint-nonempty-invariant.md`** — a small, self-contained pre-req. **Land it as Phase 7's opening plan** (or a quick standalone first). It's tiny and the mint sites are freshest right now.

## 2. ALREADY DONE in Phase 6 — do NOT rebuild

- **Defect A (block predicate over untrusted labels) shipped.** `TaintLabel::is_untrusted()` (exhaustive, `crates/runtime-core/src/plan_node.rs:37`) + predicate `record.taint.iter().any(|t| t.is_untrusted())` (`crates/executor/src/lib.rs:66`). **REUSE `is_untrusted()`. Do NOT introduce a `TrustClass`/second partition API** — a duplicate partition is the exact anti-pattern the review killed.
- Both mint sites already tag non-empty: `mint_from_read` → `[ExternalUntrusted, EmailRaw]` (`quarantine.rs:161`); `mint_from_intent` → `[UserTrusted]` (`quarantine.rs:233`). The mint-invariant task just makes that enforced at the source.

## 3. LOAD-BEARING CONSTRAINTS (the board's hardening — skipping any of these re-opens Defect B or breaks the hash chain)

1. **Two graphs, never conflated.** Causal DAG = `Event.parent_id`/`parent_hash` on the connection chain head. Value-lineage = `anchor.provenance_chain`/`read_event_id`, which *reference* `Event.id`s but are NOT causal edges. **Never assert `sink_blocked.parent_id == read_event_id`** (delete that assertion from `phase5_dispatch.rs:190` too). **KEEP both genuine-taint backstops:** `anchor.read_event_id == anchor.provenance_chain[0]`, and the DAG contains a `file_read` Event with `id == provenance_chain[0]` carrying untrusted taint.
2. **`effect_id` minted by the BROKER, passed into `submit_plan_node`** — the executor stays a pure function (no UUID minting inside it).
3. **Persist the anchor as `#[serde(default, skip_serializing_if="Option::is_none")] anchor: Option<SinkBlockedAnchor>` on `Event`** → it rides in the `payload` column → already hashed by `compute_event_hash`. **No DDL / no DB migration.** BUT a **source migration is required**: ~13 `Event { ... }` literals break — add `Event::new(...)` (sets `anchor: None`) and migrate them; the block path uses a broker-owned constructor that sets the anchor. Add a golden byte-fixture test proving existing events serialize byte-identical.
4. **`append_event` REJECTS a `sink_blocked` event with `anchor == None`** (`Err`). This makes Defect B **non-persistable through the TCB**, not merely not-triggered.
5. **Verbatim copy (T-04-03 anti-stapling):** anchor fields are cloned, never constructed. Authority map: `sink ← plan_node.sink`, `arg ← PlanArg.name`, `value_id/literal/taint/provenance_chain ← resolved ValueRecord`, `read_event_id ← provenance_chain[0]`, `effect_id ← broker param`. The executor never sets a taint field.
6. **Typed `DenyReason` enum** (`DanglingHandle`/`EmptyTaintInvariantViolation`/`MissingProvenanceAnchor`) — not reason-strings. Non-empty taint AND non-empty provenance are **global mint invariants**; the executor guards for them **before** the sensitivity/trust check (so `[UserTrusted]`+empty-provenance Denies, not Allows).
7. **Taint consistency:** persisted `Event.taint == anchor.taint == source record.taint`. **DB readers re-derive untrusted-ness by calling `is_untrusted()` on `anchor.taint`** — never store a precomputed boolean.
8. **Reshape** `ExecutorDecision::BlockedPendingConfirmation` to carry the anchor (`{ anchor: SinkBlockedAnchor }`). This breaks destructuring in `s9_acceptance.rs` and `executor/tests/executor_decision.rs` — expected RED churn.

## 4. ACC-07 acceptance (the anti-stapling sentinel — event-order-only is INSUFFICIENT)

The authoritative §9 proof becomes a **dispatch-level, after-exit, DB-alone** test (keep in-process `s9_acceptance.rs` as a faster backstop):
- File-backed DB; drive the hostile block through `dispatch_request`; **drop + reopen** the connection.
- `verify_chain` must pass **first**, THEN trust the anchor.
- DAG has a `file_read` Event with `id == anchor.read_event_id`, untrusted taint (via `is_untrusted()`); `anchor.provenance_chain[0] == that id`; `anchor.read_event_id == provenance_chain[0]`; `Event.taint == anchor.taint == record.taint`.
- **Tamper-evidence:** `UPDATE` the real `payload` column to change the literal → `verify_chain` returns **false**.
- `append_event` of a `sink_blocked` with `anchor=None` → `Err`.
- **No effect executed** on the block path (no `email_send_stub` event).
Full criteria + affected-files list: REV.2 §7–§8.

## 5. CONSTRAINTS / CAVEATS to carry into Phase 7 planning

- **⚑ `LocalWorkspace` = Trusted is UNREVIEWED by the threat lane** (grok never ran; the reviewers were non-specialists on threat — this design was NOT adversarially threat-modeled). **When Phase 7 mints values from workspace content** (`file.create` path reads / SINK-04 / HARD-04), **tag them `ExternalUntrusted`, NOT `LocalWorkspace`**, until a threat specialist rules on workspace-content trust. Record this as "unreviewed," not "cleared."
- The v0-DONE / "gate green" line in git log predates the durable genuine-taint edge. **Phase 7 is what makes ACC-07 actually true** — don't assume it's already proven.

## 6. Board provenance

Reviewed on `#caprun-tcb-review`. `AoS-codex` gave explicit REV.2 approval (design + routing only — **NOT** an implementation approval, **NOT** the v0-DONE claim; Phase 7 must still pass every §4 criterion above). `matt` approved the calls + flagged the threat-lane caveat now recorded. Full disposition: REV.2 §11.
