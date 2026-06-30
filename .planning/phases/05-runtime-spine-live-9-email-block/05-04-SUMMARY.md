---
phase: 05-runtime-spine-live-9-email-block
plan: "04"
subsystem: cli/caprun
tags: [s9-acceptance, live-block, genuine-taint, acc-02, v0-done, phase-gate]
status: complete

dependency_graph:
  requires:
    - caprun single unified dispatch + typed-claims worker (Plan 03)
    - brokerd::server durable sink_blocked path (Plan 02)
    - quarantine::mint_from_read genuine-taint mint (existing)
  provides:
    - cli/caprun/tests/s9_live_block.rs (live §9 block proof + cross-platform guard)
    - the v0 DONE acceptance evidence (live, end-to-end)
  affects:
    - cli/caprun/tests/s9_live_block.rs

tech_stack:
  added: []
  patterns:
    - live binary e2e via CARGO_BIN_EXE + temp audit DB read-back
    - assert the sink_blocked → file_read causal edge directly (not verify_chain over a multi-root chain)
    - cross-platform compile guard so cargo test stays meaningful on the dev box

key_files:
  created:
    - cli/caprun/tests/s9_live_block.rs
  modified: []

decisions:
  - "Per the plan prohibition, the live test asserts the sink_blocked → file_read causal-parent edge (sink_blocked.parent_id == file_read.id) instead of verify_chain over the full hostile chain. mint_from_read sets file_read.parent_id = None (hash-chained but not parent_id-linked); wiring that linkage is deferred to Phase 7 (SC7)."
  - "Executed inline on main (continuation of the Plan 02/03 inline fallback after subagent connection drops)."
  - "Linux-only live assertions verified under Colima/Docker; the macOS dev box runs only the cross-platform guard (the confinement stack — abstract UDS + Landlock + seccomp — is Linux-only)."

metrics:
  completed_date: "2026-06-30"
  tasks_completed: 2
  files_created: 1
  files_modified: 0
---

# Phase 05 Plan 04: Live §9 Value-Injection Block (v0 DONE Gate)

The Phase 5 deliverable, proven end to end: a real `caprun` run on hostile email content fires a live §9 block with a durable causal `sink_blocked` audit event and a non-success exit BEFORE any effect — driven by a genuine propagated taint chain, not a sink-time staple.

## What Was Built

### `cli/caprun/tests/s9_live_block.rs` (new)
- `s9_live_caprun_exits_nonzero` (`#[cfg(target_os = "linux")]`) — writes the hostile sentence (`…accounts@ev1l.com.`) to a temp workspace, runs the real caprun binary, asserts the process exit is **non-success** (T-05-13).
- `s9_live_sink_blocked_in_dag` (`#[cfg(target_os = "linux")]`) — runs caprun on the same content and asserts via the audit DB:
  - (a) a durable `sink_blocked` event exists (ACC-02 / T-05-11);
  - (b) the `file_read` event carries `ExternalUntrusted` + `EmailRaw` taint — **genuine taint** minted at read time (T-05-12);
  - (c) `sink_blocked.parent_id == file_read.id` — the block descends causally from the tainted read (a stapled implementation lacks this edge);
  - (d) no `email_send_stub`/effect event exists — the block fired before any effect (T-05-13).
- `s9_live_block_guard_binary_present` (cross-platform) — keeps `cargo test -p caprun` meaningful on macOS (asserts `CARGO_BIN_EXE_caprun` resolves); the live bodies are cfg-excluded there.

## Verification Results

### macOS dev box
| Check | Result |
|-------|--------|
| `cargo build -p caprun --tests` | green |
| `cargo test -p caprun --test s9_live_block` | 1 passed (guard); Linux bodies cfg-excluded |
| `./scripts/check-invariants.sh` | Gate 1 + Gate 2 PASS |
| `cargo test -p brokerd --test s9_acceptance` | 1/1 passed (non-negotiable regression) |
| `cargo test --workspace --no-fail-fast` | all green (27 result groups) |
| `grep -rc handle_worker_connection cli/caprun/src/` | 0 (ASM-01) |
| `grep -rc "SubmitPlanNode not wired" crates/ cli/` | 0 (ASM-02) |

### Linux (Colima/Docker) — the live §9 gate
Command:
```
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 cargo test --workspace --no-fail-fast
```
Result: **exit 0 — entire workspace green on Linux.** Key tests:
| Test | Result |
|------|--------|
| `s9_live_block::s9_live_caprun_exits_nonzero` | ok |
| `s9_live_block::s9_live_sink_blocked_in_dag` | ok |
| `e2e::substrate_demo` | ok |
| `e2e::dag_chain_integrity` | ok |
| `s9_acceptance::s9_acceptance` | ok |
| brokerd `phase5_dispatch` (6), `proto_claims` (3), quarantine, audit | ok |

The live §9 block — the v0 DONE acceptance — is proven on real kernel-confined binaries with a genuine, propagated taint chain.

## Deviations from Plan
1. **Inline execution on `main`** (continuation of the Plan 02 inline fallback after two subagent connection drops). Atomic, hook-verified commits; STATE/ROADMAP updated by the orchestrator.

## Threat Surface Scan
- **T-05-11 (block durability):** mitigated — durable `sink_blocked` asserted in the DB post-run.
- **T-05-12 (stapled taint masquerading as a block):** mitigated — `file_read` taint + `sink_blocked → file_read` causal edge asserted; a staple lacks the propagated edge.
- **T-05-13 (effect executes despite block):** mitigated — no effect event; CLI exits non-success before any effect.

## v0 DONE Status
Per CLAUDE.md, **v0 DONE = the §9 value-injection acceptance test passing**. The live §9 block now passes end-to-end on a kernel-confined worker whose only egress is broker-mediated plan nodes, with a genuine taint chain (raw read → ValueNode → sensitive sink arg → deterministic block) recorded as an unbroken causal edge (`sink_blocked.parent_id == file_read.id`) in the audit DAG. Substrate + live gate both green.

## Self-Check: PASSED
- `cli/caprun/tests/s9_live_block.rs` — exists, 3 tests; live bodies Linux-gated, guard cross-platform.
- Linux Colima run: `s9_live_*`, `substrate_demo`, `dag_chain_integrity`, `s9_acceptance` all pass.
- Commit `(this wave)` present in git log.
