---
phase: 52-minimal-linux-packaging
plan: 02
subsystem: docs
tags: [documentation, cli-reference, configuration, policy, credentials]

# Dependency graph
requires:
  - phase: 52-minimal-linux-packaging
    provides: "52-01's scripts/install-linux.sh and docs/GETTING-STARTED.md '## Install (Linux)' section — this plan does not modify either, only shares the docs/ tree"
provides:
  - "docs/CONFIGURATION.md '## caprun CLI Arguments' — corrected verb+flag usage (run/--policy/--seed-from-file plus confirm/deny/review/grant/audit), intent-kind table, corrected runnable examples, and a '### Session policy file' subsection with the minimal allowed_sinks/arg_constraints JSON example, the nine production sink ids, and the outside-workspace containment constraint"
  - "docs/CONFIGURATION.md '## Operator Configuration Checklist' — three-tier operator checklist (always-needed / sink-specific credentials / internal-do-not-set) replacing the stale Worker Environment Variables table"
affects: [52-03, packaging, design-partner-onboarding]

# Actuals (#2632)
actuals:
  tokens: 3137
  tasks: 2
  commits: 2

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Doc heading/table conventions preserved: <!-- generated-by: gsd-doc-writer --> marker, --- H2 rules, | Col | Col | Col | tables, 'Example — <label>:' bold lead-ins before fenced bash blocks"
    - "Credential guidance: prose constraint sentence then table then angle-bracket placeholder export block — never a real-token-shaped value"

key-files:
  created: []
  modified:
    - docs/CONFIGURATION.md

key-decisions:
  - "Replaced the stale '## Worker Environment Variables' table wholesale rather than patching it — the table named SESSION_ID (read by no current code) and omitted every real CAPRUN_* variable, matching RESEARCH.md's explicit 'replace, not patch' recommendation."
  - "Wrote every Tier 2/Tier 3 variable name as its own backtick-delimited token so the word-boundary verification grep (which treats an immediately-following '_' as NOT a boundary) distinguishes e.g. CAPRUN_CONFIRM from CAPRUN_CONFIRM_TIMEOUT_SECS."
  - "Kept the 'Worker protocol variables (reference)' subsection short and cross-referencing Tier 3 rather than re-listing rows, per the plan's 'do not reintroduce a row for a variable no code reads' constraint."
  - "Left the file's '## Cross-OS Testing with Colima and Docker' section untouched — out of scope for this plan (CLAUDE.md itself calls it stale; RESEARCH.md Pitfall 4 says do not rewrite it here)."

patterns-established:
  - "Backtick-isolated variable names in doc tables/prose so mechanical word-boundary greps (used by this phase's own verify commands and Plan 03's cross-check) match unambiguously even when one variable name is a prefix of another."

requirements-completed: [PKG-01]

coverage:
  - id: D1
    description: "docs/CONFIGURATION.md's '## caprun CLI Arguments' section matches the shipped argv parsing verb for verb and flag for flag: the run-form usage line, all five non-run verbs (confirm/deny/review/grant/audit), the three intent kinds, and the minimal allowed_sinks/arg_constraints policy example with the nine production sink ids and the outside-workspace containment constraint"
    requirement: "PKG-01"
    verification:
      - kind: other
        ref: "Task 1 <verify> grep chain (run-form usage line, caprun audit <session_id> <audit-db-path>, allowed_sinks, arg_constraints, http.request.write, safe-coding-workflow, stale bare-positional line count 0) — all passed this session"
        status: pass
    human_judgment: false
  - id: D2
    description: "docs/CONFIGURATION.md's '## Operator Configuration Checklist' replaces the stale Worker Environment Variables table with three tiers (always-needed / sink-specific credentials / internal-do-not-set), naming every real CAPRUN_* variable read by shipped code, retaining SESSION_ID only with the 'not read by any current code path' phrase, and using angle-bracket-only credential placeholders"
    requirement: "PKG-01"
    verification:
      - kind: other
        ref: "Task 2 <verify> grep chain (three Tier headings present, word-boundary presence loop over all 18 required variable names, SESSION_ID count-equality with the disclaimer phrase, zero real-token-shaped matches) — all passed this session"
        status: pass
    human_judgment: false
  - id: D3
    description: "No TCB/build-graph change: Cargo.toml, Cargo.lock, crates/, and cli/ are untouched by this plan"
    requirement: "PKG-01"
    verification:
      - kind: other
        ref: "git diff --exit-code -- Cargo.toml Cargo.lock crates cli (run after both tasks) + ./scripts/check-invariants.sh (all 6 gates PASS)"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-08-11
status: complete
---

# Phase 52 Plan 02: Operator Configuration Checklist Summary

**Rewrote `docs/CONFIGURATION.md`'s CLI reference and environment-variable table against the shipped code: the real verb+flag argv surface, a minimal `allowed_sinks`/`arg_constraints` policy example, and a three-tier operator checklist naming every real `CAPRUN_*` variable while explicitly retiring the never-read `SESSION_ID`.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-08-11T02:20:00Z (approx)
- **Completed:** 2026-08-11T02:44:53Z
- **Tasks:** 2/2 completed
- **Files modified:** 1 (`docs/CONFIGURATION.md`)

## Accomplishments
- `## caprun CLI Arguments` now shows the real six invocation shapes (`run`/`--policy`/`--seed-from-file` plus `confirm`/`deny`/`review`/`grant`/`audit`), an intent-kind table (`send-email-summary`, `create-file-from-report`, `safe-coding-workflow`), corrected runnable examples using an actual intent kind and parameter, and a new `### Session policy file` subsection with a minimal `allowed_sinks`/`arg_constraints` JSON example, all nine production sink ids, the outside-the-workspace containment constraint (`bind_policy` refuses a policy path at-or-beneath the workspace root), and a one-sentence statement that policy narrows but never relaxes I2.
- The stale `## Worker Environment Variables` table (which named the never-read `SESSION_ID` and omitted every real `CAPRUN_*` variable) is replaced with `## Operator Configuration Checklist`, organized into `### Tier 1 — Always needed`, `### Tier 2 — Sink-specific credentials and settings`, and `### Tier 3 — Internal and test-only (do not set)`.
- Tier 2 names every credential/setting the broker actually reads (`CAPRUN_GITHUB_TOKEN`, `CAPRUN_GIT_PUSH_TOKEN`, `CAPRUN_HTTP_WRITE_TOKEN`, `CAPRUN_SMTP_HOST`/`_PORT`/`_FROM`, `CAPRUN_GITHUB_API_BASE`, `CAPRUN_CONFIRM`, `CAPRUN_CONFIRM_TIMEOUT_SECS`), states the broker-local custody boundary (never forwarded into the confined worker or `process.exec` child), gives angle-bracket-only least-scope placeholder exports, and separately marks the optional `CAPRUN_PLANNER`/`CAPRUN_PLANNER_MODEL`/`OPENAI_API_KEY` LLM sidecar as outside the minimal deterministic path.
- Tier 3 explicitly instructs operators never to set `BROKER_SOCK`, `WORKSPACE_FILE`, `INTENT`, `PRIMARY_SEED_FILE_DERIVED`, `PLANNER_SOCK`, `CAPRUN_CODING_I2_PROOF`, or `CAPRUN_ENABLE_IPC_CREATE_SESSION`, and retains `SESSION_ID` with the exact required phrase "not read by any current code path" so a future reader knows it was deliberately kept, not overlooked.

## Task Commits

Each task was committed atomically:

1. **Task 1: Correct the CLI reference and add the minimal policy-file example** - `ab1072c` (docs)
2. **Task 2: Replace the stale environment table with the three-tier operator configuration checklist** - `5875200` (docs)

_No metadata/SUMMARY commit yet — the orchestrator handles STATE.md/ROADMAP.md per this project's override; this plan's own metadata commit is intentionally omitted (see below)._

## Files Created/Modified
- `docs/CONFIGURATION.md` - corrected `## caprun CLI Arguments` (verb+flag usage, intent-kind table, minimal policy-file example, containment constraint) and replaced `## Worker Environment Variables` with `## Operator Configuration Checklist` (three tiers)

## Decisions Made
- Replaced the stale env-var table wholesale rather than patching it, matching RESEARCH.md Pitfall 3's explicit recommendation — the old table's `SESSION_ID` row and missing `CAPRUN_*` coverage were drift, not a base to build on.
- Wrote every Tier 2/Tier 3 variable name in its own backtick span so the mechanical word-boundary verification (and Plan 03's cross-doc-to-source grep) matches each name unambiguously even where one name prefixes another (e.g. `CAPRUN_CONFIRM` vs. `CAPRUN_CONFIRM_TIMEOUT_SECS`, `CAPRUN_PLANNER` vs. `CAPRUN_PLANNER_MODEL`).
- Kept the retained "Worker protocol variables (reference)" subsection to one cross-referencing paragraph rather than a duplicate table, avoiding a second copy of Tier 3's rows that could drift independently.
- Left `docs/CONFIGURATION.md`'s `## Cross-OS Testing with Colima and Docker` section untouched — it is flagged stale by CLAUDE.md itself and explicitly out of scope for this plan (RESEARCH.md Pitfall 4); rewriting it belongs to a future verification-workflow-focused change, not this configuration-checklist plan.

## Deviations from Plan

None - plan executed exactly as written. Both tasks' full `<verify>` grep chains and all listed `<acceptance_criteria>` passed on the first attempt; no auto-fixes were needed.

## Issues Encountered

None. All verification commands specified in the plan run locally without Docker or EC2 (`grep`, `git diff --exit-code -- Cargo.toml Cargo.lock crates cli`, `./scripts/check-invariants.sh`) and all passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `docs/CONFIGURATION.md`'s CLI reference and Operator Configuration Checklist are complete and independently verified (both tasks' grep-chain verify commands, `git diff --exit-code -- Cargo.toml Cargo.lock crates cli`, `./scripts/check-invariants.sh` — all 6 gates PASS).
- This plan touched only `docs/CONFIGURATION.md`; it made no change to `docs/GETTING-STARTED.md`, `scripts/install-linux.sh`, `crates/`, `cli/`, or the Cargo manifests, so Plan 52-03 (cross-doc reconciliation / bidirectional documented-vs-source cross-check) is unblocked.
- No blockers or concerns carried forward.

---
*Phase: 52-minimal-linux-packaging*
*Completed: 2026-08-11*

## Self-Check: PASSED

- FOUND: docs/CONFIGURATION.md
- FOUND: .planning/phases/52-minimal-linux-packaging/52-02-SUMMARY.md
- FOUND commit: ab1072c (Task 1)
- FOUND commit: 5875200 (Task 2)
