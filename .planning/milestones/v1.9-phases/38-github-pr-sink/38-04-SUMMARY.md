---
phase: 38-github-pr-sink
plan: 04
subsystem: brokerd-server
tags: [github, github-pr, allowed-dispatch, grant-gate, cas, at-most-once, opaque-audit, two-phase]
status: complete

# Dependency graph
requires:
  - phase: 38-02
    provides: "has_github_grant gate, github_pr_content_key, reserve_created_pr CAS, record_github_grant + session_grants/created_prs tables"
  - phase: 38-03
    provides: "invoke_github_pr (Allowed-path sink, Wired by Plan 38-04) + prepare_github_pr + opaque github_pr_succeeded/_failed audit"
  - phase: 29-harden-idempotency
    provides: "email.send sent_plan_nodes CAS + attempt-ledger commit-before-effect pattern (mirrored here verbatim)"
  - phase: 37-http-request-sink
    provides: "http.request arm's opaque _failed terminal event + never-hold-mutex-across-await lock discipline"
provides:
  - "server.rs evaluate_plan_node_and_record — the Allowed github.pr dispatch arm (grant gate FIRST -> content CAS before POST -> invoke_github_pr on fresh only)"
  - "crates/brokerd/tests/s38_github_pr.rs — host-portable no-grant Deny + duplicate-PR at-most-once + opaque-token tests"
affects: [38-05, phase-40-mock, github-pr-dispatch]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two independent gates ahead of an Allowed sink: capability grant gate FIRST (fail-closed), then a content-derived at-most-once CAS committed BEFORE the socket"
    - "Divergent attempt/replay-suppressed marker in one transaction (mirror email.send sent_plan_nodes) keeps github_pr_attempted at exactly one per content across replays"
    - "Terminal EVENT before any terminal disposition on every denied/replay/attempt path (P33/P34 audit-gap discipline); no clear-key-on-failure (§11 MAJOR-5)"
    - "Host-portable sink tests drive the composed public primitives directly (no Linux UDS, live POST is the macOS no-op stub) — real-socket proof deferred to Phase 40"

key-files:
  created:
    - crates/brokerd/tests/s38_github_pr.rs
  modified:
    - crates/brokerd/src/server.rs

key-decisions:
  - "Server arm calls invoke_github_pr (NOT invoke_github_pr_from_resolved): the plan <action> step (4) and the sink's own doc comment both pin invoke_github_pr as 'Wired by Plan 38-04'. _from_resolved takes a pre-locked &Connection, which would force holding the audit mutex across the POST .await — explicitly forbidden. invoke_github_pr takes &Arc<Mutex<Connection>> and locks internally only for its terminal append, never across the socket."
  - "Grant gate is FIRST and fail-closed: absent has_github_grant, append an OPAQUE github_pr_denied terminal event (raw reason to eprintln only) and STOP — no content key, no CAS row, no POST. A bare Allowed decision cannot create a PR (GITHUB-02, DESIGN §4.3/§8)."
  - "The created_prs INSERT-OR-IGNORE (reserve_created_pr) and the divergent github_pr_attempted / github_pr_replay_suppressed marker commit in ONE transaction BEFORE any socket opens — mirrors the email.send CAS+attempt-ledger commit-before-effect exactly (GITHUB-04, §4.5). No clear-key-on-failure."
  - "On the fresh branch, invoke_github_pr's Err is propagated with `?` AFTER the sink has already appended its own durable opaque github_pr_failed terminal event (mirror email.send/process.exec/git.commit `?` discipline) — the terminal EVENT precedes the error unwind, never a burned dispatch with no terminal event (P33/P34 class)."
  - "github.pr mints nothing: output_value_id is left None on this branch (Gate 3 mint-site allow-list unchanged). check-invariants exits 0 (no EffectRequest, no new mint site)."
  - "Task 2 tests are HOST-PORTABLE by driving the broker-side primitives the arm composes (has_github_grant -> reserve_created_pr -> invoke_github_pr_from_resolved) in the arm's exact order against an in-memory audit db + frozen ResolvedArg snapshot — same direct-in-process style as s37_http_request.rs (which drives mint_from_http directly rather than over the Linux-only abstract UDS). The real-socket end-to-end create-PR proof is the Phase-40 mock / composed live step."

metrics:
  duration_min: 18
  completed: 2026-07-18
  tasks: 2
  files_created: 1
  files_modified: 1
  tests_added: 2
---

# Phase 38 Plan 04: github.pr Allowed-Dispatch (Grant Gate + CAS + POST) Summary

Wired the `github.pr` Allowed-decision dispatch in `server.rs`'s
`evaluate_plan_node_and_record`, composing the capability grant gate
(GITHUB-02), the duplicate-PR content CAS (GITHUB-04), and the sink POST
(GITHUB-01) into the never-blocked (untainted) path — two independent gates
stand ahead of any GitHub socket, so a bare Allowed decision alone cannot create
a PR, and a replayed identical submission creates at most one PR.

## What was built

**Task 1 — the Allowed `github.pr` arm** (`crates/brokerd/src/server.rs`,
appended after the `http.request` arm inside `evaluate_plan_node_and_record`):

1. **Grant gate FIRST** — `has_github_grant(&locked, session_id)`. Absent a live
   grant: append an OPAQUE `github_pr_denied` terminal event (actor
   `sink:github.pr:{effect_id}`, empty payload; raw reason to `eprintln` only),
   advance the head, and STOP. No content key, no CAS, no POST.
2. **Resolve the six args** (owner/repo/base/head/title/body) from the live
   per-connection `ValueStore` (fail-closed on a dangling handle) and compute
   `github_pr_content_key`.
3. **CAS before effect** — `reserve_created_pr` + the divergent
   `github_pr_attempted` (fresh) / `github_pr_replay_suppressed` (replay) marker
   commit in ONE transaction BEFORE any socket (mirror the `email.send`
   `sent_plan_nodes` CAS). No clear-key-on-failure.
4. **Fresh branch only** calls `invoke_github_pr` (locks `conn` only for its
   terminal append, never across the POST `.await`); `?`-propagates its Err after
   the sink's own durable opaque `github_pr_failed`. Mints nothing.

**Task 2 — host-portable tests** (`crates/brokerd/tests/s38_github_pr.rs`):
- `github_pr_without_grant_denies_no_attempt` (GITHUB-02): ungranted github.pr →
  one `github_pr_denied`, zero `github_pr_attempted`, zero `created_prs` rows,
  chain intact.
- `github_pr_replay_creates_at_most_one` (GITHUB-04/01): with a grant, two
  identical submits → exactly one `github_pr_attempted`, one
  `github_pr_replay_suppressed`, one `created_prs` row; `verify_chain` true; the
  bearer token literal absent from every audit payload.

## Verification

- `cargo build --workspace` — success.
- `cargo test -p brokerd` — **224 passed, 0 failed** (macOS host). The two new
  `s38_github_pr` tests pass; Linux-gated security tests compile to 0 on macOS
  (expected per CLAUDE.md, not a gap).
- `./scripts/check-invariants.sh` — **exits 0** (Gate 1 no EffectRequest, Gate 3
  mint-site allow-list byte-identical: github.pr mints nothing).
- `verify_chain` asserted true in both tests.

## Deviations from Plan

**1. [Clarification — dispatch prompt named the wrong sink entry point]**
The orchestrator's dispatch prompt said to call `invoke_github_pr_from_resolved`.
The PLAN's own `<action>` step (4) says `invoke_github_pr(...)`, and the sink's
doc comment pins `invoke_github_pr` as "Wired by Plan 38-04 (server.rs
Allowed-decision dispatch)" while `invoke_github_pr_from_resolved` is pinned to
Plan 38-05 (confirm-release) and marked `#[allow(dead_code)]`. Decisively:
`_from_resolved` takes a pre-locked `&rusqlite::Connection`, so calling it from
the async arm would force holding the audit mutex across the POST `.await` —
which the plan explicitly forbids ("NEVER hold the mutex across the sink's
`.await`"). `invoke_github_pr` takes `&Arc<Mutex<Connection>>` and locks
internally only for its terminal append. Followed PLAN + code (source of truth
per CLAUDE.md). No functional divergence from the plan's intent.

## Parallel-wave note (not my change)

The Wave-3 sibling Plan 38-05 (running concurrently in the same shared working
tree) has uncommitted edits to `crates/brokerd/src/sinks/github_pr.rs` (a shared
`pub(crate) GITHUB_ENV_LOCK`) and `crates/brokerd/src/confirmation.rs`. These
produce one `dead_code` warning (`GITHUB_ENV_LOCK never used`) until 38-05's
confirmation.rs consumer lands. This is 38-05's in-progress work — OUT of this
plan's scope, NOT staged or committed by this plan (I committed only
`server.rs` and `tests/s38_github_pr.rs`).

## Known Stubs

None. The macOS `do_pinned_post` no-op stub is a pre-existing, intentional
Linux-only-socket boundary (38-03), not a stub introduced here; the live POST is
Phase-40 mock territory.

## Self-Check: PASSED
- `crates/brokerd/src/server.rs` — FOUND (github.pr arm committed in 45bde34).
- `crates/brokerd/tests/s38_github_pr.rs` — FOUND (committed in 9fa692b).
- Commit `45bde34` (feat, Task 1) — FOUND.
- Commit `9fa692b` (test, Task 2) — FOUND.
