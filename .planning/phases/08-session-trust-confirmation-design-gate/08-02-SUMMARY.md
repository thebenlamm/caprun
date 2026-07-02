---
phase: 08-session-trust-confirmation-design-gate
plan: 02
subsystem: docs
tags: [design-gate, taint-model, executor, confirmation, audit-dag, sqlite]

# Dependency graph
requires:
  - phase: 08-01
    provides: DESIGN-session-trust-state.md (I1/I0 mechanism this doc treats as an independent sibling)
provides:
  - "planning-docs/DESIGN-confirmation-release.md — PendingConfirmation checkpoint schema, caprun confirm CLI contract, single-shot release semantics, durable-deny semantics, TCB-residency rule"
affects: [08-03-DESIGN-GATE-RECORD-v1.2, phase-10-confirmation-loop]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Durable pause/resume checkpoint (PendingConfirmation) as a sibling side table to the append-only events ledger, mirroring the existing blocked_literals precedent"
    - "Confirm as a distinct, logged endorsement path — never a second call to submit_plan_node"

key-files:
  created: [planning-docs/DESIGN-confirmation-release.md]
  modified: []

key-decisions:
  - "PendingConfirmation is a new, distinct durable record (superset of SinkBlockedAnchor), never an in-place extension of the anchor, to avoid breaking its golden-byte-fixture serialization contract"
  - "Confirm re-invokes the sink using the FROZEN Block-time-resolved args, never re-resolved at confirm time, avoiding a TOCTOU-shaped re-resolution question"
  - "v1.2 confirm does not implement auto-resolution against a prior endorsement Event for a repeat literal — every block requires its own separate confirm, deferring DESIGN-taint-model.md's broader endorsement-consultation step to a future milestone"

patterns-established:
  - "Section skeleton mirrors DESIGN-plan-executor.md exactly: Problem Being Solved -> schema-as-table -> ordered Decision Logic steps -> CLI/UX contract -> Relationship & Done-When"

requirements-completed: [PROC-01]

coverage:
  - id: D1
    description: "DESIGN-confirmation-release.md specifies the PendingConfirmation durable checkpoint schema (effect_id/session_id/full plan_node arg-set/state) persisted atomically at Block time"
    requirement: "PROC-01"
    verification:
      - kind: other
        ref: "grep -c 'PendingConfirmation' planning-docs/DESIGN-confirmation-release.md (19, >= 3 required)"
        status: pass
      - kind: other
        ref: "grep -ci 'Block time|Block-time' planning-docs/DESIGN-confirmation-release.md (2, >= 1 required)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The doc specifies confirm does NOT call submit_plan_node a second time — a distinct, logged endorsement path that directly invokes the sink using the frozen Block-time snapshot"
    requirement: "PROC-01"
    verification:
      - kind: other
        ref: "grep -c 'submit_plan_node' planning-docs/DESIGN-confirmation-release.md (6, >= 1 required)"
        status: pass
      - kind: other
        ref: "grep -ci 'MUST NOT.*submit_plan_node' planning-docs/DESIGN-confirmation-release.md (2, >= 1 required)"
        status: pass
    human_judgment: false
  - id: D3
    description: "The doc specifies single-shot release semantics: exactly one (sink, arg, literal-digest) triple, never a session-wide waiver or standing policy; deny is durable and the effect_id can never be re-confirmed"
    requirement: "PROC-01"
    verification:
      - kind: other
        ref: "grep -ci 'exactly one|single-shot' planning-docs/DESIGN-confirmation-release.md (9, >= 2 required)"
        status: pass
      - kind: other
        ref: "grep -ci 'standing.*polic|session-wide waiver' planning-docs/DESIGN-confirmation-release.md (6, >= 1 required)"
        status: pass
      - kind: other
        ref: "grep -ci 'durable' planning-docs/DESIGN-confirmation-release.md (6, >= 3 required)"
        status: pass
    human_judgment: false
  - id: D4
    description: "The doc specifies the caprun confirm <effect_id> CLI contract verbatim (exact output format, exit-code contract) and that the release path lives in the TCB, never a policy file"
    requirement: "PROC-01"
    verification:
      - kind: other
        ref: "grep -ci 'caprun confirm' planning-docs/DESIGN-confirmation-release.md (16, >= 3 required)"
        status: pass
      - kind: other
        ref: "grep -ci 'TCB' planning-docs/DESIGN-confirmation-release.md (7, >= 2 required)"
        status: pass
      - kind: other
        ref: "grep -ci 'interactive.*TTY|TTY prompt' planning-docs/DESIGN-confirmation-release.md (1, >= 1 required)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Human adversarial review (as an attacker) that the doc is precise enough for Phase 10 to implement CONFIRM-01..04 without a new design decision"
    verification: []
    human_judgment: true
    rationale: "This is a design-gate document; PROC-01's soundness (not just grep-completeness) requires the human review procedure specified in DESIGN-GATE-RECORD-v1.2.md (plan 08-03), which greps pass fully-written-but-wrong specs and cannot substitute for."

# Metrics
duration: 22min
completed: 2026-07-02
status: complete
---

# Phase 8 Plan 2: Confirmation-Release DESIGN Doc Summary

**Authored `planning-docs/DESIGN-confirmation-release.md` — the PendingConfirmation durable checkpoint schema, confirmation decision logic, `caprun confirm <effect_id>` CLI contract, single-shot release semantics, durable-deny rule, and TCB-residency requirement that unblock Phase 10's confirmation-loop implementation.**

## Performance

- **Duration:** 22 min
- **Started:** 2026-07-02T02:59:00Z
- **Completed:** 2026-07-02T03:21:42Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Specified the `PendingConfirmation` schema (`effect_id`/`session_id`/full `plan_node` arg-set/`state`) as a new, distinct durable record — a superset of `SinkBlockedAnchor`, never an in-place extension of it — persisted atomically with the `sink_blocked` Event.
- Specified the ordered Confirmation Decision Logic: reopen the persistent DB (never `:memory:`), look up `PendingConfirmation` by `effect_id`, check terminal state from persisted (never in-memory) state, confirm-and-invoke-with-frozen-args or deny-and-terminate.
- Stated the critical soundness rule explicitly: confirm MUST NOT call `executor::submit_plan_node` a second time — it is a distinct, logged human-endorsement path, mirroring `DESIGN-taint-model.md`'s Declassification & Endorsement framing.
- Specified the exact `caprun confirm <effect_id>` CLI output format (literal + taint + source + provenance), the exit-code contract, and the locked "second command, never interactive TTY" UX decision.
- Stated single-shot release semantics (exactly one triple, no standing policy — and explicitly narrower than `DESIGN-taint-model.md`'s endorsement-consultation model for v1.2), durable-deny semantics, and TCB-residency (`crates/brokerd`/`crates/executor`, never a policy file).
- Closed with a 5-condition numbered Done-When predicate and two Accepted Residual Risks sections.

## Task Commits

Each task was committed atomically:

1. **Task 1: Author the PendingConfirmation checkpoint schema + confirmation decision logic** - `e67f5c7` (docs)
2. **Task 2: Author the caprun confirm CLI contract + single-shot/durable-deny semantics + Done-When predicate** - `3cb7ce1` (docs)

_Note: this is a documentation-only plan (no code); no separate metadata commit was made before this SUMMARY commit per parallel-execution instructions._

## Files Created/Modified
- `planning-docs/DESIGN-confirmation-release.md` - New DESIGN doc: mechanism spec for the confirmation-release path (PendingConfirmation schema, decision logic, CLI contract, release semantics, Done-When)

## Decisions Made
- `PendingConfirmation` is a sibling side table to the `events` ledger (following the exact `blocked_literals` precedent), not a field added to `SinkBlockedAnchor` — avoids coupling to the anchor's existing golden-byte-fixture serialization tests.
- Confirm re-invokes the sink with the args exactly as resolved at Block time (frozen snapshot), never re-resolved at confirm time — avoids reopening a TOCTOU-shaped question, per RESEARCH.md Open Question 2's recommendation.
- v1.2's confirm path does not implement the broader "auto-resolve against a prior endorsement Event" consultation step that `DESIGN-taint-model.md`'s Declassification & Endorsement section describes as a future release valve — every block in v1.2 requires its own separate confirm. This narrowing was made explicit in the doc's Single-Shot Release Semantics section to prevent Phase 10 from over-scoping CONFIRM-02.

## Deviations from Plan

None - plan executed exactly as written. Both tasks' acceptance-criteria greps and the plan's overall `<verification>` checks (no load-bearing "should" in rule statements) pass as specified.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required. This plan produces a Markdown design document only.

## Next Phase Readiness

- `planning-docs/DESIGN-confirmation-release.md` is ready for the human adversarial review step in plan 08-03 (`DESIGN-GATE-RECORD-v1.2.md`), alongside sibling plan 08-01's `DESIGN-session-trust-state.md`.
- Phase 10 (confirmation loop implementation, CONFIRM-01..04) is blocked until 08-03 records `Decision: APPROVED` and `Gate status: UNBLOCKED` for both docs — per this document's own `**Gate:**` line and PROC-01.
- No blockers for the DESIGN doc's own completeness; the remaining work is the human review gate (out of scope for this execute-plan agent).

---
*Phase: 08-session-trust-confirmation-design-gate*
*Completed: 2026-07-02*
