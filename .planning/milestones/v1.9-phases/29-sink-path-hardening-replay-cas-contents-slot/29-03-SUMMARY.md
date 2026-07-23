---
phase: 29-sink-path-hardening-replay-cas-contents-slot
plan: 03
subsystem: security
tags: [rust, tcb, executor, i2, slot-type-binding, sink-sensitivity]

requires:
  - phase: 23-25-slot-type-binding-enforcement-t2
    provides: "expected_role() hardcoded per-sink-arg table + Step 1c SlotTypeMismatch fail-closed gate in submit_plan_node"
provides:
  - "file.create's contents arg is content-sensitive AND role-checked to Some([\"path\"]) — closes the last unconstrained slot among the two live sinks"
affects: [30-regression-live-proof]

tech-stack:
  added: []
  patterns:
    - "Content-sensitivity + role-constraint additions to sink_sensitivity.rs follow the existing hardcoded-const + .contains() arm shape (mirrors EMAIL_SEND_CONTENT_SENSITIVE / FILE_CREATE_ROUTING_SENSITIVE)"

key-files:
  created: []
  modified:
    - crates/executor/src/sink_sensitivity.rs
    - crates/executor/tests/executor_decision.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/harden01_session_integrity.rs
    - crates/brokerd/tests/s9_acceptance.rs

key-decisions:
  - "expected_role(file.create, contents) == Some(&[\"path\"]) — reuses the trusted \"path\" role rather than minting a new role name, per DESIGN-security-hardening.md §e / planner.rs:208's dual-slot reuse of the same trusted intent_value_id"
  - "5 pre-existing test fixtures across executor + brokerd that minted a file.create contents value with role None now mint with role Some(\"path\") to stay clean/all-trusted fixtures under the newly-enforced Step 1c check"
  - "unconstrained_slot_unaffected inverted to file_create_contents_role_mismatch_denies — contents is no longer the last unconstrained slot, so the prior 'unaffected' premise no longer holds"

patterns-established: []

requirements-completed: [HARDEN-05]

coverage:
  - id: D1
    description: "file.create's contents arg is content-sensitive (is_content_sensitive returns true) and role-checked to Some([\"path\"]) via expected_role; path stays routing-sensitive only (no over-widening)"
    requirement: "HARDEN-05"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_create_contents_expects_path"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_create_contents_is_content_sensitive"
        status: pass
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_create_path_not_content_sensitive"
        status: pass
    human_judgment: false
  - id: D2
    description: "The only live file.create flow (s9_live_file_create_clean_allow) still Allows — no false-positive block from the new role constraint"
    requirement: "HARDEN-05"
    verification:
      - kind: e2e
        ref: "MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_live_file_create_clean_allow' bash scripts/mailpit-verify.sh (real Linux via Colima+Docker)"
        status: pass
    human_judgment: false

duration: 25min
completed: 2026-07-17
status: complete
---

# Phase 29 Plan 03: Constrain file.create contents slot (HARDEN-05) Summary

**`file.create`'s `contents` arg is now content-sensitive and role-checked to `Some(&["path"])` in the executor TCB — closing the last unconstrained sink-arg slot from v1.5's slot-type-binding work, with zero regression to the live flow.**

## Performance

- **Duration:** 25 min
- **Started:** 2026-07-17T16:47:00Z
- **Completed:** 2026-07-17T17:11:57Z
- **Tasks:** 1 (TDD: RED → GREEN)
- **Files modified:** 5

## Accomplishments
- `FILE_CREATE_CONTENT_SENSITIVE = &["contents"]` const + `.contains()` arm added to `is_content_sensitive` — scoped to `contents` only, `path` stays excluded (over-widening guard test green)
- `expected_role(file.create, "contents")` flipped from `None` to `Some(&["path"])` — the load-bearing pin traced to `planner.rs:208`'s reuse of the trusted `"path"`-role handle in both slots
- Inverted the stale `file_create_contents_is_unconstrained` unit test to `file_create_contents_expects_path`, added `file_create_contents_is_content_sensitive` and the `file_create_path_not_content_sensitive` over-widening guard
- Live regression canary (`s9_live_file_create_clean_allow`) re-verified Allow on real Linux via `scripts/mailpit-verify.sh`

## Task Commits

Each task was committed atomically (TDD: RED then GREEN):

1. **Task 1 (RED): add failing tests for HARDEN-05** - `b8f240a` (test)
2. **Task 1 (GREEN): implement contents role/content-sensitivity + fix regression fixtures** - `32e7fc7` (feat)

**Plan metadata:** pending (this commit)

## Files Created/Modified
- `crates/executor/src/sink_sensitivity.rs` - `FILE_CREATE_CONTENT_SENSITIVE` const, `is_content_sensitive` `file.create` arm, `expected_role`'s `contents` arm flipped to `Some(&["path"])`, inverted/added unit tests
- `crates/executor/tests/executor_decision.rs` - fixed 2 fixtures (`draft_session_denies_commit_irreversible`, `non_live_session_denies_commit_irreversible_in_all_four_states`) that minted `contents` with role `None`; inverted `unconstrained_slot_unaffected` → `file_create_contents_role_mismatch_denies`
- `crates/brokerd/tests/durable_anchor.rs` - fixed `contents` mint role (`None` → `Some("path")`) in the file.create hostile-block harness
- `crates/brokerd/tests/harden01_session_integrity.rs` - fixed `contents` mint role in the cross-connection all-trusted fixture
- `crates/brokerd/tests/s9_acceptance.rs` - fixed `contents` mint role in the file.create routing-sensitive-block acceptance fixture

## Decisions Made
- Reused the `"path"` role for `contents` rather than minting a new role vocabulary (`"contents"`/`"file_body"`) — no such role-producing mint site exists anywhere in the codebase; the DESIGN doc's pin traces this end-to-end to `planner.rs:208`.
- All 5 fixed test fixtures now mint `contents` with `Some("path".to_string())` instead of `None`, matching the only live production shape, rather than leaving them role-unconstrained (which the new Step 1c check no longer permits).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed 3 test-suite regressions caused directly by the contents role constraint**
- **Found during:** Task 1 GREEN phase, `cargo test --workspace --no-fail-fast`
- **Issue:** 3 test targets (`executor` integration tests, `brokerd::durable_anchor`, `brokerd::harden01_session_integrity`, `brokerd::s9_acceptance` — 5 individual tests) minted a `file.create` `contents` value with role `None`/absent. Once `expected_role(file.create, "contents")` became `Some(&["path"])`, Step 1c's per-arg role check (which runs in plan-node arg order, BEFORE Step 2/3's collect-then-Block sensitivity loop) now hard-`Deny`s on the `contents` arg with `SlotTypeMismatch` before the loop can reach `path`'s tainted-routing-arg Block or the Step 0.5 draft/non-live class-deny check — these tests expected `BlockedPendingConfirmation` / `Denied{DraftOnlySessionDeniesCommitIrreversible}` / `Denied{NonLiveSessionDeniesCommitIrreversible}` and instead got `Denied{SlotTypeMismatch}`.
- **Fix:** Updated each fixture to mint `contents` with role `Some("path".to_string())` (the same reused trusted role the plan's own pin specifies for the live flow), restoring each test's original "clean, all-trusted" or "path-blocks-first" premise.
- **Additional fix:** `unconstrained_slot_unaffected` in `executor_decision.rs` asserted that a mismatched-role `contents` value was unaffected (`Allowed`) — this premise is now false by design (that's the entire point of HARDEN-05). Renamed/inverted to `file_create_contents_role_mismatch_denies`, asserting `Denied{SlotTypeMismatch}` instead — mirroring the sink_sensitivity.rs unit-test inversion pattern the plan specified.
- **Files modified:** `crates/executor/tests/executor_decision.rs`, `crates/brokerd/tests/durable_anchor.rs`, `crates/brokerd/tests/harden01_session_integrity.rs`, `crates/brokerd/tests/s9_acceptance.rs`
- **Verification:** `cargo test --workspace --no-fail-fast` — 0 failures (was 3 failed test targets / 8 failed test cases before the fix); `./scripts/check-invariants.sh` — all 4 gates pass
- **Committed in:** `32e7fc7` (same commit as the GREEN implementation — these fixes were necessary for the task's own GREEN state to hold across the workspace, not a separate task)

---

**Total deviations:** 1 auto-fixed (Rule 1 — regression fixes across 4 files, 8 individual test cases, all a direct necessary consequence of the plan's own change)
**Impact on plan:** No scope creep — every fixed file was a pre-existing test fixture that manually constructed a `file.create` `contents` value with an unconstrained role, which the plan's own change (correctly) no longer permits. All fixes align exactly with the plan's stated load-bearing pin (`Some(&["path"])`, the reused trusted role).

## Issues Encountered
None beyond the auto-fixed regressions above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- HARDEN-05 closed. Combined with 29-01/29-02 (HARDEN-03 replay CAS), Phase 29's two grouped items are both implemented.
- Phase 30 (regression/live-proof) can proceed once 29-01 and 29-02 also land — this plan's live regression canary (`s9_live_file_create_clean_allow`) already re-verified independently via `scripts/mailpit-verify.sh`.
- No blockers.

---
*Phase: 29-sink-path-hardening-replay-cas-contents-slot*
*Completed: 2026-07-17*

## Self-Check: PASSED

All 5 modified files verified present on disk; all 3 commits (`b8f240a` test, `32e7fc7` feat, `1e34fde` docs) verified present in git log.
