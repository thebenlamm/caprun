---
phase: 16-confirm-ux-literal-binding-negative-controls
plan: 03
subsystem: testing
tags: [rust, taint-tracking, e2e-test, security-control, cli-integration]

# Dependency graph
requires:
  - phase: 16-confirm-ux-literal-binding-negative-controls
    provides: "Plan 16-01's combined-digest producer (Event/PendingConfirmation.combined_digest, server.rs Block-time write) — the existing sink_blocked event/anchor shape this test asserts against"
  - phase: 15-deterministic-doc-action-extraction
    provides: "EXTRACT-01's live confined-worker recipient-derivation mechanism (Reply-To:/Domain: marker-anchored fragments) and Body: marker extraction, and the existing s9_live_block.rs live-test harness (run_caprun_intent_on)"
provides:
  - "A live/wire-level CONTROL-02 fixture proving a tainted body with a TRUSTED recipient still blocks with EXACTLY one anchor (\"body\"), through the real confined-worker + broker + executor stack"
affects: ["16-04 (composes CONTROL-01 into the same run as the hostile block; CONTROL-02's live proof is independent of that composition)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Body-tainted-only, recipient-trusted fixture: model recipient side on CLEAN_PATH_CONTENT's no-marker convention (derived_recipient stays None, so the TRUSTED CLI intent value routes to email.send/to) + a Body: marker only (mirrors HOSTILE_EMAIL_CONTENT's body mechanism) to isolate the content-taint dimension from the routing-taint dimension in one live run"

key-files:
  created: []
  modified:
    - cli/caprun/tests/s9_live_block.rs

key-decisions:
  - "Reused the existing run_caprun_intent_on helper and CLEAN_PATH_CONTENT/HOSTILE_EMAIL_CONTENT conventions rather than introducing a new harness — kept the new fixture/test as a direct sibling of s9_live_email_hostile_block"
  - "Asserted the single-anchor shape as two explicit checks (length == 1, then arg == \"body\") rather than one vec-equality check, so an accidental 2-anchor block (Pitfall 5 — recipient accidentally tainted too) fails on the length check with a message that names the failure mode directly"

requirements-completed: [CONTROL-02]

coverage:
  - id: D1
    description: "A body-tainted-only doc with a TRUSTED CLI recipient blocks live (non-zero exit) through the real confined-worker + broker + executor stack, with a durable sink_blocked event carrying EXACTLY one anchor named \"body\" (never [\"body\",\"to\"], never empty), and no sink_executed / email_send_succeeded event — proving the body (content) dimension is not dead code redundant with the routing-sensitivity block."
    requirement: "CONTROL-02"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_live_block.rs#s9_control02_body_tainted_recipient_trusted_blocks"
        status: pass
    human_judgment: false

# Metrics
duration: 25min
completed: 2026-07-08
status: complete
---

# Phase 16 Plan 03: CONTROL-02 Body-Tainted-Only Live Block Summary

**Live/wire-level fixture proving a tainted body with a TRUSTED recipient still blocks with exactly the `["body"]` anchor — the body (content) sensitivity dimension is independently live, not dead code redundant with the routing-sensitivity block.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-08
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added `CONTROL02_BODY_TAINTED_CONTENT`: a fixture carrying a `Body:` marker (taints `body`) but deliberately no `Reply-To:`/`Domain:` marker-anchored recipient fragments, so the confined worker's `derived_recipient` stays `None` and the planner routes the TRUSTED CLI intent value into `email.send/to`.
- Added `s9_control02_body_tainted_recipient_trusted_blocks`: runs the real `caprun` binary end to end (confined worker → broker → executor) with a trusted CLI recipient over that content, asserts a non-zero exit, a durable `sink_blocked` event with EXACTLY one anchor (explicit `length == 1` check, then `arg == "body"` check — so an accidental 2-anchor block from a mistaken recipient taint fails loudly rather than silently), and no `sink_executed`/`email_send_succeeded` event.
- Verified the compile guard: `cargo build -p caprun --tests` succeeds on macOS with the new body cfg-excluded (only the cross-platform guard test runs, as expected).
- Ran the authoritative Linux verification (`bash scripts/mailpit-verify.sh`, Colima+Docker, unprivileged, `seccomp=unconfined`): the FULL workspace suite passed, including the new test (`test s9_control02_body_tainted_recipient_trusted_blocks ... ok`).

## Task Commits

1. **Task 1: CONTROL-02 — body-tainted-only, recipient-trusted, single-anchor live block** - `23b1bc8` (test)

## Files Created/Modified

- `cli/caprun/tests/s9_live_block.rs` - added `CONTROL02_BODY_TAINTED_CONTENT` fixture + `s9_control02_body_tainted_recipient_trusted_blocks` live test (block-only, no Mailpit query needed)

## Decisions Made

- Confirmed (by direct read of `cli/caprun/src/worker.rs`'s `SendEmailSummary` extraction branch) that zero `Reply-To:`/`Domain:` doc fragments produces `derived_recipient = None` regardless of whether a `Body:` fragment is also present — the body handle is always the last element in `fragment_value_ids`, indexed by `doc_fragments.len()`. This confirmed the fixture design (no accidental recipient taint) before writing the test, rather than relying on the plan's description alone.
- Kept the fixture prose realistic (mirrors `HOSTILE_EMAIL_CONTENT`'s tone: "Q3 vendor notes...") rather than a minimal synthetic string, for consistency with the existing suite's style.

## Deviations from Plan

None - plan executed exactly as written. The fixture and test match the plan's `<action>` and `<acceptance_criteria>` exactly: no Reply-To:/Domain: markers, a Body: marker only, explicit length==1 + name=="body" assertions, no sink_executed/email_send_succeeded, reuse of the existing `run_caprun_intent_on` helper.

## Issues Encountered

None. The Linux verification (`scripts/mailpit-verify.sh`) passed on the first run; the fixture's extraction behavior matched the prediction from reading `worker.rs` directly, with no debugging needed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- CONTROL-02 is fully proven live. This plan's scope was deliberately narrow per its own SCOPE NOTE: CONTROL-01 (trusted-send delivery to Mailpit) moved to Plan 16-04, which is unaffected by this change (disjoint file: `cli/caprun/src/main.rs`, `crates/brokerd/src/confirmation.rs`, `crates/brokerd/src/audit.rs` vs. this plan's sole file `cli/caprun/tests/s9_live_block.rs`).
- No blockers for 16-04 or the phase's remaining plans.

---
*Phase: 16-confirm-ux-literal-binding-negative-controls*
*Completed: 2026-07-08*

## Self-Check: PASSED

- FOUND: cli/caprun/tests/s9_live_block.rs
- FOUND: .planning/phases/16-confirm-ux-literal-binding-negative-controls/16-03-SUMMARY.md
- FOUND commit: 23b1bc8
