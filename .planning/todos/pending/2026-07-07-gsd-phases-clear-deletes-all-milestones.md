---
created: 2026-07-07T17:02:51.000Z
title: gsd_run phases.clear deletes all milestones' phase dirs, not just previous
area: tooling
files:
  - .claude/gsd-core/workflows/new-milestone.md:205-213
---

## Problem

During `/gsd-new-milestone` (scoping v1.3), step 6 ("Cleanup and Commit") runs
`gsd_run query phases.clear --confirm` unconditionally — not gated behind
`--reset-phase-numbers`. The workflow doc's comment says this clears "leftover
phase directories from the previous milestone," but the actual command deleted
**all 90 git-tracked files across `.planning/phases/01-*` through `11-*`** —
every phase from v1.0, v1.1, and v1.2, not just v1.2's (8-11).

This contradicts observed past behavior: the milestone-start commits for both
v1.1 (`2c719a0`) and v1.2 (`4c8d843`) touched only `PROJECT.md` and
`STATE.md` — neither ever removed `.planning/phases/`. The prior milestone's
per-phase `PLAN.md`/`SUMMARY.md`/`VALIDATION.md`/etc. files (fine-grained
execution history not fully duplicated in `MILESTONES.md`'s terse summary)
survived every previous transition until this one.

Caught before it landed: the deletion was unstaged (git-tracked files, so
technically recoverable via `git checkout --`), and was reverted via
`git checkout -- .planning/phases/` before any commit. The `phases.clear` step
was skipped for the rest of this milestone-init run since there was nothing
legitimately stale to clear.

## Solution

TBD — options to consider when picking this up:
- Scope `phases.clear` to only directories belonging to the milestone that is
  actually being closed/archived (cross-reference against `MILESTONES.md` or
  a milestone→phase-range map), not a blanket wipe of `.planning/phases/`.
- Or: only run `phases.clear` when `--reset-phase-numbers` is active (matching
  the one place in the workflow doc — step 7.5 — where clearing old phase
  dirs is actually described as necessary, to avoid `01-*`/`02-*` collisions
  when numbering restarts at 1). Continuing-numbering mode (the default) never
  needs old phase dirs removed, since new phases get new numbers and don't
  collide.
- Regardless of fix direction, `phases.clear` should never delete dirs for
  phases that are NOT part of the milestone just closed, and ideally should
  refuse to run (or no-op) when nothing is stale rather than clearing
  everything found.

## Recurrence 2 (2026-07-10, v1.4 scoping)

Reproduced identically: `gsd_run query phases.clear --confirm` during
`/gsd-new-milestone` for v1.4 deleted all 187 git-tracked files across
`.planning/phases/01-*` through `17-*` (v1.0 through v1.3's full phase
history) from disk. `phase_archive_path` in the init JSON pointed at
`.planning/milestones/v1.3-phases`, but that directory was never created —
only the terse `v1.3-ROADMAP.md`/`v1.3-REQUIREMENTS.md` summaries already
present in `.planning/milestones/` exist; no actual archive of the per-phase
PLAN/SUMMARY/VERIFICATION files happened. Caught again before any commit
(deletion was unstaged, discovered mid-Phase-18-execution while checking
`git status --porcelain crates/ cli/` for an unrelated verification) and
reverted via `git restore .planning/phases/`. Still unfixed upstream as of
this recurrence — two-for-two now. Given the GSD executor
self-marks-phase-complete bug was also two-for-two before its fix landed,
treat this the same way: after every future `/gsd-new-milestone` run,
immediately `git status .planning/phases/` before doing anything else, and
do not trust that `phases.clear --confirm`'s reported archive path was
actually populated on disk.
