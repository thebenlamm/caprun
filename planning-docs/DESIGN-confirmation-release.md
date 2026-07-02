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
struct PendingConfirmation {
    effect_id:  Uuid,                    // SAME anchor key as SinkBlockedAnchor.effect_id
    session_id: Uuid,
    plan_node:  PlanNode,                // FULL resolved arg set — every arg's ValueRecord,
                                          // not just the one blocked arg
    state:      PendingConfirmationState,
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
| `session_id` | The Session the blocked `PlanNode` belonged to. Required so `caprun confirm` can look up session context and so the confirm/deny Event can be appended under the correct `session_id` in the `events` table. |
| `plan_node` | The FULL resolved arg set for the blocked sink call: every `PlanArg { name, value_id }` together with its dereferenced `ValueRecord` (`literal`, `taint`, `provenance_chain`) — not merely the one arg that triggered the Block. This is what makes re-invocation of a multi-arg sink (e.g., `file.create`'s `path` AND `contents`) possible without re-resolving anything at confirm time. |
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

---

## Confirmation Decision Logic

`caprun confirm <effect_id>` executes the following ordered steps. Every rule is MUST / MUST NOT.

**Step 1 — Reopen the persistent audit DB.** `caprun confirm` MUST reopen the SAME persistent
SQLite file the original `caprun` run used (passed as the audit-db-path argument, mirroring
`cli/caprun/src/main.rs`'s existing `audit_path` parameter). It MUST NOT operate against `:memory:`
— an in-memory DB has no state to resume from a prior process.

**Step 2 — Look up the `PendingConfirmation` row by `effect_id`.** This MUST use an indexed lookup
keyed on `effect_id` — a new capability, since no `find_event_by_effect_id`-equivalent exists in
`crates/brokerd/src/audit.rs` today (only `find_event_by_type` and `query_events_by_session`, both
keyed by `session_id`, not `effect_id`). Phase 10 MUST add either a dedicated
`find_pending_confirmation(conn, effect_id) -> Option<PendingConfirmation>` function or an indexed
`effect_id` column on the new side table (or both). If no row is found for `effect_id`, `caprun
confirm` MUST fail closed: report "unknown effect_id" and exit non-zero — it MUST NOT silently
proceed or silently no-op as success.

**Step 3 — Check `state`.** The lookup result's `state` field MUST be read from persisted
`PendingConfirmation.state`, never from any in-memory value, because the process granting or
denying is never the same OS process as the one that created the block (Step 1's constraint). If
`state` is already `Confirmed` or `Denied`, `caprun confirm` MUST refuse: no re-transition, no
retry, exit non-zero (CONFIRM-03). Only `state == Pending` MUST proceed to Step 4.

**Step 4a — Confirm path.** If the human selects confirm:

1. Display the verbatim literal + provenance to the human (CONFIRM-01) — see "caprun confirm CLI
   Contract" below for the exact output format.
2. Append a `confirm_granted` Event to the audit DAG, anchored to `effect_id`, `parent_id` set to
   the `sink_blocked` Event's id (preserving the unbroken causal chain: read → taint → block →
   confirm).
3. Transition `PendingConfirmation.state` from `Pending` to `Confirmed` — persisted, atomic with the
   `confirm_granted` Event append, same transaction discipline as the original Block-time write.
4. Directly invoke the sink adapter (e.g. `invoke_file_create`) using the FROZEN, Block-time-resolved
   args from the `PendingConfirmation.plan_node` snapshot. These args MUST NOT be re-resolved at
   confirm time — the executor's `ValueStore` from the original process is gone (per "The Problem
   Being Solved"), and re-resolving would reopen a TOCTOU-shaped question the frozen-snapshot design
   avoids by construction.

**Step 4b — Deny path.** If the human selects deny:

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
