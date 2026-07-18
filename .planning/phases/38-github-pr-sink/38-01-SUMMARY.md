---
phase: 38-github-pr-sink
plan: 01
subsystem: executor
status: complete
tags: [github.pr, sink-schema, sink-sensitivity, i2, taint, GITHUB-01, GITHUB-03]
requires: []
provides:
  - "github.pr registered sink (exact six-arg schema {owner,repo,base,head,title,body})"
  - "github.pr CommitIrreversible effect-class (explicit arm)"
  - "github.pr title/body content-sensitive; owner/repo/base/head routing-sensitive"
  - "github.pr expected_role None for all six args (role-unconstrained)"
affects:
  - "38-02/03/04/05 dispatch paths (server.rs Allowed, confirmation.rs confirm-release) build on these rows"
tech-stack:
  added: []
  patterns:
    - "table-rows-only sink extension (v1.5/v1.7 discipline, DESIGN §7)"
    - "exact-match all-required schema (file.write shape, not process.exec optional-arg asymmetry)"
    - "explicit effect-class arm over the `_` fail-closed default (T-38-03 defense)"
    - "expected_role None = role-gate disabled, Block delivered by routing/content-sensitivity + taint (reused process.exec/git.commit/http.request rationale)"
key-files:
  created: []
  modified:
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs
decisions:
  - "github.pr models exactly {owner,repo,base,head,title,body} — no draft/maintainer_can_modify/headers/method args this milestone (PR-create scope); extras fail closed (UnknownArg) at Step 0"
  - "title/body content-sensitive (marquee exfil arg), owner/repo/base/head routing-sensitive; no over-widening of routing args into content-sensitive"
  - "CommitIrreversible is a REAL explicit arm, not the `_` default, so a reorder cannot silently relax it"
  - "expected_role None for all six args — no origin_role mint site for a legit PR field; pinning Some(...) would fail-closed-Deny the legit flow; NOT an I2 bypass"
  - "github.pr does NOT mint — Gate 3 allow-list unchanged"
metrics:
  duration: "~15m"
  completed: 2026-07-18
  tasks: 2
  files: 2
---

# Phase 38 Plan 01: github.pr Executor Rows Summary

Registered `github.pr` as a first-class executor sink via table rows only — hardcoded exact six-arg schema, an explicit `CommitIrreversible` effect-class, and routing/content sensitivity (title/body content-sensitive, owner/repo/base/head routing-sensitive) — the TCB foundation both 38-02/03/04/05 dispatch paths build on, with no new `ExecutorDecision` variant, no new enforcement step, and no policy file.

## What was built

**Task 1 — `crates/executor/src/sink_schema.rs`:** Added a `KNOWN_SINKS` row for `github.pr` with `allowed == required == {owner,repo,base,head,title,body}` (exact-match, all six required — mirrors `file.write`, not `process.exec`'s optional-arg asymmetry). Any extra arg (`draft`, `headers`, `method`, …) is `Denied(UnknownArg)` at Step 0; a repeat is `DuplicateArg`; any of the six absent is `MissingArg`. Doc comment states the schema gate enforces only the arg NAME set, not taint.

**Task 2 — `crates/executor/src/sink_sensitivity.rs`:**
- `sink_effect_class`: explicit `"github.pr" => CommitIrreversible` arm (not the `_` fail-closed default — T-38-03).
- `GITHUB_PR_ROUTING_SENSITIVE = &["owner","repo","base","head"]` and `GITHUB_PR_CONTENT_SENSITIVE = &["title","body"]` consts with rationale doc comments.
- `"github.pr"` arms in `is_routing_sensitive` / `is_content_sensitive` delegating to the two consts.
- `"github.pr"` arm in `expected_role` returning `None` for all six args (role-unconstrained; Block delivered by routing/content-sensitivity + untrusted-taint check, not the structural role gate).

A tainted `title`/`body` now Blocks under the unmodified collect-then-Block loop (GITHUB-03 secret-exfil-via-PR-text defense); a tainted `owner`/`repo`/`base`/`head` Blocks as PR mis-routing (T-38-02). End-to-end Block is proven in 38-05.

## Tests added

Schema (`sink_schema::tests`): `github_pr_is_registered_sink`, `github_pr_exact_args_ok`, `github_pr_unknown_arg_denied`, `github_pr_duplicate_arg_denied`, `github_pr_missing_required_arg_denied`.

Sensitivity (`sink_sensitivity::tests`): `github_pr_is_commit_irreversible`, `github_pr_title_body_content_sensitive`, `github_pr_owner_repo_base_head_routing_sensitive`, `github_pr_routing_args_not_content_sensitive`, `github_pr_expected_role_is_none`.

## Verification results

- `cargo build --workspace` — clean (exit 0).
- `cargo test -p executor` — **84 lib passed (+10 new, was 74), 20 integration passed, 0 doctests; 0 failed.**
- `./scripts/check-invariants.sh` — exits 0; Gate 3 (mint allow-list) unchanged (github.pr does NOT mint).

## TDD Gate Compliance

Each task followed RED → GREEN with separate commits:
- `test(38-01)` schema tests (RED: `UnknownSink` at Step 0) → `feat(38-01)` schema row (GREEN).
- `test(38-01)` sensitivity tests (RED: title/body not content-sensitive, owner/repo/base/head not routing-sensitive) → `feat(38-01)` sensitivity rows (GREEN).

## Deviations from Plan

None — plan executed exactly as written. Scope held to 38-01's two files; brokerd/CLI untouched (38-02/03/04/05).

## Threat surface scan

No new security-relevant surface beyond the plan's `<threat_model>` (T-38-01/02/03 all mitigated by the rows delivered). No new endpoints, auth paths, or schema changes at trust boundaries.

## Self-Check: PASSED

- `crates/executor/src/sink_schema.rs` — FOUND (github.pr KNOWN_SINKS row present)
- `crates/executor/src/sink_sensitivity.rs` — FOUND (github.pr arms + consts present)
- Commits present: 442acf9, 6d4a732, 8cefcbd, 06d5eb5 (verified in git log)
