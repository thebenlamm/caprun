---
phase: 22-adversarial-gate-proof-residual-disclosure
plan: 01
subsystem: security
tags: [llm-planner, prompt-injection, taint, i2, executor, planner-seam, gate-01, gate-04]

requires:
  - phase: 21-adversarial-llm-planner
    provides: llm-planner pure wire types, caprun-planner OpenAI sidecar, worker-side LlmPlanner proxy behind the Planner seam
provides:
  - PlannerRequest.task_instruction channel (Option<String>) carrying attacker-controlled instruction text, verbatim-emitted by build_planner_prompt, never a bindable handle
  - Two-handle recipient offering in build_planner_request for SendEmailSummary (trusted "operator_recipient" + tainted "document_address", both mapped to sink arg "to"), keyed solely on derived_recipient presence, decoupled from task_instruction
  - Worker-side extract_instruction_fragment (Instruction: marker) reporting a genuinely-tainted DocFragment via ReportClaims (mint_from_read), literal kept worker-side and threaded through Planner::plan
  - GATE-04 deterministic sentinel-leak unit test in crates/llm-planner/src/lib.rs
affects: [22-02-composed-live-hard-gate-proof]

tech-stack:
  added: []
  patterns:
    - "Task-framing channel distinct from bindable handles: PlannerRequest carries ONE Option<String> task-framing field, never a per-handle literal — HandleLabel stays {slot_hint, value_id}"
    - "Two-candidate offering keyed on taint-source presence, decoupled from injection-marker presence, to make a later A/B/control live proof structurally possible"

key-files:
  created: []
  modified:
    - crates/llm-planner/src/lib.rs
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs
    - cli/caprun/tests/planner.rs
    - cli/caprun-planner/src/openai.rs

key-decisions:
  - "task_instruction is exempt from the 'no literal-carrying field' key-set test by design: it is task-framing text the model may read, never a value it can bind into a sink arg — the four-key set test documents and enforces this exemption."
  - "Two-handle offering is scoped to SendEmailSummary only (the plan's explicit 'to' sink-arg target); CreateFileFromReport's single-handle 'recipient'->'path' mapping is left unchanged, since GATE-01..04 targets email.send specifically."
  - "Instruction fragment extraction uses a marker ('Instruction:') distinct from Reply-To:/Domain:/Body:, so a document can carry recipient markers with no injection marker — the exact shape 22-02's control leg needs, requiring no new fixture constant."

requirements-completed: [GATE-01, GATE-04]

coverage:
  - id: D1
    description: "PlannerRequest.task_instruction field added; build_planner_prompt emits it verbatim in a delimited section when Some, byte-stable (no section) when None"
    requirement: "GATE-01"
    verification:
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#build_planner_prompt_emits_task_instruction_verbatim_when_some"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#build_planner_prompt_omits_instruction_section_when_none"
        status: pass
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#planner_request_key_set_has_no_literal_field"
        status: pass
    human_judgment: false
  - id: D2
    description: "build_planner_request offers both a trusted operator_recipient and a tainted document_address handle when derived_recipient is Some, both mapped to sink arg 'to'; decoupled from task_instruction presence"
    requirement: "GATE-01"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#build_planner_request_offers_both_recipient_candidates_with_distinct_hints"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#two_handle_offering_is_decoupled_from_task_instruction"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#response_to_plan_node_routes_tainted_document_address_into_to_when_model_picks_it"
        status: pass
    human_judgment: false
  - id: D3
    description: "Worker extracts an Instruction: marker-anchored fragment, reports it via ReportClaims (mint_from_read -> ExternalUntrusted), keeps the literal worker-side, and threads it through Planner::plan as task_instruction; DeterministicPlanner ignores it (byte-identical output)"
    requirement: "GATE-01"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#deterministic_planner_output_unchanged_when_task_instruction_threaded"
        status: pass
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#build_planner_request_threads_task_instruction_when_supplied"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gate 3: mint-site loci unaffected)"
        status: pass
    human_judgment: false
  - id: D4
    description: "GATE-04 deterministic construction-site sentinel-leak unit test: sentinel bytes from recipient/body fragments never appear in build_planner_prompt's output"
    requirement: "GATE-04"
    verification:
      - kind: unit
        ref: "crates/llm-planner/src/lib.rs#build_planner_prompt_never_leaks_sink_arg_literal_sentinels"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-11
status: complete
---

# Phase 22 Plan 01: task_instruction Injection Channel + GATE-04 Sentinel Test Summary

**Opened the genuine prompt-injection channel (`task_instruction`) from a hostile document to the LLM planner, made the injection load-bearing via a decoupled two-handle recipient offering, and added a deterministic sentinel-leak test proving sink-arg literals never enter the constructed prompt.**

## Performance

- **Duration:** ~15 min (commit-to-commit span; wall-clock session including read/design time was longer)
- **Completed:** 2026-07-11
- **Tasks:** 3/3 completed
- **Files modified:** 5

## Accomplishments

- `PlannerRequest.task_instruction: Option<String>` carries attacker-controlled instruction text as task framing — it is structurally never a bindable handle, so it can never be laundered into a sink-arg value. `build_planner_prompt` emits it verbatim in a delimited "Instructions from the source document:" section when `Some`, byte-stable (no section) when `None`.
- `build_planner_request` now offers the LLM a genuine trusted-vs-tainted choice for `SendEmailSummary` when a tainted `derived_recipient` exists: `operator_recipient` (trusted `intent_value_id`) and `document_address` (tainted `derived_recipient`), both mapped to the `email.send` `to` arg via `canonical_names`. This offering is keyed **solely** on `derived_recipient` being `Some`, independent of `task_instruction` — the structural decoupling Plan 22-02's control leg depends on.
- `worker.rs` extracts a genuinely-tainted instruction fragment from an `Instruction:` marker (distinct from `Reply-To:`/`Domain:`/`Body:`), reports it through the existing `ReportClaims` batch (`mint_from_read` → `ExternalUntrusted`, recorded in the audit DAG), and keeps the literal worker-side to thread through the `Planner::plan` seam — never resolving a broker `ValueId` back to a literal.
- Added the GATE-04 deterministic, non-network `#[test]` proving per-fragment sentinel bytes (mirroring `concat_doc_fragments`'s `"{local}@{domain}"` shape) never leak into `build_planner_prompt`'s output, replacing the retired context-dump grep.
- `DeterministicPlanner`'s output is provably byte-identical whether or not `task_instruction` is threaded (new unit test), and every existing `--test planner`/`--test e2e` assertion still passes.

## Task Commits

1. **Task 1: Add the task_instruction channel to PlannerRequest and emit it verbatim in build_planner_prompt** - `4af21f3` (feat) — also includes Task 3's GATE-04 sentinel test (same file, committed together; see Deviations)
2. **Task 2: Offer both recipient candidates + populate task_instruction from a genuinely-tainted worker extraction** - `41a417d` (feat)

**Plan metadata:** commit pending (this SUMMARY + STATE/ROADMAP updates are the orchestrator's responsibility per this plan's parallel-execution contract — this executor does not touch STATE.md/ROADMAP.md)

## Files Created/Modified

- `crates/llm-planner/src/lib.rs` — `PlannerRequest.task_instruction` field + doc-comment updates (module-level "structural literal-incapability" note sharpened); `build_planner_prompt` emits the instruction verbatim when `Some`; updated four-key-set test; new tests for verbatim emission, None-case byte-stability, and the GATE-04 sentinel-leak assertion.
- `cli/caprun/src/planner.rs` — `Planner::plan` trait method gains `task_instruction: Option<String>`; `DeterministicPlanner` ignores it; `LlmPlanner` threads it into `build_planner_request`, which now offers both recipient candidates for `SendEmailSummary` when `derived_recipient` is `Some` (decoupled from `task_instruction`), extends `canonical_names` accordingly, and carries `task_instruction` into the built `PlannerRequest`.
- `cli/caprun/src/worker.rs` — new `extract_instruction_fragment` helper (mirrors `extract_body_fragment`, marker `Instruction:`); the `SendEmailSummary` extraction arm now returns a triple `(derived_recipient, body, task_instruction)`; the instruction fragment is reported via the existing `ReportClaims` batch; the `planner.plan(...)` call site passes `task_instruction` through.
- `cli/caprun/tests/planner.rs` — updated the existing two-handle-offering test to the new 4-handle shape; added `build_planner_request_offers_both_recipient_candidates_with_distinct_hints`, `response_to_plan_node_routes_tainted_document_address_into_to_when_model_picks_it`, `build_planner_request_threads_task_instruction_when_supplied`, `two_handle_offering_is_decoupled_from_task_instruction`, and `deterministic_planner_output_unchanged_when_task_instruction_threaded`.
- `cli/caprun-planner/src/openai.rs` — test-fixture-only edit (see Deviations): `sample_request()`'s `PlannerRequest` literal gained `task_instruction: None`. Production code (`build_chat_request`) is untouched.

## Decisions Made

- Scoped the two-handle offering to `SendEmailSummary` only (`CreateFileFromReport`'s single-handle `"recipient"` → `"path"` mapping is unchanged) — Phase 22's success criteria and threat register target `email.send`'s `to` arg specifically; widening `CreateFileFromReport` was out of this plan's stated scope.
- Kept the instruction-fragment extraction worker-side-only, mirroring the existing recipient-concat pattern exactly: transform/keep worker-side, mint via `ReportClaims` for audit honesty only, never resolve the returned `ValueId` back to a literal.
- Chose `"Instruction:"` as the injection marker specifically because it is orthogonal to `Reply-To:`/`Domain:`/`Body:` — this means a "recipient markers present, no injection marker" fixture is already constructible from the existing marker vocabulary with zero new worker-side fixture constants, exactly as Plan 22-02 needs for its control leg.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - blocking compile fix] `cli/caprun-planner/src/openai.rs`'s test fixture required `task_instruction: None`**
- **Found during:** Task 1
- **Issue:** `PlannerRequest` has no `Default` impl, so adding a new field makes every existing struct-literal construction site (including this crate's own `#[cfg(test)]` fixture) exhaustiveness-fail to compile.
- **Fix:** Added `task_instruction: None` to `sample_request()`'s struct literal. No production code (`build_chat_request`, `extract_tool_arguments`, `call_openai`) was touched — the sidecar still "rides through unchanged" per the plan's own acceptance criterion; only the pre-existing test fixture needed the new field.
- **Files modified:** `cli/caprun-planner/src/openai.rs`
- **Verification:** `cargo build -p caprun-planner` and `cargo test -p caprun-planner` (fixture compiles and passes).
- **Committed in:** `4af21f3` (part of Task 1's commit)

**2. [Process note, not a Rule 1-4 deviation] Task 1 and Task 3 committed together**
- Task 3's GATE-04 sentinel test lives in the same file (`crates/llm-planner/src/lib.rs`) and section Task 1 was already editing (the `#[cfg(test)] mod tests` block). Rather than re-opening the same file for a near-immediately-following second commit, both were included in the Task 1 commit (`4af21f3`). Task 3's acceptance criteria were independently re-verified afterward (`cargo test -p llm-planner sentinel` → 1 passed) to confirm it stands on its own regardless of commit boundary.

---

**Total deviations:** 1 auto-fixed (Rule 3, compile-blocking), 1 process note (no functional impact).
**Impact on plan:** No scope creep; both deviations are structurally necessary/cosmetic, not behavioral.

## Issues Encountered

None. `cargo test -p caprun --lib` (part of the plan's own `<verify>` string) errors with "no library targets found in package `caprun`" — this is a pre-existing structural fact (the `caprun` crate has only `[[bin]]` targets, no `lib.rs`; `cli/caprun-planner/src/planner.rs`'s `mod planner` is included via `#[path]` in the binaries and in `tests/planner.rs`, never as a lib target), unrelated to and unchanged by this plan. Verified via `cargo test -p caprun --test planner --test e2e` instead (16 + 0 passed, the latter being Linux-gated), which are the suites that actually exercise this plan's changes.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

Ready for Plan 22-02 (composed live HARD GATE proof): the `task_instruction` channel exists end-to-end (worker extraction → `Planner::plan` seam → `build_planner_prompt`), the two-handle offering is proven decoupled from `task_instruction` presence (control-leg prerequisite satisfied), and GATE-04's deterministic sentinel assertion is in place. No blockers identified. `CAPRUN_PLANNER=llm` live runs (Plan 21-04's Mailpit-verified path) are unaffected — the clean path (no `Reply-To:`/`Domain:` markers) still offers a single trusted handle exactly as before.

---
*Phase: 22-adversarial-gate-proof-residual-disclosure*
*Completed: 2026-07-11*

## Self-Check: PASSED

All modified files (`crates/llm-planner/src/lib.rs`, `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs`, `cli/caprun/tests/planner.rs`, `cli/caprun-planner/src/openai.rs`) exist on disk. Both task commits (`4af21f3`, `41a417d`) verified present in `git log`.
