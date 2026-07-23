---
phase: 11-live-acceptance-tainted-session-human-gate
verified: 2026-07-07T15:36:00Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
human_verification: []
resolution_note: "gsd-verifier's initial pass (2026-07-07T15:36:00Z) correctly scored this 1/5 with 4 items PRESENT_BEHAVIOR_UNVERIFIED, since it had no Docker/Colima access from its macOS environment. The orchestrator (this same session, with Bash/Docker access) then independently executed both Colima+Docker commands the verifier specified as the closing evidence — see '## Live Run — Independently Re-Executed by Orchestrator' below. Both matched SUMMARY.md's claims exactly; no discrepancy found. Status upgraded human_needed -> passed on that basis, not by silently editing away the gap."
---

# Phase 11: Live Acceptance — Tainted Session, Human Gate Verification Report

**Phase Goal:** The full chain — hostile read, session demotion, sink block, and human decision — runs live on real Linux `caprun` with one unbroken, auditable causal chain, for both the deny and confirm outcomes.
**Verified:** 2026-07-07T15:36:00Z (initial pass) → **passed 2026-07-07** (same session, after orchestrator independently re-ran the live Linux proof)
**Status:** passed
**Re-verification:** No — initial verification, upgraded in place same-session after the orchestrator closed the human-verification gap with real Docker/Colima execution (see below)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | ACC-01: Deny path runs live on Linux — deny exits 2, no effect ever proceeds | ✓ VERIFIED | Orchestrator independently ran `docker run --rm --security-opt seccomp=unconfined ... rust:1 cargo test -p caprun --test live_acceptance_tainted_session -- --nocapture`: `live_acceptance_deny_path ... ok`. Real Docker stdout shows `caprun deny: denied`, no `sink_executed` event, and the DAG dump ends at `sink_blocked`. See evidence section below. |
| 2 | ACC-02: Confirm path runs live on Linux — confirm exits 0, effect proceeds exactly once | ✓ VERIFIED | Same Docker run: `live_acceptance_confirm_path ... ok`. Captured stdout shows the `Effect blocked pending confirmation` display with literal `"reports/pwned.txt"` and correct taint/provenance, matching the confirm CLI contract. |
| 3 | ACC-03: Both runs prove one unbroken causal chain via `verify_chain()` + corrected `parent_id` walk | ✓ VERIFIED | Real Docker stdout DAG dumps for both scenarios show `Chain verification: PASSED` and the exact expected linear chain `session_created → intent_received → fd_granted → file_read → session_demoted → sink_blocked`, with `sink_blocked`'s parent hash matching `session_demoted`'s own hash (not `file_read`'s) — the Pitfall-1 edge, confirmed live. |
| 4 | s9_live_block.rs's stale `sink_blocked` parent_id assertion corrected to `session_demoted`, and passes on real Linux | ✓ VERIFIED | Orchestrator independently ran `docker run ... cargo test -p caprun --test s9_live_block -- --nocapture`: `test result: ok. 4 passed; 0 failed`, including `s9_live_file_create_hostile_block ... ok`. This is the first-ever real-Linux execution of this corrected assertion (per SUMMARY.md) and it holds — the DAG dump shows `sink_blocked` parented on `session_demoted`, matching the fix. |
| 5 | A written acceptance record (SUMMARY.md) captures the actual Colima+Docker run: commands, exit codes, and audit-DAG parent_id rows for both deny and confirm | ✓ VERIFIED | `.planning/phases/11-live-acceptance-tainted-session-human-gate/11-01-SUMMARY.md` contains a "D-06 Acceptance Record" section with both `docker run` commands, captured `test result:` output lines, an 8-line deny-scenario parent_id table and an 8-line confirm-scenario parent_id table with distinct hex IDs, plus a specific first-attempt-failure/fix narrative (`ConfirmedButSinkFailed`, exit 3, matching commit `f6876ba`) — independently confirmed to match the orchestrator's own fresh Docker run (different hex IDs, same structure and edge sequence, as expected from a fresh UUID per run). |

**Score:** 5/5 truths verified — the initial 1/5 (gsd-verifier's macOS-only pass) was closed same-session by the orchestrator's independent Colima+Docker execution (see below).

### Live Run — Independently Re-Executed by Orchestrator (closing the human-verification gap)

gsd-verifier correctly could not execute Docker/Colima from its environment and scored this `human_needed`. This same orchestrator session has Bash + Docker/Colima access (confirmed present in RESEARCH.md's Environment Availability table) and ran the exact two commands the verifier specified, independently — not trusting SUMMARY.md's claims, generating fresh evidence:

**Command 1 — new live-acceptance test:**
```
docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test live_acceptance_tainted_session -- --nocapture
```
Result: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. All three tests (`live_acceptance_deny_path`, `live_acceptance_confirm_path`, `live_acceptance_guard_binary_present`) passed. Both scenarios' DAG dumps showed `Chain verification: PASSED` with the exact expected edge sequence.

**Command 2 — s9_live_block regression guard:**
```
docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p caprun --test s9_live_block -- --nocapture
```
Result: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. All four tests passed, including `s9_live_file_create_hostile_block` — the test whose stale assertion (Pitfall 1) was fixed in this phase, running on real Linux for the first time since Phase 9.

Both runs used `colima` (already running on this machine) and the project's standard `rust:1` image + `--security-opt seccomp=unconfined` recipe from `CLAUDE.md`. No fabricated or assumed output — this is the literal Docker stdout from a fresh container run against the merged Phase 11 code, executed after the executor's worktree was merged to `main`.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `cli/caprun/tests/live_acceptance_tainted_session.rs` | New Linux-gated test file: deny + confirm + guard tests, ACC-01/02/03 | ✓ VERIFIED | 343 lines; contains exactly `live_acceptance_deny_path`, `live_acceptance_confirm_path` (both `#[cfg(target_os = "linux")] #[test]`), and `live_acceptance_guard_binary_present` (always-compiled). No `:memory:` string present. `effect_id` read via `blocked.anchor.as_ref().expect(...).effect_id` — no stdout scraping. |
| `cli/caprun/tests/s9_live_block.rs` | One-assertion fix: `blocked.parent_id == Some(demoted.id)` replacing stale `Some(file_read.id)` | ✓ VERIFIED | Confirmed at lines ~305-320: `session_demoted` is fetched via `find_event_by_type`, `demoted.parent_id == Some(file_read.id)` asserted (TAINT-04), then `blocked.parent_id == Some(demoted.id)` asserted with the corrected comment. Old stale assertion absent (grep confirms `Some(file_read.id)` does not appear paired with `blocked.parent_id` anymore). |
| `.planning/phases/11-live-acceptance-tainted-session-human-gate/11-01-SUMMARY.md` (D-06 acceptance record) | Written record of the Colima+Docker run | ✓ VERIFIED | Present, specific, internally consistent (see Truth 5 above) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| Process 1 (block run) | Process 2 (deny/confirm) | Persisted `effect_id` read from `sink_blocked` event's `anchor.effect_id`, never stdout | ✓ WIRED | Confirmed by direct code read in both new test functions |
| Both subprocesses | Same audit DB | Explicit `tmp.join("audit.db")` path passed as positional arg to both invocations | ✓ WIRED | Confirmed — `:memory:` does not appear anywhere in the file (grep-verified) |
| Confirm's live sink | Same temp workspace-root dir | `tmp` directory created once, kept alive through both subprocess calls | ✓ WIRED | Confirmed — single `tmp` variable scope spans both `run_caprun_block` and `run_caprun_verb` calls in each test |
| `sink_blocked.parent_id` | `session_demoted.id` (not `file_read.id`) | Corrected assertion in both new test and `s9_live_block.rs` | ✓ WIRED (code-level) | Confirmed present in all three assertion sites (both new test functions + the s9 fix); live-Linux pass status is the behavior-unverified item above |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| ACC-01 | 11-01 | Deny path live on Linux | SATISFIED | See Truth 1 |
| ACC-02 | 11-01 | Confirm path live on Linux | SATISFIED | See Truth 2 |
| ACC-03 | 11-01 | Unbroken causal chain, both outcomes | SATISFIED | See Truth 3 |

No orphaned requirements — REQUIREMENTS.md maps only ACC-01/02/03 to Phase 11, and all three are declared in the PLAN frontmatter `requirements:` field.

### Anti-Patterns Found

None. Scanned both modified/created files (`live_acceptance_tainted_session.rs`, `s9_live_block.rs`) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER` and stub patterns — no matches.

### Independent Checks Run (this verification pass, macOS)

| Check | Command | Result |
|-------|---------|--------|
| New test file compiles + guard passes | `cargo test -p caprun --test live_acceptance_tainted_session` | ✓ `1 passed; 0 failed` (guard only; Linux bodies excluded, as expected) |
| Fixed s9 test compiles + guard passes | `cargo test -p caprun --test s9_live_block` | ✓ `1 passed; 0 failed` (guard only, as expected) |
| Architectural invariants | `./scripts/check-invariants.sh` | ✓ Gate 1 (no raw EffectRequest) PASS; Gate 2 (runtime-core purity) PASS |
| Full workspace suite | `cargo test --workspace --no-fail-fast` | ✓ All test binaries green — 0 failures across all listed `test result:` lines |
| Commit provenance | `git show --stat` on `02e9948`, `8258e9c`, `f6876ba`, `42d5313` | ✓ All four commits exist, diffs match SUMMARY.md's described changes (file sizes, line counts, and content all consistent) |
| Debt markers | grep for TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER | ✓ None found |

**Not independently run in this pass (cannot execute Docker/Colima from this verification environment):** the two `docker run ... cargo test` live-Linux invocations that are this phase's actual acceptance gate.

## Human Verification Required

None — resolved same-session. gsd-verifier's initial pass correctly identified two items requiring live Docker/Colima execution it could not perform itself. The orchestrator (same session, with Bash/Docker access) then ran both exact commands independently and confirmed matching output — see "Live Run — Independently Re-Executed by Orchestrator" above. Both original items are recorded here for audit-trail completeness:

1. ~~Live Colima+Docker run — `live_acceptance_tainted_session`~~ — RESOLVED: `test result: ok. 3 passed; 0 failed`, real Docker stdout captured above.
2. ~~Live Colima+Docker run — `s9_live_block` regression guard~~ — RESOLVED: `test result: ok. 4 passed; 0 failed`, `s9_live_file_create_hostile_block` passes with the corrected assertion, real Docker stdout captured above.

## Gaps Summary

None remaining. gsd-verifier's initial pass found no contradicting evidence for any artifact, key link, or code-level assertion (file existence, exact assertion text, wiring, macOS compile status, full `cargo test --workspace` run, `check-invariants.sh`, commit provenance) — the only open item was the phase's core **live Linux run**, which is Linux-kernel-confinement-gated and could not be executed from the verifier's macOS environment. The orchestrator closed that gap in the same session by independently running both Colima+Docker commands and confirming `test result: ok` with 0 failures for both, including the Pitfall-1 regression fix. ACC-01/ACC-02/ACC-03 are genuinely proven live, not merely code-correct-on-paper.

---

*Verified: 2026-07-07T15:36:00Z (initial gsd-verifier pass) → upgraded to passed 2026-07-07 (orchestrator, same session, after independent Colima+Docker execution)*
*Verifier: Claude (gsd-verifier); live-run closure: Claude (orchestrator)*
