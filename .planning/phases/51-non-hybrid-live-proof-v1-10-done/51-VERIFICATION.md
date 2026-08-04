---
phase: 51-non-hybrid-live-proof-v1-10-done
verified: 2026-08-04T00:00:00Z
status: gaps_found
score: 1/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps:
  - truth: "On real Linux, the CLI-driven edit-to-PR success path completes in one Session and passes real audit-chain verification."
    status: failed
    reason: "The authoritative scoped compose-verify proof was not run. Docker is absent, WINDOWS entry 1 remains open, and LIVE-07 remains Pending in REQUIREMENTS.md. Test implementation is not execution evidence."
    artifacts:
      - path: "cli/caprun/tests/live_acceptance_v1_10_cli.rs"
        issue: "Substantive and wired test harness exists, but its Linux + mock-egress-ca body has no recorded successful execution."
      - path: ".planning/phases/51-non-hybrid-live-proof-v1-10-done/51-VALIDATION.md"
        issue: "nyquist_compliant is false and LIVE-07 compose status is pending."
    missing:
      - "Run the documented scoped compose-verify command on a Docker-capable real-Linux environment and retain a green result."
      - "Demonstrate exit 0, exactly one session_id, git_push_succeeded, github_pr_succeeded, and caprun audit Chain verification: PASSED."
  - truth: "The real-CLI sibling run proves an independently attributable mid-loop I2 Block from genuine process output under a policy-permitted github.pr, with no PR effect and a valid audit chain."
    status: failed
    reason: "The authoritative LIVE-08 compose proof was not run (WINDOWS entry 2 is open), and the current test checks only event counts/order rather than the blocked github.pr argument's provenance back to process_exited, as noted by code review WR-02."
    artifacts:
      - path: "cli/caprun/tests/live_acceptance_v1_10_cli.rs"
        issue: "LIVE-08 asserts process_exited precedes a sink_blocked event, but does not identify the exact blocked github.pr decision or prove its body ValueId descends from that exec output."
      - path: ".planning/phases/51-non-hybrid-live-proof-v1-10-done/51-REVIEW.md"
        issue: "WR-02 records the missing provenance/attribution assertion."
    missing:
      - "Strengthen the durable proof to identify the denied github.pr node, its I2 reason, and the PR body provenance edge/root from the real process_exited output."
      - "Run the strengthened LIVE-08 scoped compose proof on real Linux and retain a green audit-chain result."
  - truth: "The full workspace regression is green on real Linux through compose-verify, with no v1.0-v1.9 regression."
    status: failed
    reason: "The full-workspace compose-verify gate was never run because Docker is unavailable. Only check-invariants.sh was independently rerun and passed."
    artifacts:
      - path: ".planning/phases/51-non-hybrid-live-proof-v1-10-done/51-VALIDATION.md"
        issue: "Full suite and scoped LIVE statuses remain pending; nyquist_compliant remains false."
      - path: ".planning/WINDOWS.md"
        issue: "Open unrun-verify ledger entries explicitly preserve the missing Docker-backed proof."
    missing:
      - "Run COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test --workspace --no-fail-fast --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh successfully on real Linux."
next_action: "Close the three verification gaps, including code-review attribution and hermeticity warnings, then re-run phase verification."
next_command: "$gsd-plan-phase --gaps"
---

# Phase 51: Non-hybrid LIVE Proof Verification Report

**Phase Goal:** On real Linux, the multi-step coding path is proven end-to-end as a CLI-driven one-Session run (success + mid-loop I2 Block), closing the v1.9 hybrid honesty gap.
**Verified:** 2026-08-04T00:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | Real-Linux CLI success completes edit → test → commit → confirmed push → PR in one Session with `verify_chain` true | ✗ FAILED | Harness exists, but the authoritative scoped compose run is unrun; Docker is absent and WINDOWS entry 1 is open. |
| 2 | SUCCESS is CLI-driven rather than hybrid, with machine-checked framing | ✓ VERIFIED | `live_acceptance_v1_10_cli.rs` spawns `CARGO_BIN_EXE_caprun` with `safe-coding-workflow`; `LIVE_07_DRIVER` and `LIVE_07_NOT` pin the claim and explicitly name the hybrid non-claim. |
| 3 | A real-CLI sibling run proves a genuine, policy-permitted mid-loop I2 Block with no effect and valid chain | ✗ FAILED | Compose proof is unrun; additionally, event ordering does not prove the blocked PR body descends from the exec output (51-REVIEW WR-02). |
| 4 | Full workspace regression is green on real Linux via compose-verify and invariants are green | ✗ FAILED | Independent `check-invariants.sh` rerun passed Gates 1–6; the full compose workspace run is unrun and WINDOWS entry 2 remains open. |

**Score:** 1/4 truths verified (0 present-but-behavior-unverified; the missing required executions are observable failures, not uncertainty)

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `cli/caprun/tests/live_acceptance_v1_10_cli.rs` | LIVE-07 and LIVE-08 real-binary harness | ⚠️ PRESENT, PROOF UNRUN | Exists and is substantive; Linux bodies are feature-gated and have no authoritative run result. |
| `cli/caprun/src/planner.rs` | Default-off `CodingI2ProofPlanner`; deterministic path remains clean | ✓ VERIFIED | Product planner routes `out_1` only in the proof planner; deterministic `plan_coding_next` remains separate. |
| `cli/caprun/src/worker.rs` | Proof selector wiring | ✓ VERIFIED | SafeCodingWorkflow selects `CodingI2ProofPlanner` only when the forwarded selector equals `1`. |
| `cli/caprun/src/main.rs` | Worker env forwarding | ⚠️ WIRED WITH REVIEW WARNING | Selector is forwarded through the worker allowlist, but review WR-01 flags ambient production activation. |
| `cli/caprun/tests/planner.rs` | Expressibility and anti-launder coverage | ✓ PRESENT | Test source exists; no passing host Cargo execution was available because OpenSSL discovery/pkg-config blocked linking. |
| `COVERAGE.md` | Honest proof scope and pending authority | ✓ VERIFIED | Explicitly says LIVE completion requires compose-verify and records current proof as pending. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| Real test process | CLI coding driver | `Command::new(CARGO_BIN_EXE_caprun)` + `safe-coding-workflow` | ✓ WIRED | Both LIVE tests spawn the real binary path. |
| Session/effect output | external grant/confirm | concurrent sidecar | ✓ WIRED | Sidecar grants the surfaced Session and confirms the surfaced git.push effect without opening a completion Session. |
| `CAPRUN_CODING_I2_PROOF=1` | `CodingI2ProofPlanner` | main allowlist → worker selection | ✓ WIRED | Static path is present, default-off, and selects the proof planner for SafeCodingWorkflow. |
| `process.exec` output | blocked `github.pr` body | bag `out_1` | ⚠️ PARTIAL | Planner wiring exists, but the acceptance assertion does not trace the denied body's durable provenance to that exec output. |
| Test assertions | real-Linux proof | compose-verify | ✗ NOT EXECUTED | Both scoped LIVE and full-workspace authority are open ledger items. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| LIVE-07 harness | `session_id`, audit events | real CLI stdout + audit SQLite DB | Intended, not executed authoritatively | ✗ UNPROVEN |
| LIVE-08 harness | `out_1`, blocked PR body | `process.exec` mint → plan bag → `github.pr` | Static route exists; durable test trace incomplete | ⚠️ PARTIAL |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Architectural invariants | `./scripts/check-invariants.sh` | Gates 1–6 passed during verification | ✓ PASS |
| Host Cargo tests | planned caprun test command | Not claimed: repository records pkg-config/OpenSSL discovery prevented linking | ? NOT RUN |
| LIVE-07 scoped proof | documented compose command | Docker unavailable; WINDOWS #1 open | ✗ FAIL (required proof absent) |
| LIVE-08/full regression | documented compose commands | Docker unavailable; WINDOWS #2 open | ✗ FAIL (required proof absent) |

### Probe Execution

No separate `probe-*.sh` is declared. The authoritative probe-equivalent is `scripts/compose-verify.sh`; it was not executable on this host because Docker is unavailable and therefore does not pass.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| LIVE-07 | 51-01, 51-02 | Real-Linux one-Session CLI success plus full regression | ✗ BLOCKED | REQUIREMENTS marks Pending; scoped and full compose runs are unrun. |
| LIVE-08 | 51-02 | Genuine attributable I2 Block under permitting policy, no effect, valid chain | ✗ BLOCKED | REQUIREMENTS marks Pending; LIVE run unexecuted and provenance assertion is incomplete. |

No Phase 51 requirement IDs are orphaned: both LIVE-07 and LIVE-08 appear in PLAN frontmatter and REQUIREMENTS.md.

### Anti-Patterns and Review Findings

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `cli/caprun/src/main.rs` | 557 | Ambient env enables proof planner in ordinary production CLI | ⚠️ Warning (WR-01) | A stale environment can alter a normal coding run after irreversible earlier steps. |
| `cli/caprun/tests/live_acceptance_v1_10_cli.rs` | 357 | Temporal event ordering used as provenance oracle | 🛑 Blocker for LIVE-08 attribution (WR-02) | Does not prove the blocked PR body derives from the real exec output. |
| `cli/caprun/tests/live_acceptance_v1_10_cli.rs` | 237 | Live subprocesses inherit arbitrary ambient environment | ⚠️ Warning (WR-03) | Authoritative results may depend on caller environment or exercise the wrong planner. |

No unreferenced `TBD`, `FIXME`, or `XXX` debt markers were found in the reviewed Phase 51 source set.

### Human Verification Required

None. The blockers are deterministic missing execution/attribution evidence. They require implementation/test correction and a real-Linux Docker-backed rerun, not subjective UAT.

### Gaps Summary

Phase 51 implemented a credible non-hybrid harness and the key static wiring, but the phase goal says **proven on real Linux**. The repository explicitly records both authoritative compose windows as open, keeps LIVE-07/LIVE-08 Pending, and leaves validation non-Nyquist. Moreover, the code review found that LIVE-08's current assertion cannot independently attribute the blocked PR argument to genuine exec provenance. Implementation-only completion therefore does not achieve the goal.

Phase 52 packaging does not defer or absorb these gaps; it depends on Phase 51 and contains no success criterion that supplies the missing LIVE proof.

---

_Verified: 2026-08-04T00:00:00Z_
_Verifier: the agent (gsd-verifier)_
