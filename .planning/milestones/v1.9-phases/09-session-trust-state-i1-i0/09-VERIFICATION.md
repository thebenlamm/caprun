---
phase: 09-session-trust-state-i1-i0
verified: 2026-07-07T02:54:21Z
status: passed
score: 6/6 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 9: Session Trust State (I1 Dynamic Demotion + I0 Creation Rule) Verification Report

**Phase Goal:** A session's trust state is mechanically tracked: reading untrusted content or being seeded from externally-derived content demotes/starts a session as draft-only, and draft-only sessions deterministically deny irreversible effects while still permitting reversible ones.
**Verified:** 2026-07-07T02:54:21Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | TAINT-01: A session is demoted to draft-only when `mint_from_read` taints a value inside it | VERIFIED | `crates/brokerd/src/quarantine.rs` Step 4 calls `update_session_status(conn, session_id, &SessionStatus::Draft)` under the SAME `conn`. Test `mint_from_read_demotes_session_to_draft` re-run individually: PASS. |
| 2 | TAINT-02: A draft-only session denies `CommitIrreversible` plan nodes that do not already Block on I2 | VERIFIED | `crates/executor/src/lib.rs` Step 0.5 (textually after the per-arg loop, before `Allowed`) returns `Denied { DraftOnlySessionDeniesCommitIrreversible { sink } }` for Draft+CommitIrreversible. Test `draft_session_denies_commit_irreversible`: PASS. |
| 3 | TAINT-03: A draft-only session still allows `MutateReversible`/`Observe` plan nodes | VERIFIED | Step 0.5's predicate is conjunctive (`Draft AND CommitIrreversible`); `#[cfg(any(test, feature="test-fixtures"))] test.observe` fixture drives the case end-to-end. Test `draft_session_allows_observe`: PASS. Confirmed the fixture is absent from a `--release` build (0 occurrences of `test.observe` string in `libexecutor.rlib`). |
| 4 | TAINT-04: Session demotion is recorded as an audit event with a causal edge to the triggering read event | VERIFIED | `session_demoted` Event built with `parent_id = Some(event_id)` where `event_id` is the just-appended `file_read` event. Test `mint_from_read_demotion_causal_edge` re-run individually: PASS. |
| 5 | ORIGIN-01: Session creation records a seed-provenance field; the CLI decides which at creation time | VERIFIED | `cli/caprun/src/main.rs` parses `--seed-from-file` before positional args, decides `SeedProvenance`, passes it to `create_session` (broker-owned), and records it in the `session_created` Event's `actor` field. Integration tests `file_derived_seed_starts_draft`/`trusted_arg_seed_starts_active`/`missing_seed_file_fails_closed`: all PASS. |
| 6 | ORIGIN-02: A session whose seed derives from external content starts draft-only and cannot auto-authorize Tier 3+ effects | VERIFIED | `brokerd::session::create_session` exhaustively matches `SeedProvenance` (`FileDerived → Draft`, `TrustedArg → Active`, no wildcard). Unit tests `create_session_file_derived_starts_draft`/`create_session_trusted_arg_starts_active` re-run individually: PASS. CLI-level `file_derived_seed_starts_draft` integration test confirms end-to-end. |

**Score:** 6/6 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/runtime-core/src/session.rs` | `SessionStatus::Draft`, `SeedProvenance{TrustedArg,FileDerived}` | VERIFIED | Both present; `Draft` inserted between `Active`/`WaitingApproval`; `SeedProvenance` has exactly 2 variants |
| `crates/runtime-core/src/executor_decision.rs` | `DenyReason::DraftOnlySessionDeniesCommitIrreversible{sink}` appended to ONE taxonomy | VERIFIED | Single variant appended; `code()`/`Display` both updated with explicit (non-wildcard) arms |
| `crates/executor/src/sink_sensitivity.rs` | `EffectClass` (3 variants) + `sink_effect_class` hardcoded, fail-closed | VERIFIED | Fail-closed `_ => CommitIrreversible`; internal `&str` match's `_` arm is explicitly permitted by DESIGN §10 (not an enum match) |
| `crates/executor/src/lib.rs` | Step 0.5 post-loop draft-only deny, 5th param `session_status` | VERIFIED | Confirmed by direct source read — Step 0.5 textually follows the closing `}` of the per-arg `for` loop and precedes the final `ExecutorDecision::Allowed` |
| `crates/executor/tests/executor_decision.rs` | 8 call sites updated + TAINT-02/03 tests | VERIFIED | 11 tests present incl. `draft_session_denies_commit_irreversible`, `draft_session_allows_observe`, `draft_session_tainted_routing_arg_still_blocks_not_denied` — all pass |
| `crates/brokerd/src/session.rs` | `update_session_status` UPDATE path; conditional `create_session` | VERIFIED | Exhaustive match, no wildcard; UPDATE (not INSERT) statement present |
| `crates/brokerd/src/quarantine.rs` | Atomic demotion in `mint_from_read`; `mint_from_intent` untouched | VERIFIED | Same `conn` param used for both the UPDATE and the Event append (no second lock); diff review of commit `e234fe4` shows zero changed lines inside `mint_from_intent`'s function body |
| `crates/brokerd/src/server.rs` | `session_status` threaded per-connection into `submit_plan_node` | VERIFIED | `&mut SessionStatus` local seeded from `initial_session_status`, set to `Draft` after `mint_from_read`, passed as `&*session_status` to `executor::submit_plan_node` — never read from `plan_node`/IPC |
| `cli/caprun/src/main.rs` | `--seed-from-file` on-ramp, fail-closed | VERIFIED | Missing/unreadable file → `anyhow` error via `.with_context`, never a silent fallback; confirmed by `missing_seed_file_fails_closed` test |
| `cli/caprun/tests/origin_seed_provenance.rs` | Integration tests for provenance→status mapping | VERIFIED | 3 tests exist and pass, spawning the real `caprun` binary |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `crates/executor/src/lib.rs` Step 0.5 | `sink_sensitivity::sink_effect_class` | `EffectClass::CommitIrreversible` predicate | WIRED | Confirmed by source read and passing tests |
| `crates/brokerd/src/quarantine.rs::mint_from_read` | `crates/brokerd/src/session.rs::update_session_status` | same `conn` param, no second lock | WIRED | Confirmed by source read; `mint_from_read_demotion_causal_edge`/`mint_from_read_demotes_session_to_draft` pass |
| `crates/brokerd/src/server.rs` `SubmitPlanNode` arm | `executor::submit_plan_node` | `&*session_status` 5th arg | WIRED | Confirmed by source read — comment explicitly states "NEVER read from plan_node/IPC" and code matches |
| `cli/caprun/src/main.rs` | `brokerd::session::create_session` / `run_broker_server` | `seed_provenance` in, `session.status` forwarded as `initial_session_status` | WIRED | Confirmed by source read |

### Behavioral Spot-Checks / Probe Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Step 0.5 placement (behavior-dependent: post-loop precedence) | `cargo test -p executor --test executor_decision draft_session_tainted_routing_arg_still_blocks_not_denied` | 1 passed | PASS |
| I1 atomic demotion + causal edge (behavior-dependent) | `cargo test -p brokerd --lib quarantine::tests::mint_from_read_demotion_causal_edge -- --exact` | 1 passed | PASS |
| I0 seed-provenance → status mapping | `cargo test -p brokerd --lib session::tests::create_session_file_derived_starts_draft -- --exact` | 1 passed | PASS |
| Full workspace green (re-confirmed independently, not trusting SUMMARY claim) | `cargo test --workspace --no-fail-fast` | 0 failed across all binaries | PASS |
| test.observe fixture never in production build | `cargo build -p executor --release` then `strings target/release/libexecutor.rlib \| grep -c test.observe` | 0 occurrences | PASS |
| Architectural invariant gates | `./scripts/check-invariants.sh` | both gates PASS | PASS |

No probe scripts (`scripts/*/tests/probe-*.sh`) exist or are referenced by this phase's plans/summaries — probe execution step N/A for this phase.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|--------------|--------|----------|
| TAINT-01 | 09-01 (types), 09-03 (behavior) | Session demoted on `mint_from_read` taint | SATISFIED | Code + passing tests, see Truth #1 |
| TAINT-02 | 09-01 (types), 09-02 (behavior) | Draft session denies `CommitIrreversible`, I2 precedence | SATISFIED | Code + passing tests, see Truth #2 |
| TAINT-03 | 09-02 | Draft session still allows `MutateReversible`/`Observe` | SATISFIED | Code + passing tests, see Truth #3 |
| TAINT-04 | 09-03 | Demotion audited with causal edge | SATISFIED | Code + passing tests, see Truth #4 |
| ORIGIN-01 | 09-01 (types), 09-04 (on-ramp) | Seed-provenance recorded, CLI decides | SATISFIED | Code + passing tests, see Truth #5 |
| ORIGIN-02 | 09-01 (types), 09-03/09-04 (behavior) | Externally-seeded session starts Draft | SATISFIED | Code + passing tests, see Truth #6 |

**Orphaned requirements check:** REQUIREMENTS.md maps exactly these 6 IDs (TAINT-01..04, ORIGIN-01..02) to Phase 9; all 6 appear in at least one plan's `requirements:` frontmatter field. No orphans.

**REQUIREMENTS.md staleness note (not a code gap):** REQUIREMENTS.md's own Traceability table (lines 71-76) still reads "Pending (types landed 09-01; behavior lands 09-0X)" for TAINT-01/02/ORIGIN-01/02, even though the top-of-file checkbox list already shows all six items `[x]` and this verification independently confirms the behavior is implemented and tested. This is a documentation-lag artifact of the traceability table not being refreshed after 09-02/03/04 executed — it does not indicate missing code. Recommend updating the table's Status column to "Complete" for all 6 rows as a docs housekeeping item, not a phase gap.

### Anti-Patterns Found

None. Scanned all 19 files touched across the phase's 4 plans (`crates/runtime-core/src/{session,executor_decision,lib}.rs`, `crates/executor/src/{sink_sensitivity,sink_schema,lib}.rs`, `crates/executor/tests/executor_decision.rs`, `crates/executor/Cargo.toml`, `crates/brokerd/src/{session,quarantine,server,lib}.rs`, `crates/brokerd/tests/{phase5_dispatch,s9_acceptance,durable_anchor,proto_claims,uds_ipc}.rs`, `cli/caprun/src/main.rs`, `cli/caprun/tests/origin_seed_provenance.rs`) for `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` markers — zero matches.

### Human Verification Required

None. All must-haves resolved programmatically with passing tests; no visual/UX/external-service surface in this phase.

### Gaps Summary

No gaps. All 6 observable truths (TAINT-01..04, ORIGIN-01..02) are verified against live source code, not SUMMARY.md prose:

- Step 0.5 in `crates/executor/src/lib.rs` is textually positioned after the per-arg I2 loop's closing brace and before the final `Allowed` return (the exact round-1 blocker B1 this phase had to avoid regressing) — confirmed by direct source read, not by trusting the plan/summary claim.
- `mint_from_read` in `crates/brokerd/src/quarantine.rs` performs the status UPDATE and the `session_demoted` Event append under the same `conn` parameter (no second lock acquisition), with `parent_id` set to the triggering `file_read` event's id — confirmed by direct source read.
- `mint_from_intent` is untouched — confirmed by diffing commit `e234fe4` and finding zero changed lines inside its function body (only doc-comment additions elsewhere in the file and a new non-regression test).
- All `SessionStatus`/`DenyReason`/`EffectClass` enum matches in the touched files are exhaustive with no `_` wildcard arm; the only `_` arms present (`sink_effect_class`'s internal `&str` match, `is_routing_sensitive`/`is_content_sensitive`) are matches over sink-name strings, explicitly permitted by DESIGN §10, not the enums themselves.
- `cargo test --workspace --no-fail-fast` was independently re-run in this verification session (not trusted from the orchestrator's prior claim) and is green: 0 failures across every test binary in the workspace.
- The DESIGN-GATE-RECORD-v1.2.md gate is APPROVED/UNBLOCKED (under the disclosed `DEC-ai-review-satisfies-human-gate` decision), so Phase 9's authorship of `crates/executor`/`crates/brokerd` code was authorized before it began.

The only non-code finding is the REQUIREMENTS.md traceability table staleness noted above — informational only, does not block phase completion.

---

*Verified: 2026-07-07T02:54:21Z*
*Verifier: Claude (gsd-verifier)*
