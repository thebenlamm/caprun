---
created: 2026-07-08T15:35:00.000Z
title: GSD plan executors can self-mark phase-level completion before verification runs
area: tooling
files:
  - .claude/gsd-core/workflows/execute-phase.md
---

## Problem

The 15-04 executor's own doc-completion commit (`3205b13`, "docs(15-04):
complete confined multi-fragment extraction plan") checked Phase 15's
top-level `- [ ]` box to `- [x]` in `.planning/ROADMAP.md`, with a
"(completed 2026-07-08)" annotation — before any independent verification
(live-Linux re-run, gsd-verifier goal-backward pass) had happened. Only the
individual plan-level checkbox (`- [x] 15-04-PLAN.md — ...`) should be an
executor's business; the phase-level line is the orchestrator's to flip,
and only after verification passes.

Caught by the orchestrating session during the post-merge diff review
(commit `ff8b6ba` reverts it to `- [ ]` with an accurate "4/4 plans
executed, independent verification pending" note) before it could be acted
on. Had it slipped past, the phase would have read DONE in ROADMAP.md with
zero verification behind it — a false-positive completion state, silently
trusting an executor's self-report exactly where the project's standing
practice ("never let the executor's own SUMMARY stand as proof") says not
to.

This is the same class of hazard as the `phases.clear` deletion bug
(`2026-07-07-gsd-phases-clear-deletes-all-milestones.md`): a GSD tooling/
workflow gap that lets an executor mutate a piece of shared planning state
it should never touch.

## Suggested fix

Executors should only ever check off their OWN plan-level line
(`- [x] {plan-id}-PLAN.md — ...`) in `.planning/ROADMAP.md`, never the
parent phase-level line or its "(completed ...)" annotation. Phase-level
completion should be written exclusively by the orchestrator's
`phase.complete` verb (or whatever verified-completion path the
execute-phase workflow uses), gated on verification having actually run
and passed. Consider a mechanical check (grep-based, mirroring
`check-invariants.sh`'s own style) that fails a wave's merge if a
worktree's diff to ROADMAP.md touches a phase-level checkbox line rather
than only its own plan-level line.

Not filed as a Claude Code / GSD upstream issue yet — recording here per
caprun-opus-77's instruction so it isn't lost before that happens.
