---
phase: 42-policy-layer-binding-enforcement-the-i2-boundary
plan: 04
subsystem: infra
tags: [policy, session-binding, containment, audit-dag, sha256, immutability, brokerd]

# Dependency graph
requires:
  - phase: 42-01
    provides: SessionPolicy type + broker_default()/allow_all() in runtime-core
  - phase: 42-02
    provides: adapter_fs::containment::refuse_if_beneath_workspace shared helper
  - phase: 42-03
    provides: deny-only pre-I2 policy gate + policy threaded through run_broker_server
provides:
  - "brokerd::policy::bind_policy(policy_path, workspace_root) → (SessionPolicy, hash)"
  - "Policy bound at session creation from a trusted source, refused if at-or-beneath the workspace root (shared helper)"
  - "Immutable session policy captured by value + threaded into run_broker_server (replaces internal placeholder)"
  - "Genuine hash-chained policy_bound audit-DAG event carrying the policy SHA-256 identity"
affects: [phase-43, phase-44, phase-45, milestone-audit, verify-work]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Trusted-source binding at session creation via the SAME shared containment predicate as MAC-key custody (no re-inline; Gate 6 covers both sites)"
    - "Bind-once-by-value immutability: policy captured at bind time, never re-read from disk mid-session"
    - "Identity-as-audit-event: SHA-256 of the canonical policy rides in the Event actor field, hashed into the SHA-256 chain (genuine, not stapled)"

key-files:
  created:
    - crates/brokerd/src/policy.rs
  modified:
    - crates/brokerd/src/lib.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/main.rs

key-decisions:
  - "CAPRUN_POLICY env var (not a new CLI flag) is the policy-path hook — lowest-surface; SDK-01 (Phase 45) formalizes the CLI surface"
  - "Policy identity hash is SHA-256 of the CANONICAL serialized SessionPolicy (BTreeSet/BTreeMap sorted order) — a semantic identity, deterministic regardless of on-disk JSON key order/whitespace"
  - "policy_bound is the chain head seeded into the broker (broker chains its first event onto policy_bound, not session_created)"
  - "8 policy-agnostic run_broker_server test call sites pass SessionPolicy::allow_all()"

patterns-established:
  - "bind_policy delegates to the shared refuse_if_beneath_workspace helper — never a second canonicalize+prefix-compare copy (anti-drift Gate 6)"
  - "Fail-closed on unresolvable/at-or-beneath/unparseable policy path (hard Err, no session); None binds broker_default() (never allow-everything, never a refusal)"

requirements-completed: [POLICY-03]

coverage:
  - id: D1
    description: "bind_policy refuses a policy path at-or-beneath the workspace root (F1-precedent) via the shared containment helper — no session"
    requirement: "POLICY-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bind_policy_refuses_path_beneath_workspace_root"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bind_policy_refuses_path_equal_to_workspace_root"
        status: pass
    human_judgment: false
  - id: D2
    description: "bind_policy fail-closes on unresolvable/unparseable input; None binds broker_default() with a deterministic hash"
    requirement: "POLICY-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bind_policy_fail_closed_on_unresolvable_path"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bind_policy_fail_closed_on_unparseable_policy"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bind_policy_none_binds_broker_default"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bind_policy_hash_is_deterministic"
        status: pass
    human_judgment: false
  - id: D3
    description: "The bound policy is immutable — a mid-session policy-file rewrite does not change the enforced allowlist"
    requirement: "POLICY-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#bound_policy_is_immutable_across_a_mid_session_file_rewrite"
        status: pass
    human_judgment: false
  - id: D4
    description: "The policy identity/hash is a genuine hash-chained policy_bound audit-DAG event (verify_chain passes; recorded hash matches)"
    requirement: "POLICY-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/policy.rs#policy_bound_event_is_genuinely_hash_chained"
        status: pass
    human_judgment: false
  - id: D5
    description: "caprun run binds the policy, appends the hash-chained policy_bound event, and threads the immutable bound policy into run_broker_server; existing e2e flows stay green under broker_default()"
    requirement: "POLICY-03"
    verification:
      - kind: integration
        ref: "cargo test -p brokerd -p caprun (all pass) + cargo build --workspace --tests"
        status: pass
      - kind: e2e
        ref: "Linux container gate (mailpit-verify.sh / composed live proof) — cfg(linux) e2e targets not compiled on macOS host"
        status: unknown
    human_judgment: false

# Metrics
duration: ~35min
completed: 2026-07-18
status: complete
---

# Phase 42 Plan 04: Policy Layer Binding Enforcement (POLICY-03) Summary

**The broker binds the session policy at session creation from a trusted source outside the confined worker's reach — refusing any at-or-beneath-workspace path via the SAME shared containment helper as MAC-key custody, capturing it immutably by value, and recording its SHA-256 identity as a genuine hash-chained policy_bound audit-DAG event.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-07-18T15:01Z (approx)
- **Completed:** 2026-07-18T15:36:02Z
- **Tasks:** 3 (Task 1 binder + Task 3 tests co-located in policy.rs; Task 2 wiring)
- **Files modified:** 4 source + 6 test files

## Accomplishments
- `crates/brokerd/src/policy.rs::bind_policy(policy_path, workspace_root) → (SessionPolicy, hash)` — trusted-source binding with F1-precedent containment, fail-closed on bad input, deterministic SHA-256 identity.
- The containment check DELEGATES to `adapter_fs::containment::refuse_if_beneath_workspace` (the shared Plan-02 helper key custody uses) — never a re-inlined copy; anti-drift Gate 6 now covers BOTH call sites and passes.
- The policy is captured BY VALUE at bind time and threaded immutably into `run_broker_server` (replacing Plan 03's internal placeholder); the immutability negative test proves a mid-session file rewrite has zero effect on enforcement.
- `caprun run` appends a genuine `policy_bound` `append_event` chained after `session_created` (hash rides in `actor`, hashed into the SHA-256 chain), seeds the broker with it as the chain head, and passes the bound policy in; `verify_chain` proves it after the fact.

## Task Commits

1. **Task 1 + Task 3: broker policy binder + immutability/audit tests** — `da27b66` (feat) — `crates/brokerd/src/policy.rs` (binder + all 9 tests, co-located in one file), `crates/brokerd/src/lib.rs` (`pub mod policy`)
2. **Task 2: bind at session creation, hash-chain policy_bound, thread immutable policy** — `cd1ccae` (feat) — `crates/brokerd/src/server.rs`, `cli/caprun/src/main.rs`, 6 test call sites updated

**Plan metadata:** (final docs commit)

## Files Created/Modified
- `crates/brokerd/src/policy.rs` (created) — `bind_policy` + `policy_identity_hash` + 9 unit tests (containment, fail-closed, broker_default, determinism, immutability, audit-chain).
- `crates/brokerd/src/lib.rs` — registered `pub mod policy`.
- `crates/brokerd/src/server.rs` — `run_broker_server` takes `policy: SessionPolicy` by value; removed the internal `broker_default()` placeholder; `Arc::new(policy)` construct-once, threaded per-connection unchanged.
- `cli/caprun/src/main.rs` — bind via `CAPRUN_POLICY` env path (fail-closed `?`), append hash-chained `policy_bound` event, seed broker with policy_bound as chain head, pass bound policy in.
- Test call sites updated (policy-agnostic `allow_all()`): `crates/brokerd/tests/{replay_cas,two_connection_intent_bypass,planner_capability_split,planner_reduced_signal,uds_ipc}.rs`, `cli/caprun/tests/harden04_featureless_create_session.rs`.

## Decisions Made
- **Policy-path hook = `CAPRUN_POLICY` env var**, not a new CLI flag (lowest surface; SDK-01/Phase 45 owns the CLI surface).
- **Hash = SHA-256 of the canonical serialized `SessionPolicy`** (BTreeSet/BTreeMap sorted order) — a deterministic semantic identity, not a raw-file-byte identity, so equivalent policies (any JSON key order/whitespace) share one identity.
- **`policy_bound` is the chain head** seeded into the broker (`initial_last_event_id`/`hash`), so the broker's first event chains onto it — the recorded policy identity is an unbroken audit-DAG edge before any effect.

## Deviations from Plan
None — plan executed exactly as written. (The plan text at Task 2 referenced removing an internal `allow_all()` placeholder; Plan 03 had actually landed `broker_default()`. Same intent — the internal construction was removed and replaced by the passed-in parameter. No behavioral divergence.)

## Issues Encountered
None. All local gates green on the macOS host: `cargo build --workspace --tests`, `cargo test -p brokerd` (189 lib + integration all pass), `cargo test -p caprun` (all pass), `bash scripts/check-invariants.sh` exit 0 (Gate 6 anti-drift PASS covering both key.rs and the new brokerd policy.rs).

## Linux verification note ([[cfg-linux-test-blindness]])
`bind_policy` + immutability + audit tests are host-portable and pass on macOS. The `main.rs`/`server.rs` `run_broker_server`-signature change links `cfg(target_os="linux")` e2e targets that macOS does NOT compile. **For the orchestrator's container gate:** run `cargo build --workspace --tests` in the Linux container to compile-check every gated caller of the changed signature, and `cargo build --workspace` before any test resolving the sibling `caprun-worker` binary ([[cargo-test-workspace-missing-sibling-binary]]). No new sink/effect behavior was added on the live path (bind default = `broker_default()`, permits all 7 production sinks), so existing composed live proof legs should remain green.

## Next Phase Readiness
- POLICY-03 (the converged BLOCKER) is closed: policy bound from a trusted source outside worker reach, immutable, hash-chained. Phase 42 (Policy Layer) is functionally complete pending the container gate + verifier.
- No blockers introduced.

---
*Phase: 42-policy-layer-binding-enforcement-the-i2-boundary*
*Completed: 2026-07-18*

## Self-Check: PASSED
- FOUND: crates/brokerd/src/policy.rs
- FOUND: 42-04-SUMMARY.md
- FOUND: commit da27b66 (Task 1+3 binder)
- FOUND: commit cd1ccae (Task 2 wiring)
