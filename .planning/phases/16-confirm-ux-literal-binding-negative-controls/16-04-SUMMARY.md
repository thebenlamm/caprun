---
phase: 16-confirm-ux-literal-binding-negative-controls
plan: 04
subsystem: security
tags: [rust, smtp, taint-tracking, session-lifecycle, audit-dag, mailpit]

# Dependency graph
requires:
  - phase: 16-confirm-ux-literal-binding-negative-controls (Plan 16-01)
    provides: "combined_digest / Event.combined_digest+blocked_arg_names / PendingConfirmation schema (Block-time write) — NOT touched by this plan (the Allowed path deliberately adds no PendingConfirmation/digest state)"
  - phase: 16-confirm-ux-literal-binding-negative-controls (Plan 16-02)
    provides: "confirm()'s chain-verify + FULL-set digest gate, current_chain_head, the email.send confirm-path CAS + email_send_attempted transaction this plan's Allowed-path append mirrors (minus the CAS)"
  - phase: 16-confirm-ux-literal-binding-negative-controls (Plan 16-03)
    provides: "CONTROL-02 body-tainted-only live block fixture/test in s9_live_block.rs, extended by this plan's std-only Mailpit HTTP client"
provides:
  - "email.send Allowed-decision dispatch in server.rs (mirrors file.create): a trusted, never-blocked plan node reaches the real SMTP adapter"
  - "BLOCKER-1 guard (a): ProvideIntent accepted EXACTLY ONCE and ONLY BEFORE any RequestFd, broker-enforced via two new per-connection dispatch_request params"
  - "BLOCKER-1 guard (b): DenyReason::NonLiveSessionDeniesCommitIrreversible — the executor now Denies a CommitIrreversible sink in ANY non-live SessionStatus (WaitingApproval/Done/Failed/RolledBack), not just Draft"
  - "BLOCKER-1 guard (c): the in-broker CreateSession IPC arm's forced-Active mint gated behind CAPRUN_ENABLE_IPC_CREATE_SESSION == exactly \"1\" (fail-closed default-deny, NOT cfg(test))"
  - "MAJOR-4: a durable opaque email_send_attempted event appended (parent-chained) BEFORE the SMTP socket opens on the Allowed path"
  - "MAJOR-5: email_smtp.rs's doc comment names both sanctioned callers (confirm()'s post-CAS special case + this new Allowed-dispatch) and the shared attempt-ledger precondition"
  - "CONTROL-01 live A/B (s9_control_ab_taint_driven): trusted-intent send Allowed AND delivered (captured in Mailpit, ZERO pending_confirmations rows) vs doc-derived send Blocked and never sent, in one run"
  - "A std-only Mailpit HTTP client (byte-level chunked-transfer-encoding-aware) in both cli/caprun/tests/s9_live_block.rs and crates/brokerd/tests/email_smtp_acceptance.rs, isolating by unique per-run recipient"
  - "scripts/mailpit-verify.sh MAILPIT_VERIFY_CMD override + CLAUDE.md Phase-16-onward verification note"
affects: ["Phase 17 (the deny-sends-nothing send-level proof mandate recorded below; the real end-to-end doc-derived confirm/deny path)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Broker-enforced per-connection ordering guard (two &mut bool locals threaded through dispatch_request, mirroring session_status's existing threading discipline) — replaces an assumed honest-worker-startup-order invariant with an actively-checked one"
    - "Runtime opt-in env-flag gate (exactly \"1\", never .is_ok()) for a test-only IPC arm — chosen over cfg(test) because cfg(test) is unset when the crate compiles as an integration-test dependency"
    - "Byte-level HTTP/1.1 chunked-transfer-encoding decoder for a dependency-light test HTTP client — never a lossy str split, to avoid UTF-8 boundary corruption at a chunk edge"

key-files:
  created: []
  modified:
    - crates/brokerd/src/server.rs
    - crates/executor/src/lib.rs
    - crates/executor/tests/executor_decision.rs
    - crates/runtime-core/src/executor_decision.rs
    - crates/brokerd/src/sinks/email_smtp.rs
    - crates/brokerd/tests/uds_ipc.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/extract_provenance_threading.rs
    - crates/brokerd/tests/phase5_dispatch.rs
    - cli/caprun/tests/e2e.rs
    - cli/caprun/tests/s9_live_block.rs
    - crates/brokerd/tests/email_smtp_acceptance.rs
    - scripts/mailpit-verify.sh
    - CLAUDE.md

key-decisions:
  - "Followed the plan's BLOCKER-1 STRUCTURE mandate literally: all three guards landed in Task 1, sequenced strictly BEFORE Task 2's email.send Allowed-dispatch, so the dispatch never exists in the repo without its guards already in place."
  - "Guard (a)'s new unit tests live as a --lib mod tests block INSIDE server.rs (not an integration test) per the plan's own verify command (cargo test -p brokerd --lib server) — required adding adapter_fs::recv_fd draining in the RequestFd-then-ProvideIntent negative test, since the RequestFd arm's SCM_RIGHTS 1-byte sendmsg payload precedes the JSON response on the wire (mirrors cli/caprun/src/worker.rs's real client-side ordering); omitting that drain corrupted the length-prefix framing and hung the test indefinitely."
  - "Executor tests for guard (b) (executor/tests/executor_decision.rs) were added even though this file was not listed in the plan's frontmatter files_modified or Task 1's <files> tag — the action text explicitly required them (\"Add a test: a CommitIrreversible sink in each of the four states...\"), so this is a Rule 2 (missing critical test coverage) auto-add, not scope creep."
  - "The CONTROL-01 hostile half's Mailpit-absence check uses the FIXED literal recipient \"accounts@ev1l.com\" (HOSTILE_EMAIL_CONTENT's concat-derived recipient) rather than a fresh UUID — this is safe because that exact address only ever arises from the doc-derived path, which always blocks across this entire suite, so there is no cross-test race despite not being unique-per-run."
  - "Real bug found and fixed during Linux verification (Rule 1, not planned): Mailpit's Go HTTP server streams large /api/v1/messages LIST responses as Transfer-Encoding: chunked despite Connection: close, once this suite's own new email.send Allowed-dispatch traffic grew the shared inbox large enough to trigger it. Both the new std-only Mailpit client (s9_live_block.rs) and the pre-existing one (email_smtp_acceptance.rs) needed a byte-level chunked decoder — this would have silently corrupted the CONTROL-01 A/B test's Mailpit query on a busy inbox."

requirements-completed: [CONTROL-01]

# Coverage metadata
coverage:
  - id: D1
    description: "BLOCKER-1 guard (a): ProvideIntent accepted exactly once and only before any RequestFd, broker-enforced; second-ProvideIntent and after-RequestFd cases both reject fail-closed (mint nothing, no chain-head advance); the happy path (before any RequestFd) is unchanged"
    requirement: "CONTROL-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/server.rs#server::tests::provide_intent_before_any_request_fd_succeeds, second_provide_intent_is_rejected, provide_intent_after_request_fd_is_rejected"
        status: pass
    human_judgment: false
  - id: D2
    description: "BLOCKER-1 guard (b): the executor Denies a CommitIrreversible sink in all four non-live SessionStatus states (WaitingApproval/Done/Failed/RolledBack) with the new NonLiveSessionDeniesCommitIrreversible reason; a non-CommitIrreversible (Observe) sink in those states is unaffected"
    requirement: "CONTROL-01"
    verification:
      - kind: unit
        ref: "crates/executor/tests/executor_decision.rs#non_live_session_denies_commit_irreversible_in_all_four_states, non_live_session_allows_observe"
        status: pass
    human_judgment: false
  - id: D3
    description: "BLOCKER-1 guard (c): the in-broker CreateSession IPC arm is disabled by default (flag unset/empty/anything but exactly \"1\" -> Error, zero new session rows) and enabled only with CAPRUN_ENABLE_IPC_CREATE_SESSION=1; the F2 negative test explicitly unsets the flag and asserts both the Error response and an unchanged sessions-table row count"
    requirement: "CONTROL-01"
    verification:
      - kind: e2e
        ref: "crates/brokerd/tests/uds_ipc.rs#linux_tests::create_session_over_ipc_denied_by_default_when_flag_unset, server_accept, create_session_round_trip"
        status: pass
    human_judgment: false
  - id: D4
    description: "The email.send Allowed-decision dispatch reaches invoke_email_smtp_from_resolved via a faithful mirror of the file.create branch (same locking, head-advance, two-phase ordering), with a durable opaque email_send_attempted event (MAJOR-4) appended BEFORE the adapter call; email_smtp.rs's doc comment names both sanctioned callers (MAJOR-5)"
    requirement: "CONTROL-01"
    verification:
      - kind: integration
        ref: "cargo build -p brokerd; cargo test -p brokerd --lib (95 passed); grep confirms no PendingConfirmation/digest construction on the Allowed path"
        status: pass
    human_judgment: false
  - id: D5
    description: "CONTROL-01 live A/B, one run: a trusted-intent clean send is Allowed AND delivered (exit 0, plan_node_evaluated present, sink_blocked absent, ZERO pending_confirmations rows, email_send_succeeded present, captured in Mailpit for a unique per-run recipient) while a doc-derived hostile send is Blocked and never sent (exit non-zero, sink_blocked present, email_send_succeeded absent, recipient absent from Mailpit)"
    requirement: "CONTROL-01"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_live_block.rs#s9_control_ab_taint_driven (run under scripts/mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D6
    description: "Every benign send-email-summary test that now triggers a live send is robust: e2e.rs's dag_chain_integrity asserts the corrected 8-event chain (... plan_node_evaluated -> email_send_attempted -> email_send_succeeded); s9_live_clean_allow_path asserts email_send_succeeded now exists; email_smtp_acceptance.rs's SMTP-03/05 tests isolate by unique per-test recipient (no purge-all, no global count); no macOS-visible test needed a fake SMTP acceptor (origin_seed_provenance.rs unchanged, per the plan's own pre-verified reasoning)"
    requirement: "CONTROL-01"
    verification:
      - kind: e2e
        ref: "bash scripts/mailpit-verify.sh (full workspace suite, exit 0) — dag_chain_integrity, s9_live_clean_allow_path, smtp_03_confirmed_send_captured_by_mailpit, smtp_05_crlf_body_cannot_smuggle_recipient all pass"
        status: pass
    human_judgment: false
  - id: D7
    description: "Full workspace regression: cargo test --workspace --no-fail-fast is green on macOS (all pre-existing + new tests); ./scripts/check-invariants.sh (Gate 1/2/3) passes; the authoritative Linux run (bash scripts/mailpit-verify.sh) passes with exit 0"
    requirement: "CONTROL-01"
    verification:
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast (macOS, 0 failed across all binaries); ./scripts/check-invariants.sh (all 3 gates PASSED); bash scripts/mailpit-verify.sh (Linux, exit 0)"
        status: pass
    human_judgment: false

# Metrics
duration: ~3h
completed: 2026-07-08
status: complete
---

# Phase 16 Plan 04: email.send Allowed-Dispatch, BLOCKER-1 Guards & CONTROL-01 Live A/B Summary

**Wires the email.send Allowed-decision dispatch (CONTROL-01's production path — a trusted, never-blocked send now actually reaches the real SMTP adapter) behind three mandatory security guards that close a self-discovered reach-to-a-send exploit, with a durable pre-send attempt ledger and a live Mailpit-captured A/B proof.**

## Performance

- **Duration:** ~3h
- **Completed:** 2026-07-08
- **Tasks:** 3 (+ 1 trivial doc-only follow-up commit)
- **Files modified:** 15

## Accomplishments

- **BLOCKER-1 guard (a) — ProvideIntent ordering.** `dispatch_request` gained two new per-connection `&mut bool` locals (`intent_provided`, `fd_requested`), threaded exactly like the existing `session_status` local. `ProvideIntent` is now accepted EXACTLY ONCE and ONLY BEFORE any `RequestFd` on a connection — a violation (second `ProvideIntent`, or one arriving after any `RequestFd`) is rejected fail-closed (`Error`, mints nothing, no chain-head advance). This converts what was previously an assumed honest-worker-startup-order invariant into a broker-enforced one, closing half of the reach-to-a-send path this plan's own coordinator panel discovered: `RequestFd` releases raw untrusted bytes without ever demoting `session_status`, so a worker that never calls `ReportClaims` could otherwise follow with a `ProvideIntent{recipient: "evil@..."}` minting an arbitrary attacker-chosen literal as fully-trusted `UserTrusted`.
- **BLOCKER-1 guard (b) — non-live-state Deny.** Added `DenyReason::NonLiveSessionDeniesCommitIrreversible` (`runtime-core`) and extended the executor's Step 0.5 `SessionStatus` match: a `CommitIrreversible` sink now Denies in ANY of `WaitingApproval`/`Done`/`Failed`/`RolledBack` (previously these four states all fell through to `Allowed`), not just `Draft`. The match stays exhaustive (no wildcard), so a future `SessionStatus` variant is a compile error here, never a silent bypass.
- **BLOCKER-1 guard (c) — CreateSession IPC arm gated.** The in-broker `CreateSession` arm's forced-`Active` `SeedProvenance::TrustedArg` mint is now behind a fail-closed RUNTIME opt-in env flag, `CAPRUN_ENABLE_IPC_CREATE_SESSION`, checked via `matches!(std::env::var(...).as_deref(), Ok("1"))` — never `.is_ok()` (an inherited empty value must NOT enable it) and never `cfg(test)` (unset when `brokerd` compiles as an integration-test dependency, which would have panicked `uds_ipc.rs`'s own tests under `mailpit-verify.sh`). `uds_ipc.rs`'s two pre-existing tests explicitly opt in; a new F2 test (`create_session_over_ipc_denied_by_default_when_flag_unset`) explicitly `remove_var`s the flag and asserts BOTH `BrokerResponse::Error` AND an unchanged `sessions` row count — the one test that would catch a future accidental default-flip.
- **email.send Allowed-decision dispatch (CONTROL-01).** A new branch in `server.rs`'s `SubmitPlanNode` handler, immediately mirroring the existing `file.create` Allowed-dispatch (same re-lock, same head-advance, same two-phase authorize-then-effect ordering): on `ExecutorDecision::Allowed` for `email.send`, the FULL arg set is resolved from the live `ValueStore` into `ResolvedArg`s (fail-closed on a dangling handle), then — **MAJOR-4** — a durable, opaque `email_send_attempted` event is appended (parent-chained onto the `plan_node_evaluated` head) BEFORE the adapter is ever called, so a crash between attempt and delivery always leaves an audit record naming `email.send`. Only after that append succeeds does `invoke_email_smtp_from_resolved` run. No `PendingConfirmation`/`combined_digest` state is ever created on this path (that shape stays exclusive to the Block path).
- **MAJOR-5 — the amended adapter comment.** `email_smtp.rs`'s doc comment no longer claims `invoke_email_smtp_from_resolved` is "never called from the allow-path" — it now names BOTH sanctioned callers (`confirm()`'s post-CAS `email.send` special case, and this new Allowed-dispatch) and restates the shared precondition once: a durable `email_send_attempted` MUST precede the call.
- **CONTROL-01 live A/B, one run.** `s9_control_ab_taint_driven` (new, `cli/caprun/tests/s9_live_block.rs`) asserts, in ONE test: (A) a trusted-intent clean send over a UNIQUE per-run recipient exits 0, has a `plan_node_evaluated` event, has NO `sink_blocked` event, has ZERO `pending_confirmations` rows for that session (the "no confirm gate" claim — stronger than "no sink_blocked"), has an `email_send_succeeded` event, AND is captured in Mailpit for that recipient; (B) a doc-derived hostile send exits non-zero, has a `sink_blocked` event, has NO `email_send_succeeded` event, and its recipient never appears in Mailpit. The doc comment uses the honest framing this plan mandates ("trusted-intent Allowed AND delivered vs doc-derived Blocked and not sent" — never "same doc, taint flipped").
- **Mailpit test-surface robustness (BLOCKER-3).** `e2e.rs`'s `dag_chain_integrity` now asserts the corrected 8-event benign chain (the Allowed dispatch adds `email_send_attempted` + `email_send_succeeded` after `plan_node_evaluated`). `s9_live_clean_allow_path` now additionally asserts `email_send_succeeded` exists (a send now actually happens on that path). `email_smtp_acceptance.rs`'s two SMTP-03/05 tests were refactored off a purge-all + global-count assertion (which now races caprun's own sends across the shared Mailpit inbox) onto a unique-per-test-recipient filter. A new std-only Mailpit HTTP client (byte-level, no new dependency) was added to `s9_live_block.rs`, reading `CAPRUN_SMTP_HOST` at Mailpit's fixed HTTP port 8025.
- **`scripts/mailpit-verify.sh` / `CLAUDE.md`.** `MAILPIT_VERIFY_CMD` is now a real override (was previously a hardcoded string masquerading as a documented override) — a caller can scope a Linux run to a single test. `CLAUDE.md` now states that FROM PHASE 16 ONWARD all Linux verification MUST go through `mailpit-verify.sh`, never the bare `docker run rust:1` recipe, and corrects the env description (`CAPRUN_SMTP_HOST=<the resolved sidecar IP>`, never the literal `"mailpit"`).
- **Real bug found and fixed during Linux verification (unplanned, Rule 1).** Mailpit's Go HTTP server streams large `/api/v1/messages` LIST responses as `Transfer-Encoding: chunked` despite `Connection: close` — this only surfaced once this plan's own new email.send Allowed-dispatch traffic grew the shared Mailpit inbox large enough (the first full Linux run failed `s9_control_ab_taint_driven` with a JSON-parse error on a chunked body: `"9d7\r\n{...}\r\n0\r\n\r\n"`). Both the new `s9_live_block.rs` client and the pre-existing `email_smtp_acceptance.rs` one needed a byte-level chunked-transfer decoder (never a lossy `str` split, to avoid corrupting a chunk boundary landing mid-multi-byte-UTF-8-character). Fixed in both; the full suite then passed cleanly on the second Linux run.

## Task Commits

1. **Task 1: BLOCKER-1 guards (a/b/c)** - `277a090` (feat)
2. **Task 2: email.send Allowed-dispatch + MAJOR-4/5** - `a2045fc` (feat)
3. **Task 3: BLOCKER-3 test surface + CONTROL-01 live A/B** - `0d4a4e8` (test)
4. **Follow-up: stale doc-comment fix** - `1c9dc6c` (docs)

## Files Created/Modified

- `crates/brokerd/src/server.rs` — guards (a)/(c), the email.send Allowed-dispatch branch (MAJOR-4), guard-(a) `--lib` unit tests
- `crates/executor/src/lib.rs` — guard (b): Step 0.5's non-live-state `SessionStatus` match now Denies `CommitIrreversible`
- `crates/executor/tests/executor_decision.rs` — guard (b) tests (all four states, plus the Observe-passthrough)
- `crates/runtime-core/src/executor_decision.rs` — `DenyReason::NonLiveSessionDeniesCommitIrreversible` + `code()`/`Display` arms
- `crates/brokerd/src/sinks/email_smtp.rs` — MAJOR-5 doc comment amendment (both sanctioned callers)
- `crates/brokerd/tests/uds_ipc.rs` — env-flag opt-in on the two existing tests + the new F2 default-deny test + env-lock
- `crates/brokerd/tests/durable_anchor.rs`, `proto_claims.rs`, `extract_provenance_threading.rs`, `phase5_dispatch.rs` — the six `dispatch_request` call sites updated for the two new `&mut bool` params
- `cli/caprun/tests/e2e.rs` — `dag_chain_integrity`'s 8-event chain assertion
- `cli/caprun/tests/s9_live_block.rs` — `s9_live_clean_allow_path`'s new assertion, the std-only Mailpit client, `s9_control_ab_taint_driven` (CONTROL-01)
- `crates/brokerd/tests/email_smtp_acceptance.rs` — unique-recipient refactor + the chunked-decoder fix
- `scripts/mailpit-verify.sh` — `MAILPIT_VERIFY_CMD` wired as a real override
- `CLAUDE.md` — Phase-16-onward verification note

## Decisions Made

- Followed the plan's BLOCKER-1 STRUCTURE mandate literally: all three guards landed in Task 1's commit, strictly before Task 2's dispatch commit — the dispatch never exists in this repo's history without its guards.
- Guard (a)'s new tests are `--lib` unit tests inside `server.rs` (not integration tests), per the plan's own verify command (`cargo test -p brokerd --lib server`).
- Added `executor/tests/executor_decision.rs` test coverage for guard (b) even though this exact file wasn't in the plan's frontmatter `files_modified` list — the action text explicitly required it (Rule 2, missing critical test coverage).
- The CONTROL-01 hostile-half Mailpit-absence check intentionally uses the fixed literal `"accounts@ev1l.com"` (not a fresh UUID) — safe because that address only ever arises from a doc-derived path, which always blocks everywhere in this suite.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Mailpit HTTP chunked-transfer-encoding not decoded**
- **Found during:** Task 3's first Linux verification run (`s9_control_ab_taint_driven` failed with a JSON parse error)
- **Issue:** Both the new `s9_live_block.rs` Mailpit HTTP client and the pre-existing `email_smtp_acceptance.rs` one assumed the raw bytes after the header separator were the complete JSON body. Once this plan's own new email.send Allowed-dispatch traffic grew the shared Mailpit inbox, a large LIST response arrived `Transfer-Encoding: chunked` (Mailpit's Go server streams JSON without a precomputed `Content-Length`), and the naive client fed the raw chunk-framed bytes straight to `serde_json::from_str`, failing with "trailing characters."
- **Fix:** Added a byte-level chunked-transfer decoder (`decode_chunked`) to both clients, gated on detecting `Transfer-Encoding: chunked` in the response headers; operates purely on `&[u8]` (never a lossy `str` split) to avoid corrupting a chunk boundary at a multi-byte UTF-8 character.
- **Files modified:** `cli/caprun/tests/s9_live_block.rs`, `crates/brokerd/tests/email_smtp_acceptance.rs`
- **Verification:** Second full Linux run (`bash scripts/mailpit-verify.sh`) passed cleanly, exit 0, all tests including `s9_control_ab_taint_driven`.
- **Committed in:** `0d4a4e8` (Task 3 commit — discovered and fixed within the same task, before committing)

---

**Total deviations:** 1 auto-fixed (Rule 1, bug fix)
**Impact on plan:** The fix was necessary for the plan's own CONTROL-01 acceptance criterion (a real Mailpit-captured A/B) to be verifiable at all under a busy shared inbox — no scope creep, and it strengthens exactly the test surface this plan was already adding.

## Issues Encountered

- Environment-level: the sandboxed Bash tool hung indefinitely on two early `cargo test -p brokerd --lib server` invocations (0.10s CPU over several minutes) before any guard-(a) unit tests were runnable — traced to a guard-(a) negative test that drove the `RequestFd` arm without draining its SCM_RIGHTS 1-byte sendmsg payload first, corrupting the subsequent length-prefix read and hanging `read_exact` on bytes that would never arrive. Fixed by adding an `adapter_fs::recv_fd` drain before reading the framed JSON response, mirroring `cli/caprun/src/worker.rs`'s real client-side ordering (`recv_fd` before consuming the JSON). Resolved before any task commit.
- The rest of the plan executed without further issues; both Linux verification runs (before/after the chunked-decoder fix) otherwise matched expectations exactly.

## User Setup Required

None — no external service configuration required. No new Cargo dependency.

## DOC-01 — I1 honest-scope qualifier (carried forward, per this plan's `<output>` spec)

"Reading raw untrusted bytes → draft-only" is **NOT** literally true at `RequestFd`-release time — the session goes `Draft` only when the broker mints untrusted taint from a REPORTED read (`ReportClaims` → `mint_from_read`). This plan's guards close the *reachable-to-a-send* path that this gap would otherwise open (a worker could `RequestFd` → read attacker bytes → never call `ReportClaims` → `ProvideIntent` an arbitrary trusted literal → `Allowed` → send), but they do NOT make the qualifier untrue. A "demote at RequestFd" fix remains explicitly OUT OF SCOPE for v1.3 (it would contradict the pinned `DESIGN-session-trust-state.md` and break CONTROL-01's own clean path) and is recorded as a v2 obligation in `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` (item 1). No doc or demo in this repo may claim "reading raw untrusted bytes → draft-only" without this qualifier.

## MAJOR-4 replay residual risk (Accepted, per this plan's `<output>` spec)

The Allowed email.send path has NO CAS/`PendingConfirmation` (there is nothing to confirm on a never-blocked decision) — a replayed `SubmitPlanNode` mints a fresh `effect_id` and would send again (N submissions ⇒ N emails). This is an explicitly-named **ACCEPTED RESIDUAL RISK** for v1.3: the durable per-attempt `email_send_attempted` ledger makes each send auditable even though it cannot prevent a duplicate. Tracked as a v2 obligation (idempotency key / CAS on the trusted-send path) in `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` (item 3).

## "Deny sends nothing" — explicit Phase 17 / ACCEPT-01 requirement (per this plan's `<output>` spec)

The hero demo's other half — a human `caprun deny`ing a Blocked confirmation results in NO SMTP send — still has **no send-level proof** in any written plan as of this SUMMARY. This is recorded here as an explicit REQUIREMENT for Phase 17 (or a dedicated ACCEPT-01 plan): a post-`caprun deny` assertion that Mailpit holds no message for the denied recipient AND no `email_send_succeeded` event exists for that effect. This MUST appear as a concrete task in a future phase plan — it must not be assumed to already be covered by the existing `deny_on_pending_block_is_durable`/`digest_mismatch_then_retry_...` tests, none of which query Mailpit.

## Next Phase Readiness

- CONTROL-01 is now fully wired and proven live: the email.send Allowed-dispatch, all three BLOCKER-1 guards, the MAJOR-4 durable attempt ledger, and the MAJOR-5 caller-comment amendment are all in place and Linux-verified.
- Phase 16 is complete: `cargo test --workspace --no-fail-fast` is fully green on macOS, `./scripts/check-invariants.sh` passes (all 3 gates), and `bash scripts/mailpit-verify.sh` (the authoritative Linux run) passes with exit 0.
- Three items are explicitly carried forward as recorded (not silently dropped) obligations: DOC-01 (I1 honest scope), the MAJOR-4 replay residual risk, and the "deny sends nothing" Phase-17/ACCEPT-01 mandate above. A fourth (verify_chain's non-authenticated-tamper-evidence scope, MAJOR-6, from Plan 16-02) remains recorded in the same v2-obligations todo file.
- No blockers for Phase 17.

---
*Phase: 16-confirm-ux-literal-binding-negative-controls*
*Completed: 2026-07-08*

## Self-Check: PASSED

All 16 modified files (server.rs, executor/src/lib.rs, executor/tests/executor_decision.rs,
runtime-core/src/executor_decision.rs, email_smtp.rs, uds_ipc.rs, durable_anchor.rs, proto_claims.rs,
extract_provenance_threading.rs, phase5_dispatch.rs, e2e.rs, s9_live_block.rs,
email_smtp_acceptance.rs, mailpit-verify.sh, CLAUDE.md, this SUMMARY) confirmed present on disk;
all four commits (`277a090`, `a2045fc`, `0d4a4e8`, `1c9dc6c`) confirmed present in git history.
