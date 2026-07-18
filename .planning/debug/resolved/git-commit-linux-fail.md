---
status: resolved
trigger: "Phase-36 git.commit sink 3 Linux behavioral tests fail on first real Linux run: git commit runs in confined child but commit does NOT happen (HEAD did not advance; committed author empty). Sink reports success despite git non-zero exit."
created: 2026-07-18T00:00:00Z
updated: 2026-07-18T00:00:00Z
---

## Current Focus

hypothesis: exec_child_ruleset grants only ReadFile|WriteFile|MakeReg on WorkspaceRoot; git commit needs MakeDir (.git/objects/xx/), RemoveFile+Refer (index.lock -> index rename), so git fails with EACCES. SEPARATELY the sink ignores exit_status and reports success on git failure.
test: capture confined child combined_output (git stderr) in-container to see the real error
expecting: git stderr shows a Permission denied / cannot create / cannot lock ref error pointing at a missing Landlock right
next_action: add temp eprintln of combined_output in test 1, build+run container, read real git error

## Symptoms

expected: git commit runs in confined child, HEAD advances to a real commit, author=caprun
actual: HEAD does not advance (no commit), committed author empty; sink returns Ok anyway
errors: (to capture) — git stderr is discarded in combined_output; sink binds _exit_status and ignores it
reproduction: run crates/brokerd/tests/git_commit_spawn.rs 3 linux tests in Linux container (cargo build --workspace FIRST)
started: first real Linux execution (previously only compile-checked — cfg-linux-test-blindness)

## Eliminated

## Evidence

- timestamp: init
  checked: crates/brokerd/src/sinks/git_commit.rs match arm
  found: Ok((_exit_status, combined_output)) — exit_status bound to _ and ignored; appends process_exited as success regardless of git exit code
  implication: confirmed the exit-code bug by inspection; git IS failing but sink reports success. Real git error is in discarded combined_output.

- timestamp: init
  checked: crates/sandbox/src/landlock.rs exec_child_ruleset
  found: workspace_access = ReadFile | WriteFile | MakeReg. No MakeDir, RemoveFile, RemoveDir, or Refer.
  implication: git commit creates .git/objects/xx/ dirs (MakeDir) and renames index.lock->index (Refer) and removes locks (RemoveFile) — likely blocked. Leading hypothesis for the git failure.

- timestamp: container-run-1
  checked: in-container `cargo build --workspace` then `cargo test -p brokerd --test git_commit_spawn` with temp eprintln of combined_output
  found: "[caprun-exec-launcher] Landlock exec_child_ruleset status: FullyEnforced" then "fatal: could not open '/dev/null' for reading and writing: Permission denied". All 3 tests fail; env-clear test shows author="" (no commit made).
  implication: PRIMARY root cause is /dev/null, NOT MakeDir. git opens /dev/null (GIT_CONFIG_GLOBAL=/dev/null and core.hooksPath=/dev/null neutralization) but /dev is not in the exec-child Landlock allow-list → EACCES → git fatal → non-zero exit → no commit. Container kernel is V3 (FullyEnforced), so Refer/Truncate available here. MakeDir/Refer is a probable SECOND layer once /dev/null is fixed (git creates .git/objects/xx/ dirs, .git/logs/, and rename-locks) — fixing both in one pass.

reasoning_checkpoint:
  hypothesis: "git commit fails because the exec-child Landlock ruleset denies /dev/null (used by the neutralization env) AND lacks the create/list/remove/rename rights git needs under the workspace .git dir; the sink then falsely reports success because it ignores the non-zero exit_status."
  confirming_evidence:
    - "Direct observation: git stderr = 'fatal: could not open /dev/null for reading and writing: Permission denied' from the confined child"
    - "exec_child_ruleset allow-list = /usr,/bin,/lib,/lib64 (system) + workspace ReadFile|WriteFile|MakeReg only — /dev absent, and no ReadDir/MakeDir/RemoveFile/Refer on workspace"
    - "sink match arm binds _exit_status and appends process_exited unconditionally"
  falsification_test: "after granting /dev/null + fuller workspace write/list rights, the 3 tests still fail → hypothesis wrong. If they pass → confirmed."
  fix_rationale: "grant the minimal device (/dev/null r/w) + the fuller write/list right set scoped to workspace that git legitimately needs; separately gate the sink success on exit_status.success(), appending a terminal process_spawn_failed FIRST on non-zero exit (P33/P34: terminal EVENT before terminal state)."
  blind_spots: "whether git needs additional paths beyond /dev/null and workspace (e.g. /dev/tty, /etc); ABI negotiation on <5.19 kernels for Refer/Truncate (mitigated: rename-within-dir needs no Refer, crate negotiates down)."

## Resolution

root_cause: |
  Two defects, both in security-boundary code, hidden by cfg-linux-test-blindness
  (compile-only until first real Linux run):
  1. PRIMARY (the blocker): the exec-child Landlock ruleset
     (crates/sandbox/src/landlock.rs::exec_child_ruleset) allow-list was
     {/usr,/bin,/lib,/lib64 read+list+exec} + {workspace read+write+makereg}. git
     commit under the confined child needs (a) /dev/null (opened O_RDWR by the
     GIT_CONFIG_GLOBAL=/dev/null + core.hooksPath=/dev/null neutralization),
     (b) /dev/urandom (CSPRNG for random temp-object filenames), and (c) fuller
     workspace rights (ReadDir to enumerate .git/objects & refs, MakeDir for
     .git/objects/xx & .git/logs, RemoveFile/Refer for lock renames). None were
     granted → git died: first "fatal: could not open '/dev/null' ... Permission
     denied", then "unable to get random bytes for temporary file: Permission
     denied" / "insufficient permission for adding an object ... .git/objects".
     (seccomp ALLOWS getrandom — mismatch-Allow — so entropy failure was Landlock
     /dev/urandom, NOT seccomp; net-deny left untouched.)
  2. SECONDARY (false success): invoke_git_commit bound `_exit_status` and
     appended a success `process_exited` (minting exec taint) regardless of git's
     exit code — so a FAILED commit reported success and the real git stderr was
     discarded in combined_output.
fix: |
  crates/sandbox/src/landlock.rs — exec_child_ruleset: add /dev/null (ReadFile|
  WriteFile) + /dev/urandom,/dev/random (ReadFile) single-file carve-outs (no
  ReadDir/MakeReg/Execute on /dev → no escape); widen workspace_access to the
  fuller non-exec set (ReadFile|WriteFile|ReadDir|MakeReg|MakeDir|RemoveFile|
  RemoveDir|Refer|Truncate) still scoped to workspace_root ONLY, still no Execute.
  Refer(V2)/Truncate(V3) negotiate away on <5.19 kernels (crate best-effort);
  git's lock renames are same-directory so need no Refer — 5.13 floor preserved.
  crates/brokerd/src/sinks/git_commit.rs — gate success on exit_status.success();
  a non-zero git exit (or spawn Err) folds into ONE failure path that appends a
  terminal process_spawn_failed event FIRST (P33/P34: terminal EVENT before
  terminal state), then returns Err with the git output — never a false
  process_exited, never minted taint on a failed commit.
verification: |
  In Linux container (Colima, kernel V3/FullyEnforced, cargo build --workspace
  first): git_commit_spawn 3/3 PASS. No regression: sandbox exec_child_confinement
  4/4 (fs-escape to /tmp still DENIED, /etc/passwd blocked, net-deny persists
  across execve, benign workspace write + legit execve succeed), confinement_
  integration 4/4, process_exec_spawn 3/3. Host (macOS): cargo build --workspace
  + cargo test -p brokerd green (git_commit 0 tests = expected cfg-linux-blindness).
  ./scripts/check-invariants.sh exit 0 (all 5 gates PASS — no raw EffectRequest,
  no new mint site, I2/plan-node path untouched).
files_changed: [crates/sandbox/src/landlock.rs, crates/brokerd/src/sinks/git_commit.rs]
