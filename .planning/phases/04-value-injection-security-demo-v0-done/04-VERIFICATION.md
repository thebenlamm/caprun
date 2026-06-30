---
phase: 04-value-injection-security-demo-v0-done
verified: 2026-06-30T00:00:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 4: Value Injection Security Demo (v0 DONE) Verification Report

**Phase Goal:** Prove the core value. A quarantined reader emits genuinely-tainted typed extracts; a deterministic non-LLM executor walks the PlanNode DAG with I2 hardcoded; a scripted plan flows a tainted value into a mediated sink's sensitive argument and is blocked with literal-value confirmation. The §9 integration test passing — with a genuine, audited taint chain — IS v0 DONE.

**Verified:** 2026-06-30
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A quarantined reader reads hostile input and emits a schema-valid typed ValueNode (Claim) whose taint originates from the read Event (never hand-set); the planner never sees the raw sentence. | VERIFIED | `extract_email_claims` (quarantine.rs) returns only `Claim { claim_type, value }` — no raw sentence field. `PlanArg` carries only `name` + `value_id` (no literal, no taint). `mint_from_read` sets taint at read time and anchors `provenance_chain[0]` to that Event id. Tests `extract_is_lossy_no_raw_sentence_in_claim` and `mint_from_read_anchor_identity` pass. |
| 2 | The deterministic non-LLM executor stub walks the PlanNode DAG with I2 hardcoded, propagating taint monotonically through edges. | VERIFIED | `executor::submit_plan_node` (executor/src/lib.rs) is a pure Rust function with hardcoded `is_routing_sensitive` map. 0 LLM calls. Anti-stapling: `grep ... ValueStore::mint` → 0; `grep ... ValueRecord {` → 0. 6 integration tests pass (Block/Allowed/Denied/cc/bcc/content cases). |
| 3 | A scripted plan (no LLM) flows the tainted ValueNode into a mediated sink stub's sensitive `to` argument; the executor sees it tainted (external.untrusted) → blocks, and surfaces a literal-value confirmation prompt for the exact address. | VERIFIED | `s9_acceptance` test builds `PlanNode { sink: SinkId("email.send"), args: [PlanArg { name: "to", value_id }] }` with opaque handle only. Executor returns `BlockedPendingConfirmation { literal_value: "accounts@ev1l.com", ... }`. `build_confirmation_prompt` produces `raw_recipient == "accounts@ev1l.com"`. Both assertions pass. |
| 4 | The §9 integration test passes end-to-end, the audit DAG shows an unbroken taint edge from the raw-read Event to the blocked sink argument, and a stapled-taint implementation would fail the test. | VERIFIED | `cargo test -p brokerd --test s9_acceptance` → `test s9_acceptance ... ok`. `provenance_chain[0]` appears 9 times in the test. Two-sided backstop: `provenance_chain[0] == read_event_id` AND `file_read_event.id == provenance_chain[0]`. `store.mint` appears 0 times in non-comment test lines. `verify_chain` returns true. Full workspace: 51 tests, 0 failures. |

**Score:** 4/4 truths verified

---

## Critical — Genuine-Taint Backstop (Non-Negotiable v0 Gate)

**Sole mint site confirmed:** `grep -rn "\.mint(" crates/brokerd/src/` returns exactly one hit: `quarantine.rs:156`. No other brokerd source path calls `ValueStore::mint` with a non-empty taint vector.

**Anti-stapling in test confirmed:** `grep -v comment | grep -c 'store\.mint'` in `s9_acceptance.rs` → 0. Taint is set exclusively by production `mint_from_read`, never by the test.

**Two-sided DAG backstop:** A fabricated UUID as `read_event_id` would pass `provenance_chain[0] == read_event_id` but fail `file_read_event.id == provenance_chain[0]` (the DAG query returns a real event). A stapled-taint implementation that sets `provenance_chain` at the sink would fail both checks. The test is a genuine backstop, not a tautology.

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/runtime-core/src/plan_node.rs` | ValueId, PlanArg (no literal/taint), PlanNode.args: Vec<PlanArg>, Provenance.provenance_chain | VERIFIED | `PlanArg { name: String, value_id: ValueId }` — confirmed no literal/taint fields. `args: Vec<PlanArg>`. `provenance_chain: Vec<uuid::Uuid>` on Provenance. |
| `crates/runtime-core/src/value_record.rs` | Broker-owned ValueRecord { id, literal, taint, provenance_chain } | VERIFIED | File exists; `grep -c "pub struct ValueRecord"` → 1. Re-exported from lib.rs. |
| `crates/runtime-core/src/executor_decision.rs` | BlockedPendingConfirmation with taint + provenance_chain fields | VERIFIED | Variant carries `taint: Vec<TaintLabel>` and `provenance_chain: Vec<uuid::Uuid>`. |
| `crates/executor/src/value_store.rs` | ValueStore::mint (sole taint writer) + resolve | VERIFIED | `pub fn mint` (1) and `pub fn resolve` (1) confirmed. mint inserts record with provided taint; resolve is read-only. |
| `crates/executor/src/sink_sensitivity.rs` | is_routing_sensitive(sink, arg_name) hardcoded email.send map | VERIFIED | `grep -c "pub fn is_routing_sensitive"` → 1. Routing-sensitive: to/cc/bcc. Content-sensitive: subject/body/attachment. No Cedar, no schema. |
| `crates/executor/src/lib.rs` | submit_plan_node deterministic decision function | VERIFIED | Pure Rust function. 0 mint refs, 0 ValueRecord constructions in non-comment lines. |
| `crates/brokerd/src/quarantine.rs` | extract_email_claims + mint_from_read genuine-taint anchor | VERIFIED | Both functions present. `mint_from_read` is the sole broker mint site. Appends file_read Event and calls `store.mint` with `provenance_chain=[event.id]` in one call. |
| `crates/brokerd/src/sinks/email_send.rs` | invoke_email_send_stub recording audit event, no real send | VERIFIED | Function present; appends `email_send_stub` Event; no SMTP/network call. |
| `crates/brokerd/src/approval.rs` | build_confirmation_prompt producing literal-value prompt payload | VERIFIED | Function present; `raw_recipient == literal_value` (byte-exact); domain extracted; known_contact false. |
| `crates/brokerd/src/audit.rs` | query_events_by_session + find_event_by_type with taint preserved | VERIFIED | Both functions present; taint round-trip confirmed by unit test. |
| `crates/brokerd/src/proto.rs` | SubmitPlanNode request + PlanNodeDecision response variants | VERIFIED | Both variants present; `grep -c "SubmitPlanNode"` and `grep -c "PlanNodeDecision"` ≥ 1. |
| `crates/brokerd/src/server.rs` | SubmitPlanNode dispatch arm calling executor::submit_plan_node | VERIFIED | `grep -c "SubmitPlanNode" server.rs` → 2 (variant match + call). Arm calls `executor::submit_plan_node` against Arc<Mutex<ValueStore>>. |
| `crates/brokerd/Cargo.toml` | executor path dependency | VERIFIED | `executor = { path = "../executor" }` present (both [dependencies] and [dev-dependencies]). |
| `crates/brokerd/tests/s9_acceptance.rs` | §9 acceptance test with genuine-taint backstop | VERIFIED | Test present; passes; genuine-taint backstop (provenance_chain[0]) present 9 times; no store.mint in non-comment lines. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| PlanArg | broker ValueRecord | opaque ValueId handle only (no literal, no taint on planner side) | VERIFIED | `PlanArg { name, value_id }` — confirmed no literal/taint fields. Taint-stripping (T-04-02) structurally impossible. |
| mint_from_read | file_read Event + ValueRecord | single function call, provenance_chain=[event.id] | VERIFIED | Both operations in one function. `provenance_chain[0]` equals the returned `event_id`. DAG query confirms event exists. |
| executor::submit_plan_node | ValueStore::resolve (read-only) | never calls mint or constructs ValueRecord | VERIFIED | Anti-stapling negative grep: 0 `ValueStore::mint`, 0 `ValueRecord {` in non-comment executor lib.rs lines. |
| brokerd SubmitPlanNode dispatch | executor::submit_plan_node | Arc<Mutex<ValueStore>> threading | VERIFIED | `grep -c "executor::submit_plan_node" brokerd/src/lib.rs` → 2 (delegation, not NotImplemented). No raw bypass. |
| s9_acceptance test | production code only | no re-implementation of taint logic in test | VERIFIED | Test imports and calls `extract_email_claims`, `mint_from_read`, `executor::submit_plan_node`, `build_confirmation_prompt`, `find_event_by_type`, `verify_chain` — all production paths. |

---

### Data-Flow Trace (Level 4)

| Data Path | Source | Produces Real Data | Status |
|-----------|--------|--------------------|--------|
| hostile content → Claim.value | `extract_email_claims` (hand-rolled scanner) | Yes — regex over real input | FLOWING |
| Claim → ValueRecord (literal + taint + chain) | `mint_from_read` → `store.mint` | Yes — minted from claim.value, real taint, real event.id | FLOWING |
| ValueId → BlockedPendingConfirmation fields | `value_store.resolve(arg.value_id)` | Yes — copied verbatim from minted record | FLOWING |
| BlockedPendingConfirmation → confirmation prompt | `build_confirmation_prompt(literal_value, taint, read_event_id)` | Yes — raw_recipient == literal_value | FLOWING |
| provenance_chain[0] → audit DAG | `find_event_by_type("file_read")` | Yes — event.id matches chain anchor | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| §9 acceptance test end-to-end | `cargo test -p brokerd --test s9_acceptance` | `test s9_acceptance ... ok` | PASS |
| Full workspace | `cargo test --workspace` | 51 passed, 0 failed, 0 ignored | PASS |
| Sole mint site in brokerd | `grep -rn "\.mint(" crates/brokerd/src/` | quarantine.rs:156 only | PASS |
| Anti-stapling in executor | `grep -v comment ... grep -c 'ValueStore::mint'` in lib.rs | 0 | PASS |
| Anti-stapling in test | `grep -v comment ... grep -c 'store\.mint'` in s9_acceptance.rs | 0 | PASS |
| Genuine-taint backstop present | `grep -c "provenance_chain\[0\]" s9_acceptance.rs` | 9 | PASS |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| REQ-quarantined-reader | 04-04 | Quarantined reader producing genuinely-tainted typed extracts | SATISFIED | `extract_email_claims` + `mint_from_read` in quarantine.rs. Taint originates at read Event, never hand-set. Tests pass. |
| REQ-executor-stub | 04-01, 04-02 | Deterministic I2 interpreter walking PlanNode DAG | SATISFIED | Handle-model types (01) + `submit_plan_node` + `ValueStore` (02). Hardcoded sensitivity map. 11 unit/integration tests pass. |
| REQ-mediated-sink-stub | 04-02, 04-03 | Mediated email.send sink with sensitive `to` argument | SATISFIED | `is_routing_sensitive` (executor) + `invoke_email_send_stub` (brokerd sinks). No send, audit Event recorded. |
| REQ-approval-hook | 04-03 | Approval hook surfacing literal value for confirmation | SATISFIED | `build_confirmation_prompt` returns `raw_recipient == exact literal`. Unit test confirms literal fidelity. |
| REQ-s9-acceptance-test | 04-05 | §9 end-to-end acceptance test — single v0 DONE gate | SATISFIED | `s9_acceptance` passes. Genuine-taint backstop (2-sided chain check) present and non-weakened. `verify_chain` true. |

**No orphaned requirements for Phase 4.** All 5 Phase 4 requirement IDs claimed in PLAN frontmatter are present in REQUIREMENTS.md and have implementation evidence.

---

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| crates/brokerd/src/server.rs | Known stub: SubmitPlanNode dispatch arm (pre-plan-04 state) was a stub returning `NotImplemented` | Resolved | Replaced by real executor delegation in plan 04 (commit 2dcedab). No remaining stub in the submit path. |
| crates/brokerd/src/lib.rs | `brokerd::submit_plan_node` was `NotImplemented` stub | Resolved | Now delegates to `executor::submit_plan_node`. `grep "NotImplemented" crates/brokerd/src/lib.rs` → no matches. |

No unresolved TBD/FIXME/XXX debt markers found in phase-modified files. No unreferenced markers.

---

### Human Verification Required

None. All success criteria are mechanically verifiable and confirmed by the test suite.

---

## Gaps Summary

No gaps. All 4 must-have truths are VERIFIED. The §9 acceptance test passes with a genuine, audited taint chain. The v0 DONE gate is closed.

---

_Verified: 2026-06-30_
_Verifier: Claude (gsd-verifier)_
