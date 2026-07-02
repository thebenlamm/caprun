# DESIGN-session-trust-state.md — Session Trust State (I1 Dynamic Demotion + I0 Creation Rule)

**Requirement:** PROC-01 (forward-references TAINT-01, TAINT-02, TAINT-03, TAINT-04, ORIGIN-01, ORIGIN-02)
**Status:** Draft — pending DESIGN-GATE-RECORD-v1.2.md approval
**Canonical source:** `planning-docs/PLAN.md` (wins on any conflict)
**Gate:** `crates/executor` and `crates/brokerd` MUST NOT gain any session-trust-state or
confirmation-release code until this document AND `planning-docs/DESIGN-confirmation-release.md`
are both reviewed and `planning-docs/DESIGN-GATE-RECORD-v1.2.md` records decision = APPROVED.

**Prior art / relationship to the approved v1.0 pair:** This document extends
`planning-docs/DESIGN-taint-model.md` (I0/I1 invariant text, genuine-taint requirement,
declassification/endorsement model) and `planning-docs/DESIGN-plan-executor.md` (ValueRecord/ValueId
handle model, PlanNode schema, Executor Decision Logic, sink sensitivity map) — both APPROVED per
`planning-docs/DESIGN-GATE-RECORD.md` round 2. This document is **additive**: it references the
existing I0/I1 invariant text rather than restating it wholesale, and it fixes the concrete
mechanism gaps RESEARCH.md (Phase 8) identified that Phase 9 (TAINT-01..04, ORIGIN-01..02) needs to
implement without making a new design decision.

---

## 1. `SessionStatus::Draft` — the new variant

`runtime_core::SessionStatus` today (`crates/runtime-core/src/session.rs`) is:

```rust
pub enum SessionStatus {
    Active,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
```

**Rule (MUST):** A new variant `Draft` MUST be added to this exact enum — no new/parallel status
type, no wrapper struct. The variant name is fixed as `Draft` (matching PLAN.md's and
`DESIGN-taint-model.md`'s prose "draft-only status").

```rust
pub enum SessionStatus {
    Active,
    Draft,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
```

**Transition rule (MUST, monotonic):** The only transition into `Draft` from a live session is
`Active → Draft`. This transition is **one-way**: a `Draft` session MUST NOT ever transition back to
`Active`. This mirrors the monotonicity of taint itself (`DESIGN-taint-model.md` §Taint Label
Vocabulary: "Taint labels are monotonic ... Labels are NEVER removed") — a session's trust posture,
once lowered by touching untrusted content, is never silently restored. There is no
`Draft → Active` transition in this document's scope; any future "session fully re-verified, restore
to Active" mechanism (if ever built) MUST be a distinct, explicitly human-gated, separately-designed
transition — it is out of scope for v1.2 and MUST NOT be implemented as a side effect of this work.

**A session created directly as `Draft`** (the I0 case, §3 below) has no prior `Active` state to
transition from — `Draft` is simply its initial status. The one-way rule applies to the *transition*,
not to session creation.

---

## 2. I1 dynamic demotion — `mint_from_read` is the sole trust-flip site

**I1 dynamic demotion (MUST):** A session MUST be demoted to `SessionStatus::Draft` at the moment
`mint_from_read` (in `crates/brokerd/src/quarantine.rs`) mints a tainted `ValueRecord` for that
session (TAINT-01). This is the same event that already appends the `file_read` audit Event and
mints the `ValueRecord` in one atomic call — the demotion MUST be co-located in that same function
(or a function it calls under the same lock/transaction), never split into a separate, later,
best-effort step.

**Trusted-path-only / anti-self-declaration (MUST — near-verbatim extension of
`DESIGN-taint-model.md`'s I0 phrasing to I1):** The session's `Draft` transition on `mint_from_read`
MUST be set by the same trusted broker function that mints the tainted `ValueRecord` — never by the
worker, and never by a flag any worker IPC message could carry. `mint_from_read` is the SOLE
trust-flip site for I1, exactly as it is already documented as the SOLE broker taint-mint site
(T-04-03, `crates/brokerd/src/quarantine.rs` module doc: "the only call site in brokerd that ...
Both operations occur in one call so the chain is unbroken"). No other function in `brokerd` MUST be
permitted to set `SessionStatus::Draft` for the I1 reason. This is the identical anti-spoofing
structure `DESIGN-taint-model.md` already states for I0 ("The tainted-seed tag MUST be set by the
trusted session-creation path from provenance — never self-declared by the creating agent") applied
to session state instead of value state: if a worker's `ReportClaims` message could itself carry
"I am now tainted, please demote me," an injected worker would simply omit that flag and the session
would remain falsely `Active`. The demotion determination MUST derive from the broker's own act of
minting untrusted taint, never from any claim the (possibly compromised) worker asserts about itself.

`mint_from_intent` (the sibling `UserTrusted`-only mint site) MUST NOT trigger a demotion — only
`mint_from_read`'s untrusted-taint mint path is a demotion trigger.

---

## 3. I0 creation rule — externally-seeded sessions start `Draft`

**I0 creation rule (MUST, restated for session creation — extends, does not restate,
`DESIGN-taint-model.md`'s I0 invariant text):** A Session whose seed derives from external or
untrusted content MUST start in `SessionStatus::Draft` at creation time — never `Active` followed by
a later demotion. `brokerd::session::create_session` (`crates/brokerd/src/session.rs`) today always
constructs `SessionStatus::Active` unconditionally; this MUST become conditional on a new
seed-provenance input.

**Seed-provenance field (MUST):** `create_session` MUST accept (or be passed alongside) a
seed-provenance determination — a value indicating whether the session's seed is trusted-arg or
file-derived. This determination MUST be recorded at `create_session` time (not inferred later).

**Who decides (MUST, trusted-path-only — mirrors I0 Acceptance Predicate condition 0 in
`DESIGN-taint-model.md`):** The trusted-arg-vs-file-derived determination is made by the `caprun` CLI
(ORIGIN-01) at intent-parsing time — this is a CLI-level, pre-broker decision about *how the intent
was supplied to the process*, which the CLI is uniquely positioned to know (it parsed `argv`/read the
file itself). The broker's `create_session` path — the same trusted path that already owns
`SessionStatus` at creation — is what actually SETS `Draft` from that provenance; the initial status
MUST NOT be self-declared by the (potentially injected) caller of `create_session`. This is the
identical anti-self-declaration principle `DESIGN-taint-model.md` states verbatim for I0: "The
tainted-seed determination is made by the trusted brokerd session-creation path from the seed's
provenance ... NOT self-declared by the agent creating the Session."

**New CLI on-ramp required (MUST name the expected shape; exact flag name is Phase 9's to
finalize):** `cli/caprun/src/main.rs` today parses only positional args
(`<intent-kind> <intent-param> <workspace-file> [audit-db-path]`) — every session today is seeded
trusted-arg by construction, and there is no existing "seed the intent from a file" input path to
demote (Pitfall 5). This document requires that Phase 9 add a new CLI input surface — for example a
`--seed-from-file <path>` flag, or a third intent-parsing branch that reads intent content from a
workspace file rather than `argv` — so that ORIGIN-01/ORIGIN-02 have something concrete to exercise.
The seed-provenance field requirement is NOT satisfiable by a pure data-model change; it REQUIRES a
new input path into the CLI, because there is currently no way to construct a file-derived intent at
all.

---

## 4. How the executor learns trust state — the broker-resolved `session_status` parameter

This is the load-bearing gap RESEARCH.md flagged (Pitfall 1 / Assumption A2): today's
`executor::submit_plan_node` signature carries no session or trust-state input at all.

**Current signature** (`crates/executor/src/lib.rs`):

```rust
pub fn submit_plan_node(
    _session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    value_store: &ValueStore,
) -> ExecutorDecision
```

**New signature (MUST):**

```rust
pub fn submit_plan_node(
    _session_id: Uuid,
    effect_id: Uuid,
    plan_node: &PlanNode,
    value_store: &ValueStore,
    session_status: &SessionStatus,
) -> ExecutorDecision
```

`submit_plan_node` gains an explicit trust-state parameter, named `session_status: &SessionStatus`.

**Trusted-path-only sourcing (MUST — the security-load-bearing rule, per RESEARCH Assumption A2):**
`session_status` MUST be resolved by the BROKER from ITS OWN session store immediately before
calling `submit_plan_node`, and MUST NOT be carried on the `PlanNode`, MUST NOT be trusted from any
worker-supplied IPC message, and MUST NOT be a field the caller can set arbitrarily. This is the same
discipline HARD-03 already uses for `session_id` (session-scoped handle resolution; cross-session
resolution denied) — the broker is the sole authority for what a session's current status is, and it
passes that authoritative value into the one TCB decision function as a parameter, never as data the
plan node itself asserts. Sourcing `session_status` from IPC or from the `PlanNode` would reopen the
exact self-declaration hole I0's Acceptance Predicate condition 0 already closed for session
creation — an injected worker or planner would simply assert `Active` and bypass every check this
document specifies. There MUST be exactly one code path that resolves `session_status`: a lookup
against the broker's session store (in-memory and/or the `sessions` table, see §5) keyed by the
already-validated `session_id`.

---

## 5. Two-table audit contract for demotion (TAINT-04) — atomic UPDATE + append-only Event

`crates/brokerd` already implements a two-table event-sourcing shape: a strict append-only `events`
table (hash-chained, `compute_event_hash`/`verify_chain` assert this) plus a `sessions` table that is
currently write-once (`persist_session`'s `INSERT`, no `UPDATE` path exists today).

**The demotion write (MUST) is an atomic pair, performed inside the same lock/transaction as the
`mint_from_read` call that triggers it:**

1. **Mutable read-model update:** `UPDATE sessions SET status = 'Draft' WHERE id = ?1` (or
   equivalent, driven through the same JSON-serialized `SessionStatus` encoding `persist_session`
   already uses) — the monotonic `Active → Draft` UPDATE path that does not exist today and MUST be
   added.
2. **Append-only ledger entry:** a `session_demoted` audit Event MUST be appended via
   `audit::append_event`, whose `parent_id` MUST equal the triggering `file_read` Event's id — the
   TAINT-04 causal edge. This is the identical "genuine anchor" discipline `mint_from_read` already
   uses for `provenance_chain[0]` (the value-lineage anchor), applied here to the causal DAG's
   `parent_id` edge (the two are separate graphs and MUST NOT be conflated — see the existing
   `mint_from_read` doc comment's explicit warning: "NOTE: `parent_id` is the CAUSAL edge; the
   value-lineage anchor ... is a SEPARATE graph (never equated)").

**Atomicity (MUST):** Both writes MUST happen atomically (same lock/transaction) so the read-model
row and the ledger can never disagree — there MUST NOT be a window where `sessions.status` says
`Draft` but no `session_demoted` Event exists, or vice versa. This mirrors the existing
`mint_from_read` guarantee that the `file_read` Event append and the `ValueRecord` mint happen in
"one call so the chain is unbroken."

**Event schema note:** This document states the causal-edge requirement
(`session_demoted.parent_id == file_read.id`). The `Event` schema itself (fields, serialization,
storage) is unchanged and owned by `brokerd::audit` (already documented in Phase 3) — this document
does not redefine it, only the new event_type value (`"session_demoted"`) and its required
`parent_id` linkage.

---

<!-- Section boundary: Task 1 (SessionStatus::Draft, I1/I0 rules, trust-state threading, audit
contract) ends here. Task 2 (executor deny mechanism, effect-class table, acceptance predicate)
appends below. -->
