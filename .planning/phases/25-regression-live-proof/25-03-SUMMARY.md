---
phase: 25-regression-live-proof
plan: 03
subsystem: testing
tags: [live-linux-verification, milestone-close, mailpit, slot-type-binding, t2-08]

requires:
  - phase: 25-regression-live-proof
    provides: Plan 01 held-out T2-06 test (non-cfg-gated s9_acceptance.rs) + Plan 02 regression audit (0 NEEDS-FIX, Mac workspace green)
provides:
  - Independent green real-Linux full-workspace regression via the bare default mailpit-verify.sh recipe
  - Proof the Plan-01 held-out T2-06 swapped-handle deny test ran (not skipped) on real Linux under the confined-worker suite
  - v1.5 DONE milestone-close evidence record (true pre-pipe exit code, verbatim sentinel, named counts, held-out-test presence)
affects: []

tech-stack:
  added: []
  patterns:
    - "Capture $? on the line immediately after the invocation, BEFORE any pipe/tee/grep — assert on the sentinel + named counts + held-out-test presence, never on exit-0-through-a-pipe."
    - "Bare default recipe only (no MAILPIT_VERIFY_CMD override): the unscoped full-workspace build+test rebuilds all sibling binaries and cannot be masked by a scoped run."

key-files:
  created:
    - .planning/phases/25-regression-live-proof/25-03-SUMMARY.md
  modified: []

key-decisions:
  - "Gate run directly by the orchestrator (not relayed through a subagent) — T2-08's entire purpose is an independent, non-laundered re-run with the true exit code captured before any pipe; a subagent's 'it passed' claim is the indirection this gate is designed to distrust. Mirrors the v1.3 coordinator-gate precedent."
  - "No code changed — this plan runs and verifies only."
---

# 25-03 SUMMARY — v1.5 DONE gate: live Linux re-verification (T2-08)

## Objective
Independently re-run the full workspace regression on real Linux via the bare default
`scripts/mailpit-verify.sh` recipe, prove it green (0 failures), confirm the Plan-01 held-out
T2-06 test ran on real Linux, and take human sign-off on the v1.5 milestone close.

## Environment precheck
- `colima status` → `colima is running` ✓
- `docker info` → reachable ✓

## How the gate was run (integrity discipline)
```
bash scripts/mailpit-verify.sh > mailpit-verify-25.log 2>&1
RESULT=$?        # captured on the VERY NEXT line, BEFORE any pipe/tee/grep
```
- **Bare default recipe** — NO `MAILPIT_VERIFY_CMD` override. The default is
  `cargo build --workspace && cargo test --workspace --no-fail-fast` (the leading build
  guarantees the `caprun-planner` nice-named binary exists — v1.4 milestone-closure finding).
- Ran unscoped over the full workspace inside the unprivileged `rust:1` container on the
  Colima-hosted `caprun-mailpit-net` with a live `axllent/mailpit` SMTP sidecar.

## Evidence (all four assertions PASS)

| # | Assertion | Result |
|---|-----------|--------|
| 1 | True exit code (captured pre-pipe) | `RESULT=0` ✓ |
| 2 | Terminal sentinel line, verbatim | `Mailpit-backed Linux verification suite PASSED.` (log line 1050) ✓ |
| 3 | Zero `test result: FAILED`; zero `error:` compile-abort; ≥1 `test result: ok` | 0 FAILED, 0 compile-abort, **46** `test result: ok` blocks ✓ |
| 4 | Held-out T2-06 test ran on real Linux (not skipped) | `test slot_type_binding_swapped_subject_recipient_denies ... ok` (log line 650) ✓ |

Additional corroboration:
- Held-out Allowed control also ran green on Linux: `test slot_type_binding_correctly_routed_allows ... ok` (line 649).
- Real-Linux tally: **309 passed / 0 failed** across 46 suites. This exceeds the Mac workspace's
  269 passed (Plan 02) by ~40 — precisely the Linux-only Landlock + seccomp + no_new_privs + e2e
  confined-worker tests that report 0-passed on Mac by design. Their non-zero pass count here
  confirms the kernel-enforced security suite actually executed on real Linux.

## What this proves
- Phase 24's shipped Step 1c slot-type binding enforcement holds under the real confined-worker
  suite on real Linux (T2-06 real-Linux leg), and its held-out proof was not silently skipped.
- Phase 24's body/doc_fragment role deviation does not break the live email.send flow.
- No fixture regression, no build-artifact placement bug (the v1.4 caprun-planner precedent) —
  the unscoped rebuild would have surfaced either.

## Self-Check: PASSED
- Bare recipe, no override: ✓
- Exit code captured before any pipe: ✓ (`RESULT=0`)
- Sentinel + 0 FAILED + held-out-test-present asserted on named artifacts, not on a piped exit: ✓
- No code changed: ✓ (`files_modified: []`)

## Human sign-off
Task 2 is a **blocking** milestone-close checkpoint — awaiting explicit human "approved" before
the phase is marked complete and v1.5 is closed. Not self-approved (autonomous:false; the v1.5
DONE gate is human-only regardless of auto_advance).
