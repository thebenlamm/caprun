---
phase: 35-design-gate-fresh-adversarial-code-trace
plan: 02
requirements: [DESIGN-16]
status: complete
---

# Plan 35-02 Summary — Fresh Non-Self Adversarial Code-Trace (DESIGN-16)

**Requirement:** DESIGN-16 — the v1.8 DESIGN doc clears a fresh, non-self,
**orchestrator-owned** adversarial code-trace before any TCB code.

## What happened

The orchestrator (not a gsd-executor) spawned a **fresh Fable-5 reviewer** (non-self:
different model, did not author the doc) to trace `planning-docs/DESIGN-git-github-http-sinks.md`
against the real code and adversarially attack the design. Two rounds:

**Round 1 — CHANGES REQUIRED (1 BLOCKER, 3 MAJOR, 1 MINOR, 1 NIT):**
- **BLOCKER-1:** git.push's stated seccomp destination-pinning is infeasible (seccomp can't
  deref `connect()`'s sockaddr; Landlock net needs kernel 6.7 > 5.13 floor). → FORK-1
  re-decided: child stays fully net-denied, destination pin is broker-mediated
  resolve-and-pin; defer git.push if no unprivileged mechanism proves feasible.
- **MAJOR-2:** push confirm surfaced only routing, not payload → §2.7 surfaces pushed
  diff + tainted-file provenance.
- **MAJOR-3:** captured push stderr could leak credential/URL into a minted taint value →
  §2.5 opaque/scrub discipline + regression test.
- **MAJOR-4:** github.pr POST base-URL not SSRF-pinned → §4.1 fixed broker trusted-config +
  §3.6 resolve-and-pin.
- **MINOR-5 / NIT-6:** CAS crash-window residual documented; `url` marked content-sensitive.
- Citation audit: ALL spot-checked `file:line` citations accurate.

**Round 2 — all 6 resolved; one new editorial MAJOR** (three summary/framing passages still
asserted the rejected net-allowed-child model) → reconciled all four passages to the
net-denied + broker-mediated model. Round-2 citations all accurate.

**Gate CLEARED** — recorded in `planning-docs/DESIGN-GATE-RECORD-v1.8.md`. Commits:
`5a113a7` (round-1 fixes), `caef6cf` (round-2 reconciliation + cleared record).

## Verification

- `scripts/check-invariants.sh` exits 0.
- `git status --porcelain crates/ cli/` empty — NO TCB code written this phase.
- The design-gate discipline (unbroken v1.0 P2 → v1.7 P31) is satisfied; Phases 36-40 unblocked.

## Standing corrections carried into Phases 36-40
See `DESIGN-GATE-RECORD-v1.8.md` "GATE CLEARED" section — 6 numbered corrections, notably
git.push net-denied-child + broker-mediated pin (defer if infeasible), the §2.7 payload
visibility, §2.5 scrub, §4.1 base-URL pin, the P33/P34 prepare_* + entry-guard, and the
Gate-3 `mint_from_http(` extension.

**Deviation from plan:** 35-02 modeled the review as executor-spawned; per the roadmap
success criterion + milestone constraint, the orchestrator ran it directly (fresh Fable-5,
non-self). This is stronger than planned, not weaker.
