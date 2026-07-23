---
phase: 31-effect-breadth-design-gate
plan: 02
subsystem: security-design-gate
tags: [design-gate, adversarial-review, process-exec, seccomp, landlock, taint, I2, gate-record]

requires:
  - phase: 31-01
    provides: planning-docs/DESIGN-effect-breadth-exec.md (the doc under review)
provides:
  - A fresh non-self Fable-5 adversarial code-trace of the effect-breadth DESIGN doc
  - Round-1 amendments folding all confirmed findings (B1/M3/M1/M2/m1/n1) into the DESIGN doc §11
  - planning-docs/DESIGN-GATE-RECORD-v1.7.md (gate CLEARED) — the hard authorization for Phases 32-34
affects: [Phase 32 (EXEC-01..04), Phase 33 (FS-01..03), Phase 34 (LIVE-01/02)]

tech-stack:
  added: []
  patterns:
    - "fresh-context-adversarial-review: orchestrator-owned Fable-5 spawn (executor has no agent tool); findings re-verified against live code before folding"
    - "gate record models on DESIGN-GATE-RECORD-v1.6/v1.5: reviewer independence + files-opened + per-finding claim/evidence/resolution + no-TCB-code reconfirmation"

key-files:
  created:
    - planning-docs/DESIGN-GATE-RECORD-v1.7.md
  modified:
    - planning-docs/DESIGN-effect-breadth-exec.md

key-decisions:
  - "Review spawn is orchestrator-owned (gsd-executor cannot spawn agents); the orchestrator ran Task 1 directly with a fresh Fable-5 reviewer"
  - "B1 (BLOCKER): stateless seccomp BPF cannot deliver recursion-exec-deny — dropped the claim, documented the real Landlock+persistent-net-deny bound"
  - "M3 (MAJOR): flipped the pinned spawn model from Option A (pre_exec) to Option B (launcher post-fork self-confinement) — strictly sounder, retires the async-signal-safety residual"
  - "M1 (MAJOR): pinned mint_from_exec call-site locus to server.rs so it agrees with the mandated Gate-3 {quarantine.rs,server.rs} allow-list"
  - "M2 (MAJOR): pinned process.exec command/args expected_role=None (Some would fail-closed-Deny the legit command at Step 1c); Block comes from sensitivity+taint"

patterns-established:
  - "Design-tightening resolutions only — no invariant weakened to make a finding go away (mirrors v1.6 F1)"

requirements-completed: [DESIGN-13, DESIGN-14]

coverage:
  - id: D1
    description: "Fresh non-self adversarial code-trace of the DESIGN doc cleared it with every finding resolved (DESIGN-13 review clause; DESIGN-14 fail-closed confirmation)"
    requirement: "DESIGN-13"
    verification:
      - kind: manual_procedural
        ref: "planning-docs/DESIGN-GATE-RECORD-v1.7.md — gate status CLEARED, distinct Fable-5 reviewer named, 11 files-opened listed, per-finding claim/evidence/resolution"
        status: pass
    human_judgment: true
    rationale: "Gate clearance is a security-judgment call (design soundness), not an automated assertion; the record documents the independent code-trace and the tightening resolutions."
  - id: D2
    description: "DESIGN-GATE-RECORD-v1.7.md exists with all required sections and reconfirms no TCB code was written this phase"
    requirement: "DESIGN-14"
    verification:
      - kind: other
        ref: "test -f planning-docs/DESIGN-GATE-RECORD-v1.7.md && grep -qi 'CLEARED' ... && [ -z \"$(git status --porcelain crates/ cli/)\" ] && bash scripts/check-invariants.sh"
        status: pass
    human_judgment: false

duration: 22min
completed: 2026-07-17
status: complete
---

# Phase 31 Plan 02: Effect-Breadth Design Gate — CLEARED

**A fresh non-self Fable-5 adversarial code-trace found 1 BLOCKER + 3 MAJOR in the effect-breadth DESIGN doc; all resolved by design-tightening amendments, gate CLEARED, authorizing Phases 32-34.**

## Performance

- **Duration:** ~22 min (review + re-verify + fold + record)
- **Tasks:** 3 (Task 1 orchestrator-owned reviewer spawn; Task 2 re-verify + fold; Task 3 gate record)
- **Files modified:** 2 (`DESIGN-effect-breadth-exec.md` amended §11 + folds; `DESIGN-GATE-RECORD-v1.7.md` created)

## Accomplishments
- **Fresh non-self review (orchestrator-owned):** a distinct Fable-5 agent code-traced the DESIGN doc against 11 live files (crates/{sandbox,brokerd,executor,runtime-core,adapter-fs} + cli + check-invariants.sh), returning severity-tagged findings with file:line evidence — not a prose skim.
- **All 4 load-bearing findings independently re-verified against live code by the orchestrator** (false-positive discipline); all confirmed REAL, none dismissed.
- **B1 (BLOCKER)** — seccomp is a stateless `SeccompFilter` (`seccomp.rs:62`, execve empty-vec always-match) → the pinned "recursion-exec-deny via the child's own filter" is unrealizable. Resolved: dropped the claim; documented the real bound (Landlock Execute-only-on-enumerated-paths + persistent socket net-deny that survives execve).
- **M3 (MAJOR)** — Option A's `pre_exec` path was pinned on "runs twice without incident," but zero real `.pre_exec(` sites exist (`main.rs:328,356` are the safe no-pre_exec shape). Resolved: flipped pinned default to Option B (launcher post-fork self-confinement, the proven `apply_confinement()` ordering); retires the async-signal-safety residual.
- **M1 (MAJOR)** — Gate-3 loci `{quarantine.rs,server.rs}` contradicted a `sinks/process_exec.rs` mint site. Resolved: pinned mint_from_exec locus to server.rs.
- **M2 (MAJOR)** — mandating `expected_role=Some` for command/args would fail-closed-Deny the legit command at Step 1c (`lib.rs:133-148`). Resolved: pinned `expected_role=None`; Block comes from sensitivity+taint (`lib.rs:156-158`), independent of the role gate — not an I2 bypass.
- **Gate CLEARED** in `DESIGN-GATE-RECORD-v1.7.md`; no TCB code written (`git status crates/ cli/` empty; check-invariants.sh green).

## Task Commits

Executed inline by the orchestrator (plan 31-02 is an orchestrator-owned review plan — the executor role cannot spawn the required independent reviewer). Committed as a single logical unit after all three tasks:

1. **Task 1: Fresh non-self Fable-5 adversarial code-trace** — reviewer spawned by orchestrator; findings + independence metadata (11 files opened, 124.5k tokens / 17 tool uses) captured.
2. **Task 2: Re-verify each finding + fold into DESIGN doc** — B1/M3/M1/M2 confirmed against live code, folded as §11 Round-1 amendments (tightening only).
3. **Task 3: Gate record + no-TCB reconfirmation** — `DESIGN-GATE-RECORD-v1.7.md` CLEARED.

## Files Created/Modified
- `planning-docs/DESIGN-GATE-RECORD-v1.7.md` — the v1.7 gate record (CLEARED), modeled on v1.6/v1.5.
- `planning-docs/DESIGN-effect-breadth-exec.md` — Round-1 amendments folded (§1.3 Option B pinned, §1.4 B1 fix, §2.4 M1 mint-locus, §2.5/§9 residual retired, §4.2/§6 M2 fix, §5 table, §11 Amendments; status → CLEARED).

## Decisions Made
- Executed 31-02 inline as orchestrator rather than via a `gsd-executor` subagent: Task 1 mandates spawning an independent Fable-5 reviewer, which the executor agent type cannot do (no Agent tool). This preserves reviewer independence (orchestrator-owned spawn) per the standing `fresh-context-adversarial-review` precedent, and the delicate TCB-doc surgery for the folds was kept on the lead rather than delegated.
- Chose Option B (flip) over the lighter "keep Option A, fix justification" for M3 — for the riskiest primitive to date, eliminating the async-signal-safety residual beats accepting it; plan 31-02 Task 2 pre-authorized folding Option B as the pinned path.

## Deviations from Plan
None on substance — plan executed as written. Structural note: Tasks 1-3 were run by the orchestrator inline (see Decisions) rather than by a spawned `gsd-executor`, because the plan's own `<critical_reviewer_independence_note>` makes the reviewer spawn orchestrator-owned.

## Issues Encountered
The review returned BLOCK (not an immediate clear) — the expected, healthy outcome of a genuine adversarial gate. Resolved via the standard review→fix→clear loop: all findings folded as design-tightening amendments, no invariant weakened, gate then CLEARED.

## User Setup Required
None.

## Next Phase Readiness
- Phases 32-34 are authorized. The DESIGN doc now pins Option B (launcher) as the exec spawn model, the real (non-seccomp) recursion bound, the server.rs mint locus, and expected_role=None for command/args — Phase 32 implementers should build to the amended §§1.3/1.4/2.4/4.2, not the pre-amendment text.
- Open deployment constants remain (exact Landlock paths for the rust:1 container, RequestFd read-count bound value, kernel-version confirmation) — flagged in §8, resolved at Phase 32.

---
*Phase: 31-effect-breadth-design-gate*
*Completed: 2026-07-17*
