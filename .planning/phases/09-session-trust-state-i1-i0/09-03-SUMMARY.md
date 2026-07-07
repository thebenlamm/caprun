---
phase: 09-session-trust-state-i1-i0
plan: 03
subsystem: brokerd
tags: [rust, session-lifecycle, audit-dag, taint-model, i1, i0]

# Dependency graph
requires:
  - phase: 09-session-trust-state-i1-i0
    plan: "09-01"
    provides: "SessionStatus::Draft, SeedProvenance { TrustedArg, FileDerived }"
  - phase: 09-session-trust-state-i1-i0
    plan: "09-02"
    provides: "executor::submit_plan_node's 5th session_status: &SessionStatus parameter, Step 0.5 draft-only deny"
provides:
  - "brokerd::session::update_session_status(conn, session_id, status) — the Active->Draft UPDATE path"
  - "brokerd::session::create_session(intent_id, seed_provenance: SeedProvenance) — conditional initial status"
  - "mint_from_read atomic I1 demotion: sessions.status=Draft + causally-linked session_demoted Event, same lock"
  - "session_status threaded per-connection through run_broker_server/handle_connection/dispatch_request"
  - "SeedProvenance recorded in the session_created Event's actor field (ORIGIN-01)"
affects: [09-04-cli-onramp, 11-live-acceptance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Widened mint_from_read's return tuple with an explicit new chain-head pair (demoted_event_id, demoted_hash), keeping the original (read_event_id, read_hash) semantics intact for provenance/anchor callers — additive tuple growth to resolve a topology conflict between two callers with different needs from the same function"
    - "Encoding an additive audit fact (seed provenance) into an existing Event field (actor) rather than widening the Event schema, when the fact is descriptive/audit-only and not itself security-decision-bearing"

key-files:
  created: []
  modified:
    - crates/brokerd/src/session.rs
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/lib.rs
    - crates/brokerd/tests/phase5_dispatch.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/uds_ipc.rs

key-decisions:
  - "mint_from_read's return tuple widened from 3 to 5 elements (added demoted_event_id, demoted_hash) rather than repurposing the existing (read_event_id, read_hash) fields — required to keep the held-out §9 anti-stapling backstop (provenance_chain[0] == read_event_id) and every existing file_read identity assertion unweakened, while still giving callers a way to avoid forking the causal DAG"
  - "SeedProvenance recorded in the session_created Event's actor field (e.g. 'broker:seed_provenance=trusted_arg'), not a new Event/session-table column — Event has no free-form metadata field, actor is still part of the hashed payload, and this avoids a schema migration (RESEARCH Open Question 2, option b)"
  - "The in-broker CreateSession IPC arm always seeds SeedProvenance::TrustedArg — this path is test-only (the live cli/caprun path calls create_session directly, not through this arm); FileDerived seeding is Plan 09-04's CLI on-ramp responsibility"

requirements-completed: [TAINT-01, TAINT-04, ORIGIN-02]

coverage:
  - id: D1
    description: "update_session_status UPDATE path added; create_session starts FileDerived sessions Draft, TrustedArg sessions Active (exhaustive match, no wildcard)"
    requirement: "ORIGIN-02"
    verification:
      - kind: unit
        ref: "cargo test -p brokerd --lib session (create_session_file_derived_starts_draft, create_session_trusted_arg_starts_active, update_session_status_mutates_persisted_row)"
        status: pass
    human_judgment: false
  - id: D2
    description: "mint_from_read atomically demotes the session to Draft and appends a session_demoted Event whose parent_id equals the triggering file_read Event id; mint_from_intent does not demote"
    requirement: "TAINT-01 / TAINT-04"
    verification:
      - kind: unit
        ref: "cargo test -p brokerd --lib quarantine (mint_from_read_demotes_session_to_draft, mint_from_read_demotion_causal_edge, mint_from_intent_does_not_demote_session)"
        status: pass
    human_judgment: false
  - id: D3
    description: "session_status threaded per-connection through server.rs and passed by reference into executor::submit_plan_node; all brokerd call sites reconciled to the new signatures; v1.1 §9 acceptance and durable-anchor tests unregressed"
    requirement: "TAINT-01 (broker-side wiring)"
    verification:
      - kind: unit
        ref: "cargo test -p brokerd --no-fail-fast (52 tests: lib unit tests, audit_dag, durable_anchor, phase5_dispatch, proto_claims, s9_acceptance)"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-07-07
status: complete
---

# Phase 9 Plan 3: Broker Session Demotion Summary

**Wired the broker to the executor's draft-only mechanism: `mint_from_read` now atomically demotes a session to `Draft` with a causally-linked `session_demoted` audit event, `create_session` starts file-derived sessions `Draft` at creation, and a broker-owned `session_status` is threaded per-connection into `executor::submit_plan_node` — while fixing a causal-DAG fork the new demotion event introduced.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-07T02:32:49Z
- **Tasks:** 3 completed
- **Files modified:** 9 (4 planned in files_modified + 5 additional test files required for the crate to compile/pass — see Deviations)

## Accomplishments

- `update_session_status(conn, session_id, status)` — the monotonic `Active -> Draft` UPDATE path (`crates/brokerd/src/session.rs`), mirroring `persist_session`'s exact param/error-handling style.
- `create_session` gains `seed_provenance: SeedProvenance`, exhaustively matched: `FileDerived -> Draft`, `TrustedArg -> Active` (ORIGIN-02, no wildcard arm).
- `mint_from_read` (`crates/brokerd/src/quarantine.rs`) performs the atomic I1 demotion as a new Step 4, under the SAME connection/lock already held by the caller: `update_session_status(..., Draft)` + a `session_demoted` Event whose `parent_id` equals the just-appended `file_read` event id (TAINT-04 causal edge). `mint_from_intent` is untouched — verified by a new non-regression test.
- `session_status` threaded through `run_broker_server` -> `handle_connection` -> `dispatch_request` as a mutable per-connection local (mirrors the existing `last_event_id`/`last_event_hash` pattern), seeded from `create_session`'s result. `ReportClaims` sets it to `Draft` after `mint_from_read`; `SubmitPlanNode` passes `&*session_status` — never a value read from `plan_node`/IPC — as the 5th argument to `executor::submit_plan_node`.
- `CreateSession` IPC arm seeds `SeedProvenance::TrustedArg` (this path is test-only) and records that provenance in the `session_created` Event's `actor` field (ORIGIN-01).
- `brokerd::submit_plan_node` wrapper (`lib.rs`) extended with the same `session_status` parameter; its 2 unit tests updated.
- `cargo test -p brokerd --no-fail-fast`: **52 passed, 0 failed** (up from a non-compiling crate at plan start). `./scripts/check-invariants.sh`: both gates PASS.
- `cargo build --workspace`: `cli/caprun` fails to compile at the `create_session`/`run_broker_server` call sites in `main.rs` — this is the exact, expected breaking-signature ripple this plan's own `<verification>` names; deferred to Plan 09-04.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add update_session_status and make create_session conditional on SeedProvenance** - `b54aade` (feat)
2. **Task 2: mint_from_read performs the atomic session demotion with a causal-edge audit event** - `e234fe4` (feat)
3. **Task 3: Thread session_status through server dispatch, record provenance, and reconcile all brokerd call sites** - `16b9973` (feat)

## Files Created/Modified

- `crates/brokerd/src/session.rs` - `update_session_status` UPDATE path; `create_session` conditional on `SeedProvenance`; 3 new inline unit tests
- `crates/brokerd/src/quarantine.rs` - `mint_from_read` Step 4 (atomic demotion + causal `session_demoted` event); return tuple widened to expose the new chain head; 3 new inline unit tests
- `crates/brokerd/src/server.rs` - `session_status` threaded through `run_broker_server`/`handle_connection`/`dispatch_request`; `ReportClaims` and `SubmitPlanNode` arms updated; `CreateSession` arm records `SeedProvenance`
- `crates/brokerd/src/lib.rs` - `submit_plan_node` wrapper extended with `session_status`; 2 tests updated
- `crates/brokerd/tests/phase5_dispatch.rs` - 4 `mint_from_read`/`executor::submit_plan_node` call sites and 2 `dispatch_request` call sites updated (new args + chain-head fix)
- `crates/brokerd/tests/s9_acceptance.rs` - 3 `executor::submit_plan_node` call sites and 2 `mint_from_read` destructuring sites updated
- `crates/brokerd/tests/durable_anchor.rs` - `mint_from_read`/`dispatch_request` call sites updated (chain-head fix; not in this plan's `files_modified` frontmatter but required for the crate to compile/pass — see Deviations)
- `crates/brokerd/tests/proto_claims.rs` - `dispatch_request` call site updated (not in frontmatter; required for compile — see Deviations)
- `crates/brokerd/tests/uds_ipc.rs` - `run_broker_server` call sites updated (Linux-gated; not in frontmatter; required for Linux CI — see Deviations)

## Decisions Made

- Widened `mint_from_read`'s return tuple to 5 elements (`read_event_id, read_hash, value_id, demoted_event_id, demoted_hash`) rather than repurposing the original two fields, so every existing provenance/anchor assertion (including the held-out §9 anti-stapling backstop, which asserts `provenance_chain[0] == read_event_id`) remains byte-for-byte correct.
- Recorded `SeedProvenance` in the `session_created` Event's `actor` field rather than adding a new field to `Event` or the `sessions` table — `Event`'s serialized form IS the audit `payload` column (no free-form metadata field exists), and this avoids a schema migration while still making the provenance tamper-evident (part of the hashed payload).
- The in-broker `CreateSession` IPC arm always seeds `TrustedArg` — it's a test-only path; `FileDerived` seeding via the CLI's new on-ramp is explicitly Plan 09-04's responsibility per this plan's own scope.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `session_demoted` forked the causal DAG, breaking `verify_chain`**

- **Found during:** Task 2/3 integration — `cargo test -p brokerd --no-fail-fast` after wiring `session_status` into `server.rs`'s `ReportClaims` arm per the plan's literal instructions.
- **Issue:** The plan's Task 3 action text says only to set `session_status = SessionStatus::Draft` after `mint_from_read` returns — it does not mention changing how `last_event_id`/`last_event_hash` are advanced. Following that literally, the connection continues chaining subsequent events (e.g. a `SubmitPlanNode` block) onto the returned `(read_event_id, read_hash)` — i.e., onto `file_read`. But `mint_from_read`'s new Step 4 already appended `session_demoted` as `file_read`'s child. The next event therefore became a topological SIBLING of `session_demoted` (both children of `file_read`), forking the audit DAG. `audit::verify_chain`'s recursive-CTE walk assumes a single linear chain (it carries one scalar `prev_hash` across the whole walk); a fork produces two rows at the same depth and the walk fails at the second sibling's `parent_hash` comparison. This surfaced empirically as 3 failing tests in `durable_anchor.rs` (`after_exit_db_alone_anti_stapling_sentinel`, `tamper_evidence_mutating_payload_breaks_verify_chain`, `redacting_side_table_literal_preserves_verify_chain_and_digest`) — all three call `verify_chain` after driving a real block through `dispatch_request`.
- **Fix:** Widened `mint_from_read`'s return tuple to also expose `(demoted_event_id, demoted_hash)` — the id/hash of the `session_demoted` event it just appended, i.e. the actual new head of the linear chain. `read_event_id`/`read_hash` (the first two tuple elements) keep their exact original meaning (`file_read`'s own identity), so no existing provenance/anchor test changed behavior. Every caller that continues the connection's causal chain (`server.rs`'s `ReportClaims` arm; `phase5_dispatch.rs`'s `block_appends_durable_causal_sink_blocked`/`append_failure_is_fail_closed`; `durable_anchor.rs`'s `build_hostile_block_db`) now advances `last_event_id`/`last_event_hash` to the demoted event, not `file_read`. Callers that don't continue the chain (most unit tests, `s9_acceptance.rs`) just ignore the two new tuple elements.
- **Files modified:** `crates/brokerd/src/quarantine.rs` (signature + doc), `crates/brokerd/src/server.rs` (`ReportClaims` arm), `crates/brokerd/tests/phase5_dispatch.rs`, `crates/brokerd/tests/durable_anchor.rs`, `crates/brokerd/tests/s9_acceptance.rs` (destructuring only, no chain-continuation change needed), `crates/brokerd/src/quarantine.rs`'s own test module (6 destructuring sites).
- **Verification:** `cargo test -p brokerd --no-fail-fast` — all 52 tests pass, including the 3 that were failing (`durable_anchor.rs`) and the two held-out anti-stapling backstops (`s9_acceptance.rs`).
- **Committed in:** `e234fe4` (mint_from_read signature/fix), `16b9973` (caller updates).

**2. [Rule 3 - Blocking issue] Additional call sites outside `files_modified` needed updating to compile**

- **Found during:** Task 3, first `cargo test -p brokerd --no-fail-fast` after Tasks 1-2 landed.
- **Issue:** The plan's `files_modified` frontmatter lists only `phase5_dispatch.rs` and `s9_acceptance.rs` as test files needing updates, but three more test files in the crate also call `dispatch_request`/`run_broker_server`/`mint_from_read` with the old signatures: `crates/brokerd/tests/proto_claims.rs` (1 `dispatch_request` call), `crates/brokerd/tests/durable_anchor.rs` (`mint_from_read` + `dispatch_request`), and `crates/brokerd/tests/uds_ipc.rs` (2 `run_broker_server` calls, Linux-gated with `#[cfg(target_os = "linux")]` so invisible to a macOS build but would break Linux CI).
- **Fix:** Updated all three to the new signatures (`&mut SessionStatus` argument for `dispatch_request`; `SessionStatus::Active` argument for `run_broker_server`; widened destructuring for `mint_from_read`), consistent with the plan's own stated goal ("brokerd compiles atomically ... else the crate does not build").
- **Files modified:** `crates/brokerd/tests/proto_claims.rs`, `crates/brokerd/tests/durable_anchor.rs`, `crates/brokerd/tests/uds_ipc.rs`.
- **Verification:** `cargo test -p brokerd --no-fail-fast` compiles and passes (uds_ipc.rs contributes 0 tests on macOS, as expected — Linux-only).
- **Committed in:** `16b9973`.

---

**Total deviations:** 2 auto-fixed (1 bug fix — DAG fork, 1 blocking-issue fix — additional call sites)
**Impact on plan:** Both fixes were necessary for `cargo test -p brokerd --no-fail-fast` to pass at all; neither changes the plan's security-relevant design (I1 demotion mechanism, atomicity, causal-edge requirement, broker-owned trust-state sourcing all implemented exactly as specified). No scope creep beyond what compiling/passing required.

## Issues Encountered

None beyond the DAG-fork bug documented above (which is the substantive finding of this plan's execution).

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. No hardcoded empty/placeholder values were introduced.

## Threat Flags

None. This plan's changes are exactly the mitigations named in its own `<threat_model>` (T-09-06 through T-09-09) — no new, undocumented security-relevant surface was introduced. The `session_created` actor-field provenance encoding is an audit-legibility addition, not a new trust boundary.

## Next Phase Readiness

- `brokerd` compiles and all 52 tests pass. `crates/executor` and `crates/runtime-core` unaffected and green.
- `cli/caprun` (`cli/caprun/src/main.rs`) fails to compile at its `create_session(intent_id)` and `run_broker_server(...)` call sites — both now require an additional argument (`SeedProvenance`, `SessionStatus` respectively). This is the exact, expected breaking-signature ripple this plan's own `<verification>` section names; Plan 09-04 must update both call sites and add the CLI's file-derived-intent on-ramp (`--seed-from-file` or equivalent) to actually exercise `SeedProvenance::FileDerived`.
- No blockers.

## Self-Check: PASSED
- FOUND: crates/brokerd/src/session.rs
- FOUND: crates/brokerd/src/quarantine.rs
- FOUND: crates/brokerd/src/server.rs
- FOUND: crates/brokerd/src/lib.rs
- FOUND: crates/brokerd/tests/phase5_dispatch.rs
- FOUND: crates/brokerd/tests/s9_acceptance.rs
- FOUND: crates/brokerd/tests/durable_anchor.rs
- FOUND: crates/brokerd/tests/proto_claims.rs
- FOUND: crates/brokerd/tests/uds_ipc.rs
- FOUND commit b54aade
- FOUND commit e234fe4
- FOUND commit 16b9973

---
*Phase: 09-session-trust-state-i1-i0*
*Completed: 2026-07-07*
