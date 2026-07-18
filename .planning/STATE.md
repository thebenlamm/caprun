---
gsd_state_version: 1.0
milestone: v1.9
milestone_name: — Authorized Egress + Policy & Audit Surface
current_phase: 44
current_phase_name: git.push — Broker-Performed Destination-Pinned Egress
status: planning
stopped_at: Phase 43 (http-write egress, HTTP-W-01) complete & verified — transitioned to Phase 44
last_updated: "2026-07-18T17:30:00.000Z"
last_activity: 2026-07-18
last_activity_desc: Phase 43 complete (compose-verify 584/0, Fable-5 APPROVE), transitioned to Phase 44
progress:
  total_phases: 6
  completed_phases: 3
  total_plans: 9
  completed_plans: 9
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-18)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended (v1.2) with session-level draft-only demotion (I1/I0) and single-shot human confirmation, (v1.3) with content-sensitive body blocking + a real broker-mediated SMTP send, (v1.4) with coherent cross-connection trust state + a boundary proven indifferent to planner intelligence, (v1.5) with slot-type binding (T2), (v1.6) hardening the standing residuals into enforced guarantees, (v1.7) extending the real sinks — `process.exec` + filesystem read/write breadth, (v1.8) adding `git.commit` + read-only `http.request` GET + `github.pr` (git.push gate-deferred), and now (v1.9) completing the authorized-write-egress loop (git.push + http-write) and adding the first trust-surface layer (a minimal per-session policy + a CLI/audit-DAG viewer) toward a design-partner-runnable slice — without weakening I0/I1/I2 or adding any raw `EffectRequest` path.
**Current focus:** Phase 44 — `git.push` broker-performed destination-pinned egress (GIT-02/03 + HYG-01): a fully-unprivileged, broker-mediated smart-HTTP transfer (net-denied child, application-layer destination pin), trusted-intent remote/refspec, force/delete hard-denied by construction, broker-env credential custody + redirect-refusal + payload-vs-destination confirm anti-TOCTOU + confirm-release; the supply-chain absence gate re-runs after the transport-dep choice. Research (`research/GIT-PUSH-EGRESS.md`, HIGH conf) assesses candidate-b FEASIBLE → plans to SHIP; the gate-authorized safety-valve defers a 3rd time only if no sound unprivileged mechanism proves out.

## Current Position

Phase: 44 — `git.push` Broker-Performed Destination-Pinned Egress
Plan: Not started
Status: Roadmapped; Phases 41-43 complete
Last activity: 2026-07-18 — Phase 43 (http-write egress, HTTP-W-01) complete & verified, transitioned to Phase 44

### Phase 43 close (2026-07-18)
- 4 plans executed sequentially on `main`, zero deviations. A distinct `http.request.write` CommitIrreversible sink (the MAJOR-1 I0-escape fix) + taint-governed body/url under I2 + exact {POST,PUT} method-enum gate + distinct fail-closed `WRITE_HOST_ALLOWLIST` reusing the shipped SSRF resolve-and-pin + broker-env-only optional credential + opaque non-minting two-phase audit + Allowed-dispatch & single-shot confirm-release (P33/P34 precheck-before-burn) + a differential acceptance test (taint the sole variable).
- **Linux gate:** `compose-verify.sh` 584 passed / 0 failed, no v1.0–v1.8 regression (all prior composed live proofs green), no cfg-linux-blindness. check-invariants all gates PASS (Gate 3 mint-site byte-identical, Gate 5 no new crate).
- **Fresh non-self Fable-5 adversarial code-trace:** APPROVE, 0 defects (2 non-actionable NITs; NIT-1 log-scrub tracked for Phase 44).
- HTTP-W-01 Complete for what Phase 43 owns (sink + differential at the decision+dispatch boundary); the requirement's live-mock-receipt sub-clause is by-design carried to Phase 46 (LIVE-05/06) — write allowlist ships empty, no live write mock until the composed proof. See `43-VERIFICATION.md`.

## Performance Metrics

**Velocity:**

- Total plans completed: 132 (v1.0: 15 + v1.1: 15 + v1.2: 11 + v1.3: 21 + v1.4: 14 + v1.5: 8 + v1.6: 14 + v1.7: 17 + v1.8: 17)
- Average duration: — min

*Updated after each plan completion. v1.8 (phases 35-38,40) shipped 2026-07-18. v1.9 (phases 41-46) IN PROGRESS 2026-07-18 — Phases 41 (design gate) + 42 (policy layer, 5 plans) + 43 (http-write, 4 plans) complete; Phase 44 (git.push) next.*

## Accumulated Context

### Decisions

**v1.9 roadmap phase structure (`/gsd-roadmapper`, 2026-07-18):** 6 phases
(41-46), 13/13 requirements mapped, 0 orphans, 0 duplicates. Continues numbering
from v1.8's Phase 40 (does NOT reset; v1.8's deferred git.push was Phase 39).
Mirrors this project's unbroken design-gate → foundation → implementation →
live-proof precedent (v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26,
v1.7 P31, v1.8 P35 — each a standalone reviewed DESIGN doc before any TCB code,
followed by implementation, followed by a separate live-proof phase). Ordering
follows both reviewers' agreed sequencing + the GIT-PUSH-EGRESS research
(candidate (b) broker-performed smart-HTTP transfer, git.push FEASIBLE):

- **Phase 41** is the design gate (DESIGN-17/18 — one DESIGN doc covering all
  three TCB pieces: git.push egress mechanism, http-write egress, AND the
  policy-vs-I2 boundary incl. POLICY-03 provenance/binding). HARD-BLOCKS Phases
  42-46. The ORCHESTRATOR (not a gsd-executor) owns the fresh non-self
  adversarial-trace spawn. The trace re-runs if the git.push trust-posture or
  transport-dep choice changes mid-implementation. No `crates/{executor,brokerd,
  sandbox,runtime-core}` TCB code before this gate clears.

- **Phase 42** is the policy foundation (POLICY-01/02/03) — bound BEFORE the
  sinks it gates. POLICY-02's enforcement (I2 is a non-bypassable, hardcoded,
  post-policy-narrowing gate) is coded inseparably from POLICY-01's policy-deny
  outcome, so all three POLICY reqs land here; the LOCKED I2-non-bypass invariant
  is re-demonstrated live in Phase 46 (LIVE-06 leg 3).

- **Phase 43** is `http.request` WRITE (HTTP-W-01) — the simpler write-egress
  sink, extending the shipped `http.request` GET path to POST/PUT on a DISTINCT
  write-allowlist, taint-governed body, differential proof. Split from git.push
  so a git.push deferral leaves it untouched.

- **Phase 44** is `git.push` (GIT-02/03) + HYG-01 — the riskiest surface;
  broker-performed smart-HTTP transfer (net-denied child, application-layer
  destination pin), credential custody, redirect-refusal, payload-vs-destination
  confirm anti-TOCTOU, confirm-release. HYG-01 lands here because the transport-dep
  choice + its supply-chain absence re-run belong with git.push. Carries the
  gate-authorized SAFETY-VALVE: if Phase 41 proved no sound fully-unprivileged
  destination-pinning mechanism, GIT-02/03 defer a 3rd time (disclosed +
  sign-off-gated) and the other three tracks still ship.

- **Phase 45** is the trust surface — thin CLI/SDK (SDK-01) + read-only
  audit-DAG viewer (U1). ON THE ACCEPTANCE CRITICAL PATH (LIVE-05 requires the
  composed proof be DRIVEN and INSPECTED via this CLI + viewer), NOT trailing
  tooling. Binds the trusted policy (POLICY-03 enforcement point); escapes
  tainted display bytes; fails closed on absent MAC key.

- **Phase 46** is the composed live proof (LIVE-05/06) — the v1.9 DONE gate.
  Composed exec→fs→git.commit→git.push→github.pr + http POST on real Linux,
  driven & inspected via the new CLI+viewer, with 5 independently-attributable
  negative legs (tainted push refspec, tainted POST body, policy-deny,
  destination-pin negative, credential-absence). Mirrors v1.2 P11, v1.3 P17,
  v1.4 P22, v1.5 P25, v1.6 P30, v1.7 P34, v1.8 P40. Depends on Phases 42-45.

### Blockers/Concerns

- Phases 42-46 are hard-blocked on Phase 41's DESIGN doc clearing a fresh
  (non-self, ORCHESTRATOR-owned) adversarial code-trace. No
  `crates/{executor,brokerd,sandbox,runtime-core}` TCB code before that gate.

- **git.push (Phase 44) safety-valve:** research (`research/GIT-PUSH-EGRESS.md`,
  HIGH confidence) assesses candidate (b) — a broker-performed smart-HTTP
  transfer reusing the already-shipped reqwest+rustls(ring)+webpki-roots+SSRF
  stack (ZERO new crates) — FEASIBLE, so git.push is planned to SHIP, not defer.
  BUT if the Phase-41 gate cannot pin a sound fully-unprivileged
  destination-pinning mechanism, GIT-02/03 defer a 3rd time — a disclosed,
  sign-off-gated deferral (the git.push leg auto-descopes from LIVE-05/06), never
  a silent drop and never shipping arbitrary net-allowed child egress
  (v1.8 BLOCKER-1: seccomp cannot pin a `connect()` destination).

- **The #1 adversarial-trace risk = the policy-vs-I2 boundary (POLICY-02/03).**
  Policy may only NARROW which sinks/args are callable (a pre-I2 gate); it can
  NEVER disable/override I2, and it must be bound from a trusted source outside
  the confined worker's reach (F1 containment, reused verbatim from `key.rs`).
  Phase 41's design gate must pin this; Phase 42 enforces it; Phase 46 proves it
  live.

- **HYG-01 supply-chain re-run:** the workspace-scoped absence assertion
  (`cargo tree --workspace -i <dep>` = absent for aws-lc-rs/openssl-sys) must
  re-run AFTER the git.push transport-dep choice, enumerating any NEW transport
  deps — the resolver-3 feature-unification lesson (v1.8 aws-lc-rs-in-workspace
  MAJOR). Any new dep must honor the ring-only recipe.

### Standing GSD-tooling mitigations (carried forward)

- `phases.clear --confirm` deletes ALL prior phase dirs from disk (documented
  bug, 5-for-5 across v1.3–v1.8 scoping) — git-status-check `.planning/phases/`
  immediately after any `phases.clear`; restore if needed.

- The last-wave executor's doc-completion commit has historically flipped
  ROADMAP.md's phase checkbox before verification (Phases 15/16) — never let ANY
  executor touch ROADMAP.md/STATE.md; the orchestrator owns phase-completion state.

- The DESIGN-gate adversarial-trace spawn is ORCHESTRATOR-owned, not a
  gsd-executor (fresh, non-self) — the [[fresh-context-adversarial-review]]
  guardrail that has caught 9+ real BLOCKER/MAJOR defects through v1.8.

## Session Continuity

Last session: 2026-07-18
Stopped at: v1.9 roadmap created (Phases 41-46, 13/13 requirements mapped)
Resume file: None

## Operator Next Steps

- Plan Phase 41 (the v1.9 DESIGN gate) with `/gsd-plan-phase 41`

## Deferred Items

Items acknowledged and deferred at prior milestone closes, re-reviewed at v1.9 roadmap creation (2026-07-18).

| Category | Item | Status |
|----------|------|--------|
| requirement | GIT-02/GIT-03 (`git.push`) — deferred at v1.8 (Phase 39) via gate-authorized deferral | now IN SCOPE as v1.9 Phase 44 (research assesses FEASIBLE; ships unless the Phase-41 gate proves no sound mechanism) |
| todo (security) | v1.3-phase16-v2-security-obligations — deferred v2 security obligations (recorded, not dropped) | pending |
| todo (tooling) | gsd-executors-must-not-write-phase-completion-state | pending (GSD process, not caprun product) |
| todo (tooling) | gsd-phases-clear-deletes-all-milestones | pending (GSD process, not caprun product) |
