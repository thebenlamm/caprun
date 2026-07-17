# HARDEN-06 Regression Audit — Independent Re-Confirmation

**Phase:** 30-regression-live-proof, Plan 01
**Purpose:** Independently sweep every Phase 27/28/29 hardening test file for
`#[ignore]`, weakened assertions, or stale/self-triggering skip conditions
before Plan 02 runs the live-proof gate. Mirrors Phase 25's
`25-REGRESSION-AUDIT.md` structure — a fresh re-run this session, not a copy
of RESEARCH's prior counts.

## Search 1: `#[ignore]` sweep across the whole workspace

```bash
grep -rn '#\[ignore\]' crates/ cli/ --include='*.rs' | grep -v '/target/'
```

**Result: 0 matches.** No test anywhere in `crates/` or `cli/` is marked
`#[ignore]`. There is nothing to give a CLEARED/NEEDS-FIX verdict on for this
sweep — the search space is empty. **Verdict: CLEARED (no ignored tests
exist).**

## Search 2: weakened-assertion / stale-comment sweep in the Phase 27/28/29
## hardening test files

```bash
grep -rn 'TODO\|FIXME\|XXX' \
  crates/brokerd/tests/harden01_session_integrity.rs \
  crates/brokerd/tests/replay_cas.rs \
  cli/caprun/tests/harden04_featureless_create_session.rs \
  crates/brokerd/src/audit.rs \
  crates/executor/src/sink_sensitivity.rs \
  crates/executor/tests/executor_decision.rs \
  cli/caprun/tests/s9_live_block.rs
```

**Result: 0 matches.** No placeholder/deferred-work markers in any of the 7
files carrying the phase's negative-test assertions.

```bash
grep -rn 'assert!(true)' crates/ cli/ --include='*.rs' | grep -v '/target/'
```

**Result: 0 matches.** No trivially-true assertion anywhere in the
workspace.

```bash
grep -n '^\s*//\s*assert' \
  crates/brokerd/tests/harden01_session_integrity.rs \
  crates/brokerd/tests/replay_cas.rs \
  cli/caprun/tests/harden04_featureless_create_session.rs \
  crates/brokerd/src/audit.rs
```

**Result: 1 hit** (`replay_cas.rs:357`), but it is a prose comment fragment
inside a multi-line explanatory comment block ("...avoids a false PASS from
checking too early.") ending mid-sentence in the phrase "before asserting the
exact count" — not a commented-out `assert!(...)` call. Read in full context
(lines 340-370), the line above it is a live `assert_eq!` on
`second_decision` and the block below performs a live poll-and-count that
feeds a real assertion further down. **Verdict: not a weakened test — a
false positive of the grep pattern, correctly triaged as non-issue.**

### Assertion-count sanity check (teeth confirmation, not exhaustive)

| File | `assert*` occurrences |
|------|------------------------|
| `crates/brokerd/tests/harden01_session_integrity.rs` | 11 |
| `crates/brokerd/tests/replay_cas.rs` | 9 |
| `cli/caprun/tests/harden04_featureless_create_session.rs` | 9 |
| `crates/executor/tests/executor_decision.rs` | 44 |

Each hardening test file carries a nonzero, substantial count of live
assertions — none has been reduced to a bare `Ok(())`/early-`return`-only
body.

### `return;` sweep — confirm no test silently short-circuits before its
### assertions

```bash
grep -rn 'return;' crates/brokerd/tests/*.rs cli/caprun/tests/*.rs | grep -v /target/
```

Relevant hits in the audited file set:

- `cli/caprun/tests/harden04_featureless_create_session.rs:215` — this IS the
  documented, intentional self-skip branch (see the dedicated evaluation
  below). It is gated on `brokerd::TEST_FIXTURES_ACTIVE`, a compile-time
  const, never on the response the test is proving. **CORRECT — the
  documented guard, not a weakened test.**
- `cli/caprun/tests/s9_live_block.rs:769` — inside the `wait_for_recipient_captured`
  polling **helper function**, not inside `s9_control_ab_taint_driven` itself
  (that test starts at line 813, after this helper's module closes). The
  `return;` fires when the poll loop finds the expected message and simply
  ends the poll early — the function `panic!`s on timeout if the condition
  is never met (line 774). **CORRECT — a poll-loop success return, not a
  weakened assertion.**
- `cli/caprun/tests/live_acceptance_v1_3.rs:334`,
  `live_acceptance_v1_4_composed.rs:556,592`,
  `llm_planner_live_accept.rs:175,212` — pre-existing (pre-v1.6) e2e/live
  test files, outside Phase 27/28/29's scope; not part of this audit's
  target set. Noted for completeness, not re-adjudicated here (out of
  scope — HARDEN-06 audits Phase 27/28/29 hardening tests specifically).

## Search 3: `#[cfg(not(target_os = "linux"))]` no-op-stub sweep in the
## hardening files (checking for a stub that quietly weakens a Linux-gated
## negative test on other platforms)

```bash
grep -n 'cfg(not(target_os' \
  crates/brokerd/tests/harden01_session_integrity.rs \
  crates/brokerd/tests/replay_cas.rs \
  cli/caprun/tests/harden04_featureless_create_session.rs \
  cli/caprun/tests/s9_live_block.rs
```

**Result: 0 matches** in any of the 4 files. None of these hardening test
files defines a non-Linux stub body at all — the entire Linux-gated test
module is simply absent on macOS (`#[cfg(target_os = "linux")] mod
linux_tests { ... }` or `#![cfg(target_os = "linux")]` at the file top), so
`cargo test` on macOS reports these as excluded, not as a passing no-op stub.
This is CLAUDE.md's documented, expected behavior ("`cargo test` on macOS
shows these as '0 passed' — that is expected, not a gap") and not a weakened
assertion.

## Dedicated evaluation: `harden04_featureless_create_session`'s self-skip

This is the one deliberate, documented skip branch anywhere in the audited
file set, and it deserves explicit adjudication rather than a blanket
CLEARED verdict.

**The guard, read directly (lines 184-216):**

```rust
if brokerd::TEST_FIXTURES_ACTIVE {
    eprintln!(
        "harden04_featureless_create_session: SKIPPING the D-10 \
         negative assertion -- brokerd::TEST_FIXTURES_ACTIVE is true, ..."
    );
    server_handle.abort();
    return;
}
// only reached when the build graph is genuinely featureless
assert!(matches!(resp, BrokerResponse::Error { .. }), "D-10 VIOLATION: ...");
```

**Is this a correct guard or a weakened test?** CORRECT — it is keyed on
`brokerd::TEST_FIXTURES_ACTIVE`, a `const` that reflects whether **this
specific build graph** actually compiled `brokerd` with the `test-fixtures`
feature, not on the response variant the test is trying to prove.

The distinction matters: if the skip were instead keyed on the *response*
(e.g. "skip if `resp` is `SessionCreated`"), a genuine D-10 regression — the
forced-Active mint arm leaking into a build that should be featureless —
would produce exactly the `SessionCreated` response the test exists to
catch, and that response would then be misread as "ambient unification must
be active" and silently routed into the skip branch. The test would report
green while providing zero protection against the exact regression it was
built for.

Because `TEST_FIXTURES_ACTIVE` reflects the actual compiled feature set
(resolved once, at compile time, from whether `test-fixtures` was enabled
for this build unit — independent of any runtime request/response), it
cannot be fooled by the very regression it is meant to detect: a genuine
D-10 break can never cause `TEST_FIXTURES_ACTIVE` to flip from `false` to
`true`. When `TEST_FIXTURES_ACTIVE` is `false` (the genuinely featureless,
scoped `-p caprun` invocation), the hard `assert!` at the bottom of the test
ALWAYS runs and ALWAYS evaluates the real `resp` — there is no path from a
featureless build back into the skip branch.

`scripts/verify-harden04-featureless.sh` (Plan 30-01 Task 1) is precisely
the mechanism that forces the non-skip path to run: it invokes `cargo test
-p caprun --test harden04_featureless_create_session` (never `--workspace`),
which keeps `crates/brokerd`'s own test targets — and their self dev-dep on
`test-fixtures` — out of the build graph, and it independently re-detects
the skip sentinel in the captured log as a hard failure condition, so a
green run of that script categorically cannot happen while this test's skip
branch fired.

**Verdict: CORRECT, feature-keyed guard — not a weakened test.**

## Per-criterion coverage confirmation (live re-grep, HARDEN-06's 5 criteria)

Each anchor below was independently re-grepped this session (not copied from
RESEARCH.md) and matches RESEARCH's Per-Criterion Verdict Table exactly — no
line-number drift since that research was recorded (2026-07-17, same day).

| # | Criterion | file:test-name | Verdict |
|---|-----------|-----------------|---------|
| 1 | Full workspace regression green via bare `scripts/mailpit-verify.sh` | N/A — whole-suite run, executed live in Plan 30-02 | COVERED (mechanism confirmed present and unmodified; live execution deferred to Plan 30-02 per this plan's scope) |
| 2 | Forged/tampered audit chain rejected by `verify_chain` | `crates/brokerd/src/audit.rs:1522` `self_consistent_forgery_without_key_is_rejected`; `:1673` `tail_truncation_detected_via_anchor_mismatch`; `:1779` `legacy_db_without_anchor_fails_closed`; `:1725` `orphan_event_injection_detected_via_live_count` (bonus F2-fix regression, Phase 28) | COVERED |
| 3 | Replayed Allowed `email.send` delivers exactly once | `crates/brokerd/tests/replay_cas.rs:255` `allowed_email_send_replay_delivers_once` (`#![cfg(target_os = "linux")]` at line 24) | COVERED |
| 4 | Forced-Active `CreateSession` arm absent from a featureless build | `cli/caprun/tests/harden04_featureless_create_session.rs:125` `featureless_create_session_denied_even_with_flag_set` — SILENTLY INERT under the bare recipe; PROVEN only via `scripts/verify-harden04-featureless.sh` (this plan's Task 1) | COVERED (test exists and has real teeth; formalized non-skip invocation now exists as of this plan; authoritative green run is Plan 30-02's job) |
| 5 | `RequestFd` on untrusted path demotes to Draft; CONTROL-01 clean path unaffected | `crates/brokerd/tests/harden01_session_integrity.rs:196` `fd_grant_on_untrusted_path_demotes_without_report_claims` (demotion) + `:397` `fd_grant_on_trusted_path_stays_active` (clean path, unit-level) + `cli/caprun/tests/s9_live_block.rs:813` `s9_control_ab_taint_driven` (clean path, full live end-to-end) | COVERED |

**Bonus (HARDEN-05, part of the Phase 27-29 hardening scope though not one
of the 5 numbered HARDEN-06 criteria in ROADMAP wording — included here for
completeness since RESEARCH's own DESIGN §j blast-radius table lists it as a
Phase 29 residual audited alongside the rest):** `crates/executor/src/sink_sensitivity.rs:360`
`file_create_contents_is_content_sensitive`,
`crates/executor/tests/executor_decision.rs:795`
`file_create_contents_role_mismatch_denies`, and
`cli/caprun/tests/s9_live_block.rs:556` `s9_live_file_create_clean_allow`
(clean-path regression) — all confirmed present at their cited anchors,
**COVERED**.

## Bottom line

**No weakened, ignored, or self-defeating hardening test found across
Phase 27/28/29 — regression coverage is intact.**

- 0 `#[ignore]` anywhere in `crates/`/`cli/`.
- 0 `assert!(true)` anywhere in the workspace.
- 0 TODO/FIXME/XXX markers in any audited hardening test file.
- 0 non-Linux no-op stub bodies quietly weakening a Linux-gated negative
  test.
- The one deliberate skip branch (`harden04_featureless_create_session`) is
  a correct, feature-keyed guard that cannot be fooled by the regression it
  exists to catch, and this plan adds the script
  (`scripts/verify-harden04-featureless.sh`) that forces its non-skip path
  to actually run.
- All 5 HARDEN-06 success criteria map to an exact, re-confirmed
  `file:test-name` anchor with a live COVERED verdict; 0 gaps found.

**No production, test, or TCB code was changed by this audit.** Only this
documentation file and (in Task 1 of this same plan) the new standalone
verification script were added.
