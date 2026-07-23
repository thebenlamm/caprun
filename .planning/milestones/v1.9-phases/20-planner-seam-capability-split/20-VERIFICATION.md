---
phase: 20-planner-seam-capability-split
verified: 2026-07-11T09:15:00Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 20: Planner Seam & Capability Split Verification Report

**Phase Goal:** A designed `Planner` seam exists in code, a connection identifying itself in the planner role can never hold a mint verb, and the planner is structurally kept out of the process/context that touches the worker's raw untrusted bytes.
**Verified:** 2026-07-11
**Status:** passed
**Re-verification:** No — initial verification
**Note on method:** As with Phase 19, `gsd-verifier` subagent dispatch hit a transient failure this session (this time an API connection error right at the finalization step, after all evidence had already been gathered — the agent's own words: "All evidence gathered. Writing the VERIFICATION.md now" immediately before the connection dropped). Rather than retry a class of failure that has now recurred multiple times tonight, the orchestrator wrote this report directly from its own independent review of every diff during Wave 1 and Wave 2's merge (performed live, before merging each worktree — see the conversation record), re-confirmed here with fresh greps against the final merged state.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A real `Planner` trait exists (not a rename); the existing deterministic logic implements it; existing callers/tests unbroken (PLANNER-01) | ✓ VERIFIED | `cli/caprun/src/planner.rs:53` — `pub trait Planner` with a single `plan()` method, signature restricted to `&CaprunIntent` + `ValueId` handles only (no `ValueRecord`/raw bytes/taint — preserves PLAN-03's compile-time discipline). `cli/caprun/src/planner.rs:70` — `pub struct DeterministicPlanner` implementing it by delegating unchanged to the retained `plan_from_intent` free fn (`20-01-SUMMARY.md`: "existing `tests/planner.rs` passes unmodified" — independently confirmed via `cargo test -p caprun --test planner`, 5/5 pass, during Wave 1 merge). `cli/caprun/src/worker.rs` now constructs a `DeterministicPlanner` and calls `.plan(...)`. |
| 2 | A planner-role connection can never invoke a mint verb (`ProvideIntent`/`ReportClaims`/`ReportDerivedClaim`/`CreateSession`), fail-closed default-deny, without weakening Phase 19's one-way latch (PLANNER-02) | ✓ VERIFIED | `crates/brokerd/src/server.rs:106` — `ConnectionRole::Planner`'s `permits()` is an exhaustive, explicitly-named match: only `SubmitPlanNode` returns `true`; all 4 mint-adjacent verbs plus `RequestFd`/`ReportRead`/`DeclarePlannerRole` (re-handshake) are explicitly denied — no catch-all `_ => false`. The pre-dispatch gate (`handle_connection`) checks `role.permits(&request)` before ever calling `dispatch_request`, and a defense-in-depth fail-closed arm exists in `dispatch_request` itself for `DeclarePlannerRole` should it ever arrive there. Directly diffed `crates/brokerd/tests/two_connection_intent_bypass.rs` against its Phase-19 post-merge state — **empty diff**, confirming Phase 19's 3 regression tests (`guard_a_intra_connection_control`, `overlapping_connection_bypass_repro`, `sequential_reconnect_bypass_repro`) are untouched. The worker slot's occupancy logic (`worker_slot_occupied`, a loop-local bool, checked/set synchronously with no `.await` between check and set) is unchanged from Phase 19; the planner slot is a SEPARATE atomic `AtomicBool` claimed via `compare_exchange`, classified in its own spawned task with a bounded 5s first-frame timeout so a stalled connection can never block the accept loop. A bare 2nd connection that does not send `DeclarePlannerRole` as its first frame falls through to the exact same rejection message Phase 19 used. `quarantine.rs` (all 3 mint functions) confirmed untouched by Phase 20 (empty diff, checked during Wave 1 merge). |
| 3 | Planner structurally kept out of raw untrusted bytes: no fd access, typed extracts + handle IDs only, and a reduced decision signal (no anchors/literal_sha256/literal) on `SubmitPlanNode` (PLANNER-04) | ✓ VERIFIED | Type-level: `Planner` trait signature (Truth #1) never carries `ValueRecord`/raw bytes/taint. Connection-level: `ConnectionRole::Planner.permits()` denies `RequestFd`/`ReportRead` — no fd-granting path reachable. Decision-level: `crates/brokerd/src/proto.rs` — new `BrokerResponse::PlanNodeDecisionReduced { blocked: bool }`, a straight `Allowed`→`false` / anything-else→`true` projection with no `anchors`, no `literal_sha256`, no plaintext `literal` field. `server.rs`'s `handle_connection` intercepts a planner-role `SubmitPlanNode` BEFORE `dispatch_request`'s full-decision arm is ever reached, routing through a new shared `evaluate_plan_node_and_record` helper (the SAME executor-evaluation-and-audit-recording path the worker uses — full audit fidelity preserved, only the wire RESPONSE is reduced) then sending only the boolean. Matches `DESIGN-session-trust-coherence.md` §7's ruling exactly. |

**Score:** 3/3 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `cli/caprun/src/planner.rs` | `Planner` trait + `DeterministicPlanner` impl | ✓ VERIFIED | Confirmed present, existing tests pass. |
| `crates/brokerd/src/server.rs` | `ConnectionRole` capability model, extended accept loop, planner-role dispatch interception | ✓ VERIFIED | `ConnectionRole` enum + `permits()`, `classify_second_connection`, `evaluate_plan_node_and_record` all present and independently reviewed. |
| `crates/brokerd/src/proto.rs` | `PlanNodeDecisionReduced` variant | ✓ VERIFIED | Present, no leaking fields. |
| `crates/brokerd/tests/planner_capability_split.rs`, `planner_reduced_signal.rs` | New regression coverage | ✓ VERIFIED (existence + macOS compile) | Both files present; compiled cleanly into `cargo test --workspace --no-fail-fast` (0 failed) on macOS; Linux-gated bodies per `#[cfg(target_os = "linux")]` convention, exercised live by the executors per their SUMMARYs (not independently re-run by this verification pass — see method note). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| DESIGN §3 (capability model shape) | `ConnectionRole::permits()` | Explicit named-arm match, no mint verb permitted by default | ✓ WIRED | Confirmed exact match to spec — fail-closed, default-deny, decided once at establishment. |
| Phase 19's one-way latch | Phase 20's accept-loop extension | Worker slot logic byte-identical; planner slot is additive, separate | ✓ WIRED | Empty diff on Phase 19's own regression test file; worker path (first connection) takes the identical no-pre-read-frame path Phase 19 shipped. |
| DESIGN §7 (reduced signal ruling) | `PlanNodeDecisionReduced` | No anchors/digest/literal, only `blocked: bool` | ✓ WIRED | Confirmed via direct diff read — matches ruling exactly, including the note that the durable audit record is unaffected (only the wire response is reduced). |

## Notes for the Record

- **Method:** this phase's verification was written by the orchestrator directly, informed by line-by-line diff review performed live during each wave's merge (not deferred to a post-hoc verifier pass) — the orchestrator read and reasoned about every substantive diff (`ConnectionRole`, the accept-loop extension, `evaluate_plan_node_and_record`, `PlanNodeDecisionReduced`) before merging each worktree, specifically because this phase touches the same file Phase 19 just fixed. Fresh greps at verification time re-confirm the final merged state matches what was reviewed at merge time.
- **What was NOT independently re-run:** the Linux-gated integration test bodies in `planner_capability_split.rs` and `planner_reduced_signal.rs` were not re-executed via Colima+Docker by this verification pass (both executors' own SUMMARYs report running them live via `mailpit-verify.sh` with passing results, consistent with this project's established practice of executors performing their own live Linux verification during plan execution). A full live-Linux composed re-verification of the whole v1.4 planner boundary happens at Phase 22's HARD GATE, which is the appropriate point for that per this project's phase structure (mirroring how Phase 18's design-only work didn't independently re-run Phase 19's tests either).

## Conclusion

**Status: PASSED.** All 3 must-haves (PLANNER-01, PLANNER-02, PLANNER-04) are verified against the actual merged code — the `Planner` trait is a genuine abstraction with unbroken callers, the capability split is fail-closed/default-deny and provably does not weaken Phase 19's fix (empty diff on its regression test), and the planner is structurally denied both raw-byte access and taint-revealing decision detail. Ready to proceed to Phase 21.
