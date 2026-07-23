---
phase: 20-planner-seam-capability-split
plan: 03
subsystem: security
tags: [brokerd, uds-ipc, decision-oracle, trust-boundary, planner-seam]

# Dependency graph
requires:
  - phase: 20-planner-seam-capability-split
    plan: 02
    provides: "ConnectionRole::{Worker,Planner} capability model, DeclarePlannerRole handshake, and the pre-dispatch capability gate in handle_connection this plan's planner-branch sits alongside"
provides:
  - "BrokerResponse::PlanNodeDecisionReduced { blocked: bool } — the ONLY decision shape a ConnectionRole::Planner connection ever receives for SubmitPlanNode"
  - "evaluate_plan_node_and_record — the shared executor-evaluation-and-durable-recording entry point both the worker's full-decision path and the planner's reduced-decision path call"
  - "planner-role SubmitPlanNode interception in handle_connection, bypassing dispatch_request's PlanNodeDecision arm entirely for that connection"
affects: [21-adversarial-planner, planner-seam-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "shared evaluation-and-record helper, caller-side response projection — the two response shapes (full vs. reduced) are constructed by their respective callers from one identical ExecutorDecision, never by two divergent evaluation paths"
    - "planner-role SubmitPlanNode is intercepted in handle_connection's pre-dispatch loop, never reaching dispatch_request at all — closes the oracle structurally rather than by filtering fields out of an already-full response"

key-files:
  created:
    - crates/brokerd/tests/planner_reduced_signal.rs
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs

key-decisions:
  - "Chose the plan's preferred implementation shape: extract the SubmitPlanNode arm's audit-append/sink-invocation body into evaluate_plan_node_and_record, called directly from handle_connection's planner branch, rather than threading a role parameter through dispatch_request's signature — avoids touching 6 existing dispatch_request direct-call test sites (phase5_dispatch, proto_claims x2, durable_anchor, extract_provenance_threading), all of which pass unmodified."
  - "The planner branch in handle_connection relies on role.permits already having proven request is SubmitPlanNode (Planner's only permitted verb) — an unreachable!() guards the pattern match rather than a duplicate verb check."

requirements-completed: [PLANNER-04]

coverage:
  - id: D1
    description: "A planner-role connection's SubmitPlanNode response carries ONLY a proceed/blocked signal — never anchors, literal_sha256, or a plaintext literal"
    requirement: PLANNER-04
    verification:
      - kind: unit
        ref: "crates/brokerd/tests/planner_reduced_signal.rs#plan_node_decision_reduced_round_trips_with_only_blocked_bool"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/planner_reduced_signal.rs#linux_tests::planner_submit_plan_node_receives_reduced_never_full (Linux, via mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The worker's own connection continues to receive the full PlanNodeDecision { decision } unchanged"
    requirement: PLANNER-04
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/phase5_dispatch.rs (unmodified, git diff empty) — all 6 tests pass"
        status: pass
    human_judgment: false
  - id: D3
    description: "Phase 19/20's regression suites (worker-slot one-way latch, planner capability split) remain unaffected by this plan's refactor"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/two_connection_intent_bypass.rs + planner_capability_split.rs (Linux, via mailpit-verify.sh) — 6/6 pass"
        status: pass
    human_judgment: false

# Metrics
duration: 35min
completed: 2026-07-11
status: complete
---

# Phase 20 Plan 03: Planner-Role Reduced Decision Signal Summary

**Closed the decision-oracle for the planner-role connection: `SubmitPlanNode` on a `ConnectionRole::Planner` connection now returns only `PlanNodeDecisionReduced { blocked: bool }`, structurally incapable of carrying `anchors`, `literal_sha256`, or a plaintext `literal`, by intercepting the planner path in `handle_connection` before it ever reaches `dispatch_request`'s full-decision arm.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-11
- **Tasks:** 2
- **Files modified:** 2 (proto.rs, server.rs) + 1 created (planner_reduced_signal.rs)

## Accomplishments
- `BrokerResponse::PlanNodeDecisionReduced { blocked: bool }` — additive variant, documented as the ONLY decision shape a `ConnectionRole::Planner` connection ever receives for `SubmitPlanNode`.
- Extracted `evaluate_plan_node_and_record` — the entire audit-and-effect side of the former `SubmitPlanNode` dispatch arm (block-time snapshot, `sink_blocked`/`plan_node_evaluated` event append, blocked-literal side-table write, pending-confirmation checkpoint, and any Allowed-decision `file.create`/`email.send` sink invocation) — out of `dispatch_request` into a standalone function that both response paths call identically. This means the reduction is a caller-side *projection* of one shared evaluation, never a second, divergently-implemented evaluation path.
- `handle_connection` now intercepts a `ConnectionRole::Planner` connection's `SubmitPlanNode` immediately after the existing `role.permits` gate (Plan 20-02), calls `evaluate_plan_node_and_record` directly, and sends `PlanNodeDecisionReduced { blocked }` — `Allowed` projects to `false`; `BlockedPendingConfirmation`, `Denied`, and `NotImplemented` all project to `true`. This request never reaches `dispatch_request` at all, so the full-decision arm is structurally unreachable for a planner connection.
- `dispatch_request`'s `SubmitPlanNode` arm (the worker path) now simply calls the same helper and sends the unchanged `PlanNodeDecision { decision }` — byte-identical behavior, zero signature change, so all 6 existing `dispatch_request` direct-call test sites needed no modification.
- New `crates/brokerd/tests/planner_reduced_signal.rs`: a macOS-runnable serde-shape unit test proving `PlanNodeDecisionReduced`'s JSON payload is exactly `{ "blocked": bool }` (no `anchors`/`literal_sha256`/`literal` key), plus a Linux-gated end-to-end integration test proving a declared planner connection's `SubmitPlanNode` response matches `PlanNodeDecisionReduced` and never `PlanNodeDecision`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add the reduced decision response and project it for the planner role** - `1a77341` (feat)
2. **Task 2: Prove the planner never receives anchors, digests, or literals** - `c49a2bd` (test)

**Plan metadata:** (this commit, docs)

## Files Created/Modified
- `crates/brokerd/src/proto.rs` - Added `BrokerResponse::PlanNodeDecisionReduced { blocked: bool }` (additive variant + doc explaining the DESIGN §7 oracle-closing rationale); documented `PlanNodeDecision` as worker-role-only.
- `crates/brokerd/src/server.rs` - Extracted `evaluate_plan_node_and_record` (the ~310-line audit/sink body of the former `SubmitPlanNode` arm) as a standalone async fn; `dispatch_request`'s `SubmitPlanNode` arm now delegates to it; `handle_connection` gained a planner-role interception branch, positioned after the existing `role.permits` gate and before the `dispatch_request` call.
- `crates/brokerd/tests/planner_reduced_signal.rs` - new: serde-shape unit test (macOS) + Linux-gated wire-level integration test.

## Decisions Made
- Followed the plan's stated preference: avoid changing `dispatch_request`'s signature by extracting the shared evaluation logic into a standalone function called from BOTH `dispatch_request`'s worker arm and `handle_connection`'s new planner branch, rather than threading a `role`/`ConnectionRole` parameter through `dispatch_request` itself. This kept all 6 existing `dispatch_request` direct-call test sites (`phase5_dispatch.rs`, `proto_claims.rs` x2, `durable_anchor.rs`, `extract_provenance_threading.rs`) completely untouched — `git diff` on those files is empty.
- The planner branch pattern-matches `request` as `BrokerRequest::SubmitPlanNode` behind an `unreachable!()` else-arm rather than a second explicit verb check, since `role.permits(&request)` (Plan 20-02, checked immediately before) already proves this is the only possible verb for a `ConnectionRole::Planner` connection at that point in the loop.
- `session_status`/`value_store` in the extracted helper keep the exact `&mut` types they had as `dispatch_request` parameters (rather than converting to `&SessionStatus`/`&ValueStore`), even though this arm never mutates either — matches the original code's borrow shape exactly and avoids any explicit reborrow syntax at the two call sites.

## Deviations from Plan

None — plan executed exactly as written, including its own explicitly-stated preferred implementation shape (avoid changing `dispatch_request`'s signature).

## Issues Encountered
None.

## Threat Model Verification

Both threats from the plan's STRIDE register were verified:
- **T-20-04** (Information Disclosure — `SubmitPlanNode` decision returned to a planner-role connection): mitigated. `evaluate_plan_node_and_record` is the single evaluation entry point; the planner branch in `handle_connection` projects its result to `PlanNodeDecisionReduced { blocked }` before it is ever serialized to the wire — no code path exists that could serialize `anchors`/`literal_sha256`/`literal` to a planner connection, proven structurally (the reduced variant has no such fields) and by the serde-shape + Linux integration tests.
- **T-20-09** (future widening of the reduced signal, accepted residual, low severity): recorded as-is in the plan; no code change required this plan. `PlanNodeDecisionReduced`'s doc comment on the variant itself notes it is the ONLY decision shape a planner connection receives, so a future change widening this would need to edit that doc comment, giving a reviewer a concrete tripwire.

## Next Phase Readiness
- The planner-role `SubmitPlanNode` seam now returns a signal a real adversarial planner (Phase 21) can safely observe without gaining any offline literal-guess-confirmer capability.
- Worker-path behavior is byte-identical pre/post this plan — `phase5_dispatch.rs` and all other worker-facing dispatch tests pass unmodified.
- Phase 19's one-way occupancy latches and Phase 20-02's capability split remain fully green on Linux (`two_connection_intent_bypass.rs` + `planner_capability_split.rs`, 6/6 via `mailpit-verify.sh`) — no regression risk carried forward.

---
*Phase: 20-planner-seam-capability-split*
*Completed: 2026-07-11*

## Self-Check: PASSED

- FOUND: crates/brokerd/tests/planner_reduced_signal.rs
- FOUND: crates/brokerd/src/proto.rs (modified)
- FOUND: crates/brokerd/src/server.rs (modified)
- FOUND commit 1a77341 (Task 1)
- FOUND commit c49a2bd (Task 2)
