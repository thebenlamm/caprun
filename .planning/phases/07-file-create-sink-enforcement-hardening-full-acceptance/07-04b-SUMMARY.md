---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 04b
subsystem: brokerd / executor / cli
tags: [SINK-01, HARD-05, file.create, PathRaw, RelativePath, two-phase-audit, intent-routing, I2]

# Dependency graph
requires:
  - phase: 07-04a
    provides: "PathRaw label, file.create path routing-sensitivity, validate_schema arg-gate, WorkspaceRoot::create_exclusive_within"
  - phase: 07-03
    provides: "WorkspaceRoot dirfd capability threaded through dispatch_request (Arc<WorkspaceRoot>)"
  - phase: 07-02
    provides: "broker-minted effect_id + SinkBlockedAnchor + two-phase durable append ordering"
provides:
  - "WorkerClaim::RelativePath(String) — path claim variant on the IPC wire"
  - "quarantine::extract_relative_path_claims (deterministic, lossy path extractor)"
  - "mint_from_read taint derived by claim_type (relative_path -> [ExternalUntrusted, PathRaw]); unknown claim_type fails closed"
  - "sinks::file_create::invoke_file_create — live file.create sink with two-phase durable audit (sink_executed / sink_execution_failed, no retry)"
  - "CaprunIntent::CreateFileFromReport + create-file-from-report CLI kind"
  - "planner + worker intent-kind-driven routing that makes BOTH §9 file.create paths reachable"
affects: [07-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Claim-type-driven taint selection kept inside the sole mint site (mint_from_read); unknown type errors, never default-tags"
    - "Live sink follows the block path's two-phase ordering: authorize (plan_node_evaluated) then effect (sink_executed), both chained"
    - "Planner routes a routing-sensitive arg by handle PROVENANCE (tainted file handle vs UserTrusted intent handle) — it never sees literal or taint"

key-files:
  created:
    - crates/brokerd/src/sinks/file_create.rs
  modified:
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/sinks.rs
    - crates/runtime-core/src/intent.rs
    - cli/caprun/src/planner.rs
    - cli/caprun/src/worker.rs
    - cli/caprun/src/main.rs
    - cli/caprun/tests/planner.rs

key-decisions:
  - "effect_id carried in the sink event's `actor` field (sink:file.create:<effect_id>) — Event has no effect_id column and adding one would break the pre-anchor golden byte-fixture (DESIGN §5, no DB migration)"
  - "invoke_file_create takes value_store + parent_id and returns (Uuid, String) — the plan's illustrative signature omitted these, but resolving opaque handles to literals and keeping the causal chain (verify_chain) intact require them"
  - "file-create early-exit is email-only: the clean file.create path is intent-driven (UserTrusted path), so it must proceed even with zero file-extracted claims"
  - "planner routes file.create/path to the first tainted file handle when present (hostile Block), else the intent handle (clean Allow); contents (content-sensitive) always uses the trusted intent handle so a value resolves"

requirements-completed: [SINK-01, HARD-05]

coverage:
  - id: T1
    description: "WorkerClaim::RelativePath live; extract_relative_path_claims yields relative_path claims (lossy); mint_from_read tags relative_path [ExternalUntrusted, PathRaw] never LocalWorkspace, errors on unknown claim_type; ReportClaims match exhaustive"
    requirement: "SINK-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/quarantine.rs#extract_finds_relative_path_in_hostile_content, extract_relative_path_is_lossy, extract_relative_path_rejects_absolute_and_email, mint_from_read_relative_path_taint_is_path_raw, mint_from_read_unknown_claim_type_errors"
        status: pass
    human_judgment: false
  - id: T2
    description: "invoke_file_create creates the file via create_exclusive_within and records sink_executed (effect_id in actor) on success / sink_execution_failed on error (no retry); broker invokes it on an Allowed file.create; check-invariants green (no EffectRequest)"
    requirement: "HARD-05"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/file_create.rs#invoke_file_create_success_records_sink_executed, invoke_file_create_failure_records_sink_execution_failed"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gate 1 no EffectRequest; Gate 2 runtime-core purity)"
        status: pass
    human_judgment: false
  - id: T3
    description: "CLI drives a file.create plan node for both a tainted workspace-derived path (Block) and a trusted intent path (Allow); worker selects extractor by intent kind; CaprunIntent/ProvideIntent match stays exhaustive"
    requirement: "SINK-01"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#plan_from_intent_create_file_clean_routes_intent_path, plan_from_intent_create_file_hostile_routes_tainted_path"
        status: pass
    human_judgment: false
  - id: T4
    description: "Live end-to-end file.create hostile-block + clean-allow through the real confined worker + broker + executor stack"
    requirement: "SINK-01"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_live_block.rs (authored in 07-05; Linux-gated — run under Colima/Docker seccomp=unconfined)"
        status: unknown
    human_judgment: true
    rationale: "The live file.create §9 tests are written in 07-05 and are #[cfg(target_os=linux)]; the create_exclusive_within enforcement is Linux-only (macOS is a no-security-claim stub, 0-passed expected per CLAUDE.md). This plan makes the sink reachable; 07-05 authors and runs the live proof."

# Metrics
duration: ~8min
completed: 2026-07-01
tasks: 3
files: 9
status: complete
---

# Phase 07 Plan 04b: file.create Live Sink Wiring Summary

**`file.create` is now a real, reachable sink end to end: the `WorkerClaim::RelativePath` variant + claim-type-driven `[ExternalUntrusted, PathRaw]` mint, the live `invoke_file_create` sink (two-phase durable audit, no retry), and intent-kind-driven CLI/worker/planner routing that makes BOTH the hostile-block and clean-allow §9 paths reachable for 07-05's live proof.**

## Performance
- **Duration:** ~8 min
- **Completed:** 2026-07-01
- **Tasks:** 3
- **Files:** 9 (1 created, 8 modified)

## Accomplishments
- `WorkerClaim::RelativePath(String)` activated in `proto.rs` (unknown wire `kind` still fails closed at deserialize).
- `quarantine::extract_relative_path_claims` — a deterministic, no-regex/no-LLM word scanner that emits `relative_path` claims (only the path token crosses IPC; the surrounding sentence is discarded — lossy guarantee).
- `mint_from_read` now derives taint from `claim.claim_type` while remaining the SOLE taint-mint site: `email_address → [ExternalUntrusted, EmailRaw]`, `relative_path → [ExternalUntrusted, PathRaw]` (NEVER `LocalWorkspace`, T-07-44); an unknown claim_type errors rather than default-tagging (T-07-47).
- `sinks::file_create::invoke_file_create` — resolves the `path`/`contents` handles, creates the file via 07-04a's `create_exclusive_within` under the 07-03 `WorkspaceRoot`, and records a two-phase durable audit event: `sink_executed` (effect_id in the actor) on success, `sink_execution_failed` on error then propagate, with NO automatic retry (T-07-45).
- The broker `SubmitPlanNode` arm invokes the live sink on an `Allowed` `file.create` decision, AFTER the authorizing `plan_node_evaluated` event is persisted (two-phase ordering) and chained.
- `CaprunIntent::CreateFileFromReport { path }` + the `create-file-from-report` CLI kind; the worker selects its extractor by intent kind and its early-exit is email-only; the planner routes `file.create/path` by handle provenance (tainted → Block, trusted → Allow), so both §9 paths are reachable.
- The full effect-path ordering is realized end to end: validate schema (04a) → path resolution under capability (03) → sensitivity → executor decision → (on Allowed) live sink invocation (HARD-05).

## Task Commits
| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | WorkerClaim::RelativePath + PathRaw claim-type-driven mint | 3d0a526 |
| 2 | Live file.create sink invocation + two-phase durable audit | 3d94e65 |
| 3 | Intent-kind-driven file.create routing (both §9 paths) | f137aed |

## Files Created/Modified
- `crates/brokerd/src/sinks/file_create.rs` — **created.** `invoke_file_create` + `resolve_arg` + 2 unit tests (success/failure two-phase audit).
- `crates/brokerd/src/proto.rs` — activated `WorkerClaim::RelativePath(String)`.
- `crates/brokerd/src/quarantine.rs` — `extract_relative_path_claims` + `looks_like_relative_path`; claim-type-driven taint in `mint_from_read`; 5 new tests.
- `crates/brokerd/src/server.rs` — `ReportClaims` handles both claim variants (exhaustive); `SubmitPlanNode` invokes `invoke_file_create` on Allowed file.create; `ProvideIntent` match extended for the new intent.
- `crates/brokerd/src/sinks.rs` — `pub mod file_create;`.
- `crates/runtime-core/src/intent.rs` — `CaprunIntent::CreateFileFromReport { path }`.
- `cli/caprun/src/planner.rs` — provenance-based routing for file.create; +2 planner tests.
- `cli/caprun/src/worker.rs` — intent-kind extractor selection; email-only early-exit; import `extract_relative_path_claims`.
- `cli/caprun/src/main.rs` — `create-file-from-report` intent kind + usage docs.
- `cli/caprun/tests/planner.rs` — hostile-block + clean-allow routing tests.

## Decisions Made
- **effect_id carried in `actor`, not a new Event field.** `Event` has no `effect_id` column; adding one would break the pre-anchor golden byte-fixture (07-02 / DESIGN §5, "no DB migration"). The `sink_executed`/`sink_execution_failed` events use `actor = "sink:file.create:<effect_id>"`, which is queryable and keeps serialization byte-compatible.
- **`invoke_file_create` signature extended beyond the plan's illustration.** The plan listed `(conn, session_id, effect_id, plan_node, workspace_root, parent_hash) -> Result<String>`. The action ("extract the path and contents args from the plan node") requires the `ValueStore` to resolve opaque handles to literals, and correct causal chaining (so `verify_chain` stays intact and no orphan root appears) requires `parent_id` in and the new event's `id` out. Final signature: `(conn, value_store, session_id, effect_id, plan_node, workspace_root, parent_id, parent_hash) -> Result<(Uuid, String)>`. The hash is still returned (inside the tuple).
- **file-create early-exit is email-only.** The clean file.create path is intent-driven (a `UserTrusted` path with no file claims), so the worker must proceed even when zero file-extracted claims exist; the email benign-content early-exit is preserved unchanged.

## Deviations from Plan

### Auto-fixed / necessary-ripple items

**1. [Rule 3 - Blocking] `invoke_file_create` needs `value_store` + `parent_id` and returns `(Uuid, String)`**
- **Found during:** Task 2 (implementing the sink).
- **Issue:** The plan's illustrative signature omitted the `ValueStore` (required to resolve opaque `path`/`contents` handles to literals) and the causal `parent_id`/returned event id (required to keep the audit chain unbroken — an event with `parent_id: None` but `parent_hash: Some` makes `verify_chain` return false and creates an orphan DAG root).
- **Fix:** Added `value_store: &ValueStore` and `parent_id: Uuid`; return `(event_id, hash)`. The sink event chains onto the just-persisted `plan_node_evaluated` head.
- **Files:** crates/brokerd/src/sinks/file_create.rs, crates/brokerd/src/server.rs — **Commit:** 3d94e65

**2. [Rule 3 - Blocking] Necessary ripples beyond the plan's listed files (intent.rs, planner.rs, main.rs)**
- **Found during:** Task 3.
- **Issue:** Task 3's action requires a new `CaprunIntent` variant and reaching both §9 paths from the real CLI; the variant lives in `runtime-core/src/intent.rs`, the routing in `cli/caprun/src/planner.rs`, and the CLI kind mapping in `cli/caprun/src/main.rs` — none listed in `files_modified` but all explicitly demanded by the action text ("Add a CaprunIntent variant", "extend the exhaustive ProvideIntent match", "Build the file.create plan node in the worker's plan step").
- **Fix:** Added `CreateFileFromReport { path }`, the planner arm (provenance routing), the `create-file-from-report` CLI kind, and planner tests.
- **Files:** crates/runtime-core/src/intent.rs, cli/caprun/src/planner.rs, cli/caprun/src/main.rs, cli/caprun/tests/planner.rs — **Commit:** f137aed

**3. [in-plan] `sinks/mod.rs` vs `sinks.rs`**
- The plan said "create sinks/mod.rs mirroring how email_send is declared today". The crate actually uses the `sinks.rs` + `sinks/` layout (not `sinks/mod.rs`); `email_send` is declared in `sinks.rs`. Mirrored by adding `pub mod file_create;` to the existing `sinks.rs`. No `sinks/mod.rs` created.

**Total deviations:** 2 blocking ripples + 1 layout clarification. No scope creep — no unrelated CLI surface added; the effect path stays plan-node-only (check-invariants green).

## Known Stubs
- `WorkspaceRoot::create_exclusive_within` on non-Linux is an intentional no-security-claim stub (07-04a), so the file I/O in `invoke_file_create` on macOS uses `create_new` + write with none of the `openat2 RESOLVE_*` guarantees. This is the documented cross-platform posture per CLAUDE.md (all v0 enforcement is Linux-only), not a gap. The `invoke_file_create` unit tests exercise the cross-platform stub on macOS; the real openat2 guarantees are covered by 07-04a's Linux-gated tests and 07-05's live proof.

## Threat Flags
None. No new network endpoints, auth paths, or trust-boundary schema beyond the plan's threat model. Mitigations addressed: T-07-44 (path claims minted `[ExternalUntrusted, PathRaw]`, never `LocalWorkspace`), T-07-45 (two-phase `sink_execution_failed` indeterminate record, no retry), T-07-47 (unknown claim_type fails closed).

## Verification Status
- `cargo build --workspace` — green, no warnings.
- `cargo test --workspace --no-fail-fast` — green on macOS: 29/29 test targets ok, 0 failures. brokerd lib 26 (incl. 5 quarantine + 2 file_create new), caprun planner 5. Linux-only enforcement/e2e tests show "0 passed" as expected (cfg-gated per CLAUDE.md).
- `./scripts/check-invariants.sh` — both gates PASS (no `EffectRequest`; runtime-core pure).

## Next Phase Readiness (07-05)
- `file.create` is live and reachable: `create-file-from-report <path>` drives a real plan node. Hostile workspace content → tainted `RelativePath` routed to `file.create/path` → `BlockedPendingConfirmation` (durable `sink_blocked` anchor, no effect). Clean intent path → `Allowed` → `invoke_file_create` → file created + `sink_executed`.
- 07-05 should run its live tests under the Colima/Docker recipe (`seccomp=unconfined`, no `--privileged`) to exercise the real `openat2` create + the confinement stack; on macOS those tests are `0-passed` by design.

## Self-Check: PASSED
- FOUND: crates/brokerd/src/sinks/file_create.rs
- FOUND commit 3d0a526 (Task 1), 3d94e65 (Task 2), f137aed (Task 3)
- `cargo test --workspace` green (29 ok, 0 failed); check-invariants PASS.

---
*Phase: 07-file-create-sink-enforcement-hardening-full-acceptance*
*Completed: 2026-07-01*
