---
phase: 49
slug: deterministic-multi-step-coding-planner
status: validated
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-29
updated: 2026-08-11
---

# Phase 49 — Validation Strategy

> Per-phase validation contract for deterministic multi-step coding planner (CODE-01/02).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` / `#[tokio::test]` via cargo |
| **Config file** | none — Cargo workspace defaults |
| **Quick run command** | `./scripts/check-invariants.sh && cargo test -p caprun --test planner -- --nocapture` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` (Linux security legs); SMTP-touching: `bash scripts/mailpit-verify.sh` |
| **Linux authority** | Host unit tests for pure planner CODE-01/02; mailpit-verify when broker integration runs |
| **Estimated runtime** | quick ~30–90s; full workspace longer |

---

## Sampling Rate

- **After every task commit:** `./scripts/check-invariants.sh && cargo test -p caprun --test planner`
- **After every plan wave:** above + `cargo test -p caprun --test stream_substrate` + `cargo test -p brokerd --test stream_multi_submit` (Linux when available)
- **Before `/gsd-verify-work`:** invariants green + coding plan_next tests green + email/file regression green
- **Max feedback latency:** ~120s for focused runs

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 49-*-coding-seq | 49-01 | 1 | CODE-01 | T-49-01 | 5 sinks in order with exact sink_schema arg names | unit | `cargo test -p caprun --test planner coding_plan_next_emits_five_sinks_in_order` | ✅ | ✅ green |
| 49-*-email-reg | 49-01 | 1 | CODE-01 | — | Email plan_next step0 matches plan() | unit | `cargo test -p caprun --test planner plan_next_step0_matches_plan_for_email` | ✅ | ✅ green |
| 49-*-file-reg | 49-01 | 1 | CODE-01 | — | File plan_next step0 matches plan() | unit | `cargo test -p caprun --test planner plan_next_step0_matches_plan_for_file` | ✅ | ✅ green |
| 49-*-no-out | 49-01/02 | 1–2 | CODE-02 | T-49-01 | Success-path args only intent bag keys (no `out_*`) | unit | `cargo test -p caprun --test planner coding_success_path_does_not_place_out_handles` | ✅ | ✅ green |
| 49-*-mint | 49-01 | 1 | CODE-02 | T-49-02 | ProvideIntent multi-mint distinct named UserTrusted handles | unit/integration | `cargo test -p brokerd --test proto_claims provide_intent_safe_coding_multi_mint_distinct_named_handles` | ✅ | ✅ green |
| 49-*-i2-proof | 49-02 | 2 | CODE-02 | T-49-01 / T-49-07 | LIVE-08 expressibility: proof path places `out_*` into sensitive arg | unit | `cargo test -p caprun --features live-proof-fixtures --test planner coding_i2_proof_places_out_handle -- --exact` | ✅ | ✅ green |
| 49-*-hyg | 49-01/02 | * | HYG-02 | T-49-08 | Gate 1/3; zero new crates | script | `./scripts/check-invariants.sh` | ✅ | ✅ green |
| 49-*-stream | 49-02 | 2 | STREAM regression | — | stream_substrate still green | unit | `cargo test -p caprun --test stream_substrate` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `cli/caprun/tests/planner.rs` — coding multi-node emission tests (CODE-01) — `coding_plan_next_emits_five_sinks_in_order`
- [x] `cli/caprun/tests/planner.rs` — success-path no-`out_*` placement (CODE-02) — `coding_success_path_does_not_place_out_handles` (strengthened: intent-minted key set)
- [x] `cli/caprun/tests/planner.rs` — LIVE-08 expressibility placement test (CODE-02) — `coding_i2_proof_places_out_handle`
- [x] ProvideIntent multi-mint / IntentAccepted named_handles round-trip tests — `crates/brokerd/tests/proto_claims.rs` (`intent_accepted_named_handles_round_trips`, `provide_intent_safe_coding_multi_mint_distinct_named_handles`)
- [x] Exhaustive CaprunIntent match compile sweep (SafeCodingWorkflow arms in planner/worker/server; plan_from_intent fail-closed)
- [x] Framework install: **none** — use existing cargo test

*Host-safe legs verified 2026-07-29: planner 26 passed, stream_substrate 9 passed, proto_claims 16 passed, check-invariants green.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CLI multi-node driver / Block-and-Hold product path | CLI-01/CONFIRM-01 | Phase 50 scope | Confirm Phase 49 does not implement `caprun run` coding verb or mid-loop hold UX |
| Non-hybrid LIVE-07/08 proof | LIVE-07/08 | Phase 51 scope | Unit expressibility only; no SUCCESS claim for CLI multi-step |

*All Phase 49 in-scope behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** Wave 0 complete; host automated legs green (2026-07-29). Ready for `/gsd-verify-work`.

## Validation Audit 2026-08-11

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

Current adversarial rerun: CODE-01 five-node sequence, CODE-02 intent-only anti-launder, missing-key fail-closed behavior, feature-gated I2 expressibility, named-handle serde, and broker multi-mint all passed. The I2 command above now includes its required non-default feature so it executes one test rather than filtering the test out.
