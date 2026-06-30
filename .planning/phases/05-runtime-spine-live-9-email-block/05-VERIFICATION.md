---
phase: 05-runtime-spine-live-9-email-block
status: passed
verified_by: orchestrator (inline — verifier subagent died on a connection drop before writing this report)
verified_date: 2026-06-30
requirements: [ASM-01, ASM-02, ASM-03, ASM-04, HARD-03, ACC-02]
must_haves_verified: 6
must_haves_total: 6
---

# Phase 05 Verification — Runtime Spine & Live §9 Email Block

**Verdict: PASSED.** The phase goal is achieved in the codebase, the architectural invariants hold, the non-negotiable §9 acceptance test is green, and the live §9 block is proven end-to-end on kernel-confined binaries under Colima/Docker.

> Verification method: this report was produced inline by the orchestrator. The `gsd-verifier` subagent was dispatched but terminated on an API streaming-connection drop (the same instability that hit the executor subagents) after 56 tool calls without writing the report. Goal-backward analysis was re-run inline against the live codebase; every check below is grounded in a command that was actually run.

## Phase Goal
`caprun` operates through a single unified `brokerd::server` dispatch path — no second executor loop — with a session-scoped broker state model, and a real `caprun` invocation on hostile input fires a live §9 block with a durable causal `sink_blocked` audit event through the existing `email.send` stub.

## Requirement-by-Requirement

| Req | Verdict | Evidence |
|-----|---------|----------|
| **ASM-01** (single dispatch path) | ✅ | `grep -rc handle_worker_connection cli/caprun/src/` = 0; `main.rs` delegates to `brokerd::server::run_broker_server` (5 refs). No second worker-connection loop in the caprun binary. |
| **ASM-02** (placeholder gone; live executor) | ✅ | `grep -rc "not wired" crates/ cli/` = 0; `server.rs` calls `executor::submit_plan_node` live (2 refs). |
| **ASM-03** (typed ReportClaims, no raw bytes) | ✅ | `worker.rs` extracts claims locally and sends `ReportClaims` (4 refs); `ReportRead` removed (0). Only `WorkerClaim::EmailAddress` crosses IPC. |
| **ASM-04** (genuine-taint mint anchor) | ✅ | `server.rs` mints via `quarantine::mint_from_read` (sole site, 4 refs); 0 direct `store.mint` on the live path. `phase5_dispatch::mint_anchors_provenance_to_file_read_event` + `s9_acceptance` assert `provenance_chain[0] == file_read.id`. |
| **HARD-03** (session-scoped handles; no trusted msg session_id) | ✅ | `SubmitPlanNode` has no `session_id` field (proto grep = 0); per-connection `ValueStore::default()` in `handle_connection`; `phase5_dispatch::handle_from_other_connection_store_is_denied` proves cross-connection handle → `Denied`. |
| **ACC-02** (durable causal fail-closed sink_blocked) | ✅ | `server.rs` appends `sink_blocked` with `parent_id: Some(prior)` and `?`-propagated (fail-closed) BEFORE the decision is sent; `phase5_dispatch::append_failure_is_fail_closed` + `block_appends_durable_causal_sink_blocked`; live `s9_live_sink_blocked_in_dag` asserts `sink_blocked.parent_id == file_read.id`, genuine taint on `file_read`, no effect event, non-zero exit. |

## Must-Haves (from plan frontmatter)
All 6 plan-level must-have clusters verified — single dispatch (05-02/05-03), genuine mint anchor (05-02), per-connection store isolation (05-02), connection-established session identity (05-02), durable fail-closed sink_blocked before response (05-02), and the live hostile-path block (05-04). See each plan's SUMMARY for the per-task acceptance-criteria greps (all satisfied).

## Test Evidence

### macOS dev box (cross-platform suite)
- `cargo test --workspace --no-fail-fast` → exit 0, 27 result groups, 0 failures.
- `cargo test -p brokerd --test s9_acceptance` → 1/1 (non-negotiable regression, unweakened).
- `cargo test -p brokerd --test phase5_dispatch` → 6/6.
- `./scripts/check-invariants.sh` → Gate 1 (no `EffectRequest` effect-to-sink type) + Gate 2 (runtime-core purity) PASS.

### Linux (Colima/Docker) — the v0 DONE gate
`docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast` → **exit 0, entire workspace green.**
- `s9_live_block::s9_live_caprun_exits_nonzero` ✅
- `s9_live_block::s9_live_sink_blocked_in_dag` ✅ (durable causal sink_blocked, genuine taint, no effect)
- `e2e::substrate_demo` ✅, `e2e::dag_chain_integrity` ✅
- `s9_acceptance::s9_acceptance` ✅

## Genuine-Taint Confirmation (anti-stapling)
The block is driven by a propagated taint chain, not a sink-time staple: `mint_from_read` is the sole mint site (taint set at read time), `provenance_chain[0]` equals the `file_read` event id (in-process backstop), and the live audit DAG shows `sink_blocked.parent_id == file_read.id` with `ExternalUntrusted`+`EmailRaw` taint on the `file_read` event. A stapled implementation would lack this edge and fail both backstops.

## Known / Deferred (not phase-blocking)
- `mint_from_read` sets `file_read.parent_id = None` (hash-chained via `parent_hash`, not `parent_id`-linked), so `verify_chain` is not asserted over the full hostile 4-event chain. This is an **intentional deferral to Phase 7 (SC7)**, documented in the 05-03/05-04 SUMMARYs and the live test; the live test asserts the `sink_blocked → file_read` causal edge directly instead.

## Conclusion
Per CLAUDE.md, **v0 DONE = the §9 value-injection acceptance test passing** — and it passes live, end-to-end, on a kernel-confined worker whose only egress is broker-mediated plan nodes, with a genuine taint chain recorded as an unbroken causal edge in the audit DAG. Phase 5 goal achieved.
