---
phase: 45-thin-cli-sdk-read-only-audit-dag-viewer
plan: 02
subsystem: security
tags: [output-encoding, control-char-neutralization, anti-drift, brokerd, confirmation, ansi-injection]

# Dependency graph
requires:
  - phase: 44-git-push-egress
    provides: "confirmation.rs git.push confirm-prompt neutralization (T-44-19) — the private neutralize_control_chars being extracted"
provides:
  - "brokerd::display::neutralize_control_chars — one shared cross-crate pub fn escaping every char::is_control() byte to visible \\xNN/\\u{NNNN}"
  - "anti-drift regression test binding the confirmation display path and the shared fn to the ONE implementation (T-45-05)"
affects: [45-03-viewer, VIEW-01, cli-caprun-audit-dag-viewer]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Shared pub fn + anti-drift test (mirrors Phase-42 F1 containment-helper extraction) — one implementation, two callers, a test that fails if a divergent copy is reintroduced."

key-files:
  created:
    - crates/brokerd/src/display.rs
  modified:
    - crates/brokerd/src/lib.rs
    - crates/brokerd/src/confirmation.rs

key-decisions:
  - "Extracted the neutralizer into a new brokerd::display module rather than runtime-core (runtime-core is pure-types; display is a brokerd concern reachable by the cli/caprun viewer via brokerd's public API)."
  - "Behavior held byte-identical: the git.push-scoped guard in render_block_display is UNCHANGED — the viewer widens neutralization universally in 45-03, not here."

patterns-established:
  - "Anti-drift shared-helper pattern: a security-relevant helper used by 2+ callers lives as ONE pub fn, guarded by a test asserting each caller resolves to it."

requirements-completed: [U1]

coverage:
  - id: D1
    description: "Shared brokerd::display::neutralize_control_chars pub fn escapes every control byte (C0/ESC/CR/LF/TAB, DEL, C1) to a visible escape and preserves printable + non-ASCII UTF-8."
    requirement: "U1"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/display.rs#neutralize_escapes_esc_preserves_surrounding_printables, neutralize_escapes_all_c0_and_del, neutralize_escapes_c1_range, neutralize_preserves_printable_and_utf8, neutralize_empty_and_control_free_unchanged"
        status: pass
    human_judgment: false
  - id: D2
    description: "confirmation.rs git.push confirm-prompt neutralization rewired to the shared fn with byte-identical behavior; existing confirmation tests still green."
    requirement: "U1"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#git_push_render_neutralizes_control_chars (+ full confirmation:: suite, 45 tests)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Anti-drift test binds the confirmation git.push display path and the shared fn to the ONE implementation (T-45-05) — a divergent copy that escaped weaker would fail it."
    requirement: "U1"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#git_push_display_and_shared_neutralizer_do_not_drift"
        status: pass
    human_judgment: false

# Metrics
duration: 12min
completed: 2026-07-18
status: complete
---

# Phase 45 Plan 02: Shared control-char neutralizer + anti-drift test Summary

**Extracted the private `neutralize_control_chars` into a single cross-crate `brokerd::display::neutralize_control_chars` pub fn, rewired confirmation.rs's git.push confirm-prompt call sites to it with byte-identical behavior, and added an anti-drift test binding both callers to the ONE implementation (WG-2 / U1 M3).**

## Performance

- **Duration:** ~12 min
- **Tasks:** 2 (both TDD)
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments
- New `brokerd::display` module owning the ONE shared `pub fn neutralize_control_chars` (exact logic + doc-comment rationale carried over from confirmation.rs), reachable cross-crate for the 45-03 viewer.
- confirmation.rs's private copy deleted; all three call sites (`render_block_display` git.push branch, `render_git_push_payload_summary` per-arg literal + frozen_new_oid) rewired to `crate::display::neutralize_control_chars` — git.push-scoped behavior UNCHANGED (T-10-04 verbatim display for every other sink preserved).
- Anti-drift regression test (`git_push_display_and_shared_neutralizer_do_not_drift`) asserting the display path and the shared fn agree byte-for-byte on a `\x1b[2K` literal — mirrors the Phase-42 F1 shared-helper pattern; closes T-45-05 by construction.

## Task Commits

1. **Task 1: Extract neutralize_control_chars into shared brokerd::display pub fn** — `ac5e818` (feat)
2. **Task 2: Rewire confirmation.rs to the shared fn + anti-drift test** — `0ece8aa` (refactor)

_TDD note: the extraction is a mechanical move of proven logic; each task's module shipped its behavior tests + implementation together, run green before commit._

## Files Created/Modified
- `crates/brokerd/src/display.rs` (created) — shared `pub fn neutralize_control_chars(&str) -> String` + 5 unit tests.
- `crates/brokerd/src/lib.rs` (modified) — `pub mod display;` export.
- `crates/brokerd/src/confirmation.rs` (modified) — private fn removed, 3 call sites rewired, anti-drift test added.

## Decisions Made
- Placed the shared fn in a new `brokerd::display` module (not runtime-core, which is pure-types and I/O-free by invariant). The cli/caprun viewer reaches it via brokerd's public API.
- Held the `if pc.sink.0 == "git.push"` guard constant — universal neutralization is 45-03's job, not an extraction-time behavior change (T-45-06 mitigation).

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None. (First `cargo test` filter `display::neutralize` matched 0 tests because the fully-qualified path is `display::tests::neutralize_*`; corrected the filter to `display::` — no code impact.)

## Verification
- `cargo build --workspace` — clean.
- `cargo test -p brokerd` full suite — **272 lib tests + all integration binaries pass, 0 failed** (incl. `display::` 5/5, `confirmation::` 45/45, anti-drift test).
- `bash scripts/check-invariants.sh` — **All invariant gates PASSED** (Gate 1 no new EffectRequest, Gate 3 mint sites unchanged, Gate 5 no aws-lc-rs, Gate 6 containment anti-drift). No new crate.

## Pre-existing Working-Tree Note
`.planning/STATE.md` and `.planning/REQUIREMENTS.md` were milestone-setup mods owned by the orchestrator — left untouched (not staged/committed/reverted).

## Next Phase Readiness
- 45-03 (read-only audit-DAG viewer, VIEW-01) can now call `brokerd::display::neutralize_control_chars` on every displayed literal, backed by the anti-drift guarantee that the confirm-prompt and viewer share one proven implementation.

---
*Phase: 45-thin-cli-sdk-read-only-audit-dag-viewer*
*Completed: 2026-07-18*

## Self-Check: PASSED
- FOUND: crates/brokerd/src/display.rs
- FOUND: commit ac5e818
- FOUND: commit 0ece8aa
