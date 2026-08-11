---
phase: 50-cli-multi-node-driver-mid-loop-confirm-continuity
plan: 01
subsystem: cli
tags: [block-and-hold, stream-protocol, confirm-01, cli-02, exit-codes, parent-pipe]

requires:
  - phase: 48-plan-stream-substrate
    provides: sequential worker plan_next loop + stream_substrate drive_stream harness
  - phase: 49-deterministic-multi-step-coding-planner
    provides: SafeCodingWorkflow + five-node coding recipe + ProvideIntent multi-mint bag seed
  - phase: 47-design-gate
    provides: DESIGN-multi-step-plan-stream.md CLEARED Option A Block-and-Hold
provides:
  - stream_hold parent↔worker line protocol (BLOCKED/DENIED/NODE_ALLOWED/STREAM_DONE + PROCEED/ABORT)
  - worker SafeCodingWorkflow stay-connected hold (no re-submit, no remint)
  - CLI-02 pure exit taxonomy mapper (0/2/3/1)
  - Wave 0 HoldContinue / HoldAbort / protocol unit proofs
affects:
  - 50-02 (main coding argv driver + mid-loop confirm orchestration)
  - 51 (LIVE-07/08 non-hybrid CLI multi-node proof)

tech-stack:
  added: []
  patterns:
    - "Parent-pipe hold protocol (caprun-stream: lines + PROCEED/ABORT tokens) — no broker Wait verb"
    - "Stay-connected Block-and-Hold for SafeCodingWorkflow only; email/file stop-on-Block exit 3"
    - "PROCEED advances step_index without re-SubmitPlanNode of blocked node"
    - "Exit taxonomy 0 success / 2 denied-aborted / 3 blocked-incomplete / 1 infra"

key-files:
  created:
    - cli/caprun/src/stream_hold.rs
    - cli/caprun/tests/stream_hold.rs
  modified:
    - cli/caprun/src/worker.rs
    - cli/caprun/tests/stream_substrate.rs

key-decisions:
  - "Hold only for CaprunIntent::SafeCodingWorkflow; email/file Block → exit 3 (blocked-incomplete)"
  - "Parent-pipe stdin/stdout protocol only — no reconnect-remint, no dual-Session, no broker Wait IPC"
  - "policy_deny shares exit 2 with other denies; distinction is DENIED code= field"
  - "Unknown hold resume token → exit 1 infra (never Proceed)"

patterns-established:
  - "stream_hold pure module shared by both bins via mod stream_hold; tests use #[path] include"
  - "drive_stream_with_hold models Proceed/Abort without counting a second submit of the blocked sink"
  - "Worker machine lines on stdout; human hold notices on stderr"

requirements-completed: [CLI-02, CONFIRM-01]

coverage:
  - id: D1
    description: "Parent↔worker hold protocol (format/parse BLOCKED/DENIED/STREAM_DONE + PROCEED/ABORT)"
    requirement: CONFIRM-01
    verification:
      - kind: unit
        ref: "cli/caprun/tests/stream_hold.rs#blocked_round_trip"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/stream_hold.rs#hold_resume_tokens_exact_match_only"
        status: pass
    human_judgment: false
  - id: D2
    description: "Worker SafeCodingWorkflow Block-and-Hold: PROCEED advances without re-submit"
    requirement: CONFIRM-01
    verification:
      - kind: unit
        ref: "cli/caprun/tests/stream_substrate.rs#hold_continue_no_resubmit_blocked_sink"
        status: pass
    human_judgment: false
  - id: D3
    description: "HoldAbort + Deny exit 2 + policy_deny code= label + exit taxonomy 0/2/3/1"
    requirement: CLI-02
    verification:
      - kind: unit
        ref: "cli/caprun/tests/stream_substrate.rs#hold_abort_stops_without_further_submits"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/stream_hold.rs#map_stream_exit_matrix_0_2_3_1"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/stream_hold.rs#empty_and_policy_deny_exit_buckets"
        status: pass
    human_judgment: false

duration: 5min
completed: 2026-07-29
status: complete
---

# Phase 50 Plan 01: Block-and-Hold Protocol + Worker Continuity Summary

**Stay-connected parent-pipe hold: SafeCodingWorkflow Block emits BLOCKED, waits PROCEED/ABORT, advances without re-submit; CLI-02 exit taxonomy 0/2/3/1 unit-proven.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-07-29T21:24:25Z
- **Completed:** 2026-07-29T21:29:06Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Shipped pure `stream_hold` protocol module (`caprun-stream:` lines + hold tokens + exit mapper) free of I/O side effects.
- Worker coding path stays connected on `BlockedPendingConfirmation`: PROCEED advances `step_index` without re-`SubmitPlanNode` / without ProvideIntent remint; ABORT exits 2.
- Non-coding Block exits 3; Denied/NotImplemented emit `DENIED code=` and exit 2; success emits `STREAM_DONE`; empty stream stays fail-closed without STREAM_DONE.
- Wave 0 proofs: protocol round-trips, exit matrix, HoldContinue five-step no re-submit of `git.push`, HoldAbort zero later submits, Phase 48 substrate + Phase 49 planner green.

## Task Commits

Each task was committed atomically:

1. **Task 1: End-to-end Block-and-Hold — protocol + worker PROCEED path** - `db9feae` (feat)
2. **Task 2: HoldAbort + deny/policy_deny labels + substrate regression lock** - `45bb416` (test)

**Plan metadata:** docs commit (this SUMMARY + STATE/ROADMAP/REQUIREMENTS)

## Files Created/Modified

- `cli/caprun/src/stream_hold.rs` — protocol types, format/parse, hold resume, exit mapper
- `cli/caprun/src/worker.rs` — SafeCodingWorkflow hold branch; DENIED/STREAM_DONE/NODE_ALLOWED lines; exit 2/3
- `cli/caprun/tests/stream_hold.rs` — protocol + exit-map unit tests
- `cli/caprun/tests/stream_substrate.rs` — `drive_stream_with_hold`, HoldContinue, HoldAbort, silent-continue guard

## Decisions Made

- Hold gated on `CaprunIntent::SafeCodingWorkflow` only (RESEARCH A5) — email/file keep stop-on-Block → exit 3.
- Parent-pipe protocol only (RESEARCH A2) — no broker Wait verb, no reconnect-remint, no dual-Session stitch.
- Exit integers 0/2/3/1 locked (RESEARCH A4); `policy_deny` distinguished via `code=` not a separate exit.
- Unknown hold token → process exit 1 (infra), never silent Proceed.

## Deviations from Plan

None - plan executed exactly as written.

(HoldAbort substrate test landed with Task 1 harness expansion; Task 2 added exit-bucket / policy_deny documentation proofs and full planner regression lock.)

## Threat Flags

None beyond plan `<threat_model>` — no new network endpoints, no new mint sites, no free-form effect path, no session-wide confirm waiver. Worker still trusts parent pipe for PROCEED (T-50-05 deferred to Plan 02 main only writing PROCEED after `ConfirmOutcome::Released`).

## Known Stubs

None.

## Test Results

```text
./scripts/check-invariants.sh                          → All gates PASSED
cargo test -p caprun --test stream_hold                → 11 passed
cargo test -p caprun --test stream_substrate           → 12 passed
cargo test -p caprun --test planner                    → 26 passed
```

Host OpenSSL via `~/.local/openssl-dev` (dev machine libssl-dev path). No mailpit / LIVE-07 claim.

## Self-Check: PASSED

- FOUND: `cli/caprun/src/stream_hold.rs`
- FOUND: `cli/caprun/tests/stream_hold.rs`
- FOUND: worker hold PROCEED/ABORT path
- FOUND: commit `db9feae`
- FOUND: commit `45bb416`
- FOUND: no `safe-coding-workflow` product verb in main.rs (Plan 02 owns argv)
- FOUND: zero new crates under `crates/`
