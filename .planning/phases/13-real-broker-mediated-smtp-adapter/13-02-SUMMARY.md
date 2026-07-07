---
phase: 13-real-broker-mediated-smtp-adapter
plan: 02
subsystem: infra
tags: [rusqlite, sqlite-transaction, confirmation, audit-dag, smtp, at-most-once]

# Dependency graph
requires:
  - phase: 13-real-broker-mediated-smtp-adapter (13-01)
    provides: "crates/brokerd/src/sinks/email_smtp.rs — invoke_email_smtp_from_resolved(conn, session_id, effect_id, resolved_args, parent_id, parent_hash)"
provides:
  - "confirm()'s email.send dispatch arm now performs a REAL SMTP send via the frozen resolved_args snapshot, with the pending->confirmed CAS and the durable email_send_attempted append committed in ONE atomic SQLite transaction before any socket opens"
  - "ConfirmOutcome::EmailSendFailed + CLI exit code 7 — the non-swallowed send-failure path, distinct from ConfirmedButSinkFailed/Denied/UnknownEffect/AlreadyTerminal/BlockedLiteralRedacted"
  - "confirm()'s signature is now conn: &mut rusqlite::Connection (deny() unchanged)"
  - "crate-visible email_smtp::SMTP_ENV_LOCK for any brokerd test that mutates CAPRUN_SMTP_* env vars"
affects: ["13-03-negative-net-linux-test", "13-04-crlf-mailpit-acceptance"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Atomic CAS + attempt-append (Pattern 2, RESEARCH.md): conn.transaction() wraps transition_state + append_event as one unit; a zero-row CAS lets the tx auto-rollback on drop (no .commit()), so no email_send_attempted is ever appended for a losing/repeat confirm."
    - "Special-cased dispatch arm bypassing the generic pre-match Step 6 transition_state call — file.create keeps using the generic call unchanged; only email.send owns its own CAS inside its own transaction."
    - "In-process fake SMTP server for a fully-in-process (no Docker/Mailpit) SEND-01 unit test: a background thread speaks just enough real SMTP (banner/EHLO/MAIL FROM/RCPT TO/DATA/dot-terminated body/QUIT) for lettre::SmtpTransport::send to complete successfully."

key-files:
  created: []
  modified:
    - crates/brokerd/src/confirmation.rs
    - cli/caprun/src/main.rs
    - crates/brokerd/src/sinks.rs
    - crates/brokerd/src/sinks/email_smtp.rs
    - cli/caprun/tests/confirm.rs
  deleted:
    - crates/brokerd/src/sinks/email_send.rs

key-decisions:
  - "Promoted email_smtp.rs's test-only ENV_LOCK mutex to a crate-visible `pub(crate) static SMTP_ENV_LOCK` (still #[cfg(test)]) so the new confirmation.rs tests that also mutate the process-global CAPRUN_SMTP_HOST/PORT env vars serialize against email_smtp.rs's own env-mutating test — both compile into the SAME brokerd lib test binary and run under cargo test's default multi-threaded parallelism, so two independent local Mutex statics would not actually have serialized anything (a real, silent flakiness risk the plan and its RESEARCH.md did not flag)."
  - "Added a genuine cross-process integration test (cli/caprun/tests/confirm.rs) proving the CLI exit-code-7 mapping end-to-end via a real `caprun confirm` subprocess against a closed SMTP port, rather than relying only on the compiler's exhaustiveness check over the match arm. Not explicitly required by the plan's file list, but directly serves the plan's own acceptance criterion (\"the CLI maps it to exit code 7\") with executed evidence instead of static reasoning."
  - "confirm()'s email.send Err path does not add its own eprintln! — the adapter (email_smtp.rs::record_send_failed) already logs raw context and appends the opaque email_send_failed event; a second log line in confirm() would be a duplicate, so I mapped Err(_) directly to Ok(ConfirmOutcome::EmailSendFailed), mirroring file.create's existing terse Err(_) => Ok(...) shape (just a different outcome variant)."

patterns-established:
  - "TDD per task, not per sub-behavior (unlike Plan 01): Task 2's RED commit added both SEND-01 and SEND-02 tests together (they exercise the same restructured dispatch arm), then one GREEN commit made both pass."

requirements-completed: [SEND-01, SEND-02]

coverage:
  - id: D1
    description: "confirm()'s email.send CAS + email_send_attempted append commit in ONE atomic SQLite transaction before any SMTP connection opens; a re-issued/duplicate confirm cannot double-fire the send — exactly one email_send_attempted event exists per effect_id regardless of how many confirms are issued"
    requirement: "SEND-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#tests::confirm_email_send_twice_records_exactly_one_attempted_event"
        status: pass
    human_judgment: false
  - id: D2
    description: "An adapter send failure after confirm surfaces as the distinct ConfirmOutcome::EmailSendFailed / CLI exit code 7 (never ConfirmedButSinkFailed's swallow-shape, never confused with denied/unknown/already-terminal/redacted), with a durable email_send_failed event and no auto-retry on re-confirm"
    requirement: "SEND-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/confirmation.rs#tests::confirm_email_send_adapter_failure_yields_email_send_failed"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs#confirm_email_send_adapter_failure_exits_7"
        status: pass
    human_judgment: false
  - id: D3
    description: "The old email_send.rs stub (invoke_email_send_stub) is deleted entirely — no dangling dead code, no stale confirm() reference — per the phase's explicit Open Question 1 DELETE decision"
    verification:
      - kind: other
        ref: "grep -rn 'invoke_email_send_stub|mod email_send' crates/ cli/ returns nothing; cargo build -p brokerd succeeds"
        status: pass
    human_judgment: false
  - id: D4
    description: "file.create's dispatch arm and its grandfathered Err(_) => Ok(ConfirmedButSinkFailed) swallow-shape are byte-for-byte unchanged"
    verification:
      - kind: other
        ref: "git diff on crates/brokerd/src/confirmation.rs shows no changes inside the \"file.create\" => match { ... } arm"
        status: pass
    human_judgment: false

duration: 45min
completed: 2026-07-07
status: complete
---

# Phase 13 Plan 02: Wire Real SMTP Send Into Confirm Path Summary

**`confirm()`'s `email.send` arm now performs a real SMTP send from the frozen `resolved_args` snapshot, with the `pending→confirmed` CAS and the durable `email_send_attempted` append committed in ONE atomic SQLite transaction before any socket opens — closing the double-fire window without a new idempotency token — plus a distinct `ConfirmOutcome::EmailSendFailed`/exit-7 path that never swallows a send failure.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-07
- **Tasks:** 3 (plus one additional cross-process integration test beyond the plan's explicit task list, see Deviations)
- **Files modified:** 5 modified, 1 deleted

## Accomplishments

- Widened `confirm()`'s `conn` parameter from `&rusqlite::Connection` to `&mut rusqlite::Connection` across all 7 call sites (`cli/caprun/src/main.rs` + 6 test call sites in `confirmation.rs`), enabling `conn.transaction()`. `deny()` was left unchanged (no transaction needed).
- Special-cased the `"email.send"` dispatch arm in `confirm()`: it now owns its own `conn.transaction()` wrapping `transition_state` (the CAS) and the durable `email_send_attempted` append as ONE atomic unit — a zero-row CAS lets the transaction auto-rollback on drop (no `.commit()`), so a repeat/re-issued confirm never appends a second attempt event and never opens a socket. The generic, unconditional pre-match `transition_state` call (Step 6) now skips `email.send` entirely, since it would otherwise consume the CAS before the atomic transaction runs.
- After the transaction commits, `confirm()` invokes Plan 01's `email_smtp::invoke_email_smtp_from_resolved` from the frozen snapshot: `Ok` → `Released`; `Err` → the new `ConfirmOutcome::EmailSendFailed` (the adapter already appended the opaque `email_send_failed` event and logged raw context — `confirm()` does not swallow and does not double-log).
- Added `ConfirmOutcome::EmailSendFailed` and wired `cli/caprun/src/main.rs`'s exit-code match to `7`, distinct from `Denied` (2) / `ConfirmedButSinkFailed` (3) / `UnknownEffect` (4) / `AlreadyTerminal` (5) / `BlockedLiteralRedacted` (6).
- Deleted `crates/brokerd/src/sinks/email_send.rs` (the `invoke_email_send_stub` no-op) and its `pub mod email_send;` declaration, after grep-confirming no other reference remained anywhere in `crates/`/`cli/`.
- Built an in-process fake SMTP server (a background thread speaking real SMTP: banner/EHLO/MAIL FROM/RCPT TO/DATA/dot-terminated body/QUIT) so the SEND-01 "first confirm actually sends and Releases" test runs fully in-process, with no Docker/Mailpit dependency — mirroring Plan 01's own "unit tests run fully in-process" discipline.
- Promoted `email_smtp.rs`'s test-only env-mutation mutex to a crate-visible `SMTP_ENV_LOCK` so the new `confirmation.rs` tests that also set `CAPRUN_SMTP_HOST`/`PORT` serialize correctly against `email_smtp.rs`'s own env-mutating test under `cargo test`'s parallel-thread default (see Deviations).
- Added a genuine cross-process integration test in `cli/caprun/tests/confirm.rs` proving the exit-code-7 mapping via a real `caprun confirm` subprocess against a closed SMTP port.

## Task Commits

Each task was committed atomically (TDD RED/GREEN pair for Task 2):

1. **Task 1: Widen confirm() to &mut Connection across all 7 call sites** — `08eb3a9` (refactor)
2. **Task 2: Special-case email.send dispatch with atomic CAS + email_send_attempted transaction**
   - `6d0f4d9` (test) — RED: fails to compile (`ConfirmOutcome::EmailSendFailed` does not exist yet); also promotes `SMTP_ENV_LOCK`
   - `8cf950b` (feat) — GREEN: atomic transaction + real send wiring, both SEND-01/SEND-02 tests pass
3. **Task 3: Delete the dead email_send.rs stub** — `a7f46f8` (chore)
4. **Additional (beyond the plan's task list): cross-process exit-code-7 integration test** — `d9acf29` (test)

**Plan metadata:** this commit (docs: complete plan)

## Files Created/Modified

- `crates/brokerd/src/confirmation.rs` — `confirm()` signature widened to `&mut Connection`; `ConfirmOutcome::EmailSendFailed` variant added; the `email.send` dispatch arm rewritten to own its own atomic CAS+attempt transaction and invoke the real adapter; new SEND-01/SEND-02 unit tests + a fake in-process SMTP server helper.
- `cli/caprun/src/main.rs` — `confirm()` call site updated to `&mut conn`; new `EmailSendFailed => (7, ...)` exit-code arm.
- `crates/brokerd/src/sinks.rs` — removed `pub mod email_send;`; updated the module doc comment to describe the current real-effect dispatch shape.
- `crates/brokerd/src/sinks/email_smtp.rs` — promoted the test-only `ENV_LOCK` to a crate-visible `pub(crate) static SMTP_ENV_LOCK` (still `#[cfg(test)]`); no production-code change.
- `crates/brokerd/src/sinks/email_send.rs` — **deleted** (the `invoke_email_send_stub` no-op, dead code after the rewire).
- `cli/caprun/tests/confirm.rs` — added `confirm_email_send_adapter_failure_exits_7`, a real-subprocess integration test for the exit-code-7 path, plus its `seed_pending_email_send_block`/`run_caprun_verb_with_env` helpers.

## Decisions Made

- **`SMTP_ENV_LOCK` promoted to crate-visible** (see key-decisions above) — a real, previously-latent test-flakiness risk this plan's new tests would otherwise have introduced, closed via Rule 1 (bug/correctness fix) rather than left for a future phase to discover as a flaky-CI mystery.
- **Added a cross-process CLI integration test for exit code 7** (see key-decisions above) — Rule 2-adjacent (a missing-verification gap for an explicit acceptance criterion), scoped as test-only, no production-code change, and directly strengthens confidence the CLI contract is genuinely reachable rather than merely type-checked.
- **No duplicate logging on the email.send Err path** — `confirm()` maps `Err(_) => Ok(ConfirmOutcome::EmailSendFailed)` without an additional `eprintln!`, since the adapter (`email_smtp.rs::record_send_failed`) already logs raw context and appends the opaque `email_send_failed` event; this mirrors `file.create`'s existing terse `Err(_) => Ok(...)` arm shape.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Promoted email_smtp.rs's `ENV_LOCK` to a crate-visible `SMTP_ENV_LOCK`**
- **Found during:** Task 2 (writing the SEND-01/SEND-02 tests)
- **Issue:** The new tests in `confirmation.rs` needed to mutate the same process-global `CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` env vars that `email_smtp.rs`'s own existing test already mutates under its own private `ENV_LOCK` mutex. Both `mod tests` blocks compile into the SAME `brokerd` lib test binary, which `cargo test` runs with multiple threads by default — two independently-scoped local mutexes would not actually serialize anything, reintroducing exactly the race the original `ENV_LOCK` comment says it exists to prevent, just across a module boundary neither the plan nor its RESEARCH.md flagged.
- **Fix:** Moved the mutex to module scope in `email_smtp.rs` as `#[cfg(test)] pub(crate) static SMTP_ENV_LOCK`, updated `email_smtp.rs`'s own test to use it, and had the new `confirmation.rs` tests acquire the same lock via `crate::sinks::email_smtp::SMTP_ENV_LOCK`.
- **Files modified:** `crates/brokerd/src/sinks/email_smtp.rs`, `crates/brokerd/src/confirmation.rs`
- **Verification:** `cargo test -p brokerd --lib` run 5 times consecutively with no flakes (all 52 tests pass each run).
- **Committed in:** `6d0f4d9` (Task 2 RED commit)

**2. [Rule 2 - Missing Verification] Added a real cross-process exit-code-7 test**
- **Found during:** Task 2 acceptance-criteria review
- **Issue:** The plan's acceptance criteria state "...and the CLI maps it to exit code 7" but the plan's task file list only names `confirmation.rs`/`main.rs` — the exit-code mapping was verifiable only by reading the match arm, not by executing it, unlike the existing file.create exit-code paths which already have real-subprocess coverage in `cli/caprun/tests/confirm.rs`.
- **Fix:** Added `confirm_email_send_adapter_failure_exits_7` to `cli/caprun/tests/confirm.rs`, reusing that file's established seed-directly-then-spawn-real-binary pattern, plus small helper additions (`run_caprun_verb_with_env`, `seed_pending_email_send_block`).
- **Files modified:** `cli/caprun/tests/confirm.rs`
- **Verification:** `cargo test -p caprun --test confirm` — 4/4 pass, including the new test.
- **Committed in:** `d9acf29` (separate, test-only commit after Task 3)

---

**Total deviations:** 2 auto-fixed (1 bug/correctness, 1 missing-verification gap). Both are test-only or test-infrastructure changes — no production-code scope creep, no architectural changes.
**Impact on plan:** Necessary for the plan's own tests to be reliable (deviation 1) and for its own acceptance criterion to be genuinely verified rather than asserted by code inspection (deviation 2).

## Issues Encountered

- Building the SEND-01 in-process fake SMTP server required reading `lettre 0.11.22`'s actual client-connection source (`~/.cargo/registry/.../lettre-0.11.22/src/transport/smtp/client/connection.rs`) to get the exact protocol sequence right (banner → EHLO → MAIL FROM → RCPT TO → DATA → dot-terminated body → the eventual QUIT sent when the transport's connection pool is dropped at function-scope end). Verified empirically — the test passed on the first attempt after writing the server against the confirmed protocol sequence, and remained stable across 5 repeated runs.
- No other issues. All three planned tasks plus the two Deviations passed on schedule; `cargo build --workspace` and `cargo test --workspace --no-fail-fast` both green throughout (Linux-only sandbox tests show their expected "0 passed" stub behavior on this macOS dev machine, per CLAUDE.md — not a gap).

## User Setup Required

None — no external service configuration required. All of this plan's tests (SEND-01's real-send success path, SEND-02's failure path, and the CLI exit-code-7 integration test) run fully in-process/in-subprocess with no Docker/Mailpit dependency. The live Mailpit-backed negative-net and CRLF-fixture acceptance tests remain Plan 03/04's job, unaffected by this plan.

## Next Phase Readiness

- `confirm()`'s `email.send` path is fully wired to Plan 01's real adapter with atomic at-most-once semantics (SEND-01) and non-swallowed failure surfacing (SEND-02) — both requirements complete.
- `crates/brokerd/src/sinks/email_send.rs` is gone; the only `email.send` dispatch target is `email_smtp.rs`.
- Plan 03 (Linux-only kernel-enforced negative-net assertion) and Plan 04 (CRLF/Mailpit acceptance test) can proceed independently — neither depends on anything this plan changed beyond the now-real `email.send` dispatch path they will exercise end-to-end against a live Mailpit.
- No blockers.

---
*Phase: 13-real-broker-mediated-smtp-adapter*
*Completed: 2026-07-07*

## Self-Check: PASSED

- FOUND: crates/brokerd/src/confirmation.rs (email.send atomic dispatch arm present)
- FOUND: crates/brokerd/src/sinks/email_smtp.rs (SMTP_ENV_LOCK present)
- MISSING (expected — deleted): crates/brokerd/src/sinks/email_send.rs
- FOUND: commit 08eb3a9
- FOUND: commit 6d0f4d9
- FOUND: commit 8cf950b
- FOUND: commit a7f46f8
- FOUND: commit d9acf29
