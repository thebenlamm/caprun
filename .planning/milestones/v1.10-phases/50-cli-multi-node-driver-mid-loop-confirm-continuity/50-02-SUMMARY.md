---
phase: 50-cli-multi-node-driver-mid-loop-confirm-continuity
plan: 02
subsystem: cli
tags: [cli-01, cli-02, confirm-01, safe-coding-workflow, mid-loop-hold, grant-pointer, exit-codes]

requires:
  - phase: 50-01
    provides: stream_hold protocol + worker SafeCodingWorkflow PROCEED/ABORT hold + exit taxonomy
  - phase: 49-deterministic-multi-step-coding-planner
    provides: CaprunIntent::SafeCodingWorkflow + five-node coding recipe
  - phase: 48-plan-stream-substrate
    provides: sequential plan_next stream loop
provides:
  - safe-coding-workflow CLI argv (JSON → SafeCodingWorkflow)
  - main mid-loop confirm orchestration (interactive + CAPRUN_CONFIRM=external)
  - piped worker + broker lifetime across hold
  - grant pointer at coding session start (no auto-grant)
  - CLI-02 product exit map 0/2/3/1 for coding + email/file honesty
  - coding_cli Wave 0 tests + COVERAGE honesty (no LIVE-07 claim)
affects:
  - 51 (LIVE-07/08 non-hybrid CLI multi-node proof on real Linux)

tech-stack:
  added: []
  patterns:
    - "JSON intent file argv for SafeCodingWorkflow (not 13 positionals)"
    - "Piped parent↔worker hold + in-process confirm primary; external poll alternate"
    - "Broker aborted only after worker terminal — never on first BLOCKED"
    - "PROCEED only after ConfirmOutcome::Released or durable confirmed"

key-files:
  created:
    - cli/caprun/tests/coding_cli.rs
    - .planning/phases/50-cli-multi-node-driver-mid-loop-confirm-continuity/COVERAGE.md
  modified:
    - cli/caprun/src/main.rs
    - .planning/phases/50-cli-multi-node-driver-mid-loop-confirm-continuity/50-VALIDATION.md

key-decisions:
  - "Interactive in-process confirm is primary; CAPRUN_CONFIRM=external or non-TTY → dual-terminal poll"
  - "External poll interval 200ms; default timeout 300s (CAPRUN_CONFIRM_TIMEOUT_SECS) → exit 3"
  - "Sink-fail mid-loop outcomes → ABORT worker + exit 1 (infra), never PROCEED as success"
  - "Email/file keep simpler wait; map worker exit 2/3 to process exit 2/3"
  - "Phase 50 does not claim LIVE-07/08 SUCCESS"

patterns-established:
  - "orchestrate_coding_stream reads protocol lines; mid-loop resolve writes PROCEED/ABORT"
  - "in_process_confirm/deny share key/F1 pattern with run_confirm_or_deny"
  - "map_run_exit_code unifies coding terminal + worker status into CLI-02 integers"

requirements-completed: [CLI-01, CLI-02, CONFIRM-01]

coverage:
  - id: D1
    description: "safe-coding-workflow argv builds CaprunIntent::SafeCodingWorkflow from JSON; reject seed-from-file; --policy accepted"
    requirement: CLI-01
    verification:
      - kind: unit
        ref: "cli/caprun/tests/coding_cli.rs#coding_intent_json_deserializes_to_safe_coding_workflow"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/coding_cli.rs#safe_coding_workflow_rejects_seed_from_file"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/coding_cli.rs#policy_flag_accepted_for_coding_argv"
        status: pass
    human_judgment: false
  - id: D2
    description: "Piped coding hold orchestration; mid-loop confirm/deny; grant pointer; broker lifetime across BLOCKED"
    requirement: CONFIRM-01
    verification:
      - kind: other
        ref: "cargo check -p caprun + main.rs orchestrate_coding_stream / resolve_mid_loop_hold"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/coding_cli.rs#safe_coding_workflow_argv_accepted_past_unknown_kind"
        status: pass
    human_judgment: false
  - id: D3
    description: "CLI-02 exit taxonomy 0/2/3/1 on product path; email/file honesty; no silent continue-past-Block"
    requirement: CLI-02
    verification:
      - kind: unit
        ref: "cli/caprun/tests/stream_hold.rs#map_stream_exit_matrix_0_2_3_1"
        status: pass
      - kind: unit
        ref: "cli/caprun/src/main.rs#map_run_exit_code"
        status: pass
    human_judgment: false
  - id: D4
    description: "COVERAGE honesty: no external API; CaprunIntent no-change; no LIVE-07 claim; Wave 0 complete"
    requirement: CLI-01
    verification:
      - kind: other
        ref: ".planning/phases/50-cli-multi-node-driver-mid-loop-confirm-continuity/COVERAGE.md"
        status: pass
      - kind: other
        ref: ".planning/phases/50-cli-multi-node-driver-mid-loop-confirm-continuity/50-VALIDATION.md"
        status: pass
    human_judgment: false

duration: 6min
completed: 2026-07-29
status: complete
---

# Phase 50 Plan 02: CLI Multi-node Driver + Mid-loop Confirm Continuity Summary

**Product `safe-coding-workflow` argv + piped same-Session hold orchestration with mid-loop confirm/deny, grant pointer, and CLI-02 exit map 0/2/3/1 — ready for Phase 51 LIVE, without claiming LIVE-07/08.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-07-29T21:30:24Z
- **Completed:** 2026-07-29T21:35:58Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `safe-coding-workflow <coding-intent.json>` arm: JSON → `CaprunIntent::SafeCodingWorkflow`; fail-closed on wrong kind / missing file / `--seed-from-file`.
- Coding path pipes worker stdin/stdout, keeps broker alive through BLOCKED, mid-loop interactive confirm (or `CAPRUN_CONFIRM=external` poll), writes PROCEED only after Released/durable confirmed.
- Prints `session_id=` + `grant: caprun grant …` at coding session start; never auto-grants.
- Maps stream terminals and worker exits to CLI-02 codes 0/2/3/1; email/file Block/Deny honesty preserved.
- Wave 0 `coding_cli` tests + COVERAGE + VALIDATION complete; no LIVE-07 SUCCESS claim.

## Task Commits

Each task was committed atomically:

1. **Task 1: Coding argv + orchestrated hold lifecycle + mid-loop confirm** - `c887614` (feat)
2. **Task 2: coding_cli tests, regressions, COVERAGE, validation Wave 0** - `71bf878` (test)

**Plan metadata:** docs commit (this SUMMARY + STATE/ROADMAP/REQUIREMENTS)

## Files Created/Modified

- `cli/caprun/src/main.rs` — safe-coding-workflow arm; grant pointer; piped orchestration; mid-loop confirm; exit map
- `cli/caprun/tests/coding_cli.rs` — Wave 0 CLI-01 argv / fail-closed / policy tests
- `.planning/phases/50-cli-multi-node-driver-mid-loop-confirm-continuity/COVERAGE.md` — no external API; identity no-change; no LIVE-07
- `.planning/phases/50-cli-multi-node-driver-mid-loop-confirm-continuity/50-VALIDATION.md` — Wave 0 + nyquist complete

## Decisions Made

- Interactive in-process confirm primary; external/non-TTY dual-terminal poll (RESEARCH A3).
- External poll 200ms / 300s default timeout → exit 3 blocked-incomplete.
- Mid-loop sink-fail → ABORT + exit 1 (not silent success).
- Hold only for SafeCodingWorkflow; email/file simpler wait with exit 2/3 mapping.
- LIVE-07/08 deferred to Phase 51.

## Deviations from Plan

None - plan executed exactly as written.

(e2e email path not re-run under mailpit: host Docker unavailable. Email success exit-0 path unchanged when SMTP available; worker exit 2/3 now map to process 2/3. Non-SMTP suite green.)

## Threat Flags

None beyond plan `<threat_model>` — no new network endpoints, no auto-grant, no remint, no dual-Session, no free-form effect path, no policy rebind mid-stream.

## Known Stubs

None.

## Test Results

```text
./scripts/check-invariants.sh                          → All gates PASSED
cargo check -p caprun                                  → ok (Task 1)
cargo test -p caprun --test coding_cli                 → 6 passed
cargo test -p caprun --test confirm                    → 4 passed
cargo test -p caprun --test grant                      → (included in batch)
cargo test -p caprun --test stream_hold                → 11 passed
cargo test -p caprun --test stream_substrate           → 12 passed
cargo test -p caprun --test planner                    → 26 passed
cargo test -p caprun --test e2e                        → skipped (Docker/mailpit unavailable on host)
```

Host OpenSSL via `~/.local/openssl-dev`. **No LIVE-07/08 SUCCESS claim.**

## Self-Check: PASSED

- FOUND: `cli/caprun/src/main.rs` safe-coding-workflow + orchestrate_coding_stream
- FOUND: `cli/caprun/tests/coding_cli.rs`
- FOUND: COVERAGE.md (no LIVE-07 claim)
- FOUND: 50-VALIDATION.md nyquist_compliant true
- FOUND: commit `c887614`
- FOUND: commit `71bf878`
- FOUND: zero new crates under `crates/`
