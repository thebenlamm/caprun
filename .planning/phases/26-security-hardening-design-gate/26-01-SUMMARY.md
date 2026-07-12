---
phase: 26-security-hardening-design-gate
plan: 01
subsystem: infra
tags: [design-gate, security, tcb, audit-chain, hmac, cas, cargo-features, slot-type, session-trust]

# Dependency graph
requires:
  - phase: 23-slot-type-binding-design-gate
    provides: DESIGN-slot-type-binding.md — the format/rigor precedent (decisions-not-options, §-per-mechanism, Adversarial-Review-Preemption, Accepted Residual Risks, Acceptance Predicate) this doc mirrors
  - phase: 18-session-trust-state (v1.4)
    provides: DESIGN-session-trust-state.md:80-81 anti-self-declaration clause that §a/D-02 reconciles and amends
provides:
  - "DESIGN-security-hardening.md — pins mechanism + fail-closed default for all five v1.6 TCB-local residuals (HARDEN-01..05)"
  - "Three cross-cutting rulings (X-01 label continuity, X-02 shared-store recovery authority, X-03 TOCTOU) pinned as one uniform rule each"
  - "Explicit X-04 ruling: Planner-connection session_status staleness folded into HARDEN-01 (shared Arc<Mutex<SessionStatus>>)"
  - "Phase 27-30 Implementation Map, Proof-plan note, and Acceptance Predicate"
affects: [26-02 (adversarial review / DESIGN-12), 27-session-conn-integrity, 28-authenticated-audit-chain, 29-sink-path-hardening, 30-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Design-gate discipline: decisions-not-options doc hard-blocks downstream TCB code until a fresh non-self review clears"
    - "Every § anchored to a re-verified file:line so downstream phases are mechanical realizations"
    - "Reuse of shipped precedents: executor's test-fixtures Cargo feature (§d), confirmation.rs SEND-01 CAS (§c), planner_slot_occupied Arc<AtomicBool> (§f/X-04)"

key-files:
  created:
    - planning-docs/DESIGN-security-hardening.md
  modified: []

key-decisions:
  - "§b key custody: stable cross-process HMAC key in a sibling <audit_path>.key file outside the workspace root (0600), read by both caprun and caprun confirm/deny — resolves the single-shot-per-session multi-process verify_chain problem"
  - "§c idempotency key = SHA256(sink.0 || sorted(arg_name, value_id) pairs); effect_id is minted fresh per call (server.rs:562) and is USELESS as a replay key"
  - "§d D-10 negative gate = a featureless-build BEHAVIORAL negative test (no opt-in) as primary; binary symbol-inspection only as optional defense-in-depth"
  - "§e expected_role(file.create, contents) = Some(&[\"path\"]) — the ONLY value ever bound to contents today is the reused path-role handle; any other list regresses s9_live_file_create_clean_allow"
  - "§b pending_confirmations FOLDED into the MAC scheme (not accepted as a second residual)"
  - "§f X-04 (Planner-connection session_status staleness) FOLDED into HARDEN-01 as a required Phase-27 fix (shared Arc<Mutex<SessionStatus>> re-read per dispatch), NOT accepted as a residual; no new HARDEN-0X id minted"

patterns-established:
  - "Cross-cutting rulings get a dedicated §f with one uniform rule each, so a fresh reviewer has a clean target"
  - "A NEW research-surfaced finding (X-04) gets its own named ruling subsection rather than being silently inherited"

requirements-completed: [DESIGN-11]

coverage:
  - id: D1
    description: "DESIGN-security-hardening.md exists and pins mechanism + fail-closed default for all five residuals (§a-§e), grounded in re-verified file:line anchors"
    requirement: DESIGN-11
    verification:
      - kind: automated_ui
        ref: "grep '## §a'..'## §e' + 'Some(&[\"path\"])' + 'fail-closed' in planning-docs/DESIGN-security-hardening.md"
        status: pass
    human_judgment: false
  - id: D2
    description: "Doc clears a fresh non-self adversarial review with every finding resolved (DESIGN-12)"
    requirement: DESIGN-12
    verification: []
    human_judgment: true
    rationale: "DESIGN-12 is satisfied by Plan 26-02 (the fresh adversarial review), not by this authoring plan. This SUMMARY covers DESIGN-11 only."

# Metrics
duration: 18min
completed: 2026-07-12
status: complete
---

# Phase 26 Plan 01: Security Hardening Design Gate Summary

**DESIGN-security-hardening.md pins the mechanism + fail-closed default for all five v1.6 TCB-local residuals (demote-at-RequestFd, keyed-MAC audit chain, Allowed-path replay CAS, compile-out forced-Active mint, file.create contents slot), three cross-cutting rulings, and an explicit fold-not-accept ruling on the newly-surfaced Planner-connection session_status staleness (X-04) — every § anchored to a re-verified file:line.**

## Performance

- **Duration:** ~18 min
- **Tasks:** 2
- **Files modified:** 1 created (design doc); 0 code files

## Accomplishments

- Authored `planning-docs/DESIGN-security-hardening.md` (801 lines, 13 sections: §0, §a–§j, Acceptance Predicate, Amendments) mirroring the v1.5 gate doc's decisions-not-options rigor.
- Pinned all five residual mechanisms with a fail-closed default and a re-verified current-code anchor each — including the load-bearing `expected_role(file.create, "contents") => Some(&["path"])` pin that protects the only live `file.create` clean-allow flow.
- Answered the single hardest open question (§b HARDEN-02 key-custody across the `caprun` → `caprun confirm` process boundary) with a concrete pinned source: a sibling `<audit_path>.key` file outside the workspace root.
- Ruled explicitly on the RESEARCH-surfaced NEW finding (X-04 Planner-connection `session_status` staleness): folded into HARDEN-01 as a required Phase-27 fix via a shared `Arc<Mutex<SessionStatus>>`, mirroring the existing `planner_slot_occupied` pattern.
- Incorporated all four RESEARCH corrections: (i) `Some(&["path"])`; (ii) `workspace_rel` new-plumbing budget; (iii) X-04 ruling; (iv) X-02 reframed to the confirm/deny process + `pending_confirmations`.

## Task Commits

1. **Task 1: §0 + §a–§e (five residual mechanisms)** - `f2ca17f` (docs)
2. **Task 2: §f–§j + closing predicate** - `2065d7d` (docs)

**Plan metadata:** (this SUMMARY + STATE/ROADMAP) committed separately by the closing step.

## Files Created/Modified

- `planning-docs/DESIGN-security-hardening.md` - The v1.6 design-gate deliverable (DESIGN-11). Pins mechanism + fail-closed default for HARDEN-01..05, the X-01/X-02/X-03 cross-cutting rulings, the X-04 fold-ruling, an Adversarial-Review-Preemption §, an Accepted Residual Risks §, the Phase 27-30 Implementation Map, a Phase-30 Proof-plan note, the Acceptance Predicate, and an empty Amendments header for Plan 26-02.

## Decisions Made

All mechanism directions were locked by CONTEXT.md (D-01..D-12, X-01..X-03); this plan pinned the discretionary specifics the research narrowed:
- **§b key custody** — sibling `<audit_path>.key` (0600) outside the workspace root, read by both processes; chosen over a per-process key (breaks `confirm()`'s `verify_chain` gate) or a key inside the DB/workspace (defeats D-04 / worker-Landlock exposure).
- **§b `pending_confirmations`** — folded into the MAC scheme (recommended over naming a second residual).
- **§c idempotency key** — derived from `SHA256(sink.0 || sorted(arg_name, value_id))` at `value_id` scope (correct D-08 per-plan-node scope), because `effect_id` is minted fresh per call.
- **§c CAS idiom** — `sent_plan_nodes` PRIMARY-KEY-constraint-violation-as-signal (distinct from `transition_state`'s `UPDATE ... WHERE`).
- **§d negative gate** — featureless-build behavioral test as primary; symbol-inspection as optional only.
- **§f X-04** — fold into HARDEN-01 (option a), no new requirement id.

## Deviations from Plan

None - plan executed exactly as written. All four RESEARCH corrections were pre-specified by the plan and incorporated; no auto-fixes (Rules 1-3) or architectural escalations (Rule 4) were needed. This is a doc-only plan; `git status --porcelain crates/ cli/` is empty.

## Issues Encountered

- The advisor tool was unavailable this session. Per project convention it is not required for this authoring plan; the fresh non-self adversarial review (DESIGN-12) is Plan 26-02's job and will use the standing `Agent(model:"fable")` fallback.

## Self-Check: PASSED

- `planning-docs/DESIGN-security-hardening.md` — FOUND (801 lines).
- Task 1 automated verify (`## §a`..`## §e` + `Some(&["path"])` + `fail-closed`) — PASS.
- Task 2 automated verify (`## §f`, `X-04`, `Adversarial-Review`, `Accepted Residual`, `Implementation Map`, `Acceptance Predicate`, `Amendments`) — PASS.
- Section-marker count (`grep -c '^## §'`) = 11 (≥ 6 required).
- Commits `f2ca17f`, `2065d7d` — FOUND in git log.
- `git status --porcelain crates/ cli/` — empty (no code written).
- `./scripts/check-invariants.sh` — all gates PASSED (run pre-authoring; doc-only plan touches no gated tokens).

## Next Phase Readiness

- **Plan 26-02 (DESIGN-12):** ready — the doc carries an empty `## Amendments (post-review)` header for round-tagged review amendments, and the Acceptance Predicate names the `DESIGN-GATE-RECORD-v1.6.md` clearance as the final condition. The highest-stakes review target is flagged in §g probe 2 (§b key custody, RESEARCH Assumption A2).
- **Phases 27-30:** each residual's blast-radius note + the §i Implementation Map give a mechanical file:line map; each phase must re-verify anchors if commits intervene.

---
*Phase: 26-security-hardening-design-gate*
*Completed: 2026-07-12*
