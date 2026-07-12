# Phase 27 — Adversarial-Review Fix Catalog

Source: independent Fable-5 code-trace of the Phase-27 diff (double-confirmed by a second reviewer), all 4 findings re-verified against live source by the orchestrator. Core mechanisms (HARDEN-01 fstat compare, X-04/F3 shared cell, HARDEN-04 featureless sibling) traced CLEAN — these are hardening fixes, not a redo.

Fix ordering by blast radius: test-integrity/detection → design-pin/trust → audit-atomicity → doc.

---

## FIX 1 (from Finding 1, MAJOR — test-gate integrity)

**Problem:** `cli/caprun/tests/harden04_featureless_create_session.rs:184` skips (returns green) whenever the response is `BrokerResponse::SessionCreated` — which is the EXACT outcome a HARDEN-04 regression produces. So if the `#[cfg]` gate on the mint arm ever breaks (arm leaks into a featureless build), even the scoped `cargo test -p caprun --test harden04_featureless_create_session` run observes `SessionCreated` and SKIPS GREEN. The hard assertion at ~:205 is only reachable when the response is already `Error`. The test can never fail on the regression it exists to catch, and the standing `cargo test --workspace` path always skips → D-10 has zero ongoing protection. The in-file comments claiming "a genuine regression still fails loudly" are FALSE and must be corrected.

**Fix:**
1. Add to `crates/brokerd/src/lib.rs` (near the top, after the `pub mod` block):
   `pub const TEST_FIXTURES_ACTIVE: bool = cfg!(feature = "test-fixtures");`
   This const reflects whether THIS build graph actually compiled brokerd with `test-fixtures` (true under `cargo test --workspace` feature-unification; false under scoped `cargo test -p caprun`).
2. In the D-10 test, replace the `matches!(resp, SessionCreated)`-based skip with logic keyed on `brokerd::TEST_FIXTURES_ACTIVE`:
   - If `brokerd::TEST_FIXTURES_ACTIVE == true`: this graph legitimately has the mint arm, so a `SessionCreated` response is expected → skip (non-failing) with the existing explanatory eprintln. (A featureless-only assertion is not meaningful here.)
   - If `brokerd::TEST_FIXTURES_ACTIVE == false` (genuinely featureless graph): the response MUST be the fail-closed `Error` even with `CAPRUN_ENABLE_IPC_CREATE_SESSION=1` set. A `SessionCreated` here is a real D-10 regression → `panic!`/`assert!` HARD FAIL. This is the teeth.
   - Correct the false comments accordingly.
3. Add a gate to `scripts/check-invariants.sh`: assert `crates/brokerd/Cargo.toml` does NOT declare `test-fixtures` as (or within) a `default` feature — otherwise `TEST_FIXTURES_ACTIVE` would be true even in a shipped build and the skip would look legitimate while the arm ships. (Grep that there is no `default = [ ... test-fixtures ... ]` in brokerd's `[features]`.) Wire it as a real failing check with a clear message.

**Verify:** `cargo test -p caprun --test harden04_featureless_create_session` still passes (featureless graph → hard assertion runs, Error observed). `cargo test --workspace` still green (unified graph → const true → legitimate skip). `./scripts/check-invariants.sh` exits 0. Prove the teeth: temporarily break the cfg gate locally (mentally or via a scratch check) — the scoped run must now FAIL, not skip. (Do not commit the break.)

---

## FIX 2 (from Finding 2, MAJOR — design-pin deviation: freeze trusted identity at startup)

**Problem:** `crates/brokerd/src/server.rs:~1170` calls `std::fs::metadata(trusted_path)?` on EVERY `RequestFd` — an ambient, symlink-following, per-grant resolution. F2 (locked, `DESIGN-security-hardening.md` §a ~:130-134) pins the `<workspace-file>` identity as "resolved once … at broker startup." As written, a post-startup swap/symlink at `trusted_path` redefines "trusted" mid-session (under-demotion). Low practical exploitability (the Landlock-confined worker cannot alias a host path; legitimate swaps fail-closed → demote), but it is a deviation from locked design text in a design-gated milestone.

**Fix (freeze once at run_broker_server entry — matches the "seeded once at run_broker_server entry" precedent; keeps main.rs's call unchanged):**
1. In `crates/brokerd/src/server.rs` `run_broker_server` (keep receiving `trusted_path: PathBuf` from main.rs), at entry — co-located with the shared `session_status` cell construction (~:187) — compute the frozen identity ONCE:
   `let trusted_inode: Option<(u64, u64)> = std::fs::metadata(&trusted_path).ok().map(|m| (m.dev(), m.ino()));`
   (`use std::os::unix::fs::MetadataExt;` already present from Phase 27.) `None` = the file was unresolvable at startup → fail-closed (every grant demotes).
2. Change the threaded param type from `&std::path::Path` (`trusted_path`) to `Option<(u64, u64)>` (`trusted_inode`, `Copy`) on `handle_connection`, `classify_second_connection`, and `dispatch_request`. Thread the frozen `trusted_inode` inward instead of the path.
3. In the `RequestFd` arm (~server.rs:1168-1176), REPLACE the per-grant closure that calls `std::fs::metadata(trusted_path)` with a compare against the frozen pair:
   `let is_trusted_labeled = match trusted_inode { Some((d, i)) => file.metadata().map(|m| m.dev() == d && m.ino() == i).unwrap_or(false), None => false };`
   Keep the F2 ordering (open → compare → demote-if-untrusted → pass_fd) and the fail-closed default. Update the nearby comment to say the trusted identity is FROZEN at startup (no per-grant re-resolution, no symlink-follow window).
4. Update ALL dispatch_request call sites to pass the new `Option<(u64,u64)>` arg instead of a path:
   - Production: server.rs:514 (inside handle_connection's loop) — pass the threaded `trusted_inode`.
   - In-module test call sites (server.rs ~:1521/1558/1578/1629/1658) and the 6 external test files (durable_anchor.rs, extract_provenance_threading.rs, phase5_dispatch.rs ×2, proto_claims.rs ×2): pass `None` UNLESS the test exercises the trusted-stay-Active path.
   - In `crates/brokerd/tests/harden01_session_integrity.rs`: the stay-Active (SC2/Test C) test MUST pass `Some((dev, ino))` of its real trusted test file (stat it in the test) so the inode compare still matches; the untrusted/demotion tests pass a non-matching `Some(...)` or `None`. Re-verify all 3 tests still pass for the RIGHT reason (Test C stays Active because the inodes match; Tests A/B demote because they don't).

**Verify:** `cargo build --workspace` exits 0 (all call sites updated). `cargo test -p brokerd -- harden01_session_integrity` passes 3/3 on Linux (via `scripts/mailpit-verify.sh` if Colima up) or confirm the ungated logic on Mac. `grep -n 'std::fs::metadata(trusted_path)' crates/brokerd/src/server.rs` returns NOTHING (per-grant resolution gone). `./scripts/check-invariants.sh` exits 0.

---

## FIX 3 (from Finding 3, MINOR — single-lock atomicity for the demotion)

**Problem:** `crates/brokerd/src/server.rs:~1191-1237`: the fd-grant demotion writes `sessions.status = Draft` under one `conn.lock()` acquisition (~:1192-1197) and appends the `session_demoted` Event under a SEPARATE `conn.lock()` acquisition (~:1231-1237). §a pins "the SAME atomic pattern `mint_from_read` already uses" and `quarantine.rs`'s doc for that pattern says "never a second, separately-locked step." A panic/failure between the two acquisitions leaves `status = Draft` with no causal `session_demoted` Event (audit-DAG gap). Direction is fail-closed (the fd is never passed — the error aborts the connection), but the atomicity pin is violated.

**Fix:** Restructure so the `update_session_status(&locked, session_id, &Draft)` UPDATE and the `append_event(&locked, &demoted_event, Some(&fd_hash))` append run under a SINGLE `conn.lock()` acquisition (one `let locked = conn.lock()...` block spanning both), mirroring `mint_from_read` in `quarantine.rs`. The shared in-memory `session_status` cell write (X-04/F3) may stay as its own `session_status.lock()` (a different mutex) but should occur within the same logical demotion block. Do not change the event contents, parent-linking (`Some(fd_event_id)`), or the `"session_demoted"` event_type literal.

**Verify:** `cargo build --workspace` + `cargo test -p brokerd` green. Read the diff: the `update_session_status` call and the `append_event` call share one `conn.lock()` guard. `./scripts/check-invariants.sh` exits 0.

---

## FIX 4 (from Finding 4, NIT — doc scoping, no code change)

**Problem:** The D-02 amendment / new doc text overstates HARDEN-01's closure. Demote-at-grant fires ONLY for NON-designated inodes; a silent worker that `RequestFd`s the designated `<workspace-file>` itself and never sends `ReportClaims` still stays `Active` while holding that file's raw bytes — this is INTENTIONAL (SC2/CONTROL-01 requires the clean designated-file path to stay Active; I2 + `mint_from_read` backstop the claims path). But `crates/brokerd/src/quarantine.rs`'s new doc text and `planning-docs/DESIGN-session-trust-state.md` §2 phrase the closure as "a silent worker … kept the session falsely Active" without scoping it to NON-designated reads.

**Fix:** One-line doc scoping in both `quarantine.rs` (the amended comment) and `DESIGN-session-trust-state.md` §2: clarify that the fd-grant demotion closes the silent-worker gap FOR NON-DESIGNATED (untrusted-inode) fd grants; a read of the designated `<workspace-file>` itself intentionally stays Active (the CONTROL-01/SC2 clean path), with I2 + `mint_from_read` as the backstop on any claims derived from it. No code change.

**Verify:** `cargo build --workspace` still exits 0 (comment-only). The doc text no longer claims an unscoped closure.

---

## Commit discipline
Commit per fix (e.g. `test(27):`, `fix(27):`, `docs(27):`). Do NOT touch STATE.md or ROADMAP.md (orchestrator owns them). After all four, ensure `cargo build --workspace`, `cargo build --workspace --release`, `cargo test --workspace --no-fail-fast` (Mac: Linux tests "0 passed" = PASS), and `./scripts/check-invariants.sh` are all green. Never weaken/`#[ignore]` a test or soften a mechanism to pass; if a genuine blocker arises, STOP and report it.
