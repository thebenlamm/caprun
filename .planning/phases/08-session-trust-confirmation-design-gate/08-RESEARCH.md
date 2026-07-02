# Phase 8: Session-Trust & Confirmation Design Gate - Research

**Researched:** 2026-07-01
**Domain:** Security-invariant design authoring (Rust TCB architecture) — no code, a DESIGN doc gate
**Confidence:** HIGH (this phase's substance is entirely derivable from the existing codebase, the v1.0 design-gate precedent, and locked project decisions — no external library uncertainty)

## Summary

Phase 8 does not write code. It produces one or two DESIGN docs under `planning-docs/` that
extend the already-approved `DESIGN-taint-model.md` / `DESIGN-plan-executor.md` pair with two new
mechanisms: (1) session-trust-state (I1 dynamic demotion on `mint_from_read`, I0 draft-only
creation) and (2) confirmation-release semantics for `BlockedPendingConfirmation`. The doc(s) must
be reviewed and recorded APPROVED in a `DESIGN-GATE-RECORD.md`-style artifact before Phase 9 or
Phase 10 write any executor/brokerd code — mirroring the v1.0 Phase 2 discipline exactly (see
`planning-docs/DESIGN-GATE-RECORD.md`, round 1 → round 2 → APPROVED).

The codebase inspection surfaced three concrete architectural gaps the DESIGN doc(s) MUST resolve
(not just describe abstractly) or Phase 9/10 will re-litigate them mid-implementation:

1. **No `SessionStatus::Draft` variant exists.** `runtime_core::SessionStatus` today is
   `Active | WaitingApproval | Done | Failed | RolledBack`. The doc must specify the new variant
   name, its transition rule (Active → Draft is one-way / monotonic, mirroring taint monotonicity),
   and — critically — **how the executor learns the session's trust state**, since
   `executor::submit_plan_node(session_id, effect_id, plan_node, value_store)` today takes no
   Session or trust-state parameter at all.
2. **No effect-class-to-sink mapping exists in the executor.** `PlanNode` carries only
   `{ sink: SinkId, args }`; `sink_sensitivity.rs` hardcodes routing/content-sensitive *args* per
   sink but has no notion of `Observe`/`MutateReversible`/`CommitIrreversible` per sink. TAINT-02/03
   require the executor to know a plan node's effect class to decide draft-only denial. The natural,
   pattern-consistent fix is a new hardcoded `sink_effect_class(sink: &SinkId) -> EffectClass`
   table in `crates/executor`, mirroring `is_routing_sensitive`/`is_content_sensitive` — not a new
   field on the locked `PlanNode` shape.
3. **The per-connection `ValueStore` does not survive process exit.** `caprun` is a single-shot
   process: it opens the audit DB, runs one session to completion (or to a `Block`), prints the DAG,
   and exits. `caprun confirm <effect_id>` is necessarily a *second, later* process invocation
   against the same persistent SQLite audit DB — but the in-memory `ValueStore` that resolved every
   `ValueId` in the original `PlanNode` (e.g. `file.create`'s untainted `contents` arg alongside its
   tainted `path` arg) is gone. The DESIGN doc must specify what gets durably persisted at Block
   time (beyond the existing `SinkBlockedAnchor`, which only carries the *one blocked arg*) so a
   later `confirm` process can actually re-invoke the sink — this is the single highest-risk gap for
   Phase 10, and it is exactly the "pause-and-resume with durable checkpoint state that survives
   restarts" pattern documented in current human-in-the-loop agent literature (Cloudflare Agents
   docs; Spring AI checkpoint-based pause/resume, 2026) — validating that a durable, full-plan
   checkpoint (not just the blocked anchor) is the standard shape for this problem, not a
   caprun-specific wrinkle.

**Primary recommendation:** Author one DESIGN doc (or two, paired, per the existing
`DESIGN-taint-model.md` / `DESIGN-plan-executor.md` split) that (a) adds `SessionStatus::Draft` and
threads trust-state into `submit_plan_node`'s signature as an explicit parameter resolved by the
broker from its own session store (never from IPC — same discipline as HARD-03's `session_id`), (b)
adds a hardcoded `sink_effect_class` table in `executor::sink_sensitivity`, (c) defines a new
`DenyReason::DraftOnlySessionDeniesCommitIrreversible`-shaped variant appended to the existing
single taxonomy (never a second enum), and (d) specifies a durable `PendingConfirmation` /
checkpoint record — persisted at Block time, keyed by `effect_id`, holding everything Phase 10's
`confirm` command needs to resolve and re-invoke the sink exactly once, then be marked
terminal (confirmed-once or denied-durably) so the same `effect_id` can never be replayed.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROC-01 | A DESIGN doc for session-trust-state and confirmation semantics exists and is reviewed before any executor code implementing this milestone is written | See Architecture Patterns §1-4 for exact content the doc must contain; Validation Architecture section defines the grep-based completeness checklist + human-review gate record process, mirroring `planning-docs/DESIGN-GATE-RECORD.md` round-2 APPROVED precedent |

*(TAINT-01..04, ORIGIN-01..02, CONFIRM-01..04 belong to Phase 9/10 respectively — included below only
as forward context so Phase 8's doc gives those phases enough to implement without re-litigating
design, per the phase brief.)*

## Project Constraints (from CLAUDE.md)

- Source of truth is `planning-docs/PLAN.md` — on any conflict with this research or the DESIGN
  doc, PLAN.md wins.
- **TCB is Rust.** The DESIGN doc's release path and draft-only deny decision must be specified as
  living in the `executor` crate (or `brokerd`, for state storage) — never a policy file, never
  Python.
- The effect path is locked to `submit_plan_node(session_id, PlanNode) -> ExecutorDecision` — the
  DESIGN doc must not propose reintroducing a raw `EffectRequest`-shaped path, and `check-invariants.sh`
  Gate 1 will fail the build if the token appears unguarded under `crates/`.
- Terminology is locked: `Intent`, `Session`, `Planner`, `Worker`, `Broker`, `Adapter`, `Effect`,
  `Artifact`, `Event`. The new session-trust concept must be named as a `Session` status/field, not
  a new public-API noun.
- Two design-gate docs already exist and are APPROVED (`DESIGN-taint-model.md`,
  `DESIGN-plan-executor.md`, per `DESIGN-GATE-RECORD.md` round 2). Phase 8's output should be
  additive revisions or paired new docs referencing them, not a rewrite — the existing docs' sha256
  gate-pinning convention means editing them invalidates the prior APPROVED record and would need a
  fresh gate round, so prefer new/paired docs over in-place edits unless a genuine correction is needed.
- Out of scope for this milestone (do not let the DESIGN doc scope-creep into): more sinks, a real
  LLM planner, Cedar policy engine, cross-host delegation, content-sensitive arg blocking, standing/
  pattern confirmation policy, interactive TTY confirmation.

## Architectural Responsibility Map

This project's own layer roles (`DEC-layer-roles`, PLAN.md §Layer roles) are the correct "tiers" —
generic web-app tiers (Browser/SSR/API/DB) do not apply to this Rust security substrate.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Session trust-state storage (Active/Draft) + transition | Broker (control plane) | — | Broker owns `Session`/`SessionStatus` lifecycle (`brokerd::session`); mirrors existing `create_session`/`persist_session` ownership |
| I1 demotion trigger (`mint_from_read` flips session to Draft) | Broker | Executor (reads the flag) | `mint_from_read` is already the sole broker-owned taint-mint site (`brokerd::quarantine`); demotion is a natural co-located side effect of that same call, recorded as an audit Event |
| I0 creation-time draft rule + seed-provenance field | CLI (`cli/caprun`) decides trusted-arg vs file-derived | Broker (`create_session` sets initial `SessionStatus`) | ORIGIN-01 explicitly assigns the trusted-arg/file-derived decision to the `caprun` CLI; the broker's `create_session` is the trusted path that sets `Draft` from that provenance, never self-declared by the (potentially injected) caller — this mirrors the I0 Acceptance Predicate's condition 0 in `DESIGN-taint-model.md` |
| Draft-only deny decision for `CommitIrreversible`-class plan nodes | Executor (one TCB function) | — | Locked decision in STATE.md: "draft-only deny decided in the executor, never a broker pre-check — one TCB deny function, one DenyReason taxonomy" |
| Effect-class-per-sink lookup (new) | Executor (`sink_sensitivity`-style hardcoded table) | — | Consistent with the existing hardcoded `is_routing_sensitive`/`is_content_sensitive` pattern; PlanNode's shape stays locked (no new field) |
| Confirmation-release execution (re-invoke sink after confirm) | Broker / Adapter (sink invocation) | Executor (may re-run the decision to confirm no other block fires) | Mirrors today's `invoke_file_create` call site in `brokerd::server`, which already runs post-executor-Allowed; confirm is the same call site reached via a different (later, second-process) trigger |
| Confirmation/deny audit anchoring | Broker (audit DAG) | — | `SinkBlockedAnchor.effect_id` already exists as the anchor key (CONFIRM-04); broker already owns `append_event`/`insert_blocked_literal` |
| `caprun confirm <effect_id>` CLI surface | CLI (`cli/caprun`, new subcommand) | Broker (reopens persistent DB, dispatches release) | Same shape as the existing single-shot `caprun` binary — a second, explicit invocation, not a background daemon (per locked confirm-UX decision) |

## Standard Stack

**N/A for this phase.** Phase 8 produces Markdown design documents only — no crate changes, no new
Cargo dependencies, no new external packages. All types referenced below already exist in
`runtime-core`/`executor`/`brokerd` (see Code Examples). The stack constraint from PROJECT.md
(`CON-stack-tcb`: Rust, tokio, serde, sqlx/rusqlite, nix/rustix, landlock, seccompiler, ed25519-dalek)
is unchanged and carries forward unmodified into Phase 9/10.

## Package Legitimacy Audit

**N/A — no packages installed in this phase.** Phase 8 is documentation-only; the Package
Legitimacy Gate protocol does not apply. Phase 9/10 (which do add executor/brokerd code but are
not expected to add new Cargo dependencies either, per the unchanged stack constraint) should
re-run this gate only if they introduce a new crate dependency.

## Architecture Patterns

### System flow this DESIGN doc must specify (annotated with the gaps found)

```
 ┌─────────────────────────── caprun (process 1, single run) ───────────────────────────┐
 │                                                                                        │
 │  CLI parses intent-kind + seed source ──▶ [GAP: ORIGIN-01 needs a CLI-level marker    │
 │  (trusted-arg vs file-derived) — no such flag exists on today's positional-arg CLI]   │
 │           │                                                                            │
 │           ▼                                                                            │
 │  brokerd::session::create_session(intent_id)                                          │
 │    → SessionStatus::Active   [GAP: needs Draft variant + I0 rule: if seed provenance  │
 │                                is file-derived, start Draft, never self-declared]      │
 │           │                                                                            │
 │           ▼                                                                            │
 │  worker connects, self-confines, sends RequestFd/ReportClaims/SubmitPlanNode over UDS  │
 │           │                                                                            │
 │           ▼                                                                            │
 │  brokerd::quarantine::mint_from_read(...)                                             │
 │    → appends file_read Event (tainted)                                                │
 │    → mints ValueRecord in per-connection ValueStore                                   │
 │    [GAP: TAINT-01 needs this SAME call site (or one adjacent to it) to ALSO flip the  │
 │     session's in-memory/persisted trust state to Draft + append a session_demoted     │
 │     Event with parent_id = the file_read event's id (TAINT-04 causal edge)]           │
 │           │                                                                            │
 │           ▼                                                                            │
 │  executor::submit_plan_node(session_id, effect_id, plan_node, value_store)             │
 │    [GAP: signature has NO session-trust-state input today. Must add a parameter —      │
 │     e.g. `session_status: &SessionStatus` or `is_draft: bool` — resolved by the        │
 │     BROKER from ITS OWN session store, never trusted from the plan_node or IPC.]       │
 │    Step 0: sink_schema::validate_schema                                                │
 │    Step 0.5 [NEW]: sink_effect_class(sink) == CommitIrreversible && session is Draft   │
 │        → Denied { reason: DenyReason::<new variant> }   [ONE taxonomy, ONE function]   │
 │    Step 1-3: existing resolve / empty-taint / empty-provenance / routing-sensitivity   │
 │           │                                                                            │
 │           ▼ (BlockedPendingConfirmation)                                               │
 │  brokerd::server appends sink_blocked Event (SinkBlockedAnchor, keyed by effect_id)    │
 │    [GAP: only the ONE blocked arg's ValueRecord is captured in the anchor. A           │
 │     multi-arg sink (file.create: path + contents) needs its OTHER resolved args        │
 │     persisted too, or `confirm` cannot reconstruct the PlanNode to re-invoke the sink.  │
 │     RECOMMENDATION: persist a durable PendingConfirmation record at Block time         │
 │     containing the full resolved arg set (literal+taint+provenance per arg), not just  │
 │     the anchor's single blocked arg.]                                                  │
 │           │                                                                            │
 │  caprun process exits (non-zero) ──────────────────────────────────────────────────────┘
                                    │
                                    │  (durable SQLite audit DB persists across process boundary)
                                    ▼
 ┌──────────────────────── caprun confirm <effect_id> (process 2, LATER) ────────────────┐
 │                                                                                        │
 │  Reopen the SAME persistent audit DB (NOT ":memory:")                                 │
 │  Look up the sink_blocked Event whose anchor.effect_id == <effect_id>                 │
 │    [GAP: no query-by-effect_id helper exists in brokerd::audit today — only            │
 │     find_event_by_type/query_events_by_session. Needs `find_event_by_effect_id` or     │
 │     equivalent, scanning anchor payloads, OR effect_id becomes an indexed column.]     │
 │  Check: has this effect_id already been confirmed or denied?                          │
 │    [GAP: needs a terminal-state marker distinct from sink_blocked itself — e.g. a       │
 │     confirm_granted / confirm_denied Event anchored to the same effect_id — CONFIRM-03 │
 │     requires "cannot be re-confirmed," which means this check MUST be durable, not      │
 │     in-memory, since it must survive across separate `confirm` invocations too.]       │
 │  On confirm: display verbatim literal + provenance (CONFIRM-01), append confirm_granted│
 │    Event anchored to effect_id, then RE-INVOKE the sink using the persisted            │
 │    PendingConfirmation record (releases exactly this (sink,arg,literal-digest) triple — │
 │    CONFIRM-02 — never a session-wide waiver)                                          │
 │  On deny: append confirm_denied Event anchored to effect_id; durable — no retry path   │
 └────────────────────────────────────────────────────────────────────────────────────────┘
```

### Pattern 1: Single TCB deny function, single taxonomy (carry forward from Phase 7)

**What:** `executor::submit_plan_node` is the ONE function that returns `ExecutorDecision::Denied`.
`DenyReason` is the ONE enum. Phase 7's own doc comment on `DenyReason` states this explicitly:
"the ONE base denial error enum ... never introduce a second denial error type."

**When to use:** Every new deny condition (draft-only session denies `CommitIrreversible`) is a new
variant appended to `DenyReason`, checked inside `submit_plan_node`, in the same match-exhaustive
style already used for `TaintLabel::is_untrusted()` (explicit match, no wildcard arm, so a future
variant is a compile error if unhandled — see Pitfall below).

```rust
// Source: crates/runtime-core/src/executor_decision.rs (existing, read this session)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DenyReason {
    DanglingHandle,
    EmptyTaintInvariantViolation,
    MissingProvenanceAnchor,
    UnknownSink(String),
    UnknownArg(String),
    DuplicateArg(String),
    MissingArg(String),
    // Phase 9 adds, e.g.:
    // DraftOnlySessionDeniesCommitIrreversible { sink: SinkId },
}
```

### Pattern 2: Genuine-anchor discipline extends to session-status, not just taint

**What:** `DESIGN-taint-model.md`'s I0 Acceptance Predicate condition 0 already states the exact
principle Phase 8 must restate for I1 dynamic demotion: *"The tainted-seed determination is made by
the trusted brokerd session-creation path from the seed's provenance ... NOT self-declared by the
agent creating the Session."* The DESIGN doc must state the parallel rule for I1: **the session's
`Draft` transition on `mint_from_read` is set by the same trusted broker function that mints the
tainted `ValueRecord` — never by the worker, never by a flag the worker's IPC message could carry.**
This is the same anti-stripping structure as the `ValueId` handle model, applied to session state
instead of value state.

**When to use:** Any time the doc specifies *where* the Draft flip happens, state it as "inside
`mint_from_read` (or a function it calls under the same lock/transaction)," never "the worker
reports it is now tainted."

### Pattern 3: Two-table event-sourcing (append-only ledger + mutable read-model) already exists

**What:** `brokerd::audit` has a strict append-only `events` table (`compute_event_hash`/`verify_chain`
assert this) plus a separate `sessions` table that IS a live row per session (currently write-once
via `persist_session`'s `INSERT`, no `UPDATE`). Session demotion needs an `UPDATE sessions SET
status = 'Draft' ...` (the mutable read-model) **in addition to** an append-only `session_demoted`
audit Event (the ledger) with `parent_id` = the triggering `file_read` event id (TAINT-04's causal
edge). The DESIGN doc must state both writes happen atomically (same lock/transaction) so the
read-model and the ledger can never disagree.

```rust
// Source: crates/brokerd/src/session.rs (existing — read this session)
pub fn persist_session(conn: &rusqlite::Connection, session: &Session) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, intent_id, status, created_at) VALUES (?1, ?2, ?3, ?4)",
        // ... no UPDATE path exists yet — Phase 9 needs one, but Phase 8's DESIGN doc
        // should specify the contract (one UPDATE, monotonic Active→Draft only) so Phase 9
        // doesn't invent an ad hoc schema.
    )?;
    Ok(())
}
```

### Pattern 4: Durable checkpoint for pause/resume across a process boundary

**What:** Because `caprun` is single-shot and `caprun confirm` is a later, separate process, the
confirmation-release mechanism is structurally a **pause-and-resume workflow with a durable
checkpoint**, not an in-memory callback. This matches the standard external pattern for
human-in-the-loop agent tool calls: persist the full pending-action payload before halting, so a
later process can resume from exactly that checkpoint, survive restarts, and never re-execute a
consumed approval [CITED: Cloudflare Agents docs — human-in-the-loop patterns; Spring AI
checkpoint-based pause/resume pattern, 2026].

**When to use:** The DESIGN doc should specify a `PendingConfirmation` (or similarly named) durable
record, written atomically with the `sink_blocked` Event, containing: `effect_id`, `session_id`,
`plan_node` (or enough of it — sink + every arg's resolved `ValueRecord`, not just the blocked one),
and a terminal-state field (`Pending | Confirmed | Denied`) that a later `confirm` invocation reads,
transitions exactly once, and never re-transitions (CONFIRM-03's durability requirement literally
requires this to be checked from persisted state, since the granting/denying process is not the
same process that created the block).

### Recommended DESIGN doc structure (mirrors the existing approved pair)

```
planning-docs/
├── DESIGN-session-trust-state.md      # I1 demotion + I0 creation rule + new DenyReason variant
│                                       # + effect-class-per-sink table
└── DESIGN-confirmation-release.md     # PendingConfirmation checkpoint schema, caprun confirm
                                        # CLI contract, single-shot release semantics, durable deny
```

Alternatively, a single paired doc is acceptable if it stays as rigorous as the existing two — the
project precedent (`DESIGN-GATE-RECORD.md`) reviews docs as a pair regardless of file count, so
either shape satisfies PROC-01 as long as both mechanisms (I1/I0 demotion, confirmation release) are
each specified with the same MUST/MUST-NOT rigor as the existing docs.

### Anti-Patterns to Avoid

- **Deciding draft-only denial in the broker before calling the executor.** Locked decision
  explicitly rejects this — "never a broker pre-check." The broker's job is only to resolve and pass
  in the session's trust state; the executor's job is the deny decision.
- **Adding a second `DenyReason`-like enum for draft-only denials.** Must extend the existing
  taxonomy (see Pattern 1).
- **Treating `caprun confirm` as if it runs in the same process/memory as the original block.** It
  does not — see Pattern 4. Any design that references "the ValueStore" as if it is still populated
  after `confirm` runs is unimplementable without a durable checkpoint.
- **Standing/pattern confirmation policy.** Explicitly out of scope (STATE.md, REQUIREMENTS.md
  Out-of-Scope table) — confirm releases exactly one `(sink, arg, literal-digest)` triple.
- **Interactive TTY prompt for confirmation.** Locked UX decision is the second-command shape
  (`caprun confirm <effect_id>`), for testability and headless use.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Pending-approval workflow persistence | A bespoke workflow/state-machine engine | A minimal, single-purpose `PendingConfirmation` row in the existing SQLite audit DB, following the same append/side-table pattern already used for `blocked_literals` | The project already has exactly this shape (redactable side table keyed by event id); a new generic workflow engine would be scope creep the milestone explicitly excludes (Cedar/rich policy engine out of scope) |
| Confirm/deny audit trail | A new audit subsystem | The existing `events` table + `SinkBlockedAnchor.effect_id` anchor (CONFIRM-04 explicitly anchors to it) | Reuse the identical append-only, hash-chained mechanism already proven in Phase 4/7 |
| Effect-class lookup | A dynamic/config-driven policy table | A hardcoded Rust match in `executor::sink_sensitivity` (or a sibling module), mirroring `is_routing_sensitive` | Consistent with `CON-i2-non-bypassable` — sensitivity/class is a security property, not a config knob |

**Key insight:** Nothing in Phase 8's scope requires new infrastructure. Every mechanism it must
specify (durable checkpoint, append-only audit anchor, hardcoded sensitivity table, single deny
taxonomy) already has a proven, approved analog in the shipped v1.0/v1.1 code. The DESIGN doc's job
is to extend these patterns by exact analogy, not invent new ones.

## Common Pitfalls

### Pitfall 1: Specifying draft-only denial without specifying how trust state reaches the executor
**What goes wrong:** The DESIGN doc states "draft-only sessions deny CommitIrreversible" but never
resolves that `submit_plan_node`'s current signature has no session/trust-state parameter — Phase 9
then either invents an ad hoc broker pre-check (violating the locked "executor decides" rule) or
stalls on a signature question that should have been settled at design time.
**Why it happens:** The DESIGN doc for I2 (Phase 2) never needed session state — I2 is purely a
function of the `PlanNode` and the `ValueStore`. I1/I0 introduce the first coupling between session
identity and executor decision logic.
**How to avoid:** Explicitly specify the new parameter (name, type, and — critically — that it is
resolved by the broker from ITS OWN session store, not carried on the `PlanNode` or trusted from any
worker-supplied value) in the DESIGN doc, not left as an implementation detail for Phase 9.
**Warning signs:** A DESIGN doc that says "the executor checks the session's Draft status" without
showing the updated function signature or explaining where the parameter is sourced from.

### Pitfall 2: Anchor captures only the blocked arg, silently assuming single-arg sinks
**What goes wrong:** `SinkBlockedAnchor` (existing, Phase 7) is scoped to exactly one blocked arg —
correct for I2 (only the ONE tainted routing-sensitive arg matters for the Block/Confirm decision),
but insufficient for Phase 10's confirm-and-resume, which must re-invoke the ENTIRE sink call
(`file.create` needs both `path` — the blocked one — and `contents`).
**Why it happens:** I2's anchor was designed to prove *taint genuineness for the blocked arg only*;
it was never designed to reconstruct a full effect invocation.
**How to avoid:** The DESIGN doc must introduce a distinct durable record (see Pattern 4) that is a
superset of the anchor — the full resolved `PlanNode` (all args' `ValueRecord`s) — not attempt to
overload `SinkBlockedAnchor` itself (which has its own tamper-evidence/redactability contract tied
to `payload` hashing; adding fields to it risks breaking the existing golden byte-fixture tests).
**Warning signs:** Any Phase 9/10 code that tries to call `invoke_file_create` from within `confirm`
using only fields already on `SinkBlockedAnchor`.

### Pitfall 3: Treating `caprun confirm` as safe to re-run the executor's decision logic unchanged
**What goes wrong:** If `confirm` simply re-submits the original `PlanNode` to
`submit_plan_node`, it will Block again — the value is still tainted (taint is monotonic, never
removed). The DESIGN doc must specify that confirmation is a DISTINCT authorization path — an
explicit, logged **endorsement** of the exact literal (mirroring `DESIGN-taint-model.md`'s
Declassification & Endorsement section, which already anticipates and resolves exactly this
tension for the executor's UX rule 4 "no silent learning") — not a second pass through the same
deny logic.
**Why it happens:** It is tempting to reuse `submit_plan_node` as the single re-entry point since it
is "the one TCB function," but that function's contract is "decide given current taint state," and
taint never clears. Confirmation is a human override of that decision for exactly one occurrence,
not a taint-state change.
**How to avoid:** Specify explicitly: confirm does NOT call `submit_plan_node` again. Confirm
transitions the persisted `PendingConfirmation` record to `Confirmed`, records a `confirm_granted`
Event anchored to `effect_id`, and directly invokes the sink adapter (e.g. `invoke_file_create`)
using the persisted resolved args — bypassing (not re-running) the I2 taint check for this one,
already-adjudicated, already-human-approved occurrence.
**Warning signs:** A design or implementation that calls `executor::submit_plan_node` a second time
for the same `effect_id`.

### Pitfall 4: Non-exhaustive match on the new DenyReason variant or new EffectClass enum
**What goes wrong:** `TaintLabel::is_untrusted()`'s doc comment explicitly warns about this
("Pitfall 5" in that file): a `matches!()`-style implicit wildcard arm silently treats a new,
unhandled variant as the safe case. If a future `EffectClass` or `DenyReason` variant is added
without updating every exhaustive match, the failure mode is a silent fail-open, not a compile error.
**Why it happens:** Convenient shorthand (`matches!`, `_ => ...`) is easy to reach for.
**How to avoid:** The DESIGN doc should explicitly require (as it already does for `TaintLabel`) that
any new enum introduced for effect-class or deny-reason purposes use an explicit `match` with every
variant listed and no wildcard arm, so the Rust compiler enforces exhaustiveness at every call site.
**Warning signs:** `_ => false` or `_ => EffectClass::Observe` anywhere in the new code the DESIGN
doc's examples show.

### Pitfall 5: I0's "file-derived seed" has no current CLI on-ramp
**What goes wrong:** ORIGIN-01 requires "the `caprun` CLI decides which at creation time
(trusted-arg vs file-derived)," but today's CLI (`cli/caprun/src/main.rs`) only accepts
positional args (`<intent-kind> <intent-param> <workspace-file>`) — every session today is
seeded trusted-arg by construction. There is no existing "seed the intent from a file" path to
demote. Phase 9/11 will need a new CLI surface (a flag or intent variant) to exercise ORIGIN-02
live. If the DESIGN doc is silent on this, Phase 9 may under-scope ORIGIN-01/02 as "just add a
field" without realizing a new CLI input path is also required to ever set it to file-derived.
**Why it happens:** The existing CLI was built for the narrower v1.1 scope (typed intent from
CLI args only); v1.2 is the first milestone to need an external/file-derived seed source.
**How to avoid:** The DESIGN doc should at least name the expected shape (e.g., a CLI flag such as
`--seed-from-file <path>`, or a third intent-parsing branch) even if the concrete CLI flag name is
left to Phase 9, so the "seed-provenance field" requirement is understood to require a new input
path, not just a new struct field.
**Warning signs:** A DESIGN doc or plan that treats ORIGIN-01 as a pure data-model change with no
CLI-surface implication.

## Code Examples

Existing internal patterns the DESIGN doc should reference/extend (all read directly from the repo
this session — not hypothetical):

### Exhaustive-match discipline to carry forward for any new enum
```rust
// Source: crates/runtime-core/src/plan_node.rs (existing)
pub fn is_untrusted(&self) -> bool {
    match self {
        TaintLabel::ExternalUntrusted
        | TaintLabel::EmailRaw
        | TaintLabel::PdfRaw
        | TaintLabel::LlmGenerated
        | TaintLabel::WorkerExtracted
        | TaintLabel::PathRaw => true,
        TaintLabel::UserTrusted | TaintLabel::LocalWorkspace => false,
    }
}
```

### Sole-mint-site discipline to carry forward for the new session-demotion write
```rust
// Source: crates/brokerd/src/quarantine.rs (existing) — mint_from_read is the
// SOLE broker taint-mint site; both the audit Event append AND the ValueStore
// mint happen in ONE function call so the chain is unbroken. The DESIGN doc
// should specify that session-demotion follows this exact shape: the same
// (or an adjacent, same-transaction) call also updates SessionStatus and
// appends a session_demoted Event with parent_id = the file_read event's id.
pub fn mint_from_read(/* ... */) -> Result<(Uuid, String, ValueId)> {
    // 1. append file_read Event (taint set here, never at sink time)
    // 2. mint ValueRecord (provenance_chain[0] == event_id)
    // [Phase 9 extension point]: 3. flip SessionStatus to Draft + append
    //    session_demoted Event, parent_id = event_id (TAINT-04)
}
```

### Existing sink_sensitivity hardcoded-table pattern to mirror for effect-class
```rust
// Source: crates/executor/src/sink_sensitivity.rs (existing)
pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
// Phase 9 extension point (new function, same module, same style):
// pub fn sink_effect_class(sink: &SinkId) -> EffectClass {
//     match sink.0.as_str() {
//         "email.send" => EffectClass::CommitIrreversible,
//         "file.create" => EffectClass::CommitIrreversible,
//         _ => EffectClass::Observe, // or fail-closed unknown-sink handling —
//                                    // DESIGN doc should specify explicitly
//     }
// }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| I2-only value-injection defense (v1.0/v1.1) | I0 (session-seed) + I1 (session-touch) + I2 (value-in-sink-arg), three layered invariants | v1.2 (this milestone) | Closes the "planner/intent-creation injection" residual risk explicitly accepted in `DESIGN-taint-model.md` §Accepted Residual Risks |
| Flag/log blocked effect, exit (no release path) | Durable pause/resume checkpoint + explicit single-shot human release | v1.2 Phase 10 | Turns "exit 1" into a usable runtime — matches the general industry HITL pattern of physically halting execution and persisting a pending-approval state that survives restarts [CITED: Cloudflare Agents docs; Spring AI checkpoint pattern, 2026] |

**Prior art already adopted by this project (unchanged, carry forward):** CaMeL (arXiv 2503.18813,
Google DeepMind/ETH Zurich, 2025) — P-LLM/Q-LLM structural split, variable/handle model; FIDES
(arXiv 2505.23643) — information-flow control model. Recent (2026) related architectural defenses
in the same space — IsolateGPT (execution isolation), Progent (programmable privilege control),
MELON (masked re-execution) — are relevant background but do not change caprun's approach: caprun's
differentiator (kernel enforcement + audit DAG + genuine taint chain) remains distinct from all of
these, none of which combine kernel-level confinement with a hash-chained audit DAG. [CITED: web
search this session — CaMeL/DeepMind prompt-injection defense literature summary, 2026]

**Deprecated/outdated:** None — this phase extends rather than replaces any existing mechanism.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The new `SessionStatus` variant should be named `Draft` (matching PLAN.md/DESIGN-taint-model.md's prose "draft-only status") | Architecture Patterns Pattern 3 | Low — cosmetic; any consistent name works as long as the DESIGN doc fixes it once |
| A2 | `submit_plan_node`'s new trust-state parameter should be sourced from the broker's own session store (not from `PlanNode`/IPC) | Pattern 1, Pitfall 1 | High if wrong — sourcing it from IPC would reopen the exact self-declaration hole I0's acceptance predicate condition-0 already closed for session creation; this is a security-load-bearing assumption, not a style choice, and should be explicitly confirmed in the DESIGN doc review (not just inferred from analogy) |
| A3 | Effect-class-per-sink should be a new hardcoded function in `executor::sink_sensitivity` (or a sibling module in the same crate), not a field added to the locked `PlanNode` struct | Pattern 1/Code Examples | Medium — if the reviewer prefers a `PlanNode.effect_class` field instead, that is a broader (but still workable) change to the locked shape; DEC-architectural-lock-plan-nodes locks the `submit_plan_node`/`PlanNode{sink,args}` shape's *purpose* (opaque handles, no raw EffectRequest) rather than literally forbidding any additional field, so this is a design choice, not a hard violation, but should be decided explicitly rather than left ambiguous |
| A4 | Confirmation-release should persist a full `PendingConfirmation` record (all resolved args) rather than extending `SinkBlockedAnchor` | Pitfall 2, Pattern 4 | Medium — extending the anchor is possible but risks breaking the anchor's existing golden-byte-fixture serialization tests (`DESIGN §5`); a separate record avoids that coupling entirely, but the reviewer may prefer a different persistence shape |
| A5 | ORIGIN-01's "file-derived seed" needs a new CLI input surface not present today | Pitfall 5 | Low-medium — if Phase 9 discovers an existing hidden path this research missed, the CLI-surface work item can be dropped; worth a quick recheck at Phase 9 planning time |

**If this table is empty:** N/A — see entries above; all are architecture-shape recommendations
for the DESIGN doc's authors to ratify or override during the human review gate, not claims
presented as already-decided.

## Open Questions

1. **Does the draft-only deny check run before or after the existing schema/resolve/taint-empty
   guards in `submit_plan_node`?**
   - What we know: schema validation (Step 0) already runs first, fail-closed, before any resolve.
     The routing-sensitivity taint check (Step 2) currently runs per-arg, after resolve.
   - What's unclear: whether the draft-only `CommitIrreversible` check should run as a new "Step 0.5"
     (immediately after schema validation, independent of any individual arg's taint — since it
     depends only on the session's status and the sink's effect class, not on any arg at all) or be
     interleaved with the existing per-arg loop.
   - Recommendation: Step 0.5 (session/sink-level, before the per-arg loop) is cleaner — it does not
     depend on iterating args at all, and it fails closed earliest, consistent with the existing
     "fail closed as early as possible" ordering discipline documented in `crates/executor/src/lib.rs`.

2. **What exactly is "released" on confirm — is I2's routing-sensitivity check ever re-run for the
   OTHER (non-blocked) args of a multi-arg sink, or are they trusted as-is from the original
   resolution?**
   - What we know: only the routing-sensitive `path`/`to`/`cc`/`bcc` arg triggers Block; other args
     (e.g., `contents`) may themselves have been resolved from a tainted-but-content-sensitive
     source and were NOT re-checked at Block time (content-sensitive tainted args don't block in v0,
     per the existing Step 3 comment).
   - What's unclear: whether confirm should re-verify that no OTHER arg's state has changed/degraded
     between Block and Confirm (unlikely in v0's short-lived single-run model, but worth stating
     explicitly rather than leaving implicit).
   - Recommendation: DESIGN doc should state explicitly that confirm re-invokes the sink with the
     args AS THEY WERE RESOLVED AT BLOCK TIME (frozen snapshot in the `PendingConfirmation` record),
     not re-resolved at confirm time — this is simpler, matches the "single-shot literal-digest"
     framing in CONFIRM-02, and avoids reopening a TOCTOU-shaped question.

3. **Should the `DenyReason` variant carry the offending `SinkId` (like `UnknownSink(String)` does)
   or be a bare unit variant?**
   - What we know: existing schema-validation variants (`UnknownSink`, `UnknownArg`, etc.) all carry
     the offending name as a `String` payload for audit/CLI legibility.
   - What's unclear: whether the draft-only variant should follow that convention (`DraftOnlySession
     DeniesCommitIrreversible { sink: SinkId }` or similar) for consistency.
   - Recommendation: Yes, carry the `SinkId` (and possibly the session id, though that's already
     implicit in the surrounding Event) — consistent with the existing pattern and useful for audit
     legibility; this is a low-stakes naming/shape detail the human reviewer can finalize quickly.

## Validation Architecture

> This phase produces a DESIGN doc, not code — "tests" here means the same grep-based completeness
> checklist + human adversarial review gate the project already used for Phase 2
> (`planning-docs/DESIGN-GATE-RECORD.md`), not `cargo test`.

### "Test Framework" for this phase
| Property | Value |
|----------|-------|
| Framework | Grep-based completeness checklist (bash), then human adversarial review — see `planning-docs/DESIGN-GATE-RECORD.md` round 2 for the exact precedent format |
| Config file | None — the checklist is authored per-gate-record, matching each DESIGN doc's Done-When predicate |
| Quick run command | `grep -iE "draft-only\|Draft\|dynamic.taint\|session-demot" planning-docs/DESIGN-session-trust-state.md` (example; adapt per doc's actual headings) |
| Full suite command | The 5-step "How to Verify" human review procedure — read end-to-end as an attacker, confirm every MUST/MUST-NOT, re-run `shasum -a 256` on the reviewed docs, then set Decision/Gate status in the gate record |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|---------------------|-------------|
| PROC-01 | DESIGN doc(s) exist, are complete, and are recorded APPROVED before Phase 9/10 code | grep-based completeness checklist + human review | `grep -c "MUST\|MUST NOT" planning-docs/DESIGN-session-trust-state.md planning-docs/DESIGN-confirmation-release.md` (non-zero, then human review) | ❌ Wave 0 — the DESIGN docs and the gate-record file themselves must be authored in this phase's plan |

### Sampling Rate
- **Per task commit:** re-run the grep completeness check after each DESIGN doc section is drafted.
- **Per wave merge:** full human "read as an attacker" pass per `DESIGN-GATE-RECORD.md`'s 5-step
  procedure.
- **Phase gate:** `DESIGN-GATE-RECORD.md`-equivalent artifact (e.g.
  `planning-docs/DESIGN-GATE-RECORD-v1.2.md` or an appended round in the existing file) must show
  `Decision: APPROVED` and `Gate status: UNBLOCKED` before Phase 9 or Phase 10 may begin any
  `crates/executor` or `crates/brokerd` edits for this milestone's new mechanisms.

### Wave 0 Gaps
- [ ] `planning-docs/DESIGN-session-trust-state.md` (or paired doc) — covers TAINT-01..04, ORIGIN-01..02
- [ ] `planning-docs/DESIGN-confirmation-release.md` (or paired doc) — covers CONFIRM-01..04
- [ ] A gate-record artifact (new file, or a new round appended to `DESIGN-GATE-RECORD.md`) recording
      the human review Decision/Gate status for these new docs, per PROC-01
- [ ] No cargo/pytest framework gap — this phase adds no code, so no test-file gaps apply

## Security Domain

> `security_enforcement` is not set to `false` anywhere in `.planning/config.json` (absent = enabled).
> This phase produces design decisions with direct security consequence even though it writes no
> code — the ASVS mapping below is at the design-review level, evaluated by the human gate reviewer,
> not by an automated code scanner (there is no code yet).

### Applicable ASVS Categories (design-review level)

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V1 Architecture, Design and Threat Modeling | yes | This IS the artifact — the DESIGN doc(s) themselves constitute the V1 documentation; review against the same MUST/MUST-NOT rigor as the approved I1/I2 docs |
| V4 Access Control | yes | Draft-only denial and confirmation-release must both be specified as decided by the trusted TCB (executor for deny, broker+TCB for release) — never a policy file or client-supplied flag; this is `CON-i2-non-bypassable` extended to the new mechanisms |
| V6 Cryptography | no (unchanged) | SHA-256 hash-chaining of the audit DAG is unchanged by this phase; no new crypto surface introduced |
| V8 Data Protection / Audit | yes | Session-demotion and confirm/deny events must be durably audited with causal edges (TAINT-04, CONFIRM-04) — the DESIGN doc must specify the exact `parent_id`/anchor linkage, not just "log it" |

### Known Threat Patterns for this stack (design-level, extending the existing threat model)

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Session self-declares "trusted" seed to bypass I0 | Spoofing / Elevation of Privilege | Trusted-path-only determination (Pattern 2 above) — the DESIGN doc must state this explicitly for I1's demotion trigger, not just for I0's creation rule (which already states it) |
| Replaying/re-confirming an already-decided `effect_id` | Tampering / Repudiation | Durable terminal-state check before granting confirm (CONFIRM-03) — must be checked against persisted state, not in-memory, since confirm runs in a separate process from the block |
| Re-running `submit_plan_node` on confirm silently blocks again (or, if "fixed" carelessly, silently bypasses I2 for future unrelated blocks) | Elevation of Privilege | Confirm must NOT re-invoke the general I2 decision function — it is a distinct, logged endorsement path scoped to exactly one `effect_id` (Pitfall 3) |
| Multi-arg sink confirm without full arg-set persistence executes with stale/wrong non-blocked args | Tampering | Durable `PendingConfirmation` snapshot of ALL resolved args at Block time (Pitfall 2, Pattern 4) |

## Sources

### Primary (HIGH confidence)
- `planning-docs/PLAN.md` — canonical build order, locked decisions, effect classes, architectural lock (read this session)
- `planning-docs/DESIGN-taint-model.md` — I0/I1 invariant text, genuine-taint requirement, declassification/endorsement model (read this session)
- `planning-docs/DESIGN-plan-executor.md` — ValueRecord/ValueId handle model, PlanNode schema, executor decision logic, sink sensitivity map (read this session)
- `planning-docs/DESIGN-GATE-RECORD.md` — the exact review/approval process precedent this phase's gate record must follow (read this session)
- `crates/executor/src/lib.rs`, `sink_sensitivity.rs`, `value_store.rs` — current executor decision logic, fail-closed ordering, sole-taint-writer discipline (read this session)
- `crates/runtime-core/src/{session,executor_decision,effect,plan_node,value_record}.rs` — current `SessionStatus` (no Draft), `DenyReason` taxonomy, 3-class `Effect` enum, `PlanNode`/`ValueRecord` shapes (read this session)
- `crates/brokerd/src/{session,audit,quarantine,approval,server}.rs` — session lifecycle (write-once, no UPDATE), append-only audit DAG, `mint_from_read`/`mint_from_intent` sole-mint-site pattern, confirmation-prompt builder, `SubmitPlanNode` dispatch and post-Allow sink invocation (read this session)
- `cli/caprun/src/main.rs` — single-shot process model, positional-arg-only intent seeding (confirms Pitfall 5) (read this session)
- `.planning/{REQUIREMENTS,STATE,PROJECT,ROADMAP}.md`, `planning-docs/MILESTONE-v1.2-SEED.md` — locked v1.2 scope, requirement IDs, and the recommendation already on record for executor-decides/second-command/single-shot (read this session)

### Secondary (MEDIUM confidence)
- Cloudflare Agents docs — human-in-the-loop patterns (pause/resume, durable state) [WebSearch this session]
- Spring AI checkpoint-based pause/resume pattern (Medium, 2026) [WebSearch this session]
- CaMeL/DeepMind prompt-injection defense summary and 2026 follow-on architectures (IsolateGPT, Progent, MELON) [WebSearch this session]

### Tertiary (LOW confidence)
- None used as load-bearing claims in this document.

## Metadata

**Confidence breakdown:**
- Standard stack: N/A — no new packages this phase
- Architecture: HIGH — every gap identified is grounded in direct reading of the current codebase, not inference from documentation alone
- Pitfalls: HIGH — each pitfall traces to a specific existing function/struct whose current shape does not yet support the new requirement

**Research date:** 2026-07-01
**Valid until:** Stable until the codebase's session/executor/brokerd shapes change (i.e., effectively until Phase 9/10 land) — re-check if Phase 9 planning surfaces a different `submit_plan_node` signature than assumed here.
