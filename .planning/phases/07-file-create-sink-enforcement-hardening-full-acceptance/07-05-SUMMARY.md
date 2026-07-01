---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 05
subsystem: brokerd / cli-caprun (tests) + brokerd causal-chain fix
tags: [ACC-03, ACC-04, ACC-05, ACC-07, v0-DONE, durable-anchor, genuine-taint, live-e2e, I2]
requires:
  - "07-02: SinkBlockedAnchor + Event.anchor + append_event Defect-B guard + broker-minted effect_id"
  - "07-04b: live file.create sink (invoke_file_create), WorkerClaim::RelativePath, intent-kind routing (both §9 paths reachable)"
  - "07-03/07-04a: WorkspaceRoot dirfd capability + create_exclusive_within + validate_schema arg-gate"
provides:
  - "crates/brokerd/tests/durable_anchor.rs — canonical ACC-07 after-exit DB-alone anti-stapling sentinel + tamper-evidence"
  - "crates/brokerd/tests/s9_acceptance.rs::s9_acceptance_file_create_path_block — fast in-process file.create backstop"
  - "cli/caprun/tests/s9_live_block.rs — live file.create hostile-block (ACC-03) + clean-allow (ACC-04) + causal chain (ACC-05), Linux-gated, VERIFIED on real Linux"
  - "Threaded causal parent_id through mint_from_read / mint_from_intent — the live audit DAG is now ONE unbroken parent_id chain (verify_chain TRUE on the full live flow)"
affects: []
tech-stack:
  added: []
  patterns:
    - "After-exit / DB-alone proof: file-backed DB, drop+reopen, verify_chain FIRST then the value-lineage backstops on the persisted anchor (event-order alone is insufficient)"
    - "Two graphs, shared node ids, never equated: causal parent_id chain (verify_chain) vs value-lineage provenance_chain (anchor)"
    - "Block short-circuits on the routing-sensitive arg first (path before contents), so contents need not resolve on the block path"
key-files:
  created:
    - crates/brokerd/tests/durable_anchor.rs
  modified:
    - cli/caprun/tests/s9_live_block.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/phase5_dispatch.rs
decisions:
  - "Threaded parent_id through mint_from_read/mint_from_intent (production fix) — REQUIRED for ACC-05; the live DAG previously had 3 parent_id=None roots so verify_chain returned false and the fd_granted→file_read→sink_blocked edge was absent. Aligns code with DESIGN §0 (causal edges threaded on the chain head) and the quarantine.rs 'Phase 7 wires the parent_id chain' TODO."
  - "On a BLOCK the evaluation event IS sink_blocked (the Allow branch is plan_node_evaluated — mutually exclusive). The live hostile-block chain is fd_granted→file_read→sink_blocked; no plan_node_evaluated is asserted on the block path (must_haves.truths uses the correct disjunction '(sink_blocked | sink_executed)')."
  - "durable_anchor harness mirrors phase5_dispatch: mint the tainted value via mint_from_read (parent_hash=None → single clean root), then drive the BLOCK through the real dispatch_request SubmitPlanNode arm; contents uses a fresh ValueId::new() (never resolved because path blocks first)."
metrics:
  duration: ~45min
  completed: 2026-07-01
  tasks: 5
  files: 5
status: complete
---

# Phase 7 Plan 05: Full Live §9 Acceptance + Canonical ACC-07 Proof Summary

**v0-DONE gate closed: a real kernel-confined `caprun` `file.create` run blocks a genuine-tainted workspace path (no file, non-zero exit, durable `sink_blocked` with a real anchor, no effect) and allows a trusted intent path (file created, `sink_executed`); each run is ONE unbroken causal chain (ACC-05); and the canonical ACC-07 proof is a dispatch-level, after-exit, DB-alone anti-stapling sentinel (`verify_chain` FIRST, then the `file_read` genuine-taint backstops on the persisted anchor) plus tamper-evidence.**

This plan restores a continuously-proven LIVE §9 guarantee (Phase 6's email hostile block became unreachable) and closes the durable genuine-taint edge the git-log "v0 DONE" line predated.

## What Was Built

- **`crates/brokerd/tests/durable_anchor.rs` (new) — the CANONICAL ACC-07 proof.**
  - `after_exit_db_alone_anti_stapling_sentinel`: drives a genuine `file.create` hostile block END TO END through the real `dispatch_request` SubmitPlanNode arm against a **file-backed** SQLite DB, then **drops + reopens** the connection (process-exit simulation) and reconstructs the proof from the persisted DB ALONE, asserting **in order**: (1) `verify_chain` TRUE first; (2) `sink_blocked` carries `Some(anchor)`; (3) a real `file_read` DAG event with `id == anchor.read_event_id == anchor.provenance_chain[0]` and untrusted/`PathRaw` taint (`is_untrusted()`); (4) `Event.taint == anchor.taint` + byte-exact literal + `sink=file.create`, `arg=path`; (5) NO `sink_executed`/`email_send_stub`.
  - `tamper_evidence_mutating_payload_breaks_verify_chain`: confirms `verify_chain` TRUE, runs a raw `UPDATE events SET payload = REPLACE(payload, <literal>, ...)` on the `sink_blocked` row (mutating the anchor literal inside the hashed payload), reopens → `verify_chain` FALSE. The durable anchor is tamper-evident.
  - Cross-platform (the block path performs NO file I/O): passes on macOS and Linux.
- **`crates/brokerd/tests/s9_acceptance.rs::s9_acceptance_file_create_path_block` (new) — fast in-process backstop.** A `[ExternalUntrusted, PathRaw]` value (via `mint_from_read`) routed into `file.create/path` → `BlockedPendingConfirmation`; held-out genuine-taint backstop `provenance_chain[0] == read_event_id` retained (unweakened), real DAG `file_read` carries `PathRaw` never `LocalWorkspace`, `verify_chain` true.
- **`cli/caprun/tests/s9_live_block.rs` (new tests) — live file.create §9, Linux-gated.**
  - `s9_live_file_create_hostile_block` (ACC-03 + ACC-05): real `caprun` run, hostile workspace path token → tainted `RelativePath` → `file.create/path` → BLOCK. Non-zero exit, no file on disk, durable `sink_blocked` + genuine anchor, no `sink_executed`, parent-linked `fd_granted → file_read → sink_blocked`, `verify_chain` true.
  - `s9_live_file_create_clean_allow` (ACC-04 + ACC-05): real `caprun` run, trusted intent path → ALLOW → file created under the workspace root with expected contents, durable `sink_executed` (effect_id in actor), no `sink_blocked`, `verify_chain` true, `sink_executed` parented onto `plan_node_evaluated`.
- **Causal-chain production fix (`quarantine.rs` + `server.rs`):** `mint_from_read` / `mint_from_intent` now take `parent_id: Option<Uuid>` and set it on the appended Event; the broker `ReportClaims`/`ProvideIntent` arms pass `Some(*last_event_id)`. The live audit DAG is now ONE unbroken `parent_id` chain (`session_created → intent_received → fd_granted → file_read → sink_blocked|plan_node_evaluated`), so `verify_chain` is TRUE on the full live flow — the prerequisite for ACC-05. See Deviations.

## Task Commits

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 + 2 | after-exit DB-alone sentinel + tamper-evidence (durable_anchor.rs) | c0a26b6 |
| 5 | in-process file.create/path block backstop (s9_acceptance.rs) | 129d64c |
| — | fix: thread parent_id (ACC-05 causal chain) — required deviation | 9324450 |
| 3 + 4 | live file.create hostile-block + clean-allow + chain (s9_live_block.rs) | c39a638 |

## Verification

- `cargo test -p brokerd --test durable_anchor` — 2/2 pass on macOS (and Linux).
- `cargo test --workspace --no-fail-fast` — GREEN on macOS: 30/30 test targets ok, 0 failures (brokerd lib 26, durable_anchor 2, s9_acceptance 3, phase5_dispatch 6, audit_dag 2, proto_claims 6; caprun planner 5, s9_live_block guard 1). Linux-only live bodies show 0-passed on macOS (cfg-gated, expected per CLAUDE.md).
- `./scripts/check-invariants.sh` — both gates PASS (no `EffectRequest`; runtime-core pure).
- **Real Linux (Colima/Docker, `seccomp=unconfined`, no `--privileged`):** `cargo test -p brokerd -p caprun --no-fail-fast` exit 0. Explicitly observed: `s9_live_file_create_hostile_block` ok, `s9_live_file_create_clean_allow` ok, `s9_live_clean_allow_path` ok, `s9_live_block_guard_binary_present` ok (`test result: ok. 4 passed`). All brokerd targets (incl. durable_anchor, s9_acceptance) green under the exit-0 `--no-fail-fast` run.
- Empirical proof of the fix: a probe of the exact live multi-event shape returned `verify_chain=false` BEFORE the fix and `verify_chain=true` after.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1/3 — Bug blocking ACC-05] Threaded `parent_id` through `mint_from_read` / `mint_from_intent` (production TCB fix in a tests-only plan).**
- **Found during:** Task 3/4 design (verifying the ACC-05 causal-chain assumption).
- **Issue:** The live audit DAG did NOT form one unbroken causal (`parent_id`) chain. `mint_from_read` and `mint_from_intent` hardcoded `Event.parent_id = None`, so `intent_received` and `file_read` became extra `parent_id` roots. `verify_chain` walks the `parent_id`-derived depth order, so on the real multi-event live flow it returned **false** — making ACC-05 (one unbroken causal chain, `verify_chain` true) **unreachable**, and the `fd_granted → file_read → sink_blocked` edge **absent** from the `parent_id` graph. Verified empirically (probe: `VERIFY_CHAIN_RESULT=false`).
- **Fix:** Added `parent_id: Option<Uuid>` to both mint functions; the broker `ReportClaims`/`ProvideIntent` arms pass `Some(*last_event_id)`; standalone callers pass `None`. Value-lineage (`provenance_chain`) is unchanged — the two graphs stay distinct (DESIGN §0); only the CAUSAL edge is now threaded. Post-fix probe: `verify_chain=true` on the full live chain.
- **Why this is in-scope despite "tests only":** ACC-05 is a hard success criterion and is impossible without this. The fix aligns code with **DESIGN §0** ("causal DAG edges are `parent_id`/`parent_hash` threaded on the connection chain head") and with the explicit `quarantine.rs` TODO "Phase 7 wires the parent_id chain." This is deferred Phase-7 wiring that had to land in the final acceptance plan. Surgical: signature + call-site threading only, no new types/abstractions.
- **Files modified:** crates/brokerd/src/quarantine.rs, crates/brokerd/src/server.rs (+ test call-site updates in phase5_dispatch.rs, s9_acceptance.rs, durable_anchor.rs). **Commit:** 9324450.
- **Blast radius verified:** all 26 brokerd tests + full workspace + Linux run green.

**2. [Rule 1 — Faithful assertion] Block-path chain asserts `sink_blocked`, not `plan_node_evaluated`.**
- **Found during:** Task 3.
- **Issue:** Task 3's done-line lists the chain as `fd_granted → file_read → plan_node_evaluated → sink_blocked`. In the actual event model (`server.rs` SubmitPlanNode) a decision persists EITHER `sink_blocked` (Block) OR `plan_node_evaluated` (Allow) — never both. Asserting a non-existent `plan_node_evaluated` on the block path would be a false assertion.
- **Fix:** The hostile-block test asserts the real chain `fd_granted → file_read → sink_blocked` (parent-linked via `parent_id`). This matches `must_haves.truths` line 18, which uses the correct disjunction `(sink_blocked | sink_executed)`. The clean-allow test asserts `plan_node_evaluated → sink_executed`.
- **Files:** cli/caprun/tests/s9_live_block.rs. **Commit:** c39a638.

## Known Stubs

None. Every anchor field is wired from real resolved-record data; the block path persists a genuine chain and executes no effect; the clean path performs a real `create_exclusive_within` write.

## Threat Flags

None. No new network endpoints, auth paths, or trust-boundary schema beyond the plan's threat model. Mitigations exercised: T-07-51 (after-exit DB-alone sentinel: `verify_chain` first, then persisted-anchor backstops), T-07-52 (payload tamper → `verify_chain` false), T-07-53 (`file_read` `id == read_event_id == provenance_chain[0]` + untrusted taint — not weakenable), T-07-54 (no `sink_executed`/`email_send_stub` + no file on the block path).

## Notes for Downstream / Coverage caveats

- The live `caprun` runs (`s9_live_block.rs`) and the confinement stack are **Linux-only** (`#[cfg(target_os="linux")]`); they show 0-passed on macOS by design. This plan **VERIFIED them on real Linux** via the Colima/Docker recipe — not left pending. The cross-platform in-process backstops (`durable_anchor.rs`, `s9_acceptance.rs`) pass on both.
- `Event::sink_blocked` remains the sole anchor-setting constructor; the append-guard keeps a no-anchor `sink_blocked` non-persistable.
- The `parent_id`-threading fix touches TCB files (`quarantine.rs`, `server.rs`) — reviewers should confirm the two-graph separation held (value-lineage untouched; only causal `parent_id` now set from the chain head).

## Self-Check: PASSED
- FOUND: crates/brokerd/tests/durable_anchor.rs, cli/caprun/tests/s9_live_block.rs, crates/brokerd/tests/s9_acceptance.rs, crates/brokerd/src/quarantine.rs, 07-05-SUMMARY.md
- FOUND commits: c0a26b6 (Tasks 1+2), 129d64c (Task 5), 9324450 (parent_id fix), c39a638 (Tasks 3+4)
- Cross-platform tests green on macOS (30/30 targets); live §9 file.create tests VERIFIED on real Linux (Docker, exit 0); check-invariants PASS.
