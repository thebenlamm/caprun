---
phase: 33-filesystem-read-write-breadth
plan: 01
subsystem: adapter-fs
tags: [rust, openat2, landlock-adjacent-fs, workspace-root, nix]

requires:
  - phase: 31-effect-breadth-design-gate
    provides: "DESIGN-effect-breadth-exec.md §3.2 pinning O_WRONLY|O_TRUNC, no-O_CREAT write/edit model"
provides:
  - "WorkspaceRoot::write_within — existing-file-only write primitive beneath WorkspaceRoot (Linux impl + non-Linux stub)"
  - "5 write_within_* inline tests (4 negative/edge + 1 positive), proven green on real Linux"
affects: [33-filesystem-read-write-breadth (sibling plans: file.write sink module, dispatch arm, executor tables), 34-live-proof-regression]

tech-stack:
  added: []
  patterns:
    - "existing-file-only fs primitive: O_WRONLY|O_TRUNC via openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS), no O_CREAT — the exact structural sibling of create_exclusive_within's new-file-only O_CREAT|O_EXCL"

key-files:
  created: []
  modified:
    - crates/adapter-fs/src/workspace.rs

key-decisions:
  - "write_within omits O_CREAT/O_EXCL and .mode(...) entirely per DESIGN §3.2 — a missing target fails closed with ENOENT rather than silently creating, keeping create authority exclusively with create_exclusive_within"
  - "Negative test set NOT assumed inherited from read_within (O_RDONLY) or create_exclusive_within (O_CREAT|O_EXCL) — wrote the full absolute/traversal/symlink/ENOENT set fresh for the new O_WRONLY|O_TRUNC flag combination"

patterns-established:
  - "write_within: sibling method pattern for existing-file-only fs primitives — same openat2/RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS resolution, differing only in OFlag open-mode set"

requirements-completed: [FS-02]

coverage:
  - id: D1
    description: "write_within writes bytes into an EXISTING workspace file resolved beneath WorkspaceRoot, using the same openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS) kernel-atomic resolution as read_within/create_exclusive_within"
    requirement: "FS-02"
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#write_within_overwrites_existing"
        status: pass
    human_judgment: false
  - id: D2
    description: "a missing target path fails closed with ENOENT (never silently creates the file) — proving no O_CREAT path exists"
    requirement: "FS-02"
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#write_within_missing_target_enoent"
        status: pass
    human_judgment: false
  - id: D3
    description: "absolute paths, `..`-traversal, and symlink-escape are all rejected at kernel resolution time for the new O_WRONLY|O_TRUNC flag combination"
    requirement: "FS-02"
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#write_within_absolute_path_rejected"
        status: pass
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#write_within_parent_traversal_rejected"
        status: pass
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#write_within_symlink_escape_rejected"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-07-18
status: complete
---

# Phase 33 Plan 01: write_within existing-file-only fs primitive Summary

**Added `WorkspaceRoot::write_within` (O_WRONLY|O_TRUNC via openat2, no O_CREAT) as the existing-file-only sibling to `create_exclusive_within`, with a fresh 5-test negative/edge set proving fail-closed ENOENT-on-missing-target and kernel-level absolute/traversal/symlink rejection, verified green on real Linux via Colima.**

## Performance

- **Duration:** 12 min
- **Started:** 2026-07-18T00:00:00Z (approx, first task commit 9295f1f)
- **Completed:** 2026-07-18T00:12:47Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- `WorkspaceRoot::write_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()>` added immediately after `create_exclusive_within`, both `#[cfg(target_os = "linux")]` (real `openat2(O_WRONLY|O_TRUNC, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`) and `#[cfg(not(target_os = "linux"))]` (macOS stub, ENOENT-parity, no security claim) variants.
- 4 NOT-inherited negative tests (`write_within_absolute_path_rejected`, `write_within_parent_traversal_rejected`, `write_within_symlink_escape_rejected`, `write_within_missing_target_enoent`) plus 1 positive truncation-semantics test (`write_within_overwrites_existing`) added to the existing `#[cfg(test)] mod tests`.
- All 5 tests verified green on real Linux via `MAILPIT_VERIFY_CMD='cargo test -p adapter-fs write_within' bash scripts/mailpit-verify.sh` — true exit 0 captured before any pipe, named test counts asserted (5 passed; 0 failed).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add write_within (Linux impl + non-Linux stub) beside create_exclusive_within** - `9295f1f` (feat)
2. **Task 2: Add the NOT-inherited O_WRONLY|O_TRUNC negative test set + ENOENT test, verified on Linux** - `06d2aa4` (test)

**Plan metadata:** committed after SUMMARY.md is written (worktree mode — this commit follows).

## Files Created/Modified
- `crates/adapter-fs/src/workspace.rs` - added `write_within` (Linux impl + non-Linux stub) and 5 new inline tests (`write_within_*`)

## Decisions Made
- Mirrored `create_exclusive_within`'s exact structure for `write_within`, changing only the `OFlag` set (`O_WRONLY | O_TRUNC` instead of `O_CREAT | O_EXCL | O_WRONLY`) and omitting `.mode(...)` (meaningless without `O_CREAT`) — per DESIGN §3.2 and 33-PATTERNS.md's exact excerpt.
- Reused `nix::libc::ENOENT`/`EXDEV` for OS-error assertions (matching the existing `create_exclusive_*` test convention) rather than adding a new `libc` direct dependency.

## Deviations from Plan

None - plan executed exactly as written. Both tasks matched the plan's `<action>` and `<behavior>` blocks verbatim; no auto-fixes, no architectural questions, no auth gates.

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `write_within` is ready for the sibling plans in this phase to build on: the `file.write` sink module (`invoke_file_write`), the `"file.write"` dispatch arm in `server.rs`, and the `sink_schema.rs`/`sink_sensitivity.rs` executor table entries (per 33-PATTERNS.md's pattern assignments for those files).
- No blockers. Scope was deliberately limited to the adapter-fs primitive + its tests, per the plan's objective — sink module, dispatch, and executor tables land in sibling plans.

---
*Phase: 33-filesystem-read-write-breadth*
*Completed: 2026-07-18*

## Self-Check: PASSED
- FOUND: crates/adapter-fs/src/workspace.rs
- FOUND: .planning/phases/33-filesystem-read-write-breadth/33-01-SUMMARY.md
- FOUND: commit 9295f1f (Task 1)
- FOUND: commit 06d2aa4 (Task 2)
