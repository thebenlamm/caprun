# Synthesis Summary

Single entry point for `gsd-roadmapper`. Synthesized from classified planning
docs by `gsd-doc-synthesizer`. Mode: **new** (net-new bootstrap).

## Doc counts by type

- SPEC: 1 (`planning-docs/PLAN.md` — "AgentOS v0 — Definitive Plan", confidence
  high, precedence 0)
- ADR: 0 · PRD: 0 · DOC: 0 · UNKNOWN: 0

Total source docs: 1. The single SPEC is internally reconciled and
self-consistent; its `(DECIDED)` items are authoritative locked decisions.

## Decisions locked (9)

All from `planning-docs/PLAN.md`. See `decisions.md`.

- DEC-platform-linux-only
- DEC-product-boundary
- DEC-security-invariants (I0 / I1 / I2)
- DEC-layer-roles
- DEC-fd-pass-policy
- DEC-terminology
- DEC-architectural-lock-plan-nodes
- DEC-canonical-docs
- DEC-repo-layout

## Requirements extracted (13)

From the build order (M0 / M0-design / M1) and the §9 acceptance test. See
`requirements.md`.

- M0 — Substrate: REQ-runtime-core, REQ-sandbox, REQ-brokerd-core,
  REQ-adapters-fs, REQ-substrate-demo, REQ-api-stub-plan-node
- M0-design (blocks M1 executor code): REQ-design-taint-model,
  REQ-design-plan-executor
- M1 — Security Demo = v0 DONE: REQ-quarantined-reader, REQ-executor-stub,
  REQ-mediated-sink-stub, REQ-approval-hook, REQ-s9-acceptance-test

**v0 DONE gate:** Only REQ-s9-acceptance-test passing = v0 done. Substrate alone
is not sufficient.

## Constraints (11)

See `constraints.md`. Type breakdown:

- api-contract: 1 (CON-broker-api-shape)
- schema: 2 (CON-effect-classes, CON-repo-layout)
- nfr: 3 (CON-stack-tcb, CON-platform-linux-only, CON-i2-non-bypassable)
- protocol: 5 (CON-i1-taint-default, CON-i0-session-creation,
  CON-fd-pass-revocation, CON-s9-taint-genuineness, and the I2 enforcement
  protocol noted under CON-i2-non-bypassable)

## Context topics (6)

See `context.md`: Thesis/framing, Explicitly out of v0, Residual risks, Post-v0
roadmap, Immediate next actions, Excluded cross-references.

## Conflicts

- Blockers: 0
- Competing variants: 0
- Auto-resolved: 0
- Info: 1 (single-SPEC ingest note)

Cycle detection ran; no resolvable cross-doc edges, no cycles. Full report:
`../INGEST-CONFLICTS.md`.

## Per-type intel files

- Decisions: `decisions.md`
- Requirements: `requirements.md`
- Constraints: `constraints.md`
- Context: `context.md`

## Status

READY — no blockers, no competing variants. Safe to route to `gsd-roadmapper`.
