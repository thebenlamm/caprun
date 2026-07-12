---
phase: 27-session-connection-integrity-hardening
verified: 2026-07-12T20:13:43Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 27: Session & Connection Integrity Hardening Verification Report

**Phase Goal:** fd release to the confined worker (`RequestFd`) itself demotes the session to draft-only for the I1 reason, AND the `CreateSession`-IPC forced-`Active` mint arm is physically excluded from the production binary at compile time — both in `server.rs`'s session/connection lifecycle.

**Verified:** 2026-07-12T20:13:43Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap SC1–SC4)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Requesting an fd (RequestFd) for a workspace file demotes the session to Draft for the I1 reason, even if the worker never reports reading it back | ✓ VERIFIED | `server.rs` `RequestFd` arm (:1122-1266): fstat `(dev,ino)` compare on the already-open granted `File` vs `trusted_path`; on mismatch, writes `Draft` via `update_session_status` + locks the shared cell to `Draft`, all BEFORE `pass_fd` and independent of `ReportClaims`. Behavioral test `fd_grant_on_untrusted_path_demotes_without_report_claims` passes on both macOS (`cargo test -p brokerd --test harden01_session_integrity`) and real Linux (independently re-run via `scripts/mailpit-verify.sh`, 3/3 ok). |
| 2 | A benign, fragment-free document read still leaves the session Active, and the CONTROL-01 clean-send path still completes ungated | ✓ VERIFIED | `fd_grant_on_trusted_path_stays_active` passes (Mac + Linux, independently re-run). Real Linux CONTROL-01 regression `s9_control_ab_taint_driven` independently re-run via `scripts/mailpit-verify.sh` — `ok`. |
| 3 | The CreateSession-IPC forced-Active mint arm is excluded from a default production build via a compile-time feature/cfg — grep/build evidence, not merely `CAPRUN_ENABLE_IPC_CREATE_SESSION` runtime gating | ✓ VERIFIED | `crates/brokerd/src/server.rs` has two `create_session_arm` siblings: `#[cfg(any(test, feature = "test-fixtures"))]` (mint, retains env-gate as defense-in-depth) and `#[cfg(not(any(test, feature = "test-fixtures")))]` (returns the same `Error`, no `env::var` read at all). Independently rebuilt: default `cargo build --workspace --release` → `strings target/release/libbrokerd.rlib \| grep -c HARDEN04_MINT_ARM_PRESENT_v1_6` = **0**; a separate `cargo build -p brokerd --release --features test-fixtures` (isolated `CARGO_TARGET_DIR`) → same grep = **1**. |
| 4 | Existing test fixtures that relied on the runtime env-flag opt-in still exercise the forced-Active behavior under an explicit test-only compile feature — no coverage silently lost | ✓ VERIFIED | `crates/brokerd/Cargo.toml` gained `[features] test-fixtures = []` + self dev-dep `brokerd = { path = ".", features = ["test-fixtures"] }` (executor-crate precedent). All 3 pre-existing `uds_ipc.rs` tests (`server_accept`, `create_session_round_trip`, `create_session_over_ipc_denied_by_default_when_flag_unset`) independently re-run on real Linux via `scripts/mailpit-verify.sh` — all `ok`, no `#[ignore]` added (grep confirms). D-10 behavioral negative gate (`cli/caprun/tests/harden04_featureless_create_session.rs`) independently re-run on real Linux (scoped `-p caprun --test harden04_featureless_create_session`) — `ok`, proving a scoped-featureless build denies `CreateSession` even with `CAPRUN_ENABLE_IPC_CREATE_SESSION=1` set. |

**Score:** 4/4 truths verified (0 present-but-behavior-unverified)

### Plan-Level Must-Haves (27-01, 27-02 frontmatter)

All truths, artifacts, key_links, and prohibitions declared in both plans' frontmatter were independently checked against the actual code (not the SUMMARY narrative):

| Must-have (abridged) | Status | Evidence |
|---|---|---|
| fstat (dev,ino) compare, not path-string | ✓ VERIFIED | `server.rs:1168-1174` uses `MetadataExt::dev()/ino()` only; no `to_str()`/`rel_path ==` gates the decision (grep confirms) |
| `session_status` is a single shared, monotonic `Arc<Mutex<SessionStatus>>`, constructed once at `run_broker_server` entry, re-read at top of every `dispatch_request` and at the Planner Step-0.5 branch | ✓ VERIFIED | `server.rs:187` (`Arc::new(Mutex::new(initial_session_status))`), `:1112-1115` (dispatch_request re-read), `:545-548` (Planner branch re-read). Only construction sites reference `SessionStatus::Active` (grep) — no re-seed after entry |
| `session_demoted` Event parented on `fd_granted`, reusing the literal event_type | ✓ VERIFIED | `server.rs:1221-1239` — `demoted_event` parent_id = `fd_event_id`, `event_type` = `"session_demoted"` (matches `quarantine.rs`'s `mint_from_read` literal) |
| All 12 `dispatch_request` + 7 `run_broker_server` call sites updated; workspace green | ✓ VERIFIED | `cargo build --workspace` exits 0; `cargo test --workspace --no-fail-fast` exits 0 (Mac) |
| No `nix` crate added | ✓ VERIFIED | `grep nix crates/brokerd/Cargo.toml` — no match |
| `test-fixtures` feature + self dev-dep (executor precedent) | ✓ VERIFIED | `crates/brokerd/Cargo.toml:33-47` |
| Featureless sibling has no `std::env::var` read | ✓ VERIFIED | `server.rs:1060-1066` — unconditional `create_session_arm_disabled_response` call, no env read |
| quarantine.rs stale "SOLE I1 trust-flip site" comment corrected | ✓ VERIFIED | `grep -n "SOLE I1 trust-flip site" crates/brokerd/src/quarantine.rs` → no match (exit 1); replaced with "TWO broker-side I1 trust-flip sites" (:389) |
| DESIGN-session-trust-state.md §2/§5 amended | ✓ VERIFIED | §2 (:65-111) names both trust-flip sites and the Phase-27-realized note; §5 (:231-239) documents the `fd_granted -> session_demoted` causal edge |

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/brokerd/tests/harden01_session_integrity.rs` | 3 new tests (negative demotion, cross-connection Draft visibility, clean-path stays-Active) | ✓ VERIFIED | 426 lines, 3 `#[tokio::test]` fns, none Linux-gated; all 3 pass on Mac AND independently re-run on real Linux |
| `cli/caprun/tests/harden04_featureless_create_session.rs` | D-10 behavioral negative gate | ✓ VERIFIED | 228 lines, `#[cfg(target_os = "linux")]`-gated body (reports 0 on Mac by design per CLAUDE.md); independently re-run and passed on real Linux |
| `crates/brokerd/Cargo.toml` `[features]` block | `test-fixtures = []` + self dev-dep | ✓ VERIFIED | present, verbatim executor precedent |
| `crates/brokerd/src/server.rs` dual-`#[cfg]` sibling split | mint vs. Error sibling | ✓ VERIFIED | both `#[cfg(any(test, feature = "test-fixtures"))]` and `#[cfg(not(any(...)))]` present |
| `cli/caprun/src/main.rs` trusted-path derivation + threading | new positional arg into `run_broker_server` | ✓ VERIFIED | `:202` (`trusted_workspace_path`), `:255` (passed into the call) |

### Key Link Verification

| From | To | Via | Status |
|---|---|---|---|
| `main.rs` trusted-path | `run_broker_server` | new positional arg | ✓ WIRED |
| `run_broker_server` shared cell | `dispatch_request` | `&Arc<Mutex<SessionStatus>>` param, re-read at top | ✓ WIRED |
| `RequestFd` fd-grant demotion | audit DAG | `session_demoted` Event parented on `fd_granted` | ✓ WIRED (genuine causal edge, not stapled) |
| `CreateSession` arm dispatch | cfg-gated sibling | `create_session_arm(...)` call resolves to whichever sibling compiled | ✓ WIRED |

### Behavioral Spot-Checks / Independent Re-Execution

All of the following were run directly by the verifier (not taken from SUMMARY.md), both on macOS and on real Linux via `scripts/mailpit-verify.sh` (Colima+Docker):

| Check | Command | Result | Status |
|---|---|---|---|
| Workspace build | `cargo build --workspace` | exit 0 | ✓ PASS |
| Release build (default) | `cargo build --workspace --release` | exit 0 | ✓ PASS |
| Invariant gates | `./scripts/check-invariants.sh` | Gates 1/2/3 PASS | ✓ PASS |
| Full workspace tests (Mac) | `cargo test --workspace --no-fail-fast` | exit 0, 0 failures | ✓ PASS |
| harden01 tests (Mac) | `cargo test -p brokerd --test harden01_session_integrity` | 3/3 ok | ✓ PASS |
| harden01 tests (real Linux) | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test harden01_session_integrity --no-fail-fast' bash scripts/mailpit-verify.sh` | 3/3 ok | ✓ PASS |
| D-10 featureless gate (real Linux, scoped) | `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test harden04_featureless_create_session --no-fail-fast' bash scripts/mailpit-verify.sh` | 1/1 ok | ✓ PASS |
| uds_ipc 3 named tests (real Linux) | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test uds_ipc -- server_accept / create_session' bash scripts/mailpit-verify.sh` (2 runs) | 3/3 ok | ✓ PASS |
| CONTROL-01 clean-send regression (real Linux) | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block -- s9_control_ab_taint_driven' bash scripts/mailpit-verify.sh` | 1/1 ok | ✓ PASS |
| SC3 marker absent (default release) | `strings target/release/libbrokerd.rlib \| grep -c HARDEN04_MINT_ARM_PRESENT_v1_6` | 0 | ✓ PASS |
| SC3 marker present (test-fixtures build) | `strings <isolated-target>/release/libbrokerd.rlib \| grep -c HARDEN04_MINT_ARM_PRESENT_v1_6` | 1 | ✓ PASS |
| No `nix` crate | `grep nix crates/brokerd/Cargo.toml` | no match | ✓ PASS |
| No stale SOLE-trust-flip claim | `grep "SOLE I1 trust-flip site" crates/brokerd/src/quarantine.rs` | no match (exit 1) | ✓ PASS |

The two pre-existing, unrelated `spawn caprun-planner sidecar` failures (`live_acceptance_v1_4_composed_three_legs`, `llm_planner_clean_allow_delivers`/`llm_planner_live_accept`) noted in `deferred-items.md` are a known unrelated container build-ordering gotcha (matches the previously-recorded `cargo-test-workspace-missing-sibling-binary` incident) and are correctly out of scope for Phase 27 — confirmed neither test touches `CreateSession`, `test-fixtures`, `RequestFd`, or any file this phase modified.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| HARDEN-01 | 27-01 | fd release demotes session to Draft for I1, CONTROL-01 clean path unaffected | ✓ SATISFIED | See SC1/SC2 rows above; independently re-verified on Mac + real Linux |
| HARDEN-04 | 27-02 | CreateSession forced-Active mint compile-excluded from default build | ✓ SATISFIED | See SC3/SC4 rows above; independently re-verified via `strings` on two separately-built rlibs + real-Linux D-10 test |

**Process note (not a code gap):** `.planning/REQUIREMENTS.md`'s traceability table (line 61) still lists `HARDEN-04 | Phase 27 | Pending` and the `HARDEN-04` checkbox (line 20) is unchecked, even though `ROADMAP.md` marks both 27-01 and 27-02 complete and this verification independently confirms the HARDEN-04 code is landed and working. This is a documentation-reconciliation lag (the same pattern noted in the project's own prior-incident memory — "record sign-off before last plan" / requirements not yet reconciled after the final plan of a phase completes), not evidence the code is missing. Recommend running the phase-completion reconciliation step so `REQUIREMENTS.md` reflects the verified-complete state before starting Phase 28.

### Anti-Patterns Found

None. No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` markers found in any file modified by this phase. No `#[ignore]` added to any test (grep confirmed on `uds_ipc.rs` and the 4 Linux-gated test files that gained the placeholder trusted-path argument).

### Human Verification Required

None. All must-haves are either directly grep/build-verifiable or covered by a passing behavioral test independently re-run by the verifier on both macOS and real Linux (via Colima+Docker `scripts/mailpit-verify.sh`).

### Deferred Items

None applicable to this phase's own success criteria — the phase-27-scoped SC3 evidence and D-10 behavioral gate were explicitly bounded to phase-27 scope per the plan (the formal milestone-wide release-binary proof and full live-Linux regression are Phase 30's stated scope, §j of the DESIGN doc, and were correctly not required here).

### Gaps Summary

No gaps. All 4 roadmap Success Criteria and all plan-frontmatter must-haves (truths, artifacts, key_links, prohibitions) for both 27-01 and 27-02 were independently verified against the actual code and via fresh test runs on both macOS and real Linux — not merely accepted from SUMMARY.md narrative. The only discrepancy found (REQUIREMENTS.md traceability lag for HARDEN-04) is a documentation bookkeeping issue, not a goal-achievement gap, and is called out above for reconciliation before Phase 28.

---

_Verified: 2026-07-12T20:13:43Z_
_Verifier: Claude (gsd-verifier)_
