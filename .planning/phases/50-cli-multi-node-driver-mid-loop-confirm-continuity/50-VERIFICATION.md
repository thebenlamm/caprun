---
phase: 50-cli-multi-node-driver-mid-loop-confirm-continuity
verified: 2026-07-29T21:39:19Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 50: CLI Multi-node Driver & Mid-loop Confirm Continuity — Verification Report

**Phase Goal:** A design partner can drive the full multi-node coding chain from the real CLI with honest stop semantics, and mid-stream Block-and-Hold keeps the same Session across confirm/deny

**Verified:** 2026-07-29T21:39:19Z  
**Status:** passed  
**Re-verification:** No — initial verification  
**Requirements:** CLI-01, CLI-02, CONFIRM-01

## Goal Achievement

### Observable Truths

Roadmap Success Criteria are the contract. Plan must_haves add detail but do not reduce scope. LIVE-07/08 non-hybrid SUCCESS is **Phase 51** (explicitly out of Phase 50 scope).

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | `caprun run` accepts coding multi-step intent + workspace + trusted `--policy`, binds policy at session creation (POLICY-03), and drives the multi-node coding chain (product path) | ✓ VERIFIED | `main.rs` `"safe-coding-workflow"` arm deserializes JSON → `CaprunIntent::SafeCodingWorkflow`, rejects `--seed-from-file`; single `bind_policy(...)` before `create_session`; piped worker + `orchestrate_coding_stream`. Tests: `coding_cli` (6/6), `planner::coding_plan_next_emits_five_sinks_in_order` |
| 2 | Existing Block → review/confirm/deny/grant surfaces preserved and pointed at from driver; silent continue-past-Block forbidden | ✓ VERIFIED | Early verb dispatch for confirm/deny/review/grant/audit intact (`main.rs` ~91–173). Mid-loop `print_effect_pointers` emits review/confirm/deny. Grant pointer at coding session start only (no auto-grant). `block_without_proceed_is_not_success` + `unknown_hold_token_is_not_proceed` + main writes `PROCEED` only after `Released`/`Confirmed` |
| 3 | Honest stop semantics: I2 Block → stop/hold + effect_id + review pointer; policy_deny distinct via `code=`; Deny aborts remaining; exit codes 0/2/3/1 | ✓ VERIFIED | Worker emits `BLOCKED`/`DENIED code=`/`STREAM_DONE`; exit 2 deny/abort, 3 non-coding Block, 0 STREAM_DONE. `map_stream_exit` + `map_run_exit_code`. Tests: `map_stream_exit_matrix_0_2_3_1`, `empty_and_policy_deny_exit_buckets`, `denied_round_trip_including_policy_deny`, `hold_abort_stops_without_further_submits` |
| 4 | `BlockedPendingConfirmation` holds same Session (Block-and-Hold); no re-open ProvideIntent, re-bind policy, or mint new trusted values | ✓ VERIFIED | Worker SafeCodingWorkflow arm: emit BLOCKED → stdin PROCEED → `step_index += 1` without re-`SubmitPlanNode` / without ProvideIntent. Single `BrokerRequest::ProvideIntent` send site before stream loop. Single `create_session` + single `bind_policy` in main; broker aborted only after worker terminal. Test: `hold_continue_no_resubmit_blocked_sink` (submit=5, `git.push` once) |
| 5 | Human confirm/deny on durable pending; remaining nodes after Allowed release; no dual-Session stitch; no session-wide waiver | ✓ VERIFIED | `resolve_mid_loop_hold` → `in_process_confirm`/`deny` → `brokerd::confirmation::{confirm,deny}` (same durable path as CLI verbs); external mode polls `list_pending_confirmations_for_session` + `PendingConfirmationState::Confirmed`. PROCEED only on `ConfirmOutcome::Released` or durable Confirmed. Confirm tests 4/4 green. No dual-session / remint product path in code |

**Score:** 5/5 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `cli/caprun/src/stream_hold.rs` | Protocol parse/format + exit mapper + hold tokens | ✓ VERIFIED | 249 lines; pure; `caprun-stream:` lines; PROCEED/ABORT exact match; `map_stream_exit` 0/2/3/1 |
| `cli/caprun/src/worker.rs` | SafeCodingWorkflow Block-and-Hold; DENIED/STREAM_DONE; exit 2/3 | ✓ VERIFIED | Hold branch ~450–513; Deny exit 2; non-coding Block exit 3; STREAM_DONE on success; ProvideIntent once before loop |
| `cli/caprun/src/main.rs` | safe-coding-workflow argv; piped orchestration; mid-loop confirm; exit map; grant pointer | ✓ VERIFIED | Coding arm ~333–356; grant pointer ~447–449; piped stdio ~647–649; `orchestrate_coding_stream` ~835–946; `map_run_exit_code` ~732–758 |
| `cli/caprun/tests/stream_hold.rs` | Protocol + exit-map unit tests | ✓ VERIFIED | 11 tests, all pass |
| `cli/caprun/tests/stream_substrate.rs` | HoldContinue / HoldAbort / silent-continue guard | ✓ VERIFIED | `hold_continue_no_resubmit_blocked_sink`, `hold_abort_stops_without_further_submits`, `block_without_proceed_is_not_success` pass (12 host tests incl. linux filter) |
| `cli/caprun/tests/coding_cli.rs` | CLI-01 argv contracts | ✓ VERIFIED | 6 tests, all pass |
| Phase `COVERAGE.md` | No external API; no LIVE-07 claim | ✓ VERIFIED | Explicit Phase 51 deferral; CaprunIntent identity no-change |
| Phase `50-VALIDATION.md` | Wave 0 / nyquist complete | ✓ VERIFIED | `nyquist_compliant: true`, `wave_0_complete: true` |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| JSON intent file | `CaprunIntent::SafeCodingWorkflow` → worker ProvideIntent once → `plan_next` stream | argv + INTENT env | ✓ WIRED | coding_cli deserializes; main match arm; worker single ProvideIntent then sequential loop |
| `bind_policy` once at session create | immutable policy for hold window | `session_policy` into broker | ✓ WIRED | One call site; no rebind mid-stream |
| worker `BLOCKED` | main mid-loop confirm/deny | `parse_line` + `resolve_mid_loop_hold` → PROCEED/ABORT stdin | ✓ WIRED | `orchestrate_coding_stream` match arm writes PROCEED only after Released/Confirmed |
| `BlockedPendingConfirmation` | no re-SubmitPlanNode of blocked node | PROCEED → `step_index++` / HoldContinue | ✓ WIRED | worker.rs + `drive_stream_with_hold` Proceed path |
| `ExecutorDecision::Denied` | DENIED `code=` + exit 2 | `reason.code()` | ✓ WIRED | worker Denied/NotImplemented arms; `map_stream_exit(DeniedAborted)=2` |
| `stream_hold::map_stream_exit` | worker exits + main `map_run_exit_code` | shared module | ✓ WIRED | `mod stream_hold` in both `main.rs` and `worker.rs` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| Worker BLOCKED line | `effect_id`, `sink` | `anchors[0].anchor` from broker `PlanNodeDecision` | Yes — real decision anchors | ✓ FLOWING |
| Main mid-loop confirm | `ConfirmOutcome` | `brokerd::confirmation::confirm` against audit DB | Yes — durable pending row | ✓ FLOWING |
| Main grant pointer | `session_id` | `create_session` UUID | Yes — real session id | ✓ FLOWING |
| Coding intent | `CaprunIntent::SafeCodingWorkflow` | JSON file via serde | Yes — operator JSON | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| HoldContinue no re-submit | `cargo test -p caprun --test stream_substrate hold_continue_no_resubmit_blocked_sink -- --exact` | ok | ✓ PASS |
| HoldAbort no later submits | `cargo test -p caprun --test stream_substrate hold_abort_stops_without_further_submits -- --exact` | ok | ✓ PASS |
| Silent continue forbidden | `cargo test -p caprun --test stream_substrate block_without_proceed_is_not_success -- --exact` | ok | ✓ PASS |
| Exit taxonomy 0/2/3/1 | `cargo test -p caprun --test stream_hold map_stream_exit_matrix_0_2_3_1 -- --exact` | ok | ✓ PASS |
| Protocol + exit suite | `cargo test -p caprun --test stream_hold` | 11 passed | ✓ PASS |
| Substrate suite | `cargo test -p caprun --test stream_substrate` | 12 passed | ✓ PASS |
| coding_cli argv | `cargo test -p caprun --test coding_cli` | 6 passed | ✓ PASS |
| confirm regression | `cargo test -p caprun --test confirm` | 4 passed | ✓ PASS |
| grant regression | `cargo test -p caprun --test grant` | 2 passed | ✓ PASS |
| planner regression | `cargo test -p caprun --test planner` | 26 passed | ✓ PASS |
| Architectural gates | `./scripts/check-invariants.sh` | All gates PASSED | ✓ PASS |
| Email e2e (mailpit) | `cargo test -p caprun --test e2e` | SKIP — Docker/mailpit unavailable on this host | ? SKIP |

### Probe Execution

| Probe | Command | Result | Status |
| ----- | ------- | ------ | ------ |
| N/A | Phase 50 does not declare `scripts/*/tests/probe-*.sh` | — | SKIPPED |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| CLI-01 | 50-02 | Coding multi-step argv + policy bind + multi-node driver; Block surfaces pointed at | ✓ SATISFIED | `safe-coding-workflow` arm; coding_cli 6/6; grant/review/confirm/deny pointers; bind_policy once |
| CLI-02 | 50-01, 50-02 | Honest stop semantics + exit codes 0/2/3/1; no silent continue-past-Block | ✓ SATISFIED | stream_hold + stream_substrate + worker/main exit map |
| CONFIRM-01 | 50-01, 50-02 | Same-Session Block-and-Hold; durable confirm; no remint/dual-Session/waiver | ✓ SATISFIED | worker hold + main orchestration + confirm regression + no remint path |

**Orphaned requirements:** none (REQUIREMENTS.md maps CLI-01/02 and CONFIRM-01 only to Phase 50).

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| — | — | No `TBD`/`FIXME`/`XXX` in phase-touched CLI sources/tests | — | — |
| — | — | No auto-grant / auto-confirm on coding run path | — | — |
| — | — | No `EffectRequest` free-form path under crates/ (Gate 1 PASS) | — | — |
| — | — | No LIVE-07 SUCCESS assertion in coding_cli or COVERAGE | — | — |

### Prohibitions (judgment-tier)

| Prohibition | Status | Evidence |
| ----------- | ------ | -------- |
| No reconnect-remint / dual-Session stitch product path | ✓ satisfied | stream_hold module docs + worker hold only; single create_session; no continue-remint verb |
| No re-open ProvideIntent on hold | ✓ satisfied | Single send site before loop; PROCEED advances step only |
| No session-wide confirm waiver | ✓ satisfied | Per-effect confirm/deny only |
| No silent advance past Block without PROCEED | ✓ satisfied | Worker waits stdin; unknown token → exit 1; main PROCEED only after Released |
| No auto-confirm / auto-grant inside run | ✓ satisfied | Grant pointer print only; confirm requires human/external terminal |
| No free-form effect path | ✓ satisfied | Gate 1 PASS |
| No LIVE-07/08 SUCCESS claim | ✓ satisfied | COVERAGE + VALIDATION + coding_cli comments |

### Human Verification Required

None required for Phase 50 pass.

Optional / deferred (not gating):

- Interactive TTY mid-loop confirm UX polish (discretionary; automated protocol is green).
- Non-hybrid LIVE multi-node success on real Linux git.push + github.pr → **Phase 51**.

### Gaps Summary

No blocking gaps. Phase 50 product path is implemented and Wave 0 proofs pass on host:

- Parent↔worker hold protocol + exit taxonomy
- Worker Block-and-Hold (no re-submit)
- CLI `safe-coding-workflow` argv + policy bind + mid-loop orchestration wiring
- Confirm/grant regression green
- Honest non-claim of LIVE-07/08 (Phase 51)

Email `e2e` suite was not re-run here (Docker/mailpit absent); email/file Block/Deny exit mapping is present in code (`map_run_exit_code` worker-code path) and is not the phase goal. LIVE multi-node SUCCESS is intentionally deferred.

---

_Verified: 2026-07-29T21:39:19Z_  
_Verifier: Claude (gsd-verifier)_

## VERIFICATION PASSED
