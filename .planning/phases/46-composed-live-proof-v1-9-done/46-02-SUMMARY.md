---
phase: 46-composed-live-proof-v1-9-done
plan: 02
subsystem: testing
tags: [live-proof, composed, git-push, github-pr, http-write, caprun-audit, caprun-run, LIVE-05]

requires:
  - phase: 46-01
    provides: "POST /ingest -> 201 http.request.write mock endpoint (http_write_succeeded delivery)"
  - phase: 44
    provides: "git.push always-confirm-gate + confirmation::confirm release; evaluate_and_confirm pattern"
  - phase: 43
    provides: "http.request.write dispatch arm + differential legs"
  - phase: 45
    provides: "caprun run -> I2 Block -> surface + caprun audit read-only viewer (U1)"
  - phase: 38
    provides: "evaluate_plan_node_and_record_for_test (test-fixtures verbatim delegate to the live arm)"
provides:
  - "cli/caprun/tests/live_acceptance_v1_9_composed.rs — the v1.9 composed SUCCESS proof (half of the DONE gate)"
  - "Composed authorized-write chain (process.exec -> file.write -> git.commit -> git.push -> github.pr -> http.request.write POST) over ONE shared persisted audit.db through the REAL broker arms"
  - "Genuine caprun audit subprocess inspection (Chain verification: PASSED) per composed session"
  - "Genuine caprun run I2-Block leg landing in the shared audit.db"
affects: ["46-03", "46-04", "v1.9-milestone-DONE"]

tech-stack:
  added: []
  patterns:
    - "Composed-in-crate through the real broker arms (evaluate_plan_node_and_record_for_test) — not stapled, not a hand-rolled mirror"
    - "One shared persisted audit.db + sibling .key, F1-safe sibling layout, per-session verify_chain, final ORDER-BY-rowid sweep"
    - "CLI-driven block leg pointed at the shared db so it joins the composed session set + is caprun-audit-inspected"
    - "Framing-honesty module doc distinguishing composed-in-crate vs caprun-audit-inspected vs caprun-run-driven (v1.3 DOC-01)"

key-files:
  created:
    - cli/caprun/tests/live_acceptance_v1_9_composed.rs
  modified:
    - cli/caprun/Cargo.toml

key-decisions:
  - "Drive the multi-sink chain through evaluate_plan_node_and_record_for_test (the real production arm), not the individual invoke_* fns the v1.8 composed test used — closes the Phase-38 mirror-drift finding and exercises the always-confirm-gate + Allowed-dispatch arms verbatim."
  - "The CLI-driven caprun run Block leg targets the SHARED persisted audit.db (reusing the seeded .key via caprun's idempotent load_or_create_key) so its session joins the composed set, is swept, and is caprun-audit-inspected — a documented deviation from s45's own-fresh-db helper."
  - "Use SessionPolicy::broker_default() for every leg (PRODUCTION_SINKS permits all six sinks) rather than allow_all()."

patterns-established:
  - "Composed live proof driven through the real broker arms with genuine (provenance_chain[0] == real read-event id) non-stapled taint on the exec/commit mints."
  - "Framing-honesty module doc + machine-checkable disclosure grep (positive phrase present, negative overclaim absent)."

requirements-completed: [LIVE-05]

coverage:
  - id: D1
    description: "Composed authorized-write SUCCESS chain (process.exec -> file.write -> git.commit -> git.push confirm-release -> github.pr mock 201 -> http.request.write POST /ingest 201) over ONE shared persisted audit.db, each leg its own session, driven through the REAL broker arms with per-session verify_chain true."
    requirement: "LIVE-05"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/live_acceptance_v1_9_composed.rs#live_acceptance_v1_9_composed_success_chain (compose-verify, --features brokerd/mock-egress-ca, Linux)"
        status: unknown
    human_judgment: true
    rationale: "Authoritative pass is the Linux compose-verify gate the orchestrator runs at phase close (mock-egress-ca + both sidecars). On the macOS dev host the Linux body is cfg-excluded (0 legs run) and the pre-commit Linux Docker check is compile-only (--no-run); the runtime green is not yet observed here."
  - id: D2
    description: "git.push leg proves BlockedPendingConfirmation (always-confirm-gate, no auto-dispatch) THEN a genuine confirmation::confirm release to exactly one git_push_succeeded; push token + remote URL absent from all audit payloads."
    requirement: "LIVE-05"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/live_acceptance_v1_9_composed.rs#live_acceptance_v1_9_composed_success_chain (git.push leg, Linux+mock-egress-ca)"
        status: unknown
    human_judgment: true
    rationale: "Same as D1 — the confirm-release dispatch reaches a real mock git-receive-pack only on the Linux compose-verify gate."
  - id: D3
    description: "Genuine compiled caprun audit subprocess reports Chain verification: PASSED for every composed session (6 in-crate success + 1 CLI block), and at least one leg is genuinely caprun-run-driven (I2 Block surfaced + audited)."
    requirement: "LIVE-05"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/live_acceptance_v1_9_composed.rs#live_acceptance_v1_9_composed_success_chain (caprun audit / caprun run legs, Linux+mock-egress-ca)"
        status: unknown
    human_judgment: true
    rationale: "The caprun run leg self-confines a worker (Landlock+seccomp, Linux-only); its runtime pass is the compose-verify gate."
  - id: D4
    description: "Framing-honesty module doc states BLUNTLY what is composed-in-crate through the real broker arms vs caprun-audit-inspected vs caprun-run-driven, with the machine-checkable disclosure phrase present and the whole-chain overclaim absent."
    requirement: "LIVE-05"
    verification:
      - kind: automated
        ref: "grep -qi 'composed in-crate through the real broker arms' && grep -qi 'caprun audit' && grep -qi 'caprun run' && ! grep -qi 'caprun run drives the entire chain'"
        status: pass
    human_judgment: false
  - id: D5
    description: "Host-portable guard test keeps macOS cargo test -p caprun meaningful (CARGO_BIN_EXE_caprun resolves); Linux body cfg-excluded on host."
    requirement: "LIVE-05"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/live_acceptance_v1_9_composed.rs#live_acceptance_v1_9_composed_guard_binary_present"
        status: pass
    human_judgment: false

duration: ~45min
completed: 2026-07-18
status: complete
---

# Phase 46 Plan 02: Composed Live SUCCESS Proof (v1.9 DONE) Summary

**A Linux-gated composed live proof that drives the full authorized-write chain (process.exec → filesystem edit → git.commit → git.push confirm-release → github.pr → http.request.write POST) through the REAL broker arms over ONE shared persisted audit.db, inspects every session via a genuine `caprun audit` subprocess, and includes a genuine `caprun run` I2-Block leg — half of the v1.9 DONE gate (LIVE-05).**

## Performance

- **Duration:** ~45 min
- **Tasks:** 3
- **Files modified:** 2 (1 created, 1 modified)

## What is driven by what — FRAMING HONESTY (stated bluntly)

The three layers are deliberately NOT conflated (v1.3 DOC-01 discipline, mirrored verbatim in the test's module doc):

- **Composed in-crate through the real broker arms:** the six-sink authorized-write SUCCESS chain is submitted to the ACTUAL production dispatch arm `brokerd::server::evaluate_plan_node_and_record_for_test` (the `test-fixtures`-gated verbatim delegate to the live `evaluate_plan_node_and_record`), and the git.push leg is released through the real `brokerd::confirmation::confirm`. This is NOT expressible as a single `caprun run` (that verb plans one intent → one node → one sink; only email/file intents exist), and a multi-node composed-intent planner is out-of-scope new TCB. The composition is faithful (real sinks, real mock endpoints under `mock-egress-ca`, genuine non-stapled taint, per-session `verify_chain` true) because it runs the same arms the live daemon runs.
- **`caprun audit`-inspected:** every composed session is inspected by the REAL compiled `caprun audit <session> <db>` subprocess asserting `Chain verification: PASSED` — 100% real CLI, the read-only viewer proven in s45.
- **`caprun run`-driven:** one genuine `caprun run --policy <trusted> create-file-from-report …` subprocess drives a confined worker whose tainted `file.create` path I2-Blocks; the block is surfaced (`effect_id` + `caprun review` pointer) and audited. `caprun run` drives ONLY this single confined block leg; it never expresses the multi-sink write chain.

## Accomplishments

- New `cli/caprun/tests/live_acceptance_v1_9_composed.rs` — `#[cfg(target_os = "linux")]` composed proof; single sequential `#[tokio::test]`; one shared persisted `audit.db` (never `:memory:`) + sibling `.key` seeded before any append, F1-safe sibling layout.
- Six SUCCESS legs, each its own session, through the real broker arms:
  - **process.exec** + **git.commit** — Allowed → the REAL confined launcher runs → output minted by the arm; assert `provenance_chain[0]` == the real `process_exited` event id (genuine, non-stapled taint — Pitfall 1).
  - **file.write** — trusted path/contents (role `"path"`) Allows → `sink_executed`.
  - **git.push** — clean remote/refspec Allow at the executor → the always-confirm-gate re-gates to `BlockedPendingConfirmation` (NO auto-dispatch) → `confirmation::confirm` → exactly ONE `git_push_succeeded`; token + remote-URL absent from all payloads. Pushes a SMALL one-commit mock repo (see note).
  - **github.pr** — record grant → arm CAS + POST to the mock → exactly ONE `github_pr_succeeded`; bearer token never in any payload/actor (GITHUB-01, incl. a `ghp_` scan).
  - **http.request.write POST** — clean body → arm POSTs the 46-01 mock `POST /ingest` on the write-allowlisted host → 201 → exactly ONE `http_write_succeeded`.
- Genuine `caprun audit` subprocess per session (`Chain verification: PASSED`), plus a `sink_blocked` render assertion for the CLI-block session.
- Genuine `caprun run` I2-Block leg landing in the SHARED audit.db (reusing the seeded key), so it is swept + audited alongside the composed chain.
- Final sweep opens the shared audit.db ONCE (`ORDER BY rowid`, never `LIMIT 1`), asserts EXACTLY the 7-session composed set exists and every `verify_chain` is independently true.
- Host-portable guard test (CARGO_BIN_EXE_caprun resolves).

## git.push small-repo / 10MB pack-cap note

The git.push leg pushes a **SMALL one-commit mock repo** (`setup_git_push_repo`, mirroring s44). The 10MB pack-cap is therefore **non-blocking here** and is **deferred** (v1.9 REQUIREMENTS Out of Scope) — this composed proof does not exercise pack-cap enforcement.

## Task Commits

1. **Task 1: scaffold + shared persisted audit.db + framing-honesty doc** — `72a2bb5` (test)
2. **Task 2: six composed SUCCESS legs through the real broker arms** — `6076a59` (test)
3. **Task 3: genuine caprun audit inspection + caprun run Block leg + sweep** — `b5cc25c` (test)

## Files Created/Modified

- `cli/caprun/tests/live_acceptance_v1_9_composed.rs` — the composed SUCCESS proof (created).
- `cli/caprun/Cargo.toml` — dev-only `brokerd` self-feature enable `["test-fixtures"]` (modified; see Deviations).

## Verification

- **Host (macOS):** `cargo test -p caprun --test live_acceptance_v1_9_composed` → guard test passes (1 passed), Linux legs cfg-excluded (0 run — expected, CLAUDE.md).
- **Framing-honesty greps (Task 1):** `composed in-crate through the real broker arms` present; `caprun audit` + `caprun run` referenced; `caprun run drives the entire chain` overclaim ABSENT — all PASS.
- **Linux type-check (pre-commit, Docker rust:1 + `--features brokerd/mock-egress-ca --no-run`):** compiled clean (exit 0, executable built) — the `#[cfg(target_os = "linux")]` body type-checks under the mock-egress-ca feature (which macOS `cargo test` cannot see — cfg-linux-test-blindness).
- **`./scripts/check-invariants.sh`:** all gates PASS — Gate 1 (no new `EffectRequest`), Gate 3 (no new mint site), Gate 4/4b (test-fixtures + mock-egress-ca not default), Gate 5 (no aws-lc-rs / no new crate).
- **Authoritative (orchestrator at phase close):** `COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_9_composed --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh` — runtime green (every leg + caprun audit/run legs, verify_chain PASSED) is verified there, not on the macOS host.

## Decisions Made

- Drive every leg through `evaluate_plan_node_and_record_for_test` (the real arm) rather than the individual `invoke_*` fns the v1.8 composed test used — exercises the always-confirm-gate + Allowed-dispatch arms verbatim (closes the Phase-38 mirror-drift finding).
- Point the CLI-driven `caprun run` Block leg at the SHARED audit.db so its session joins the composed set (swept + audited).
- `SessionPolicy::broker_default()` for all legs (PRODUCTION_SINKS permits all six sinks).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Enabled `brokerd/test-fixtures` for caprun's test build**
- **Found during:** Task 2 (driving the six legs through the real arm).
- **Issue:** `brokerd::server::evaluate_plan_node_and_record_for_test` is gated behind brokerd's non-default `test-fixtures` feature (server.rs:1677). caprun depended on brokerd WITHOUT that feature, so the arm the plan mandates was not visible to the test binary — a compile blocker.
- **Fix:** Added a dev-only self-feature enable `brokerd = { path = "../../crates/brokerd", features = ["test-fixtures"] }` to `cli/caprun/Cargo.toml [dev-dependencies]` (mirrors brokerd's own self dev-dependency pattern). Dev-dependencies are excluded from `cargo build`, so the production binary + the compose-verify feature-OFF release guard are unaffected, and check-invariants Gate 4 (test-fixtures never a DEFAULT feature) still passes.
- **Files modified:** `cli/caprun/Cargo.toml`.
- **Verification:** Host + Linux (mock-egress-ca) compile green; `check-invariants.sh` Gate 4 PASS.
- **Committed in:** `72a2bb5` (Task 1 commit).

**2. [Deviation - documented] CLI-block leg targets the shared audit.db (not s45's own-fresh-db helper)**
- **Found during:** Task 3 (genuine caprun run Block leg + final sweep).
- **Issue:** Task 3(d) requires the final sweep to assert the composed success + CLI-block session set in ONE shared db, but s45's `run_genuine_hostile_file_create` creates its own fresh db.
- **Fix:** Wrote `run_cli_block_on_shared_db` — the same s45 hostile-file-create flow (same hostile content, trusted `file.create` policy, F1-safe layout), but pointed at the SHARED audit.db. caprun's idempotent `load_or_create_key` reads back the pre-seeded 32-byte `.key`, so the block session is MAC'd under the SAME key and joins the composed set for the sweep + `caprun audit` inspection.
- **Files modified:** `cli/caprun/tests/live_acceptance_v1_9_composed.rs`.
- **Verification:** F1-safety confirmed (audit.db is a sibling of `ws_clirun`, never beneath it); Linux type-check green.
- **Committed in:** `b5cc25c` (Task 3 commit).

---

**Total deviations:** 2 (1 Rule-3 blocking wiring fix, 1 documented approach deviation).
**Impact on plan:** Both necessary to satisfy the plan's own mandates (drive through the real arm; sweep the CLI-block session in the shared db). No scope creep; no security surface added.

## Issues Encountered

- macOS `cargo test` cannot type-check the `#[cfg(target_os = "linux")]` body (cfg-linux-test-blindness). Mitigated by a compile-only Linux Docker type-check (`rust:1`, `--features brokerd/mock-egress-ca --no-run`) before each commit; the authoritative runtime pass remains the orchestrator's compose-verify gate.

## Out-of-scope discoveries (not fixed — scope boundary)

- Pre-existing `unused_import: RulesetCreatedAttr` warning in `crates/sandbox/src/landlock.rs:18` surfaced during the Linux compile. Not caused by this plan; left untouched.

## Next Phase Readiness

- LIVE-05 SUCCESS half is delivered. The v1.9 DONE gate additionally needs 46-03 (the 5 independently-attributable negative legs + full compose-verify regression + milestone DONE record) and the orchestrator's authoritative compose-verify run.
- 46-03 can reuse this file's shared-db + real-arm + caprun-audit patterns for its negative legs.

## Self-Check: PASSED

- FOUND: cli/caprun/tests/live_acceptance_v1_9_composed.rs
- FOUND: cli/caprun/Cargo.toml (modified, committed)
- FOUND: .planning/phases/46-composed-live-proof-v1-9-done/46-02-SUMMARY.md
- FOUND: commits 72a2bb5, 6076a59, b5cc25c in git log

---
*Phase: 46-composed-live-proof-v1-9-done*
*Completed: 2026-07-18*
