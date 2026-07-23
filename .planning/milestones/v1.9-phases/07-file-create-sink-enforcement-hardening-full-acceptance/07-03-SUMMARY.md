---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 03
subsystem: infra
tags: [openat2, dirfd, landlock, RESOLVE_BENEATH, RESOLVE_NO_SYMLINKS, nix, adapter-fs, brokerd, path-traversal, TOCTOU]

# Dependency graph
requires:
  - phase: 05-runtime-spine-live-9-email-block
    provides: unified broker dispatch (run_broker_server / dispatch_request) with the RequestFd arm and per-connection state threading
provides:
  - "adapter_fs::workspace::WorkspaceRoot(OwnedFd) capability with dirfd-anchored read_within (openat2 RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)"
  - "Arc<WorkspaceRoot> threaded through run_broker_server -> handle_connection -> dispatch_request"
  - "broker RequestFd arm resolves worker-supplied paths beneath the workspace dirfd (HARD-04 read side closed)"
affects: [07-04-file-create-sink, SINK-04, write-side create_exclusive_within]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "cfg-gated real/stub syscall wrapper (Linux openat2 real impl + non-Linux no-security-claim stub) mirroring sandbox::landlock::deny_all_filesystem"
    - "dirfd capability: open the anchor once via nix open(O_DIRECTORY|O_RDONLY), resolve all subsequent reads beneath it via a single openat2 syscall (TOCTOU-safe)"
    - "worker sends a root-RELATIVE path; broker derives the root from the workspace-file parent (zero new CLI surface)"

key-files:
  created:
    - crates/adapter-fs/src/workspace.rs
  modified:
    - crates/adapter-fs/src/lib.rs
    - crates/brokerd/src/server.rs
    - cli/caprun/src/main.rs
    - crates/brokerd/tests/uds_ipc.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/phase5_dispatch.rs

key-decisions:
  - "RESEARCH Q2 Option (a): derive the workspace root from the workspace-file parent; worker sends a root-relative basename — no new CLI arg, e2e call-sites unchanged"
  - "Both RESOLVE_BENEATH and RESOLVE_NO_SYMLINKS are required — RESOLVE_BENEATH alone does not block in-tree symlink traversal"
  - "Resolution + open are a single openat2 syscall (no validate-then-open / no TOCTOU window)"
  - "No tempfile dev-dep in adapter-fs — enforcement tests use std::env::temp_dir() with uniquely-named subdirs"

patterns-established:
  - "Pattern 1: WorkspaceRoot dirfd capability — the single sanctioned way for the broker to open a worker-supplied path"
  - "Pattern 2: Arc<WorkspaceRoot> is cloned per accepted connection exactly like conn (Arc<Mutex<Connection>>)"

requirements-completed: [HARD-04]

coverage:
  - id: D1
    description: "WorkspaceRoot::read_within resolves reads beneath a dirfd anchor; absolute, .., and symlink-escape paths are rejected at kernel resolution time"
    requirement: "HARD-04"
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#legit_relative_read_ok, absolute_path_rejected, parent_traversal_rejected, symlink_escape_rejected (Linux-gated)"
        status: unknown
    human_judgment: true
    rationale: "Enforcement tests are #[cfg(target_os = \"linux\")]-gated; the dev machine is macOS (0-passed expected). Real openat2 RESOLVE_* semantics must be confirmed on Linux via the Colima/Docker recipe before this deliverable can be auto-passed."
  - id: D2
    description: "Broker RequestFd arm resolves worker-supplied paths via WorkspaceRoot::read_within instead of ambient std::fs::File::open; parameter threaded through run_broker_server/handle_connection/dispatch_request"
    requirement: "HARD-04"
    verification:
      - kind: integration
        ref: "cargo test -p brokerd --no-fail-fast"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gate 1: no EffectRequest; Gate 2: runtime-core purity)"
        status: pass
    human_judgment: false

# Metrics
duration: 20min
completed: 2026-07-01
status: complete
---

# Phase 7 Plan 03: HARD-04 Workspace-Root Dirfd Capability (read side) Summary

**Broker no longer opens worker-supplied paths via ambient `std::fs::File::open`: every `RequestFd` read now resolves beneath a single dirfd anchor via `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`, rejecting absolute/traversal/symlink-escape paths at kernel resolution time.**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-01
- **Tasks:** 2
- **Files modified:** 7 (1 created, 6 modified)

## Accomplishments
- New `adapter_fs::workspace::WorkspaceRoot(OwnedFd + PathBuf)` capability: `open()` anchors the root via `nix open(O_DIRECTORY|O_RDONLY)`; `read_within()` is a cfg-gated pair (Linux `openat2` real impl / non-Linux no-security-claim stub) mirroring `sandbox::landlock::deny_all_filesystem`.
- Four Linux-gated enforcement tests: legit in-root read OK; absolute path, `..` traversal (both EXDEV), and in-tree symlink-escape all `Err`.
- `Arc<WorkspaceRoot>` threaded through `run_broker_server` → `handle_connection` → `dispatch_request`, cloned per accepted connection exactly like `conn`.
- Broker `RequestFd` arm now calls `WorkspaceRoot::read_within` (fail-closed) — the concrete HARD-04 unrestricted-open vulnerability is closed on the read side.
- `main()` opens the workspace-root dirfd once from the `workspace-file` parent and hands the worker a root-relative `WORKSPACE_FILE` basename (no new CLI surface).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add WorkspaceRoot dirfd capability + read_within to adapter-fs** - `9d80ffb` (feat)
2. **Task 2: Thread Arc<WorkspaceRoot> through the broker and resolve RequestFd under it** - `af92b7f` (feat)

## Files Created/Modified
- `crates/adapter-fs/src/workspace.rs` - WorkspaceRoot capability + read_within + 4 Linux-gated enforcement tests (created)
- `crates/adapter-fs/src/lib.rs` - register `pub mod workspace;`
- `crates/brokerd/src/server.rs` - thread Arc<WorkspaceRoot> through the three server fns; RequestFd arm uses read_within
- `cli/caprun/src/main.rs` - open workspace-root dirfd once; send worker a root-relative WORKSPACE_FILE; pass Arc<WorkspaceRoot> to run_broker_server
- `crates/brokerd/tests/uds_ipc.rs` - update 2 run_broker_server call sites for the new parameter
- `crates/brokerd/tests/proto_claims.rs` - update dispatch_request call site
- `crates/brokerd/tests/phase5_dispatch.rs` - add ws_root() helper; update 2 dispatch_request call sites

## Decisions Made
- Adopted RESEARCH Q2 Option (a): derive the workspace root from the `workspace-file` parent and send a root-relative basename — zero new CLI arg, existing e2e call-sites unchanged.
- Required BOTH `RESOLVE_BENEATH` and `RESOLVE_NO_SYMLINKS` (RESOLVE_BENEATH alone does not block in-tree symlink traversal — RESEARCH Q1 caveat).
- Single `openat2` syscall performs resolution + open atomically (no validate-then-open, no TOCTOU window).
- `adapter-fs` has no `tempfile` dev-dependency, so tests use `std::env::temp_dir()` with uniquely-named subdirs (per the plan's explicit fallback).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated existing broker test call sites for the new WorkspaceRoot parameter**
- **Found during:** Task 2 (threading Arc<WorkspaceRoot>)
- **Issue:** Adding the `workspace_root` parameter to `run_broker_server` / `dispatch_request` broke compilation of existing tests (`uds_ipc.rs`, `proto_claims.rs`, `phase5_dispatch.rs`), which call those functions with the old arity.
- **Fix:** Added an `Arc<WorkspaceRoot>` (anchored on `std::env::temp_dir()`) at each call site — those tests exercise CreateSession/ProvideIntent/SubmitPlanNode, never the RequestFd arm, so any valid directory anchor is sufficient. Added a `ws_root()` helper in `phase5_dispatch.rs`.
- **Files modified:** crates/brokerd/tests/uds_ipc.rs, crates/brokerd/tests/proto_claims.rs, crates/brokerd/tests/phase5_dispatch.rs
- **Verification:** `cargo test -p brokerd --no-fail-fast` passes (no failures).
- **Committed in:** af92b7f (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The fix was required to keep the build/tests green after the parameter was threaded. No behavioral change to the tests, no scope creep.

## Issues Encountered
None. `nix::fcntl::open` returns `OwnedFd` on 0.31.3 (used directly, no `from_raw_fd` needed); `openat2`/`OpenHow`/`ResolveFlag` are already available behind the `fs` feature — zero new dependencies.

## Known Stubs
- `WorkspaceRoot::read_within` on non-Linux is an intentional no-security-claim stub (`std::fs::File::open(root.join(rel))`), mirroring `sandbox::landlock::deny_all_filesystem`. This exists solely so the crate compiles on the macOS dev machine; all enforcement claims are Linux-only per CLAUDE.md. Not a gap.

## User Setup Required
None - no external service configuration required.

To run the Linux-only enforcement tests from the Mac (per CLAUDE.md):
`colima start && docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p adapter-fs workspace`

## Next Phase Readiness
- The shared `WorkspaceRoot` capability is in place for SINK-04 (07-04) to add its write-side method (`create_exclusive_within`, `O_CREAT|O_EXCL`).
- Prohibition recorded for downstream plans: any value minted from workspace file content stays `ExternalUntrusted`, never `LocalWorkspace` (this plan mints no values).

## Self-Check

- FOUND: crates/adapter-fs/src/workspace.rs
- FOUND commit 9d80ffb (Task 1)
- FOUND commit af92b7f (Task 2)

## Self-Check: PASSED

---
*Phase: 07-file-create-sink-enforcement-hardening-full-acceptance*
*Completed: 2026-07-01*
