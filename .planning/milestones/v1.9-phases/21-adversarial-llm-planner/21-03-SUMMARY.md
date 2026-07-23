---
phase: 21-adversarial-llm-planner
plan: 03
subsystem: planner
tags: [rust, planner-seam, worker, unix-socket, tdd]

# Dependency graph
requires:
  - phase: 21-adversarial-llm-planner
    plan: 01
    provides: "The pure `llm-planner` wire contract (`PlannerRequest`, `HandleLabel`, `PlannerResponse`, `ResponseArg`) `LlmPlanner` builds requests from and validates responses against."
provides:
  - "`LlmPlanner`: a second `impl Planner` (the Phase-20 seam) that proxies to an off-process LLM sidecar over an abstract UDS, forwarding only `ValueId` handles + slot hints and mapping the reply back to a `PlanNode` via a fail-closed pure validator."
  - "Env-selected planner construction in the confined worker (`CAPRUN_PLANNER=llm` vs default `DeterministicPlanner`) — zero regression to the existing deterministic path."
  - "caprun-main sidecar spawn/teardown wiring (env `CAPRUN_PLANNER=llm` gated) that will start the Plan-02 `caprun-planner` binary and propagate `PLANNER_SOCK`/`CAPRUN_PLANNER` to the worker."
affects: ["21-02-llm-sidecar", "21-04-live-acceptance"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Planner-side pure-function split: `build_planner_request`/`response_to_plan_node` are pure (no I/O), so the fail-closed request/response mapping is unit-testable without a live sidecar; the `LlmPlanner::plan()` trait method wraps them with the actual UDS connect/send/recv and the fail-closed `process::exit(1)` on any error."
    - "Duplicated (not shared) framed-JSON helpers between worker.rs and planner.rs — required because `tests/planner.rs` compiles `src/planner.rs` standalone via `#[path]` with no access to `worker.rs`'s private items; both copies use the identical 4-byte-LE-length-prefix + JSON wire format."

key-files:
  created: []
  modified:
    - cli/caprun/Cargo.toml
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs
    - cli/caprun/src/main.rs
    - cli/caprun/tests/planner.rs

key-decisions:
  - "LlmPlanner's fail-closed path uses std::process::exit(1) inside plan() (not a Result), because Planner::plan()'s trait signature is `-> PlanNode` (infallible) — this matches the plan's explicit instruction and the worker's existing fail-closed exit posture on a §9 block."
  - "The sidecar reply is expected as a single framed PlannerResponse (no separate typed 'error signal' wire variant) — any sidecar-side failure that can't produce a valid response manifests as a connect failure, a closed connection, or a JSON-parse failure on our recv_framed, all of which this proxy treats identically: fail closed. This avoids requiring an unstated shared error-wire-type between the two Wave-2 sibling plans (21-02 and 21-03), which depend only on 21-01, not on each other."
  - "build_planner_request always offers all three slots (recipient/subject/body) regardless of intent kind, using the exact override rule plan_from_intent already uses (derived_recipient/body win when Some, else the trusted fallback) — since all six incoming routing params are non-Option after resolution, no slot is ever actually omitted in practice; the 'non-None handles' language in the plan is future-proofing for intents that might not populate all three."
  - "connect_to_sidecar reimplements a blocking (std, not tokio) bounded connect-retry, because Planner::plan() is a synchronous trait method — the worker's own broker connect-retry (worker.rs) is tokio-async and structurally can't be called from here."

requirements-completed: [PLANNER-03]

coverage:
  - id: D1
    description: "LlmPlanner implements the Phase-20 Planner trait with the identical six-param plan() signature; DeterministicPlanner unchanged"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#plan_from_intent_create_file_clean_routes_intent_path (regression, unchanged)"
        status: pass
      - kind: other
        ref: "cargo build -p caprun --bins"
        status: pass
    human_judgment: false
  - id: D2
    description: "response_to_plan_node validates sink + every value_id against the offered set, failing closed on unknown sink / unoffered handle; Ok for a valid response"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#response_to_plan_node_ok_for_valid_response"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#response_to_plan_node_err_for_unknown_sink"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#response_to_plan_node_err_for_unoffered_handle"
        status: pass
    human_judgment: false
  - id: D3
    description: "build_planner_request offers exactly {recipient, subject, body} handles tagged with slot hints, using the same routing override rule as plan_from_intent, with available_sinks by intent kind"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#build_planner_request_offers_recipient_subject_body_handles"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#build_planner_request_create_file_offers_file_create_sink"
        status: pass
    human_judgment: false
  - id: D4
    description: "llm-planner dependency stays pure (no reqwest) in the caprun binary's dependency tree"
    requirement: "PLANNER-03"
    verification:
      - kind: other
        ref: "cargo tree -p caprun | grep -c reqwest -> 0"
        status: pass
    human_judgment: false
  - id: D5
    description: "Worker selects LlmPlanner vs DeterministicPlanner by CAPRUN_PLANNER env var; default (unset) preserves every existing e2e test's behavior unchanged"
    requirement: "PLANNER-03"
    verification:
      - kind: integration
        ref: "cargo test -p caprun --test e2e (Linux-gated; 0 passed on macOS by design, compiles clean)"
        status: pass
      - kind: other
        ref: "cargo build -p caprun --bins (0 warnings)"
        status: pass
    human_judgment: true
    rationale: "The CAPRUN_PLANNER=llm live path (sidecar spawn + worker connecting post-confinement) requires the Plan-02 caprun-planner binary to exist and a Linux target to run against — genuinely exercisable only once 21-02 and 21-04's live acceptance run; this plan's own scope is compile-time wiring + unit coverage of the pure functions, verified above."
  - id: D6
    description: "No crates/brokerd, crates/executor, or ConnectionRole/DeclarePlannerRole dispatch code modified — TCB-adjacent, not a TCB change"
    requirement: "PLANNER-03"
    verification:
      - kind: other
        ref: "git diff --name-only touches only cli/caprun/{Cargo.toml,src/planner.rs,src/worker.rs,src/main.rs,tests/planner.rs}"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh"
        status: pass
    human_judgment: false

duration: ~15min
completed: 2026-07-11
status: complete
---

# Phase 21 Plan 03: Worker-Side LlmPlanner Proxy Summary

**`LlmPlanner` implements Phase 20's `Planner` trait as a thin, fail-closed proxy to an off-process LLM sidecar over an abstract UDS — forwards only `ValueId` handles + slot hints, never a literal — with `CAPRUN_PLANNER=llm` env selection in the worker and matching sidecar-spawn wiring in caprun main; default deterministic path unchanged, zero TCB (brokerd/executor) modifications.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-11
- **Tasks:** 2 completed
- **Files modified:** 5 (`cli/caprun/Cargo.toml`, `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs`, `cli/caprun/src/main.rs`, `cli/caprun/tests/planner.rs`)

## Accomplishments
- Added `llm-planner` as a path dependency of `caprun` — confirmed `cargo tree -p caprun | grep -c reqwest` still prints `0`.
- Implemented `LlmPlanner` (`impl Planner`) in `cli/caprun/src/planner.rs`: the identical six-param `plan()` signature as `DeterministicPlanner`, connecting to `\0` + `PLANNER_SOCK` with a bounded blocking connect-retry, sending a framed `PlannerRequest`, and mapping the framed `PlannerResponse` reply to a `PlanNode` via the pure `response_to_plan_node` validator. Any connect/transport/validation failure prints a clear stderr message and `std::process::exit(1)` — fail closed, no `PlanNode` ever submitted.
- Implemented `build_planner_request` (pure): builds the `PlannerRequest` offering exactly `{recipient, subject, body}` handles using the SAME routing-override rule `plan_from_intent` already uses, plus the effective `offered`/`known_sinks` allowlists to feed straight into `response_to_plan_node` — `available_sinks` is `["email.send"]` for `SendEmailSummary`, `["file.create"]` for `CreateFileFromReport`.
- Wired `cli/caprun/src/worker.rs`: `CAPRUN_PLANNER` env selects `Box<dyn Planner>` — `"llm"` constructs `LlmPlanner::new(PLANNER_SOCK)`, everything else (unset/other) stays `DeterministicPlanner`. Documented the ordering invariant (sidecar connect happens AFTER `sandbox::apply_confinement()`, legal because the seccomp filter only denies `AF_INET`/`AF_INET6`/execve, not `AF_UNIX`).
- Wired `cli/caprun/src/main.rs`: when `CAPRUN_PLANNER=llm`, spawns the `caprun-planner` sidecar (resolved via `current_exe().parent()`, same pattern as the worker binary) after the broker task and before the worker, passing `PLANNER_SOCK` + forwarded `OPENAI_API_KEY` + `CAPRUN_PLANNER_MODEL`; propagates `PLANNER_SOCK` + `CAPRUN_PLANNER=llm` into the worker's env; tears the sidecar down (`kill` + `wait` via `spawn_blocking`) after the worker exits, mirroring the existing `broker_task.abort()` teardown. When `CAPRUN_PLANNER` is unset, spawns no sidecar and adds nothing to the worker's env — the default path is byte-for-byte unchanged.
- 10/10 unit tests green in `cli/caprun/tests/planner.rs` (6 pre-existing `plan_from_intent` regression tests + 4 new tests for `build_planner_request`/`response_to_plan_node`, TDD RED→GREEN).

## Task Commits

Task 1 followed a RED→GREEN TDD cycle (2 commits); Task 2 was a single non-TDD wiring commit:

1. **Task 1: LlmPlanner proxy — impl Planner, fail-closed response mapping**
   - `0f2704f` (test) — RED: added `llm-planner` path dependency + 4 new tests referencing not-yet-existing `build_planner_request`/`response_to_plan_node`; confirmed `cargo test -p caprun --test planner` fails to compile (6 compile errors).
   - `e5bc003` (feat) — GREEN: implemented `LlmPlanner`, `build_planner_request`, `response_to_plan_node`, plus the sidecar connect/framed-IO helpers; 10/10 tests green, `cargo tree -p caprun | grep -c reqwest` == 0.
2. **Task 2: Worker selection + caprun main sidecar spawn wiring**
   - `0aae350` (feat) — `worker.rs` env-selected `Box<dyn Planner>`; `main.rs` sidecar spawn/env-propagation/teardown. `cargo build -p caprun --bins` clean (0 warnings — the Task 1 dead-code warnings on `LlmPlanner`'s methods resolved once this task calls them); `cargo test -p caprun --test e2e/planner/confirm` all green; `./scripts/check-invariants.sh` passes.

**Plan metadata:** this SUMMARY's own commit (see below).

## Files Created/Modified
- `cli/caprun/Cargo.toml` — added `llm-planner = { path = "../../crates/llm-planner" }`.
- `cli/caprun/src/planner.rs` — added `LlmPlanner` (`impl Planner`), `build_planner_request`, `response_to_plan_node`, `connect_to_sidecar`, and duplicated `send_framed`/`recv_framed` framed-JSON helpers.
- `cli/caprun/src/worker.rs` — replaced the fixed `DeterministicPlanner` construction with an env-selected `Box<dyn Planner>`.
- `cli/caprun/src/main.rs` — added sidecar spawn (step 3b) and teardown (step 5b) gated on `CAPRUN_PLANNER=llm`, plus worker env propagation.
- `cli/caprun/tests/planner.rs` — added 4 unit tests for the new pure functions.

## Decisions Made
- `LlmPlanner`'s fail-closed path is `std::process::exit(1)` inside `plan()` (not a `Result`) because `Planner::plan()`'s trait signature is infallible (`-> PlanNode`) — matches the plan's instruction and the worker's existing fail-closed exit posture on a §9 block.
- The sidecar reply wire format is a single framed `PlannerResponse` with no separate typed "error signal" — any sidecar-side failure surfaces as a connect failure, closed connection, or JSON-parse failure on `recv_framed`, all treated identically (fail closed). This sidesteps needing an unstated shared error-wire-type between this plan (21-03, depends only on 21-01) and its Wave-2 sibling (21-02), which is being built in a separate parallel worktree.
- `send_framed`/`recv_framed` are duplicated (not imported) from `worker.rs` into `planner.rs`, because `tests/planner.rs` compiles `src/planner.rs` standalone via `#[path]` — it has no access to `worker.rs`'s private items. Both copies use the identical 4-byte-LE-length-prefix + JSON wire format.
- `build_planner_request` always offers all three slots (`recipient`/`subject`/`body`), using the exact override rule `plan_from_intent` already uses — since all six incoming routing params resolve to non-`Option` handles, no slot is ever actually omitted in practice.

## Deviations from Plan

None — plan executed exactly as written. The `Cargo.lock` update from adding the `llm-planner` path dependency is expected automatic build-artifact churn, bundled into the Task 1 RED commit (needed for the failing test to even resolve the `llm_planner` import).

## Issues Encountered

None. Both tasks' verification commands passed on the first run; no auto-fixes were needed. The initial `cargo test -p caprun --test planner` RED run correctly failed to compile with the expected "cannot find function" errors before implementation.

## User Setup Required

None — no external service configuration required by this plan. (Plan 02, built in a sibling worktree, owns the `OPENAI_API_KEY`/`CAPRUN_PLANNER_MODEL` user-setup requirements for the sidecar itself.)

## Next Phase Readiness

`LlmPlanner` is selectable behind the Phase-20 seam and ready to integrate with Plan 02's `caprun-planner` sidecar binary once that lands (parallel Wave-2 worktree). The wire format this proxy expects (framed JSON `PlannerRequest` out, framed JSON `PlannerResponse` in, over `\0` + `PLANNER_SOCK`) is the integration contract Plan 02 must match; Plan 04 (live acceptance) is the point where end-to-end compatibility between the two sibling plans gets verified for the first time. No blockers for merging this plan's own scope — it builds, all its own tests pass, `cargo tree -p caprun` stays reqwest-free, and `./scripts/check-invariants.sh` passes.

---
*Phase: 21-adversarial-llm-planner*
*Completed: 2026-07-11*

## Self-Check: PASSED

All modified files verified present (`cli/caprun/Cargo.toml`, `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs`, `cli/caprun/src/main.rs`, `cli/caprun/tests/planner.rs`, this SUMMARY.md); all 3 commit hashes (`0f2704f`, `e5bc003`, `0aae350`) verified present in git log; `cargo test -p caprun --test planner` re-confirmed 10/10 passing; `cargo build -p caprun --bins` clean; `./scripts/check-invariants.sh` passes.
