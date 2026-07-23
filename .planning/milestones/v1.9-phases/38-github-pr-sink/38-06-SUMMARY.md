---
phase: 38-github-pr-sink
plan: 06
subsystem: api
tags: [github, test-coverage, adversarial-review, real-dispatch, brokerd]

# Dependency graph
requires:
  - phase: 38-04
    provides: server.rs Allowed-github.pr dispatch arm (grant gate -> content CAS -> POST) in evaluate_plan_node_and_record
provides:
  - test-fixtures-gated pub delegate evaluate_plan_node_and_record_for_test (verbatim forwarder to the real dispatch fn)
  - real-function integration test for the github.pr no-grant -> Deny case (drives production code, not a mirror)
affects: [github, git-github-adapters, test-coverage]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "test-fixtures-gated pub delegate to a crate-private dispatch fn so tests/ can drive the REAL arm without widening the production API (mirrors the HARDEN-04 CreateSession mint-arm discipline)"

key-files:
  created: []
  modified:
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/s38_github_pr.rs
    - crates/brokerd/Cargo.toml

key-decisions:
  - "Achieved a REAL-function test (not the documented-mirror fallback): the no-grant Deny leg Denies at the grant gate before any content key/CAS/POST, so it is fully host-portable and needs no live GitHub or Linux UDS."
  - "Exposed the crate-private evaluate_plan_node_and_record via a #[cfg(any(test, feature = \"test-fixtures\"))] pub verbatim delegate rather than making it unconditionally pub — zero production API surface added (test-fixtures is never a default feature, enforced by check-invariants Gate 4)."
  - "Left the existing dispatch_github_pr_like_arm mirror in place (its replay/CAS/POST leg still needs Phase-40's composed live-proof) and annotated it to point at the new real-arm coverage."

status: complete
---

# Phase 38 Plan 06: github.pr Real-Dispatch Test-Coverage Fix Summary

Closes Phase-38 adversarial review finding #2: the s38 integration tests only
exercised a hand-rolled mirror (`dispatch_github_pr_like_arm`) of the real
`github.pr` Allowed-dispatch arm, so mirror/real drift went uncaught (a
false-assurance regression test).

## Follow-up note (adversarial finding #2)

- Added `github_pr_without_grant_denies_via_real_dispatch` to
  `crates/brokerd/tests/s38_github_pr.rs`. It builds a `ValueStore` of six
  UserTrusted args + a `github.pr` `PlanNode`, then drives the REAL production
  `evaluate_plan_node_and_record` (through a `test-fixtures`-gated verbatim
  delegate) with a session holding NO github grant. The executor Allows the
  untainted node; the arm's own grant gate turns that into an opaque
  `github_pr_denied` — asserting exactly one `github_pr_denied`, zero
  `github_pr_attempted`, zero `created_prs` CAS rows, and `verify_chain`
  intact. No CAS row and no POST are ever reached (denied before them).
- Production change kept minimal: a `#[cfg(any(test, feature =
  "test-fixtures"))]` `pub` delegate `evaluate_plan_node_and_record_for_test`
  that forwards verbatim to the private real fn (no security logic touched;
  absent from production builds). Added `adapter-fs` as a dev-dependency for
  the `WorkspaceRoot` arg.
- Outcome: REAL-function test achieved (not the documented-mirror fallback).
  `crates/brokerd/tests/s38_github_pr.rs` now has 3 tests (was 2), all green;
  `check-invariants.sh` exits 0; `cargo test -p brokerd` green (178+ unit,
  s38 3/3, no regressions).
