---
phase: 33-filesystem-read-write-breadth
plan: 05
subsystem: testing
tags: [rust, cargo, taint-chain, i2-enforcement, cfg-linux, mailpit-verify]

# Dependency graph
requires:
  - phase: 33-filesystem-read-write-breadth (33-01..04)
    provides: write_within (adapter-fs), file.write executor tables (sink_sensitivity, sink_schema), RequestFd limiter, file.write broker sink + dispatch arm
provides:
  - "s9_file_write_block.rs: FS-03 genuine-taint-Block acceptance test for file.write"
  - "Linux compile-check + scoped test run enumerating every #[cfg(target_os=\"linux\")] caller across the whole FS-01/02/03 change set"
affects: [phase-34-composed-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "In-process I2 acceptance test that exercises submit_plan_node directly (no CLI subprocess), mirroring s9_process_exec_block.rs's non-live half"
    - "Chain onto mint_from_read's chain_head_id (demoted_id), never read_event_id, when appending a subsequent event — avoids DAG forking (mint_from_read's own doc warning)"

key-files:
  created:
    - cli/caprun/tests/s9_file_write_block.rs
  modified: []

key-decisions:
  - "s9_file_write_block.rs is NOT cfg(target_os=\"linux\")-gated — invoke_file_write/write_within's Linux-vs-stub split is irrelevant here since this test only exercises the platform-independent executor::submit_plan_node decision, never the live sink"
  - "Used mint_from_read with claim_type relative_path (path slot, origin_role matches file.write's [path, relative_path]) and doc_fragment (contents slot, origin_role matches file.write's [path, exec_output, doc_fragment]) to produce genuinely role-admissible tainted values"
  - "sink_blocked events chain onto mint_from_read's returned chain_head_id/chain_head_hash (the session_demoted event), not read_event_id — using read_event_id would fork the DAG (sink_blocked and session_demoted would both be children of file_read), breaking verify_chain's single-linear walk"

patterns-established:
  - "Cross-platform (non-cfg-gated) S9-style acceptance test for sinks whose live effect is a single in-broker syscall (no spawn/confinement split), contrasted with Linux-gated tests for sinks that DO spawn a child process"

requirements-completed: [FS-01, FS-02, FS-03]

coverage:
  - id: D1
    description: "Tainted file.write `path` (routing-sensitive) deterministically Blocks via the unmodified submit_plan_node, with a genuine (non-stapled) provenance_chain[0] anchor to the real file_read event"
    requirement: "FS-03"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/s9_file_write_block.rs#s9_file_write_tainted_path_blocks_with_genuine_anchor"
        status: pass
    human_judgment: false
  - id: D2
    description: "Tainted file.write `contents` (content-sensitive) deterministically Blocks via the unmodified submit_plan_node, with a genuine (non-stapled) provenance_chain[0] anchor to the real file_read event"
    requirement: "FS-03"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/s9_file_write_block.rs#s9_file_write_tainted_contents_blocks_with_genuine_anchor"
        status: pass
    human_judgment: false
  - id: D3
    description: "A clean (UserTrusted) path+contents pair is not blocked (positive control)"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/s9_file_write_block.rs#s9_file_write_clean_trusted_pair_is_allowed"
        status: pass
    human_judgment: false
  - id: D4
    description: "The entire FS-01/02/03 change set compiles under real Linux (all #[cfg(target_os=\"linux\")] callers enumerated) and every new FS test target passes on real Linux"
    verification:
      - kind: integration
        ref: "MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going' bash scripts/mailpit-verify.sh (true exit 0)"
        status: pass
      - kind: integration
        ref: "MAILPIT_VERIFY_CMD='cargo test -p adapter-fs write_within && cargo test -p brokerd file_write && cargo test -p brokerd request_fd && cargo test -p executor file_write && cargo test -p caprun --test s9_file_write_block' bash scripts/mailpit-verify.sh (true exit 0)"
        status: pass
    human_judgment: false

duration: 30min
completed: 2026-07-18
status: complete
---

# Phase 33 Plan 05: FS-03 Acceptance Test + Mandatory Linux Verification Summary

**s9_file_write_block.rs proves a genuine (non-stapled) taint-Block on file.write's path/contents slots in-process, cross-platform; the full FS-01/02/03 change set compiles and passes on real Linux with zero cfg-linux blind spots.**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-07-18T00:00:00Z (approx)
- **Completed:** 2026-07-18T00:36:26Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added `cli/caprun/tests/s9_file_write_block.rs`: 3 tests proving FS-03's I2 guarantee on `file.write` — tainted `path` Blocks, tainted `contents` Blocks, clean trusted pair Allows — each asserting the genuine-taint backstop (`provenance_chain[0]`/`read_event_id` equal the real `file_read` DAG event, never fabricated).
- Ran the mandatory Linux compile enumeration (`cargo build --tests --workspace --keep-going` via `scripts/mailpit-verify.sh`) — the whole workspace, including every `#[cfg(target_os="linux")]` test target from Plans 01/03/04, compiles clean (warnings only, no errors).
- Ran the scoped Linux test run across every new FS target (`adapter-fs::write_within`, `brokerd::file_write`, `brokerd::request_fd`, `executor::file_write`, `caprun::s9_file_write_block`) — all named tests pass on real Linux.

## Task Commits

1. **Task 1: FS-03 genuine-taint-Block acceptance test (s9_file_write_block.rs)** - `265ab83` (test)
2. **Task 2: Mandatory Linux compile-check + scoped Linux test run** - no commit (verification-only; no files modified beyond Task 1's test file)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `cli/caprun/tests/s9_file_write_block.rs` - FS-03 genuine-taint-Block acceptance test for `file.write` (3 tests, cross-platform, not cfg-linux-gated)

## Decisions Made
- The test is deliberately NOT `#[cfg(target_os = "linux")]`-gated: unlike `s9_process_exec_block.rs` (which spawns a real launcher via `tokio::process::Command`, a Linux-vs-stub split), `file.write`'s live sink is a single in-broker `openat2` and this test never even calls `invoke_file_write` — it exercises only `executor::submit_plan_node`, which has no platform split. This is stated explicitly in the file's doc comment to preempt a future reviewer flagging it as a missing cfg-gate.
- Chose `claim_type: "relative_path"` for the tainted `path` case and `claim_type: "doc_fragment"` for the tainted `contents` case — both produce `origin_role` values that file.write's Step-1c role gate (`sink_sensitivity::expected_role`) admits, so the test exercises the REAL I2 Block path (taint + sensitivity) rather than accidentally tripping the earlier `SlotTypeMismatch` structural Deny.
- Corrected a chain-forking pitfall during authoring: `sink_blocked` events must chain onto `mint_from_read`'s returned `chain_head_id`/`chain_head_hash` (the `session_demoted` event it also appends), never `read_event_id` — using `read_event_id` would make `sink_blocked` a sibling of `session_demoted` (both children of `file_read`), forking the DAG and failing `verify_chain`. This mirrors `mint_from_read`'s own doc-comment warning, applied correctly here on the first attempt after reading it.

## Deviations from Plan

None - plan executed exactly as written. No auto-fixes were needed; both `check-invariants.sh` gates and all local + Linux test runs passed on the first full attempt.

## Issues Encountered
None - the one non-trivial design decision (chaining onto `chain_head_id` rather than `read_event_id`) was caught during authoring by reading `mint_from_read`'s own doc comment before writing the assertion, not discovered via a failing test.

## Linux Verification Detail

**Step 1 — Linux compile enumeration:**
```
MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going' bash scripts/mailpit-verify.sh
```
True exit captured before any pipe: `0`. Output: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1m 16s` — every crate (including `sandbox`, `caprun-exec-launcher`, `caprun-planner`) and every test target compiled with only pre-existing warnings (unused import in `crates/sandbox/src/landlock.rs`, dead-code in `cli/caprun/src/planner.rs`'s `LlmPlanner`), no errors. `Mailpit-backed Linux verification suite PASSED.`

**Step 2 — scoped Linux test run:**
```
MAILPIT_VERIFY_CMD='cargo test -p adapter-fs write_within && cargo test -p brokerd file_write && cargo test -p brokerd request_fd && cargo test -p executor file_write && cargo test -p caprun --test s9_file_write_block' bash scripts/mailpit-verify.sh
```
True exit captured before any pipe: `0`. Named test results (all `ok`):
- `adapter-fs`: `workspace::tests::write_within_absolute_path_rejected`, `write_within_missing_target_enoent`, `write_within_parent_traversal_rejected`, `write_within_symlink_escape_rejected`, `write_within_overwrites_existing` (5/5)
- `brokerd` (file_write): `sinks::file_write::tests::invoke_file_write_failure_records_sink_execution_failed`, `invoke_file_write_success_records_sink_executed` (2/2)
- `brokerd` (request_fd): `server::tests::provide_intent_before_any_request_fd_succeeds`, `request_fd_count_limit`, `provide_intent_after_request_fd_is_rejected`, `request_fd_repeated_reads_under_bound_succeed` (4/4)
- `executor` (file_write): `sink_schema::tests::file_write_exact_args_ok`, `file_write_missing_required_arg_denied`, `file_write_unknown_arg_denied`, `sink_sensitivity::tests::file_write_contents_expects_path_exec_output_or_doc_fragment`, `file_write_contents_is_content_sensitive`, `file_write_contents_not_routing_sensitive`, `file_write_is_commit_irreversible`, `file_write_path_expects_path_or_relative_path`, `file_write_path_is_routing_sensitive`, `file_write_path_not_content_sensitive`, `file_write_unknown_arg_is_unconstrained` (11/11)
- `caprun` (`s9_file_write_block`): `s9_file_write_clean_trusted_pair_is_allowed`, `s9_file_write_tainted_path_blocks_with_genuine_anchor`, `s9_file_write_tainted_contents_blocks_with_genuine_anchor` (3/3)

`Mailpit-backed Linux verification suite PASSED.` No Phase 34 composed live-proof was run — scope held to per-requirement tests only, per the plan's prohibition.

`./scripts/check-invariants.sh` (macOS, run after Task 1): all 4 gates PASSED (no raw `EffectRequest`, `runtime-core` purity intact, mint call sites still restricted to sanctioned loci, `test-fixtures` not a default feature).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FS-01/02/03 are now fully proven: implementation (Plans 01-04) + genuine-taint acceptance (this plan) + zero cfg-linux blind spots on the whole change set.
- Phase 34 (LIVE-01/02) is the only outstanding item: the full composed live proof across exec + fs sinks together, deliberately out of scope here.

---
*Phase: 33-filesystem-read-write-breadth*
*Completed: 2026-07-18*

## Self-Check: PASSED
- FOUND: cli/caprun/tests/s9_file_write_block.rs
- FOUND: .planning/phases/33-filesystem-read-write-breadth/33-05-SUMMARY.md
- FOUND: commit 265ab83 (test task)
- FOUND: commit ec6b19b (docs/summary)
