---
phase: 32-process-exec-sink-broker-spawned-confined-child
plan: 06
subsystem: brokerd
tags: [rust, landlock, seccomp, process-exec, taint-model, linux-verification, cfg-linux-blindness]

# Dependency graph
requires:
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 05
    provides: "mint_from_exec + output_value_id wiring — the genuine-taint-anchor exec-output mint this plan's acceptance test drives"
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 04
    provides: "invoke_process_exec — the spawn+capture+two-phase-audit sink this plan's acceptance test AND confinement test drive directly"
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 03
    provides: "caprun-exec-launcher — the real, self-confining launcher binary this plan's exec_child_confinement.rs drives directly"
  - phase: 32-process-exec-sink-broker-spawned-confined-child
    plan: 02
    provides: "exec_child_ruleset / exec_child_filter — the Landlock/seccomp primitives this plan found and fixed two real gaps in"
provides:
  - "cli/caprun/tests/s9_process_exec_block.rs — EXEC-02/EXEC-03 acceptance: genuine (non-stapled) exec-output taint -> deterministic I2 Block, clean-allow control, tainted-command negative"
  - "crates/sandbox/tests/exec_child_confinement.rs — fs-escape denied, net-deny persists across execve (A6 empirically confirmed), benign write + legitimate execve succeed, driving the REAL caprun-exec-launcher"
  - "Two genuine Linux-only bug fixes required for ANY process.exec Linux verification to pass: a landlock 0.4.5 API-mismatch compile error in exec_child_ruleset, and an EXEC_CWD=\"\" chdir(\"\") bug in run_launcher"
  - "Two Landlock ruleset gaps closed in exec_child_ruleset: missing ReadDir (CPython stdlib import bootstrap) and missing MakeReg (creating a new file, distinct from WriteFile on an existing one)"
  - "All four Phase 32 success criteria (EXEC-01..04) proven per-requirement on REAL Linux via scripts/mailpit-verify.sh; REQUIREMENTS.md EXEC-01..04 marked Complete"
affects: ["Phase 34 (LIVE-01 composed live acceptance builds on this plan's proven-on-Linux primitives; LIVE-02 full-workspace regression must stay green against these Landlock ruleset fixes)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sibling-binary resolution must walk ancestor directories, not assume a single fixed .parent() hop: a cargo test integration-test binary lives one directory deeper (target/{debug,release}/deps/) than the production binary (target/{debug,release}/) — resolve_launcher_path() in both crates/brokerd/src/sinks/process_exec.rs and crates/sandbox/tests/exec_child_confinement.rs now walk up to 3 ancestor dirs"
    - "Landlock's ReadFile only gates opening a file's CONTENTS for read; ReadDir (directory listing, getdents/readdir) and MakeReg (creating a NEW file, distinct from WriteFile on an existing one) are separate rights that a narrow-allow exec-child ruleset must grant explicitly for any real-world target beyond a trivial single-binary like /bin/echo"
    - "In-process acceptance-test pattern (mirrors s9_acceptance.rs): drive the production mint_from_read/mint_from_exec + executor::submit_plan_node functions directly, inlining ONLY the private evaluate_plan_node_and_record's block-recording orchestration (Event::sink_blocked + append_event) — never re-implement taint/decision logic in the test itself"

key-files:
  created:
    - cli/caprun/tests/s9_process_exec_block.rs
    - crates/sandbox/tests/exec_child_confinement.rs
  modified:
    - crates/sandbox/src/landlock.rs
    - crates/brokerd/src/sinks/process_exec.rs
    - crates/sandbox/Cargo.toml
    - Cargo.lock
    - .planning/phases/32-process-exec-sink-broker-spawned-confined-child/32-VALIDATION.md
    - .planning/REQUIREMENTS.md

key-decisions:
  - "Routed the exec-output handle into a SECOND process.exec plan node's `command` arg (not email.send/body as the plan's read_first examples suggested) — email.send/body and file.create/path/contents are ALL role-checked (T2, DESIGN-slot-type-binding.md) and reject mint_from_exec's `origin_role = Some(\"exec_output\")` with a SlotTypeMismatch Denied BEFORE the taint/Block check ever runs; process.exec's own command/args are deliberately role-unconstrained (no legitimate exec command has an origin_role-producing mint site) — empirically discovered running the test's first draft in the Linux container."
  - "'Routed to a non-sensitive context' (scenario b) is represented by the exec-output handle simply never being submitted in any PlanNode — every arg on every sink CURRENTLY registered in sink_sensitivity.rs is either routing- or content-sensitive by design (I2 must Block everywhere a tainted value could redirect or exfiltrate), so there is no genuinely non-sensitive sink/arg pair to route into. Documented explicitly rather than fabricating a fake sink."
  - "Fixed crates/brokerd/src/sinks/process_exec.rs's launcher-path resolution and EXEC_CWD handling, and crates/sandbox/src/landlock.rs's exec_child_ruleset — all outside this plan's declared files_modified — because they are genuine bugs that block the mandatory Linux verification this exact plan is responsible for running (Rule 1/Rule 3). All four fixes were found via, and verified against, the real Colima+Docker Linux container, never guessed."
  - "Marked EXEC-01 complete in REQUIREMENTS.md alongside this plan's own frontmatter-declared EXEC-02/03/04 — the Per-Task Verification Map and Validation Sign-Off in 32-VALIDATION.md now show ALL FOUR genuinely proven on Linux (process_exec_spawn.rs + exec_child_confinement.rs + s9_process_exec_block.rs), and this is the phase's last plan; leaving EXEC-01 'Pending' would misrepresent a demonstrably complete state."

patterns-established: []

requirements-completed: [EXEC-01, EXEC-02, EXEC-03, EXEC-04]

coverage:
  - id: D1
    description: "EXEC-03 acceptance: a trusted process.exec Allows; invoke_process_exec spawns the real confined launcher and durably appends process_exited; mint_from_exec mints the captured output rooted on that SAME event id (confirmed via a fresh DB lookup); routing the handle into a second process.exec's command arg deterministically Blocks with anchor.provenance_chain[0] == anchor.read_event_id == the process_exited event id (non-stapled) and verify_chain true"
    requirement: "EXEC-03"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_process_exec_block.rs#linux::s9_process_exec_genuine_taint_block (Linux-only, run via scripts/mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D2
    description: "CLEAN ALLOW control: a benign process.exec Allows; its unconditionally-tainted output, never routed anywhere, causes no Block"
    requirement: "EXEC-03"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_process_exec_block.rs#linux::s9_process_exec_clean_allow_unrouted_output_causes_no_block"
        status: pass
    human_judgment: false
  - id: D3
    description: "TAINTED-COMMAND negative: a process.exec whose command is itself untrusted-tainted (mint_from_read) Blocks before any spawn — no process_exited event is ever appended"
    requirement: "EXEC-03"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_process_exec_block.rs#linux::s9_process_exec_tainted_command_blocks_before_spawn"
        status: pass
    human_judgment: false
  - id: D4
    description: "Exec-child confinement: fs-escape (write outside workspace + system paths) denied; outbound socket(AF_INET) denied, empirically confirming A6 (net-deny persists across the launcher's own execve); benign in-workspace write+read AND the legitimate target execve both succeed — all driving the REAL caprun-exec-launcher binary with an unconfined /usr/bin/python3 target (proving inheritance, not self-application)"
    requirement: "EXEC-04"
    verification:
      - kind: integration
        ref: "crates/sandbox/tests/exec_child_confinement.rs (3 Linux tests, run via scripts/mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Mandatory Linux compile-check + invariant gates: check-invariants.sh 4/4 PASS (incl. Gate-3 mint_from_exec( line); container cargo build --tests --keep-going enumerates zero cfg(linux) errors; scoped Linux test run (exec_child_confinement 4/4, process_exec_spawn 3/3, s9_process_exec_block 4/4) all pass with real exit 0 captured before any pipe"
    verification:
      - kind: other
        ref: "bash scripts/check-invariants.sh; MAILPIT_VERIFY_CMD='cargo build --workspace && cargo build --tests --keep-going' bash scripts/mailpit-verify.sh; MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p sandbox --test exec_child_confinement && cargo test -p brokerd process_exec && cargo test -p caprun --test s9_process_exec_block' bash scripts/mailpit-verify.sh"
        status: pass
    human_judgment: false

duration: 45min
completed: 2026-07-17
status: complete
---

# Phase 32 Plan 06: Linux acceptance proof + mandatory compile-check Summary

**Wrote and ran, ON REAL LINUX (Colima+Docker, `rust:1`, `seccomp=unconfined`), the phase's per-requirement acceptance tests for all four EXEC-01..04 success criteria — and in the process found and fixed FOUR genuine bugs (two blocking compile/runtime errors in prior plans' code, two missing Landlock rights) that a Mac-only build could never have caught.**

## Performance

- **Duration:** 45 min
- **Started:** 2026-07-17T18:26:00Z (approx, continuation of Phase 32 session)
- **Completed:** 2026-07-17T19:08:25Z
- **Tasks:** 3 completed (plus one prerequisite fix commit)
- **Files modified:** 8 (2 created, 6 modified)

## Accomplishments

- Ran the mandatory Linux container FIRST (before writing any new test), per CLAUDE.md's cfg-linux-blindness guard — and immediately hit a genuine compile error in `crates/sandbox/src/landlock.rs::exec_child_ruleset` (32-02's code, never before compiled on Linux): `path_beneath_rules` (landlock 0.4.5) takes path-LIKE items and resolves them to a `PathFd` INTERNALLY; the code passed an already-constructed `PathFd`, which does not implement `AsRef<Path>`. Fixed by passing `workspace_root` (a `&Path`) directly.
- Found and fixed a second genuine bug: `crates/brokerd/src/sinks/process_exec.rs::run_launcher` unconditionally set `EXEC_CWD=<cwd.unwrap_or("")>`, so every process.exec invocation with no explicit `cwd` arg set `EXEC_CWD` to an EMPTY STRING (not unset) — the launcher then called `Command::current_dir("")` -> `chdir("")` -> ENOENT, failing EVERY exec with a misleading "target not found" error. Fixed by only setting `EXEC_CWD` when `cwd` is `Some`. Also hardened `invoke_process_exec`'s sibling-binary resolution (`resolve_launcher_path`): a single fixed `.parent()` hop only works inside the production `caprun` binary; a `cargo test` integration-test binary lives one directory deeper (`target/debug/deps/`), so the launcher was never found when `invoke_process_exec` was exercised directly by a test (32-04's `process_exec_spawn.rs` AND this plan's new `s9_process_exec_block.rs` both do this).
- Wrote `cli/caprun/tests/s9_process_exec_block.rs` (Linux-gated, 4 tests): the EXEC-03 genuine-taint acceptance (routing the exec-output handle into a SECOND process.exec's `command` arg — NOT email.send/body, after discovering email.send/body's role-check rejects `origin_role = Some("exec_output")` before the taint check ever runs), the clean-allow control, and the tainted-command-Blocks-before-spawn negative.
- Wrote `crates/sandbox/tests/exec_child_confinement.rs` (Linux-gated, 3 tests) driving the REAL `caprun-exec-launcher` binary with an unconfined `/usr/bin/python3` target — and found TWO real Landlock ruleset gaps in `exec_child_ruleset` in the process: missing `AccessFs::ReadDir` (CPython's own stdlib import bootstrap needs directory listing, not just file-content reads — `/bin/echo` never exposed this) and missing `AccessFs::MakeReg` (creating a brand-new file requires this DISTINCT right from `WriteFile`, which only governs existing files). Both fixed and empirically re-verified.
- Ran, in order, capturing real exit codes (never laundered through a pipe): `check-invariants.sh` (4/4 PASS), the mandatory Linux compile-check (`cargo build --tests --keep-going` via `scripts/mailpit-verify.sh`, zero cfg(linux) errors), and the scoped Linux test run (`exec_child_confinement` 4/4, `process_exec_spawn` 3/3, `s9_process_exec_block` 4/4, all pass).
- Updated `32-VALIDATION.md` (Per-Task Verification Map all ✅, Wave 0 Requirements + Validation Sign-Off checked, `nyquist_compliant`/`wave_0_complete` set true) and marked EXEC-01..04 Complete in `REQUIREMENTS.md`.

## Task Commits

Each task was committed atomically:

1. **Prerequisite fix: two Linux-only cfg(linux) bugs blocking process.exec verification** - `61d90c3` (fix)
2. **Task 1: EXEC-02/EXEC-03 acceptance test** - `57d30e7` (test)
3. **Task 2: exec-child confinement negative-assertion test (+ Landlock ruleset fixes)** - `b23fcff` (test)
4. **Task 3: mandatory Linux compile-check + invariant gates** - `aa03cfe` (docs)
5. **Requirements traceability update** - `7426568` (docs)

## Files Created/Modified

- `cli/caprun/tests/s9_process_exec_block.rs` - new; EXEC-02/EXEC-03 per-requirement acceptance (Linux-gated, 4 tests + guard)
- `crates/sandbox/tests/exec_child_confinement.rs` - new; exec-child confinement negative-assertion + capability tests (Linux-gated, 3 tests + guard)
- `crates/sandbox/src/landlock.rs` - `exec_child_ruleset`: fixed the `PathFd`/`path_beneath_rules` compile error; added `AccessFs::ReadDir` + `AccessFs::MakeReg`
- `crates/brokerd/src/sinks/process_exec.rs` - `run_launcher`: only sets `EXEC_CWD` when `cwd` is `Some`; new `resolve_launcher_path` helper walks ancestor dirs
- `crates/sandbox/Cargo.toml` - added `serde_json` dev-dependency (EXEC_ARGS_JSON encoding for the confinement test)
- `Cargo.lock` - updated (serde_json now reachable from sandbox's dev-dependency graph)
- `.planning/phases/32-process-exec-sink-broker-spawned-confined-child/32-VALIDATION.md` - Per-Task Verification Map, Wave 0 Requirements, Validation Sign-Off all updated/checked; frontmatter `nyquist_compliant`/`wave_0_complete` set true
- `.planning/REQUIREMENTS.md` - EXEC-01..04 marked Complete

## Decisions Made

- **Routed the exec-output handle into a second `process.exec`'s `command` arg**, not `email.send`/`body` as the plan's read_first examples suggested — `email.send`/`body` and `file.create`/`path`/`contents` are ALL role-checked (T2, `DESIGN-slot-type-binding.md`) and reject `mint_from_exec`'s `origin_role = Some("exec_output")` with a `SlotTypeMismatch` `Denied` BEFORE the taint/Block check ever runs; `process.exec`'s own `command`/`args` are deliberately role-unconstrained. Discovered empirically running the test's first draft in the Linux container.
- **"Routed to a non-sensitive context" is represented by the handle never being submitted anywhere** — every arg on every currently-registered sink is either routing- or content-sensitive by design; there is no genuinely non-sensitive sink/arg pair to route into without fabricating one. Documented explicitly in the test's doc comments rather than inventing a fake sink.
- **Fixed four bugs outside this plan's declared `files_modified`** (`crates/brokerd/src/sinks/process_exec.rs`, `crates/sandbox/src/landlock.rs`) because all four are genuine bugs blocking the mandatory Linux verification this exact plan owns (Rule 1/Rule 3). All were found via, and re-verified against, the real Colima+Docker container — never guessed or inferred from reading code alone.
- **Marked EXEC-01 complete alongside EXEC-02/03/04** — this plan's own frontmatter only listed EXEC-02/03/04, but the Validation Sign-Off's per-task map now shows all four genuinely proven on Linux, and this is the phase's last plan; leaving EXEC-01 "Pending" would misrepresent a demonstrably complete state.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] landlock 0.4.5 `PathFd`/`path_beneath_rules` compile error in `exec_child_ruleset`**
- **Found during:** Task 3 preparation (mandatory Linux container run, BEFORE writing new tests)
- **Issue:** `path_beneath_rules<I, P, A>` requires `P: AsRef<Path>` and resolves each item to a `PathFd` INTERNALLY; the code pre-constructed a `PathFd` and passed it in, which does not implement `AsRef<Path>` — a genuine `E0277` compile error that never surfaced on Mac (the whole function is a `#[cfg(not(target_os="linux"))]` no-op stub there).
- **Fix:** Pass `workspace_root` (a `&Path`) directly to `path_beneath_rules`.
- **Files modified:** `crates/sandbox/src/landlock.rs`
- **Verification:** `cargo build --workspace` in the `rust:1` container, before/after diff.
- **Committed in:** `61d90c3`

**2. [Rule 1 - Bug] `EXEC_CWD=""` -> `chdir("")` -> ENOENT in `run_launcher`**
- **Found during:** Task 3 preparation (running 32-04's own `process_exec_spawn.rs` in the Linux container to establish a clean baseline before adding new tests)
- **Issue:** `run_launcher` set `EXEC_CWD=cwd.unwrap_or("")` unconditionally — when no `cwd` arg was supplied, this set the env var to an EMPTY STRING (not unset); the launcher's `main.rs` reads it as `Some("")` and calls `Command::current_dir("")`, which performs `chdir("")` — POSIX `chdir("")` fails with `ENOENT`, making EVERY process.exec invocation with no explicit `cwd` fail with a misleading "target not found" error attributed to the target command.
- **Fix:** Only call `cmd.env("EXEC_CWD", dir)` when `cwd` is `Some`.
- **Files modified:** `crates/brokerd/src/sinks/process_exec.rs`
- **Verification:** `cargo test -p brokerd --test process_exec_spawn` in the container: 0/3 passing before, 3/3 passing after.
- **Committed in:** `61d90c3`

**3. [Rule 1 - Bug] `invoke_process_exec`'s sibling-binary resolution failed inside a test binary**
- **Found during:** Same investigation as #2 (the first failure mode, before finding the `EXEC_CWD` bug underneath it)
- **Issue:** `current_exe().parent().join("caprun-exec-launcher")` assumes a single fixed depth; a `cargo test` integration-test binary lives at `target/debug/deps/<name>-<hash>`, one directory deeper than the production `caprun` binary (`target/debug/`) — so this resolution NEVER found the launcher when `invoke_process_exec` was called directly by a test (both 32-04's `process_exec_spawn.rs` and this plan's new `s9_process_exec_block.rs`).
- **Fix:** New `resolve_launcher_path()` helper walks up to 3 ancestor directories from `current_exe()`'s parent, returning the first that contains `caprun-exec-launcher`.
- **Files modified:** `crates/brokerd/src/sinks/process_exec.rs`
- **Verification:** Same container re-run as #2.
- **Committed in:** `61d90c3`

**4. [Rule 1 - Bug] Two missing Landlock rights in `exec_child_ruleset`'s system-path grant**
- **Found during:** Task 2 (writing `exec_child_confinement.rs`, driving a real `/usr/bin/python3` target)
- **Issue:** (a) `system_access = ReadFile | Execute` lacked `ReadDir` — CPython's stdlib import bootstrap needs directory listing while resolving `sys.path` candidates, failing with `Fatal Python error: Failed to import encodings module`; `/bin/echo` never exposed this (no runtime directory enumeration). (b) `workspace_access = ReadFile | WriteFile` lacked `MakeReg` — Landlock gates CREATING a brand-new file via a right DISTINCT from `WriteFile` (which only covers opening/truncating an EXISTING file); a benign `open(new_path, 'w')` failed `PermissionError` even with `WriteFile` granted.
- **Fix:** Added `AccessFs::ReadDir` to `system_access` and `AccessFs::MakeReg` to `workspace_access`.
- **Files modified:** `crates/sandbox/src/landlock.rs`
- **Verification:** All 3 `exec_child_confinement.rs` Linux tests pass after both fixes; re-ran `confinement_integration.rs` (worker deny-all, unaffected) and `process_exec_spawn.rs` to confirm no regression.
- **Committed in:** `b23fcff`

---

**Total deviations:** 4 auto-fixed (4 bugs — 2 blocking compile/runtime errors, 2 missing Landlock rights)
**Impact on plan:** All four fixes were genuine, necessary prerequisites for the mandatory Linux verification this exact plan owns — without them, NEITHER the new tests NOR 32-04's own pre-existing Linux test could have passed. No scope creep beyond what was required to make the Linux container run green; each fix is narrowly scoped to the exact broken code path.

## Issues Encountered

**Cwd-drift self-correction (harness-level, not a code issue):** Early in this session, several `Bash` tool calls used `cd /Users/benlamm/Workspace/AgentOS && ...` (the MAIN checkout) instead of operating in the assigned worktree — a first `Edit` attempt correctly failed with a worktree-boundary error, which surfaced the drift. All subsequent commands (including every fix, test write, and the mandatory Linux verification) were run from the correct worktree root (`/Users/benlamm/Workspace/AgentOS/.claude/worktrees/agent-a1ae1f548eb3b985a`), confirmed via `pwd`/`git rev-parse --show-toplevel` before proceeding. No file writes or commits happened against the main checkout — the early commands were read-only greps/builds in the main tree (harmless, target/ is gitignored there too).

## User Setup Required

None - no external service configuration required. Colima was already running; this plan used the existing Docker daemon.

## Next Phase Readiness

- All four Phase 32 success criteria (EXEC-01..04) are now genuinely proven per-requirement on REAL Linux: broker-spawned kernel-confined child (EXEC-01/EXEC-04), genuine non-stapled exec-output taint mint (EXEC-02), deterministic I2 Block with an unbroken audit-DAG edge (EXEC-03), fail-closed schema + durable spawn/exit audit (EXEC-04).
- `exec_child_ruleset`'s Landlock rights (`ReadFile | ReadDir | Execute` on system paths, `ReadFile | WriteFile | MakeReg` on the workspace) are now empirically validated against a REAL, non-trivial target (`/usr/bin/python3`, not just `/bin/echo`) — Phase 34's composed live acceptance can build on this without re-discovering the same gaps.
- `invoke_process_exec`'s sibling-binary resolution is now robust to being called from either the production `caprun` binary OR a `cargo test` integration-test binary — a latent trap for any FUTURE test that exercises this function directly is now closed.
- No blockers. `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (Mac, full suite, no regressions), and `./scripts/check-invariants.sh` (4/4 gates) are all green on Mac; the mandatory Linux container run (`scripts/mailpit-verify.sh`) is green for both the compile-check and the scoped test run. Full composed live acceptance (LIVE-01, all criteria in ONE run) remains Phase 34.

---
*Phase: 32-process-exec-sink-broker-spawned-confined-child*
*Completed: 2026-07-17*

## Self-Check: PASSED

All created/modified files verified present on disk; all six commit hashes
(`61d90c3`, `57d30e7`, `b23fcff`, `aa03cfe`, `7426568`, `9b20374`) verified
present in `git log`. `cargo build --workspace`, `cargo test --workspace
--no-fail-fast` (Mac, full suite), and `./scripts/check-invariants.sh` (4/4
gates) all green on Mac after the final commit. The mandatory Linux
container run (`scripts/mailpit-verify.sh`) is green for both the compile
check and the scoped test run (exec_child_confinement 4/4, process_exec_spawn
3/3, s9_process_exec_block 4/4), real exit 0 captured before any pipe.
