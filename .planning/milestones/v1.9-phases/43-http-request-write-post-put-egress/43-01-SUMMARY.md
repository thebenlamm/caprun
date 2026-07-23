---
phase: 43-http-request-write-post-put-egress
plan: 01
subsystem: executor-tcb
status: complete
tags: [http-write, egress, i0-gate, i2, policy, method-enum]
requirements: [HTTP-W-01]
provides:
  - "http.request.write registered as a DISTINCT CommitIrreversible sink in the executor TCB"
  - "method-enum {POST,PUT} fail-closed gate in submit_plan_node"
  - "http.request.write on the broker_default()/allow_all() production allowlist"
requires:
  - "planning-docs/DESIGN-v1.9-egress-policy.md §2.0/§2.2/§2.6 (design gate)"
affects:
  - "Phase 43 Plan 02+ (broker WRITE network path) will dispatch this classified/gated sink"
key-files:
  created: []
  modified:
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs
    - crates/executor/src/lib.rs
    - crates/executor/tests/executor_decision.rs
    - crates/runtime-core/src/executor_decision.rs
    - crates/runtime-core/src/policy.rs
decisions:
  - "method-enum gate placed at Step 0.3 (after policy gate, before collect-then-Block) scoped to http.request.write only; invalid method Denies fail-closed, a valid method falls through so a tainted body still Blocks (never masked)."
  - "Exact case-sensitive match on the method literal ({POST,PUT}); taint of the method is irrelevant because the distinct sink id already fixes host/effect-class (§2.6)."
  - "New DenyReason::InvalidMethod {sink, method} as owned Strings (IPC-wire deserializable), following the SlotTypeMismatch/PolicyDeny precedent — one denial taxonomy, no second error type."
metrics:
  tasks: 3
  commits: 3
  files_modified: 6
  tests_added: 24
  completed: 2026-07-18
---

# Phase 43 Plan 01: Register http.request.write (executor TCB classification) Summary

Registered `http.request.write` as a DISTINCT, `CommitIrreversible` sink in the executor TCB — the load-bearing I0-gate pin from DESIGN-v1.9-egress-policy §2.0 (`[rev: MAJOR-1]`) — with its exact-match arg schema, taint-sensitivity tables, `{POST,PUT}` method-enum fail-closed gate, and production-policy allowlist entry. No broker network path (that is Plan 02+); this plan makes the executor classify, schema-gate, taint-govern, and method-validate the new WRITE sink correctly, leaving the GET `http.request` Observe path untouched.

## Tasks Completed

| Task | Name | Commit | Key files |
|------|------|--------|-----------|
| 1 | Register http.request.write in KNOWN_SINKS (schema) | `bb64ce1` | crates/executor/src/sink_schema.rs |
| 2 | Classify CommitIrreversible + sensitivity tables | `3c16081` | crates/executor/src/sink_sensitivity.rs |
| 3 | method-enum fail-closed gate + production-policy allowlist | `4415b86` | crates/executor/src/lib.rs, crates/executor/tests/executor_decision.rs, crates/runtime-core/src/executor_decision.rs, crates/runtime-core/src/policy.rs |

## What was built

- **Schema (Task 1):** exact-match `{url, body, method}` (all three `allowed` AND `required`) `SinkSchema` row for `http.request.write`. GET `http.request` row unchanged (single-arg `{url}`) — verified by an explicit regression test.
- **Classification + sensitivity (Task 2):** EXPLICIT `"http.request.write" => EffectClass::CommitIrreversible` arm (redundant with the `_ =>` fail-closed default, per §2.0 so a schema-gate reorder cannot silently relax it). `HTTP_REQUEST_WRITE_ROUTING_SENSITIVE = ["url"]` and `HTTP_REQUEST_WRITE_CONTENT_SENSITIVE = ["url","body"]` consts wired into `is_routing_sensitive`/`is_content_sensitive`; `expected_role` returns `None` for url/body/method (structural role gate is a documented no-op; the Block comes from sensitivity+taint).
- **method-enum gate + policy (Task 3):** Step 0.3 in `submit_plan_node`, scoped to `http.request.write` only, resolves the `method` literal and `Denied(InvalidMethod)` unless it EQUALS exactly `"POST"`/`"PUT"` (case-sensitive). Placed before the collect-then-Block loop; a valid method falls through so a valid-method + tainted-body node still Blocks (differential-acceptance leg, §2.5). Added `DenyReason::InvalidMethod {sink, method}` (code `invalid_method`, Display, serde round-trip). Added `http.request.write` to `PRODUCTION_SINKS` (seven → eight) so `broker_default()`/`allow_all()` permit it.

## must_haves verification

- **I0-escape fix:** `http_write_draft_session_denies_commit_irreversible` proves a Draft session I0-denies a valid clean POST (CommitIrreversible, not Observe). ✅
- **Tainted body/url Block:** `http_write_valid_method_tainted_body_still_blocks` and `..._tainted_url_still_blocks` prove I2 Blocks on the real content/routing arms; untainted-body node reaches Allowed (`..._clean_body_allowed`). ✅
- **method enum fail-closed:** invalid (`GET`), mis-cased (`post`), empty (`   `), and tainted-garbage (`POST\r\nHost: evil`) methods all Deny `InvalidMethod`. ✅
- **Policy allowlist:** `broker_default_permits_http_request_write` + updated eight-sink assertion; `allow_all()` reaches the unmodified I2 loop (no PolicyDeny for the new sink). ✅

## Test results

- `cargo test -p executor --no-fail-fast` — **100 + 29 + 4 = 133 passed, 0 failed** (unit + executor_decision + policy_gate targets).
- `cargo test -p runtime-core --no-fail-fast` — **53 passed, 0 failed** (incl. policy 15, executor_decision 3 for the new variant).
- `cargo test --workspace --no-fail-fast` — **495 passed, 0 failed** (macOS host; `#[cfg(target_os="linux")]` security tests compile to no-ops here, as expected per CLAUDE.md — not a gap).
- `cargo build --workspace` — clean.
- `./scripts/check-invariants.sh` — **all gates PASSED** (Gate 1 no EffectRequest, Gate 3 no new mint site, Gate 5 ring-only crypto, etc.). This plan touches no sink dispatch and mints nothing.

24 tests added (8 schema, 5 sensitivity, 10 executor_decision integration, 1 runtime-core decision unit).

## Deviations from Plan

None — plan executed as written. The method-enum gate landed at Step 0.3 (after the Step-0.25 policy gate, before the collect-then-Block loop) as the plan's ordering directive required; an unresolvable method handle deliberately falls through to the loop's existing `DanglingHandle` Deny rather than duplicating that logic (still fail-closed, never Allowed).

## Known Stubs

None. This plan is executor-TCB classification/gating only; the broker network dispatch for `http.request.write` is Plan 02+ scope (documented in the plan objective, not a stub).

## Self-Check: PASSED

- Commits `bb64ce1`, `3c16081`, `4415b86` present in `git log`.
- All six modified files exist and compile; `cargo build --workspace` clean.
- check-invariants.sh all gates PASSED.
