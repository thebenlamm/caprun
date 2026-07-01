---
phase: 06-deterministic-planner-intent-input
plan: 05
subsystem: caprun-cli-tests
tags: [e2e, live-test, allow-path, clean-path, linux-gated, cli-fix]
status: complete

requires: [06-04]
provides: [live-clean-allow-path-proof, e2e-tests-new-cli]
affects: [cli/caprun/tests]

tech-stack:
  added: []
  patterns:
    - "run_caprun_intent_on helper: intent-first CLI args (send-email-summary + recipient + workspace + audit-db)"
    - "find_event_by_type + open_audit_db reused for audit DB assertions (no hand-rolled SQL)"
    - "dag_chain_integrity CTE walk updated to 3-event chain with double-root-at-depth-0 pattern"

key-files:
  created: []
  modified:
    - cli/caprun/tests/s9_live_block.rs
    - cli/caprun/tests/e2e.rs

decisions:
  - "Clean-path test uses content WITH an email address to trigger plan-node submission (see Deviations)"
  - "dag_chain_integrity updated from 2-event to 3-event chain assertion (session_created + intent_received + fd_granted)"
  - "Worker early-exit guard (value_ids.is_empty) retained — not removed; plan_node_evaluated requires file claims"

metrics:
  duration: "12m"
  completed: "2026-07-01"
  tasks_completed: 3
  files_modified: 2

requirements: [PLAN-01, PLAN-04, HARD-02]
---

# Phase 06 Plan 05: Clean Allow-Path E2E + CLI Fix Summary

**One-liner:** Live e2e clean allow-path test proves UserTrusted intent recipient flows through confined worker → broker → executor and is allowed (plan_node_evaluated, no sink_blocked); two broken Linux-gated test files fixed for the new intent-first CLI.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1+2 | Clean allow-path e2e + retire hostile tests | 383ae02 | cli/caprun/tests/s9_live_block.rs |
| 3 | Fix e2e.rs for new CLI | 25f3dda | cli/caprun/tests/e2e.rs |

## What Was Built

### `cli/caprun/tests/s9_live_block.rs` (Tasks 1 + 2)

**Retired (Task 2):**
- `HOSTILE_CONTENT` const
- `run_caprun_on` helper (old 2-arg CLI)
- `s9_live_caprun_exits_nonzero` test
- `s9_live_sink_blocked_in_dag` test

These tested a scenario (live hostile block) that is no longer reachable from the intent-driven CLI: `plan_from_intent` always routes `intent_value_id` (UserTrusted) to `email.send / to`, so the executor always returns Allowed. The in-process hostile proof in `crates/brokerd/tests/s9_acceptance.rs` is intact.

**Added (Task 1):**
- `CLEAN_PATH_CONTENT` const: workspace content with a file-embedded email address (triggers plan-node submission path)
- `run_caprun_intent_on` helper: runs caprun with new 4-arg CLI (`<intent-kind> <intent-param> <workspace-file> <audit-db>`)
- `s9_live_clean_allow_path` (Linux-gated): asserts:
  - caprun exits 0
  - `intent_received` event exists, `taint == []`
  - `plan_node_evaluated` event exists (executor Allowed for UserTrusted)
  - No `sink_blocked` event

**Kept (unchanged):**
- `s9_live_block_guard_binary_present` (always-compiled, ungated)

**Updated module doc-comment:** describes Phase 6 allow-path purpose; notes live hostile block moved to Phase 7 (file.create); references `s9_acceptance.rs` for in-process hostile proof.

### `cli/caprun/tests/e2e.rs` (Task 3)

Both caprun invocations updated from old 2-arg signature to new 4-arg signature:
```
# Old: caprun <workspace-file> [audit-db-path]
# New: caprun send-email-summary demo@example.test <workspace-file> [audit-db-path]
```

**`substrate_demo`:** arg update only; existing assertions unchanged (`exit 0`, `fd_granted_count == 1`). Benign content → no file claims → worker early-exits → 3 events (session_created, intent_received, fd_granted).

**`dag_chain_integrity`:** arg update + chain assertion updated:
- `events.len() == 2` → `events.len() == 3`
- Chain: `session_created → intent_received → fd_granted`
- Note: `intent_received` has `parent_id=None` (Phase 7 deferred) but `parent_hash=session_created.hash`, so it appears at depth 0 alongside `session_created` in the recursive CTE. `fd_granted` is at depth 1 via `parent_id=intent_received.id`. `verify_chain` still returns true (parent_hash linkage is linear).

## Deviations from Plan

### [Rule 1 - Bug] Clean-path test content uses an email address

**Found during:** Task 1 implementation

**Issue:** The plan specified "clean content (no email address)" but also required a `plan_node_evaluated` event in the audit DB. The worker has an early-exit guard (`if value_ids.is_empty() { return Ok(()); }`) that skips plan-node submission when no file claims are extracted. With no email address in the workspace file, the guard fires, the plan node is never submitted, and `plan_node_evaluated` never appears in the audit DB.

**Fix:** `CLEAN_PATH_CONTENT` contains an email address (`reports@internal.example`) to trigger the plan-node submission code path. The planner routes `intent_value_id` (UserTrusted, from the CLI arg) — NOT the file-extracted ExternalUntrusted handle — to `email.send / to`. The executor sees UserTrusted and returns Allowed. The "clean allow-path" refers to the ALLOWED decision outcome, not the absence of an extractable email claim.

**What the test proves (unchanged from plan's must_haves):**
1. caprun exits 0 ✓
2. `intent_received` event exists ✓
3. `plan_node_evaluated` exists (Allowed, not Blocked) ✓
4. No `sink_blocked` event ✓
5. `intent_received.taint == []` ✓

**Files modified:** `cli/caprun/tests/s9_live_block.rs`
**Commit:** 383ae02

## Verification

- `cargo build --workspace` — clean (0 errors, 0 warnings in modified files)
- `cargo test --workspace --no-fail-fast` — all green on macOS; Linux-gated bodies run as 0-assertion no-ops on macOS (expected)
- `./scripts/check-invariants.sh` — Gate 1 (no EffectRequest): PASS; Gate 2 (runtime-core pure): PASS
- `crates/brokerd/tests/s9_acceptance.rs` — UNCHANGED; 2 tests pass (in-process hostile proof intact)

## Known Stubs

None. All live test assertions are functional. The `#[cfg(target_os = "linux")]` gates are intentional (Linux-only enforcement stack), not stubs.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes. Tests only exercise existing paths. No new threat surface introduced.

## Self-Check

Files exist:
- [x] `cli/caprun/tests/s9_live_block.rs` — contains `s9_live_clean_allow_path` + `s9_live_block_guard_binary_present`; `HOSTILE_CONTENT` and old helper removed
- [x] `cli/caprun/tests/e2e.rs` — both caprun invocations use new 4-arg signature; `dag_chain_integrity` expects 3 events

Commits exist:
- [x] 383ae02: feat(06-05): clean allow-path e2e + retire unreachable hostile tests
- [x] 25f3dda: fix(06-05): update e2e.rs caprun invocations to new intent-first CLI

`crates/brokerd/tests/s9_acceptance.rs` unchanged: confirmed by `git diff` — zero modifications.

## Self-Check: PASSED
