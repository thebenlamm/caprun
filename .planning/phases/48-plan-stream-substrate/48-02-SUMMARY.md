---
phase: 48-plan-stream-substrate
plan: 02
subsystem: testing
tags: [stream, bag, taint, i2, deny-abort, f-01, f-02, stream-01, stream-02]

requires:
  - phase: 48-01
    provides: plan_next + sequential worker loop + opaque bag + stream_multi_submit
provides:
  - stream_substrate deny-abort remaining (no further SubmitPlanNode after Deny)
  - stream_substrate Block no re-submit (F-02 substrate stop)
  - F-01 bag-any-Some multi-sink unit + multi-sink docs authority
  - Linux genuine taint-via-bag Block + provenance backstop + verify_chain
  - COVERAGE.md (no external API) + Wave 0 validation complete
affects: [49-coding-recipe, 50-cli-multi-node-hold]

tech-stack:
  added: []
  patterns:
    - "Pure stream driver harness mirrors worker branch table + bag insert (host-safe)"
    - "Linux hybrid taint spine: mint_from_exec → bag out_0 → plan command arg → I2 Block"

key-files:
  created:
    - cli/caprun/tests/stream_substrate.rs
    - .planning/phases/48-plan-stream-substrate/COVERAGE.md
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - .planning/phases/48-plan-stream-substrate/48-VALIDATION.md

key-decisions:
  - "Host-safe stream proofs use a pure drive_stream harness aligned with worker branch table (option b) rather than extracting a lib from the worker binary"
  - "Linux taint-via-bag is hybrid in-crate multi-node (s9 spine + bag intermediate) — framed as substrate not LIVE-07 CLI multi-step DONE"
  - "F-01 docs-only on proto/server; worker multi-sink comments already correct from 48-01"

patterns-established:
  - "stream_substrate.rs is the STREAM expansion home (deny/block/empty/single/taint-via-bag)"
  - "apply_stream_decision is the shared F-01 + branch pure function used by host and Linux legs"

requirements-completed: [STREAM-01, STREAM-02]

coverage:
  - id: D1
    description: "Deny aborts remaining — submit count stops at denied step (no third SubmitPlanNode)"
    requirement: STREAM-01
    verification:
      - kind: unit
        ref: cli/caprun/tests/stream_substrate.rs#deny_aborts_remaining_no_further_submit
        status: pass
    human_judgment: false
  - id: D2
    description: "Block stops without re-submit of blocked node (F-02 substrate)"
    requirement: STREAM-01
    verification:
      - kind: unit
        ref: cli/caprun/tests/stream_substrate.rs#block_stops_without_resubmit
        status: pass
    human_judgment: false
  - id: D3
    description: "F-01 bag stores any Some(output_value_id) multi-sink; docs multi-sink authority"
    requirement: STREAM-02
    verification:
      - kind: unit
        ref: cli/caprun/tests/stream_substrate.rs#f01_bag_stores_any_some_output_value_id_multi_sink
        status: pass
    human_judgment: false
  - id: D4
    description: "Linux genuine taint-via-bag Block with process_exited provenance + verify_chain"
    requirement: STREAM-02
    verification:
      - kind: integration
        ref: cli/caprun/tests/stream_substrate.rs#linux::taint_via_bag_exec_output_blocks_with_genuine_provenance
        status: pass
    human_judgment: false

duration: 5min
completed: 2026-07-28
status: complete
---

# Phase 48 Plan 02: Stream Substrate Expansion Summary

**Deny-abort + Block no re-submit proofs, Linux genuine taint-via-bag, F-01 multi-sink docs, Wave 0 validation complete — substrate ready for Phase 49 coding planner.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-07-28T01:09:00Z
- **Completed:** 2026-07-28T01:13:35Z
- **Tasks:** 2/2
- **Files modified:** 5

## Accomplishments

- Host-safe `stream_substrate` harness proves fail-closed mid-stream branches (Deny abort remaining, Block no re-submit, empty fail-closed, single Allowed success)
- F-01 bag-any-Some multi-sink unit + proto/server comment authority match mint arms (`process.exec` / `git.commit` / `http.request`)
- Linux taint-via-bag: bagged `mint_from_exec` handle → `process.exec`/`command` → `BlockedPendingConfirmation` with `provenance_chain[0] == process_exited` + `verify_chain` true
- COVERAGE.md records no external API/SDK for Phase 48; 48-VALIDATION Wave 0 + nyquist complete

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Deny-abort + Block no re-submit + F-01 bag unit | `9127566` | `cli/caprun/tests/stream_substrate.rs` |
| 2 | Linux taint-via-bag + F-01 docs + COVERAGE + validation | `7589c63` | `stream_substrate.rs`, `proto.rs`, `server.rs`, `COVERAGE.md`, `48-VALIDATION.md` |

## Deviations from Plan

### Auto-fixed Issues

None — plan executed as written. Worker F-01 multi-sink comments were already corrected in Plan 01 (no further worker edit required). Host-safe harness used option (b) (branch-table mirror) because `caprun-worker` is a binary without a lib target; harness docs require alignment with `worker.rs` match arms.

## Known Stubs

None.

## Threat Flags

None beyond plan `<threat_model>` mitigations (T-48-01/04/09/10 covered by tests + docs).

## Verification

| Check | Result |
|-------|--------|
| `./scripts/check-invariants.sh` | PASS |
| `cargo test -p caprun --test stream_substrate` | 9 passed (8 host + 1 Linux taint-via-bag) |
| `cargo test -p caprun --test planner` | 22 passed |
| No CaprunIntent coding / no CLI multi-node product | PASS (scope held) |
| Gate 3 mint sites unchanged | PASS |

## Host note

Host lacks system `pkg-config`/`libssl-dev`; tests used userland openssl under `/tmp/caprun-openssl-debs`. Linux taint leg ran natively (this host is Linux). Prefer `mailpit-verify.sh` when Docker is the verification path:

```bash
MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test stream_substrate -- --nocapture' \
  bash scripts/mailpit-verify.sh
```

(`cargo build --workspace` first so `caprun-exec-launcher` sibling is present.)

## Next

Phase 48 complete at plan level. Phase 49 can pile coding planner nodes on this substrate; Phase 50 owns Block-and-Hold product UX.

## Self-Check: PASSED

- FOUND: `cli/caprun/tests/stream_substrate.rs`
- FOUND: `crates/brokerd/src/proto.rs` (F-01 multi-sink docs)
- FOUND: `crates/brokerd/src/server.rs` (multi-submit response comment)
- FOUND: `.planning/phases/48-plan-stream-substrate/COVERAGE.md`
- FOUND: `.planning/phases/48-plan-stream-substrate/48-VALIDATION.md`
- FOUND: commit `9127566` (Task 1)
- FOUND: commit `7589c63` (Task 2)
- Host stream_substrate 9/9 + planner 22/22 + check-invariants green
