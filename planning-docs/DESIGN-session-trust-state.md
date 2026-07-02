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

## 6. Effect-class-per-sink — the new `sink_effect_class` table (mirrors "Sink Sensitivity Map")

TAINT-02/03 require the executor to know a plan node's effect class in order to decide draft-only
denial. Today `crates/executor/src/sink_sensitivity.rs` hardcodes routing/content-sensitive **args**
per sink (`is_routing_sensitive`, `is_content_sensitive`) but has no notion of
`Observe`/`MutateReversible`/`CommitIrreversible` per sink.

**New hardcoded function (MUST):** `sink_effect_class(sink: &SinkId) -> EffectClass` MUST be added to
`crates/executor/src/sink_sensitivity.rs` (or a sibling module in the same crate), mirroring the
existing `is_routing_sensitive`/`is_content_sensitive` hardcoded-match pattern exactly — no dynamic
lookup, no config file, no runtime-registered table.

**`EffectClass` (MUST be a new enum in the executor crate, or `runtime_core` if shared):** three
variants only — `Observe`, `MutateReversible`, `CommitIrreversible` — mirroring the locked 3-class
`Effect` ontology already fixed in `crates/runtime-core/src/effect.rs` (`CON-effect-classes`:
"Exactly three variants ... This shape is locked ... Do not add a fourth top-level variant"). This is
a **sink-level classification** returned by a hardcoded function, not the planner-facing `Effect` enum
itself — `PlanNode` carries only `{ sink, args }` and stays locked (see below).

**PlanNode shape stays locked (MUST NOT add a field):** `sink_effect_class` MUST be a hardcoded
table keyed by `SinkId`, NOT a new field added to the locked `PlanNode` struct
(RESEARCH Assumption A3, `CON-i2-non-bypassable`). Effect class is a security property of the sink
identity, not planner-supplied data — adding it as a `PlanNode` field would let an injected planner
assert its own effect class, reopening the exact self-declaration hole the `ValueId` handle model
closes for taint.

**v0/v1.2 sink mapping (MUST, explicit):**

```rust
pub fn sink_effect_class(sink: &SinkId) -> EffectClass {
    match sink.0.as_str() {
        "email.send"  => EffectClass::CommitIrreversible,
        "file.create" => EffectClass::CommitIrreversible,
        _ => /* see fail-closed rule below */
    }
}
```

Both currently-live sinks (`email.send`, `file.create`) MUST map to `EffectClass::CommitIrreversible`
— both are irreversible/external effects per the existing sink sensitivity map
(`DESIGN-plan-executor.md` §Sink Sensitivity Map: `email.send` is `effect_class: CommitIrreversible,
tier: 4`; `file.create`'s `O_EXCL`/dirfd-mediated create is likewise a one-shot irreversible write).

**Unknown-sink handling MUST be fail-closed (explicit, not left to the `_ =>` wildcard's implicit
default):** An unknown sink MUST NOT default to a permissive class (`Observe` or
`MutateReversible`) — that would let an unregistered sink silently bypass the draft-only deny check
below. This document specifies: an unknown sink is treated as `EffectClass::CommitIrreversible`
(the most restrictive class) for the purposes of `sink_effect_class`. Justification: `sink_schema`'s
existing `UnknownSink` check already runs at Step 0 (before any effect-class check — see §8 below)
and denies unregistered sinks outright, so `sink_effect_class` is in practice only ever called with
an already-validated, known `SinkId`. Treating the theoretically-unreachable "unknown sink" branch of
`sink_effect_class` as maximally restrictive (rather than `Observe`) is the fail-closed choice that
costs nothing (the branch is dead in the live path) and prevents any future refactor that removes or
reorders the Step 0 schema gate from silently reintroducing a permissive default.

---

## 7. New `DenyReason` variant — appended to the ONE taxonomy

**Rule (MUST):** Exactly one variant MUST be appended to the existing `DenyReason` enum in
`crates/runtime-core/src/executor_decision.rs`:

```rust
pub enum DenyReason {
    DanglingHandle,
    EmptyTaintInvariantViolation,
    MissingProvenanceAnchor,
    UnknownSink(String),
    UnknownArg(String),
    DuplicateArg(String),
    MissingArg(String),
    // v1.2 addition:
    DraftOnlySessionDeniesCommitIrreversible { sink: SinkId },
}
```

The variant carries the offending `SinkId` (per RESEARCH Open Question 3), matching the existing
`UnknownSink(String)` convention of carrying the offending name for audit/CLI legibility.

**Single-taxonomy discipline (MUST NOT):** This is appended to the ONE existing `DenyReason`
taxonomy — the enum's own doc comment already states this discipline: "the ONE base denial error
enum for Phase 7 ... never introduce a second denial error type." A second, parallel
`DenyReason`-like enum for draft-only denials MUST NOT be introduced.

---

## 8. Executor decision-logic placement — post-loop class deny, executor-only, one TCB function

`crates/executor/src/lib.rs`'s current `submit_plan_node` ordering is: Step 0 (schema validation,
fail-closed) → per-arg loop (Step 1 resolve, Step 1a empty-taint guard, Step 1b empty-provenance
guard, Step 2 routing-sensitivity block, Step 3 content-sensitive marking, unimplemented) → `Allowed`.

**Precedence rule (MUST, amended per DESIGN-REVIEW-v1.2-round1.md B1):** The per-arg I2 taint Block
(Steps 1/1a/1b/2/3) MUST take precedence over the draft-only class deny. If any arg Blocks during the
per-arg loop, `submit_plan_node` returns `BlockedPendingConfirmation` exactly as it does today — the
draft-only check below is never reached for that call. The draft-only deny is a class-level backstop
that fires only when the per-arg loop completes with no Block: a session may be `Draft`, its args may
all look clean (untainted, or tainted but not routing-sensitive), yet the session itself is still
untrustworthy — this is exactly TAINT-01/ORIGIN-02's target case (clean-looking values from an
untrusted context; instructions may be injected even where no single arg trips the I2 routing-sensitive
check). Reversing this precedence — denying before the loop runs, as round 1 of this document
specified — makes ACC-01/ACC-02 unsatisfiable (the I2 Block never fires on any `Draft` session) and
breaks the v1.1 §9 live acceptance test; see the round-1 review for the full trace.

**New "Step 0.5" (MUST, placement corrected):** The draft-only `CommitIrreversible` deny check MUST
run as a new Step 0.5 — **after the per-arg loop completes with no Block**, and before the function
returns `Allowed`. (The name "Step 0.5" is retained from the original numbering for continuity with
RESEARCH.md and the round-1 gate record; it does not imply the check runs before Step 0's neighbors —
its actual position in the ordered sequence is last, immediately before the `Allowed` return.)

**Predicate (MUST) — exhaustive match over `SessionStatus` (fixes Pitfall m1):**

```rust
// Step 0.5 — after the per-arg loop completes with NO Block, before returning Allowed.
match *session_status {
    SessionStatus::Draft => {
        if sink_effect_class(&plan_node.sink) == EffectClass::CommitIrreversible {
            return ExecutorDecision::Denied {
                reason: DenyReason::DraftOnlySessionDeniesCommitIrreversible {
                    sink: plan_node.sink.clone(),
                },
            };
        }
        // Draft + non-CommitIrreversible: fall through to Allowed (TAINT-03).
    }
    SessionStatus::Active => {
        // No deny from this gate; fall through to Allowed.
    }
    SessionStatus::WaitingApproval
    | SessionStatus::Done
    | SessionStatus::Failed
    | SessionStatus::RolledBack => {
        // No deny from THIS gate. These states are not a session-trust concern this document
        // governs — the broker's session-lifecycle contract does not route new plan-node
        // submissions to a WaitingApproval/Done/Failed/RolledBack session in the first place
        // (submit_plan_node is only reachable for a session actively accepting effects). Matched
        // explicitly, not with a wildcard arm, so a future SessionStatus variant is a compile
        // error here, not a silent fail-open (consistent with §10's exhaustive-match discipline).
    }
}
```

The exhaustive match (rather than a bare `== Draft` equality check) is a defense-in-depth fix for
Pitfall m1: it costs nothing functionally (behavior for every current variant is unchanged) and
guarantees a future `SessionStatus` variant cannot silently bypass this gate through an unhandled
wildcard arm.

**Executor-only, never a broker pre-check (MUST, locked decision restated):** This decision MUST be
made in ONE executor TCB function — `submit_plan_node` — and MUST NOT be duplicated or pre-empted as
a broker pre-check before `submit_plan_node` is even called. This is the locked project decision
carried forward from the milestone seed and STATE.md: "Draft-only deny decision must live in the
executor (one TCB deny function, one DenyReason taxonomy), never a broker pre-check." The broker's
only responsibility toward this check is correctly resolving and passing in `session_status` (§4);
the deny decision itself belongs exclusively to the executor.

---

## 9. Non-regression MUSTs

- **A `Draft` session with a tainted routing-sensitive arg MUST Block (I2), never Denied (I1/I0)
  (ACC-01/ACC-02, amended per B1).** The per-arg loop (Steps 1/1a/1b/2/3) runs to completion — or
  returns `BlockedPendingConfirmation` — for every session regardless of `session_status`. Step 0.5
  (the draft-only class deny) is reached ONLY when the loop completes with no Block. This is the
  precedence B1 required: I2's per-arg Block always wins over I1/I0's class-level deny when both would
  otherwise apply to the same call.
- **A `Draft` session MUST still allow `MutateReversible` and `Observe` class plan nodes (TAINT-03).**
  A `Draft` session submitting a plan node whose `sink_effect_class` is `Observe` or
  `MutateReversible` MUST pass through the per-arg loop exactly as an `Active` session would, and MUST
  NOT be denied by the post-loop Step 0.5 check — Step 0.5's predicate is conjunctive
  (`CommitIrreversible AND Draft`), so a non-`CommitIrreversible` sink never trips it regardless of
  session state. *(Pitfall m2: both current production sinks — `email.send`, `file.create` — are
  `CommitIrreversible`, so this requirement is untestable against `KNOWN_SINKS` as it stands today.
  Phase 9's verifier MUST exercise it via a test-only sink registered with
  `EffectClass::Observe`/`MutateReversible` (or an equivalent fake sink registry) — name the chosen
  fixture explicitly in Phase 9's plan so verification does not stall on an untestable requirement.)*
- **The new check MUST NOT alter or weaken the existing I2 routing-sensitivity block on genuine
  taint.** Step 0.5 is purely additive and runs strictly after the loop — it MUST NOT change Step
  1/1a/1b/2/3's existing logic, ordering, or the values they read from `value_store`, and it MUST NOT
  run before them. This protects the v1.1 §9 acceptance test unchanged: an `Active` session with a
  tainted routing-sensitive arg still Blocks exactly as it does today, reaching Step 0.5 only if no
  Block fired — Step 0.5's predicate requires `Draft`, so it never denies for an `Active` session
  either way.

---

## 10. Exhaustive-match discipline (Pitfall 4) — no wildcard arm, ever

**Rule (MUST):** Any new enum introduced by this document — `EffectClass`, and the extended
`DenyReason` — MUST be matched exhaustively, with no wildcard `_` arm, at every call site. This
mirrors the existing discipline `TaintLabel::is_untrusted()` already documents and enforces
(`crates/runtime-core/src/plan_node.rs`): "This method uses an EXPLICIT `match self` with NO wildcard
arm. Adding a new `TaintLabel` variant without updating this match is a compile error, not a silent
false-allow."

**Explicitly forbidden (MUST NOT):** `matches!()`-style shorthand and `_ => <permissive-default>`
arms are explicitly forbidden for `EffectClass` and `DenyReason` handling. A future variant added to
either enum without updating every match site MUST be a compile error, never a silently-accepted
fail-open default. (Note: the fail-closed rule for `sink_effect_class`'s own internal unknown-sink
handling in §6 is a distinct concern — that is a match over `&str` sink names inside one hardcoded
function, not a match over the `EffectClass`/`DenyReason` enum variants themselves; both disciplines
apply simultaneously and do not conflict.)

---

## 11. I1/I0 Acceptance Predicate (Done When)

This document's I1/I0 mechanism is satisfied when the following predicate holds for every Session and
every `submit_plan_node` call in scope of this design:

0. **Trust state is broker-resolved from the broker's own session store, never self-declared.**
   `session_status` passed into `submit_plan_node` MUST originate from a lookup the broker performs
   against its own session store keyed by `session_id` — never from the `PlanNode`, never from any
   worker-supplied IPC field, never asserted by the caller. A creating or reporting agent's assertion
   about its own session's trust state is never authoritative.
1. **A `mint_from_read` call demotes the session to `Draft` with a causally-linked audit Event.**
   When `mint_from_read` mints a tainted `ValueRecord` for a session, that same atomic operation MUST
   (a) set `sessions.status = 'Draft'` for that session's row, and (b) append a `session_demoted`
   Event whose `parent_id` equals the triggering `file_read` Event's id.
2. **An externally-seeded session starts `Draft` at creation.** A Session whose seed provenance is
   file-derived (as determined by the trusted `caprun` CLI path and passed to the broker's
   `create_session`) MUST start with `status == Draft`, never `Active` followed by a later demotion.
3. **A `CommitIrreversible` plan node on a `Draft` session, that does NOT already Block on I2, is
   Denied, decided in the executor (amended per B1).** A plan node whose
   `sink_effect_class(sink) == EffectClass::CommitIrreversible`, submitted while
   `session_status == SessionStatus::Draft`, and whose per-arg loop completes with NO Block, MUST
   return `ExecutorDecision::Denied { reason: DraftOnlySessionDeniesCommitIrreversible { sink } }`,
   and this decision MUST be made inside `submit_plan_node` — never by a broker pre-check that
   short-circuits before the executor is called.
4. **The per-arg I2 Block always takes precedence over condition (3)'s class-level deny (B1).** A
   plan node carrying a tainted routing-sensitive arg MUST return `BlockedPendingConfirmation` from
   the per-arg loop regardless of `session_status` — including on a `Draft` session — and Step 0.5
   MUST NOT be evaluated (and therefore cannot pre-empt the Block) whenever the loop returns a Block.
   This is what keeps ACC-01/ACC-02 satisfiable: confirming a Block on a `Draft` session's tainted arg
   IS the literal-value human-gate act I0/I1 require, per `DESIGN-confirmation-release.md`'s "Two
   Independent Mechanisms."
5. **`MutateReversible`/`Observe` still succeed on a `Draft` session.** A plan node whose
   `sink_effect_class` is `Observe` or `MutateReversible`, submitted on a `Draft` session, MUST NOT be
   denied by Step 0.5 and MUST proceed through the existing per-arg taint checks unaffected.

All 6 conditions MUST hold simultaneously. Condition (0) is what makes (1), (2), (3), and (4)
meaningful: without trusted, broker-resolved trust-state sourcing, an injected worker or planner
simply asserts its session is `Active` and bypasses Step 0.5 entirely — the identical shape as the
taint-stripping hole the `ValueId` handle model closes for I2, and the identical shape as the
self-declaration hole `DESIGN-taint-model.md`'s I0 Acceptance Predicate condition 0 already closes
for session creation. Condition (4) is what keeps this document's mechanism from silently defeating
`DESIGN-confirmation-release.md`'s: without I2-Block-takes-precedence, no `Draft` session's tainted
arg could ever reach a confirmable Block, and the confirmation-release mechanism would have no live
entry point — this was the round-1 bug this amendment fixes. Condition (5) exists so that the
mechanism's restriction is verifiably scoped — a demoted session is not rendered inert, only
restricted from `CommitIrreversible` effects, which is what TAINT-03 requires and what makes the I1
dynamic-taint model usable rather than a de facto kill switch.

---

## 12. Accepted Residual Risks

**1. Demotion race between `mint_from_read` and a concurrent `submit_plan_node`**

Because `mint_from_read` and `submit_plan_node` are separate calls, a race is theoretically possible
where a `submit_plan_node` call for a `CommitIrreversible` sink resolves `session_status` as `Active`
a moment before a concurrent `mint_from_read` demotes the same session to `Draft`, allowing a plan
node to proceed on the boundary.

*Accepted for v1.2:* Mitigated structurally by v0/v1.2's single-shot, single-session, effectively
single-threaded-per-session process model (`caprun` runs one session to completion per process
invocation, per `cli/caprun/src/main.rs`'s single-worker-per-session design) — there is no
multi-worker-per-session concurrency in scope for this milestone (post-v0 roadmap: "v2 —
multi-worker decomposition, parallel execution"). §5's atomicity requirement (the `sessions` UPDATE
and `session_demoted` Event append happen inside one lock/transaction) ensures the demotion itself is
never partially visible. Full protection against a genuine multi-worker race is deferred to the v2
multi-worker milestone, where `session_status` resolution and `submit_plan_node` dispatch would need
to be serialized per-session (e.g., a per-session mutex held across both the mint and the submit
call) — out of scope here.

**2. Unknown-sink fail-closed choice is currently unreachable, not yet tested against a live gap**

§6's fail-closed unknown-sink handling for `sink_effect_class` is specified as maximally restrictive
(`CommitIrreversible`), but because Step 0's schema validation already rejects any `SinkId` not in the
`KNOWN_SINKS` registry before Step 0.5 runs, this branch is currently dead code in the live path.

*Accepted for v1.2:* Documented explicitly (§6) so a future refactor that reorders or removes the
Step 0 schema gate does not silently reintroduce a permissive default at this call site. No
additional runtime enforcement is required while Step 0's ordering guarantee holds; Phase 9's
implementation MUST preserve Step 0 running before Step 0.5 (see §8's fixed placement rule).

---

*End of DESIGN-session-trust-state.md*
