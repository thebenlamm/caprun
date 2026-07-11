---
phase: 19-cross-connection-trust-coherence-fix
plan: 01
subsystem: security
tags: [tokio, unix-socket, broker, trust-coherence, rust]

# Dependency graph
requires:
  - phase: 18-trust-boundary-coherence-design-gate
    provides: "DESIGN-session-trust-coherence.md (CLEARED gate) specifying the one-way-latch fix shape and the sequential-reconnect test requirement"
provides:
  - "One-way, session-lifetime occupancy latch in run_broker_server's accept loop"
  - "3 independent fresh-broker regression test variants (guard-a control, overlapping, sequential-reconnect) proving the fix"
affects: [19-02-mailpit-live-verification]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Loop-local bool occupancy latch (no Arc/Mutex/AtomicBool) — accept loop is single-threaded per session"
    - "Per-test-variant fresh run_broker_server instance on a distinct abstract socket, factored via a shared spawn_fresh_broker(variant) helper"

key-files:
  created: []
  modified:
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/two_connection_intent_bypass.rs

key-decisions:
  - "Latch is a plain loop-local bool (session_slot_occupied), set once on first accept, never cleared for the life of run_broker_server — no Drop guard, no release-on-disconnect, no reconnect path, per DESIGN §2's explicit rejection of that unsound alternative."
  - "Rejected 2nd connections still complete the accept and receive one framed BrokerResponse::Error before the stream is dropped, so the caller gets a diagnosable response rather than a bare RST (DESIGN §2)."
  - "Restructured the single two_connection_intent_bypass_repro test into 3 independent #[tokio::test] fns, each via a shared spawn_fresh_broker(variant) helper that opens its own in-memory audit DB, seeds its own sessions row, builds its own workspace + attacker_doc.txt, and spawns run_broker_server on its own distinct socket (caprun-2conn-bypass-{pid}-{variant}) — required because a shared broker instance would let an earlier variant's connection trip the latch for the whole instance (DESIGN-GATE-RECORD-v1.4.md Round 2, finding F2)."
  - "Added a NEW sequential-reconnect variant (conn#1 RequestFd then clean disconnect, THEN conn#2 connects) per DESIGN §1 line 34 — this is exactly the case a release-on-disconnect implementation would fail, and the one-way latch closes it identically to the overlapping case."

requirements-completed: [TRUST-01, TRUST-02]

coverage:
  - id: D1
    description: "One-way occupancy latch added to run_broker_server's accept loop: 2nd connection rejected with a single framed BrokerResponse::Error before any handle_connection/ValueStore/session_status/intent_provided/fd_requested state is constructed for it; latch never cleared for the life of the invocation."
    requirement: "TRUST-01"
    verification:
      - kind: unit
        ref: "cargo build -p brokerd (clean, 0 warnings)"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gate 1/2/3 all PASS)"
        status: pass
      - kind: unit
        ref: "cargo test -p brokerd --lib (95 passed, 0 failed)"
        status: pass
      - kind: other
        ref: "grep gates: session_slot_occupied appears only at init/set/check (no reset); no release/Drop-clearing occurrence found"
        status: pass
    human_judgment: false
  - id: D2
    description: "two_connection_intent_bypass.rs restructured into 3 per-variant fresh-broker tests (guard_a_intra_connection_control, overlapping_connection_bypass_repro, sequential_reconnect_bypass_repro); #[ignore] removed; Part C safe-outcome predicate byte-identical to pre-fix HEAD in both repro variants."
    requirement: "TRUST-02"
    verification:
      - kind: unit
        ref: "cargo build --tests -p brokerd (compiles clean on Mac; module gated #[cfg(target_os = \"linux\")], 0 tests run on macOS by design)"
        status: pass
      - kind: other
        ref: "grep -c '#\\[tokio::test\\]' -> 3 real test fns (a 4th literal match was a doc-comment string, since rephrased to remove the false positive)"
        status: pass
      - kind: other
        ref: "grep -c 'ignore' two_connection_intent_bypass.rs -> 0"
        status: pass
      - kind: other
        ref: "diff of the panic!/assert! predicate block in both overlapping_connection_bypass_repro and sequential_reconnect_bypass_repro against `git show HEAD:...` lines 310-323 -> byte-identical (no diff output)"
        status: pass
    human_judgment: true
    rationale: "Actual RED->GREEN observation of these 3 Linux-only tests happens on real Linux in Plan 02 via scripts/mailpit-verify.sh, per this plan's own DONE-gate language. This plan's own scope is Mac-side compile/grep gates only; a human/Plan-02 must confirm the Linux run is green before the fix is considered fully proven."

# Metrics
duration: 5min
completed: 2026-07-11
status: complete
---

# Phase 19 Plan 01: Cross-Connection Trust Coherence Fix Summary

**One-way occupancy latch closes the cross-connection ProvideIntent bypass in run_broker_server's accept loop; the regression test now has 3 independent fresh-broker variants including a new sequential-reconnect repro, `#[ignore]` removed.**

## Performance

- **Duration:** ~5 min (commit timestamps 23:37:09 -> 23:41:47)
- **Started:** 2026-07-11T03:37:09Z
- **Completed:** 2026-07-11T03:41:47Z
- **Tasks:** 2 completed
- **Files modified:** 2

## Accomplishments
- Added a one-way, session-lifetime occupancy latch (`session_slot_occupied: bool`, `crates/brokerd/src/server.rs` line 111) to `run_broker_server`'s accept loop, set exactly once on first accept and never cleared — a 2nd connection to the same session socket now receives a single framed `BrokerResponse::Error` and is dropped before any `handle_connection` task, `ValueStore`, or `session_status`/`intent_provided`/`fd_requested` state is ever constructed for it.
- Confirmed no shared multi-connection state, `Arc`/`Mutex`/`AtomicBool`, or `Drop` guard was introduced — matches DESIGN §2's ruling that a loop-local `bool` is sufficient because the accept loop is single-threaded per session.
- Restructured `crates/brokerd/tests/two_connection_intent_bypass.rs` from one `#[ignore]`d 3-part test into 3 independent `#[tokio::test]` functions (`guard_a_intra_connection_control`, `overlapping_connection_bypass_repro`, `sequential_reconnect_bypass_repro`), each spawning its OWN fresh `run_broker_server` instance on its own distinct socket via a shared `spawn_fresh_broker(variant)` helper, and removed the top-level `#[ignore]`.
- Added the NEW `sequential_reconnect_bypass_repro` variant (conn#1 `RequestFd` then clean disconnect, then conn#2 connects and attempts `ProvideIntent`) required by DESIGN §1 line 34 — this is exactly the case a release-on-disconnect implementation would have failed.
- Verified the safe-outcome `panic!`/`assert!` predicate block is byte-identical to pre-fix HEAD in both `overlapping_connection_bypass_repro` and `sequential_reconnect_bypass_repro` (diffed directly, no output).

## Task Commits

1. **Task 1: Add a one-way occupancy latch to run_broker_server's accept loop (TRUST-01)** - `4843dfc` (feat)
2. **Task 2: Restructure two_connection_intent_bypass.rs into 3 fresh-broker variants and un-ignore (TRUST-02)** - `9467e2a` (test)

**Plan metadata:** (this commit, appended after self-check)

## Files Created/Modified
- `crates/brokerd/src/server.rs` - one-way occupancy latch (`session_slot_occupied`) added to `run_broker_server`'s accept loop, before `tokio::spawn`, per-connection locals untouched
- `crates/brokerd/tests/two_connection_intent_bypass.rs` - restructured into 3 per-variant fresh-broker `#[tokio::test]` fns via a shared `spawn_fresh_broker` helper; `#[ignore]` removed; new sequential-reconnect variant added

## Decisions Made
- Latch implementation: plain loop-local `bool` (`session_slot_occupied`), never `Arc`/`Mutex`/`AtomicBool` — matches DESIGN §2's explicit "no cross-task sharing needed" ruling since the accept loop is single-threaded per session.
- Rejected connections still complete the accept and get one framed `BrokerResponse::Error` naming the reason, then the stream is dropped via `continue` — no audit event appended for the rejection (DESIGN §2: not a correctness requirement).
- Test restructuring used a shared `spawn_fresh_broker(variant)` helper (per the plan's own action text instruction to factor shared setup) rather than duplicating the audit-DB/session-seed/workspace/spawn boilerplate inline in each of the 3 test fns.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Doc-comment literal string collided with the `#[tokio::test]` grep-count acceptance gate**
- **Found during:** Task 2 verification
- **Issue:** A code comment written as `// #[tokio::test] runs never collide...` contained the literal substring `#[tokio::test]`, so `grep -c '#\[tokio::test\]'` returned 4 instead of the expected 3 (3 real attributes + 1 doc-comment false positive).
- **Fix:** Reworded the comment to `// async test runs never collide...` (no functional change), so the grep count now reflects only the 3 real test attributes.
- **Files modified:** crates/brokerd/tests/two_connection_intent_bypass.rs
- **Verification:** `grep -c '#\[tokio::test\]'` now returns exactly 3.
- **Committed in:** 9467e2a (Task 2 commit)

**Note on the `run_broker_server` grep-count acceptance criterion:** the plan's acceptance criteria state `grep -c 'run_broker_server' ... returns 3 (one per variant)`, but the plan's own action text explicitly instructs factoring the shared setup (including the `run_broker_server` spawn) into one reusable helper. Following that explicit instruction (which also avoids ~90 lines of triplicated setup boilerplate) means the literal call site appears once, in `spawn_fresh_broker`, called by all 3 variants — not 3 separate inline call sites. The substantive property the gate exists to verify — each variant gets its OWN fresh broker instance on its OWN distinct socket, latch starting unset — is preserved and independently confirmed: 3 distinct socket-name strings (`caprun-2conn-bypass-{pid}-{variant}` for `guard-a`/`overlapping`/`sequential`), 3 independent `#[tokio::test]` fns, each calling `spawn_fresh_broker` exactly once. This is a plan-authoring ambiguity (mechanical grep heuristic vs. the plan's own explicit code-quality instruction), not a functional gap; documented here per Rule 3 (resolving a blocking inconsistency without weakening the substantive safety property being tested).

---

**Total deviations:** 1 auto-fixed (1 blocking/grep-gate wording fix) + 1 documented plan-authoring ambiguity resolution (factored helper vs. literal grep-count heuristic, substance preserved).
**Impact on plan:** No scope creep, no weakening of any safe-outcome assertion. Both deviations are test-file wording/structure only; `crates/brokerd/src/server.rs`'s fix logic is exactly as specified in Task 1's action text.

## Issues Encountered
None beyond the two items documented above under Deviations.

## User Setup Required
None - no external service configuration required. Actual Linux-side GREEN observation of the 3 tests (via `scripts/mailpit-verify.sh`) is explicitly deferred to Plan 02, per this plan's own verification section.

## Next Phase Readiness
- Plan 02 can now run `scripts/mailpit-verify.sh` on real Linux to observe all 3 `two_connection_intent_bypass` variants go GREEN, proving the fix end-to-end.
- If Plan 02's Linux run comes back red on any variant, gap-closure returns to `crates/brokerd/src/server.rs` and/or `crates/brokerd/tests/two_connection_intent_bypass.rs` per this plan's own verification note.
- No blockers identified for Plan 02.

---
*Phase: 19-cross-connection-trust-coherence-fix*
*Completed: 2026-07-11*

## Self-Check: PASSED

- FOUND: crates/brokerd/src/server.rs
- FOUND: crates/brokerd/tests/two_connection_intent_bypass.rs
- FOUND: .planning/phases/19-cross-connection-trust-coherence-fix/19-01-SUMMARY.md
- FOUND commit: 4843dfc (Task 1)
- FOUND commit: 9467e2a (Task 2)
- FOUND commit: 95e8103 (SUMMARY.md)
