---
phase: 51-non-hybrid-live-proof-v1-10-done
plan: 06
subsystem: security-design
tags: [sqlite, audit-chain, concurrency, design-gate, tamper-evidence]

requires:
  - phase: 51-05
    provides: Failing audit-chain fork and contended-append regression oracles
provides:
  - Transactional append-at-head design contract for the coordinated D1+D2 repair
  - Complete classified inventory of 45 production append_event call sites
  - Unprefilled PENDING independent-review gate for the Phase 51 gap closure
affects: [51-07, 51-08, 51-04-rerun, LIVE-07, LIVE-08]

tech-stack:
  added: []
  patterns: [single public append choke point, BEGIN IMMEDIATE head selection, independent design-gate clearance]

key-files:
  created:
    - planning-docs/DESIGN-audit-append-concurrency.md
  modified:
    - planning-docs/DESIGN-GATE-RECORD-v1.10.md

key-decisions:
  - "append_event remains the single public append entry point and derives the authoritative parent from the durable head inside its transaction."
  - "The causal parent_id chain and provenance read_event_id graph remain distinct and must never be unified."
  - "AUDIT_BUSY_TIMEOUT_MS is pinned to 5000, but timeout ownership and BEGIN IMMEDIATE must land together because timeout alone does not repair D1."
  - "Both latent defect instances are FIXED BY CONSTRUCTION through the append choke point."

patterns-established:
  - "Design-before-TCB: a written contract and PENDING non-self review gate precede the audit implementation change."
  - "Inventory-backed review: every production append call site is classified before the adversarial trace."

requirements-completed: [LIVE-07, LIVE-08]

coverage:
  - id: D1
    description: "The D1+D2 repair has a mechanically checkable design contract that preserves verifier and provenance invariants."
    requirement: LIVE-07
    verification:
      - kind: other
        ref: "51-06 PLAN heading/content checks plus ./scripts/check-invariants.sh"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every production append_event call site is inventoried and classified for Plan 51-08 review."
    requirement: LIVE-08
    verification:
      - kind: other
        ref: "Appendix A row/count and required-anchor checks"
        status: pass
    human_judgment: false
  - id: D3
    description: "The audit append-at-head gate remains PENDING for an orchestrator-owned fresh non-self adversarial trace."
    requirement: LIVE-08
    verification: []
    human_judgment: true
    rationale: "Plan 51-08 intentionally owns reviewer identity, findings, date, and verdict; executor self-review cannot clear this gate."

duration: 3min
completed: 2026-08-05
status: complete
---

# Phase 51 Plan 06: Audit Append-at-Head Concurrency Design Summary

**Transactional append-at-head contract with a 45-site production inventory and an intentionally unprefilled independent-review gate**

## Performance

- **Duration:** 3 min
- **Started:** 2026-08-05T22:02:18Z
- **Completed:** 2026-08-05T22:04:40Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Pinned `append_event` as the sole public append choke point, with the durable head selected under `BEGIN IMMEDIATE` and caller parent state treated as advisory.
- Closed the two attractive wrong turns in writing: `verify_chain` stays unchanged, and the causal chain must never be unified with `read_event_id` / `provenance_chain`.
- Mechanically inventoried 45 production call sites: 2 `session-root`, 40 `chain-continuation`, and 3 `in-transaction` sites.
- Opened the Phase 51 audit append gate as PENDING with every reviewer field `TBD`; no self-review clearance was recorded.

## Task Commits

Each task was committed atomically:

1. **Task 1: Write the append-at-head concurrency design note** — `c9e1576`
2. **Task 2: Build the append call-site inventory and open a PENDING gate record entry** — `66b69ac`

## Files Created/Modified

- `planning-docs/DESIGN-audit-append-concurrency.md` — D1+D2 design authority, non-goals, rollback oracle, latent-instance dispositions, and production append inventory.
- `planning-docs/DESIGN-GATE-RECORD-v1.10.md` — PENDING Phase 51 gap-closure gate awaiting Plan 51-08's independent review.

## Decisions Made

- The persisted parent and MAC input come from the durable head read within the append transaction; local caller parent state cannot select the row's parent.
- Existing enclosing transactions at `server.rs:1184`, `server.rs:1557`, and `confirmation.rs:1163` must become IMMEDIATE to make re-entrant append safe.
- The `sink_blocked` group's existing mutex-protected, fail-closed ordering is preserved; expanding its crash atomicity is outside D1+D2 scope.
- `record_github_grant` and the dual-connection same-seed head are both fixed by construction, provided every production append continues through the sole choke point.

## Verification

- All eleven required headings and every content anchor passed exact grep checks.
- Appendix A contains exactly 45 production rows and all three required in-transaction anchors.
- The gate record has status PENDING, six reviewer fields set to `TBD`, and an empty revision-history table.
- `./scripts/check-invariants.sh`: Gates 1–6 PASSED after both tasks and at plan close.
- `git diff --check`: PASSED.
- Committed plan range changes only the two planning documents; no path under `crates/` or `cli/` changed.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Authentication Gates

None.

## Known Stubs

None. The gate record's `TBD` reviewer values and empty revision history are intentional required state owned by Plan 51-08, not implementation stubs.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 51-07 may implement the coordinated D1+D2 repair exactly as pinned. The design gate is deliberately not cleared: Plan 51-08 must run an orchestrator-owned fresh non-self adversarial code trace against the landed implementation before clearance. LIVE evidence, completion ledgers, and the unchanged Plan 51-04 rerun remain outstanding and were not touched here.

## Self-Check: PASSED

- Created design document exists.
- Both task commits are reachable.
- Required headings, inventory totals, PENDING gate structure, zero-source-change boundary, and invariant Gates 1–6 were re-verified.

---
*Phase: 51-non-hybrid-live-proof-v1-10-done*
*Completed: 2026-08-05*
