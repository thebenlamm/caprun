
## RESOLVED (2026-07-18, Phase 34 gap-closure)

Fixed in commit `3d18a9d`: `run_launcher` now `env_clear()`s + sets a minimal
`SAFE_EXEC_PATH` + only the `EXEC_*` vars — fixes BOTH the Allowed and
confirm-release exec paths in the one shared helper. Linux-gated regression test
`run_launcher_env_clear_prevents_broker_secret_leak` asserts the broker's secrets
(sentinel + `OPENAI_API_KEY`) are absent from a confined `/usr/bin/env` child's
output. A fresh non-self Fable-5 adversarial trace of the diff returned APPROVED;
full Linux regression green (391/391, true-exit-0). The adjacent worker-spawn leak
the same trace surfaced was also fixed (commit `b0a7f49`); the planner-sidecar
variant is deferred to [[2026-07-18-planner-sidecar-env-clear]] (v1.8, lower risk).
