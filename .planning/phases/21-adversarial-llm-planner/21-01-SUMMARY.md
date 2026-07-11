---
phase: 21-adversarial-llm-planner
plan: 01
subsystem: planner
tags: [rust, serde, wire-contract, llm-planner, tdd]

# Dependency graph
requires:
  - phase: 20-planner-seam-capability-split
    provides: "The `Planner` trait seam (`cli/caprun/src/planner.rs`) and the broker's `ConnectionRole` capability model that a planner-role connection will speak this wire contract over."
provides:
  - "New pure lib crate `llm-planner`: the literal-free wire contract (`PlannerRequest`, `HandleLabel`, `PlannerResponse`, `ResponseArg`) between the worker-side `LlmPlanner` proxy and the out-of-process LLM sidecar."
  - "`build_planner_prompt(&PlannerRequest) -> String` — pure prompt constructor, the GATE-04 seam Phase 22's sentinel test will target."
  - "`build_tool_schema(&PlannerRequest) -> serde_json::Value` — `emit_plan_node` tool schema whose `value_id` enum is derived exactly from offered handles."
  - "`parse_planner_response(...) -> Result<PlannerResponse, PlannerError>` — fail-closed validator: Err on malformed JSON, unknown sink, or unoffered handle; never fabricates a handle."
affects: ["21-02-llm-sidecar", "21-03-worker-proxy", "22-hard-gate-composed-acceptance"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure literal-free wire-type crate: no reqwest/tokio, structural key-set serde tests prove the absence of a literal-carrying field (rather than asserting it in prose)."
    - "Fail-closed validator taking caller-supplied `offered`/`known_sinks` sets — no wildcard fallback arm; every rejection path is an explicit `PlannerError` variant."

key-files:
  created:
    - crates/llm-planner/Cargo.toml
    - crates/llm-planner/src/lib.rs
  modified:
    - Cargo.lock

key-decisions:
  - "HandleLabel/PlannerRequest carry no literal field at all (not even an Option) — the structural guarantee is proven by asserting the serialized JSON key set equals exactly {slot_hint, value_id} / {intent_kind, available_handles, available_sinks}."
  - "parse_planner_response takes `offered: &[ValueId]` and `known_sinks: &[String]` as explicit caller-supplied allowlists rather than trusting anything in the parsed PlannerResponse itself — the sidecar/proxy in Plans 02/03 must pass the exact set offered that request."
  - "PlannerError has no catch-all/wildcard variant; MalformedJson/UnknownSink/UnknownHandle are the only three outcomes besides Ok, matching the plan's fail-closed requirement."

requirements-completed: [PLANNER-03]

coverage:
  - id: D1
    description: "Literal-free wire contract (PlannerRequest/HandleLabel/PlannerResponse/ResponseArg) proven structurally incapable of carrying a literal via key-set serde tests"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#planner_request_key_set_has_no_literal_field"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#handle_label_key_set_has_no_literal_field"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#planner_request_round_trips_through_serde_json"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#planner_response_round_trips_through_serde_json"
        status: pass
    human_judgment: false
  - id: D2
    description: "build_planner_prompt is a pure function built only from handle IDs + slot hints (the GATE-04 seam)"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#build_planner_prompt_contains_every_handle_id_and_slot_hint"
        status: pass
    human_judgment: false
  - id: D3
    description: "build_tool_schema structurally constrains value_id to exactly the offered handle IDs"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#build_tool_schema_value_id_enum_equals_offered_handles"
        status: pass
    human_judgment: false
  - id: D4
    description: "parse_planner_response fails closed on unknown sink, unoffered handle, and malformed JSON; Ok on valid response"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#parse_planner_response_ok_for_valid_response"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#parse_planner_response_err_for_unknown_sink"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#parse_planner_response_err_for_unoffered_handle"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#parse_planner_response_err_for_malformed_json"
        status: pass
    human_judgment: false
  - id: D5
    description: "llm-planner crate is pure — no reqwest, no tokio in its dependency tree; full workspace still builds and check-invariants.sh still passes"
    verification:
      - kind: other
        ref: "cargo tree -p llm-planner (manual inspection, no reqwest/tokio present)"
        status: pass
      - kind: other
        ref: "cargo build --workspace"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-11
status: complete
---

# Phase 21 Plan 01: llm-planner Wire Contract Summary

**New pure `llm-planner` crate carrying the literal-free handle-only wire contract (PlannerRequest/HandleLabel/PlannerResponse/ResponseArg), a pure `build_planner_prompt` seam, an `enum`-constrained `build_tool_schema`, and a fail-closed `parse_planner_response` validator — zero reqwest/tokio, 10/10 unit tests green on macOS.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-11
- **Tasks:** 2 completed
- **Files modified:** 2 created (`crates/llm-planner/Cargo.toml`, `crates/llm-planner/src/lib.rs`), 1 lockfile update (`Cargo.lock`)

## Accomplishments
- Created the `llm-planner` pure lib crate (no reqwest, no tokio) — picked up automatically by the existing `crates/*` workspace-members glob, no root `Cargo.toml` edit.
- Defined the literal-free wire types (`PlannerRequest`, `HandleLabel`, `PlannerResponse`, `ResponseArg`), reusing `runtime_core::plan_node::ValueId` so the sidecar and proxy speak the same opaque-handle representation the broker mints. Structurally proved (key-set serde tests, not prose) that neither `PlannerRequest` nor `HandleLabel` has any literal-carrying field.
- Implemented `build_planner_prompt` — a single pure function reading only handle IDs + slot hints + intent_kind + sink names, with no I/O — the exact seam Phase 22's GATE-04 sentinel assertion will target.
- Implemented `build_tool_schema` — derives the `emit_plan_node` JSON schema whose `value_id` field's `enum` is built exactly from `PlannerRequest.available_handles`, so a conforming tool-call can reference only an offered handle.
- Implemented `parse_planner_response` + `PlannerError` (MalformedJson/UnknownSink/UnknownHandle, no wildcard fallback) — fails closed on any response whose sink or arg `value_id` wasn't in the caller-supplied allowlists.

## Task Commits

Both tasks used `tdd="true"` and followed a RED→GREEN cycle (2 commits each):

1. **Task 1: Create the llm-planner crate and its typed wire contract**
   - `82b0a10` (test) — RED: placeholder test referencing not-yet-existing wire types; confirmed `cargo test -p llm-planner --lib` fails to compile.
   - `bf0110b` (feat) — GREEN: implemented `PlannerRequest`/`HandleLabel`/`PlannerResponse`/`ResponseArg` + round-trip and key-set tests; 4/4 green.
2. **Task 2: Prompt builder, tool schema, and fail-closed response validator**
   - `6366a33` (test) — RED: placeholder test referencing `build_planner_prompt`/`build_tool_schema`/`parse_planner_response`/`PlannerError`; confirmed compile failure.
   - `8896e26` (feat) — GREEN: implemented all three functions + `PlannerError`; 10/10 tests green (full suite).

**Plan metadata:** this SUMMARY's own commit (see below).

## Files Created/Modified
- `crates/llm-planner/Cargo.toml` — new pure lib crate; deps: `runtime-core` (path), `serde` (workspace, derive), `serde_json` (workspace). No reqwest, no tokio.
- `crates/llm-planner/src/lib.rs` — wire types, `build_planner_prompt`, `build_tool_schema`, `parse_planner_response`, `PlannerError`, and 10 unit tests.
- `Cargo.lock` — updated automatically by `cargo build` to include the new workspace member (no new external dependency; `llm-planner` v0.1.0 entry only).

## Decisions Made
- HandleLabel/PlannerRequest have zero literal/value/text fields (not even an `Option<String>`) — the absence is proven structurally via key-set serde assertions on the parsed `serde_json::Value`, not just documented in prose, so a future accidental field addition breaks the test immediately.
- `parse_planner_response` takes explicit `offered: &[ValueId]` and `known_sinks: &[String]` parameters rather than trusting any field embedded in the parsed response — the sidecar (Plan 02) and worker-side proxy (Plan 03) are both responsible for passing the exact allowlist they offered that request.
- `PlannerError` has exactly three variants (`MalformedJson`, `UnknownSink`, `UnknownHandle`) with no wildcard/catch-all arm, matching the plan's explicit "no wildcard fallback" instruction.

## Deviations from Plan

None — plan executed exactly as written. `Cargo.lock` was updated as an automatic side effect of `cargo build` picking up the new workspace member; this is expected build-artifact churn, not a logic change to any other crate.

## Issues Encountered

None. `cargo build --workspace` and `./scripts/check-invariants.sh` both passed on the first run after Task 2's implementation; no auto-fixes were needed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

`crates/llm-planner` is ready for Plan 02 (the out-of-process LLM sidecar, which will call `build_planner_prompt`/`build_tool_schema`/`parse_planner_response` over its own transport) and Plan 03 (the worker-side `LlmPlanner` proxy implementing the `Planner` trait from Phase 20). No blockers. `cargo tree -p llm-planner` was manually inspected and confirmed to contain no `reqwest` and no `tokio` — Plan 02 will be the first to add real network/process wiring, deliberately kept out of this pure crate.

---
*Phase: 21-adversarial-llm-planner*
*Completed: 2026-07-11*

## Self-Check: PASSED

All created files verified present (`crates/llm-planner/Cargo.toml`, `crates/llm-planner/src/lib.rs`, this SUMMARY.md); all 5 commit hashes (`82b0a10`, `bf0110b`, `6366a33`, `8896e26`, `ee6fabb`) verified present in git log; `cargo test -p llm-planner --lib` re-run and confirmed 10/10 passing.
