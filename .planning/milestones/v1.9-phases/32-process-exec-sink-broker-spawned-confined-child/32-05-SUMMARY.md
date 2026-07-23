---
phase: 32-process-exec-sink-broker-spawned-confined-child
plan: 05
subsystem: brokerd
tags: [rust, taint-model, mint-site, process-exec, i1, i2, wire-protocol]

# Dependency graph
requires:
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 01
    provides: "TaintLabel::ExecRaw + process.exec sink schema/sensitivity tables"
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 04
    provides: "invoke_process_exec — spawns caprun-exec-launcher, captures combined stdout+stderr, records the two-phase process_exited/process_spawn_failed audit, returns (event_id, hash, combined_output)"
provides:
  - "crates/brokerd/src/quarantine.rs::mint_from_exec — the SOLE process.exec output-mint site, rooted on invoke_process_exec's already-appended process_exited event id (genuine, non-stapled taint chain)"
  - "scripts/check-invariants.sh Gate 3's 4th mint-call-site restriction (mint_from_exec()"
  - "BrokerResponse::PlanNodeDecision.output_value_id — the opaque exec-output ValueId handle returned to the worker on an Allowed process.exec decision"
  - "server.rs's Allowed && sink==process.exec dispatch arm: spawn, mint, return handle"
affects: ["32-06 (Linux acceptance test drives a tainted exec output routed into a sensitive sink arg to a deterministic Block, EXEC-03)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "mint_from_exec mints ONLY — it does not append its own audit Event; it roots provenance_chain[0] on the caller-supplied spawn_event_id (invoke_process_exec's already-appended process_exited id), the strongest non-stapling guarantee (mirrors mint_from_read_anchor_identity's single-event anchor, one event serving both the exit record and the mint root)"
    - "evaluate_plan_node_and_record now returns (ExecutorDecision, Option<ValueId>) instead of a bare ExecutorDecision — every caller (planner-reduced branch, worker branch) must explicitly acknowledge/discard the second element"

key-files:
  created: []
  modified:
    - crates/brokerd/src/quarantine.rs
    - scripts/check-invariants.sh
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/worker.rs
    - crates/brokerd/tests/replay_cas.rs
    - crates/brokerd/tests/two_connection_intent_bypass.rs
    - crates/brokerd/tests/harden01_session_integrity.rs

key-decisions:
  - "mint_from_exec does NOT demote the session (mirrors mint_from_derivation, NOT mint_from_read's I1 worker-report demotion) — exec taint is set structurally by this function, not via a worker self-report, so no I1 trust-flip is implicated; I2 Blocks a tainted exec value at the sink regardless of session status. Pinned per the plan's locked decision (RESEARCH A2); flagged for a fresh adversarial review in Phase 34."
  - "output_value_id is a plain required field on BrokerResponse::PlanNodeDecision, deliberately NOT #[serde(default)] (Pitfall 8) — every construction/destructure site is compiler-forced to acknowledge it, closing the silent-default-masks-a-missed-site threat (T-32-21)."
  - "PlanNodeDecisionReduced (the planner wire) is unchanged — the planner call site destructures evaluate_plan_node_and_record's returned tuple and explicitly discards the ValueId (T-04-02: the reduced signal never carries the handle)."
  - "invoke_process_exec is called with conn passed directly (never pre-locked) — it locks internally only for its own append_event call, never across the spawn/capture/timeout .await sequence, mirroring the file.create/email.send Allowed-dispatch arms' locking discipline."

patterns-established: []

requirements-completed: []  # See "REQUIREMENTS.md Not Updated (Deliberate)" below

coverage:
  - id: D1
    description: "mint_from_exec mints the combined exec output as a ValueNode with taint [ExternalUntrusted, ExecRaw], origin_role Some(\"exec_output\"), provenance_chain == [spawn_event_id] (genuine, non-stapled — the same id invoke_process_exec's process_exited event already carries); does not append a fresh event of its own"
    requirement: "EXEC-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/quarantine.rs#tests::mint_from_exec_anchor_identity"
        status: pass
    human_judgment: false
  - id: D2
    description: "check-invariants.sh Gate 3 restricts a 4th token mint_from_exec( to quarantine.rs + server.rs, added in the SAME commit that introduces mint_from_exec"
    verification:
      - kind: unit
        ref: "bash scripts/check-invariants.sh (4/4 gates PASS, Gate 3 line present); grep -rn 'mint_from_exec(' crates/ cli/ confirms only quarantine.rs (def + test) and server.rs (call site)"
        status: pass
    human_judgment: false
  - id: D3
    description: "On an Allowed process.exec decision, server.rs calls invoke_process_exec then mint_from_exec and returns the minted ValueId to the worker via BrokerResponse::PlanNodeDecision.output_value_id (Some only for Allowed process.exec, None otherwise); PlanNodeDecisionReduced (planner wire) is unchanged"
    requirement: "EXEC-03"
    verification:
      - kind: unit
        ref: "cargo build --workspace (compiles with the new required output_value_id field, all call sites updated); cargo test -p brokerd --no-run (all test binaries compile)"
        status: pass
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast (full suite green, no regressions, incl. replay_cas.rs, two_connection_intent_bypass.rs, harden01_session_integrity.rs, planner_reduced_signal.rs)"
        status: pass
    human_judgment: false
  - id: D4
    description: "The worker learns the exec-output ValueId (never the raw bytes — I1 preserved); worker.rs destructures and binds output_value_id to a local it can later route into a subsequent PlanArg"
    verification:
      - kind: unit
        ref: "cargo build --workspace compiles cli/caprun with the new destructure; worker.rs never resolves/prints the literal, only holds the opaque handle"
        status: pass
    human_judgment: false

duration: 20min
completed: 2026-07-17
status: complete
---

# Phase 32 Plan 05: mint_from_exec + output_value_id wiring Summary

**`mint_from_exec` mints captured `process.exec` output as a genuinely-rooted untrusted `ValueNode` — provenance_chain anchored on `invoke_process_exec`'s already-appended `process_exited` event id, never a stapled/fresh root — and the minted handle is now wired back to the worker via a new required `BrokerResponse::PlanNodeDecision.output_value_id` field, closing the producer→consumer path EXEC-03's later Block depends on.**

## Performance

- **Duration:** 20 min
- **Started:** 2026-07-17T22:05:00Z (approx, continuation of Phase 32 session)
- **Completed:** 2026-07-17T22:25:00Z
- **Tasks:** 3 completed
- **Files modified:** 8

## Accomplishments

- Added `crates/brokerd/src/quarantine.rs::mint_from_exec` beside `mint_from_read`/`mint_from_derivation` — the SOLE `process.exec` output-mint site. Mints ONLY (no fresh Event appended): `provenance_chain == [spawn_event_id]` (the caller's already-appended `process_exited` event id), taint `[ExternalUntrusted, ExecRaw]`, `origin_role Some("exec_output")`. Does NOT demote the session (locked decision, mirrors `mint_from_derivation`). Documented the fail-closed unknown-classification discipline (DESIGN §2.3) even though exec output has exactly one recognized shape today.
- Extended `scripts/check-invariants.sh` Gate 3 with a 4th `check_mint_token "mint_from_exec(" ...` call restricting the token to `quarantine.rs`/`server.rs`, landed in the SAME commit as `mint_from_exec` itself (DESIGN §2.4 mandate). Updated the Gate 3 header/summary comments for accuracy.
- Added `output_value_id: Option<ValueId>` to `BrokerResponse::PlanNodeDecision` (proto.rs) — a plain required field, no `#[serde(default)]` (Pitfall 8) — and compile-fixed every flagged site: server.rs's sole production construction, worker.rs's destructure, and three test-file match arms (`replay_cas.rs`, `two_connection_intent_bypass.rs`, `harden01_session_integrity.rs`) that assert on `decision` only. `PlanNodeDecisionReduced` (planner wire) untouched.
- Changed `evaluate_plan_node_and_record`'s return type to `(ExecutorDecision, Option<ValueId>)` and added the `Allowed && plan_node.sink.0 == "process.exec"` dispatch arm mirroring the `file.create`/`email.send` Allowed arms: calls `invoke_process_exec` (conn passed directly, never pre-locked), advances the causal chain head to the returned `process_exited` event, then calls `mint_from_exec` rooted on that SAME event id, and returns `Some(value_id)`. Every other sink/decision returns `None`. The worker call site now forwards the real handle; the planner call site explicitly discards it (T-04-02).

## Task Commits

Each task was committed atomically:

1. **Task 1: mint_from_exec + Gate 3 extension (same commit)** - `52af35c` (feat)
2. **Task 2: Add the output_value_id response field + compile-fix all sites** - `f0ff1b0` (feat)
3. **Task 3: server.rs process.exec Allowed dispatch arm — spawn, mint, return the handle** - `69ab5bb` (feat)

## Files Created/Modified

- `crates/brokerd/src/quarantine.rs` - added `mint_from_exec` (sole process.exec output-mint site) + inline `mint_from_exec_anchor_identity` unit test
- `scripts/check-invariants.sh` - Gate 3 4th `check_mint_token "mint_from_exec("` call + updated header/summary comments
- `crates/brokerd/src/proto.rs` - `BrokerResponse::PlanNodeDecision` gained required `output_value_id: Option<ValueId>` field; `PlanNodeDecisionReduced` unchanged
- `crates/brokerd/src/server.rs` - `evaluate_plan_node_and_record` returns `(ExecutorDecision, Option<ValueId>)`; new process.exec Allowed dispatch arm (spawn + mint); planner call site discards the ValueId; worker call site forwards it
- `cli/caprun/src/worker.rs` - destructures `output_value_id` from `PlanNodeDecision`, binds it to a local for future routing
- `crates/brokerd/tests/replay_cas.rs` - two match arms updated with `, ..`
- `crates/brokerd/tests/two_connection_intent_bypass.rs` - two match arms updated with `, ..`
- `crates/brokerd/tests/harden01_session_integrity.rs` - one match arm updated with `, ..` (the `:382` string literal inside a scripted-planner stdout format string was correctly left untouched — not a destructure)

## Decisions Made

- **`mint_from_exec` does NOT demote the session** — mirrors `mint_from_derivation`'s "does NOT demote" (not `mint_from_read`'s I1 worker-report demotion). Exec taint is set structurally at mint time, not via a worker self-report, so no I1 trust-flip is implicated; I2 Blocks a tainted exec value at the sink regardless of session status. Pinned per the plan's locked decision (RESEARCH A2) — flagged in the function's doc comment for a fresh adversarial reviewer in Phase 34 to confirm.
- **`output_value_id` is a required field, never `#[serde(default)]`** — the compiler forces every construction/destructure site to explicitly acknowledge it, so a future new sink cannot silently forget to populate/consume the handle (closes T-32-21).
- **`PlanNodeDecisionReduced` stays untouched** — the planner connection's reduced wire never carries the exec-output handle (T-04-02); the planner call site in server.rs explicitly destructures-and-discards the second tuple element rather than silently ignoring it.

## Deviations from Plan

None - plan executed exactly as written. The plan's locked decisions (args JSON-encode from 32-04, output_value_id wire field, mint locus server.rs) were all followed as specified.

## Issues Encountered

None. `cargo build --workspace`, `cargo test -p brokerd --no-run`, `cargo test --workspace --no-fail-fast` (full suite, no regressions), and `./scripts/check-invariants.sh` (4/4 gates, including the new Gate-3 `mint_from_exec(` line) are all green on Mac after each task.

## User Setup Required

None - no external service configuration required.

## REQUIREMENTS.md Not Updated (Deliberate)

This plan's frontmatter lists `requirements: [EXEC-02, EXEC-03]`. **Deliberately left `.planning/REQUIREMENTS.md` unmarked**, mirroring 32-01/32-03/32-04's precedent: EXEC-02/EXEC-03 span this plan's mint+wire wiring AND 32-06's Linux acceptance test that must actually route a tainted exec output into a sensitive sink arg and observe the deterministic Block. Marking them `Complete` now — before that live composed proof exists — would be factually premature ("Substrate working ≠ v0 done", project CLAUDE.md hard constraint #1). Left for the orchestrator/32-06 to mark complete once the full requirement is genuinely delivered end-to-end.

## Next Phase Readiness

- `mint_from_exec` is genuinely rooted (non-stapled): `provenance_chain == [spawn_event_id]` where `spawn_event_id` is the SAME id `invoke_process_exec` (32-04) already appended as `process_exited` — verified by the anchor-identity unit test.
- `BrokerResponse::PlanNodeDecision.output_value_id` is wired end-to-end: minted in server.rs's process.exec Allowed arm, forwarded to the worker, and the worker now holds the opaque handle (never the raw bytes) ready to route into a subsequent `PlanArg`.
- 32-06's Linux acceptance test can now drive: submit a benign `process.exec` plan node → Allowed → receive `output_value_id` → route that handle into a sensitive sink arg (e.g. `email.send`'s `body`) → assert the executor's UNMODIFIED collect-then-Block loop Blocks it (EXEC-03's live composed proof).
- No blockers. `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (all suites, no regressions), and `./scripts/check-invariants.sh` (4/4 gates) are all green on Mac.

---
*Phase: 32-process-exec-sink-broker-spawned-confined-child*
*Completed: 2026-07-17*

## Self-Check: PASSED

All modified files verified present on disk; all three task commit hashes
(`52af35c`, `f0ff1b0`, `69ab5bb`) verified present in `git log`; `cargo build
--workspace`, `cargo test --workspace --no-fail-fast` (full suite, no
regressions), and `./scripts/check-invariants.sh` (4/4 gates PASS, including
the new Gate-3 `mint_from_exec(` line) all green after the final commit.
