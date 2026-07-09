---
phase: 16-confirm-ux-literal-binding-negative-controls
plan: 02
subsystem: security
tags: [rust, sha256, sqlite, audit-dag, confirm-binding, cli]

# Dependency graph
requires:
  - phase: 16-confirm-ux-literal-binding-negative-controls (Plan 16-01)
    provides: "combined_digest(&[(&str, &str)]) shared primitive; Event.combined_digest/blocked_arg_names additive fields; PendingConfirmation.combined_digest/blocked_arg_names fields, computed once at Block time over the FULL resolved_args set"
provides:
  - "render_block_display rewritten as an ALL-args narration loop (blocked AND trusted), each marked [BLOCKED]/[trusted], verbatim/untruncated, in byte-wise-ascending arg_name order (the same canonical order combined_digest binds), plus the Draft/untrusted-seeded posture statement"
  - "caprun review <effect_id> — a read-only pre-decision surface (MAJOR-8): prints the same narration confirm/deny would show, WITHOUT transitioning state, appending any event, or invoking any sink"
  - "audit::current_chain_head(conn, session_id) — the session's leaf event (id, hash) on a linear chain"
  - "confirm()'s chain-verify (audit::verify_chain FIRST, fail closed on a broken chain) + FULL-set recompute-and-compare integrity gate (BLOCKER-2) before any send"
  - "ConfirmOutcome::DigestMismatch — an integrity alarm (row left Pending, never auto-denied), with a durable confirm_digest_mismatch event parented on current_chain_head (never blocked_event_id — MAJOR-7 no-fork)"
  - "confirm_granted/confirm_denied re-parented onto current_chain_head instead of blocked_event_id directly (behavior-preserving in the single-shot case)"
affects: ["16-03 (concurrent, disjoint files — s9_live_block.rs)", "16-04", "Phase 17 (real end-to-end doc-derived confirm/deny path)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Gate order: verify_chain FIRST (tamper-evidence of the read path), THEN recompute-and-compare over the frozen snapshot (integrity of the compared value) — never the reverse, since a compare against an unverified read proves nothing"
    - "Chain-head threading (not mid-chain-id parenting): compute current_chain_head ONCE per confirm()/deny() invocation and thread it forward, mirroring quarantine.rs's mint_from_read chain_head_id/chain_head_hash discipline — prevents DAG forks on any multi-append-per-block sequence"
    - "Fail-closed-uniformly: a missing sink_blocked Event, a missing combined_digest field, and a genuine digest mismatch are all treated as the SAME DigestMismatch outcome — no separate error path that could accidentally propagate as an Err instead of a fail-closed outcome"

key-files:
  created: []
  modified:
    - crates/brokerd/src/confirmation.rs
    - crates/brokerd/src/audit.rs
    - cli/caprun/src/main.rs

key-decisions:
  - "T-14-08 two-commit discipline followed literally: COMMIT 1 (1f3336b) adds a genuine 2-blocked-arg email.send fixture and a #[should_panic] regression test proving the CURRENT assert!(blocked_count <= 1) guard fires; COMMIT 2 (b61e043) removes that guard and replaces it with full ALL-args narration, updating the SAME test to assert the new narrated output instead of the panic."
  - "SOURCE-LABEL: dropped the Round-2 kind-aware Source-label instruction (unimplementable without threading conn/find_event_by_id across three call sites); instead simply removed the misleading hardcoded 'file_read' literal, rendering the source as the bare provenance-root event id. Kind-aware labeling is deferred (not shipped half-done)."
  - "DigestMismatch leaves the row Pending (retriable), never auto-transitions to Denied — an attacker who can trigger a mismatch (e.g. tampering a trusted arg) must not thereby gain the power to force-terminate a confirmation a human might legitimately retry. This is a documented design decision, not an oversight."
  - "verify_chain's scope is recorded HONESTLY (MAJOR-6): it detects single-store and non-recomputing multi-store tampering by recomputing hashes from the SAME SQLite store; it is NOT authenticated or externally-anchored. Nothing pins the chain head, so an actor with events-table write access could forge the chain end-to-end. This is recorded as an Accepted Residual Risk (chain-head-not-anchored) with a v2 obligation (keyed-MAC / external head-pin) — no test in this plan asserts a stronger claim than this."
  - "review() takes no state-check on PendingConfirmationState — it renders the narration for ANY existing row (Pending or terminal), since it is purely read-only and harmless either way; only find_pending_confirmation's None case (unknown effect_id) is special-cased to UnknownEffect."

requirements-completed: [CONFIRM-01, CONFIRM-03, CONFIRM-04]

coverage:
  - id: D1
    description: "T-14-08 two-commit regression proof: the un-modified assert!(blocked_count <= 1) plurality guard panics against a genuine 2-blocked-arg (to + body) email.send fixture in a committed test, BEFORE a separate commit replaces the guard with real narration"
    requirement: "CONFIRM-04"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#render_block_display_panics_on_genuine_two_blocked_arg_block_t14_08 (commit 1f3336b, guard still in place) -> render_block_display_narrates_all_args_marked_blocked_or_trusted (commit b61e043, guard replaced)"
        status: pass
    human_judgment: false
  - id: D2
    description: "render_block_display narrates EVERY resolved_arg (blocked AND trusted) in byte-wise-ascending arg_name order, each marked [BLOCKED]/[trusted], verbatim/untruncated literal, taint, source event, provenance chain, plus the Draft/untrusted-seeded posture and irreversible-send statement; no hardcoded 'file_read' mislabel; single-blocked-arg case still renders correctly; a 5000-byte literal is shown in full with no truncation"
    requirement: "CONFIRM-04"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#render_block_display_narrates_all_args_marked_blocked_or_trusted, render_block_display_single_blocked_arg_still_renders_correctly, render_block_display_does_not_truncate_a_long_literal"
        status: pass
    human_judgment: false
  - id: D3
    description: "caprun review <effect_id> is a genuine read-only pre-decision surface: prints the same narration confirm/deny would show, without transitioning state, appending any event, or invoking any sink; running it twice leaves state Pending both times; unknown effect_id returns UnknownEffect"
    requirement: "CONFIRM-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#review_prints_narration_without_mutating_state_or_appending_event, review_on_unknown_effect_id_returns_unknown_effect"
        status: pass
    human_judgment: false
  - id: D4
    description: "audit::current_chain_head returns the session's leaf event (id, hash) on a linear chain (the last-appended event, not the root), and None for a session with no events"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#current_chain_head_returns_last_appended_event_on_linear_chain, current_chain_head_returns_none_for_session_with_no_events"
        status: pass
    human_judgment: false
  - id: D5
    description: "confirm() calls audit::verify_chain BEFORE trusting the sink_blocked Event's digest, then recomputes combined_digest over the FULL current resolved_args set (blocked AND trusted) and compares to the hash-chained Event's digest before any send; a TAMPERED TRUSTED arg is caught (BLOCKER-2); a RENAMED arg is caught (Round-6 name-binding); a self-consistent literal+Event-digest edit that breaks the chain hash is caught by verify_chain specifically (distinct from the plain compare)"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#confirm_fails_closed_with_digest_mismatch_when_trusted_arg_tampered, confirm_fails_closed_with_digest_mismatch_when_arg_renamed_post_block, confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently"
        status: pass
    human_judgment: false
  - id: D6
    description: "The confirm_digest_mismatch event (and confirm_granted/confirm_denied) parent on current_chain_head, never blocked_event_id; a mismatch followed by a retry leaves audit::verify_chain STILL TRUE (no DAG fork); existing single-shot parent_id==blocked_event_id assertions in prior tests still pass unchanged"
    requirement: "CONFIRM-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#digest_mismatch_then_retry_does_not_fork_dag_verify_chain_stays_true, confirm_on_pending_file_create_releases_and_creates_file, deny_on_pending_block_is_durable (pre-existing, unchanged assertions)"
        status: pass
    human_judgment: false
  - id: D7
    description: "Workspace-wide regression: all pre-existing brokerd/caprun tests (including the cross-process cli/caprun/tests/confirm.rs integration suite) pass unmodified after the render_block_display rewrite and the head-based re-parenting; full workspace green; invariants pass"
    requirement: "CONFIRM-01"
    verification:
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast; cargo test -p caprun --test confirm; ./scripts/check-invariants.sh"
        status: pass
    human_judgment: false

# Metrics
duration: ~50min
completed: 2026-07-08
status: complete
---

# Phase 16 Plan 02: Confirm UX, Literal Binding & Negative Controls (Verifier + Narration) Summary

**Rewrites `render_block_display` to narrate every resolved arg (blocked AND trusted) verbatim in the digest's canonical order, adds a read-only `caprun review` pre-decision surface, and wires `confirm()`'s chain-verify + FULL-set recompute-and-compare integrity gate — parenting every confirm-phase event on the current chain head so a mismatch never forks the audit DAG.**

## Performance

- **Duration:** ~50 min
- **Completed:** 2026-07-08
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- **T-14-08 two-commit proof, then full narration.** A genuine 2-blocked-arg (`to` + `body`) email.send fixture proved the pre-existing `assert!(blocked_count <= 1)` plurality guard in `render_block_display` panics correctly (COMMIT `1f3336b`, guard untouched), BEFORE a separate commit (`b61e043`) removed that guard and replaced the single-arg display with a loop over EVERY `resolved_args` element — sorted byte-wise ascending by `arg_name` (the same canonical order `combined_digest` binds), each marked `[BLOCKED]` or `[trusted]`, literal shown verbatim with no truncation (a 5000-byte body fixture proves this), plus a `taint`, `Source:` (provenance-root event id), and `Provenance chain:` line, followed once by the Effect ID and a Draft/untrusted-seeded posture statement declaring that confirming authorizes an irreversible external send.
- **Dropped the misleading hardcoded `"file_read"` Source label** (the Round-2 kind-aware instruction was unimplementable without threading `conn`/`find_event_by_id` across three call sites — explicitly deferred per the coordinator's stated fallback) — the source now renders as the bare provenance-root event id.
- **`caprun review <effect_id>`** — a brand-new, genuinely read-only command. It finds the `PendingConfirmation`, prints the SAME `render_block_display` narration `confirm`/`deny` would show, and returns without ever transitioning state, appending an event, or invoking `executor::submit_plan_node`/any sink. This closes MAJOR-8: previously `render_block_display`'s only two call sites were INSIDE `confirm()`/`deny()`, AFTER the operator had already typed the decision verb, so "confirming" happened before the human had actually seen the bytes.
- **`audit::current_chain_head(conn, session_id)`** — a new helper returning the session's leaf event `(id, hash)` on a linear chain (the most-recently-appended event, via a `NOT IN (SELECT parent_id ...)` query).
- **`confirm()`'s integrity gate, in order:** (1) `audit::verify_chain` FIRST — fails closed on a broken chain before trusting anything read back via `find_event_by_id` (which only deserializes, never checks hashes); (2) recompute `combined_digest` over the FULL current `resolved_args` set (blocked AND trusted together — BLOCKER-2, never the blocked subset) using the SAME shared 16-01 primitive, compared byte-for-byte against the hash-chained `sink_blocked` Event's stored digest, using the SAME frozen in-memory snapshot the sink dispatch itself uses (no intervening DB read — no TOCTOU window). Either failure appends a durable `confirm_digest_mismatch` event and returns `ConfirmOutcome::DigestMismatch`, invoking NO sink.
- **No-fork parenting (MAJOR-7):** `confirm_digest_mismatch`, `confirm_granted`, and `confirm_denied` all now parent on `current_chain_head` — computed once per invocation and threaded forward — rather than `pc.blocked_event_id` directly. In the single-shot case (nothing appended since Block) the head IS `blocked_event_id`, so every pre-existing `parent_id == Some(blocked_event_id)` assertion still passes unchanged. A committed test drives a mismatch (row stays Pending) then a successful retry, asserting `verify_chain` stays TRUE throughout — proving the fix actually prevents the DAG fork this repo has empirically hit before (`quarantine.rs`'s `mint_from_read` doc comment).
- **`ConfirmOutcome::DigestMismatch`** is documented as an integrity ALARM, not an operator deny — the row is deliberately left `Pending` (retriable) rather than auto-transitioned to `Denied`, so an attacker who triggers a mismatch cannot thereby force-terminate a confirmation a human might legitimately retry.
- Wired the `caprun review` verb and the `DigestMismatch` exit code (8) into `cli/caprun/src/main.rs`'s dispatch and exit-code contract.

## Task Commits

1. **Task 1 COMMIT 1: T-14-08 regression proof (guard fires)** - `1f3336b` (test)
2. **Task 1 COMMIT 2: ALL-args narration + caprun review** - `b61e043` (feat)
3. **Task 2: chain-verify + FULL-set digest gate + no-fork parenting** - `f8dc3c2` (feat)

**T-14-08 two-commit gate (both SHAs, coordinator mandate):** panic-proof commit `1f3336b` → narration-replaces-it commit `b61e043`.

## Files Created/Modified

- `crates/brokerd/src/confirmation.rs` - `render_block_display` rewritten as an ALL-args narration loop; `review()` added; `ConfirmOutcome::{Reviewed, DigestMismatch}` added; `confirm()`/`deny()` re-parented on `current_chain_head`; `confirm()`'s chain-verify + FULL-set recompute-and-compare gate; new fixture/test helpers (`make_two_blocked_email_send_pending_confirmation`, `mutate_resolved_arg_literal`, `rename_resolved_arg`, `tamper_event_payload_digest_inconsistently`)
- `crates/brokerd/src/audit.rs` - `current_chain_head(conn, session_id)` helper + tests
- `cli/caprun/src/main.rs` - `caprun review <effect_id>` verb wired into the confirm/deny/review dispatch; `DigestMismatch` exit-code arm (8) added to the exit-code contract

## Decisions Made

- Followed the coordinator's T-14-08 two-commit discipline literally: the SAME fixture and the SAME test function were carried across both commits, mutating the test's assertions (from `#[should_panic]` to real narration assertions) rather than deleting and re-adding — so the git history shows a genuine behavior replacement, not a silent drop.
- Dropped the kind-aware Source-label requirement (Round-2) per the coordinator's stated fallback — implementing it would require threading `conn`/`find_event_by_id` through `render_block_display`'s three call sites (`confirm()`, `deny()`, `review()`) for a cosmetic improvement; the actual defect (the hardcoded, sometimes-wrong `"file_read"` literal) is fixed without that signature change. Recorded as a deferred enhancement, not shipped half-done.
- `review()` does not gate on `PendingConfirmationState` — it renders the narration for any row that exists (Pending or terminal), since it has no side effects either way. Only an absent row (unknown `effect_id`) is special-cased to `UnknownEffect`, matching `confirm`/`deny`'s existing fail-closed contract (T-10-03).
- Chose to fail closed UNIFORMLY (a single `DigestMismatch` outcome) whether the `sink_blocked` Event is missing, its `combined_digest` field is `None`, or the recompute simply disagrees — rather than three separate error paths — so there is exactly one integrity-alarm code path to reason about and test.
- `verify_chain`'s honest scope (MAJOR-6) is stated in the `DigestMismatch` variant's own doc comment as well as here: it is NOT authenticated or externally-anchored, and the "chain-head-not-anchored" gap is an Accepted Residual Risk with a v2 obligation (keyed-MAC / external head-pin) — no test in this plan asserts a stronger claim.

## Deviations from Plan

None — plan executed exactly as written, including the two-commit T-14-08 discipline, the SOURCE-LABEL fallback the plan itself specified, and the DigestMismatch state-posture decision the plan itself mandated (leave Pending, do not auto-deny).

## Honesty Flags (as required by the plan's `<output>` spec)

- **CONFIRM-01 traceability:** Proven here against a HAND-BUILT synthetic multi-arg seed (DB-alone fixture built directly via `PendingConfirmation`/`ResolvedArg` construction), NOT a real confined-worker doc-derived block. Complete against a proxy fixture; the real end-to-end doc-derived confirm/deny path is Phase 17.
- **MAJOR-6 (verify_chain honest scope):** `audit::verify_chain` detects single-store tampering (the payload/hash desync a bare deserialize-and-compare would miss) and non-recomputing multi-store tampering; it does NOT provide authenticated or externally-anchored integrity — nothing pins the chain head, so an actor with raw `events` table write access could forge the entire chain, including a self-consistent `verify_chain`-passing forgery. This is recorded as an **Accepted Residual Risk** (chain-head-not-anchored) with a **v2 obligation**: a keyed-MAC or external head-pin mechanism, tracked outside this phase's scope (see `.planning/todos/pending`). No committed test in this plan asserts a stronger claim than "detects the specific single-store/non-recomputing-multi-store tamper classes exercised here."

## Issues Encountered

None. The plan's own verification greps (no `assert!` plurality guard remaining in `render_block_display`; `confirm()` contains no `ValueStore`/`ValueId` re-resolution; `confirm()` calls `audit::verify_chain`; no `Some(pc.blocked_event_id)` parent left on the mismatch/granted/denied appends; no hardcoded `"file_read"` literal) all passed on the first check. All pre-existing tests (including the cross-process `cli/caprun/tests/confirm.rs` integration suite, which asserts `parent_id == Some(blocked_event_id)` for `confirm_granted`/`confirm_denied` in the single-shot case) passed unmodified.

## User Setup Required

None — no external service configuration required. No new Cargo dependency this phase.

## Next Phase Readiness

- The read-only `caprun review` surface, the ALL-args narration, and the chain-verify + FULL-set digest gate are all in place for Plan 16-03 (negative controls, `s9_live_block.rs`, a disjoint file set) and Plan 16-04 to build on.
- The Accepted Residual Risk (chain-head-not-anchored, MAJOR-6) is the one open item this plan surfaces for future work — a v2 obligation, not a blocker for this phase.
- No blockers.

---
*Phase: 16-confirm-ux-literal-binding-negative-controls*
*Completed: 2026-07-08*

## Self-Check: PASSED

All 3 modified files confirmed present on disk; all three task commits (`1f3336b`, `b61e043`, `f8dc3c2`) confirmed present in git history; `cargo test -p brokerd --lib confirmation` (32 tests), `cargo test -p brokerd --lib audit` (12 tests), `cargo test -p caprun --test confirm` (4 tests), `cargo test --workspace --no-fail-fast` (0 failed), and `./scripts/check-invariants.sh` (all gates PASSED) all verified green in this session.
