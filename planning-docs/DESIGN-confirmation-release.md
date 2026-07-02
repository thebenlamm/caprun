# DESIGN-confirmation-release.md — AgentOS Confirmation-Release Mechanism (v1.2)

**Requirement:** PROC-01 (forward-referencing CONFIRM-01, CONFIRM-02, CONFIRM-03, CONFIRM-04)
**Status:** Draft — pending DESIGN-GATE-RECORD-v1.2.md approval
**Canonical source:** `planning-docs/PLAN.md` (wins on any conflict)
**Gate:** `crates/executor` and `crates/brokerd` MUST NOT gain confirmation-release code until
this document AND `DESIGN-session-trust-state.md` are recorded APPROVED in
`DESIGN-GATE-RECORD-v1.2.md`.

**Prior art:** This document extends `DESIGN-plan-executor.md`'s `ValueRecord`/`ValueId` handle
model and `DESIGN-taint-model.md`'s Declassification & Endorsement section to a new durable,
cross-process pause/resume workflow. It does not redefine either — it specifies the mechanism that
sits between a `BlockedPendingConfirmation` decision and a later human release.

---

## The Problem Being Solved

`caprun` is a **single-shot process**. It opens the audit DB, creates a Session, runs exactly one
session to completion or to a `Block`, prints the audit DAG, and exits (`cli/caprun/src/main.rs`).
When the executor returns `ExecutorDecision::BlockedPendingConfirmation`, the broker appends a
`sink_blocked` Event anchored by `SinkBlockedAnchor` and the process exits non-zero. Nothing about
this process survives past that exit.

`caprun confirm <effect_id>` is necessarily a **SECOND, LATER process** — invoked by the human,
potentially minutes, hours, or days after the block, against the **same persistent SQLite audit
DB** (never `:memory:` — a confirm workflow against an in-memory DB has nothing to confirm against
once the blocking process exits). By the time this second process runs:

- The in-memory `executor::ValueStore` (`crates/executor/src/value_store.rs`) that resolved every
  `ValueId` in the original `PlanNode` — including the untainted `contents` arg alongside the
  tainted `path` arg for a `file.create` block — is **gone**. It was a per-connection, in-memory
  structure; it does not survive process exit and MUST NOT be assumed to.
- The `SinkBlockedAnchor` inside the `sink_blocked` Event's hashed payload carries only the ONE
  blocked arg's resolved value (`arg`, `value_id`, `literal_sha256`, `taint`, `provenance_chain`).
  It was designed to prove taint genuineness for the blocked arg, not to reconstruct a full effect
  invocation.

This is structurally a **durable pause-and-resume workflow with a checkpoint**, not an in-memory
callback. The full pending-action payload MUST be persisted BEFORE the process halts, so a later,
independent process can resume from exactly that checkpoint and re-invoke the sink — this MUST NOT
depend on any in-memory state from the original blocking process. Everything in this document
follows from that one constraint.

---

## PendingConfirmation Schema

`PendingConfirmation` is a NEW, DISTINCT durable record. It is a **superset of `SinkBlockedAnchor`**,
NOT an extension of it: `SinkBlockedAnchor` rides inside the hashed `payload` column of the
`sink_blocked` Event and has its own tamper-evidence / redactability contract (`literal_sha256`,
golden-byte-fixture serialization tests per `DESIGN-plan-executor.md` §5). Adding fields to
`SinkBlockedAnchor` to carry the full arg set risks breaking that existing contract. `PendingConfirmation`
is therefore a sibling record, persisted alongside (never inside) the anchor.

```rust
// planning-docs shape — not yet in crates/. Phase 10 implements this exactly.
// Fixes Pitfall M1: PlanNode carries only opaque ValueId handles (see
// crates/runtime-core/src/plan_node.rs) — it cannot hold literal/taint/provenance data, and the
// in-memory ValueStore that could resolve those handles is gone by confirm time (see "The Problem
// Being Solved"). PendingConfirmation therefore persists a RESOLVED SNAPSHOT, not a PlanNode.
struct PendingConfirmation {
    effect_id:    Uuid,                  // SAME anchor key as SinkBlockedAnchor.effect_id
    session_id:   Uuid,
    sink:         SinkId,                // the blocked plan node's sink (from PlanNode.sink)
    resolved_args: Vec<ResolvedArg>,     // FULL resolved arg set, captured at Block time
    state:        PendingConfirmationState,
}

struct ResolvedArg {
    name:             String,            // matches the original PlanArg.name
    value_id:         ValueId,           // the original PlanArg.value_id, kept for audit traceability
    literal:          String,            // the dereferenced ValueRecord's literal, frozen at Block time
    taint:            Vec<TaintLabel>,   // the dereferenced ValueRecord's taint set, frozen at Block time
    provenance_chain: Vec<Uuid>,         // the dereferenced ValueRecord's provenance chain, frozen at Block time
}

enum PendingConfirmationState {
    Pending,
    Confirmed,
    Denied,
}
```

**Field | Purpose table:**

| Field | Purpose |
|-------|---------|
| `effect_id` | The SAME identifier as `SinkBlockedAnchor.effect_id` (CONFIRM-04's anchor key). Confirm/deny audit Events anchor to this identical id, preserving one unbroken causal chain from block through decision. Broker-minted, never client- or worker-supplied. |
| `session_id` | The Session the blocked plan node belonged to. Required so `caprun confirm` can look up session context and so the confirm/deny Event can be appended under the correct `session_id` in the `events` table. |
| `sink` | The blocked plan node's `SinkId` (e.g. `file.create`), copied from the original `PlanNode.sink` at Block time. `PlanNode` itself is never persisted here — only this locked, opaque identifier. |
| `resolved_args` | The FULL resolved arg set for the blocked sink call: one `ResolvedArg` per original `PlanArg`, each carrying its dereferenced `ValueRecord`'s `literal`, `taint`, and `provenance_chain` — not merely the one arg that triggered the Block. This is what makes re-invocation of a multi-arg sink (e.g., `file.create`'s `path` AND `contents`) possible without re-resolving anything at confirm time, and it is the field M1 required: `PlanNode` alone (opaque `ValueId` handles) cannot carry this data once the original process's `ValueStore` is gone. |
| `state` | `Pending \| Confirmed \| Denied`. MUST start `Pending` at persistence time. Transitions exactly once, in exactly one direction: `Pending → Confirmed` or `Pending → Denied`. Never `Confirmed → Denied`, never `Denied → Confirmed`, never re-entry into `Pending`. |

**Persistence contract (MUST):**

- `PendingConfirmation` MUST be persisted **atomically with the `sink_blocked` Event append** — in
  the same transaction, under the same connection lock already used by `append_event` +
  `insert_blocked_literal` (see `crates/brokerd/src/audit.rs`'s existing pattern of writing the
  hashed anchor and the redactable side-table literal together). This mirrors the sole-mint-site
  discipline already used for `mint_from_read` (`crates/brokerd/src/quarantine.rs`): the same
  broker-owned write path that produces the `Block` decision is the only writer of the checkpoint
  that makes that `Block` resumable.
- `PendingConfirmation` MUST be written to the same persistent SQLite audit DB as the `events` table
  (a new side table, following the exact `blocked_literals` precedent — a redactable/mutable side
  table keyed by an id from the append-only ledger — never a separate database or file).
- If the `sink_blocked` Event append fails, the `PendingConfirmation` write MUST NOT happen (and
  vice versa) — the two writes MUST succeed or fail together, so a `sink_blocked` Event can never
  exist without a corresponding `PendingConfirmation` row, and no orphaned `PendingConfirmation` can
  exist without an anchoring `sink_blocked` Event.
- **Redaction interplay (fixes Pitfall m3):** `blocked_literals` is redactable by design (v1.1) —
  redacting a blocked literal MUST also redact (or otherwise invalidate for release) the same literal
  wherever it appears inside that `effect_id`'s `PendingConfirmation.resolved_args`. A
  `PendingConfirmation` row holds the blocked arg's literal PLUS every other arg's literal, so
  redacting only the `blocked_literals` side table while leaving an un-redacted copy in
  `resolved_args` would silently defeat the redaction. Phase 10 MUST either redact both side tables
  in the same operation, or have `caprun confirm` refuse to release (fail closed) any `effect_id`
  whose `blocked_literals` entry has been redacted.

---

## Confirmation Decision Logic

`caprun confirm <effect_id>` and `caprun deny <effect_id>` (see "caprun confirm CLI Contract" below —
fixes Pitfall M2) share Steps 1–3 as common setup, then branch by WHICH COMMAND was invoked — never
by an interactive prompt inside one command. Every rule is MUST / MUST NOT.

**Step 1 — Reopen the persistent audit DB.** Both commands MUST reopen the SAME persistent
SQLite file the original `caprun` run used (passed as the audit-db-path argument, mirroring
`cli/caprun/src/main.rs`'s existing `audit_path` parameter). Neither MUST operate against `:memory:`
— an in-memory DB has no state to resume from a prior process.

**Step 2 — Look up the `PendingConfirmation` row by `effect_id`.** This MUST use an indexed lookup
keyed on `effect_id` — a new capability, since no `find_event_by_effect_id`-equivalent exists in
`crates/brokerd/src/audit.rs` today (only `find_event_by_type` and `query_events_by_session`, both
keyed by `session_id`, not `effect_id`). Phase 10 MUST add either a dedicated
`find_pending_confirmation(conn, effect_id) -> Option<PendingConfirmation>` function or an indexed
`effect_id` column on the new side table (or both). If no row is found for `effect_id`, both
commands MUST fail closed: report "unknown effect_id" and exit non-zero — neither MUST silently
proceed or silently no-op as success.

**Step 3 — Check `state`.** The lookup result's `state` field MUST be read from persisted
`PendingConfirmation.state`, never from any in-memory value, because the process granting or
denying is never the same OS process as the one that created the block (Step 1's constraint). If
`state` is already `Confirmed` or `Denied`, both commands MUST refuse: no re-transition, no
retry, exit non-zero (CONFIRM-03). Only `state == Pending` MUST proceed to Step 4.

**Step 4a — `caprun confirm <effect_id>` path:**

1. Display the verbatim literal + provenance to the human (CONFIRM-01) — see "caprun confirm CLI
   Contract" below for the exact output format.
2. Append a `confirm_granted` Event to the audit DAG, anchored to `effect_id`, `parent_id` set to
   the `sink_blocked` Event's id (preserving the unbroken causal chain: read → taint → block →
   confirm).
3. Transition `PendingConfirmation.state` from `Pending` to `Confirmed` — persisted, atomic with the
   `confirm_granted` Event append, same transaction discipline as the original Block-time write.
4. Directly invoke the sink adapter (e.g. `invoke_file_create`) using the FROZEN, Block-time-resolved
   args from the `PendingConfirmation.resolved_args` snapshot. These args MUST NOT be re-resolved at
   confirm time — the executor's `ValueStore` from the original process is gone (per "The Problem
   Being Solved"), and re-resolving would reopen a TOCTOU-shaped question the frozen-snapshot design
   avoids by construction.
5. **At-most-once semantics (fixes Pitfall M3, MUST be explicit):** steps 3 and 4 are NOT one atomic
   transaction — a durable state transition to `Confirmed` is written before the sink is invoked,
   because the sink invocation itself cannot be made transactional with a SQLite write (it is a
   syscall against the filesystem/network, e.g. `openat2`). This document deliberately chooses
   **at-most-once**, not exactly-once, for confirm: if the process crashes, or the sink invocation
   fails (e.g. `O_EXCL` conflict, per Accepted Residual Risk 2 below), the state remains permanently
   `Confirmed` with no retry (Step 3 refuses re-transition). This is an accepted risk, not an
   oversight — see the exit-code table and the `sink_invocation_failed` Event below for how a caller
   observes this outcome.
   - If the sink invocation in step 4 fails, append a `sink_invocation_failed` Event to the audit
     DAG, anchored to the same `effect_id`, `parent_id` set to the `confirm_granted` Event's id — so
     the DAG shows `confirm_granted` with no successful invocation, distinguishing this outcome from
     both a clean confirm-and-release and a deny.
   - `caprun confirm` MUST exit non-zero in this case (see the exit-code table's dedicated row) —
     a scripted caller MUST be able to distinguish "released" (exit 0) from "confirm recorded, sink
     invocation failed" (exit non-zero, distinct from deny/unknown/already-terminal) without parsing
     stdout text.

**Step 4b — `caprun deny <effect_id>` path:**

1. Append a `confirm_denied` Event to the audit DAG, anchored to `effect_id`, `parent_id` set to the
   `sink_blocked` Event's id.
2. Transition `PendingConfirmation.state` from `Pending` to `Denied` — persisted, atomic with the
   `confirm_denied` Event append.
3. Terminal. No retry path. The sink is never invoked.

---

## Confirm MUST NOT Re-Invoke `submit_plan_node`

This is the critical soundness rule of this document.

**`caprun confirm` MUST NOT call `executor::submit_plan_node` a second time for the same
`effect_id`.**

Taint is monotonic — it is never cleared (`DESIGN-plan-executor.md` §Taint Propagation Rules, Rule
1). If confirm simply re-submitted the original `PlanNode` to `submit_plan_node`, one of two things
happens, both wrong:

- The value is still tainted, so `submit_plan_node` re-evaluates the same routing-sensitivity check
  and returns `Block` again — confirm becomes a no-op that can never actually release anything.
- OR, if the deny-again behavior were patched around by some special-cased bypass inside
  `submit_plan_node` itself, that bypass would silently weaken I2 for every OTHER, unrelated future
  block that happens to share code paths with it — the exact "policy file disables I2" failure mode
  `CON-i2-non-bypassable` forbids, except now baked into the TCB function itself instead of an
  external policy file.

Confirm is therefore a **DISTINCT, logged, human-endorsement path** — not a second pass through the
general I2 decision function. This mirrors `DESIGN-taint-model.md`'s Declassification & Endorsement
framing near-verbatim: release is never a silent allowlist mutation or a re-run of the general
decision rule; it is a broker-owned, TCB-resident audit Event, scoped to exactly one `effect_id`,
that authorizes exactly one already-adjudicated occurrence. The taint label on the underlying value
is never mutated or removed by a confirm — monotonicity is preserved exactly as
`DESIGN-taint-model.md` requires. What changes is not the value's taint; it is that a human-endorsed
audit edge (`confirm_granted`, anchored to `effect_id`) now exists, and the broker's confirm path
reads that edge — via the `PendingConfirmation` checkpoint, never via `submit_plan_node` — to decide
whether to invoke the sink for this one effect_id.

---

## caprun confirm CLI Contract

**Invocation (fixes Pitfall M2 — two distinct verbs, no interactive branch):**

- `caprun confirm <effect_id> [audit-db-path]` — releases the blocked effect (Step 4a).
- `caprun deny <effect_id> [audit-db-path]` — durably denies it (Step 4b).

Both use the same positional-arg shape as the existing `caprun` binary's `audit_path` trailer
(`cli/caprun/src/main.rs`), defaulting to the same convention if omitted. WHICH command the human
runs is what selects the branch in "Confirmation Decision Logic" above — there is no in-process
choice between confirm and deny; the two verbs are the only decision surface.

Both are **SECOND, EXPLICIT commands** — never an interactive TTY prompt. This is a locked UX
decision (mirroring the locked confirm-UX decision in STATE.md and `REQUIREMENTS.md`'s Out of Scope
table): `caprun confirm <effect_id>` and `caprun deny <effect_id>` MUST both be scriptable and
testable — invocable from a script, a CI harness, or a human's shell — without requiring a live
interactive terminal session attached to the original blocking process. Neither MUST be implemented
as a prompt embedded inside the original `caprun` invocation that blocks waiting for stdin.

**Exact terminal output format when the block is pending** (mirrors `DESIGN-plan-executor.md`
§Literal-Value Confirmation UX — verbatim, not abstract; shown by both `caprun confirm` and
`caprun deny` before acting, so the human sees the same evidence regardless of which verb they run):

```
$ caprun confirm <effect_id>

Effect blocked pending confirmation.

Effect ID:         <effect_id>
Sink:               file.create
Arg:                path
Literal value:      "../../etc/passwd"
Taint:              [external.untrusted, path.raw]
Source:             file_read evt_a1b2c3...  (session <session_id>)
Provenance chain:   evt_a1b2c3... -> evt_d4e5f6... -> (this arg)

This value came from untrusted content read during this session. Run
`caprun confirm <effect_id>` to release this EXACT value, or
`caprun deny <effect_id>` to block it permanently.
```

No interactive chooser is presented (fixes Pitfall M2's contradiction with the non-interactive lock
above) — the two commands to run are printed as text, and the human picks by which one they invoke.

The output MUST show: the literal value verbatim (not a category or summary), the sink and arg
name, the taint labels, the source read Event id and session id, and the `provenance_chain`
summary — the same "raw value, not a vague warning" discipline
`DESIGN-plan-executor.md` §Literal-Value Confirmation UX Rule 1 requires for the original Block
prompt.

**Exit-code contract:**

| Outcome | Exit code |
|---------|-----------|
| Confirm succeeds; sink invoked | 0 |
| Confirm recorded (`confirm_granted` appended, state → `Confirmed`) but sink invocation failed (fixes Pitfall M3) | non-zero, distinct from every row below |
| Deny recorded | non-zero |
| Unknown `effect_id` (no `PendingConfirmation` row found) | non-zero |
| `effect_id` already terminal (`Confirmed` or `Denied`) — refused | non-zero |

Only a successful confirm-and-release returns 0. Every other outcome — confirm-recorded-but-failed,
deny, unknown id, already-terminal id — MUST return a non-zero exit code, and the
confirm-recorded-but-failed case MUST be distinguishable from the others (e.g. a dedicated exit code
value), so a scripting/CI caller reading only the exit code can tell "released" from "confirm was
granted but nothing actually ran" from every other non-release outcome, without parsing output text.

---

## Single-Shot Release Semantics (CONFIRM-02)

A confirm releases **EXACTLY ONE** `(sink, arg, literal-digest)` triple. This is unambiguous and
MUST hold without exception:

- A confirm on `effect_id` X MUST NOT create any standing policy, exact-match allowlist entry, or
  session-wide waiver. It authorizes exactly one already-blocked, already-adjudicated occurrence —
  the one identified by `effect_id`.
- Every future block — even of the byte-identical literal, in the same sink/arg, in the same or a
  different session — MUST require its own separate `caprun confirm <new_effect_id>` call. A prior
  confirm MUST NOT be consulted, matched against, or auto-applied to any later block.
- Standing/pattern confirmation policy (an allowlist that pre-permits future occurrences of a
  literal or a pattern) is EXPLICITLY OUT OF SCOPE for this milestone
  (`.planning/REQUIREMENTS.md` Out of Scope table: "Standing/exact-match confirmation policy —
  Confirm is single-shot in v1.2; standing policy is scope creep"). This document MUST NOT imply,
  anywhere, that a confirm has any effect beyond its own `effect_id`.

This is a narrower semantics than `DESIGN-taint-model.md`'s Declassification & Endorsement section,
which describes a *post-v1.2* design where an endorsement Event MAY later be consulted for a
byte-identical literal. **For v1.2, that consultation step is explicitly NOT implemented** — the
endorsement Event exists (as `confirm_granted`, anchored to `effect_id`) for audit completeness, but
nothing in the confirm/deny path reads a PRIOR endorsement Event to auto-resolve a NEW block. Every
block requires its own confirm. Building the "auto-resolve against a prior endorsement" consultation
step is deferred to a future milestone.

---

## Durable-Deny Semantics (CONFIRM-03)

Once `PendingConfirmation.state` transitions to `Denied`:

- The blocked effect MUST NEVER proceed. The sink adapter MUST NEVER be invoked for that
  `effect_id` after a deny.
- The same `effect_id` MUST NEVER later be confirmed. There is no retry path, no override, no
  "deny was a mistake, try again" flow. A denied `effect_id` is terminal forever.
- This durability MUST be enforced by reading the persisted `PendingConfirmation.state` column from
  the audit DB — never from any in-memory state — because (restating the cross-process constraint
  from "Confirmation Decision Logic" Step 3 for CLI-contract completeness) the process that decides
  confirm-or-deny is never the same process that created the original block, and a second, later
  invocation of `caprun confirm` on an already-`Denied` `effect_id` is itself a distinct THIRD
  process that MUST see the same durable `Denied` state and refuse identically.

---

## TCB-Residency (CONFIRM-04)

The confirm/deny decision logic and the sink re-invocation specified in this document MUST be Rust
functions living inside `crates/brokerd` and/or `crates/executor` — the TCB. They MUST NEVER be:

- a configuration file,
- a policy engine (Cedar or otherwise),
- or any externally swappable component that a non-TCB actor could edit to alter the release
  decision.

This mirrors `CON-i2-non-bypassable` and `DESIGN-taint-model.md`'s I2 framing — "Policy files may
gate which sinks are callable; they MUST NOT disable I2" — and extends that same framing explicitly
to the confirmation-release path: policy MAY exist post-v1.2 to gate which sinks are eligible for
confirmation at all, but the confirm/deny decision itself, the terminal-state check, and the sink
re-invocation MUST remain hardcoded, TCB-resident Rust — never a swappable policy artifact.

---

## Relationship to Session-Trust-State DESIGN Doc & Done-When

### Two Independent Mechanisms

`DESIGN-session-trust-state.md` (session-trust-state: I1 dynamic demotion + I0 draft-only creation)
and this document (confirmation-release) are **two independent mechanisms.** Neither subsumes the
other, and satisfying one does not imply anything about the other:

- A Draft session's `CommitIrreversible`-class denial (I1/I0, decided by the executor's draft-only
  deny check) is a DISTINCT code path from a routing-sensitive I2 `Block`-then-Confirm. A session
  does NOT need to be `Draft` for a sink call to `Block` on a tainted routing-sensitive arg — I2
  applies regardless of session trust state, exactly as `DESIGN-plan-executor.md`'s executor
  decision logic always has.
- Confirming a `Block` (this document's mechanism) does NOT change the Session's trust state.
  `PendingConfirmation.state` transitioning to `Confirmed` has no effect on `SessionStatus`; a
  Session that was `Draft` before a confirm remains `Draft` after it, and vice versa. The two state
  machines are orthogonal.
- **This composition is designed, not incidental (amended per B1).** `DESIGN-session-trust-state.md`
  §8/§9/§11 requires the per-arg I2 Block to take precedence over the class-level draft-only deny —
  which means a `Draft` session's tainted routing-sensitive arg reaches exactly this document's
  Block-then-confirm mechanism, never the silent class-level Denied. A `caprun confirm` on that Block
  IS the literal-value human gate I0/I1 demand ("cannot **auto**-authorize Tier 3+ effects" —
  `REQUIREMENTS.md` ORIGIN-02): a human-endorsed, single-shot, TCB-resident confirm is not
  auto-authorization, so releasing a `Draft` session's confirmed Block is exactly the intended,
  designed interaction between the two mechanisms, not an accidental escape hatch.

### Done-When Predicate

This document satisfies PROC-01 (for the confirmation-release half) when the following are all
true:

1. `caprun confirm <effect_id>` displays the verbatim literal + provenance to the human, in the
   exact output format specified above (CONFIRM-01).
2. A confirm releases exactly one `(sink, arg, literal-digest)` triple — single-shot, with no
   standing policy, allowlist, or session-wide waiver created or consulted (CONFIRM-02).
3. A deny is durable: the effect never proceeds, and the same `effect_id` can never later be
   confirmed (CONFIRM-03).
4. Confirm and deny decisions are audited (`confirm_granted` / `confirm_denied` Events) and
   anchored to the same `effect_id` as `SinkBlockedAnchor.effect_id`, preserving one unbroken
   causal chain from block through decision (CONFIRM-04).
5. The confirm/deny decision logic and the sink re-invocation live in the TCB (`crates/brokerd` /
   `crates/executor`), never in a policy file or externally swappable artifact (CONFIRM-04).

All 5 conditions MUST hold simultaneously.

---

## Accepted Residual Risks

**1. A human confirming the wrong literal due to display-format limitations**

`DESIGN-plan-executor.md`'s Literal-Value Confirmation UX section specifies raw-AND-canonical
display, punycode-decoding, homoglyph-folding, and RTL-marker surfacing for the ORIGINAL Block
prompt (email recipients specifically). This document's `caprun confirm` output format (above)
reuses the same "show the literal, not a category" discipline and the same source/provenance
display, but v0/v1.2's `file.create` sink's blocked arg is a filesystem path, not an email address
— punycode/homoglyph canonicalization is an email-specific concern that does not directly apply to
paths. Path-specific confusables (e.g., visually similar Unicode characters in a filename, or a
path that looks locally-scoped but resolves outside the workspace root) are a distinct residual
risk this document does not fully canonicalize.

*Accepted for v1.2:* The literal path is displayed byte-exact (no truncation, no elision) and the
`provenance_chain` is shown, giving the human the raw material to inspect. Path-specific
canonicalization display rules (e.g., explicit `..`/symlink-target resolution shown alongside the
raw string) are deferred to a future milestone alongside any additional sinks that reintroduce
email-shaped routing args.

**2. A confirmed effect executing against a workspace that has changed between Block and Confirm**

Because `caprun confirm` re-invokes the sink using the FROZEN Block-time-resolved args (per
"Confirmation Decision Logic" Step 4a.4), and confirm may run an arbitrary amount of time after the
original block, the on-disk workspace state at confirm time may differ from what it was at block
time (e.g., a file at the target path may since have been created by an unrelated process).

*Accepted for v1.2:* The sink adapter's own invariants (e.g., `file.create`'s `O_EXCL` +
`openat2 RESOLVE_BENEATH`, per PROJECT.md's `SINK-01..04`) still apply at confirm-invocation time
and will fail closed on conflicts (e.g., `O_EXCL` rejects a pre-existing file) — this document does
not weaken those adapter-level guarantees. A broader workspace-snapshot-consistency guarantee across
the block/confirm boundary is out of scope for v1.2.
