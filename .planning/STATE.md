---
gsd_state_version: 1.0
milestone: v1.4
milestone_name: — Trust-Boundary Integrity & the Adversarial Planner
current_phase: 19
current_phase_name: Cross-Connection Trust Coherence Fix
status: executing
stopped_at: "ROADMAP.md + REQUIREMENTS.md traceability written for v1.4 (5 phases: 18-22)"
last_updated: "2026-07-11T03:28:51.418Z"
last_activity: 2026-07-11
last_activity_desc: Phase 18 complete, transitioned to Phase 19
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 2
  completed_plans: 2
  percent: 20
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-10)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended (v1.2) with session-level draft-only demotion (I1/I0) and single-shot human confirmation, (v1.3, SHIPPED) with content-sensitive body blocking, a real broker-mediated SMTP send, and a composed live acceptance, and now (v1.4) with coherent cross-connection trust state and a boundary proven indifferent to planner intelligence.
**Current focus:** Phase 18 — Trust-Boundary Coherence Design Gate

## Current Position

Phase: 19 — Cross-Connection Trust Coherence Fix
Plan: Not started
Status: Executing Phase 18
Last activity: 2026-07-11 — Phase 18 complete, transitioned to Phase 19

## Performance Metrics

**Velocity:**

- Total plans completed: 27 (v1.0: 15 + v1.1: 15 + v1.2: 11 + v1.3: 21 [Phases 12-17])
- Average duration: — min

**By Phase (v1.2):**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 8 | 3 | - | - |
| 9 | 4 | - | - |
| 10 | 3 | - | - |
| 11 | 1 | - | - |
| 13 | 4 | - | - |
| 14 | 2 | - | - |
| 15 | 4 | - | - |
| 16 | 4 | - | - |
| 18 | 2 | - | - |

*Updated after each plan completion. v1.3 (phases 12-17) shipped 2026-07-09 — 21/21 plans complete. v1.4 (phases 18-22) roadmapped 2026-07-10, no plans yet.*
| Phase 14 P02 | 50min | 3 tasks | 10 files |
| Phase 15-deterministic-doc-action-extraction P01 | 75min | 3 tasks | 3 files |
| Phase 15-deterministic-doc-action-extraction P02 | 11min | 3 tasks | 3 files |
| Phase 15-deterministic-doc-action-extraction P03 | 55min | 2 tasks | 3 files |
| Phase 15 P04 | ~2h10min | 3 tasks | 11 files |
| Phase 16 P01 | 40min | 2 tasks | 8 files |
| Phase 16 P02 | 50min | 2 tasks | 3 files |
| Phase 16 P03 | 25min | 1 tasks | 1 files |
| Phase 16 P04 | 3h | 3 tasks | 16 files |

## Accumulated Context

### Decisions

v1.4 scoping decisions (fix shape = reject 2nd connection, MAJOR-2 replay
re-earned in writing not new CAS, T2 deferred to v1.5) are recorded in
`.planning/PROJECT.md`'s Key Decisions table and `.planning/REQUIREMENTS.md`.

**Roadmap phase structure (`/gsd-roadmapper`, 2026-07-10):** 5 phases (18-22),
19/19 requirements mapped, 0 orphans. Phase 0 (the fix) splits into two
phases mirroring this project's established design-gate/implementation
precedent (v1.0 Phase 2, v1.2 Phase 8, v1.3 Phase 12 — each a standalone
reviewed DESIGN doc before any TCB code): **Phase 18** is the design gate
(DESIGN-01..06 — `DESIGN-session-trust-coherence.md` + fresh adversarial
panel), **Phase 19** is the fix itself (TRUST-01..03, DOC-02 — broker rejects
a 2nd connection, `two_connection_intent_bypass_repro` goes green by fixing
the broker not weakening the test, `mailpit-verify.sh` independently re-run
green, PROJECT.md's DOC-02 correction finalized). Phase 1+ (planner) follows
the "seam design → implementation → adversarial proof" shape named at
scoping: **Phase 20** designs and introduces the `Planner` trait/seam +
per-verb capability split + co-location boundary (PLANNER-01/02/04),
**Phase 21** builds the adversarial LLM planner itself on that seam
(PLANNER-03), and **Phase 22** is the live HARD GATE proof plus the T2
residual disclosure (GATE-01..04, T2-01) — mirroring the project's separate
live-acceptance phases (v1.2 Phase 11, v1.3 Phase 17). Phase 18 is a hard
gate: no `server.rs` code for the trust-coherence fix may be written before
its DESIGN doc clears a fresh (non-self) adversarial review.

- [Phase 14]: blocked_literals gained an arg column and composite (event_id, arg) PRIMARY KEY so a plural sink_blocked event can persist every blocked arg's literal, not just the first. — The plan required iterating all anchors and writing every literal; the prior single-column event_id PK would PK-collide on a 2nd insert for a genuinely-plural block.
- [Phase 14]: render_block_display's plural-block fail-closed guard re-derives the executor's is_routing_sensitive||is_content_sensitive && tainted predicate over PendingConfirmation.resolved_args instead of threading a new field through PendingConfirmation. — Avoids a schema/struct ripple across every PendingConfirmation-constructing test fixture while giving a precise (no false-positive) plurality check, since brokerd already depends on the executor crate.
- [Phase ?]: 15-01: mint_from_derivation mints the ValueRecord before constructing the derivation Event (reverse of mint_from_read's order), since the event's hashed payload embeds derived_value_id == the minted value_id
- [Phase ?]: 15-01: check-invariants.sh Gate 3 exempts files under tests/ and #[cfg(test)] modules (in addition to the 3 named allowed loci) to avoid false-flagging pre-existing legitimate test infrastructure that already calls mint_from_read/ValueStore::mint directly
- [Phase ?]: 15-02: assert_unbroken_edge/genuine_derivation_binds implemented as reusable Result-returning routines shared by the positive per-anchor walk and both anti-staple negative controls (finding #2/#10/#12 identity-bound predicate, no-LIMIT derivation scan)
- [Phase ?]: 15-03: mint_from_read Err surfaced as BrokerResponse::Error on the wire (not connection-killing ?) for the whole ReportClaims arm, not just DocFragment — Fail-closed for attacker-controlled input; no behavior change for EmailAddress/RelativePath (never actually fail today)
- [Phase ?]: 15-03: ReportDerivedClaim resolves input handles to owned ValueRecord clones before calling mint_from_derivation — Avoids simultaneous mutable+immutable ValueStore borrow, per mint_from_derivation's own documented calling convention
- [Phase ?]: 15-04: plan_from_intent gains trusted_subject_handle/trusted_body_handle params (6 total) beyond the must_haves' literally-quoted 4-param signature — resolves an internal plan-text inconsistency; needed for finding #6's genuinely-distinct handles under PLAN-03
- [Phase ?]: 15-04: dag_chain_integrity corrected to 6 benign events (not the plan-specified 4) after empirical Colima/Docker verification exposed that Task 3's three sequential mint_from_intent calls each append their own intent_received event
- [Phase ?]: 15-04: s9_live_email_hostile_block added as a new live test to satisfy the plan's own verification line requiring live email-BLOCK coverage, not explicitly named in the task action text
- [Phase ?]: combined_digest binds sha256(name)‖sha256(literal) per element over the FULL resolved_args set (blocked+trusted), byte-wise-ascending arg_name order, per DESIGN Round-6 amendment
- [Phase ?]: Migration gated on PRAGMA table_info presence check, not blind ALTER TABLE + error-catch
- [Phase 16]: T-14-08 two-commit discipline: proved the plurality guard panics (commit 1f3336b) before replacing it with full ALL-args narration (commit b61e043)
- [Phase 16]: confirm()'s DigestMismatch leaves the row Pending (integrity alarm, not an operator deny) so an attacker triggering a mismatch cannot force-terminate a retriable confirmation
- [Phase 16]: verify_chain's scope recorded honestly: detects single-store/non-recomputing-multi-store tampering only, NOT authenticated/externally-anchored -- chain-head-not-anchored is an Accepted Residual Risk with a v2 obligation
- [Phase 16]: CONTROL-02 live fixture models CLEAN_PATH_CONTENT's no-marker recipient side + a Body: marker only, verified against worker.rs's extraction branch directly before writing the test, to guarantee no accidental recipient taint (Pitfall 5).
- [Phase ?]: 16-04: All three BLOCKER-1 guards (ProvideIntent ordering, non-live-state Deny, CreateSession IPC opt-in) landed in Task 1, strictly before Task 2's email.send Allowed-dispatch, so the dispatch never exists without its guards.
- [Phase ?]: 16-04: CreateSession IPC arm gated behind CAPRUN_ENABLE_IPC_CREATE_SESSION == exactly "1" (runtime opt-in, fail-closed default-deny) instead of cfg(test), which is unset when brokerd compiles as an integration-test dependency.
- [Phase ?]: 16-04: MAJOR-4 replay residual risk (no CAS on the Allowed email.send path) accepted for v1.3; durable per-attempt ledger makes each send auditable. Tracked as v2 obligation. **Superseded by v1.4 DESIGN-02**: this must be RE-EARNED in writing against the adaptive-planner threat model, not silently inherited.
- [Phase ?]: 16-04: 'Deny sends nothing' send-level proof recorded as an explicit Phase 17/ACCEPT-01 requirement — not yet covered by any test.

### Pending Todos

- `.planning/todos/pending/2026-07-07-gsd-phases-clear-deletes-all-milestones.md` — GSD tooling bug: `gsd_run query phases.clear --confirm` deletes ALL milestones' phase dirs, not just the previous one's leftovers. Not yet fixed upstream; carries forward to v1.4.
- `.planning/todos/pending/2026-07-08-gsd-executors-must-not-write-phase-completion-state.md` — GSD tooling bug: the last-wave executor's own doc-completion commit repeatedly flips ROADMAP.md's phase-level checkbox before verification (2-for-2, Phases 15/16). Did NOT recur at Phase 17 (closed manually by the orchestrator after independent verification, per this bug). Not yet fixed upstream; carries forward to v1.4. **Mitigation for v1.4 (per global learned-rules): never let ANY phase-18-22 executor touch ROADMAP.md/STATE.md — the orchestrator updates phase-completion state itself.**
- `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` — 5 v2 security obligations (demote-at-RequestFd, verify_chain keyed-MAC, Allowed-path replay CAS, CreateSession build-excluded path, kind-aware Source label). All 5 are already honestly disclosed in PROJECT.md's DOC-01 paragraph and residual-risks clause as of v1.3; this todo tracks the actual v2 fix work. Carries forward to v1.4 (one of the 5 — Allowed-path replay CAS — is directly addressed by v1.4's DESIGN-02 re-earning exercise, not closed).

### Blockers/Concerns

- Phase 19 (the fix) and everything downstream (Phases 20-22) is hard-blocked on Phase 18's DESIGN doc clearing a fresh adversarial review. No `server.rs` change for the trust-coherence fix before that gate.

## Deferred Items

Items acknowledged and deferred at prior milestone closes:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| uat | 03-UAT.md (Phase 03, v1.0 milestone — passed, 0 pending scenarios; benign stale artifact) | passed | 2026-07-01 |
| todo | gsd-phases-clear-deletes-all-milestones (GSD tooling bug) | open | 2026-07-09 |
| todo | gsd-executors-must-not-write-phase-completion-state (GSD tooling bug) | open | 2026-07-09 |
| todo | v1.3-phase16-v2-security-obligations (5 v2 security items, already disclosed in DOC-01) | open | 2026-07-09 |
| requirement | T2 slot-type binding enforcement | deferred to v1.5 | 2026-07-10 |

Re-acknowledged unchanged at v1.2 milestone close on 2026-07-07 (same
pre-existing item, still benign). Re-acknowledged at v1.3 milestone close on
2026-07-09 via `audit-open` — all 4 open items (1 UAT, 3 todos) reviewed and
accepted as known/benign or already-tracked v2 work; none block v1.3's close.

## Session Continuity

Last session: 2026-07-10
Stopped at: ROADMAP.md + REQUIREMENTS.md traceability written for v1.4 (5 phases: 18-22)
Resume file: None

## Operator Next Steps

- Run `/gsd-plan-phase 18` to plan the Trust-Boundary Coherence Design Gate phase.
