---
phase: 34-regression-live-proof-v1-7-done
plan: 03
subsystem: release-gate
tags: [orchestrator-owned, release-gate, cfg-linux, adversarial-review, tcb, exec-05, d-15, d-16]

requires:
  - phase: 34-01
    provides: invoke_process_exec_from_resolved — the confirm-release sink under review
  - phase: 34-02
    provides: async confirm() + process.exec Step-4.75 guard + Step-7 dispatch + main.rs async plumbing — the confirm-release TCB diff under review

provides:
  - "Gate A (D-15) recorded: Linux compile-check of every cfg(target_os=\"linux\") target is true-exit-0 (captured before pipe)"
  - "Gate B (D-16) recorded: fresh non-self Fable-5 adversarial code-trace of the confirm-release TCB diff — APPROVED after one CHANGES-REQUIRED round; all findings resolved"
  - "LIVE-01 (34-04) authorized: both blocking gates green"

affects: [34-04]

autonomous: false
gate: blocking
outcome: both-gates-green
requirements: [EXEC-05]
---

# 34-03 SUMMARY — EXEC-05 confirm-release release gates (ORCHESTRATOR-OWNED)

The two MANDATORY, orchestrator-owned gates between the EXEC-05 TCB slice
(34-01 + 34-02) and the composed live proof (34-04). Both are blocking; LIVE-01
was authorized only after both went green. Run in AUTO_MODE, but genuinely
executed (not rubber-stamped): the plan prohibits any gsd-executor from
self-adjudicating the adversarial trace, and the D-16 guardrail has repeatedly
caught real MAJORs green gates missed.

## Gate A — Linux compile-check of all cfg(linux) targets (D-15) ✓

Forces compilation of every `#[cfg(target_os = "linux")]` target (a green macOS
build compiles ZERO such targets — the project's standing cfg-linux blindness
gotcha), via `scripts/mailpit-verify.sh` in the unprivileged `rust:1` Colima
container, true exit code captured BEFORE any pipe (`set +e; …; rc=$?; set -e`).

**Initial run** (`MAILPIT_VERIFY_CMD='cargo build --tests --workspace --keep-going'`):
- **`true_exit=0`** (recorded `/tmp/34-03-compile-rc.txt`). Log tail: full
  workspace `--tests` build compiled clean in the container (`caprun-planner`,
  `caprun-exec-launcher`, sandbox lib-test, etc.), "Mailpit-backed Linux
  verification suite PASSED." Duration ~1m10s.

**Post-fix re-run** (after the Gate-B fixes changed TCB source —
`cargo build --workspace && cargo build --tests --workspace --keep-going &&
cargo test -p brokerd --lib confirmation && cargo test -p caprun --test s9_process_exec_block`):
- **`true_exit=0`** (recorded `/tmp/34-03-recheck-rc.txt`). brokerd confirmation
  lib tests **37/37**; `s9_process_exec_block` **6/6**; "Mailpit-backed Linux
  verification suite PASSED."

## Gate B — Fresh non-self Fable-5 adversarial code-trace of the confirm-release diff (D-16) ✓

A fresh-context Fable-5 agent (no chat history, no self-justification) was handed
ONLY the confirm-release diff (`git diff 28797ff..HEAD` of `process_exec.rs`,
`confirmation.rs`, `main.rs`, `s9_process_exec_block.rs`) and asked to break it.

### Round 1 verdict: **CHANGES-REQUIRED** (the guardrail's 8th real catch)

| # | Severity | Finding | Disposition |
|---|----------|---------|-------------|
| 1 | **MAJOR** | Burned one-shot confirmation with NO terminal audit event: `invoke_process_exec_from_resolved`'s pre-spawn `Err` legs (frozen `args` JSON parse, launcher-path resolution) `?`-propagated AFTER Step-5 `confirm_granted` + Step-6 CAS→Confirmed. Worker-reachable (`validate_schema` checks arg NAMES only). The exact P33 MAJOR-1 audit-gap class. | **FIXED** (commit `7b3c8ae`) |
| 2 | MINOR | New `mint_from_exec` site bypassed Gate 3 allow-list via inline `planner-discipline-allow` marker instead of same-commit extension. | **FIXED** (mint removed → no site → marker dropped; allow-list byte-identical) |
| 3 | MINOR | Released mint was dead ceremony (throwaway in-memory `ValueStore` dropped immediately) with a lying `Err → ConfirmedButSinkFailed` branch mislabeling an already-executed effect. | **FIXED** (mint removed; durable taint lives on `process_exited`) |
| 4 | NIT | Review bundle omitted `email_smtp_acceptance.rs` async conversion. | Verified correct out-of-band; process-only, no action |
| 5 | NIT | Allowed-path Err-context discard shape duplicated. | Pre-existing; left as-is (surgical changes) |

### Fixes (commits `7b3c8ae`, `3da3571`)

- Extracted `prepare_process_exec()` — all fallible pre-spawn prep, one source of
  truth used by BOTH the new confirm() **Step 4.8 precheck** (runs before the
  burn → fail-closed-**RECOVERABLE**, row stays Pending) AND the sink (folds
  pre-spawn into the same `Result` as the spawn, so EVERY failure appends a
  durable `process_spawn_failed` FIRST — defense-in-depth for the residual
  TOCTOU window). Precheck + dispatch cannot drift (same function, same input).
- Removed the dead mint; Step-7 arm maps sink success → `Released` directly.
- Regression test `confirm_on_process_exec_malformed_args_does_not_burn_confirmation`
  (runs on any platform; precheck fails at JSON parse pre-spawn). Proven to FAIL
  against pre-fix code on all three assertions (Err / Pending / no confirm_granted).
- Rewrote two stale TCB doc comments that described the removed mint.

### Round 2 verdict (fresh re-trace of the fixed diff): **APPROVED**

MAJOR verified closed on EVERY path via instruction-by-instruction enumeration,
**including the TOCTOU** (launcher deleted between precheck and sink): the sink's
`Err` branch appends `process_spawn_failed` on the `granted` head before the
error escapes. The only residual burn-without-terminal-event path requires an
audit-DB write to itself fail — logically unclosable and byte-identical to the
pre-existing file.create/file.write/email arms. Mint removal breaks nothing
(`mint_from_exec` sites = server.rs + quarantine.rs only; Gate 3 PASS). Regression
test confirmed real (fails pre-fix, passes post-fix, live-executed).

Re-trace surfaced ONE further MINOR — a stale doc comment on
`invoke_process_exec_from_resolved` still claiming a confirmation.rs mint site —
**FIXED** in commit `3da3571`. No unresolved MAJOR/CHANGES-REQUIRED finding
remains open.

## Gate outcome

Both blocking gates green (Gate A true-exit-0 ×2; Gate B APPROVED, all findings
resolved). **LIVE-01 (34-04) is authorized.**

## Self-Check: PASSED

- Gate A true-exit-0 captured before pipe (recorded, ×2 incl. post-fix). ✓
- Gate B fresh non-self adversarial trace APPROVED, all findings resolved/recorded. ✓
- macOS: all 4 invariant gates PASS (Gate 3 clean, confirmation.rs no longer a mint site); brokerd 37/37 confirmation lib tests. ✓
