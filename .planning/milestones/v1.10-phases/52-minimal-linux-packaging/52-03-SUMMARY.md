---
phase: 52-minimal-linux-packaging
plan: 03
subsystem: docs
tags: [documentation, packaging, install, cross-doc-consistency, boundary-statement]

# Dependency graph
requires:
  - phase: 52-minimal-linux-packaging
    provides: "52-01's scripts/install-linux.sh and docs/GETTING-STARTED.md '## Install (Linux)' section"
  - phase: 52-minimal-linux-packaging
    provides: "52-02's docs/CONFIGURATION.md '## Operator Configuration Checklist' (three tiers)"
provides:
  - "README.md repository-layout tree lists scripts/install-linux.sh, cli/caprun-exec-launcher/, and cli/caprun-planner/, replacing the stale single-entry CLI tree"
  - "README.md 'Build & test' section pointer to bash scripts/install-linux.sh and docs/GETTING-STARTED.md, placed before the container/Colima subsection"
  - "docs/GETTING-STARTED.md boundary statement: the install-time layout check is not the Linux security verification; names scripts/compose-verify.sh as the authoritative harness"
  - "Phase-level cross-document consistency: bidirectional CAPRUN_* name check, three-binary naming check, credential-shape scan, TCB/dependency diff check — all re-verified clean at close of phase"
  - "Recorded human sign-off on the design-partner install walkthrough, with the two host-bound residual verifications explicitly named as known-unverified, not passed"
affects: [packaging, design-partner-onboarding, ship-gate]

# Actuals (#2632)
actuals:
  tokens: 717
  tasks: 3
  commits: 3

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cross-document reconciliation gate: fix the document when a bidirectional grep fails, never relax the gate or add code to make a document true"

key-files:
  created: []
  modified:
    - README.md
    - docs/GETTING-STARTED.md

key-decisions:
  - "Task 2's gates all passed on first run against the state left by Plan 01/02 plus Task 1 — no document required correction beyond the boundary-statement sentence, which was added directly rather than discovered as a gap."
  - "The two host-bound verifications (clean design-partner machine install; cold first-time reader) are recorded as explicitly known-unverified residuals per the human's decision, not folded into the 'checks that passed' list and not treated as blocking the milestone."
  - "The pre-existing interruption/concurrency residual (T-52-07, accepted disposition) is carried forward unchanged from Plan 01 — no new mitigation was built or attempted in this plan."

patterns-established: []

requirements-completed: [PKG-01]

coverage:
  - id: D1
    description: "README.md's repository-layout tree lists scripts/install-linux.sh, caprun-exec-launcher/, and caprun-planner/, and a build-section pointer directs the reader to the install path without duplicating or re-litigating the container recipe"
    requirement: "PKG-01"
    verification:
      - kind: other
        ref: "Task 1 verify grep chain (install-linux.sh count>=2, caprun-exec-launcher, caprun-planner, GETTING-STARTED, colima start, docker-cache.sh, generated-by marker all present)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Cross-document consistency: every CAPRUN_* token in README.md/GETTING-STARTED.md/CONFIGURATION.md is read by shipped Rust source; caprun-worker and caprun-exec-launcher named in both README.md and GETTING-STARTED.md; zero real-token-shaped credential values across all four phase-written files; docs/GETTING-STARTED.md names scripts/compose-verify.sh as the authoritative Linux verification and states the install-time check is not a security proof"
    requirement: "PKG-01"
    verification:
      - kind: other
        ref: "Task 2 full verify chain, re-run in this continuation session: check-invariants.sh (6/6 gates PASS), git diff --exit-code -- Cargo.toml Cargo.lock crates cli (clean), bash -n scripts/install-linux.sh (exit 0), bidirectional CAPRUN_* cross-check (PASS, 0 unread names), three-binary naming check (PASS), credential-shape scan (0 matches), compose-verify.sh named in GETTING-STARTED.md (PASS), dry-run install into mktemp -d (exit 0, exactly caprun/caprun-exec-launcher/caprun-worker), fault-injection re-install into chmod-555 destination (refused, sha256sum of prior binaries unchanged)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Human sign-off on the design-partner-facing install walkthrough, with the two host-bound verifications this phase could not perform recorded as known-unverified residuals rather than passed, per explicit user decision that neither gates the milestone"
    requirement: "PKG-01"
    verification: []
    human_judgment: true
    rationale: "Reading a walkthrough for first-time-reader clarity and confirming a clean-host install are judgment calls only a human can make; this session recorded the human's explicit approval and their decision on how to treat the two residuals it could not itself verify (no clean host / no cold reader available on this dev box)."

duration: ~15min
completed: 2026-08-11
status: complete
---

# Phase 52 Plan 03: README Reconciliation, Cross-Document Consistency Gate, and Human Sign-off Summary

**README.md's repository-layout tree and build-section now name the installer, the separately packaged `caprun-exec-launcher`, and `caprun-planner`; the phase-level cross-document consistency gate re-verified clean; and a human has signed off on the design-partner install walkthrough with two host-bound verifications explicitly recorded as known-unverified residuals rather than passed.**

## Performance

- **Duration:** ~15 min (this continuation session; tasks 1-2 were completed and committed by a prior session)
- **Tasks:** 3/3 completed (2 by a prior session, task 3 checkpoint resolved in this session)
- **Files modified:** 2 (`README.md`, `docs/GETTING-STARTED.md`)

## Accomplishments
- `README.md`'s repository-layout tree gained an `install-linux.sh` entry under `scripts/` and a corrected `cli/` subtree naming `caprun/`, `caprun-exec-launcher/`, and `caprun-planner/` — replacing a stale single-entry form that implied the CLI tree produces only the orchestrator and the worker.
- `README.md`'s "Build & test" section gained a short pointer (`bash scripts/install-linux.sh`, naming `docs/GETTING-STARTED.md` as the walkthrough) placed before the container/Colima subsection, which remains byte-identical.
- `docs/GETTING-STARTED.md` carries an explicit boundary statement: the post-install layout check confirms only that the three binaries are present and executable — it does not exercise confinement, does not open a Session, and is not a substitute for the Linux security verification. `scripts/compose-verify.sh` is named as the authoritative harness.
- The phase-level cross-document consistency gate (bidirectional `CAPRUN_*` name check against `crates/`/`cli/` source, three-binary naming check across `README.md`/`GETTING-STARTED.md`, credential-shape scan across all four phase-written files, TCB/dependency diff check, `check-invariants.sh`) was re-run in this session and passed clean with no corrections needed.
- A human read the full design-partner install walkthrough (`docs/GETTING-STARTED.md` `## Install (Linux)`, `docs/CONFIGURATION.md` `## Operator Configuration Checklist`), ran the installer themselves, and approved.

## Task Commits

Each task was committed atomically:

1. **Task 1: Narrow README update — layout tree entries and a build-section pointer** - `42acfc9` (docs)
2. **Task 2: Cross-document consistency gate, boundary statement, phase regression check** - `904d2ab` (docs)
3. **Task 3: Human sign-off on the design-partner install walkthrough** - checkpoint, resolved by explicit human approval (no code change; see "Human Sign-off" below)

**Plan metadata:** this commit (docs: complete 52-03 plan)

## Files Created/Modified
- `README.md` - repository-layout tree entries for `install-linux.sh`, `caprun-exec-launcher/`, `caprun-planner/`; build-section pointer to the install path
- `docs/GETTING-STARTED.md` - boundary statement naming `scripts/compose-verify.sh` as the authoritative Linux verification, distinct from the install-time layout check

## Decisions Made
- Task 2's full gate chain passed on the first run against the state Plan 01/02 and Task 1 left behind — no document required a fix beyond adding the boundary-statement sentence, which was written directly into `docs/GETTING-STARTED.md`'s `## Install (Linux)` section rather than discovered as a drift gap.
- The two host-bound verifications this phase's automation cannot perform (clean design-partner machine install; a cold first-time reader following the doc) are recorded here as **known-unverified residuals**, not as passed checks. Per the human's explicit decision at the checkpoint: **"Neither gates the milestone — record both as known residuals. Rationale: Phase 52 is docs + a thin script, and the install path was verified locally by dry-run and fault injection."**
- The pre-existing interruption/concurrency residual (T-52-07, `accept` disposition, carried from Plan 01) is reaffirmed unchanged: an install interrupted between the final three renames, or two concurrent installs into one destination, can leave a binary set mixed across two builds. Documented recovery is re-running the installer. No lock protocol was built — out of scope per RESEARCH.md "Don't Hand-Roll."

## Human Sign-off

The human reviewing the checkpoint gave **explicit approval** of the design-partner-facing install walkthrough (`docs/GETTING-STARTED.md` `## Install (Linux)` through the manual-equivalent commands, and `docs/CONFIGURATION.md` `## Operator Configuration Checklist`), confirming:
- The walkthrough is followable without consulting source.
- The `cargo install --path cli/caprun` insufficiency warning states its cause (`caprun-exec-launcher` is a separate Cargo package), not just the assertion.
- Running `bash scripts/install-linux.sh --dest "$(mktemp -d)"` produced a `PASS` line and exactly three executables in the named destination.
- The three-tier `docs/CONFIGURATION.md` checklist is unambiguous and every credential example is an obvious placeholder.
- The install documentation correctly names `scripts/compose-verify.sh` as the authoritative Linux verification and does not claim the layout check proves confinement.
- The recorded interruption/concurrency residual is accepted as-is.

**Item 7 (open question) — resolved by the human:** whether the two verifications this host could not perform (clean-machine install; cold first-time reader) should gate the milestone. Answer: **"Neither gates the milestone — record both as known residuals."**

## What Was Verified On This Host (and passed)

- `./scripts/check-invariants.sh` — all 6 gates PASS
- `bash -n scripts/install-linux.sh` — exit 0
- TCB/dependency fence: `git diff --exit-code -- Cargo.toml Cargo.lock crates cli` — exit 0 (clean; this documentation phase changed no trusted-computing-base code and added no dependency)
- Dry-run install: `INSTALL_DEST="$(mktemp -d)" bash scripts/install-linux.sh` — exit 0, produced exactly `caprun`, `caprun-exec-launcher`, `caprun-worker`, all executable
- Fault-injection re-install refusal: `chmod 555` the destination, re-run — refused with a named `FAIL` message; `sha256sum` of the prior three binaries unchanged before/after
- Bidirectional `CAPRUN_*` cross-check: every `CAPRUN_*` token in `README.md`, `docs/GETTING-STARTED.md`, `docs/CONFIGURATION.md` matched a `grep` hit in `crates/` or `cli/` Rust sources — 0 undocumented/unread names
- Credential-shape scan: `grep -cE 'gh[pousr]_[A-Za-z0-9]{20,}'` across `README.md`, `docs/GETTING-STARTED.md`, `docs/CONFIGURATION.md`, `scripts/install-linux.sh` — 0 matches
- Three-binary naming check: `caprun-worker` and `caprun-exec-launcher` present in both `README.md` and `docs/GETTING-STARTED.md`
- `docs/GETTING-STARTED.md` names `scripts/compose-verify.sh` in the boundary statement

## Known Residuals (recorded, not gating)

These are recorded as explicitly **known-unverified**, per the human's decision at the Task 3 checkpoint — neither gates the milestone:

1. **Install on a clean design-partner machine — not performed.** No clean host was available in this session; performing it would require provisioning an EC2 instance per `CLAUDE.md`'s standing guidance for Linux verification work that this dev box cannot do locally.
2. **A cold first-time reader following the doc — not performed.** No naive reader was available to walk the documentation blind; the human sign-off in this session was performed by a reviewer already familiar with the project.
3. **Interruption/concurrency residual (carried forward, `accept` disposition, T-52-07).** An install interrupted between the final three renames, or two concurrent installs into one destination, can leave a binary set mixed across two builds. Documented recovery is re-running the installer. No lock protocol was built — out of scope per RESEARCH.md "Don't Hand-Roll."

**What was NOT run:** The Linux security verification harness (`scripts/compose-verify.sh` / `scripts/mailpit-verify.sh`) was not run in this session. Both require Docker on a provisioned Linux host, unavailable on this dev box per `CLAUDE.md`. This is unrelated to the two residuals above — this phase made no change to `crates/` or `cli/` (confirmed by the clean TCB diff), so no new Linux security verification was required for this phase's own gate; the harness remains the authoritative reference named in the boundary statement for any reader who wants to independently confirm the confinement claims this phase does not itself re-prove.

## Deviations from Plan

None - plan executed exactly as written by the prior session (Tasks 1-2) and completed here (Task 3 checkpoint resolution). No auto-fixes were needed; all Task 2 gates passed on the first re-run in this continuation session.

## Issues Encountered

None. All verification commands specified in the plan run locally without Docker or EC2 and all passed on re-run in this session.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All three PKG-01 install-path documents (`README.md`, `docs/GETTING-STARTED.md`, `docs/CONFIGURATION.md`) are internally consistent, verified by the bidirectional cross-check.
- The install-time layout check is documented as a layout check, not a security proof, with `scripts/compose-verify.sh` named as the authoritative harness (D-09).
- The phase demonstrably changed no code under `crates/` or `cli/`, added no crate and no dependency (D-10, HYG-02).
- The interruption/concurrency residual is surfaced, not buried (T-52-07).
- A human has read the design-partner-facing walkthrough and signed off, with the two host-bound verifications recorded as known-unverified residuals per explicit human decision.
- Per this repo's standing mitigation (recorded in `CLAUDE.md`/`.planning/STATE.md`), this executor did NOT modify `.planning/STATE.md` or `.planning/ROADMAP.md` — phase-completion state is the orchestrator's responsibility.
- No blockers or concerns carried forward beyond the three recorded residuals above.

---
*Phase: 52-minimal-linux-packaging*
*Completed: 2026-08-11*

## Self-Check: PASSED

- FOUND: README.md (install-linux.sh, caprun-exec-launcher, caprun-planner entries)
- FOUND: docs/GETTING-STARTED.md (compose-verify.sh boundary statement)
- FOUND: .planning/phases/52-minimal-linux-packaging/52-03-SUMMARY.md
- FOUND commit: 42acfc9 (Task 1)
- FOUND commit: 904d2ab (Task 2)
