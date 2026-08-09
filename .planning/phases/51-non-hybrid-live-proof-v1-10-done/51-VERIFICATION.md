---
phase: 51-non-hybrid-live-proof-v1-10-done
verified: 2026-08-09T04:05:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 1/4
  gaps_closed:
    - "The retained real-Linux scoped compose run proves the CLI-driven LIVE-07 success path in one Session with a passing audit chain."
    - "The retained real-Linux scoped compose run proves the exact LIVE-08 github.pr/body I2 attribution, no PR effect, and a passing audit chain."
    - "The retained real-Linux full-workspace compose run is green; the OPENAI_API_KEY-dependent early-return coverage is explicitly disclosed and is not part of LIVE-07/LIVE-08."
  gaps_remaining: []
  regressions: []
warnings:
  - id: CR-01
    severity: high
    statement: "record_github_grant persists session_grants before separately appending github_grant_authorized; append failure followed by retry can leave an active grant with no authorization event."
    disposition: "Actual security/audit-integrity defect requiring follow-up, but not a blocker to Phase 51's existential LIVE-07/LIVE-08 proof: the one-shot grant sidecar and scoped test would fail on the append-error path, and the retained run passed."
next_action: "Proceed to Phase 52, while scheduling CR-01 as a high-priority transactional grant/audit fix with a fault-and-retry regression."
next_command: "$gsd-plan-phase 52"
---

# Phase 51: Non-hybrid LIVE Proof Verification Report

**Phase Goal:** On real Linux, the multi-step coding path is proven end-to-end as a CLI-driven one-Session run (success + mid-loop I2 Block), closing the v1.9 hybrid honesty gap.
**Verified:** 2026-08-09T04:05:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure and evidence remediation

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | On real Linux, the CLI-driven edit → test → commit → confirmed push → PR path completes in one Session and passes real audit-chain verification. | ✓ VERIFIED | The hashed scoped compose log at source `cb34b9124916164397697a948c0c4804db221c82` names `linux::live_07_cli_multi_node_one_session_verify_chain ... ok` and reports 4/4 green. The executed test requires driver exit 0, exactly one surfaced Session, `caprun audit` exit 0 with `Chain verification: PASSED`, and exactly one `git_push_succeeded` plus one `github_pr_succeeded`. |
| 2 | The SUCCESS proof is CLI-driven, not a hybrid in-crate composition, and the framing is machine-pinned. | ✓ VERIFIED | `live_acceptance_v1_10_cli.rs:1-6,21-24,358-429` invokes the real `caprun run ... safe-coding-workflow` binary and explicitly excludes `evaluate_plan_node_and_record_for_test` from the DONE claim. The retained scoped Linux execution ran this test. |
| 3 | A sibling real-CLI run proves a genuine, policy-permitted mid-loop I2 Block attributable to process output, with no PR effect and a valid chain. | ✓ VERIFIED | The scoped log names `linux::live_08_cli_mid_loop_i2_block_genuine_taint ... ok`. At the executed revision, its hard assertions require a permitting policy, non-policy-deny blocked exit, one Session, `github_pr_succeeded == 0`, one durable `github.pr/body` anchor with untrusted taint, `anchor.read_event_id` selecting the unique `process.exec` exit, provenance-root equality, and passing `caprun audit`. The order-independent attribution regression also ran green. |
| 4 | The authoritative full-workspace compose regression and invariant gates are green with no Phase 51 regression. | ✓ VERIFIED | The retained full log hashes correctly and ends `Composed Linux verification suite PASSED (Mailpit + mock GitHub).` It includes the v1.9 composed success test. An independent current `./scripts/check-invariants.sh` rerun passed Gates 1–6. The two absent-`OPENAI_API_KEY` early returns are disclosed and are unrelated to LIVE-07/LIVE-08. |

**Score:** 4/4 truths verified (0 present-but-behavior-unverified)

## Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `cli/caprun/tests/live_acceptance_v1_10_cli.rs` | Real-binary LIVE-07/LIVE-08 harness and exact attribution oracle | ✓ VERIFIED | Substantive, compiled, executed in the retained scoped run, and still byte-unchanged from proven source for this scope. |
| `cli/caprun/src/planner.rs` | Default-off I2 proof planner, separate from the deterministic success planner | ✓ VERIFIED | Proof planner routes the genuine `out_1` handle only on the fixture path; ordinary success-path anti-laundering remains separate. |
| `cli/caprun/src/worker.rs` / `cli/caprun/src/main.rs` | Fixture-contained selector wiring | ✓ VERIFIED | Selector is forwarded/selected only under `live-proof-fixtures`; subprocess fixtures clear ambient environment. |
| `crates/brokerd/src/audit.rs`, `server.rs`, `confirmation.rs` | Serialized append-at-durable-head repair | ✓ VERIFIED | Immediate append transaction and explicit timeout are present; committed fork/contention regressions pass; independent trace found zero blockers. |
| `51-LIVE-SCOPED.log` | Complete scoped real-Linux compose output | ✓ VERIFIED | SHA-256 `dc57e49b2d75ec0d040da69a780f13485507148b5efe19d0457640d41916b98d`; four tests passed, including both Linux LIVE bodies. |
| `51-LIVE-FULL.log` | Complete full-workspace compose output | ✓ VERIFIED | SHA-256 `4bcb275b98dde637d7ac644a60227d33cc5ec47acf65e8898e2f1a4d4b34ee3e`; composed suite passed. |
| `51-LIVE-EVIDENCE.md` | Accurate command, environment, hashes, results, and caveats | ✓ VERIFIED | Remediation commit `3b4ef4f` distinguishes raw-log observations from assertion-backed consequences and discloses absent-key skips. |
| `51-VALIDATION.md` | Executed evidence map | ✓ VERIFIED | `status: complete`, `nyquist_compliant: true`, scoped and full hashes mapped to LIVE-07/LIVE-08. |

## Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| Real `caprun` binary | One multi-node coding Session | `run --policy ... safe-coding-workflow` | ✓ WIRED | Test launches `CARGO_BIN_EXE_caprun`; exactly one `session_id=` is required. |
| Blocked push | External confirmation | `CAPRUN_CONFIRM=external` sidecar | ✓ WIRED | Sidecar confirms the pending push while the same driver remains connected. |
| Session surface | GitHub grant | External `caprun grant` sidecar | ✓ WIRED | Grant is not auto-minted in the worker; a sidecar failure fails the test. |
| Session audit DB | Chain proof | Real `caprun audit` subprocess | ✓ WIRED | Helper requires audit exit 0 and `Chain verification: PASSED`. |
| `process.exec` output | `github.pr/body` blocked anchor | `mint_from_exec` → bag `out_1` → durable `read_event_id` | ✓ WIRED | Anchor-first oracle selects by durable identity, then checks actor, cardinalities, and provenance root. |
| Scoped/full commands | Evidence/status ledgers | Raw logs + SHA-256 + post-green reconciliation | ✓ WIRED | Commits `43eb822`, `43fdb67`, and `3b4ef4f` preserve correct ordering and evidence provenance. |

## Data-Flow Trace (Level 4)

| Artifact | Data | Source | Produces Real Data | Status |
|---|---|---|---|---|
| LIVE-07 acceptance body | Session id and durable success events | Real CLI stdout plus SQLite audit DB | Yes | ✓ FLOWING |
| LIVE-08 acceptance body | Block anchor, exit events, provenance ids | Real CLI execution plus SQLite audit DB | Yes | ✓ FLOWING |
| Evidence record | Execution result and source identity | Immutable retained logs, git revision, recomputed hashes | Yes | ✓ FLOWING |

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Order-independent LIVE-08 attribution | `cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca live_08_attribution_is_independent_of_exit_event_order -- --exact --test-threads=1` | 1 passed, 0 failed | ✓ PASS |
| Security invariant gates | `./scripts/check-invariants.sh` | Gates 1–6 passed | ✓ PASS |
| Scoped retained proof | Recomputed hash + inspected named-test roster | Hash match; LIVE-07, LIVE-08, oracle, and guard all `ok` | ✓ PASS |
| Full retained regression | Recomputed hash + inspected terminal marker/test roster | Hash match; composed suite passed | ✓ PASS |

## Probe Execution

No separate `probe-*.sh` is declared by Phase 51. The documented authoritative runnable checks are the scoped and full `scripts/compose-verify.sh` commands, whose immutable outputs were independently inspected above.

## Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|---|---|---|---|---|
| LIVE-07 | 51-01, 51-03, 51-04 | Real-Linux CLI one-Session edit-to-PR success with valid audit chain and green regression | ✓ SATISFIED | Scoped 4/4 compose proof, exact assertions, full green compose log, corrected evidence record. |
| LIVE-08 | 51-02, 51-03, 51-04, 51-09 | Genuine process-output I2 block under permitted PR sink, no effect, valid chain, honest framing | ✓ SATISFIED | Scoped named test green; durable anchor/read-event/provenance oracle and reverse-order regression green. |

No Phase 51 requirement is orphaned.

## Prohibition Checks

| Prohibition family | Status | Evidence |
|---|---|---|
| No hybrid or dual-Session DONE claim | ✓ VERIFIED | Real CLI launch, one-session assertion, framing constants/docs, retained execution. |
| No auto-confirm/auto-grant/session waiver | ✓ VERIFIED | External sidecar drives explicit CLI actions; worker does not grant automatically. |
| No success-path `out_*` laundering | ✓ VERIFIED | Fixture proof planner is feature/default contained; deterministic planner remains distinct. |
| No policy-deny or stapled-taint substitute for LIVE-08 | ✓ VERIFIED | Permitting policy, durable `sink_blocked` anchor, untrusted taint, and real process event attribution are asserted. |
| No new crates, raw effect path, mint sites, or packaging scope | ✓ VERIFIED | Invariant gates pass; source/test diff from proven revision is empty; Phase 52 owns packaging. |

## Anti-Patterns and Security Review

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/brokerd/src/audit.rs` | 542-564 | `session_grants` insert commits before separately transactional audit append | ⚠️ High follow-up (CR-01) | A later append failure plus retry can leave an active unaudited grant. This is a real security/audit-integrity defect, but it did not occur in or invalidate the retained one-shot LIVE proof. |
| `crates/brokerd/src/server.rs` | 1044-1049 | Comment overstates multi-write DB atomicity | ℹ️ Info | Pre-existing, fail-closed limitation already recorded by the independent trace. |
| `crates/brokerd/src/audit.rs` | 1033-1037 | Stale 19-site comment vs verified 45-site inventory | ℹ️ Info | Documentation drift only. |

### CR-01 disposition

CR-01 is observable and should be fixed: `record_github_grant` performs the capability insert in autocommit, `append_event` then opens a distinct transaction, and `has_github_grant` trusts only the row. An append failure therefore leaves the grant active, while a retry's `INSERT OR IGNORE` suppresses the missing event.

It is **not a blocker to LIVE-07, LIVE-08, or the Phase 51 goal**. Those contracts require proof of an actual CLI-driven real-Linux run, not universal atomicity of every grant failure/retry. In the scoped proof the external sidecar invokes the grant once and propagates a non-zero/error result; if the event append had failed, the sidecar and LIVE test would have failed. The retained test passed, and its subsequent chain and sink assertions passed. CR-01 therefore cannot retroactively falsify that execution. It remains high priority because it violates the broader intended audit completeness of the grant capability path and has no fault-injection regression.

## Disconfirmation Pass

- **Partial broader invariant:** grant authorization and its audit event are not transactionally atomic (CR-01), although the Phase 51 executed path is proven.
- **Potentially misleading green tests:** the full log's `live_acceptance_v1_4_composed_three_legs` and `llm_planner_clean_allow_delivers` return early without `OPENAI_API_KEY`; the evidence record now discloses this and does not use them for LIVE-07/LIVE-08.
- **Uncovered error path:** no test forces `append_event` failure after the grant row insert and then retries. This is the regression required for CR-01.

## Human Verification Required

None. Plan 51-04's blocking evidence checkpoint received explicit `approved` after remediation commit `3b4ef4f` and a fresh independent re-review returned APPROVE, BLOCKER 0, unresolved MAJOR 0. No unresolved behavior-dependent truth remains.

## Gaps Summary

No Phase 51 goal gap remains. The previous three gaps were closed by the retained scoped and full real-Linux runs, the anchor-first LIVE-08 repair, evidence remediation, and explicit human approval. CR-01 is a real high-priority security follow-up, but it is outside and does not falsify the observable LIVE proof contract.

---

_Verified: 2026-08-09T04:05:00Z_
_Verifier: the agent (gsd-verifier)_
