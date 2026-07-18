---
phase: 43-http-request-write-post-put-egress
plan: 04
subsystem: brokerd / acceptance
tags: [http-write, HTTP-W-01, differential, i2, i0, taint, egress, acceptance-test]
requires:
  - "43-01: http.request.write executor-TCB classification (CommitIrreversible + method-enum gate)"
  - "43-02: WRITE egress path (SSRF-pinned, empty WRITE_HOST_ALLOWLIST, opaque non-minting audit)"
  - "43-03: Allowed-dispatch (server.rs) + confirm-release (confirmation.rs) wiring"
provides:
  - "HTTP-W-01 differential acceptance proof: taint is the sole variable between a Blocked and an Allowed write"
affects:
  - "crates/brokerd/tests/ (new integration test only — no production code touched)"
tech-stack:
  added: []
  patterns:
    - "differential (anti-stapling / anti-regression) acceptance: clean leg MUST Allow+reach-egress, tainted leg MUST Block"
    - "real-dispatch integration via evaluate_plan_node_and_record_for_test (no hand-rolled mirror)"
    - "genuine taint minted through mint_from_http (real http_response_received provenance), never a hand-set field"
key-files:
  created:
    - "crates/brokerd/tests/s43_http_write_differential.rs"
  modified: []
decisions:
  - "Task 2 dispatch legs kept HOST-PORTABLE (not #[cfg(target_os=linux)]-gated): the default empty WRITE_HOST_ALLOWLIST bails pre-socket identically on macOS and Linux, so the terminal http_write_failed event proves egress-reached on both — a Linux gate would only make the test a macOS no-op, reducing coverage. Genuine live mock-endpoint delivery = Phase 46 LIVE-06."
metrics:
  duration: ~35m
  completed: 2026-07-18
  tasks: 2
  files: 1
  commits: 2
status: complete
---

# Phase 43 Plan 04: HTTP-W-01 Differential Acceptance Test Summary

A committed, green differential acceptance test that proves HTTP-W-01 the hard
way: with host/url/method/policy held byte-identical, a **tainted** request `body`
Blocks under I2 while a **clean** body is Allowed and reaches the live write
dispatch — taint the sole variable — plus the I0 draft-deny leg and the
method-enum fail-closed Deny. No production code was touched; this plan is the
requirement proof for the sink wired by 43-01/02/03.

## What was built

`crates/brokerd/tests/s43_http_write_differential.rs` (679 lines, 5 tests):

**Task 1 — decision-level (host-portable, 3 tests):**
- `leg_a_i0_draft_session_denies_write_commit_irreversible` — a draft session +
  `http.request.write` (clean body, valid POST) Denies
  `DraftOnlySessionDeniesCommitIrreversible`, proving the distinct WRITE id is
  `CommitIrreversible` (the MAJOR-1 I0-escape fix; the GET id is Observe).
- `legs_b_and_c_taint_is_the_sole_variable` — the differential core. The SAME
  `url`/`method` value handles are reused in both plan nodes; the SAME
  `broker_default()` policy and `Active` status drive both; the body literal is
  identical. Only the body's **taint** differs (tainted via `mint_from_http`,
  clean via `mint_from_intent`). LEG B Blocks with a single anchor naming `body`
  (genuine, non-empty provenance chain, `read_event_id == provenance_chain[0]`);
  LEG C is `Allowed`. Byte-identical url/method handles + literals are asserted
  literally, so a future divergence fails the test. `verify_chain` asserted true.
- `leg_d_method_outside_enum_denies_fail_closed` — a `DELETE` method Denies
  `InvalidMethod` at the fail-closed enum gate, never a confirmable Block.

**Task 2 — dispatch-level (host-portable, 2 tests):** drive the ACTUAL production
dispatch arm via `brokerd::server::evaluate_plan_node_and_record_for_test` (the
`test-fixtures`-gated verbatim delegate — no hand-rolled mirror).
- `dispatch_clean_body_reaches_egress_terminal_event` — the clean Allowed decision
  flows into `invoke_http_write_sink`, bails at the empty `WRITE_HOST_ALLOWLIST`
  (pre-socket), and appends a terminal `http_write_failed` FIRST then propagates
  Err (expected). Asserts: exactly one `http_write_failed`, zero
  `http_write_succeeded`, **zero `sink_blocked`** (a blanket-block regression would
  fail here), url+body absent from every payload (opaque), `verify_chain` true.
- `dispatch_tainted_body_blocks_and_never_writes` — the tainted decision records
  one `sink_blocked` on `body` and **never enters the write arm** (zero
  `http_write_*` terminal events), `output_value_id` None, `verify_chain` true. A
  blanket-allow regression would emit an `http_write_*` here and fail.

## The differential property (why "not blocked" alone is insufficient)

- A **block-everything I2 regression** fails LEG C and the clean dispatch leg (the
  clean body must Allow and reach egress).
- A **blanket-allow regression** fails LEG B and the tainted dispatch leg (the
  tainted body must Block, no write).
- The tainted body is minted through the **real** `mint_from_http` path (a genuine
  `http_response_received`-rooted provenance chain), so the Block rides a real
  audit-DAG edge — never a stapled tag (T-43-13). `verify_chain` is asserted true
  on every leg.

## Test results

`cargo test -p brokerd --test s43_http_write_differential --no-fail-fast` →
**5 passed / 0 failed** on the macOS host.

| Test | Level | Host-portable? |
|------|-------|----------------|
| leg_a_i0_draft_session_denies_write_commit_irreversible | decision | yes |
| legs_b_and_c_taint_is_the_sole_variable | decision | yes |
| leg_d_method_outside_enum_denies_fail_closed | decision | yes |
| dispatch_clean_body_reaches_egress_terminal_event | dispatch | yes (see note) |
| dispatch_tainted_body_blocks_and_never_writes | dispatch | yes (see note) |

**Linux-gated vs host-portable note:** all 5 legs run on the macOS host. Under the
default feature set `WRITE_HOST_ALLOWLIST` is empty, so `invoke_http_write` bails
at the write-allowlist gate BEFORE any DNS/socket on both macOS and Linux —
appending the opaque `http_write_failed` terminal identically. Nothing here touches
a socket, so no `#[cfg(target_os="linux")]` gate was added (it would only make the
dispatch legs macOS no-ops, reducing coverage). The genuine live mock-endpoint
DELIVERY (`mock-egress-ca` feature, real TLS POST on Linux) + credential-absence-
after-a-real-write assertion compose in **Phase 46 (LIVE-06)**, as scoped by the
plan.

`cargo build --workspace` run before tests (sibling-binary rule): clean.
`./scripts/check-invariants.sh`: **All invariant gates PASSED** (Gates 1–6,
including Gate 3 mint-call-site restriction and Gate 4/4b feature-default checks).

## Deviations from Plan

**1. [Rule 3 - host-portability] Task 2 dispatch legs are host-portable, not
`#[cfg(target_os="linux")]`-gated.**
- **Found during:** Task 2.
- **Issue:** The plan directed gating the "socket-touching" dispatch assertions
  behind `#[cfg(target_os="linux")]`. In reality, the default empty
  `WRITE_HOST_ALLOWLIST` (`crates/brokerd/src/sinks/http_request.rs`) causes
  `invoke_http_write` to bail at the allowlist gate BEFORE any socket, on BOTH
  platforms — so there are no socket-touching assertions to gate; the terminal
  `http_write_failed` event proves egress-reached identically on macOS and Linux.
- **Resolution:** Kept the dispatch legs host-portable (a Linux gate would make
  them macOS no-ops, reducing coverage). Documented the rationale inline in the
  test file's Task-2 header and here. The genuine live-socket delivery remains
  Phase 46 (LIVE-06), matching the plan's scoping.
- **Files modified:** test only.

**2. [Process] Task 1 commit omitted the Co-Authored-By trailer.**
- The Task 1 commit `661ab19` does not carry the `Co-Authored-By: Claude Opus 4.8`
  trailer; the Task 2 commit `2a971ff` does. History was not rewritten to fix the
  earlier commit (both are local, unpushed) to preserve the clean two-commit
  split; recorded here for honesty.

No production code was changed, so no auto-fixed bugs / missing-functionality /
threat-flag deviations apply.

## Commits

- `661ab19` test(43-04): decision-level HTTP-W-01 differential (I0/taint/method legs)
- `2a971ff` test(43-04): dispatch-level HTTP-W-01 differential (egress reached vs blocked)

## Self-Check: PASSED

- `crates/brokerd/tests/s43_http_write_differential.rs` — FOUND on disk.
- Commit `661ab19` — FOUND in git log.
- Commit `2a971ff` — FOUND in git log.
- `cargo test -p brokerd --test s43_http_write_differential` — 5 passed / 0 failed.
- `./scripts/check-invariants.sh` — all gates PASSED.
