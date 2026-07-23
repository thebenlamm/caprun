---
phase: 44-git-push-broker-performed-destination-pinned-egress
plan: 03
subsystem: brokerd-egress
tags: [git-push, pack-generation, binary-launcher, ssrf-pin, credential-custody, opaque-audit, anti-toctou]
requires:
  - git-push protocol substrate + WG-1 frozen-IP primitive (Plan 44-02) — parse_advertisement / parse_report_status / validate_git_refspec / build_command_list / resolve_and_vet / build_pinned_client
  - Pattern-B confined launcher (Phase 32/34) — run_launcher / SAFE_EXEC_PATH / env_clear / resolve_launcher_path
  - git.commit confinement recipe (Phase 36, A14) — -c core.hooksPath=/dev/null + GIT_CONFIG_NOSYSTEM/GLOBAL neutralization + exit-code gating
  - http_write opaque two-phase audit + broker-env credential shape (Phase 43, A9)
provides:
  - run_launcher_capture_bytes — WG-2 binary-capable confined spawn (raw stdout/stderr Vec<u8> + optional stdin) sharing run_launcher's confinement stack
  - confined git pack-objects pack generation (net-denied, workspace-Landlocked, config-neutralized, exit-code-gated) + resolve_new_oid rev-parse
  - broker-env CAPRUN_GIT_PUSH_TOKEN credential + distinct GIT_PUSH_HOST_ALLOWLIST (WG-9) + opaque scrubbed git_push_succeeded/_failed two-phase audit
  - invoke_git_push_from_resolved — the confirm-release-only frozen-IP two-request transfer driver (WG-7 anti-TOCTOU oid freeze, redirect-none, no invoke_pinned_post)
affects:
  - Plan 44-04 (confirm-release Step-7 dispatch wires invoke_git_push_from_resolved + threads frozen_new_oid from the pending confirmation)
tech-stack:
  added: []
  patterns:
    - refactor-not-fork the confined-spawn machinery (one configure_confined_command; run_launcher unchanged byte-for-byte)
    - binary-safe capture (separate Vec<u8> stdout/stderr, optional stdin) for a packfile that a lossy-UTF-8 merged String would corrupt
    - clone git.commit's Pattern-B confinement recipe for pack-gen; reuse the shipped SSRF resolve-and-pin verbatim (never re-implement)
    - opaque scrubbed two-phase audit (terminal failed-event FIRST, P33/P34) + credential/URL log scrub (MINOR-4)
key-files:
  created: []
  modified:
    - crates/brokerd/src/sinks/process_exec.rs
    - crates/brokerd/src/sinks/git_push.rs
    - crates/brokerd/src/sinks/http_request.rs
decisions:
  - "run_launcher_capture_bytes shares the confinement stack with run_launcher via a new configure_confined_command helper — the confinement machinery is refactored, never forked (a drift there = a security regression). run_launcher keeps its exact signature/behavior; git.commit/process.exec/rev-parse callers are byte-untouched."
  - "pack-gen realized as git pack-objects --revs --stdout --thin --delta-base-offset (RESEARCH §2 primary), NOT git send-pack --stateless-rpc — stdin is a tiny text rev-list, stdout the binary pack; strictly less surface. build_pack_revlist omits the ^<old> exclusion for a create (WG-6)."
  - "The push credential header is Basic auth (x-access-token:<token>), the git-over-HTTPS token convention, set ONLY on the receive-pack POST. Plan 44-04's mock receive-pack endpoint must accept this scheme (flagged)."
  - "GIT_PUSH_HOST_ALLOWLIST ships EMPTY (fail-closed, WG-9) — release can push to NOTHING until an operator surfaces a target; the mock host is admitted only under the non-default mock-egress-ca feature, with a not(mock-egress-ca) base-set invariant test."
  - "The full end-to-end oid-mismatch / ng-report-status / redirect-refused integration assertions require the live git-receive-pack mock (Plan 44-04). Here they are realized host-portably as the pure assert_frozen_oid equality gate (WG-7) + parse_report_status (44-02) + build_pinned_client redirect-none, plus the audit-fold proven via the allowlist gate. No POST is attempted on a mismatch by code ordering (assert_frozen_oid precedes command-list/pack/POST)."
  - "module-scoped allow(dead_code) on git_push.rs retained: the driver's Linux socket/git legs stub out on macOS, so the substrate parsers/command-list/pack helpers are reachable there only from unit tests. Narrows once Plan 44-04's dispatch arm consumes invoke_git_push_from_resolved on the Linux live path."
metrics:
  duration: ~55m
  completed: 2026-07-18
status: complete
---

# Phase 44 Plan 03: WG-2 binary pack-gen + frozen-IP git.push transfer driver Summary

Built the confined pack-generation + wire-transport half of the broker-performed `git.push`: the WG-2 binary-capable confined-spawn variant (a binary packfile cannot survive `run_launcher`'s lossy-UTF-8 merged String), a net-denied `git pack-objects` Pattern-B child, broker-env credential custody + a distinct `GIT_PUSH_HOST_ALLOWLIST`, an opaque scrubbed non-minting two-phase audit, and `invoke_git_push_from_resolved` — the confirm-release-only two-request driver that assembles Plan 44-02's protocol + this plan's pack over ONE frozen-IP redirect-none client with a WG-7 anti-TOCTOU new-oid freeze. Zero new crates.

## What was built

**Task 1 — WG-2 binary launcher + confined pack generation** (commit `1a2961a`)
- `process_exec.rs`: extracted `configure_confined_command` (env_clear → SAFE_EXEC_PATH → EXEC_* → extra_env → workspace root → kill_on_drop) shared by `run_launcher` (unchanged) and the new `run_launcher_capture_bytes` — which captures raw `Vec<u8>` stdout SEPARATE from stderr (no `from_utf8_lossy`, no merge) with OPTIONAL stdin, raced with capped reads + `wait()` under the same timeout/byte-cap/kill_on_drop. Linux round-trip tests prove non-lossy bytes + stdout/stderr separation + stdin feeding.
- `git_push.rs`: `build_pack_revlist` (create omits `^<old>`, WG-6); `generate_pack` runs `git pack-objects` as a net-denied, workspace-Landlocked, git-config-neutralized, exit-code-gated child via the binary launcher; `resolve_new_oid` runs `git rev-parse --verify <ref>^{commit}` via the existing String `run_launcher`. Socket/git legs Linux-gated, macOS stubs.
- `http_request.rs`: widened `validate_url` / `check_body_cap` / `MAX_RESPONSE_BODY_BYTES` to `pub(crate)` (RESEARCH A4/A6) for driver reuse.

**Task 2 — credential + distinct allowlist + opaque scrubbed audit** (commit `dc2627c`)
- `git_push_token()`: OPTIONAL `CAPRUN_GIT_PUSH_TOKEN` broker-env-only reader (None when unset is valid; never a plan arg/ValueNode/audit/child env).
- `GIT_PUSH_HOST_ALLOWLIST` distinct from the GET/WRITE lists, empty in release, mock host only under `mock-egress-ca` with a `not(mock-egress-ca)` base-set invariant.
- `scrub_secrets`/`strip_userinfo_urls`: strip token + remote URL + any generic `scheme://userinfo@` before the `eprintln!` logger (MINOR-4, folds Phase-43 NIT-1).
- `append_push_outcome`: OPAQUE two-phase audit — `git_push_failed` appended FIRST then a scrubbed non-swallowed Err (P33/P34); `git_push_succeeded` on a clean report-status; mints nothing.

**Task 3 — invoke_git_push_from_resolved transfer driver** (commit `6f2a911`)
- Single confirm-release entry point (no auto-Allowed variant; git.push is always confirm-gated). Host-portable gates BEFORE any resolve: `validate_url` → distinct `is_git_push_host_allowlisted` → `validate_git_refspec`.
- ONE `resolve_and_vet` frozen IP + ONE `build_pinned_client` that BOTH the info/refs GET and the receive-pack POST ride; `invoke_pinned_post` (re-resolves) never used; redirect-none refuses a 3xx (non-success status → Err).
- WG-7: `assert_frozen_oid` refuses a live rev-parse != the human-confirmed frozen oid BEFORE any command-list/pack/POST. `generate_pack` from the confirmed `{advertised old-oid, frozen new-oid}` range; credential set ONLY on the POST (Basic `x-access-token`). Every fallible leg folds into a terminal `git_push_failed` first; `parse_report_status` fail-closed.

## Verification

- `cargo build --workspace`: clean, no warnings.
- `cargo test -p brokerd`: **252 passed / 0 failed** (lib; git_push module 41 = 28 from 44-02 + 13 new: 2 pack, 4 cred, 4 audit, 3 transfer). Full brokerd suite (all integration binaries) green.
- `cargo test -p executor`: **143 passed / 0 failed** (110 + 29 + 4) — no sibling-crate breakage.
- Linux-gated legs (`run_launcher_capture_bytes` round-trip, `generate_pack` exit-code, socket/git transfer) compile and show 0-passed on the macOS host — expected (cfg-linux-test-blindness), NOT a gap; exercised on the Linux gate / compose-verify per Plan 44-04.
- `./scripts/check-invariants.sh`: **all gates PASS** — Gate 1 (no new EffectRequest), Gate 3 mint-site allow-list byte-identical (git_push mints NOTHING), Gate 4b (mock-egress-ca never default), Gate 5 (aws-lc-rs absent + no openssl-sys via reqwest), Gate 6.
- HYG-01: `cargo tree --workspace -i aws-lc-rs` → absent; NO Cargo.toml/Cargo.lock change (zero new crate).

## Deviations from Plan

**1. [Rule 1 — Blocking issue] Widened three `http_request.rs` items to `pub(crate)`**
- **Found during:** Task 1 / Task 3.
- **Issue:** The driver needs `validate_url` (RESEARCH A4) for the push remote and `check_body_cap`/`MAX_RESPONSE_BODY_BYTES` (A6) to cap the two response reads; all three were private. 44-02 had already widened `build_pinned_client`/`vet_resolved`/`resolve_and_vet` for exactly this reason.
- **Fix:** Widened the three to `pub(crate)` with doc notes; logic byte-unchanged. `http_request.rs` was not in the plan's `files_modified` (which listed only process_exec.rs + git_push.rs), so this file is a documented add.
- **Commit:** `1a2961a`.

**2. [Rule 1 — Test realization] Full oid-mismatch / ng / redirect integration deferred to the Plan 44-04 live mock**
- **Issue:** The plan's Task-3 oid-mismatch / `ng` report-status / redirect-refused end-to-end tests require a live `git-receive-pack` server, which does not exist until Plan 44-04's mock (RESEARCH WG-9).
- **Fix:** Realized host-portably per the plan's "where host-portable" qualifier: the pure `assert_frozen_oid` equality gate (WG-7), `parse_report_status` fail-closed (already covered in 44-02's `mod tests`), `build_pinned_client` redirect-none (44-02), and the audit-fold proven via the allowlist gate (`driver_non_allowlisted_host_folds_into_opaque_git_push_failed`). "No POST attempted on a mismatch" is guaranteed by code ordering (`assert_frozen_oid` precedes command-list/pack/POST). The full live integration is Plan 44-04's Linux proof.

**3. [Rule 3 — Unpinned detail resolved] Push credential scheme**
- The DESIGN said "credential header" generically. Chose Basic auth `x-access-token:<token>` (the git-over-HTTPS token convention) set only on the POST. Plan 44-04's mock receive-pack endpoint must accept this scheme — flagged in decisions.

## Known Stubs

- macOS `#[cfg(target_os = "linux")]` no-op stubs for `generate_pack` / `resolve_new_oid` / `run_git_push_network` and its socket helpers — intentional platform splits (the confinement + socket + real git legs are Linux-only per CLAUDE.md), NOT data stubs.
- `GIT_PUSH_HOST_ALLOWLIST` is empty in the release build — an intentional fail-closed WG-9 posture (a push has no target until an operator surfaces one), NOT an incomplete stub. The mock host is admitted only under `mock-egress-ca` for the Plan 44-04/44-05 live proof.

No stubs block the plan's goal: `invoke_git_push_from_resolved` is complete and ready for Plan 44-04's confirm-release Step-7 dispatch to consume.

## Self-Check: PASSED

- Commits verified present: `1a2961a` (Task 1), `dc2627c` (Task 2), `6f2a911` (Task 3).
- Modified files present: `process_exec.rs`, `git_push.rs`, `http_request.rs`.
- `.planning/STATE.md` / `.planning/ROADMAP.md` untouched (orchestrator-owned).
