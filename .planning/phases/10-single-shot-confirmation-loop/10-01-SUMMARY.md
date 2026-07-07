---
phase: 10-single-shot-confirmation-loop
plan: 01
subsystem: brokerd
tags: [rusqlite, sqlite, sha256, audit-dag, serde, side-table]

# Dependency graph
requires: []
provides:
  - "pending_confirmations SQLite side table (effect_id PK, session_id, blocked_event_id, sink, resolved_args JSON, workspace_root_path, state) appended to brokerd's SCHEMA_DDL"
  - "event_hash_by_id(conn, event_id) -> Result<Option<String>> helper in audit.rs"
  - "crates/brokerd/src/confirmation.rs — PendingConfirmation, ResolvedArg, PendingConfirmationState types"
  - "insert_pending_confirmation / find_pending_confirmation / transition_state accessors with a SQL-enforced one-way state machine"
affects: [10-02, 10-03]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Side-table state machine with the terminal check baked into the SQL predicate (UPDATE ... WHERE state = 'pending') rather than a read-then-write race"
    - "Redactable/mutable side table keyed by an id from the append-only events ledger (mirrors blocked_literals) — never adding columns to the hashed events table"

key-files:
  created:
    - crates/brokerd/src/confirmation.rs
  modified:
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/lib.rs

key-decisions:
  - "pending_confirmations columns are all TEXT (effect_id PK, session_id, blocked_event_id, sink, resolved_args, workspace_root_path, state), STRICT table, no migration machinery (additive DDL only, matching the project's existing no-migration-framework convention)"
  - "resolved_args persisted as one JSON-serialized Vec<ResolvedArg> TEXT blob, mirroring the events.payload precedent, never a normalized child table (RESEARCH Open Question 2)"
  - "workspace_root_path persisted now per Assumption A2 (RESEARCH Open Question 1) — Plan 02 populates it, Plan 03 consumes it; changing this later only affects Plan 02/03, not this plan's security substance"
  - "PendingConfirmationState::from_str fails closed (Err) on any unrecognized persisted string rather than silently defaulting to Pending"

requirements-completed: [CONFIRM-03, CONFIRM-04]

coverage:
  - id: D1
    description: "pending_confirmations DDL table + event_hash_by_id helper added to audit.rs; existing SCHEMA_DDL/append_event/events table untouched"
    requirement: CONFIRM-04
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::pending_confirmations_insert_and_duplicate_effect_id_fails"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::event_hash_by_id_returns_hash_for_known_event"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::event_hash_by_id_returns_none_for_unknown_id"
        status: pass
    human_judgment: false
  - id: D2
    description: "confirmation.rs types (PendingConfirmation, ResolvedArg, PendingConfirmationState) + insert/find/transition_state accessors, with a SQL-enforced one-way Pending->Confirmed/Denied transition"
    requirement: CONFIRM-03
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::insert_then_find_round_trips_all_fields"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::transition_pending_to_confirmed_then_denied_is_refused"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::transition_pending_to_denied_then_confirmed_is_refused"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::find_unknown_effect_id_returns_none"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::pending_confirmation_state_from_str_rejects_unknown_string"
        status: pass
    human_judgment: false

duration: 20min
completed: 2026-07-07
status: complete
---

# Phase 10 Plan 01: Confirmation Checkpoint Substrate Summary

**Durable pending_confirmations SQLite side table + confirmation.rs record types/accessors giving a later, separate `caprun confirm`/`deny` process the SQL-guarded one-way state machine and full resolved-arg snapshot it needs to resume a block.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-07-07T03:55Z (approx)
- **Completed:** 2026-07-07T04:15Z
- **Tasks:** 2/2
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments
- Appended a `pending_confirmations` STRICT table to brokerd's `SCHEMA_DDL`, keyed on `effect_id PRIMARY KEY`, alongside the existing `blocked_literals` side table.
- Added `event_hash_by_id(conn, event_id)` to `audit.rs`, mirroring `get_blocked_literal`'s `QueryReturnedNoRows -> None` shape, for a later confirm/deny process to fetch the anchoring `sink_blocked` event's hash as `parent_hash`.
- Created `crates/brokerd/src/confirmation.rs` with `PendingConfirmation`, `ResolvedArg`, and `PendingConfirmationState` types, plus `insert_pending_confirmation` / `find_pending_confirmation` / `transition_state` accessors.
- `transition_state`'s `UPDATE ... WHERE effect_id = ?2 AND state = 'pending'` guard enforces the CONFIRM-03 fail-closed terminal check in SQL itself — a terminal row matches 0 rows on any re-transition attempt, verified by unit tests in both directions (Confirmed-then-Denied-refused, Denied-then-Confirmed-refused).
- Registered `pub mod confirmation;` in `crates/brokerd/src/lib.rs`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add pending_confirmations DDL + event_hash_by_id helper to audit.rs** - `ec29205` (feat)
2. **Task 2: Create confirmation.rs — PendingConfirmation types + side-table accessors** - `bc73289` (feat)

**Plan metadata:** (SUMMARY.md commit, see below)

## Files Created/Modified
- `crates/brokerd/src/audit.rs` - Added `pending_confirmations` DDL block + `event_hash_by_id` helper + 3 new unit tests
- `crates/brokerd/src/confirmation.rs` - New module: `PendingConfirmation`/`ResolvedArg`/`PendingConfirmationState` types, `insert_pending_confirmation`/`find_pending_confirmation`/`transition_state` accessors, 5 unit tests
- `crates/brokerd/src/lib.rs` - Registered `pub mod confirmation;`

## Decisions Made
- `pending_confirmations` schema and `resolved_args`-as-JSON-blob decisions match RESEARCH Open Question 2 and PATTERNS.md's mapped analog exactly — no deviation from the plan's specified shape.
- `workspace_root_path` persisted per Assumption A2 (RESEARCH Open Question 1), surfaced explicitly in the plan's `<verification>` section as a decision that only affects Plan 02/03 if revisited, not this plan's security substance.

## Deviations from Plan

None - plan executed exactly as written. All types, accessors, DDL columns, and test behaviors match the plan's `<action>`/`<acceptance_criteria>` blocks verbatim.

## Issues Encountered

One minor build error during Task 1: an SQL comment inside the `SCHEMA_DDL` Rust string literal contained a literal `"..."` quoted phrase, which terminated the outer Rust string early (Rust 2021 "unknown prefix" / unexpected-token errors). Fixed by rewording the comment to avoid embedded double quotes; re-verified with `cargo build -p brokerd` before proceeding. Not a plan deviation — a syntax-level self-correction within Task 1's own commit.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- The persisted-state layer (`pending_confirmations` table + `confirmation.rs` accessors + `event_hash_by_id`) is in place for Plan 02 (block-time wiring: populate `PendingConfirmation` including `workspace_root_path` and insert it atomically with the `sink_blocked` event) and Plan 03 (the `caprun confirm`/`deny` CLI dispatch and sink re-invocation).
- `cargo test -p brokerd audit`, `cargo test -p brokerd confirmation`, `cargo build --workspace`, `cargo test --workspace --no-fail-fast`, and `./scripts/check-invariants.sh` are all green.
- No blockers.

---
*Phase: 10-single-shot-confirmation-loop*
*Completed: 2026-07-07*
