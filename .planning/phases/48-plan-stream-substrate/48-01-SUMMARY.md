# Phase 48 Plan 01 — SUMMARY

**Plan:** 48-01 — Tracer: plan_next + sequential worker loop + opaque bag + broker multi-submit  
**Date:** 2026-07-28  
**Status:** Complete  
**Requirements:** STREAM-01, STREAM-02 (tracer slice)

## What shipped

1. **Additive `Planner::plan_next` + `PlanStreamContext`** (`cli/caprun/src/planner.rs`)  
   - Default one-shot adapter keeps email/file/LLM green  
   - Handles are opaque `ValueId` only (named-slot bag map)

2. **Worker sequential stream loop** (`cli/caprun/src/worker.rs`)  
   - N× `SubmitPlanNode` on one connection  
   - Opaque bag seeded from ProvideIntent handles  
   - Stores **any** `Some(output_value_id)` under `out_{step}` (F-01)  
   - Allowed → continue; Block → exit 1 no re-submit; Deny → abort remaining  
   - Empty stream fails closed  

3. **Broker STREAM-01 integration tests** (`crates/brokerd/tests/stream_multi_submit.rs`)  
   - Multi-submit different nodes + `verify_chain`  
   - Mid-stream second ProvideIntent rejected  

4. **Planner unit tests** extended for bag opacity, plan_next one-shot adapter, multi-node test planner placing bag handles  

## Verification

| Check | Result |
|-------|--------|
| `./scripts/check-invariants.sh` | PASS |
| `cargo test -p caprun --test planner` | 22 passed |
| `cargo test -p brokerd --test stream_multi_submit` | 2 passed (Linux) |
| No new crates / no EffectRequest / Gate 3 | PASS |

## Host note

Host lacked `pkg-config`/`libssl-dev`; tests run with userland openssl debs extracted under `/tmp/caprun-openssl-debs` (not a project dependency). Prefer `mailpit-verify.sh` when Docker is available.

## Commits

- `c1bb2b6` feat(48-01): plan_next + opaque bag + sequential worker loop  
- (this summary + stream_multi_submit test commit)

## Next

Plan 48-02: deny-abort / Block no re-submit tests, Linux taint-via-bag, F-01 comment drift docs.
