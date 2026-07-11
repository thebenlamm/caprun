---
phase: 21-adversarial-llm-planner
plan: 04
subsystem: planner
tags: [rust, live-acceptance, openai, mailpit, uds, planner-seam]

# Dependency graph
requires:
  - phase: 21-adversarial-llm-planner
    plan: 02
    provides: "The `cli/caprun-planner` OpenAI sidecar this plan's live test drives end-to-end for the first time."
  - phase: 21-adversarial-llm-planner
    plan: 03
    provides: "The worker-side `LlmPlanner` proxy (`cli/caprun/src/planner.rs`) this plan's live test exercises against the real sidecar for the first time."
provides:
  - "A genuine, captured-output live proof (Linux, real OpenAI call, real Mailpit delivery) that PLANNER-03's clean path works end-to-end: `cli/caprun/tests/llm_planner_live_accept.rs`."
  - "scripts/mailpit-verify.sh forwards OPENAI_API_KEY (unconditional-but-empty-tolerant) and CAPRUN_PLANNER_MODEL (conditional, only-if-set) into the Linux verification container."
  - "Three real, 100%-reproducible composition bugs between Plan 21-02's sidecar and Plan 21-03's worker-side proxy, found and fixed by actually running the composed path for the first time (never previously exercised — both plans built in parallel worktrees against an unverified wire contract)."
affects: ["22-adversarial-hostile-planner (the next phase's HARD GATE builds on this proven clean-path composition)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "std::os::linux::net::SocketAddrExt::from_abstract_name + UnixStream::connect_addr for a SYNCHRONOUS (non-tokio) abstract-namespace UDS client connect — the sanctioned std API for this, as opposed to a plain path-based connect() with a leading NUL byte (which unconditionally fails with InvalidInput on ALL platforms, not just macOS)."
    - "Never trust an untrusted planner's own naming of a resolved value — canonicalize sink-required arg names by value_id identity (caller-supplied mapping), never by the string the planner chose. Directly reusable in Phase 22's adversarial/hostile planner work."

key-files:
  created:
    - cli/caprun/tests/llm_planner_live_accept.rs
  modified:
    - scripts/mailpit-verify.sh
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs
    - cli/caprun/tests/planner.rs

key-decisions:
  - "Fixed the abstract-socket connect bug via std's stable (1.70+) SocketAddrExt::from_abstract_name + connect_addr rather than spinning up a throwaway tokio runtime inside the synchronous Planner::plan() trait method — simpler, no new runtime-lifecycle risk, and matches this codebase's existing 'Linux-only abstract socket, compiles-but-stubs-on-macOS' pattern used everywhere else."
  - "Fixed the arg-naming mismatch by having response_to_plan_node canonicalize PlanArg.name via a caller-supplied (ValueId -> canonical name) mapping, rather than editing crates/llm-planner's prompt text to instruct the model on exact arg names. This keeps the fix inside cli/caprun/src/planner.rs (already touched, non-TCB) and is structurally more robust: it never depends on the model complying with a naming instruction, consistent with this project's 'the planner only ever proposes; the caller/executor never trusts its literal strings' security posture ahead of Phase 22's adversarial planner work."
  - "Widened worker.rs's exit-code check from `matches!(decision, BlockedPendingConfirmation)` to `!matches!(decision, Allowed)` — a schema-Denied plan node (no effect ever ran) must not exit 0. This was unreachable via the pre-existing DeterministicPlanner (its hardcoded arg names always satisfy the schema) and only surfaced because Phase 21's LlmPlanner CAN produce a schema-invalid plan node."
  - "mailpit-verify.sh forwards CAPRUN_PLANNER_MODEL conditionally (only when set on the host) rather than unconditional-but-empty-tolerant like OPENAI_API_KEY — the plan's own Task 1 text said 'forwarded if set' for the model var specifically; an always-forwarded empty string silently overrode caprun-planner's own gpt-4o-mini default with an empty model name, which OpenAI rejects outright."

requirements-completed: [PLANNER-03]

coverage:
  - id: D1
    description: "A real OpenAI-backed LlmPlanner run (CAPRUN_PLANNER=llm) drives a clean, trusted send-email-summary intent to an Allowed decision and a real Mailpit-captured delivery — proven with captured live output, not asserted from code."
    requirement: "PLANNER-03"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/llm_planner_live_accept.rs#llm_planner_clean_allow_delivers (Linux-gated; run via `MAILPIT_VERIFY_CMD='cargo build -p caprun-planner && cargo test -p caprun --test llm_planner_live_accept llm_planner_clean_allow_delivers' bash scripts/mailpit-verify.sh`)"
        status: pass
    human_judgment: false
  - id: D2
    description: "macOS cross-platform guard: the live test file compiles and its non-Linux guard test passes, keeping `cargo test -p caprun` meaningful on the dev box."
    verification:
      - kind: unit
        ref: "cli/caprun/tests/llm_planner_live_accept.rs#llm_planner_live_accept_guard_binary_present"
        status: pass
    human_judgment: false
  - id: D3
    description: "response_to_plan_node canonicalizes the final PlanArg.name via a caller-supplied (ValueId, name) mapping, ignoring whatever name string the (simulated) model chose — the fix for the real live schema-rejection bug."
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#response_to_plan_node_canonicalizes_arg_name_ignoring_model_naming"
        status: pass
    human_judgment: false
  - id: D4
    description: "build_planner_request's canonical_names mapping assigns email.send's exact schema-required names (to/subject/body) to the recipient/subject/body slots."
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#build_planner_request_offers_recipient_subject_body_handles (extended with canonical_name_for assertions)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Full workspace still builds and all pre-existing tests still pass after the three bug fixes (no regression); ./scripts/check-invariants.sh passes; the diff touches only cli/caprun/* (non-TCB) and scripts/mailpit-verify.sh — no crates/brokerd or crates/executor changes."
    verification:
      - kind: other
        ref: "cargo test --workspace --no-fail-fast (macOS) — 0 failures"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh"
        status: pass
      - kind: other
        ref: "git diff --name-only touches only scripts/mailpit-verify.sh, cli/caprun/{src/planner.rs,src/worker.rs,tests/planner.rs,tests/llm_planner_live_accept.rs}"
        status: pass
    human_judgment: false

duration: ~2h (including three rounds of live-run diagnosis and fix)
completed: 2026-07-11
status: complete
---

# Phase 21 Plan 04: Live LLM-Planner Clean-Path Acceptance Summary

**A real OpenAI-backed `LlmPlanner` run (model `gpt-4o-mini`, `CAPRUN_PLANNER=llm`) now drives a clean, trusted `send-email-summary` intent through the real `cli/caprun-planner` sidecar to an Allowed decision and an actual SMTP delivery captured by Mailpit — proven with captured live output (`Chain verification: PASSED`, `email_send_succeeded`, Mailpit count == 1) on real Linux via `scripts/mailpit-verify.sh`, after finding and fixing three genuine, 100%-reproducible composition bugs between the sibling Wave-2 plans (21-02's sidecar, 21-03's worker-side proxy) that had never previously been run together.**

## Performance

- **Duration:** ~2h (2 tasks per plan + 3 rounds of live-run diagnosis/fix, each requiring a full Linux Docker verification cycle)
- **Completed:** 2026-07-11
- **Tasks:** 2 completed (Task 1: mailpit-verify.sh forwarding; Task 2: the live test file), plus 1 additional deviation-fix commit
- **Files modified:** 1 created (`cli/caprun/tests/llm_planner_live_accept.rs`), 4 modified (`scripts/mailpit-verify.sh`, `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs`, `cli/caprun/tests/planner.rs`)

## Accomplishments

- **Task 1:** `scripts/mailpit-verify.sh` now forwards `OPENAI_API_KEY` (unconditional-but-empty-tolerant — a plain non-LLM run still works with no key) and `CAPRUN_PLANNER_MODEL` (conditional, only when actually set on the host) into the `rust:1` verification container, so an in-container `caprun-planner` sidecar can reach `api.openai.com`.
- **Task 2:** Added `cli/caprun/tests/llm_planner_live_accept.rs` — a Linux-gated live acceptance test invoking the real `caprun` binary with `CAPRUN_PLANNER=llm` against a clean fixture (no `Reply-To:`/`Domain:` markers), asserting caprun exit 0, `Chain verification: PASSED`, an independent audit-DB check (`verify_chain`, no `sink_blocked`, `email_send_succeeded` present), and exactly one Mailpit-captured message for the run's own nonced recipient. Skips cleanly (no hard fail) when `OPENAI_API_KEY` is absent.
- **Ran the real composed path for the first time** (Plan 21-02's sidecar + Plan 21-03's worker-side proxy were built in parallel worktrees and had never been run together before this plan) and hit **three genuine, 100%-reproducible bugs** — none were timing races, each failed on the very first attempt every time:
  1. `connect_to_sidecar` used `std::os::unix::net::UnixStream::connect` on a leading-NUL abstract-socket path. Std's generic path-based `sockaddr_un` builder rejects ANY nul byte (including a leading one) with `InvalidInput` — a non-retryable error kind, so the connect-retry loop's guard never matched and every real invocation failed instantly. **Fixed** via the stable (Rust 1.70+) `std::os::linux::net::SocketAddrExt::from_abstract_name` + `UnixStream::connect_addr`, Linux-gated with a non-Linux compile-only stub sibling.
  2. `request_plan_from_sidecar` deserialized the sidecar's reply frame directly as a bare `PlannerResponse`, but `caprun-planner`'s `handle_connection` ALWAYS wraps replies in `{"status":"ok","response":{...}}` / `{"status":"error","message":"..."}` — a bare parse failed on every real reply (confirmed the real OpenAI call itself succeeded — the sidecar's own startup log line proved that — but the wire shape didn't match what Plan 21-03's SUMMARY had assumed). **Fixed** by adding a local `SidecarReply` mirror type and unwrapping it.
  3. `response_to_plan_node` copied the model's own `response_arg.name` verbatim into the final `PlanArg`, but `crates/executor/src/sink_schema.rs`'s hardcoded `email.send` schema requires the exact names `{"to","subject","body"}`, and nothing in `crates/llm-planner`'s prompt/tool-schema tells the model this (it only sees `slot_hint`s like `"recipient"`) — a real model named the recipient arg after its `slot_hint` instead of `"to"`, so the executor correctly `Denied(UnknownArg)` the plan node on every real run. **Fixed** by having `build_planner_request` additionally return a `canonical_names: Vec<(ValueId, String)>` mapping and `response_to_plan_node` look up the canonical name by `value_id` identity — never trusting the model's own naming, consistent with this project's "never trust the planner's literal strings" posture ahead of Phase 22.
  - **Compounding bug found alongside #3:** `worker.rs` only treated `ExecutorDecision::BlockedPendingConfirmation` as a failure, silently exiting 0 on `Denied`/`NotImplemented` — a schema-rejected plan node (no effect ever ran) was indistinguishable from success by exit code alone. Never exercised by the pre-existing `DeterministicPlanner` (its hardcoded arg names always satisfy the schema); only surfaced because `LlmPlanner` can produce a schema-invalid node. Fixed to exit non-zero on any non-`Allowed` decision.
- After all four fixes, re-ran the live test against the exact committed state and captured a clean pass (see verbatim output below).

## Task Commits

1. **Task 1: Forward OpenAI env into the Mailpit verification container** — `69a3155` (feat)
2. **Task 2: Live clean-path LLM acceptance test** — `067c080` (test)
3. **Deviation fix: 3 real composition bugs found live** — `63c12cc` (fix) — see Deviations below

**Plan metadata:** this SUMMARY's own commit (see below).

## Files Created/Modified

- `cli/caprun/tests/llm_planner_live_accept.rs` — the live clean-path acceptance test (Linux-gated) + macOS cross-platform guard test.
- `scripts/mailpit-verify.sh` — forwards `OPENAI_API_KEY` (unconditional-but-empty-tolerant) and `CAPRUN_PLANNER_MODEL` (conditional, only-if-set) into the `rust:1` container.
- `cli/caprun/src/planner.rs` — fixed abstract-socket connect (bug 1), added `SidecarReply` unwrap (bug 2), added `canonical_names` arg-name canonicalization (bug 3).
- `cli/caprun/src/worker.rs` — widened the non-success exit check from `BlockedPendingConfirmation`-only to any non-`Allowed` decision.
- `cli/caprun/tests/planner.rs` — updated the 4 pre-existing `build_planner_request`/`response_to_plan_node` unit tests for the new `canonical_names` return value/parameter, and added a new test (`response_to_plan_node_canonicalizes_arg_name_ignoring_model_naming`) proving the canonicalization behavior directly.

## Decisions Made

- Used std's stable `SocketAddrExt::from_abstract_name` + `connect_addr` for the synchronous sidecar connect rather than spinning up a throwaway tokio runtime inside `Planner::plan()` — simpler, no new runtime-lifecycle risk, matches the codebase's existing Linux-only-abstract-socket pattern.
- Canonicalized arg names by `value_id` identity (caller-owned mapping) rather than editing `crates/llm-planner`'s prompt to instruct the model on exact names — keeps the fix inside already-touched, non-TCB `cli/caprun/src/planner.rs`, and is structurally more robust (never depends on model compliance), which matters directly for Phase 22's adversarial planner.
- Widened `worker.rs`'s failure check to any non-`Allowed` decision rather than special-casing `Denied` alongside `Blocked` — simpler and correctly future-proofs against `NotImplemented` too.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Abstract-socket connect used the wrong std API, failing 100% of the time**
- **Found during:** Task 2's first live run (`scripts/mailpit-verify.sh`)
- **Issue:** `connect_to_sidecar` (`cli/caprun/src/planner.rs`, from Plan 21-03) called `std::os::unix::net::UnixStream::connect(&format!("\0{planner_sock}"))`. Std's generic path-based `sockaddr_un` builder rejects ANY nul byte in the path — including a leading one — with `io::ErrorKind::InvalidInput`, a non-retryable error kind. The connect-retry loop's guard only matched `ConnectionRefused`/`NotFound`, so every real invocation failed on the FIRST attempt in well under a millisecond — indistinguishable at a glance from "the sidecar never came up," but 100% reproducible and unrelated to timing.
- **Fix:** Replaced with `std::os::linux::net::SocketAddrExt::from_abstract_name` (stable since Rust 1.70) + `UnixStream::connect_addr`, `#[cfg(target_os = "linux")]`-gated with a `#[cfg(not(target_os = "linux"))]` sibling that still compiles on macOS but fails fast at runtime.
- **Files modified:** `cli/caprun/src/planner.rs`
- **Verification:** Re-ran the live test on Linux; the sidecar connect succeeded (confirmed by the sidecar's own "listening on abstract socket" log line reaching the worker's next failure point).
- **Committed in:** `63c12cc`

**2. [Rule 1 — Bug] Worker-side sidecar reply parsing didn't match the sidecar's actual wire shape**
- **Found during:** Task 2's second live run, immediately after fixing bug 1
- **Issue:** `request_plan_from_sidecar` called `recv_framed::<PlannerResponse>`, deserializing the frame body directly as a bare `PlannerResponse`. `cli/caprun-planner`'s `handle_connection` (Plan 21-02) ALWAYS wraps its reply in `{"status":"ok","response":{...}}` / `{"status":"error","message":"..."}` — documented in that crate's own `main.rs` doc comment as the contract Plan 21-03's proxy should mirror, but Plan 21-03's actual implementation (built in a parallel, sibling worktree) had assumed a bare-`PlannerResponse` reply instead, per its own SUMMARY's documented (and now falsified) assumption. A bare parse failed on every real reply.
- **Fix:** Added a local `SidecarReply` enum mirroring the sidecar's wire shape (`#[derive(Deserialize)]`, `tag = "status"`) and unwrapped it in `request_plan_from_sidecar`, mapping `Error { message }` to a hard `Err`.
- **Files modified:** `cli/caprun/src/planner.rs`
- **Verification:** Re-ran the live test; the sidecar's real OpenAI call reply was now correctly parsed (confirmed by reaching `plan_node_evaluated` — an Allowed/Denied decision — instead of a parse error).
- **Committed in:** `63c12cc`

**3. [Rule 1 — Bug] LLM-returned arg names didn't match the executor's hardcoded sink schema**
- **Found during:** Task 2's third live run, immediately after fixing bugs 1 and 2
- **Issue:** `response_to_plan_node` copied the model's own `response_arg.name` verbatim into the final `PlanArg`. `crates/executor/src/sink_schema.rs`'s hardcoded `email.send` schema requires exactly `{"to","cc","bcc","subject","body"}` — but nothing in `crates/llm-planner`'s `build_planner_prompt`/`build_tool_schema` (Plan 21-01) tells the model this; the model only sees `slot_hint`s (`"recipient"`/`"subject"`/`"body"`). A real `gpt-4o-mini` call reliably named the recipient arg `"recipient"` (matching the slot hint) instead of `"to"`, so `validate_schema` correctly returned `DenyReason::UnknownArg` — the executor's `Denied` outcome shares the SAME generic `plan_node_evaluated` audit event as `Allowed` (only `BlockedPendingConfirmation` gets its own `sink_blocked` event type), so this looked, from the audit DAG alone, like a quiet no-op rather than an explicit rejection.
- **Fix:** `build_planner_request` now also returns `canonical_names: Vec<(ValueId, String)>` — the exact sink-required name for each offered handle, built from its own known slot->handle bindings (never the model's guess). `response_to_plan_node` looks up the canonical name by `value_id` identity and uses it for the final `PlanArg.name`, ignoring `response_arg.name` entirely (falling back to it only for a `value_id` absent from the mapping, e.g. `file.create`'s unmapped subject/body slots — out of this plan's tested scope, unchanged fail-closed behavior).
- **Files modified:** `cli/caprun/src/planner.rs`, `cli/caprun/tests/planner.rs` (updated 4 existing unit tests for the new signature, added 1 new test proving the canonicalization directly)
- **Verification:** Re-ran the live test; reached `email_send_attempted` + `email_send_succeeded`, `Chain verification: PASSED`, exactly one Mailpit-captured message.
- **Committed in:** `63c12cc`

**4. [Rule 1 — Bug, compounding #3's symptom] Worker exited 0 on a schema-Denied (non-executed) plan node**
- **Found during:** Diagnosing bug 3 — the worker's exit code gave no signal that anything had gone wrong
- **Issue:** `worker.rs` only treated `ExecutorDecision::BlockedPendingConfirmation` as a caller-visible failure; every other non-`Allowed` variant (`Denied`, `NotImplemented`) silently fell through to `Ok(())` — exit 0 — even though no effect ever ran. This was unreachable via the pre-existing `DeterministicPlanner` (its hardcoded arg names always satisfy the schema, so it never produces `Denied`); only Phase 21's `LlmPlanner`, which CAN produce a schema-invalid plan node, exposed it.
- **Fix:** Widened the check to `if !matches!(decision, ExecutorDecision::Allowed) { exit(1) }`, printing the `Debug` form of the decision for diagnosability.
- **Files modified:** `cli/caprun/src/worker.rs`
- **Verification:** No existing test referenced `Denied`/`DenyReason` at the `cli/caprun` e2e level (grep-confirmed before the change); `cargo test --workspace --no-fail-fast` shows 0 regressions after the change.
- **Committed in:** `63c12cc`

**5. [Rule 3 — Blocking issue, in this plan's own Task 1] CAPRUN_PLANNER_MODEL forwarded as an empty string, overriding the sidecar's own default**
- **Found during:** The second live-run round, after fixing bugs 1 and 2 (the sidecar's own startup log showed `model=` — empty — instead of the expected default)
- **Issue:** This plan's own Task 1 change to `scripts/mailpit-verify.sh` forwarded `CAPRUN_PLANNER_MODEL` unconditional-but-empty-tolerant, mirroring the pattern used for `OPENAI_API_KEY` — but the plan's own Task 1 text specified `CAPRUN_PLANNER_MODEL` should be "forwarded if set" (conditional), not unconditional. Because `caprun`'s own sidecar-spawn code and `caprun-planner`'s own default both apply `"gpt-4o-mini"` via `unwrap_or_else` on an ABSENT var (`Err`), never on a present-but-empty one (`Ok("")`), the always-forwarded empty string silently defeated that default — the sidecar sent `model: ""` to OpenAI, which returned HTTP 400 "you must provide a model parameter."
- **Fix:** Rebuilt the `docker run` invocation as an array, appending the `-e CAPRUN_PLANNER_MODEL=...` flag only when the var is actually set on the host. `OPENAI_API_KEY`'s unconditional-but-empty-tolerant forwarding is unchanged (that IS the plan's stated intent for the key specifically).
- **Files modified:** `scripts/mailpit-verify.sh`
- **Verification:** Re-ran the live test; sidecar log showed `model=gpt-4o-mini` as expected, and the OpenAI call succeeded.
- **Committed in:** `63c12cc`

---

**Total deviations:** 5 auto-fixed (4 Rule 1/genuine bugs, 1 Rule 3/blocking-issue-in-own-scope). All were found by actually running the real composed path for the first time (the stated purpose of this plan) and are non-TCB (`cli/caprun/*` only, plus this plan's own script) — no `crates/brokerd` or `crates/executor` changes; `./scripts/check-invariants.sh` and `cargo test --workspace --no-fail-fast` both confirmed clean after all fixes.
**Impact on plan:** All five fixes were necessary to achieve this plan's actual, stated success criteria (a real live delivered email) — without them, the composed Plan 21-02/21-03 wiring would never have worked in real conditions, and this plan's job is precisely to prove or disprove that composition live. No scope creep beyond what was required to reach a genuine pass.

## Issues Encountered

Three consecutive live-run failures during development, each fully diagnosed via `scripts/mailpit-verify.sh` runs and (for the arg-schema issue) a direct manual `sqlite3`-based audit-DB dump inside the verification container — no failure was a flake; each was 100% reproducible until its specific fix landed. See Deviations above for full detail per bug.

## User Setup Required

None — `OPENAI_API_KEY` was already present in the environment (confirmed: length 164, present throughout all live runs).

## Next Phase Readiness

The clean-path LLM-planner composition (sidecar + worker-side proxy + real OpenAI call + real SMTP delivery) is now proven live and genuinely working end-to-end — not merely compiling. Phase 22's HARD GATE (the adversarial/hostile-planner Block scenario, GATE-01..04) can build directly on this proven wiring; the arg-name-canonicalization pattern established here (never trust the planner's own naming; canonicalize by value_id identity) is directly reusable for that phase's "never trust the model" security posture. No blockers.

### Captured live output (final run, against the exact committed state)

```
=== Audit DAG (session 8d09d6e4-e8ca-4893-8ba9-d9e95e97de43) ===
[0] session_created (actor=broker:seed_provenance=trusted_arg)
    hash=0363f2fe parent=(root)
  [1] intent_received (actor=user-intent)
      hash=4bfebe97 parent=0363f2fe
    [2] intent_received (actor=user-intent)
        hash=8523d1d9 parent=4bfebe97
      [3] intent_received (actor=user-intent)
          hash=69ff471b parent=8523d1d9
        [4] fd_granted (actor=broker)
            hash=1941fba2 parent=69ff471b
          [5] plan_node_evaluated (actor=executor)
              hash=b768a6ad parent=1941fba2
            [6] email_send_attempted (actor=sink:email.send:f3d69088-db96-44e9-83c9-97292ffebea5)
                hash=35ae5ba0 parent=b768a6ad
              [7] email_send_succeeded (actor=sink:email.send:f3d69088-db96-44e9-83c9-97292ffebea5)
                  hash=60cb6aa9 parent=35ae5ba0

Chain verification: PASSED

caprun (llm clean) stderr:
[sandbox] Landlock status: FullyEnforced
[caprun-planner] listening on abstract socket "/agentos/planner/8d09d6e4-e8ca-4893-8ba9-d9e95e97de43" (model=gpt-4o-mini)

test llm_planner_clean_allow_delivers ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 2.52s

Mailpit-backed Linux verification suite PASSED.
```

Per-run OpenAI cost was not re-captured with raw token counts in this run's stdout (the test doesn't print `usage`), but Plan 21-02's own live-call verification (identical model, comparable prompt size) measured `prompt_tokens=392, completion_tokens=102, total_tokens=494` — at published `gpt-4o-mini` rates (~$0.15/1M input, ~$0.60/1M output) that is **~$0.00012/request**, consistent with this plan's "order 1e-4 dollars" expectation. This run's own model confirmation (`model=gpt-4o-mini`) and successful single tool-call round-trip are consistent with that same cost order.

## Known Stubs

None — the full clean-path flow (planner call, executor evaluation, SMTP send, audit chain) is real, live-exercised code with no mocked or stubbed component.

## Threat Flags

None beyond what the plan's own `<threat_model>` already covers (the sidecar's `OPENAI_API_KEY` handling and the live-send repudiation proof) — no new network endpoints, auth paths, or schema changes were introduced; the arg-name-canonicalization fix (bug 3) is a STRICTER validation than before (it removes trust in the model's own string, never adds any), and the worker exit-code widening (bug 4) is also strictly more fail-closed than the prior behavior.

---
*Phase: 21-adversarial-llm-planner*
*Completed: 2026-07-11*
