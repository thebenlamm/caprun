---
phase: 09-session-trust-state-i1-i0
plan: 04
subsystem: cli/caprun
tags: [rust, cli, session-provenance, i0, origin]

# Dependency graph
requires:
  - phase: 09-session-trust-state-i1-i0
    plan: "09-01"
    provides: "SeedProvenance { TrustedArg, FileDerived }, SessionStatus::Draft"
  - phase: 09-session-trust-state-i1-i0
    plan: "09-03"
    provides: "brokerd::session::create_session(intent_id, seed_provenance), run_broker_server's initial_session_status parameter"
provides:
  - "cli/caprun --seed-from-file <path> on-ramp — the only place that can construct a file-derived intent"
  - "cargo test --workspace green (macOS baseline) for the first time this milestone"
affects: [11-live-acceptance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Collect argv into a Vec<String> and peek-and-strip an optional leading flag by index, rather than trying to push-back onto a std::env::args() iterator — avoids Peekable/re-injection gymnastics for a one-off optional flag."
    - "A CLI-decided provenance enum flows straight into a broker-owned constructor (create_session) that alone computes the resulting trust-state; the CLI only forwards the broker's answer (session.status) into the next call (run_broker_server) — never re-derives or re-asserts it."

key-files:
  created:
    - cli/caprun/tests/origin_seed_provenance.rs
  modified:
    - cli/caprun/src/main.rs

key-decisions:
  - "--seed-from-file's file content REPLACES the positional <intent-param> slot entirely (no redundant/ignored positional arg is consumed when the flag is present) — chosen over RESEARCH.md's illustrative usage string that kept <intent-param> positional even with the flag present, since Pitfall 4/A2 explicitly left the exact shape open and an ignored-but-required positional arg is a needless footgun."
  - "The integration test spawns the REAL caprun binary (same env!(\"CARGO_BIN_EXE_caprun\") pattern as e2e.rs/s9_live_block.rs) rather than adding a lib.rs to make main.rs's parsing logic unit-testable — create_session/persist_session/session_created all complete in main() before the broker's Linux-only abstract-socket bind and worker spawn, so the resulting sessions.status row is durable and macOS-assertable regardless of whether the rest of the run succeeds. This avoids introducing a lib target not in the plan's files_modified, and exercises the actual CLI code path rather than a duplicate of its logic."

requirements-completed: [ORIGIN-01, ORIGIN-02]

coverage:
  - id: D1
    description: "--seed-from-file present => SeedProvenance::FileDerived (file content becomes the intent parameter); absent => SeedProvenance::TrustedArg (today's positional behavior unchanged); exhaustive, no ambiguous default"
    requirement: "ORIGIN-01"
    verification:
      - kind: integration
        ref: "cargo test -p caprun --test origin_seed_provenance (file_derived_seed_starts_draft, trusted_arg_seed_starts_active)"
        status: pass
    human_judgment: false
  - id: D2
    description: "create_session receives the CLI-decided seed_provenance; run_broker_server receives session.status as initial_session_status — the CLI never self-declares status, only forwards the broker's computed value"
    requirement: "ORIGIN-01"
    verification:
      - kind: unit
        ref: "cargo build -p caprun (compiles against Plan 03's new signatures)"
        status: pass
    human_judgment: false
  - id: D3
    description: "a missing/unreadable --seed-from-file path is a hard error (fail-closed, V5) — never a silent fallback to trusted-arg; zero session rows are ever created on that path"
    requirement: "ORIGIN-02"
    verification:
      - kind: integration
        ref: "cargo test -p caprun --test origin_seed_provenance (missing_seed_file_fails_closed)"
        status: pass
    human_judgment: false
  - id: D4
    description: "cargo test -p caprun and cargo test --workspace are green (macOS baseline) — the 14-call-site signature ripple from Plans 01-03 is fully reconciled"
    requirement: "ORIGIN-02"
    verification:
      - kind: integration
        ref: "cargo test -p caprun --no-fail-fast (all tests pass); cargo test --workspace --no-fail-fast (all tests pass, Linux-gated tests show 0 passed as expected)"
        status: pass
    human_judgment: false

duration: ~10min
completed: 2026-07-07
status: complete
---

# Phase 9 Plan 4: CLI Seed-Provenance On-Ramp Summary

**Added the `--seed-from-file <path>` CLI on-ramp that lets `caprun` decide seed-provenance and feed it to the broker's `create_session`, giving ORIGIN-01/02 something concrete to exercise for the first time — closing the "no on-ramp exists" gap and bringing `cargo test --workspace` fully green for the first time this phase.**

## Performance

- **Duration:** ~10 min
- **Completed:** 2026-07-07T02:42:57Z
- **Tasks:** 2 completed
- **Files modified:** 2 (1 modified, 1 created — exactly as planned)

## Accomplishments

- `cli/caprun/src/main.rs`: parses an optional `--seed-from-file <path>` flag BEFORE the existing positional args (peek-and-strip by index over a collected `Vec<String>`, mirroring the file's existing `.next().ok_or_else(...)` error style). Presence reads the intent parameter from that file (replacing the positional `<intent-param>` slot entirely) and sets `SeedProvenance::FileDerived`; absence keeps today's behavior unchanged and sets `SeedProvenance::TrustedArg`. The file read is fail-closed (V5): a missing/unreadable path is a hard `anyhow` error via `.with_context(...)`, never a silent fallback.
- `create_session(intent_id, seed_provenance.clone())` is called with the CLI-decided provenance (Plan 03's new signature); the resulting `session.status` is forwarded, unmodified, into `run_broker_server`'s new `initial_session_status` argument — the CLI never self-declares status (DESIGN §3, mitigates T-09-10).
- The `session_created` Event's `actor` field records the seed-provenance tag (`broker:seed_provenance=trusted_arg` / `=file_derived`), mirroring the pattern already established in `server.rs`'s in-broker `CreateSession` IPC arm (Plan 03).
- New `cli/caprun/tests/origin_seed_provenance.rs` (auto-discovered by Cargo, no `[[test]]` entry needed): 3 tests spawning the real `caprun` binary and asserting the persisted `sessions.status` row directly —
  - `file_derived_seed_starts_draft`: `--seed-from-file` => `Draft` (ORIGIN-02).
  - `trusted_arg_seed_starts_active`: no flag => `Active` (regression guard).
  - `missing_seed_file_fails_closed`: nonexistent `--seed-from-file` path => non-zero exit, stderr names the flag, and zero `sessions` rows are ever created (proves the fail-closed error fires before `open_audit_db` is even reached in `main()`).
- `cargo build -p caprun`: compiles clean — the 14-call-site signature ripple from Plans 01/02/03 is fully reconciled.
- `cargo test -p caprun --no-fail-fast`: all tests pass (3 new + all existing e2e/planner/s9_live_block tests).
- `cargo test --workspace --no-fail-fast`: **green** (macOS baseline) — first full-workspace green of Phase 9/milestone v1.2. Linux-gated security/e2e tests show 0 passed, as expected per CLAUDE.md.
- `./scripts/check-invariants.sh`: both gates PASS.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add the --seed-from-file on-ramp and feed SeedProvenance into create_session** - `93d1848` (feat)
2. **Task 2: Integration test — file-derived seed starts Draft, trusted-arg starts Active** - `3d66422` (test)

## Files Created/Modified

- `cli/caprun/src/main.rs` - `--seed-from-file` flag parse (peek-and-strip before positional args); `create_session`/`run_broker_server` call sites updated to Plan 03's new signatures; `session_created` Event actor records provenance
- `cli/caprun/tests/origin_seed_provenance.rs` - new integration test file proving the provenance -> status mapping and the fail-closed missing-file path, macOS-runnable

## Decisions Made

- `--seed-from-file`'s file content replaces the positional `<intent-param>` slot entirely (usage becomes `caprun --seed-from-file <path> <intent-kind> <workspace-file> [audit-db-path]`) rather than keeping a now-ignored positional arg present in the invocation. RESEARCH.md's illustrative usage string (Pitfall 4) kept `<intent-param>` positional even with the flag present, but explicitly left the exact flag shape open (A2); requiring a caller to pass a value that gets silently discarded is a worse, more error-prone shape.
- The integration test drives the real `caprun` binary (same pattern as `e2e.rs`/`s9_live_block.rs`) rather than adding a `cli/caprun/src/lib.rs` to make the parsing logic separately unit-testable. `create_session`/`persist_session`/the `session_created` event append all complete in `main()` before the broker's Linux-only abstract-socket bind and the worker spawn — so the `sessions.status` row is durable and assertable regardless of platform or whether the rest of the run succeeds. This exercises the actual CLI code path (not a duplicate of its logic) without introducing a new crate target outside the plan's `files_modified`.

## Deviations from Plan

None — plan executed exactly as written. Both tasks' acceptance criteria were met without requiring any Rule 1/2/3 auto-fixes or Rule 4 architectural escalation.

## TDD Gate Compliance

Task 2 was tagged `tdd="true"`, but this plan's task ORDER is implementation-then-test (Task 1 = the `--seed-from-file` on-ramp implementation; Task 2 = the integration test proving it), not the canonical RED-then-GREEN sequence. Git log shows `feat` (`93d1848`) before `test` (`3d66422`) — the reverse of the standard TDD gate order. This is a plan-structure characteristic (the plan's own task split), not an executor deviation: Task 1's acceptance criteria required only `cargo build -p caprun` to compile (no test), and Task 2's `<behavior>` block describes assertions against Task 1's already-built on-ramp rather than a not-yet-built feature. No RED phase was skipped for any single unit of behavior — the test was written fresh in Task 2 and passed on first run without needing further implementation changes, which is the expected outcome when the underlying code was already correct.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Known Stubs

None. No hardcoded empty/placeholder values were introduced.

## Threat Flags

None. This plan's changes are exactly the mitigations named in its own `<threat_model>` (T-09-10, T-09-11) — no new, undocumented security-relevant surface was introduced. `--seed-from-file` is the plan's own explicitly-scoped new untrusted-input surface, fail-closed per V5.

## Next Phase Readiness

- Phase 9 (Session Trust State — I1 + I0) is now fully complete: all 4 plans (09-01 through 09-04) executed, `cargo test --workspace --no-fail-fast` green on macOS for the first time this milestone.
- ORIGIN-01/02 are exercised end-to-end at the CLI level for the first time — the "no on-ramp exists" gap flagged by RESEARCH Pitfall 4 is closed.
- Phase 11 (live acceptance) depends on Phase 9 (this phase) and Phase 10 (confirmation loop, independent). No blockers for either.
- The full live confined run (worker spawn + Landlock/seccomp, real Linux) for this on-ramp has not yet been re-verified via the Colima/Docker recipe — per this plan's own `<verification>` section, that re-run belongs to `/gsd-verify-work`, not this execution.

## Self-Check: PASSED
- FOUND: cli/caprun/src/main.rs
- FOUND: cli/caprun/tests/origin_seed_provenance.rs
- FOUND commit 93d1848
- FOUND commit 3d66422

---
*Phase: 09-session-trust-state-i1-i0*
*Completed: 2026-07-07*
