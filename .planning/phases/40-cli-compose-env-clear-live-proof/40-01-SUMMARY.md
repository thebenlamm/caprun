---
phase: 40-cli-compose-env-clear-live-proof
plan: 01
subsystem: cli
tags: [security, env-clear, sidecar, ENV-01, defense-in-depth]
status: complete
requires:
  - Phase-37 MAJOR-1 fix (rustls-no-provider + ring + compiled-in webpki-roots) — makes env_clear hermetic for TLS
  - Phase-34 worker env_clear() (the discipline this mirrors)
provides:
  - env_clear()'d caprun-planner sidecar spawn with a unit-tested minimal allowlist (ENV-01)
  - planner_sidecar_allowlist_env() pure helper (cli/caprun/src/main.rs)
affects:
  - Plan 40-04 (composed live proof — the LIVE HTTPS/OpenAI run confirms hermetic env_clear)
tech-stack:
  added: []
  patterns:
    - "static Mutex<()> env-lock + save/restore EnvGuard for process-global env tests (mirrors GITHUB_ENV_LOCK / SMTP_ENV_LOCK)"
key-files:
  created: []
  modified:
    - cli/caprun/src/main.rs
decisions:
  - "Sidecar env_clear() FIRST, then OPENAI_API_KEY (sole secret, set separately), then envs(allowlist) — secret never enumerated by the non-secret builder"
  - "Optional passthroughs (CAPRUN_PLANNER_MODEL / HTTPS_PROXY / NO_PROXY) forwarded only when present AND non-empty — never an empty string"
  - "SESSION_ID dead worker-spawn entry removed (grep-confirmed zero readers in all rust source)"
metrics:
  duration: ~15m
  completed: 2026-07-18
  tasks: 2
  files: 1
  commits: 4
---

# Phase 40 Plan 01: env_clear() the caprun-planner sidecar Summary

`env_clear()`'d the caprun-planner sidecar `Command` and gave it an explicit, unit-tested minimal allowlist (`PLANNER_SOCK` + minimal `PATH` always; optional `CAPRUN_PLANNER_MODEL` / `HTTPS_PROXY` / `NO_PROXY`; `OPENAI_API_KEY` set separately as the sole secret) so no ambient broker env (notably `CAPRUN_SMTP_*`) survives into the sidecar — closing the ENV-01 defense-in-depth gap the Phase-34 fresh adversarial trace flagged. Hermetic for TLS because egress uses compiled-in webpki-roots. Also dropped the dead `SESSION_ID` worker-spawn entry.

## What was built

**Task 1 — sidecar env_clear() + allowlist builder (ENV-01):**
- New pure helper `planner_sidecar_allowlist_env(planner_sock: &str) -> Vec<(String, String)>` returns the NON-SECRET surviving env: `PLANNER_SOCK` + minimal `PATH` always, plus `CAPRUN_PLANNER_MODEL` / `HTTPS_PROXY` / `NO_PROXY` each only when present and non-empty. Reads the parent env by explicit key name — never enumerates the full env.
- Spawn site rewired: `cmd.env_clear()` FIRST → `OPENAI_API_KEY` set conditionally (the ONLY secret, forwarded exactly as before) → `cmd.envs(planner_sidecar_allowlist_env(&planner_sock))`.
- Code comments cite ENV-01 / 40-CONTEXT decision 1 / DESIGN §5.2 and explain WHY it is now hermetic (webpki-roots → no `SSL_CERT_*` / system cert store).
- 5 unit tests over the helper using a `static Mutex<()>` env-lock + save/restore `EnvGuard` (mirrors the workspace's `GITHUB_ENV_LOCK` / `SMTP_ENV_LOCK` convention).

**Task 2 — drop dead worker-spawn entry (T-40-02):**
- `grep -rn "SESSION_ID" --include="*.rs" cli crates` → only the writer + one stale comment; zero `std::env::var` readers (worker.rs never read it; HARD-03 removed the field).
- Removed the `.env("SESSION_ID", ...)` line and the comment falsely listing it among vars the worker reads. Worker spawn otherwise untouched (still `env_clear()`'d, worker not weakened).

## Verification

- `./scripts/check-invariants.sh` → exit 0 (no raw `EffectRequest`; boundaries intact).
- `cargo build --workspace` → clean.
- `cargo test -p caprun` → **43 passed, 0 failed** across all test binaries (includes the 5 new `planner_sidecar_allowlist_*` unit tests + all pre-existing e2e/confirm/harden04/live-acceptance tests — no regression).
- Targeted: `cargo test -p caprun planner_sidecar_allowlist` → 5/5 pass.

The full hermetic-under-`env_clear()` proof (a LIVE OpenAI sidecar run + broker-side live HTTPS GET) is validated in Plan 40-04's composed Linux run; this plan lands the code that run confirms.

## Deviations from Plan

None beyond one in-scope correctness touch: the plan named `cli/caprun-worker/` as the worker grep path, but the worker is actually the `caprun-worker` bin at `cli/caprun/src/worker.rs`. Grepped all rust source (`cli crates`) instead — strictly broader, confirming zero readers. Not a behavior change.

Also removed a stale code comment (main.rs:405) that listed `SESSION_ID` among vars "the worker actually reads" — it never did; leaving it would have contradicted the removed entry (Rule 1, doc-consistency).

## Self-Check: PASSED
- `cli/caprun/src/main.rs` — FOUND (modified; helper + env_clear + test module present).
- Commits FOUND: `6e36634` (test/RED), `de0f652` (feat/GREEN), `574326a` (refactor/Task 2).

## TDD Gate Compliance
Task 1 (`tdd="true"`): RED commit `6e36634` (test referencing the not-yet-existing helper → crate failed to compile, confirmed via `cargo build -p caprun --tests`), then GREEN commit `de0f652` (helper + spawn wiring → 5/5 pass). No REFACTOR commit needed. Gate sequence satisfied.
