---
phase: 49-deterministic-multi-step-coding-planner
plan: 01
subsystem: planner
tags: [coding-planner, plan-stream, named-handles, multi-mint, CODE-01, CODE-02, SafeCodingWorkflow]

requires:
  - phase: 48-plan-stream-substrate
    provides: "plan_next + PlanStreamContext + sequential worker loop + opaque bag + stream_substrate tests"
provides:
  - "CaprunIntent::SafeCodingWorkflow closed coding variant with operator-typed fields"
  - "IntentAccepted.named_handles additive multi-mint wire"
  - "ProvideIntent sequential mint_from_intent multi-mint (Gate 3 only)"
  - "DeterministicPlanner::plan_next five-node coding recipe"
  - "Worker coding bag seed + claim-extract demotion skip"
  - "CODE-01/02 unit proofs (planner + proto_claims)"
affects:
  - 49-02-expressibility-coverage
  - 50-cli-multi-node-confirm-hold
  - 51-live-coding-proof

tech-stack:
  added: []
  patterns:
    - "Additive named_handles Vec<(String,ValueId)> on IntentAccepted; email/file pass empty vec"
    - "Static step-index plan_next recipe placing only intent-minted bag keys (never out_*)"
    - "Coding success path skips RequestFd/ReportClaims demotion"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/intent.rs
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs
    - cli/caprun/tests/planner.rs
    - crates/brokerd/tests/proto_claims.rs

key-decisions:
  - "Variant name SafeCodingWorkflow with 13 operator String fields (add-alongside email/file)"
  - "named_handles as ordered Vec pairs; primary value_id is write_path; all 13 keys also in named_handles"
  - "path and contents origin_role Some(path) for file.write Step 1c role gate"
  - "Coding worker path skips RequestFd entirely (no dummy seed-file demotion risk)"
  - "plan_from_intent coding arm returns non-product sink coding.use_plan_next (fail-closed if misused)"
  - "LlmPlanner exits fail-closed for SafeCodingWorkflow (no LLM multi-step)"

patterns-established:
  - "ProvideIntent multi-mint loop over ordered (bag_key, literal, role) pairs — Gate 3 only"
  - "plan_next_one_shot free fn shared by trait default + DeterministicPlanner non-coding arm"
  - "Success-path coding bag keys: write_path, write_contents, test_command, test_args, commit_message, push_remote, push_refspec, pr_owner, pr_repo, pr_base, pr_head, pr_title, pr_body"

requirements-completed: [CODE-01, CODE-02]

coverage:
  - id: D1
    description: "DeterministicPlanner::plan_next emits five sinks in order with exact sink_schema arg names"
    requirement: CODE-01
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#coding_plan_next_emits_five_sinks_in_order"
        status: pass
    human_judgment: false
  - id: D2
    description: "Success-path plan_next never places out_* handles into sink args"
    requirement: CODE-02
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#coding_success_path_does_not_place_out_handles"
        status: pass
    human_judgment: false
  - id: D3
    description: "ProvideIntent SafeCodingWorkflow multi-mints 13 distinct UserTrusted named handles"
    requirement: CODE-02
    verification:
      - kind: unit
        ref: "crates/brokerd/tests/proto_claims.rs#provide_intent_safe_coding_multi_mint_distinct_named_handles"
        status: pass
    human_judgment: false
  - id: D4
    description: "Email/file one-shot plan_next adapter remains green"
    requirement: CODE-01
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#plan_next_step0_matches_plan_for_email"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-07-29
status: complete
---

# Phase 49 Plan 01: Deterministic Multi-step Coding Planner Tracer Summary

**Closed CaprunIntent::SafeCodingWorkflow + Gate-3 multi-mint named_handles + DeterministicPlanner five-node plan_next recipe with CODE-01/02 unit proofs — no CLI multi-node (Phase 50) and no LIVE claim (Phase 51).**

## Performance

- **Duration:** 12 min
- **Started:** 2026-07-29T19:48:21Z
- **Completed:** 2026-07-29T20:00:00Z
- **Tasks:** 2
- **Files modified:** 9 (7 production/test + 2 fixture construction sites)

## Accomplishments

- Added closed `CaprunIntent::SafeCodingWorkflow` with 13 operator-typed String fields; email/file variants byte-stable.
- Extended `IntentAccepted` with additive `named_handles: Vec<(String, ValueId)>`; ProvideIntent multi-mints via sequential `mint_from_intent` only (Gate 3).
- `DeterministicPlanner::plan_next` emits `file.write → process.exec → git.commit → git.push → github.pr` with exact sink_schema arg names; success path never places `out_*`.
- Worker coding path seeds bag from named_handles and skips RequestFd/claim demotion; email/file path unchanged.
- Host unit proofs green: planner CODE-01/02 + proto_claims multi-mint distinct UserTrusted handles; `check-invariants.sh` Gates 1+3 green.

## Task Commits

Each task was committed atomically:

1. **Task 1: End-to-end coding recipe — intent, multi-mint, bag seed, plan_next** - `cb76008` (feat)
2. **Task 2: CODE-01 emission + CODE-02 multi-mint + email/file regression tests** - `ce5fc05` (test)

**Plan metadata:** (pending final docs commit)

## Files Created/Modified

- `crates/runtime-core/src/intent.rs` — SafeCodingWorkflow closed variant
- `crates/brokerd/src/proto.rs` — IntentAccepted.named_handles additive field
- `crates/brokerd/src/server.rs` — ProvideIntent multi-mint loop for coding; email/file empty named_handles
- `cli/caprun/src/planner.rs` — plan_coding_next, plan_next override, fail-closed plan()/LlmPlanner arms
- `cli/caprun/src/worker.rs` — named_handles destructure; coding bag seed + claim skip
- `cli/caprun/tests/planner.rs` — CODE-01 five-node + CODE-02 anti-launder + missing-key tests
- `crates/brokerd/tests/proto_claims.rs` — named_handles round-trip + multi-mint dispatch test
- `crates/brokerd/tests/replay_cas.rs` / `two_connection_intent_bypass.rs` — IntentAccepted field exhaustiveness

## Decisions Made

- **SafeCodingWorkflow name + field set** locked per RESEARCH Pattern 1 (DESIGN §8.3 naming discretion).
- **All 13 keys in named_handles** (including write_path); primary `value_id` equals write_path handle.
- **Skip RequestFd for coding** rather than keep a dummy seed-file read that risks demotion (CODE-02).
- **Staging folded into test_command/test_args** — no sixth node for CODE-01 minimum.
- **No CLI coding verb** — main.rs remains fail-closed on unknown intent kinds (Phase 50).

## Deviations from Plan

None - plan executed exactly as written.

Minor implementation notes (not deviations):
- `plan_next_one_shot` extracted as free fn with `?Sized` so trait default and DeterministicPlanner share the one-shot adapter body.
- SafeCodingWorkflow with `primary_file_derived=true` is rejected fail-closed at ProvideIntent (operator-typed multi-mint only).

## Issues Encountered

- Host lacked `pkg-config`/`libssl-dev` for lettre's native-tls path; used user-local extracted debs under `~/.local/openssl-dev` for compile/test (no repo change). Docker/mailpit-verify not required for these host-safe unit tests.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 49-02 can add LIVE-08 expressibility test-only path + COVERAGE.md.
- Phase 50 can productize CLI multi-node coding verb + Block-and-Hold on this spine.
- Phase 51 LIVE proof still blocked until CLI + confirm-hold product surface exists.

## Self-Check: PASSED

- All key files present
- Commits `cb76008`, `ce5fc05` present in git log
- `check-invariants.sh` green
- `cargo test -p caprun --test planner` 25 passed
- `cargo test -p brokerd --test proto_claims` 16 passed

---
*Phase: 49-deterministic-multi-step-coding-planner*
*Completed: 2026-07-29*
