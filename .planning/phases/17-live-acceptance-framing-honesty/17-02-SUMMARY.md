---
phase: 17-live-acceptance-framing-honesty
plan: 02
subsystem: testing
tags: [rust, cli, live-acceptance, mailpit, taint, confirm-deny]

requires:
  - phase: 17-live-acceptance-framing-honesty
    plan: "17-01"
    provides: "brokerd::provenance_proof (not yet consumed by this plan — reserved for Plan 03's tooth #2 insertion at the TEETH-2-INSERTION-POINT sentinel)"
provides:
  - "cli/caprun/tests/live_acceptance_v1_3.rs — the composed ACCEPT-01 live acceptance scenario: shared-audit-db harness (email-sink variant of run_caprun_block/run_caprun_verb), a per-run nonce-doc builder + expected-recipient computation, a Mailpit count_messages_for_recipient primitive, LIMIT-1-free session/effect discovery, and the composed #[test] fn live_acceptance_v1_3_composed proving teeth #1/#3/#4/#5 live on Linux"
affects: ["17-03"]

tech-stack:
  added: []
  patterns:
    - "Shared-audit-db, sequential-invocation composition: 3 caprun process invocations (2 hostile-block-then-decide + 1 clean) sharing ONE persistent audit.db path, never :memory:, extended from live_acceptance_tainted_session.rs's 2-invocation file.create pattern to the email sink and to 3 sessions."
    - "LIMIT-1-free session/effect discovery: SELECT id FROM sessions ORDER BY rowid [DESC] replaces the unqualified LIMIT-1 anti-pattern everywhere a multi-session shared DB is queried — captured immediately after each invocation."
    - "Per-run-nonce fixture isolation: a distinct Uuid::new_v4() per hostile leg embedded in the Domain: fragment (not the literal ev1l.com), so Mailpit count assertions are airtight against cross-leg/cross-binary pollution in the shared sidecar inbox."

key-files:
  created:
    - cli/caprun/tests/live_acceptance_v1_3.rs
  modified: []

key-decisions:
  - "Both plan tasks' code (the harness scaffolding of Task 1 and the composed #[test] of Task 2) were written together in a single Write call and landed in one commit (51a1a56) — the harness functions and the test that consumes them were designed as one cohesive unit and splitting the file write across two commits would have required an artificial intermediate state (a harness with no caller). Task 2's live verification was then run separately against that already-committed code and passed with zero code changes required, so no second commit exists for Task 2 — documented here as the deviation from strict one-commit-per-task."
  - "hostile_doc_with_nonce_domain keeps Reply-To: accounts and the Body: line byte-identical to s9_live_block.rs's HOSTILE_EMAIL_CONTENT, varying only the Domain: fragment — preserving the genuine two-fragment concat derivation (EXTRACT-01) rather than weakening the fixture to hit the nonce requirement."
  - "The deny leg's ledger-absence assertion queries events directly by (session_id, event_type, actor) rather than via find_event_by_type, so it can assert a COUNT (0) rather than rely on find_event_by_type's first-match Option semantics — matches the plan's explicit two-half non-negotiable-A requirement (on-the-wire count AND ledger absence)."

requirements-completed: []

coverage:
  - id: D1
    description: "live_acceptance_v1_3.rs compiles under cargo build --workspace --tests on macOS (live bodies cfg-excluded, guard test present); no unqualified LIMIT-1 session lookup anywhere in the file; all Task-1-listed harness symbols are defined"
    requirement: "ACCEPT-01"
    verification:
      - kind: unit
        ref: "cargo build --workspace --tests"
        status: pass
    human_judgment: false
  - id: D2
    description: "live_acceptance_v1_3_composed passes live on real Linux under scripts/mailpit-verify.sh: confirm leg blocks then confirms (exit 0), sends exactly once to its own nonced recipient (Mailpit count==1) with an email_send_succeeded event; deny leg blocks then denies (exit 2), sends nothing to its own nonced recipient (Mailpit count==0) with no email_send_attempted/email_send_succeeded event carrying its effect_id in the actor; clean control leg is Allowed and delivered (plan_node_evaluated present, no sink_blocked, pending_confirmations==0, email_send_succeeded, Mailpit capture); all 3 sessions' verify_chain is true; exactly 3 session rows exist in the shared audit.db"
    requirement: "ACCEPT-01"
    verification:
      - kind: integration
        ref: "MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3 live_acceptance_v1_3_composed -- --nocapture' bash scripts/mailpit-verify.sh (Colima+Docker, Linux) — 1 passed, 0 failed; all 3 Chain verification: PASSED lines present; script's own final line: 'Mailpit-backed Linux verification suite PASSED.'"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-07-09
status: complete
---

# Phase 17 Plan 02: Composed ACCEPT-01 Live Acceptance Scenario (Teeth #1/#3/#4/#5) Summary

**Built `cli/caprun/tests/live_acceptance_v1_3.rs` — three sequential real `caprun` process invocations (hostile-block→confirm, a SEPARATE hostile-block→deny, clean-control) sharing ONE persistent `audit.db`, and proved live on real Linux (via `scripts/mailpit-verify.sh`) that the confirm leg sends exactly once, the deny leg sends nothing (both the Mailpit count AND the audit-ledger absence), the clean leg delivers ungated, and all three sessions independently `verify_chain`-true.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-09
- **Tasks:** 2
- **Files modified:** 1 (new file)

## Accomplishments

- Created `cli/caprun/tests/live_acceptance_v1_3.rs` with:
  - `hostile_doc_with_nonce_domain(nonce)` / `expected_recipient(nonce)` — per-run nonced hostile fixture (COORD-A revised fixture rule), keeping `Reply-To: accounts` and the `Body:` line byte-identical to `s9_live_block.rs`'s `HOSTILE_EMAIL_CONTENT`, varying only the `Domain:` fragment to `{nonce}.ev1l.test` — never the reserved `accounts@ev1l.com` literal.
  - `run_caprun_email_on(recipient, content, audit_db, tag)` and `run_caprun_verb(verb, effect_id, audit_db)` — shared-audit-db process runners extending `live_acceptance_tainted_session.rs`'s `file.create` pattern to the email sink.
  - `mod mailpit_client` — the reproduced Mailpit HTTP client (host/message_ids/detail_addressed_to/wait_for_recipient_captured, including the chunked-transfer-decode handling) plus the new `count_messages_for_recipient` numeric primitive for the "deny sends nothing" / "confirm sends exactly once" assertions.
  - `all_session_ids(conn)` / `latest_session_id(conn)` / `discover_latest_blocked_effect_id(audit_db)` — `ORDER BY rowid` session/effect discovery, replacing the unqualified `LIMIT 1` anti-pattern that only ever worked by construction in existing tests (17-RESEARCH.md Pitfall 1).
  - `#[test] fn live_acceptance_v1_3_composed()` — the full composed scenario (confirm leg, deny leg, clean-control leg, end-of-run `verify_chain` sweep), with the `TEETH-2-INSERTION-POINT` sentinel left for Plan 03's genuine-taint re-proof.
  - `#[test] fn live_acceptance_v1_3_guard_binary_present()` — always-compiled macOS guard.
- Ran the live verification via `MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3 live_acceptance_v1_3_composed -- --nocapture' bash scripts/mailpit-verify.sh` under Colima+Docker: **PASSED** on the first attempt — 1 passed, 0 failed; all three sessions' `Chain verification: PASSED`; the script's own gate line `Mailpit-backed Linux verification suite PASSED.`

## Task Commits

1. **Task 1 + Task 2 (harness + composed test)** - `51a1a56` (feat) — see Deviations below for why both tasks' code landed in one commit.

_Note: no plan-metadata commit in this file — worktree mode (isolation="worktree"); the orchestrator commits STATE.md/ROADMAP.md centrally after merge._

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_v1_3.rs` (new, 565 lines) — the composed ACCEPT-01 live acceptance scenario: harness scaffolding (Task 1) + the composed `#[test]` (Task 2), all `#[cfg(target_os = "linux")]`-gated except the macOS guard test.

## Decisions Made

- **Single-commit landing for both tasks:** the harness functions (Task 1) and the composed test that calls them (Task 2) were authored together in one `Write` call, since the harness has no independent meaning without a caller and the composed test cannot exist without the harness — splitting them into two commits would have required committing a harness with zero call sites, an artificial intermediate state. Task 2's `<verify>` (the live `mailpit-verify.sh` run) was executed as its own distinct step against the already-committed code and passed with zero code changes, so there is no second commit — documented here rather than fabricating a no-op commit.
- **Fixture fidelity over minimal-diff nonce insertion:** `hostile_doc_with_nonce_domain` keeps every line of `HOSTILE_EMAIL_CONTENT` byte-identical except the `Domain:` line, preserving the genuine two-fragment `Reply-To:`/`Domain:` concat derivation (EXTRACT-01) that produces a real two-anchor Block — confirmed live: both hostile legs' audit DAGs show a `derivation` event followed by `sink_blocked` with both `body` and `to` args BLOCKED.
- **Deny-leg ledger-absence via direct COUNT query:** rather than reusing `find_event_by_type` (which only returns the FIRST match by type, an `Option`), the deny leg's ledger-absence assertion runs `SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = ?2 AND actor = ?3` for both `email_send_attempted` and `email_send_succeeded`, scoped to `sink:email.send:{deny_effect_id}` — matching the plan's non-negotiable-A(ii) requirement precisely (asserts absence of THIS effect's send events, not merely "no event of that type exists at all in the session").

## Deviations from Plan

### Auto-fixed Issues

None — the plan's task specifications were precise enough (near-pseudocode) that no Rule 1-3 auto-fixes were needed; the composed test passed live on the first Colima/Docker run.

### Process Deviation (documented, not a Rule 1-4 issue)

**1. Both tasks' code landed in a single commit (`51a1a56`) instead of two.** See "Decisions Made" above for the rationale. No code correctness impact — `cargo build --workspace --tests` was run and passed before the commit (satisfying Task 1's `<done>` gate), and the live Mailpit verification was run and passed as a separate step after the commit (satisfying Task 2's `<verify>`/`<done>` gate). Both tasks' individual `<done>` criteria are independently verified in this SUMMARY's `coverage` section (D1, D2).

## Issues Encountered

None. The live composed run passed on the first attempt: three real `caprun` process invocations against one shared `audit.db`, both hostile legs producing genuine two-anchor blocks (`to` + `body`, both `[external.untrusted, worker.extracted]`/`[external.untrusted]` tainted), the confirm leg releasing exactly once (Mailpit count 1 for its own nonced recipient, `email_send_succeeded` present), the deny leg releasing nothing (Mailpit count 0 for its own nonced recipient, no `email_send_attempted`/`email_send_succeeded` event for its effect_id), the clean-control leg delivering ungated (`plan_node_evaluated` present, no `sink_blocked`, `pending_confirmations` == 0, `email_send_succeeded` present, Mailpit-captured), and all three sessions' `Chain verification: PASSED`.

## User Setup Required

None beyond the already-standing Colima + Docker + `scripts/mailpit-verify.sh` recipe (unchanged from Phase 13/16) — no new environment requirement, no new Cargo dependency.

## Next Plan Readiness

- `live_acceptance_v1_3_composed`'s body ends with the exact sentinel comment `// TEETH-2-INSERTION-POINT: genuine-taint re-proof appended by Plan 03 (17-03)` immediately before the function's closing brace, so Plan 03 can append tooth #2 (the HARD-GATE genuine-taint re-proof reusing `brokerd::provenance_proof`'s `assert_unbroken_edge`/`genuine_derivation_binds`, promoted in Plan 01) without re-deriving the function.
- The confirm leg's and deny leg's `effect_id`s (`confirm_effect_id`, `deny_effect_id`) and their respective `session_id`s are already in scope at the sentinel's insertion point via `confirm_session_id`/`deny_session_id` local bindings — Plan 03 does not need to re-discover them.
- `cargo build --workspace` and the live Mailpit-scoped run both pass; no regression to any existing live test (`s9_live_block.rs`, `live_acceptance_tainted_session.rs` were read but not modified).

## Self-Check: PASSED

- FOUND: `cli/caprun/tests/live_acceptance_v1_3.rs`
- FOUND: `51a1a56` (git log --oneline --all)
- Verified: `cargo build --workspace --tests` — 0 errors, 0 warnings
- Verified: `grep -n "SELECT id FROM sessions LIMIT 1" cli/caprun/tests/live_acceptance_v1_3.rs` — no matches
- Verified: `grep -n "accounts@ev1l.com" cli/caprun/tests/live_acceptance_v1_3.rs` — no matches
- Verified: `grep -n '```' cli/caprun/tests/live_acceptance_v1_3.rs` — no matches (no fenced code blocks)
- Verified: `grep -n "TEETH-2-INSERTION-POINT" cli/caprun/tests/live_acceptance_v1_3.rs` — sentinel present
- Verified (live, Linux, Colima+Docker): `MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3 live_acceptance_v1_3_composed -- --nocapture' bash scripts/mailpit-verify.sh` — exit code captured via `$?` before any pipe (redirected to a log file, no `| tail`/`| grep` on the invocation itself) — `1 passed; 0 failed`; 3/3 `Chain verification: PASSED`; script's own `Mailpit-backed Linux verification suite PASSED.` line present.

---
*Phase: 17-live-acceptance-framing-honesty*
*Completed: 2026-07-09*
