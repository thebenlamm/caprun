---
created: 2026-07-17T23:50:00.000Z
title: env_clear() the process.exec launcher spawn before live exec (Phase 34)
area: security
resolves_phase: 34
files:
  - crates/brokerd/src/sinks/process_exec.rs:215
---

## Problem

Source: fresh Fable-5 adversarial TCB trace of the Phase 32 `process.exec` diff
(verdict APPROVE-WITH-FIXES — no BLOCKER/MAJOR; this is the sole MINOR).

`run_launcher` (`crates/brokerd/src/sinks/process_exec.rs:~215-238`) builds the
`TokioCommand` for the confined child **without `env_clear()`**. The broker runs
as a tokio task inside the unconfined `caprun` process, which holds
`OPENAI_API_KEY` and `CAPRUN_SMTP_*` in its environment. So the exec child
inherits them.

A `command="env"` / `printenv` — legitimately authorable as a **UserTrusted**
command — would capture `OPENAI_API_KEY` into `combined_output`, which is then
minted (`mint_from_exec`) and stored, and shown verbatim in the confirmation UX
if a human ever confirms.

## Why this is NOT a Phase 32 blocker

- Network exfiltration is blocked: the exec child's seccomp filter denies
  `socket(AF_INET/AF_INET6)`, and the confinement persists across `execve`.
- The command must be **UserTrusted** to run at all.
- So this is **defense-in-depth debt, not an open exfil path**. Phase 32's four
  success criteria (EXEC-01..04) are met and independently proven on real Linux.

## Fix (do before LIVE-01/02 in Phase 34)

In `run_launcher`, call `cmd.env_clear()` and then set only the `EXEC_*` vars the
launcher actually needs (`EXEC_CWD`, `EXEC_ARGS_JSON`, workspace root, etc.).
Add a Linux-gated test asserting the exec child's environment contains none of
the broker's secrets (e.g. run `command="env"` and assert `OPENAI_API_KEY`
absent from captured output).

## Related NITs from the same review (optional, non-blocking)

- Post-Allowed `args_json` decode error (`process_exec.rs:133`) returns `Err`
  via `?` before `run_launcher`, so it appends no `process_spawn_failed` durable
  event. Broker-internal (args are broker-minted UserTrusted), audit-completeness
  only.
- `_args` is fully decoded in the broker at `process_exec.rs:133` purely for
  "parity" and never used — the launcher re-decodes `EXEC_ARGS_JSON` itself. Dead
  work; harmless.
- Landlock grants read+`Execute` on all of `/usr`,`/bin`,`/lib`,`/lib64` — broad
  but necessary and already flagged as an in-container tightening item in
  DESIGN §8. Acceptable for v0.

## RESOLVED (2026-07-18, Phase 34 gap-closure)

Fixed in commit `3d18a9d`: `run_launcher` now `env_clear()`s + a minimal
`SAFE_EXEC_PATH` + only the `EXEC_*` vars — fixing BOTH the Allowed and
confirm-release exec paths in the one shared helper. Linux-gated regression test
`run_launcher_env_clear_prevents_broker_secret_leak` asserts the broker's secrets
are absent from a confined `/usr/bin/env` child's output. Fresh non-self Fable-5
trace APPROVED; full Linux regression green (391/391, true-exit-0). Adjacent
worker-spawn leak also fixed (`b0a7f49`, APPROVED, e2e-verified); planner-sidecar
variant deferred to `2026-07-18-planner-sidecar-env-clear.md` (v1.8).
