---
phase: 02-security-design-gate
plan: "01"
subsystem: security-design
tags: [taint-model, security-spec, design-gate, prompt-injection-defense]
dependency_graph:
  requires: []
  provides: [DESIGN-taint-model.md, REQ-design-taint-model]
  affects: [planning-docs/DESIGN-GATE-RECORD.md, crates/executor (gated)]
tech_stack:
  added: []
  patterns: [dynamic-taint, planner-worker-split, audit-dag-genuine-taint, information-flow-control]
key_files:
  created:
    - planning-docs/DESIGN-taint-model.md
  modified: []
decisions:
  - "Dynamic taint is the default I1 enforcement mode; hard planner/worker split reserved for Tier 3+ tasks only"
  - "Genuine-taint requirement: taint MUST originate from a read Event in the audit DAG, never hand-set at the sink"
  - "Two-attack-mode distinction captured: instruction injection (split defeats) vs value injection (I2/executor defeats, not the split)"
  - "v0 taint label vocabulary defined as minimal set: external.untrusted and email.raw only"
  - "Three accepted residual risks documented: steganographic encoding, planner/intent-creation injection, fd revocation after SCM_RIGHTS"
metrics:
  duration: "146s (2m 26s)"
  completed_date: "2026-06-29"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 0
status: complete
---

# Phase 02 Plan 01: DESIGN-taint-model.md Formal Security Spec Summary

**One-liner:** Authored `planning-docs/DESIGN-taint-model.md` — the dynamic taint model security spec with all nine required sections, formal MUST/MUST NOT invariant predicates, two-attack-mode threat model, genuine-taint requirement, and I0/I1/I2 invariants sourced verbatim from PLAN.md.

---

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Author DESIGN-taint-model.md with all nine required sections | a3cba2d | planning-docs/DESIGN-taint-model.md |

---

## Acceptance Criteria Results

All 21 grep-based acceptance criteria passed against `planning-docs/DESIGN-taint-model.md`:

| Check | Result |
|-------|--------|
| File exists | PASS |
| Dynamic-taint default stated | PASS |
| Draft-only behavior stated | PASS |
| Planner/worker split stated | PASS |
| Tier 3 reference present | PASS |
| Quarantined worker defined | PASS |
| I0 invariant present | PASS |
| Untrusted reference present | PASS |
| I1 invariant present | PASS |
| Read Event stated | PASS |
| Hand-set / stapled wording present | PASS |
| Instruction injection stated | PASS |
| Value injection stated | PASS |
| "Necessary but not sufficient" framing | PASS |
| Tier 0 defined | PASS |
| Tier 4 defined | PASS |
| `external.untrusted` label defined | PASS |
| `email.raw` label defined | PASS |
| Residual risks documented | PASS |
| REQ-design-taint-model traced | PASS |
| No file under crates/executor/ | PASS |

---

## Decisions Made

1. **Dynamic taint as default, split reserved for Tier 3+.** Confirmed from PLAN.md §Security Model. The hard planner/worker split is NOT a blanket default — it is reserved for tasks that touch Tier 3+ sinks.

2. **Genuine-taint requirement.** Taint labels on a ValueNode MUST trace to a read Event in the audit DAG. Hand-setting taint at the sink is explicitly named as the failing case for the §9 acceptance test.

3. **Two-attack-mode distinction.** Instruction injection is defeated by the split; value injection is NOT defeated by the split and requires I2 (the plan executor). "The planner/worker split is necessary but not sufficient" is stated explicitly in the doc.

4. **Minimal taint label vocabulary for v0.** Only `external.untrusted` and `email.raw` are defined. Label extensibility is post-v0.

5. **Three residual risks accepted for v0.** Steganographic encoding, planner/intent-creation injection, and fd revocation after SCM_RIGHTS are documented as accepted risks with v0 mitigations.

---

## Document Structure

The nine required sections are present with these exact headings:

1. `## Invariant Statements` — I0, I1, I2 with MUST/MUST NOT predicates
2. `## Default Taint Model — Dynamic Taint` — rule, rationale, scope
3. `## High-Risk Mode — Hard Planner/Worker Split (Tier 3+)` — privileged planner, quarantined worker, trigger
4. `## Trust Tier Definitions` — Tier 0–4 table + human involvement rules
5. `## Worker Output Contract` — typed/lossy extract fields, I2 gap explained
6. `## Taint Label Vocabulary (v0)` — external.untrusted, email.raw, monotonicity, v0 hardcoded map
7. `## Threat Model — I1 Attack Surface` — three attack modes, split necessary-but-not-sufficient
8. `## Accepted Residual Risks` — three risks documented with v0 mitigations
9. `## Genuine-Taint Requirement & I0 Acceptance Predicate` — DAG provenance requirement + I0 done-when predicate

Prior art cited: CaMeL (arXiv 2503.18813) P-LLM/Q-LLM split; FIDES (arXiv 2505.23643) IFC. AgentOS audit-DAG genuine-taint requirement distinguishes this design.

---

## Deviations from Plan

None — plan executed exactly as written. All nine sections authored with all acceptance criteria met on first pass.

---

## Known Stubs

None — this plan produces a design document, not code. No data sources or UI components are involved.

---

## Threat Flags

No new runtime threat surface was introduced. This plan authors a markdown spec only. The STRIDE threats for the system being designed are documented inside the DESIGN-taint-model.md §Threat Model section. Threats T-02-01 and T-02-02 from the plan's threat model are mitigated: all required invariant strings are present (T-02-01), and both "read Event" and "hand-set"/"stapled" strings are present (T-02-02).

---

## Self-Check: PASSED

- `planning-docs/DESIGN-taint-model.md` exists: CONFIRMED
- Commit `a3cba2d` exists: CONFIRMED
- No files under `crates/executor/`: CONFIRMED
- All 21 acceptance criteria grep checks: PASS
