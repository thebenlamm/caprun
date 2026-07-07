---
phase: 10-single-shot-confirmation-loop
plan: 03
subsystem: security
tags: [rust, sqlite, rusqlite, cli, audit-dag, confirmation-loop]

# Dependency graph
requires:
  - phase: 10-single-shot-confirmation-loop (plans 01/02)
    provides: PendingConfirmation/ResolvedArg/PendingConfirmationState types + side-table accessors (Plan 01); invoke_file_create_from_resolved frozen-literal sink re-invocation (Plan 02); server.rs SubmitPlanNode-arm wiring that persists the checkpoint at Block time
provides:
  - "confirm()/deny() decision logic + ConfirmOutcome enum + render_block_display (crates/brokerd/src/confirmation.rs)"
  - "caprun confirm <effect_id> [db] / caprun deny <effect_id> [db] CLI verbs with a distinct exit code per outcome (cli/caprun/src/main.rs)"
  - "cli/caprun/tests/confirm.rs cross-process integration test proving single-shot release + durable deny + audit anchoring"
affects: [11-live-acceptance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fresh-read terminal-state check: confirm/deny always re-read PendingConfirmation.state from the DB, never from a cache, because the process deciding is never the process that created the block"
    - "At-most-once state transition BEFORE sink invocation (transition_state persisted first; sink failure never rolls state back)"
    - "Redaction-gate fail-closed: get_blocked_literal(None) refuses release even though PendingConfirmation.resolved_args still holds its own literal copy"

key-files:
  created:
    - cli/caprun/tests/confirm.rs
  modified:
    - crates/brokerd/src/confirmation.rs
    - cli/caprun/src/main.rs
    - cli/caprun/Cargo.toml

key-decisions:
  - "email.send confirm branch mirrors invoke_email_send_stub's no-op append (constructs an empty-args PlanNode locally since email.send has no live effect in v1.2) rather than adding email-specific frozen-literal plumbing that no acceptance criterion exercises"
  - "Taint label CLI display uses a new dotted-lowercase helper (external.untrusted, path.raw, etc.) since TaintLabel has no existing Display/as_str impl in the codebase"
  - "Provenance-chain event ids are truncated to 8 hex chars for terminal display (matching cli/caprun/src/main.rs's existing &hash[..8] convention) rather than the DESIGN doc's illustrative 6-char example — display-only, not a byte-exact-literal requirement"

requirements-completed: [CONFIRM-01, CONFIRM-02, CONFIRM-03, CONFIRM-04]

coverage:
  - id: D1
    description: "caprun confirm <effect_id> displays the verbatim blocked literal (byte-exact, in quotes) plus taint labels and provenance chain (CONFIRM-01)"
    requirement: "CONFIRM-01"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs#confirm_releases_once_and_second_confirm_is_already_terminal"
        status: pass
    human_judgment: false
  - id: D2
    description: "A confirm releases exactly one (sink, arg, literal-digest) triple — single-shot, no standing policy/allowlist created or consulted; confirm never calls executor::submit_plan_node (CONFIRM-02)"
    requirement: "CONFIRM-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirm_on_pending_file_create_releases_and_creates_file"
        status: pass
      - kind: other
        ref: "grep -v '^\\s*//' crates/brokerd/src/confirmation.rs | grep -c submit_plan_node -> 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "A deny is durable: the effect never proceeds and the same effect_id can never later be confirmed, across separate OS processes (CONFIRM-03)"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#deny_on_pending_block_is_durable"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs#deny_is_durable_and_confirm_after_deny_is_already_terminal"
        status: pass
    human_judgment: false
  - id: D4
    description: "confirm_granted/confirm_denied events exist in the audit DAG anchored (parent_id) to the sink_blocked event, with effect_id carried in the actor field (CONFIRM-04)"
    requirement: "CONFIRM-04"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs#confirm_releases_once_and_second_confirm_is_already_terminal (assert_anchored_event)"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs#deny_is_durable_and_confirm_after_deny_is_already_terminal (assert_anchored_event)"
        status: pass
    human_judgment: false
  - id: D5
    description: "caprun confirm/deny CLI dispatch maps each ConfirmOutcome to a distinct exit code (0/2/3/4/5/6; 1 = usage error) and is checked before the --seed-from-file/intent-kind parse"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs#confirm_and_deny_on_unknown_effect_id_exit_4"
        status: pass
      - kind: manual_procedural
        ref: "manual: target/debug/caprun confirm <random-uuid> <db> -> exit 4; confirm <bad-uuid> -> exit 1; confirm (no args) -> exit 1"
        status: pass
    human_judgment: false

# Metrics
duration: 11min
completed: 2026-07-07
status: complete
---

# Phase 10 Plan 03: Single-Shot Confirmation-Release Decision Logic + CLI + Cross-Process Test Summary

**TCB-resident `confirm`/`deny` decision logic in `crates/brokerd/src/confirmation.rs`, `caprun confirm`/`caprun deny` CLI verbs with a 6-way exit-code contract, and a cross-process integration test proving single-shot release and durable deny across separate OS processes.**

## Performance

- **Duration:** ~11 min (base commit 00:27:11 → last task commit 00:37:57)
- **Completed:** 2026-07-07
- **Tasks:** 3/3
- **Files modified:** 3 (1 new)

## Accomplishments

- `confirm()`/`deny()` + `ConfirmOutcome` (6 variants) + `render_block_display` implemented in `crates/brokerd/src/confirmation.rs`, following the DESIGN doc's Steps 1-4a/4b exactly: fresh-read terminal-state check, redaction gate, verbatim display, `confirm_granted`/`confirm_denied` event anchored onto the `sink_blocked` event, at-most-once state transition persisted BEFORE the sink is invoked, then dispatch to the frozen-literal `invoke_file_create_from_resolved` (or the `email.send` no-op stub) — never `executor::submit_plan_node` (verified: 0 grep matches).
- `caprun confirm <effect_id> [db]` / `caprun deny <effect_id> [db]` added as the VERY FIRST branch in `main()`, dispatched before `--seed-from-file` and the intent-kind match, with fail-closed handling for a missing/malformed `effect_id` (exit 1) and the full exit-code contract (Released=0, Denied=2, ConfirmedButSinkFailed=3, UnknownEffect=4, AlreadyTerminal=5, BlockedLiteralRedacted=6).
- `cli/caprun/tests/confirm.rs` drives the real compiled `caprun` binary as separate subprocesses against a persistent SQLite audit DB, proving CONFIRM-01..04 end-to-end without needing a Linux worker (seeds the Pending block directly via brokerd's public API, mirroring server.rs's block-time write).

## Task Commits

1. **Task 1: confirm/deny decision logic + ConfirmOutcome in confirmation.rs** - `444d309` (feat)
2. **Task 2: caprun confirm/deny CLI dispatch + exit codes + Cargo [[test]] target** - `1f7175c` (feat)
3. **Task 3: Cross-process integration test tests/confirm.rs** - `9b4dcce` (test)

**Plan metadata:** committed by the orchestrator after wave merge (worktree-mode: STATE.md/ROADMAP.md are NOT touched by this agent).

## Files Created/Modified

- `crates/brokerd/src/confirmation.rs` - Added `ConfirmOutcome`, `render_block_display`, `taint_label_display`, `short_evt`, `confirm`, `deny`, and 5 new `#[cfg(test)]` unit tests covering release/re-confirm/deny/redaction/unknown-id.
- `cli/caprun/src/main.rs` - Added the `confirm`/`deny` first-arg dispatch branch and `run_confirm_or_deny` helper (parse → open persistent DB → dispatch into `brokerd::confirmation` → map `ConfirmOutcome` to exit code).
- `cli/caprun/Cargo.toml` - Added the `[[test]] name = "confirm" path = "tests/confirm.rs"` entry.
- `cli/caprun/tests/confirm.rs` (new) - Cross-process integration test: seeds a Pending block via brokerd's API, spawns the real `caprun confirm`/`deny` binary as subprocesses, asserts exit codes, file-creation side effects, and audit-DAG anchoring via raw SQL.

## Decisions Made

- **`email.send` confirm branch:** constructs an empty-args `PlanNode` locally and calls the existing `invoke_email_send_stub` rather than adding a frozen-literal-aware email variant — email.send has no live effect in v1.2 and no acceptance criterion exercises this path; this keeps the branch minimal while still satisfying "mirror the existing no-op append."
- **Taint label CLI rendering:** added `taint_label_display` (an exhaustive match producing `external.untrusted`, `path.raw`, etc.) since `TaintLabel` has no existing `Display`/`as_str` implementation anywhere in the codebase — mirrors the exhaustive-match discipline of `TaintLabel::is_untrusted` (no wildcard arm, so a new variant is a compile error, not a silent gap).
- **Provenance-chain id truncation:** used 8 hex chars (`cli/caprun/src/main.rs`'s existing `&hash[..8]` audit-DAG print convention) rather than the DESIGN doc's illustrative 6-char example (`evt_a1b2c3...`). This is a display-only choice — the acceptance criteria require the byte-exact **literal** value and the presence of `Taint:`/`Provenance chain:` lines, not an exact truncation width for event ids.

## Deviations from Plan

None — plan executed exactly as written. All three tasks' acceptance criteria were met without needing Rule 1/2/3 auto-fixes; the three decisions above are within the plan's own stated latitude ("mirror the existing... no-op append", "exact terminal output format" for the literal/taint/provenance-chain *lines*, not literal event-id widths).

## Issues Encountered

- The `#[cfg(test)] mod tests` block in `confirmation.rs` initially failed to compile because the new confirm/deny test helpers called `append_event` without importing it into the test module's local `use` list (the outer module import wasn't in scope for the nested test additions). Fixed by adding `append_event` to the `use crate::audit::{...}` import inside the test module — a one-line, immediately-obvious fix (Rule 3, blocking) resolved before the first `cargo test` run completed cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All 5 DESIGN "Done-When" conditions hold simultaneously: verbatim display (CONFIRM-01), single-shot release with no standing policy (CONFIRM-02), durable deny (CONFIRM-03), TCB-resident audited confirm/deny anchored via `parent_id` (CONFIRM-04), and confirm/deny decision logic lives entirely in `crates/brokerd` (never a policy file).
- `cargo test --workspace --no-fail-fast` is green (32 `test result: ok` blocks, 0 failed) on this Mac; `cargo build --workspace` and `./scripts/check-invariants.sh` both pass.
- Phase 11 (live acceptance) can now build the real §9-style deny/confirm scenario on real Linux (Colima+Docker): this plan's macOS-runnable tests seed the block via brokerd's API rather than a live worker run — the genuine worker-produced block → confirm/deny path is exercised there (ACC-01/02/03).
- No blockers.

---
*Phase: 10-single-shot-confirmation-loop*
*Completed: 2026-07-07*
