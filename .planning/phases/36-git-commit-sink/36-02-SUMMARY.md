---
phase: 36-git-commit-sink
plan: 02
subsystem: brokerd
tags: [git, sink, brokerd, tcb, exec, launcher, taint, rce-mitigation]
status: complete
requires:
  - "36-01: git.commit registered in KNOWN_SINKS + MutateReversible + message content-sensitive"
provides:
  - "git.commit broker sink: invoke_git_commit (Pattern B, confined caprun-exec-launcher spawn)"
  - "run_launcher extended with extra_env: &[(&str,&str)] param (pub(crate)); process.exec passes &[] (byte-identical)"
  - "server.rs git.commit Allowed-dispatch arm minting captured output via the EXISTING mint_from_exec (no new mint site)"
  - "git config/hook neutralization (GIT_CONFIG_NOSYSTEM, GIT_CONFIG_GLOBAL=/dev/null, -c core.hooksPath=/dev/null, -c user.name/email, GIT_TERMINAL_PROMPT=0, env_clear'd child)"
affects:
  - crates/brokerd/src/sinks/process_exec.rs
  - crates/brokerd/src/sinks/git_commit.rs
  - crates/brokerd/src/sinks.rs
  - crates/brokerd/src/server.rs
  - crates/brokerd/tests/git_commit_spawn.rs
tech-stack:
  added: []
  patterns:
    - "Pattern B sink: git IS an exec under the shipped launcher; reuse run_launcher + mint_from_exec verbatim"
    - "broker-constructed trusted argv (never arg-derived subcommand) + command-line -c hooksPath override (highest git precedence)"
    - "two-phase process_exited/process_spawn_failed durable audit, never mints (Gate 3)"
    - "Linux-gated spawn tests (#[cfg(target_os=\"linux\")]) — 0 on macOS is expected (cfg-linux-test-blindness)"
key-files:
  created:
    - crates/brokerd/src/sinks/git_commit.rs
    - crates/brokerd/tests/git_commit_spawn.rs
  modified:
    - crates/brokerd/src/sinks/process_exec.rs
    - crates/brokerd/src/sinks.rs
    - crates/brokerd/src/server.rs
decisions:
  - "git.commit reuses run_launcher + mint_from_exec unchanged — no new mint site, no new TaintLabel (DESIGN §1.4)"
  - "run_launcher grew extra_env (surgical); both process.exec call sites pass &[] so process.exec child env is byte-identical"
  - "git.commit deliberately NOT on the Step 4.75 confirm-release allow-list — a tainted message just Blocks (no P33/P34 confirm-release audit-gap surface this phase)"
  - "env-clear proven git-specifically: broker GIT_AUTHOR_NAME/GIT_COMMITTER_NAME (which would OVERRIDE -c user.name if inherited) must NOT appear as the commit author — a genuine, non-weak assertion, not an output-absence check"
  - "planted-hook sentinel uses pure /bin/sh redirection (: > file) inside the workspace root, so its absence proves the hook never fired (not that a subordinate write was merely Landlock-blocked)"
metrics:
  duration: ~25m
  completed: 2026-07-18
  tasks: 3
  files: 5
---

# Phase 36 Plan 02: brokerd git.commit sink dispatch Summary

Wired the broker-side `git.commit` sink end-to-end: a new `sinks/git_commit.rs`
module that dispatches Pattern B (reusing the v1.7 `caprun-exec-launcher` +
`run_launcher`), a `server.rs` Allowed-dispatch arm that mints the captured
output via the EXISTING `mint_from_exec`, and the git config/hook neutralization
that closes the P2 RCE pitfall. This is the mechanical realization of
DESIGN-git-github-http-sinks.md §1.1/§1.4/§1.5 (CONTEXT decisions 1, 4, 5,
GIT-01), building on Plan 36-01's executor classification (MutateReversible,
`message` content-sensitive).

## What Was Built

### Task 1 — run_launcher extra_env + git_commit.rs sink (commit dffc8ee)
- `run_launcher` gained a trailing `extra_env: &[(&str, &str)]` param and is now
  `pub(crate)`; the vars are applied AFTER `env_clear()` + `PATH` + `EXEC_*`
  metadata. Both existing `process.exec` call sites (`invoke_process_exec`,
  `invoke_process_exec_from_resolved`) pass `&[]`, so process.exec's child env is
  byte-identical to before — its Linux-gated tests are unchanged.
  `resolve_launcher_path`/`SAFE_EXEC_PATH` made `pub(crate)` for reuse.
- New `crates/brokerd/src/sinks/git_commit.rs` `invoke_git_commit` (mirrors
  `invoke_process_exec`'s shape/signature): resolves the `message` arg (fail-
  closed if absent), builds the broker-constructed TRUSTED argv
  `["-c","core.hooksPath=/dev/null","-c","user.name=caprun","-c","user.email=caprun@localhost","commit","-m",<message>]`,
  sets cwd = `workspace_root.root_path()`, passes
  `extra_env=[GIT_CONFIG_NOSYSTEM=1, GIT_CONFIG_GLOBAL=/dev/null, GIT_TERMINAL_PROMPT=0]`,
  spawns via `run_launcher` (bare-name `git` resolved by the launcher's execve
  through `SAFE_EXEC_PATH`), and records the SAME two-phase durable audit
  (`process_exited` tainted `[ExternalUntrusted, ExecRaw]` on success;
  `process_spawn_failed` untainted FIRST then propagate on failure, no retry,
  actor `sink:git.commit:<effect_id>`). NEVER mints (module Gate-3 doc note).
- Registered `pub mod git_commit;` in `sinks.rs`. `caprun-exec-launcher`
  untouched — the git env flows through `run_launcher`'s `cmd.env` → launcher
  inheritance → `git execve`.

### Task 2 — server.rs Allowed-dispatch arm + mint (commit a53be4a)
- Added a `git.commit` arm to `evaluate_plan_node_and_record` immediately after
  the `process.exec` arm, mirroring it verbatim: guard
  `matches!(decision, Allowed) && plan_node.sink.0 == "git.commit"`, call
  `invoke_git_commit`, advance `*last_event_id`/`*last_event_hash` to the
  returned `process_exited` event, then `mint_from_exec(value_store, session_id,
  combined_output, sink_event_id)` and set `output_value_id`. Because git IS an
  exec under Pattern B, its output correctly reuses `mint_from_exec` — no new
  mint site, no new `TaintLabel`. The mint call stays in `server.rs` (Gate-3
  sanctioned locus); `git_commit.rs` never mints.

### Task 3 — Linux-gated spawn tests (commit 1eceb89)
- New `crates/brokerd/tests/git_commit_spawn.rs`, `#[cfg(target_os = "linux")]`,
  mirroring `process_exec_spawn.rs` (TEST_KEY, temp workspace, seed-root event,
  `find_event_by_type`, `verify_chain`). Each test `git init`s a temp repo and
  stages a file, then invokes the git.commit path:
  1. **genuine-commit + unbroken-audit-DAG-edge**: an Allowed git.commit with a
     UserTrusted message produces a REAL commit (`git rev-parse --verify HEAD`
     resolves, subject matches), appends a chained `process_exited` event, and —
     minted via the SAME `mint_from_exec` server.rs uses — the ValueRecord's
     `provenance_chain[0]` EQUALS that event id (anti-staple), taint pair intact,
     `verify_chain` intact.
  2. **neutralization / planted-hook negative (P2 RCE)**: a planted executable
     `.git/hooks/pre-commit` (pure `/bin/sh` redirection sentinel) + a repo-local
     `alias.evil` — assert NEITHER sentinel is created (hook inert via
     `-c core.hooksPath=/dev/null`; alias never invoked because the broker builds
     the exact argv) AND the commit still succeeds.
  3. **exec-child env-clear**: broker `GIT_AUTHOR_NAME`/`GIT_COMMITTER_NAME`
     sentinels (which OVERRIDE `-c user.name` iff inherited) must NOT be the
     commit author/committer — proving the git child inherited none of the broker
     env. A genuine git-specific proof, not a weak output-absence check.

## Verification Results

- `cargo build --workspace` — clean (brokerd, caprun compile).
- `cargo build --tests --workspace` — clean; only 6 PRE-EXISTING dead-code
  warnings in `cli/caprun/tests/../src/planner.rs` (out of scope, not touched).
- `cargo build -p brokerd` — clean.
- `cargo test -p brokerd` (host-portable) — **all green**: 4/4 lib +
  proto_claims 14, phase5_dispatch 6, s9_acceptance 5, harden01 3,
  planner_capability_split 2, planner_reduced_signal 1, etc.; **no regressions**.
- `cargo test -p brokerd process_exec` — process.exec behavior unchanged by the
  `run_launcher` extension (its spawn tests are Linux-gated → 0 on macOS).
- `git_commit_spawn.rs` + `process_exec_spawn.rs` — **0 tests on macOS**,
  EXPECTED per cfg-linux-test-blindness (the `#[cfg(target_os="linux")]` module
  is not compiled on Mac), never a pass.
- `./scripts/check-invariants.sh` — **all 4 gates PASS, exit 0** (no
  EffectRequest token; `git_commit.rs` never mints — Gate 3 green; the only
  git.commit `mint_from_exec` call is the sanctioned `server.rs` locus; tests/
  are Gate-3 exempt).

## Scoped Decision (flag for the verifier)

Per the plan's SCOPED DECISION and DESIGN §9: Phase 36 wires the git.commit
**Allowed path + I2 Block-detection ONLY**. There is deliberately NO
confirm-release path — no `invoke_git_commit_from_resolved`, no
`prepare_git_commit`, and **git.commit is NOT added to the Step 4.75
confirm-release allow-list**. Rationale: git.commit is a LOCAL, reversible
(MutateReversible) op; the P33/P34 confirm-release audit-gap discipline is scoped
to `git.push`/`github.pr` (CommitIrreversible) only. A tainted `message`
therefore Blocks deterministically, and any confirm attempt fails
closed-recoverable (row stays Pending, NO `confirm_granted` appended, NO audit
gap) because git.commit is absent from the Step 4.75 allow-list. This is the
correct security posture for this phase, not a gap.

## Linux Verification Deferred to Phase 40

The three `git_commit_spawn.rs` tests genuinely spawn the confined launcher + the
system `git`, so they are `#[cfg(target_os="linux")]`-gated and show **0 tests on
this macOS dev host — expected, not a gap** (cfg-linux-test-blindness). Per the
execution brief and CLAUDE.md, the LIVE Linux run
(`scripts/mailpit-verify.sh` with
`MAILPIT_VERIFY_CMD='cargo test -p brokerd --test git_commit_spawn'`) is owned by
Phase 40. **CAVEAT (cfg-linux-test-blindness):** because macOS does not compile
the `#[cfg(target_os="linux")]` module, the `mod linux` body was NOT compile-
checked here — Phase 40 must run `cargo build --tests` inside the Linux container
FIRST (per the standing lesson) to catch any Linux-only compile error before
asserting the tests pass. All type/signature usages were manually reconciled
against the real APIs (`invoke_git_commit`, `mint_from_exec`, `WorkspaceRoot`,
audit helpers, `ValueRecord` pub fields).

## Deviations from Plan

None affecting behavior. Two implementation refinements (both strengthen the
tests, within Task 3's stated intent):
- **[Rule 1 - test robustness]** The planted-hook sentinel uses pure `/bin/sh`
  redirection (`: > HOOK_FIRED_SENTINEL`) inside the workspace root instead of a
  `touch`/`git`-dependent script, so its absence proves the hook never fired
  (not that a subordinate binary was merely unreachable under confinement) —
  closes a false-pass path (false-assurance-regression-test lesson).
- **[Rule 1 - test strength]** The env-clear test asserts the commit
  author/committer is the trusted `caprun` identity (broker
  `GIT_AUTHOR_NAME`/`GIT_COMMITTER_NAME` would override `-c user.name` if the
  child inherited them) — a genuine git-specific leak proof, stronger than a
  generic output-absence check.

## Known Stubs

None. `invoke_git_commit` is fully wired to a real spawn + audit; the server arm
mints real output; no placeholder/empty-value paths.

## Threat Surface

No new security surface beyond the plan's threat register. T-36-04 (planted
hook/alias RCE) mitigated by `-c core.hooksPath=/dev/null` + `GIT_CONFIG_NOSYSTEM`
+ `GIT_CONFIG_GLOBAL=/dev/null` + broker-constructed argv (planted-hook negative
test). T-36-05 (broker env leak) mitigated by `run_launcher`'s `env_clear()`
(env-clear test). T-36-06 (mint staple) mitigated by rooting on the real
`process_exited` event via the existing `mint_from_exec` (anti-staple test).
T-36-07 (net egress) accept — seccomp net-deny UNCHANGED, sandbox untouched.
No new dependencies (T-36-SC accept holds — reuses shipped launcher + system git).

## Self-Check: PASSED

- crates/brokerd/src/sinks/git_commit.rs — FOUND (created).
- crates/brokerd/tests/git_commit_spawn.rs — FOUND (created).
- crates/brokerd/src/sinks/process_exec.rs — FOUND (modified: run_launcher extra_env).
- crates/brokerd/src/sinks.rs — FOUND (modified: git_commit registered).
- crates/brokerd/src/server.rs — FOUND (modified: git.commit dispatch arm).
- Commit dffc8ee — FOUND (Task 1).
- Commit a53be4a — FOUND (Task 2).
- Commit 1eceb89 — FOUND (Task 3).
