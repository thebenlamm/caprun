---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 01
subsystem: executor / runtime-core / brokerd
tags: [HARD-05, mint-invariant, typed-denial, fail-closed, I2]
requires:
  - "ValueStore::mint + ValueRecord (executor crate)"
  - "ExecutorDecision + submit_plan_node (runtime-core / executor)"
provides:
  - "ValueStore::mint returns Result<ValueId, MintInvariantError> — empty taint/provenance unconstructable"
  - "runtime_core::DenyReason (base denial enum: DanglingHandle / EmptyTaintInvariantViolation / MissingProvenanceAnchor)"
  - "executor empty-taint/empty-provenance guards run before is_routing_sensitive"
affects:
  - "07-02 (consumes DenyReason for ExecutorDecision reshape)"
  - "07-04 (extends DenyReason with schema-validation variants)"
tech-stack:
  added: []
  patterns:
    - "Fallible constructor makes an invariant hold by construction, not convention"
    - "Typed denial taxonomy replaces free-form reason strings"
    - "Defense-in-depth: source-side (mint) reject + executor guard before sensitivity check"
key-files:
  created: []
  modified:
    - crates/executor/src/value_store.rs
    - crates/runtime-core/src/executor_decision.rs
    - crates/runtime-core/src/lib.rs
    - crates/executor/src/lib.rs
    - crates/brokerd/src/quarantine.rs
    - crates/executor/tests/executor_decision.rs
    - crates/runtime-core/tests/task2_types.rs
decisions:
  - "DenyReason is the ONE base denial enum for Phase 7; 07-04 extends it — no second error type"
  - "Empty taint no longer means 'clean/allow' anywhere; the :108 allow-path test uses [UserTrusted] + a real event id"
metrics:
  duration: ~15min
  completed: 2026-07-01
  tasks: 5
  files: 7
status: complete
---

# Phase 7 Plan 01: Mint Non-Empty Invariant + Typed DenyReason Foundation Summary

Made the "every ValueRecord carries ≥1 taint label and ≥1 provenance event id" invariant true by construction — `ValueStore::mint` is now fallible and rejects empty taint/provenance before minting an id — and landed the typed `DenyReason` base enum with the executor's empty-value guards moved ahead of the routing-sensitivity check so an all-trusted-but-unprovenanced value Denies instead of reaching `Allowed` (the codex #5 hole).

## What Was Built

- **`ValueStore::mint` → `Result<ValueId, MintInvariantError>`** (`value_store.rs`): rejects `EmptyTaint` and `EmptyProvenance` before allocating an id. An empty-taint/empty-provenance record is now unconstructable through the sanctioned path (HARD-05, T-07-12).
- **`DenyReason` typed enum** (`runtime-core/executor_decision.rs`): `DanglingHandle`, `EmptyTaintInvariantViolation`, `MissingProvenanceAnchor` with `code()` + `Display`; re-exported from `runtime_core`. `ExecutorDecision::Denied` now carries `reason: DenyReason` (was `String`).
- **Executor guard reordering** (`executor/lib.rs`): after `resolve()`, the empty-taint then empty-provenance guards run BEFORE `is_routing_sensitive` (T-07-11). An empty-taint arg can no longer slip past the `any(is_untrusted)` block (an empty iterator is never untrusted).
- **Ripple fixes**: both live mint callers (`mint_from_read`, `mint_from_intent`) propagate the mint `Result` with `?` via `map_err → anyhow` (no live behavior change — both pass non-empty). The `executor_decision.rs` allow-path test (`:108`) now mints `[UserTrusted]` + a real event id instead of `vec![], vec![]`, and `task2_types.rs` constructs `Denied` with `DenyReason::DanglingHandle`.

## Task Commits

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | RED: failing mint invariant tests | b5980aa |
| 2 | GREEN: Result-returning mint + MintInvariantError | 81bcceb |
| 3 | Typed DenyReason enum + Denied reshape | d0cb0f9 |
| 4 | Executor guards moved up + Denied uses DenyReason | 3f46491 |
| 5 | Ripple: mint callers + :108 allow-shape + task2_types | c510f08 |

## Verification

- `cargo test -p executor --lib value_store` — 5/5 green (empty taint → `EmptyTaint`, empty provenance → `EmptyProvenance`, non-empty → `Ok` + round-trip).
- `cargo build --workspace && cargo test --workspace --no-fail-fast` — green on macOS. Linux-only enforcement tests show "0 passed" as expected (cfg-gated; not a gap).
- `./scripts/check-invariants.sh` — both gates PASS (no `EffectRequest`; runtime-core stays I/O-free).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `task2_types.rs` Denied construction used a String**
- **Found during:** Task 5 (workspace test build)
- **Issue:** `crates/runtime-core/tests/task2_types.rs:77` constructed `ExecutorDecision::Denied { reason: "policy".to_string() }`, which no longer compiles after the Task 3 type change to `reason: DenyReason`.
- **Fix:** Imported `DenyReason` and changed the construction to `DenyReason::DanglingHandle`. This test was not in the plan's caller list but is a direct ripple of the `Denied` reshape.
- **Files modified:** crates/runtime-core/tests/task2_types.rs
- **Commit:** c510f08

Tasks 1 and 2 were authored as a RED→GREEN pair as the plan explicitly anticipated (the RED tests reference `MintInvariantError`, which does not exist until GREEN, so the crate fails to compile in the RED state) — committed as two separate `test(...)` then `feat(...)` commits.

## Known Stubs

None. All changes are complete and wired; no placeholder values introduced.

## Threat Flags

None. No new network endpoints, auth paths, file access, or trust-boundary schema introduced beyond the plan's threat model (T-07-11/12/13 are the addressed mitigations).

## Notes for Downstream

- **07-02** consumes `runtime_core::DenyReason` when reshaping `ExecutorDecision`. `BlockedPendingConfirmation` was intentionally left FLAT here (07-02 owns it).
- **07-04** must EXTEND `DenyReason` with schema-validation variants — do NOT introduce a second denial error type, and do NOT reintroduce `reason: String`.
- Pre-existing unused-import warning (`PlanArg`, `PlanNode`) in `task2_types.rs` was left untouched (out of scope — not introduced by this plan).

## Self-Check: PASSED
- All 7 modified files exist and are committed.
- Commits b5980aa, 81bcceb, d0cb0f9, 3f46491, c510f08 present in git log.
