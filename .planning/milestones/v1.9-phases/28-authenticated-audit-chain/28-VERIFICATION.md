---
phase: 28-authenticated-audit-chain
verified: 2026-07-13T00:00:00Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 28: Authenticated Audit Chain Verification Report

**Phase Goal:** `verify_chain` becomes an authenticated-integrity check rather than a corruption detector — an actor with `events`-table write access can no longer produce a chain that `verify_chain` accepts.
**Verified:** 2026-07-13
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth (ROADMAP Success Criteria) | Status | Evidence |
|---|---|---|---|
| 1 | `verify_chain` rejects a chain where an event row has been rewritten and every descendant hash/parent_hash recomputed to be internally self-consistent — the exact forgery that previously passed. | ✓ VERIFIED | `crates/brokerd/src/audit.rs::compute_event_hash`/`verify_event_hash` are keyed HMAC-SHA256 (domain-separated + length-framed via `mac_frame`), not unkeyed SHA-256. Test `audit::tests::self_consistent_forgery_without_key_is_rejected` builds a fully self-consistent forged chain under the **public unkeyed** algorithm (the only one an events-table-only attacker can reproduce) and asserts `verify_chain` rejects it. Independently re-run by this verifier: `cargo test -p brokerd --lib self_consistent_forgery_without_key_is_rejected` → **1 passed**. |
| 2 | The chain's authenticity depends on a secret key or an out-of-store anchor that a bare `events`-table writer cannot derive or reproduce. | ✓ VERIFIED | Key is a `getrandom`-CSPRNG-generated 32-byte secret in `<audit_path>.key`, loaded by `cli/caprun/src/key.rs::load_or_create_key`, held **outside** the SQLite file. Test `audit::tests::verify_chain_is_key_dependent` proves two different keys over identical fields yield different MACs and `verify_chain` under the wrong key returns `false` on an untampered chain. Independently re-run: `cargo test -p brokerd --lib verify_chain_is_key_dependent` → **1 passed**. F1 fail-closed startup refusal (key/audit-DB path beneath workspace root → hard error before any key is generated/returned) independently re-run: `cargo test -p caprun --bin caprun f1_refusal_when_audit_under_workspace_root` → **1 passed**; wired into `cli/caprun/src/main.rs:230-231`, BEFORE the broker task is spawned (main.rs:281), so the refusal is broker-startup-enforced, not operator convention. |
| 3 | An untampered chain continues to verify true — no false positives; existing confirm-path and live-acceptance callers of `verify_chain` are unaffected. | ✓ VERIFIED | `verify_chain`'s own doc-cited sanity assertions (`self_consistent_forgery_without_key_is_rejected`, `tail_truncation_detected_via_anchor_mismatch`, `legacy_db_without_anchor_fails_closed`) each assert the untampered chain verifies `true` BEFORE the tamper is applied. `key_file_reused_across_calls` proves cross-process key stability (two independent loads → identical bytes), the load-bearing property for `confirm()`'s separate-OS-process `verify_chain` gate. Both production `verify_chain` callers (`confirmation.rs::confirm`/`deny`, `main.rs`'s end-of-run assertion) use the SAME threaded key. Evidence provided (not re-run by this verifier, full-suite): macOS `cargo test --workspace` 283 passed/0 failed; Linux `scripts/mailpit-verify.sh` 322 passed/0 failed — both include `s9_live_block`, `durable_anchor`, `confirm`, `confinement_integration` targets. |

**Score:** 3/3 truths verified (0 present-but-behavior-unverified)

### Extended coverage beyond the literal SC wording (§b DESIGN pins, independently confirmed)

| Item | Status | Evidence |
|---|---|---|
| Tail-truncation (DELETE-the-tail, D-04) detected | ✓ VERIFIED | `chain_anchor(session_id, head_event_id, head_hash, event_count, mac)` upserted atomically inside `append_event` under the same connection lock as the events INSERT (`audit.rs:605-633`); `verify_chain` cross-checks it. Independently re-run: `tail_truncation_detected_via_anchor_mismatch` → **1 passed**. |
| Legacy/un-anchored DB fails closed | ✓ VERIFIED | `legacy_db_without_anchor_fails_closed` → **1 passed**. Absent anchor row → `verify_chain` returns `false`, never silently trusted. |
| `pending_confirmations` folded into the MAC scheme (X-02) | ✓ VERIFIED | Whole-row MAC (`mac` column, domain `caprun.audit.pending-confirmation.v1`) computed by `insert_pending_confirmation`, recomputed atomically with `state` by `transition_state` in ONE `UPDATE`. `confirm()`/`deny()` both call `verify_pending_confirmation_mac` immediately after `find_pending_confirmation`, BEFORE the terminal-state branch reads `pc.state` (closes the flip-back window). Independently re-run: `flip_back_denied_to_pending_caught_by_mac` (raw-SQL Denied→Pending flip caught by MAC even though the `state` column literally reads `"pending"`) → **1 passed**. |
| `deny()` gains the SAME integrity gate `confirm()` has (previously had none) | ✓ VERIFIED | `crates/brokerd/src/confirmation.rs::deny` (lines 933-985) now runs the pending_confirmations MAC gate (Step 1.5) AND `verify_chain` (Step 2.5) before any state transition or event append. Independently re-run: `deny_fails_closed_on_tampered_state` → **1 passed**. |
| Constant-time MAC comparison (never `==`/`!=` on hex) | ✓ VERIFIED | `verify_event_hash`, `verify_anchor_mac`, `verify_pending_confirmation_mac` all use `Mac::verify_slice`, confirmed by direct code read. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/brokerd/src/audit.rs` | keyed `compute_event_hash`/`verify_event_hash`, `chain_anchor` table + upsert/verify, `mac_frame` shared helper | ✓ VERIFIED | Present, substantive, wired into all 19 `append_event` call sites (grep-confirmed across `server.rs`/`quarantine.rs`) and both `verify_chain` callers. |
| `crates/brokerd/src/confirmation.rs` | `pending_confirmations` MAC fold, `confirm()`/`deny()` entry gates | ✓ VERIFIED | `build_pending_confirmation_mac`/`verify_pending_confirmation_mac`/`transition_state` present; both `confirm` and `deny` gate at entry. |
| `cli/caprun/src/key.rs` | `load_or_create_key` + F1 fail-closed refusal | ✓ VERIFIED | Present; getrandom-backed, 0600, idempotent, F1 containment check runs before any key is generated/returned. |
| `cli/caprun/src/main.rs` | key loaded before broker spawn (run path) and in `run_confirm_or_deny` (confirm/deny path) | ✓ VERIFIED | `key::load_or_create_key` called at `main.rs:230` (before `broker_task` spawn at `:281`) and at `:444`/`:465` for confirm/deny. |

### Key Link Verification

| From | To | Via | Status |
|---|---|---|---|
| `main.rs` run path | `key::load_or_create_key` | called before `run_broker_server` spawns | ✓ WIRED |
| `run_broker_server`/`server.rs` | `audit::append_event`/`compute_event_hash` | `key: Arc<[u8; 32]>` threaded through signatures (`server.rs:179,367,488,654,1026,1150`) | ✓ WIRED |
| `confirmation::confirm`/`deny` | `audit::verify_chain` + `verify_pending_confirmation_mac` | both gates run at function entry before any state read/mutation | ✓ WIRED |
| `run_confirm_or_deny` | `key::load_or_create_key` | key loaded from `pc.workspace_root_path` before gates run | ✓ WIRED |

### Behavioral Spot-Checks (independently re-run by this verifier, single named tests)

| Behavior | Command | Result | Status |
|---|---|---|---|
| Self-consistent forgery without key rejected | `cargo test -p brokerd --lib self_consistent_forgery_without_key_is_rejected` | 1 passed | ✓ PASS |
| Key-dependence (wrong key → false, no false positive) | `cargo test -p brokerd --lib verify_chain_is_key_dependent` | 1 passed | ✓ PASS |
| Tail-truncation detected via anchor mismatch | `cargo test -p brokerd --lib tail_truncation_detected_via_anchor_mismatch` | 1 passed | ✓ PASS |
| Legacy un-anchored DB fails closed | `cargo test -p brokerd --lib legacy_db_without_anchor_fails_closed` | 1 passed | ✓ PASS |
| Flip-back Denied→Pending caught by MAC | `cargo test -p brokerd --lib flip_back_denied_to_pending_caught_by_mac` | 1 passed | ✓ PASS |
| `deny()` fails closed on tampered state | `cargo test -p brokerd --lib deny_fails_closed_on_tampered_state` | 1 passed | ✓ PASS |
| F1 refusal (audit DB under workspace root) | `cargo test -p caprun --bin caprun f1_refusal_when_audit_under_workspace_root` | 1 passed | ✓ PASS |
| Cross-process key stability | `cargo test -p caprun --bin caprun key_file_reused_across_calls` | 1 passed | ✓ PASS |
| `./scripts/check-invariants.sh` | 4/4 gates | PASS | ✓ PASS |

Full-workspace run NOT repeated here (already independently green per phase evidence: macOS 283/0, Linux via `mailpit-verify.sh` 322/0) — targeted named-test re-runs above are the fresh, non-SUMMARY-trusting proof for this verification.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| HARDEN-02 | 28-01..28-05 | Authenticated audit chain (keyed MAC + anchored head), tamper/truncation/flip-back detected, fail-closed | ✓ SATISFIED | All 3 ROADMAP success criteria + the extended §b DESIGN pins independently verified above. |

No orphaned requirements — HARDEN-02 is the sole requirement mapped to Phase 28.

### Anti-Patterns Found

None. `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` grep across all 4 modified/created files (`audit.rs`, `confirmation.rs`, `key.rs`, `main.rs`) returned zero matches.

### Documentation Reconciliation Gap (non-blocking, process-only)

`.planning/ROADMAP.md` Phase 28 still shows "Plans: 4/5 plans executed" and the 28-05 checkbox unchecked; `.planning/REQUIREMENTS.md` HARDEN-02 row still reads "Pending". This is **expected and intentional per 28-05-SUMMARY.md's own note**: the last plan's executor deliberately does not flip ROADMAP/REQUIREMENTS, leaving that reconciliation to the orchestrator after the phase-level Linux gate passes (documented project precedent from v1.5 Phase 25 — "record sign-off before last plan" gotcha). The Linux gate has now passed (322/0, per phase evidence) — ROADMAP.md and REQUIREMENTS.md should be updated to reflect Phase 28 complete / HARDEN-02 satisfied as part of closing this phase. This is a bookkeeping action, not a code gap — all code-level truths are independently verified above.

### Human Verification Required

None. Every must-have truth was either directly code-traced or independently re-run as a named test by this verifier (not merely accepted from SUMMARY.md).

### Gaps Summary

No gaps. All 3 ROADMAP Success Criteria for Phase 28, plus the extended DESIGN §b mechanisms (tail-truncation detection, `pending_confirmations` fold, `deny()` gate parity, F1 broker-enforced startup refusal, constant-time comparison), are implemented, wired, and independently proven via fresh test execution — not SUMMARY.md narrative alone.

### Independent Adversarial Code-Trace (orchestrator-run, fresh context)

Per project precedent (Phase 27: an independent trace caught 4 real issues a green
verifier missed), a fresh-context hostile reviewer (Fable-5) traced the full diff +
callers/tests. Verdict: **no BLOCKERs, no MAJORs.** All 7 pre-flagged traps confirmed
handled against real code (verify_chain fail-closed on every non-happy path; domain-
separated + length-framed MAC via `mac_frame` with 3 distinct tags; anchor binds a
read-back monotonic count; `taint` preserved in MAC input; F1 canonicalizes + covers
the key path; whole-row `pending_confirmations` MAC; **tamper tests have real teeth** —
each fails if its guard is removed, so the Phase-27 false-assurance pattern is absent).

### Accepted Residuals (HARDEN-02)

Three MINOR residuals surfaced by the trace; none block completion (core guarantee —
an actor with events-table write access cannot get content into the chain
`verify_chain` accepts — holds).

- **(a) Snapshot-replay of a genuine old `(events-prefix, anchor)` pair at count k.**
  Count-binding defeats truncate-and-keep-anchor, but restoring a validly-MAC'd older
  anchor together with its matching prefix verifies true. Closing this needs an external
  monotonic root of trust (TPM/hardware counter) beyond a per-session keyed head — out
  of HARDEN-02 scope. **Accepted; candidate for a future hardware-anchored phase.**
- **(b) Orphan/unreferenced-row injection — FIXED (commit `49aed6d`).** `verify_chain`
  now cross-checks `walked_count` against live `COUNT(*)` per session; regression test
  `orphan_event_injection_detected_via_live_count` added. Proven no false-positive on
  single/multi-session chains (Linux gate 323/0).
- **(c) `append_event`'s event-INSERT + anchor-UPSERT are two autocommits under one
  held mutex, not one SQLite transaction.** The doc comment's "atomically" is w.r.t. the
  mutex (no interleaving), not a transaction. A crash between the two writes leaves the
  anchor stale → next `verify_chain` fails **closed** (never open). A proper inner
  `conn.transaction()` is unsafe here: `append_event` takes `&Connection` and is already
  called inside the email.send CAS path's outer `conn.transaction()`, so an inner
  transaction would attempt an unsupported nested transaction and break that caller.
  **Accepted (fail-closed; fixing it correctly is a larger refactor with no security gain).**

---

_Verified: 2026-07-13_
_Verifier: Claude (gsd-verifier) + independent adversarial trace (Fable-5, orchestrator-run)_
