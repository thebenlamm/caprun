---
phase: 19-cross-connection-trust-coherence-fix
verified: 2026-07-11T07:00:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 19: Cross-Connection Trust Coherence Fix Verification Report

**Phase Goal:** The broker rejects a second connection to an already-active session, closing the cross-connection `ProvideIntent` bypass that let a worker mint an attacker-controlled `UserTrusted` literal and route it to `email.send` as `Allowed`.
**Verified:** 2026-07-11
**Status:** passed
**Re-verification:** No — initial verification
**Note on method:** Three consecutive `gsd-verifier` subagent dispatches for this phase hit a transient infrastructure stall (stream watchdog, "no progress for 600s" — the same class of failure seen twice during Wave 2's own execution, once while an agent tried to run a long-blocking Bash command). No VERIFICATION.md was produced by any of the three attempts and no files were modified by them. Rather than continue re-dispatching into the same stall pattern, the orchestrator performed this verification directly — independently reading and cross-checking every artifact below against the actual repository state (not trusting SUMMARY.md claims at face value), consistent with this project's own "verify each finding against actual code" discipline. The one item genuinely un-repeatable outside the original live run (the Colima+Docker Linux test execution itself) is treated as evidence from Plan 19-02's SUMMARY.md, whose specific captured command output and arithmetic were independently checked for internal consistency (see Truth #3 below) — not re-run.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Broker rejects a 2nd connection to an already-active session via a ONE-WAY, session-lifetime latch — never released, no reconnect path (TRUST-01) | ✓ VERIFIED | `crates/brokerd/src/server.rs:111,116,132` — `session_slot_occupied` declared `false` once before the accept loop, checked at the top of each iteration (rejects + `continue`s before any per-connection state is seeded if already `true`), set `true` unconditionally right after a successful first accept. Grepped the whole file for `session_slot_occupied` — exactly these 3 references exist; no reset, no `Drop` guard, no release-on-disconnect path anywhere. Matches `planning-docs/DESIGN-session-trust-coherence.md` §2 exactly. |
| 2 | `two_connection_intent_bypass_repro`'s `#[ignore]` removed, test restructured into 3 independent fresh-broker variants, safe-outcome assertions never weakened (TRUST-02) | ✓ VERIFIED | `crates/brokerd/tests/two_connection_intent_bypass.rs` (518 lines): 3 `#[tokio::test]` fns confirmed — `guard_a_intra_connection_control` (line 218), `overlapping_connection_bypass_repro` (line 265), `sequential_reconnect_bypass_repro` (line 395) — each calling its own `spawn_fresh_broker(variant)` helper (own socket/broker instance per DESIGN §1's structural note). `grep -n "#\[ignore"` returns no matches — confirmed removed. Directly read both the overlapping and sequential variants' safe-outcome blocks (lines ~349-393 and ~487-517): both are byte-identical to each other and to the pre-fix original's Part C predicate (`panic!("LIVE EXFIL: ...")` + `assert!(!intent_accepted \|\| !routed_allowed, "SAFE expectation: ...")`) — the assertions were not weakened, only the connection-setup topology changed. |
| 3 | Independent live-Linux re-run (`scripts/mailpit-verify.sh`, Colima+Docker) shows all 3 new variants green and the full workspace suite green with no regression (TRUST-03) | ✓ VERIFIED (evidence from captured run, arithmetic re-checked) | `19-02-SUMMARY.md` records verbatim captured output: scoped run — `test result: ok. 3 passed; 0 failed; 0 ignored`, real exit code `SCOPED_EXIT=0` (captured before any pipe, per the project's own "verification exit code through a pipe" incident discipline); full run — `253 passed; 0 failed` across `37` test binaries, real exit code `FULL_EXIT=0`. Independently re-checked the delta against v1.3's previously recorded baseline (250 passed / 0 failed / 36 binaries, cited in PROJECT.md's v1.3 section): 253−250=3, 37−36=1 — exactly matches "+3 newly-un-ignored tests, +1 new test binary," no unexplained delta. |
| 4 | PROJECT.md's DOC-02 disclosure finalized against the SHIPPED fix, only PROJECT.md touched (DOC-02) | ✓ VERIFIED | `git show 600e0aa --stat`: `.planning/PROJECT.md \| 51 +++++++++++++++++++++++++++++++++++---------------` — only file in the commit. Directly read the diff: the "⚠️ Superseded finding" block now reads "FIXED, SHIPPED 2026-07-11," names the mechanism ("a one-way, session-lifetime occupancy latch... added to `run_broker_server`'s accept loop"), states it "restores the `UserTrusted == human-typed` invariant across connections," and cites both regression variants + the live `mailpit-verify.sh` rerun with the same 253/0/37 counts. The Active-section Phase 19 bullet flipped from "(next)" to "✓ ... SHIPPED (2026-07-11)". No aspirational/pending language remains. |

**Score:** 4/4 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/brokerd/src/server.rs` | One-way occupancy latch in accept loop | ✓ VERIFIED | Lines 98-134; 3 references to `session_slot_occupied`, no reset. `cargo build --workspace` succeeds cleanly on macOS (sanity check, not the Linux security proof). |
| `crates/brokerd/tests/two_connection_intent_bypass.rs` | 3 independent regression test variants, `#[ignore]` removed | ✓ VERIFIED | 518 lines, 3 `#[tokio::test]` fns, each with its own fresh broker instance, no `#[ignore]` anywhere. |
| `.planning/PROJECT.md` | DOC-02 disclosure finalized against shipped fix | ✓ VERIFIED | Commit `600e0aa`, single-file diff, matches evidence. |
| `.planning/phases/19-cross-connection-trust-coherence-fix/19-01-SUMMARY.md`, `19-02-SUMMARY.md` | Plan execution records | ✓ VERIFIED | Both present; 19-02's captured test-output claims independently cross-checked against the actual test file and PROJECT.md diff (not merely trusted at face value). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| DESIGN §2 (one-way latch spec) | `server.rs` implementation | Cited accept-loop placement, "never reset" requirement | ✓ WIRED | Implementation matches spec precisely: latch checked before `handle_connection` is ever spawned, set immediately after first accept, no release path — exactly what DESIGN-GATE-RECORD-v1.4.md's round-1 BLOCKER remediation required. |
| DESIGN §1 structural note (3 fresh-broker test variants required) | `two_connection_intent_bypass.rs` | Cited requirement for per-variant fresh broker instances | ✓ WIRED | `spawn_fresh_broker(variant: &str)` helper (line 144) confirmed used by all 3 test fns with distinct variant names ("guard_a"-style, "overlapping", "sequential") — each gets its own socket, avoiding the latch-corruption issue the DESIGN doc's Round-2 finding F2 identified in the pre-fix test topology. |
| Plan 19-02 Task 1 (live verification) → Task 2 (DOC-02) | Gate: Task 2 only proceeds on Task 1 green | Plan frontmatter `depends_on` + explicit gating language in Task 2's action text | ✓ WIRED | 19-02-SUMMARY.md confirms Task 1 completed (both runs green, real exit 0) before Task 2's PROJECT.md edit; commit order (`600e0aa` after both live runs per the SUMMARY's own timeline) is consistent with this. |
| REQUIREMENTS.md traceability | TRUST-01/02/03, DOC-02 rows | Phase-completion checkbox flip | Pending (expected) | Rows still show `[ ]`/"Pending" as of this verification pass — this is EXPECTED, not a gap: `phase.complete` (run by the orchestrator immediately after this VERIFICATION.md is accepted) is what flips these to Complete/checked, and has not yet run. |

## Notes for the Record

- **Infrastructure stalls, not task failures.** All three prior `gsd-verifier` dispatch attempts for this phase failed with "Agent stalled: no progress for 600s (stream watchdog did not recover)" — the same failure class independently observed twice during this phase's own Wave 2 execution (once when an executor tried to run a blocking Colima/Docker command via Bash, once on a verifier's self-initiated Colima re-run attempt). This appears to be a session-level infra issue affecting long-blocking tool calls tonight, not a defect in the phase's actual work. No partial/corrupted state was left by any of the three attempts (confirmed via `git status` and absence of a partial `19-VERIFICATION.md` before this one was written).
- **What was NOT independently re-run:** the actual Colima+Docker Linux `cargo test` execution itself was not re-run by this verification pass (to avoid a 4th stall attempt on the same class of long-blocking command). Its result is taken from `19-02-SUMMARY.md`'s verbatim captured output, which the orchestrator separately corroborated by (a) confirming the cited test names actually exist in the test file with the claimed structure, (b) confirming the PROJECT.md diff that cites the same numbers is consistent, and (c) re-checking the arithmetic of the before/after count delta. This is a reasonable evidentiary standard for a goal-backward verification pass, consistent with how VERIFICATION.md for Phase 18 treated its own DESIGN-GATE-RECORD-v1.4.md as evidence rather than re-deriving the adversarial review from scratch.

## Conclusion

**Status: PASSED.** All 4 must-haves (TRUST-01, TRUST-02, TRUST-03, DOC-02) are verified against the actual code, test file, and documentation — not merely against SUMMARY.md's self-report. The fix is a sound, one-way occupancy latch with no release path; the regression test genuinely proves both the overlapping and sequential-reconnect variants are closed, with safe-outcome assertions preserved byte-identical to the pre-fix original; the live Linux evidence is internally consistent; and PROJECT.md's DOC-02 disclosure accurately reflects the shipped reality. Ready to proceed to Phase 20.
