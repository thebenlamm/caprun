---
phase: 28-authenticated-audit-chain
plan: 05
subsystem: security
tags: [rust, hmac, hmac-sha256, pending-confirmations, mac, tamper-evidence, rusqlite, x-02]

# Dependency graph
requires:
  - phase: 28-authenticated-audit-chain
    provides: "28-03: keyed HMAC-SHA256 compute_event_hash/append_event/verify_chain + shared mac_frame(mac, domain, fields) helper with the pending_confirmation domain tag reserved (b\"caprun.audit.pending-confirmation.v1\") and minimal run_confirm_or_deny key-load wiring for both confirm/deny verbs"
  - phase: 28-authenticated-audit-chain
    provides: "28-04: chain_anchor MAC'd monotonic-head table + verify_chain tail-truncation cross-check"
provides:
  - "pending_confirmations folded into the broker-key MAC scheme: a new `mac TEXT NOT NULL DEFAULT ''` column, idempotent migration, whole-row MAC computed by insert_pending_confirmation and atomically recomputed with state by transition_state"
  - "verify_pending_confirmation_mac(key, &pc) -> bool — constant-time (Mac::verify_slice), fail-closed whole-row MAC verification, called by confirm()/deny() immediately after find_pending_confirmation, BEFORE the terminal-state branch reads pc.state"
  - "deny() gains the SAME integrity gates confirm() has: the pending_confirmations MAC-verify gate AND the verify_chain gate — deny() previously had NEITHER"
  - "run_confirm_or_deny's existing (Plan 03) key-load wiring for both verbs now feeds a fully-wired MAC/chain-verify gate on both paths"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pending_confirmations whole-row MAC reuses audit::mac_frame (made pub(crate)) with its own reserved domain tag b\"caprun.audit.pending-confirmation.v1\", distinct from EVENT_MAC_DOMAIN/ANCHOR_MAC_DOMAIN — build_pending_confirmation_mac constructs the keyed Hmac<Sha256> over an explicit `state` parameter (not necessarily pc.state) so transition_state can compute the MAC for the NEW state without a temporary struct copy."
    - "transition_state's signature changed from (conn, effect_id, new_state) to (conn, key, pc: &PendingConfirmation, new_state) — the caller's already-fetched pc supplies the other whole-row fields the new MAC must bind; the UPDATE sets state and mac in ONE statement, never two."
    - "MAC verify placement: immediately after find_pending_confirmation returns Some(pc), BEFORE the terminal-state (pc.state != Pending) branch — closes the Pitfall-5 window where a flip-back-to-Pending tamper would otherwise pass Step 2's plain guard undetected until a later gate."

key-files:
  created: []
  modified:
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/confirmation.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/main.rs
    - cli/caprun/tests/confirm.rs
    - crates/brokerd/tests/email_smtp_acceptance.rs

key-decisions:
  - "MAC scope: the WHOLE row (effect_id, session_id, blocked_event_id, sink, resolved_args, blocked_arg_names, combined_digest, workspace_root_path, state) — not only state+combined_digest as the DESIGN doc's literal text names (28-RESEARCH.md Assumption A3 / Open Question 2). resolved_args/blocked_arg_names/workspace_root_path are equally forgeable by a bare pending_confirmations-table writer and equally load-bearing for confirm()'s Step 4.5b recompute-and-compare — narrowing to just state+combined_digest would have left a real gap. This is a strict superset of the DESIGN doc's literal text, consistent with its framing as \"fold into the scheme.\""
  - "verify_pending_confirmation_mac(key, &pc) added a `pub mac: String` field to the PendingConfirmation struct (rather than returning a (PendingConfirmation, String) tuple from find_pending_confirmation) — matches the plan's literal must_haves signature and lets confirm()/deny() call the verify function directly on the row they already have in scope, no restructuring of every call site's destructuring pattern."
  - "append_confirm_digest_mismatch_event renamed to append_digest_mismatch_event with a new `verb: &str` param (\"confirm\" or \"deny\") — both verbs now share ONE integrity-alarm helper/event_type (confirm_digest_mismatch), with `verb` only affecting the event's `actor` field so the audit trail records which decision verb tripped the alarm. Avoided duplicating the helper for deny()."
  - "deny()'s new verify_chain gate (Step 2.5) runs AFTER the display (Step: render_block_display) and terminal-state check, mirroring confirm()'s Step 4.5a's relative position — the pending_confirmations MAC gate (Step 1.5) is the one that must run BEFORE terminal-state (Pitfall 5); verify_chain has no such 'before Step 2' constraint since it doesn't read pc.state at all."
  - "review() left deliberately unauthenticated (MAJOR-8, plan's explicit scope) — a one-line doc comment now states this explicitly rather than leaving it silently ambiguous."
  - "cli/caprun/src/main.rs needed ZERO code changes this plan — Plan 03's deviation already wired the F1-checked key-load for BOTH confirm and deny via pc.workspace_root_path (documented in 28-03-SUMMARY.md's 'Next Phase Readiness'). Only the stale mod-level doc comment (which forward-referenced Plan 05's then-still-open scope) was corrected in-PR."

requirements-completed: [HARDEN-02]

coverage:
  - id: D1
    description: "pending_confirmations gains a mac TEXT NOT NULL DEFAULT '' column (idempotent PRAGMA table_info-gated migration mirroring migrate_pending_confirmations_schema), computed by insert_pending_confirmation and atomically recomputed with state by transition_state's single UPDATE"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::insert_then_find_round_trips_all_fields (asserts verify_pending_confirmation_mac true post-insert)"
        status: pass
    human_judgment: false
  - id: D2
    description: "verify_pending_confirmation_mac is constant-time (Mac::verify_slice, never a plain string ==/!= compare) and fails closed on a hex-decode error or MAC mismatch"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "grep -vE '^\\s*//' crates/brokerd/src/confirmation.rs | grep -c 'pc.mac ==|mac == |!= pc.mac' == 0; fn body uses mac.verify_slice"
        status: pass
    human_judgment: false
  - id: D3
    description: "A raw-SQL flip-back of a terminal row's state back to 'pending' (mac left unrecomputed) is caught by the whole-row MAC check — distinct from Step 2's plain != Pending guard, which would NOT catch a flip-back TO Pending"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::flip_back_denied_to_pending_caught_by_mac"
        status: pass
    human_judgment: false
  - id: D4
    description: "deny() gains the SAME pending_confirmations MAC gate AND verify_chain gate confirm() has — deny() previously had NEITHER integrity check; both fire BEFORE any state mutation"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirmation::tests::deny_fails_closed_on_tampered_state"
        status: pass
    human_judgment: false
  - id: D5
    description: "run_confirm_or_deny loads the F1-checked key for both confirm and deny (already wired by Plan 03); an untampered cross-process confirm/deny still verifies true — no false positives"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "cargo build --workspace (exit 0); cargo test --workspace --no-fail-fast (283 passed / 0 failed, exit 0); ./scripts/check-invariants.sh (exit 0, 4/4 PASS)"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs (4/4 passed, macOS-run cross-process suite; the A2 positive control for cross-process key custody under the now-MAC'd pending_confirmations scheme — full Linux gate deferred to bash scripts/mailpit-verify.sh, this phase's Linux verification plan)"
        status: pass
  - id: D6
    description: "review() remains deliberately unauthenticated (MAJOR-8 pre-decision, non-authoritative surface) — no gate added there, documented explicitly"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#review (doc comment states the deliberate exclusion; no verify_pending_confirmation_mac/verify_chain call in its body)"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-07-13
status: complete
---

# Phase 28 Plan 05: pending_confirmations Whole-Row MAC + confirm()/deny() Entry Gates Summary

**Folds `pending_confirmations` into the same broker-key HMAC-SHA256 MAC scheme as the events chain (whole-row, domain-separated), and gives `deny()` the SAME fail-closed integrity gates `confirm()` already had — closing the flip-back/delete gap on the one table that survives a confirm/deny process restart, per DESIGN's X-02 uniform ruling.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-12T21:42Z (approx, immediately following Plan 04's completion)
- **Completed:** 2026-07-13T02:06Z (commit timestamps)
- **Tasks:** 3
- **Files modified:** 6 (0 created)

## Accomplishments
- `audit.rs`: added a `mac TEXT NOT NULL DEFAULT ''` column to `pending_confirmations`'s DDL, extended `migrate_pending_confirmations_schema` with a `PRAGMA table_info`-gated idempotent migration for the new column, and made `mac_frame` `pub(crate)` so `confirmation.rs` can reuse it.
- `confirmation.rs`: added `pub mac: String` to the `PendingConfirmation` struct; `PENDING_CONFIRMATION_MAC_DOMAIN = b"caprun.audit.pending-confirmation.v1"`; `build_pending_confirmation_mac`/`compute_pending_confirmation_mac_for_state`/`verify_pending_confirmation_mac` (constant-time, `Mac::verify_slice`, fail-closed on hex-decode error or mismatch) — MACs the WHOLE row (see Decisions).
- `insert_pending_confirmation` now takes `key: &[u8]` and computes+stores the initial MAC; `find_pending_confirmation` reads the `mac` column back into the struct; `transition_state` now takes `key: &[u8]` and `pc: &PendingConfirmation`, recomputing and rewriting `mac` in the SAME `UPDATE` statement as `state`.
- `confirm()` and `deny()` both gained a Step 1.5 gate: `verify_pending_confirmation_mac(key, &pc)` immediately after `find_pending_confirmation` returns, BEFORE the terminal-state branch reads `pc.state` — closing Pitfall 5's window.
- `deny()` additionally gained a Step 2.5 gate: `audit::verify_chain(conn, ...)` — the SAME chain-verify gate `confirm()`'s Step 4.5a has. `deny()` previously had NO integrity check of any kind.
- `append_confirm_digest_mismatch_event` renamed to `append_digest_mismatch_event` with a new `verb: &str` param, shared by both `confirm()`'s and `deny()`'s mismatch paths.
- `ConfirmOutcome::DigestMismatch`'s doc comment corrected (was stale re: "pending_confirmations has no MAC of its own yet" — now accurate). `review()` gained a one-line doc comment explicitly stating it is deliberately unauthenticated (MAJOR-8).
- `main.rs`'s `mod key;` doc comment updated — it previously forward-referenced Plan 05 as still-open scope; corrected to reflect completion.
- Task 3: two new tests — `flip_back_denied_to_pending_caught_by_mac` (real `deny()`, then a raw-SQL flip-back to `state='pending'` without recomputing `mac`, proving the MAC catches what Step 2's plain guard would not) and `deny_fails_closed_on_tampered_state` (proving `deny()`'s brand-new gate rejects a MAC-invalid row, with no state transition and no `confirm_denied` event).
- Test-fixture compile fixes: `cli/caprun/tests/confirm.rs` (2 sites) and `crates/brokerd/tests/email_smtp_acceptance.rs` (1 site) threaded `key`/`TEST_KEY` through the now-4-arg `insert_pending_confirmation` and added the `mac` struct field.

## Task Commits

Each task was committed atomically (Tasks 1 and 2 landed together — see Deviations):

1. **Task 1+2: pending_confirmations whole-row MAC fold + confirm()/deny() entry gates** - `473d306` (feat)
2. **Task 3: flip-back-to-Pending caught by MAC; deny() rejects tampered state** - `dfd2743` (test)

## Files Created/Modified
- `crates/brokerd/src/audit.rs` - `mac` column + idempotent migration, `mac_frame` visibility bump to `pub(crate)`
- `crates/brokerd/src/confirmation.rs` - `mac` struct field, MAC compute/verify helpers, `insert_pending_confirmation`/`find_pending_confirmation`/`transition_state` signature changes, `confirm()`/`deny()` gate additions, `append_digest_mismatch_event` rename, doc corrections, 2 new tests
- `crates/brokerd/src/server.rs` - `insert_pending_confirmation` call site threads `key`; `PendingConfirmation` literal gets a placeholder `mac` field
- `cli/caprun/src/main.rs` - `mod key;` doc comment correction only (no functional change — Plan 03 already wired the key-load)
- `cli/caprun/tests/confirm.rs` - `insert_pending_confirmation` call sites thread `key`; `mac` placeholder field
- `crates/brokerd/tests/email_smtp_acceptance.rs` - same

## Decisions Made
See `key-decisions` in frontmatter — most notably: the MAC covers the WHOLE `pending_confirmations` row (not just `state`+`combined_digest`), `PendingConfirmation` gained a `pub mac: String` field (rather than a tuple return from `find_pending_confirmation`), `transition_state`'s signature now takes the already-fetched `pc` so the new MAC can bind its unchanged fields, and `cli/caprun/src/main.rs` required zero functional changes (only a stale doc-comment fix) since Plan 03 already completed the key-load wiring for both verbs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `insert_then_find_round_trips_all_fields` test needed rewriting, not just a `mac` field addition**
- **Found during:** Task 1, immediately after adding the `mac` field to `PendingConfirmation`
- **Issue:** The existing test asserted `assert_eq!(found, pc)` — a whole-struct equality check. Once `mac` became a real, MAC-computed field, `found.mac` (the real persisted MAC) would never equal `pc.mac` (a caller-side placeholder set before insert), making this assertion fail for the RIGHT reason (the plan's own change) but via the WRONG mechanism (accidental placeholder mismatch, not a meaningful test of round-trip correctness).
- **Fix:** Rewrote the test to assert each meaningful field individually, plus a positive assertion that `verify_pending_confirmation_mac(TEST_KEY, &found)` is true — a stronger, more accurate test of "round-trips all fields" than blind struct equality would have been even before this change.
- **Files modified:** `crates/brokerd/src/confirmation.rs`
- **Verification:** `cargo test -p brokerd -- insert_then_find_round_trips_all_fields` passes.
- **Committed in:** `473d306` (Task 1+2 commit)

**2. [Rule 3 - Blocking] Combined Task 1 and Task 2 into a single commit**
- **Found during:** Attempting to split Task 1 (storage-layer MAC fold) from Task 2 (confirm()/deny() gate wiring) into separate atomic commits
- **Issue:** Task 1's `transition_state` signature change (`effect_id: &str` → `key: &[u8], pc: &PendingConfirmation`) makes `confirm()`'s and `deny()`'s EXISTING `transition_state` call sites fail to compile without a matching argument-list fix. That minimal compile-fix is structurally inseparable from Task 2's own gate-addition edits to the SAME functions (`confirm()`/`deny()`) in a way that survives Rust's whole-crate compilation requirement for a clean, buildable intermediate commit.
- **Fix:** Verified (via `git stash --keep-index` isolating the Task 3 test-only hunk) that the combined Task 1+2 changeset builds clean and passes the full 99-test `brokerd` baseline (matching Plan 04's ending count exactly) BEFORE committing — i.e., the commit is still a well-formed, independently-verifiable checkpoint, just covering two of the plan's three tasks. Task 3's two new tests were still committed separately and cleanly (verified via `git add -p` hunk isolation).
- **Files modified:** n/a (process decision, not a code fix)
- **Verification:** `cargo build --workspace` + `cargo test --workspace --no-fail-fast` both green on the Task-1+2-only tree before commit.
- **Committed in:** `473d306`

---

**Total deviations:** 2 (1 Rule-1 test-correctness fix, 1 Rule-3 process/commit-granularity note). Neither affects the plan's must_haves, artifacts, or acceptance criteria — all are satisfied.
**Impact on plan:** None on scope or security properties. The commit-granularity note (#2) reflects a genuine Rust-compilation constraint, not a shortcut; both commits are independently buildable, testable checkpoints.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness
- `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (283 passed / 0 failed across the whole workspace; `brokerd`'s own lib-test count is 101 — the 99-test Plan 04 baseline plus this plan's 2 new tests), and `./scripts/check-invariants.sh` (4/4 gates PASS) all green at hand-off. No blockers.
- **Linux gate (`bash scripts/mailpit-verify.sh`) NOT run in this environment** (macOS dev box, per CLAUDE.md) — this is this plan's last remaining verification surface. `cli/caprun/tests/confirm.rs`'s 4 cross-process tests (which run unconditionally on macOS too, and passed here) are the A2 positive control for cross-process key custody under the now-fully-MAC'd `pending_confirmations` scheme; they — plus `s9_live_block.rs` and the other Linux-gated live-acceptance files this phase's earlier plans already fixed for F1-safe layout — must be re-verified green on Linux before Phase 28 as a whole is marked done. This is the phase-level (not plan-level) verification step per this phase's structure (mirrors Plans 03/04's same note).
- This was the LAST plan in Phase 28 (per the orchestrator guardrail): this SUMMARY intentionally does NOT run `roadmap update-plan-progress`, does NOT flip the phase-level ROADMAP checkbox, and does NOT touch REQUIREMENTS.md sign-off — the orchestrator owns that reconciliation after the phase-level Linux gate passes.
- All five `pending_confirmations`-adjacent residuals named in Phase 28's original scoping (HARDEN-02's `confirm()`/`deny()` uniform integrity gate, X-02) are now closed for the events chain (Plans 03-04) AND `pending_confirmations` (this plan) — HARDEN-02 as a whole is code-complete pending the Linux verification pass.

---
*Phase: 28-authenticated-audit-chain*
*Completed: 2026-07-13*

## Self-Check: PASSED

`.planning/phases/28-authenticated-audit-chain/28-05-SUMMARY.md` confirmed present on disk. Both task commits confirmed present in `git log --oneline --all`: `473d306` (Task 1+2, feat), `dfd2743` (Task 3, test).
