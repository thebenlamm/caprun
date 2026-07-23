---
phase: 31-effect-breadth-design-gate
verified: 2026-07-17T21:15:00Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 31: Effect-Breadth Design Gate Verification Report

**Phase Goal:** A reviewed DESIGN doc pins the broker-spawned confined-child-`exec` model and the filesystem read/write-breadth model, and clears a fresh non-self adversarial code-trace — hard-blocking every subsequent TCB-code phase.
**Verified:** 2026-07-17
**Status:** passed
**Re-verification:** No — initial verification

This is a doc-only design-gate phase (CLAUDE.md hard constraint: two design-gate docs block executor code). Verification target is document existence, document content, gate clearance, and absence of TCB code — not `cargo test` of new features, since no runnable code is produced this phase.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `DESIGN-effect-breadth-exec.md` pins BOTH the broker-spawned confined-child `process.exec` model AND the fs read/write-breadth model | VERIFIED | §1 (78-287) pins spawn ownership, Landlock/seccomp/rlimits/timeout/byte-cap confinement, argv-array command schema; §2 (290-409) pins the sole `mint_from_exec` mint site; §3 (412-488) pins multi-file read (N × existing path) and `O_WRONLY\|O_TRUNC` write/edit. All code citations (seccomp.rs:64-66, landlock.rs:17-32, quarantine.rs mint_from_read shape, rlimits.rs, main.rs:311-378, workspace.rs create_exclusive_within) independently spot-checked against live code by this verifier — every citation matches exactly. |
| 2 | The doc pins fail-closed defaults for both new sinks (§5 table); nothing disables/bypasses I2; no new raw EffectRequest path | VERIFIED | §5 (587-599) is a complete per-arg default-posture table. §4.4 states both sinks stay on `PlanNode{sink,args}`; `check-invariants.sh` Gate 1 (EffectRequest-token absence) re-run by this verifier: PASS, zero hits. |
| 3 | `process.exec` command/args classified routing- AND content-sensitive (not `expected_role=None` in the naive/unconstrained sense); Gate 3 extension for `mint_from_exec(` mandated | VERIFIED | §4.2 (509-547) pins `is_routing_sensitive`/`is_content_sensitive = true` for both `command` and `args` — confirmed against `crates/executor/src/lib.rs:133-158` and `sink_sensitivity.rs`: the Block is delivered by the sensitivity+taint check (independent of `expected_role`); `expected_role=None` here only disables the separate structural role-gate, not the I2 Block — this is the Round-1 M2 finding, correctly resolved, not a bypass. §2.4 mandates the Gate 3 extension for `mint_from_exec(`; verified live `check-invariants.sh` Gate 3 today greps only `mint_from_read(`/`mint_from_derivation(`/`.mint(` — the doc's claim it would NOT catch a new token is accurate. |
| 4 | `DESIGN-GATE-RECORD-v1.7.md` exists, gate CLEARED, distinct non-self Fable-5 reviewer named, files-opened listed, revision-history table, per-finding claim/evidence/resolution, no-TCB-code reconfirmation; §11 Amendments records the review outcome | VERIFIED | File exists (215 lines). Gate status `✅ CLEARED (2026-07-17, Round 1)`. Reviewer identity section names "Claude Fable 5" as author≠reviewer, lists 11 files opened + targeted greps, 124,554 tokens/17 tool uses. Revision History table present. 6 findings documented (B1 BLOCKER, M3/M1/M2 MAJOR, m1 MINOR, n1 NIT), each with Claim/Code evidence/Resolution, all resolved by design-tightening (not weakening). §11 "Amendments (post-review)" in the DESIGN doc mirrors all 6. Verdict: CLEARED, no open blocker. |
| 5 | No TCB code written this phase | VERIFIED | `git status --porcelain crates/ cli/` empty. `git diff --stat v1.6..HEAD -- crates/ cli/` empty (zero lines changed since the v1.6 tag). `bash scripts/check-invariants.sh` exits 0 (all 4 gates PASS), re-run live by this verifier. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `planning-docs/DESIGN-effect-breadth-exec.md` | §0-§11, pins both models + fail-closed defaults | VERIFIED | 883 lines, all 12 sections (§0-§11) present with required content; not a stub — every code citation spot-checked matches live code exactly. |
| `planning-docs/DESIGN-GATE-RECORD-v1.7.md` | Gate CLEARED with reviewer independence, findings, no-TCB-code reconfirmation | VERIFIED | 215 lines, all required sections present, gate CLEARED. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| DESIGN doc §2/§4/§6 claims | live crates/ code | file:line citations | WIRED | Spot-checked citations: `seccomp.rs:62-66` (execve deny), `landlock.rs:17-32` (`deny_all_filesystem`), `quarantine.rs:301-390` (`mint_from_read` non-stapled shape), `lib.rs:133-158` (role-gate vs sensitivity+taint independence), `sink_sensitivity.rs:40-181` (tables), `workspace.rs:108-151` (`O_CREAT\|O_EXCL` vs pinned `O_WRONLY\|O_TRUNC`), `rlimits.rs` (`RLIMIT_CPU` is CPU-seconds not wall-clock), `main.rs:305-378` (planner sidecar + worker spawn + `child.kill()` teardown), `check-invariants.sh:24-38,50-141` (Gate 1 / Gate 3 loci) — all confirmed accurate against the actual repo, not fabricated. |
| Round-1 findings (B1/M3/M1/M2/m1/n1) | DESIGN doc §11 amendments | resolution ledger | WIRED | Every gate-record finding has a matching §11 amendment entry in the DESIGN doc; sections cited (§1.4/§1.3/§2.4/§4.2/§6/§3.1) all contain the described post-amendment text (verified by direct read, e.g. §1.3 pins Option B, §4.2 pins `expected_role=None` with the sensitivity+taint rationale). |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| DESIGN-13 | 31-01, 31-02 | DESIGN doc pins both models + clears fresh non-self adversarial code-trace | SATISFIED | Doc exists with both models pinned (§1-§3); gate record shows CLEARED with a distinct Fable-5 code-trace reviewer. |
| DESIGN-14 | 31-01, 31-02 | Doc pins fail-closed defaults, consistent with I0/I1/I2, no I2 bypass, no raw EffectRequest path | SATISFIED | §4-§5 pin the table; §6 checklist asserts no bypass; Gate 1 confirms zero `EffectRequest` tokens. |

Note: `.planning/REQUIREMENTS.md` still shows `- [ ]` unchecked boxes and status "Pending" for DESIGN-13/DESIGN-14 (line 27-34, 123-124). This is expected pre-close state per this project's standing convention — checkbox/status-table flips happen at the phase-completion/record step after verification passes, not during plan execution (see project memory: executors are deliberately kept from touching ROADMAP/STATE.md). Not treated as a gap; flagged here for the orchestrator's phase-completion step to reconcile.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `planning-docs/DESIGN-effect-breadth-exec.md` | 484, 672, 674 | `TBD Phase 32`/`TBD Phase 33` (sink id literal, test-file names) | ℹ️ Info | Each instance references a named, formal follow-up phase (32/33) and is explicitly categorized by the doc itself as a "deployment constant, not a model gap" (§8). Consistent with this being a design-gate doc that deliberately pins the model while deferring implementation-time literals. Not an unaudited debt marker — does not block gate clearance. |

No `FIXME`/`XXX`/`HACK`/`PLACEHOLDER` markers found. No stub patterns, empty implementations, or hardcoded-empty-data patterns found (not applicable — no code produced this phase).

### Behavioral Spot-Checks

Step 7b: SKIPPED (no runnable entry points — doc-only phase, no code produced).

### Probe Execution

Not applicable — no probes declared or implied for this doc-only design-gate phase.

### Human Verification Required

None. Gate clearance (design soundness) is itself the human-judgment security call already recorded and evidenced in `DESIGN-GATE-RECORD-v1.7.md` via a fresh, non-self, code-tracing adversarial review with independently re-verified findings — per the phase's own stated verification contract, this constitutes the verifiable artifact rather than an open human-verification item.

### Gaps Summary

None. All 5 must-haves verified against the actual repository (not SUMMARY.md claims): the DESIGN doc exists and pins both effect-primitive models with fail-closed defaults; every code citation checked by this verifier traces accurately to live code; `process.exec` command/args are genuinely sensitivity-classified (not silently unconstrained); the mandated Gate 3 extension is explicit; the gate record documents a genuine non-self adversarial code-trace (11 files opened, 6 findings, all resolved by design-tightening, none by weakening an invariant) and reads CLEARED; and no TCB code exists under `crates/` or `cli/` since the v1.6 tag, with `check-invariants.sh` green.

---

_Verified: 2026-07-17_
_Verifier: Claude (gsd-verifier)_
