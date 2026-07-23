---
phase: 15-deterministic-doc-action-extraction
plan: 03
subsystem: security
tags: [ipc-protocol, taint-tracking, provenance, wire-dispatch, brokerd]

requires:
  - phase: 15-deterministic-doc-action-extraction
    provides: "mint_from_derivation (Plan 01) — the provenance-threading, fail-closed derived-value mint"
provides:
  - "WorkerClaim::DocFragment + TransformKind::Concat + BrokerRequest::ReportDerivedClaim + BrokerResponse::DerivedClaimReceived (additive wire protocol)"
  - "Live broker dispatch arm that resolves input ValueIds and mints derived values via mint_from_derivation, with mint_from_read's DocFragment '@'-guard surfaced as Denied on the wire"
affects: [15-04-worker-transform-wiring, 17-acceptance]

tech-stack:
  added: []
  patterns:
    - "Fail-closed-but-connection-continues error surfacing: dispatch_request arms that can fail on ATTACKER-CONTROLLED input (not internal invariant violations) match the Result explicitly and send BrokerResponse::Error + return Ok(()), rather than propagating via `?` (which would kill the whole connection) — mirrors the pre-existing CreateSession arm's pattern."
    - "Resolve-then-clone-then-mint: input_value_ids are resolved against the per-connection ValueStore and cloned to OWNED ValueRecords before calling mint_from_derivation, avoiding a simultaneous mutable (mint) + immutable (resolve) borrow of the same ValueStore."

key-files:
  created: []
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/proto_claims.rs

key-decisions:
  - "mint_from_read's Err on the ReportClaims arm is now match'd explicitly (not `?`) so an attacker-controlled failure (the doc_fragment '@'-guard) surfaces as BrokerResponse::Error on the wire and the connection stays alive, instead of being propagated as a connection-killing anyhow::Error via dispatch_request's Result -- this changes prior behavior for ALL claim types, not just DocFragment, but is safe: EmailAddress/RelativePath never actually fail mint_from_read today (unreachable in practice), and the new explicit-match discipline is strictly more correct."
  - "TransformKind is a real Rust enum (not the bare `&str`/String tag mint_from_derivation takes internally) with an explicit `as_mint_tag()` method as the SINGLE wire-tag-to-mint-tag mapping site, per Task 1's instruction to define a typed enum for audit narration while mint_from_derivation's own internal match stays a string tag (Wave 1's existing signature, unchanged)."
  - "Resolved input records are cloned to owned ValueRecords in the ReportDerivedClaim arm before calling mint_from_derivation, per its own doc comment's stated calling convention -- avoids a mutable+immutable ValueStore borrow conflict that would otherwise be a compile error."

requirements-completed: [EXTRACT-01]

coverage:
  - id: D1
    description: "WorkerClaim::DocFragment, TransformKind::Concat, BrokerRequest::ReportDerivedClaim, BrokerResponse::DerivedClaimReceived added additively to proto.rs; existing variants unchanged; serde round-trips pass"
    requirement: "EXTRACT-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/proto.rs#proto::tests::doc_fragment_claim_round_trips, transform_kind_concat_round_trips, report_derived_claim_request_round_trips, derived_claim_received_response_round_trips"
        status: pass
    human_judgment: false
  - id: D2
    description: "ReportClaims dispatch arm handles DocFragment via mint_from_read and surfaces its Err (including the looks_like_doc_fragment '@'-guard) as BrokerResponse::Error on the live wire — an assembled recipient sent as a fresh DocFragment is rejected, mints nothing, no ClaimsReceived sent"
    requirement: "EXTRACT-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/proto_claims.rs#report_claims_dispatch_rejects_assembled_recipient_as_doc_fragment"
        status: pass
    human_judgment: false
  - id: D3
    description: "New ReportDerivedClaim dispatch arm resolves every input_value_id against the connection's ValueStore (fail-closed on any unresolved handle), calls mint_from_derivation under the conn lock (the sole new mint call), advances the causal chain head only on Ok, and surfaces mint_from_derivation's fail-closed guards (non-file_read-rooted union element, concat byte-verify mismatch) as Denied with no chain-head advance on Err"
    requirement: "EXTRACT-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/proto_claims.rs#report_derived_claim_dispatch_threads_provenance_from_resolved_inputs, report_derived_claim_dispatch_rejects_non_file_read_root_at_index_0, report_derived_claim_dispatch_rejects_concat_byte_mismatch, report_derived_claim_dispatch_rejects_unresolvable_input"
        status: pass
    human_judgment: false
  - id: D4
    description: "proto_claims.rs holds no exhaustive-match/variant-count assertion that the new additive variants would break (finding #13 check) — confirmed by inspection before adding new tests; existing tests (unknown_claim_kind_fails_closed etc.) untouched"
    verification:
      - kind: other
        ref: "crates/brokerd/tests/proto_claims.rs — manual read-before-edit confirmed no such assertion exists; no existing test line removed or altered"
        status: pass
    human_judgment: false

duration: ~55min
completed: 2026-07-08
status: complete
---

# Phase 15 Plan 03: Live Broker-Dispatch Wiring for Transform-Derived Mints Summary

**`BrokerRequest::ReportDerivedClaim` dispatch arm resolves worker-supplied input handles and mints the derived value ONLY through `mint_from_derivation` (Plan 01), fail-closed on any unresolved handle, non-file_read-rooted union element, or concat byte-verify mismatch — proven by 5 new live-dispatch tests that call `dispatch_request` directly, not a hand-built record.**

## Performance

- **Duration:** ~55 min
- **Started:** 2026-07-08T~13:05Z
- **Completed:** 2026-07-08T14:04:33Z
- **Tasks:** 2 (both TDD)
- **Files modified:** 3 (`crates/brokerd/src/proto.rs`, `crates/brokerd/src/server.rs`, `crates/brokerd/tests/proto_claims.rs`)

## Accomplishments

- **`WorkerClaim::DocFragment(String)`** — additive wire variant carrying only the extracted fragment token; existing `EmailAddress`/`RelativePath` variants unchanged.
- **`TransformKind` enum** (`Concat` only, Phase 15 scope) with `as_mint_tag()` — the single explicit mapping from the wire-level tag to the `&str` tag `mint_from_derivation` matches on internally.
- **`BrokerRequest::ReportDerivedClaim { transformed_literal, transform, input_value_ids }`** and **`BrokerResponse::DerivedClaimReceived { value_id }`** — additive request/response pair, no `session_id` field (same HARD-03 contract as `SubmitPlanNode`/`ReportClaims`).
- **`ReportClaims` dispatch arm** gained the `DocFragment` mapping AND changed its `mint_from_read` call from `?`-propagation to an explicit `match` — `mint_from_read`'s `looks_like_doc_fragment` guard Err (an assembled `'@'`-containing recipient submitted as a fresh `DocFragment`) now surfaces as `BrokerResponse::Error` on the live wire (finding #1c) instead of killing the connection.
- **New `ReportDerivedClaim` dispatch arm**: resolves every `input_value_id` against the per-connection `ValueStore` (fail-closed Error response if any is unresolvable — Pitfall 1), clones resolved records to owned `ValueRecord`s (avoiding a borrow conflict with `mint_from_derivation`'s `&mut ValueStore` param), calls `mint_from_derivation` under the conn lock as the SOLE new mint call, and mirrors `ReportClaims`' chain-head-advance discipline exactly: `*last_event_id`/`*last_event_hash` advance to the derivation event ONLY on `Ok`; on `Err` (zero inputs / any-element non-file_read-rooted untrusted union / concat byte-verify mismatch) responds `Error` and advances nothing. Does not touch `session_status` (not an I1 trust-flip site).
- **5 new live-dispatch integration tests** in `proto_claims.rs`, each calling `dispatch_request` directly (not a hand-built `mint_from_derivation` unit-test walk): provenance-threading proof, finding #1c (assembled-recipient-as-DocFragment rejection), finding #3 (UserTrusted-handle-at-index-0 rejection), MAJOR-1 (concat byte-verify-mismatch rejection), and unresolvable-input-handle rejection — plus 3 additional serde round-trip tests for the new wire types wrapped in their containing requests/responses.

## Task Commits

1. **Task 1 RED — failing proto.rs serde round-trip tests** — `5bb224d` (test)
2. **Task 1 GREEN — DocFragment/TransformKind/ReportDerivedClaim/DerivedClaimReceived wire types** — `089cf1f` (feat)
3. **Task 2 RED — failing live-dispatch tests in proto_claims.rs** — `e61c71b` (test)
4. **Task 2 GREEN — DocFragment + ReportDerivedClaim dispatch arms** — `e480dbb` (feat)

_Both tasks (tdd="true") each produced a RED (compile-failure) commit followed by a GREEN (passing) commit._

## Files Created/Modified

- `crates/brokerd/src/proto.rs` — `WorkerClaim::DocFragment`, `TransformKind` enum + `as_mint_tag()`, `BrokerRequest::ReportDerivedClaim`, `BrokerResponse::DerivedClaimReceived`; 4 new unit tests in a new `#[cfg(test)] mod tests`.
- `crates/brokerd/src/server.rs` — `ReportClaims` arm's `DocFragment` claim mapping + explicit-match error surfacing for `mint_from_read`; new `ReportDerivedClaim` dispatch arm (resolve → clone → `mint_from_derivation` → chain-head advance on Ok / Error response on Err).
- `crates/brokerd/tests/proto_claims.rs` — 3 additional serde round-trip tests (DocFragment-via-ReportClaims, ReportDerivedClaim, DerivedClaimReceived) + a `DispatchHarness` test helper + 5 live-dispatch security tests.

## Decisions Made

- **`mint_from_read`'s Err handling changed for the WHOLE `ReportClaims` arm, not just `DocFragment`**: the plan's finding #1c specifically calls out surfacing the doc_fragment guard's Err, but since `mint_from_read` is called generically per-claim inside one loop, the explicit `match`-instead-of-`?` change necessarily applies uniformly to all three claim types in that arm. This is safe and strictly more correct: `EmailAddress`/`RelativePath` never actually produced an Err from `mint_from_read` in practice (their taint match arms are infallible), so this is a no-behavior-change for those two variants and a genuine fail-closed fix for `DocFragment`.
- **`TransformKind` kept as a typed enum with `as_mint_tag() -> &'static str`** rather than threading a bare `String`/`&str` tag through the wire protocol — per Task 1's own instruction ("for audit narration") and consistent with the codebase's existing exhaustive-match-fails-closed discipline (an unrecognized wire tag fails at deserialize, not at a runtime string comparison).
- **Owned-clone resolve pattern in the `ReportDerivedClaim` arm**: `mint_from_derivation`'s own doc comment (Wave 1) explicitly states the caller must resolve `ValueId`s to owned `ValueRecord` clones before calling it, to avoid a simultaneous mutable (`store: &mut ValueStore` for the mint) + immutable (`resolve()` borrows) borrow conflict. Implemented exactly as documented — `resolved: Vec<ValueRecord>` (owned clones) built first, then `input_refs: Vec<&ValueRecord>` borrowed from that separate owned Vec (never from `value_store` itself).

## Deviations from Plan

None — plan executed exactly as written. The one item worth flagging for transparency (not a deviation, an inherent Rust cross-file compile constraint): Task 1's own scoped verify command (`cargo test -p brokerd --lib proto`) cannot fully compile in isolation, because `WorkerClaim`/`BrokerRequest` are matched exhaustively in `server.rs` (a different file in the same crate) — adding the new variants in Task 1 necessarily breaks that exhaustive match until Task 2 lands. This is documented explicitly in the Task 1 GREEN commit message. Both tasks were still committed atomically per the plan's own task split; the crate as a whole (and both tasks' own verify commands) are fully green after Task 2's commit, which is the state verified below.

## Issues Encountered

None beyond the documented compile-dependency note above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- `ReportDerivedClaim`/`DerivedClaimReceived` are ready for Plan 04 (the confined worker) to consume: a worker calls `extract_doc_fragments`/`concat_doc_fragments` (Plan 01), reports the raw fragments via `ReportClaims`, then reports the concatenated literal via `ReportDerivedClaim` referencing the returned `value_ids` — the broker never re-applies the transform.
- No blockers. `cargo test --workspace --no-fail-fast` is fully green (all crates, no failures) and `./scripts/check-invariants.sh` (all 3 gates, including Gate 3's mint-call-site restriction) PASSES at this plan's completion. `cargo test -p brokerd` alone: 14/14 in `proto_claims.rs` (including the 5 new live-dispatch security tests), 4/4 in `proto`'s own unit test module, all other pre-existing brokerd test files unchanged and passing.
- Note (per plan's own success criteria): Plan 15-02 was executing concurrently in a separate worktree from the same base commit, touching only `audit.rs`/`extract_provenance_threading.rs`/`hostile_doc.txt` — confirmed disjoint from this plan's file list (`proto.rs`/`server.rs`/`tests/proto_claims.rs`). Any test-count reconciliation at merge time is expected and owned by the orchestrator, not this plan.

---
*Phase: 15-deterministic-doc-action-extraction*
*Completed: 2026-07-08*

## Self-Check: PASSED

All claimed files found on disk (`crates/brokerd/src/proto.rs`, `crates/brokerd/src/server.rs`, `crates/brokerd/tests/proto_claims.rs`, this SUMMARY.md); all 4 claimed commit hashes (`5bb224d`, `089cf1f`, `e61c71b`, `e480dbb`) verified present in `git log --oneline --all`.
