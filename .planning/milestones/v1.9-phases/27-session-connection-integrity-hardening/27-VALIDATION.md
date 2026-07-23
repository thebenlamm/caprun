---
phase: 27
slug: session-connection-integrity-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-12
---

# Phase 27 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `cargo test` (workspace) + Cargo integration-test binaries under each crate's `tests/` dir |
| **Config file** | none — Cargo.toml `[dependencies]`/`[dev-dependencies]`/`[features]`; no separate test-runner config |
| **Quick run command** | `cargo build --workspace` (catches signature-mismatch compile errors across all `dispatch_request` call sites) then `cargo test -p brokerd --no-fail-fast` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` (macOS — Linux-gated tests no-op); `bash scripts/mailpit-verify.sh` (Linux live proof) |
| **Estimated runtime** | ~60–120 seconds (macOS build+test); Linux container run longer |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --workspace` (fast cross-call-site compile check) + `cargo test -p brokerd --no-fail-fast`
- **After every plan wave:** Run `cargo test --workspace --no-fail-fast`
- **Before `/gsd-verify-work`:** Full suite must be green on macOS; Linux `mailpit-verify.sh` strongly recommended before marking Phase 27 done
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 27-01-* | 01 | 1 | HARDEN-01 | I1 (instruction injection) | `RequestFd` on an untrusted (non-`workspace_rel`) path with no subsequent `ReportClaims` demotes the session to `Draft` at fd-grant time | unit/integration (Linux-gated) | `cargo test -p brokerd --no-fail-fast -- request_fd_demotes` | ❌ W0 | ⬜ pending |
| 27-01-* | 01 | 1 | HARDEN-01 | I1 | Fragment-free clean read on the trusted `workspace_rel` path (fstat inode match) stays `Active`; CONTROL-01 clean-send still completes ungated | integration (Linux, Mailpit) | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3' bash scripts/mailpit-verify.sh` | ✅ (regression) | ⬜ pending |
| 27-01-* | 01 | 1 | HARDEN-01 (X-04 fold) | I0 (draft-only seed) | A Planner connection accepted after a Worker-connection demotion observes `Draft`, not a stale `Active` snapshot (shared `Arc<Mutex<SessionStatus>>`, monotonic Active→Draft) | unit/integration | `cargo test -p brokerd --no-fail-fast -- planner_sees_demotion` | ❌ W0 | ⬜ pending |
| 27-02-* | 02 | 1 | HARDEN-04 | forced-Active mint | Featureless build: `CreateSession` over the socket always returns fail-closed `Error` — no runtime opt-in exists (D-10 primary gate) | behavioral negative (built without `test-fixtures`) | featureless build + `cargo test -p brokerd --no-fail-fast -- create_session_featureless` (exact cfg-exclusion invocation confirmed empirically per A4) | ❌ W0 | ⬜ pending |
| 27-02-* | 02 | 1 | HARDEN-04 | forced-Active mint | Existing 3 `uds_ipc.rs` tests (`server_accept`, `create_session_round_trip`, `create_session_over_ipc_denied_by_default_when_flag_unset`) still exercise the forced-Active arm under the new `test-fixtures` feature | regression | `cargo test -p brokerd --features test-fixtures --no-fail-fast -- uds_ipc` | ✅ (re-verify empirically) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Task IDs are placeholders (`27-PP-TT`) — the planner assigns concrete IDs; the mapping is by requirement + behavior.*

---

## Wave 0 Requirements

- [ ] New test — fd-grant-time demotion on an untrusted (non-`workspace_rel`) path with no `ReportClaims`, asserting `Draft` (HARDEN-01 primary behavior).
- [ ] New test — Planner connection accepted after a Worker-connection demotion observes `Draft` (X-04/F3 fold: shared `Arc<Mutex<SessionStatus>>`, monotonic).
- [ ] New test — featureless-build behavioral negative gate for `CreateSession` (D-10, HARDEN-04 primary proof).
- [ ] Verify empirically (do NOT assume) whether the existing 3 `uds_ipc.rs` tests need `features = ["test-fixtures"]` in their effective build to keep passing for the right reason (A4) — Cargo's default dev-dependency feature unification may already cover it (as it does for `executor`).

*Framework is already installed (Rust/Cargo). No new test harness — only new test cases in existing/new Cargo test targets.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Featureless **release** binary physically lacks the forced-Active mint arm | HARDEN-04 | Phase-30 owns the formal milestone proof; the definitive check is a `cargo build --workspace --release` (default features) followed by grep/symbol absence — distinct from `cargo test`, which unifies dev-dep features and would re-include the arm | Build featureless release, then grep the arm's marker string / disassemble; assert absent. Recorded here so Phase 30 doesn't re-derive it. |

*All Phase-27 in-scope behaviors have automated verification; the release-binary absence proof is deferred to Phase 30 by design (§j).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
