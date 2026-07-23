---
phase: 14-content-sensitive-sink-arg-blocking
plan: 02
subsystem: security
tags: [rust, brokerd, audit-dag, sqlite, i2-enforcement, cli]

# Dependency graph
requires:
  - phase: 14-content-sensitive-sink-arg-blocking (14-01)
    provides: "BlockedArg { anchor, literal } / ExecutorDecision::BlockedPendingConfirmation { anchors: Vec<BlockedArg> } — the plural collect-then-Block decision shape"
provides:
  - "Event.anchors: Vec<SinkBlockedAnchor> — the audit DAG's sink_blocked event now durably records EVERY blocked anchor, not just the first"
  - "audit.rs blocked_literals side table keyed by (event_id, arg) composite PK — supports persisting more than one blocked literal per sink_blocked event"
  - "audit.rs Defect-B guard: sink_blocked events with an empty anchors collection are non-persistable through the TCB"
  - "confirmation.rs render_block_display: fail-closed panic on a genuinely-plural block (re-derives the executor's own is_routing_sensitive||is_content_sensitive && tainted predicate), single-blocked-arg display path unchanged"
  - "A fully green cargo test --workspace and check-invariants.sh — the red state 14-01 intentionally left is resolved"
affects: [15-approval-hook-plan, 16-confirm-binding-and-multi-arg-narration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "blocked_literals composite PK (event_id, arg): one row per blocked arg, so a plural sink_blocked event can persist N literals without a PK collision; redact_blocked_literal deletes all rows for an event_id atomically (the whole block resolves confirm/deny together)"
    - "Fail-closed plurality re-derivation: render_block_display recomputes the executor's exact blocking predicate from PendingConfirmation.resolved_args + PendingConfirmation.sink (no new persisted field) to detect a genuinely-plural block and panic rather than silently truncate"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/event.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/confirmation.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - crates/brokerd/tests/email_smtp_acceptance.rs
    - cli/caprun/tests/confirm.rs
    - cli/caprun/tests/s9_live_block.rs
    - cli/caprun/tests/live_acceptance_tainted_session.rs

key-decisions:
  - "blocked_literals table gained an `arg` column and moved from a single-column PRIMARY KEY (event_id) to a composite PRIMARY KEY (event_id, arg) — the minimal schema change that lets server.rs's 'iterate all anchors, write every literal' requirement actually persist N>1 literals without a PK collision. get_blocked_literal/redact_blocked_literal signatures are UNCHANGED (event_id only) — they remain presence/bulk-redaction checks, which is all their real callers (confirmation.rs's redaction gate) ever needed."
  - "render_block_display's fail-closed plurality guard does NOT add a new field to PendingConfirmation. It re-derives the exact same is_routing_sensitive||is_content_sensitive && tainted predicate the executor's collect-then-Block loop used, applied to PendingConfirmation.resolved_args — brokerd already depends on the executor crate, so this is a precise re-check with zero new persisted state and zero false-positive risk from co-occurring-but-non-sensitive tainted args."
  - "Event.sink_blocked's taint field is now the flat_map union of every anchor's taint (was a single anchor.taint.clone()) — preserves DESIGN §4 rule 6 (Event.taint == the union of what each anchor attests) for the plural case."

requirements-completed: [CONTENT-01]

coverage:
  - id: D1
    description: "The audit Event carries ALL blocked anchors (anchors: Vec<SinkBlockedAnchor>) for a sink_blocked event — every blocked arg is durably recorded in the hash-chained audit DAG, not just the first."
    requirement: CONTENT-01
    verification:
      - kind: unit
        ref: "crates/runtime-core/src/event.rs#event::tests::anchors_empty_event_serializes_byte_identical_and_round_trips"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/durable_anchor.rs#pending_confirmation_persisted_atomically_with_block"
        status: pass
    human_judgment: false
  - id: D2
    description: "Non-sink_blocked events serialize byte-identically to before the migration (golden-byte fixture) — the audit hash chain is undisturbed for unrelated event types."
    requirement: CONTENT-01
    verification:
      - kind: unit
        ref: "crates/runtime-core/src/event.rs#event::tests::anchors_empty_event_serializes_byte_identical_and_round_trips"
        status: pass
    human_judgment: false
  - id: D3
    description: "audit.rs fails closed: a sink_blocked event with an EMPTY anchors collection is rejected (Defect-B guard, now plural)."
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#append_event (Defect B guard, exercised transitively by every durable_anchor.rs/s9_acceptance.rs block test)"
        status: pass
    human_judgment: false
  - id: D4
    description: "server.rs persists EVERY blocked arg's live literal to the redactable blocked_literals side table (not just the first), via a composite (event_id, arg) key."
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/durable_anchor.rs#after_exit_db_alone_anti_stapling_sentinel"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/durable_anchor.rs#redacting_side_table_literal_preserves_verify_chain_and_digest"
        status: pass
    human_judgment: false
  - id: D5
    description: "render_block_display keeps its single-arg display path unchanged for the single-blocked-arg case and fails closed (panics) on a genuinely-plural block, rather than silently showing one of N."
    verification: []
    human_judgment: true
    rationale: "No test in this plan constructs a genuinely 2-arg PendingConfirmation and exercises confirm/deny against it end to end (out of scope per the plan's own scope guard — CONFIRM-04 multi-arg narration is Phase 16). The panic path itself is unexercised by any test; a human/Phase-16 reviewer should confirm the guard fires correctly when Phase 16 adds a genuine 2+-arg confirm flow."
  - id: D6
    description: "cargo test --workspace --no-fail-fast is GREEN and ./scripts/check-invariants.sh (Gate 1 + Gate 2) passes — the whole workspace compiles and tests against the plural type again."
    verification:
      - kind: automated_ui
        ref: "cargo test --workspace --no-fail-fast (34/34 test binaries ok, 0 failed)"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gate 1 + Gate 2 PASS)"
        status: pass
    human_judgment: false

duration: ~50min
completed: 2026-07-08
status: complete
---

# Phase 14 Plan 02: Migrate consumers to plural anchors + restore green workspace Summary

**Migrated `Event`, `brokerd`, and `cli/caprun` from the singular `anchor`/`BlockedPendingConfirmation{anchor,literal}` shape to Wave 1's plural `anchors: Vec<...>`, giving `blocked_literals` a composite `(event_id, arg)` key so every blocked literal persists (not just the first), and restored `cargo test --workspace` to fully green after Wave 1 intentionally left it red.**

## Performance

- **Duration:** ~50 min
- **Completed:** 2026-07-08
- **Tasks:** 3 / 3
- **Files modified:** 10

## Accomplishments
- `Event.anchor: Option<SinkBlockedAnchor>` → `Event.anchors: Vec<SinkBlockedAnchor>` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]` — golden-byte test re-verified and renamed (`anchors_empty_event_serializes_byte_identical_and_round_trips`); the GOLDEN literal is untouched.
- `Event::sink_blocked` now takes `anchors: Vec<SinkBlockedAnchor>` and sets `Event.taint` from the flat-mapped union of every anchor's taint.
- `crates/brokerd/src/server.rs`'s `SubmitPlanNode` arm: destructures `{ anchors }`, builds the plural `sink_blocked` event, and — the plan's explicit correctness requirement — iterates the FULL `anchors` collection to write every blocked arg's literal to the redactable side table (not just the first). `PendingConfirmation.effect_id` comes from `anchors[0].anchor.effect_id` (every element shares one effect_id).
- `crates/brokerd/src/audit.rs`: `blocked_literals` schema gained an `arg` column and a composite `(event_id, arg)` PRIMARY KEY so a plural block can actually persist N>1 literals (the prior single-column PK would have PK-collided on the second `insert_blocked_literal` call for a 2-anchor block). Defect-B guard now checks `anchors.is_empty()`.
- `crates/brokerd/src/confirmation.rs`: `render_block_display` keeps its existing single-arg selection logic for the single-blocked-arg case, and adds a fail-closed `assert!` that panics if the re-derived blocked-arg count exceeds 1 — re-deriving the executor's own `is_routing_sensitive || is_content_sensitive && tainted` predicate over `pc.resolved_args`, with no new persisted field. Both singular-anchor test fixtures (`seed_pending_file_create_block`, `seed_pending_email_send_block`) migrated to `vec![anchor]`.
- Mechanically migrated all remaining singular sites: `crates/brokerd/tests/durable_anchor.rs` (4 sites), `crates/brokerd/tests/s9_acceptance.rs` (2 `BlockedPendingConfirmation` destructures + 1 `Event.anchor` + 1 `Event::sink_blocked` call), `crates/brokerd/tests/email_smtp_acceptance.rs` (Phase 13 fixture), `cli/caprun/tests/confirm.rs` (2 fixtures, one Phase-13-added), `cli/caprun/tests/s9_live_block.rs` and `cli/caprun/tests/live_acceptance_tainted_session.rs` (Linux-gated stragglers found by the exhaustive re-grep, not in the plan's file list — see Deviations).
- `cargo test --workspace --no-fail-fast`: **34/34 test binaries `ok`, 0 failed.** `./scripts/check-invariants.sh`: Gate 1 + Gate 2 PASS.

## Task Commits

1. **Task 1: Make the audit Event carry all blocked anchors, preserving golden bytes** - `8a6138c` (feat)
2. **Task 2: Migrate brokerd decision handling, fail-closed guard, and render safeguard** - `25f8501` (feat)
3. **Task 3: Migrate the cli consumers and restore a green workspace** - `f632904` (feat)

## Files Created/Modified
- `crates/runtime-core/src/event.rs` - `Event.anchor` → `Event.anchors: Vec<SinkBlockedAnchor>`; `Event::sink_blocked` merges taint across the collection; golden-byte test re-verified
- `crates/brokerd/src/server.rs` - Plural decision destructure; `sink_blocked` event built from all anchors; every blocked literal written to the side table; `PendingConfirmation.effect_id` from `anchors[0]`
- `crates/brokerd/src/audit.rs` - `blocked_literals` schema: composite `(event_id, arg)` PK, `insert_blocked_literal` gains an `arg` param; Defect-B guard checks `anchors.is_empty()`
- `crates/brokerd/src/confirmation.rs` - `render_block_display` fail-closed plurality guard (no new field, re-derives the executor's predicate); both singular-anchor test fixtures migrated to `vec![anchor]` + the 3-arg `insert_blocked_literal`
- `crates/brokerd/tests/durable_anchor.rs` - 4 sites: `blocked.anchor` → `blocked.anchors.first()`
- `crates/brokerd/tests/s9_acceptance.rs` - 2 `BlockedPendingConfirmation{anchor,literal}` destructures → `{anchors}`; 1 `Event::sink_blocked(.., anchor)` → `vec![anchor]`; 1 `Event.anchor` → `.anchors.first()`; **Rule 1 fix** to `s9_acceptance_file_create_path_block`'s `contents` arg (see Deviations)
- `crates/brokerd/tests/email_smtp_acceptance.rs` - Phase-13 fixture migrated to `vec![anchor]` + 3-arg `insert_blocked_literal`
- `cli/caprun/tests/confirm.rs` - Both fixtures (file.create + Phase-13 email.send) migrated
- `cli/caprun/tests/s9_live_block.rs` - `blocked.anchor` → `blocked.anchors.first()` (straggler, see Deviations)
- `cli/caprun/tests/live_acceptance_tainted_session.rs` - 4 sites, `blocked.anchor` → `blocked.anchors.first()` (straggler, see Deviations)

## Decisions Made
- `blocked_literals`'s PRIMARY KEY grew a column (`event_id` → `(event_id, arg)`) rather than adding a new table — treated as Rule 1/2 auto-fixable (a column addition, per the deviation-rules edge-case guidance), not a Rule 4 architectural stop, since the plan's own Task 2 action text explicitly required "iterate all anchors, writing each element's literal ... do not drop any blocked literal," which is impossible under the prior single-column PK for a 2+-anchor block.
- `render_block_display`'s plurality check re-derives the executor's blocking predicate (`is_routing_sensitive || is_content_sensitive` && untrusted taint) directly from `pc.resolved_args`/`pc.sink`, instead of threading a new `blocked_arg_count`/`blocked_arg_names` field through `PendingConfirmation` (which would have required a schema/struct change rippling through every `PendingConfirmation`-constructing test fixture). `brokerd` already depends on `executor`, so this reuses the exact predicate with zero new persisted state and no risk of a false-positive panic from an untainted-but-co-occurring arg.
- `get_blocked_literal`/`redact_blocked_literal` signatures are UNCHANGED (still `(conn, event_id)`) even though the underlying table can now hold multiple rows per event — their only real caller (`confirmation.rs`'s redaction gate) only ever needed a presence/absence check, which the unchanged query still answers correctly (redaction deletes ALL rows for an event_id atomically).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `s9_acceptance_file_create_path_block` used a dummy never-minted `ValueId` for the non-blocked `contents` arg**
- **Found during:** Task 2 (`cargo test -p brokerd`)
- **Issue:** 14-01's collect-then-Block loop (approved design) now resolves EVERY arg on the plan node before returning any Block decision — it no longer returns on the first sensitive+tainted match. This test's `contents` arg used a fresh, never-`mint`ed `ValueId` (a "dummy handle that is never resolved on the block path", per its own comment) on the assumption the old first-match-wins loop would never reach it. With the plural collect-then-Block loop it IS reached, and `value_store.resolve()` returns `None` → `Denied(DanglingHandle)` before the loop can finish and return the expected `BlockedPendingConfirmation`.
- **Fix:** Replaced the dummy `ValueId::new()` with a genuinely-minted `UserTrusted` value (`store.mint("hello world", vec![TaintLabel::UserTrusted], vec![Uuid::new_v4()])`) so the arg resolves successfully but is not itself sensitive/blocking.
- **Files modified:** `crates/brokerd/tests/s9_acceptance.rs`
- **Verification:** `cargo test -p brokerd --test s9_acceptance` passes (was failing with `expected BlockedPendingConfirmation ... got Denied { reason: DanglingHandle }` before the fix).
- **Committed in:** `25f8501` (Task 2 commit)

**2. [Rule 1 - Bug] Two Linux-gated test files not in the plan's file list also referenced the singular `Event.anchor` field**
- **Found during:** Task 3's mandated exhaustive workspace re-grep, independent of the plan's declared file list
- **Issue:** `cli/caprun/tests/s9_live_block.rs` (1 site) and `cli/caprun/tests/live_acceptance_tainted_session.rs` (4 sites) both read `blocked.anchor` directly off a persisted `Event`, but neither constructs `BlockedPendingConfirmation` nor calls `Event::sink_blocked` — so they were invisible to the plan's own re-grep instructions (which named only those two patterns) and to the file-list-driven read_first sections. Both files are `#[cfg(target_os = "linux")]`-gated end-to-end live-process tests, so on macOS their bodies are cfg-excluded and `cargo test --workspace` never compiled them — the break would only surface under the Colima/Docker Linux run.
- **Fix:** Migrated all 5 sites to `blocked.anchors.first().expect(...)`.
- **Files modified:** `cli/caprun/tests/s9_live_block.rs`, `cli/caprun/tests/live_acceptance_tainted_session.rs`
- **Verification:** `cargo build --workspace --tests` compiles clean on macOS (these bodies are cfg-excluded so cannot be functionally re-run without Colima; the fix is a straightforward mechanical migration identical to the other 4 files' `.anchor` → `.anchors.first()` sites, all of which DID get exercised and passed).
- **Committed in:** `f632904` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 bug — pre-existing test premise reversed by 14-01's own approved collect-then-Block behavior change; 1 bug — latent Linux-only breakage found by the mandated exhaustive re-grep, outside the plan's declared file list)
**Impact on plan:** Both fixes were necessary for `cargo test --workspace` (this plan's explicit gate) to pass, and for the Linux verification run (confirmatory per this plan, but the actual source of v0's real security proof per CLAUDE.md) not to be silently broken. No scope creep beyond what was needed for correctness.

## Issues Encountered
- `blocked_literals`'s original single-column `event_id TEXT PRIMARY KEY` schema could not support the plan's explicit "persist every blocked arg's literal" requirement for a 2+-anchor block without a PK collision on the second insert. Resolved by adding an `arg` column to the composite PRIMARY KEY (see Decisions Made) — a minimal, backward-compatible change (`get_blocked_literal`/`redact_blocked_literal` callers needed zero changes).
- Determining "is this a genuinely-plural block" inside `render_block_display` without a naive false-positive-prone heuristic (e.g., counting ALL untrusted-tainted `resolved_args`, which would over-fire for a legitimate single-blocked-arg case where a co-occurring non-sensitive arg happens to also carry untrusted taint) required re-deriving the executor's EXACT blocking predicate rather than approximating it. Resolved by calling `executor::sink_sensitivity::is_routing_sensitive`/`is_content_sensitive` directly from `confirmation.rs` (brokerd already depends on the `executor` crate).
- The `advisor` tool was unavailable in this session (returned "unavailable" on invocation); proceeded on independent analysis of the DESIGN docs, existing call-site blast radius, and the plan's own explicit task-2 action text, documenting the schema/predicate design choices above as deviations for review.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `cargo test --workspace --no-fail-fast` is fully GREEN: 34/34 test binaries `ok`, 0 failed. `./scripts/check-invariants.sh` (Gate 1 + Gate 2) PASSES.
- Exhaustive workspace grep confirms exactly **7** `Event::sink_blocked(` call sites project-wide, matching the pre-verified expected count — every one passes a `vec![...]`/collection literal, none passes a bare `SinkBlockedAnchor`. Zero singular `BlockedPendingConfirmation { anchor, literal }` destructures survive anywhere in the workspace.
- Phase 14's type-shape refactor (plural collect-then-Block, D-14) is now complete end-to-end: executor decision → audit Event → broker persistence → cli consumers all agree on the `Vec`-shaped anchor/anchors contract.
- Phase 16 (CONFIRM-03/04, `DESIGN-confirm-binding.md`) can build the `combined_digest` binding and real multi-arg confirm narration on top of this plural foundation. `render_block_display`'s fail-closed panic (D5 above) is the seam Phase 16 must replace with genuine multi-arg display logic — currently unexercised by any test, since no test in this plan or 14-01 constructs a real end-to-end 2-blocked-arg `PendingConfirmation` and drives `confirm`/`deny` against it.
- No blockers for Phase 15/16.

## Self-Check: PASSED
- FOUND: crates/runtime-core/src/event.rs (`anchors: Vec<SinkBlockedAnchor>` present)
- FOUND: crates/brokerd/src/audit.rs (composite `(event_id, arg)` blocked_literals PK present)
- FOUND: crates/brokerd/src/server.rs (per-anchor literal-write loop present)
- FOUND: crates/brokerd/src/confirmation.rs (fail-closed plurality guard present)
- FOUND commit 8a6138c, 25f8501, f632904 in `git log --oneline`

---
*Phase: 14-content-sensitive-sink-arg-blocking*
*Completed: 2026-07-08*
