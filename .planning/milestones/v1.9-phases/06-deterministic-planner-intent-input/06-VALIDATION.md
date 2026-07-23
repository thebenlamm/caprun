---
phase: 6
slug: deterministic-planner-intent-input
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-30
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `06-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`) |
| **Config file** | `Cargo.toml` workspace members (no separate test config) |
| **Quick run command** | `cargo test --workspace --no-fail-fast` (macOS — Linux e2e `#[cfg]` stubs pass as 0-assertion no-ops, which is correct) |
| **Full suite command** | `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast` |
| **Estimated runtime** | ~60 seconds (macOS quick) / ~5 min (Linux full via Colima) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --workspace --no-fail-fast`
- **After every plan wave:** Run the full suite
- **Before `/gsd-verify-work`:** Full suite must be green on Linux (Colima/Docker)
- **Max feedback latency:** ~60 seconds (macOS quick loop)

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| PLAN-01 | caprun parses `<intent-kind> <intent-param> <workspace-file>` | unit/e2e | `cargo test -p caprun --test e2e` | ❌ W0 | ⬜ pending |
| PLAN-02 | `plan_from_intent(SendEmailSummary, intent_vid, []) → PlanNode{email.send, to: intent_vid}` | unit | `cargo test -p caprun` (new `tests/planner.rs`) | ❌ W0 | ⬜ pending |
| PLAN-03 | Planner fn receives only `CaprunIntent` + `ValueId` handles — no raw bytes/taint (compile-time) | compile | `cargo build --workspace` | compile-time | ⬜ pending |
| PLAN-04 | `mint_from_intent` mints UserTrusted record + appends `intent_received`; `provenance_chain[0] == event_id` | unit | `cargo test -p brokerd quarantine` | ❌ W0 | ⬜ pending |
| PLAN-04 | `ProvideIntent` IPC dispatch: broker receives intent, mints, returns ValueId | unit | `cargo test -p brokerd --test proto_claims` (extend) | ❌ W0 | ⬜ pending |
| HARD-02 | `TaintLabel::UserTrusted.is_untrusted()` returns false | unit | `cargo test -p runtime-core` | ❌ W0 | ⬜ pending |
| HARD-02 | `TaintLabel::ExternalUntrusted.is_untrusted()` returns true | unit | `cargo test -p runtime-core` | ❌ W0 | ⬜ pending |
| HARD-02 | Executor: UserTrusted-only record in routing-sensitive arg → Allowed | unit | `cargo test -p executor --test executor_decision` | ❌ W0 | ⬜ pending |
| HARD-02 | Executor: ExternalUntrusted record in routing-sensitive arg → still BlockedPendingConfirmation | unit | `cargo test -p executor --test executor_decision` (existing) | Extend | ⬜ pending |
| PLAN-04+HARD-02 | Clean allow-path in-process: mint_from_intent → PlanNode → Allowed | integration | `cargo test -p brokerd --test s9_acceptance` (extend) | ❌ W0 | ⬜ pending |
| PLAN-01+PLAN-04+HARD-02 | Live caprun clean-path: exits 0, `plan_node_evaluated` + `intent_received` in DAG | e2e (Linux `#[cfg]`) | `cargo test -p caprun --test s9_live_block` (extend) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Files to create/extend before implementation lands its behavior:

- [ ] `crates/runtime-core/src/intent.rs` — ADD `CaprunIntent` enum + `TaintLabel::is_untrusted()` (extend existing file)
- [ ] `crates/brokerd/src/quarantine.rs` — ADD `mint_from_intent()` (extend existing file)
- [ ] `crates/brokerd/src/proto.rs` — ADD `ProvideIntent` + `IntentAccepted` wire types (extend existing file)
- [ ] `crates/brokerd/src/server.rs` — ADD `ProvideIntent` dispatch arm (extend existing file)
- [ ] `crates/executor/src/lib.rs` — UPDATE blocking predicate to use `is_untrusted()` (edit existing file)
- [ ] `cli/caprun/src/planner.rs` — NEW file: `plan_from_intent()`
- [ ] `cli/caprun/src/main.rs` — UPDATE arg parsing (edit existing file)
- [ ] `cli/caprun/src/worker.rs` — ADD ProvideIntent send + intent_value_id use (edit existing file)
- [ ] `crates/executor/tests/executor_decision.rs` — ADD HARD-02 cases (extend existing file)
- [ ] `crates/brokerd/tests/quarantine.rs` (or extend `s9_acceptance.rs`) — ADD `mint_from_intent` anchor test
- [ ] `cli/caprun/tests/s9_live_block.rs` — ADD Linux-gated clean-path e2e test

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live clean allow-path on real Linux kernel confinement | PLAN-01+PLAN-04+HARD-02 | Landlock/seccomp enforcement is Linux-only; macOS stubs are no-ops | Run full suite via Colima/Docker (see Test Infrastructure); confirm caprun exits 0 and `intent_received` + `plan_node_evaluated` events are in the audit DB |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s (macOS quick loop)
- [ ] `nyquist_compliant: true` set in frontmatter (after planner maps every task to a verify)

**Approval:** pending
