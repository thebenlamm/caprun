---
phase: 37-http-request-get-egress
plan: 01
subsystem: security
tags: [taint, executor, sink-schema, http, egress, i2]

# Dependency graph
requires:
  - phase: 36-git-commit
    provides: git.commit exact-match single-arg sink + MutateReversible exemplar to mirror
provides:
  - TaintLabel::HttpRaw — compile-forced untrusted external-response-body label (the label Plan 03's inbound mint stamps)
  - http.request registered in KNOWN_SINKS (exact-match single-arg {url})
  - sink_effect_class(http.request) == Observe (first real Observe sink)
  - url routing- AND content-sensitive; expected_role(url) == None
affects: [37-02, 37-03, http egress module, inbound response mint]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "New untrusted TaintLabel added to the exhaustive is_untrusted() match (no wildcard) — omission is a compile error"
    - "Observe-classified read sink: table row only, no new ExecutorDecision variant, I2 unweakened"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/plan_node.rs
    - crates/runtime-core/tests/intent_taint.rs
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs
    - crates/brokerd/src/confirmation.rs

key-decisions:
  - "HttpRaw added to is_untrusted() exhaustive match (=> true) with NO wildcard — a future omission is a build failure, not a silent false-allow (DESIGN §3.4)"
  - "http.request url is BOTH routing- and content-sensitive (NIT-6/§8 defense-in-depth) — a secret in the query string is exfil"
  - "expected_role(http.request, url) == None — unconstrained structural gate; the Block comes from routing/content-sensitivity + taint, exactly like process.exec/git.commit; NOT an I2 bypass"
  - "GET only this milestone — no method/headers/body args; those Deny(UnknownArg) at Step 0"

patterns-established:
  - "Adding a TaintLabel variant compile-forces every exhaustive match; the only two are is_untrusted() and taint_label_display()"

requirements-completed: [HTTP-01, HTTP-02]

coverage:
  - id: D1
    description: "TaintLabel::HttpRaw exists and is_untrusted() returns true (compile-forced into the exhaustive match)"
    requirement: "HTTP-02"
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_http_raw_returns_true"
        status: pass
    human_judgment: false
  - id: D2
    description: "http.request registered in KNOWN_SINKS with exact-match {url} schema (url allowed AND required); unknown/duplicate/missing arg Denied"
    requirement: "HTTP-01"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#http_request_is_registered_sink,http_request_exact_args_ok,http_request_unknown_arg_denied,http_request_duplicate_arg_denied,http_request_missing_required_arg_denied"
        status: pass
    human_judgment: false
  - id: D3
    description: "sink_effect_class(http.request) == Observe (first real Observe sink)"
    requirement: "HTTP-01"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#http_request_is_observe"
        status: pass
    human_judgment: false
  - id: D4
    description: "http.request url is routing- AND content-sensitive; expected_role(url) == None (unconstrained structural gate)"
    requirement: "HTTP-01"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#http_request_url_is_routing_and_content_sensitive,http_request_url_expected_role_is_none"
        status: pass
    human_judgment: false

# Metrics
duration: 12min
completed: 2026-07-18
status: complete
---

# Phase 37 Plan 01: http.request runtime-core + executor table foundations Summary

**TaintLabel::HttpRaw (compile-forced untrusted) plus the hardcoded executor rows that make http.request a callable, Observe-classified GET sink whose single `url` arg is routing- AND content-sensitive with an unconstrained role gate — no network, mint, or dispatch code (those are Plans 02/03).**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-07-18T07:24Z
- **Completed:** 2026-07-18T07:36Z
- **Tasks:** 2
- **Files modified:** 5 (2 source + 1 test in runtime-core; 2 source in executor; 1 Rule-3 fix in brokerd)

## Accomplishments
- `TaintLabel::HttpRaw` added after `ExecRaw`, wired into `is_untrusted()`'s exhaustive `match self` (`=> true`) with no wildcard — a future omission is a compile error, not a silent false-allow.
- `http.request` registered in `KNOWN_SINKS` with an exact-match single-arg `{url}` schema (allowed AND required; GET only, no method/headers/body).
- `sink_effect_class(http.request) == Observe` — the first real `Observe` sink (only the test-only `test.observe` was Observe before); a read Allowed even in a Draft session.
- `url` is BOTH routing- and content-sensitive (`HTTP_REQUEST_ROUTING_SENSITIVE`/`HTTP_REQUEST_CONTENT_SENSITIVE`); `expected_role(http.request, "url") == None` — unconstrained structural gate, the Block comes entirely from sensitivity + taint.

## Task Commits

1. **Task 1: TaintLabel::HttpRaw, compile-forced untrusted** — `0041ab7` (feat, TDD test+impl)
2. **Task 2: register http.request Observe sink + url sensitivity rows** — `1b1e046` (feat, TDD test+impl; includes Rule-3 brokerd display fix)

_TDD note: RED verified for both tasks (Task 1 compile-error on missing variant; Task 2 seven failing tests) before GREEN._

## Files Created/Modified
- `crates/runtime-core/src/plan_node.rs` — added `HttpRaw` variant + untrusted-arm wiring; doc count seven→eight.
- `crates/runtime-core/tests/intent_taint.rs` — `is_untrusted_http_raw_returns_true` truth-table test.
- `crates/executor/src/sink_schema.rs` — `http.request` `KNOWN_SINKS` row + 5 schema tests.
- `crates/executor/src/sink_sensitivity.rs` — Observe arm, routing/content consts + wiring, `expected_role` arm + 3 tests.
- `crates/brokerd/src/confirmation.rs` — `http.raw` arm in `taint_label_display` exhaustive match (Rule 3).

## Decisions Made
- Followed plan as specified. `url` content-sensitivity retained per NIT-6/§8 (query-string exfil defense-in-depth).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `http.raw` arm to `taint_label_display`**
- **Found during:** Task 2 (full `cargo build --workspace`)
- **Issue:** The new `TaintLabel::HttpRaw` variant compile-forced a non-exhaustive-match error (E0004) in `brokerd/src/confirmation.rs:524`'s `taint_label_display` — a second exhaustive `match label` sharing the same Pitfall-5 no-wildcard discipline as `is_untrusted()`.
- **Fix:** Added `TaintLabel::HttpRaw => "http.raw"` (dotted-lowercase CLI display rendering, mirroring `exec.raw`/`path.raw`).
- **Files modified:** crates/brokerd/src/confirmation.rs
- **Verification:** `cargo build --workspace` clean; grep confirmed these are the ONLY two exhaustive `TaintLabel` matches in the tree (no cfg-linux-gated arms — grep ignores cfg).
- **Committed in:** `1b1e046` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking). This is the intended compile-time enforcement of the exhaustive-match discipline, not scope creep — the plan's own key_links note that omission must be a compile error.
**Impact on plan:** In-scope; the fix is a mechanical display arm. No behavior change.

## Issues Encountered
None beyond the expected compile-forced brokerd arm.

## Test Results (real counts)
- `cargo test -p runtime-core`: 4 + 12 + 7 + 13 = 36 passed, 0 failed (incl. `is_untrusted_http_raw_returns_true`).
- `cargo test -p executor`: 74 + 20 = 94 passed, 0 failed (incl. all 8 new http.request tests).
- `cargo build --workspace`: clean.
- `./scripts/check-invariants.sh`: exit 0 (Gate 1 no EffectRequest token; Gate 2 runtime-core purity unaffected).

_No Linux-only tests were run: this is a wave-1 no-I/O pure-type/table change with no `#[cfg(target_os="linux")]` code paths introduced._

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- 37-02 (egress module) and 37-03 (inbound response mint) can now reference `TaintLabel::HttpRaw` and the `http.request` sink rows.
- No network, mint, or dispatch code exists yet (deliberately deferred to Plans 02/03).

## Self-Check: PASSED
- All 5 modified files present on disk.
- Both task commits (`0041ab7`, `1b1e046`) present in git history.

---
*Phase: 37-http-request-get-egress*
*Completed: 2026-07-18*
