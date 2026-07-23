---
phase: 06-deterministic-planner-intent-input
plan: "01"
subsystem: runtime-core
tags: [rust, serde, taint, executor, caprun-intent, tdd]

requires:
  - phase: 05-runtime-spine-live-9-email-block
    provides: TaintLabel enum (7 variants), ValueId, PlanNode, ValueRecord — all extended here

provides:
  - CaprunIntent enum (SendEmailSummary { recipient }) with serde(tag="kind") for stable wire format
  - TaintLabel::is_untrusted() exhaustive match — compile-enforced; no wildcard
  - Re-export of CaprunIntent from runtime-core root
  - intent_taint.rs test suite (9 tests: round-trip, fail-closed, 7-variant truth table)

affects:
  - 06-02 (executor HARD-02 predicate fix builds on is_untrusted())
  - 06-03 (broker IPC ProvideIntent builds on CaprunIntent type)
  - 06-04 (CLI planner builds on CaprunIntent + serde)
  - 06-05 (end-to-end clean allow-path uses all of the above)

tech-stack:
  added: []
  patterns:
    - "TDD RED/GREEN per feature: test file created first (compile-error RED), then implementation (GREEN)"
    - "Exhaustive match self without wildcard for security predicates (no matches! macro)"
    - "serde(tag) for enum discriminant stability between process boundaries"

key-files:
  created:
    - crates/runtime-core/src/intent.rs (CaprunIntent added alongside existing Intent/IntentStatus)
    - crates/runtime-core/tests/intent_taint.rs (9 tests)
  modified:
    - crates/runtime-core/src/plan_node.rs (TaintLabel::is_untrusted() added)
    - crates/runtime-core/src/lib.rs (CaprunIntent re-exported)

key-decisions:
  - "CaprunIntent uses #[serde(tag = \"kind\")] — stable wire shape between caprun main and worker (Pitfall 4)"
  - "TaintLabel::is_untrusted() uses explicit match self with no wildcard — false-allow is worse than false-block (Pitfall 5)"
  - "Single variant only (SendEmailSummary) — YAGNI; second variant deferred to Phase 7 when file.create scope is clear"

patterns-established:
  - "Pattern: exhaustive match for security predicates — never matches!() or _ => default"
  - "Pattern: serde(tag) on cross-process enums to prevent deserialization mismatch"

requirements-completed: [PLAN-02, PLAN-03, HARD-02]

coverage:
  - id: D1
    description: "CaprunIntent::SendEmailSummary serializes to JSON and deserializes back to an equal value"
    requirement: PLAN-02
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#caprun_intent_serde_round_trip"
        status: pass
    human_judgment: false
  - id: D2
    description: "Deserializing an unknown intent kind (e.g. LaunchMissiles) returns Err (fail-closed, V5 input validation)"
    requirement: PLAN-02
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#caprun_intent_unknown_kind_fails_deserialization"
        status: pass
    human_judgment: false
  - id: D3
    description: "TaintLabel::UserTrusted.is_untrusted() == false; LocalWorkspace.is_untrusted() == false"
    requirement: HARD-02
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_user_trusted_returns_false"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_local_workspace_returns_false"
        status: pass
    human_judgment: false
  - id: D4
    description: "All five untrusted labels return true from is_untrusted()"
    requirement: HARD-02
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_external_untrusted_returns_true"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_email_raw_returns_true"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_pdf_raw_returns_true"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_llm_generated_returns_true"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_worker_extracted_returns_true"
        status: pass
    human_judgment: false
  - id: D5
    description: "CaprunIntent and TaintLabel::is_untrusted() re-exported from runtime-core root; check-invariants.sh Gate 2 (purity) stays green"
    requirement: PLAN-03
    verification:
      - kind: unit
        ref: "cargo build --workspace (all downstream crates compile against new surface)"
        status: pass
      - kind: unit
        ref: "./scripts/check-invariants.sh (Gate 1 + Gate 2 both PASS)"
        status: pass
    human_judgment: false

duration: 4min
completed: "2026-07-01"
status: complete
---

# Phase 06 Plan 01: CaprunIntent enum + TaintLabel::is_untrusted() Summary

**Typed v0 intent enum and exhaustive taint predicate added to runtime-core: CaprunIntent::SendEmailSummary with stable serde wire format, TaintLabel::is_untrusted() with compile-enforced exhaustive match, and a 9-test suite covering round-trip, fail-closed, and all 7 taint labels.**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-07-01T00:24:21Z
- **Completed:** 2026-07-01T00:27:34Z
- **Tasks:** 3 (TDD: tasks 1 and 2 each had RED + GREEN commits)
- **Files modified:** 4

## Accomplishments

- `CaprunIntent` enum added to `runtime-core/src/intent.rs` with `serde(tag="kind")` for stable wire format between caprun main and the worker process (Pitfall 4 prevented)
- `TaintLabel::is_untrusted()` added with explicit `match self` and no wildcard arm — adding a future untrusted variant forces a compile error instead of silently treating it as trusted (Pitfall 5 / T-06-01 mitigated)
- 9-test suite in `intent_taint.rs`: serde round-trip, fail-closed unknown-kind, and all 7 TaintLabel variants in the truth table
- All downstream crates compile (`cargo build --workspace`); check-invariants.sh Gate 1 + Gate 2 both green

## Task Commits

1. **Task 1 TDD RED — failing intent_taint tests** - `79a7a57` (test)
2. **Task 1 TDD GREEN — CaprunIntent enum + re-export** - `cc93e7b` (feat)
3. **Task 2 TDD GREEN — TaintLabel::is_untrusted()** - `246c4d8` (feat)
4. **Task 3 — fail-closed unknown kind test** - `e5f0be1` (test)

_Note: Task 2 TDD RED shares commit 79a7a57 with Task 1 (is_untrusted tests were written together in the single test file RED phase)._

## Files Created/Modified

- `crates/runtime-core/src/intent.rs` — added `CaprunIntent` enum above existing `IntentStatus`
- `crates/runtime-core/src/lib.rs` — extended re-export to include `CaprunIntent`
- `crates/runtime-core/src/plan_node.rs` — added `impl TaintLabel { pub fn is_untrusted() }`
- `crates/runtime-core/tests/intent_taint.rs` — new file, 9 tests

## Decisions Made

- `#[serde(tag = "kind")]` on CaprunIntent — required for stable JSON between two process boundaries; internal tagging produces `{ "kind": "SendEmailSummary", "recipient": "..." }` without requiring a content wrapper, matching the documented Pitfall 4 anti-pattern
- Explicit `match self` with no wildcard in `is_untrusted()` — a false-allow (new untrusted label silently trusted) is a worse failure mode than a false-block; compile error is the right enforcement
- Single variant only (SendEmailSummary) — Phase 7 will add the `RelativePath`/`file.create` surface when the capability model is designed; no YAGNI stubs added

## Deviations from Plan

None — plan executed exactly as written. TDD RED/GREEN split was combined (single test file covers both tasks) which is consistent with the plan structure (Task 3 is the test file task).

## Known Stubs

None — no placeholder values or hardcoded empty returns introduced.

## Threat Flags

No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries beyond what the plan's threat model already covers (T-06-01 and T-06-02 both mitigated).

## Issues Encountered

None — implementation matched the research patterns exactly.

## Next Phase Readiness

- Wave 2 (plan 02: executor HARD-02 predicate fix, plan 03: broker IPC ProvideIntent) can now compile against `TaintLabel::is_untrusted()` and `CaprunIntent`
- The exhaustive match enforces that any Phase 7+ TaintLabel additions force an `is_untrusted()` update at compile time

---
*Phase: 06-deterministic-planner-intent-input*
*Completed: 2026-07-01*
