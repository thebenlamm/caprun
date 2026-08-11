# Phase 48 — API / Integration Coverage

**Phase:** 48-plan-stream-substrate  
**Claim:** Phase 48 implements **internal plan-stream substrate only** (worker sequential `plan_next` loop + opaque `ValueId` handle bag + fail-closed mid-stream branches + broker multi-submit continuity).

## External API / SDK

**None.** This phase has no external API surface, no public SDK, and no third-party integration under test. Coverage gates that expect HTTP/SDK client contracts do not apply.

| Surface | Status |
|---------|--------|
| External HTTP/REST API | N/A — not in scope |
| Public client SDK | N/A — not in scope |
| Third-party SaaS integration | N/A — not in scope |
| Internal worker ↔ broker UDS | Existing IPC; substrate composes `SubmitPlanNode` × N |

## What is covered instead

- Host unit/harness: `cli/caprun/tests/stream_substrate.rs`, `cli/caprun/tests/planner.rs`
- Broker multi-submit: `crates/brokerd/tests/stream_multi_submit.rs`
- Linux genuine taint-via-bag: `stream_substrate` Linux cfg leg (mailpit-verify when Docker available)

## Exemption rationale

STREAM-01/02 close the worker one-shot composition gap. Product multi-node CLI (LIVE-07/08) and coding recipe (Phase 49) are out of scope; no new external contract is shipped.
