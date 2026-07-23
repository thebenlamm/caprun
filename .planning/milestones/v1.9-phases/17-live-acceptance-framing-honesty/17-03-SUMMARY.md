---
phase: 17-live-acceptance-framing-honesty
plan: 03
subsystem: testing
tags: [rust, cli, live-acceptance, taint-proof, provenance, anti-staple]

requires:
  - phase: 17-live-acceptance-framing-honesty
    plan: "17-01"
    provides: "brokerd::provenance_proof — assert_unbroken_edge / genuine_derivation_binds / union_provenance_chains, promoted for cross-crate reuse"
  - phase: 17-live-acceptance-framing-honesty
    plan: "17-02"
    provides: "cli/caprun/tests/live_acceptance_v1_3.rs — the composed 3-session live scenario, with the confirm/deny hostile sessions' effect_id/session_id already in scope at the TEETH-2-INSERTION-POINT sentinel"
provides:
  - "The milestone's HARD GATE (tooth #2): live_acceptance_v1_3_composed now re-proves genuine taint descent for BOTH hostile sessions' `to`+`body` anchors, using the promoted brokerd::provenance_proof predicates — not a reimplementation"
  - "Both anti-staple controls (A: fabricated root, B: naive re-anchored staple) proven to reject against THIS run's live anchors"
affects: []

tech-stack:
  added: []
  patterns:
    - "Self-consistency derivation-record reconstruction: a live test recomputes a derived anchor's expected provenance roots by scanning the session's OWN derivation events (NO-LIMIT SELECT, matching by derived_value_id) rather than pinning against an out-of-band ground truth — distinct from Phase-15's DB-alone test, which has independent mint-return roots available."
    - "Anti-staple Control B always chains onto the session's live current_chain_head, never a mid-chain node (e.g. sink_blocked) — avoids the Phase-16 MAJOR-7 DAG-fork bug class when the mutation happens after confirm/deny has already advanced the chain."

key-files:
  created: []
  modified:
    - cli/caprun/tests/live_acceptance_v1_3.rs
    - cli/caprun/Cargo.toml
    - Cargo.lock

key-decisions:
  - "Added `executor` as a cli/caprun dev-dependency (path-based workspace crate, already a normal dependency of brokerd) so Control B's in-process mint_from_read call could construct a ValueStore — not a new external registry package, so out of scope for the package-install checkpoint restriction."
  - "Updated the file's module-level doc comment (which described tooth #2 as a future TEETH-2-INSERTION-POINT sentinel) to reflect that Plan 17-03 has now appended it, so a bare grep for the sentinel token does not falsely match a stale prose reference."

requirements-completed: [ACCEPT-01]

coverage:
  - id: D1
    description: "live_acceptance_v1_3_composed re-proves genuine taint descent (assert_unbroken_edge + genuine_derivation_binds) for BOTH the `to` and `body` anchors of BOTH hostile sessions (confirm leg, deny leg) produced by this composed live run, plus EXTRACT-03 taint survival on the derived `to`"
    requirement: "ACCEPT-01"
    verification:
      - kind: integration
        ref: "MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3 live_acceptance_v1_3_composed -- --nocapture' bash scripts/mailpit-verify.sh (Colima+Docker, Linux) — 1 passed, 0 failed"
        status: pass
    human_judgment: false
  - id: D2
    description: "Anti-staple control A (fabricated root) and control B (naive re-anchored staple, minted onto the live current_chain_head) are both rejected on the correct predicate, and verify_chain remains true after control B's append"
    requirement: "ACCEPT-01"
    verification:
      - kind: integration
        ref: "same live_acceptance_v1_3_composed run above; the test panics on any assertion failure, so '1 passed; 0 failed' proves every control assertion held"
        status: pass
    human_judgment: false

duration: 35min
completed: 2026-07-09
status: complete
---

# Phase 17 Plan 03: Tooth-#2 Genuine-Taint Re-Proof (HARD GATE) Summary

**Appended the milestone's HARD GATE to `live_acceptance_v1_3_composed`: both hostile sessions' `to`/`body` anchors from THIS composed live run are re-proven as an unbroken, genuinely-derived taint edge via the promoted `brokerd::provenance_proof` predicates, with both anti-staple controls (fabricated root, naive re-anchored staple chained onto the live chain head) rejected — verified live on Linux via `scripts/mailpit-verify.sh`.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-09T22:09:00Z
- **Tasks:** 2
- **Files modified:** 3 (`cli/caprun/tests/live_acceptance_v1_3.rs`, `cli/caprun/Cargo.toml`, `Cargo.lock`)

## Accomplishments

- Replaced the `// TEETH-2-INSERTION-POINT` sentinel in `live_acceptance_v1_3_composed` with the tooth-#2 assertion block, importing `brokerd::provenance_proof::{assert_unbroken_edge, genuine_derivation_binds, union_provenance_chains}` — the SAME functions Phase 15's DB-alone HARD-GATE test consumes, never reimplemented.
- For BOTH hostile sessions (confirm leg, deny leg): loaded the session's `sink_blocked` event, asserted exactly two distinctly-named anchors (`to`, `body`), reconstructed the `to` anchor's expected roots from its own `derivation` event's payload (a self-consistency reconstruction), and proved the per-anchor unbroken edge for both anchors, the payload-bound `genuine_derivation_binds` predicate for the derived `to`, and EXTRACT-03 taint survival.
- Anti-staple control A (a fabricated `Uuid::new_v4()` root) rejected with "does not resolve", for both sessions.
- Anti-staple control B (a naive `mint_from_read` re-anchor of the already-assembled recipient literal, using the `email_address` claim shape) minted onto the confirm-leg session's LIVE `current_chain_head` (not the mid-chain `sink_blocked` node — avoiding the Phase-16 MAJOR-7 fork-bug class), rejected specifically on the payload-binding predicate while a genuine derivation event provably exists in the session, followed by an explicit post-append `assert!(verify_chain(...))` that remained true.
- Added `executor` as a `cli/caprun` dev-dependency (path-based workspace crate) so control B's in-process mint could construct a `ValueStore`.
- Ran the live verification via `scripts/mailpit-verify.sh` (Colima+Docker, Linux): **1 passed, 0 failed** — every new assertion (including both hostile sessions' full tooth-#2 walk and control B's post-append `verify_chain`) held.

## Task Commits

Each task was committed atomically:

1. **Task 1: Append the tooth-#2 genuine-taint re-proof to the composed live test** - `2d6ef25` (feat)
2. **Task 2: Record the "one unbroken DAG" interpretation sentence in the plan SUMMARY** - this file (no separate code commit; SUMMARY.md is the deliverable, committed alongside this plan's metadata per worktree-mode convention)

_Note: no plan-metadata commit in this file's own history — worktree mode (isolation="worktree"); the orchestrator commits STATE.md/ROADMAP.md centrally after merge. This SUMMARY.md itself is committed by this plan's own worktree-mode commit (see below)._

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_v1_3.rs` - Tooth-#2 assertion block (both hostile sessions' per-anchor unbroken-edge proof, payload-bound genuine-derivation, EXTRACT-03 survival, both anti-staple controls); module doc comment updated to no longer describe tooth #2 as a future sentinel.
- `cli/caprun/Cargo.toml` - Added `executor` as a dev-dependency (path-based workspace crate).
- `Cargo.lock` - Regenerated dependency edge for the new `executor` dev-dependency.

## Decisions Made

- **`executor` added as a dev-dependency, not a new external package.** Control B's naive re-anchor mint needs `executor::value_store::ValueStore` in scope; `brokerd` already depends on `executor` as a normal (non-dev) dependency, so this is a path-based workspace-internal addition, not a registry install — out of scope for the package-legitimacy checkpoint restriction.
- **Module doc comment updated.** The file's header previously described tooth #2 as future work "appended by Plan 03 (17-03) at the `TEETH-2-INSERTION-POINT` sentinel." Left verbatim, a bare `grep -n "TEETH-2-INSERTION-POINT"` would still match that prose reference even after the actual sentinel comment in the function body was removed. Updated the prose to describe tooth #2 as already delivered by this plan, so the grep for the sentinel token now returns zero matches file-wide (matching the plan's `<done>` criterion literally, not just in spirit).
- **Control B applied to the confirm-leg session only** (per the plan's explicit permission — "running it on both is acceptable but not required, the control proves the predicate's teeth, session-independent"). Running the mutating control against a second session would add no additional proof value.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `executor` as a cli/caprun dev-dependency**
- **Found during:** Task 1
- **Issue:** `brokerd::quarantine::mint_from_read`'s signature takes `&mut executor::value_store::ValueStore`, but `cli/caprun`'s `Cargo.toml` did not depend on the `executor` crate directly (only transitively through `brokerd`), so the type was unnameable from the test file.
- **Fix:** Added `executor = { path = "../../crates/executor" }` under `[dev-dependencies]` — a path-based workspace-internal crate already depended on directly by `brokerd`, not a new external package.
- **Files modified:** `cli/caprun/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo build --workspace --tests` — 0 errors, 0 warnings; `./scripts/check-invariants.sh` — all 3 gates PASS.
- **Committed in:** `2d6ef25` (Task 1 commit)

**2. [Rule 1 - Bug/Precision] Updated the stale "future sentinel" doc-comment reference**
- **Found during:** Task 1 self-check
- **Issue:** After removing the actual `// TEETH-2-INSERTION-POINT` sentinel comment from the function body, a prose reference to the same token remained in the file's module-level doc comment, which would still satisfy a naive `grep` for the literal token even though the real insertion point was already resolved.
- **Fix:** Rewrote the doc-comment paragraph to describe tooth #2 as delivered by this plan rather than pending.
- **Files modified:** `cli/caprun/tests/live_acceptance_v1_3.rs`
- **Verification:** `grep -n "TEETH-2-INSERTION-POINT" cli/caprun/tests/live_acceptance_v1_3.rs` returns no matches.
- **Committed in:** `2d6ef25` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking dependency addition, 1 precision fix to a stale doc comment)
**Impact on plan:** Both auto-fixes necessary for the code to compile and for the plan's own `<done>` criterion to hold literally. No scope creep — no design change, no new production code path, no additional test assertions beyond what the plan specified.

## Issues Encountered

None beyond the two auto-fixes above. The live Mailpit-scoped run passed on the first attempt after the code compiled: both hostile sessions' `to`/`body` anchors passed the per-anchor unbroken-edge proof and the payload-bound genuine-derivation predicate, EXTRACT-03 taint survival held, anti-staple control A was rejected on "does not resolve" for both sessions, and anti-staple control B was rejected on the payload-binding predicate (with a derivation event provably present) while `verify_chain` remained true after its append — chained onto the confirm-leg session's live `current_chain_head`, never the mid-chain `sink_blocked` node.

## User Setup Required

None beyond the already-standing Colima + Docker + `scripts/mailpit-verify.sh` recipe (unchanged from Phase 13/16) — no new environment requirement, no new external Cargo dependency (only an internal workspace path dependency).

## Interpretation Clarifications (COORD-SUMMARY, COORD-INTERP)

**"One unbroken audit DAG" interpretation.** In ACCEPT-01, "one unbroken audit DAG" means **per-session `verify_chain` integrity across a SHARED audit.db log** — three sessions persisted in one shared SQLite file, each independently chain-verified true, plus a re-proven genuine-taint descent for the hostile sessions' anchors (this plan's tooth #2). It explicitly does **NOT** mean a single cross-session `parent_id`-linked chain spanning the confirm/deny/clean sessions — a cross-session linked chain would contradict `DESIGN-session-trust-state.md`'s pinned one-invocation-one-session model, and would additionally be structurally impossible here since confirm and deny are mutually exclusive terminal states on one `PendingConfirmation` (17-RESEARCH.md Pitfall 2), requiring a minimum of 3 separate sessions rather than one continuous chain.

**Self-consistency scope of the live anchor pin.** The live run's `to`/`body` anchor pin in this plan is a **self-consistency check**: the expected provenance roots for the derived `to` anchor are reconstructed from the SAME derivation event record whose `derived_value_id` is being checked against that anchor's own `value_id` — it is **NOT an independently-sourced ground-truth pin**. The independent ground-truth root pin (via out-of-band mint-return values `reply_to_read_id`/`domain_read_id`, captured by the test harness BEFORE the DB-alone reconstruction) exists only in Phase-15's still-green DB-alone test (`crates/brokerd/tests/extract_provenance_threading.rs`) — that remains the sole source of truth for that specific independence property. This nuance does NOT weaken the substantive anti-staple teeth exercised here: the per-element real-`file_read` check, the payload-bound `genuine_derivation_binds` predicate, and both anti-staple controls (A: fabricated root, B: naive re-anchored staple) all hold independently of this self-consistency nuance, and all passed live against real hostile-session data in this run. The proof has real teeth; it must simply not be oversold as an independently-sourced ground-truth pin.

## Next Phase Readiness

- `live_acceptance_v1_3_composed` now proves all five assertion teeth (#1 per-session verify_chain, #2 genuine-taint re-proof with anti-staple controls, #3 clean-control delivery, #4 confirm-sends-once/deny-sends-nothing, #5 exactly 3 sessions) live on real Linux.
- This is the last code wave of the final phase (17). No further code plans depend on this one; Plan 04 (documentation) carries the PROJECT.md-facing version of the COORD-SUMMARY interpretation sentence above.
- Per the coordinator gate's explicit instruction: this SUMMARY marks ONLY this plan's own line — it does not flip ROADMAP.md's phase-level completion checkbox (that remains the orchestrator's responsibility after all wave agents merge).

## Self-Check: PASSED

- FOUND: `cli/caprun/tests/live_acceptance_v1_3.rs`
- FOUND: `2d6ef25` (git log --oneline --all)
- Verified: `cargo build --workspace --tests` — 0 errors, 0 warnings
- Verified: `./scripts/check-invariants.sh` — all 3 gates PASS
- Verified: `grep -n "TEETH-2-INSERTION-POINT" cli/caprun/tests/live_acceptance_v1_3.rs` — no matches
- Verified: `grep -n "accounts@ev1l.com" cli/caprun/tests/live_acceptance_v1_3.rs` — no matches
- Verified: `grep -n '```' cli/caprun/tests/live_acceptance_v1_3.rs` — no matches (no fenced code blocks)
- Verified (live, Linux, Colima+Docker): `MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3 live_acceptance_v1_3_composed -- --nocapture' bash scripts/mailpit-verify.sh` — exit code captured via `$?` before any pipe (redirected to a log file) — `1 passed; 0 failed`; 3/3 `Chain verification: PASSED` lines present (from the 3 caprun subprocess invocations' own internal print, not the test's assertions); script's own `Mailpit-backed Linux verification suite PASSED.` line present.

---
*Phase: 17-live-acceptance-framing-honesty*
*Completed: 2026-07-09*
