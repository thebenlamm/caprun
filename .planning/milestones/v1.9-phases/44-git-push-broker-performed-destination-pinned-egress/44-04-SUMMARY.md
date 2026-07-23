---
phase: 44-git-push-broker-performed-destination-pinned-egress
plan: 04
subsystem: security
tags: [git-push, confirm-release, anti-toctou, taint, audit-dag, brokerd, wg-7, wg-8]

# Dependency graph
requires:
  - phase: 44-01
    provides: git.push executor registration (schema + routing-sensitive remote/refspec + CommitIrreversible class)
  - phase: 44-02
    provides: pkt-line/report-status parsers, validate_git_refspec, structural command-list, WG-1 frozen-IP pin
  - phase: 44-03
    provides: WG-2 confined pack-gen + rev-parse, broker-env credential, host allowlist, scrub_secrets, invoke_git_push_from_resolved (confirm-release-only), WG-7 assert_frozen_oid
provides:
  - git.push always-confirm-gate in server.rs (clean Allowed -> BlockedPendingConfirmation; NO auto-dispatch)
  - frozen_new_oid snapshot field (WG-7) persisted + whole-row MAC-covered + migrated
  - confirm() Step-4.75 guard + Step-4.8d prepare_git_push precheck + Step-7 dispatch arm (P33/P34 in sync)
  - WG-8 confirm-prompt payload-provenance renderer with control-char neutralization (T-44-19)
affects: [44-05, phase-46 compose-verify, v1.9 milestone close]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Policy re-gate: rewrite an Allowed decision into a synthetic BlockedPendingConfirmation BEFORE the generic block machinery, so clean-Allowed and I2-Block converge on ONE frozen-oid insert"
    - "Whole-row MAC extension: append a new field as the LAST length-framed mac element (legacy rows fail closed)"

key-files:
  created: []
  modified:
    - crates/brokerd/src/server.rs (always-confirm-gate rewrite + I2-block freeze + Linux tests)
    - crates/brokerd/src/confirmation.rs (frozen_new_oid field, MAC, guard/precheck/dispatch, WG-8 renderer, tests)
    - crates/brokerd/src/audit.rs (pending_confirmations frozen_new_oid column + migration)
    - crates/brokerd/src/sinks/git_push.rs (prepare_git_push, freeze_new_oid, resolve_new_oid stdout-only fix, tests)

key-decisions:
  - "git.push is ALWAYS confirm-gated: there is NO bare Allowed->auto-dispatch arm. A clean/untainted git.push freezes its new-oid, assembles a MAC'd pending confirmation, and returns BlockedPendingConfirmation. The tainted-remote/refspec path I2-Blocks onto the SAME frozen-oid confirm gate."
  - "Both git.push paths converge by rewriting an Allowed git.push into a synthetic BlockedPendingConfirmation (anchors = the routing args) BEFORE the shared block machinery — so the existing freeze/sink_blocked/blocked_literals/pending-insert path handles both uniformly (no bespoke assembly)."
  - "frozen_new_oid rides the existing whole-row MAC (appended as the last framed element); a tampered oid or a forged git.push pending row fails verification."
  - "WG-8 renderer is a PURE function over the frozen snapshot (frozen_new_oid + resolved_args) — no network AND no git read at confirm; it surfaces the frozen commit oid + per-routing-arg taint provenance, honoring the DESIGN §1.6 no-byte-identity accepted residual."

patterns-established:
  - "Control-char neutralization (T-44-19): tainted literals in the git.push confirm prompt are escaped to visible \\xNN before display, in BOTH the new payload summary AND the existing verbatim per-arg line — git.push only, preserving T-10-04 verbatim for all other sinks."
status: complete
---

# Phase 44 Plan 04: git.push Broker Control-Flow Wiring (dispatch + confirm-release) Summary

Wired `git.push` into the broker as an ALWAYS-confirm-gated sink: even a clean/untainted push freezes its new-oid, inserts a MAC'd pending confirmation, and returns `BlockedPendingConfirmation` (never auto-dispatched); `confirm()` gains the entry-guard + a fail-closed-recoverable `prepare_git_push` precheck + the Step-7 transfer dispatch threading the frozen oid; and the confirm prompt surfaces the frozen commit + a control-char-neutralized per-arg taint-provenance summary.

## What was built (per task)

- **Task 1 — confirm-release wiring** (`git_push.rs`, `confirmation.rs`, `audit.rs`): `prepare_git_push` (socket-free shared precheck) + `freeze_new_oid` (single WG-7 freeze entry point). `PendingConfirmation.frozen_new_oid` field — schema column + idempotent migration + insert/find accessors + framed as the last whole-row MAC element. `confirm()`: `git.push` added to the Step-4.75 entry-guard AND a Step-4.8d `prepare_git_push` precheck (pre-burn, fail-closed-RECOVERABLE) AND the Step-7 dispatch arm calling `invoke_git_push_from_resolved` with the frozen oid — guard/precheck/dispatch kept in sync, drift comment updated.
- **Task 2 — server.rs always-confirm-gate**: an `Allowed` git.push is rewritten into a synthetic `BlockedPendingConfirmation` (anchors = the routing args) BEFORE the generic block machinery, which then freezes the oid, appends `sink_blocked` + `blocked_literals`, and inserts the pending confirmation UNIFORMLY. Clean-Allowed and tainted-I2-Block converge on ONE frozen-oid insert. No git.push code path opens a socket without a human confirm.
- **Task 3 — WG-8 renderer**: `render_block_display` gains a git.push branch (frozen commit oid + per-routing-arg taint flag + §1.6 no-byte-identity note), plus `neutralize_control_chars` applied to the new summary AND the existing verbatim per-arg literal (git.push only) — the T-44-19 ANSI/audit-line-spoofing defense.

## THE #1 correctness pin (explicitly confirmed)

There is **NO auto-dispatch arm for git.push**. A clean, untainted (Allowed) git.push does NOT open a socket — it freezes the new-oid, assembles a MAC'd pending confirmation, and returns `BlockedPendingConfirmation`. Verified on Linux: `clean_git_push_is_confirm_gated_never_auto_dispatched` asserts the decision is `BlockedPendingConfirmation`, the pending row carries a non-empty `frozen_new_oid` + a valid whole-row MAC, a synthetic `sink_blocked` event exists, and NO `git_push_succeeded`/`git_push_failed` event is appended. `invoke_git_push_from_resolved` is confirm-release-only (Step-7).

## Tests + verification

- **macOS host** (`cargo test -p brokerd`): full suite green (host-portable git.push tests: 7 `prepare_git_push` + `git_push_frozen_new_oid_is_mac_covered` + `git_push_precheck_failure_leaves_row_pending_no_burn` + 4 WG-8 render tests). Linux-gated + `mock-egress-ca`-gated tests compile to 0 (expected, cfg-linux-test-blindness).
- **check-invariants.sh**: ALL gates PASS — Gate 1 (no `EffectRequest`), Gate 3 (mint sites byte-identical — no confirmation.rs/server.rs git.push mint), Gate 5 (no new crate / aws-lc-rs absent).
- **Linux container** (`rust:1`, `cargo build --workspace` first for the confined launcher):
  - default build: `server:: confirmation::` = **52 passed / 0 failed**; git.push focused tests green (`clean_git_push_is_confirm_gated_never_auto_dispatched`, `tainted_git_push_blocks_with_frozen_oid`, both confirmation git.push tests).
  - `mock-egress-ca`: the 4 git.push gate/confirm-release tests = **4 passed / 0 failed** (incl. `clean_git_push_pending_row_is_confirm_releasable` reaching Step-7 -> terminal `git_push_failed` -> `ConfirmedButSinkFailed`, and `git_push_confirm_releases_once_reaching_step7_dispatch`).
  - Both feature builds compiled the Linux-gated tests (cfg-linux-test-blindness guarded via container `cargo build --workspace --tests`).

## Deviations from Plan

1. **[Rule 1 - Bug] `resolve_new_oid` read the merged launcher stream** (`git_push.rs`, Plan 44-03 code). `run_launcher` MERGES stdout+stderr, and the confined launcher writes a `[caprun-exec-launcher] Landlock …` diagnostic to stderr — so the merged-stream parse fed that diagnostic into `validate_oid`, which failed on the interleaved text (the freeze never produced a valid oid). Switched to `run_launcher_capture_bytes` (stdout separated) and parse the oid from stdout only. Exposed by the first test to freeze a real repo. Commit `bc7c73b`.
2. **Convergence shape**: the plan suggested "a single helper that freezes + assembles + MACs + inserts, called from both paths." Implemented convergence more surgically by rewriting an Allowed git.push into a synthetic `BlockedPendingConfirmation` (anchors = routing args) BEFORE the EXISTING shared block machinery — so both paths reuse the ALREADY-shared freeze (`freeze_new_oid`) + MAC (`insert_pending_confirmation`) + insert, avoiding a restructure of the generic block path (surgical-changes constraint). Security-relevant convergence holds: neither path can insert a git.push pending row without a frozen oid + a valid MAC. A synthetic empty-anchor block is rejected by the Defect B guard, so the routing args are anchored (semantically: the destination the human authorizes).
3. **WG-8 per-file scope**: the plan aspires to "enumerate the changed files in the range + flag per-file taint." `render_block_display` is a pure, sync function whose only input is the pending-confirmation snapshot (`frozen_new_oid` + `resolved_args = {remote, refspec}`) — it has NO workspace/git/value-store access at confirm time and must not perform a network OR git read (§1.6). The renderer therefore surfaces the frozen commit oid + a per-ROUTING-ARG taint-provenance summary (the payload-destination values that ARE in the snapshot), which is the provenance surface achievable purely. This honors §1.6 ("surface provenance for human judgment, not byte-identity") without over-promising. The enforcement (taint-Block, per-push confirm gate, new-oid freeze/anti-TOCTOU) is intact from Tasks 1-2 + Plan 44-03.

## Known Stubs

None. (git.push's live network/`git` legs remain `#[cfg(target_os = "linux")]` per the project's Linux-only-security-tests discipline — not stubs, exercised on the Linux gate.)

## Deferred / out-of-scope

See `deferred-items.md`: pre-existing bare-container Linux failures unrelated to this plan — two `process_exec::capture_bytes_tests` (confined `/bin/cat`,`/bin/sh` exec; `process_exec.rs` byte-identical to the pre-plan baseline) and three `email.send` tests requiring the Mailpit sidecar (`scripts/mailpit-verify.sh`, CLAUDE.md Phase 16+). Not regressions.

## Self-Check: PASSED

- All 5 commits exist (`409d037`, `96ab595`, `bc7c73b`, `8b43c38`, `c48e606`).
- Modified files exist and compile: `server.rs`, `confirmation.rs`, `audit.rs`, `sinks/git_push.rs`.
- No auto-dispatch arm for git.push (verified by Linux test + no `invoke_git_push*` call outside confirmation.rs Step-7).
