---
phase: 36-git-commit-sink
plan: 01
subsystem: executor
tags: [git, sink, executor, tcb, i2, taint]
status: complete
requires: []
provides:
  - "git.commit registered in KNOWN_SINKS (exact-match schema, allowed=required=[message])"
  - "sink_effect_class(git.commit) => MutateReversible (first non-CommitIrreversible real sink)"
  - "is_content_sensitive(git.commit, message) => true (taint carrier)"
  - "expected_role(git.commit, message) => None (unconstrained Step-1c)"
affects:
  - crates/executor/src/sink_schema.rs
  - crates/executor/src/sink_sensitivity.rs
tech-stack:
  added: []
  patterns:
    - "hardcoded TCB table rows (schema + sensitivity + role), never a policy file"
    - "exact-match single-arg schema (mirrors file.write)"
    - "deliberate MutateReversible exception to the fail-closed CommitIrreversible default"
key-files:
  created: []
  modified:
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs
decisions:
  - "Scope git.commit args to `message` ONLY — no paths/pathspec (Phase 36 commits already-staged changes), tightening the Step-0 gate."
  - "git.commit is MutateReversible (reversible local commit, no external egress) so it survives an I1-demoted Draft session — a justified, single-arm exception to the unknown=>CommitIrreversible default."
  - "`message` expected_role is None (unconstrained) — no origin_role mint site for a legit commit message; the Block comes from is_content_sensitive + taint, not the structural role gate."
metrics:
  duration: ~8m
  completed: 2026-07-18
  tasks: 2
  files: 2
---

# Phase 36 Plan 01: git.commit executor TCB rows Summary

Added the hardcoded executor TCB table rows that make `git.commit` a callable, correctly-classified sink — the `KNOWN_SINKS` exact-match schema row, the `MutateReversible` effect-class arm, and the `message` content-sensitivity + role rows — entirely within `crates/executor`, with no brokerd/launcher dependency. This is the mechanical realization of DESIGN-git-github-http-sinks.md §1.2/§1.3 (CONTEXT decisions 2 and 3). The broker sink dispatch is deliberately deferred to Plan 36-02.

## What Was Built

### Task 1 — KNOWN_SINKS schema row (commit 42d272c)
- New `SinkSchema` for `git.commit`: `allowed = ["message"]`, `required = ["message"]`.
- Exact-match, single-arg shape mirroring `file.write` (both allowed and required), NOT `process.exec`'s optional-arg asymmetry.
- No paths/pathspec arg — Phase 36 commits already-STAGED changes, so `git commit -m <message>` needs no pathspec; a single exact-match arg tightens the Step-0 fail-closed gate.
- Tests: registered-sink (never UnknownSink), exact-args-ok, unknown-arg denied (`paths`), duplicate-arg denied, missing-required denied.

### Task 2 — effect-class + sensitivity + role (commit af9c345)
- `sink_effect_class`: added `"git.commit" => EffectClass::MutateReversible` before the fail-closed `_ => CommitIrreversible` default. FIRST non-CommitIrreversible real sink. A local commit is reversible (`git reset`/`--amend`/branch delete) with no external egress, matching `ReversibleEffect` in the locked 3-class ontology — so an Allowed `git.commit` survives an I1-demoted (Draft) session. The `lib.rs` caller uses `== EffectClass::CommitIrreversible` equality (not a match), so exhaustiveness is unaffected and no fourth variant is introduced.
- `is_content_sensitive`: added `GIT_COMMIT_CONTENT_SENSITIVE = &["message"]` const + `"git.commit"` arm. `message` is the taint CARRIER — a tainted message Blocks under the UNMODIFIED collect-then-Block loop, exactly like an `email.send` body.
- `expected_role`: added `"git.commit" => match arg_name { "message" => None, _ => None }`. `message` is deliberately unconstrained at the Step-1c structural role gate (reusing the process.exec command/args rationale) — no origin_role mint site exists for a legit commit message; pinning `Some(...)` would fail-closed-Deny the legit flow. Not an I2 bypass — the Block comes entirely from content-sensitivity + the untrusted-taint check.
- No `is_routing_sensitive` arm added — `git.commit` has no routing-sensitive arg, so `message` correctly falls through to `_ => false`; asserted by a fall-through test rather than a table row.
- Tests: MutateReversible classification, message content-sensitive, message NOT routing-sensitive (fall-through), message expected_role None.

## Verification Results

- `cargo build --workspace` — clean (executor, brokerd, caprun all compile).
- `cargo test -p executor` — **66 lib + 20 integration = 86 passed, 0 failed** on macOS (host-portable; no Linux-gated tests in this plan).
  - `sink_schema` subset: 22 passed (5 new git.commit tests).
  - `sink_sensitivity` subset: 37 passed (4 new git.commit tests).
- `./scripts/check-invariants.sh` — **all 4 gates PASS, exit 0** (no new EffectRequest token; sensitivity stays hardcoded; no policy file; test-fixtures not a brokerd default feature).
- No fourth `EffectClass` variant introduced; `sink_effect_class` callers stay exhaustive.

## TDD Gate Compliance

Both tasks followed RED→GREEN. RED was verified before implementation:
- Task 1 RED: 5 tests failing on `Err(UnknownSink("git.commit"))`.
- Task 2 RED: 2 tests failing (effect-class defaulted CommitIrreversible; content-sensitive defaulted false); the routing/role tests passed immediately via correct fall-through defaults, as the plan specified.

Note: RED and GREEN were combined into one atomic `feat` commit per task (rather than separate `test`/`feat` commits) because the plan type is `execute` and `tdd_mode` is false at the plan level; the RED failures are documented above and in the commit bodies.

## Deviations from Plan

None — plan executed exactly as written.

## Threat Surface

No new security surface beyond the plan's threat register (T-36-01 EoP via the MutateReversible arm, T-36-02 InfoDisclosure via message content-sensitivity, T-36-03 Tampering via the exact-match schema) — all three mitigated as designed. No new dependencies (T-36-SC accept holds). The broker dispatch that would actually execute a commit is Plan 36-02, not this plan.

## Self-Check: PASSED

- crates/executor/src/sink_schema.rs — FOUND, modified.
- crates/executor/src/sink_sensitivity.rs — FOUND, modified.
- Commit 42d272c — FOUND (Task 1).
- Commit af9c345 — FOUND (Task 2).
