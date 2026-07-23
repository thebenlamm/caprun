---
phase: 16-confirm-ux-literal-binding-negative-controls
plan: 01
subsystem: security
tags: [rust, sha256, sqlite, audit-dag, confirm-binding]

# Dependency graph
requires:
  - phase: 14-content-sensitive-sink-arg-blocking
    provides: plural SinkBlockedAnchor/BlockedArg (D-14 Collect-then-Block), PendingConfirmation/ResolvedArg persistence
  - phase: 15-value-lineage-and-derivation
    provides: Event whole-payload additive-field precedent (derived_value_id/input_value_ids/transform_kind)
provides:
  - "Event.combined_digest / Event.blocked_arg_names additive fields (hash-chained, byte-identical for non-block events)"
  - "The single shared combined_digest(&[(&str, &str)]) primitive in confirmation.rs (partition-binding + name-binding, byte-wise-ascending order, name-uniqueness asserted)"
  - "PendingConfirmation.combined_digest / .blocked_arg_names fields + widened insert/find SQL"
  - "pending_confirmations schema widened + idempotent open-time ALTER TABLE migration for pre-existing DBs"
  - "server.rs's Block-time write computing the digest ONCE over the FULL resolved_args set and threading it into both Event and PendingConfirmation"
affects: ["16-02 (verifier/narration, reuses combined_digest)", "16-03", "16-04"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Shared producer/verifier primitive: combined_digest() owns canonical order + uniqueness assertion so no caller can diverge on encoding"
    - "Idempotent schema migration gated on PRAGMA table_info presence check (never a blind ALTER TABLE)"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/event.rs
    - crates/brokerd/src/confirmation.rs
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/email_smtp_acceptance.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - cli/caprun/tests/confirm.rs

key-decisions:
  - "combined_digest binds sha256(arg_name) ‖ sha256(literal) per element (both fixed-width 64-hex), byte-wise-ascending arg_name order, over EVERY current resolved_args element (blocked AND trusted) — per the Round-6 DESIGN amendment, not the original blocked-subset-only scoping"
  - "blocked_arg_names is retained but is DISPLAY-MARKING metadata only; it does not gate the digest's domain"
  - "Migration gated on PRAGMA table_info(pending_confirmations) column presence, not a blind ALTER TABLE + error-catch"
  - "server.rs restructured to resolve the full resolved_args snapshot + compute the digest BEFORE constructing the sink_blocked Event, so one computation feeds both the Event and PendingConfirmation under the existing single locked write"

requirements-completed: [CONFIRM-03]

coverage:
  - id: D1
    description: "Shared combined_digest primitive: partition-binding (boundary-shift), name-binding (rename bypass), order-invariant, duplicate-name fail-closed"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#combined_digest_single_element_matches_expected_formula, combined_digest_partition_binding_boundary_shift_differs, combined_digest_name_binding_rename_differs, combined_digest_input_order_invariant, combined_digest_transposed_literals_differs, combined_digest_duplicate_arg_name_panics"
        status: pass
    human_judgment: false
  - id: D2
    description: "Event carries combined_digest/blocked_arg_names additively; non-sink_blocked events serialize byte-identically; sink_blocked events round-trip both fields"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/runtime-core/src/event.rs#anchors_empty_event_serializes_byte_identical_and_round_trips, sink_blocked_event_combined_digest_round_trips"
        status: pass
    human_judgment: false
  - id: D3
    description: "PendingConfirmation persists combined_digest/blocked_arg_names; insert/find round-trip both fields"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#insert_then_find_round_trips_all_fields"
        status: pass
    human_judgment: false
  - id: D4
    description: "pending_confirmations schema migration: a legacy 7-column DB is widened idempotently at open time; a widened INSERT then succeeds on first AND second open"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#pending_confirmations_migration_widens_legacy_schema_idempotently"
        status: pass
    human_judgment: false
  - id: D5
    description: "A genuine block with a TRUSTED arg and a TAINTED arg durably records ONE combined digest over the FULL resolved_args set (not the blocked subset), identical between the hash-chained Event and the mirrored PendingConfirmation, with blocked_arg_names as the ordered blocked-subset-only display marker"
    requirement: "CONFIRM-03"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/durable_anchor.rs#combined_digest_covers_full_set_and_matches_between_event_and_pending_confirmation"
        status: pass
    human_judgment: false
  - id: D6
    description: "All external Event::sink_blocked / PendingConfirmation call sites (including cfg(target_os = \"linux\") ones) updated; workspace green on macOS AND Linux; invariants pass"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "cargo test --workspace --no-fail-fast (macOS); docker run rust:1 cargo test --workspace --no-run (Linux compile gate, exit 0); scripts/mailpit-verify.sh (Linux full suite incl. SMTP-03/SMTP-05); ./scripts/check-invariants.sh"
        status: pass
    human_judgment: false

# Metrics
duration: 40min
completed: 2026-07-08
status: complete
---

# Phase 16 Plan 01: Combined-Digest Binding (Producer Side) Summary

**Adds a shared `combined_digest()` primitive (SHA-256 over `sha256(name)‖sha256(literal)` per element, byte-wise-ascending order, over the FULL resolved_args set) and wires server.rs's Block-time write to compute it once and persist it identically into the hash-chained `sink_blocked` Event and the mirrored `PendingConfirmation`, with an idempotent open-time schema migration for pre-existing DBs.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-08
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- `Event` gains `combined_digest: Option<String>` and `blocked_arg_names: Vec<String>` additive fields, mirroring the Phase 15 `derived_value_id` precedent — non-`sink_blocked` events remain byte-identical (golden-byte test extended, not broken); a `sink_blocked` Event round-trips both fields intact.
- The single shared `combined_digest(&[(&str, &str)]) -> String` primitive lives in `crates/brokerd/src/confirmation.rs` (module scope): asserts arg-name uniqueness (fail-closed panic on duplicates), sorts by byte-wise ascending `arg_name` internally (input order never matters), and hashes `sha256(name)‖sha256(literal)` per element into one outer SHA-256 — partition-binding (a `to`/`body` boundary shift changes the digest) and name-binding (renaming a bound arg changes the digest even with an identical literal).
- `PendingConfirmation` gains `combined_digest: String` and `blocked_arg_names: Vec<String>`, widening `insert_pending_confirmation`/`find_pending_confirmation` to a 9-column INSERT/SELECT.
- `pending_confirmations`'s `CREATE TABLE` DDL is widened for fresh DBs; a new `migrate_pending_confirmations_schema` (gated on `PRAGMA table_info` presence, called on every `open_audit_db`) idempotently `ALTER TABLE`s a pre-existing 7-column DB so `caprun confirm` reusing a run's original DB never hits "no such column."
- `server.rs`'s `SubmitPlanNode` Block arm was restructured: the full `resolved_args` snapshot is now resolved *before* constructing the `sink_blocked` Event, the digest is computed ONCE over the WHOLE set (blocked AND trusted args together — the BLOCKER-2 widening, not just the blocked subset), and both the digest and the ordered `blocked_arg_names` (derived from the `anchors` collection, display-marking metadata only) are threaded into both the Event and the `PendingConfirmation` under the existing single locked write.
- All external `Event::sink_blocked`/`PendingConfirmation` construction call sites updated across the workspace, including the two `#[cfg(target_os = "linux")]`-gated sites in `email_smtp_acceptance.rs` invisible to a macOS `cargo test`.
- A new integration test in `durable_anchor.rs` drives a genuine block (tainted `path` + trusted `contents`) through the real `dispatch_request` path and asserts: Event/PendingConfirmation carry an identical digest; the digest equals an independently-recomputed full-set digest; a blocked-subset-only recompute does NOT match (falsification proof of the BLOCKER-2 widening); `blocked_arg_names` is exactly `["path"]`.

## Task Commits

1. **Task 1: Whole-Event digest fields + the single shared combined_digest primitive** - `b8a8553` (feat)
2. **Task 2: Persist the FULL-set digest + names + open-time migration** - `28aa86f` (feat)

## Files Created/Modified

- `crates/runtime-core/src/event.rs` - `combined_digest`/`blocked_arg_names` additive fields on `Event`; `Event::sink_blocked` signature widened; golden-byte + round-trip tests
- `crates/brokerd/src/confirmation.rs` - shared `combined_digest()` primitive; `PendingConfirmation` schema widened; `insert_pending_confirmation`/`find_pending_confirmation` widened; seed-helper test fixtures updated to compute real digests
- `crates/brokerd/src/audit.rs` - `pending_confirmations` DDL widened; `migrate_pending_confirmations_schema` (idempotent, PRAGMA-gated); migration test
- `crates/brokerd/src/server.rs` - Block-time write restructured to compute the digest once over the full snapshot and thread it into both the Event and the PendingConfirmation
- `crates/brokerd/tests/durable_anchor.rs` - new digest-parity + full-set-vs-blocked-subset falsification test
- `crates/brokerd/tests/email_smtp_acceptance.rs` - `#[cfg(linux)]` seed helper updated (invisible to macOS `cargo test`, verified via Colima/Docker + Mailpit)
- `crates/brokerd/tests/s9_acceptance.rs` - in-process taint-consistency check's `Event::sink_blocked` call updated with placeholder digest args (not exercising CONFIRM-03 here)
- `cli/caprun/tests/confirm.rs` - both seed helpers (file.create, email.send) updated to compute and thread real digests

## Decisions Made

- Followed the Round-6 DESIGN amendment exactly: the digest covers every current `resolved_args` element (blocked AND trusted), binds `sha256(name)‖sha256(literal)` (not literal-only), in byte-wise-ascending `arg_name` order, with uniqueness asserted before hashing.
- Chose the `PRAGMA table_info` presence-check gating option for the migration (over the "duplicate column name" error-catch alternative the plan also permitted) — cleaner control flow, no reliance on parsing a specific SQLite error string.
- Restructured `server.rs`'s Block arm (hoisted the `resolved_args` resolution loop above the `Event::sink_blocked` construction) rather than computing the digest twice or threading a placeholder — this was necessary because the digest must be known before the Event is built, and the original code built the Event first.

## Deviations from Plan

None — plan executed exactly as written. Task 1 intentionally left the in-crate `server.rs`/seed-helper `Event::sink_blocked` calls with placeholder `(None, vec![])` args per the plan's own phased instruction ("Task 2 owns the exhaustive external sweep"); Task 2 replaced every one of them with the real computed values, per the plan's explicit call-site list (confirmed via two independent greps — the plan's list and my own).

## Issues Encountered

None. The full Linux verification (both the bare `--no-run` compile gate and the Mailpit-backed full test suite via `scripts/mailpit-verify.sh`) passed cleanly on the first attempt after the call-site sweep, confirming the `#[cfg(target_os = "linux")]`-gated `email_smtp_acceptance.rs` updates were correct.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The shared `combined_digest()` primitive, `Event.combined_digest`/`blocked_arg_names`, and `PendingConfirmation.combined_digest`/`blocked_arg_names` are all in place for Plan 16-02 to build the VERIFIER side: `confirm()`'s recompute-and-compare over the frozen `resolved_args` snapshot, and the per-arg Block narration (`render_block_display` rewrite) that marks each arg BLOCKED vs TRUSTED using `blocked_arg_names`.
- No blockers. The `pending_confirmations` schema and its migration are backward-compatible; a pre-Phase-16 DB opens and widens transparently.

---
*Phase: 16-confirm-ux-literal-binding-negative-controls*
*Completed: 2026-07-08*

## Self-Check: PASSED

All 8 modified/read files confirmed present on disk; both task commits (`b8a8553`, `28aa86f`) confirmed present in git history.
