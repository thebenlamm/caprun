# Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance - Context

**Gathered:** 2026-06-30
**Status:** Ready for planning
**Source:** Phase-7 handoff (`planning-docs/PHASE-7-HANDOFF.md`) + board-ratified design docs

<domain>
## Phase Boundary

`file.create` becomes a real, hardened sink; every enforcement edge case from channel review is closed; the `RelativePath` claim variant completes the `ReportClaims` enum; and the **full live §9 acceptance contract** is green on a real Linux `caprun` run:
- hostile block with **genuine-taint proof** (durable, survives process exit),
- clean allow-path,
- one unbroken causal audit chain per run.

This phase is **what actually makes ACC-07 (the durable genuine-taint edge) true.** The "v0 DONE gate green" line in git log (Phase 4) predates this edge — do not assume it behind you.

**Two distinct work streams:**
1. **Fully specced — implement, do NOT re-derive.** The ACC-07 durable `SinkBlockedAnchor` and the mint invariant are board-ratified. The design docs below are the spec, not suggestions.
2. **Needs design work.** `file.create` sink internals (SINK-01..04: `openat2`, dirfd, `O_EXCL`, TOCTOU-safety) and the HARD-04 workspace-root capability model — these remain Phase 7's own to design; research covers them.
</domain>

<decisions>
## Implementation Decisions (LOCKED — from board review, do not re-open)

### Durable anchor (ACC-07 / "Defect B") — spec is DESIGN doc §4–§8
- **`SinkBlockedAnchor` struct** carries `effect_id, sink, arg, value_id, literal, taint, provenance_chain, read_event_id` (see DESIGN §4 for exact shape).
- **`effect_id` is BROKER-minted**, passed into `submit_plan_node(session_id, effect_id, plan_node, store)`. The executor stays a **pure function** — no UUID minting inside it.
- **Persistence = serialized anchor in the `payload` column**, via `#[serde(default, skip_serializing_if="Option::is_none")] anchor: Option<SinkBlockedAnchor>` on `Event`. Rides in `payload` → already hashed by `compute_event_hash`. **No DDL / no DB migration.**
- **A SOURCE migration IS required:** adding `Event.anchor` breaks ~13 `Event { ... }` literals. Add `Event::new(...)` (sets `anchor: None`), migrate literals to it; the block path uses a broker-owned anchor-setting constructor. Add a **golden byte-fixture test** proving existing events serialize byte-identical.
- **`append_event` REJECTS** a `sink_blocked` event with `anchor == None` (returns `Err`) — makes "Defect B" non-persistable through the TCB, not merely not-triggered.
- **Reshape** `ExecutorDecision::BlockedPendingConfirmation` to `{ anchor: SinkBlockedAnchor }`. Breaks destructuring in `s9_acceptance.rs` and `executor/tests/executor_decision.rs` — expected RED churn.

### Anti-stapling (T-04-03) — verbatim copy, never construct
- Anchor fields are **cloned from the resolved `ValueRecord`**, never constructed. Authority map: `sink ← plan_node.sink`, `arg ← PlanArg.name`, `value_id/literal/taint/provenance_chain ← resolved ValueRecord`, `read_event_id ← provenance_chain[0]`, `effect_id ← broker param`. The executor **never sets a taint field.**

### Two graphs, never conflated
- **Causal DAG** = `Event.parent_id`/`parent_hash` on the connection chain head. **Value-lineage** = `anchor.provenance_chain`/`read_event_id` (references `Event.id`s but are NOT causal edges).
- **Never assert `sink_blocked.parent_id == read_event_id`** — delete that assertion (incl. `phase5_dispatch.rs:190`).
- **KEEP both genuine-taint backstops:** `anchor.read_event_id == anchor.provenance_chain[0]`; and the DAG contains a `file_read` Event with `id == provenance_chain[0]` carrying untrusted taint (via `is_untrusted()`).

### One partition source of truth — REUSE `is_untrusted()`
- `TaintLabel::is_untrusted()` (exhaustive, `crates/runtime-core/src/plan_node.rs:37`) already shipped in Phase 6. **REUSE it.** Do **NOT** introduce a `TrustClass`/second partition API — a duplicate partition is the exact anti-pattern the review killed.
- The durable anchor stores **raw labels**; DB readers **re-derive** untrusted-ness by calling `is_untrusted()` on `anchor.taint`. **Never store a precomputed boolean.**
- Taint consistency: persisted `Event.taint == anchor.taint == source record.taint`.

### Mint invariant (prereq — land as Phase 7's opening plan)
- `ValueStore::mint` **rejects** empty `taint` and empty `provenance_chain` (returns `Result<ValueId, MintInvariantError>`). Spec: `TASK-mint-nonempty-invariant.md`.
- **Executor defense-in-depth guard moves UP** — evaluated right after `resolve`, **before** the sensitivity/trust check: empty-taint → `Denied(EmptyTaintInvariantViolation)`, empty-provenance → `Denied(MissingProvenanceAnchor)` (so `[UserTrusted]`+empty-provenance Denies, not Allows).
- **Typed `DenyReason` enum** (`DanglingHandle`/`EmptyTaintInvariantViolation`/`MissingProvenanceAnchor`) — NOT reason-strings.

### ACC-07 acceptance = dispatch-level, after-exit, DB-alone (event-order-only is INSUFFICIENT)
- File-backed DB; drive the hostile block through `dispatch_request`; **drop + reopen** the connection.
- `verify_chain` must pass **first**, THEN trust the anchor.
- DAG has a `file_read` Event with `id == anchor.read_event_id`, untrusted taint; `provenance_chain[0] == that id`; `Event.taint == anchor.taint == record.taint`.
- **Tamper-evidence:** `UPDATE` the real `payload` column to change the literal → `verify_chain` returns **false**.
- `append_event` of a `sink_blocked` with `anchor=None` → `Err`.
- **No effect executed** on the block path (no sink-executed event).
- Keep in-process `s9_acceptance.rs` as a faster backstop (updated for the reshape).

### Claude's Discretion (design in this phase — see RESEARCH.md)
- `file.create` sink internals: `openat2` with `RESOLVE_*` flags, dirfd anchoring, `O_EXCL` exclusive creation, TOCTOU-safe validate-then-write.
- HARD-04 workspace-root capability model (read-side prerequisite for SINK-04 write-side) — shared ONE capability model across `RequestFd` reads and `file.create` path resolution.
- `RelativePath` variant wiring into `ReportClaims`, broker validation, path resolution under the capability, taint/provenance assignment.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### ACC-07 durable anchor + mint invariant (LOCKED SPEC — implement, don't re-derive)
- `planning-docs/DESIGN-durable-anchor-and-label-partition.md` (REV.2) — the authoritative ACC-07 anchor spec. §0 two-graph model, §3 mint invariants, §4 anchor struct, §5 persistence, §6 ratified decisions, §7 acceptance criteria, §8 affected files.
- `planning-docs/TASK-mint-nonempty-invariant.md` — the self-contained mint-invariant prereq (RED-first, one atomic commit).
- `planning-docs/PHASE-7-HANDOFF.md` — the handoff summarizing both, with the "already done in Phase 6 / do not rebuild" list.

### Phase scope + requirements
- `.planning/ROADMAP.md` — Phase 7 goal + 8 Success Criteria.
- `.planning/REQUIREMENTS.md` — SINK-01..04, HARD-01/04/05/06, ACC-01/03/04/05/06/07.

### Source of truth (reuse, do not rebuild)
- `crates/runtime-core/src/plan_node.rs` — `TaintLabel::is_untrusted()` (line 37), `PlanNode`, `ValueNode`.
- `crates/executor/src/lib.rs` — the executor predicate (line 65-66); `ExecutorDecision`.
- `crates/executor/src/value_store.rs` — `ValueStore::mint` (line 42, target of the invariant).
- `crates/brokerd/src/quarantine.rs` — `mint_from_read` (line 161, `[ExternalUntrusted, EmailRaw]`), `mint_from_intent` (line 233, `[UserTrusted]`).
- `crates/brokerd/src/server.rs` — `sink_blocked` append (line 333), `submit_plan_node` wiring; broker mints `effect_id`.
- `crates/brokerd/src/audit.rs` — `compute_event_hash` (line 69-85), `verify_chain` (line 238-261), `append_event`.
- `crates/brokerd/tests/s9_acceptance.rs` — in-process §9 backstop (reshape target).
- `crates/brokerd/tests/phase5_dispatch.rs` — line 190 `parent_id == read_event_id` assertion to DELETE.
- `adapter-fs` crate — the fs-effect path (SCM_RIGHTS fd passing) for `file.create`.
</canonical_refs>

<specifics>
## Specific Ideas

- **Land the mint invariant FIRST** (Phase 7's opening plan or standalone) — it's tiny, additive, and the mint sites are freshest now. Re-grep `\.mint(` against final HEAD before starting (06-05 may have added a caller).
- The Phase-6 email hostile block became **unreachable** (intent CLI always routes a `UserTrusted` recipient into `email.send.to`); its two live tests were retired. **SC5 restores the live §9 hostile block via `file.create`** (tainted path from `mint_from_read`) — this is the required re-establishment of a continuously-proven live §9 guarantee.
- All ACC-07 anchor work is **cross-platform (no Linux gate).** The `file.create` sink enforcement (openat2/dirfd/O_EXCL) and the live e2e §9 run ARE Linux-gated (`#[cfg(target_os = "linux")]`).
</specifics>

<deferred>
## Deferred Ideas

- Cedar/Biscuit/policy engine, LLM planner, SMTP/real send, standing policy/auto-confirm, cross-host delegation, plugin system, complex approval UI, I0 seed-state, `EffectSchema` generalization, a pinned-root defense for `verify_chain` (pre-existing gap — explicitly NOT this work). (DESIGN §10.)
</deferred>

<constraints>
## Constraints / Caveats (carry into planning)

- **⚑ `LocalWorkspace` = Trusted is UNREVIEWED by the threat lane** (the threat specialist never ran). When Phase 7 mints values from workspace content (`file.create` path reads / SINK-04 / HARD-04), **tag them `ExternalUntrusted`, NOT `LocalWorkspace`**, until a threat specialist rules on workspace-content trust. Record as "unreviewed," not "cleared." Directly tensions HARD-02.
- **Already DONE in Phase 6 — do NOT rebuild:** `is_untrusted()` predicate (Defect A shipped); both mint sites already tag non-empty. Reuse; do not duplicate.
- Effect path is locked (Gate 1): the broker takes **plan nodes**, never a raw `EffectRequest`. `check-invariants.sh` fails the build if `EffectRequest` appears under `crates/`.
- TCB is Rust. The executor stays a pure, non-LLM, deterministic function.
</constraints>

---

*Phase: 07-file-create-sink-enforcement-hardening-full-acceptance*
*Context gathered: 2026-06-30 from Phase-7 handoff + board-ratified design docs*
