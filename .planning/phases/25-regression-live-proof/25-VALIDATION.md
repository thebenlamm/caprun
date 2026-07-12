---
phase: 25
slug: regression-live-proof
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-11
---

# Phase 25 — Validation Strategy

> Per-phase validation contract for the v1.5 milestone-close gate.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust workspace) + `scripts/mailpit-verify.sh` (Linux-live via Colima+Docker) |
| **Config file** | none — workspace `Cargo.toml`; `scripts/mailpit-verify.sh` for Linux |
| **Quick run command** | `cargo test -p brokerd` / `cargo test -p executor` (Mac, for the held-out test's Mac-buildable layer) |
| **Full suite command** | `bash scripts/mailpit-verify.sh` (DEFAULT recipe, NO `MAILPIT_VERIFY_CMD` override — the milestone-close gate) |
| **Estimated runtime** | ~2–4 min Mac workspace; ~5–10 min live Linux verify (container build + SMTP) |

> **Milestone-close discipline (load-bearing):** T2-08 is the v1.5 DONE gate. Run the ACTUAL DEFAULT
> `bash scripts/mailpit-verify.sh` — scoped `MAILPIT_VERIFY_CMD` runs have masked real bugs before
> (v1.4 `caprun-planner` binary-placement bug). Capture the true exit code BEFORE any pipe; assert on
> named tests + 0-failure counts, never on exit-0-through-a-pipe. Independent re-run at gate time — do
> not trust a prior/scoped pass. Colima+Docker confirmed running this session.

---

## Sampling Rate

- **After every task commit:** crate-scoped `cargo test -p <crate>` for the touched crate
- **After the held-out test lands (T2-06):** `cargo test -p brokerd` (Mac layer) green
- **Before milestone close:** `bash scripts/mailpit-verify.sh` (default) independently green, 0 failures
- **Max feedback latency:** ~4 min Mac / ~10 min Linux

---

## Per-Task Verification Map

> Populated/finalized by the planner. IDs illustrative.

| Task ID | Plan | Wave | Requirement | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------------|-----------|-------------------|--------|
| 25-01-01 | 01 | 1 | T2-06 | swapped subject↔recipient (both UserTrusted) → SlotTypeMismatch Denied via real broker path; audit `plan_node_evaluated` event recorded; `verify_chain` true | integration | `cargo test -p brokerd <held_out_test>` | ⬜ pending |
| 25-02-01 | 02 | 1 | T2-07 | regression audit: no fixture at a role-checked slot silently passes with `origin_role: None`; no reliance on old permissive behavior | audit+unit | `cargo test --workspace --no-fail-fast` | ⬜ pending |
| 25-03-01 | 03 | 2 | T2-08 | full Linux regression independently green, 0 failures, default recipe | live-linux | `bash scripts/mailpit-verify.sh` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red. Planner finalizes IDs/waves.*

---

## Genuine-Chain Discipline (project tripwire #1 — NOT stapled)

T2-06 must prove a GENUINE audited chain, never a stapled assertion:
- The value carries a REAL `origin_role` assigned at mint time by the broker (`mint_from_intent`), not a hand-set field at the sink.
- The `Denied` is the executor's DETERMINISTIC output from that role vs the slot's expected role.
- **What is actually recorded (per RESEARCH):** a `Denied` yields a bare `plan_node_evaluated` audit event with `anchors: []` — the `SlotTypeMismatch` reason is NOT in the DAG payload. Assert on the event's presence + `verify_chain` staying true + the `ExecutorDecision::Denied{SlotTypeMismatch}` returned by the evaluation — do NOT assert a reason-rich DAG record that does not exist.
- Both handles are `UserTrusted` and otherwise valid, so neither I0 (class-deny) nor I2 (untrusted-Block) fires — ONLY Step 1c catches them. That isolation is what makes the test prove T2 specifically.

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements — `cargo test` + `scripts/mailpit-verify.sh`
are already wired. The held-out test likely hand-mirrors the audit-DAG append (as
`clean_path_intent_value_evaluates_to_allowed` does) if `evaluate_plan_node_and_record` is not
`pub`-reachable from `crates/brokerd/tests/` — planner to confirm.*

---

## Manual-Only / Environment-Gated Verifications

| Behavior | Requirement | Why | Instructions |
|----------|-------------|-----|--------------|
| Live Linux SMTP deny/deliver proof | T2-08 | Landlock/seccomp + real SMTP are Linux-only | `bash scripts/mailpit-verify.sh` with Colima+Docker up (confirmed running) |

---

## Validation Sign-Off

- [ ] T2-06 held-out test asserts only what is actually recorded (generic event + verify_chain), genuine role chain
- [ ] T2-07 audit re-confirmed: no silent-bypass fixture; Mac-buildable direct-mint files independently re-swept
- [ ] T2-08 default `mailpit-verify.sh` independently green, true exit code captured pre-pipe, 0 failures
- [ ] `nyquist_compliant: true` set once map is finalized

**Approval:** pending
