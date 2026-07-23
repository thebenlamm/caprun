---
phase: 10-single-shot-confirmation-loop
verified: 2026-07-07T00:00:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 10: Single-Shot Confirmation Loop Verification Report

**Phase Goal:** A human can inspect a blocked effect's verbatim literal and provenance, then release it exactly once or durably deny it, via a second CLI command — never a session-wide waiver or standing policy.
**Verified:** 2026-07-07
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `caprun confirm <effect_id>` against a `BlockedPendingConfirmation` effect displays the verbatim literal value and its provenance to the human | ✓ VERIFIED | `render_block_display` (`crates/brokerd/src/confirmation.rs:274-332`) prints byte-exact literal in quotes, `Taint:`, `Source:`, `Provenance chain:` lines. Exercised end-to-end by cross-process test `confirm_releases_once_and_second_confirm_is_already_terminal` (`cli/caprun/tests/confirm.rs`), independently re-run: asserts stdout contains `"released.txt"` and a `Taint:` line. |
| 2 | Confirming releases exactly that one `(sink, arg, literal-digest)` triple — effect proceeds once, no standing policy or session-wide waiver created | ✓ VERIFIED | `confirm()` dispatches directly to `invoke_file_create_from_resolved` from the frozen `resolved_args` snapshot; zero non-comment references to `submit_plan_node`, `ValueStore`, or any allowlist structure in `confirmation.rs` (independently grepped: `grep -v '^\s*//' crates/brokerd/src/confirmation.rs \| grep -c submit_plan_node` → 0; likewise for `invoke_file_create_from_resolved`'s body). Test `confirm_twice_returns_already_terminal_and_creates_no_new_file` proves a second confirm creates no additional file. |
| 3 | Denying is durable: the effect never proceeds, and the same `effect_id` cannot later be confirmed | ✓ VERIFIED | `transition_state`'s `UPDATE ... WHERE state = 'pending'` SQL guard (`confirmation.rs:203-213`) enforces the terminal check atomically. Unit tests `transition_pending_to_denied_then_confirmed_is_refused` and `deny_on_pending_block_is_durable`, plus cross-process test `deny_is_durable_and_confirm_after_deny_is_already_terminal` (independently re-run, passed), prove deny durability across separate OS processes: deny exits 2, a later confirm on the same effect_id (distinct process) exits 5, target file never created. |
| 4 | Every confirm/deny decision is recorded as an audit event anchored to `SinkBlockedAnchor.effect_id`, and the release path executes in the TCB (not a policy file) | ✓ VERIFIED | `confirm_granted`/`confirm_denied` events appended with `parent_id = Some(pc.blocked_event_id)` and `actor = "confirm:{effect_id}"` / `"deny:{effect_id}"` (`confirmation.rs:379-389`, `464-473`). Cross-process test asserts via raw SQL (`assert_anchored_event`) that `parent_id` equals the seeded `sink_blocked` event id. All decision logic lives in `crates/brokerd/src/confirmation.rs` (TCB, not a policy file) — verified by file location and by `./scripts/check-invariants.sh` (Gate 1 PASS, no `EffectRequest` token; Gate 2 PASS, runtime-core purity intact). |

**Score:** 4/4 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/brokerd/src/confirmation.rs` | `PendingConfirmation`, `ResolvedArg`, `PendingConfirmationState`, `insert_pending_confirmation`/`find_pending_confirmation`/`transition_state`, `confirm`/`deny`/`ConfirmOutcome`/`render_block_display` | ✓ VERIFIED | All types/functions present, read and confirmed against Plan 01/03 must_haves verbatim. |
| `pending_confirmations` DDL in `crates/brokerd/src/audit.rs` | 7-column STRICT table, effect_id PK | ✓ VERIFIED | Confirmed via passing unit tests (`cargo test -p brokerd audit`) and code read. |
| `WorkspaceRoot::root_path` in `crates/adapter-fs/src/workspace.rs` | Platform-independent accessor | ✓ VERIFIED | Present at line 71-73, unit test `root_path_returns_the_opened_root` passes. |
| `invoke_file_create_from_resolved` in `crates/brokerd/src/sinks/file_create.rs` | ValueStore-free frozen-literal sink re-invocation | ✓ VERIFIED | Function body contains zero `ValueStore`/`submit_plan_node` references (independently verified via `awk` isolation + grep). |
| `SubmitPlanNode` block-arm wiring in `crates/brokerd/src/server.rs` | Atomic insert of `PendingConfirmation` with `sink_blocked` event | ✓ VERIFIED | Lines 453-520: full-arg resolution + `insert_pending_confirmation` inside the same `conn.lock()` scope as `append_event` + `insert_blocked_literal`. Integration test `pending_confirmation_persisted_atomically_with_block` (`crates/brokerd/tests/durable_anchor.rs`) exercises this through the REAL `submit_plan_node`/broker dispatch path (not hand-seeded), independently re-run and passing. |
| `caprun confirm`/`caprun deny` CLI dispatch in `cli/caprun/src/main.rs` | First-arg branch, 6-way exit code mapping | ✓ VERIFIED | Lines 47-87 (dispatch), 302-344 (`run_confirm_or_deny`, exit-code mapping matches DESIGN doc exactly: 0/2/3/4/5/6, 1=usage error). |
| `cli/caprun/tests/confirm.rs` + `[[test]]` entry | Cross-process integration test | ✓ VERIFIED | File exists, `[[test]] name = "confirm"` registered in `cli/caprun/Cargo.toml`; drives the real compiled `caprun` binary as separate subprocesses against a persistent SQLite DB. Independently re-run: 3/3 tests pass. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `server.rs` SubmitPlanNode block arm | `confirmation::insert_pending_confirmation` | Same `conn.lock()` scope as `append_event(sink_blocked)` + `insert_blocked_literal` | ✓ WIRED | Read directly (lines 486-522); atomicity is structural (same lock scope, fail-closed `?` propagation). |
| `confirmation::confirm` | `sinks::file_create::invoke_file_create_from_resolved` | Direct call with frozen `pc.resolved_args` | ✓ WIRED | Line 403-411 of `confirmation.rs`; never routes through `submit_plan_node` (grep-verified 0 matches). |
| `cli/caprun/src/main.rs` | `brokerd::confirmation::{confirm, deny}` | `run_confirm_or_deny` helper | ✓ WIRED | Lines 302-324; opens persistent DB, dispatches, maps `ConfirmOutcome` to exit code. |
| `confirm`/`deny` | Redaction gate | `crate::audit::get_blocked_literal` returning `None` → `BlockedLiteralRedacted` | ✓ WIRED | Line 363-365; unit test `confirm_with_redacted_blocked_literal_refuses_to_release` passes. |

### Behavioral Spot-Checks / Independent Re-Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full workspace test suite | `cargo test --workspace --no-fail-fast` | All test-result blocks report `0 failed` (32+ blocks) | ✓ PASS |
| Architectural invariants | `./scripts/check-invariants.sh` | Gate 1 (no `EffectRequest`) PASS; Gate 2 (runtime-core purity) PASS | ✓ PASS |
| CONFIRM-02 non-bypassability | `grep -v '^\s*//' crates/brokerd/src/confirmation.rs \| grep -c submit_plan_node` | `0` | ✓ PASS |
| No `EffectRequest` bypass token | `grep -rn 'EffectRequest' crates/` | No matches | ✓ PASS |
| Cross-process confirm/deny integration test | `cargo test -p caprun --test confirm` | 3/3 passed | ✓ PASS |
| Brokerd confirmation unit tests | `cargo test -p brokerd confirmation` (implicitly, via full suite) | All pass, including redaction-gate and unknown-effect-id cases | ✓ PASS |
| Atomic block-time checkpoint (real submit_plan_node path) | `cargo test -p brokerd --test durable_anchor` | 4/4 passed, incl. `pending_confirmation_persisted_atomically_with_block` | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CONFIRM-01 | 10-03 | Verbatim literal + provenance display via `caprun confirm` | ✓ SATISFIED | `render_block_display` + cross-process test assertions on stdout content. |
| CONFIRM-02 | 10-02, 10-03 | Single-shot release, no standing policy | ✓ SATISFIED | Frozen-literal re-invocation, zero `submit_plan_node`/allowlist references, second-confirm test. |
| CONFIRM-03 | 10-01, 10-03 | Durable deny, cannot be re-confirmed | ✓ SATISFIED | SQL-guarded one-way state machine + cross-process deny-then-confirm test. |
| CONFIRM-04 | 10-01, 10-02, 10-03 | Audited, anchored to effect_id, TCB-resident | ✓ SATISFIED | `confirm_granted`/`confirm_denied` events with `parent_id` anchoring, verified via raw-SQL assertion in the integration test; logic lives in `crates/brokerd`. |

No orphaned requirements — REQUIREMENTS.md maps only CONFIRM-01..04 to Phase 10, and all four appear in the plans' `requirements` frontmatter across the three plans.

### Anti-Patterns Found

None. Scanned all phase-modified files (`confirmation.rs`, `audit.rs`, `server.rs`, `sinks/file_create.rs`, `adapter-fs/src/workspace.rs`, `cli/caprun/src/main.rs`, `cli/caprun/tests/confirm.rs`) for `TODO`/`FIXME`/`XXX`/`HACK`/`PLACEHOLDER`/`placeholder`/`not yet implemented` — zero matches.

### Human Verification Required

None. All four Done-When conditions from `DESIGN-confirmation-release.md` are demonstrably true via passing, independently re-executed tests — including the cross-process integration test that drives the real compiled binary, which is the only honest way to test CONFIRM-03's cross-process durability claim. No behavior-dependent truth was left unexercised.

### Gaps Summary

None. All plan must-haves, all four ROADMAP success criteria, and all five DESIGN-confirmation-release.md Done-When conditions are verified against the actual codebase (not merely SUMMARY.md claims) via direct code reading, independent re-execution of `cargo test --workspace --no-fail-fast`, `./scripts/check-invariants.sh`, `cargo test -p caprun --test confirm`, and `cargo test -p brokerd --test durable_anchor`, plus targeted greps for the CONFIRM-02 non-bypassability and `EffectRequest` gates.

---

_Verified: 2026-07-07_
_Verifier: Claude (gsd-verifier)_
