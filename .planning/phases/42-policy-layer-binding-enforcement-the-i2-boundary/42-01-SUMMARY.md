---
phase: 42-policy-layer-binding-enforcement-the-i2-boundary
plan: 01
subsystem: runtime-core (policy layer foundation)
tags: [policy, POLICY-01, deny-by-default, executor-decision, runtime-core-purity]
requires:
  - runtime-core::plan_node::SinkId
  - crates/executor/src/sink_schema.rs (KNOWN_SINKS — the seven production sinks)
provides:
  - runtime-core::policy::SessionPolicy (hardcoded-schema, deny-by-default narrowing gate)
  - runtime-core::policy::{ArgConstraint, PolicyDenyKind}
  - runtime-core::executor_decision::DenyReason::PolicyDeny (distinct wire variant)
affects:
  - Plan 03 (executor gate) — calls SessionPolicy::evaluate / permits_sink, maps PolicyDenyKind → DenyReason::PolicyDeny
  - Plan 04 (broker binder) — returns SessionPolicy::broker_default() for bind_policy(None, ..); never edits policy.rs
tech-stack:
  added: []            # v1.9 adds ZERO new crates; serde/serde_json already workspace deps
  patterns:
    - "Result<(), _>-only evaluation surface (no Allow variant) — structural POLICY-02 pre-condition"
    - "Owned-field DenyReason variant (v1.5 SlotTypeMismatch wire-safety precedent)"
    - "BTreeSet/BTreeMap for deterministic serde ordering"
key-files:
  created:
    - crates/runtime-core/src/policy.rs
  modified:
    - crates/runtime-core/src/lib.rs
    - crates/runtime-core/src/executor_decision.rs
decisions:
  - "PolicyDenyKind kept internal to policy.rs (not serde); the wire-crossing type is DenyReason::PolicyDeny with owned String fields."
  - "ArgConstraint allowlist entries matched as PREFIXES — coarsely covers both host (url prefix) and path-prefix cases; fine-grained F1 fs containment deferred to adapter-fs (Plan 02)."
  - "An empty ArgConstraint allowlist denies every literal (fail-closed)."
  - "constraint tags: sink-not-allowed / arg-not-allowlisted (generic, stable machine-readable)."
metrics:
  duration: ~15m
  completed: 2026-07-18
status: complete
---

# Phase 42 Plan 01: Policy Layer Foundation (SessionPolicy + DenyReason::PolicyDeny) Summary

Landed the two pure `runtime-core` foundation pieces the whole policy layer builds on — a hardcoded-schema `SessionPolicy` deny-by-default narrowing gate (POLICY-01) and a distinct, machine-checkable `DenyReason::PolicyDeny` executor-decision tag — as tested types with no enforcement wiring, so Plans 03/04 build against a frozen contract.

## What was built

**Task 1 — `SessionPolicy` (`crates/runtime-core/src/policy.rs`, `lib.rs`)** — commit `3828511`
- `SessionPolicy { allowed_sinks: BTreeSet<String>, arg_constraints: BTreeMap<String, BTreeMap<String, ArgConstraint>> }`, deriving `Debug, Clone, PartialEq, serde::{Serialize, Deserialize}`.
- Deny-by-default on two axes: sink not in `allowed_sinks` → deny; a configured coarse arg allowlist that the literal fails → deny; an unconstrained arg on an allowed sink → permit.
- Evaluation surface returns **only** permit-or-deny: `evaluate(&self, sink, arg_name, literal) -> Result<(), PolicyDenyKind>` and `permits_sink(&self, sink) -> bool`. **No Allow-and-skip path exists** (structural POLICY-02 pre-condition, T-42-01).
- Three constructors, all defined here (wave 1): `default_fail_closed()` (empty, denies everything), `allow_all()` (permits the production sinks, no arg constraints), `broker_default()` (EXPLICIT allowlist of the seven production sinks — `email.send, file.create, file.write, process.exec, git.commit, http.request, github.pr` — and denies any unlisted/future sink e.g. `git.push`).
- Internal `PolicyDenyKind` (`SinkNotAllowed` / `ArgNotAllowlisted`) with `constraint_tag()` → stable machine-readable tags. Not serde'd itself; the wire type is `DenyReason::PolicyDeny`.
- 10 unit tests: empty-denies-all, allowed-sink-permits/denies-another, coarse-constraint permit+deny, unconstrained-arg permit, empty-allowlist fail-closed, serde JSON round-trip, broker_default 7-sinks + unlisted-deny, allow_all, default_fail_closed empty, tag stability.
- runtime-core purity preserved — check-invariants Gate 2 PASS.

**Task 2 — `DenyReason::PolicyDeny` (`crates/runtime-core/src/executor_decision.rs`)** — commit `9ba3d47`
- Added `PolicyDeny { sink: String, arg: Option<String>, constraint: String }` — owned fields per the v1.5 `SlotTypeMismatch` wire-safety precedent (the enum derives `Deserialize` and crosses the IPC wire; borrowed `&'static` refs are not deserializable).
- Extended BOTH exhaustive matches (no wildcard): `code()` → `"policy_deny"`; `Display` renders a sink-level or arg-scoped message naming sink (+ arg when present) and constraint.
- Unit test `policy_deny_code_and_display`: asserts `code()=="policy_deny"`, non-empty Display naming sink/arg, and that it is a `Denied{reason}` — never a `BlockedPendingConfirmation` (POLICY-01 / LIVE-06 leg-3 attributability).

## Verification

- `cargo test -p runtime-core` — 15 lib + all integration tests pass, 0 failed.
- `cargo build --workspace` — compiles cleanly; the new variant links across executor, brokerd, and cli.
- `./scripts/check-invariants.sh` — EXIT 0, all gates PASS (Gate 2 runtime-core purity in particular).
- Per [[cfg-linux-test-blindness]]: all tests here are pure Rust and run on the macOS host directly — no `#[cfg(target_os="linux")]` targets in this plan. Full Linux gating runs at phase end.

## Deviations from Plan

**1. [Rule 3 - Blocking] Reworded a doc comment to avoid check-invariants Gate 2 false-positive**
- **Found during:** Task 1 verification.
- **Issue:** Gate 2 greps `crates/runtime-core/src/` for the literal tokens `std::io|std::fs|std::net|tokio|async fn`. My purity doc comment quoted those exact tokens (`no std::fs/std::io/std::net/tokio/async`), tripping the grep and failing the gate even though the code is pure.
- **Fix:** Reworded the comment to "no filesystem, network, or async tokens" — no literal forbidden tokens; code unchanged.
- **Files modified:** `crates/runtime-core/src/policy.rs` (comment only).
- **Commit:** folded into `3828511`.

## Known Stubs

None. Both types are complete for their wave-1 contract. Enforcement wiring (calling `evaluate`/`permits_sink` from the executor and binding a policy in the broker) is intentionally deferred to Plans 03 and 04 — this plan lands types + tests only, by design.

## Threat Flags

None. No new network endpoints, auth paths, file access, or trust-boundary schema introduced — this plan is pure types. The two threat-register mitigations for this plan (T-42-01 no-Allow evaluation surface; T-42-02 distinct PolicyDeny tag) are both satisfied structurally, as verified by tests.

## Self-Check: PASSED
- FOUND: `crates/runtime-core/src/policy.rs`
- FOUND commit `3828511` (Task 1) and `9ba3d47` (Task 2) in git log.
