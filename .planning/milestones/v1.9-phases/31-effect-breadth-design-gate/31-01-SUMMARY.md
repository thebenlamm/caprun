---
phase: 31-effect-breadth-design-gate
plan: 01
subsystem: design-gate
tags: [landlock, seccomp, pre_exec, openat2, taint-model, i2, slot-type-binding, process-exec, filesystem]

# Dependency graph
requires:
  - phase: 23-slot-type-binding-design-gate
    provides: "DESIGN-slot-type-binding.md — the §-numbered section shape, origin_role tag mechanism, and expected_role fail-closed contract this doc extends to two new sinks"
  - phase: 07-value-injection-design-gate (v0)
    provides: "DESIGN-taint-model.md / DESIGN-plan-executor.md — the mint-time taint discipline (anti-stapling) and the PlanNode{sink,args} handle model this doc's exec-output mint follows"
provides:
  - "planning-docs/DESIGN-effect-breadth-exec.md (§0-§10) — the pinned process.exec broker-spawned confined-child model, exec-output taint mint, fs read/write breadth model, I2/slot-type-binding table entries for both new sinks, and the fail-closed defaults table"
affects: [32-process-exec-implementation, 33-filesystem-breadth-implementation, 34-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pre_exec-based confinement (broker applies Landlock/seccomp/rlimits inside Command::pre_exec, BEFORE the arbitrary child's own execve) — the first genuinely new confinement ordering since v1.0 M0's worker self-confinement"
    - "sensitivity-classified sink args (routing- AND content-sensitive) as the pattern for any future sink whose own args carry attacker-consequential authority, not just its payload"

key-files:
  created:
    - planning-docs/DESIGN-effect-breadth-exec.md
  modified: []

key-decisions:
  - "process.exec spawn ownership: Option A (broker pre_exec's the child inline) is the v1.7 default; Option B (dedicated caprun-exec-launcher binary) is the documented fallback if the fresh adversarial reviewer rules the pre_exec async-signal-safety residual a blocker."
  - "process.exec's own command AND args are classified BOTH routing- AND content-sensitive (never expected_role=None) — a tainted command is arbitrary code execution, strictly worse than a tainted email recipient."
  - "No process.exec command allowlist for v1.7 — confinement (Landlock/seccomp/rlimits/network-deny) is the sole control; deferred to a future POL-01 policy milestone."
  - "The exec child's Landlock ruleset is a NEW narrow-allow-list constructor, never deny_all_filesystem() reused verbatim (it has zero allow-rules and would block the target binary from loading)."
  - "Recursion-exec-deny is the fail-closed default for the exec child's own descendants — no shell-script/nested-exec support in v1.7."
  - "fs write/edit sink uses O_WRONLY|O_TRUNC (no O_CREAT, no O_EXCL) — ENOENT on a missing target, never silently creates — kept structurally distinct from file.create's O_CREAT|O_EXCL new-file-only semantics."
  - "No per-session RequestFd read-count limiter exists today (confirmed by a full read of the handler, not a grep miss) — this doc pins that Phase 33 MUST add one as a resource-exhaustion guard, even though multi-file read is otherwise already mechanically supported by the existing single-file path called N times."

patterns-established:
  - "Fail-closed defaults table (§5): one row per new sink arg with an explicit deny-unless-allowed posture — mirrors DESIGN-slot-type-binding.md §3's expected-role table shape, generalized to non-role dimensions (schema, taint, kernel confinement)."

requirements-completed: [DESIGN-13, DESIGN-14]

coverage:
  - id: D1
    description: "DESIGN-effect-breadth-exec.md pins the broker-spawned confined-child process.exec model (spawn ownership, kernel confinement, stdout/stderr capture, exec-output taint mint) and the fs read/write-breadth model"
    requirement: "DESIGN-13"
    verification:
      - kind: other
        ref: "grep -q 'broker-spawned' && grep -q 'mint_from_exec' && grep -q 'pre_exec' planning-docs/DESIGN-effect-breadth-exec.md"
        status: pass
    human_judgment: true
    rationale: "Design correctness (does the model actually close the threats it claims to) is a judgment call requiring a fresh non-self adversarial code-trace (Plan 31-02), not a grep assertion. The grep confirms presence/structure only."
  - id: D2
    description: "Fail-closed defaults table for both new sinks (exec command/arg schema + posture, exec-output taint label + origin_role, fs read/write path & slot constraints); process.exec command/args classified routing- AND content-sensitive; mandated check-invariants.sh Gate 3 extension for mint_from_exec("
    requirement: "DESIGN-14"
    verification:
      - kind: other
        ref: "bash scripts/check-invariants.sh"
        status: pass
      - kind: other
        ref: "grep -q 'O_WRONLY' && grep -qi 'content-sensitive' && grep -q 'openat2' && grep -q 'Gate 3' planning-docs/DESIGN-effect-breadth-exec.md"
        status: pass
    human_judgment: true
    rationale: "Whether the pinned fail-closed defaults are genuinely sound (not merely present) is the subject of Plan 31-02's fresh adversarial code-trace, per this project's standing design-gate precedent."

duration: ~35min
completed: 2026-07-17
status: complete
---

# Phase 31 Plan 01: Effect-Breadth Design Gate — DESIGN Doc Summary

**Authored `planning-docs/DESIGN-effect-breadth-exec.md` (§0-§10, ~730 lines): pins the broker-spawned `pre_exec`-confined-child model for `process.exec` (Option A default, Option B launcher fallback), the sole `mint_from_exec` taint-mint site rooted at a new `process_exited` Event, the `O_WRONLY|O_TRUNC` fs write/edit sink, and a complete fail-closed defaults table — with `process.exec`'s own command/args pinned routing- AND content-sensitive under the unmodified I2 collect-then-Block loop.**

## Performance

- **Duration:** ~35 min (tool-timestamp span; actual session included extensive code-tracing)
- **Started:** 2026-07-17T20:33:59Z
- **Completed:** 2026-07-17T20:39:37Z (final task-3 commit); SUMMARY authored immediately after
- **Tasks:** 3/3 completed
- **Files modified:** 1 (`planning-docs/DESIGN-effect-breadth-exec.md`, created)

## Accomplishments

- Wrote the complete `process.exec` broker-spawned confined-child model (§1): spawn ownership (Option A/B), a NEW narrow-allow Landlock ruleset (explicitly not `deny_all_filesystem()` reused verbatim), reused seccomp network-deny with no execve-deny for the one legitimate exec + recursion-exec-deny, reused rlimits + a NEW wall-clock timeout + captured-output byte cap, and the argv-array (never `sh -c`) command schema.
- Wrote the sole exec-output taint mint site (§2): a new `mint_from_exec` helper mirroring `mint_from_read`'s exact non-stapled shape, `TaintLabel::ExecRaw` + `ExternalUntrusted`, `origin_role = Some("exec_output")`, and mandated the `check-invariants.sh` Gate 3 extension Phase 32 must add (Gate 3 today would NOT catch a new `mint_from_exec(` call site).
- Wrote the filesystem read/write-breadth model (§3): multi-file read as the existing `RequestFd` path invoked N times (with a NEW per-session read-count upper bound pinned, since none exists today), and the write/edit sink as `O_WRONLY|O_TRUNC` (no `O_CREAT`/`O_EXCL`, `ENOENT`-on-missing).
- Wrote the I2/slot-type-binding table entries for both new sinks (§4) — both stay on the unmodified `submit_plan_node` collect-then-Block loop, table entries only — with `process.exec` command/args pinned routing- AND content-sensitive (the single highest-consequence decision in the doc).
- Wrote the complete fail-closed defaults table (§5), the security-invariant checklist (§6), validation architecture pointer (§7), open items (§8, deployment constants deferred), accepted residual risks (§9, incl. the `pre_exec` async-signal-safety residual), and the checkable Acceptance Predicate (§10).
- Verified `scripts/check-invariants.sh` exits 0 (all 4 gates PASS) after the doc's completion.

## Task Commits

Each task was committed atomically, doc-only (`planning-docs/` scope, no `crates/`/`cli/` changes):

1. **Task 1: §0-§2 — Purpose/Scope, process.exec confined-child model, exec-output taint mint** - `8e75c8e` (docs)
2. **Task 2: §3-§5 — fs read/write breadth, I2/slot-type binding, fail-closed defaults table** - `70d3787` (docs)
3. **Task 3: §6-§10 — invariant checklist, validation pointer, open items, residual risks, acceptance predicate; ran check-invariants.sh** - `45be5cc` (docs)

_No TDD tasks — this is a doc-only design-gate plan._

## Files Created/Modified

- `planning-docs/DESIGN-effect-breadth-exec.md` - New v1.7 design-gate doc, §0-§10, pinning the `process.exec` confined-child model + exec-output taint mint + fs read/write breadth model + I2/slot-type-binding table entries + fail-closed defaults table, per DESIGN-13/DESIGN-14.

## Decisions Made

See `key-decisions` in frontmatter above. Summarized: Option A (`pre_exec`-inline) for exec-child spawn with Option B (dedicated launcher binary) as the documented fallback; `process.exec` command/args classified routing- AND content-sensitive (not unconstrained); no command allowlist for v1.7 (confinement is the sole control); a NEW narrow-allow Landlock ruleset (never `deny_all_filesystem()` reused verbatim) for the exec child; recursion-exec-deny as the fail-closed default; the fs write/edit sink is `O_WRONLY|O_TRUNC` only, structurally distinct from `file.create`'s `O_CREAT|O_EXCL`; and a NEW per-session `RequestFd` read-count limiter is required even though no limiter exists today and multi-file read is otherwise already mechanically supported.

## Deviations from Plan

None — plan executed exactly as written. Every `<read_first>` code citation was independently re-verified against live code during authoring (not copied verbatim from RESEARCH.md's approximate line ranges); all citations in the final doc reflect the actual file:line observed this session (e.g. `sink_sensitivity.rs:163-164` for the `file.create` path role entry, `sink_sensitivity.rs:176` for the `contents` role entry, `main.rs:372-378` for the `child.kill()` teardown path) rather than RESEARCH.md's approximate ranges, which is expected per this project's "re-verify file:line, don't trust research at authoring time" convention.

## Issues Encountered

None. The `RequestFd` handler read (`crates/brokerd/src/server.rs:1229-1394`) confirmed RESEARCH's Open Question 1 finding directly: no per-session read-count limiter exists, which the doc now pins as a required fail-closed addition for Phase 33.

## User Setup Required

None - no external service configuration required (doc-only phase, no code, no dependencies).

## Next Phase Readiness

- `planning-docs/DESIGN-effect-breadth-exec.md` is complete and ready for Plan 31-02's fresh non-self adversarial code-trace review, which produces `planning-docs/DESIGN-GATE-RECORD-v1.7.md` and is owned by the orchestrator (not a `gsd-executor`), per `.planning/REQUIREMENTS.md`'s standing precedent.
- No `crates/executor` / `crates/brokerd` / `crates/sandbox` / `crates/runtime-core` code exists yet — confirmed via `git status --porcelain crates/ cli/` (empty) after every task commit.
- `scripts/check-invariants.sh` exits 0 (all 4 gates PASS) — no architectural-invariant regression from this doc's prose.
- Phase 32 (process.exec implementation) is blocked until Plan 31-02's gate clears with all findings resolved.

---
*Phase: 31-effect-breadth-design-gate*
*Completed: 2026-07-17*

## Self-Check: PASSED

- FOUND: `planning-docs/DESIGN-effect-breadth-exec.md` (734 lines, §0-§10 all present)
- FOUND: commit `8e75c8e` (Task 1: §0-§2)
- FOUND: commit `70d3787` (Task 2: §3-§5)
- FOUND: commit `45be5cc` (Task 3: §6-§10)
- `bash scripts/check-invariants.sh` exits 0 (Gates 1-4 all PASS)
- `git status --porcelain crates/ cli/` is empty — no TCB code written this plan
