---
phase: 13-real-broker-mediated-smtp-adapter
plan: 03
subsystem: infra
tags: [seccomp, landlock, confinement, integration-test, grep-gate, smtp]

# Dependency graph
requires:
  - phase: 03-sandbox-confinement
    provides: apply_worker_filter seccomp socket(AF_INET/AF_INET6) EPERM deny, confine-probe binary, negative_net test pattern
provides:
  - "confine-probe smtp <host> <port> op — real connect() attempt under confinement (Pitfall 5 option b)"
  - "Linux-only negative_net_smtp_mailpit integration test proving kernel-enforced denial against a real host:port"
  - "scripts/check-smtp-secrets-absent.sh — structural grep gate proving CAPRUN_SMTP_ tokens absent from the caprun-worker spawn block"
affects: [13-01-real-broker-mediated-smtp-adapter, phase-14-content-sensitive-blocking]

# Tech tracking
tech-stack:
  added: []
  patterns: ["confine-probe op extension pattern (widen run_linux dispatch to take full argv for ops needing extra args)"]

key-files:
  created:
    - scripts/check-smtp-secrets-absent.sh
  modified:
    - crates/sandbox/src/bin/confine-probe.rs
    - crates/sandbox/tests/confinement_integration.rs

key-decisions:
  - "Pitfall 5 option (b) chosen: new confine-probe smtp <host> <port> op performs a real TCP connect() attempt, rather than reusing probe_net() verbatim — more faithful to 'attempts an actual connect() to the Mailpit host:port'"
  - "check-smtp-secrets-absent.sh scans only the Command::new(&worker_binary)...spawn() block in cli/caprun/src/main.rs, not the whole file — avoids false positives from the separate caprun confirm dispatch path (also in main.rs) which is not a worker-spawn path"

patterns-established:
  - "confine-probe ops needing extra args (beyond the op name) read them from the full argv passed into run_linux, keeping fs/net/exec unchanged"

requirements-completed: [SMTP-01, SMTP-02]

coverage:
  - id: D1
    description: "confine-probe smtp <host> <port> op attempts a real connect() under confinement"
    requirement: "SMTP-01"
    verification:
      - kind: unit
        ref: "cargo build -p sandbox --bin confine-probe (compiles on macOS + Linux)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Linux-only negative test proves confined connect() to Mailpit host:port is kernel-denied (EPERM at socket())"
    requirement: "SMTP-01"
    verification:
      - kind: integration
        ref: "crates/sandbox/tests/confinement_integration.rs#negative_net_smtp_mailpit — run via Colima+Docker rust:1 container, `cargo test -p sandbox`"
        status: pass
    human_judgment: false
  - id: D3
    description: "Grep gate fails if CAPRUN_SMTP_ token appears in the caprun-worker spawn block; passes on current tree"
    requirement: "SMTP-02"
    verification:
      - kind: other
        ref: "bash scripts/check-smtp-secrets-absent.sh (exit 0 on clean tree; exit 1 verified via temporary token insertion, then reverted)"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-07-07
status: complete
---

# Phase 13 Plan 03: Real Broker-Mediated SMTP Adapter — Sandbox/Grep Verification Summary

**New host:port-aware `confine-probe smtp` op + Linux-only negative-net test proving a confined connect() to Mailpit is kernel-denied by the existing seccomp filter, plus a grep gate proving `CAPRUN_SMTP_` tokens never reach the caprun-worker spawn path.**

## Performance

- **Duration:** 12 min (18:35:47 → 18:38:28 UTC-4, first to last task commit)
- **Started:** 2026-07-07T18:35:47-04:00
- **Completed:** 2026-07-07T18:38:28-04:00
- **Tasks:** 3
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments
- Added `confine-probe smtp <host> <port>` — a real `TcpStream::connect()` attempt under confinement, exercising the existing `apply_worker_filter` seccomp `socket(AF_INET/AF_INET6)` EPERM deny at the syscall boundary before any handshake (Pitfall 5 option b, explicit decision documented in the code and this summary)
- Added `negative_net_smtp_mailpit`, a `#[cfg(target_os = "linux")]` integration test spawning the new op against `CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` (defaulting to `127.0.0.1:1025`), asserting exit 0 (kernel-denied) — **verified passing on real Linux** via the project's Colima+Docker recipe (4/4 tests passed: fs, net, exec, smtp)
- Added `scripts/check-smtp-secrets-absent.sh`, a structural grep gate isolating the `caprun-worker` spawn block in `cli/caprun/src/main.rs` and failing if `CAPRUN_SMTP_` appears there — verified both the pass case (clean tree) and the fail case (temporary token insertion, confirmed exit 1, then reverted)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add host:port-aware confine-probe smtp op** - `b0279b2` (feat)
2. **Task 2: Linux-only negative-net test — confined connect() to Mailpit is kernel-denied** - `368d613` (test)
3. **Task 3: SMTP-02 grep gate — SMTP endpoint tokens absent from worker-spawn paths** - `bbf40d6` (feat)

_Note: Task 2 was declared `tdd="true"` but its implementation (`probe_smtp`) was already built in Task 1 per the plan's own task ordering — see "TDD Gate Compliance" below._

## Files Created/Modified
- `crates/sandbox/src/bin/confine-probe.rs` - Added `probe_smtp(host, port)` (Linux-only) and widened `run_linux`'s dispatch to accept the full argv so `smtp` can read `args[2]`/`args[3]`; `fs`/`net`/`exec` unchanged
- `crates/sandbox/tests/confinement_integration.rs` - Added `negative_net_smtp_mailpit`, mirroring `negative_net`'s spawn-and-assert-exit-0 pattern against the new `smtp` op
- `scripts/check-smtp-secrets-absent.sh` - New executable grep gate scanning the `caprun-worker` spawn block for `CAPRUN_SMTP_`

## Decisions Made
- **Pitfall 5 option (b):** built a new `smtp` op performing a real `connect()` attempt rather than reusing `probe_net()` verbatim. Both readings satisfy the same kernel-enforced claim (denial happens at `socket()`, before `connect()` can proceed) but option (b) is defense-in-depth: if the seccomp filter were ever loosened to allow `socket()` but still deny `connect()`, this probe would still correctly detect that at the connect boundary instead of silently reporting a false "blocked" from `socket()` alone.
- **Grep gate scope:** scanned only the `Command::new(&worker_binary)...spawn()` block in `main.rs`, not the whole file, because `main.rs` also contains the (unrelated) `caprun confirm`/`deny` dispatch path (`run_confirm_or_deny`, calling into `brokerd::confirmation::confirm`) — the SAME file, but not a worker-spawn path. Scanning the whole file would risk a false-positive gate failure once the sibling 13-01 plan's adapter work legitimately reads `CAPRUN_SMTP_*` env vars somewhere in the confirm path.
- Confirmed by repo-wide grep (`grep -rln "Command::new" crates/`, `grep -rn "caprun-worker"`) that `cli/caprun/src/main.rs` is the ONLY worker-spawn code path — no `crates/sandbox/` helper constructs a worker `Command`.

## Deviations from Plan

None - plan executed exactly as written. All three tasks match their `<action>`/`<acceptance_criteria>` blocks.

### TDD Gate Compliance

Task 2 declared `tdd="true"` but has no `<implementation>` block — only `<behavior>` and `<action>`, because the plan's own task ordering placed the implementation (`probe_smtp`) in Task 1, before the test in Task 2. A traditional RED-first cycle (write a failing test, then implement) was therefore not applicable within Task 2's scope: the code under test already existed. Additionally, this test is `#[cfg(target_os = "linux")]`-gated and shows "0 passed" on the macOS dev machine regardless of implementation state (per CLAUDE.md's documented convention), so a RED phase could not have been observed as a failing assertion on this machine even if attempted — only a Linux run distinguishes pass/fail. In lieu of the formal RED→GREEN split, the test's correctness was verified empirically: it was run on real Linux via the Colima+Docker recipe and confirmed passing (4/4 tests, including `negative_net_smtp_mailpit`). This is a plan-structure characteristic, not an execution deviation — no user permission or fix was needed.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required. (Mailpit sidecar setup for full acceptance-gate testing is the sibling 13-01 plan's / a later plan's concern; this plan's Linux test defaults to `127.0.0.1:1025` and passes without a live Mailpit listener, since the seccomp denial fires at `socket()` before any TCP handshake would occur.)

## Next Phase Readiness
- SMTP-01's kernel-enforced negative assertion and SMTP-02's structural secrets-absence gate are both independently proven, ahead of/parallel to the sibling 13-01 plan's adapter work (`crates/brokerd/src/sinks/email_smtp.rs`).
- No blockers. `scripts/check-smtp-secrets-absent.sh` should be re-run once 13-01's adapter and any worker-spawn changes land, to confirm the gate still passes against the merged tree.

---
*Phase: 13-real-broker-mediated-smtp-adapter*
*Completed: 2026-07-07*
