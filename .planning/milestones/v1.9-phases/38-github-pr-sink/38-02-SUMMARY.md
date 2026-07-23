---
phase: 38-github-pr-sink
plan: 02
subsystem: api
tags: [github, capability-grant, cas, idempotency, sqlite, combined_digest, audit-dag]

# Dependency graph
requires:
  - phase: 28-security-hardening
    provides: "keyed HMAC audit chain (append_event/current_chain_head), migration presence-check idiom"
  - phase: 29-security-hardening
    provides: "sent_plan_nodes CAS + plan_node_idempotency_key (the INSERT-OR-IGNORE before-effect pattern mirrored here)"
  - phase: 16-confirm-binding
    provides: "combined_digest partition-blindness-resistant per-field digest primitive"
  - phase: 20-planner-seam
    provides: "ConnectionRole fail-closed default-deny capability precedent"
provides:
  - "session_grants table + record_github_grant + has_github_grant (GITHUB-02 session-scoped auth-grant capability)"
  - "created_prs CAS table + github_pr_content_key + reserve_created_pr (GITHUB-04 duplicate-PR at-most-once defense)"
  - "caprun grant <session_id> [audit-db-path] CLI verb"
affects: [38-04, 38-05, github-pr-dispatch, github-pr-confirm]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Session-scoped capability record keyed by session_id PRIMARY KEY (mirrors ConnectionRole fail-closed default-deny)"
    - "Content-derived CAS key via combined_digest over resolved literals (never effect_id, never a PlanNode field)"
    - "Distinct human CLI verb for a capability grant, separate from per-effect confirm/deny"

key-files:
  created:
    - cli/caprun/tests/grant.rs
  modified:
    - crates/brokerd/src/audit.rs
    - cli/caprun/src/main.rs

key-decisions:
  - "record_github_grant gates the github_grant_authorized event on the fresh INSERT (rows_affected==1) so a replayed grant is a genuine no-op — never a duplicate authorize event on the chain."
  - "github_pr_content_key reuses combined_digest (NOT plan_node_idempotency_key's plain-concat) because owner/repo/base/head/title/body are attacker-influenceable variable-length literals needing partition-blindness defense."
  - "run_grant hands the F1 custody helper a throwaway sibling workspace root (<audit_path>.grant-ws) — grant has no workspace file, but the existing broker key must still be read back so the grant event chains under the same key."

patterns-established:
  - "Pattern 1: NEW capability = own table keyed by scope-id + fail-closed existence check helper + own audit event; gate absent-row -> Deny."
  - "Pattern 2: replay defense = content-derived idempotency key committed via INSERT OR IGNORE before the effect; no clear-key-on-failure."

requirements-completed: [GITHUB-02, GITHUB-04]

coverage:
  - id: D1
    description: "session_grants capability: record_github_grant records a durable session-scoped grant + opaque github_grant_authorized event; has_github_grant is the fail-closed gate"
    requirement: GITHUB-02
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#github_grant_false_on_fresh_db"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#github_grant_recorded_makes_has_true_and_emits_opaque_event"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#github_grant_is_session_scoped"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#github_grant_record_is_idempotent"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#github_grant_migration_is_idempotent"
        status: pass
    human_judgment: false
  - id: D2
    description: "duplicate-PR CAS: github_pr_content_key is content-derived + partition-safe via combined_digest; reserve_created_pr is at-most-once per content"
    requirement: GITHUB-04
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#created_pr_content_key_is_deterministic"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#created_pr_content_key_changes_when_any_field_changes"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#created_pr_content_key_resists_partition_blindness"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#created_pr_reserve_fresh_then_suppressed"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#created_pr_migration_is_idempotent"
        status: pass
    human_judgment: false
  - id: D3
    description: "caprun grant CLI verb records a session-scoped grant and exits 0; malformed session id fails closed"
    requirement: GITHUB-02
    verification:
      - kind: integration
        ref: "cli/caprun/tests/grant.rs#grant_records_session_scoped_capability_and_exits_0"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/grant.rs#grant_with_malformed_session_id_exits_nonzero"
        status: pass
    human_judgment: false

# Metrics
duration: 6min
completed: 2026-07-18
status: complete
---

# Phase 38 Plan 02: Session auth-grant capability + duplicate-PR CAS Summary

**Session-scoped github.pr auth-grant (session_grants + has_github_grant gate) and a content-derived duplicate-PR CAS (created_prs + combined_digest-keyed reserve_created_pr), plus the distinct `caprun grant` human CLI verb — the two independent gates and the replay defense Plans 38-04/38-05 consume.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-07-18T09:05:43Z
- **Completed:** 2026-07-18T09:11:37Z
- **Tasks:** 3
- **Files modified:** 3 (2 modified, 1 created)

## Accomplishments
- GITHUB-02: `session_grants` table (session-scoped, `session_id` PRIMARY KEY) + `record_github_grant` (INSERT OR IGNORE + opaque `github_grant_authorized` event, replay-suppressed) + `has_github_grant` fail-closed existence gate — the single gate both dispatch paths will consult to Deny absent a live grant.
- GITHUB-04: `created_prs` CAS table + `github_pr_content_key` (built on `combined_digest` over the resolved owner/repo/base/head/title/body literals — content-derived, partition-safe, never effect_id/PlanNode-keyed) + `reserve_created_pr` (INSERT-OR-IGNORE before-effect, at-most-once, no clear-key-on-failure).
- `caprun grant <session_id> [audit-db-path]` verb wired as a distinct first-branch human action mirroring the confirm/deny wiring; malformed session id fails closed.
- Two new presence-check migrations (`migrate_session_grants_schema`, `migrate_created_prs_schema`, no backfill) wired into `open_audit_db`; both re-runnable/idempotent across a broker restart.

## Task Commits

Each task was committed atomically:

1. **Task 1 + Task 2: session auth-grant capability + duplicate-PR CAS (audit.rs)** - `2c6785b` (feat)
2. **Task 3: caprun grant CLI verb** - `e0f9704` (feat)

_Tasks 1 and 2 both live in `crates/brokerd/src/audit.rs` (single-sourced DDL + helpers so the table shapes, the check/record helpers, and the content-key derivation cannot drift) and were committed together as one cohesive brokerd change._

## Files Created/Modified
- `crates/brokerd/src/audit.rs` - Added `session_grants` + `created_prs` tables to `SCHEMA_DDL`; `migrate_session_grants_schema` + `migrate_created_prs_schema`; `record_github_grant`, `has_github_grant`, `github_pr_content_key`, `reserve_created_pr`; 10 unit tests.
- `cli/caprun/src/main.rs` - New `grant` first-branch verb; `run_grant` + `load_grant_key` helpers.
- `cli/caprun/tests/grant.rs` - Cross-process CLI integration tests for the grant verb.

## Decisions Made
- **Replay no-op on the grant event:** `record_github_grant` emits the `github_grant_authorized` event ONLY when the `INSERT OR IGNORE` freshly inserts (`rows_affected == 1`). A repeated grant is then a genuine no-op (capability stays single, chain does not grow a duplicate authorize event) — the cleaner mirror of the CAS "suppress on replay" discipline. The must_haves' "records the capability AND the event" holds on the first grant; idempotency holds on replay.
- **combined_digest for the PR content key (not plain concat):** unlike `plan_node_idempotency_key` (fixed-width UUIDs + schema-fixed names, where plain concat is provably safe), the six PR fields are attacker-influenceable variable-length literals, so the key is built on `combined_digest` — its per-field fixed-width inner-hash discipline is the partition-blindness defense (verified by `created_pr_content_key_resists_partition_blindness`).
- **Grant key-load via a throwaway sibling workspace root:** `caprun grant` has no workspace file to feed the F1 fail-closed custody check, so `load_grant_key` creates a throwaway `<audit_path>.grant-ws` sibling directory (never an ancestor of the audit DB, so F1 passes by construction), reads the existing broker key back through the SAME `key::load_or_create_key` helper confirm/deny use (so the grant event chains under the same key), then removes the dir.

## Deviations from Plan
None - plan executed exactly as written. No auto-fixes required; no architectural changes; no auth gates.

## Issues Encountered
- A SQL comment I added to `SCHEMA_DDL` contained a double-quoted `"github.pr"`, which terminated the Rust `"..."` string literal and broke the build. Fixed by using single quotes (`'github.pr'`) inside the SQL comment. Caught immediately by the first `cargo test` compile; resolved before any commit.

## User Setup Required
The session auth-grant is a runtime human action (`caprun grant <session>`); minimal-scope PAT provisioning (fine-grained token: Pull requests: write + Contents: read ONLY) is operator responsibility, surfaced by the verb's OPERATOR NOTICE and exercised live in Phase 40. No environment configuration is required to build or test this plan.

## Next Phase Readiness
- `has_github_grant(session_id)` is ready for Plans 38-04/38-05 to consult (Deny absent a live grant — a bare confirm cannot create a PR).
- `github_pr_content_key(...)` + `reserve_created_pr(...)` are ready as the single content-derived, before-effect CAS both dispatch sites call.
- The token itself is NOT handled here (Plan 38-03); no `CAPRUN_GITHUB_TOKEN` is read or persisted by this plan.

## Verification
- `cargo build --workspace` — clean.
- `cargo test -p brokerd` — 162 lib tests pass (incl. 10 new: 5 grant, 5 CAS); all integration/doc targets green, 0 failed.
- `cargo build -p caprun` + `cargo test -p caprun` — all targets green (grant.rs: 2/2; confirm.rs: 4/4; no regressions).
- `./scripts/check-invariants.sh` — exits 0 (no EffectRequest, no mint-token misuse, grant/CAS events opaque).

## Self-Check: PASSED
- All created/modified files present on disk.
- Both task commits (`2c6785b`, `e0f9704`) present in git history.

---
*Phase: 38-github-pr-sink*
*Completed: 2026-07-18*
