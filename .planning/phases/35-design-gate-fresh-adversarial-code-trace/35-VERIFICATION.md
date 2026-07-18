---
phase: 35-design-gate-fresh-adversarial-code-trace
verified: 2026-07-18T06:40:00Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
verified_by: orchestrator (autonomous mode) — design-gate phase; the DESIGN-16 gate IS the verification
gaps: []
---

# Phase 35 Verification — DESIGN Gate + Fresh Adversarial Code-Trace

**Goal:** A reviewed DESIGN doc pins the mechanism + fail-closed default for all four v1.8
sinks and clears a fresh non-self adversarial code-trace — hard-blocking every subsequent
TCB-code phase.

## Success criteria (goal-backward)

1. ✅ **`planning-docs/DESIGN-git-github-http-sinks.md` exists and pins, per sink:** the
   dispatch pattern (A/B), effect-class (git.commit=MutateReversible, git.push &
   github.pr=CommitIrreversible, http.request GET=Observe), I2-sensitive sink args, taint
   flow, confinement; plus `mint_from_http`+`TaintLabel::HttpRaw`+session demotion, git
   config/hook neutralization, git.push destination-pinning + broker-mediated egress +
   credential injection + captured-output scrub, SSRF resolve-and-pin (GET + github.pr
   POST base-URL), the session-scoped auth-grant, env_clear webpki-roots TLS policy, and
   the duplicate-PR CAS. **Verified:** doc present (§0-§12, revised); check-invariants exits 0.

2. ✅ **The doc closes all 11 design-gate-blocking pitfalls with a NAMED mechanism each**
   (§6 table), weakens no invariant (§7: I0 unaffected, I1 preserved+extended, I2 not
   bypassed, no raw EffectRequest, sink sensitivity stays hardcoded), and introduces no raw
   effect-request path. **Verified:** §6/§7 present; check-invariants Gate 1 (EffectRequest)
   green.

3. ✅ **A fresh, non-self, orchestrator-owned adversarial code-trace cleared the doc, all
   findings resolved, recorded in a gate record; no TCB code written before the gate
   cleared.** **Verified:** `planning-docs/DESIGN-GATE-RECORD-v1.8.md` records 2 rounds
   (round 1: 1 BLOCKER + 3 MAJOR + 1 MINOR + 1 NIT, all resolved; round 2: all confirmed
   resolved + 1 editorial MAJOR reconciled), all citations verified against real code,
   `git status --porcelain crates/ cli/` empty (NO TCB code exists yet). Reviewer was a
   fresh Fable-5 (non-self), orchestrator-spawned — satisfying DESIGN-16.

## Notes
- The gate did its job: caught a real BLOCKER (git.push seccomp-egress infeasibility) that
  forced a sound FORK-1 re-decision — the 9th real defect this discipline has caught.
- Standing corrections for Phases 36-40 are enumerated in the gate record. FLAG: git.push
  (Phase 39) may defer if no unprivileged destination-pin mechanism proves feasible.
