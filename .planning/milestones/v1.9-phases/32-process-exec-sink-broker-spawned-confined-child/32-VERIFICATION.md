---
phase: 32-process-exec-sink-broker-spawned-confined-child
verified: 2026-07-17T23:45:00Z
status: passed
score: 4/4 must-haves verified (all 4 ROADMAP success criteria + all 4 EXEC-01..04 requirements)
behavior_unverified: 0
overrides_applied: 0
re_verification: null
---

# Phase 32: `process.exec` Sink — Broker-Spawned Confined Child Verification Report

**Phase Goal:** caprun can run a command as a broker-spawned confined child whose captured stdout/stderr are genuinely taint-minted and deterministically I2-enforced.
**Verified:** 2026-07-17T23:45:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Methodology note (important)

This verification did NOT rely on SUMMARY.md's claims of a passing Linux run.
The verifier independently re-ran the mandatory Linux verification from
scratch, in its own process, via Colima+Docker (`rust:1`, `seccomp=unconfined`),
scoped exactly to the phase's new test targets:

```
MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p sandbox --test exec_child_confinement && cargo test -p brokerd process_exec && cargo test -p caprun --test s9_process_exec_block' bash scripts/mailpit-verify.sh
```

Real exit code captured before any pipe: `REAL_EXIT_CODE:0`. Named-test counts
observed directly in this verifier's own log (not copied from SUMMARY.md):
- `exec_child_confinement`: 4/4 pass (3 real Linux confinement assertions + 1 cross-platform guard)
- `process_exec_spawn`: 3/3 pass
- `s9_process_exec_block`: 4/4 pass (3 real Linux acceptance assertions + 1 cross-platform guard)

`bash scripts/check-invariants.sh` was independently re-run on this machine:
exit 0, all 4 gates PASS, including the Gate-3 `mint_from_exec(` line. A full
`cargo test --workspace --no-fail-fast` was independently re-run on macOS:
0 failed across every suite (Linux-gated suites correctly report 0/0 —
expected per CLAUDE.md's cfg-linux-test-blindness note, not a gap).

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria, all 4)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A `process.exec` plan-node sink runs a command as a broker-spawned confined child process, never via the worker's own `execve` | ✓ VERIFIED | `crates/brokerd/src/sinks/process_exec.rs::invoke_process_exec` spawns `caprun-exec-launcher` (`resolve_launcher_path`) via `tokio::process::Command`, never the worker; the launcher (`cli/caprun-exec-launcher/src/main.rs`) self-confines (`apply_rlimits` → `exec_child_ruleset` → `exec_child_filter`, each `.expect()`-aborting) THEN `execve`s the target (`CommandExt::exec()`). Independently re-run on real Linux: `exec_child_confinement.rs` 3/3 real assertions pass — fs-escape outside WorkspaceRoot denied, outbound `socket(AF_INET)` denied (EPERM), benign in-workspace write + the legitimate target `execve` both succeed. |
| 2 | The child's stdout/stderr are captured and taint-minted as untrusted, with provenance genuinely rooted at the `exec` Event (sole mint site, no stapling) | ✓ VERIFIED | `invoke_process_exec` appends a `process_exited` Event FIRST (`crates/brokerd/src/sinks/process_exec.rs:159-175`), returns its id; `server.rs`'s Allowed/`process.exec` arm (line ~1047-1074) calls `quarantine::mint_from_exec(value_store, session_id, combined_output, sink_event_id)` rooted on that SAME id — `mint_from_exec` (quarantine.rs:838-853) does NOT append its own event. Unit test `mint_from_exec_anchor_identity` (quarantine.rs:1897) asserts `provenance_chain == [spawn_event_id]`. Independently re-run Linux e2e test `s9_process_exec_genuine_taint_block` re-queries the DB for the `process_exited` row (not just the in-memory return) and asserts `minted.provenance_chain == [exec_event_id]` — genuine, non-stapled. |
| 3 | A tainted exec-output value routed to a sensitive sink arg is deterministically Blocked, verifiable as an unbroken audit-DAG edge with `verify_chain` true | ✓ VERIFIED | `process.exec`'s own `command`/`args` are BOTH routing- and content-sensitive (`sink_sensitivity.rs:95,102,114,131`), so routing the minted exec-output handle into a second `process.exec`'s `command` arg hits the unmodified collect-then-Block loop. Independently re-run Linux test `s9_process_exec_genuine_taint_block` asserts `ExecutorDecision::BlockedPendingConfirmation`, `anchor.provenance_chain[0] == exec_event_id`, `anchor.read_event_id == exec_event_id`, a durable `sink_blocked` event, no `sink_executed` event, and `verify_chain(...) == true` over the whole session (`session_created → process_exited → sink_blocked`). |
| 4 | The exec child is kernel-confined (Landlock+seccomp+default-deny net+resource/time limits), the sink is fail-closed on arg-schema, and a durable audit Event records spawn+exit | ✓ VERIFIED | `exec_child_ruleset` (narrow-allow Landlock, distinct from `deny_all_filesystem()`) + `exec_child_filter` (reused net-deny, no execve-deny) + reused `apply_rlimits` + a NEW `EXEC_WALL_CLOCK_TIMEOUT` (30s `tokio::time::timeout`) + a 10 MiB combined byte cap (`MAX_COMBINED_OUTPUT_BYTES`, fail-closed via a shared `AtomicUsize`). Schema: `KNOWN_SINKS` `process.exec` row requires `command`, Denies unknown/duplicate/missing args (`sink_schema.rs` unit tests). Two-phase durable audit: `process_exited`/`process_spawn_failed`, chained onto `parent_id`/`parent_hash`. Independently re-run: `process_exec_spawn.rs` 3/3 (spawn+capture, timeout-kill, byte-cap-fail-closed, all asserting `verify_chain`); `exec_child_confinement.rs` 3/3 real Linux Landlock/seccomp assertions against the REAL launcher + an unconfined `/usr/bin/python3` target. |

**Score:** 4/4 truths verified (0 present-but-behavior-unverified)

### Requirements Coverage (EXEC-01..04)

| Requirement | Source Plan(s) | Status | Evidence |
|---|---|---|---|
| EXEC-01 (broker-spawned confined child, never worker execve) | 32-01, 32-03, 32-04, 32-06 | ✓ SATISFIED | `invoke_process_exec` spawns `caprun-exec-launcher`; launcher self-confines then execve's; confirmed on real Linux (see above) |
| EXEC-02 (genuine, non-stapled taint mint) | 32-01, 32-05, 32-06 | ✓ SATISFIED | `mint_from_exec` rooted on `invoke_process_exec`'s `process_exited` event id; anchor-identity unit test + Linux e2e DB re-query both confirm |
| EXEC-03 (deterministic I2 Block, unbroken audit-DAG edge, `verify_chain` true) | 32-01, 32-04, 32-05, 32-06 | ✓ SATISFIED | `s9_process_exec_genuine_taint_block` (independently re-run, PASS) — `BlockedPendingConfirmation`, non-stapled anchor, durable `sink_blocked`, `verify_chain` true |
| EXEC-04 (kernel confinement, fail-closed schema, durable spawn/exit audit) | 32-01, 32-02, 32-03, 32-04, 32-06 | ✓ SATISFIED | `exec_child_ruleset`/`exec_child_filter`, wall-clock timeout, byte cap, `KNOWN_SINKS` schema Deny tests, two-phase audit — all independently re-run green on real Linux |

No orphaned requirements: `.planning/REQUIREMENTS.md` traceability table maps EXEC-01..04 to Phase 32 only, all four marked `Complete`, consistent with this verifier's independent findings.

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/runtime-core/src/plan_node.rs` | `TaintLabel::ExecRaw`, untrusted, no wildcard arm | ✓ VERIFIED | Variant present line 28; `is_untrusted()` arm line 53; `cargo build --workspace` compiles with zero wildcard arms |
| `crates/executor/src/sink_schema.rs` | `process.exec` `KNOWN_SINKS` row | ✓ VERIFIED | Row at line 64; 5 inline schema tests (command-only OK, full-args OK, missing-command Denied, unknown-arg Denied, duplicate-arg Denied) |
| `crates/executor/src/sink_sensitivity.rs` | routing/content-sensitivity + expected_role tables | ✓ VERIFIED | `PROCESS_EXEC_ROUTING_SENSITIVE`/`PROCESS_EXEC_CONTENT_SENSITIVE` consts + arms in all 4 classifier functions; inline tests pass |
| `crates/sandbox/src/landlock.rs` | `exec_child_ruleset` narrow-allow, distinct from `deny_all_filesystem` | ✓ VERIFIED | Distinct function; grants `ReadFile+ReadDir+Execute` on enumerated system paths, `ReadFile+WriteFile+MakeReg` on WorkspaceRoot only; `deny_all_filesystem` byte-for-byte unchanged |
| `crates/sandbox/src/seccomp.rs` | `exec_child_filter`, net-deny reused, no execve-deny | ✓ VERIFIED | Socket deny block identical to `apply_worker_filter`; execve/execveat deny entries absent; `apply_worker_filter` unchanged |
| `cli/caprun-exec-launcher/src/main.rs` | self-confining launcher, no `sh -c`, no shell join | ✓ VERIFIED | rlimits→landlock→seccomp order, `.expect()`-abort-before-execve, `CommandExt::exec()`, no `sh -c`/pre_exec/arg-joining |
| `crates/brokerd/src/sinks/process_exec.rs` | `invoke_process_exec`: spawn/capture/timeout/cap/two-phase audit | ✓ VERIFIED | Full trace above; never mints (`.mint(`/`mint_from_exec` absent — Gate 3 clean) |
| `crates/brokerd/src/quarantine.rs` | `mint_from_exec`, sole mint site | ✓ VERIFIED | Mints only, no fresh event; anchor-identity test |
| `scripts/check-invariants.sh` | Gate 3 4th `mint_from_exec(` token | ✓ VERIFIED | Line 136, restricted to `quarantine.rs`+`server.rs`; independently re-run, exit 0 |
| `crates/brokerd/src/proto.rs` / `server.rs` | `output_value_id` wire field, Allowed dispatch arm | ✓ VERIFIED | Required field (no `#[serde(default)]`); server.rs arm spawns+mints+returns handle; planner path discards it (T-04-02) |
| `cli/caprun/tests/s9_process_exec_block.rs` | EXEC-02/03 acceptance (block, clean-allow, tainted-command negative) | ✓ VERIFIED | 4 tests, independently re-run on Linux, 4/4 pass |
| `crates/sandbox/tests/exec_child_confinement.rs` | confinement negative-assertion against the real launcher | ✓ VERIFIED | 4 tests (3 real + 1 guard), independently re-run on Linux, 4/4 pass |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `invoke_process_exec` (process_exec.rs) | `mint_from_exec` (quarantine.rs, called from server.rs) | shared `process_exited` event id, never a fresh root | ✓ WIRED | server.rs line ~1071 passes `sink_event_id` (== the id `invoke_process_exec` just returned) directly into `mint_from_exec`'s `spawn_event_id` param |
| `server.rs` Allowed/process.exec arm | worker (`cli/caprun/src/worker.rs`) | `BrokerResponse::PlanNodeDecision.output_value_id` | ✓ WIRED | required field, compiler-forced at every construction/destructure site; worker destructures and holds the opaque handle, never raw bytes (I1 preserved) |
| exec-output ValueId | executor's `submit_plan_node` collect-then-Block loop | routed into a 2nd `process.exec`'s `command` arg | ✓ WIRED | Confirmed by the independently-re-run `s9_process_exec_genuine_taint_block` — real `BlockedPendingConfirmation` with a non-stapled anchor |
| `check-invariants.sh` Gate 3 | `mint_from_exec(` call sites | grep restriction to `quarantine.rs`+`server.rs` | ✓ WIRED | Confirmed present + passing on independent re-run |

### Behavioral Spot-Checks / Probe Execution

This phase's "probes" are the Linux-gated integration/e2e test files themselves
(no separate `scripts/*/tests/probe-*.sh`). Per the mandate not to trust
SUMMARY.md pass claims, the verifier ran them independently, from scratch, in
its own container process (not the SUMMARY-reported run):

| Test target | Command | Result | Status |
|---|---|---|---|
| `check-invariants.sh` (Mac) | `bash scripts/check-invariants.sh` | 4/4 gates PASS, Gate-3 `mint_from_exec(` line present | ✓ PASS |
| `cargo test --workspace --no-fail-fast` (Mac) | full workspace | 0 failed across every suite (Linux-gated suites 0/0 as expected) | ✓ PASS |
| `exec_child_confinement` (Linux, Colima) | `cargo test -p sandbox --test exec_child_confinement` | 4/4 (fs-escape denied, net-deny persists across execve, benign write+legit execve succeed, guard) | ✓ PASS |
| `process_exec` (Linux, Colima) | `cargo test -p brokerd process_exec` | 3/3 (spawn+capture, wall-clock timeout kill, byte-cap fail-closed) | ✓ PASS |
| `s9_process_exec_block` (Linux, Colima) | `cargo test -p caprun --test s9_process_exec_block` | 4/4 (genuine-taint Block, clean-allow control, tainted-command negative, guard) | ✓ PASS |

Real exit code captured before any pipe: `REAL_EXIT_CODE:0` (written directly
to a log file by this verifier's own shell, never laundered through `| tail`).

### Anti-Patterns Found

None. Grepped all Phase-32 TCB files (`process_exec.rs`, `quarantine.rs`,
`landlock.rs`, `seccomp.rs`, launcher `main.rs`, `sink_schema.rs`,
`sink_sensitivity.rs`, `server.rs`) for `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/
`PLACEHOLDER`/"not yet implemented" — zero hits. No stub returns, no
hardcoded-empty stand-ins on any live path.

### Requirements Coverage Table Cross-Check (REQUIREMENTS.md)

`.planning/REQUIREMENTS.md` marks EXEC-01, EXEC-02, EXEC-03, EXEC-04 all
`Complete`, mapped to Phase 32 only — matches this verifier's independent
findings above. No orphaned requirements for this phase.

### Human Verification Required

None. All four success criteria are independently, mechanically re-provable
on real Linux by this verifier (not merely re-stated from SUMMARY.md), and
none of them are behavior-dependent in a way presence/wiring checks alone
could miss — the state-transition and non-stapling invariants (EXEC-02/03)
were exercised by a fresh test run this session, not just read as code.

### Gaps Summary

None. All 4 ROADMAP success criteria and all 4 EXEC-01..04 requirement IDs are
genuinely delivered — verified via direct source-code trace AND an
independent, from-scratch re-run of the mandatory Linux (Colima+Docker,
`rust:1`, `seccomp=unconfined`) verification, plus a fresh Mac
`check-invariants.sh` and full-workspace `cargo test` pass. The phase's own
SUMMARY.md claims were treated as unverified narrative until independently
reproduced; they were reproduced. Full composed live acceptance (LIVE-01, all
criteria in one run) remains correctly deferred to Phase 34 per
`.planning/REQUIREMENTS.md` — not a Phase 32 gap.

---

_Verified: 2026-07-17T23:45:00Z_
_Verifier: Claude (gsd-verifier)_
