---
phase: 28-authenticated-audit-chain
plan: 04
subsystem: security
tags: [rust, hmac, hmac-sha256, audit-chain, mac, tamper-evidence, rusqlite, tail-truncation]

# Dependency graph
requires:
  - phase: 28-authenticated-audit-chain
    provides: "28-03: keyed HMAC-SHA256 compute_event_hash/append_event/verify_chain + shared mac_frame(mac, domain, fields) helper with the anchor domain tag reserved (b\"caprun.audit.anchor.v1\")"
provides:
  - "chain_anchor(session_id, head_event_id, head_hash, event_count, mac) STRICT table in SCHEMA_DDL — a single MAC'd row per session recording the current chain head and the actual persisted event_count"
  - "compute_anchor_mac/verify_anchor_mac — anchor-row MAC via the shared mac_frame helper, domain-separated from the event MAC (b\"caprun.audit.anchor.v1\" vs b\"caprun.audit.event.v1\")"
  - "append_event atomically upserts the chain_anchor row under the same already-held connection lock as the events INSERT — every one of the 19 production call sites inherits this for free, no new call site added anywhere"
  - "verify_chain cross-checks the chain_anchor row (MAC verify + head/count match against the recomputed walk) — tail-truncation via raw-SQL DELETE (bypassing append_event) now returns false; a legacy session with events but no anchor row fails closed"
affects: [28-05-authenticated-audit-chain]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "chain_anchor MAC reuses Plan 03's shared mac_frame(mac, domain, fields) helper with its own reserved domain tag b\"caprun.audit.anchor.v1\" (distinct from EVENT_MAC_DOMAIN) — closes cross-record-type MAC replay between events and anchors."
    - "The anchor MAC binds session_id + head_event_id + head_hash + event_count (decimal-string-encoded, length-framed by mac_frame) — binding event_count, not just the head hash, is what lets verify_chain detect a replayed-old-anchor-plus-truncated-chain pair, not merely a re-MAC'd head."
    - "event_count is read back via SELECT COUNT(*) FROM events WHERE session_id = ? AFTER the events INSERT, inside the same append_event call — never assumed via +1 arithmetic (28-RESEARCH.md Anti-Pattern)."
    - "Anchor upsert uses INSERT ... ON CONFLICT(session_id) DO UPDATE, folded directly into append_event's body under the connection lock every one of its 19 production callers already holds — mirrors quarantine.rs::mint_from_read's two-write same-lock atomicity discipline, never a second caller-invoked step."
    - "migrate_chain_anchor_schema mirrors migrate_pending_confirmations_schema's presence-check-before-acting idiom (sqlite_master-gated) but, since chain_anchor is a whole new table (not a widened one) already created idempotently by SCHEMA_DDL's own CREATE TABLE IF NOT EXISTS, it asserts presence and fails loudly rather than re-issuing a second copy of the same DDL statement — kept the acceptance criterion's exact-one-occurrence bar for the literal DDL string honest instead of duplicating it."

key-files:
  created: []
  modified:
    - crates/brokerd/src/audit.rs

key-decisions:
  - "migrate_chain_anchor_schema does NOT re-issue the CREATE TABLE statement: SCHEMA_DDL's own CREATE TABLE IF NOT EXISTS chain_anchor (run via execute_batch on every open_audit_db call, unconditionally) already handles idempotent creation on a legacy DB missing the table. Duplicating the same DDL string a second time inside the migration function would have satisfied the plan's literal action text but violated its own acceptance criterion (grep -c 'CREATE TABLE IF NOT EXISTS chain_anchor' == 1) — resolved by making the migration function a defensive sqlite_master presence ASSERTION (fails loudly if somehow still absent after SCHEMA_DDL ran) rather than a second creation path. This still satisfies the plan's must_haves truth (\"idempotent chain_anchor migration mirroring migrate_pending_confirmations_schema\") — it follows the exact presence-check-before-acting idiom — while keeping the DDL single-sourced."
  - "chain_anchor's fail-closed 'untrusted until re-anchored' behavior for legacy/absent-anchor sessions lives entirely in verify_chain (a runtime check), not in the schema migration function — matching 28-PATTERNS.md's explicit guidance that this is 'a separate runtime check... not a schema migration per se.'"
  - "event_count and head fields are bound into the anchor MAC via mac_frame's length-framed encoding of event_count.to_string().as_bytes() — no fixed-width integer encoding needed, since mac_frame already makes every field boundary unambiguous."

requirements-completed: [HARDEN-02]

coverage:
  - id: D1
    description: "chain_anchor(session_id, head_event_id, head_hash, event_count, mac) STRICT table added to SCHEMA_DDL, MAC'd via the shared mac_frame helper under domain tag b\"caprun.audit.anchor.v1\" (distinct from the event domain)"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "grep -c 'CREATE TABLE IF NOT EXISTS chain_anchor' crates/brokerd/src/audit.rs == 1; grep -c 'ON CONFLICT(session_id)' == 1"
        status: pass
    human_judgment: false
  - id: D2
    description: "Anchor upsert is folded INSIDE append_event under the same already-held connection lock as the events INSERT, over the actual read-back event_count (COUNT(*), never +1 arithmetic) — no new call site added at any of the 19 production append_event sites"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "grep -n 'event_count' crates/brokerd/src/audit.rs shows a COUNT(*) read-back, not an assumed increment; grep -n 'append_event' crates/brokerd/src/server.rs unchanged (7 lines, same as Plan 03) — no new call site"
        status: pass
    human_judgment: false
  - id: D3
    description: "verify_chain cross-checks the chain_anchor row: absent -> false (legacy fail-closed), MAC-invalid -> false, head/count mismatch -> false; tail-truncation (raw-SQL DELETE bypassing append_event) is now detected"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::tail_truncation_detected_via_anchor_mismatch"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::legacy_db_without_anchor_fails_closed"
        status: pass
    human_judgment: false
  - id: D4
    description: "An untampered, normally-appended anchored chain still verifies true — no false-positive regression from the new anchor cross-check"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "sanity assertions inside tail_truncation_detected_via_anchor_mismatch and legacy_db_without_anchor_fails_closed both assert verify_chain(...) == true on the freshly-appended, untampered chain before tampering; the pre-existing self_consistent_forgery_without_key_is_rejected and verify_chain_is_key_dependent tests (Plan 03) also still assert true-on-genuine-key post-anchor-cross-check"
        status: pass
    human_judgment: false
  - id: D5
    description: "cargo build --workspace clean, cargo test -p brokerd all green (99/99, +2 from this plan), cargo test --workspace --no-fail-fast 0 failed, ./scripts/check-invariants.sh 4/4 PASS"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "cargo build --workspace (exit 0); cargo test -p brokerd (99 passed / 0 failed, exit 0); cargo test --workspace --no-fail-fast (48 test-result lines, all 'ok', 0 FAILED, exit 0); ./scripts/check-invariants.sh (4/4 gates PASS, exit 0)"
        status: pass
    human_judgment: false

duration: 24min
completed: 2026-07-13
status: complete
---

# Phase 28 Plan 04: MAC'd Chain-Anchor Monotonic Head + Tail-Truncation Detection Summary

**Adds a MAC'd `chain_anchor(session_id, head_event_id, head_hash, event_count)` table, upserted atomically inside `append_event`, and extends `verify_chain` to cross-check it — turning tail-truncation (raw-SQL DELETE of the last N events) from a previously-invisible attack into a detected one, and failing closed on legacy pre-Phase-28 databases with no anchor row.**

## Performance

- **Duration:** 24 min
- **Started:** 2026-07-13T01:17Z (approx, from session start)
- **Completed:** 2026-07-13T01:41Z
- **Tasks:** 2
- **Files modified:** 1 (0 created)

## Accomplishments
- `audit.rs`: added `chain_anchor` STRICT table to `SCHEMA_DDL` (single row per session: `head_event_id`, `head_hash`, `event_count`, `mac`), `ANCHOR_MAC_DOMAIN = b"caprun.audit.anchor.v1"` (distinct from `EVENT_MAC_DOMAIN`, reserved by Plan 03), and `migrate_chain_anchor_schema` — a `sqlite_master`-gated presence assertion mirroring `migrate_pending_confirmations_schema`'s idiom, called from `open_audit_db` alongside the existing migration.
- `compute_anchor_mac`/`verify_anchor_mac` — build the anchor row's MAC via the shared `mac_frame(mac, domain, fields)` helper over `session_id`, `head_event_id`, `head_hash`, `event_count` (decimal-string-encoded); `verify_anchor_mac` uses `Mac::verify_slice` (constant-time), never a `==`/`!=` compare.
- `append_event` now atomically upserts the `chain_anchor` row (`INSERT ... ON CONFLICT(session_id) DO UPDATE`) immediately after the events INSERT, under the SAME already-held `conn` lock — `event_count` is read back via `SELECT COUNT(*) FROM events WHERE session_id = ?` (never assumed via `+1`). Every one of the 19 production `append_event` call sites inherits this for free; no second call site was added anywhere.
- `verify_chain`'s contract now, after the existing per-row keyed-MAC walk: (1) loads the session's `chain_anchor` row — absent → `false` (fail-closed, untrusted-until-re-anchored); (2) verifies the anchor row's own MAC (constant-time) — mismatch → `false`; (3) asserts the walk's final `(id, hash)` and total row count equal the anchor's `head_event_id`/`head_hash`/`event_count` — mismatch → `false`. A genuinely untampered chain still verifies `true` (no false positive), since `append_event` keeps the anchor in sync with every append.
- Two new tests in `audit.rs`'s `#[cfg(test)]` module: `tail_truncation_detected_via_anchor_mismatch` (append 2 events, raw-SQL `DELETE` the tail bypassing `append_event`, assert `verify_chain` now returns `false`, plus a sanity assertion that the pre-tamper chain verified `true`) and `legacy_db_without_anchor_fails_closed` (append normally, raw-SQL `DELETE FROM chain_anchor` to simulate a legacy/un-anchored session, assert `false`).

## Task Commits

Each task was committed atomically:

1. **Task 1: chain_anchor table + idempotent migration + atomic upsert folded into append_event** - `d7afbd6` (feat)
2. **Task 2: verify_chain anchor cross-check + fail-closed on absent anchor; tail-truncation + legacy-DB tests** - `47b98ed` (feat)

## Files Created/Modified
- `crates/brokerd/src/audit.rs` - `chain_anchor` table + `ANCHOR_MAC_DOMAIN`, `migrate_chain_anchor_schema`, `compute_anchor_mac`/`verify_anchor_mac`, atomic anchor upsert inside `append_event`, anchor cross-check inside `verify_chain`, 2 new tests

## Decisions Made
See `key-decisions` in frontmatter — most notably: `migrate_chain_anchor_schema` does NOT re-issue the `CREATE TABLE` DDL a second time (SCHEMA_DDL's own `CREATE TABLE IF NOT EXISTS` already handles idempotent creation); it instead asserts the table's presence and fails loudly if somehow absent, keeping the DDL single-sourced while still satisfying the plan's "idempotent migration mirroring `migrate_pending_confirmations_schema`" truth and its own literal-occurrence-count acceptance criterion.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] migrate_chain_anchor_schema re-creating chain_anchor would have violated the plan's own acceptance criterion**
- **Found during:** Task 1, immediately after first implementation pass
- **Issue:** The plan's action text says to add a migration function that "mirrors `migrate_pending_confirmations_schema`'s idiom... and call it from `open_audit_db`." A literal reading — re-issuing the `CREATE TABLE chain_anchor` DDL inside that function as a fallback — produced THREE occurrences of the literal string `CREATE TABLE IF NOT EXISTS chain_anchor` in the file (SCHEMA_DDL, the migration function's fallback DDL, and a doc-comment mention), directly violating the plan's own acceptance criterion (`grep -c 'CREATE TABLE IF NOT EXISTS chain_anchor' == 1`).
- **Fix:** Rewrote `migrate_chain_anchor_schema` to be a presence ASSERTION (query `sqlite_master`, return an error if the table is somehow still absent after `SCHEMA_DDL` ran) rather than a second creation path, and reworded the doc comment to avoid repeating the literal DDL phrase. This keeps the DDL single-sourced in `SCHEMA_DDL` (already idempotent via `CREATE TABLE IF NOT EXISTS`) while the migration function still performs a real, `migrate_pending_confirmations_schema`-idiom presence check.
- **Files modified:** `crates/brokerd/src/audit.rs`
- **Verification:** `grep -c 'CREATE TABLE IF NOT EXISTS chain_anchor' crates/brokerd/src/audit.rs` == 1; `cargo build --workspace` clean; `cargo test -p brokerd` green.
- **Committed in:** `d7afbd6` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — a self-caught correctness/acceptance-criterion conflict, fixed before commit, no scope change).
**Impact on plan:** None — both tasks' must_haves truths, artifacts, and acceptance criteria are fully satisfied; the fix only changed HOW the migration function's idempotency is implemented, not what it guarantees.

## Issues Encountered
None beyond the deviation documented above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness
- Plan 05 (`pending_confirmations` MAC fold + `deny()`'s new integrity gate) can build directly on this plan's `mac_frame`-based anchor pattern, reusing its own reserved domain tag `b"caprun.audit.pending-confirmation.v1"` (per Plan 03's doc comments) and the now fully anchor-cross-checked `verify_chain`.
- `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (0 failed), and `./scripts/check-invariants.sh` (4/4 gates) all green at hand-off. No blockers.
- Linux gate (`bash scripts/mailpit-verify.sh`) not run in this environment (macOS dev box, per CLAUDE.md) — this plan touches only `crates/brokerd/src/audit.rs` (no test-fixture directory layout, no SMTP/live-acceptance surface), so no Linux-gated file needed fixing; the phase's overall Linux verification remains scoped to a later plan/Phase 30 per this phase's structure.

---
*Phase: 28-authenticated-audit-chain*
*Completed: 2026-07-13*
