---
phase: 28-authenticated-audit-chain
plan: 01
subsystem: testing
tags: [rust, cargo, hmac, getrandom, sha2, test-fixtures, f1-refusal, audit-chain]

# Dependency graph
requires:
  - phase: 27-session-connection-integrity-hardening
    provides: HARDEN-01 (demote-at-RequestFd) + HARDEN-04 (compile-out forced-Active mint), the immediately-prior landed hardening phase
provides:
  - 7 live-test fixtures migrated to the F1-safe directory layout (workspace file under its own subdirectory, audit.db a sibling of that subdirectory)
  - hmac 0.12.1 and getrandom 0.4 available as direct crates/brokerd dependencies, workspace-pinned
affects: [28-02-authenticated-audit-chain, 28-03-authenticated-audit-chain, 28-04-authenticated-audit-chain, 28-05-authenticated-audit-chain]

# Tech tracking
tech-stack:
  added: ["hmac 0.12.1 (workspace-pinned, crates/brokerd direct dep)", "getrandom 0.4 (workspace-pinned, crates/brokerd direct dep, already transitively resolved via uuid)"]
  patterns: ["F1-safe test fixture layout: workspace file under its own `ws_dir`/`workspace` subdirectory, audit.db a sibling of that subdirectory — mirrors cli/caprun/tests/confirm.rs, now the only layout used across all live fixtures"]

key-files:
  created: []
  modified:
    - cli/caprun/tests/s9_live_block.rs
    - cli/caprun/tests/e2e.rs
    - cli/caprun/tests/live_acceptance_tainted_session.rs
    - cli/caprun/tests/live_acceptance_v1_3.rs
    - cli/caprun/tests/live_acceptance_v1_4_composed.rs
    - cli/caprun/tests/llm_planner_live_accept.rs
    - cli/caprun/tests/origin_seed_provenance.rs
    - Cargo.toml
    - crates/brokerd/Cargo.toml
    - Cargo.lock

key-decisions:
  - "s9_live_block.rs's run_caprun_file_create now returns the workspace subdirectory (ws_dir) as its third tuple element instead of the outer tmp dir, since callers use that value as the actual broker-derived workspace root for existence assertions."
  - "live_acceptance_tainted_session.rs's run_caprun_block signature changed from taking `tmp: &Path` (doubling as workspace root) to `ws_dir: &Path` (the dedicated workspace subdirectory); both callers now mint ws_dir explicitly and use it for post-run existence assertions instead of tmp."
  - "live_acceptance_v1_3.rs and live_acceptance_v1_4_composed.rs's run_caprun_email_on gained a new `ws_dir: &Path` parameter (replacing the `audit_db.parent()` derivation) so the shared multi-invocation workspace root is an explicit, F1-safe sibling directory of audit.db rather than audit.db's own parent."

requirements-completed: [HARDEN-02]

coverage:
  - id: D1
    description: "All 7 previously-vulnerable live-test fixtures relocated to the F1-safe layout (workspace file under its own subdirectory, audit.db a sibling of that subdirectory, never a direct child of the workspace root)"
    requirement: "HARDEN-02"
    verification:
      - kind: other
        ref: "grep -n 'join(\"audit.db\")' across the 7 fixture files — audit.db's parent (tmp) differs from each workspace file's parent (ws_dir/workspace) in every case"
        status: pass
      - kind: unit
        ref: "cargo test --workspace --no-fail-fast (macOS) — 274 passed / 0 failed, identical to pre-migration baseline"
        status: pass
    human_judgment: false
  - id: D2
    description: "hmac 0.12.1 and getrandom 0.4 added as direct crates/brokerd dependencies, workspace-pinned, compiling clean"
    requirement: "HARDEN-02"
    verification:
      - kind: other
        ref: "cargo build -p brokerd"
        status: pass
      - kind: other
        ref: "cargo tree -p brokerd -i hmac (resolves 0.12.1) / cargo tree -p brokerd -i getrandom (resolves 0.4.3)"
        status: pass
    human_judgment: false

duration: 7min
completed: 2026-07-13
status: complete
---

# Phase 28 Plan 01: F1-Safe Fixture Migration + HMAC/getrandom Deps Summary

**7 live-test fixtures relocated to confirm.rs's F1-safe subdirectory layout (behaviorally a no-op — 274/274 tests still pass) plus hmac 0.12.1 + getrandom 0.4 wired into crates/brokerd, with zero crypto logic written — pure groundwork for HARDEN-02's F1 refusal and keyed-MAC audit chain landing in Plans 02-05.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-07-13T00:21:51Z
- **Completed:** 2026-07-13T00:28:44Z
- **Tasks:** 2
- **Files modified:** 10 (7 test fixtures, Cargo.toml, crates/brokerd/Cargo.toml, Cargo.lock)

## Accomplishments
- Migrated all 7 vulnerable live-test fixtures (`s9_live_block.rs`, `e2e.rs`, `live_acceptance_tainted_session.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`, `llm_planner_live_accept.rs`, `origin_seed_provenance.rs`) to the F1-safe directory layout — the workspace file (or shared multi-invocation workspace root) now lives under its own subdirectory, with `audit.db` a sibling of that subdirectory rather than a direct child of the workspace root.
- Proved the migration is behaviorally a no-op: `cargo test --workspace --no-fail-fast` on macOS shows the identical 274 passed / 0 failed both before and after, with the same set of test names (Linux-gated tests correctly show 0 passed on macOS in both runs).
- Added `hmac = "0.12.1"` and `getrandom = "0.4"` to the root `[workspace.dependencies]` and referenced both from `crates/brokerd/Cargo.toml`; `cargo build -p brokerd` compiles clean against the existing `sha2 = "0.10"` pin, `hmac` resolves to exactly `0.12.1` (not `0.13.0`), and `getrandom` resolves to `0.4.3` with zero new dependency-graph growth (already transitively present via `uuid`'s `v4` feature).
- Re-ran `./scripts/check-invariants.sh` after both tasks — all 4 gates (raw effect-to-sink, runtime-core purity, mint-call-site restriction, test-fixtures non-default) still pass; no TCB source under `crates/*/src` was touched.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate 7 live-test fixtures to the F1-safe directory layout** - `23b6e15` (test)
2. **Task 2: Add hmac 0.12.1 and getrandom 0.4 dependencies to brokerd** - `048e64b` (chore)

**Plan metadata:** (this commit, immediately following)

## Files Created/Modified
- `cli/caprun/tests/s9_live_block.rs` - `run_caprun_intent_on`/`run_caprun_file_create` now write the workspace file into a `ws_dir` subdirectory; `run_caprun_file_create` returns `ws_dir` (not the outer tmp dir) as its workspace-root tuple element
- `cli/caprun/tests/e2e.rs` - both `substrate_demo`/`dag_chain_integrity` setups relocate the workspace file into a `ws_dir` subdirectory sibling of `audit.db`
- `cli/caprun/tests/live_acceptance_tainted_session.rs` - `run_caprun_block` signature changed to take `ws_dir: &Path`; both deny/confirm-path tests mint `ws_dir` explicitly and assert against it instead of `tmp`
- `cli/caprun/tests/live_acceptance_v1_3.rs` - `run_caprun_email_on` gained a `ws_dir: &Path` param, replacing the `audit_db.parent().join(...)` derivation, at all 3 call sites
- `cli/caprun/tests/live_acceptance_v1_4_composed.rs` - same `run_caprun_email_on` `ws_dir` param change, at all 3 call sites (LLM-planner 3-leg live proof)
- `cli/caprun/tests/llm_planner_live_accept.rs` - workspace file relocated into a `ws_dir` subdirectory
- `cli/caprun/tests/origin_seed_provenance.rs` - `setup_tmp` relocates the workspace file into a `ws_dir` subdirectory (this file's tests run on macOS, not Linux-gated — verified they remained green)
- `Cargo.toml` - added `hmac = "0.12.1"` and `getrandom = "0.4"` to `[workspace.dependencies]`
- `crates/brokerd/Cargo.toml` - added `hmac = { workspace = true }` and `getrandom = { workspace = true }` to `[dependencies]`
- `Cargo.lock` - updated to include `hmac v0.12.1` and `digest v0.10.7` (getrandom already present)

## Decisions Made
- Where a fixture's workspace-root value was consumed downstream for post-run file-existence assertions (`s9_live_block.rs::run_caprun_file_create`, `live_acceptance_tainted_session.rs::run_caprun_block`), the function's return/parameter contract was updated to expose the new `ws_dir` (not the outer `tmp`) as the workspace root, since that is what the broker actually derives as `workspace_file.parent()` and where the live `file.create` sink writes.
- `live_acceptance_v1_3.rs`/`live_acceptance_v1_4_composed.rs`'s `run_caprun_email_on` needed an explicit new `ws_dir: &Path` parameter (rather than continuing to derive the workspace root from `audit_db.parent()`) because these fixtures share ONE workspace root across 2-3 sequential `caprun` invocations (COORD-T5) — inverting the derivation cleanly separates "the shared workspace root" from "the audit DB path" as two independent, F1-safe siblings.
- No other deviations from the plan's mechanical action text — every change is confined to path bindings, `create_dir_all` calls, and their accompanying comments (verified via `git diff` review excluding those tokens, confirming zero assertion/marker/intent text was altered).

## Deviations from Plan

None - plan executed exactly as written. Both tasks landed within their described scope; no cryptographic, key-custody, or refusal logic was introduced (correctly deferred to Plans 02/03).

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All 7 live fixtures are now F1-safe; Plan 02's fail-closed startup refusal can land without turning the live suite red.
- `hmac`/`getrandom` are available and compile-verified for Plan 03's keyed `compute_event_hash`/`append_event`/`verify_chain` work.
- No blockers. `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (274/274, unchanged), and `./scripts/check-invariants.sh` (4/4 gates) all green at hand-off.

---
*Phase: 28-authenticated-audit-chain*
*Completed: 2026-07-13*

## Self-Check: PASSED

All 10 modified/created files confirmed present on disk; all 3 commits
(`23b6e15` fixture migration, `048e64b` hmac/getrandom deps, plus this
summary's own commit) confirmed present in git log.
