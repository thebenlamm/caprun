---
phase: 38-github-pr-sink
plan: 05
subsystem: api
tags: [github, confirm-release, i2-taint, audit-dag, p33-p34, brokerd]

# Dependency graph
requires:
  - phase: 38-01
    provides: github.pr executor sensitivity + CommitIrreversible effect-class rows
  - phase: 38-02
    provides: session github.pr auth-grant (has_github_grant/record_github_grant) + duplicate-PR CAS (github_pr_content_key/reserve_created_pr)
  - phase: 38-03
    provides: prepare_github_pr precheck + invoke_github_pr_from_resolved sink dispatch
provides:
  - github.pr confirm-release path in confirmation.rs (Step-4.75 guard admits github.pr; Step-4.8b grant-gate + prepare precheck + content-key derivation pre-burn; Step-7 CAS-then-dispatch arm)
  - regression proof that no confirm_granted dangles without a terminal github_pr_* event (P33/P34 audit-gap class closed for github.pr)
affects: [39-git-push-sink, github, confirm-release, git-github-adapters]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "confirm-release TWO-gate pre-burn discipline: grant gate (GITHUB-02) + prepare_* precheck run BEFORE Step 5 confirm_granted / Step 6 burn (fail-closed-RECOVERABLE, row stays Pending)"
    - "content-derived idempotency key derived pre-burn (read-only) so the post-burn Step-7 CAS carries no fallible arg-lookup leg"
    - "every post-burn leg (fresh invoke / replay suppress / CAS-error) folds into a terminal github_pr_* event — terminal EVENT closes the burned one-shot (§9)"
    - "shared cfg(test) pub(crate) env lock across sink + confirmation test modules to serialize process-wide CAPRUN_GITHUB_TOKEN (mirrors SMTP_ENV_LOCK)"

key-files:
  created: []
  modified:
    - crates/brokerd/src/confirmation.rs
    - crates/brokerd/src/sinks/github_pr.rs

key-decisions:
  - "Derive the github_pr_content_key in the pre-burn Step-4.8b region (pure/read-only) rather than in Step 7 — keeps the post-burn CAS free of any fallible arg-lookup that could dangle a burned confirmation."
  - "On a CAS replay (not-fresh) append a distinct github_pr_replay_suppressed terminal marker (mirrors email_send_replay_suppressed) rather than a lie of github_pr_succeeded; on a CAS error itself append github_pr_failed — both keep the no-dangling invariant."
  - "Promote github_pr's env lock to a cfg(test) pub(crate) module-level static so confirmation.rs's confirm tests share ONE lock with the sink tests (a separate lock would not actually serialize the process-wide env)."

patterns-established:
  - "Pattern: confirm-releasable sink wiring = (1) add to Step-4.75 allow-list, (2) pre-burn gates+precheck+key-derivation, (3) Step-7 CAS-then-dispatch with every leg folding to a terminal event."

requirements-completed: [GITHUB-02, GITHUB-03]

coverage:
  - id: D1
    description: "A tainted title/body github.pr Blocks (BlockedPendingConfirmation) and render_block_display shows the literal verbatim, marked [BLOCKED] (GITHUB-03 — human sees exactly what would leave the boundary)"
    requirement: "GITHUB-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#github_pr_tainted_title_blocks_and_shows_verbatim"
        status: pass
    human_judgment: false
  - id: D2
    description: "confirm() WITHOUT a live session auth-grant fails closed — the row stays Pending, no confirm_granted, no PR attempt (GITHUB-02: a bare confirm cannot create a PR)"
    requirement: "GITHUB-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#github_pr_confirm_without_grant_does_not_burn"
        status: pass
    human_judgment: false
  - id: D3
    description: "confirm() WITH a live grant + valid precheck proceeds to Step-7 dispatch; confirm_granted is followed by a terminal github_pr_* event, the created_prs CAS row is reserved, verify_chain true (github.pr admitted at the Step-4.75 guard; §9 no-dangling)"
    requirement: "GITHUB-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#github_pr_confirm_with_grant_proceeds_no_dangling"
        status: pass
    human_judgment: false
  - id: D4
    description: "P33/P34 regression — a malformed (empty) frozen title fails at the pre-burn prepare_github_pr precheck (even WITH a grant): row stays Pending, NO confirm_granted, no CAS row, no terminal event (the recurring confirm-release audit-gap class closed)"
    requirement: "GITHUB-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#github_pr_confirm_malformed_precheck_does_not_burn"
        status: pass
    human_judgment: false

# Metrics
duration: ~20min
completed: 2026-07-18
status: complete
---

# Phase 38 Plan 05: github.pr Confirm-Release Path Summary

**Wired the github.pr confirm-release path in confirmation.rs — Step-4.75 guard admits github.pr, a two-gate pre-burn region (live auth-grant + prepare_github_pr precheck + content-key derivation) runs BEFORE confirm_granted/the one-shot burn, and a Step-7 CAS-then-dispatch arm folds every post-burn leg into a terminal github_pr_* event, closing the recurring P33/P34 audit-gap class for github.pr.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-07-18T09:14:00Z
- **Completed:** 2026-07-18T09:33:55Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Extended the Step-4.75 entry-guard allow-list to admit `github.pr` (a confirm-releasable sink absent from the list is denied at the guard — the DESIGN §9 required extension).
- Added Step-4.8b pre-burn gates for github.pr: `has_github_grant` grant gate (GITHUB-02, the SECOND independent gate — a bare confirm cannot create a PR), the `prepare_github_pr` precheck (same fn the dispatch uses, so they cannot drift), and pre-burn derivation of the content-idempotency key — ALL before Step 5 `confirm_granted` and Step 6 burn (fail-closed-RECOVERABLE: row stays Pending).
- Added the Step-7 `github.pr` dispatch arm: `reserve_created_pr` CAS before the POST; FRESH invokes `invoke_github_pr_from_resolved` (opaque terminal event first), REPLAY appends `github_pr_replay_suppressed`, CAS-error folds `github_pr_failed` — no dangling confirm_granted (§9). No mint token added (Gate 3 byte-identical).
- Added all four regression/block tests, including the plan-checker-mandated 4th (`github_pr_confirm_malformed_precheck_does_not_burn`) that mirrors the process.exec no-burn discipline.

## Task Commits

1. **Task 1: Step-4.75 guard + prepare precheck + Step-7 github.pr dispatch** - `f843129` (feat)
2. **Task 2: tainted-block, no-grant-Deny, grant-proceeds, no-dangling-confirm tests** - `43ea19c` (test)

## Files Created/Modified
- `crates/brokerd/src/confirmation.rs` - Step-4.75 allow-list extension; Step-4.8b github.pr grant-gate + precheck + content-key derivation; Step-7 CAS-then-dispatch arm; four github.pr confirm-release tests + seed helper.
- `crates/brokerd/src/sinks/github_pr.rs` - promoted `GITHUB_ENV_LOCK` to a `#[cfg(test)] pub(crate)` module-level static so the confirmation tests share one env lock with the sink tests.

## Verification

- `cargo build --workspace` — clean, **0 warnings**.
- `cargo test -p brokerd` — **224 passed / 0 failed / 0 warnings** across all targets (lib + integration + doc). Includes the `s38_github_pr.rs` integration suite.
- The four new tests all pass:
  - `github_pr_tainted_title_blocks_and_shows_verbatim` — PASS (tainted title AND body verbatim + [BLOCKED]).
  - `github_pr_confirm_without_grant_does_not_burn` — PASS (bare confirm fails closed, no burn, no PR attempt).
  - `github_pr_confirm_with_grant_proceeds_no_dangling` — PASS (confirm_granted followed by terminal github_pr_failed on the macOS POST stub, CAS row reserved, verify_chain true).
  - **`github_pr_confirm_malformed_precheck_does_not_burn` — PASS** (the 4th, plan-checker-mandated test: empty frozen title fails at the pre-burn precheck even WITH a grant; row Pending, NO confirm_granted, no CAS row, no terminal event — the P33/P34 audit-gap class).
- `./scripts/check-invariants.sh` — **exits 0** (Gate 1 no EffectRequest; Gate 3 mint-site allow-list byte-identical — no mint token in confirmation.rs).

## Decisions Made
- Derived the content-idempotency key pre-burn (read-only) so the post-burn Step-7 CAS carries no fallible arg-lookup leg that could dangle a burned confirmation.
- Chose a distinct `github_pr_replay_suppressed` terminal marker for the CAS-replay branch (mirroring `email_send_replay_suppressed`) rather than a false `github_pr_succeeded`; a CAS error itself appends `github_pr_failed`. Both preserve the no-dangling invariant.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Shared env lock across test modules**
- **Found during:** Task 2 (grant-proceeds test)
- **Issue:** The confirm-with-grant test sets `CAPRUN_GITHUB_TOKEN` process-wide. github_pr.rs's tests also mutate `CAPRUN_GITHUB_*` under a test-local lock; a separate lock in confirmation.rs would NOT serialize the two, risking a flaky cross-module env race in the multi-threaded brokerd test binary.
- **Fix:** Promoted github_pr's `GITHUB_ENV_LOCK` to a `#[cfg(test)] pub(crate)` module-level static (exactly mirroring `email_smtp::SMTP_ENV_LOCK`) and referenced it from both test modules.
- **Files modified:** `crates/brokerd/src/sinks/github_pr.rs` (not in the plan's `files_modified`; zero overlap with the parallel 38-04, which touches server.rs, and 38-03 which owns github_pr.rs is already complete).
- **Verification:** `cargo build --workspace` clean (0 warnings after cfg(test)-gating the static); full brokerd suite green.
- **Committed in:** `43ea19c` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** Necessary for correct, non-flaky test serialization. No scope creep — the only extra file is the sink's own module whose test lock it already owned.

## Issues Encountered
- Initial build emitted a `static GITHUB_ENV_LOCK is never used` warning (a `pub(crate)` static referenced only from cfg(test) code). Resolved by `#[cfg(test)]`-gating it, matching `SMTP_ENV_LOCK`. Zero warnings after.

## User Setup Required
None - no external service configuration required (a live github.pr POST requires `CAPRUN_GITHUB_TOKEN` + a grant at runtime; the macOS test path stubs the POST).

## Next Phase Readiness
- github.pr is fully confirm-releasable: tainted title/body Blocks and shows verbatim; a PR is created on confirm only with a live grant; every burned confirmation is followed by a terminal github_pr_* event.
- Phase 39 (git.push) can mirror this exact confirm-release wiring pattern (`prepare_git_push` + entry-guard extension + terminal-event-before-state).

## Self-Check: PASSED
- Commits `f843129`, `43ea19c` present in git log.
- `.planning/phases/38-github-pr-sink/38-05-SUMMARY.md` exists.
- github.pr guard + Step-7 arm wired in confirmation.rs.

---
*Phase: 38-github-pr-sink*
*Completed: 2026-07-18*
