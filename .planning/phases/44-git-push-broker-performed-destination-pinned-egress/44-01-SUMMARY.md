---
phase: 44-git-push-broker-performed-destination-pinned-egress
plan: 01
subsystem: executor-tcb
tags: [git-push, sink-registration, i2, effect-ontology, policy]
requires:
  - executor sink_schema / sink_sensitivity registration pattern (Phase 43 http.request.write)
  - runtime-core policy PRODUCTION_SINKS allowlist
provides:
  - git.push registered as a CommitIrreversible, routing-sensitive {remote,refspec} sink in the executor TCB
  - IrreversibleEffect::GitPush ontology reconciled to `refspec` (WG-4)
  - git.push on the broker_default()/allow_all() production allowlist
affects:
  - Plan 44-02 (broker transfer layer: validate_git_refspec value-gate, --force/deletion refusal)
  - Plan 44-04 (confirm-time payload-provenance surface for pushed pack content)
tech-stack:
  added: []
  patterns:
    - explicit-arm effect-class registration (fail-closed `_ =>` backstop retained)
    - routing-sensitive-only sink (no content-sensitive arg; pack content surfaced at confirm)
key-files:
  created: []
  modified:
    - crates/executor/src/sink_schema.rs
    - crates/executor/src/sink_sensitivity.rs
    - crates/runtime-core/src/effect.rs
    - crates/runtime-core/src/policy.rs
    - crates/llm-planner/src/lib.rs
    - crates/runtime-core/tests/task2_types.rs
decisions:
  - "GitPush ontology field renamed branch -> refspec (WG-4) rather than mapping refspec-dst->branch: one identical name across effect ontology, sink schema, and sensitivity tables removes the desync-able field (T-44-04)."
  - "git.push has NO content-sensitive arg: pushed PACK content is worker-controlled and surfaced at confirm (DESIGN §1.6), not gated as an I2 content arg. is_content_sensitive falls through to the fail-safe `_ => false` default (no over-widening)."
  - "expected_role(git.push, remote|refspec) = None: no origin_role mint site for a legit trusted-intent push; Some(..) would fail-closed-Deny the legit flow. The Block comes from routing-sensitivity + taint, not the role gate."
  - "llm-planner UnknownSink test left behaviorally unchanged (WG-5): executor registration does not teach the stub planner; a real planner-learns-git.push change is Out-of-Scope for v1.9 (v2)."
metrics:
  duration: ~15m
  completed: 2026-07-18
status: complete
---

# Phase 44 Plan 01: Register git.push in the executor TCB Summary

Registered `git.push` as a distinct `CommitIrreversible`, routing-sensitive `{remote, refspec}` sink in the deterministic executor TCB — schema gate, effect-class, taint-sensitivity tables, and production-policy allowlist — mirroring the shipped Phase 43 `http.request.write` registration, and reconciled the `IrreversibleEffect::GitPush` ontology field to `refspec` (WG-4) plus the `PRODUCTION_SINKS` / llm-planner `UnknownSink` couplings (WG-5). No broker network path here: this makes the executor classify and gate `git.push` so I0/I1/I2 fire for free.

## What was built

**Task 1 — KNOWN_SINKS row + GitPush ontology reconcile (WG-4)** (commit `36e4fb4`)
- Added an exact-match, both-required `git.push` row `{remote, refspec}` to `KNOWN_SINKS` (`sink_schema.rs`), mirroring `file.write`/`git.commit`. Row comment documents: both args I2 routing-sensitive (a tainted value Blocks downstream, does NOT Deny at Step 0), from TRUSTED intent, and the `--force`/deletion/`+`-refspec structural refusal is a broker transfer-layer VALUE gate (Plan 44-02), not this name-set gate.
- Renamed `IrreversibleEffect::GitPush { remote, branch }` → `{ remote, refspec }` in `effect.rs` so the effect ontology keys on the same name as the sink args + DESIGN §1.3 (T-44-04), and updated the sole constructor in `tests/task2_types.rs`.
- Added mirrored schema unit tests (registered / exact-args / missing-remote / missing-refspec / unknown-arg / duplicate).

**Task 2 — CommitIrreversible + routing-sensitive remote/refspec** (commit `4af79b3`)
- Added an EXPLICIT `"git.push" => EffectClass::CommitIrreversible` arm (redundantly backstopped by the `_ =>` fail-closed default) so a draft/untrusted-seeded session I0-denies a push, never an Observe fall-through (T-44-01).
- Added `GIT_PUSH_ROUTING_SENSITIVE = &["remote","refspec"]` + the `is_routing_sensitive` arm (T-44-02). No content-sensitive arm (default `_ => false`). `expected_role` arm returns `None` for both args.
- Added mirrored unit tests including an explicit-arm `git_push_is_commit_irreversible` assertion.

**Task 3 — production-policy allowlist + llm-planner coupling (WG-5)** (commit `cc56b78`)
- Added `"git.push"` to `PRODUCTION_SINKS` so `broker_default()`/`allow_all()` permit it (T-44-03); fixed the stale prose sink counts (`eight`/`seven` → `nine`) across the three affected comments.
- Flipped the pre-Phase-44 negative `!permits_sink("git.push")` assertion to positive; added a dedicated `broker_default_permits_git_push` test; retargeted the deny-unlisted example to a genuinely-unregistered id (`deploy.service`); renamed the count-bound test `...eight...` → `...nine...`.
- Made the llm-planner `git.push => UnknownSink` test intent explicit (WG-5) with a premise guard asserting the stub planner is not offered `git.push`; behavior unchanged.

## Verification

- `cargo build --workspace` — clean.
- `cargo test -p executor --no-fail-fast` — 143 passed / 0 failed (110 lib + 29 + 4).
- `cargo test -p runtime-core --no-fail-fast` — 54 passed / 0 failed (22 + 12 + 7 + 13).
- `cargo test -p llm-planner --no-fail-fast` — 14 passed / 0 failed.
- `./scripts/check-invariants.sh` — all gates PASSED (exit 0): no raw effect-to-sink type (Gate 1), no new mint site (Gate 3), no new crate/aws-lc-rs (Gate 5), containment anti-drift (Gate 6).

Note (per CLAUDE.md): all `#[cfg(target_os="linux")]` enforcement/e2e tests compile to no-ops on the macOS host — expected, not a gap. This plan touches only pure classification/policy tables (no sink dispatch, no confined path), so there is no Linux-only surface introduced here.

## Deviations from Plan

**1. [Rule 1 — accuracy] Fixed two additional stale prose counts beyond policy.rs:209.**
- **Found during:** Task 3.
- **Issue:** The plan (and hard constraint M3) named the `policy.rs:209` "seven real production sinks" prose comment. Two sibling prose counts described the SAME `PRODUCTION_SINKS` slice and were also stale after adding `git.push`: the doc header ("The eight currently-callable production sinks") and the `broker_default()` doc ("the seven currently-callable production sinks").
- **Fix:** Updated all three to `nine` so no comment describing the slice remains inaccurate. No numeric assert depends on any of them (prose-only). Surgical: same count, same slice.
- **Files modified:** `crates/runtime-core/src/policy.rs`.
- **Commit:** `cc56b78`.

**2. [Rule 1 — test correctness] Retargeted the deny-unlisted assertion to a new unlisted id.**
- **Found during:** Task 3.
- **Issue:** The count-bound test used `git.push` as its "future/unknown sink is NOT callable" example. Adding `git.push` to the allowlist makes it a permitted sink, so it can no longer serve as the unlisted example without deleting the deny-by-default coverage.
- **Fix:** Moved `git.push` into the permitted loop and used a genuinely-unregistered id (`deploy.service`) for the deny-unlisted assertion, preserving the test's fail-closed intent.
- **Files modified:** `crates/runtime-core/src/policy.rs`.
- **Commit:** `cc56b78`.

Both are direct consequences of the plan's own required allowlist change (Rule 1 scope), not new scope. All other work matches the plan exactly.

## Known Stubs

None. This plan wires real classification/policy tables consumed by the shipped I0/I1/I2 executor loop; no placeholder data or unwired surface.

## Self-Check: PASSED

- `crates/executor/src/sink_schema.rs` — FOUND (git.push row + tests).
- `crates/executor/src/sink_sensitivity.rs` — FOUND (explicit arm + routing const + tests).
- `crates/runtime-core/src/effect.rs` — FOUND (GitPush.refspec).
- `crates/runtime-core/src/policy.rs` — FOUND (PRODUCTION_SINKS + flipped tests).
- `crates/llm-planner/src/lib.rs` — FOUND (WG-5 intent + guard).
- Commits `36e4fb4`, `4af79b3`, `cc56b78` — all FOUND in `git log`.
