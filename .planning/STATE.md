---
gsd_state_version: 1.0
milestone: v1.8
milestone_name: — Git/GitHub Adapters
current_phase: 40
current_phase_name: CLI Compose, Sidecar env_clear() & Composed Live Proof (v1.8 DONE)
status: planning
stopped_at: Phase 38 complete; Phase 39 (git.push) deferred to v1.9; advancing to Phase 40
last_updated: "2026-07-18T10:00:00.000Z"
last_activity: 2026-07-18
last_activity_desc: Phase 38 complete; git.push (Phase 39, GIT-02/03) deferred to v1.9 per DESIGN-gate BLOCKER-1 constraint; advanced to Phase 40
progress:
  total_phases: 5
  completed_phases: 4
  total_plans: 12
  completed_plans: 16
  percent: 80
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-18)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended (v1.2) with session-level draft-only demotion (I1/I0) and single-shot human confirmation, (v1.3, SHIPPED) with content-sensitive body blocking, a real broker-mediated SMTP send, and a composed live acceptance, (v1.4, SHIPPED) with coherent cross-connection trust state and a boundary proven indifferent to planner intelligence, (v1.5, SHIPPED) with a structural check that a value's semantic origin matches the semantic role of the slot it's routed into (closing the v1.4 T2 residual), (v1.6, SHIPPED) hardening the standing residuals that made several of those guarantees "true only incidentally" into enforced guarantees, (v1.7, SHIPPED) extending the set of real sinks — `process.exec` (captured+tainted command output) and filesystem read/write breadth, and now (v1.8) adding the external-effect sinks that make a coding agent's work durable and shareable — `git.commit`, `git.push`, `github.pr`, and read-only `http.request` egress — proving the Safe Coding Agent anchor end-to-end.
**Current focus:** Phase 35 — DESIGN Gate + Fresh Adversarial Code-Trace (DESIGN-15/16, `DESIGN-git-github-http-sinks.md` + fresh non-self adversarial code-trace)

## Current Position

Phase: 39 — `git.push` Sink
Plan: Not started
Status: Roadmapped, ready to plan Phase 35
Last activity: 2026-07-18 — Phase 38 complete, transitioned to Phase 39

## Performance Metrics

**Velocity:**

- Total plans completed: 115 (v1.0: 15 + v1.1: 15 + v1.2: 11 + v1.3: 21 + v1.4: 14 + v1.5: 8 + v1.6: 14 + v1.7: 17)
- Average duration: — min

*Updated after each plan completion. v1.7 (phases 31-34) shipped 2026-07-18 — 17/17 plans complete. v1.8 (phases 35-40) roadmapped 2026-07-18, no plans yet.*

## Accumulated Context

### Decisions

**v1.8 roadmap phase structure (`/gsd-roadmapper`, 2026-07-18):** 6 phases
(35-40), 15/15 requirements mapped, 0 orphans, 0 duplicates. Continues numbering
from v1.7's Phase 34 (does NOT reset). Mirrors this project's established
design-gate → implementation → live-proof precedent (v1.0 P2, v1.2 P8, v1.3 P12,
v1.4 P18, v1.5 P23, v1.6 P26, v1.7 P31 — each a standalone reviewed DESIGN doc
before any TCB code, followed by implementation, followed by a separate
live-proof phase). Dependency-forced ordering per research (`research/SUMMARY.md`,
HIGH confidence):

- **Phase 35** is the design gate (DESIGN-15/16 — `DESIGN-git-github-http-sinks.md`
  pinning effect-class per sink, `mint_from_http` inbound-taint + demotion, git
  config/hook neutralization, git.push destination pinning + credential-injection
  mechanism, the SSRF resolve-and-pin model, the github.pr human auth-grant model,
  the env_clear() TLS-cert allowlist policy, duplicate-PR CAS semantics, and new
  TaintLabel variants — closing all 11 design-gate-blocking pitfalls). HARD-BLOCKS
  Phases 36-40. The ORCHESTRATOR (not a gsd-executor) owns the review spawn. No
  `crates/executor`/`brokerd`/`sandbox`/`runtime-core` TCB code before this gate
  clears.

- **Phase 36** is `git.commit` alone (GIT-01) — lowest risk, reuses the v1.7
  `caprun-exec-launcher` + `mint_from_exec` pattern near-verbatim.

- **Phase 37** is `http.request` GET (HTTP-01..03) — establishes the NEW
  `mint_from_http` inbound-taint mechanism that `github.pr` reuses; must land
  before Phase 38.

- **Phase 38** is `github.pr` (GITHUB-01..04) — reuses the Phase-37 http egress
  infra + inbound mint; adds the bearer token, the new human auth-grant, and
  duplicate-PR CAS.

- **Phase 39** is `git.push` (GIT-02/03) — hardest: network-from-confined-child +
  push-credential injection; done after the credential boundary is proven by
  Phase 38's `github.pr`.

- **Phase 40** is CLI compose + sidecar env_clear() (ENV-01) + the composed live
  Linux proof with adversarial attack legs (LIVE-03/04). Mirrors v1.2 P11, v1.3
  P17, v1.4 P22, v1.5 P25, v1.6 P30, v1.7 P34. Depends on Phases 36-39 all
  landing.

### Blockers/Concerns

- Phases 36-39 (implementation) and Phase 40 (regression/live proof) are
  hard-blocked on Phase 35's DESIGN doc (`planning-docs/DESIGN-git-github-http-sinks.md`)
  clearing a fresh (non-self) adversarial code-trace. No `crates/executor` /
  `crates/brokerd` / `crates/sandbox` / `crates/runtime-core` TCB code before that
  gate.

- Phase 38 (`github.pr`) is hard-blocked on Phase 37 (`http.request`) landing
  first — `github.pr` reuses `mint_from_http`, which does not exist until Phase 37.

- Phase 39 (`git.push`) is hard-blocked on Phase 38 (`github.pr`) landing first —
  the push-credential-injection design is meant to be proven incrementally after
  the bearer-token boundary is proven by `github.pr`.

- The DESIGN doc must settle two genuine forks before it can clear review: the
  `git.push` network path (net-allowed confined child vs. in-broker git lib —
  reopens default-deny-net) and the rustls crypto provider (aws-lc-rs vs. ring).

### Standing GSD-tooling mitigations (carried forward)

- `phases.clear --confirm` deletes ALL prior phase dirs from disk (documented bug,
  4-for-4 across v1.3–v1.6 scoping) — git-status-check `.planning/phases/`
  immediately after any `phases.clear`; restore if needed.

- The last-wave executor's doc-completion commit has historically flipped
  ROADMAP.md's phase checkbox before verification (Phases 15/16) — never let ANY
  executor touch ROADMAP.md/STATE.md; the orchestrator owns phase-completion state.

## Session Continuity

Last session: 2026-07-18
Stopped at: v1.8 roadmap created (Phases 35-40)
Resume file: None

## Operator Next Steps

- Plan Phase 35 (the design gate) with `/gsd-plan-phase 35`.

## Deferred Items

Items acknowledged and deferred at the v1.7 milestone close (2026-07-18), re-reviewed at v1.8 roadmap creation (2026-07-18).

| Category | Item | Status |
|----------|------|--------|
| todo (security) | planner-sidecar-env-clear — env_clear() the caprun-planner sidecar spawn | in progress — directly in scope as v1.8 ENV-01 (Phase 40); close this todo once v1.8 ships |
| todo (security) | v1.3-phase16-v2-security-obligations — deferred v2 security obligations (recorded, not dropped) | pending |
| todo (tooling) | gsd-executors-must-not-write-phase-completion-state | pending (GSD process, not caprun product) |
| todo (tooling) | gsd-phases-clear-deletes-all-milestones | pending (GSD process, not caprun product) |
| uat | Phase 03 UAT — passed, 0 pending scenarios (stale audit flag, v1.0-era) | passed |
