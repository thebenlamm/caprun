---
phase: 21-adversarial-llm-planner
plan: 02
subsystem: planner
tags: [rust, reqwest, tokio, openai, uds, sidecar, llm-planner]

# Dependency graph
requires:
  - phase: 21-adversarial-llm-planner
    plan: 01
    provides: "The literal-free `llm-planner` wire contract (PlannerRequest/HandleLabel/PlannerResponse/ResponseArg), `build_planner_prompt`, `build_tool_schema`, and the fail-closed `parse_planner_response` validator this plan's openai.rs and main.rs build on."
provides:
  - "New bin crate `cli/caprun-planner`: the out-of-process LLM sidecar and the ONLY place in the workspace that depends on reqwest and reaches the network."
  - "`openai::build_chat_request` / `openai::extract_tool_arguments` / `openai::call_openai` — the real OpenAI tool-calling client, forcing emit_plan_node and validating fail-closed via `llm_planner::parse_planner_response`."
  - "`main.rs`'s abstract-UDS accept loop (`\\0` + PLANNER_SOCK) speaking the workspace's standard 4-byte-LE-prefix + JSON framing, single request per connection, no fs/broker/audit capability."
affects: ["21-03-worker-proxy", "21-04-composed-live-acceptance"]

# Tech tracking
tech-stack:
  added: ["reqwest 0.13.4 (default-features=false, features=[rustls, json]) — isolated to cli/caprun-planner only"]
  patterns:
    - "Out-of-process network sidecar pattern: reqwest/HTTP capability lives ONLY in a separate bin crate with no fs/broker/audit access, communicating over the same abstract-UDS + 4-byte-LE-length-prefix + JSON framing every other IPC transport in this workspace uses."
    - "Tagged-enum wire reply (`{\"status\":\"ok\",...}` / `{\"status\":\"error\",...}`) as the fail-closed signal a downstream proxy maps to \"no usable plan node\"."

key-files:
  created:
    - cli/caprun-planner/Cargo.toml
    - cli/caprun-planner/src/lib.rs
    - cli/caprun-planner/src/openai.rs
    - cli/caprun-planner/src/main.rs
  modified:
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "reqwest feature name is `rustls` (not `rustls-tls`, which the plan's prose used) — the 0.11/0.12-era feature name was renamed by the time 0.13.4 resolved from crates.io; verified via `cargo add --dry-run` before pinning. `default-features = false` + `rustls`/`json` only — confirmed via the same dry-run that no native-tls/openssl-sys feature is pulled in."
  - "Added `src/lib.rs` (`pub mod openai;`) alongside `main.rs` so `openai.rs`'s pure helpers can be unit-tested with `cargo test -p caprun-planner --lib` exactly as Task 1's own `<verify>` step specifies — not explicitly listed in the plan's artifact list but required for that exact verification command to have a lib target to run against."
  - "The sidecar's fail-closed wire reply is a locally-defined tagged enum (`SidecarReply::Ok { response } | Error { message }`, JSON `{\"status\":\"ok\"/\"error\",...}`) living only in main.rs — NOT added to the shared `llm-planner` crate, since Plan 21-01 is already closed and this plan's file list doesn't touch it. Plan 21-03's worker-side proxy will need to mirror this exact shape to interoperate; documented here and in main.rs's doc comment for that coordination."

requirements-completed: [PLANNER-03]

coverage:
  - id: D1
    description: "reqwest supply-chain legitimacy gate (Task 0) — pre-cleared by the orchestrator with crates.io API evidence before this plan dispatched"
    verification: []
    human_judgment: true
    rationale: "Package-legitimacy checkpoints (gate=\"blocking-human\") are never auto-approvable; a human-directed orchestrator session cleared it independently with real crates.io API evidence prior to this dispatch (see Deviations/Checkpoint section)."
  - id: D2
    description: "openai::build_chat_request forces the emit_plan_node tool call and embeds a tool schema whose value_id enum equals exactly the offered handle IDs"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "cli/caprun-planner/src/openai.rs#build_chat_request_forces_emit_plan_node_and_embeds_handle_enum"
        status: pass
    human_judgment: false
  - id: D3
    description: "openai::extract_tool_arguments returns Ok on a tool-call response and Err on a content-only (free-text) response — never falls back to message.content"
    requirement: "PLANNER-03"
    verification:
      - kind: unit
        ref: "cli/caprun-planner/src/openai.rs#extract_tool_arguments_ok_on_tool_call_response"
        status: pass
      - kind: unit
        ref: "cli/caprun-planner/src/openai.rs#extract_tool_arguments_err_on_content_only_response"
        status: pass
    human_judgment: false
  - id: D4
    description: "reqwest is isolated to cli/caprun-planner only — zero occurrences in the caprun package's dependency tree (which covers both the caprun and caprun-worker bin targets)"
    verification:
      - kind: other
        ref: "cargo tree -p caprun | grep -c reqwest -> 0"
        status: pass
    human_judgment: false
  - id: D5
    description: "caprun-planner compiles on macOS and produces a binary; sidecar fails fast on missing PLANNER_SOCK/OPENAI_API_KEY"
    requirement: "PLANNER-03"
    verification:
      - kind: other
        ref: "cargo build -p caprun-planner && test -x target/debug/caprun-planner"
        status: pass
      - kind: manual_procedural
        ref: "PLANNER_SOCK unset -> caprun-planner exits with 'PLANNER_SOCK env var is required' (manually observed)"
        status: pass
    human_judgment: false
  - id: D6
    description: "openai::call_openai makes a genuine live OpenAI tool-calling HTTP call, offered handle IDs are copied verbatim by the model, and parse_planner_response accepts the result"
    requirement: "PLANNER-03"
    verification:
      - kind: e2e
        ref: "manual live run via a transient (unstaged, deleted before commit) examples/live_smoke.rs against https://api.openai.com/v1/chat/completions, model gpt-4o-mini-2024-07-18 — HTTP 200, forced emit_plan_node tool call, all 3 offered handle IDs referenced verbatim, call_openai returned Ok(PlannerResponse). Cost: prompt_tokens=392, completion_tokens=102, total=494 (~$0.00012 at published gpt-4o-mini per-token rates, matching the plan's 'order 1e-4 dollars' estimate)."
        status: pass
    human_judgment: false
  - id: D7
    description: "Full workspace still builds and ./scripts/check-invariants.sh still passes after adding the sidecar crate"
    verification:
      - kind: other
        ref: "cargo build --workspace"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh"
        status: pass
    human_judgment: false

duration: ~20min
completed: 2026-07-11
status: complete
---

# Phase 21 Plan 02: caprun-planner LLM Sidecar Summary

**New `cli/caprun-planner` bin crate — the sole reqwest-holding process in the workspace — makes a genuine, live OpenAI tool-calling request (verified against api.openai.com: HTTP 200, forced `emit_plan_node` call, all handle IDs copied verbatim, ~$0.00012/request) and serves it over an abstract-UDS accept loop using the workspace's standard framing, with zero filesystem/broker/audit capability.**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-11
- **Tasks:** 2 completed (Task 0 checkpoint pre-cleared by orchestrator; no code produced by it)
- **Files modified:** 4 created (`cli/caprun-planner/Cargo.toml`, `src/lib.rs`, `src/openai.rs`, `src/main.rs`), 2 modified (`Cargo.toml`, `Cargo.lock`)

## Accomplishments
- Created the `cli/caprun-planner` bin crate and registered it in the root `[workspace] members` list — the ONLY crate in the workspace depending on `reqwest` (confirmed: `cargo tree -p caprun` shows zero `reqwest` occurrences, covering both the `caprun` and `caprun-worker` bin targets).
- `openai::build_chat_request` builds the OpenAI chat-completions body from a `PlannerRequest` alone: a system+user message pair (user content = `llm_planner::build_planner_prompt(req)`), one declared tool (`emit_plan_node`, `parameters` = `llm_planner::build_tool_schema(req)`), and `tool_choice` forcing that function — so the model cannot answer with free-text `message.content`.
- `openai::extract_tool_arguments` reads `choices[0].message.tool_calls[0].function.arguments` only, erroring on any other shape (including a content-only response) — never a free-text fallback.
- `openai::call_openai` makes the one async HTTP call, then hands the extracted arguments to `llm_planner::parse_planner_response` with this request's own offered handles/known sinks for fail-closed validation — every failure mode (transport error, non-2xx, missing tool call, validation rejection) is `Err`.
- `main.rs`'s accept loop reads `PLANNER_SOCK` / `OPENAI_API_KEY` / `CAPRUN_PLANNER_MODEL` (default `gpt-4o-mini`) from the environment, fails fast if `PLANNER_SOCK` or `OPENAI_API_KEY` is unset, binds `\0` + `PLANNER_SOCK` via tokio's native abstract-path support (the broker's verified pattern), and services one framed `PlannerRequest` -> `openai::call_openai` -> one framed `SidecarReply` per connection — no retry framework, no fs/broker/audit capability handed to the process.
- **Live-verified the real OpenAI call end to end** (manual, transient example — see Deviations): HTTP 200 from `api.openai.com/v1/chat/completions`, model `gpt-4o-mini-2024-07-18` returned a forced `emit_plan_node` tool call referencing all 3 offered handle IDs verbatim, and `call_openai` returned `Ok(PlannerResponse)`. Token usage: `prompt_tokens=392, completion_tokens=102, total_tokens=494` — at published `gpt-4o-mini` rates (~$0.15/1M input, ~$0.60/1M output) that's roughly **$0.00012 per request**, matching the plan's "order 1e-4 dollars" estimate.

## Task Commits

1. **Task 1: OpenAI tool-calling client (request builder, HTTP call, extractor)** - `554b13b` (feat)
2. **Task 2: Abstract-UDS sidecar accept loop** - `72d58f0` (feat)

Task 0 (the `checkpoint:human-verify gate="blocking-human"` reqwest supply-chain gate) produced no code — see Deviations below for how it was cleared.

**Plan metadata:** this SUMMARY's own commit (see below).

## Files Created/Modified
- `cli/caprun-planner/Cargo.toml` — new bin+lib crate: `llm-planner`/`runtime-core` (path), `tokio`/`serde`/`serde_json`/`anyhow` (workspace), `reqwest = "=0.13.4"` pinned, `default-features = false`, `features = ["rustls", "json"]`.
- `cli/caprun-planner/src/lib.rs` — `pub mod openai;`, exists so the pure helpers in `openai.rs` are unit-testable via `cargo test -p caprun-planner --lib`.
- `cli/caprun-planner/src/openai.rs` — `build_chat_request`, `extract_tool_arguments`, `call_openai`, 3 unit tests (static JSON only, no live call).
- `cli/caprun-planner/src/main.rs` — the abstract-UDS accept loop, `SidecarReply` wire type, framed read/write helpers matching the workspace's standard 4-byte-LE-prefix + JSON framing.
- `Cargo.toml` (root) — added `"cli/caprun-planner"` to `[workspace] members` (surgical edit; no other members touched).
- `Cargo.lock` — updated by `cargo build` to include the new crate + its dependency tree (reqwest, hyper-rustls, rustls, etc.).

## Decisions Made
- reqwest's feature is named `rustls` in 0.13.4, not `rustls-tls` as the plan's prose said — confirmed via `cargo add --dry-run` before pinning; `default-features = false` + `["rustls", "json"]` pulls in zero native-tls/openssl-sys surface (dry-run showed only `__rustls`/`__rustls-aws-lc-rs`/`__tls`/`json`/`rustls` enabled).
- Added `src/lib.rs` so the plan's own Task 1 `<verify>` command (`cargo test -p caprun-planner --lib`) has a lib target to run against; `main.rs`'s bin target implicitly links this lib via the package's own `caprun_planner::openai` path (standard Cargo behavior when both `src/lib.rs` and `src/main.rs` exist).
- The sidecar's error-signal wire type (`SidecarReply`) is defined locally in `main.rs`, not added to the shared `llm-planner` crate — Plan 21-01 is closed and this plan's declared file list doesn't touch it. Its exact JSON shape (`{"status":"ok","response":{...}}` / `{"status":"error","message":"..."}`) is documented in `main.rs`'s doc comment for Plan 21-03's worker-side proxy to mirror.

## Deviations from Plan

### Checkpoint: pre-cleared, not re-verified interactively

**Task 0** (`checkpoint:human-verify gate="blocking-human"`, `autonomous: false`) required a human to confirm `reqwest`'s legitimacy on crates.io before any dependency was added. Per this dispatch's explicit instructions, the orchestrator (a human-directed session) had **already cleared this checkpoint independently, before dispatching this plan**, with real evidence: `curl https://crates.io/api/v1/crates/reqwest` returned 573,024,238 total downloads, 141,264,970 recent downloads, not yanked, latest version 0.13.4, source repository `github.com/seanmonstar/reqwest` (seanmonstar also maintains `hyper`). This executor treated the checkpoint as approved and proceeded directly to Task 1 — it never re-prompted for interactive confirmation (this agent runs backgrounded with no interactive channel; doing so would have hung indefinitely). The exact pinned version (`0.13.4`) and feature set (`default-features = false`, `rustls` + `json`) match what the checkpoint's `<how-to-verify>` steps required, modulo the `rustls-tls` -> `rustls` feature-name correction below.

### Auto-fixed Issues

**1. [Rule 3 - Blocking compile fix] `serde` missing from `cli/caprun-planner/Cargo.toml`**
- **Found during:** Task 2 (`cargo build -p caprun-planner` after adding `main.rs`'s `SidecarReply` type)
- **Issue:** `main.rs` needed `#[derive(serde::Serialize)]` on `SidecarReply`, but the plan's Task 1 dependency list only specified `serde_json`, not `serde` — `serde` was never a direct dependency, so the derive macro path failed to resolve (`E0433: unresolved crate serde`).
- **Fix:** Added `serde = { workspace = true }` to `cli/caprun-planner/Cargo.toml` (the workspace-level `serde` already has the `derive` feature enabled, so no feature-list changes were needed).
- **Files modified:** `cli/caprun-planner/Cargo.toml`
- **Commit:** `72d58f0`

**2. [Rule 2 — no code change, verification-methodology addition] reqwest feature name mismatch**
- **Found during:** Task 1, before adding the dependency
- **Issue:** The plan's `<action>` text specifies "features rustls-tls + json", but `reqwest` 0.13.4 (the version that actually resolves from crates.io today) renamed that feature to `rustls` — `cargo add --dry-run` confirmed `rustls-tls` is an unrecognized feature name for this version, listed under "disabled features" with no such entry, while `rustls` is accepted and correctly excludes every native-tls/openssl-sys feature.
- **Fix:** Used `rustls` (not `rustls-tls`) in the `features` list; verified via `cargo add --dry-run` before committing to the pin, and again after the real dependency was added that `cargo tree -p caprun-planner` shows only the rustls-based TLS stack (`rustls`, `rustls-webpki`, `hyper-rustls`, `tokio-rustls`) — no `native-tls`/`openssl-sys`.
- **Files modified:** `cli/caprun-planner/Cargo.toml`
- **Commit:** `554b13b`

## Live-Call Verification (Cost Awareness)

Per the plan's "Cost awareness" note, the live-call check that the plan itself deferred to Plan 04 was pulled forward and run manually during this plan's execution (per this dispatch's explicit success criteria), using a **transient, unstaged, never-committed** `cli/caprun-planner/examples/live_smoke.rs` that called `openai::build_chat_request` + a raw `reqwest` POST (to capture `usage`) and then the real `openai::call_openai` path. The example was deleted immediately after the run; `git status --short` was confirmed clean before any commit. Result:

- HTTP status: `200 OK`
- Model: `gpt-4o-mini-2024-07-18`
- `usage`: `prompt_tokens=392, completion_tokens=102, total_tokens=494`
- The model's forced `emit_plan_node` tool call referenced all 3 offered handle IDs verbatim (no fabricated handle, no literal), sink `email.send`
- `call_openai` returned `Ok(PlannerResponse { sink: "email.send", args: [...3 args...] })`
- Estimated cost at published `gpt-4o-mini` per-token pricing (~$0.15/1M input, ~$0.60/1M output): **~$0.00012/request** — consistent with the plan's "order 1e-4 dollars" estimate.

This confirms end-to-end: a real, separate-process OpenAI tool-calling client exists, forces the schema-constrained tool call, and validates fail-closed via `parse_planner_response` — the substance of PLANNER-03/T-21-04/T-21-05. The abstract-UDS transport layer itself (`main.rs`'s accept loop) could NOT be live-tested on macOS — `tokio::net::UnixListener::bind("\0...")` errors at runtime on macOS ("paths must not contain interior null bytes"; empirically confirmed) because abstract-namespace sockets are a Linux-only kernel feature, exactly as `cli/caprun/src/worker.rs`'s own doc comment notes for its abstract-socket `connect`. That UDS-transport verification is Linux-only and belongs to the project's standard Linux verification recipe (`scripts/mailpit-verify.sh` / Colima+Docker), consistent with `CLAUDE.md`'s existing guidance — it is out of scope for this plan's own macOS-only build verification and is the natural target of Plan 21-03/21-04's composed acceptance test.

## Issues Encountered

None beyond the two auto-fixed deviations above. `cargo build --workspace` and `./scripts/check-invariants.sh` both passed after Task 2's implementation.

## User Setup Required

None — `OPENAI_API_KEY` was already present in the environment (confirmed at dispatch: `OPENAI_API_KEY is set: yes`), matching the plan's `user_setup` note.

## Next Phase Readiness

`cli/caprun-planner` is ready for Plan 21-03 (the worker-side `LlmPlanner` proxy implementing the `Planner` trait from Phase 20, which will connect to this sidecar's `PLANNER_SOCK` and speak the exact framing/`SidecarReply` shape documented in `main.rs`). No blockers. The UDS-transport half of this plan (the accept loop itself, as opposed to the OpenAI HTTP call it wraps) has not yet been exercised end-to-end because that requires Linux (abstract sockets) and a peer client (Plan 21-03's proxy) — both land in the next plan(s), consistent with the plan's own wave sequencing.

---
*Phase: 21-adversarial-llm-planner*
*Completed: 2026-07-11*

## Self-Check: PASSED

All created files verified present (`cli/caprun-planner/Cargo.toml`, `cli/caprun-planner/src/lib.rs`, `cli/caprun-planner/src/openai.rs`, `cli/caprun-planner/src/main.rs`, this SUMMARY.md); both commit hashes (`554b13b`, `72d58f0`) verified present in git log; `cargo test -p caprun-planner --lib` re-run and confirmed 3/3 passing; `cargo tree -p caprun` re-confirmed zero `reqwest` occurrences; `cargo build --workspace` and `./scripts/check-invariants.sh` re-run and confirmed passing.
