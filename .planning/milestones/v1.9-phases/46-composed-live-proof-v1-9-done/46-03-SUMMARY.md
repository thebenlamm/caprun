---
phase: 46-composed-live-proof-v1-9-done
plan: 03
subsystem: testing
tags: [live-acceptance, i2, policy, git-push, http-write, credential-custody, destination-pin, audit-dag]

# Dependency graph
requires:
  - phase: 46-composed-live-proof-v1-9-done (46-01)
    provides: the POST /ingest http-write mock endpoint (composed-run reachability)
  - phase: 43-http-write
    provides: s43 http.request.write differential (tainted-body I2 Block shape reused verbatim)
  - phase: 44-git-push
    provides: s44 git.push differential (tainted-remote Block, redirect-refusal, clean-push credential-absence reused)
  - phase: 42-policy-layer
    provides: policy_gate + DenyReason::PolicyDeny (code()=="policy_deny") for the distinct-tag leg
provides:
  - "crates/brokerd/tests/s46_negative_legs_composed.rs — the LIVE-06 composed negative-leg proof"
  - "Five independently-attributable negative legs over ONE shared persisted audit.db, each with a DISTINCT machine-checkable tag asserted separately"
  - "Credential-absence extended to the broker LOG on the ERROR path (W1 / DESIGN §1.4), captured non-vacuously via a subprocess"
affects: [46-04, v1.9-DONE-gate, milestone-verification]

# Tech tracking
tech-stack:
  added: [libc (brokerd dev-dependency — FD-redirect capture; pre-existing workspace dep via sandbox)]
  patterns:
    - "Composed multi-mechanism negative proof over one shared persisted audit.db, one session per leg, end-of-run verify_chain sweep"
    - "Subprocess + --nocapture to capture a broker eprintln! that libtest's output-capture would otherwise intercept before FD 2"

key-files:
  created:
    - crates/brokerd/tests/s46_negative_legs_composed.rs
  modified:
    - crates/brokerd/Cargo.toml
    - Cargo.lock

key-decisions:
  - "One composed #[tokio::test] fn gated #[cfg(all(target_os=\"linux\", feature=\"mock-egress-ca\"))] (union gate) — legs 1/4/5 exercise confined git children + the pinned mock socket; a host-portable guard keeps macOS meaningful."
  - "Leg 3 policy-deny built via a serde-deserialized SessionPolicy permitting git.push + http.request.write but omitting email.send — no new TCB, decision-level."
  - "Leg 5b broker-log capture runs the error-path push in a re-exec'd subprocess with --nocapture (libc dup2 → temp file → read INSIDE it), because libtest's output-capture propagates to spawned threads and intercepts eprintln! before FD 2 — an in-process dup2 is vacuous (proven empirically)."

patterns-established:
  - "Distinct-tag separation: sink_blocked (I2) vs code()==\"policy_deny\" asserted in one side-by-side block so mechanisms are provably distinct (POLICY-02)."
  - "Non-vacuous broker-log absence: the error path (redirect-refused) where scrub_secrets→eprintln! actually fires, never the clean 200 push (Ok arm emits no log)."

requirements-completed: [LIVE-06]

coverage:
  - id: D1
    description: "Leg 1 — genuinely-tainted git.push remote I2-Blocks under a git.push-PERMITTING policy, emitting a sink_blocked event anchored on the tainted arg"
    requirement: "LIVE-06"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s46_negative_legs_composed.rs#composed::s46_negative_legs_composed_all_legs (LEG 1)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Leg 2 — genuinely-tainted http.request.write body I2-Blocks under a write-PERMITTING policy; sink_blocked emitted, no http_write_* terminal (never writes)"
    requirement: "LIVE-06"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s46_negative_legs_composed.rs#composed::s46_negative_legs_composed_all_legs (LEG 2)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Leg 3 — policy-deny of an omitted sink yields Denied{PolicyDeny} code()==\"policy_deny\" + generic plan_node_evaluated, NO sink_blocked; the two tags asserted SEPARATELY from the I2 legs"
    requirement: "LIVE-06"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s46_negative_legs_composed.rs#composed::s46_negative_legs_composed_all_legs (LEG 3 + distinct-tag block)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Leg 4 — a /redirect/* push is refused by the frozen redirect-none client (never followed) → ConfirmedButSinkFailed + one git_push_failed; the destination pin holds"
    requirement: "LIVE-06"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s46_negative_legs_composed.rs#composed::s46_negative_legs_composed_all_legs (LEG 4)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Leg 5a — after a REAL clean confirmed 200 push, the sentinel token + remote URL are absent from every event payload AND actor column"
    requirement: "LIVE-06"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s46_negative_legs_composed.rs#composed::s46_negative_legs_composed_all_legs (LEG 5a)"
        status: pass
    human_judgment: false
  - id: D6
    description: "Leg 5b — on the ERROR-PATH push (redirect-refused, where scrub_secrets→eprintln! fires), the captured broker stderr contains NEITHER the token NOR the raw remote host/URL, with the eprintln marker present (non-vacuous)"
    requirement: "LIVE-06"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s46_negative_legs_composed.rs#composed::leg5b_error_path_push_worker (driven by all_legs)"
        status: pass
    human_judgment: false

# Metrics
duration: ~65min
completed: 2026-07-18
status: complete
---

# Phase 46 Plan 03: Five LIVE-06 Negative Legs (Composed) Summary

**Five independently-attributable negative legs — two I2 Blocks under a policy that PERMITS the sink, a distinct policy-deny (`code()=="policy_deny"`, no `sink_blocked`), a destination-pin redirect refusal, and credential-absence across value store / audit chain / broker log — proven in ONE composed run over a shared persisted `audit.db` on real Linux.**

## Performance

- **Duration:** ~65 min
- **Tasks:** 3
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments
- **LIVE-06 negative clause proven** on real Linux via `scripts/compose-verify.sh --features brokerd/mock-egress-ca`: all 5 legs green, feature-OFF guard passed, final `verify_chain` sweep asserts EXACTLY the 5 negative-leg sessions.
- **Distinct machine-checkable tags asserted separately** (POLICY-02): the I2 legs (tainted remote, tainted body) emit `sink_blocked` while running a sink+arg the policy EXPLICITLY PERMITS; the policy-deny leg emits `code()=="policy_deny"` + generic `plan_node_evaluated` with NO `sink_blocked` — proving policy narrows WHICH sinks are callable but never disables I2.
- **Destination pin proven to hold live:** a `/redirect/*` push is refused (never followed) → `ConfirmedButSinkFailed` + exactly one terminal `git_push_failed`, `confirm_granted` released to Step-7 first.
- **Credential absence extended to the broker LOG** (W1 / DESIGN §1.4): captured non-vacuously on the ERROR path where `scrub_secrets`→`eprintln!` (git_push.rs:784) actually fires — the token and raw remote host/URL are absent from the captured stderr.

## Task Commits

1. **Task 1: Scaffold — shared persisted audit.db, host guard** - `443a484` (test)
2. **Task 2: Legs 1-3 — two I2 Blocks + distinct policy-deny** - `4736e47` (test)
3. **Task 3: Legs 4-5 — pin refusal + credential absence + sweep** - `2ce7966` (test)

## Files Created/Modified
- `crates/brokerd/tests/s46_negative_legs_composed.rs` - The composed LIVE-06 negative-leg proof (5 legs, one shared persisted audit.db, host guard, subprocess leg-5b worker).
- `crates/brokerd/Cargo.toml` - Added `libc` to `[dev-dependencies]` (FD-redirect capture; pre-existing workspace dep via sandbox — HYG-01 unaffected).
- `Cargo.lock` - `libc` added to the brokerd dependency list (only change).

## Reused-verbatim vs newly-paired (per plan `<output>`)

**Reused verbatim (shipped s43/s44 assertion shapes):**
- Leg 1 tainted-remote I2 Block anchor shape — from s44 `legs_b_and_c` / `assert_block_on_arg` (anchor names arg, `read_event_id == provenance_chain[0]`).
- Leg 2 tainted-body Block + no-`http_write_*` — from s43 `dispatch_tainted_body_blocks_and_never_writes`.
- Leg 4 redirect refusal (`ConfirmedButSinkFailed`, one `git_push_failed`, `confirm_granted`) — from s44 `leg_e_receive_pack_redirect_is_refused`.
- Leg 5a value-store + audit-chain credential absence — from s44 `leg_c_clean_confirmed_push` (payload + actor scan).
- Shared persisted audit.db + `seed_test_key` + `all_session_ids` ORDER-BY-rowid sweep — from `live_acceptance_v1_8_composed.rs`.

**Newly paired (the plan's genuinely new work):**
- **Policy-deny distinctness (Leg 3, G2):** a serde-built `SessionPolicy` permitting git.push + http.request.write but omitting email.send → `Denied{PolicyDeny}` recorded as `plan_node_evaluated`, asserted SEPARATELY from the I2 legs' `sink_blocked` in a dedicated side-by-side tag block. Decision-level, no new TCB.
- **Broker-log credential absence (Leg 5b, G4):** the error-path push's scrubbed `eprintln!` captured and asserted clean of the token + raw remote host/URL.

## Broker-log clause: WHICH push proved it (for 46-04 honest framing)

Leg 5b's broker-log absence was proven on the **error-path push (the LEG-4 redirect-refused push)** — the ONLY push where `scrub_secrets`→`eprintln!` at git_push.rs:784 fires (the clean 200 push takes the `Ok` arm and emits NO log, so a log check on it is VACUOUS). The captured stderr contained the `"[brokerd] git.push failed"` marker (NON-VACUOUS proof the logger ran) and NEITHER `TOKEN_SENTINEL` NOR `github-mock.caprun.test` / the redirect URL.

## Decisions Made
- **Union cfg gate for the composed fn.** The single sequential `#[tokio::test]` is gated `#[cfg(all(target_os="linux", feature="mock-egress-ca"))]`. Legs 2 & 3 are host-portable in isolation (already proven in s43 / the policy_gate unit tests), but the composed single-fn run must be gated at the union because legs 1/4/5 use the confined git children (block-time oid freeze) + the pinned mock git-receive-pack socket. A host-portable guard keeps `cargo test -p brokerd` meaningful on macOS (0 run — expected).
- **Leg 3 uses a serde-deserialized policy** (the trusted-JSON binder shape) rather than a new constructor — `SessionPolicy` fields are private and the type derives Deserialize, so no runtime-core change was needed.

## Deviations from Plan

### Auto-fixed / mechanism deviations

**1. [Rule 1 — Bug] Leg 5b broker-log capture: subprocess + `--nocapture`, not an in-process `dup2`**
- **Found during:** Task 3 (first two compose-verify runs FAILED at leg 5b with `captured=<>`).
- **Issue:** The plan specified capturing the broker `eprintln!` via an in-process `libc dup/dup2 → temp file`. Empirically (verified with a standalone probe) Rust's libtest output-capture is active under the default `cargo test` invocation compose-verify uses, and it **propagates to spawned threads**, intercepting `eprintln!` BEFORE it reaches FD 2 — so ONLY a direct `libc::write(2,..)` reaches a dup2'd FD. The broker logs via `eprintln!`, so an in-process (or spawned-thread) dup2 captured NOTHING — a vacuous assertion, the exact false-assurance trap the plan warns against ([[false-assurance-regression-test]]).
- **Fix:** Run the error-path push in a re-exec'd SUBPROCESS (`std::env::current_exe()` with `--exact composed::leg5b_error_path_push_worker --nocapture`). A fresh `--nocapture` process has NO output-capture, so its `eprintln!` reaches its real FD 2 — which the worker `dup2`'s onto the temp log file (the specified `libc dup/dup2 → temp file → read` mechanism, now non-vacuous). The worker runs the SAME `evaluate_and_confirm` against the SAME shared `audit.db` (its redirect session persists → counted in the sweep) and emits `S46_WORKER_SESSION`/`S46_WORKER_OUTCOME` markers on FD 1 for the parent. The parent reads the temp file for the log assertions and the shared DB for the leg-4 pin assertions.
- **Files modified:** crates/brokerd/tests/s46_negative_legs_composed.rs
- **Verification:** compose-verify run #3 — `leg5b` marker `"git.push failed"` present, token + host absent; all 5 legs pass.
- **Committed in:** `2ce7966`

**2. [Rule 3 — Blocking] Session-scoped `pending_confirmations` / token-hit queries**
- **Found during:** Task 3.
- **Issue:** The s44 helpers query `pending_confirmations` / audit hits with no session filter (safe for their single-session `:memory:` db). On the SHARED persisted db, leg 1's tainted-block leaves a pending-confirmation row, so an unqualified `SELECT` is ambiguous.
- **Fix:** Added `WHERE session_id = ?` to the `evaluate_and_confirm` effect-id lookup and the leg-5a token-hit scan.
- **Files modified:** crates/brokerd/tests/s46_negative_legs_composed.rs
- **Verification:** compose-verify #3 green; final sweep asserts exactly 5 sessions.
- **Committed in:** `443a484` / `2ce7966`

---

**Total deviations:** 2 (1 bug-class mechanism fix, 1 blocking shared-db scoping). No scope creep — the leg set, tags, and assertions match the plan.

## Issues Encountered
- Two compose-verify runs failed at leg 5b (empty capture) before the subprocess mechanism was adopted; root-caused via a standalone FD-2-vs-libtest-capture probe. Resolved (deviation 1).

## check-invariants
`./scripts/check-invariants.sh` — **all gates PASS**, including Gate 3 (no new mint site; tests/ exempt), Gate 4b (mock-egress-ca never default), and Gate 5 (aws-lc-rs + openssl-sys absent from the workspace build graph — HYG-01 holds; libc is not C-crypto). No new `EffectRequest` (Gate 1), no new crate (Gate 5 — libc is a pre-existing workspace dev-dep).

## Test results
- **Authoritative (Linux, compose-verify):** `s46_negative_legs_composed` — **3 passed / 0 failed** (guard + composed all_legs + leg5b worker no-op); feature-OFF guard passed; "Composed Linux verification suite PASSED".
- **Host (macOS) full brokerd suite:** green — s46 host guard passes (composed body 0-run, expected); s43 (5), s44 (3) and all sibling targets unaffected.

## Next Phase Readiness
- LIVE-06's negative-leg clause is satisfied. Full-workspace regression + the milestone DONE record are 46-04's scope. 46-04 should frame the broker-log clause honestly as proven on the error-path (redirect-refused) push, per the note above.

## Self-Check: PASSED
- `crates/brokerd/tests/s46_negative_legs_composed.rs` — FOUND
- `.planning/phases/46-composed-live-proof-v1-9-done/46-03-SUMMARY.md` — FOUND
- Commits `443a484`, `4736e47`, `2ce7966` — all FOUND in git log

---
*Phase: 46-composed-live-proof-v1-9-done*
*Completed: 2026-07-18*
