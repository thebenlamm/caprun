---
phase: 08-session-trust-confirmation-design-gate
plan: 01
subsystem: design-docs
tags: [design-gate, session-trust, I0, I1, executor, taint]
dependency-graph:
  requires:
    - planning-docs/DESIGN-taint-model.md (APPROVED, referenced/extended)
    - planning-docs/DESIGN-plan-executor.md (APPROVED, referenced/extended)
  provides:
    - planning-docs/DESIGN-session-trust-state.md (Draft — pending DESIGN-GATE-RECORD-v1.2.md approval)
  affects:
    - Phase 9 (TAINT-01..04, ORIGIN-01..02 implementation)
    - Phase 8 Plan 03 (gate-record artifact, will reference this doc's sha256)
tech-stack:
  added: []
  patterns:
    - "MUST/MUST NOT normative-verb-only design doc (mirrors DESIGN-taint-model.md / DESIGN-plan-executor.md)"
    - "Hardcoded per-sink lookup table pattern (sink_effect_class mirrors is_routing_sensitive)"
    - "Single-taxonomy DenyReason extension (append, never a second enum)"
    - "Trusted-path-only / anti-self-declaration framing extended from I0 to I1"
key-files:
  created:
    - planning-docs/DESIGN-session-trust-state.md
  modified: []
decisions:
  - "SessionStatus::Draft transition is one-way monotonic (Active -> Draft only); no Draft -> Active path is specified or permitted in this document's scope"
  - "session_status is a new explicit parameter on submit_plan_node, broker-resolved from its own session store — never carried on PlanNode or trusted from IPC"
  - "sink_effect_class is a new hardcoded function in the executor crate (not a PlanNode field) mapping email.send and file.create to EffectClass::CommitIrreversible; unknown sinks fail closed as CommitIrreversible (currently dead code given Step 0's schema gate, but documented for future-proofing)"
  - "Draft-only deny for CommitIrreversible sinks runs as Step 0.5 in submit_plan_node — after schema validation, before the per-arg loop — decided in exactly one executor TCB function, never a broker pre-check"
  - "DraftOnlySessionDeniesCommitIrreversible appended to the existing single DenyReason taxonomy, carrying the offending SinkId"
metrics:
  duration: ~35min
  completed: 2026-07-02
status: complete
---

# Phase 8 Plan 01: Session-Trust-State DESIGN Doc Summary

Authored `planning-docs/DESIGN-session-trust-state.md` — a 446-line MUST/MUST-NOT invariant spec extending the APPROVED `DESIGN-taint-model.md`/`DESIGN-plan-executor.md` pair with the I1 dynamic-demotion mechanism, the I0 creation-time draft rule, the new broker-resolved `session_status` executor parameter, the hardcoded `sink_effect_class` table, and the `DraftOnlySessionDeniesCommitIrreversible` deny path — precise enough for Phase 9 to implement TAINT-01..04/ORIGIN-01..02 without new design decisions.

## What Was Built

Two tasks, both writing to the same file (Task 1 creates, Task 2 appends), mirroring the existing `DESIGN-taint-model.md` structure exactly:

**Task 1 — Session-trust-state invariant + threading (`planning-docs/DESIGN-session-trust-state.md` §1-5):**
- `SessionStatus::Draft` variant spec: added to the existing `Active | WaitingApproval | Done | Failed | RolledBack` enum, with a one-way monotonic `Active → Draft` transition rule (mirrors taint monotonicity).
- I1 dynamic-demotion invariant: `mint_from_read` (`crates/brokerd/src/quarantine.rs`) is specified as the sole trust-flip site, co-located with the existing tainted-`ValueRecord` mint, trusted-path-only (never worker-declared) — near-verbatim extension of `DESIGN-taint-model.md`'s I0 anti-self-declaration phrasing.
- I0 creation rule: a seed-provenance field at `create_session` time, determined by the `caprun` CLI (trusted-arg vs file-derived), set by the broker's trusted `create_session` path — never self-declared. Names the expected new CLI on-ramp shape (`--seed-from-file <path>` or a third intent-parsing branch) since today's CLI has no file-derived-seed input path at all (Pitfall 5).
- Executor trust-state threading: specifies `submit_plan_node`'s new signature with an explicit `session_status: &SessionStatus` parameter, broker-resolved from its own session store — never carried on `PlanNode` or trusted from IPC (mirrors HARD-03's `session_id` discipline).
- Two-table audit contract (TAINT-04): an atomic pair — `UPDATE sessions SET status = 'Draft'` on the mutable read-model row plus an append-only `session_demoted` Event whose `parent_id` equals the triggering `file_read` Event id — both in the same lock/transaction so the read-model and ledger can never disagree.

**Task 2 — Executor deny mechanism + acceptance predicate (`planning-docs/DESIGN-session-trust-state.md` §6-12):**
- `sink_effect_class(sink: &SinkId) -> EffectClass` — new hardcoded function mirroring `is_routing_sensitive`/`is_content_sensitive`, mapping `email.send` and `file.create` to `CommitIrreversible`; PlanNode shape stays locked (no new field). Unknown-sink handling is explicitly fail-closed (treated as `CommitIrreversible`, not a permissive default), documented as currently-dead-code given Step 0's existing schema gate but future-proofed against a reordering refactor.
- `DraftOnlySessionDeniesCommitIrreversible { sink: SinkId }` appended to the ONE existing `DenyReason` taxonomy in `crates/runtime-core/src/executor_decision.rs`.
- "Step 0.5" placement in `submit_plan_node`: runs immediately after schema validation (Step 0), before the per-arg resolve/taint loop, since it depends only on session status + sink effect class. Decided in exactly one executor TCB function — never a broker pre-check.
- Non-regression MUSTs: `Draft` sessions still allow `MutateReversible`/`Observe` plan nodes (TAINT-03); the new check does not alter the existing I2 routing-sensitivity block (protects the v1.1 §9 acceptance unchanged).
- Exhaustive-match discipline mandated for `EffectClass` and `DenyReason` (no wildcard arm), mirroring `TaintLabel::is_untrusted()`'s existing discipline.
- Numbered "I1/I0 Acceptance Predicate (Done When)" (5 conditions, starting at 0 — trust state is broker-resolved, never self-declared) and an "Accepted Residual Risks" section (demotion race under future multi-worker concurrency; unknown-sink fail-closed choice currently unreachable).

## Verification

All plan-specified acceptance-criteria greps pass with margin:

| Grep | Threshold | Actual |
|------|-----------|--------|
| `SessionStatus::Draft` | ≥1 | 7 |
| `mint_from_read` | ≥1 | 9 |
| `seed-provenance` (case-insensitive) | ≥1 | 4 |
| `session_demoted` | ≥1 | 4 |
| `parent_id` | ≥1 | 5 |
| `session_status` | ≥1 | 12 |
| anti-self-declaration phrasing | ≥1 | 9 |
| `sink_effect_class` | ≥1 | 13 |
| `DraftOnlySessionDeniesCommitIrreversible` | ≥1 | 3 |
| `EffectClass` | ≥1 | 12 |
| `CommitIrreversible` (case-insensitive) | ≥1 | 18 |
| `Step 0.5` (case-insensitive) | ≥1 | 12 |
| `exhaustive` (case-insensitive) | ≥1 | 2 |
| `Done When`/`Acceptance Predicate` | ≥1 | 5 |
| `MUST` count | ≥20 | 67 |
| executor-decides phrasing | ≥1 | 5 |
| `**Gate:**` header line | present | present |

Spot-checked for load-bearing `should` — none found (MUST/MUST NOT are the sole normative verbs).

## Deviations from Plan

None — plan executed exactly as written. Both tasks' `<action>` instructions were followed section-by-section against the exact source-code shapes read in `<read_first>` (current `SessionStatus` enum, current `DenyReason` enum, current `submit_plan_node` signature, current `sink_sensitivity.rs` pattern, current `mint_from_read`/`persist_session` code).

## Known Stubs

None. This plan produces only a design document — no code, no data flow, no UI.

## Threat Flags

None. This phase's threat register (T-08-01..04) is fully addressed by the doc's content itself — the mitigations specified (trusted-path-only demotion, executor-only deny placement, atomic causal audit edge, exhaustive-match discipline) are the deliverable, not an external code change requiring a separate flag.

## Self-Check: PASSED

- `planning-docs/DESIGN-session-trust-state.md` — FOUND (446 lines)
- Commit `3dc4f97` (Task 1) — FOUND in `git log --oneline`
- Commit `a3824d7` (Task 2) — FOUND in `git log --oneline`
