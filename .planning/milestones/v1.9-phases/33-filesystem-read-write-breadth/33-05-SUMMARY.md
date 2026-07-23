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

## Adversarial-Review Fixes

Post-execution hardening pass applying the verified findings of a fresh
non-self Fable-5 adversarial code-trace
(`33-ADVERSARIAL-REVIEW.md`, reviewed at HEAD `d9ba149`) of the whole
Phase-33 `file.write` TCB diff. 1 MAJOR + 3 MINOR + 3 NIT — all applied.

**MAJOR-1 (`crates/brokerd/src/confirmation.rs`, `crates/brokerd/src/sinks/file_write.rs`) — commit `ad602f2`.**
A blocked `file.write` had no Step-7 confirm-release dispatch arm: a human
`caprun confirm` would durably burn the one-shot confirmation
(`confirm_granted` appended, state → Confirmed) with no write performed and
no terminal `sink_executed`/`sink_execution_failed` event — an audit-DAG
gap. `process.exec` shared the same gap. Fixed by adding
`invoke_file_write_from_resolved` (mirrors `invoke_file_create_from_resolved`
verbatim) plus a `"file.write"` Step-7 dispatch arm, AND a pre-Step-5 entry
guard refusing any sink outside `{file.create, email.send, file.write}`
before `confirm_granted` is appended or state transitions — so an
un-dispatchable sink can never burn the confirmation (row stays `Pending`,
fail-closed-recoverable). Two new tests: a file.write confirm-release
round-trip (release → write lands → `sink_executed` chained → `verify_chain`
true → state Confirmed) and an entry-guard regression (an un-dispatchable
`process.exec` block: `confirm()` returns `Err`, row stays `Pending`, no
`confirm_granted` event exists).

**MINOR-2 (`crates/brokerd/src/server.rs`) — landed inside commit `ad602f2`
(staging-order mistake; the fix itself is correct, only its commit
attribution is off).** `*fd_request_count += 1` could wrap `u32` back to 0
after 2^32 denied round-trips (no `overflow-checks` profile anywhere in the
workspace), failing the RequestFd guard OPEN. Changed to
`fd_request_count.saturating_add(1)`.

**MINOR-3 (`crates/adapter-fs/src/workspace.rs`) — commit `849a1b4`.**
`write_within`'s `openat2(O_WRONLY|O_TRUNC)` would open any existing
non-symlink target, including a FIFO; `O_WRONLY` on a reader-less FIFO
blocks the calling thread indefinitely inside `conn.lock()` — a broker-wide
freeze reachable via a hostile workspace path landing on a FIFO. Fixed with
`O_NONBLOCK` (reader-less FIFO now fails immediately with `ENXIO`) plus a
post-open `fstat`/`S_ISREG` guard rejecting any non-regular target
fail-closed. New `cfg(target_os="linux")` test creates a FIFO via
`nix::unistd::mkfifo` and asserts `write_within` rejects it rather than
hanging. Same commit corrected two doc claims: NIT-6 (re-verified against
live source — `write_within` and `create_exclusive_within` both call
`sync_all()`; the catalog's "asymmetric durability" claim did not hold and
was corrected rather than propagated) and NIT-7 (the non-Linux dev stub now
rejects an absolute `rel_path`, since `PathBuf::join` silently replaces the
base on an absolute argument).

**MINOR-4 (`crates/brokerd/src/server.rs`) — commit `64f4b87`.** Doc-only:
added a note at `MAX_REQUEST_FD_PER_SESSION` naming the three facts its
"per-session" framing actually depends on (RequestFd is worker-only, one
worker connection per session, a second connection is rejected).

**NIT-5 (`crates/brokerd/src/sinks/file_write.rs`) — landed inside commit
`ad602f2`.** Tightened the module doc to "on a *filesystem* error ... append
FIRST" — a missing/dangling arg handle in `resolve_arg` propagates
pre-effect, before any `sink_execution_failed` event would be appended.

### Verification

- `./scripts/check-invariants.sh`: all 4 gates PASSED after every fix (no
  new `EffectRequest`, `runtime-core` purity intact, no new mint site, no
  `test-fixtures` default feature).
- `cargo build --workspace` and `cargo test -p brokerd -p adapter-fs
  --no-fail-fast`: clean on macOS, 116/116 `brokerd` lib tests green
  (including both new `confirmation.rs` tests), 0 failures anywhere.
- **Linux gate 1 (compile enumeration):**
  `MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going' bash scripts/mailpit-verify.sh`
  — TRUE exit `0` (captured before any pipe), 0 `^error` lines, `Mailpit-backed Linux verification suite PASSED.`
- **Linux gate 2 (scoped named tests):**
  `MAILPIT_VERIFY_CMD='cargo test -p brokerd -p adapter-fs -p caprun --no-fail-fast -- <named tests>' bash scripts/mailpit-verify.sh`
  — TRUE exit `0`. All 8 named tests found and `ok`:
  `write_within_fifo_rejected_not_hung`,
  `confirm_on_undispatchable_sink_does_not_burn_confirmation`,
  `confirm_on_pending_file_write_releases_and_writes_file`,
  `invoke_file_write_from_resolved_success_records_sink_executed`,
  `invoke_file_write_from_resolved_failure_records_sink_invocation_failed`,
  `s9_file_write_tainted_path_blocks_with_genuine_anchor`,
  `s9_file_write_tainted_contents_blocks_with_genuine_anchor`,
  `s9_file_write_clean_trusted_pair_is_allowed`.

### Commits

- `ad602f2` — `fix(33): wire file.write confirm-release dispatch + entry guard (MAJOR-1)` (also carries MINOR-2, NIT-5)
- `849a1b4` — `fix(33): reject FIFO/non-regular write_within targets (MINOR-3)` (also carries NIT-6, NIT-7)
- `64f4b87` — `docs(33): note MAX_REQUEST_FD_PER_SESSION's per-session dependency (MINOR-4)`

No new mint site, no `ExecutorDecision` variant, no `EffectRequest` — I2
stays table-entries-only throughout.
