---
phase: 49
slug: deterministic-multi-step-coding-planner
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-29
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
| 49-*-coding-seq | TBD | 1 | CODE-01 | T-49-launder | 5 sinks in order with exact sink_schema arg names | unit | `cargo test -p caprun --test planner coding_` | ❌ W0 | ⬜ pending |
| 49-*-email-reg | TBD | 1 | CODE-01 | — | Email plan_next step0 matches plan() | unit | `cargo test -p caprun --test planner plan_next_step0_matches_plan_for_email` | ✅ | ⬜ pending |
| 49-*-file-reg | TBD | 1 | CODE-01 | — | File plan_next step0 matches plan() | unit | `cargo test -p caprun --test planner plan_next_step0_matches_plan_for_file` | ✅ | ⬜ pending |
| 49-*-no-out | TBD | 1 | CODE-02 | T-49-launder | Success-path args only intent bag keys (no `out_*`) | unit | `cargo test -p caprun --test planner coding_success_path_does_not_place_out_handles` | ❌ W0 | ⬜ pending |
| 49-*-mint | TBD | 1 | CODE-02 | T-49-remint | ProvideIntent multi-mint distinct named UserTrusted handles | unit/integration | broker/proto mint tests | ❌ W0 | ⬜ pending |
| 49-*-i2-proof | TBD | 2 | CODE-02 | T-49-launder | LIVE-08 expressibility: proof path places `out_*` into sensitive arg | unit | `cargo test -p caprun --test planner coding_i2_proof_places_out_handle` | ❌ W0 | ⬜ pending |
| 49-*-hyg | TBD | * | HYG-02 | T-49-effect | Gate 1/3; zero new crates | script | `./scripts/check-invariants.sh` | ✅ | ⬜ pending |
| 49-*-stream | TBD | * | STREAM regression | — | stream_substrate still green | unit | `cargo test -p caprun --test stream_substrate` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cli/caprun/tests/planner.rs` — coding multi-node emission tests (CODE-01)
- [ ] `cli/caprun/tests/planner.rs` — success-path no-`out_*` placement (CODE-02)
- [ ] `cli/caprun/tests/planner.rs` — LIVE-08 expressibility placement test (CODE-02)
- [ ] ProvideIntent multi-mint / IntentAccepted named_handles round-trip tests (`crates/brokerd/tests/proto_claims.rs` or sibling)
- [ ] Exhaustive CaprunIntent match compile sweep (plan task, not a test file)
- [ ] Framework install: **none** — use existing cargo test

*Existing infrastructure covers email/file plan_next, stream substrate, Gate scripts; coding-specific tests are Wave 0.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CLI multi-node driver / Block-and-Hold product path | CLI-01/CONFIRM-01 | Phase 50 scope | Confirm Phase 49 does not implement `caprun run` coding verb or mid-loop hold UX |
| Non-hybrid LIVE-07/08 proof | LIVE-07/08 | Phase 51 scope | Unit expressibility only; no SUCCESS claim for CLI multi-step |

*If none beyond deferrals: "All Phase 49 in-scope behaviors have automated verification planned."*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
