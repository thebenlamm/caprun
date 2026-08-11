---
gsd_state_version: 1.0
milestone: v1.10
milestone_name: Multi-step Safe Coding Agent Loop
status: Awaiting next milestone
stopped_at: v1.10 complete — run /gsd-new-milestone
last_updated: "2026-08-11T18:42:06.388Z"
last_activity: 2026-08-11
last_activity_desc: Milestone v1.10 archived and tagged
progress:
  total_phases: 7
  completed_phases: 7
  total_plans: 23
  completed_plans: 23
current_phase: null
current_phase_name: —
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-11)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended through v1.9 with authorized write egress, minimal per-session policy (never overrides I2), and thin CLI/audit surfaces, and through v1.10 with a single-Session CLI-driven multi-node Safe Coding Agent path — without weakening I0/I1/I2 or adding any raw `EffectRequest` path.
**Current focus:** Planning next milestone (`/gsd-new-milestone`)

## Current Position

Phase: — (v1.10 complete)
Plan: —
Status: Awaiting next milestone
Last activity: 2026-08-11 — Milestone v1.10 archived (override_closeout)

## Performance Metrics

**Velocity:**

- Total plans completed: 177 (v1.0: 15 + v1.1: 15 + v1.2: 11 + v1.3: 21 + v1.4: 14 + v1.5: 8 + v1.6: 14 + v1.7: 17 + v1.8: 17 + v1.9: 22 + v1.10: 23)
- Average duration: — min

*Updated after each plan completion. v1.9 (phases 41-46) shipped 2026-07-18. v1.10 (phases 47-52) roadmap created 2026-07-23.*
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 47 P01 | 4min | 2 tasks | 1 files |
| Phase 48 P02 | 5min | 2 tasks | 5 files |
| Phase 49 P01 | 12min | 2 tasks | 9 files |
| Phase 49 P02 | 4min | 2 tasks | 5 files |
| Phase 50 P01 | 5min | 2 tasks | 4 files |
| Phase 50 P02 | 6min | 2 tasks | 4 files |
| Phase 51 P01 | 18min | 1 tasks | 2 files |
| Phase 51 P02 | 12min | 2 tasks | 8 files |
| Phase 51.1 P01 | 35min | 3 tasks | 5 files |
| Phase 51.1 P02 | 12min | 2 tasks | 1 files |
| Phase 51.1 P03 | 8min | 2 tasks | 3 files |

## Accumulated Context

### Decisions

**v1.10 roadmap phase structure (`/gsd-roadmapper`, 2026-07-23):** 6 phases
(47-52), 13/13 requirements mapped, 0 orphans, 0 duplicates. Continues numbering
from v1.9's Phase 46 (does NOT reset). Mirrors this project's unbroken
design-gate → substrate → feature → product surface → live-proof precedent
(v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26, v1.7 P31, v1.8 P35,
v1.9 P41). Research (`research/SUMMARY.md`) informs order; requirements drive
coverage. Granularity: standard (6 phases). CONFIRM-01 folded into the CLI phase
(Phase 50) because mid-loop Block-and-Hold is the product path for always-confirm
`git.push` and is too thin as a standalone single-req phase; packaging stays
separate as a distinct design-partner deliverable.

- **Phase 47** is the design gate (DESIGN-19/20 + HYG-02) —
  `DESIGN-multi-step-plan-stream.md` pins stream shape, handle bag, Block-and-Hold,
  I1×coding-loop bounds, instruction vs value channels, deny/abort semantics, and
  zero-new-crate hygiene. HARD-BLOCKS Phases 48-52. The ORCHESTRATOR (not a
  gsd-executor) owns the fresh non-self adversarial-trace spawn. The trace re-runs
  if stream shape, confirm-hold, or trusted-arg mint path changes mid-implementation.
  No multi-step TCB code in `crates/{executor,brokerd,sandbox,runtime-core}` or the
  worker submit/confirm-hold path before this gate clears.

- **Phase 48** is plan-stream substrate (STREAM-01/02) — worker sequential
  multi-submit + opaque `output_value_id` handle bag + chain-head continuity.
  Broker multi-submit is already legal; this is the worker loop + handle discipline
  before a coding recipe piles nodes on. First TCB/worker code phase.

- **Phase 49** is the deterministic multi-step coding planner (CODE-01/02) —
  scripted edit→test→commit→push→PR over shipped sinks; trusted-intent args only on
  the success path; email/file single-node paths remain green. No LLM multi-step.

- **Phase 50** is CLI multi-node driver + mid-loop confirm continuity
  (CLI-01/02, CONFIRM-01) — `caprun run` drives the coding chain; honest stop
  semantics/exit codes; Block-and-Hold same Session for always-confirm `git.push`
  and I2 Block confirm-release; no dual-Session stitch; no session-wide waiver.
  ON THE ACCEPTANCE CRITICAL PATH (LIVE-07 requires CLI-driven multi-node).

- **Phase 51** is the non-hybrid LIVE proof (LIVE-07/08) — the v1.10 DONE gate.
  CLI-driven success path full chain + mid-loop I2 Block with genuine taint;
  `verify_chain` true; framing honesty (no hybrid DONE claim); full-workspace
  regression via compose-verify. Mirrors v1.2 P11, v1.3 P17, v1.4 P22, v1.5 P25,
  v1.6 P30, v1.7 P34, v1.8 P40, v1.9 P46.

- **Phase 52** is minimal Linux packaging (PKG-01) — co-located three sibling
  bins + env/credential checklist; thin install script OK; not cargo-dist/deb/snap.
  Ships after LIVE so install path matches proven binary layout.

- [Phase ?]: Stream shape = sequential multi-node on existing Planner seam; reject batch DAG
- [Phase ?]: Handle bag opaque ValueIds only; ProvideIntent exactly once; planner never mints
- [Phase ?]: Block-and-Hold same Session; reject reconnect-remint and dual-Session stitch
- [Phase ?]: Trusted-intent success path; no CommitIrreversible Draft weaken
- [Phase ?]: DESIGN-20 orchestrator-owned non-self; re-runs on stream/confirm/mint pivots
- [Phase ?]: HYG-02 default zero new crates and zero new mint sites
- [Phase ?]: Host stream proofs use pure drive_stream harness aligned with worker branch table (binary has no lib target)
- [Phase ?]: Linux taint-via-bag is hybrid in-crate multi-node with bag intermediate — substrate not LIVE-07 CLI DONE
- [Phase ?]: SafeCodingWorkflow closed variant with 13 operator fields + named_handles multi-mint (CODE-01/02)
- [Phase ?]: Coding worker skips RequestFd/claim demotion; success-path plan_next never places out_*
- [Phase ?]: Test-only CodingI2ProofPlanner for LIVE-08 expressibility; production DeterministicPlanner never places out_*
- [Phase ?]: CaprunIntent SafeCodingWorkflow is add-alongside closed enum (not promote/replace email/file)
- [Phase ?]: Hold only for SafeCodingWorkflow; email/file Block → exit 3
- [Phase ?]: Parent-pipe hold protocol only — no reconnect-remint, dual-Session, or broker Wait verb
- [Phase ?]: CLI-02 exit taxonomy 0/2/3/1; policy_deny distinguished via DENIED code= field
- [Phase ?]: Interactive mid-loop confirm primary; CAPRUN_CONFIRM=external or non-TTY → dual-terminal poll
- [Phase ?]: PROCEED only after ConfirmOutcome::Released or durable confirmed; sink-fail → exit 1
- [Phase ?]: Phase 50 does not claim LIVE-07/08 SUCCESS — Phase 51 owns LIVE
- [Phase ?]: Phase 51: Forward brokerd/mock-egress-ca through caprun so crate-level LIVE cfg gates execute.

### Blockers/Concerns

- Phases 48-52 are hard-blocked on Phase 47's DESIGN doc clearing a fresh
  (non-self, ORCHESTRATOR-owned) adversarial code-trace. No multi-step TCB /
  worker submit/confirm-hold code before that gate.

- **Primary adversarial-trace risks (from research):** cross-node taint laundering
  via `output_value_id`; ProvideIntent reopened as mid-stream trust valve; Draft
  demotion "fixed" by weakening CommitIrreversible rules; hybrid composition
  rebranded as CLI multi-step; mid-loop confirm splitting Sessions. Mitigations
  locked at design gate: ProvideIntent once, handle-only planner, per-node I2,
  trusted-intent success path, Block-and-Hold same Session, LIVE DONE requires
  real multi-node `caprun run`.

- **HYG-02:** default zero new crates; Gate 1/3 discipline; compose-verify remains
  authoritative Linux gate. Mapped to Phase 47 as design-locked constraint;
  re-asserted in LIVE success criteria.

- **Carried non-blocking (not v1.10 scope):** git.push 10MB pack-cap (fails closed);
  leg-5b scrub-branch hardening; LLM multi-step tool-use (future).

- **BLOCKING v1.10 DONE (found 2026-08-04 by executing 51-04 on real Linux):**
  **D1** — the audit hash chain FORKS after any external append. `handle_connection`
  threads `last_event_id`/`last_event_hash` in memory (`server.rs:547-548`) and never
  re-reads the durable head, while `caprun grant`/`confirm` append off a fresh
  `current_chain_head` (`audit.rs:548`, `confirmation.rs:852`). Deterministic, not a
  race — Block-and-Hold guarantees the ordering. `verify_chain` correctly returns
  false. **`verify_chain` must NOT be loosened** — the chain is linear by design
  (`audit.rs:1203-1219`); the "DAG" in PLAN.md is the provenance/taint edge graph, not
  the event `parent_id` chain, and loosening deletes HARDEN-02 tail-truncation
  detection. The append path is the bug. **D2** — the append is not transactional
  (no `BEGIN IMMEDIATE`); must be designed WITH D1, because a bare `busy_timeout`
  converts D1's crash into a successfully committed fork. Closed by plans 51-05..51-08.
  **v1.9 is NOT affected** — verified against the tag (`stream_hold.rs` absent; no
  `CAPRUN_CONFIRM`/external handling; no `submit_plan_node`). Two latent same-class
  instances exist, unreachable in any shipped flow: the dual-connection same-seed head
  (`server.rs:307-382`) and `record_github_grant` (`audit.rs:546-561`).

### Standing GSD-tooling mitigations (carried forward)

- `phases.clear --confirm` deletes ALL prior phase dirs from disk (documented
  bug, 5-for-5 across v1.3–v1.8 scoping) — git-status-check `.planning/phases/`
  immediately after any `phases.clear`; restore if needed.

- The last-wave executor's doc-completion commit has historically flipped
  ROADMAP.md's phase checkbox before verification — never let ANY executor touch
  ROADMAP.md/STATE.md; the orchestrator owns phase-completion state.

- The DESIGN-gate adversarial-trace spawn is ORCHESTRATOR-owned, not a
  gsd-executor (fresh, non-self) — the [[fresh-context-adversarial-review]]
  guardrail that has caught real BLOCKER/MAJOR defects through v1.9.

### Roadmap Evolution

- Phase 51.1 inserted after Phase 51: Grant/Audit Atomicity (CR-01) — record_github_grant can durably authorize github.pr with no authorization event in the tamper-evident chain (URGENT)

## Session Continuity

Last session: 2026-08-11
Stopped at: v1.10 complete — archived
Resume file: —

## Operator Next Steps

- Run `/gsd-new-milestone` to define the next version (questioning → research → requirements → roadmap).
- Fresh `REQUIREMENTS.md` is created by that workflow after this close deletes the v1.10 file.

## Deferred Items

Items acknowledged and deferred at milestone close on 2026-08-11 (`override_closeout`), plus carried residuals.

| Category | Item | Status |
|----------|------|--------|
| debug | knowledge-base | acknowledged at v1.10 close (GSD debug knowledge base file; not an open investigation) |
| todo (tooling) | gsd-phases-clear-deletes-all-milestones | acknowledged at v1.10 close — pending (GSD process) |
| todo (tooling) | gsd-executors-must-not-write-phase-completion-state | acknowledged at v1.10 close — pending (GSD process) |
| todo (security) | v1.3-phase16-v2-security-obligations — deferred v2 security obligations | acknowledged at v1.10 close — pending |
| functional (caprun) | git.push `generate_pack` 10 MB pack-cap — fails CLOSED (safe) but blocks large-repo pushes | pending — non-blocking residual (PUSH-CAP-01) |
| test-hardening (caprun) | LIVE-06 leg-5b scrub-branch hardening on push error path | pending — optional (SCRUB-01) |
| future | LLM multi-step / ReAct tool-use loop on v1.4 sidecar | deferred past v1.10 (LLM-MS-01) |
| future | github.pr merge/comment, richer coding recipes, replan-from-observation | deferred (CODE-BREADTH-01) |
| future | Broader packaging (deb/snap/cargo-dist), Mac best-effort install | deferred (PACK-02) |
| verification | Phase 47 manager-stale verification flag | override at v1.10 close — milestone audit scored 47 passed 4/4 |
| packaging residual | concurrent/interrupted install can leave mixed binaries; clean-machine + cold-reader walkthroughs host-bound | accepted at Phase 52 / milestone audit |

