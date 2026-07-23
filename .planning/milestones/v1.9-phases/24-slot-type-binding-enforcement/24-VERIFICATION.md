---
phase: 24-slot-type-binding-enforcement
verified: 2026-07-11T00:00:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 24: Slot-Type Binding Enforcement Verification Report

**Phase Goal:** The executor structurally enforces that a resolved value's semantic origin role matches its plan-node slot's expected role, per Phase 23's design ruling — closing the v1.4 T2 residual (a misrouted `UserTrusted` handle is now caught even though it is neither untrusted nor a class-level deny).
**Verified:** 2026-07-11
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Every minted value carries a semantic origin-role tag; I0/I1 trust classification unaffected (SC1) | VERIFIED | `crates/runtime-core/src/value_record.rs:37` — `pub origin_role: Option<String>` with `#[serde(default)]`. `ValueStore::mint` (`value_store.rs:61-80`) threads a 4th param `origin_role` verbatim onto the record. All 3 broker mint sites populate it per DESIGN §2/§4/§9: `mint_from_read` reuses `claim.claim_type` verbatim (`quarantine.rs:369`); `mint_from_intent` takes a caller-supplied role (`quarantine.rs:452,481`); `mint_from_derivation` derives from `transform_kind`'s `"concat"` arm with a `inputs.len()==2` arity guard, never reading `inputs[*].origin_role` (`quarantine.rs:686-711`, confirmed no `inputs[` read of `.origin_role`). `server.rs`'s intent-variant match (`:1300-1313`) selects `primary_role` in the SAME arm as `primary_literal`, threaded to the shared `mint_from_intent` call (`:1330-1338`) — not hardcoded at the shared call site. `TaintLabel`/mint taint logic untouched — origin_role is a parallel field, never folded into taint. |
| 2 | Hardcoded per-sink-arg expected-role table exists in `crates/executor`, scoped to `email.send`/`file.create` (SC2) | VERIFIED | `crates/executor/src/sink_sensitivity.rs:147-162` — `expected_role(sink, arg_name) -> Option<&'static [&'static str]>`, hardcoded `match sink.0.as_str()` mirroring `is_routing_sensitive`/`is_content_sensitive`'s discipline. No config file, no framework. 18 unit tests pass (`cargo test -p executor --lib sink_sensitivity`, all 18 green, ran directly by verifier). |
| 3 | New exhaustive `DenyReason::SlotTypeMismatch` variant, no wildcard arm, all exhaustive matches updated (SC3) | VERIFIED | `crates/runtime-core/src/executor_decision.rs:73` — `SlotTypeMismatch { sink: String, arg: String, expected: Vec<String>, found: Option<String> }`, owned types (never `&'static`), serde-Deserializable. Both exhaustive matches (`code()` :93, `Display` :125-133) updated, neither carries a `_ =>` wildcard arm. `cargo build --workspace` exits 0 (verified directly), confirming no missed exhaustive match anywhere in the workspace (a miss would be a compile error under the no-wildcard discipline). Test `slot_type_mismatch_code_and_display` passes. |
| 4 | `submit_plan_node` hard-Denies a role↔slot mismatch (or no-role at a role-checked slot), per-arg, without weakening I0/I2 precedence (SC4) | VERIFIED | `crates/executor/src/lib.rs:111-146` — Step 1c inserted between the empty-provenance guard (1b) and the sensitivity check, matches `expected_role`'s `Option` explicitly (`if let Some(expected) = ...`, no `.unwrap_or(&[])` anywhere in `lib.rs` or `sink_sensitivity.rs`, confirmed by grep — 0 hits in both files). A `None` role (`record.origin_role.as_deref() => None => false`) or a role not in the list returns hard `Denied { SlotTypeMismatch }` immediately — never pushed into the `blocked` collect-then-Block vec, never `BlockedPendingConfirmation`. Sensitivity check (Step 2/3, I2) and Step 0.5 (I0 class-deny) remain textually unchanged and positionally after Step 1c. 4 new integration tests (`role_mismatch_denies`, `role_none_at_role_checked_slot_denies`, `matching_role_tainted_still_blocks`, `unconstrained_slot_unaffected`) run directly by verifier — all pass, confirming the precedence-preservation and fail-closed claims behaviorally, not just structurally. |

**Score:** 4/4 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/runtime-core/src/value_record.rs` | `origin_role: Option<String>` field, `#[serde(default)]` | VERIFIED | Confirmed at :37, preceded by `#[serde(default)]` |
| `crates/executor/src/value_store.rs` | `ValueStore::mint` 4th param | VERIFIED | :66, threaded into record literal :80 |
| `crates/brokerd/src/quarantine.rs` | 3 `mint_from_*` wrappers thread role | VERIFIED | mint_from_read :369, mint_from_intent :481, mint_from_derivation :686-734 |
| `crates/brokerd/src/server.rs` | `primary_role` selected inside intent-variant match | VERIFIED | :1300-1313 (match), :1337 (threaded to shared call) |
| `crates/runtime-core/src/executor_decision.rs` | `DenyReason::SlotTypeMismatch` + 2 exhaustive-match arms | VERIFIED | Variant :73; `code()` arm :93; `Display` arm :125-133 |
| `crates/executor/src/sink_sensitivity.rs` | `expected_role()` hardcoded table | VERIFIED | :147-162, pinned membership matches DESIGN §3 (with one documented deviation — see below) |
| `crates/executor/src/lib.rs` | Step 1c in `submit_plan_node` | VERIFIED | :111-146, between Step 1b (:105-109) and the `let sensitive` line (:154) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `ValueStore::mint` | `ValueRecord` | 4th param threaded into struct literal | WIRED | `value_store.rs:80` |
| `mint_from_intent` | `server.rs` dispatch | role selected in intent-variant match, not hardcoded at shared call | WIRED | `server.rs:1300-1313` → `:1337` |
| `mint_from_derivation` | `Concat` transform | role derived from `transform_kind`, never from `inputs[*].origin_role` | WIRED | `quarantine.rs:686-711`; grep confirms no `inputs[*].origin_role` read |
| `sink_sensitivity::expected_role` | `submit_plan_node` Step 1c | explicit `Option` match, no `.unwrap_or(&[])` | WIRED | `lib.rs:131-146`; grep confirms 0 `unwrap_or(&[])` hits in both files |
| Step 1c | `DenyReason::SlotTypeMismatch` construction | hard `Denied`, not collected into `blocked` vec | WIRED | `lib.rs:137-144` returns immediately, does not touch `blocked.push` |

### Behavioral Spot-Checks (run directly by verifier, not taken from SUMMARY)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `cargo build --workspace` | `cargo build --workspace` | exit 0 | PASS |
| `./scripts/check-invariants.sh` (Gates 1/2/3) | `./scripts/check-invariants.sh` | all 3 gates PASS | PASS |
| Executor decision tests incl. the 4 new Step 1c tests | `cargo test -p executor --test executor_decision` | 20/20 passed, incl. `role_mismatch_denies`, `role_none_at_role_checked_slot_denies`, `matching_role_tainted_still_blocks`, `unconstrained_slot_unaffected` | PASS |
| `expected_role()` table unit tests | `cargo test -p executor --lib sink_sensitivity` | 18/18 passed | PASS |
| Full Mac workspace regression | `cargo test --workspace --no-fail-fast` | exit 0; 46 `test result: ok` blocks, 0 `FAILED` (Linux-only security tests correctly report 0-passed by design, not a gap) | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| T2-02 | 24-01 | origin-role tag threaded through the 3 mint sites, additive, I0/I1 unaffected | SATISFIED | See Truth 1 above |
| T2-03 | 24-03 | hardcoded per-sink-arg expected-role table | SATISFIED | See Truth 2 above |
| T2-04 | 24-02 | exhaustive `DenyReason::SlotTypeMismatch`, no wildcard, both matches updated | SATISFIED | See Truth 3 above |
| T2-05 | 24-03 | `submit_plan_node` hard-Denies role↔slot mismatch, fail-closed, I0/I2 precedence preserved | SATISFIED | See Truth 4 above |

No orphaned requirements: REQUIREMENTS.md maps exactly T2-02..05 to Phase 24 and all 4 appear in the plans' `requirements:` frontmatter. T2-06/07/08 are correctly scoped to Phase 25 (out of scope here, per the task instructions) and are `[ ]` unchecked in REQUIREMENTS.md, consistent with Phase 25 not yet having run.

### Anti-Patterns Found

None. Grepped all 7 production files modified this phase (`value_record.rs`, `value_store.rs`, `quarantine.rs`, `server.rs`, `sink_sensitivity.rs`, `lib.rs`, `executor_decision.rs`) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER` — zero hits. No `.unwrap_or(&[])` in the two files where DESIGN forbids it (grep confirms 0). No new `#[ignore]` attributes. No stub returns (`return null`/`return {}`/empty-array stubs) — all changed logic is either data-threading (Plan 01) or the load-bearing enforcement branch (Plan 03), both exercised by passing tests.

### Known Deviation — Verified Explicitly (not rubber-stamped)

**Claim (SUMMARY 24-03):** `email.send`'s `body` expected-role list was changed from the DESIGN §3 pin `Some(["body"])` to `Some(["body","doc_fragment"])`.

**Verified directly against code, not accepted on SUMMARY's word:**

1. **(a) Recipient slots do NOT accept `doc_fragment` — the exfiltration-critical check.** `sink_sensitivity.rs:150`: `"to" | "cc" | "bcc" => Some(&["recipient", "email_address"])`. No `doc_fragment` anywhere in the `to`/`cc`/`bcc` arm. Confirmed by the passing unit test `email_send_to_cc_bcc_expect_recipient_or_email_address` and by direct code read. A `doc_fragment`-tagged (untrusted-extracted body) value routed into `to` still hard-Denies at Step 1c.
2. **(b) `body` is content-sensitive (I2-sensitive), satisfying DESIGN §3/F4's table-construction invariant.** `EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body"]` (`sink_sensitivity.rs:78`), consumed by `is_content_sensitive` (:102-107). So a tainted `doc_fragment`-tagged value at `body` still hits I2's per-arg Block (Step 2/3) regardless of role match — the role check is not the sole gate for this untrusted vocabulary, exactly as the invariant requires.
3. **(c) Step 1c is fail-closed.** `lib.rs:131-146`: `expected_role` is matched with `if let Some(expected)`, never `.unwrap_or`; inside, `record.origin_role.as_deref()` is matched explicitly — `Some(role) => expected.contains(&role)`, `None => false`. A `None` role at a role-checked slot and a role-not-in-list both fall through to `!role_ok` and return a hard `Denied { SlotTypeMismatch }` immediately — never pushed to the `blocked` vec, never `BlockedPendingConfirmation`. Confirmed by direct read and by the passing `role_none_at_role_checked_slot_denies` test.

**Traceability of the deviation's rationale, independently confirmed:** `cli/caprun/src/worker.rs`'s `SendEmailSummary` arm reports the `Body:` marker fragment as `WorkerClaim::DocFragment`; `server.rs`'s `ReportClaims` dispatch assigns `claim_type: "doc_fragment"`; `mint_from_read` reuses `claim_type` verbatim as `origin_role` (`quarantine.rs:369`). There is no separate `"body"` claim_type anywhere in the codebase (`quarantine.rs:315-341` enumerates exactly `email_address`/`relative_path`/`doc_fragment`). The DESIGN-literal table would therefore hard-Deny every real hostile-body-content flow instead of reaching I2's existing Block — the deviation is a genuine bug-fix against a stale DESIGN pin, not a weakening. This is an intentional, safety-preserving, well-documented deviation from DESIGN §3's literal text; it does not require an override (it strengthens correctness, does not reduce a must-have) but the deviation is flagged here per the task's explicit instruction and should still be folded back into `DESIGN-slot-type-binding.md` §3 in a follow-up doc pass (SUMMARY 24-03 already flags this; not yet done as of this verification — a documentation-only gap, not a code gap, and does not block Phase 24's goal).

### Data-Flow Trace (Level 4)

Not applicable — Phase 24 has no rendering/dashboard artifacts; all deliverables are Rust TCB logic verified via direct code read + passing tests above.

### Probe Execution

Not applicable — no `scripts/*/tests/probe-*.sh` conventions apply to this phase; `./scripts/check-invariants.sh` (the phase's own gate script) was run directly and PASSED (see Behavioral Spot-Checks).

### Human Verification Required

None. All 4 truths are structural/deterministic Rust logic, independently re-derivable via direct code read and directly-run tests — no visual, real-time, or external-service behavior involved.

### Gaps Summary

No gaps. All 4 Phase 24 success criteria (T2-02..05) are verified by direct code inspection (every cited file:line was read by the verifier, not taken from SUMMARY.md) and by directly re-running the test suite (not trusting SUMMARY's reported pass/fail): `cargo build --workspace` (exit 0), `./scripts/check-invariants.sh` (3/3 gates PASS), `cargo test -p executor --test executor_decision` (20/20 pass, including all 4 new Step 1c tests), `cargo test -p executor --lib sink_sensitivity` (18/18 pass), `cargo test --workspace --no-fail-fast` (exit 0, 46 test-result-ok blocks, 0 FAILED). The one documented deviation (body's expected-role list) was independently verified against live code (not just accepted from the SUMMARY narrative) and confirmed safe under DESIGN §3/F4's own invariant. T2-06/07/08 (Phase 25) are correctly out of scope and untouched.

One non-blocking documentation debt: `planning-docs/DESIGN-slot-type-binding.md` §3's `body` row still reads `Some(["body"])` in the doc text — the code has diverged from the doc (correctly, per the traced rationale) but the doc has not yet been amended to match. This is tracked informally in SUMMARY 24-03 but has no REQUIREMENTS.md ID and is not a gap against T2-02..05. Recommend folding a doc-sync line item into Phase 25.

---

_Verified: 2026-07-11_
_Verifier: Claude (gsd-verifier)_
