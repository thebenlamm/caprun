---
phase: 44
status: passed
verified_by: orchestrator
date: 2026-07-18
---

# Phase 44 Verification — `git.push` Broker-Performed Destination-Pinned Egress (GIT-02/03, HYG-01)

**Verdict: PASSED. `git.push` SHIPS — it did NOT defer a 3rd time.** The research-pinned
Candidate (b) (broker-performed smart-HTTP transfer) proved sound under the fresh adversarial
code-trace, so the §1.9 safety-valve was not triggered. Proven green on real Linux via an
INDEPENDENT orchestrator re-run of `compose-verify.sh` (**COMPOSE_VERIFY_EXIT=0, 668 passed / 0
failed**, script self-reports "Composed Linux verification suite PASSED"). The fresh non-self
Fable-5 adversarial code-trace of the full ~4,500-line TCB diff returned **APPROVE — 0 security
defects across all 8 attack surfaces**. 5 plans executed sequentially on `main`.

## Goal-backward check

Phase 44's goal: caprun can push to a TRUSTED-intent remote via a fully-unprivileged,
broker-mediated, destination-pinned egress with the push child kept net-denied — completing the
edit→test→commit→push→open-PR loop — without weakening I0/I1/I2, adding any raw `EffectRequest`
path, or adding any new crate.

| Requirement | Evidence | Status |
|-------------|----------|--------|
| GIT-02 (broker-mediated pinned egress, net-denied child, force/delete denied, credential custody) | Broker-performed two-request smart-HTTP (`git_push.rs::run_git_push_network`) over the shipped reqwest-ring resolve-and-pin: ONE `resolve_and_vet` SSRF-vetted `SocketAddr` + ONE `build_pinned_client` (redirect-none) serving BOTH the info/refs GET and the git-receive-pack POST — IP FROZEN across both (WG-1), `invoke_pinned_post` never used, `validate_url`→write-allowlist gate BEFORE resolve. The pack-gen child runs `git pack-objects` under the UNCHANGED `exec_child_filter` net-deny (AF_INET/AF_INET6 → EPERM; seccomp is the §1.8 backstop, not the pin) via `run_launcher_capture_bytes` sharing `run_launcher`'s confinement (`configure_confined_command`). remote/refspec from TRUSTED intent, never `.git/config`. `--force`/`--force-with-lease`/`:delete`(zero-oid)/`+`-refspec HARD-DENIED by construction (`validate_git_refspec` in BOTH the precheck and transfer paths + `build_command_list` zero-oid refusal + no force capability in `RECEIVE_PACK_CAPS`) — unreachable even via confirm. Credential broker-env-only (`CAPRUN_GIT_PUSH_TOKEN`, Basic `x-access-token`), never in a ValueNode/plan-arg/audit/child/planner, never followed across a redirect; output `scrub_secrets`/`strip_userinfo_urls` before any log/audit; opaque non-minting audit (Gate 3 byte-identical). Tests: s44 leg_c (real delivery to mock git-receive-pack, `git_push_succeeded`, credential absent), leg_d, leg_e. | ✅ Complete |
| GIT-03 (tainted remote/refspec Blocks; confirm-release; payload freeze; taint-provenance at confirm; P33/P34) | git.push is ALWAYS confirm-gated — the #1 correctness pin: there is NO bare Allowed→auto-dispatch arm (`invoke_git_push_from_resolved` has exactly one non-test caller, confirmation.rs Step-7; grep-verified). A clean/untainted Allowed git.push is rewritten in `server.rs` into a synthetic `BlockedPendingConfirmation` (freeze the new-oid, resolve args, build a MAC'd pending row via `build_pending_confirmation_mac`, insert, return Block); a tainted remote/refspec ALSO Blocks under I2 → same gate. The confirm prompt surfaces the commit range + a per-arg taint-provenance summary (WG-8) computed over the LOCALLY-computed range from `frozen_new_oid` (no network read at confirm; remote-base divergence is the §1.6 ACCEPTED residual), with tainted literal bytes control-char-neutralized. Anti-TOCTOU (WG-7): the new-oid is frozen + MAC'd into the pending row, `assert_frozen_oid` refuses on divergence, the pack is content-addressed from the frozen OID. P33/P34: `prepare_git_push` precheck at Step-4.8d BEFORE Step-5 `confirm_granted`/Step-6 burn (fail-closed-recoverable, row stays Pending); entry-guard allow-list AND Step-7 dispatch match BOTH list git.push (in sync); every transfer failure folds into a terminal `git_push_failed` FIRST; regression test for no-dangling-confirm_granted. Tests: s44 leg_a (I0 draft-deny — CommitIrreversible), leg_b (tainted Blocks on the named arg, genuine provenance), legs_b_and_c (taint the sole variable), clean-path row confirm-releasable end-to-end (W1 test). | ✅ Complete |
| HYG-01 (post-transport-dep absence re-run + compose feature-OFF guard + Gate 4b workspace-wide) | ZERO new crates — `cargo tree --workspace -i aws-lc-rs` and `-i openssl-sys` absent AFTER all transport code; reqwest unchanged `=0.13.4`; `Cargo.lock` unmodified across the phase. `check-invariants` Gate 4b broadened to a workspace-wide grep; `compose-verify.sh` feature-OFF guard confirms the `mock-egress-ca` mock write host + anchor are absent from the release build (`GIT_PUSH_HOST_ALLOWLIST` ships empty). | ✅ Complete |

## Hard-constraint checks

- **No raw `EffectRequest`** (Gate 1), **no new mint site** (Gate 3 byte-identical — `git_push.rs`
  mints nothing; the transport response never enters the value store), **no new crate** (Gate 5 /
  HYG-01). `check-invariants.sh` all gates (1/2/3/4/4b/5/6) PASS. ✅
- **Always-confirm-gate (the #1 pin):** no auto-dispatch arm for git.push; clean Allowed →
  BlockedPendingConfirmation; `invoke_git_push_from_resolved` confirm-release-only. Confirmed by the
  adversarial trace (single non-test caller, grep-verified) AND s44 leg_c (clean → confirm → deliver). ✅
- **Linux (authoritative, independent orchestrator re-run):** `compose-verify.sh` (full workspace +
  `brokerd/mock-egress-ca` + mock GitHub/git-receive-pack) — **668 passed / 0 failed, exit 0**. All
  5 s44 legs green (leg_a I0-deny, legs_b_and_c taint-sole-variable, leg_c dispatch real delivery,
  leg_d force/delete refused, leg_e redirect refused). No v1.0–v1.8 regression — all prior composed
  live proofs (v1.3/v1.4/v1.7/v1.8) green. ✅

## Adversarial code-trace (standing v1.9 per-phase discipline)

A fresh non-self, orchestrator-owned Fable-5 code-trace of the full Phase-44 TCB diff
(`eff5b2d..HEAD`, ~4,500 lines) traced all 8 briefed attack surfaces against live code and returned
**APPROVE — 0 security defects** (no BLOCKER/MAJOR/MINOR/NIT vulnerability). Confirmed sound: the
auto-dispatch escape is closed (single non-test caller); the destination pin is frozen across both
requests with redirect-none on both; force/delete are structurally refused (unreachable via
confirm); no credential/URL leak into ValueNode/audit/logs (incl. 401/407/redirect legs, via
`scrub_secrets`/`strip_userinfo_urls`, and reqwest 0.13.x Display no longer embeds the URL); the
anti-TOCTOU freeze holds (frozen OID MAC'd + `assert_frozen_oid`); the confirm-release audit-gap is
closed (precheck-before-burn, guard/match in sync, MAC-verified pending row); the pack-gen child is
net-denied + env_clear'd + credential-free; nothing minted; zero new crates.

**One non-security functional note (NOT a defect — flagged for Phase 46 / the team before LIVE-05/06):**
`generate_pack` captures pack stdout under the shared 10 MB `MAX_COMBINED_OUTPUT_BYTES` cap. A pack
exceeding ~10 MB fails **CLOSED** (safe — no partial push, terminal `git_push_failed`), but this
would block pushes of any non-trivial repo history. Recorded as a deferred item; revisit for
LIVE-05/06 (which pushes a small mock repo, so not blocking for Phase 46 itself).

## Process notes (tripwires re-hit + reinforced)

- **[[cfg-linux-test-blindness]] re-hit TWICE:** the full-workspace compose-verify surfaced two
  latent Linux-only defects that scoped/host-only runs had missed and passed as green — a
  `frozen_new_oid` compile break in 44-04's confirm.rs test literals, and a 44-03
  `capture_bytes_tests` stderr over-assertion (the benign launcher Landlock diagnostic). Both fixed
  in 44-05 (`812d80f`/`8755606`/`8f16436`). Reinforces the standing rule: **run the full
  compose-verify at each git/exec plan close, not a scoped `MAILPIT_VERIFY_CMD`.**
- **A real inter-plan bug caught by sequential execution:** 44-04 found + fixed a 44-03 defect
  (`bc7c73b` — `resolve_new_oid` was reading the merged launcher stream instead of stdout only,
  which would have contaminated the frozen oid). Sequential-on-main + real tests earned their keep.
- **A 44-01 verification gap:** 44-01 shipped a failing brokerd policy test (`bind_policy_none_binds_broker_default`
  asserted `!permits_sink("git.push")`) because it ran only `cargo test -p runtime-core`, not the
  brokerd sibling; 44-02 caught + fixed it (`ddb6d91`). The Linux gate would have caught it at phase
  close regardless.

## Notes

- The §1.9 safety-valve (disclosed, sign-off-gated deferral) was NOT triggered — the mechanism
  proved sound. GIT-02/03 ship as v1.9 Phase 44.
- The composed live proof (LIVE-05/06, Phase 46) will exercise git.push in the full
  exec→fs→git.commit→git.push→github.pr + http POST chain, driven & inspected via the Phase-45
  CLI + audit-DAG viewer, with the 5 independently-attributable negative legs.
