---
gsd_state_version: 1.0
milestone: v1.9
milestone_name: — Authorized Egress + Policy & Audit Surface
current_phase: 46
current_phase_name: Composed Live Proof (v1.9 DONE)
status: planning
stopped_at: Phase 45 (CLI/SDK + audit-DAG viewer, SDK-01 + U1) complete & verified — transitioned to Phase 46 (the v1.9 DONE gate)
last_updated: "2026-07-18T21:30:00.000Z"
last_activity: 2026-07-18
last_activity_desc: Phase 45 SHIPPED (compose-verify 691/0, Fable-5 APPROVE; M7 + viewer fail-closed sound; BiDi neutralizer hardened), transitioned to Phase 46
progress:
  total_phases: 6
  completed_phases: 5
  total_plans: 18
  completed_plans: 18
  percent: 83
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-18)

**Core value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended (v1.2) with session-level draft-only demotion (I1/I0) and single-shot human confirmation, (v1.3) with content-sensitive body blocking + a real broker-mediated SMTP send, (v1.4) with coherent cross-connection trust state + a boundary proven indifferent to planner intelligence, (v1.5) with slot-type binding (T2), (v1.6) hardening the standing residuals into enforced guarantees, (v1.7) extending the real sinks — `process.exec` + filesystem read/write breadth, (v1.8) adding `git.commit` + read-only `http.request` GET + `github.pr` (git.push gate-deferred), and now (v1.9) completing the authorized-write-egress loop (git.push + http-write) and adding the first trust-surface layer (a minimal per-session policy + a CLI/audit-DAG viewer) toward a design-partner-runnable slice — without weakening I0/I1/I2 or adding any raw `EffectRequest` path.
**Current focus:** Phase 46 — Composed Live Proof (LIVE-05/06), the v1.9 DONE gate. A composed workflow — `process.exec` (test) → filesystem edit → `git.commit` → `git.push` → `github.pr` PLUS an `http.request` POST leg — runs on real Linux (mock git remote + mock endpoint), DRIVEN via `caprun run` and INSPECTED via `caprun audit`, every step gated/tainted/audit-DAG-chained + `verify_chain` true. Plus 5 independently-attributable negative legs (LIVE-06): (1) tainted push remote/refspec I2-Blocks, (2) tainted POST body I2-Blocks, (3) a policy-deny leg (distinct machine-checkable tag from the I2-Block, where the I2 legs run a policy-PERMITTED sink+arg so policy is provably not what's blocking), (4) a destination-pin negative (redirected/off-pin push/POST refused at the broker/app layer), (5) a credential-absence assertion (no credential/URL material in the value store or audit chain after a real push). Full-workspace regression green, no v1.0–v1.8 regression. Depends on Phases 42-45.

## Current Position

Phase: 46 — Composed Live Proof (v1.9 DONE)
Plan: Not started
Status: Roadmapped; Phases 41-45 complete
Last activity: 2026-07-18 — Phase 45 (CLI/SDK + audit-DAG viewer, SDK-01 + U1) SHIPPED & verified, transitioned to Phase 46

### Phase 45 close (2026-07-18)
- 4 plans executed sequentially on `main`. SDK-01: a `caprun run <intent> <workspace> [--policy <path>]` verb binding the trusted policy at session creation (POLICY-03 enforcement point) + surfacing the blocked effect_id + `caprun review` pointer on an I2 Block (Matt #2). The **M7 anti-laundering TCB fix**: a file-derived `--seed-from-file` literal is minted TAINTED via the EXISTING broker-side `mint_from_read` site in the ProvideIntent arm (operator literals stay trusted via mint_from_intent, DISJOINT; file-derived provenance threaded per-literal through the ProvideIntent proto + worker.rs; NO second mint site — Gate 3 holds; proven non-vacuous). U1: a read-only `caprun audit <session>` viewer rendering events/decisions + verify_chain, using a load-ONLY fail-closed `load_existing_key` (refuses absent key + `:memory:`, F1 containment, opens read-only, mints/appends nothing), neutralizing every displayed literal via the shared `brokerd::display::neutralize_control_chars`.
- **Plan-checker caught the M7 mechanism as a BLOCKER pre-execution** (the `--seed-from-file` laundering path was verified in shipped code); the fix was folded in + re-verified before executing 45-01.
- **Linux gate (independent orchestrator re-run):** compose-verify 691/0, exit 0, incl. the genuine end-to-end run→Block→review→audit loop + the U1 negatives; no v1.0–v1.8 regression. check-invariants all gates PASS.
- **Fresh Fable-5 adversarial trace:** APPROVE — M7 anti-laundering (defended by TWO independent controls: trusted-main-set session Draft status + both sinks CommitIrreversible→Draft-denies) and viewer fail-closed both sound. Surfaced ONE genuine decision-surface MINOR (pre-existing): the shared `neutralize_control_chars` only caught `is_control()` (Cc), missing the Trojan-Source BiDi/zero-width class (CVE-2021-42574, category Cf) — LIVE on the git.push confirm prompt (a tainted refspec with U+202E visually reversed the human's confirm prompt). **FIXED this phase (`e31257a`):** escape the format-spoof set (U+202A..U+202E, U+2066..U+2069, U+200B..U+200F, U+FEFF) alongside control chars + tests; the confirmation.rs anti-drift test confirms the git.push confirm path picks it up automatically; re-verified post-fix compose-verify 691/0. See `45-VERIFICATION.md`.

### Phase 44 close (2026-07-18)
- 5 plans executed sequentially on `main`. `git.push` SHIPPED — did NOT defer a 3rd time (the research-pinned Candidate (b) broker-performed smart-HTTP transfer proved sound under the fresh adversarial trace; §1.9 safety-valve not triggered). Broker-performed two-request smart-HTTP (info/refs GET + git-receive-pack POST) over the shipped reqwest-ring resolve-and-pin with the IP FROZEN across both requests (WG-1) + redirect refused; pack-gen child net-denied under the unchanged exec_child_filter (WG-2 `run_launcher_capture_bytes` + `git pack-objects`); force/`--force-with-lease`/`:delete`/`+`refspec HARD-DENIED by construction; broker-env-only credential (Basic x-access-token) scrubbed from value-store/audit/logs; opaque non-minting audit; **ALWAYS confirm-gated — NO auto-dispatch arm** (clean Allowed → synthetic BlockedPendingConfirmation with a MAC'd frozen-new-oid pending row; `invoke_git_push_from_resolved` confirm-release-only, single non-test caller) + WG-7 anti-TOCTOU freeze + WG-8 taint-provenance renderer + P33/P34 precheck-before-burn; HYG-01 zero-new-crate re-asserted.
- **Linux gate (independent orchestrator re-run):** compose-verify 668/0, exit 0, all 5 s44 legs green incl. leg_c real delivery to mock git-receive-pack + leg_d force/delete refused + leg_e redirect refused; no v1.0–v1.8 regression. check-invariants all gates PASS.
- **Fresh Fable-5 adversarial trace:** APPROVE, 0 security defects across 8 surfaces. ONE non-security functional note (recorded as a Deferred Item): `generate_pack` uses the shared 10 MB `MAX_COMBINED_OUTPUT_BYTES` cap → a >10MB pack fails CLOSED (safe, no partial push) but would block large-repo pushes; revisit before/at LIVE-05/06 (Phase 46 pushes a small mock repo, so not blocking for 46).
- Process: [[cfg-linux-test-blindness]] re-hit TWICE (two latent Linux-only defects from 44-03/44-04 caught only by the FULL compose-verify, not scoped runs — fixed in 44-05); a real 44-03 bug (`resolve_new_oid` reading the merged stream) caught+fixed by 44-04; a 44-01 brokerd-test gap (ran only runtime-core) caught+fixed by 44-02. See `44-VERIFICATION.md`.

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
| functional (caprun) | git.push `generate_pack` uses the shared 10 MB `MAX_COMBINED_OUTPUT_BYTES` cap → a >10MB pack fails CLOSED (safe, no partial push) but blocks large-repo pushes (Fable-5 Phase-44 non-security note) | pending — revisit before/at LIVE-05/06 (Phase 46 pushes a small mock repo, non-blocking for 46) |
