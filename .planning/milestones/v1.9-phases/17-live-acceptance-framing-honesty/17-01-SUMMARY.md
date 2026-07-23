---
phase: 17-live-acceptance-framing-honesty
plan: 01
subsystem: testing
tags: [rust, brokerd, provenance, taint-proof, refactor]

requires:
  - phase: 15-deterministic-doc-action-extraction
    provides: "assert_unbroken_edge / genuine_derivation_binds / union_provenance_chains — the EXTRACT-02/03 HARD-GATE proof predicates, originally private to crates/brokerd/tests/extract_provenance_threading.rs"
provides:
  - "brokerd::provenance_proof — a new public module re-exporting the three Phase-15 genuine-taint proof predicates for cross-crate reuse"
  - "The Phase-15 HARD-GATE test refactored to consume the promoted module instead of local duplicate defs (single source of truth)"
affects: [17-02, 17-03]

tech-stack:
  added: []
  patterns:
    - "Promote-don't-duplicate: verification predicates that must be re-run identically from a different crate's integration test are moved to a public lib module, never reimplemented, so drift is structurally impossible (COORD-T2)."

key-files:
  created:
    - crates/brokerd/src/provenance_proof.rs
  modified:
    - crates/brokerd/src/lib.rs
    - crates/brokerd/tests/extract_provenance_threading.rs

key-decisions:
  - "Moved (not copied) the three functions verbatim, including doc comments citing findings #2/#10/#12, into crates/brokerd/src/provenance_proof.rs; the test file now imports them rather than defining local copies."
  - "provenance_proof is declared pub (not cfg(test)-gated) since it is read-only, no-mint, no-I/O-beyond-the-caller's-connection — consistent with the already-public verify_chain in the same crate."

requirements-completed: [ACCEPT-01]

coverage:
  - id: D1
    description: "brokerd::provenance_proof exists as a public module exporting assert_unbroken_edge, genuine_derivation_binds, and union_provenance_chains, callable from other crates (e.g. cli/caprun's future live test)"
    requirement: "ACCEPT-01"
    verification:
      - kind: unit
        ref: "cargo build -p brokerd"
        status: pass
    human_judgment: false
  - id: D2
    description: "The Phase-15 HARD-GATE test (extract_provenance_threading.rs) consumes the promoted module with zero behavioral change — no forked/duplicate implementation remains"
    requirement: "ACCEPT-01"
    verification:
      - kind: integration
        ref: "cargo test -p brokerd --test extract_provenance_threading -- --nocapture (4/4 pass: builds_two_anchor_block, extract_02_and_03_positive_proof_both_anchors, extract_02_anti_staple_control_a_fabricated_root_is_rejected, extract_02_anti_staple_control_b_reanchored_staple_is_rejected)"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-09
status: complete
---

# Phase 17 Plan 01: Promote Phase-15 Taint Proof Predicates Summary

**Moved `assert_unbroken_edge`, `genuine_derivation_binds`, and `union_provenance_chains` out of the private brokerd test binary into a new public `brokerd::provenance_proof` module, so Phase 17's live composed test (a different crate) can re-run the exact same HARD-GATE check instead of reimplementing it.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-09T21:47:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Created `crates/brokerd/src/provenance_proof.rs` — a new public module holding the three genuine-taint proof predicates, moved verbatim (including their load-bearing doc comments citing findings #2/#10/#12) from `extract_provenance_threading.rs`.
- Declared `pub mod provenance_proof;` in `crates/brokerd/src/lib.rs`, alongside the existing `pub mod audit;`.
- Refactored the Phase-15 HARD-GATE test to import and consume the promoted functions instead of local duplicate definitions — all four Phase-15 tests still pass unchanged, now exercising the single shared implementation.

## Task Commits

Each task was committed atomically:

1. **Task 1: Extract the three proof predicates into a public brokerd::provenance_proof module** - `fd08ad4` (feat)
2. **Task 2: Refactor the Phase-15 HARD-GATE test to consume the promoted module** - `b79df61` (refactor)

_Note: no plan-metadata commit in this file — worktree mode (isolation="worktree"); the orchestrator commits STATE.md/ROADMAP.md centrally after merge._

## Files Created/Modified
- `crates/brokerd/src/provenance_proof.rs` - New public module: `union_provenance_chains`, `assert_unbroken_edge`, `genuine_derivation_binds` (moved verbatim, doc comments preserved)
- `crates/brokerd/src/lib.rs` - Added `pub mod provenance_proof;` declaration
- `crates/brokerd/tests/extract_provenance_threading.rs` - Deleted the three local duplicate function definitions; imports the promoted versions from `brokerd::provenance_proof` instead; no test body/fixture/assertion changed

## Decisions Made
- Moved (not copied) the three functions verbatim, including doc comments citing findings #2/#10/#12, to guarantee zero drift between the Phase-15 DB-alone proof and Phase 17's forthcoming live proof (COORD-T2).
- Kept the module public and non-`cfg(test)`-gated: these are read-only audit-verification utilities (query + compare, no mint, no I/O beyond the caller's read connection), consistent with brokerd's reference-monitor role and the already-public `verify_chain`.

## Deviations from Plan

None - plan executed exactly as written. Both tasks matched their `<action>` and `<done>` criteria precisely; no Rule 1-4 auto-fixes were needed.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `brokerd::provenance_proof` is now importable from `cli/caprun`'s test crate, unblocking Plan 02/03's live composed test which must re-prove genuine taint descent using these exact predicates (COORD-T2).
- `cargo build --workspace` and `./scripts/check-invariants.sh` both pass after this plan's changes; no regression to the existing HARD-GATE test.

## Self-Check: PASSED

- FOUND: crates/brokerd/src/provenance_proof.rs
- FOUND: fd08ad4 (git log --oneline --all)
- FOUND: b79df61 (git log --oneline --all)
- Verified: `cargo test -p brokerd --test extract_provenance_threading` — 4 passed, 0 failed
- Verified: `grep -n "^fn assert_unbroken_edge\|^fn genuine_derivation_binds\|^fn union_provenance_chains" crates/brokerd/tests/extract_provenance_threading.rs` — no matches (no local duplicate definitions remain)

---
*Phase: 17-live-acceptance-framing-honesty*
*Completed: 2026-07-09*
