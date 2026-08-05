# DESIGN — Audit Append-at-Head Concurrency

**Phase:** 51 gap closure  
**Status:** PENDING independent adversarial review  
**Scope:** Contract for the coordinated D1+D2 repair; no implementation is authorised by this document alone.

## 1. Invariant restated: the event parent_id chain is linear

The per-session event `parent_id` chain is **LINEAR BY DESIGN**. The contract is already explicit at `audit.rs:1203-1219` and `confirmation.rs:777-783`: each event extends the one current causal head. The MAC'd `chain_anchor` stores one `head_event_id` plus one `event_count`; that representation cannot authenticate competing branches as valid heads. Loosening `verify_chain` is **FORBIDDEN**, because accepting a branch would also delete HARDEN-02 tail-truncation detection by severing the anchor's single-head-and-count cross-check.

**`verify_chain` is correct; the append path is the bug.**

## 2. Two graphs, never conflated

The causal event chain and the provenance graph are different graphs. Event `parent_id` records causal append order. `read_event_id`, `provenance_chain`, and `SinkBlockedAnchor` record the value lineage used by I2 and M7. The existing comments at `server.rs:941` and `server.rs:973` already state that the causal head is never `read_event_id`.

**Hard rule:** this change must not read, write, derive, or unify `read_event_id` or `provenance_chain`. A repair that “unifies the two graphs” silently destroys the I2 attribution contract and is rejected.

## 3. The choke point

The single public append entry point, `brokerd::audit::append_event`, becomes the transactional append-at-head operation while retaining its name, parameter list, and return type. This placement keeps all 141 existing call sites compiling unchanged, removes the ability to fork instead of adding a safe parallel API beside an unsafe one, and covers both latent instances described in Section 8.

The published contract is:

- the durable session head read inside the transaction is **AUTHORITATIVE**;
- the caller-supplied `parent_hash` is advisory and cannot select the persisted parent;
- the return value remains the new row's hash; and
- no public forking append survives the change.

Any variant placement is acceptable only if it preserves that last property. A public raw append capable of persisting a caller-selected stale parent is rejected by this gate.

## 4. Transaction discipline

One `BEGIN IMMEDIATE`, expressed as rusqlite `TransactionBehavior::Immediate` through `transaction_with_behavior`, wraps the durable-head read, the `events` `INSERT`, and the `chain_anchor` upsert. The write lock is acquired before parent selection. Because `parent_id` is serialized into the payload covered by the event MAC, selecting it outside the transaction permits it to become stale between selection and insertion.

Re-entrancy is explicit: `append_event` checks `rusqlite::Connection::is_autocommit`. If the handle is in autocommit mode, it opens and commits the immediate transaction. If the handle already represents an enclosing transaction, it joins that boundary rather than opening a nested transaction, and reads the head inside that boundary. The enclosing transactions at `server.rs:1184`, `server.rs:1557`, and `confirmation.rs:1163` must themselves switch from default DEFERRED behavior to `TransactionBehavior::Immediate`; otherwise their read-then-upgrade shape can still receive an SQLite busy failure that is deliberately not retried.

## 5. busy_timeout ownership

Measured fact: rusqlite 0.32.1 calls `sqlite3_busy_timeout(db, 5000)` unconditionally in `InnerConnection::open_with_flags` at `inner_connection.rs:119`. The absence of a `busy_timeout` token in this repository therefore did not mean a zero timeout, and a regression test that merely asserts a non-zero pragma today would be a false red.

The repair introduces the module constant `AUDIT_BUSY_TIMEOUT_MS` with value `5000` and makes `open_audit_db` set the timeout explicitly. This moves ownership of the guarantee into the TCB and prevents a driver upgrade from changing it implicitly. The timeout alone does **not** fix D1; the immediate transaction does. Both must land together because a timeout alone can turn the observed crash into a successfully committed fork.

## 6. parent_id rebinding and payload consequences

Rebinding preserves the event's `id`, but the persisted `parent_id` is the durable head rather than the parent locally constructed by the caller. The serialized payload and its MAC consequently cover the rebound durable parent, and the hash input uses the corresponding durable `parent_hash`. No caller may assert on its locally constructed `parent_id` after append. The implementation must audit the repository for assertions of that shape and update none by weakening the persisted-parent contract.

## 7. Atomicity non-regression for the sink_blocked group

The group at `server.rs:1017-1059` currently executes the event append, every `insert_blocked_literal`, and `insert_pending_confirmation` as separate autocommit statements under one process mutex. The repair deliberately leaves this group at its current database atomicity: append-at-head makes the event append internally atomic, while the side-table statements retain their existing fail-closed ordering under the same mutex. It must become neither less ordered nor partially reordered. Hoisting the group into a new encompassing transaction is outside this defect repair because it changes crash atomicity beyond the D1+D2 contract; any future such change requires its own design decision.

## 8. Latent instance dispositions

- `record_github_grant` (`audit.rs:546-561`) — **FIXED BY CONSTRUCTION**. Its existing read-then-append pair may still supply an advisory stale hash, but the Section 3 choke point discards that authority and binds the row to the durable head under its immediate transaction. This disposition would be false if the function bypassed `append_event`, or if any public append path could still persist the supplied parent.
- Dual-connection same-seed head (`server.rs:307-382`) — **FIXED BY CONSTRUCTION**. Both connections may retain identical initial in-memory heads (and the Planner role remains documented unused), but every eventual append is rebound at the Section 3 choke point. This disposition would be false if any production append bypassed that choke point or chose its persisted parent before acquiring the write boundary.

## 9. Non-goals

- `verify_chain` remains unchanged.
- The MAC scheme, `EVENT_MAC_DOMAIN`, `ANCHOR_MAC_DOMAIN`, and `chain_anchor` semantics remain unchanged.
- The `current_chain_head` query remains unchanged.
- The provenance graph, executor I2, and policy are untouched.
- HYG-02 remains intact: zero new crates, zero new mint sites, and no `EffectRequest` token; invariant Gates 1 and 3 remain authoritative.
- `caprun run` output does not change.
- Plan 51-04 is neither re-planned nor weakened; it will be rerun unchanged after gap closure.

## 10. Acceptance oracle and rollback

The committed oracle is `crates/brokerd/tests/audit_chain_fork_regression.rs`, specifically `broker_append_after_external_grant_must_not_fork_chain` and `contended_appends_from_independent_connections_stay_linear`. Both tests must move from failing to passing without being edited.

The implementation is one revertible commit range confined to `crates/brokerd/src/`. Reversibility is rated **costly**, not one-way: the audit database format and MAC scheme do not change, so reverting the implementation leaves existing databases readable. Reconsidering the contract after implementation would invalidate this oracle and the required adversarial trace.
