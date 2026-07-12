---
phase: 24-slot-type-binding-enforcement
plan: 03
subsystem: security
tags: [rust, tcb, taint-tracking, executor, fail-closed]

# Dependency graph
requires:
  - phase: 24-slot-type-binding-enforcement (plan 01)
    provides: origin_role threaded through ValueRecord/ValueStore::mint/quarantine.rs/server.rs
  - phase: 24-slot-type-binding-enforcement (plan 02)
    provides: DenyReason::SlotTypeMismatch { sink, arg, expected, found } variant + both exhaustive matches
provides:
  - "expected_role(sink, arg_name) -> Option<&'static [&'static str]> hardcoded table in sink_sensitivity.rs, scoped to email.send + file.create"
  - "Step 1c fail-closed per-arg role check wired into submit_plan_node, between Step 1b and the sensitivity check"
  - "The corrected DESIGN §3 table entry for email.send's body slot (adds the doc_fragment untrusted spelling), traced against live worker.rs/server.rs code"
affects: [25-regression-and-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Structural per-arg guard tier (return-immediately, same as Steps 1/1a/1b) — Step 1c never joins the Steps 2/3 collect-then-Block vec"
    - "Fail-closed Option<&[&str]> contract: None means unconstrained (documented scope-out), Some(list) means role-checked, Some(&[]) never constructed, no unwrap_or(&[]) collapse"

key-files:
  created: []
  modified:
    - crates/executor/src/sink_sensitivity.rs
    - crates/executor/src/lib.rs
    - crates/executor/tests/executor_decision.rs

key-decisions:
  - "expected_role's body row is Some([\"body\", \"doc_fragment\"]), not the DESIGN-pinned Some([\"body\"]) alone — traced live: cli/caprun/src/worker.rs's SendEmailSummary arm reports the Body: marker fragment as WorkerClaim::DocFragment, server.rs's ReportClaims dispatch assigns claim_type \"doc_fragment\", and mint_from_read reuses it verbatim as origin_role. There is no separate \"body\" claim_type in the codebase, so the DESIGN table as literally pinned would hard-Deny every real hostile-body-content flow instead of reaching I2's existing Block (CONTENT-01/CONTROL-02, shipped since Phase 14). Safe under DESIGN §3/F4: body is content-sensitive, so I2 still Blocks a tainted doc_fragment-tagged value regardless of role match."
  - "Step 1c inserted as a per-arg return-immediately guard (mirroring Steps 1/1a/1b), never collected into the blocked vec — preserves I2-before-I0 precedence and requires zero new BlockedArg/SinkBlockedAnchor shape, per DESIGN §6"
  - "expected_role's lookup is matched explicitly on the Option (Some/None) at the Step 1c call site — no .unwrap_or(&[]) anywhere in sink_sensitivity.rs or lib.rs, confirmed by grep"

requirements-completed: [T2-03, T2-05]

coverage:
  - id: D1
    description: "expected_role() hardcoded table added to sink_sensitivity.rs, scoped to email.send (to/cc/bcc, subject, body) and file.create (path, contents unconstrained)"
    requirement: T2-03
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#tests (8 new expected_role tests, all pass)"
        status: pass
    human_judgment: false
  - id: D2
    description: "submit_plan_node hard-Denies a role<->slot mismatch (and a None origin_role at a role-checked slot) via Step 1c, before the sensitivity check"
    requirement: T2-05
    verification:
      - kind: integration
        ref: "crates/executor/tests/executor_decision.rs#role_mismatch_denies"
        status: pass
      - kind: integration
        ref: "crates/executor/tests/executor_decision.rs#role_none_at_role_checked_slot_denies"
        status: pass
    human_judgment: false
  - id: D3
    description: "A role-matching but tainted value at a routing/content-sensitive slot still Blocks (I2 precedence preserved); an unconstrained slot (file.create contents) is unaffected by Step 1c"
    requirement: T2-05
    verification:
      - kind: integration
        ref: "crates/executor/tests/executor_decision.rs#matching_role_tainted_still_blocks"
        status: pass
      - kind: integration
        ref: "crates/executor/tests/executor_decision.rs#unconstrained_slot_unaffected"
        status: pass
    human_judgment: false
  - id: D4
    description: "Full Mac workspace green after Step 1c lands, including the pre-existing extract_provenance_threading.rs live body-block regression tests"
    requirement: T2-05
    verification:
      - kind: integration
        ref: "cargo build --workspace"
        status: pass
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gates 1/2/3)"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-07-12
status: complete
---

# Phase 24 Plan 03: Slot-Type Binding Enforcement Summary

**Hardcoded `expected_role()` table + fail-closed Step 1c role check wired into `submit_plan_node` — a misrouted `UserTrusted` value now hard-Denies with `SlotTypeMismatch` before it can reach a sink, closing the v1.4 T2 residual; I0/I2 precedence unchanged.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-12T03:03:33Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- `expected_role(sink, arg_name) -> Option<&'static [&'static str]>` added to `sink_sensitivity.rs`, mirroring `is_routing_sensitive`/`is_content_sensitive`'s hardcoded-match discipline — scoped to the two live sinks, with the fail-closed `Option`-not-empty-slice contract enforced by construction (`Some(&[])` never constructed).
- Step 1c wired into `submit_plan_node`, inserted between the empty-provenance guard (1b) and the sensitivity check (2/3): a role<->slot mismatch, or a `None` origin_role at a role-checked slot, returns `Denied { SlotTypeMismatch }` immediately — following the exact return-immediately idiom of Steps 1/1a/1b, never joining the Steps 2/3 collect-then-Block `blocked` vec.
- I0/I2 precedence proven intact: `matching_role_tainted_still_blocks` shows a role-matching-but-tainted value still Blocks; the existing `draft_session_tainted_routing_arg_still_blocks_not_denied` and `non_live_session_denies_commit_irreversible_in_all_four_states` tests (unmodified) continue to pass unchanged.
- Corrected the DESIGN §3 pinned table: `email.send`'s `body` row is `Some(["body", "doc_fragment"])`, not `Some(["body"])` alone (see Deviations — this was caught by a real pre-existing test failure, not speculation).
- Full Mac workspace green: `cargo build --workspace` exit 0, `cargo test --workspace --no-fail-fast` exit 0 (all binaries "ok"), `./scripts/check-invariants.sh` all 3 gates PASS.

## Task Commits

1. **Task 1: Add the hardcoded expected_role() table to sink_sensitivity.rs** - `81a1c4e` (feat)
2. **Task 2: Insert Step 1c fail-closed role check into submit_plan_node** - `2ce7908` (feat, includes the body/doc_fragment table correction)

**Plan metadata:** pending (this commit)

## Files Created/Modified
- `crates/executor/src/sink_sensitivity.rs` - added `expected_role()`, 8 new unit tests covering every table row plus unknown-arg/unknown-sink cases
- `crates/executor/src/lib.rs` - inserted Step 1c into `submit_plan_node`, constructing `DenyReason::SlotTypeMismatch`
- `crates/executor/tests/executor_decision.rs` - 4 new integration tests: `role_mismatch_denies`, `role_none_at_role_checked_slot_denies`, `matching_role_tainted_still_blocks`, `unconstrained_slot_unaffected` (plan asked for 3; a 4th — the explicit `None`-role case — was added because DESIGN §7 item 1 and item 2 are two distinct failure paths and both are load-bearing)

## Decisions Made
- Step 1c is a per-arg, return-immediately structural guard — same tier as Steps 1/1a/1b, never collected into the `blocked` vec. Zero new `BlockedArg`/`SinkBlockedAnchor` shape added, per DESIGN §6.
- `expected_role`'s lookup is matched explicitly on the `Option` at the call site (`match record.origin_role.as_deref() { Some(role) => expected.contains(&role), None => false }`) — no `.unwrap_or(&[])` anywhere in either modified file (confirmed by grep, 0 hits in both).
- `email.send`'s `body` expected-role list includes `"doc_fragment"` alongside `"body"` — see Deviations for full rationale.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Corrected the DESIGN §3 pinned table's `body` row to include the `"doc_fragment"` untrusted spelling**
- **Found during:** Task 2 verification (`cargo test --workspace --no-fail-fast` failed 3 tests in `crates/brokerd/tests/extract_provenance_threading.rs` with "a durable sink_blocked event must exist" panics, after Step 1c first landed with the DESIGN-literal `Some(["body"])` table entry).
- **Issue:** DESIGN-slot-type-binding.md §3 pins `email.send`'s `body` row as `Some(["body"])` only. But the ONLY production vocabulary for hostile-extracted `Body:` content is `"doc_fragment"` — traced live: `cli/caprun/src/worker.rs`'s `SendEmailSummary` arm reports the `Body:` marker fragment as `WorkerClaim::DocFragment` (worker.rs:236-237); `crates/brokerd/src/server.rs`'s `ReportClaims` dispatch maps it to `Claim { claim_type: "doc_fragment", .. }` (server.rs:1075-1076); `mint_from_read` reuses `claim_type` verbatim as `origin_role` (per Plan 01/DESIGN §2). There is no separate `"body"` claim_type anywhere in the codebase — `email_address`, `relative_path`, and `doc_fragment` are the only three legal claim types (`quarantine.rs:315-341`). With the table as literally pinned, every real hostile-body-content flow would hard-Deny at Step 1c instead of reaching I2's existing per-arg Block — silently regressing the CONTENT-01/CONTROL-02 body-Block acceptance behavior shipped since Phase 14 (and very likely breaking the Linux-only live acceptance tests `s9_live_block.rs`'s `s9_control02_body_tainted_recipient_trusted_blocks` and `live_acceptance_v1_4_composed.rs`, though those weren't run on this Mac).
- **Fix:** Changed `sink_sensitivity.rs`'s `expected_role` for `email.send`/`body` from `Some(&["body"])` to `Some(&["body", "doc_fragment"])`, mirroring the exact dual-vocabulary pattern DESIGN §2/§3 already uses for `to`/`cc`/`bcc` (`["recipient", "email_address"]`) and `path` (`["path", "relative_path"]`). Confirmed safe under DESIGN §3's own F4 table-construction invariant: `body` is content-sensitive (`is_content_sensitive`), so a tainted `doc_fragment`-tagged value at `body` still hits I2's per-arg Block regardless of role match — the role check never becomes the sole gate for this untrusted vocabulary.
- **Files modified:** `crates/executor/src/sink_sensitivity.rs` (table + doc comment + renamed unit test), `crates/executor/src/lib.rs` (unrelated `.unwrap_or(&[])` doc-comment wording fix in the same commit, see below)
- **Verification:** `cargo test --workspace --no-fail-fast` exit 0 (all 3 previously-failing `extract_provenance_threading.rs` tests now pass); `./scripts/check-invariants.sh` all gates PASS.
- **Committed in:** `2ce7908` (Task 2 commit)

**2. [Rule 1 - Bug] Doc-comment text tripped the plan's own negative grep acceptance criterion twice**
- **Found during:** Task 1 and Task 2 acceptance-criteria verification.
- **Issue:** Explanatory prose in both `sink_sensitivity.rs` and `lib.rs` literally contained the string `.unwrap_or(&[])` while describing the anti-pattern to avoid, which the acceptance criterion's `grep -c 'unwrap_or(&\[\])'` counted as a hit (expected 0, got 1 in each file) — a false positive on the literal grep, not an actual `.unwrap_or(&[])` call site.
- **Fix:** Reworded both comments to describe the anti-pattern without spelling out the exact banned token, following the same technique 24-02-SUMMARY.md already used for the identical grep trap.
- **Files modified:** `crates/executor/src/sink_sensitivity.rs`, `crates/executor/src/lib.rs`
- **Verification:** both greps now return 0; behavior unchanged (comment-only edit).
- **Committed in:** `81a1c4e` (Task 1), `2ce7908` (Task 2)

---

**Total deviations:** 2 auto-fixed (1 missing critical / table correction, 1 grep-trap wording fix)
**Impact on plan:** The table correction is the substantive deviation — it is a Rust-TCB security-table change discovered via a genuine failing pre-existing test, not a hypothetical. It is scoped identically to the plan's own mechanism (same file, same const-array pattern, same F4 invariant), adds no new architecture, and was necessary to avoid silently breaking a previously-shipped, load-bearing security acceptance behavior (CONTENT-01/CONTROL-02). No scope creep beyond the one table row. Flagging for Phase 25's regression audit / live acceptance re-run to independently confirm on Linux.

## Issues Encountered
- Advisor tool was unavailable for this session (errored "unavailable"); no fallback agent-spawning tool was available in this environment either, so the table-correction judgment call was made directly, with full evidence-trail documentation above so it can be independently re-verified.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 24 (T2-02..05) is now fully implemented: origin_role threading (Plan 01), the `DenyReason::SlotTypeMismatch` variant (Plan 02), and the `expected_role()` table + Step 1c enforcement (this plan) all land, workspace-green.
- **Flag for Phase 25 (T2-06/07/08):** the held-out swapped-recipient/subject acceptance test and the independent `scripts/mailpit-verify.sh` Linux re-run should specifically re-confirm the `body`/`doc_fragment` table correction against the LIVE `s9_control02_body_tainted_recipient_trusted_blocks` and `s9_live_block.rs` scenarios (Linux-gated, not run on this Mac) — this plan's Mac-side regression coverage (`extract_provenance_threading.rs`) is a faithful mirror of that flow but is not the live acceptance test itself.
- No other blockers. `DESIGN-slot-type-binding.md`'s §3 table should be amended in a follow-up doc pass to match the corrected `body` row, so the doc and code stay in sync for future readers.

---
*Phase: 24-slot-type-binding-enforcement*
*Completed: 2026-07-12*

## Self-Check: PASSED
- FOUND: crates/executor/src/sink_sensitivity.rs
- FOUND: crates/executor/src/lib.rs
- FOUND: crates/executor/tests/executor_decision.rs
- FOUND: .planning/phases/24-slot-type-binding-enforcement/24-03-SUMMARY.md
- FOUND commit: 81a1c4e
- FOUND commit: 2ce7908
