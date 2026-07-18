---
created: 2026-07-18T00:00:00.000Z
title: env_clear() the caprun-planner sidecar spawn (defense-in-depth, v1.8)
area: security
resolves_phase: null
files:
  - cli/caprun/src/main.rs:323
---

## Problem

Source: fresh Fable-5 adversarial trace of the Phase-34 worker `env_clear()` fix
(commit `b0a7f49`) — verdict APPROVED, this is the sole MINOR follow-up.

The LLM planner sidecar `Command` (`cli/caprun/src/main.rs:~320-328`, spawned only
when `CAPRUN_PLANNER=llm`) is NOT `env_clear()`'d, so it inherits the unconfined
`caprun` parent's full environment — including `CAPRUN_SMTP_*`, which the sidecar
does not need. It legitimately receives `OPENAI_API_KEY` (set explicitly) to make
its outbound OpenAI call.

## Why this is LOWER risk than the exec-child / worker leaks (already fixed in P34)

- The sidecar is **trusted code** — it runs NO untrusted instructions and processes
  no untrusted content (unlike the worker, which handles tainted bytes by design).
- It holds a secret (`OPENAI_API_KEY`) by design already.
- So the marginal exposure is only the *ambient* `CAPRUN_SMTP_*` it never uses.

## Why it was NOT fixed in Phase 34 (deliberate deferral)

The sidecar is **unconfined and makes an outbound HTTPS/TLS call** (reqwest +
rustls). `env_clear()`'ing it risks breaking that call via env the TLS/HTTP stack
may read (e.g. `HTTPS_PROXY`/`NO_PROXY`, `SSL_CERT_FILE`/`SSL_CERT_DIR`, locale) —
a regression that only a live-OpenAI run (usually skipped when `OPENAI_API_KEY` is
unset) would catch. Taking that risk for the lowest-severity item at milestone
close was not worth it.

## Fix (v1.8)

`cmd.env_clear()` then set only what the sidecar needs: `OPENAI_API_KEY`,
`PLANNER_SOCK`, `CAPRUN_PLANNER_MODEL` (already set explicitly), plus a minimal
`PATH` and whatever the TLS/HTTP stack genuinely requires — determined by a live
`CAPRUN_PLANNER=llm` run with `OPENAI_API_KEY` set (do NOT ship without that live
verification, given the TLS-env risk above). Confirm the LLM planner path
(`live_acceptance_v1_4_composed`, `llm_planner_live_accept`) still delivers.

## Related NIT (same review, optional)

- `cli/caprun/src/main.rs` worker spawn allowlists `SESSION_ID`, but `worker.rs`
  never reads it (HARD-03 removed the field). Dead non-secret entry — drop it or
  leave it; harmless.
