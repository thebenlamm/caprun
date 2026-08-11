---
phase: 52-minimal-linux-packaging
reviewed: 2026-08-11T15:42:42Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - scripts/install-linux.sh
  - docs/GETTING-STARTED.md
  - docs/CONFIGURATION.md
  - README.md
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 52: Code Review Report

**Reviewed:** 2026-08-11T15:42:42Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

`scripts/install-linux.sh` was reviewed as executable code and traced end-to-end (argument parsing, destination resolution, build, stage, install, post-install check, kernel advisory), including two behaviors verified experimentally rather than by inspection alone (`mv` into a pre-existing same-named directory, and the effect of `--dest ""`). The core staged-rename design is sound: nothing enters `DEST` until all three binaries are staged and proven executable, the fixed helpers-first/orchestrator-last move order is correctly implemented, the `EXIT` trap only ever removes the (by then empty) staging directory on the success path, and the script does not touch `crates/`, `cli/`, `Cargo.toml`, or `Cargo.lock`, does not escalate privileges, and does not invoke a package manager or mutate git state — all compliant with CLAUDE.md's phase constraints. `--dest` missing-value and unrecognized-flag cases both fail closed as documented.

Two real gaps were found in the safety guarantees the script advertises: the post-install executability check can pass even when a binary was silently nested inside a pre-existing same-named directory (breaking the required binary co-location), and an explicitly empty `--dest ""` is silently treated as "not given" and falls back to the default destination instead of being rejected — the exact "malformed invocation silently does the wrong thing" failure mode this class of installer needs to avoid. A third, minor edge case in the `mkdir -p`/writability guard produces a misleading error message (not a functional failure) when the destination path already exists as a plain file.

The three documentation files were cross-checked against the actual CLI parser (`cli/caprun/src/main.rs`), the worker environment-variable contract (`cli/caprun/src/worker.rs`), the SMTP default constants (`crates/brokerd/src/sinks/email_smtp.rs`), and the two source-code line references cited in the script's own header comment (`cli/caprun/src/main.rs:607-611`, `crates/brokerd/src/sinks/process_exec.rs:736-756`) — all matched. No wrong commands, no real credentials, no security-guidance errors, and no claim that the layout check proves confinement (`docs/GETTING-STARTED.md` explicitly disclaims this: "This is a layout check, not a security proof"). One documentation accuracy issue was found: `docs/GETTING-STARTED.md`'s "Manual equivalent" section claims the plain `install` command is "the script's entire behavior," which overstates the equivalence — the manual one-shot `install` has no staging/atomicity protection against a mid-copy interruption, unlike the script.

## Warnings

### WR-01: Post-install layout check can report "PASS" over a binary silently nested inside a same-named directory

**File:** `scripts/install-linux.sh:156-166`
**Issue:** If `${DEST}/caprun-worker` (or either of the other two binary names) already exists as a *directory* rather than a file — e.g. left over from a previous manual mistake or another tool — `mv -f "${STAGE_DIR}/caprun-worker" "${DEST}/caprun-worker"` does not overwrite it. POSIX `mv` with an existing-directory target moves the source *into* that directory under its original basename (i.e. produces `${DEST}/caprun-worker/caprun-worker` — actually `${DEST}/caprun-worker/<stage-basename>`, verified below), silently succeeding (exit 0) rather than erroring. The post-install check on line 162 (`[ ! -x "${DEST}/${bin}" ]`) then also silently passes, because `test -x` is true for any directory the invoking user can traverse — it does not distinguish "executable regular file" from "searchable directory." The script prints `PASS — all 3 required binaries installed and executable in ${DEST}` even though `caprun-worker` is not actually a binary at that path, breaking the co-location invariant the script exists to guarantee (`caprun`'s worker lookup at `cli/caprun/src/main.rs:607-611` is a single `current_exe().parent()` hop with no fallback, so it will fail at spawn time with a bare OS error despite the script's own success message).

Verified experimentally:
```
$ mkdir -p dest/caprun-worker && touch src && chmod 755 src
$ mv -f src dest/caprun-worker; echo "mv exit: $?"
mv exit: 0
$ ls dest/caprun-worker
src                      # nested under the original name, not replacing the dir
$ [ -x dest/caprun-worker ] && echo TRUE
TRUE                     # the "is it a binary" check passes on the directory
```
**Fix:** Tighten the post-install check to also assert regular-file-ness, and ideally guard the `mv` targets too:
```bash
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ ! -f "${DEST}/${bin}" ] || [ ! -x "${DEST}/${bin}" ]; then
        echo "FAIL — ${DEST}/${bin} missing, not a regular file, or not executable" >&2
        exit 1
    fi
done
```
Optionally also pre-flight-reject a pre-existing directory collision before staging even begins (`[ -d "${DEST}/${bin}" ] && FAIL`), so the error surfaces before the build/stage work rather than after.

### WR-02: `--dest ""` is silently accepted and falls back to the default instead of failing closed

**File:** `scripts/install-linux.sh:76-84, 111`
**Issue:** The argument parser's missing-value guard (`if [ $# -lt 2 ]`) only rejects `--dest` when no second token is present at all; it does not reject an explicitly empty string. `bash scripts/install-linux.sh --dest ""` passes that guard (two tokens are present), sets `DEST_FLAG=""`, and then `DEST="${DEST_FLAG:-${INSTALL_DEST:-$HOME/.local/bin}}"` on line 111 treats the empty string the same as "unset" (`:-` triggers on empty *or* unset), silently discarding the user's explicit `--dest ""` and installing to the default location instead of failing with an error. This is precisely the "malformed invocation silently does the wrong thing rather than failing closed" pattern the script is otherwise careful to avoid (e.g. the missing-value and unrecognized-flag cases both do fail closed).

Verified experimentally:
```
$ set -- --dest ""
# ... parser loop ...
DEST_FLAG=[]
Resolved DEST=[/home/ben/.local/bin]   # silently defaulted instead of erroring
```
**Fix:** Reject an empty `--dest` value explicitly:
```bash
--dest)
    if [ $# -lt 2 ] || [ -z "$2" ]; then
        echo "FAIL — --dest requires a non-empty directory argument" >&2
        usage
        exit 1
    fi
    DEST_FLAG="$2"
    shift 2
    ;;
```

### WR-03: "Manual equivalent" in GETTING-STARTED.md overstates equivalence with the script

**File:** `docs/GETTING-STARTED.md:75-81`
**Issue:** The doc states: "these commands are the script's entire behavior, so this walkthrough is sufficient even if you don't want to run the repository script," then shows a single `install -m 755 target/release/caprun target/release/caprun-worker target/release/caprun-exec-launcher "$HOME/.local/bin/"` call. This is not actually equivalent to the script's behavior: the script stages all three binaries into a temp directory first and proves each is executable *before* any of them touch the real destination, then moves them into place back-to-back — specifically so a build or copy failure, or a process kill mid-sequence, cannot leave a **freshly-partial** set in the destination (this exact guarantee is called out three paragraphs earlier in the same doc, and in the script's own header comment). The one-shot `install src1 src2 src3 dest/` manual form copies the three files sequentially with no staging step; if it is killed after copying one or two of the three, `${HOME}/.local/bin` is left with a partial/mixed set of binaries with no equivalent protection. Calling the manual commands "the script's entire behavior" therefore overstates what the manual path actually guarantees.
**Fix:** Qualify the claim, e.g.: "these commands reproduce the script's end state on a successful, uninterrupted run — the script additionally stages files before installing them, so it does not risk leaving a partial set behind if interrupted mid-copy; the manual form here does not have that protection."

## Info

### IN-01: Misleading error message when `--dest` already exists as a non-directory file

**File:** `scripts/install-linux.sh:116-120`
**Issue:** If `${DEST}` already exists as a regular file (not a directory), `mkdir -p "${DEST}"` fails (suppressed by `2>/dev/null || true`), and the subsequent `[ ! -d "${DEST}" ]` check is true, so the script exits with: `"FAIL — ${DEST} is not writable; ..."`. The script does correctly fail closed here (verified: `mkdir -p` on an existing plain file returns exit 1, and `-d` correctly reports false), but the error message is inaccurate for this specific case — the problem is "`${DEST}` exists and is not a directory," not "not writable." A user hitting this will be pointed at the wrong remediation (permissions) instead of the actual one (remove/rename the conflicting file, or pick a different `--dest`).
**Fix:**
```bash
if [ -e "${DEST}" ] && [ ! -d "${DEST}" ]; then
    echo "FAIL — ${DEST} exists and is not a directory; choose a different --dest" >&2
    exit 1
fi
if [ ! -d "${DEST}" ] || ! test -w "${DEST}"; then
    echo "FAIL — ${DEST} is not writable; ..." >&2
    exit 1
fi
```

### IN-02: Relative `--dest` paths resolve against the repo root, not the caller's working directory

**File:** `scripts/install-linux.sh:65-70, 76-84, 111`
**Issue:** The script does `cd "${REPO_ROOT}"` near the top (line 70) before argument-derived `DEST` is ever used. A relative `--dest` value (e.g. `--dest ../mybin`) is therefore resolved relative to the repository root, not relative to the directory the user was in when they invoked the script — which matters if the script is invoked with a path (`bash /some/path/scripts/install-linux.sh --dest ../mybin`) from a directory other than the repo root. The script's own usage text and `--help` output don't mention this. This is a minor surprise rather than a bug given the documented "run from the workspace root" convention, but a relative `--dest` silently changes meaning if that convention isn't followed.
**Fix:** Either resolve `--dest` to an absolute path *before* the `cd "${REPO_ROOT}"` (capture `$PWD` first), or note the relative-path resolution behavior in `usage()`/`--help`.

---

_Reviewed: 2026-08-11T15:42:42Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
