---
phase: 29-sink-path-hardening-replay-cas-contents-slot
verified: 2026-07-17T18:00:00Z
status: passed
score: 6/6 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 29: Sink-Path Hardening (Replay CAS + Contents Slot) Verification Report

**Phase Goal:** the trusted `email.send` path is replay-safe (at-most-once, matching the confirm path's transaction discipline), and `file.create`'s `contents` arg is no longer an unconstrained slot.
**Verified:** 2026-07-17T18:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `sent_plan_nodes` CAS table exists, migrates idempotently | ✓ VERIFIED | `crates/brokerd/src/audit.rs:216-221` DDL; `:349-364` `migrate_sent_plan_nodes_schema`; wired at `:435` in `open_audit_db`. Unit test `sent_plan_nodes_migration_is_idempotent` (`:1421`) run independently — 1 passed. |
| 2 | `plan_node_idempotency_key` is order-invariant, sink-scoped, value-distinguishing, deterministic, content-derived (never `effect_id`) | ✓ VERIFIED | `audit.rs:404-419` — sorts args by name, hashes `sink.0` + `(name, value_id)` pairs; no `effect_id` reference in fn body. 4 unit tests (`idempotency_key_is_order_invariant/sink_scoped/distinguishes_value_id/is_deterministic`) run independently — 4 passed. |
| 3 | A replayed identical Allowed `email.send` delivers AT MOST ONCE (1 Mailpit delivery, 1 `sent_plan_nodes` row, 1 `email_send_attempted` event) | ✓ VERIFIED | `server.rs:901-1028`: CAS `INSERT OR IGNORE` + branch-divergent event append (`email_send_attempted` vs `email_send_replay_suppressed`) committed via `tx.commit()` (`:1003`) BEFORE `invoke_email_smtp_from_resolved` (`:1017`, only on `rows_affected==1`). Live-Linux test `allowed_email_send_replay_delivers_once` (`crates/brokerd/tests/replay_cas.rs:255-412`) re-run independently via `scripts/mailpit-verify.sh` on real Linux (Colima) — true exit 0 captured before pipe; `test allowed_email_send_replay_delivers_once ... ok`, all 3 named assertions present and passing. |
| 4 | CAS `INSERT` + attempt-append commit atomically BEFORE any SMTP socket opens | ✓ VERIFIED | Single `let tx = locked.transaction()?;` (`server.rs:956`) wraps the CAS insert (`:958-967`) and `append_event` (`:998`); `tx.commit()?` (`:1003`) precedes the `invoke_email_smtp_from_resolved` call (`:1017`), which sits in the `else` branch reached only after commit. |
| 5 | `file.create`'s `contents` arg is content-sensitive AND role-checked to `Some(&["path"])`; `path` unchanged, no over-widening | ✓ VERIFIED | `sink_sensitivity.rs:85` `FILE_CREATE_CONTENT_SENSITIVE = &["contents"]`; `:112` `.contains()` arm (not unconditional `true`); `:176` `"contents" => Some(&["path"])`; `:164` `"path"` arm unchanged. Independently ran `cargo test -p executor sink_sensitivity` — 20/20 passed, including `file_create_contents_expects_path`, `file_create_contents_is_content_sensitive`, `file_create_path_not_content_sensitive` (over-widening guard). |
| 6 | The only live `file.create` flow still Allows (no false-positive block) | ✓ VERIFIED | `s9_live_file_create_clean_allow` re-run independently on real Linux via `scripts/mailpit-verify.sh` — `test s9_live_file_create_clean_allow ... ok`. Also confirmed the newly-enforced Step 1c check required fixing 4 pre-existing test fixtures (`executor_decision.rs`, `durable_anchor.rs`, `harden01_session_integrity.rs`, `s9_acceptance.rs`) from `contents` role `None` to `Some("path")` — inspected each diff; genuine regression repair (fixture now mints the reused trusted `"path"` role matching the real planner shape), not a weakened assertion. `unconstrained_slot_unaffected` was correctly inverted to `file_create_contents_role_mismatch_denies`, now asserting a proper `Denied(SlotTypeMismatch)` — a strengthening, not a softening. |

**Score:** 6/6 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/brokerd/src/audit.rs` | `sent_plan_nodes` DDL + migration + `plan_node_idempotency_key` + unit tests | ✓ VERIFIED | All present, exact pinned shape, tests pass |
| `crates/brokerd/src/server.rs` | CAS-guarded Allowed `email.send` dispatch | ✓ VERIFIED | Transaction wraps CAS+append, commit-before-SMTP; stale REPLAY RESIDUAL comment replaced with honest D-08-scoped comment |
| `crates/brokerd/tests/replay_cas.rs` | Linux-only double-submit integration test | ✓ VERIFIED | Exists, `#![cfg(target_os = "linux")]`-gated, drives real `run_broker_server`, asserts all 3 counts; passed live on Linux |
| `crates/executor/src/sink_sensitivity.rs` | `file.create` `contents` role/sensitivity treatment | ✓ VERIFIED | Const + arms added per plan, inverted + guard tests present and passing |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `server.rs` Allowed `email.send` block | `audit::plan_node_idempotency_key` | direct fn call `crate::audit::plan_node_idempotency_key(&plan_node.sink, &plan_node.args)` (`server.rs:912-913`) | ✓ WIRED | Confirmed by source read |
| `server.rs` CAS insert | `sent_plan_nodes` table | `INSERT OR IGNORE INTO sent_plan_nodes ... VALUES (?1,?2,?3,?4)` (`server.rs:958-967`) | ✓ WIRED | Column order/types match 29-01's DDL |
| `expected_role(file.create, contents)` | `planner.rs:208`'s reused `"path"`-role literal | `Some(&["path"])` accepts the only live production value | ✓ WIRED | Confirmed by live Linux regression pass (`s9_live_file_create_clean_allow`) |
| `open_audit_db` | `migrate_sent_plan_nodes_schema` | called immediately after `migrate_chain_anchor_schema` (`audit.rs:434-435`) | ✓ WIRED | Confirmed by source read |

### Behavioral Spot-Checks / Probe Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| brokerd migration + key unit tests (macOS) | `cargo test -p brokerd idempotency_key` / `sent_plan_nodes` | 4 passed / 1 passed, 0 failed | ✓ PASS |
| executor sink_sensitivity unit tests (macOS) | `cargo test -p executor sink_sensitivity` | 20 passed, 0 failed | ✓ PASS |
| Full workspace (macOS) | `cargo test --workspace --no-fail-fast` | 0 failed across all crates/binaries | ✓ PASS |
| Architectural invariants | `./scripts/check-invariants.sh` | All 4 gates PASS (no `EffectRequest`, runtime-core pure, mint restricted, test-fixtures not default) | ✓ PASS |
| Workspace build | `cargo build --workspace` | Compiles clean | ✓ PASS |
| **Live Linux replay CAS proof** | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test replay_cas allowed_email_send_replay_delivers_once' bash scripts/mailpit-verify.sh` (re-run independently by this verifier via Colima, NOT trusting the prior SUMMARY claim) | `test allowed_email_send_replay_delivers_once ... ok`; true exit 0 captured before pipe | ✓ PASS |
| **Live Linux file.create regression** | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_live_file_create_clean_allow' bash scripts/mailpit-verify.sh` (re-run independently) | `test s9_live_file_create_clean_allow ... ok` | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| HARDEN-03 | 29-01, 29-02 | Replayed Allowed `email.send` sends at most once via idempotency key/CAS | ✓ SATISFIED | CAS table + key (29-01), transaction wiring + live proof (29-02), all independently re-verified above |
| HARDEN-05 | 29-03 | `file.create` `contents` carries expected-role/sensitivity treatment (I2 slot-type discipline) | ✓ SATISFIED | `sink_sensitivity.rs` edits + inverted/guard tests, live regression canary re-verified |

**Note on REQUIREMENTS.md checkbox state:** `.planning/REQUIREMENTS.md` still shows HARDEN-03/HARDEN-05 as `[ ]`/"Pending" (traceability table). This is a known record-lag artifact of `update-plan-progress` auto-flipping the ROADMAP checkbox on last-plan completion without touching REQUIREMENTS.md — per this phase's task instructions, this is NOT treated as a code gap; `phase.complete` reconciles it after this verification. Both requirement IDs (`HARDEN-03`, `HARDEN-05`) are correctly declared in all 3 plans' frontmatter and traced to real, tested code above.

### Anti-Patterns Found

None. Scanned all 4 modified files (`audit.rs`, `server.rs`, `sink_sensitivity.rs`, `replay_cas.rs`) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER` — zero matches. No stub returns, no hardcoded-empty stand-ins on the changed code paths.

### Human Verification Required

None. All truths were verifiable via source trace plus independently re-run automated/behavioral tests (both macOS unit suites and live Linux Mailpit-backed integration tests), not SUMMARY claims.

### Gaps Summary

No gaps. All 6 derived observable truths verified against source code, all artifacts exist/substantive/wired, both HARDEN-03 and HARDEN-05 requirement IDs are satisfied with independently re-run evidence (this verifier did not trust the prior SUMMARY.md or context-provided claims — it re-ran the Linux gate itself via Colima and captured true exit codes before any pipe, per the project's own documented "verification exit code through pipe" lesson). The only non-code item (REQUIREMENTS.md checkbox lag) is explicitly out of scope per phase instructions and is expected to be reconciled by `phase.complete`.

---

_Verified: 2026-07-17T18:00:00Z_
_Verifier: Claude (gsd-verifier)_
