---
phase: 52-minimal-linux-packaging
plan: 01
subsystem: infra
tags: [bash, install-script, packaging, docs]

# Dependency graph
requires: []
provides:
  - "scripts/install-linux.sh — thin Linux source-build installer producing exactly caprun/caprun-worker/caprun-exec-launcher in one destination directory"
  - "docs/GETTING-STARTED.md '## Install (Linux)' section — script path + manual equivalent + cargo-install insufficiency warning + corrected CLI usage/troubleshooting content"
affects: [52-02, packaging, design-partner-onboarding]

# Actuals (#2632)
actuals:
  tokens: 4160
  tasks: 3
  commits: 3

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "House shell-script style (compose-verify.sh/mailpit-verify.sh/docker-cache.sh): header doc-comment with Usage/env-override table, set -euo pipefail, ${VAR:-default}, echo \"FAIL — <what> <why>\" >&2; exit 1, trap cleanup EXIT"
    - "Stage-then-rename install: mktemp -d inside the destination, install -m 755 + test -x per binary, then three back-to-back same-filesystem mv operations in a fixed order (helpers first, orchestrator last)"

key-files:
  created:
    - scripts/install-linux.sh
  modified:
    - docs/GETTING-STARTED.md

key-decisions:
  - "Destination-usability guard (mkdir -p + test -w) runs before the build/staging steps so a refused re-install into a now-unwritable destination never partially replaces a previously installed set (D-06 boundary)."
  - "Kernel-version check (uname -r vs 5.13) is advisory-only — it warns and still exits 0, since crates/sandbox already negotiates the Landlock ABI down at runtime."
  - "caprun-planner is built by the workspace release build but deliberately never copied — verified by a body-scoped grep asserting zero non-comment mentions of the token."

patterns-established:
  - "Same-filesystem stage-then-rename as the sanctioned alternative to a hand-rolled lock protocol for a 3-file atomic-ish install (per RESEARCH.md 'Don't Hand-Roll')."

requirements-completed: [PKG-01]

coverage:
  - id: D1
    description: "bash scripts/install-linux.sh builds the release binaries and installs exactly caprun, caprun-worker, caprun-exec-launcher (all executable) into one destination directory, defaulting to ${HOME}/.local/bin and overridable via --dest/INSTALL_DEST"
    requirement: "PKG-01"
    verification:
      - kind: e2e
        ref: "manual invocation: INSTALL_DEST=\"$(mktemp -d)\" bash scripts/install-linux.sh; ls verified exact three-file set, all -x"
        status: pass
    human_judgment: false
  - id: D2
    description: "Fail-fast guards: non-Linux host, missing cargo, unwritable destination, bogus flag each exit non-zero with an actionable FAIL/usage message; a non-writable destination on re-install leaves the prior binaries' sha256sum unchanged (D-06 fault injection)"
    requirement: "PKG-01"
    verification:
      - kind: e2e
        ref: "manual invocation: chmod 555 destination + re-run; sha256sum before/after compared equal; --bogus-flag and --dest with a chmod-500 parent both exercised"
        status: pass
    human_judgment: false
  - id: D3
    description: "docs/GETTING-STARTED.md documents both the script and the manual-equivalent commands, states cargo install --path cli/caprun is insufficient (naming the separate-package root cause), names the three required binaries plus the optional caprun-planner sidecar, and carries the corrected CLI usage line"
    requirement: "PKG-01"
    verification:
      - kind: other
        ref: "grep assertions per Task 3 acceptance criteria (caprun-exec-launcher count >=4, cargo install --path cli/caprun present, corrected usage line present, stale 'two binaries'/bare-positional-usage strings absent, colima start still present)"
        status: pass
    human_judgment: false

duration: ~34min
completed: 2026-08-11
status: complete
---

# Phase 52 Plan 01: Minimal Linux Install Script Summary

**A `bash scripts/install-linux.sh` one-liner builds caprun's release binaries and stage-then-renames exactly `caprun`, `caprun-worker`, `caprun-exec-launcher` into `${HOME}/.local/bin` (or `--dest`), with fail-fast OS/toolchain/writability guards and a documented manual-equivalent path in `docs/GETTING-STARTED.md`.**

## Performance

- **Duration:** ~34 min (includes one cold `cargo build --workspace --release`, ~23m16s, run once to warm the target/release cache)
- **Tasks:** 3/3 completed
- **Files modified:** 2 (1 created: `scripts/install-linux.sh`; 1 modified: `docs/GETTING-STARTED.md`)

## Accomplishments
- `scripts/install-linux.sh`: builds `cargo build --workspace --release`, stages the three required sibling binaries into a `mktemp -d` directory inside the destination, verifies each is executable, then moves them into place back-to-back in a fixed order (helpers first, orchestrator last), followed by a non-destructive post-install `test -x` layout check and a PATH hint.
- Fail-fast guards added: non-Linux host, missing `cargo`, and an unwritable destination all exit non-zero with a named, actionable message — the destination-usability guard runs before the build/staging steps, proven by fault injection (install → chmod 555 destination → re-run fails, prior binaries' `sha256sum` unchanged).
- A non-blocking kernel advisory (`uname -r` < 5.13) warns without failing, matching how `crates/sandbox` already negotiates the Landlock ABI down at runtime.
- `docs/GETTING-STARTED.md` gained a `## Install (Linux)` section (script command, manual-equivalent commands, the `cargo install --path cli/caprun` insufficiency warning naming `cli/caprun-exec-launcher` as a separate package), a corrected "Clone and build" binary list, a corrected CLI usage fence matching the real verb+flag shape, an extended sibling note, and a new "Common setup issues" entry for the exec-launcher failure mode.

## Task Commits

Each task was committed atomically:

1. **Task 1: End-to-end install — build, stage, co-locate three siblings, verify, and document the one command** - `3d885c8` (feat)
2. **Task 2: Fail-fast guards, explicit destination override, and the PATH/kernel advisories** - `14143c6` (feat)
3. **Task 3: Manual-equivalent commands, the cargo-install insufficiency warning, and corrected CLI/troubleshooting content** - `e0b172f` (docs)

_No metadata/SUMMARY commit yet — that is this commit._

## Files Created/Modified
- `scripts/install-linux.sh` - new Linux source-build installer; builds, stages, installs, and verifies the three required sibling binaries; OS/toolchain/writability guards; kernel advisory
- `docs/GETTING-STARTED.md` - new "## Install (Linux)" section, corrected "Clone and build" binary list, corrected CLI usage fence + runnable example, extended sibling note, new "Common setup issues" entry, "Next steps" pointer

## Decisions Made
- Moved the destination-usability guard (`mkdir -p` + `test -w`) to run before the build step, ahead of where Task 1's initial staging-time `mkdir -p` would have placed it, so the D-06 "no partial replacement on a refused re-install" guarantee holds structurally rather than by convention. The staging step's own `mkdir -p` was then redundant and removed in favor of a comment noting the destination is already verified.
- Wrote the writability check as `test -w "${DEST}"` (rather than `[ ! -w "${DEST}" ]`) so the guard is both correct and matches the plan's literal `test -w|\[ -w ` acceptance-criteria grep.
- Kept the manual-equivalent commands and the `cargo install --path cli/caprun` insufficiency warning inside the same `## Install (Linux)` H2 (not split across sections), matching D-01's "documentation remains sufficient without running the script" requirement in one place a reader can find it.

## Deviations from Plan

None — plan executed exactly as written. One correction was needed mid-execution: the initial destination-writability guard used `[ ! -w "${DEST}" ]`, which the plan's own acceptance-criteria grep (`grep -qE 'test -w|\[ -w '`) does not match (the literal `\[ -w ` pattern requires no `!` between the bracket and `-w`). Rewrote as `test -w "${DEST}"` before the Task 2 commit — caught and fixed during this plan's own verification pass, not left as a residual (Rule 1 — bug caught before commit, not a deviation from the plan's design).

## Issues Encountered
- The workspace's first `cargo build --workspace --release` on this dev box is a genuinely cold compile (~23 minutes, no prior release-profile cache) — well beyond the harness's single-command timeout. Ran it once in the background ahead of the scripted verify commands to warm `target/release/`; every subsequent `scripts/install-linux.sh` invocation in this session's verification then completed in ~1 second (`cargo build` recognized nothing had changed). This is a one-time cold-cache cost, not a script defect — a fresh design-partner checkout will see the same first-run cost, which the script's own progress echo ("Building release binaries...") already sets expectations for.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `scripts/install-linux.sh` and the `## Install (Linux)` doc section are complete and independently verified (script-level `bash -n`, dry-run installs, fault injection, forbidden-token greps, `git diff --exit-code -- Cargo.toml Cargo.lock crates cli`, `./scripts/check-invariants.sh`).
- Plan 52-02 (Operator Configuration Checklist in `docs/CONFIGURATION.md`) is unblocked — it does not depend on anything this plan changed beyond the shared `docs/` tree, and this plan made no code change to `crates/`, `cli/`, or the Cargo manifests.
- No blockers or concerns carried forward.

---
*Phase: 52-minimal-linux-packaging*
*Completed: 2026-08-11*
