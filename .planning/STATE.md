---
gsd_state_version: 1.0
milestone: v1.7
milestone_name: — Effect Breadth I
current_phase: 7
status: Awaiting next milestone
stopped_at: v1.7 roadmap created (Phases 31-34)
last_updated: "2026-07-18T05:19:58.701Z"
last_activity: 2026-07-18
last_activity_desc: Milestone v1.7 completed and archived
progress:
  total_phases: 4
  completed_phases: 4
  total_plans: 17
  completed_plans: 17
  percent: 100
current_phase_name: regression-live-proof-v1-7-done
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-18)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended (v1.2) with session-level draft-only demotion (I1/I0) and single-shot human confirmation, (v1.3, SHIPPED) with content-sensitive body blocking, a real broker-mediated SMTP send, and a composed live acceptance, (v1.4, SHIPPED) with coherent cross-connection trust state and a boundary proven indifferent to planner intelligence, (v1.5, SHIPPED) with a structural check that a value's semantic origin matches the semantic role of the slot it's routed into (closing the v1.4 T2 residual), (v1.6, SHIPPED) hardening the standing residuals that made several of those guarantees "true only incidentally" into enforced guarantees, and now (v1.7) extending the set of real sinks — `process.exec` (captured+tainted command output) and filesystem read/write breadth — each through the same plan-node → taint → executor(I2) → audit path, toward the Safe Coding Agent anchor.
**Current focus:** v1.7 shipped (2026-07-18) — planning next milestone (v1.8 — Git/GitHub Adapters)

## Current Position

Phase: Milestone v1.7 complete
Plan: —
Status: Awaiting next milestone
Last activity: 2026-07-18 — Milestone v1.7 completed and archived

## Performance Metrics

**Velocity:**

- Total plans completed: 64 (v1.0: 15 + v1.1: 15 + v1.2: 11 + v1.3: 21 + v1.4: 14 + v1.5: 8 + v1.6: 14)
- Average duration: — min

*Updated after each plan completion. v1.6 (phases 26-30) shipped 2026-07-17 — 14/14 plans complete. v1.7 (phases 31-34) roadmapped 2026-07-17, no plans yet.*

## Accumulated Context

### Decisions

**v1.7 roadmap phase structure (`/gsd-roadmapper`, 2026-07-17):** 4 phases
(31-34), 11/11 requirements mapped, 0 orphans, 0 duplicates. Continues numbering
from v1.6's Phase 30 (does NOT reset). Mirrors this project's established
design-gate → implementation → live-proof precedent (v1.0 P2, v1.2 P8, v1.3 P12,
v1.4 P18, v1.5 P23, v1.6 P26 — each a standalone reviewed DESIGN doc before any
TCB code, followed by implementation, followed by a separate live-proof phase):

- **Phase 31** is the design gate (DESIGN-13/14 — `DESIGN-effect-breadth-exec.md`
  pinning the broker-spawned confined-child-`exec` model + the fs read/write-breadth
  model + fail-closed defaults for both new sinks, cleared by a fresh non-self
  adversarial code-trace). HARD-BLOCKS Phases 32-34. `process.exec` under
  Landlock+seccomp is the riskiest primitive to date; the ORCHESTRATOR (not a
  gsd-executor) owns the review spawn. No `crates/executor`/`brokerd`/`sandbox`/
  `runtime-core` TCB code before this gate clears.

- The 7 implementation requirements split into **2 implementation phases by blast
  radius / subsystem coherence** rather than one bundled phase or seven
  single-requirement phases: **Phase 32** is `process.exec` alone (EXEC-01..04) —
  a genuinely new confined-child spawn path in the broker + sandbox, the riskiest
  primitive, substantial enough for its own phase. **Phase 33** is filesystem
  breadth (FS-01..03) — the adapter-fs fd-passing seam (read many files + write/
  edit existing files beyond `file.create`'s `O_EXCL`). Independent of each other;
  both depend only on Phase 31.

- **Phase 34** is the regression/live-proof phase (LIVE-01/02 — composed
  acceptance on real Linux: exec-tainted Block + clean Allow + fs write/edit
  audited, genuine non-stapled taint chain, `verify_chain` true; full-workspace
  regression green with no regression to v1.0–v1.6, true-exit-before-pipe, a
  dedicated negative test per new sink). Mirrors v1.2 P11, v1.3 P17, v1.4 P22,
  v1.5 P25, v1.6 P30. Depends on Phases 32 AND 33 both landing.

### Blockers/Concerns

- Phases 32, 33 (implementation) and Phase 34 (regression/live proof) are
  hard-blocked on Phase 31's DESIGN doc (`planning-docs/DESIGN-effect-breadth-exec.md`)
  clearing a fresh (non-self) adversarial code-trace. No `crates/executor` /
  `crates/brokerd` / `crates/sandbox` / `crates/runtime-core` TCB code before that gate.

- `process.exec` fundamentally changes the confinement model (a new
  broker-spawned confined-child spawn path) — this is why v1.7 opens with a design
  gate + adversarial review rather than a bare "add a sink" plan.

### Standing GSD-tooling mitigations (carried forward)

- `phases.clear --confirm` deletes ALL prior phase dirs from disk (documented bug,
  4-for-4 across v1.3–v1.6 scoping) — git-status-check `.planning/phases/`
  immediately after any `phases.clear`; restore if needed.

- The last-wave executor's doc-completion commit has historically flipped
  ROADMAP.md's phase checkbox before verification (Phases 15/16) — never let ANY
  executor touch ROADMAP.md/STATE.md; the orchestrator owns phase-completion state.

## Session Continuity

Last session: 2026-07-17
Stopped at: v1.7 roadmap created (Phases 31-34)
Resume file: None

## Operator Next Steps

- Start the next milestone with /gsd-new-milestone

## Deferred Items

Items acknowledged and deferred at the v1.7 milestone close (2026-07-18). None are v1.7 requirements; all are pre-existing tech debt or a v1.8 follow-up. Closeout: override_closeout (all 31-34 phases verified passed; deferrals below).

| Category | Item | Status |
|----------|------|--------|
| todo (security) | planner-sidecar-env-clear — env_clear() the caprun-planner sidecar spawn | pending (v1.8; lower risk — trusted code, TLS-env regression risk) |
| todo (security) | v1.3-phase16-v2-security-obligations — deferred v2 security obligations (recorded, not dropped) | pending |
| todo (tooling) | gsd-executors-must-not-write-phase-completion-state | pending (GSD process, not caprun product) |
| todo (tooling) | gsd-phases-clear-deletes-all-milestones | pending (GSD process, not caprun product) |
| uat | Phase 03 UAT — passed, 0 pending scenarios (stale audit flag, v1.0-era) | passed |
