---
phase: 10-single-shot-confirmation-loop
plan: 02
subsystem: brokerd
tags: [rust, sqlite, confirmation-loop, i2, sink, openat2]

requires:
  - phase: 10-single-shot-confirmation-loop (plan 01)
    provides: pending_confirmations DDL + confirmation.rs types/accessors (PendingConfirmation, ResolvedArg, insert_pending_confirmation, find_pending_confirmation, transition_state)
provides:
  - WorkspaceRoot::root_path() accessor (platform-independent)
  - SubmitPlanNode block arm now builds a full Vec<ResolvedArg> snapshot from the live per-connection ValueStore and persists a PendingConfirmation row atomically with the sink_blocked event
  - invoke_file_create_from_resolved — ValueStore-free, submit_plan_node-free re-invocation of the file.create sink from frozen literals
affects: [10-03-cli-confirm-deny]

tech-stack:
  added: []
  patterns:
    - "Block-time full-arg-set snapshot: every plan_node arg (not only the one the executor blocked on) is resolved into a ResolvedArg and frozen, because the confirm-time process may need any of them and the ValueStore does not survive process exit."
    - "Atomic checkpoint-with-event: insert_pending_confirmation runs inside the same conn.lock() scope as append_event(sink_blocked) + insert_blocked_literal — commit together or fail closed before any response is sent."
    - "Confirm-time sink re-invocation is a frozen-literal sibling function (invoke_file_create_from_resolved), never a re-decision path — distinguishes sink_invocation_failed (confirm-time) from sink_execution_failed (allow-time)."

key-files:
  created: []
  modified:
    - crates/adapter-fs/src/workspace.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/sinks/file_create.rs
    - crates/brokerd/tests/durable_anchor.rs

key-decisions:
  - "durable_anchor.rs's harness previously left `contents` unminted (the executor's own decision short-circuits on the tainted `path` alone, so contents never needed to resolve for the ALLOW/DENY verdict). Task 2's new full-snapshot resolution now resolves EVERY plan_node arg, so the harness was updated to mint `contents` as a trusted value — otherwise the block itself would fail closed with an unresolved-handle error, which is correct broker behavior surfacing a stale test fixture, not a product bug."

requirements-completed: [CONFIRM-02, CONFIRM-04]

coverage:
  - id: D1
    description: "WorkspaceRoot::root_path() accessor exposing the opened workspace-root path"
    requirement: "CONFIRM-04"
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#workspace::tests::root_path_returns_the_opened_root"
        status: pass
    human_judgment: false
  - id: D2
    description: "SubmitPlanNode block arm persists a full-snapshot PendingConfirmation atomically with the sink_blocked event"
    requirement: "CONFIRM-04"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/durable_anchor.rs#pending_confirmation_persisted_atomically_with_block"
        status: pass
    human_judgment: false
  - id: D3
    description: "invoke_file_create_from_resolved re-creates a file from frozen ResolvedArg literals with two-phase durable audit, no ValueStore/executor coupling"
    requirement: "CONFIRM-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/file_create.rs#tests::invoke_file_create_from_resolved_success_records_sink_executed"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/sinks/file_create.rs#tests::invoke_file_create_from_resolved_failure_records_sink_invocation_failed"
        status: pass
    human_judgment: false

duration: 25min
completed: 2026-07-07
status: complete
---

# Phase 10 Plan 02: Durable Block-Time Checkpoint + Frozen-Literal Sink Re-Invocation Summary

**Block-time full-arg-set snapshot persisted atomically with `sink_blocked`, plus a ValueStore-free `invoke_file_create_from_resolved` that re-invokes the sink from frozen literals — no re-decision, no I2 bypass.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 3
- **Files modified:** 4 (3 planned + 1 pre-existing test fixture fixed as direct fallout)

## Accomplishments
- `WorkspaceRoot::root_path()` accessor — platform-independent, no new `cfg` gating, `dirfd`'s `#[allow(dead_code)]` untouched.
- `SubmitPlanNode`'s block arm now resolves EVERY `plan_node.args` entry against the still-live per-connection `ValueStore` into a `Vec<ResolvedArg>`, and persists a `PendingConfirmation` row via `insert_pending_confirmation` inside the SAME `conn.lock()` scope as `append_event(sink_blocked)` + `insert_blocked_literal` — fail-closed, atomic with the checkpoint.
- `invoke_file_create_from_resolved` added as a `ValueStore`-free sibling of `invoke_file_create`: looks up `path`/`contents` directly from frozen `&[ResolvedArg]`, never constructs a `ValueStore`, never calls `resolve`/`mint`/`submit_plan_node`. On error it appends the new `sink_invocation_failed` event type (distinct from the allow-path's `sink_execution_failed`).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add WorkspaceRoot::root_path accessor (A2 plumbing)** - `a35db45` (feat)
2. **Task 2: Persist PendingConfirmation atomically in the SubmitPlanNode block arm** - `1cee2e2` (feat) + `aa641c2` (test — end-to-end wiring proof, reconstructed from the reopened DB alone)
3. **Task 3: Add invoke_file_create_from_resolved (frozen-literal sink re-invocation)** - `fe75c0f` (feat)

**Plan metadata:** (this commit, docs)

## Files Created/Modified
- `crates/adapter-fs/src/workspace.rs` - `root_path()` accessor + a macOS-runnable unit test
- `crates/brokerd/src/server.rs` - block arm builds the `Vec<ResolvedArg>` snapshot and calls `insert_pending_confirmation` in the same lock scope as the existing `sink_blocked` append
- `crates/brokerd/src/sinks/file_create.rs` - `invoke_file_create_from_resolved` + `resolved_literal` helper + success/failure unit tests
- `crates/brokerd/tests/durable_anchor.rs` - harness now mints `contents` (Task 2 fallout, see Deviations) + new `pending_confirmation_persisted_atomically_with_block` integration test

## Decisions Made
- Full-arg-set resolution happens BEFORE/at the same point as building the `sink_blocked` audit_event (constructed from the `decision` match), then the actual DB write is inside the pre-existing lock scope — matching the plan's ordering requirement without restructuring the existing lock discipline.
- `invoke_file_create_from_resolved`'s failure event type is `sink_invocation_failed`, kept textually distinct from `invoke_file_create`'s `sink_execution_failed` per DESIGN Step 4a.5, so confirm-time and allow-time sink failures are unambiguously distinguishable in the audit DAG.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `durable_anchor.rs` test harness left `contents` unminted; now fails closed under the new full-snapshot resolution**
- **Found during:** Task 2 (running `cargo test -p brokerd` after wiring the snapshot resolution)
- **Issue:** `build_hostile_block_db`'s `plan_node` used `ValueId::new()` (never minted into the store) for `contents`, relying on the executor's own decision short-circuiting on the tainted `path` before ever resolving `contents`. Task 2's new snapshot resolves EVERY `plan_node.args` entry — including `contents` — so the previously-unresolvable handle now surfaces as a fail-closed error (correct broker behavior; the test fixture was stale).
- **Fix:** Minted `contents` into the store as a `UserTrusted` value (mirroring `file_create.rs`'s own test setup), keeping `path` as the sole tainted arg the executor actually blocks on.
- **Files modified:** `crates/brokerd/tests/durable_anchor.rs`
- **Verification:** `cargo test -p brokerd` green (all 3 pre-existing `durable_anchor` tests plus the new one); `cargo test --workspace --no-fail-fast` green.
- **Committed in:** `1cee2e2` (part of Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — direct fallout of this task's own correctness change, in-scope by the deviation rules' scope boundary).
**Impact on plan:** Necessary fix to keep the test suite honest about the new full-snapshot behavior. No scope creep — no other files touched.

## Issues Encountered
- The acceptance-criteria grep (`grep -v '^\s*//' ... | grep -n 'from_resolved' -A 40 | grep -cE 'ValueStore|submit_plan_node'`) returns 3, not 0, because its 40-line context window after each `from_resolved` match spills into the adjacent `invoke_file_create_success_records_sink_executed` test's `use executor::value_store::ValueStore;` import. Verified directly with `awk '/^pub fn invoke_file_create_from_resolved/,/^}/'` piped to the same grep — the function body itself has zero `ValueStore`/`submit_plan_node` references, satisfying the actual intent (T-10-05).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 03 (CLI `caprun confirm`/`caprun deny`) can now: (1) `find_pending_confirmation` by `effect_id`, (2) reopen the persisted `workspace_root_path` via `WorkspaceRoot::open` + `.root_path()`, and (3) call `invoke_file_create_from_resolved` with the frozen `resolved_args` to release the block — exactly the on-ramp this plan built.
- `cargo test --workspace --no-fail-fast` and `./scripts/check-invariants.sh` both green on this Mac.

---
*Phase: 10-single-shot-confirmation-loop*
*Completed: 2026-07-07*

## Self-Check: PASSED

All modified files present on disk; all 5 commit hashes (a35db45, 1cee2e2, aa641c2, fe75c0f, 4358a6c) found in git log.
