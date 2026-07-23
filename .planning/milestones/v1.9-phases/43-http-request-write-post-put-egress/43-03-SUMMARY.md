---
phase: 43-http-request-write-post-put-egress
plan: 03
subsystem: brokerd-control-plane
tags: [http-write, confirm-release, dispatch, audit, p33-p34, HTTP-W-01]
requires:
  - "43-01 (executor TCB: http.request.write CommitIrreversible + method-enum gate)"
  - "43-02 (broker sink: invoke_http_write_sink / invoke_http_write_from_resolved / prepare_http_write)"
provides:
  - "server.rs Allowed-decision dispatch arm for http.request.write"
  - "confirmation.rs confirm-release wiring (entry-guard + Step-4.8c precheck + Step-7 dispatch)"
  - "a dispatchable + confirm-releasable http.request.write sink (HTTP-W-01 loop closed end-to-end)"
affects:
  - crates/brokerd/src/server.rs
  - crates/brokerd/src/confirmation.rs
tech-stack:
  patterns:
    - "mirror-the-shipped-sink-arm (github.pr / process.exec) minus grant-gate/CAS"
    - "terminal-EVENT-before-terminal-STATE (P33/P34 confirm-release audit-gap discipline)"
    - "shared prepare_* between pre-burn precheck and Step-7 dispatch (no validation drift)"
key-files:
  created: []
  modified:
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/confirmation.rs
decisions:
  - "http.request.write dispatches on a bare Allowed decision — NO auth-grant gate and NO content CAS (DESIGN §2: a single confirm-releasable write), unlike github.pr."
  - "The Step-4.8c confirm precheck reuses the SAME prepare_http_write the Step-7 dispatch calls, so precheck and dispatch validate url/method/body identically and cannot drift."
  - "Entry-guard allow-list AND Step-7 dispatch match both gain http.request.write and stay in sync; the drift-arm comment set was updated to match."
metrics:
  duration: "~40 min"
  completed: "2026-07-18"
  tasks: 2
  files: 2
status: complete
---

# Phase 43 Plan 03: http.request.write Dispatch + Confirm-Release Wiring Summary

Wired the Plan 43-02 `http_write` egress into the broker's two control-flow paths — the Allowed-decision dispatch (server.rs) and the single-shot confirm-release (confirmation.rs) — closing HTTP-W-01 end-to-end: an untainted-body write to a write-allowlisted host dispatches live, and a tainted-body write that Blocks under I2 is releasable by exactly one `caprun confirm`, preserving the terminal-event-before-terminal-state discipline (the recurring P33/P34 confirm-release audit-gap class).

## What was built

### Task 1 — server.rs Allowed-decision dispatch arm (commit `aaa6a3e`)
- Added an `Allowed && sink == "http.request.write"` arm after the github.pr arm. It calls `invoke_http_write_sink(conn, key, value_store, session_id, effect_id, plan_node, *last_event_id, last_event_hash)` and advances the chain head to the returned terminal event.
- **Simpler than github.pr:** no auth-grant gate, no content CAS (DESIGN §2 defines a single confirm-releasable write). A bare Allowed decision dispatches directly.
- **CONSUMES, no mint:** `output_value_id` stays `None` (Gate 3 byte-identical).
- **Lock discipline:** the sink locks `conn` internally only for its synchronous terminal append; the arm never holds the audit-db mutex across the write `.await`.
- **Terminal-event-first:** the sink appends its opaque `http_write_{succeeded,failed}` event before any `?` unwinds; only an `Allowed` decision reaches the arm (Block/Deny never opens the socket — T-43-09).
- In-crate test drives the real `evaluate_plan_node_and_record` (not a hand-rolled mirror): an untainted `http.request.write` is Allowed by the executor, reaches the dispatch, and records an opaque `http_write_failed` terminal event (host write stub) with no `sink_blocked` and no `http_response_received` mint event.

### Task 2 — confirmation.rs confirm-release (commit `23b26b7`)
- **(1) Step-4.75 entry-guard allow-list:** added `"http.request.write"` to the `match pc.sink.0` arm (a confirm-releasable sink absent from the guard is denied before the burn).
- **(2) Step-4.8c pre-burn precheck:** `prepare_http_write(&pc.resolved_args)` runs BEFORE Step 5 appends `confirm_granted` / Step 6 burns the one-shot. A failure is fail-closed-RECOVERABLE (row stays `Pending`), using the SAME prepare the Step-7 dispatch uses.
- **(3) Step-7 dispatch arm:** `"http.request.write" =>` calls `invoke_http_write_from_resolved(...)` from the frozen `ResolvedArg` snapshot (async), mapping `Ok -> Released`, `Err -> ConfirmedButSinkFailed`. Every failure folds into an opaque `http_write_failed` terminal event first. No CAS/dedup; no mint (Gate 3 byte-identical).
- **(4) Drift comment:** the guard/match-drift error-arm comment set now lists `http.request.write`, keeping the documented invariant accurate.
- Tests: a tainted-body write Blocks then `confirm` releases EXACTLY ONCE with a terminal `http_write_*` chained on `confirm_granted` (verify_chain true; a second confirm returns `AlreadyTerminal`); an empty-body precheck failure leaves the row `Pending` with no `confirm_granted` and no terminal event (the P33/P34 regression).

## Verification

- `cargo build --workspace` — clean (macOS host).
- `cargo test -p brokerd server:: --no-fail-fast` — 6/6 (incl. new dispatch test).
- `cargo test -p brokerd confirmation:: --no-fail-fast` — 38/38 (incl. both new tests).
- `cargo test -p brokerd --no-fail-fast` — 209 lib + all integration binaries green, 0 failed.
- `./scripts/check-invariants.sh` — ALL gates PASS (Gate 3 mint-site allow-list byte-identical; no new mint site; no raw effect-to-sink bypass).
- **Real Linux** via `bash scripts/mailpit-verify.sh` (unprivileged `rust:1` container on the Mailpit network): the done-gate `cargo build --workspace --tests` compiled clean (no [[cfg-linux-test-blindness]] breakage — the signatures were unchanged from 43-02), and the scoped `server::` (6/6) + `confirmation::` (38/38) tests passed.

## Deviations from Plan

None — plan executed exactly as written. No public signatures changed (43-02 pre-created `invoke_http_write_sink` / `invoke_http_write_from_resolved` / `prepare_http_write` with the consumed signatures), so no Linux-gated callers needed threading. The `#[allow(dead_code)]` attributes on the two invoke functions in http_write.rs (Plan 43-02's file, outside this plan's declared files) are now satisfied by the wiring but were left untouched to stay surgical; an unnecessary `allow(dead_code)` produces no warning.

## Known Stubs

None. On the macOS host / default-empty `WRITE_HOST_ALLOWLIST` the live write egress fails at the gate (by design), which the tests exercise as the audited failure path; this is not a stub — the dispatch and confirm-release control flow is fully wired.

## Self-Check: PASSED

- FOUND: crates/brokerd/src/server.rs (Allowed dispatch arm + in-crate test)
- FOUND: crates/brokerd/src/confirmation.rs (guard + Step-4.8c precheck + Step-7 arm + 2 tests)
- FOUND commit aaa6a3e (Task 1), 23b26b7 (Task 2)
