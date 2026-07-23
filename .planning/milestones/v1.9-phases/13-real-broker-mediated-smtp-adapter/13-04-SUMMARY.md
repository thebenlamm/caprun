---
phase: 13-real-broker-mediated-smtp-adapter
plan: 04
subsystem: infra
tags: [mailpit, docker, smtp, crlf-injection, acceptance-test, colima]

# Dependency graph
requires:
  - phase: 13-real-broker-mediated-smtp-adapter (13-01)
    provides: "crates/brokerd/src/sinks/email_smtp.rs — build_message, invoke_email_smtp_from_resolved"
  - phase: 13-real-broker-mediated-smtp-adapter (13-02)
    provides: "confirmation.rs::confirm()'s email.send atomic CAS + real send dispatch"
  - phase: 13-real-broker-mediated-smtp-adapter (13-03)
    provides: "confine-probe smtp op + negative_net_smtp_mailpit Linux test"
provides:
  - "scripts/mailpit-verify.sh — reusable Mailpit sidecar + Linux verification helper (Phase 17 will reuse it)"
  - "crates/brokerd/tests/email_smtp_acceptance.rs — SMTP-03 real-capture + SMTP-05 CRLF-fixture acceptance tests, verified passing on real Linux against a live Mailpit instance"
  - "Empirically-confirmed Mailpit HTTP API field path (DETAIL endpoint, always-array To/Cc/Bcc) — recorded for any future Mailpit-asserting test"
  - "Fix to Plan 03's negative_net_smtp_mailpit test (resolve Mailpit IP outside confinement) — a regression this plan's own Mailpit-hostname wiring introduced and then fixed"
affects: ["phase-17-live-acceptance-run"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dependency-light raw HTTP/1.1 client (TcpStream + Connection: close + read-to-EOF) instead of adding a new HTTP crate dependency for test-only Mailpit API calls"
    - "Shared external-resource test lock (MAILPIT_TEST_LOCK) + explicit inbox-clear (DELETE /api/v1/messages) to serialize acceptance tests against one live sidecar, mirroring email_smtp.rs::SMTP_ENV_LOCK's rationale"
    - "Resolve a Docker sidecar's container IP OUTSIDE any confined child process, then pass the concrete IP (never a hostname) into a seccomp-confined probe — avoids a DNS lookup itself needing a blocked socket() call"

key-files:
  created:
    - scripts/mailpit-verify.sh
    - crates/brokerd/tests/email_smtp_acceptance.rs
  modified:
    - CLAUDE.md

key-decisions:
  - "Assert against Mailpit's DETAIL endpoint (GET /api/v1/message/{ID}), never the LIST endpoint (GET /api/v1/messages) — empirically confirmed the two endpoints have DIFFERENT shapes for absent Cc/Bcc (LIST returns null, DETAIL always returns an array). RESEARCH.md's Pitfall 4 predicted an array shape from the swagger schema but did not flag this LIST-vs-DETAIL divergence; this plan discovered and documented it live."
  - "Resolve the Mailpit sidecar's container IP via `docker inspect` and pass the concrete IP as CAPRUN_SMTP_HOST, never the Docker DNS hostname 'mailpit' — the hostname form breaks Plan 03's negative_net_smtp_mailpit test (see Deviations)."
  - "Used a raw std::net::TcpStream-based HTTP/1.1 client for the two Mailpit HTTP API calls (GET/DELETE) instead of adding a new http-client crate — serde_json was already a brokerd dependency, keeping the acceptance test dependency-light per the plan's own instruction."

patterns-established:
  - "Pattern: TDD RED/GREEN for integration/acceptance tests whose production code already exists (from Plans 01/02) — RED is a deliberately wrong assertion proving the harness genuinely observes live Mailpit state (not a tautology), confirmed FAILING against a real Mailpit instance on real Linux; GREEN corrects the assertion, confirmed PASSING the same way. Mirrors Plan 03's own precedent for TDD-declared tasks whose implementation predates the test."

requirements-completed: [SMTP-03, SMTP-05]

coverage:
  - id: D1
    description: "scripts/mailpit-verify.sh starts a Mailpit sidecar on a user-defined Docker network, runs the existing unprivileged rust:1 verification container on that network (installing libssl-dev/pkg-config first), and tears the sidecar down afterward"
    requirement: "SMTP-03"
    verification:
      - kind: other
        ref: "bash -n scripts/mailpit-verify.sh"
        status: pass
      - kind: integration
        ref: "bash scripts/mailpit-verify.sh (full run, real Colima+Docker) — exit 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "SMTP-03: a confirmed email.send effect (clean recipient/subject/body) results in exactly one message captured by Mailpit, addressed to the intended recipient, driven entirely through confirm() (never a test-only bypass)"
    requirement: "SMTP-03"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/email_smtp_acceptance.rs#smtp_03_confirmed_send_captured_by_mailpit (Linux-only, run via scripts/mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D3
    description: "SMTP-05: a tainted body carrying a CR/LF-then-Bcc: attacker@evil.com injection sequence does NOT smuggle a recipient — Mailpit's parsed To/Cc/Bcc show ONLY the intended recipient, verified via Mailpit's HTTP API DETAIL endpoint, not merely that the send succeeded"
    requirement: "SMTP-05"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/email_smtp_acceptance.rs#smtp_05_crlf_body_cannot_smuggle_recipient (Linux-only, run via scripts/mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D4
    description: "cargo build -p brokerd --tests compiles clean on macOS (the new test file is cfg(target_os=\"linux\")-gated per item, matching project convention)"
    verification:
      - kind: other
        ref: "cargo build -p brokerd --tests (macOS)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Full cargo test --workspace --no-fail-fast passes end-to-end under scripts/mailpit-verify.sh on real Linux, including Plan 03's negative_net_smtp_mailpit test (fixed regression) and both new SMTP-03/05 tests"
    verification:
      - kind: integration
        ref: "bash scripts/mailpit-verify.sh (full workspace run) — exit 0, all test binaries ok"
        status: pass
    human_judgment: false

duration: 70min
completed: 2026-07-08
status: complete
---

# Phase 13 Plan 04: SMTP-03/SMTP-05 Mailpit Acceptance Evidence Summary

**A reusable Mailpit sidecar helper (`scripts/mailpit-verify.sh`) plus a live-Mailpit acceptance test file proving both that a confirmed `email.send` effect is really captured by Mailpit (SMTP-03) and that a CR/LF-then-`Bcc:` body injection cannot smuggle a recipient into the captured envelope (SMTP-05) — verified empirically on real Linux, not assumed from lettre's reputation.**

## Performance

- **Duration:** ~70 min
- **Started:** 2026-07-07 (session continuation, retry after an unrelated concurrent-commit halt)
- **Completed:** 2026-07-08
- **Tasks:** 3 (plus one Rule-1 deviation fix discovered during final verification)
- **Files modified:** 3 (2 created, 1 modified)

## Accomplishments

- Built `scripts/mailpit-verify.sh`: starts an `axllent/mailpit` sidecar on a user-defined Docker network, runs the existing unprivileged `rust:1` Colima+Docker verification container on that same network (installing `libssl-dev`/`pkg-config` first — lettre's `native-tls` transitive dependency), and tears the sidecar down unconditionally via an `EXIT` trap.
- Empirically smoke-tested the Docker network wiring by hand (not merely assumed from RESEARCH.md's Assumption A2): discovered container-name DNS requires an explicit `--network-alias`, and — more consequentially — discovered that passing a DNS *hostname* as `CAPRUN_SMTP_HOST` breaks Plan 03's kernel-confined negative-net test (see Deviations). Fixed by resolving Mailpit's container IP outside confinement and using that IP for the actual verification run.
- Empirically confirmed Mailpit's live HTTP API schema by hand (sending real SMTP messages via `python smtplib`, then `curl`ing both endpoints) rather than trusting RESEARCH.md's WebFetch-summarized (Pitfall 4, MEDIUM confidence) schema: the LIST endpoint (`GET /api/v1/messages`) returns `null` for absent `Cc`/`Bcc`, while the DETAIL endpoint (`GET /api/v1/message/{ID}`) always returns an array (`[]` when absent) — a real divergence not flagged by the research. Documented this in both the script and the test file.
- Added `crates/brokerd/tests/email_smtp_acceptance.rs` with two `#[cfg(target_os = "linux")]`-gated acceptance tests, each following TDD RED (deliberately wrong assertion, confirmed failing against a live Mailpit) → GREEN (corrected assertion, confirmed passing):
  - `smtp_03_confirmed_send_captured_by_mailpit` (SMTP-03): seeds a Pending `email.send` block, drives it through the real `confirm()` entry point, polls Mailpit's HTTP API, and asserts exactly one captured message addressed to the intended recipient.
  - `smtp_05_crlf_body_cannot_smuggle_recipient` (SMTP-05): same harness with a body literal carrying `"hi there\r\nBcc: attacker@evil.com"`, asserting Mailpit's parsed `To`/`Cc`/`Bcc` contain ONLY the intended recipient and never the attacker address.
- Built a dependency-light raw HTTP/1.1 client (`std::net::TcpStream` + `Connection: close` + read-to-EOF) for the two Mailpit API calls, avoiding a new HTTP crate dependency — `serde_json` was already a `brokerd` dependency.
- Added `MAILPIT_TEST_LOCK` + `clear_mailpit_inbox` to serialize the two acceptance tests against the one shared external Mailpit inbox (Rule 1 fix, discovered adding the second test — see Deviations).
- Extended `CLAUDE.md`'s "Linux-only security tests" section with a pointer to `scripts/mailpit-verify.sh` for phases needing Mailpit (13, 17).
- **Verified the full `cargo test --workspace --no-fail-fast` passes end-to-end on real Linux via `bash scripts/mailpit-verify.sh` (exit 0)** — including both new SMTP-03/05 tests and Plan 03's `negative_net_smtp_mailpit` test (fixed regression, see Deviations), not merely "compiles on macOS."

## Task Commits

Each task was committed atomically (TDD RED/GREEN pairs for Tasks 2/3):

1. **Task 1: Mailpit sidecar helper + verification recipe extension + live schema probe** — `5bb2301` (feat)
2. **Task 2: SMTP-03 acceptance test — confirmed effect captured by Mailpit**
   - `435fd06` (test) — RED: deliberate wrong message-count assertion, confirmed FAILING against live Mailpit
   - `d341ead` (feat) — GREEN: corrected assertion, confirmed PASSING against live Mailpit
3. **Task 3: SMTP-05 CRLF fixture — tainted body cannot smuggle a recipient**
   - `dac907b` (test) — RED: deliberate "attacker address IS present" assertion, confirmed FAILING (Bcc was actually empty — the correct outcome), plus `MAILPIT_TEST_LOCK`/`clear_mailpit_inbox` (Rule 1 fix)
   - `0c9bb7d` (feat) — GREEN: corrected to the real negative assertion, confirmed PASSING
4. **Additional (beyond the plan's task list, Rule 1 deviation): fix Mailpit-hostname regression in the negative-net test** — `4ff05af` (fix)

**Plan metadata:** this commit (docs: complete plan)

## Files Created/Modified

- `scripts/mailpit-verify.sh` — new executable: Mailpit sidecar lifecycle + Linux verification recipe extension, with the empirically-confirmed field path and network-wiring notes recorded inline.
- `crates/brokerd/tests/email_smtp_acceptance.rs` — new acceptance test file: SMTP-03/SMTP-05, a dependency-light raw HTTP client, and a shared seed-and-confirm harness.
- `CLAUDE.md` — "Linux-only security tests" section extended with a pointer to the new script.

## Decisions Made

- **Assert against Mailpit's DETAIL endpoint, not the LIST endpoint** — see key-decisions in frontmatter. This was empirically discovered, not assumed.
- **Resolve Mailpit's container IP via `docker inspect` rather than relying on Docker DNS** for the actual verification run — required to avoid breaking Plan 03's kernel-confined negative-net test (see Deviations). The `--network-alias mailpit` convenience alias is kept in the script for manual debugging only.
- **Raw `TcpStream`-based HTTP client instead of a new HTTP crate dependency** — matches the plan's "keep it dependency-light" instruction; `serde_json` was already available.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `MAILPIT_TEST_LOCK` + `clear_mailpit_inbox` — cross-test race on one shared external Mailpit inbox**
- **Found during:** Task 3 (writing the second acceptance test in the same file)
- **Issue:** With two tests now sharing ONE external Mailpit inbox (a real SMTP server, not an in-process fixture), `cargo test`'s default multi-threaded runner would let them race — one test's `wait_for_message_count` could observe the OTHER test's message, producing flaky pass/fail depending on interleaving.
- **Fix:** Added a `MAILPIT_TEST_LOCK` mutex (mirrors `email_smtp.rs::SMTP_ENV_LOCK`'s rationale for a different shared resource) that each test acquires for its entire body, plus `clear_mailpit_inbox()` (Mailpit's `DELETE /api/v1/messages`) called at the start of each test so it starts from a known-empty inbox.
- **Files modified:** `crates/brokerd/tests/email_smtp_acceptance.rs`
- **Verification:** Ran the full test file 3 consecutive times under `cargo test`'s default parallel runner on real Linux — stable pass every time, in varying test-execution order.
- **Committed in:** `dac907b` (Task 3 RED commit)

**2. [Rule 1 - Bug] Resolve Mailpit's container IP outside confinement — Mailpit-hostname regression in Plan 03's negative-net test**
- **Found during:** Final full-workspace verification pass (`bash scripts/mailpit-verify.sh`, run to confirm `cargo test --workspace --no-fail-fast` truly passes, per this plan's own instruction to verify end-to-end rather than settle for "compiles")
- **Issue:** `mailpit-verify.sh`'s Task-1 design set `CAPRUN_SMTP_HOST=mailpit` (a Docker DNS hostname, per RESEARCH.md's own recommended recipe). Plan 03's pre-existing `negative_net_smtp_mailpit` test drives `confine-probe smtp <host> <port>` INSIDE a seccomp-confined child process. That process's default-deny net filter blocks `socket()` unconditionally — which ALSO blocks the DNS query a hostname lookup needs, so the confined probe failed with "Temporary failure in name resolution" (exit 2, unexpected error) instead of the expected `EPERM`-at-`connect()` proof (exit 0). This broke a previously-passing Plan 03 test, directly caused by this plan's own Task 1 environment-variable choice.
- **Fix:** Resolve the Mailpit sidecar's container IP via `docker inspect` OUTSIDE any confined process, then pass that concrete IP (never the hostname) as `CAPRUN_SMTP_HOST` for the actual verification run. This avoids any DNS lookup ever needing to happen inside the confined probe process. The `--network-alias mailpit` convenience alias is kept in the script (for manual `curl`/shell debugging) but is no longer relied on by the automated run.
- **Files modified:** `scripts/mailpit-verify.sh`
- **Verification:** Full `cargo test --workspace --no-fail-fast` under `bash scripts/mailpit-verify.sh` now exits 0, with `negative_net_smtp_mailpit` passing (`[confine-probe] smtp: correctly blocked (errno=1, addr=172.18.0.2:1025)`) alongside both new SMTP-03/05 tests.
- **Committed in:** `4ff05af` (separate fix commit, after Task 3's GREEN)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — bug/correctness, both test-infrastructure only, no production-code scope creep).
**Impact on plan:** Both were necessary for the plan's own tests to be reliable (deviation 1) and for the full verification recipe this plan is responsible for delivering to actually pass end-to-end rather than silently regress a sibling plan's test (deviation 2). No architectural changes.

## Issues Encountered

- The RESEARCH.md's Pitfall 4 schema prediction (WebFetch-summarized, MEDIUM confidence) was directionally correct (array-of-`{Name, Address}`) but missed a real LIST-vs-DETAIL endpoint divergence for absent `Cc`/`Bcc` — resolved by empirically probing a live Mailpit instance by hand before writing any test assertions, per the plan's own exploratory-step instruction.
- This plan's dispatch was a retry: a prior attempt halted immediately at the worktree base-mismatch check (0 commits made, no work lost) because a concurrent session was committing unrelated Phase 14 planning docs to `main` at the same time. Those commits are unrelated to this plan's files and were left untouched.

## User Setup Required

None — no external service configuration required. Docker/Colima was already running on the dev machine; `scripts/mailpit-verify.sh` is self-contained (starts and tears down its own Mailpit sidecar).

## Next Phase Readiness

- Phase 13 (Real Broker-Mediated SMTP Adapter) is now fully verified end-to-end: SMTP-01/02 (Plan 03), SEND-01/02 (Plan 02), and SMTP-03/05 (this plan) all have passing Linux evidence.
- `scripts/mailpit-verify.sh` is ready for Phase 17's live acceptance run to reuse directly.
- No blockers.

---
*Phase: 13-real-broker-mediated-smtp-adapter*
*Completed: 2026-07-08*

## Self-Check: PASSED

- FOUND: scripts/mailpit-verify.sh
- FOUND: crates/brokerd/tests/email_smtp_acceptance.rs
- FOUND: commit 5bb2301
- FOUND: commit 435fd06
- FOUND: commit d341ead
- FOUND: commit dac907b
- FOUND: commit 0c9bb7d
- FOUND: commit 4ff05af
