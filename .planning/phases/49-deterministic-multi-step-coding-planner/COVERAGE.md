# Phase 49 — API / Integration Coverage

**Phase:** 49-deterministic-multi-step-coding-planner  
**Claim:** **No external API integration.** Phase 49 extends `CaprunIntent` + the planner recipe over **existing shipped sinks** (`file.write`, `process.exec`, `git.commit`, `git.push`, `github.pr`). No new external service surface, no public SDK, no third-party client under test.

## External API / SDK

| Surface | Status |
|---------|--------|
| External HTTP/REST API | N/A — not in scope |
| Public client SDK | N/A — not in scope |
| Third-party SaaS integration | N/A — uses existing broker sinks only |
| New crates / packages | **Zero** (HYG-02) |
| Internal worker ↔ broker UDS | Existing IPC; coding multi-mint + bag seed compose ProvideIntent + plan_next × N |

## What is covered instead

- Host unit: `cli/caprun/tests/planner.rs` — CODE-01 five-node emission, CODE-02 anti-launder, LIVE-08 expressibility (test-only `CodingI2ProofPlanner`), email/file plan_next regression
- Broker multi-mint: `crates/brokerd/tests/proto_claims.rs` — `IntentAccepted.named_handles` + SafeCodingWorkflow ProvideIntent
- Stream substrate regression: `cli/caprun/tests/stream_substrate.rs` (Phase 48)
- Architectural gates: `./scripts/check-invariants.sh` Gates 1–6

## Assumption-delta: CaprunIntent add-alongside

**Delta:** `CaprunIntent` gains a closed `SafeCodingWorkflow` variant **add-alongside** the existing `SendEmailSummary` and `CreateFileFromReport` variants. Coding is a new primary identity, not a promotion or replacement of email/file.

| Option | Disposition | Rationale |
|--------|-------------|-----------|
| **Add-alongside closed enum variant** (chosen) | Accepted | Closed enum is primary identity (DESIGN §8.3); coding is a distinct workflow with 13 operator-typed fields; email/file remain byte-stable one-shot paths |
| Promote email into multi-step | Rejected | Would overload SendEmailSummary semantics and risk regressing the single-node email path |
| Free-form `HashMap` / open tool map | Rejected | Violates closed-enum discipline and PLAN-03 / HYG-02 (authority via free-form keys) |
| Replace email/file with coding-only enum | Rejected | Breaks v1.9 product paths; email/file must remain green (CODE-01) |

**Implications for later phases:** Phase 50 adds CLI multi-node driver for the coding variant without changing this identity choice. Phase 51 LIVE proofs compose the same closed intent + plan_next spine.

## Framing honesty

- Phase 49 proves **planner expressibility** and **trusted-intent bag discipline**.
- Phase 49 does **not** claim LIVE-07/08 CLI multi-step DONE (Phase 51).
- LIVE-08 unit test (`coding_i2_proof_places_out_handle`) is bag-routing expressibility only.

## Exemption rationale

Coverage gates that expect HTTP/SDK client contracts do not apply. Product multi-node CLI and non-hybrid LIVE proofs are Phases 50–51.
