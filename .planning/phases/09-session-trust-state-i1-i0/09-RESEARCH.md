# Phase 9: Session Trust State (I1 + I0) - Research

**Researched:** 2026-07-06
**Domain:** Rust TCB extension — session lifecycle state machine, executor decision logic, broker audit DAG (internal architecture only; no new external libraries)
**Confidence:** HIGH

## Summary

There is no CONTEXT.md for this phase (discuss-phase was skipped; running under `--auto`). Its role
is filled by `planning-docs/DESIGN-session-trust-state.md`, approved under
`planning-docs/DESIGN-GATE-RECORD-v1.2.md` (round 2, `Decision: APPROVED`, `Gate status: UNBLOCKED`,
under the logged `DEC-ai-review-satisfies-human-gate` decision). That document is exhaustively
prescriptive — it names exact function signatures, exact enum shapes, exact ordering, and even
supplies Rust code for the new Step 0.5 predicate. **This phase is implementation, not design.** The
job of this research is to map every MUST in that document onto the actual current shape of
`crates/runtime-core`, `crates/brokerd`, `crates/executor`, and `cli/caprun` — verified by direct
reading of the live source this session — and to surface the mechanical gaps the design doc doesn't
(and isn't supposed to) resolve: which call sites break, where new per-connection state must be
threaded, and how to make TAINT-03 actually testable.

Six concrete, verified gaps drive the plan:

1. `executor::submit_plan_node` gains a new `session_status: &SessionStatus` parameter — a
   **breaking signature change** with **14 existing call sites** across `crates/brokerd/src/lib.rs`,
   `crates/brokerd/src/server.rs`, `crates/brokerd/tests/{phase5_dispatch,s9_acceptance}.rs`, and
   `crates/executor/tests/executor_decision.rs` that must all be updated in the same change.
2. `crates/brokerd/src/server.rs`'s per-connection state (`handle_connection`) currently threads
   `last_event_id`/`last_event_hash` as mutable locals across the connection's message loop, but
   tracks **no session-status equivalent at all**. A new mutable `session_status: SessionStatus`
   local must be threaded the same way, seeded from session creation and updated in place whenever
   `ReportClaims` triggers a demotion — never re-derived from IPC.
3. `brokerd::session::create_session` unconditionally sets `SessionStatus::Active` and
   `persist_session` only ever `INSERT`s — there is no `UPDATE sessions SET status=...` path in the
   codebase today. Both are needed (ORIGIN-01/02 creation-time Draft; TAINT-01/04 post-creation
   demotion).
4. `cli/caprun/src/main.rs` parses only positional args and has no file-derived-intent input surface
   at all — ORIGIN-01 cannot be exercised without a new CLI on-ramp (the design doc explicitly
   defers the exact flag name to this phase).
5. TAINT-03 (`Draft` + `Observe`/`MutateReversible` still Allowed) has no live sink to exercise: both
   entries in `KNOWN_SINKS` (`email.send`, `file.create`) are `CommitIrreversible`. The design doc
   requires Phase 9 to name a concrete test-only sink fixture; this research recommends one (see
   Common Pitfalls #3 and Code Examples).
6. `crates/brokerd/src/audit.rs`'s `sessions` schema and `find_event_by_type`/
   `query_events_by_session` are keyed by `session_id`, not `effect_id` — irrelevant to Phase 9 itself
   but worth naming so the plan doesn't confuse this phase's audit needs (session-keyed) with Phase
   10's (`effect_id`-keyed).

**Primary recommendation:** Implement `DESIGN-session-trust-state.md` verbatim — it is already
approved and answers nearly every design question. This phase's plan should be organized around the
concrete current-code gaps above, not around re-deriving the mechanism.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `SessionStatus::Draft` variant + monotonic transition rule | Domain types (`runtime-core`) | — | Pure data type, no I/O; mirrors existing `SessionStatus` definition site |
| I1 demotion trigger (`mint_from_read` sets `Draft`) | Broker / control plane (`brokerd::quarantine`) | Database (`brokerd::audit` — sessions UPDATE + events INSERT) | Broker is the trusted mint site; DB is where the atomic pair lands |
| I0 creation-time Draft (seed-provenance decision) | CLI orchestrator (`cli/caprun`) | Broker (`brokerd::session::create_session` sets status from the CLI's determination) | CLI is uniquely positioned to know how intent was supplied (argv vs file read); broker remains the trusted setter, never self-declared |
| Trust-state resolution for executor decisions | Broker (`brokerd::server` per-connection state) | Executor (`executor::submit_plan_node`, receives as a parameter) | Broker resolves and owns `session_status`; executor only consumes it — never resolves its own trust state |
| Draft-only `CommitIrreversible` deny decision (Step 0.5) | Executor (`executor::submit_plan_node`, one TCB function) | — | Locked project decision: never a broker pre-check |
| `sink_effect_class` hardcoded map | Executor (`executor::sink_sensitivity` or sibling module) | — | Mirrors existing hardcoded `is_routing_sensitive`/`is_content_sensitive` pattern |
| Demotion audit event + causal edge | Database / audit DAG (`brokerd::audit::append_event`) | — | Existing append-only, hash-chained mechanism; no new subsystem |

## Project Constraints (from CLAUDE.md)

- **TCB is Rust.** All session-trust-state, I0/I1 logic must live in Rust inside `crates/`. No Python
  anywhere in this phase's scope (Python is non-TCB-experiments-only).
- **Terminology is locked**: `Session`, `Broker`, `Worker`, `Planner`, `Adapter`, `Effect`, `Event`,
  `Artifact`, `Intent`. `ExecutionContext` never appears in public API — not relevant to this phase's
  new types, but any new struct/field naming must not violate it.
- **Effect path is locked**: the broker takes `PlanNode { sink, args: Vec<ValueNode> }` — Phase 9 MUST
  NOT add a field to `PlanNode` to carry effect class or session status (confirmed by
  `DESIGN-session-trust-state.md` §6: `sink_effect_class` is a hardcoded function keyed by `SinkId`,
  never a `PlanNode` field). `check-invariants.sh` Gate 1 (no `EffectRequest` token) is unaffected by
  this phase's changes.
- **v0 DONE / §9 acceptance must not regress.** `cli/caprun/tests/s9_live_block.rs` (Linux-only) and
  `crates/brokerd/tests/s9_acceptance.rs` (in-process) both call `executor::submit_plan_node` with the
  *old* 4-arg signature; both must be updated to pass a `session_status` (an `Active` session in
  their existing scenarios) and must still pass unchanged in outcome.
- **Linux-only security tests.** Any new Linux-gated live test this phase adds (e.g., exercising the
  new CLI flag end-to-end) follows the existing `#[cfg(target_os = "linux")]` pattern in
  `cli/caprun/tests/`; do not "fix" the macOS 0-passed result.
- **Two design-gate docs already exist and are approved** — `DESIGN-taint-model.md`,
  `DESIGN-plan-executor.md` (v1.0/v1.1, prior), plus the two v1.2 docs
  (`DESIGN-session-trust-state.md`, `DESIGN-confirmation-release.md`, approved 2026-07-06). Phase 9
  is cleared to write `crates/executor`/`crates/brokerd` code per
  `planning-docs/DESIGN-GATE-RECORD-v1.2.md`'s Gate status: UNBLOCKED.
- **Out of scope this milestone** (do not build): more sinks beyond `email.send`/`file.create`
  (except the test-only fixture sink this research recommends — see Pitfall 3, which is explicitly
  anticipated by the design doc, not scope creep), Cedar/policy engine, cross-host delegation,
  content-sensitive arg blocking, standing confirmation policy, interactive TTY prompts.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TAINT-01 | Session demoted to draft-only when `mint_from_read` taints a value | §2 of DESIGN-session-trust-state.md; verified current `mint_from_read` (crates/brokerd/src/quarantine.rs:177-238) has no session-status write today — this is the extension point |
| TAINT-02 | Draft-only session denies `CommitIrreversible` plan nodes that do not already Block on I2, via new `DenyReason`, decided in the executor | §6-§8 of the design doc; verified current `submit_plan_node` (crates/executor/src/lib.rs:43-142) ends at `Allowed` with no class-deny step — Step 0.5 is a new, final addition before that return |
| TAINT-03 | Draft-only session still allows `MutateReversible`/`Observe` plan nodes | §9 non-regression MUST + explicit test-vehicle requirement; verified both live sinks in `KNOWN_SINKS` (crates/executor/src/sink_schema.rs:40-54) are `CommitIrreversible` — untestable without a new fixture (see Pitfall 3) |
| TAINT-04 | Demotion recorded as audit event with causal edge to the triggering read event | §5 of the design doc (two-table atomic UPDATE + append); verified `sessions` table is INSERT-only today (crates/brokerd/src/session.rs:37-49, crates/brokerd/src/audit.rs:26-31) — new UPDATE path required |
| ORIGIN-01 | Session creation records seed-provenance field; `caprun` CLI decides trusted-arg vs file-derived | §3 of the design doc; verified `cli/caprun/src/main.rs` has only positional-arg parsing (lines 45-75) — no file-derived-intent path exists to decide between |
| ORIGIN-02 | Externally-seeded session starts draft-only, cannot auto-authorize Tier 3+ | §3 + §11 condition 2 of the design doc; verified `create_session` (crates/brokerd/src/session.rs:18-27) unconditionally sets `Active` — must become conditional |
</phase_requirements>

## Standard Stack

No new external packages this phase. Every change is an extension of existing crates already in the
workspace (`runtime-core`, `brokerd`, `executor`, `cli/caprun`) using dependencies already declared in
their `Cargo.toml` files (`serde`, `serde_json`, `rusqlite`, `uuid`, `chrono`, `tokio`, `anyhow`). No
`cargo add` is required. **Package Legitimacy Audit is not applicable** — skipping per the protocol's
own scope ("required whenever this phase installs external packages").

## Architecture Patterns

### System Architecture Diagram — I1 demotion + Step 0.5 deny, end to end

```
 CLI parse (cli/caprun/src/main.rs)
   │
   ├─ ORIGIN-01: CLI decides seed-provenance (trusted-arg vs file-derived)
   │      │
   │      ▼
   │  create_session(intent_id, seed_provenance)  [NEW param]
   │      │  seed_provenance = file-derived → status = Draft   (ORIGIN-02)
   │      │  seed_provenance = trusted-arg   → status = Active
   │      ▼
   │  persist_session()  (INSERT sessions row, status already set)
   │      │
   │      ▼
   │  run_broker_server(..., initial_session_status)  [NEW param, threaded like
   │                                                    initial_last_event_id/hash]
   │
   ▼
 handle_connection loop (crates/brokerd/src/server.rs)
   session_status: SessionStatus  ◄── mutable local, seeded from initial_session_status
   │
   ├─ ReportClaims { claims } ─────────────────────────────────┐
   │     for each claim:                                       │
   │       mint_from_read(conn, store, session_id, claim, ...)  │  TAINT-01 / TAINT-04
   │         1. append file_read Event (taint set here)         │
   │         2. mint tainted ValueRecord                        │
   │         3. [NEW] UPDATE sessions SET status='Draft'        │  atomic pair,
   │         4. [NEW] append session_demoted Event               │  same tx/lock
   │            (parent_id = file_read event id)                │
   │       session_status = SessionStatus::Draft  [NEW: update  │
   │            the threaded local after mint_from_read returns]│
   └──────────────────────────────────────────────────────────┘
   │
   ├─ SubmitPlanNode { plan_node } ─────────────────────────────┐
   │     effect_id = broker-minted Uuid                         │
   │     decision = executor::submit_plan_node(                 │
   │         session_id, effect_id, &plan_node, value_store,    │
   │         &session_status)   [NEW 5th param, broker-resolved,│
   │                             never from plan_node/IPC]      │
   │         │                                                  │
   │         ▼  (inside executor::submit_plan_node)             │
   │     Step 0: sink_schema::validate_schema  (unchanged)      │
   │     Steps 1/1a/1b/2/3: per-arg resolve/taint/routing-Block │
   │         (unchanged; if any arg Blocks → return here,       │
   │          Step 0.5 never reached — I2 precedence, B1 fix)   │
   │     Step 0.5 [NEW]: match *session_status {                │
   │         Draft if sink_effect_class(sink) ==                │  TAINT-02
   │           CommitIrreversible => Denied {                   │
   │             DraftOnlySessionDeniesCommitIrreversible }      │
   │         Draft (otherwise) => fall through to Allowed        │  TAINT-03
   │         Active => fall through to Allowed                   │
   │         WaitingApproval|Done|Failed|RolledBack => no-op     │
   │       }                                                     │
   │     → Allowed                                              │
   └──────────────────────────────────────────────────────────┘
```

### Recommended file-level change map (no new crates, no new top-level modules)

```
crates/runtime-core/src/
├── session.rs              # add SessionStatus::Draft variant
├── executor_decision.rs    # add DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink }
                             # (EffectClass enum may live here OR in executor — see Open Questions)
crates/executor/src/
├── lib.rs                  # submit_plan_node gains `session_status: &SessionStatus` param;
                             # new Step 0.5 block after the per-arg loop
├── sink_sensitivity.rs     # new `sink_effect_class(sink: &SinkId) -> EffectClass` fn (or sibling
                             # module `effect_class.rs`), fail-closed unknown-sink handling
├── sink_schema.rs           # (only if adding a test-only sink — see Pitfall 3)
crates/brokerd/src/
├── session.rs               # create_session gains seed-provenance param; new
                             # update_session_status(conn, session_id, SessionStatus) fn (new — no
                             # UPDATE path exists today)
├── quarantine.rs            # mint_from_read performs the atomic UPDATE + session_demoted append
├── server.rs                 # thread `session_status` per-connection (mirrors last_event_id/hash);
                              # run_broker_server/handle_connection gain initial_session_status param;
                              # SubmitPlanNode arm passes &session_status into submit_plan_node
├── lib.rs                    # brokerd::submit_plan_node wrapper also needs the new param (or an
                              # explicit decision to deprecate it — see Open Questions)
cli/caprun/src/
├── main.rs                   # new CLI on-ramp for file-derived intent (e.g. `--seed-from-file`);
                              # decide seed-provenance; pass to create_session
```

### Pattern 1: Sole-mint-site discipline extends to session demotion (carry forward from Phase 4/7)

**What:** `mint_from_read` is documented as the SOLE broker taint-mint site (T-04-03). This phase
extends that same function to also be the SOLE session-demotion site — no other function may set
`SessionStatus::Draft` for the I1 reason.
**When to use:** Any time a session's trust state changes as a *reaction* to reading untrusted
content.
**Example:**
```rust
// Source: crates/brokerd/src/quarantine.rs (current, lines 177-238) — extension point marked
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_id: Option<Uuid>,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId)> {
    // ... existing Steps 1-3 (file_read Event append, ValueRecord mint) unchanged ...

    // NEW Step 4 (TAINT-01/TAINT-04): atomic demotion pair, same connection/lock as above.
    // 4a. UPDATE sessions SET status = 'Draft' WHERE id = ?1
    crate::session::update_session_status(conn, session_id, &SessionStatus::Draft)?;
    // 4b. append session_demoted Event, parent_id == event_id (the file_read event just appended)
    let demoted_event = Event::new(
        Uuid::new_v4(),
        Some(event_id),                 // causal edge to the triggering read (TAINT-04)
        session_id,
        "broker".into(),
        "session_demoted".into(),
        Utc::now(),
        vec![],
    );
    append_event(conn, &demoted_event, Some(&read_hash))?;

    Ok((event_id, read_hash, value_id))
}
```

### Pattern 2: Trusted-path-only sourcing for `session_status` (broker resolves, never IPC/PlanNode)

**What:** `session_status` must be resolved by the broker's own state, passed as a parameter into
`submit_plan_node` — identical discipline to how `session_id` is already sourced from the connection,
never from the IPC message (HARD-03).
**When to use:** Any executor input whose value an injected worker/planner could otherwise assert.
**Example:**
```rust
// Source: crates/brokerd/src/server.rs (current dispatch_request, SubmitPlanNode arm, line ~347) —
// extension point. `session_status` is a NEW mutable local threaded exactly like
// `last_event_id`/`last_event_hash` are threaded today across handle_connection's message loop.
BrokerRequest::SubmitPlanNode { plan_node } => {
    let effect_id = Uuid::new_v4();
    let decision = executor::submit_plan_node(
        session_id,
        effect_id,
        &plan_node,
        value_store,
        session_status,       // &SessionStatus — broker-owned, never from `plan_node` or IPC
    );
    // ... existing audit-append / sink-invocation logic unchanged ...
}
```

### Pattern 3: Exhaustive match, no wildcard, for every new enum (carry forward from `TaintLabel::is_untrusted`)

**What:** `EffectClass` and the extended `DenyReason` MUST be matched exhaustively at every call
site — this is already the project's established discipline (`TaintLabel::is_untrusted()`,
`crates/runtime-core/src/plan_node.rs`).
**When to use:** Every place `EffectClass` or `SessionStatus` participates in a security decision.
**Example:** see DESIGN-session-trust-state.md §8's Step 0.5 code block — an exhaustive match over
all 6 `SessionStatus` variants (`Active`, `Draft`, `WaitingApproval`, `Done`, `Failed`, `RolledBack`),
not a bare `== Draft` equality check (fixes Pitfall m1 from round-1 review).

### Anti-Patterns to Avoid

- **Re-deriving the design instead of implementing it.** `DESIGN-session-trust-state.md` already
  answers signature shape, ordering, atomicity, and the exhaustive-match requirement. A plan that
  re-litigates any of these (e.g., proposes Step 0.5 running *before* the per-arg loop) reopens a
  round-1 blocker (B1) that was already found and fixed.
- **A broker pre-check that short-circuits before `submit_plan_node` is called.** This is explicitly
  forbidden — the locked decision is "one executor TCB function, one taxonomy," never a broker-side
  duplicate deny.
- **Sourcing `session_status` from `PlanNode` or any worker-supplied IPC field.** This is the exact
  self-declaration hole the design doc's condition 0 closes; an injected worker would simply assert
  `Active`.
- **Adding a field to `PlanNode` to carry `EffectClass` or session status.** `PlanNode` shape is
  locked (`DEC-architectural-lock-plan-nodes`); `sink_effect_class` must be a hardcoded lookup keyed
  by `SinkId`, never planner-supplied data.
- **A second `DenyReason`-like enum for draft-only denials.** The doc comment on `DenyReason` already
  states "never introduce a second denial error type" — extend the ONE enum.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Session state persistence | A new session-state table or cache | Extend the existing `sessions` table with an `UPDATE` path (mirrors the existing `blocked_literals` redactable-side-table precedent for "add a mutation path to an append-friendly schema") | The two-table event-sourcing shape (mutable read-model `sessions` + append-only `events`) already exists; adding one `UPDATE` statement is the minimal extension |
| Effect-class-per-sink lookup | A config file, runtime registry, or Cedar policy | A hardcoded Rust `match` in `executor::sink_sensitivity` (or a sibling module), mirroring `is_routing_sensitive` exactly | `CON-i2-non-bypassable`: sensitivity/class is a security property, not a configuration knob |
| Draft-only deny decision | A broker-side pre-check before calling the executor | One new arm (Step 0.5) inside the existing `submit_plan_node` | Locked decision: one TCB deny function, one taxonomy |
| Trust-state transport into the executor | A global/thread-local "current session" singleton | An explicit `&SessionStatus` parameter, broker-resolved and passed by the caller | Matches the existing pattern for `value_store`/`session_id` — explicit parameters, no hidden global state, and it keeps the executor a pure function |

**Key insight:** Every mechanism this phase needs (mutable read-model + append-only ledger, hardcoded
sink classification, single deny taxonomy, exhaustive-match discipline) already has a proven analog
shipped in v1.0/v1.1. The work is extension by exact analogy, verified against the current code, not
invention.

## Common Pitfalls

### Pitfall 1: Breaking the 14 existing `submit_plan_node` call sites silently
**What goes wrong:** Adding `session_status: &SessionStatus` as a 5th parameter to
`executor::submit_plan_node` is a breaking signature change. Verified call sites this session:
`crates/brokerd/src/lib.rs` (1 definition + 2 test calls), `crates/brokerd/src/server.rs` (1 call),
`crates/brokerd/tests/phase5_dispatch.rs` (2 calls), `crates/brokerd/tests/s9_acceptance.rs` (3
calls), `crates/executor/tests/executor_decision.rs` (8 calls) — **14 total call sites**, all of
which currently pass 4 args and will fail to compile until updated.
**Why it happens:** The design doc correctly specifies the new signature but doesn't (and shouldn't)
enumerate call sites — that's an implementation-time fact, not a design fact.
**How to avoid:** Treat "update all 14 call sites to pass `&SessionStatus::Active` (preserving
existing test semantics) or a deliberately `Draft` status (for new tests)" as an explicit task, not
an afterthought discovered mid-compile. `crates/brokerd/src/lib.rs`'s own `submit_plan_node` wrapper
also needs its signature extended (or an explicit decision to deprecate/remove it — see Open
Questions, it appears otherwise unused by `server.rs`, which calls `executor::submit_plan_node`
directly).
**Warning signs:** `cargo build --workspace` failing with "expected 5 arguments, found 4" across
multiple crates simultaneously — expected, not a regression, but should be planned for, not
discovered.

### Pitfall 2: `session_status` resolved from the DB on every call vs. threaded in-memory — pick one, consistently
**What goes wrong:** The design doc says `session_status` must be "resolved by the BROKER from ITS
OWN session store" and allows either "in-memory and/or the `sessions` table." If the plan mixes
approaches (e.g., threads an in-memory value but ALSO periodically re-queries the DB inconsistently),
the two can drift, or the implementation becomes needlessly complex.
**Why it happens:** `server.rs`'s existing pattern (`last_event_id`/`last_event_hash` threaded as
mutable locals across `handle_connection`'s loop) is the natural model to mirror, but it's easy to
instead reach for a DB round-trip per `SubmitPlanNode` call "to be safe," which is both slower and
inconsistent with the existing per-connection state pattern.
**How to avoid:** Thread `session_status: SessionStatus` as a mutable local in `handle_connection`,
seeded from a new `initial_session_status` parameter passed down from `create_session`'s result in
`main.rs`, and update it in place immediately after `mint_from_read` demotes the session inside the
`ReportClaims` arm — exactly mirroring how `last_event_id`/`last_event_hash` are updated after each
event append. This is single-threaded-per-session (per the design doc's Accepted Residual Risk 1), so
no lock/race concern beyond what already exists for the chain-head variables.
**Warning signs:** A `get_session_status` DB query function that gets called from multiple
inconsistent places, or a plan that doesn't specify where the per-connection variable is declared.

### Pitfall 3: TAINT-03 has no live sink to exercise — untestable without a named fixture
**What goes wrong:** Both entries in `KNOWN_SINKS` (`crates/executor/src/sink_schema.rs:40-54`) —
`email.send` and `file.create` — map to `EffectClass::CommitIrreversible`. A plan that writes "verify
TAINT-03 passes" without naming a concrete Observe/MutateReversible sink fixture will either skip real
coverage or discover the gap mid-implementation (this exact gap was flagged m2 in
`DESIGN-REVIEW-v1.2-round1.md` and is explicitly named as a Phase 9 responsibility in
`DESIGN-session-trust-state.md` §9).
**Why it happens:** v1.1's sink registry was built for the narrower live-acceptance scope (two
irreversible-effect sinks); v1.2 is the first milestone needing a non-`CommitIrreversible` sink to
exist at all.
**How to avoid:** Recommend a `#[cfg(test)]`-gated test-only sink entry, following the exact
precedent this codebase already uses for platform-gating (`#[cfg(target_os = "linux")]` in
`cli/caprun/tests/`). Concretely: add a small `#[cfg(test)]` conditional branch to `schema_for`
(or a parallel `TEST_KNOWN_SINKS` slice consulted only under `cfg(test)`) registering e.g.
`"test.observe"` with an empty/simple arg schema, and a matching arm in `sink_effect_class` mapping
it to `EffectClass::Observe`. This lets an integration test build a real `PlanNode` targeting that
sink and drive it through the FULL `submit_plan_node` path (Step 0 schema gate → per-arg loop → Step
0.5) on a `Draft` session, proving TAINT-03 end-to-end rather than only unit-testing
`sink_effect_class`/Step 0.5's predicate in isolation. This recommendation is **not directly specified
by the design doc** — it explicitly leaves the fixture choice ("a test-only sink ... or an equivalent
fake sink registry") to Phase 9 — so treat the exact mechanism as a plan-time decision, not a locked
requirement; the `#[cfg(test)]`-gate approach is recommended here because it never appears in the
production `KNOWN_SINKS` surface, avoiding any question about a phantom sink reaching real dispatch
logic in `server.rs` (which only special-cases `file.create` for live invocation — an Allowed decision
against `test.observe` would simply have no adapter invoked, matching today's behavior for any
non-`file.create` Allow).
**Warning signs:** A plan whose verification step for TAINT-03 only unit-tests
`sink_effect_class(&SinkId("something".into())) == EffectClass::Observe` without ever constructing a
`PlanNode` and calling the real `submit_plan_node` end to end.

### Pitfall 4: ORIGIN-01/02 has no CLI on-ramp — don't treat it as a pure data-model change
**What goes wrong:** `cli/caprun/src/main.rs` (verified this session, lines 45-75) parses exactly
`<intent-kind> <intent-param> <workspace-file> [audit-db-path]` — every session today is seeded
trusted-arg by construction. Adding a `seed_provenance` field to `create_session` alone does nothing
observable without a new input path that can actually produce a file-derived seed.
**Why it happens:** The existing CLI was built for the narrower v1.1 scope (typed intent from CLI
args only); this is the first milestone needing an externally-derived seed source at all.
**How to avoid:** Add a new CLI surface — the design doc suggests `--seed-from-file <path>` as an
example but explicitly leaves the exact flag name to this phase. A minimal, low-risk choice: a new
optional flag parsed before the existing positional args, e.g.
`caprun [--seed-from-file <path>] <intent-kind> <intent-param> <workspace-file> [audit-db-path]`,
where presence of the flag sets `seed_provenance = FileDerived` (and the intent's parameter is read
from that file rather than `argv`), and its absence sets `seed_provenance = TrustedArg` (today's
existing behavior, unchanged). Whatever shape is chosen, it must feed a concrete
`SeedProvenance`-like value (a new small enum, likely in `runtime-core` alongside `SessionStatus`)
into `create_session`.
**Warning signs:** A plan that only edits `crates/brokerd/src/session.rs` and treats ORIGIN-01/02 as
"done" without any `cli/caprun/src/main.rs` change or new integration test exercising the flag.

### Pitfall 5: Two-table demotion write not made atomic with the triggering mint
**What goes wrong:** If the `sessions` UPDATE and the `session_demoted` Event append happen as two
separate connection-lock acquisitions (rather than inside the same lock/transaction as the
`mint_from_read` call that triggers them), a crash or error between the two could leave
`sessions.status` still `Active` while a `file_read` event with untrusted taint already exists — or
vice versa, an orphaned `session_demoted` event with no matching status change.
**Why it happens:** `mint_from_read`'s current implementation already acquires the connection lock
once, in `server.rs`'s `ReportClaims` arm (`let locked = conn.lock()...; mint_from_read(&locked, ...)`)
— it's tempting but wrong to add the UPDATE/append as a *second*, separately-locked call after
`mint_from_read` returns.
**How to avoid:** Perform the UPDATE + `session_demoted` append INSIDE `mint_from_read` itself (or a
function it calls), under the SAME `&rusqlite::Connection` reference already passed in and already
locked by the caller — never acquire a second lock. This mirrors the existing guarantee that the
`file_read` Event append and the `ValueRecord` mint happen in "one call so the chain is unbroken."
**Warning signs:** A `server.rs` `ReportClaims` arm that calls `mint_from_read(...)`, releases the
lock, then reacquires it separately to update `sessions.status`.

### Pitfall 6: Non-exhaustive match on `SessionStatus` or the new `EffectClass`/`DenyReason` variant
**What goes wrong:** A `matches!()`-style check (`matches!(session_status, SessionStatus::Draft)`) or
a `_ => false` wildcard arm silently treats a future `SessionStatus`/`EffectClass` variant as the safe
(non-denying) case — the exact failure mode `TaintLabel::is_untrusted()`'s doc comment already warns
against in this codebase.
**Why it happens:** Convenient shorthand is easy to reach for, especially when only one variant
(`Draft`) is functionally new.
**How to avoid:** Follow the design doc's own §8 code block exactly — an explicit `match
*session_status { SessionStatus::Draft => {...}, SessionStatus::Active => {...},
SessionStatus::WaitingApproval | SessionStatus::Done | SessionStatus::Failed |
SessionStatus::RolledBack => {...} }` with every variant named, no `_` arm. Do the same for any new
`EffectClass` match.
**Warning signs:** `_ =>` anywhere in the new Step 0.5 code or in `sink_effect_class`'s
`EffectClass`-facing match (its *internal* `&str` sink-name match is a distinct, permitted case per
the design doc §10 — only the `EffectClass`/`DenyReason`/`SessionStatus` enum matches themselves must
be exhaustive).

## Code Examples

Verified against the live repository this session (all file:line references current as of this
research):

### Current `SessionStatus` (crates/runtime-core/src/session.rs:12-18) — the type this phase extends
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SessionStatus {
    Active,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
// Phase 9 adds `Draft` (design doc §1) — must be inserted, not appended, if variant
// ordering matters for any derive; PartialEq/serde derives here are order-independent for
// enum tag matching (serde tags by variant name, not position), so insertion position is safe.
```

### Current `create_session` (crates/brokerd/src/session.rs:18-27) — unconditionally Active today
```rust
pub fn create_session(intent_id: Uuid) -> Session {
    let now = Utc::now();
    Session {
        id: Uuid::new_v4(),
        intent_id,
        status: SessionStatus::Active,   // ORIGIN-02: must become conditional on seed-provenance
        created_at: now,
        updated_at: now,
    }
}
```

### Current `persist_session` (crates/brokerd/src/session.rs:37-49) — INSERT only, no UPDATE path
```rust
pub fn persist_session(conn: &rusqlite::Connection, session: &Session) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, intent_id, status, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            session.id.to_string(),
            session.intent_id.to_string(),
            serde_json::to_string(&session.status)?,
            session.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}
// NEW function needed (no existing analog): an UPDATE counterpart, e.g.
// pub fn update_session_status(conn: &rusqlite::Connection, session_id: Uuid, status: &SessionStatus)
//     -> anyhow::Result<()> {
//     conn.execute(
//         "UPDATE sessions SET status = ?1 WHERE id = ?2",
//         rusqlite::params![serde_json::to_string(status)?, session_id.to_string()],
//     )?;
//     Ok(())
// }
```

### Current `submit_plan_node` end (crates/executor/src/lib.rs:139-142) — where Step 0.5 attaches
```rust
        // Step 3: Content-sensitive tainted args (subject/body/attachment) do NOT
        // Block in v0 — Tier-4 verbatim review is deferred to the approval-hook plan.
    }

    ExecutorDecision::Allowed
}
// Phase 9: insert the Step 0.5 match (design doc §8) between the closing `}` of the per-arg
// `for` loop and this final `ExecutorDecision::Allowed` line.
```

### Existing sink_sensitivity hardcoded-table pattern to mirror exactly for `sink_effect_class`
```rust
// Source: crates/executor/src/sink_sensitivity.rs:32-39 (current, verified this session)
pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
// New function, same module or sibling, same style (design doc §6):
// pub fn sink_effect_class(sink: &SinkId) -> EffectClass {
//     match sink.0.as_str() {
//         "email.send"  => EffectClass::CommitIrreversible,
//         "file.create" => EffectClass::CommitIrreversible,
//         #[cfg(test)]
//         "test.observe" => EffectClass::Observe,   // Pitfall 3 fixture
//         _ => EffectClass::CommitIrreversible,      // fail-closed unknown-sink (design doc §6)
//     }
// }
```

## State of the Art

Not applicable in the usual external-ecosystem sense — this phase is a pure internal Rust TCB
extension with no external library or framework dependency shift. The one "state of the art" fact
worth recording: the design doc itself represents a *correction* from an earlier internal draft
(round 1 of this same milestone specified Step 0.5 running *before* the per-arg I2 loop, which broke
ACC-01/ACC-02; round 2 fixed the ordering). Any future work touching this code must preserve the
round-2 ordering — see Common Pitfalls #6 and the design doc §8/§9/§11.

| Old Approach (round 1, superseded) | Current Approach (round 2, approved) | When Changed | Impact |
|---|---|---|---|
| Step 0.5 (draft-only class deny) runs BEFORE the per-arg I2 loop | Step 0.5 runs AFTER the per-arg loop completes with no Block | 2026-07-02, `DESIGN-REVIEW-v1.2-round1.md` B1 → fixed in round 2 | Round-1 ordering made ACC-01/ACC-02 unsatisfiable and would have broken the v1.1 §9 live test; round-2 ordering is what Phase 9 must implement |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `#[cfg(test)]`-gated test-only sink (`"test.observe"`) is the right mechanism to make TAINT-03 testable end-to-end, rather than some other "equivalent fake sink registry" the design doc leaves open. | Common Pitfalls #3, Code Examples | Low — this is a test-infrastructure choice, not a security decision; if the planner prefers a different mechanism (e.g., a runtime-injectable sink registry used only in test builds), the underlying requirement (name a concrete fixture) is unaffected. Confidence: MEDIUM (my synthesis, not directly specified in the design doc). |
| A2 | A new optional `--seed-from-file <path>` flag (parsed before the existing positional args) is a reasonable shape for the CLI on-ramp; the design doc explicitly defers the exact flag name/shape to this phase. | Common Pitfalls #4, Architecture Patterns | Low-Medium — flag name/shape is cosmetic and can be revised without touching the security-relevant mechanism (broker-side, trusted-path Draft-setting from a CLI-supplied provenance value), but the planner should treat this as an open naming decision, not a locked requirement. |
| A3 | `EffectClass` should live in the `executor` crate (alongside `sink_sensitivity`), not in `runtime-core`, since nothing outside `executor` currently needs to construct or match on it. The design doc says "a new enum in the executor crate, or `runtime_core` if shared" without picking one. | Architecture Patterns, Code Examples | Low — either placement compiles and satisfies the design doc; `runtime-core` placement would only matter if a future phase needs `EffectClass` outside the executor (none currently do). |
| A4 | `crates/brokerd/src/lib.rs`'s standalone `submit_plan_node` wrapper (distinct from `executor::submit_plan_node`) should also gain the new parameter rather than being left broken/deprecated, since it is a documented "sole public effect entry point for brokerd" even though `server.rs` does not currently call through it. | Common Pitfalls #1, Open Questions | Medium — if left un-updated it fails to compile once the executor signature changes, forcing a decision at implementation time anyway; flagging it now avoids a mid-implementation surprise. |

## Open Questions

1. **Should `crates/brokerd/src/lib.rs`'s `submit_plan_node` wrapper be updated, or removed?**
   - What we know: it delegates to `executor::submit_plan_node` and has its own unit tests, but
     `crates/brokerd/src/server.rs` (the live dispatch path) calls `executor::submit_plan_node`
     directly and does not use this wrapper. It looks like a Phase-1-era convenience API that the
     live server bypassed as the architecture matured.
   - What's unclear: whether it's still considered a supported public entry point (its doc comment
     claims "the sole public effect entry point for brokerd," which is no longer literally true given
     `server.rs`'s direct call) or safe to delete.
   - Recommendation: the plan should explicitly decide — most likely, extend its signature to accept
     `session_status` too (defaulting callers to `&SessionStatus::Active` to preserve its two existing
     tests' semantics) since deleting a documented public function is a larger, separate concern than
     this phase's scope; but this is a real decision point, not a mechanical fix.

2. **Exact seed-provenance enum shape and where it lives.**
   - What we know: the design doc requires a "value indicating whether the session's seed is
     trusted-arg or file-derived," decided by the CLI, recorded at `create_session` time.
   - What's unclear: whether this should be a new `SeedProvenance { TrustedArg, FileDerived }` enum
     (recommended, mirrors the project's preference for typed enums over booleans/strings — see
     `DenyReason`, `TaintLabel`, `SessionStatus` all being enums, never raw strings) stored as a new
     field on `Session`, or whether it's consumed only transiently at `create_session` call time to
     decide `SessionStatus` and not persisted as its own column at all.
   - Recommendation: ORIGIN-01 says "records a seed-provenance field" — read literally, this implies
     persisting it (not just consuming it transiently), most likely as a new nullable/optional column
     on the `sessions` table or folded into the `session_created` audit Event's payload. The design
     doc doesn't pin the persistence mechanism explicitly; the plan should decide between (a) a new
     `sessions` table column, or (b) recording it only in the `session_created` Event's payload
     (simpler, no schema migration, consistent with "no DB migration" precedent from Phase 7's anchor
     work) — (b) is likely the lower-risk choice given the existing `Event` payload already carries
     arbitrary serialized data and the `sessions` table has historically been kept minimal.

3. **Concurrency/race between `mint_from_read` and `submit_plan_node` (accepted residual risk, not
   this phase's to solve).**
   - What we know: the design doc's §12 explicitly accepts this race for v1.2, citing the
     single-worker-per-session process model.
   - What's unclear: nothing — this is a documented, accepted risk, restated here only so the
     planner does not attempt to "fix" it as an unplanned scope addition.
   - Recommendation: do not add any per-session locking/serialization beyond what already exists;
     defer to the v2 multi-worker milestone per the design doc.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo toolchain | All crate edits, `cargo build --workspace` | ✓ | workspace `resolver = "3"`, edition 2021 (existing, unchanged) | — |
| Colima + Docker (`rust:1` image, `--security-opt seccomp=unconfined`) | Running Linux-only security tests from macOS (e.g., if this phase adds a new live CLI-flag test) | Assumed ✓ per CLAUDE.md ("Colima installed") — not re-verified this session since no new Linux-only test was written yet | — | If unavailable, new Linux-gated tests still compile and no-op on macOS (`0 passed`, expected per project convention) — not a blocker for planning, only for live verification |

No new external service/tool dependency is introduced by this phase.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (existing workspace-wide harness) |
| Config file | Workspace `Cargo.toml` (resolver = "3"); no separate test-framework config |
| Quick run command | `cargo test -p executor && cargo test -p brokerd` (fast, macOS-safe subset covering the new Step 0.5 / demotion logic without the Linux-only e2e suite) |
| Full suite command | `cargo test --workspace --no-fail-fast` (macOS: Linux-gated tests show 0 passed, expected); Linux security tests via the Colima/Docker recipe in CLAUDE.md for full ACC-style verification |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TAINT-01 | `mint_from_read` demotes session to `Draft` | unit | `cargo test -p brokerd --lib mint_from_read` (extend existing `crates/brokerd/src/quarantine.rs` test module with a new `mint_from_read_demotes_session_to_draft` test) | ✅ module exists — ❌ new test case, Wave 0 |
| TAINT-02 | `Draft` + `CommitIrreversible` (no I2 Block) → Denied | unit | `cargo test -p executor --test executor_decision draft_session_denies_commit_irreversible` | ❌ new test, Wave 0 (extend `crates/executor/tests/executor_decision.rs`) |
| TAINT-03 | `Draft` + `Observe`/`MutateReversible` → still Allowed | unit/integration | `cargo test -p executor --test executor_decision draft_session_allows_observe` | ❌ new test + new test-only sink fixture, Wave 0 (see Pitfall 3) |
| TAINT-04 | Demotion is an audited event, `parent_id` == triggering read event | unit | `cargo test -p brokerd --lib mint_from_read_demotion_causal_edge` | ❌ new test, Wave 0 |
| ORIGIN-01 | Seed-provenance recorded, decided by CLI | integration | `cargo test -p caprun --test <new_file>` (or extend an existing cli test) exercising the new CLI flag | ❌ new CLI flag + new test, Wave 0 |
| ORIGIN-02 | File-derived seed starts `Draft` | unit + integration | `cargo test -p brokerd --lib create_session_file_derived_starts_draft` plus a CLI-level integration test | ❌ new test, Wave 0 |
| (regression) v1.1 §9 acceptance unaffected | Existing hostile-block / clean-allow paths unchanged | existing integration | `cargo test -p brokerd --test s9_acceptance` and (Linux) `cargo test -p caprun --test s9_live_block` | ✅ exists — must be updated for the new `submit_plan_node` signature (Pitfall 1), not rewritten in behavior |

### Sampling Rate
- **Per task commit:** `cargo test -p executor` and/or `cargo test -p brokerd` scoped to the crate
  just touched.
- **Per wave merge:** `cargo test --workspace --no-fail-fast` (macOS baseline) — confirms no
  cross-crate signature breakage among the 14 call sites in Pitfall 1.
- **Phase gate:** Full workspace test green on macOS, PLUS the Colima/Docker Linux recipe re-run for
  any new or touched Linux-gated test (`s9_live_block.rs` and any new CLI-flag live test), before
  `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `crates/executor/tests/executor_decision.rs` — needs new tests for TAINT-02/TAINT-03 (draft-only
      deny, draft-only-but-non-CommitIrreversible allow) and all 8 existing calls updated for the new
      5th parameter.
- [ ] `crates/brokerd/src/quarantine.rs` test module — needs new tests for TAINT-01/TAINT-04 (session
      demotion + causal edge).
- [ ] `crates/brokerd/src/session.rs` test module (currently has none inline — verify at plan time) —
      needs new tests for ORIGIN-01/02 (`create_session` with file-derived provenance starts `Draft`).
- [ ] A new or extended `cli/caprun/tests/*.rs` integration test exercising the new CLI on-ramp
      (ORIGIN-01 — ties the CLI's provenance decision to the resulting session status).
- [ ] `crates/brokerd/tests/phase5_dispatch.rs`, `crates/brokerd/tests/s9_acceptance.rs`,
      `crates/brokerd/src/lib.rs` tests — all need their `submit_plan_node` call sites updated for the
      new parameter (mechanical, not new coverage, but will fail to compile otherwise — see Pitfall 1).
- [ ] Test-only sink fixture (`"test.observe"` or equivalent) needs to be added to
      `crates/executor/src/sink_schema.rs`/`sink_sensitivity.rs` under `#[cfg(test)]` before TAINT-03
      can be exercised end-to-end (see Pitfall 3).

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V1 Architecture, Design and Threat Modeling | yes | This phase implements an already-approved design doc (`DESIGN-session-trust-state.md`); the plan must preserve every MUST/MUST-NOT verbatim, not reinterpret them |
| V4 Access Control | yes | Draft-only denial decided exclusively in the executor TCB (`submit_plan_node`), never a broker pre-check or policy file — `CON-i2-non-bypassable` extended to session-trust |
| V5 Input Validation | yes (indirect) | The new CLI on-ramp (ORIGIN-01) is a new untrusted-input surface (a file path/content read from disk) — must fail closed on malformed/missing files, consistent with the project's existing "fail-closed" discipline (e.g., `mint_from_read`'s unknown-`claim_type` error) |
| V8 Data Protection / Audit | yes | `session_demoted` event must carry the exact `parent_id` causal edge (TAINT-04); atomicity with the `sessions` UPDATE is a hard MUST, not best-effort |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Worker/IPC self-declares session as `Active` to bypass Step 0.5 | Spoofing / Elevation of Privilege | `session_status` sourced exclusively from broker-owned per-connection state (never `PlanNode`/IPC) — design doc §4/§11 condition 0 |
| A future `SessionStatus`/`EffectClass` variant silently bypasses the deny check via a wildcard match arm | Elevation of Privilege (silent fail-open) | Exhaustive match, no `_` arm, on every enum touched by this phase (Common Pitfalls #6) |
| Session-demotion write partially applied (status updated but no audit event, or vice versa) on crash/error | Repudiation / Tampering | Atomic same-lock/transaction pair for the `sessions` UPDATE + `session_demoted` append (Common Pitfalls #5) |
| Unregistered/unknown sink silently treated as a permissive `EffectClass` | Elevation of Privilege | Fail-closed: unknown sink → `EffectClass::CommitIrreversible` (most restrictive), and in practice unreachable because Step 0's schema gate already denies unregistered sinks first (design doc §6, Accepted Residual Risk 2) |
| Demotion race between concurrent `mint_from_read` and `submit_plan_node` calls | Tampering (TOCTOU) | Accepted residual risk for v1.2 given the single-worker-per-session process model (design doc §12) — not to be "fixed" out of scope this phase |

## Sources

### Primary (HIGH confidence — all read directly this session)
- `planning-docs/DESIGN-session-trust-state.md` — the authoritative, APPROVED spec for this phase's
  entire mechanism (SessionStatus::Draft, mint_from_read trigger, I0 creation rule, Step 0.5
  placement, EffectClass, DenyReason extension, Acceptance Predicate)
- `planning-docs/DESIGN-GATE-RECORD-v1.2.md` — approval record; confirms Decision: APPROVED, Gate
  status: UNBLOCKED, and the round-1→round-2 B1/M1-M3/m1-m3 fix history
- `planning-docs/DESIGN-confirmation-release.md` — Phase 10's design (read for the "Two Independent
  Mechanisms" interplay Phase 9 must not assume/preclude)
- `planning-docs/PHASE-7-HANDOFF.md` — continuity notes (anti-stapling, effect_id minting discipline,
  taint-consistency) this phase must not regress
- `crates/runtime-core/src/{session,executor_decision,plan_node,effect,event,lib}.rs` — current
  `SessionStatus` (no Draft), `DenyReason` taxonomy, `PlanNode`/`SinkId`/`ValueId` shapes, `Effect`
  3-class lock, `Event::new`/`Event::sink_blocked` constructors
- `crates/executor/src/{lib,sink_schema,sink_sensitivity,value_store}.rs` — current
  `submit_plan_node` (4-arg, no session_status), `KNOWN_SINKS` (both entries CommitIrreversible),
  `is_routing_sensitive` hardcoded-match pattern to mirror
- `crates/brokerd/src/{session,quarantine,audit,server,lib}.rs` — current `create_session`
  (unconditional Active), `persist_session` (INSERT-only), `mint_from_read`/`mint_from_intent` (sole
  mint sites), `sessions`/`events`/`blocked_literals` schema DDL, `handle_connection`'s per-connection
  state threading pattern, `dispatch_request`'s `SubmitPlanNode` arm
- `cli/caprun/src/main.rs` — current positional-arg-only CLI, `create_session` call site, no
  file-derived-intent path
- `.planning/{REQUIREMENTS,STATE,PROJECT,ROADMAP}.md` — locked v1.2 scope, requirement IDs
  TAINT-01..04/ORIGIN-01..02, prior decisions (executor-decides, no broker pre-check)
- Direct grep across the workspace confirming all 14 `submit_plan_node` call sites (Pitfall 1) and
  the `sessions`/`CreateSession` usage split between `cli/caprun/src/main.rs` (direct call) and
  `crates/brokerd/src/server.rs`'s `BrokerRequest::CreateSession` arm (test-only path)

### Secondary (MEDIUM confidence)
- None — no external documentation or library research was needed this phase (see Metadata).

### Tertiary (LOW confidence)
- None used as load-bearing claims in this document. All `[ASSUMED]`-tier claims are isolated to the
  Assumptions Log above and are explicitly non-security-load-bearing naming/mechanism choices, not
  claims about the approved design's substance.

## Metadata

**Confidence breakdown:**
- Standard stack: N/A — no new packages this phase, nothing to assess
- Architecture: HIGH — every gap identified traces to a specific, currently-read function/struct
  whose shape does not yet support the new requirement; the mechanism itself is already
  adversarially-reviewed and approved (not this research's own design)
- Pitfalls: HIGH — each pitfall is grounded in a concrete, verified current-code fact (exact call-site
  count, exact missing UPDATE path, exact sink registry contents), not speculation

**Deliberate scoping note:** This research did not invoke the `tool_strategy` external-research seam
(no `research-plan`/WebSearch/Context7 calls) because Phase 9 introduces zero new external
dependencies and the entire security-relevant design space is already resolved by the approved
`DESIGN-session-trust-state.md`. All effort went into verifying that document's claims against the
live codebase rather than researching external libraries or patterns.

**Research date:** 2026-07-06
**Valid until:** Until `crates/executor`, `crates/brokerd`, `crates/runtime-core`, or `cli/caprun`
change meaningfully from the state read this session, or until the design doc is amended — recommend
re-verifying call-site counts (Pitfall 1) immediately before planning if any other phase work has
landed in the interim.
