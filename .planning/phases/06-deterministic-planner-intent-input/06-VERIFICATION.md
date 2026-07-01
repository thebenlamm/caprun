---
phase: 06-deterministic-planner-intent-input
verified: 2026-06-30T12:00:00Z
status: passed
score: 19/19
behavior_unverified: 0
overrides_applied: 0
linux_live_gate:
  executed: true
  executed_by: orchestrator (Colima/Docker, --security-opt seccomp=unconfined, no --privileged)
  command: 'docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast'
  result: 'exit 0 — full workspace green; s9_live_clean_allow_path PASSED (Landlock FullyEnforced); e2e substrate_demo + dag_chain_integrity PASSED; no Phase 3/5 regressions'
  phase7_flag: 'caprun diagnostic prints "Chain verification: FAILED" on the clean run — full causal-chain/parent_id verify is a KNOWN Phase-5 deferral owned by Phase 7 SC7/ACC-05/ACC-07; not a Phase 6 success criterion. Phase 7 must resolve verify_chain over fd_granted → file_read → plan_node_evaluated.'
behavior_unverified_items:
  - truth: "A live `caprun send-email-summary <recipient> <clean-file>` run exits 0 (success)"
    test: "Run docker with --security-opt seccomp=unconfined and invoke: cargo test -p caprun --test s9_live_block s9_live_clean_allow_path"
    expected: "caprun exits 0; audit DB has intent_received + plan_node_evaluated; no sink_blocked; intent_received taint is empty"
    why_human: "s9_live_clean_allow_path is #[cfg(target_os = linux)]. On macOS cargo test runs 0 assertions. The full confinement stack (abstract UDS + Landlock + seccomp) is Linux-only. In-process analog clean_path_intent_value_evaluates_to_allowed PASSES on macOS and proves the code-path; the live gate requires Colima/Docker."
  - truth: "The run's audit DB contains an intent_received event AND a plan_node_evaluated event"
    test: "Covered by s9_live_clean_allow_path on Linux — see above"
    expected: "find_event_by_type returns intent_received and plan_node_evaluated rows"
    why_human: "Same Linux-gate as above"
  - truth: "The run's audit DB contains NO sink_blocked event"
    test: "Covered by s9_live_clean_allow_path on Linux — see above"
    expected: "find_event_by_type returns None for sink_blocked"
    why_human: "Same Linux-gate as above"
  - truth: "The intent_received event carries no taint (taint: [])"
    test: "Covered by s9_live_clean_allow_path on Linux — see above"
    expected: "intent_received.taint.is_empty() == true"
    why_human: "Same Linux-gate as above"
human_verification:
  - test: "Run the Linux clean allow-path live e2e gate"
    expected: "cargo test -p caprun --test s9_live_block (under Colima/Docker with --security-opt seccomp=unconfined) exits 0 with s9_live_clean_allow_path PASSED"
    why_human: "Abstract UDS + Landlock + seccomp confinement is Linux-only. The in-process proof passes on macOS. The live gate must run under Colima or equivalent. Command: docker run --rm --security-opt seccomp=unconfined -v \"$PWD\":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test s9_live_block"
---

# Phase 6: Deterministic Planner & Intent Input — Verification Report

**Phase Goal:** `caprun` accepts typed intents and a deterministic non-LLM planner translates them into plan nodes, with `mint_from_intent` enabling a clean allow-path that does not block at the executor.
**Verified:** 2026-06-30
**Status:** passed (all automated checks + the Linux live-confinement gate executed green; see `linux_live_gate` in frontmatter)
**Re-verification:** No — initial verification; Linux gate run by orchestrator after initial `human_needed` verdict

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | CaprunIntent::SendEmailSummary round-trips through serde_json unchanged | VERIFIED | `caprun_intent_serde_round_trip` passes; `#[serde(tag = "kind")]` confirmed in intent.rs |
| 2 | TaintLabel::UserTrusted.is_untrusted() == false and LocalWorkspace.is_untrusted() == false | VERIFIED | `is_untrusted_user_trusted_returns_false` and `is_untrusted_local_workspace_returns_false` pass in intent_taint.rs |
| 3 | ExternalUntrusted/EmailRaw/PdfRaw/LlmGenerated/WorkerExtracted .is_untrusted() == true | VERIFIED | 5 variant-specific truth-table tests pass in intent_taint.rs |
| 4 | Adding a new TaintLabel variant without updating is_untrusted() is a compile error | VERIFIED | `match self` in plan_node.rs line 38-45 has NO wildcard arm and NO `matches!` macro — confirmed by source read |
| 5 | A ValueRecord tainted [UserTrusted] in email.send/to → ExecutorDecision::Allowed | VERIFIED | `hard02_usertrusted_only_allows` PASSES using `vec![TaintLabel::UserTrusted]` (NOT empty) |
| 6 | A ValueRecord tainted [ExternalUntrusted, EmailRaw] in email.send/to → BlockedPendingConfirmation | VERIFIED | `hard02_externaltainted_still_blocks` PASSES |
| 7 | The UserTrusted-only allow test FAILS if predicate reverted (no vacuous HARD-02) | VERIFIED | Test mints `vec![TaintLabel::UserTrusted]` — would return Block under old `!record.taint.is_empty()` predicate; fix is provably load-bearing |
| 8 | mint_from_intent mints a [UserTrusted] ValueRecord AND appends intent_received event in one call | VERIFIED | `mint_from_intent_taint_on_record_empty_on_event` PASSES; record.taint = [UserTrusted], event.taint = [] |
| 9 | The minted record's provenance_chain[0] equals the intent_received event id (genuine anchor) | VERIFIED | `mint_from_intent_anchor_identity` PASSES; asserts `record.provenance_chain[0] == intent_event_id` AND event exists in audit DAG |
| 10 | The intent_received audit EVENT carries no taint (taint: []) | VERIFIED | Same test asserts `evt.taint.is_empty()` |
| 11 | ProvideIntent dispatch mints inside the per-connection ValueStore and returns IntentAccepted { value_id } | VERIFIED | `provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle` PASSES; dispatch arm in server.rs lines 359-382 confirmed |
| 12 | In-process clean path (mint_from_intent → email.send/to PlanNode) evaluates to Allowed | VERIFIED | `clean_path_intent_value_evaluates_to_allowed` in s9_acceptance.rs PASSES; also asserts intent_received + plan_node_evaluated in DAG, no sink_blocked |
| 13 | caprun parses `<intent-kind> <intent-param> <workspace-file> [audit-db-path]` and rejects unknown kinds | VERIFIED | main.rs lines 41-68 confirmed; e2e.rs uses new 4-arg CLI `send-email-summary demo@example.test <path> <path>` |
| 14 | plan_from_intent(SendEmailSummary, intent_vid, &[]) → PlanNode { sink email.send, args [PlanArg to = intent_vid] } | VERIFIED | `plan_from_intent_send_email_summary_maps_to_email_send` and `plan_from_intent_ignores_file_value_ids` PASS; planner.rs signature is pure (no ValueRecord, no taint) |
| 15 | Worker sends ProvideIntent AFTER apply_confinement and BEFORE RequestFd, uses returned handle in planned arg | VERIFIED | worker.rs ordering confirmed: apply_confinement (line 77) → ProvideIntent send (line 85) → IntentAccepted recv → RequestFd send (line 99); plan_from_intent called with intent_value_id (line 147) |
| 16 | Live `caprun send-email-summary <recipient> <clean-file>` run exits 0 | PRESENT_BEHAVIOR_UNVERIFIED | s9_live_clean_allow_path is #[cfg(target_os = "linux")] — macOS: 0-assertion no-op. Requires Colima/Docker. |
| 17 | Live audit DB contains intent_received AND plan_node_evaluated events | PRESENT_BEHAVIOR_UNVERIFIED | Same Linux gate |
| 18 | Live audit DB contains NO sink_blocked event | PRESENT_BEHAVIOR_UNVERIFIED | Same Linux gate |
| 19 | Live intent_received event carries empty taint | PRESENT_BEHAVIOR_UNVERIFIED | Same Linux gate |

**Score:** 15/19 truths VERIFIED (4 PRESENT_BEHAVIOR_UNVERIFIED — live Linux e2e, macOS gate expected)

---

### Integrity Check: §9 In-Process Hostile-Block Proof (CON-s9-taint-genuineness)

The `s9_acceptance` test in `crates/brokerd/tests/s9_acceptance.rs` is INTACT.

**Anti-stapling assertion preserved:** `assert_eq!(provenance_chain[0], read_event_id, "GENUINE-TAINT BACKSTOP...")` at line 156-161 — not weakened. The test still routes a genuinely-tainted `mint_from_read` value into `email.send/to`, asserts `BlockedPendingConfirmation`, and verifies the DAG event-id chain. `s9_acceptance` PASSES on macOS.

**New in-process clean-path sibling:** `clean_path_intent_value_evaluates_to_allowed` added alongside `s9_acceptance` — exercises the positive path without touching the existing hostile-block test.

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/runtime-core/src/intent.rs` | `pub enum CaprunIntent` with `SendEmailSummary { recipient: String }`, `#[serde(tag = "kind")]` | VERIFIED | 54 lines, substantive, re-exported from lib.rs |
| `crates/runtime-core/src/plan_node.rs` | `impl TaintLabel { pub fn is_untrusted() }` exhaustive match | VERIFIED | Lines 23-47, no wildcard arm, doc-comment states invariant |
| `crates/runtime-core/src/lib.rs` | Re-exports `CaprunIntent` | VERIFIED | Line 20: `pub use intent::{CaprunIntent, Intent, IntentStatus};` |
| `crates/runtime-core/tests/intent_taint.rs` | Serde round-trip + fail-closed + is_untrusted truth table | VERIFIED | 71 lines, 9 tests, all pass |
| `crates/executor/src/lib.rs` | Blocking predicate: `record.taint.iter().any(|t| t.is_untrusted())` | VERIFIED | Line 66 confirmed; not `!is_empty()` |
| `crates/executor/tests/executor_decision.rs` | HARD-02 allow + regression cases | VERIFIED | `hard02_usertrusted_only_allows` uses [UserTrusted] not []; `hard02_externaltainted_still_blocks` present |
| `crates/brokerd/src/quarantine.rs` | `pub fn mint_from_intent(...)` | VERIFIED | Lines 202-236, mirrors mint_from_read structure, event.taint=[], record.taint=[UserTrusted] |
| `crates/brokerd/src/proto.rs` | `BrokerRequest::ProvideIntent` + `BrokerResponse::IntentAccepted` | VERIFIED | Lines 43 and 88-90, placed before RequestFd in enum |
| `crates/brokerd/src/server.rs` | `ProvideIntent` dispatch arm | VERIFIED | Lines 359-382 in dispatch_request; calls mint_from_intent inside per-connection value_store; returns IntentAccepted |
| `cli/caprun/src/planner.rs` | `pub fn plan_from_intent(&CaprunIntent, ValueId, &[ValueId]) -> PlanNode` | VERIFIED | Pure, infallible, no I/O, no ValueRecord/taint access |
| `cli/caprun/src/main.rs` | Intent arg parsing + INTENT env var | VERIFIED | Lines 41-131; `.env("INTENT", serde_json::to_string(&intent)?)` confirmed |
| `cli/caprun/src/worker.rs` | ProvideIntent send/recv after confinement, planner call | VERIFIED | apply_confinement (77) → ProvideIntent (85) → IntentAccepted (93) → RequestFd (99) → plan_from_intent (147) |
| `cli/caprun/tests/planner.rs` | Unit test for plan_from_intent | VERIFIED | 3 tests pass |
| `cli/caprun/tests/s9_live_block.rs` | `s9_live_clean_allow_path` (Linux-gated) | VERIFIED (wired) | Present, #[cfg(target_os = "linux")], correct assertions; macOS body 0-assertion no-op as expected |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `cli/caprun/src/main.rs` | `cli/caprun/src/worker.rs` (subprocess) | `.env("INTENT", serde_json::to_string(&intent)?)` | WIRED | Line 131; worker reads `INTENT` env var at line 58 |
| `cli/caprun/src/worker.rs` | `crates/brokerd/src/server.rs` | `BrokerRequest::ProvideIntent` IPC after apply_confinement | WIRED | Ordering: confinement (77) → send ProvideIntent (85) → recv IntentAccepted (93) |
| `crates/brokerd/src/server.rs` | `crates/brokerd/src/quarantine.rs` | `mint_from_intent(...)` inside ProvideIntent arm | WIRED | Server imports `mint_from_intent` at line 43; calls it at line 374 inside the per-connection `value_store` |
| `crates/brokerd/src/quarantine.rs` | `crates/executor/src/value_store.rs` | `store.mint(literal, [UserTrusted], [event_id])` | WIRED | quarantine.rs line 233 |
| `cli/caprun/src/worker.rs` | `cli/caprun/src/planner.rs` | `crate::planner::plan_from_intent(&intent, intent_value_id, &value_ids)` | WIRED | Line 147 |
| `cli/caprun/src/planner.rs` | `crates/executor/src/lib.rs` (via broker submit) | `PlanNode → BrokerRequest::SubmitPlanNode` → `executor::submit_plan_node` | WIRED | Worker line 150; server.rs SubmitPlanNode arm line 325 |
| `crates/executor/src/lib.rs` | `crates/runtime-core/src/plan_node.rs` | `t.is_untrusted()` at line 66 | WIRED | Predicate calls `TaintLabel::is_untrusted()` imported from runtime-core |

---

### Behavioral Spot-Checks

| Behavior | Command / Test | Result | Status |
|----------|----------------|--------|--------|
| CaprunIntent serde round-trip | `cargo test -p runtime-core --test intent_taint` | 9 passed | PASS |
| HARD-02 allow case [UserTrusted] | `cargo test -p executor --test executor_decision hard02` | 2 passed | PASS |
| mint_from_intent anchor identity | `cargo test -p brokerd "quarantine::tests"` | all 7 pass | PASS |
| ProvideIntent dispatch wired | `cargo test -p brokerd provide_intent_dispatch` | PASS | PASS |
| In-process clean allow-path | `cargo test -p brokerd clean_path_intent_value_evaluates_to_allowed` | PASS | PASS |
| s9_acceptance intact | `cargo test -p brokerd s9_acceptance` | PASS | PASS |
| plan_from_intent pure mapping | `cargo test -p caprun --test planner` | 3 passed | PASS |
| Full workspace suite | `cargo test --workspace --no-fail-fast` | All PASS (0 failures); Linux e2e bodies: 0-count no-ops (expected) | PASS |
| Live clean allow-path e2e | `s9_live_clean_allow_path` on macOS | 0-assertion no-op (expected) | SKIP (Linux-only — needs Colima/Docker) |

---

### Probe Execution

No `probe-*.sh` scripts declared for Phase 6. Architectural invariant gate used instead.

| Gate | Command | Result | Status |
|------|---------|--------|--------|
| Gate 1: No `EffectRequest` token in crates/ | `./scripts/check-invariants.sh` | PASS | PASS |
| Gate 2: runtime-core purity (no I/O/async/network) | `./scripts/check-invariants.sh` | PASS | PASS |

---

### Requirements Coverage

| Requirement | Phase | Description | Status | Evidence |
|-------------|-------|-------------|--------|----------|
| PLAN-01 | Phase 6 | `caprun` accepts an intent input alongside the workspace | SATISFIED | main.rs lines 41-68; e2e.rs + planner test use new 4-arg CLI |
| PLAN-02 | Phase 6 | Deterministic non-LLM planner maps typed intent enum to `PlanNode{sink, args}` | SATISFIED | planner.rs `plan_from_intent`; tests pass |
| PLAN-03 | Phase 6 | Planner never sees raw bytes or taint labels — handles only | SATISFIED | Signature `(&CaprunIntent, ValueId, &[ValueId]) -> PlanNode`; `..` ignores recipient; no ValueRecord/taint param |
| PLAN-04 | Phase 6 | `mint_from_intent` mints trusted values anchored to `intent_received` event | SATISFIED | quarantine.rs mint_from_intent; anchor identity test passes; ProvideIntent dispatch wired |
| HARD-02 | Phase 6 | Executor blocking predicate over explicitly-untrusted labels; UserTrusted does NOT block | SATISFIED | executor/src/lib.rs line 66: `.any(|t| t.is_untrusted())`; hard02 tests pass with [UserTrusted] not [] |

All 5 requirements SATISFIED. REQUIREMENTS.md traceability confirmed: PLAN-01..04, HARD-02 all map to Phase 6 (marked Complete in traceability table at line 166-170).

---

### Anti-Patterns Found

No `TBD`, `FIXME`, or `XXX` markers found in any Phase 6 modified files. No placeholder or stub patterns detected. The executor, quarantine, planner, and CLI files are fully implemented.

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | — | — | — |

No anti-patterns found.

---

### Human Verification Required

#### 1. Linux Live Clean Allow-Path E2E (Plan 05 Gate)

**Test:** Run under Colima/Docker with Linux kernel ≥5.13 and `--security-opt seccomp=unconfined`:

```bash
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 \
  cargo test -p caprun --test s9_live_block
```

**Expected:** `s9_live_clean_allow_path` PASSES: caprun exits 0, audit DB contains `intent_received` (taint: []) and `plan_node_evaluated`, no `sink_blocked` event.

**Why human:** `s9_live_clean_allow_path` body is `#[cfg(target_os = "linux")]`. The confinement stack (abstract-namespace UDS bypass of Landlock, Landlock deny-all-fs, seccomp deny-execve) is Linux-only. macOS cargo test runs a 0-assertion no-op per the recorded intentional decision in PLAN 05. The in-process proof (`clean_path_intent_value_evaluates_to_allowed` in s9_acceptance.rs) passes on macOS and proves the code path; this live gate proves the confinement composability.

**Note:** The live hostile email block is NOT expected to pass in Phase 6 — this is a recorded ROADMAP handoff decision. `plan_from_intent` always routes `intent_value_id` (UserTrusted) to `email.send/to`, so the hostile-block live test was retired in Phase 6 and moves to Phase 7 (file.create path). The in-process `s9_acceptance` test remains as the hostile-block proof.

---

### Gaps Summary

No gaps. All 15 verifiable truths pass. The 4 PRESENT_BEHAVIOR_UNVERIFIED truths are all from Plan 05's Linux-only live e2e test — this is an expected, intentional design choice (macOS = 0-assertion no-op, documented in PLAN 05 and ROADMAP). The in-process analogs of all 4 assertions pass on macOS:

- Live run exits 0 → in-process: executor returns Allowed (VERIFIED)
- intent_received event in DB → in-process: find_event_by_type returns event (VERIFIED)
- no sink_blocked → in-process: find_event_by_type returns None (VERIFIED)
- intent_received.taint empty → in-process: evt.taint.is_empty() (VERIFIED)

The only outstanding action is the Colima/Docker Linux gate run.

---

_Verified: 2026-06-30T12:00:00Z_
_Verifier: Claude (gsd-verifier)_
