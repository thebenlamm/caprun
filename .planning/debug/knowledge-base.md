# GSD Debug Knowledge Base

Resolved debug sessions. Used by `gsd-debugger` to surface known-pattern hypotheses at the start of new investigations.

---

## git-commit-linux-fail — git.commit confined child cannot commit (Landlock rights + false-success exit code)
- **Date:** 2026-07-18
- **Error patterns:** git commit, HEAD did not advance, could not open '/dev/null' for reading and writing, Permission denied, unable to get random bytes for temporary file, insufficient permission for adding an object to repository database, .git/objects, Landlock, exec_child_ruleset, confined child, EACCES
- **Root cause:** exec-child Landlock allow-list ({/usr,/bin,/lib,/lib64}+workspace{read,write,makereg}) omitted the paths/rights git commit needs — /dev/null (config/hooks neutralization opens it O_RDWR), /dev/urandom (CSPRNG temp-object filenames), and the fuller workspace set (ReadDir/MakeDir/RemoveFile/Refer for .git object dirs + lock renames). seccomp allows getrandom, so the entropy failure was Landlock /dev/urandom, not seccomp. Separately, invoke_git_commit ignored the child exit status and reported success (minting exec taint) on a failed commit. Both hidden by cfg-linux-test-blindness (compile-only until first real Linux run).
- **Fix:** Landlock exec_child_ruleset: add /dev/null (r/w) + /dev/urandom,/dev/random (read) single-file carve-outs (no ReadDir/MakeReg/Execute on /dev); widen workspace_access to ReadFile|WriteFile|ReadDir|MakeReg|MakeDir|RemoveFile|RemoveDir|Refer|Truncate, still workspace-scoped, still no Execute (Refer/Truncate negotiate away below kernel 5.19; same-dir lock renames need no Refer). git_commit sink: gate success on exit_status.success(); non-zero exit or spawn Err appends terminal process_spawn_failed FIRST (P33/P34), then errs — no false process_exited, no minted taint on failure.
- **Files changed:** crates/sandbox/src/landlock.rs, crates/brokerd/src/sinks/git_commit.rs
---
