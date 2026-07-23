---
phase: 29-sink-path-hardening-replay-cas-contents-slot
plan: 02
subsystem: security
tags: [rusqlite, sqlite-transaction, cas, idempotency, mailpit, smtp, audit-chain]

requires:
  - phase: 29-sink-path-hardening-replay-cas-contents-slot/29-01
    provides: "sent_plan_nodes CAS table + plan_node_idempotency_key(sink, args) content-derived key fn (both in audit.rs)"
provides:
  - "server.rs's Allowed email.send dispatch block is CAS-guarded: idempotency_key computed from broker-resolved sink+value_ids, INSERT OR IGNORE + attempt-marker append committed in one rusqlite::Transaction BEFORE any SMTP socket opens"
  - "crates/brokerd/tests/replay_cas.rs — Linux-only, Mailpit-backed live proof that a replayed identical Allowed email.send delivers exactly once"
affects: [29-sink-path-hardening-replay-cas-contents-slot/29-03, 30-live-proof]

tech-stack:
  added: []
  patterns:
    - "Branch-divergent audit event type inside a CAS transaction: fresh send appends email_send_attempted, suppressed replay appends a DISTINCT email_send_replay_suppressed marker — keeps the primary attempt-ledger event type at exactly-one-per-plan-node while still recording every replay durably"
    - "conn: &Arc<Mutex<Connection>> transaction composition: `let mut locked = conn.lock()?; let tx = locked.transaction()?;` — the tx composes UNDER the mutex (concurrent broker tasks), architecturally distinct from confirmation.rs's single-process &mut Connection"

key-files:
  created:
    - crates/brokerd/tests/replay_cas.rs
  modified:
    - crates/brokerd/src/server.rs

key-decisions:
  - "The event appended inside the CAS transaction DIVERGES by branch (email_send_attempted on fresh send vs. email_send_replay_suppressed on replay), not a single shared event type on both paths — required to satisfy the plan's must_haves.truths literal wording (\"exactly one email_send_attempted event... across two identical submits\"); a naive read of the task action text (\"append the email_send_attempted... event as appropriate\") would have appended the SAME event type on both paths, producing TWO email_send_attempted events per replay pair and breaking that exact assertion. Caught during self-review before running the Linux gate, not by a failing test."
  - "idempotency_key is computed from plan_node.sink/plan_node.args (the PlanArg value_ids) BEFORE the resolve-loop, not from resolved_args' literal values — matches plan_node_idempotency_key's signature (&SinkId, &[PlanArg]) from 29-01 and the plan's own placement instruction (\"at the top of the block\")."
  - "replay_cas.rs duplicates the Mailpit HTTP-polling helpers (http_request/decode_chunked/http_get_json/addresses/fetch_message_detail) from email_smtp_acceptance.rs rather than sharing a module — each integration-test file in this crate compiles as its own binary with no shared `tests/common` module (confirmed: no such module exists), matching this file's own established convention."

patterns-established:
  - "Drive a real run_broker_server instance over a real Unix socket for a same-connection double-submit (ProvideIntent once, then SubmitPlanNode with the IDENTICAL PlanNode twice) — a narrower, single-connection variant of two_connection_intent_bypass.rs's spawn_fresh_broker harness."

requirements-completed: [HARDEN-03]

coverage:
  - id: D1
    description: "The Allowed email.send dispatch block computes a content-derived idempotency_key (never effect_id-keyed), wraps the CAS INSERT OR IGNORE + attempt-marker append in one atomic transaction committed before any SMTP socket opens, and suppresses the second SMTP send on replay (rows_affected == 0)"
    requirement: "HARDEN-03"
    verification:
      - kind: unit
        ref: "cargo build --workspace (clean) + ./scripts/check-invariants.sh (4/4 gates)"
        status: pass
    human_judgment: false
  - id: D2
    description: "A replayed identical Allowed email.send delivers exactly once — proven live on real Linux: 1 Mailpit delivery, 1 sent_plan_nodes row, 1 email_send_attempted event across two identical submits"
    requirement: "HARDEN-03"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/replay_cas.rs#allowed_email_send_replay_delivers_once — run via scripts/mailpit-verify.sh on real Linux (Colima), true exit 0, 1 passed"
        status: pass
      - kind: integration
        ref: "full workspace bash scripts/mailpit-verify.sh (wave-merge regression check) — true exit 0, all test binaries green, no regressions to SMTP-03/SMTP-05/two_connection_intent_bypass/SEND-01"
        status: pass
    human_judgment: false

duration: ~50min
completed: 2026-07-17
status: complete
---

# Phase 29 Plan 02: Replay CAS Dispatch-Site Wiring Summary

**Wired the HARDEN-03 replay CAS into server.rs's Allowed email.send dispatch (CAS INSERT OR IGNORE + attempt-marker append committed in one transaction before any SMTP socket opens) and proved at-most-once-per-plan-node live on real Linux via a new Mailpit-backed double-submit integration test.**

## Performance

- **Duration:** ~50 min (includes two live Linux/Colima/Mailpit gate runs)
- **Completed:** 2026-07-17
- **Tasks:** 2
- **Files modified:** 2 (1 modified, 1 created)

## Accomplishments
- `server.rs`'s Allowed `email.send` dispatch block now computes `idempotency_key = crate::audit::plan_node_idempotency_key(&plan_node.sink, &plan_node.args)` at the top of the block, from broker-resolved handles only.
- Fixed the confirmed immutable-binding landmine at the former `server.rs:926` (`let locked` → `let mut locked`) so `Connection::transaction(&mut self)` compiles.
- CAS `INSERT OR IGNORE INTO sent_plan_nodes` + a durable attempt-marker `append_event` now commit in ONE atomic `rusqlite::Transaction`, strictly BEFORE `invoke_email_smtp_from_resolved` ever runs.
- On replay (`rows_affected == 0`): the SMTP send is suppressed, a `email_send_replay_suppressed` event is durably appended instead of a second `email_send_attempted`, and the transaction still commits cleanly.
- Replaced the stale "REPLAY RESIDUAL RISK" comment (formerly `server.rs:959`) with an honest, unsoftened statement of the new guarantee: at-most-once PER PLAN NODE (identical `value_id`s), not a bounded-sends-per-session guarantee (D-08 caveat).
- Added `crates/brokerd/tests/replay_cas.rs` (`#![cfg(target_os = "linux")]`) — spawns a real `run_broker_server` instance, drives `ProvideIntent` then submits the IDENTICAL `SubmitPlanNode` twice on one connection, and asserts all three: exactly one Mailpit delivery, one `sent_plan_nodes` row, one `email_send_attempted` event.
- **Ran the live Linux gate myself** (Colima was already running): `MAILPIT_VERIFY_CMD='... --test replay_cas allowed_email_send_replay_delivers_once' bash scripts/mailpit-verify.sh` → true exit 0, `1 passed`. Then ran the FULL workspace `bash scripts/mailpit-verify.sh` (wave-merge regression check) → true exit 0, all test binaries green (SMTP-03/SMTP-05 acceptance, `two_connection_intent_bypass`, confirm-path SEND-01, chain verification — no regressions).

## Task Commits

Each task was committed atomically:

1. **Task 1: CAS-guard the Allowed email.send dispatch (HARDEN-03)** - `a22ac9b` (feat)
2. **Task 2: Linux double-submit replay integration test (HARDEN-03)** - `7041470` (feat) — this commit also carries a fix to Task 1's code (the branch-divergent-event-type refinement, see Deviations), discovered while writing this task's assertions and landed alongside the new test rather than amending the already-made Task 1 commit.

## Files Created/Modified
- `crates/brokerd/src/server.rs` — Allowed `email.send` dispatch block: CAS-guarded transaction, branch-divergent event type, updated comments.
- `crates/brokerd/tests/replay_cas.rs` — new Linux-only, Mailpit-backed double-submit integration test (`allowed_email_send_replay_delivers_once`), duplicating the Mailpit HTTP-polling helpers from `email_smtp_acceptance.rs` (no shared test-support module exists in this crate).

## Decisions Made
- Diverge the in-transaction event type by branch (`email_send_attempted` on fresh / `email_send_replay_suppressed` on replay) rather than appending the same event type on both paths — required by the plan's own literal success criterion ("exactly one `email_send_attempted` event... across two identical submits"). Caught this during my own pre-Linux-gate review of the first draft, not by a failing test — the initial Task 1 commit appended `email_send_attempted` unconditionally on both branches, which would have produced two such events per replay pair. Fixed before writing the test, then verified with both a targeted single-test Linux run and a full-workspace Linux run.
- `idempotency_key` is computed from `plan_node.sink`/`plan_node.args` (the un-resolved `PlanArg` value_ids), matching the 29-01 function signature and the plan's placement instruction, not from the resolved literal values.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed a self-introduced double-count of email_send_attempted before it ever reached a test**
- **Found during:** Task 2 (writing the integration test's assertions against the plan's own "exactly one email_send_attempted event" success criterion)
- **Issue:** My first draft of Task 1's CAS-guard appended an `email_send_attempted` event on BOTH the fresh-send and the suppressed-replay branch (reading the task action text's "append the email_send_attempted ... event as appropriate for audit continuity" too literally). That would have produced two `email_send_attempted` events per replay pair, contradicting the plan's `must_haves.truths` wording.
- **Fix:** Diverged the in-transaction event type by branch: fresh send → `email_send_attempted`; suppressed replay → a distinct `email_send_replay_suppressed` marker event (both still chain onto the audit DAG head and commit in the same atomic transaction).
- **Files modified:** `crates/brokerd/src/server.rs`
- **Verification:** `cargo build --workspace`, `./scripts/check-invariants.sh` (4/4), then the live Linux `replay_cas.rs` assertion (`COUNT(*) FROM events WHERE event_type = 'email_send_attempted'` == 1) passed on the first real Linux run — no red/fix cycle needed on Linux itself.
- **Committed in:** `7041470` (Task 2 commit, alongside the new test)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug caught pre-Linux-gate during my own review)
**Impact on plan:** Necessary for correctness against the plan's own literal success criteria. No scope creep — the fix is entirely inside the Task 1 block the plan already scoped.

## Issues Encountered
None beyond the deviation above — both the isolated `replay_cas` test and the full-workspace Linux suite passed on the first real-Linux run.

## User Setup Required

None - no external service configuration required. (Colima + the `scripts/mailpit-verify.sh` sidecar were already available in this environment and used directly to verify, rather than deferring to the orchestrator's post-return Linux gate — both a scoped single-test run and the full-workspace run came back true-exit-0 green.)

## Next Phase Readiness

- `sent_plan_nodes`/`plan_node_idempotency_key` (29-01) and the CAS-guarded dispatch site + its Linux proof (29-02) are both complete — HARDEN-03 is fully closed.
- `cargo build --workspace` compiles clean (the `dead_code` warning on `plan_node_idempotency_key` noted in 29-01's summary is now resolved — it is consumed at the dispatch site).
- `./scripts/check-invariants.sh` — all 4 gates pass (no `EffectRequest`, no carried `PlanNode` idempotency field).
- Full workspace `cargo test --workspace --no-fail-fast` (macOS) — all green, no regressions.
- Full workspace `bash scripts/mailpit-verify.sh` (real Linux) — all green, no regressions to SMTP-03/SMTP-05/confirm-path SEND-01/two_connection_intent_bypass.
- Ready for 29-03 (HARDEN-05 contents slot) and Phase 30's live A/B proof, per the phase's wave plan.

---
*Phase: 29-sink-path-hardening-replay-cas-contents-slot*
*Completed: 2026-07-17*

## Self-Check: PASSED

- FOUND: crates/brokerd/src/server.rs
- FOUND: crates/brokerd/tests/replay_cas.rs
- FOUND: a22ac9b (feat: CAS-guard the Allowed email.send dispatch)
- FOUND: 7041470 (feat: Linux double-submit replay integration test)
