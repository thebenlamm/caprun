---
phase: 44-git-push-broker-performed-destination-pinned-egress
plan: 02
subsystem: brokerd-egress
tags: [git-push, pkt-line, smart-http, ssrf-pin, structural-denial, protocol-substrate]
requires:
  - http_request.rs SSRF resolve-and-pin infra (Phase 40) — build_pinned_client / vet_resolved / ssrf_check
  - git.push executor-TCB registration (Plan 44-01)
provides:
  - pure host-portable git-push protocol substrate — pkt-line encode/decode, ref-advertisement parser, report-status parser
  - validate_git_refspec value-gate + build_command_list (force/deletion unreachable by construction; create distinguished from delete, WG-6)
  - WG-1 single-frozen-IP primitive — pub(crate) build_pinned_client/vet_resolved + resolve_and_vet(host) -> SocketAddr
affects:
  - Plan 44-03 (two-request driver + confined pack-gen consume these; dispatch/confirm-release)
tech-stack:
  added: []
  patterns:
    - pure-protocol substrate module (no socket / no git binary / no async) — fully macOS-testable
    - two-layer value-level structural denial (validate_git_refspec + zero-new-oid command-list refusal)
    - reuse the shipped SSRF resolve-and-pin classifier verbatim (widen visibility, never re-implement)
key-files:
  created:
    - crates/brokerd/src/sinks/git_push.rs
  modified:
    - crates/brokerd/src/sinks.rs
    - crates/brokerd/src/sinks/http_request.rs
    - crates/brokerd/src/policy.rs
decisions:
  - "side-band-64k is NOT advertised (RESEARCH §3 simplest-correct-subset): report-status arrives on the main band, no band demux. Fixed RECEIVE_PACK_CAPS = 'report-status agent=caprun' — no side-band, no force capability, so a force update is not expressible."
  - "object-format defaults to SHA-1: validate_oid accepts BOTH 40-hex (SHA-1) and 64-hex (SHA-256) oids, but the command-list caps do NOT negotiate object-format=sha256. A SHA-256 remote is an untested assumption flagged for 44-03 / real-remote verification."
  - "resolve_and_vet's DNS leg is Linux-gated (macOS no-op stub, mirroring do_pinned_get); its host-portable contract is the vet_resolved/freeze half — one frozen SocketAddr + fail-close on any denied IP — covered by host-portable tests reusing the shipped ssrf fixtures."
  - "module-scoped allow(dead_code) on git_push.rs + allow(dead_code) on resolve_and_vet: their consumers (44-03 two-request driver) are not present yet, so a non-test build sees them unused. Narrows to nothing once 44-03's dispatch arm lands."
metrics:
  duration: ~40m
  completed: 2026-07-18
status: complete
---

# Phase 44 Plan 02: Broker git-push protocol substrate + WG-1 frozen-IP primitive Summary

Built the pure, host-portable protocol half of the broker-performed `git.push` transfer — pkt-line encode/decode, the `git-receive-pack` ref-advertisement + report-status parsers, the two-layer `validate_git_refspec` + zero-new-oid command-list structural denial (force/deletion unreachable even via confirm, create distinguished from delete), and the WG-1 single-frozen-IP primitive that lets ONE pinned client serve both requests of the two-request exchange with no re-resolve. No socket, no git binary, no new crate; the confined pack bytes + wire driver are Plan 44-03.

## What was built

**Task 1 — pkt-line + advertisement + report-status parsers** (commit `95e0566`)
- New `crates/brokerd/src/sinks/git_push.rs`: `pkt_line` (4-hex big-endian length prefix) + `flush_pkt`; `read_pkt` decode (fail-closed on truncated/short/non-hex length and truncated payload; `0000` => Flush; empty => clean end).
- Advertisement parser: skips the `# service=git-receive-pack` pkt + flush, splits the FIRST ref line on NUL for capabilities, collects `refname -> oid`, and signals CREATE (`old_oid_for` => `None`) for an unadvertised ref (WG-6). Handles the empty-repo `capabilities^{}` line.
- Report-status parser: requires a clean `unpack ok` AND ≥1 per-ref `ok <ref>`; ANY `unpack <err>` / `ng <ref> <reason>` / unrecognized line is a fail-closed push failure (T-44-08). No side-band demux (not advertised).
- Registered `pub mod git_push` in `sinks.rs`. 19 host-portable unit tests (round-trip + malformed + adv-parse + report-parse).

**Task 2 — validate_git_refspec value-gate + structural command-list** (commit `d12ea80`)
- `validate_git_refspec` (pub(crate)) mirrors `validate_write_method`: fail-closed on a leading `+` (force), any `--force`/`--force-with-lease`/`--flag`-shaped token, and an empty `<src>` (`:dst` deletion) / malformed `<src>:<dst>`. Ok for a plain `<src>:<dst>` or bare `<ref>` (RESEARCH §5 layer 1).
- `build_command_list` (pub(crate)) emits `pkt_line("<old> <new> <refname>\0<fixed-caps>")` + flush from caller-supplied oids; REFUSES a zero-new-oid delete by construction for ANY input (RESEARCH §5 layer 2, DESIGN §1.3) while ALLOWING a zero-old-oid create (WG-6 — refusal keys on new-oid ONLY). Fixed caps carry NO side-band, NO force. `old-oid` stays a caller param (frozen advertisement, WG-6/T-44-07). 9 unit tests.

**Task 3 — WG-1 single-frozen-IP primitive** (commit `ad8da19`)
- Widened `vet_resolved` + `build_pinned_client` from private to `pub(crate)` in `http_request.rs` — logic UNCHANGED (redirect-none, ring TLS, `.resolve(host, pinned)`, `ssrf_check` fail-close).
- Added `resolve_and_vet(host) -> Result<SocketAddr>` (Linux impl + macOS stub): the resolve + `vet_resolved` half of `resolve_and_pin` that RETURNS the vetted addr `resolve_and_pin` discards — so a caller freezes ONE IP and builds ONE `build_pinned_client(host, addr)` client for BOTH the info/refs GET and the receive-pack POST, no re-resolve, redirect-none in force for both (DESIGN §1.5). Doc-note: `invoke_pinned_post` is FORBIDDEN for the git flow (it re-resolves, RESEARCH A7). `resolve_and_pin`/`invoke_pinned_post`/GET/http-write paths UNCHANGED. 2 host-portable tests reusing the shipped ssrf fixtures.

## Verification

- `cargo build --workspace` — clean, no warnings (macOS host).
- `cargo test -p brokerd sinks::git_push:: --lib` — 28 passed / 0 failed.
- `cargo test -p brokerd sinks::http_request:: --lib` — 50 passed / 0 failed (2 new).
- `cargo test -p brokerd --no-fail-fast` — 239 lib passed / 0 failed (+ integration targets green).
- `./scripts/check-invariants.sh` — ALL gates PASSED (exit 0). **Gate 3 mint-site allow-list byte-identical** (git_push.rs mints nothing — no `mint_*`/`.mint()` token). **Gate 5 green** — aws-lc-rs absent from the workspace build graph; no openssl-sys via reqwest.
- HYG-01 supply-chain (§7): `cargo tree -i aws-lc-rs` => absent; `reqwest` unchanged `v0.13.4`; `Cargo.lock` untouched `e87b95b..HEAD` — **zero new crates**.

Note (CLAUDE.md): the socket leg of `resolve_and_vet` is `#[cfg(target_os = "linux")]` (macOS no-op stub) — it compiles to a no-op here, expected not a gap. The Linux-gated done-check is a container `cargo build --workspace --tests` + `cargo test -p brokerd sinks::git_push:: sinks::http_request::` via `scripts/mailpit-verify.sh` (watch [[cfg-linux-test-blindness]]); everything else in this plan is pure/host-portable and fully exercised on macOS.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Fixed a stale brokerd broker_default negative test broken by Plan 44-01**
- **Found during:** full-suite verification after Task 3.
- **Issue:** `brokerd/src/policy.rs::bind_policy_none_binds_broker_default` asserted `!permits_sink("git.push")` and used `git.push` as its unknown-sink example. Plan 44-01 added `git.push` to `PRODUCTION_SINKS` (so `broker_default` now permits it) and flipped the **runtime-core** twin (`policy.rs:555`) but MISSED this brokerd sibling. The test failed at 44-01's tip (`e87b95b`) — a pre-existing failure NOT caused by this plan's changes, but it blocks this plan's required `cargo test -p brokerd` green gate.
- **Fix:** Mirrored 44-01's runtime-core fix exactly — moved `git.push` (and the Phase-43 `http.request.write`, also stalely absent) into the permitted-sinks loop and retargeted the deny-unlisted assertion to a genuinely-unregistered id (`deploy.service`); corrected the `seven`→`nine` count. No mechanism weakened: the deny-by-default negative coverage is preserved on a real unlisted sink. This is a Rule 1 correction of a false-assertion test to match 44-01's already-shipped design decision (git.push IS a production sink, T-44-03).
- **Files modified:** `crates/brokerd/src/policy.rs`.
- **Commit:** `ddb6d91`.

All other work matches the plan exactly.

## Assumptions (flagged for Plan 44-03 / real-remote verification)

- **No side-band-64k** (RESEARCH §3 A3): report-status is read on the main band. If a target remote forces side-band, the report-status parser needs band demux.
- **object-format = SHA-1** (A2-adjacent): `validate_oid` accepts both 40- and 64-hex widths, but the command-list caps do not negotiate `object-format=sha256`. A SHA-256 remote is untested here.
- **`--thin` pack acceptance** (A2) is a 44-03/live concern — no pack generation in this plan.

## Known Stubs

None. Every function is real, pure, and unit-tested. The only intentionally-deferred surface is the Linux `resolve_and_vet` DNS leg (macOS no-op stub, mirroring the shipped `do_pinned_get`/`do_pinned_post` cfg split) and the module-scoped `allow(dead_code)` — both because the CONSUMER (Plan 44-03's two-request driver) has not landed yet, not because any logic is placeholder. No hardcoded UI-flowing empty values, no "TODO/coming soon".

## Threat Flags

None. This plan introduces no new network endpoint, auth path, or trust-boundary schema beyond the DESIGN §1.3/§1.5 surfaces already in the plan's `<threat_model>` (T-44-05..08). The reused SSRF classifier is widened in visibility only, logic unchanged.

## Self-Check: PASSED

- `crates/brokerd/src/sinks/git_push.rs` — FOUND (pkt-line + parsers + validate_git_refspec + build_command_list + 28 tests).
- `crates/brokerd/src/sinks/http_request.rs` — FOUND (pub(crate) build_pinned_client/vet_resolved + resolve_and_vet + 2 tests).
- `crates/brokerd/src/sinks.rs` — FOUND (pub mod git_push).
- `crates/brokerd/src/policy.rs` — FOUND (deviation fix).
- `.planning/phases/44-.../44-02-SUMMARY.md` — FOUND.
- Commits `95e0566`, `d12ea80`, `ad8da19`, `ddb6d91` — all FOUND in `git log`.
