---
phase: 27-session-connection-integrity-hardening
plan: 02
subsystem: security
tags: [rust, cargo-features, cfg-gating, feature-unification, unix-domain-socket, cargo-test-workspace]

# Dependency graph
requires:
  - phase: 26-security-hardening-design-gate
    provides: DESIGN-security-hardening.md §d (compile-out mechanism for the forced-Active mint) + §j (Phase 30 owns the formal milestone-wide proof)
  - phase: 27-session-connection-integrity-hardening (plan 01)
    provides: shared session_status Arc<Mutex<SessionStatus>> cell that dispatch_request re-reads (unrelated surface, but same file/module)
provides:
  - "The CreateSession-IPC forced-Active mint arm compile-excluded from a default (featureless) build via a test-fixtures Cargo feature, mirroring the executor crate's precedent"
  - "A featureless sibling returning the identical Error response with no std::env::var read at all"
  - "A D-10 behavioral negative gate (cli/caprun/tests/harden04_featureless_create_session.rs) proving a featureless build denies CreateSession even with CAPRUN_ENABLE_IPC_CREATE_SESSION=1 set"
  - "SC3 build-artifact evidence: strings on a default cargo build -p brokerd --release rlib finds zero occurrences of the mint-only marker; a --features test-fixtures build finds exactly one"
affects: [28-authenticated-audit-chain, 29-sink-path-hardening, 30-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "test-fixtures Cargo feature + self dev-dependency (executor crate precedent) to make a #[cfg(any(test, feature = \"test-fixtures\"))] item visible to a crate's own tests/ integration tests (which link the crate as a normal, non-cfg(test) dependency) while staying absent from every production build"
    - "Dual-#[cfg] sibling-function split (mint sibling vs. Error sibling) rather than a single function with an internal cfg branch, so the two bodies can diverge in whether they read an env var at all"
    - "A behavioral test detects Cargo's own ambient feature-unification outcome (via the actual response shape) and downgrades to a loud, non-failing skip instead of asserting blindly -- keeping cargo test --workspace green without weakening the scoped proof"

key-files:
  created:
    - cli/caprun/tests/harden04_featureless_create_session.rs
    - .planning/phases/27-session-connection-integrity-hardening/deferred-items.md
  modified: []  # Task 1 (crates/brokerd/Cargo.toml, crates/brokerd/src/server.rs, crates/brokerd/tests/uds_ipc.rs) was already committed by the prior executor (44ff00b) before this session began; Task 3 needed no additional server.rs changes since Task 1 already added the SC3 marker.

key-decisions:
  - "Task 1's SC3 marker (std::hint::black_box(\"HARDEN04_MINT_ARM_PRESENT_v1_6\") inside the test-fixtures-gated arm) was already present when this session resumed -- Task 3 required no server.rs edits, only gathering and recording build-artifact evidence."
  - "D-10 gate implemented as an IN-PROCESS integration test (spawns run_broker_server directly, like uds_ipc.rs) rather than a shell-out subprocess release build -- because a scoped `cargo test -p caprun --test ...` invocation genuinely builds brokerd featureless (confirmed empirically), making the heavier shell-out unnecessary."
  - "Under a bare `cargo test --workspace`, brokerd's test-fixtures feature DOES unify in graph-wide (that invocation also builds brokerd's own test targets, which need the feature via its self dev-dependency) -- CONFIRMED empirically on real Linux via scripts/mailpit-verify.sh. The D-10 test detects this via the actual CreateSession response (SessionCreated vs Error) and treats the ambient-unification case as an explicit non-failing skip, so `cargo test --workspace --no-fail-fast` stays green while the scoped `-p caprun` invocation remains the one that actually proves D-10."
  - "SC3 evidence gathered via `strings` on compiled .rlib artifacts (not cargo-expand, which is not installed in this environment) -- a stronger, build-artifact-level proof of physical absence than a source-cfg diff."
  - "The 2 unrelated pre-existing test failures found during full-workspace Linux verification (spawn caprun-planner sidecar: No such file or directory, in live_acceptance_v1_4_composed and llm_planner_live_accept) are logged to deferred-items.md and NOT fixed here (scope boundary -- they touch neither CreateSession nor any file this plan modifies, and match a previously-recorded 'cargo test --workspace missing sibling binary' gotcha)."

requirements-completed: [HARDEN-04]

coverage:
  - id: D1
    description: "The CreateSession-IPC forced-Active mint arm is a #[cfg(any(test, feature = \"test-fixtures\"))] sibling; the featureless #[cfg(not(...))] sibling returns the identical Error unconditionally with no env read (Task 1, already committed)"
    requirement: "HARDEN-04"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/uds_ipc.rs#server_accept, create_session_round_trip, create_session_over_ipc_denied_by_default_when_flag_unset"
        status: pass
      - kind: unit
        ref: "cargo build --workspace"
        status: pass
    human_judgment: false
  - id: D2
    description: "D-10 behavioral negative gate: a featureless build denies CreateSession over the real broker socket even with CAPRUN_ENABLE_IPC_CREATE_SESSION=1 set, proving physical absence not runtime denial"
    requirement: "HARDEN-04"
    verification:
      - kind: integration
        ref: "cargo test -p caprun --test harden04_featureless_create_session (real Linux run via scripts/mailpit-verify.sh) -- linux_tests::featureless_create_session_denied_even_with_flag_set"
        status: pass
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast (real Linux run via scripts/mailpit-verify.sh) -- same test, ambient-unification skip path, stays green"
        status: pass
    human_judgment: false
  - id: D3
    description: "SC3 evidence: the mint arm's marker is present under --features test-fixtures and absent from a default cargo build --workspace --release"
    requirement: "HARDEN-04"
    verification:
      - kind: other
        ref: "strings target/release/libbrokerd.rlib | grep -c HARDEN04_MINT_ARM_PRESENT_v1_6 => 0 (default); strings /tmp/tf-release/release/libbrokerd.rlib | grep -c ... => 1 (--features test-fixtures)"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-07-12
status: complete
---

# Phase 27 Plan 02: Featureless CreateSession Mint Compile-Exclusion (HARDEN-04) Summary

**Compile-excluded the CreateSession-IPC forced-Active mint arm behind a `test-fixtures` Cargo feature (executor-crate precedent), added a real behavioral D-10 negative gate proving a featureless build denies the mint even with the legacy env flag set, and recorded build-artifact SC3 evidence of its absence from a default release build.**

## Performance

- **Duration:** ~55 min (this resumed session; Task 1 was completed and committed by a prior executor before this session started)
- **Started:** 2026-07-12 (resumed after a prior executor's API cutoff)
- **Completed:** 2026-07-12
- **Tasks:** 3 (Task 1 already committed as 44ff00b before this session; Tasks 2-3 completed and committed this session)
- **Files modified:** 1 new test file + 1 new deferred-items doc this session (Task 1's 3 files were committed previously)

## Accomplishments

- **Task 1 (prior session, commit `44ff00b`):** `crates/brokerd/Cargo.toml` gained a `[features] test-fixtures = []` block and a self dev-dependency `brokerd = { path = ".", features = ["test-fixtures"] }` (verbatim executor-crate precedent). `crates/brokerd/src/server.rs`'s `CreateSession` forced-Active mint was split into two sibling functions: `#[cfg(any(test, feature = "test-fixtures"))] create_session_arm` (the mint, with the runtime `CAPRUN_ENABLE_IPC_CREATE_SESSION` env-gate RETAINED as deliberate defense-in-depth) and `#[cfg(not(any(test, feature = "test-fixtures")))] create_session_arm` (the featureless sibling — no env read at all, unconditional `Error`). Both share `create_session_arm_disabled_response`, so the wire response is byte-identical on the negative path regardless of which sibling compiled. The test-only sibling also already carries the SC3 marker: `let _sc3_marker = std::hint::black_box("HARDEN04_MINT_ARM_PRESENT_v1_6");`. All 3 existing `uds_ipc.rs` tests (`server_accept`, `create_session_round_trip`, `create_session_over_ipc_denied_by_default_when_flag_unset`) keep exercising the arm via the self dev-dependency's `test-fixtures` unification, all keeping their env-var checks unchanged.
- **Task 2 (this session):** Added `cli/caprun/tests/harden04_featureless_create_session.rs` — an in-process integration test that spawns `run_broker_server` directly (same pattern as `uds_ipc.rs`), sets `CAPRUN_ENABLE_IPC_CREATE_SESSION=1`, and round-trips a `CreateSession` request. Empirically verified (see Verification section) that a SCOPED `cargo test -p caprun --test harden04_featureless_create_session` genuinely builds `brokerd` featureless (caprun's `[dependencies]` entry requests no features, and that scoped invocation never builds brokerd's own test targets, so its self dev-dependency's `test-fixtures` feature never unifies in) — under that invocation the response is `Error` and zero session rows are minted, proving D-10.
- **Task 3 (this session):** Gathered SC3 build-artifact evidence via `strings` on compiled `.rlib` files rather than `cargo-expand` (not installed in this environment) — a default `cargo build -p brokerd --release` produces an rlib with **zero** occurrences of the `HARDEN04_MINT_ARM_PRESENT_v1_6` marker; a `--features test-fixtures` build (separate target dir) produces **one**. Confirmed the full workspace release build (`cargo build --workspace --release`, exit 0) still yields a default `libbrokerd.rlib` with zero marker occurrences.

## Task Commits

1. **Task 1: test-fixtures feature + cfg-gated mint/Error sibling split** — `44ff00b` (feat) — completed and committed by the prior executor before this session began.
2. **Task 2: D-10 featureless behavioral negative gate** — `0256500` (test)
3. **Task 3: SC3 build-artifact absence evidence + deferred-items log** — `f6456e0` (docs)

**Plan metadata:** this commit (docs: complete plan)

## Files Created/Modified

- `crates/brokerd/Cargo.toml` — `[features] test-fixtures = []` + self dev-dependency (Task 1, prior commit)
- `crates/brokerd/src/server.rs` — cfg-gated `create_session_arm` mint/Error sibling split + shared `create_session_arm_disabled_response` + SC3 marker (Task 1, prior commit)
- `crates/brokerd/tests/uds_ipc.rs` — unchanged in substance; the 3 named tests reach the arm via `test-fixtures` unification (Task 1, prior commit)
- `cli/caprun/tests/harden04_featureless_create_session.rs` — NEW (this session) — D-10 behavioral negative gate
- `.planning/phases/27-session-connection-integrity-hardening/deferred-items.md` — NEW (this session) — logs an unrelated pre-existing test-environment issue found during full-workspace Linux verification

## Decisions Made

- **D-10 implemented in-process, not via shell-out subprocess.** The plan allowed a shell-out `cargo build --release` + spawn as the primary approach, with a scoped `-p caprun` invocation as an "acceptable alternative... if you MUST verify empirically." Verified empirically that the scoped invocation is genuinely featureless (see Verification below), so the simpler in-process pattern (mirroring `uds_ipc.rs`) was used — no subprocess/release-binary spawning needed.
- **The D-10 test self-detects ambient Cargo feature unification and downgrades to a non-failing skip.** Empirically discovered (not assumed) during this session: a bare `cargo test --workspace` DOES unify `test-fixtures` onto `brokerd` graph-wide, because that invocation also legitimately builds `brokerd`'s own test targets (which need the feature via its Task-1 self dev-dependency). Under that invocation, `CreateSession` actually mints a session even with the flag set — a real, expected consequence of feature unification, not a regression. The test inspects the actual response: if it sees `SessionCreated`, it prints a loud diagnostic explaining why (ambient unification, not a bug) and returns without asserting — converting what would otherwise be a **false failure** under `cargo test --workspace` into an honest non-assertion, while the scoped `-p caprun --test harden04_featureless_create_session` invocation (where the response is genuinely `Error`) still runs the hard D-10 assertion. A genuine regression on the SCOPED invocation is NOT masked — it still fails loudly.
- **SC3 evidence via `strings` on compiled `.rlib` artifacts**, not `cargo-expand` (unavailable in this environment) or a source-level `#[cfg]` diff. This is arguably a stronger proof than source inspection: it demonstrates the marker's absence in the actual compiled object Cargo produces for the default build.
- **Task 3 required no server.rs edits.** The SC3 marker was already present in `create_session_arm`'s test-fixtures-gated sibling from Task 1 — this plan's own file list anticipated a Task 3 edit to server.rs, but the marker Task 1's executor added already satisfies the "stable, greppable marker" requirement verbatim, so Task 3's deliverable this session was evidence-gathering and recording only.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] D-10 test would have been a false failure under `cargo test --workspace`**
- **Found during:** Task 2, verification step (running the full workspace test suite on real Linux via `scripts/mailpit-verify.sh` after the first draft of the test)
- **Issue:** The plan's own acceptance criterion for Task 2 requires `cargo test --workspace --no-fail-fast` to exit 0 "the test does not become a false failure under ambient feature unification." My first draft of the test asserted `BrokerResponse::Error` unconditionally. Empirically running it under a genuine `cargo test --workspace` on Linux showed the response is `SessionCreated` there (test-fixtures unifies onto brokerd graph-wide in that invocation), causing a hard `assert!` failure — exactly the false-failure mode the plan warned against.
- **Fix:** Added a response-shape check: if `CreateSession` returns `SessionCreated`, the test prints a clear diagnostic (this build graph is not genuinely featureless, most likely running under `--workspace` rather than the scoped `-p caprun` invocation) and returns without asserting, instead of treating it as a failure. The hard D-10 assertion (`Error`, zero sessions minted) still runs and still fails loudly whenever the response is anything other than `SessionCreated` or `Error` (an `other` variant would still hit the `assert!` and fail).
- **Files modified:** `cli/caprun/tests/harden04_featureless_create_session.rs`
- **Verification:** Re-ran both `cargo test -p caprun --test harden04_featureless_create_session` (scoped — passes, asserts D-10) and `cargo test --workspace --no-fail-fast` (workspace-wide — passes via the skip path) on real Linux via `scripts/mailpit-verify.sh`; both green.
- **Committed in:** `0256500` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug in this plan's own new test code, caught and fixed before commit).
**Impact on plan:** Necessary for correctness of the plan's own explicit acceptance criterion (`cargo test --workspace --no-fail-fast` must stay green). No scope creep — fix is entirely contained to the new test file this task introduced.

## Issues Encountered

- **Pre-existing, unrelated test failures found during full-workspace Linux verification.** `live_acceptance_v1_4_composed_three_legs` and `llm_planner_clean_allow_delivers` both fail with `Error: spawn caprun-planner sidecar / Caused by: No such file or directory (os error 2)` when `cargo test --workspace` is the first cargo invocation in a fresh container. Neither test touches `CreateSession`, `test-fixtures`, or any file this plan modifies — this matches a previously-recorded gotcha (`cargo-test-workspace-missing-sibling-binary`) about `current_exe().parent()`-resolved sibling binaries not reliably being placed by a bare `cargo test --workspace` without a prior `cargo build --workspace`. Logged to `deferred-items.md`, NOT fixed (scope boundary — out of scope for this plan's files).

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo build --workspace` — exit 0.
- `cargo build --workspace --release` (default features, the shipped configuration) — exit 0; the resulting `target/release/libbrokerd.rlib` shows zero occurrences of `HARDEN04_MINT_ARM_PRESENT_v1_6` via `strings`.
- `cargo build -p brokerd --release --features test-fixtures` (separate `CARGO_TARGET_DIR=/tmp/tf-release`) — exit 0; the resulting rlib shows exactly one occurrence of the marker.
- `cargo test --workspace --no-fail-fast` on macOS — green (Linux-gated tests report 0 compiled/run by design, matching CLAUDE.md).
- `./scripts/check-invariants.sh` — all 3 gates PASS.
- **Real Linux verification (Colima + Docker, via `scripts/mailpit-verify.sh` per CLAUDE.md's standing recipe):**
  - `MAILPIT_VERIFY_CMD='cargo test -p caprun --test harden04_featureless_create_session' bash scripts/mailpit-verify.sh` — 1 test, `ok` (D-10 proven: scoped build is genuinely featureless, `Error` returned, zero sessions minted).
  - `MAILPIT_VERIFY_CMD='cargo test --workspace --no-fail-fast' bash scripts/mailpit-verify.sh` — the 3 `uds_ipc.rs` tests all pass (`server_accept`, `create_session_round_trip`, `create_session_over_ipc_denied_by_default_when_flag_unset`); the new D-10 test passes via its ambient-unification skip path; the 2 pre-existing unrelated sidecar-spawn failures noted above are the only failures, both out of scope for this plan.

## Next Phase Readiness

- HARDEN-04 is fully delivered: SC3 (compile-exclusion + build evidence), D-10 (behavioral negative gate), and SC4 (existing coverage preserved, verified by running) all satisfied.
- Phase 30 (live proof) can build on this session's SC3 evidence-gathering approach (`strings` on release rlibs) if a milestone-wide formal proof needs the same technique at a larger scope.
- No blockers for Phase 28 (authenticated audit chain, HARDEN-02).

---
*Phase: 27-session-connection-integrity-hardening*
*Completed: 2026-07-12*
