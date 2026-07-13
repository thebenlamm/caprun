---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: — Security Hardening
current_phase: 28
current_phase_name: authenticated-audit-chain
status: executing
stopped_at: Phase 26 context gathered
last_updated: "2026-07-13T00:30:19.380Z"
last_activity: 2026-07-13
last_activity_desc: Phase 28 execution started
progress:
  total_phases: 5
  completed_phases: 2
  total_plans: 9
  completed_plans: 5
  percent: 40
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-12)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended (v1.2) with session-level draft-only demotion (I1/I0) and single-shot human confirmation, (v1.3, SHIPPED) with content-sensitive body blocking, a real broker-mediated SMTP send, and a composed live acceptance, (v1.4, SHIPPED) with coherent cross-connection trust state and a boundary proven indifferent to planner intelligence, (v1.5, SHIPPED) with a structural check that a value's semantic origin matches the semantic role of the slot it's routed into (closing the v1.4 T2 residual), and now (v1.6) hardening the standing residuals that made several of those guarantees "true only incidentally" into enforced guarantees.
**Current focus:** Phase 28 — authenticated-audit-chain

## Current Position

Phase: 28 (authenticated-audit-chain) — EXECUTING
Plan: 2 of 5
Status: Ready to execute
Last activity: 2026-07-13 — Phase 28 execution started

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 51 (v1.0: 15 + v1.1: 15 + v1.2: 11 + v1.3: 21 [Phases 12-17] + v1.4: 14 [Phases 18-22]) + v1.5: 8 [Phases 23-25]
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
| 19 | 2 | - | - |
| 20 | 3 | - | - |
| 21 | 4 | - | - |
| 22 | 3 | - | - |
| 23 | 2 | - | - |
| 24 | 3 | - | - |
| 25 | 3 | - | - |
| 26-30 | 0 | - | - |
| 26 | 2 | - | - |
| 27 | 2 | - | - |

*Updated after each plan completion. v1.3 (phases 12-17) shipped 2026-07-09 — 21/21 plans complete. v1.4 (phases 18-22) shipped 2026-07-11 — 14/14 plans complete. v1.5 (phases 23-25) shipped 2026-07-12 — 8/8 plans complete. v1.6 (phases 26-30) roadmapped 2026-07-12, no plans yet.*
| Phase 14 P02 | 50min | 3 tasks | 10 files |
| Phase 15-deterministic-doc-action-extraction P01 | 75min | 3 tasks | 3 files |
| Phase 15-deterministic-doc-action-extraction P02 | 11min | 3 tasks | 3 files |
| Phase 15-deterministic-doc-action-extraction P03 | 55min | 2 tasks | 3 files |
| Phase 15 P04 | ~2h10min | 3 tasks | 11 files |
| Phase 16 P01 | 40min | 2 tasks | 8 files |
| Phase 16 P02 | 50min | 2 tasks | 3 files |
| Phase 16 P03 | 25min | 1 tasks | 1 files |
| Phase 16 P04 | 3h | 3 tasks | 16 files |
| Phase 24 P01 | 25min | 3 tasks | 9 files |
| Phase 24 P02 | 25min | 1 tasks | 1 files |
| Phase 24 P03 | 25min | 2 tasks | 3 files |
| Phase 28 P01 | 7min | 2 tasks | 10 files |

## Accumulated Context

### Decisions

v1.6 scoping decisions (five TCB-local hardening items, no new external-effect
surface; breadth — Git/GitHub/test adapters, patch/PR, workspace snapshots —
deliberately deferred to v1.7) are recorded in `.planning/PROJECT.md`'s
Current Milestone section and `.planning/REQUIREMENTS.md`.

**Roadmap phase structure (`/gsd-roadmapper`, 2026-07-12):** 5 phases (26-30),
8/8 requirements mapped, 0 orphans. Mirrors this project's established
design-gate/implementation/live-proof precedent (v1.0 P2, v1.2 P8, v1.3 P12,
v1.4 P18, v1.5 P23 — each a standalone reviewed DESIGN doc before any TCB
code, followed by implementation, followed by a separate live-proof phase):
**Phase 26** is the design gate (DESIGN-11/12 — `DESIGN-security-hardening.md`

+ fresh non-self adversarial review covering all five residuals' mechanisms

and fail-closed defaults) — hard-blocks Phases 27-29. The five HARDEN items
split into **3 implementation phases by blast radius** rather than one
bundled phase (keeps each phase's success criteria independently verifiable)
or five single-requirement phases (avoids trivial fragmentation): **Phase 27**
groups HARDEN-01 (demote-at-RequestFd) + HARDEN-04 (CreateSession
forced-Active compile-exclusion) — both land in `server.rs`'s session/
connection-lifecycle surface. **Phase 28** is HARDEN-02 alone (audit-chain
keyed-MAC/anchoring — a self-contained mechanism with its own key/anchor
custody and threat model, substantial enough to warrant its own phase).
**Phase 29** groups HARDEN-03 (Allowed-path replay CAS) + HARDEN-05
(`file.create` `contents` expected-role table entry) — both are sink-
dispatch-level hardening even though the mechanisms differ. **Phase 30** is
the regression/live-proof phase (HARDEN-06 — full `mailpit-verify.sh`
re-run + a dedicated negative test per closed residual), mirroring v1.2 P11,
v1.3 P17, v1.4 P22, v1.5 P25; depends on Phases 27, 28, and 29 all landing.

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
- [Phase 16]: verify_chain's scope recorded honestly: detects single-store/non-recomputing-multi-store tampering only, NOT authenticated/externally-anchored -- chain-head-not-anchored is an Accepted Residual Risk with a v2 obligation — this is now v1.6's HARDEN-02
- [Phase 16]: CONTROL-02 live fixture models CLEAN_PATH_CONTENT's no-marker recipient side + a Body: marker only, verified against worker.rs's extraction branch directly before writing the test, to guarantee no accidental recipient taint (Pitfall 5).
- [Phase ?]: 16-04: All three BLOCKER-1 guards (ProvideIntent ordering, non-live-state Deny, CreateSession IPC opt-in) landed in Task 1, strictly before Task 2's email.send Allowed-dispatch, so the dispatch never exists without its guards.
- [Phase ?]: 16-04: CreateSession IPC arm gated behind CAPRUN_ENABLE_IPC_CREATE_SESSION == exactly "1" (runtime opt-in, fail-closed default-deny) instead of cfg(test), which is unset when brokerd compiles as an integration-test dependency. — this runtime-vs-compile-time gap is now v1.6's HARDEN-04.
- [Phase ?]: 16-04: MAJOR-4 replay residual risk (no CAS on the Allowed email.send path) accepted for v1.3; durable per-attempt ledger makes each send auditable. Tracked as v2 obligation. Superseded by v1.4 DESIGN-02 (re-earned in writing, no new CAS added). Now v1.6's HARDEN-03.
- [Phase 18]: v1.4's cross-connection trust-coherence fix used a one-way, session-lifetime occupancy latch (not release-on-disconnect) after a fresh adversarial round caught the weaker design was still bypassable via sequential reconnect.
- [Phase 22]: v1.4's T2 (slot-type binding) residual became v1.5's scope; v1.5 shipped it. The remaining five DOC-01 residuals (demote-at-RequestFd, verify_chain auth, replay CAS, compile-exclusion, file.create contents slot) are now v1.6's scope.
- [Phase ?]: 24-01: mint_from_derivation's concat arm guards inputs.len() == 2 before assigning origin_role Some(recipient) — any other arity gets None, I2 remains the backstop
- [Phase ?]: 24-01: server.rs selects primary_role inside the :1294 intent-variant match (same arm as primary_literal), never hardcoded at the shared mint_from_intent call — avoids mistagging a file.create path as recipient
- [Phase 24]: SlotTypeMismatch fields are owned String/Vec<String>/Option<String>, never &'static — DenyReason crosses the IPC wire via serde Deserialize (DESIGN F1)
- [Phase 24]: No new ExecutorDecision variant added — reused the existing Denied { reason } carrier (A3)
- [Phase 24]: Step 1c wired as a per-arg return-immediately guard (Steps 1/1a/1b tier), never joining the collect-then-Block vec — preserves I0/I2 precedence, zero new anchor shapes — DESIGN §6
- [Phase 24]: email.send body's expected-role list corrected to [body, doc_fragment] (DESIGN §3 table amendment) — the only production vocabulary for hostile-extracted body content is doc_fragment (worker.rs WorkerClaim::DocFragment -> server.rs claim_type -> mint_from_read origin_role); the DESIGN-literal [body] alone would have hard-Denied the shipped CONTENT-01/CONTROL-02 body-Block flow instead of reaching I2 — caught by extract_provenance_threading.rs test failures; safe under F4 since body is content-sensitive
- [Phase 25]: file.create's contents arg has no expected-role entry at all (uses executor's routing-sensitive default) — this gap is now v1.6's HARDEN-05.
- [Phase 28]: 28-01: run_caprun_file_create/run_caprun_block now return/take ws_dir (the workspace subdirectory) instead of the outer tmp dir, since that is the broker-derived workspace root used for post-run assertions
- [Phase 28]: 28-01: live_acceptance_v1_3.rs/v1_4_composed.rs's run_caprun_email_on gained an explicit ws_dir param replacing the audit_db.parent() derivation, to keep the shared multi-invocation workspace root F1-safe as a sibling of audit.db

### Pending Todos

- `.planning/todos/pending/2026-07-07-gsd-phases-clear-deletes-all-milestones.md` — GSD tooling bug: `gsd_run query phases.clear --confirm` deletes ALL milestones' phase dirs, not just the previous one's leftovers. Not yet fixed upstream; recurred a 2nd time at v1.4 scoping (per learned-rules). Carries forward to v1.6 — git-status-check `.planning/phases/` immediately after any `phases.clear` invocation.
- `.planning/todos/pending/2026-07-08-gsd-executors-must-not-write-phase-completion-state.md` — GSD tooling bug: the last-wave executor's own doc-completion commit repeatedly flips ROADMAP.md's phase-level checkbox before verification (2-for-2, Phases 15/16). Did NOT recur at Phases 17-25 (mitigation held: never let ANY executor touch ROADMAP.md/STATE.md — the orchestrator updates phase-completion state itself). Not yet fixed upstream; carries forward to v1.6 as a standing mitigation, not a re-test.
- `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` — the 5 v2 security obligations this todo tracks (demote-at-RequestFd, verify_chain keyed-MAC, Allowed-path replay CAS, CreateSession build-excluded path, kind-aware Source label) are now DIRECTLY IN SCOPE as v1.6 (HARDEN-01..05; the kind-aware-Source-label item remains a deferred UX nicety, not part of v1.6). Close this todo once v1.6 ships.

### Blockers/Concerns

- Phases 27, 28, and 29 (implementation) and Phase 30 (regression/live proof) are hard-blocked on Phase 26's DESIGN doc (`planning-docs/DESIGN-security-hardening.md`) clearing a fresh (non-self) adversarial review. No `crates/executor`/`crates/brokerd`/`crates/runtime-core` hardening code before that gate.

## Deferred Items

Items acknowledged and deferred at prior milestone closes:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| uat | 03-UAT.md (Phase 03, v1.0 milestone — passed, 0 pending scenarios; benign stale artifact) | passed | 2026-07-01 |
| todo | gsd-phases-clear-deletes-all-milestones (GSD tooling bug) | open | 2026-07-09 |
| todo | gsd-executors-must-not-write-phase-completion-state (GSD tooling bug) | open | 2026-07-09 |
| todo | v1.3-phase16-v2-security-obligations (5 v2 security items) | in progress — now v1.6's roadmap | 2026-07-09 |
| requirement | T2 slot-type binding enforcement | ✓ DELIVERED — v1.5 Phases 23-25 (enforced in TCB, proven live on Linux) | closed 2026-07-12 |

Re-acknowledged unchanged at v1.2 milestone close on 2026-07-07 (same
pre-existing item, still benign). Re-acknowledged at v1.3 milestone close on
2026-07-09 via `audit-open` — all 4 open items (1 UAT, 3 todos) reviewed and
accepted as known/benign or already-tracked v2 work; none block v1.3's close.
Re-acknowledged at v1.4 milestone close on 2026-07-11 — no new items opened;
T2 slot-type binding moved from "deferred" to "in scope" as v1.5's roadmap.
Re-acknowledged at v1.5 milestone close on 2026-07-12 via `audit-open` — the
SAME 4 pre-existing cross-milestone items (1 UAT [Phase 03, passed], 3 v1.3-era
todos), none from v1.5 and none blocking; all v1.5 phases verified passed (11/11
requirements). T2 slot-type binding now DELIVERED (no longer a deferred item).
v1.6 roadmap created 2026-07-12 — the v1.3-phase16-v2-security-obligations todo
moved from "open" to "in progress" as its 5 items became v1.6's HARDEN-01..05.

## Session Continuity

Last session: 2026-07-13T00:29:56.088Z
Stopped at: Phase 26 context gathered
Resume file: .planning/phases/26-security-hardening-design-gate/26-CONTEXT.md

## Operator Next Steps

- Run `/gsd-plan-phase 26` to plan the Security Hardening Design Gate phase.
