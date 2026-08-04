---
phase: 51-non-hybrid-live-proof-v1-10-done
reviewed: 2026-08-04T00:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - cli/caprun/Cargo.toml
  - cli/caprun/src/main.rs
  - cli/caprun/src/planner.rs
  - cli/caprun/src/worker.rs
  - cli/caprun/tests/planner.rs
  - cli/caprun/tests/live_acceptance_v1_10_cli.rs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 51: Code Review Report

**Reviewed:** 2026-08-04T00:00:00Z
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

The Phase 51 CLI proof path, worker planner selection, opaque-handle routing, and Linux acceptance harness were reviewed. The implementation has no demonstrated credential leak, but it ships a proof-only planner selector in the ordinary production CLI and the new acceptance proof has provenance and environment-isolation gaps that can make its result unreliable.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: An ambient environment variable can switch a production coding run onto the destructive proof planner

**File:** `/home/ben/Workspace/caprun/cli/caprun/src/main.rs:557`

**Issue:** Every ordinary `caprun run safe-coding-workflow` trusts the ambient `CAPRUN_CODING_I2_PROOF=1` value and forwards it through the otherwise restricted worker environment. The worker then selects `CodingI2ProofPlanner` at `/home/ben/Workspace/caprun/cli/caprun/src/worker.rs:382-398`, deliberately replacing the operator-provided PR body with `out_1`. This is test instrumentation embedded in the production behavior boundary: a stale shell/service environment, wrapper, or caller can silently change the plan and force a late I2 denial after file write, process execution, commit, and confirmed push have already occurred. "Default off" does not make an ambient process variable a trustworthy test-only authorization boundary.

**Fix:** Compile the proof planner and selector only for an explicit non-default test feature, and reject the selector in normal builds. Prefer driving the negative proof through a dedicated test binary or test-only CLI flag that is unavailable in release builds, for example:

```rust
#[cfg(feature = "live-proof-fixtures")]
if std::env::var("CAPRUN_CODING_I2_PROOF").as_deref() == Ok("1") {
    worker_planner_env.push(("CAPRUN_CODING_I2_PROOF", "1".to_owned()));
}
```

Gate `CodingI2ProofPlanner` and the worker selection arm with the same feature, and have non-feature builds fail explicitly if that variable is present rather than silently honoring it.

### WR-02: LIVE-08 does not prove that the blocked sink descends from the process output it claims to test

**File:** `/home/ben/Workspace/caprun/cli/caprun/tests/live_acceptance_v1_10_cli.rs:357-364`

**Issue:** The test establishes only temporal ordering: one `process_exited` row exists and its rowid precedes the first `sink_blocked` row. It never checks the blocked event's sink/reason, the submitted PR body's `ValueId`, or a provenance edge from that value back to the `process_exited` event. Any unrelated `sink_blocked` event emitted later in the same session would satisfy these assertions, so a regression that routes `pr_body` instead of `out_1`, or attaches unrelated/stapled taint, could still be credited as the required genuine non-stapled taint proof.

**Fix:** Query the exact denied `github.pr` decision/event and assert its reason is the I2 taint reason (not merely "not policy_deny"). Then inspect the durable argument/provenance records to assert that the `body` handle equals the output minted by the session's `process.exec` event and that its provenance parent/root is that event. At minimum, also assert the machine-readable terminal is `DENIED` for `sink=github.pr` with the expected I2 code; rowid ordering alone must not be the provenance oracle.

### WR-03: The live CLI subprocesses inherit arbitrary host environment and are not hermetic

**File:** `/home/ben/Workspace/caprun/cli/caprun/tests/live_acceptance_v1_10_cli.rs:237-253`

**Issue:** Both live `caprun run` commands and the `caprun()` sidecar helper inherit the test runner's complete environment. Phase-sensitive controls such as `CAPRUN_PLANNER`, `CAPRUN_CODING_I2_PROOF`, `CAPRUN_POLICY`, proxy variables, and other `CAPRUN_*` settings can therefore alter planner selection, routing, confirmation, or network behavior. In particular, ambient `CAPRUN_CODING_I2_PROOF=1` changes the LIVE-07 success case into the LIVE-08 planner, while ambient `CAPRUN_PLANNER=llm` makes coding fail before the intended proof. This makes the authoritative acceptance result dependent on the caller's shell/CI environment and risks both false failures and testing a different configuration than the test describes.

**Fix:** Build subprocess commands from an empty environment and add back only the exact fixture allowlist required by compose networking and binary execution. Explicitly remove mutually exclusive control variables for each case. Apply the same helper to `caprun run`, `grant`, `confirm`, and `audit`, e.g. start with `Command::new(...).env_clear()` and then set a known `PATH`, required mock endpoint/proxy variables, confirmation mode, proof selector only for LIVE-08, and the two opaque fixture tokens.

---

_Reviewed: 2026-08-04T00:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
