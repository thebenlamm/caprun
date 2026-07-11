---
phase: 18-trust-boundary-coherence-design-gate
plan: 01
subsystem: security-design
tags: [design-doc, trust-boundary, session-coherence, taint-model, replay-risk, capability-model, brokerd]

# Dependency graph
requires:
  - phase: 16 (v1.3, session-trust-state / confirm-binding)
    provides: SessionStatus::Draft, guard(a) ProvideIntent ordering, BlockedPendingConfirmation shape
provides:
  - "planning-docs/DESIGN-session-trust-coherence.md — the v1.4 Phase 0 design gate, resolving DESIGN-01..06"
affects: [phase-19-implementation, phase-20-planner-seam-design]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Design-gate-before-TCB-code discipline (mirrors v1.0 Phase 2 / v1.2 Phase 8 / v1.3 Phase 12): reject-2nd-connection fix specified against exact server.rs line numbers before any code is written"
    - "Per-verb capability split (mint-verb-exclusion) as the mechanism for future capability-restricted connections, distinct from single-connection-per-session enforcement"

key-files:
  created:
    - planning-docs/DESIGN-session-trust-coherence.md
  modified: []

key-decisions:
  - "Fix shape: reject a 2nd connection to an already-active session (DESIGN-01) — shared coherent multi-connection state named and rejected in writing (larger blast radius, more TCB surface, no legitimate N>1 need)"
  - "Replay risk (MAJOR-2/DESIGN-02) re-earned in writing as an Accepted Residual Risk under the adaptive-planner threat model — bounded to trusted/human-typed recipients, no new CAS designed, v2 CAS obligation carried forward"
  - "Three-mint-site audit (DESIGN-03): only ProvideIntent (mint_from_intent) yields a TRUSTED handle from a supplied string; mint_from_read and mint_from_derivation are safe via the sensitivity map and byte-verify/provenance-rootedness respectively; reopening tripwire named"
  - "Decision oracle (DESIGN-04): Phase 20's planner connection must receive a REDUCED allow/block signal, never the full anchors/literal_sha256/literal set, tied to the per-verb capability split"
  - "guard-(c) (CAPRUN_ENABLE_IPC_CREATE_SESSION) re-confirmed not widened by this fix; compile-exclusion recommended for a future milestone, not committed in v1.4"
  - "T2 (slot-type binding) explicitly recorded as out-of-scope/deferred to v1.5 — not designed in this document"

requirements-completed: [DESIGN-01, DESIGN-02, DESIGN-03, DESIGN-04, DESIGN-05, DESIGN-06]

coverage:
  - id: D1
    description: "DESIGN-session-trust-coherence.md authored, resolving DESIGN-01..06 across nine numbered sections (§1 the bug, §2 fix shape, §3 capability split, §4 guard-c, §5 mint-site audit, §6 replay re-earned, §7 decision oracle, §8 acceptance predicate, §9 accepted residual risks)"
    requirement: "DESIGN-01"
    verification:
      - kind: unit
        ref: "Task 1 automated grep gate (reject/2nd-connection/shared-coherent-multi-connection/ProvideIntent/ReportClaims/ReportDerivedClaim/CAPRUN_ENABLE_IPC_CREATE_SESSION all present)"
        status: pass
      - kind: unit
        ref: "Task 2 automated grep gate (mint_from_read/mint_from_intent/mint_from_derivation/re-earn/literal_sha256/oracle/residual/v1.5-or-deferred all present)"
        status: pass
    human_judgment: true
    rationale: "This is a design document whose substantive correctness (does the fix shape actually close the bug, is the mint-site reduction actually narrower-and-correct, is the replay re-earning actually justified) requires a fresh adversarial human/AI review, not just grep-based presence checks. Plan 18-02's adversarial gate is the mechanism that judges this — this plan's own grep gates only confirm the required topics are present, not that the reasoning is sound."
  - id: D2
    description: "No TCB/source file modified — plan scope is planning-docs/*.md only"
    verification:
      - kind: other
        ref: "git status --porcelain crates/ cli/ (empty output)"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-07-11
status: complete
---

# Phase 18 Plan 01: DESIGN-session-trust-coherence.md Summary

**Authored the v1.4 Phase 0 design gate: reject-a-2nd-connection fix specified against server.rs's exact accept-loop and per-connection-local line numbers, the replay risk re-earned in writing under the adaptive-planner threat model, and a corrected narrower three-mint-site audit with a named reopening tripwire.**

## Performance

- **Duration:** 55 min
- **Started:** 2026-07-11T03:07Z (approx, per plan start)
- **Completed:** 2026-07-11T03:07Z
- **Tasks:** 2
- **Files modified:** 1 (created)

## Accomplishments

- `planning-docs/DESIGN-session-trust-coherence.md` created and fully populated across §1-§9, resolving all six DESIGN-01..06 requirements.
- §2 names reject-a-2nd-connection-to-an-active-session as the CHOSEN fix and "shared coherent multi-connection state" as the REJECTED alternative, with a written rejection rationale (larger blast radius, more TCB surface, no legitimate N>1 need) — never presented as the chosen approach. Cites `server.rs`'s actual accept loop (lines 98-121) and per-connection locals (`session_status`:148, `intent_provided`/`fd_requested`:153-154) so Phase 19 has an unambiguous implementation target, including where the authoritative occupancy gate must sit (before `handle_connection` seeds those locals, not after).
- §3 specifies the per-verb capability split (a connection may hold NO mint verb: `ProvideIntent`, `ReportClaims`, `ReportDerivedClaim`), forward-looking for Phase 20's planner connection, and explains why it is a distinct mechanism from §2's occupancy gate rather than a substitute for it.
- §4 re-confirms guard-(c) (`CAPRUN_ENABLE_IPC_CREATE_SESSION`) is not widened by this fix and gives a for/against recommendation on compile-exclusion without committing v1.4 to it.
- §5 audits all three mint sites (`mint_from_read` ~428, `mint_from_intent` ~949/962/979, `mint_from_derivation` ~849) and states the corrected narrower claim verbatim ("the only mint yielding a TRUSTED handle from a supplied string is `ProvideIntent`"), plus the reopening tripwire tied to the sink sensitivity map's blocking treatment of `WorkerExtracted`/`ExternalUntrusted`.
- §6 re-earns the replay risk (MAJOR-2) in writing under the adaptive-planner threat model, bounds amplification to trusted/human-typed recipients (untrusted content still Blocks), points to the existing durable `email_send_attempted` per-attempt ledger, and adds no new CAS design — the minimal-CAS v2 obligation is carried forward explicitly.
- §7 documents the `Allowed`-vs-`BlockedPendingConfirmation{anchors}` decision oracle and the `literal_sha256` offline-guess confirmer, ruling that Phase 20's planner connection must receive a reduced allow/block signal rather than the full anchor/digest/literal set.
- §8 (Acceptance Predicate) and §9 (Accepted Residual Risks) close out the doc, with T2 (slot-type binding) explicitly recorded as out-of-scope/deferred to v1.5 and not designed here.
- Both plan-specified automated grep verification gates (Task 1 and Task 2) pass.
- No `server.rs` or other TCB/source file was touched — `git status --porcelain crates/ cli/` is empty.

## Task Commits

Each task was committed atomically:

1. **Task 1: Author the structural-mechanism sections (fix shape, capability split, guard-(c))** - `5577998` (docs)
2. **Task 2: Author the adversarial-ruling sections (mint-site audit, replay re-earned, decision oracle)** - `8c640fd` (docs)

_Note: the file was authored in full, then split into two commits along the exact §1-§4 / §5-§9 task boundary (Task 1 committed first with only §1-§4 content present on disk, then the full §1-§9 content restored and committed as Task 2's addition) so each task's commit reflects only that task's contribution, per the atomic-per-task commit protocol._

## Files Created/Modified

- `planning-docs/DESIGN-session-trust-coherence.md` - the v1.4 Phase 0 design gate doc, §1 the bug precisely (cross-connection ProvideIntent bypass), §2 fix shape (reject 2nd connection, chosen vs. shared-state rejected), §3 per-verb capability split, §4 guard-(c) status, §5 three-mint-site audit, §6 replay re-earned, §7 decision oracle, §8 acceptance predicate, §9 accepted residual risks.

## Decisions Made

- Fix shape: reject-a-2nd-connection-to-an-active-session is specified as an accept-time occupancy gate that must resolve BEFORE `handle_connection` seeds its per-connection trust locals — this placement detail (not explicitly spelled out in the plan's read_first material beyond "resolved BEFORE `handle_connection` seeds its per-connection locals") is the load-bearing implementability point for Phase 19, so it was made explicit and repeated in §2's summary paragraph.
- The per-verb capability split (§3) was framed as a distinct mechanism from the occupancy gate (§2) — reconciling the apparent tension that Phase 20 wants a second connection to the same session while Phase 0 forbids exactly that for the worker's own connection. This reconciliation (capability-set decided at establishment vs. occupancy-slot count) was necessary to keep §2 and §3 internally consistent and is called out explicitly in §3's second paragraph.
- The decision-oracle ruling (§7) was tied explicitly back to §3's capability model per the plan's own instruction, giving Phase 20 a single coherent design target (reduced capability set implies reduced decision signal) rather than two independently-justified rulings.

## Deviations from Plan

None - plan executed exactly as written. Both tasks' acceptance criteria and automated verify gates were met without needing any Rule 1-4 deviation. The only implementation-level choice made beyond the plan's literal text was splitting the single-file authoring into two commits along the task boundary (see Task Commits note above) to preserve per-task commit atomicity — this is a process/commit-mechanics choice, not a content deviation from the plan's required sections or claims.

## Issues Encountered

None. All context files (PROJECT.md, STATE.md, v1.4-SCOPE.md, v1.4-REVIEW-RECONCILIATION.md, DESIGN-session-trust-state.md, server.rs, executor_decision.rs, two_connection_intent_bypass.rs, planner.rs, REQUIREMENTS.md) were read and cross-checked against the plan's cited line numbers before writing — all citations in the plan (accept loop 98-121, session_status:148, intent_provided/fd_requested:153-154, guard(a):904, guard-(c):267-283, mint_from_read arm ~393-479/call~428, ProvideIntent arm ~894-1011/mint calls 949/962/979, ReportDerivedClaim arm ~795-892/mint call~849, email.send Allowed dispatch ~705-788/replay comment~770-776, email_send_attempted ledger~750-768, executor_decision.rs Allowed/BlockedPendingConfirmation 185-202, literal_sha256:144) were verified against the actual file contents and found accurate, so no correction was needed.

## User Setup Required

None - no external service configuration required. This plan produces only a markdown design document.

## Next Phase Readiness

- `planning-docs/DESIGN-session-trust-coherence.md` exists and is ready for Plan 18-02's fresh adversarial panel (`planning-docs/DESIGN-GATE-RECORD-v1.4.md`), which is the hard gate before any `server.rs` code for the trust-coherence fix.
- Phase 19 (the fix implementation) and everything downstream (Phases 20-22) remain blocked until Plan 18-02 records decision = APPROVED.
- No blockers identified in this plan's own scope; the design doc's own §9 records five accepted residual risks (replay-without-CAS re-earned, the three-mint-site tripwire, the worker-connection oracle, guard-(c) remaining a runtime flag, and T2 deferred to v1.5) that Plan 18-02's reviewer should specifically re-check.

---
*Phase: 18-trust-boundary-coherence-design-gate*
*Completed: 2026-07-11*
