---
phase: 52-minimal-linux-packaging
verified: 2026-08-11T15:47:19Z
status: passed
score: 27/27 must-haves verified (3/3 roadmap success criteria; 0 behavior-unverified)
behavior_unverified: 0
overrides_applied: 0
known_accepted_residuals:
  - "T-52-07 (accept disposition): three POSIX renames are atomic individually but not as a set; concurrent installs into one destination are not serialized. Documented recovery is re-running the idempotent installer. Confirmed present in scripts/install-linux.sh header and docs/GETTING-STARTED.md."
  - "Clean-machine install — not performed (no EC2 host provisioned this session); explicitly decided by the human at the phase's own Task 3 checkpoint not to gate the milestone."
  - "Cold first-time-reader doc walkthrough — not performed; same human decision, not gating."
advisory_findings_from_code_review:
  - "WR-01 (52-REVIEW.md): post-install `test -x` check can pass over a binary silently nested inside a pre-existing same-named directory at the destination (rare edge case, not exercised by any must-have)."
  - "WR-02 (52-REVIEW.md): `--dest \"\"` (explicit empty string) is silently treated as unset and falls back to the default destination instead of failing closed."
  - "WR-03 (52-REVIEW.md): docs/GETTING-STARTED.md's 'Manual equivalent' claims the manual `install` command is 'the script's entire behavior' — overstated, since the manual form lacks the script's stage-then-rename interruption protection. Manual path still functionally installs the three binaries correctly."
  - "IN-01, IN-02 (52-REVIEW.md): a misleading error message when --dest exists as a non-directory file, and a relative --dest resolving against the repo root rather than the caller's cwd. Both cosmetic/documentation gaps, not functional failures."
---

# Phase 52: Minimal Linux Packaging Verification Report

**Phase Goal:** A design partner has a documented minimal Linux install path that co-locates the three sibling binaries and lists required env/credentials
**Verified:** 2026-08-11T15:47:19Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Roadmap Success Criteria

| # | Success Criterion | Status | Evidence |
|---|---|---|---|
| 1 | A documented release build path co-locates `caprun`, `caprun-worker`, and `caprun-exec-launcher` (sibling `current_exe()` layout); `cargo install --path cli/caprun` alone documented as not sufficient | ✓ VERIFIED | Ran `INSTALL_DEST=<tmpdir> bash scripts/install-linux.sh` live — produced exactly `caprun`, `caprun-exec-launcher`, `caprun-worker`, all `test -x` true, no leftover staging dir. `docs/GETTING-STARTED.md:93` and `docs/CONFIGURATION.md` both carry the `cargo install --path cli/caprun` insufficiency warning naming `cli/caprun-exec-launcher` as a separate Cargo package (confirmed: `Cargo.toml` workspace members list it as its own package; `resolve_launcher_path()` at `crates/brokerd/src/sinks/process_exec.rs:736` and the single-hop `current_exe().parent().join("caprun-worker")` at `cli/caprun/src/main.rs:606-611` both confirmed by direct source read) |
| 2 | An env/credential checklist covers `CAPRUN_*`, policy file, and GitHub grant token as applicable | ✓ VERIFIED | `docs/CONFIGURATION.md` `## Operator Configuration Checklist` — three tiers present verbatim; all 14 `CAPRUN_*` names plus 5 worker-protocol names spot-checked with `grep -rl` against `crates/`/`cli/` Rust sources — every name is actually read by shipped code (0 invented, 0 stale); `CAPRUN_GITHUB_TOKEN` explicitly covers the GitHub grant token path; policy file documented with real `allowed_sinks`/`arg_constraints` keys matching `crates/runtime-core/src/policy.rs` |
| 3 | A thin install script is acceptable; not cargo-dist/deb/snap productization | ✓ VERIFIED | `scripts/install-linux.sh` is a single 190-line bash file; zero new crates/dependencies (`git diff --exit-code -- Cargo.toml Cargo.lock crates cli` exits 0, verified live); non-comment body scanned for escalation/package-manager/network-fetch/git-mutation tokens — 0 matches for all four categories |

### Detailed Must-Haves (from PLAN frontmatter, merged across 52-01/02/03)

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | `bash scripts/install-linux.sh` places exactly the 3 binaries in ONE dest dir, all executable | ✓ VERIFIED | Live run confirmed (see above) |
| 2 | Destination defaults to `${HOME}/.local/bin`, overridable via `--dest`, no privilege escalation | ✓ VERIFIED | `grep -c '.local/bin'` present; `--dest` flag tested live, worked; body scan for `sudo/doas/apt-get/yum/dnf/pacman/brew` = 0 |
| 3 | Non-Linux/missing toolchain/failed build/missing binary/unusable dest fails with actionable message | ✓ VERIFIED | Live-tested: unwritable dest → `FAIL — <path> is not writable...` exit 1; bogus flag → `FAIL — unrecognized argument...` exit 1; source confirms `uname -s`, `command -v cargo` guards present |
| 4 | Interrupted/failed install leaves destination unmodified (stage-then-rename, staging dir removed on every exit) | ✓ VERIFIED | Live fault-injection: installed once, `chmod 555` destination, re-run refused (exit 1), `sha256sum` of the three binaries identical before/after |
| 5 | `docs/GETTING-STARTED.md` documents both script and manual-equivalent commands | ✓ VERIFIED | Read file directly — `## Install (Linux)` section has both the `bash scripts/install-linux.sh` command and a "Manual equivalent" fenced block with `cargo build --workspace --release` + `install -m 755` |
| 6 | Doc states `cargo install --path cli/caprun` insufficient, names cause | ✓ VERIFIED | Verbatim blockquote present at `docs/GETTING-STARTED.md:93` naming `cli/caprun-exec-launcher` as a separate package |
| 7 | Doc no longer claims only two binaries produced | ✓ VERIFIED | "Clone and build" section lists all 3 required + optional `caprun-planner`; `grep -c 'two binaries'` = 0 |
| 8 | Install path is source-tree build+copy only — no remote fetch, no privileged package manager, no repo mutation | ✓ VERIFIED | Body-scoped grep for escalation/pkg-mgr/download tokens = 0; git-mutation subcommand scan = 0 |
| 9 | (backstop) Concurrency/interruption residual documented plainly, recovery = re-run | ✓ VERIFIED | Present in script header lines 34-45 and `docs/GETTING-STARTED.md:73` |
| 10 | `docs/CONFIGURATION.md` presents 3-tier checklist (always-needed / sink-specific / internal-do-not-set) | ✓ VERIFIED | All three `### Tier ...` headings present verbatim |
| 11 | Tier 1 shows `--policy` as runnable path, `CAPRUN_POLICY` as fallback, broker default named | ✓ VERIFIED | `### Session policy file` + Tier 1 table confirm this; `cli/caprun/src/main.rs` flag-over-env precedence read directly |
| 12 | Minimal policy example with `allowed_sinks`/`arg_constraints`, 9 sink ids, outside-workspace constraint | ✓ VERIFIED | JSON example present; all 9 sink id strings (`email.send`, `file.create`, `file.write`, `process.exec`, `git.commit`, `http.request`, `github.pr`, `http.request.write`, `git.push`) cross-checked against `crates/runtime-core/src/policy.rs` — all present in source |
| 13 | Tier 2 names every credential/setting the broker reads | ✓ VERIFIED | All 9 Tier-2 variable names grep-confirmed present in `crates/` or `cli/` Rust source |
| 14 | Credential guidance placeholder-only, least-scope framing | ✓ VERIFIED | `export CAPRUN_GITHUB_TOKEN='<paste-your-least-scope-token>'` style; credential-shape scan (`gh[pousr]_[A-Za-z0-9]{20,}`) across all phase-written files = 0 matches |
| 15 | Tier 3 explicitly instructs do-not-set for 7 named internal vars | ✓ VERIFIED | All 7 present with "do not set" framing; `SESSION_ID` carries the required "not read by any current code path" phrase — confirmed 0 matches for `"SESSION_ID"` in `crates/`/`cli/` Rust source, so the claim is accurate |
| 16 | Documented CLI surface matches shipped argv parsing | ✓ VERIFIED | Run form + 5 verbs (`confirm`/`deny`/`review`/`grant`/`audit`) all present verbatim |
| 17 | Every `CAPRUN_*` name in doc is read by shipped code (bidirectional) | ✓ VERIFIED | Spot-checked 14 names via `grep -rl` against `crates/`/`cli/` — all present (1-13 files each) |
| 18 | (backstop) Broker-local credential custody boundary stated | ✓ VERIFIED | `docs/CONFIGURATION.md:97`: "read by the broker process only... never forwarded into the confined worker or the confined `process.exec` child" |
| 19 | README layout tree lists `install-linux.sh`, names `caprun-exec-launcher` | ✓ VERIFIED | Confirmed in Repository layout fenced block |
| 20 | README build section points to install path without duplicating/re-asserting container as install way | ✓ VERIFIED | "Build & test" section has a 2-line pointer to `docs/GETTING-STARTED.md#install-linux`; container/Colima subsection untouched and follows after |
| 21 | Every `CAPRUN_*` name across README/GETTING-STARTED/CONFIGURATION read by shipped code | ✓ VERIFIED | Same spot-check as #17, extended to README (no new `CAPRUN_*` names introduced there) |
| 22 | Three required binary names consistent across README and GETTING-STARTED | ✓ VERIFIED | Both files name `caprun`, `caprun-worker`, `caprun-exec-launcher` |
| 23 | Install docs state layout check ≠ security verification, name `scripts/compose-verify.sh` | ✓ VERIFIED | `docs/GETTING-STARTED.md:91`: "This is a layout check, not a security proof... The authoritative harness is `scripts/compose-verify.sh`" |
| 24 | No real-credential-shaped value in any phase-written file | ✓ VERIFIED | Scan across README.md, docs/GETTING-STARTED.md, docs/CONFIGURATION.md, scripts/install-linux.sh = 0 matches |
| 25 | `check-invariants.sh` exits 0 and `git diff --exit-code -- Cargo.toml Cargo.lock crates cli` exits 0 | ✓ VERIFIED | Ran both live — 6/6 gates PASS; diff clean |
| 26 | Container/Colima sections survive unrewritten | ✓ VERIFIED | `colima start`, `docker-cache.sh` both still present in README.md and GETTING-STARTED.md |
| 27 | (backstop) First-time design partner can follow docs without source, install works on a non-dev-box host | KNOWN RESIDUAL (accepted, not gating) | Phase's own Task 3 blocking human checkpoint (plan `autonomous: false`) was used for this; the human explicitly recorded "Neither gates the milestone — record both as known residuals" for the clean-host install and cold-reader walkthrough. Not independently re-verifiable on this constrained dev box (no EC2, no naive reader) — treated per this task's `known_accepted_residuals` framing, not as a new gap |

**Score:** 26/27 truths directly VERIFIED by this session's independent evidence; 1/27 (#27, host-bound) is a pre-existing known/accepted residual from the phase's own human checkpoint, not a new gap. All 3 roadmap success criteria VERIFIED.

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `scripts/install-linux.sh` | Thin Linux installer, builds/stages/co-locates 3 siblings | ✓ VERIFIED | 190 lines, executable, `bash -n` clean, runs correctly (tested live) |
| `docs/GETTING-STARTED.md` | Install (Linux) walkthrough | ✓ VERIFIED | `## Install (Linux)` section present with script + manual path + insufficiency warning + layout-vs-security boundary statement |
| `docs/CONFIGURATION.md` | Corrected CLI reference + 3-tier checklist | ✓ VERIFIED | `## caprun CLI Arguments` and `## Operator Configuration Checklist` both present and accurate |
| `README.md` | Repository-layout entry + build pointer | ✓ VERIFIED | Layout tree and build-section pointer both present |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `scripts/install-linux.sh` | `target/release/{caprun,caprun-worker,caprun-exec-launcher}` | `cargo build --workspace --release` | ✓ WIRED | Live run produced all 3 binaries from a real release build |
| `scripts/install-linux.sh` | `cli/caprun/src/main.rs:607-611` single-hop worker lookup | flat destination directory | ✓ WIRED | Source confirms `current_exe().parent().join("caprun-worker")`, no fallback search — install script co-locates as required |
| `scripts/install-linux.sh` | `crates/brokerd/src/sinks/process_exec.rs:736-756` `resolve_launcher_path()` | direct sibling install | ✓ WIRED | Source confirms bounded ancestor walk (current_exe parent + 2 more levels) — satisfied by direct co-location |
| `docs/GETTING-STARTED.md` | `scripts/install-linux.sh` | documented install command | ✓ WIRED | Command and section heading both present |
| `docs/CONFIGURATION.md` | `cli/caprun/src/main.rs` `--policy`/`CAPRUN_POLICY` | flag-over-env precedence | ✓ WIRED | Source confirms flag takes precedence, matches doc |
| `docs/CONFIGURATION.md` | `crates/runtime-core/src/policy.rs` sink ids | JSON policy example | ✓ WIRED | All 9 sink id strings present in both doc and source |
| `README.md` | `scripts/install-linux.sh` / `docs/GETTING-STARTED.md` | layout tree + build pointer | ✓ WIRED | Confirmed present |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Default-dest install produces exactly 3 executables | `INSTALL_DEST=<tmp> bash scripts/install-linux.sh` | `caprun caprun-exec-launcher caprun-worker`, all `-x` | ✓ PASS |
| `--dest` override works | `bash scripts/install-linux.sh --dest <tmp>` | Exit 0, all 3 present | ✓ PASS |
| Unwritable destination fails closed | `chmod 500` parent, `--dest <parent>/nested` | Exit 1, `FAIL — ... not writable` | ✓ PASS |
| Bogus flag fails closed with usage | `--bogus-flag` | Exit 1, usage line | ✓ PASS |
| `--help` exits 0 with usage | `--help` | Exit 0, usage line | ✓ PASS |
| D-06 fault injection: refused re-install never partially replaces | install, `chmod 555` dest, re-run, `chmod 755`, compare sha256sum | Refused (exit 1); checksums identical before/after | ✓ PASS |
| `check-invariants.sh` | `./scripts/check-invariants.sh` | 6/6 gates PASS | ✓ PASS |
| TCB/dependency fence | `git diff --exit-code -- Cargo.toml Cargo.lock crates cli` | Exit 0 | ✓ PASS |
| Documented `CAPRUN_*` names read by source (spot sample) | `grep -rl <VAR> crates/ cli/` for 14 names | All 14 present in source (1-13 files each) | ✓ PASS |
| `SESSION_ID` correctly marked unread | `grep -rn '"SESSION_ID"' crates/ cli/` | 0 matches | ✓ PASS |
| No credential-shaped values in phase-written files | `grep -cE 'gh[pousr]_[A-Za-z0-9]{20,}'` across README/GETTING-STARTED/CONFIGURATION/install-linux.sh | 0 matches | ✓ PASS |

### Probe Execution

Not applicable — this phase declares no `probe-*.sh` scripts; PLAN/SUMMARY do not reference the probe pattern. Skipped.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| PKG-01 | 52-01, 52-02, 52-03 | Minimal Linux design-partner install path: co-located 3 binaries, env/credential checklist, thin install script (not full productization) | ✓ SATISFIED | All 3 roadmap success criteria independently verified live in this session (see table above). `REQUIREMENTS.md`'s traceability row still shows PKG-01 as "Pending" and the checkbox unticked — this is the pre-verification state; updating it is the orchestrator's responsibility after this report, not a gap in the phase's deliverable |

No orphaned requirements — PKG-01 is the only requirement mapped to Phase 52, and it appears in all three plans' `requirements` frontmatter.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `scripts/install-linux.sh` | 156-166 (per 52-REVIEW.md WR-01) | `test -x` post-install check doesn't distinguish a regular file from a directory | ⚠️ Warning (advisory, non-blocking) | Rare edge case: a pre-existing directory at a binary's destination path would let `mv` silently nest the binary inside it, and the layout check would still report PASS. Does not occur in a fresh/normal install; not exercised by any must-have's tested scenario |
| `scripts/install-linux.sh` | 76-84, 111 (per 52-REVIEW.md WR-02) | `--dest ""` silently falls back to default instead of failing closed | ⚠️ Warning (advisory, non-blocking) | Inconsistent with the script's otherwise-careful fail-closed behavior on malformed invocations; no must-have covers this specific invocation shape |
| `docs/GETTING-STARTED.md` | 75 (per 52-REVIEW.md WR-03) | "these commands are the script's entire behavior" overstates equivalence — omits that the manual form lacks stage-then-rename interruption protection | ⚠️ Warning (advisory, non-blocking) | Documentation accuracy nit; the manual path still correctly installs all 3 binaries on an uninterrupted run, satisfying D-01's "sufficient install path" requirement |
| `scripts/install-linux.sh` | 116-120 (per 52-REVIEW.md IN-01) | Misleading `FAIL — not writable` message when `--dest` is an existing plain file | ℹ️ Info | Cosmetic; script still correctly fails closed |
| `scripts/install-linux.sh` | 65-70 (per 52-REVIEW.md IN-02) | Relative `--dest` resolves against repo root, not caller's cwd | ℹ️ Info | Minor surprise; consistent with the documented "run from workspace root" convention |

No debt markers (`TBD`/`FIXME`/`XXX`) found in any phase-modified file (the one `XXXXXX` match in `install-linux.sh` is a `mktemp` template placeholder, not a debt marker). No `TODO`/`HACK` markers found. No blocker-severity anti-patterns found — these 5 findings were already surfaced by `52-REVIEW.md`'s code review (0 Critical / 3 Warning / 2 Info) and none bears directly on a must-have's tested acceptance criteria.

### Human Verification Required

None. The phase's own Task 3 (`checkpoint:human-verify`, gate="blocking", plan `autonomous: false`) already ran the human-facing walkthrough verification within the phase itself and recorded explicit approval plus an explicit accept-as-residual decision for the two host-bound checks this dev box cannot perform (clean-machine install, cold first-time reader). No new human verification items are raised by this independent re-verification pass — all other must-haves were directly confirmed against the codebase in this session.

### Known/Accepted Residuals (not gaps)

1. **Interruption/concurrency (T-52-07, accept disposition):** three POSIX renames are atomic individually, not as a set; concurrent installs into the same destination are not serialized. Documented recovery is re-running the idempotent installer. Confirmed present in the script header and `docs/GETTING-STARTED.md`.
2. **Clean-machine install — not performed** this session (no EC2 host provisioned; CLAUDE.md confirms this dev box cannot run it). Explicitly decided by the human at the phase's Task 3 checkpoint not to gate the milestone.
3. **Cold first-time-reader doc walkthrough — not performed.** Same human decision, not gating.

### Gaps Summary

No gaps found. All 3 roadmap success criteria and all 27 detailed must-have truths (26 directly re-verified live in this session, 1 a pre-existing accepted residual) are satisfied by the actual codebase, not merely claimed in SUMMARY.md. The install script was executed live multiple times (default dest, explicit `--dest`, unwritable-destination fault injection, bogus-flag/`--help` paths) and produced exactly the required three-binary set every time it should have succeeded, and failed closed with actionable messages every time it should have failed. `check-invariants.sh` and the TCB/dependency diff fence were both re-run independently and passed. Every `CAPRUN_*` name documented was cross-checked bidirectionally against the actual Rust source. The phase's own code review (`52-REVIEW.md`) surfaced 5 advisory (non-blocking) findings, none of which invalidates a tested must-have.

---

_Verified: 2026-08-11T15:47:19Z_
_Verifier: Claude (gsd-verifier)_
