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

## Appendix A: production append call-site inventory

This inventory was derived by searching `append_event(` under `crates/*/src/` and `cli/*/src/`, then excluding definitions and calls inside test-only items/modules. Line numbers describe the pre-fix tree reviewed by Plan 51-06. The three rows whose connection is a `Transaction` also name the enclosing transaction's line anchor.

| File | Line | Calling function | Role | Required treatment |
|---|---:|---|---|---|
| `cli/caprun/src/main.rs` | 481 | `main` (`session_created`) | `session-root` | `explicit note` — genuine first event; choke point preserves NULL parent on an empty session |
| `cli/caprun/src/main.rs` | 506 | `main` (`policy_bound`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/audit.rs` | 561 | `record_github_grant` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/confirmation.rs` | 808 | `append_digest_mismatch_event` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/confirmation.rs` | 1095 | `confirm` (`confirm_granted`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/confirmation.rs` | 1163/1179 | `confirm` (`email_send_attempted`) | `in-transaction` | `enclosing transaction must become IMMEDIATE` |
| `crates/brokerd/src/confirmation.rs` | 1304 | `confirm` (`email_send_suppressed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/confirmation.rs` | 1329 | `confirm` (`email_send_failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/confirmation.rs` | 1469 | `deny` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/quarantine.rs` | 372 | `mint_from_read` (`raw_read`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/quarantine.rs` | 417 | `mint_from_read` (`session_demoted`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/quarantine.rs` | 493 | `mint_from_intent` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/quarantine.rs` | 784 | `mint_from_derivation` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/quarantine.rs` | 937 | `mint_from_http` (`http_response_read`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/quarantine.rs` | 969 | `mint_from_http` (`session_demoted`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/server.rs` | 1022 | `evaluate_plan_node_and_record` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/server.rs` | 1184/1226 | `evaluate_plan_node_and_record` (`sent_plan_node`) | `in-transaction` | `enclosing transaction must become IMMEDIATE` |
| `crates/brokerd/src/server.rs` | 1437 | `evaluate_plan_node_and_record` (`sink_execution_failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/server.rs` | 1509 | `evaluate_plan_node_and_record` (`policy_denied`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/server.rs` | 1557/1579 | `evaluate_plan_node_and_record` (`sent_plan_node`) | `in-transaction` | `enclosing transaction must become IMMEDIATE` |
| `crates/brokerd/src/server.rs` | 1803 | `create_session_arm` | `session-root` | `explicit note` — disabled arm creates the genuine first event for its session |
| `crates/brokerd/src/server.rs` | 1985 | `dispatch_request` (`raw_read`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/server.rs` | 2068 | `dispatch_request` (`session_demoted`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/email_smtp.rs` | 265 | `invoke_email_smtp_from_resolved` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/email_smtp.rs` | 299 | `record_send_failed` | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_create.rs` | 95 | `invoke_file_create` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_create.rs` | 111 | `invoke_file_create` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_create.rs` | 201 | `invoke_file_create_from_resolved` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_create.rs` | 218 | `invoke_file_create_from_resolved` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_write.rs` | 104 | `invoke_file_write` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_write.rs` | 120 | `invoke_file_write` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_write.rs` | 214 | `invoke_file_write_from_resolved` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/file_write.rs` | 231 | `invoke_file_write_from_resolved` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/git_commit.rs` | 200 | `invoke_git_commit` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/git_commit.rs` | 224 | `invoke_git_commit` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/git_push.rs` | 776 | `append_push_outcome` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/git_push.rs` | 794 | `append_push_outcome` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/github_pr.rs` | 196 | `append_pr_outcome` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/github_pr.rs` | 224 | `append_pr_outcome` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/http_write.rs` | 184 | `append_write_outcome` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/http_write.rs` | 217 | `append_write_outcome` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/process_exec.rs` | 177 | `invoke_process_exec` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/process_exec.rs` | 199 | `invoke_process_exec` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/process_exec.rs` | 317 | `invoke_process_exec_from_resolved` (`succeeded`) | `chain-continuation` | `inherits choke point (no call-site edit)` |
| `crates/brokerd/src/sinks/process_exec.rs` | 334 | `invoke_process_exec_from_resolved` (`failed`) | `chain-continuation` | `inherits choke point (no call-site edit)` |

**Completeness result:** 45 production call sites: 2 `session-root`, 40 `chain-continuation`, and 3 `in-transaction`. No production call site outside this table exists. `crates/brokerd/src/policy.rs` also contains the `session_created`/`policy_bound` append pattern at lines 373 and 389, but only inside its `#[cfg(test)] mod tests`; it was mechanically excluded, while the production equivalents are the first two `cli/caprun/src/main.rs` rows. No production append exists in `file_write`'s sibling `file_create`-unrelated modules beyond those listed, and the terminal sink files named here form the adversarial trace checklist for Plan 51-08.
