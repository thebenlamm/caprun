---
phase: 29-sink-path-hardening-replay-cas-contents-slot
plan: 01
subsystem: security
tags: [sqlite, rusqlite, sha2, audit-chain, cas, idempotency, tdd]

requires:
  - phase: 28-authenticated-audit-chain
    provides: "migrate_chain_anchor_schema's whole-new-table migration idiom; audit.rs's Sha256/hex usage conventions"
provides:
  - "sent_plan_nodes(idempotency_key TEXT PRIMARY KEY, effect_id TEXT NOT NULL, session_id TEXT NOT NULL, sent_at TEXT NOT NULL) STRICT table in SCHEMA_DDL"
  - "migrate_sent_plan_nodes_schema fn wired into open_audit_db (idempotent presence-check, no backfill)"
  - "plan_node_idempotency_key(sink: &SinkId, args: &[PlanArg]) -> String — pub(crate), content-derived, order-invariant"
affects: [29-sink-path-hardening-replay-cas-contents-slot/29-02, 29-sink-path-hardening-replay-cas-contents-slot/29-03]

tech-stack:
  added: []
  patterns:
    - "Whole-new-table idempotent migration: sqlite_master presence-check, no backfill for legacy DBs (mirrors migrate_chain_anchor_schema)"
    - "Content-derived CAS key: SHA256(sink.0 || sorted (arg_name, value_id) pairs), never effect_id-keyed"
    - "RED-stub TDD for a pure Rust fn: compile a constant-returning stub first, prove 2/4 property tests fail against it, then implement real logic"

key-files:
  created: []
  modified:
    - crates/brokerd/src/audit.rs

key-decisions:
  - "plan_node_idempotency_key uses direct-concatenation hashing (sink.0 + name + value_id bytes fed into one hasher), not combined_digest's fixed-width-inner-hash-per-field discipline — justified because ValueId is always a fixed-width 36-char UUID string and arg names are schema-fixed, so there is no attacker-controlled variable-length partition-blindness collision risk (29-RESEARCH.md Assumption A2)"
  - "sent_plan_nodes migration deliberately does NOT backfill legacy DBs — a pre-Phase-29 database's past Allowed-dispatch sends were never CAS-protected, mirroring chain_anchor's own no-backfill precedent"
  - "RED phase used a constant-returning stub (not todo!()) so the test suite compiles and 2 of 4 property tests (sink-scoping, value-distinguishing) genuinely fail before the real implementation lands — order-invariance and determinism trivially hold against any constant function and are not meaningful RED signals for this property set"

patterns-established:
  - "Groundwork-then-core split for HARDEN-03: this plan lands the CAS table + key derivation in isolation (macOS-runnable, unit-tested); plan 29-02 wires the dispatch-site transaction that consumes both"

requirements-completed: [HARDEN-03]

coverage:
  - id: D1
    description: "sent_plan_nodes CAS table exists in SCHEMA_DDL with the DESIGN-pinned shape and migrates idempotently on repeated open_audit_db calls"
    requirement: "HARDEN-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::sent_plan_nodes_migration_is_idempotent"
        status: pass
    human_judgment: false
  - id: D2
    description: "plan_node_idempotency_key is order-invariant, sink-scoped, value-distinguishing, deterministic, and content-derived (never effect_id-keyed)"
    requirement: "HARDEN-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::idempotency_key_is_order_invariant"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::idempotency_key_is_sink_scoped"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::idempotency_key_distinguishes_value_id"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::idempotency_key_is_deterministic"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-17
status: complete
---

# Phase 29 Plan 01: Replay CAS Groundwork Summary

**Content-derived `plan_node_idempotency_key` (SHA256 of sink + sorted arg-name/value_id pairs) and a new `sent_plan_nodes` CAS table with idempotent migration, both landed in `audit.rs` in isolation ahead of the dispatch-site wiring in plan 29-02.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-07-17T16:54:00Z
- **Completed:** 2026-07-17T17:09:00Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added `sent_plan_nodes(idempotency_key TEXT PRIMARY KEY, effect_id, session_id, sent_at)` STRICT table to `SCHEMA_DDL`, adjacent to `chain_anchor`
- Added `migrate_sent_plan_nodes_schema` mirroring `migrate_chain_anchor_schema`'s whole-new-table presence-check idiom (fails loudly if missing after DDL ran, no backfill for legacy DBs), wired into `open_audit_db` immediately after `migrate_chain_anchor_schema`
- Added `plan_node_idempotency_key(sink: &SinkId, args: &[PlanArg]) -> String` — `pub(crate)`, computes `SHA256(sink.0 || sorted (arg_name, value_id) pairs)`, sorted by arg name for order-invariance
- Followed full TDD RED/GREEN cycle for the idempotency-key fn: RED-stub (constant string) proven to fail sink-scoping and value-distinguishing tests, then real implementation made all 4 property tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add sent_plan_nodes table + idempotent migration (HARDEN-03)** - `cf9fd96` (feat)
2. **Task 2: Add content-derived plan_node_idempotency_key derivation (HARDEN-03)** - RED: `bebf4c5` (test), GREEN: `b00c782` (feat)

## Files Created/Modified
- `crates/brokerd/src/audit.rs` - Added `sent_plan_nodes` DDL, `migrate_sent_plan_nodes_schema`, `plan_node_idempotency_key`, and their unit tests (5 new tests total)

## Decisions Made
- Direct-concatenation hash shape (not `combined_digest`'s fixed-width-inner-hash-per-field) for `plan_node_idempotency_key` — pinned in the plan's task action text and re-justified in the fn's doc comment (29-RESEARCH.md Assumption A2: `ValueId` is fixed-width, arg names are schema-fixed, so no partition-blindness risk).
- Used a constant-returning RED stub (rather than `todo!()`) so the full test suite compiles and exercises real failure signal (2 of 4 tests fail against the stub) before the GREEN implementation — `todo!()` would panic on every test rather than producing meaningful pass/fail differentiation.

## Deviations from Plan

None — plan executed exactly as written. Both tasks' acceptance criteria were met verbatim: table shape, migration wiring order, key-derivation formula, and all four D-08-scoped unit test names match the plan's task text.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `plan_node_idempotency_key` and `sent_plan_nodes` are ready for plan 29-02 to consume at the `server.rs` Allowed `email.send` dispatch site (wrap in a `rusqlite::Transaction`, `INSERT` before `append_event`, commit before SMTP).
- `cargo build --workspace` compiles clean; `cargo test -p brokerd` — 107 unit tests + all integration test files pass (no regressions).
- `./scripts/check-invariants.sh` — all 4 gates pass (no `EffectRequest` token introduced, runtime-core purity intact, mint-call-site restriction intact, `test-fixtures` not a default feature).
- No Linux-only code added this plan (per plan's own note) — the Linux behavioral proof (real CAS-blocked replay via Mailpit) lives in plan 29-02/30.
- `plan_node_idempotency_key` currently shows a `dead_code` warning on `cargo build` (expected — it is `pub(crate)` and only consumed by its own `#[cfg(test)]` tests until plan 29-02 wires the dispatch-site call; no lint-deny gate exists in this workspace, so this does not fail any check).

---
*Phase: 29-sink-path-hardening-replay-cas-contents-slot*
*Completed: 2026-07-17*

## Self-Check: PASSED

- FOUND: crates/brokerd/src/audit.rs
- FOUND: .planning/phases/29-sink-path-hardening-replay-cas-contents-slot/29-01-SUMMARY.md
- FOUND: cf9fd96 (feat: sent_plan_nodes table + migration)
- FOUND: bebf4c5 (test: RED — plan_node_idempotency_key)
- FOUND: b00c782 (feat: GREEN — plan_node_idempotency_key)
- FOUND: 02fe671 (docs: plan summary)
