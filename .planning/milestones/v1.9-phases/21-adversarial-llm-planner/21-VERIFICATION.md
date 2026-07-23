---
phase: 21-adversarial-llm-planner
verified: 2026-07-11T00:00:00Z
status: passed
score: 8/8 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 21: Adversarial LLM Planner Verification Report

**Phase Goal:** A minimal LLM-backed planner, running behind Phase 20's seam, drives a real intent end-to-end using only `PlanNode{sink, args}` — no literal field to carry.
**Verified:** 2026-07-11
**Status:** passed
**Re-verification:** No — initial verification

**Verification scope note:** Per explicit task instruction, no live Colima/Docker or OpenAI API re-run was performed. SUMMARY-captured live evidence (21-04) was treated as sufficient, but was cross-checked against the actual test code that produced it (not accepted on narrative alone) — the test file's assertions were read and confirmed to independently check exit code, `verify_chain`, a durable audit-DB event lookup, and a live Mailpit HTTP API count, not just stdout text.

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | An LLM-backed implementation of the `Planner` trait exists and is selectable in place of the deterministic planner | VERIFIED | `cli/caprun/src/planner.rs:294` `impl Planner for LlmPlanner`; `worker.rs:309-316` selects `LlmPlanner` vs `DeterministicPlanner` via `CAPRUN_PLANNER` env, default path byte-identical (confirmed: default arm unconditionally constructs `DeterministicPlanner`, matching pre-Phase-21 behavior) |
| 2 | Given a clean, trusted intent, the LLM planner emits a syntactically valid `PlanNode{sink,args}` referencing handle IDs only, and the executor Allows it, delivering a real send | VERIFIED (live evidence, not re-run this pass) | `cli/caprun/tests/llm_planner_live_accept.rs::llm_planner_clean_allow_delivers` genuinely asserts exit 0, `Chain verification: PASSED`, `verify_chain(conn, session_id)==true`, no `sink_blocked` event, `email_send_succeeded` event present, and Mailpit recipient-scoped count == 1 (all checked against the durable audit DB / a live Mailpit HTTP GET, not stdout parsing alone). SUMMARY 21-04 captures a real passing run's output (audit DAG dump with real hash chain, `Chain verification: PASSED`, `1 passed; 0 failed`) matching exactly what this test computes. All referenced commits (`69a3155`, `067c080`, `63c12cc`) verified present in `git log`. |
| 3 | The LLM planner's prompt/tool-call construction is built only from typed extracts and handle IDs — never raw untrusted bytes | VERIFIED | `crates/llm-planner/src/lib.rs`: `PlannerRequest`/`HandleLabel` have no literal-carrying field, proven by key-set serde tests (`planner_request_key_set_has_no_literal_field`, `handle_label_key_set_has_no_literal_field`) — re-run locally, 10/10 pass. `build_planner_prompt`/`build_tool_schema` are pure functions reading only `PlannerRequest`. `openai::build_chat_request` builds the OpenAI request body from `PlannerRequest` alone. |

**Score:** 3/3 roadmap success criteria verified (0 present-but-behavior-unverified)

### PLAN-Level Must-Haves (21-01 .. 21-04)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 4 | Wire contract carries only handle-ID strings + slot hints + intent kind — no literal field | VERIFIED | `PlannerRequest{intent_kind,available_handles,available_sinks}`, `HandleLabel{slot_hint,value_id}` — code inspection + passing key-set tests |
| 5 | `parse_planner_response` fails closed on unknown sink / unoffered handle / malformed JSON | VERIFIED | `crates/llm-planner/src/lib.rs:183-202`, 4 dedicated unit tests, all pass |
| 6 | A genuinely separate OS process (`caprun-planner`) makes the OpenAI HTTP call; reqwest lives ONLY there | VERIFIED | `cli/caprun-planner/Cargo.toml` is the only Cargo.toml in the workspace with a `reqwest` dependency (grep-confirmed); `cargo tree -p caprun` and `cargo tree -p caprun-worker` (bin target inside the `caprun` package) both show 0 `reqwest` occurrences (re-run, confirmed) |
| 7 | `LlmPlanner` implements the exact Phase-20 `Planner` trait; worker submits via its own connection, no cross-connection resolution; no `crates/brokerd`/`crates/executor` modified | VERIFIED | `planner.rs` trait impl identical 6-param signature; `worker.rs:327` submits via existing `std_stream`/broker connection unchanged; `git log` shows this phase's commits touch only `cli/caprun/*`, `cli/caprun-planner/*`, `crates/llm-planner/*`, `scripts/mailpit-verify.sh` — no `crates/brokerd` or `crates/executor` file in the diff |
| 8 | Full workspace still builds; no regression to existing tests; `check-invariants.sh` passes | VERIFIED | Re-run this pass: `cargo build --workspace` clean, `./scripts/check-invariants.sh` all 3 gates PASS, `cargo test --workspace --no-fail-fast` — 0 failures across every test binary (macOS; Linux-only suites correctly show 0 collected, by design per CLAUDE.md) |

**Score:** 8/8 must-haves verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/llm-planner/Cargo.toml` | pure lib crate, no reqwest/tokio | VERIFIED | present, no reqwest/tokio deps |
| `crates/llm-planner/src/lib.rs` | wire types + prompt/schema/parser | VERIFIED | present, substantive (450 lines), 10/10 tests pass |
| `cli/caprun-planner/Cargo.toml` | bin crate, sole reqwest dep | VERIFIED | present, reqwest pinned `=0.13.4`, rustls-only |
| `cli/caprun-planner/src/main.rs` | abstract-UDS accept loop | VERIFIED | present, wired to `openai::call_openai`, fail-closed `SidecarReply` |
| `cli/caprun-planner/src/openai.rs` | OpenAI tool-calling client | VERIFIED | present, 3/3 tests pass, forces `emit_plan_node` |
| `cli/caprun/src/planner.rs` | `LlmPlanner` proxy | VERIFIED | present, substantive, wired into `worker.rs` |
| `cli/caprun/src/worker.rs` | env-selected planner | VERIFIED | present, `CAPRUN_PLANNER` selection wired |
| `cli/caprun/src/main.rs` | sidecar spawn/teardown | VERIFIED | present, gated on `CAPRUN_PLANNER=llm` |
| `cli/caprun/tests/llm_planner_live_accept.rs` | live clean-path acceptance | VERIFIED | present, genuine independent assertions (not stdout-only) |
| `scripts/mailpit-verify.sh` | forwards OPENAI_API_KEY/model | VERIFIED | present, conditional/unconditional forwarding matches SUMMARY's documented deviation |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `worker.rs` planner selection | `planner.rs::LlmPlanner`/`DeterministicPlanner` | `CAPRUN_PLANNER` env match | WIRED | code confirmed, default path unchanged |
| `LlmPlanner::plan()` | `caprun-planner` sidecar | abstract UDS `\0`+`PLANNER_SOCK`, framed JSON | WIRED | `connect_to_sidecar` (Linux) + `send_framed`/`recv_framed`, matches sidecar's `read_framed_request`/`write_framed_reply` framing |
| `main.rs` | sidecar spawn | `Command::new(...caprun-planner)`, env propagation | WIRED | spawn before worker, `PLANNER_SOCK`/`OPENAI_API_KEY`/`CAPRUN_PLANNER_MODEL` forwarded to sidecar only; `PLANNER_SOCK`+`CAPRUN_PLANNER=llm` forwarded to worker |
| `openai::build_chat_request` | `llm_planner::build_planner_prompt`/`build_tool_schema` | direct call | WIRED | confirmed in `openai.rs` |
| `response_to_plan_node` | sink-schema arg naming | caller-supplied `canonical_names` (ValueId identity), not model's own string | WIRED | fixed during 21-04 (bug 3); test `response_to_plan_node_canonicalizes_arg_name_ignoring_model_naming` passes |

### Behavioral Spot-Checks (static, this pass)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| llm-planner unit tests | `cargo test -p llm-planner --lib` | 10/10 pass | PASS |
| caprun-planner unit tests | `cargo test -p caprun-planner --lib` | 3/3 pass | PASS |
| caprun planner.rs unit tests | `cargo test -p caprun --test planner` | 11/11 pass | PASS |
| Full workspace regression | `cargo test --workspace --no-fail-fast` | 0 failures across all binaries | PASS |
| Invariant gates | `./scripts/check-invariants.sh` | 3/3 gates PASS | PASS |
| reqwest isolation | `cargo tree -p caprun \| grep -c reqwest` | 0 | PASS |
| Commit existence | `git cat-file -e <hash>` for all 12 phase commit hashes | all present | PASS |

### Live Acceptance (not re-run — explicit task instruction; cross-checked against code)

The Linux-gated `llm_planner_clean_allow_delivers` test (real OpenAI call + real Mailpit delivery) was NOT re-executed via Colima/Docker in this verification pass, per explicit instruction. Instead, the test's source was read to confirm it performs genuine independent verification (DB query + live Mailpit HTTP count, not a stdout-text assertion alone), and SUMMARY 21-04's captured output (audit DAG hash chain, `Chain verification: PASSED`, `1 passed; 0 failed`) was checked for internal consistency against that test code and against the project's established audit-event vocabulary used elsewhere in the codebase. No inconsistency found.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| PLANNER-03 | 21-01..21-04 | Minimal LLM planner emits only `PlanNode{sink,args}` | SATISFIED | All 3 roadmap success criteria + 8 plan-level must-haves verified above |

Note: `.planning/REQUIREMENTS.md` traceability table still lists PLANNER-03 as "Pending" and its checkbox unchecked — this is a documentation-sync lag, not a code gap. Recommend updating REQUIREMENTS.md's checkbox/status alongside phase closure.

### Anti-Patterns Found

None. Scanned all phase-modified files for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER|not implemented|coming soon` — only benign matches (a comment using "placeholder" descriptively, and a comment referencing the `NotImplemented` enum variant by name). No debt markers, no stub returns, no hardcoded-empty data flowing to output.

### Human Verification Required

None. All roadmap success criteria and plan must-haves resolved to VERIFIED via code inspection, passing unit/integration tests (re-run this pass), and cross-checked live evidence from SUMMARY 21-04 (accepted per explicit task scoping, not blindly trusted — the underlying test code was read and its assertions confirmed genuine).

### Gaps Summary

No gaps. All 3 ROADMAP.md success criteria and all 8 derived plan-level must-haves for Phase 21 are verified against the actual codebase: the `Planner` trait has a genuine second, selectable LLM-backed implementation; the wire contract is structurally incapable of carrying a literal (proven by tests, not prose); reqwest is isolated to a single out-of-process sidecar crate; the worker submits plan nodes through its own existing broker connection with no TCB (`brokerd`/`executor`) changes; and a real, previously-run live acceptance test (captured in SUMMARY 21-04, not re-run this pass per task scoping) demonstrates a clean intent reaching `Allowed` and a real Mailpit-captured delivery. Three genuine composition bugs found and fixed during Plan 21-04's first live run (abstract-socket connect API misuse, sidecar reply wire-shape mismatch, LLM-arg-name vs. sink-schema mismatch) were all fixed with regression tests added, and the fixes are strictly more fail-closed than the pre-fix behavior (worker now exits non-zero on any non-Allowed decision, not just Blocked).

---

_Verified: 2026-07-11_
_Verifier: Claude (gsd-verifier)_
