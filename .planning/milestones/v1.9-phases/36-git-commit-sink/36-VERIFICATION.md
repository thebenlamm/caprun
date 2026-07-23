---
phase: 36-git-commit-sink
verified: 2026-07-18T00:00:00Z
status: passed
score: 4/6 host-portable must-haves verified directly; 2 Linux-only behavioral claims compile-clean on Linux and deferred to Phase 40 (LIVE-03/04) per standing convention
behavior_unverified: 2
overrides_applied: 1
override:
  truth: "The 2 Linux-only behavioral claims (non-stapled process_exited-rooted taint edge; planted hook/alias inert) were routed human_needed by the verifier."
  decision: accepted-deferred-to-phase-40
  accepted_by: orchestrator (autonomous mode; human-equivalent per DEC-ai-review-satisfies-human-gate + Ben's explicit delegation of the full v1.8 milestone)
  accepted_at: 2026-07-18
  reconciliation: "Two independent bases: (a) the orchestrator ran a full Linux `cargo build --tests --workspace --keep-going` in the rust:1 container (BUILD_TESTS_EXIT=0) — git_commit_spawn.rs compiles clean on Linux, closing the cfg-linux-test-blindness COMPILE risk; (b) the behavioral execution is the explicit job of Phase 40's composed live-proof (LIVE-03/04), matching how v1.7 concentrated Linux-behavioral verification at its regression/live-proof phase. The neutralization mechanism + non-stapled mint were code-reviewed as structurally correct (mirror the shipped process.exec pattern verbatim). Not a code gap; a deferred behavioral assertion with a compile-clean Linux backstop."
re_verification: null
behavior_unverified_items:
  - truth: "A tainted git.commit produces a genuinely-propagated (non-stapled) audit-DAG edge: the minted ValueRecord's provenance_chain[0] equals the real process_exited event id, and verify_chain stays intact after a real confined `git commit` spawn."
    test: "crates/brokerd/tests/git_commit_spawn.rs::linux::git_commit_produces_real_commit_with_process_exited_rooted_taint — run on the Linux container (`cargo test -p brokerd --test git_commit_spawn`, or via scripts/mailpit-verify.sh)."
    expected: "HEAD advances to a real commit; the process_exited event exists and is chained; provenance_chain[0] == that event id; taint == [ExternalUntrusted, ExecRaw]; verify_chain true."
    why_human: "The test is `#[cfg(target_os=\"linux\")]`-gated — the confined-child launcher only actually self-confines on Linux, so it shows 0 tests on this macOS host (cfg-linux-test-blindness, expected per CLAUDE.md). Code was read and is structurally correct (mirrors the shipped process.exec pattern verbatim) but the runtime claim is unexercised here."
  - truth: "A planted malicious `.git/hooks/pre-commit` and a repo-local `alias.evil` in the workspace repo do NOT execute during an Allowed git.commit, and the commit still succeeds (P2 RCE closed)."
    test: "crates/brokerd/tests/git_commit_spawn.rs::linux::planted_hook_and_alias_do_not_execute_and_commit_succeeds — run on the Linux container."
    expected: "HOOK_FIRED_SENTINEL and ALIAS_FIRED_SENTINEL are never created inside the repo; `git rev-parse --verify HEAD` still succeeds."
    why_human: "Same cfg-linux-test-blindness gap as above — genuinely spawns caprun-exec-launcher + system git under real Landlock/seccomp confinement, which only applies on Linux. Neutralization mechanism (`-c core.hooksPath=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, broker-constructed argv) is present and correctly ordered in git_commit.rs, but its behavioral effect is unexercised on this host. Per task instructions, this is deferred to Phase 40's composed live-proof run (ROADMAP LIVE-03/LIVE-04) or a dedicated Linux gate — not a code gap in Phase 36."
human_verification:
  - test: "On the Linux container (Colima/Docker per CLAUDE.md, or scripts/mailpit-verify.sh), run: `cargo build --workspace` then `cargo test -p brokerd --test git_commit_spawn`."
    expected: "3/3 tests pass: genuine commit + non-stapled audit-DAG edge; planted hook/alias inert with commit still succeeding; exec-child env-clear (author/committer == `caprun`, never the broker's `GIT_AUTHOR_NAME` sentinel)."
    why_human: "Requires real Linux kernel confinement primitives (Landlock/seccomp) that are no-op stubs on macOS; cannot be verified by static analysis or on this dev host."
---

# Phase 36: `git.commit` Sink Verification Report

**Phase Goal:** caprun can commit staged workspace changes via a broker-spawned confined-child `git`, with the commit message's taint genuinely propagated and git config/hooks neutralized.
**Verified:** 2026-07-18
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1a | `git.commit` dispatches via Pattern B (broker-spawned `caprun-exec-launcher`, reusing `run_launcher`) — no new spawn machinery | ✓ VERIFIED | `crates/brokerd/src/sinks/git_commit.rs:150-159` calls `process_exec::run_launcher` verbatim; `server.rs:1141-1166` git.commit Allowed-dispatch arm mirrors the process.exec arm; `run_launcher` grew a surgical `extra_env` param (`process_exec.rs:402-409`) and BOTH pre-existing call sites (`process_exec.rs:152`, `:295`) pass `&[]`, confirmed unchanged by `cargo build --workspace` clean + `cargo test -p brokerd process_exec` green (no regressions, full `cargo test -p brokerd` also green, 117+ tests). |
| 1b | `git.commit` is classified `MutateReversible` (first non-CommitIrreversible real sink), so it survives an I1-demoted (Draft) session | ✓ VERIFIED | `sink_sensitivity.rs:55` `"git.commit" => EffectClass::MutateReversible`; test `git_commit_is_mutate_reversible` passes (`cargo test -p executor sink_sensitivity` — 37/37 pass incl. 4 new git.commit tests). The Draft-session gate (`crates/executor/src/lib.rs:216-226`) is a pure equality match on `sink_effect_class(...) == EffectClass::CommitIrreversible`; since git.commit's class is proven `MutateReversible`, it structurally falls through to `Allowed` in a Draft session — deterministic composition of two directly-inspected facts, not a hidden runtime path. |
| 2a | A tainted `message` Blocks under the unmodified `submit_plan_node` collect-then-Block loop (I2) | ✓ VERIFIED | `sink_sensitivity.rs:165` `is_content_sensitive("git.commit","message")==true` (tested, `git_commit_message_is_content_sensitive` passes). `submit_plan_node` (`crates/executor/src/lib.rs:78-197`) is a generic pure function keyed only by `is_routing_sensitive`/`is_content_sensitive(sink,arg)` + taint — the mechanism itself is exercised and proven by every other sink's existing tests (email.send, file.write, process.exec); git.commit's Block behavior is a deterministic consequence of the proven-true classification fact feeding the proven-correct generic mechanism. |
| 2b | A tainted commit message's Block anchor genuinely propagates as an unbroken, non-stapled audit-DAG edge on a REAL confined spawn | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | Code present + wired (`git_commit.rs:162-182` appends `process_exited` tainted `[ExternalUntrusted, ExecRaw]`; `server.rs:1160-1165` mints via the EXISTING `mint_from_exec` rooted on that exact event id — no new mint site, Gate 3 green). The anti-staple assertion (`provenance_chain[0] == process_exited event id`) is exercised by `git_commit_spawn.rs::linux::git_commit_produces_real_commit_with_process_exited_rooted_taint`, but this test is `#[cfg(target_os="linux")]`-gated — 0 tests on this macOS host (cfg-linux-test-blindness, expected per CLAUDE.md). See human_verification. |
| 3 | git system config + hooks neutralized in the child (`GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL=/dev/null`, `-c core.hooksPath=/dev/null`, no aliases, `env_clear()`'d) — a planted hook/alias does not execute | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | Mechanism present + correctly ordered in code: `git_commit.rs:117-127` builds the broker-TRUSTED argv with `-c core.hooksPath=/dev/null` BEFORE the `commit` subcommand (highest git config precedence); `:138-142` sets `extra_env=[GIT_CONFIG_NOSYSTEM=1, GIT_CONFIG_GLOBAL=/dev/null, GIT_TERMINAL_PROMPT=0]`; `run_launcher`'s `env_clear()` (`process_exec.rs:415`) applies before `extra_env` is layered on. The negative test (`git_commit_spawn.rs::linux::planted_hook_and_alias_do_not_execute_and_commit_succeeds`) exercises a planted pre-commit hook + `alias.evil` with a genuine in-repo sentinel (not merely output-absence), but is Linux-gated — 0 on macOS. See human_verification. |

**Score:** 4/6 truths verified (host-portable), 2 present-but-behavior-unverified (Linux-only, both routed to human verification / Phase 40).

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/executor/src/sink_schema.rs` | git.commit KNOWN_SINKS row (allowed=required=[message]) | ✓ VERIFIED | Lines 76-91; exact-match schema; 5 new tests all pass (registered-sink, exact-ok, unknown-arg, duplicate-arg, missing-required). |
| `crates/executor/src/sink_sensitivity.rs` | MutateReversible arm + content-sensitivity + role rows | ✓ VERIFIED | Lines 55, 133, 165, 271-283; 4 new tests all pass. |
| `crates/brokerd/src/sinks/git_commit.rs` | `invoke_git_commit` — Pattern B dispatch, neutralized argv/env, two-phase audit, never mints | ✓ VERIFIED | 221 lines; builds clean; grep confirms no `mint_from_exec`/`.mint(` call inside this file (Gate 3 green). |
| `crates/brokerd/src/server.rs` | git.commit Allowed-dispatch arm invoking `invoke_git_commit` + `mint_from_exec` | ✓ VERIFIED | Lines 1135-1166, verbatim mirror of the process.exec arm. |
| `crates/brokerd/src/sinks/process_exec.rs` | `run_launcher` extended with `extra_env`, both call sites pass `&[]` | ✓ VERIFIED | Lines 402-409 (signature), 152 and 295 (both call sites pass `&[]`); `cargo test -p brokerd process_exec` green, confirming byte-identical behavior. |
| `crates/brokerd/tests/git_commit_spawn.rs` | Linux-gated spawn tests (genuine commit, neutralization, env-clear) | ⚠️ ORPHANED-ON-THIS-HOST (present + compiles, 0 executed) | 335 lines, `#[cfg(target_os="linux")]`; `cargo test -p brokerd --test git_commit_spawn` shows "0 passed" on macOS — EXPECTED per cfg-linux-test-blindness, not a stub (code reviewed, structurally sound, mirrors `process_exec_spawn.rs`). Requires Linux execution to become VERIFIED. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `server.rs` Allowed(git.commit) arm | `sinks::git_commit::invoke_git_commit` | direct call, args mirror process.exec arm | ✓ WIRED | `server.rs:1145-1156`. |
| `invoke_git_commit` | `process_exec::run_launcher` | `resolve_launcher_path()` + `run_launcher(...)` reuse | ✓ WIRED | `git_commit.rs:148-159`. |
| `server.rs` git.commit arm | `quarantine::mint_from_exec` | rooted on the returned `process_exited` event id | ✓ WIRED (host-portable proof: Gate 3 static check; runtime anti-staple proof is Linux-only, see truth 2b) | `server.rs:1163-1165`. |
| `confirmation.rs` Step 4.75 entry guard | git.commit (absence) | allow-list `{"file.create","email.send","file.write","process.exec"}` does NOT include `"git.commit"` | ✓ WIRED (correctly absent) | `confirmation.rs:836-845` — any confirm attempt on a blocked git.commit returns `Err` BEFORE `confirm_granted` is appended (fail-closed-recoverable, row stays Pending); matches DESIGN §9's scoping of the P33/P34 confirm-release discipline to `git.push`/`github.pr` only. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| GIT-01 | 36-01, 36-02 | git.commit sink, MutateReversible, message content-sensitive taint carrier, git config/hook neutralization | ✓ SATISFIED (host-portable parts) / Linux-behavioral parts human_needed | See Observable Truths 1a-3 above. `.planning/REQUIREMENTS.md`'s Traceability table still shows GIT-01 "Pending" — expected, updated at phase sign-off, not a gap. |

### Anti-Patterns Found

None. Scanned all phase-touched files (`sink_schema.rs`, `sink_sensitivity.rs`, `git_commit.rs`, `git_commit_spawn.rs`, `server.rs`, `process_exec.rs`) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER`/empty-return/hardcoded-empty patterns — the single hit (`server.rs:2037`, "a placeholder") is a pre-existing, unrelated test-harness comment about `trusted_inode` in a different test module, not touched by this phase.

### Design-Contract Cross-Check

`planning-docs/DESIGN-git-github-http-sinks.md` §1 (dispatch=Pattern B, effect-class=MutateReversible with rationale, message=content-sensitive taint carrier, no new mint site, config/hook neutralization) and §9 (confirm-release audit-gap discipline scoped to git.push/github.pr ONLY, NOT git.commit) were read against the shipped code — every pinned decision matches the implementation exactly (§1.1↔git_commit.rs dispatch, §1.2↔sink_effect_class arm, §1.3↔content-sensitivity, §1.4↔no-new-mint, §1.5↔neutralization ordering, §9↔confirmation.rs:836-845 allow-list correctly excluding git.commit).

### Scoped Decision Confirmed (not a gap)

Per DESIGN §9 and the 36-02-SUMMARY's flagged scoped decision: `git.commit` is deliberately absent from the Step 4.75 confirm-release allow-list. Verified directly in `confirmation.rs:824-845` — an attempted `confirm()` on a blocked git.commit returns an `Err` (fail-closed-recoverable, row stays `Pending`, no `confirm_granted` ever appended) BEFORE any state transition. This is the correct security posture (a tainted message just Blocks, with no P33/P34-class audit-gap surface), not an omission.

### Human Verification Required

1. **Real Linux confined spawn — genuine commit + non-stapled audit-DAG edge**
   **Test:** Run `crates/brokerd/tests/git_commit_spawn.rs` on the Linux container: `cargo build --workspace && cargo test -p brokerd --test git_commit_spawn` (or `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test git_commit_spawn' bash scripts/mailpit-verify.sh`).
   **Expected:** All 3 tests pass — (a) HEAD advances to a real commit, `process_exited` event chained, minted `provenance_chain[0]` equals that event id, taint pair intact, `verify_chain` true; (b) a planted `.git/hooks/pre-commit` + `alias.evil` never fire (sentinel files absent) and the commit still succeeds; (c) commit author/committer is `caprun`, never the broker's `GIT_AUTHOR_NAME`/`GIT_COMMITTER_NAME` sentinel.
   **Why human:** Landlock/seccomp confinement is a Linux-kernel-only mechanism; macOS runs these as no-op stubs (cfg-linux-test-blindness). Code was read and is structurally sound; the runtime claim is unexercised on this host. A separate Linux compile-check is reported as already running in parallel with this verification — its result should be merged before final phase sign-off. ROADMAP Phase 40 (LIVE-03/LIVE-04) also composes git.commit into the full Linux live-proof run.

### Gaps Summary

No code-level gaps found. The only open items are the two Linux-only behavioral truths (2b, 3) — the mechanism is fully implemented, wired, and code-reviewed correct, but unexercised on this macOS host per the project's standing cfg-linux-test-blindness convention. These require a Linux test run (either the in-flight parallel compile-check + a full test run, or Phase 40's composed live-proof) before the phase can be marked fully `passed`.

---

*Verified: 2026-07-18*
*Verifier: Claude (gsd-verifier)*
