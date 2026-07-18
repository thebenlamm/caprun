---
phase: 45-thin-cli-sdk-read-only-audit-dag-viewer
plan: 01
subsystem: cli
tags: [sdk, cli, policy, taint, i2, provenance, audit-dag, m7]

# Dependency graph
requires:
  - phase: 42-policy-layer
    provides: bind_policy (POLICY-03) trusted-source policy binder + F1 containment refusal
  - phase: 15-multi-field-extraction
    provides: mint_from_read sole broker taint-mint site + mint_from_intent trusted mint
provides:
  - "`caprun run <intent-kind> <intent-param> <workspace-file> [--policy <path>] [audit-db-path]` verb (bare-positional form preserved)"
  - "`--policy <path>` flag over the single bind_policy enforcement point (CAPRUN_POLICY is the fallback)"
  - "post-I2-Block operator loop: each Pending effect_id + sink + review/confirm/deny pointer surfaced on stdout"
  - "M7 structural anti-laundering: file-derived intent literals minted TAINTED via mint_from_read, disjoint from the trusted mint"
  - "read-only audit::list_pending_confirmations_for_session query"
  - "per-literal file-derived provenance flag threaded through ProvideIntent (proto + worker)"
affects: [45-02, 45-03, 45-04, audit-dag-viewer]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "per-literal provenance signal on an IPC message (ProvideIntent.primary_file_derived) driving disjoint broker mint routing"
    - "differential requirement proof driving the REAL production dispatch arm (not mint helpers) to prove routing"

key-files:
  created:
    - crates/brokerd/tests/s45_sdk_run_surface.rs
  modified:
    - cli/caprun/src/main.rs
    - cli/caprun/src/worker.rs
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/audit.rs

key-decisions:
  - "M7 signalled per-literal via a required (no serde-default) ProvideIntent.primary_file_derived bool, mirroring PlanNodeDecision.output_value_id's Pitfall-8 discipline"
  - "claim_type selected in the intent-variant match (email_address / relative_path), forcing any new variant to declare its file-derived taint shape"
  - "M7 differential drives the real ProvideIntent arm (dispatch_request) for both legs to prove routing, not just the mint helpers"

patterns-established:
  - "Pattern: a file/stream/env-derived intent literal is TAINTED at the broker's sole mint_from_read site, never laundered through mint_from_intent"
  - "Pattern: leading CLI flags (--seed-from-file, --policy) parsed in any order before positionals, behind an optional run verb"

requirements-completed: [SDK-01]

coverage:
  - id: D1
    description: "`caprun run` verb + --policy flag over the single bind_policy call; bare-positional form unchanged"
    requirement: "SDK-01"
    verification:
      - kind: unit
        ref: "cargo test -p caprun (full suite, incl. shipped e2e — passes no verb)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Post-I2-Block surfacing of each Pending effect_id + sink + review/confirm/deny pointer (read-only query)"
    requirement: "SDK-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#list_pending_confirmations_returns_only_pending_rows_for_session"
        status: pass
    human_judgment: false
  - id: D3
    description: "M7 anti-laundering: file-derived intent literal minted TAINTED via mint_from_read, I2-Blocks in sink arg; operator literal stays trusted/Allowed"
    requirement: "SDK-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/s45_sdk_run_surface.rs#m7_file_derived_literal_is_tainted_and_i2_blocks_with_genuine_anchor"
        status: pass
      - kind: integration
        ref: "crates/brokerd/tests/s45_sdk_run_surface.rs#m7_operator_literal_is_trusted_and_allowed"
        status: pass
      - kind: other
        ref: "bash scripts/check-invariants.sh (Gate 3: mint_from_read reused broker-side, no second site)"
        status: pass
    human_judgment: false

# Metrics
duration: ~45min
completed: 2026-07-18
status: complete
---

# Phase 45 Plan 01: Thin CLI/SDK Run Surface + M7 Anti-Laundering Summary

**`caprun run` verb with a `--policy` flag over the single bind_policy call, a post-I2-Block review/confirm/deny operator pointer, and the M7 disjointness guarantee — file-derived intent literals minted TAINTED via mint_from_read instead of laundered through the trusted intent mint.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-18
- **Tasks:** 3 of 3
- **Files modified:** 5 (1 created)

## Accomplishments
- **M7 anti-laundering (the #1 correctness item, a TCB change):** the ProvideIntent arm now mints a FILE-DERIVED primary literal (recipient/path) via the EXISTING broker-side `mint_from_read` — `email_address`→`[ExternalUntrusted, EmailRaw]` / `relative_path`→`[ExternalUntrusted, PathRaw]`, a genuine `file_read` event + session-demote, linear chain threaded via the mint's `chain_head`. Operator-typed literals stay on `mint_from_intent` (`UserTrusted`), DISJOINT. No second mint site — Gate 3 passes.
- `caprun run` verb added as a legible alias for the bare-positional intent-run; the bare-positional form (which the shipped e2e suite uses) is unchanged.
- `--policy <path>` flag threaded to the SAME single `bind_policy(Option<&Path>, workspace_root)` call `CAPRUN_POLICY` feeds (env is the fallback; neither → `broker_default()`); no second policy binder, F1 containment refusal reused verbatim.
- Post-run I2 Block now surfaces each Pending `effect_id` + `sink` + the three actionable `caprun review/confirm/deny <effect_id> <real-audit-db-path>` pointers via a read-only `pending_confirmations` query.

## Task Commits

1. **Task 1: `caprun run` verb + `--policy` flag (WG-6)** - `3ddfc4c` (feat)
2. **Task 2: Post-Block effect_id + review/confirm/deny surfacing (WG-5)** - `ed534c4` (feat)
3. **Task 3: M7 anti-laundering — file-derived literal minted TAINTED via mint_from_read (WG-1)** - `b3de4c4` (feat)

_Task 3 was TDD: RED verified by temporarily forcing the laundering path (file-derived leg failed with `[UserTrusted]`), GREEN with the mint routing in place — committed as the final GREEN state._

## Files Created/Modified
- `cli/caprun/src/main.rs` - `run` verb dispatch; `--policy`/`--seed-from-file` leading-flag loop; policy binding prefers the flag then env; post-Block surfacing block; forwards `PRIMARY_SEED_FILE_DERIVED` to the worker.
- `cli/caprun/src/worker.rs` - reads `PRIMARY_SEED_FILE_DERIVED` and sets `primary_file_derived` on ProvideIntent.
- `crates/brokerd/src/proto.rs` - required `primary_file_derived: bool` on `ProvideIntent` (no serde default, Pitfall-8).
- `crates/brokerd/src/server.rs` - ProvideIntent arm routes a file-derived primary literal through `mint_from_read`; operator literals through `mint_from_intent`; per-variant `primary_claim_type`. Updated 4 in-crate unit-test construction sites.
- `crates/brokerd/src/audit.rs` - read-only `list_pending_confirmations_for_session` + unit test.
- `crates/brokerd/tests/s45_sdk_run_surface.rs` - **created**; M7 differential driving the real ProvideIntent arm for both legs.
- Sibling integration tests updated for the new field: `replay_cas.rs`, `two_connection_intent_bypass.rs`, `proto_claims.rs`, `planner_capability_split.rs`.

## Decisions Made
- **M7 mechanism (pinned by the plan-checker BLOCKER):** the taint mint MUST be broker-side (Gate 3 restricts `mint_from_read(` to quarantine.rs + server.rs), so the file-derived case is minted in the ProvideIntent arm — never CLI-side, never a second mint site. Reuses `mint_from_read` verbatim.
- **Per-literal signal, not session status:** session status is per-session and cannot distinguish a file-derived recipient from an operator-typed one in an Active session; a required `primary_file_derived` bool on ProvideIntent carries the per-literal provenance.
- **Test drives the real arm:** the differential exercises `dispatch_request`'s actual ProvideIntent arm for both legs (not the mint helpers directly), so it proves the ARM's routing — verified non-vacuous by forcing the laundering path and observing the file-derived leg fail.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `cargo test -p caprun --lib` target does not exist**
- **Found during:** Task 1 verification
- **Issue:** The plan's Task 1 `<automated>` verify runs `cargo test -p caprun --lib`, but caprun is a binary-only crate with no library target, so `--lib` errors ("no library targets found").
- **Fix:** Ran the full `cargo test -p caprun` (the binary unit tests + integration tests, which is strictly broader and includes the shipped e2e suite). No source change.
- **Verification:** Full caprun suite green.
- **Committed in:** N/A (verification-command adjustment only, no code change).

---

**Total deviations:** 1 (a verify-command adjustment; no code/scope change).
**Impact on plan:** None — the broader test command fully covers the intended verification.

## Issues Encountered
None beyond the verify-command note above. All broker + caprun suites green.

## Verification Evidence
- `cargo build --workspace` — compiles (macOS host).
- `cargo test -p brokerd` — **266 lib tests + all integration tests pass, 0 failed** (incl. the new `s45_sdk_run_surface` 2/2 and the audit query unit test).
- `cargo test -p caprun` — full suite pass, 0 failed (shipped e2e suite, which passes no verb, stays green — SDK-01 "extends, does not replace" holds).
- `cargo test --workspace` — no failures across the workspace.
- `bash scripts/check-invariants.sh` — **all gates PASS**, including Gate 3 (mint_from_read reused broker-side, no second mint site), Gate 1 (no raw EffectRequest), Gate 5 (no new crate / no aws-lc-rs).
- **M7 differential non-vacuous:** forcing the laundering path (route file-derived through mint_from_intent) fails the file-derived leg with `[UserTrusted]`; restored to the mint_from_read routing → GREEN.
- **Linux legs:** none required — the M7 differential is host-portable (decision-level over an in-memory audit DB + ValueStore; the UnixStream::pair carries only the framed IPC response, as proto_claims.rs's dispatch tests do on both platforms). No `#[cfg(target_os="linux")]` leg was added, so `scripts/mailpit-verify.sh` is not needed for this plan's tests.

## Threat Register Outcome
- T-45-01 (Elevation, file-derived literal laundered as UserTrusted) — **mitigated** (Task 3, proven differentially).
- T-45-02 (Tampering, --policy beneath workspace) — **mitigated** (flag reuses the same bind_policy F1 refusal).
- T-45-03 (Repudiation, buried blocked effect_id) — **mitigated** (Task 2 read-only surfacing).
- T-45-04 (Tampering, TCB drift) — **mitigated** (no new crate, no new mint site, no raw EffectRequest; check-invariants green).

## Next Phase Readiness
- SDK-01 "define → point → run" half is complete. 45-02..04 (read-only audit-DAG viewer + remaining SDK surface) can build on the `run` verb, the `--policy` flag, and the operator-loop surfacing.
- No blockers.

## Self-Check: PASSED
- Created files exist: `crates/brokerd/tests/s45_sdk_run_surface.rs`, `45-01-SUMMARY.md`.
- Task commits exist: `3ddfc4c`, `ed534c4`, `b3de4c4`.

---
*Phase: 45-thin-cli-sdk-read-only-audit-dag-viewer*
*Completed: 2026-07-18*
