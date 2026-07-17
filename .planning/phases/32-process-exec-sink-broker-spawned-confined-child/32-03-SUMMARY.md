---
phase: 32-process-exec-sink-broker-spawned-confined-child
plan: 03
subsystem: cli
tags: [process-exec, launcher, landlock, seccomp, self-confinement, workspace-member]

# Dependency graph
requires:
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 02
    provides: sandbox::exec_child_ruleset / sandbox::exec_child_filter
provides:
  - "cli/caprun-exec-launcher binary — a workspace-member helper crate, spawned unconfined by the broker (32-04), that self-confines post-fork (apply_rlimits -> exec_child_ruleset -> exec_child_filter) then self-replaces via execve into an arbitrary target command"
  - "EXEC_COMMAND/EXEC_ARGS_JSON/EXEC_CWD/EXEC_WORKSPACE_ROOT env-var contract for the broker->launcher channel"
affects: [32-04 (broker spawns this launcher, resolved via current_exe().parent().join(...))]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Option B launcher: self-confinement in the launcher's OWN address space post-fork, never inside a Command::pre_exec closure — mirrors sandbox::apply_confinement()'s proven ordering, applied to the exec-child variants"

key-files:
  created:
    - cli/caprun-exec-launcher/Cargo.toml
    - cli/caprun-exec-launcher/src/main.rs
  modified:
    - Cargo.toml

key-decisions:
  - "Confinement is applied strictly rlimits -> exec_child_ruleset -> exec_child_filter, each .expect()-aborting on failure BEFORE the execve of the target -- fail-closed, no target ever runs unconfined (DESIGN §5, §6)."
  - "command is executed directly (CommandExt::exec), never via sh -c; args is JSON-decoded from EXEC_ARGS_JSON into Vec<String> and passed as distinct argv elements via Command::args, never shell-joined (DESIGN §1.5, Pitfall 7)."
  - "No environment passthrough by default to the target (DESIGN §1.5, T-32-11) -- mirrors the existing OPENAI_API_KEY-to-planner-sidecar-only precedent."

patterns-established: []

requirements-completed: [EXEC-01, EXEC-04]

coverage:
  - id: D1
    description: "caprun-exec-launcher is a workspace member binary that reads EXEC_COMMAND/EXEC_ARGS_JSON/EXEC_CWD/EXEC_WORKSPACE_ROOT and self-confines in rlimits->landlock->seccomp order before execve'ing the target, never via sh -c"
    requirement: "EXEC-01, EXEC-04"
    verification:
      - kind: unit
        ref: "cargo build -p caprun-exec-launcher && cargo build --workspace (Mac); ./scripts/check-invariants.sh 4/4 PASS; grep confirms no sh -c / pre_exec usage / arg-joining in main.rs (only doc-comments explaining why they're avoided)"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-17
status: complete
---

# Phase 32 Plan 03: `caprun-exec-launcher` helper binary Summary

**New `cli/caprun-exec-launcher` workspace-member binary that self-confines post-fork (rlimits -> exec_child_ruleset -> exec_child_filter, the exact `apply_confinement()` ordering applied to the exec-child primitives) and then self-replaces via `execve` into an arbitrary target command read from `EXEC_*` env vars — the mechanism the broker-spawned `process.exec` sink (32-04) uses to run a kernel-confined arbitrary child.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-07-17T21:40:00Z (approx, continuation of Phase 32 session)
- **Completed:** 2026-07-17T21:53:00Z
- **Tasks:** 2 completed
- **Files modified:** 3 (`Cargo.toml`, `cli/caprun-exec-launcher/Cargo.toml`, `cli/caprun-exec-launcher/src/main.rs`)

## Accomplishments

- Added `"cli/caprun-exec-launcher"` explicitly to the workspace `members` array (no `cli/*` glob exists, so this must be listed by name).
- Created `cli/caprun-exec-launcher/Cargo.toml` mirroring `caprun-planner`'s package block, with deps `sandbox` (path), `anyhow`, `serde_json` (workspace).
- Implemented `main() -> !` in `cli/caprun-exec-launcher/src/main.rs`:
  - Reads `EXEC_COMMAND` (required), `EXEC_ARGS_JSON` (JSON-decoded `Vec<String>`, default empty), `EXEC_CWD` (optional), `EXEC_WORKSPACE_ROOT` (required — the launcher independently resolves this to build its own Landlock allow-rule, since it cannot share the broker's in-process dirfd).
  - Self-confines in the mandatory order `sandbox::apply_rlimits()` -> `sandbox::exec_child_ruleset(workspace_root)` -> `sandbox::exec_child_filter()`, each `.expect()`-aborting on failure so no confinement gap can precede the exec.
  - Self-replaces via `std::os::unix::process::CommandExt::exec()` into the target — never `sh -c`, `args` passed as distinct argv elements via `Command::args(&args)`, never shell-joined.
  - No environment passthrough to the target by default.

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold the caprun-exec-launcher crate + workspace membership** - `6d342ac` (feat)
2. **Task 2: Launcher main — self-confine post-fork then execve the target** - `d18e488` (feat)

## Files Created/Modified

- `Cargo.toml` - added `"cli/caprun-exec-launcher"` to workspace `members`
- `cli/caprun-exec-launcher/Cargo.toml` - new package, deps `sandbox`/`anyhow`/`serde_json`
- `cli/caprun-exec-launcher/src/main.rs` - new; self-confinement + self-replacing exec implementation

## Decisions Made

- Followed 32-RESEARCH.md Pattern 1's sketch verbatim for the confinement ordering and env-var contract; no deviation from the recommended shape.
- `EXEC_WORKSPACE_ROOT` is a plain path-string env var (not a shared dirfd) — the launcher is a separate process from the broker and must independently `PathFd::new()`-resolve it inside `exec_child_ruleset` (already implemented in 32-02).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. `cargo build -p caprun-exec-launcher` and `cargo build --workspace` are both green on Mac after each task; `./scripts/check-invariants.sh` passes all 4 gates. The binary is placed at `target/debug/caprun-exec-launcher`, confirming the sibling-binary resolution path the broker (32-04) will use via `current_exe().parent().join(...)`.

On Mac, `sandbox::apply_rlimits`/`exec_child_ruleset`/`exec_child_filter` are `#[cfg(not(target_os = "linux"))]` no-op stubs (established in 32-02), so this launcher would `exec` unconfined on Mac — expected; the real Landlock+seccomp confinement is Linux-only and is exercised by 32-06's container-based negative tests, not this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

`cli/caprun-exec-launcher` is a compiled, workspace-member binary ready for 32-04's broker-side `process.exec` sink to spawn (unconfined) via the same `current_exe().parent().join("caprun-exec-launcher")` resolution pattern already used for `caprun-worker`/`caprun-planner`. 32-04 must populate the `EXEC_COMMAND`/`EXEC_ARGS_JSON`/`EXEC_CWD`/`EXEC_WORKSPACE_ROOT` env vars on the `Command` it builds, wire `Stdio::piped()` for stdout/stderr capture, and read the launcher's inherited-through-exec output bytes. The Linux enforcement path (actual Landlock/seccomp syscalls, and the confinement-negative-assertion test per DESIGN §7) is not yet exercised on this Mac dev machine — that is 32-06's scope.

---
*Phase: 32-process-exec-sink-broker-spawned-confined-child*
*Completed: 2026-07-17*

## Self-Check: PASSED

All created/modified files verified present on disk; both task commit hashes
(`6d342ac`, `d18e488`) verified present in git log; `cargo build --workspace`
and `./scripts/check-invariants.sh` (4/4 gates) both green after the final
commit.
