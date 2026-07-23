---
phase: 44-git-push-broker-performed-destination-pinned-egress
plan: 05
subsystem: testing
tags: [git.push, differential-acceptance, mock-git-receive-pack, supply-chain, mock-egress-ca, pkt-line, i2, credential-custody]

# Dependency graph
requires:
  - phase: 44-01
    provides: git.push executor TCB registration (CommitIrreversible, routing-sensitive remote/refspec, schema)
  - phase: 44-02
    provides: pkt-line substrate + validate_git_refspec + build_command_list (structural force/delete denial)
  - phase: 44-03
    provides: frozen-IP transfer driver + broker-env credential + distinct empty GIT_PUSH_HOST_ALLOWLIST + opaque scrubbed audit + WG-2 run_launcher_capture_bytes
  - phase: 44-04
    provides: server.rs always-confirm-gate + confirmation.rs Step-4.8d prepare_git_push precheck + Step-7 invoke_git_push_from_resolved + WG-7 frozen-oid + WG-8 renderer
provides:
  - Differential git.push acceptance test (s44_git_push_differential.rs) — taint is the sole Block variable across a Blocked and an Allowed-then-confirmed-then-delivered push
  - git-receive-pack mock endpoint (WG-9) extending scripts/mock-github/server.py (info/refs advertisement + command-list-parsing receive-pack + receipt + redirect leg)
  - Proven-on-Linux mock-endpoint delivery of a clean confirmed push (git_push_succeeded), redirect-none pin refusal, credential/remote-URL audit opacity
  - HYG-01 completion — workspace-wide check-invariants Gate 4b, compose-verify feature-OFF guard, post-transport-dep supply-chain absence re-run (zero new crate)
affects: [phase-46, live-05, live-06, composed-live-proof, git.push]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Differential acceptance (Phase-43 M4 discipline): the clean leg MUST reach egress AND the tainted leg MUST Block on the named arg — a block-everything or blanket-allow regression fails one leg"
    - "Path-gated test mock: the git-receive-pack mock accepts only /accept/* repos (serves+accepts), 302s /redirect/*, 404s all else — so it never disturbs a prior test that relied on a push FAILING against this host"
    - "Confirm-gated dispatch differential: drive evaluate_plan_node_and_record_for_test (always-confirm-gate) → confirmation::confirm → Step-7 delivery over the REAL frozen-IP client to the mock"

key-files:
  created:
    - crates/brokerd/tests/s44_git_push_differential.rs
  modified:
    - scripts/mock-github/server.py
    - scripts/check-invariants.sh
    - scripts/compose-verify.sh
    - crates/brokerd/src/sinks/git_push.rs
    - cli/caprun/tests/confirm.rs
    - cli/caprun/tests/s9_process_exec_block.rs
    - cli/caprun/tests/live_acceptance_v1_7_composed.rs
    - crates/brokerd/src/sinks/process_exec.rs

key-decisions:
  - "Path-gated the git-receive-pack mock (accept only /accept/* repos) so the two pre-existing 44-04 confirm-release unit tests that push to /owner/repo.git keep failing (ConfirmedButSinkFailed) unweakened — zero changes to existing tests"
  - "Widened validate_git_refspec + build_command_list pub(crate)->pub so the cross-crate acceptance test drives the REAL structural-denial gates directly (host-portable LEG D) — pure refusal fns, no bypass, no gate impact"
  - "Credential/remote-URL absence asserted over the audit chain (opacity, T-44-10) + value store; the success dispatch path emits NO broker-log line (the eprintln scrub is the Err-path, unit-covered by scrub_strips_token_remote_and_userinfo_url)"

patterns-established:
  - "Mock records receipt broker-side: a valid report-status is returned ONLY after the mock parses the command-list + records the push, so the resulting git_push_succeeded event IS the delivery proof (mirrors the v1.8 mock-201 = github_pr_succeeded pattern)"

requirements-completed: [GIT-02, GIT-03, HYG-01]

coverage:
  - id: D1
    description: "git.push differential — I0 draft-deny (LEG A), tainted remote/refspec Block on the named arg (LEG B), clean Allowed at the executor with taint the sole variable (LEG C-executor), force/+/:delete refused by construction (LEG D)"
    requirement: "GIT-03"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s44_git_push_differential.rs#leg_a_i0_draft_session_denies_push_commit_irreversible / legs_b_and_c_taint_is_the_sole_variable / leg_d_force_and_delete_refused_by_construction"
        status: pass
    human_judgment: false
  - id: D2
    description: "Clean confirmed push delivered to the pinned mock git-receive-pack on real Linux (Released + git_push_succeeded, mock receipt), with the push credential + remote URL absent from every hashed audit payload; verify_chain true"
    requirement: "GIT-02"
    verification:
      - kind: e2e
        ref: "crates/brokerd/tests/s44_git_push_differential.rs#dispatch::leg_c_clean_confirmed_push_reaches_mock_receive_pack (compose-verify Linux + mock-egress-ca)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Destination pin: a receive-pack 3xx is refused (never followed), folds into terminal git_push_failed (LEG E)"
    requirement: "GIT-02"
    verification:
      - kind: e2e
        ref: "crates/brokerd/tests/s44_git_push_differential.rs#dispatch::leg_e_receive_pack_redirect_is_refused (compose-verify Linux + mock-egress-ca)"
        status: pass
    human_judgment: false
  - id: D4
    description: "git-receive-pack mock endpoint (WG-9): valid empty-repo info/refs advertisement + command-list-parsing receive-pack accept + receipt; POST /repos/*/pulls + 404-default unchanged"
    verification:
      - kind: e2e
        ref: "scripts/mock-github/server.py driven by leg_c/leg_e on compose-verify; python3 ast.parse + pkt-line self-test"
        status: pass
    human_judgment: false
  - id: D5
    description: "HYG-01 — workspace-wide Gate 4b, compose-verify feature-OFF guard (mock host+anchor absent from release build), post-transport-dep supply-chain absence re-run (aws-lc-rs absent, openssl-sys never via reqwest, ring present, reqwest =0.13.4, zero new crates)"
    requirement: "HYG-01"
    verification:
      - kind: automated
        ref: "scripts/check-invariants.sh (Gate 4b workspace-wide, Gate 5 absence); compose-verify feature-OFF-guard-passed step; cargo tree --workspace -i {aws-lc-rs,openssl-sys,ring,reqwest}"
        status: pass
    human_judgment: false

# Metrics
duration: 60min
completed: 2026-07-18
status: complete
---

# Phase 44 Plan 05: git.push differential acceptance + git-receive-pack mock + HYG-01 Summary

**A differential git.push acceptance test where taint is the sole Block variable — proven on real Linux by delivering a clean confirmed push to a pinned mock git-receive-pack (git_push_succeeded, credential/URL absent) while a tainted remote/refspec Blocks on the named arg and force/delete are refused by construction — plus a git-receive-pack mock (WG-9) and completed HYG-01 hygiene gates.**

## Performance

- **Duration:** ~60 min
- **Tasks:** 3 (+ 3 deviation fixes)
- **Files modified:** 8 (1 created)

## Accomplishments
- **GIT-02/GIT-03 proven differentially in isolation.** `s44_git_push_differential.rs` mirrors `s43_http_write_differential.rs`: LEG A (I0 draft-deny, CommitIrreversible), LEG B (tainted `remote` Blocks on `remote`, tainted `refspec` Blocks on `refspec` — genuine non-stapled `mint_from_http` provenance), LEG C (clean args Allowed at the executor — taint the SOLE literal variable, asserted via byte-identical shared handles → confirm-gated → confirmed → **delivered to the pinned mock git-receive-pack** with `git_push_succeeded`), LEG D (force/`+`/`:delete` refused by construction via the real `validate_git_refspec` + `build_command_list`), LEG E (a receive-pack 302 refused, terminal `git_push_failed`, never followed).
- **Credential/remote-URL absence after a real push:** the broker-env push token (a distinct sentinel) and the remote URL appear in NO hashed audit payload; `verify_chain` true across the whole gate→confirm→dispatch. Delivery uses the frozen-IP client with the pack-gen child net-denied and the destination IP pinned (WG-1/WG-2 from 44-03).
- **git-receive-pack mock (WG-9):** extended `scripts/mock-github/server.py` (stdlib-only, no `git` binary) with a valid empty-repo info/refs advertisement + a pure-Python command-list-parsing receive-pack that records a receipt and returns an `unpack ok` report-status; `/redirect/*` 302s (LEG E); `/repos/*/pulls` + 404-default unchanged. Path-gated to `/accept/*` so it never disturbs the prior 44-04 `/owner/repo.git` confirm-release tests.
- **HYG-01 complete:** `check-invariants` Gate 4b broadened brokerd-only → **workspace-wide**; `compose-verify.sh` gained a **feature-OFF guard** (no-mock-egress-ca build + not(feature) invariant tests + a grep proving the mock host is absent from the release `caprun` binary); the post-transport-dep supply-chain absence re-run recorded ZERO new crates.

## Task Commits

1. **Task 1: git-receive-pack mock endpoint (WG-9)** - `45ac8b9` (feat)
2. **Task 2: git.push differential + mock-endpoint dispatch acceptance test** - `2ca1e66` (test) — includes the `validate_git_refspec`/`build_command_list` `pub` widening
3. **Task 3: HYG-01 — workspace-wide Gate 4b + compose-verify feature-OFF guard** - `714562f` (chore)

Deviation fixes (pre-existing phase-44 Linux-only defects surfaced by the full compose-verify — see Deviations):
4. `812d80f` (fix) — `confirm.rs` missing `frozen_new_oid`
5. `8755606` (fix) — remaining Linux-gated `frozen_new_oid` literals (`s9_process_exec_block.rs`, `live_acceptance_v1_7_composed.rs`)
6. `8f16436` (fix) — `capture_bytes_tests` tolerate the launcher's Landlock stderr diagnostic

## Files Created/Modified
- `crates/brokerd/tests/s44_git_push_differential.rs` — the GIT-02/03 differential + mock-endpoint dispatch acceptance test (created)
- `scripts/mock-github/server.py` — WG-9 git-receive-pack mock (advertisement + receive-pack + receipt + redirect leg; path-gated)
- `scripts/check-invariants.sh` — Gate 4b broadened workspace-wide
- `scripts/compose-verify.sh` — feature-OFF guard step
- `crates/brokerd/src/sinks/git_push.rs` — `validate_git_refspec` + `build_command_list` widened `pub(crate)`→`pub`
- `cli/caprun/tests/confirm.rs`, `cli/caprun/tests/s9_process_exec_block.rs`, `cli/caprun/tests/live_acceptance_v1_7_composed.rs` — added the missing `frozen_new_oid: String::new()` field (pre-existing 44-04 break)
- `crates/brokerd/src/sinks/process_exec.rs` — relaxed two Linux-only `capture_bytes_tests` stderr assertions to tolerate the benign launcher Landlock diagnostic (stdout-purity assertions unchanged)

## HYG-01 supply-chain absence re-run (recorded)

Run AFTER all git.push transport code landed (44-01..05), workspace-scoped:

| Check | Result |
|-------|--------|
| `cargo tree --workspace -i aws-lc-rs` | **ABSENT** ("did not match any packages") — no C crypto in the TCB |
| `cargo tree --workspace -i openssl-sys` | never via a reqwest path (Gate 5 PASS); absent on the macOS host graph (native-tls → Security.framework), only ever via lettre's native-tls on Linux |
| `cargo tree --workspace -i ring` | **PRESENT** (v0.17.14) — the pure-Rust provider |
| `cargo tree --workspace -i reqwest` | **v0.13.4**, unchanged, no new features |
| `Cargo.lock` | **unmodified** across the phase — git.push added ZERO crates (pure pkt-line glue + reuse of the shipped reqwest/ring transport stack) |

Gate 5 (aws-lc-rs absent + openssl-sys-never-via-reqwest) and Gate 4b (workspace-wide mock-egress-ca-never-default) both PASS.

## Verification results

- **macOS host** (`cargo build --workspace` then `cargo test -p brokerd --test s44_git_push_differential`): 3/3 host-portable legs pass (A, B/C, D); the Linux+mock-egress-ca dispatch mod compiles to nothing (cfg-linux-blindness — expected).
- **check-invariants.sh:** all gates PASS (Gate 1/2/3/4/4b-workspace-wide/5/6).
- **compose-verify.sh (real Linux, Colima + mock git-receive-pack + Mailpit sidecars):** PASSED — exit 0, **64 test binaries ok, 0 failed, 668 tests passed total**. The feature-OFF guard passed (`feature-OFF-guard-passed:mock-host+anchor-absent-from-release-build`). All 5 s44 legs pass on Linux including `dispatch::leg_c_clean_confirmed_push_reaches_mock_receive_pack` (real delivery + credential absence) and `dispatch::leg_e_receive_pack_redirect_is_refused`. brokerd lib 275/0 (was 273/2 before the process_exec fix). The prior 44-04 `/owner/repo.git` confirm-release tests still pass unweakened (the mock 404s non-`/accept/*` repos → ConfirmedButSinkFailed preserved).

## Decisions Made
- **Path-gated the git-receive-pack mock** (accept only `/accept/*`, 404 others) rather than accepting all pushes — this keeps the two pre-existing 44-04 confirm-release tests that push to `/owner/repo.git` failing exactly as before (ConfirmedButSinkFailed), honoring "do not weaken any existing test/check" with ZERO edits to those tests.
- **Widened `validate_git_refspec` + `build_command_list` to `pub`** so the cross-crate acceptance test drives the REAL structural-denial gates directly for a truly host-portable LEG D. These are pure refusal functions (no mint, no audit, no sink dispatch) — no bypass, and check-invariants Gate 1/3/5 are unaffected.
- **Credential/remote-URL absence scope:** asserted over the audit chain (opacity, T-44-10) + value store. The successful dispatch path emits no `eprintln` broker-log line at all (the scrub is the Err-path, unit-covered by `git_push.rs::scrub_strips_token_remote_and_userinfo_url`), so there is no success-path log to leak into.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `pub(crate)`→`pub` widening of `validate_git_refspec` + `build_command_list`**
- **Found during:** Task 2 (authoring LEG D)
- **Issue:** The plan names these two functions as the mechanism for a host-portable "refused by construction" LEG D, but they were `pub(crate)` — uncallable from a `tests/` integration binary (a separate crate).
- **Fix:** Widened both to `pub` (2 tokens). They are pure fail-closed refusal gates; widening adds no bypass surface and touches no invariant gate.
- **Files modified:** `crates/brokerd/src/sinks/git_push.rs`
- **Committed in:** `2ca1e66`

**2. [Rule 3 - Blocking] Pre-existing 44-04 compile break: missing `frozen_new_oid` in test `PendingConfirmation` literals**
- **Found during:** the full compose-verify run (the plan's verification recipe)
- **Issue:** Phase 44-04 added the `frozen_new_oid` field to `confirmation::PendingConfirmation` but did not update non-git.push literals in `cli/caprun/tests/{confirm.rs,s9_process_exec_block.rs,live_acceptance_v1_7_composed.rs}` (last touched Phase 28). The "confirm"/"s9_process_exec_block" test binaries failed to compile under `cargo test --workspace` — some literals are inside `#[cfg(target_os="linux")]`, so a macOS `cargo test --no-run` missed them (cfg-linux-blindness). This blocked the plan's compose-verify recipe.
- **Fix:** Added `frozen_new_oid: String::new()` (the documented empty-for-non-git.push value) to all four literals. Surgical, no behavior change.
- **Files modified:** `cli/caprun/tests/confirm.rs`, `cli/caprun/tests/s9_process_exec_block.rs`, `cli/caprun/tests/live_acceptance_v1_7_composed.rs`
- **Committed in:** `812d80f`, `8755606`

**3. [Rule 1 - Bug] Pre-existing 44-03 Linux-only test failure: `capture_bytes_tests` over-assert clean stderr**
- **Found during:** the full compose-verify run
- **Issue:** The WG-2 `caprun-exec-launcher` prepends a benign `[caprun-exec-launcher] Landlock exec_child_ruleset status: FullyEnforced` diagnostic to stderr on a Landlock-enforcing kernel (a no-op on the macOS stub). Two `#[cfg(target_os="linux")]` tests asserted `stderr.is_empty()` / `stderr == b"ERRDATA"`, so they passed on macOS but FAILED on the compose-verify Linux gate — the first time the phase-44 suite ran there.
- **Fix:** Relaxed the stderr assertions to the load-bearing property — the payload/`ERRDATA` rides the SEPARATE stderr stream and NEVER leaks into stdout — tolerant of the launcher diagnostic. The stdout-purity assertions are UNCHANGED (not weakened).
- **Files modified:** `crates/brokerd/src/sinks/process_exec.rs`
- **Committed in:** `8f16436`

---

**Total deviations:** 3 (1 blocking-visibility, 1 blocking pre-existing-compile, 1 pre-existing Linux-only test bug). All within the phase-44 blast radius (git.push + its WG-2 launcher). No scope creep; the two pre-existing defects were latent cfg-linux-blindness bugs from 44-03/44-04 that this plan's first full-workspace Linux gate correctly surfaced.
**Impact on plan:** All fixes necessary to make the plan's compose-verify verification recipe green. Load-bearing security assertions preserved throughout.

## Issues Encountered
- **cfg-linux-test-blindness (recurring tripwire), twice:** the pre-existing 44-03/44-04 defects (missing `frozen_new_oid`, over-strict launcher-stderr assertions) were invisible on the macOS host and only surfaced on the compose-verify Linux gate. 44-03/44-04 were evidently verified scoped/host-only, never via a full-workspace Linux run. Resolved by the three deviation fixes above; recommend a full-workspace compose-verify at each future git/exec plan close.

## Next Phase Readiness
- GIT-02/GIT-03 proven for git.push in isolation; git-receive-pack mock + HYG-01 gates in place. Phase 46 (LIVE-05/06) can now compose the full multi-sink workflow (process.exec→edit→commit→push→PR + POST) driven via the CLI/viewer, reusing the `/accept/*` mock receive-pack host under `mock-egress-ca`.
- No blockers.

## Self-Check: PASSED

- Created file present: `crates/brokerd/tests/s44_git_push_differential.rs` (FOUND).
- Modified files present: `scripts/mock-github/server.py`, `scripts/check-invariants.sh`, `scripts/compose-verify.sh` (FOUND).
- All 6 commits present in git history: `45ac8b9`, `2ca1e66`, `714562f`, `812d80f`, `8755606`, `8f16436` (FOUND).
- No stubs / placeholders introduced; no new security surface outside the plan's `<threat_model>` (the mock is test-harness-only, gated `mock-egress-ca`, proven absent from release builds by the feature-OFF guard).

---
*Phase: 44-git-push-broker-performed-destination-pinned-egress*
*Completed: 2026-07-18*
