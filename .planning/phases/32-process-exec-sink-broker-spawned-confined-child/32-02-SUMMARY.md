---
phase: 32-process-exec-sink-broker-spawned-confined-child
plan: 02
subsystem: sandbox
tags: [landlock, seccomp, seccomp-bpf, confinement, process-exec, kernel-security]

# Dependency graph
requires:
  - phase: 31-design-gate
    provides: DESIGN-effect-breadth-exec.md (confinement model, Option B launcher, B1 seccomp-recursion resolution)
provides:
  - "sandbox::landlock::exec_child_ruleset(workspace_root) — narrow Landlock allow-list for a process.exec child"
  - "sandbox::seccomp::exec_child_filter() — seccomp filter reusing net-deny, no execve-deny"
  - "Both re-exported from sandbox crate root"
affects: [32-03 (caprun-exec-launcher, applies these to itself post-fork), 32-06 (confinement negative tests)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-constructor confinement split: worker-wide deny-all (deny_all_filesystem/apply_worker_filter, unchanged) vs. exec-child narrow-allow (exec_child_ruleset/exec_child_filter, new) — chosen by caller, not a config flag."

key-files:
  created: []
  modified:
    - crates/sandbox/src/landlock.rs
    - crates/sandbox/src/seccomp.rs
    - crates/sandbox/src/lib.rs

key-decisions:
  - "System path allow-list hardcoded as [\"/usr\",\"/bin\",\"/lib\",\"/lib64\"] per DESIGN §8 item 1 — an enumerated constant, not a PATH walk; exact strings deferred to 32-06 in-container verification."
  - "exec_child_filter omits execve/execveat deny entirely rather than attempting a one-shot allow — DESIGN §1.4 B1: a stateless BPF filter cannot express recursion-deny; the grandchild bound is Landlock's Execute allow-list plus the same persistent net-deny."

patterns-established: []

requirements-completed: [EXEC-04]

coverage:
  - id: D1
    description: "exec_child_ruleset() grants ReadFile+Execute on enumerated system paths and ReadFile+WriteFile (no Execute) on WorkspaceRoot only, denying everything else, distinct from deny_all_filesystem()"
    requirement: "EXEC-04"
    verification:
      - kind: unit
        ref: "cargo build -p sandbox && cargo build -p sandbox --tests (Mac stub compiles; Linux path exercised by 32-06 container tests)"
        status: pass
    human_judgment: false
  - id: D2
    description: "exec_child_filter() reuses the socket(AF_INET/AF_INET6) deny byte-for-byte and contains no SYS_execve/SYS_execveat deny entry"
    requirement: "EXEC-04"
    verification:
      - kind: unit
        ref: "cargo build -p sandbox && cargo build -p sandbox --tests; manual diff confirms apply_worker_filter() unchanged"
        status: pass
    human_judgment: false

duration: 8min
completed: 2026-07-17
status: complete
---

# Phase 32 Plan 02: exec-child Landlock + seccomp confinement primitives Summary

**New `exec_child_ruleset()` (Landlock narrow-allow: system-path Execute + workspace-only ReadWrite) and `exec_child_filter()` (seccomp net-deny without execve-deny), added beside the unchanged worker constructors for the upcoming `caprun-exec-launcher` (32-03) to self-apply post-fork.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-07-17T21:41:00Z
- **Completed:** 2026-07-17T21:49:00Z
- **Tasks:** 2 completed
- **Files modified:** 3 (`landlock.rs`, `seccomp.rs`, `lib.rs`)

## Accomplishments
- Added `exec_child_ruleset(workspace_root: &Path)` to `crates/sandbox/src/landlock.rs` — a distinct, narrower-than-open Landlock ruleset (NOT `deny_all_filesystem()` reused) granting `ReadFile+Execute` on an enumerated system-path allow-list and `ReadFile+WriteFile` (no `Execute`) on the workspace root only.
- Added `exec_child_filter()` to `crates/sandbox/src/seccomp.rs` — reuses `apply_worker_filter()`'s socket(AF_INET/AF_INET6) deny block byte-for-byte, but omits the `SYS_execve`/`SYS_execveat` deny entries so the launcher's own upcoming `execve` succeeds.
- Both functions have `#[cfg(not(target_os="linux"))]` no-op stubs and are re-exported from `sandbox`'s crate root.
- Confirmed `deny_all_filesystem()` and `apply_worker_filter()` are byte-for-byte unchanged (pure additions, no deletions in either diff).

## Task Commits

Each task was committed atomically:

1. **Task 1: exec_child_ruleset() — Landlock narrow allow-list** - `9300c2d` (feat)
2. **Task 2: exec_child_filter() — seccomp net-deny, no execve-deny** - `64a7f83` (feat)

**Plan metadata:** (this SUMMARY commit, see below)

## Files Created/Modified
- `crates/sandbox/src/landlock.rs` - Added `exec_child_ruleset()` (Linux impl + no-op stub) beside unchanged `deny_all_filesystem()`
- `crates/sandbox/src/seccomp.rs` - Added `exec_child_filter()` (Linux impl + no-op stub) beside unchanged `apply_worker_filter()`
- `crates/sandbox/src/lib.rs` - Re-exported `exec_child_ruleset` and `exec_child_filter`

## Decisions Made
- Followed 32-RESEARCH.md Pattern 2/Pattern 3 code shapes verbatim (verified against vendored `landlock-0.4.5` crate source and the existing `seccompiler` 0.5.0 usage in this file).
- System allow-list hardcoded as `["/usr","/bin","/lib","/lib64"]` — an explicit enumerated constant per the "no dynamic registry" discipline; DESIGN §8 defers exact-string verification to 32-06's in-container `ldd`/`which` check.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. Both `cargo build -p sandbox` and `cargo build -p sandbox --tests` are green after each task; `./scripts/check-invariants.sh` passes (all 4 gates) after both tasks landed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

`sandbox::exec_child_ruleset` and `sandbox::exec_child_filter` are compiled, re-exported, and ready for the `caprun-exec-launcher` (32-03) to apply to itself post-fork, before its `execve` of the target binary. The Linux enforcement path (actual Landlock/seccomp syscalls) is not yet exercised on this Mac dev machine — 32-06's container-based negative tests are the point where these primitives get their first real kernel-level verification, including confirming the exact system path list against the verification container's layout.

---
*Phase: 32-process-exec-sink-broker-spawned-confined-child*
*Completed: 2026-07-17*
