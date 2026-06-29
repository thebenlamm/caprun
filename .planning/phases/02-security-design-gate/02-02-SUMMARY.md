---
phase: 02-security-design-gate
plan: "02"
subsystem: security-design
tags: [design, executor, taint, value-injection, i2]
status: complete

dependency_graph:
  requires:
    - 02-01-SUMMARY.md  # DESIGN-taint-model.md (sibling plan, wave 1)
  provides:
    - planning-docs/DESIGN-plan-executor.md
  affects:
    - Phase 4 crates/executor (hard-gated until DESIGN-GATE-RECORD.md approved)
    - planning-docs/DESIGN-GATE-RECORD.md (Phase 2 Plan 03 gate artifact)

tech_stack:
  added: []
  patterns:
    - Monotonic taint propagation through plan DAG
    - Deterministic non-LLM executor decision rule (Block/Proceed)
    - Literal-value human confirmation UX via FAMP
    - CaMeL-convergent ValueNode/PlanNode API shape

key_files:
  created:
    - planning-docs/DESIGN-plan-executor.md
  modified: []

decisions:
  - "v0 sink sensitivity map is hardcoded in Rust — no Cedar/schema system (RESOLVES A4)"
  - "v0 plan DAG is a linear PlanNode sequence — branching DAG is post-v0 (RESOLVES A2)"
  - "DESIGN-plan-executor.md references taint-edge requirement; Event schema lives with brokerd in Phase 3 (RESOLVES open question 2)"
  - "subject/body of email.send are non-sensitive args; only to/cc/bcc are sensitive recipient args"

metrics:
  duration_minutes: 12
  completed_date: "2026-06-29"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 0
---

# Phase 02 Plan 02: DESIGN-plan-executor.md Summary

**One-liner:** Formal spec for I2 value-injection defense — ValueNode/PlanNode schemas, v0 hardcoded email.send sensitivity map, monotonic taint propagation rules, deterministic non-LLM Block/Proceed decision logic, taint provenance requirement, and FAMP literal-value confirmation UX.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Author DESIGN-plan-executor.md with all ten required sections | 18bc27b | planning-docs/DESIGN-plan-executor.md |

## What Was Built

`planning-docs/DESIGN-plan-executor.md` — the formal security specification for the deterministic plan executor that closes the value-injection gap (I2). The document contains exactly the ten required sections:

1. **The Problem Being Solved** — explains why the planner/worker split (I1) defeats instruction injection but NOT value injection; states that without the executor, value injection is not defeated; includes the CaMeL convergence table.
2. **ValueNode Schema** — specifies three fields: `literal` (actual value), `provenance` (EventId of read Event in audit DAG), `taint` (list of labels e.g. `external.untrusted`, `email.raw`); states taint MUST originate from the read Event, never hand-set at the sink.
3. **PlanNode Schema** — specifies `sink` (SinkId) and `args` (list of named ValueNodes); states the locked broker API `submit_plan_node(session_id, plan_node) -> ExecutorDecision`; prohibits raw `EffectRequest { effect, args: Map }` paths to sinks.
4. **Plan DAG Structure** — states v0 uses a linear `Vec<PlanNode>` sequence (sufficient for §9); resolves research assumption A2; defers branching DAG to post-v0.
5. **Sink Sensitivity Map (v0: hardcoded)** — specifies `email.send` with `to`/`cc`/`bcc` as sensitive (recipient args an attacker can redirect), `subject`/`body` as non-sensitive (content args); `effect_class: CommitIrreversible`, `tier: 4`; hardcoded in Rust, no Cedar.
6. **Taint Propagation Rules** — five explicit rules: monotonic (labels never removed), per-dataflow-edge, originate at read Events, no provenance-less taint accepted, executor never adds taint labels.
7. **Executor Decision Logic** — deterministic non-LLM rule: for each `(arg_name, value_node)` in PlanNode.args, if `arg_name` is in `sensitive_args` AND `value_node.taint` is non-empty → return `Block`; otherwise → `Proceed`. Pure function, no LLM in enforcement path.
8. **Taint Provenance Requirement** — dedicated section: taint MUST originate from a read Event in the audit DAG; hand-set or stapled taint is invalid; §9 fails if taint is not genuine; explains the taint-stapling attack; resolves open question 2 (Event schema lives with brokerd in Phase 3).
9. **Literal-Value Confirmation UX** — states exact prompt format: "Proposed recipient `<literal>` came from untrusted content. Confirm this exact address to proceed."; delivered via FAMP; no auto-confirm for Tier 3+ values from tainted sources; standing policy may permit exact values but cannot disable I2 globally.
10. **Relationship to Broker's Callability Gate & Done-When** — states broker gate (is sink callable?) and executor gate (is tainted value permitted in sensitive arg?) are independent; both must pass; includes the Done-When predicate checklist.

## Deviations from Plan

None — plan executed exactly as written. All ten sections authored as specified with MUST/MUST NOT predicate language. All 15 acceptance criteria grep checks pass.

## Threat Model Coverage

The three threats from the plan's threat model are addressed:

| Threat | Mitigation in Document |
|--------|----------------------|
| T-02-03: Sink sensitivity undefined | §5 "Sink Sensitivity Map (v0: hardcoded)" specifies `email.send` with named sensitive args `to`/`cc`/`bcc` — hardcoded, not deferred |
| T-02-04: Taint provenance omitted (taint-stapling) | §8 "Taint Provenance Requirement" is a dedicated section containing both "read Event" and "hand-set"/"stapled" terms |
| T-02-05: Schema-valid malicious value treated as safe | §1 "The Problem Being Solved" explicitly states schema validation checks shape not truth; value injection is NOT defeated by the split alone |

## Known Stubs

None — this plan produces a specification document, not runtime code. No stubs exist.

## Threat Flags

None — this plan authors a markdown spec. No new runtime trust boundaries, network endpoints, auth paths, or schema changes are introduced by the deliverable itself.

## Self-Check: PASSED

- `planning-docs/DESIGN-plan-executor.md` exists: VERIFIED
- Task commit `18bc27b` exists: VERIFIED (`git log --oneline | grep 18bc27b`)
- All 15 acceptance criteria grep checks: PASSED (run immediately after file creation)
- No `crates/executor` files created: VERIFIED
