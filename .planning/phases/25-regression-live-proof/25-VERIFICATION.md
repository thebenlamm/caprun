---
phase: 25-regression-live-proof
verified: 2026-07-12T13:45:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/4
  gaps_closed:
    - "Human confirms the v1.5 DONE milestone-close (blocking checkpoint, 25-03-PLAN.md Task 2) — now durably recorded in 25-03-SUMMARY.md 'Human sign-off — OBTAINED' (explicit human 'approved', 2026-07-12)"
  gaps_remaining: []
  regressions: []
  notes:
    - "Sign-off was genuinely obtained in-session at the checkpoint but recorded to the repo AFTER the initial verification run — a recording lag, not a missing approval. The record now exists at 25-03-SUMMARY.md line 83."
    - "REQUIREMENTS.md traceability still shows T2-06/T2-08 as 'Pending' due to a known write-path lag (25-01 worktree and 25-03 orchestrator-run never wrote REQUIREMENTS.md; only 25-02 did). This is reconciled by `gsd-tools query phase.complete 25`, which is gated on this passing verdict. It is a downstream tracking sync, not an engineering gap — verifier does not edit REQUIREMENTS.md directly."
---

# Phase 25: Regression & Live Proof Verification Report

**Phase Goal:** The T2 gap is demonstrably closed on real Linux — a deliberately swapped
subject/recipient handle pair produces the new deny with an unbroken audited chain, existing
tests no longer rely on permissive `UserTrusted`-in-any-slot behavior, and the full workspace
regression is independently re-verified green (the v1.5 DONE gate).

**Verified:** 2026-07-12T13:45:00Z
**Status:** passed
**Re-verification:** Yes — after closure of the single milestone-close sign-off blocker

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A held-out acceptance test proves a plan node with a deliberately swapped subject↔recipient handle pair (both `UserTrusted`) produces the new deny, with a corresponding audit-DAG event and `verify_chain` true (T2-06) | VERIFIED | `crates/brokerd/tests/s9_acceptance.rs` fns `slot_type_binding_swapped_subject_recipient_denies` (line 556) + `slot_type_binding_correctly_routed_allows` (line 700). Both mint via `mint_from_intent` with genuine `origin_role` tags, causally chained (recipient mint's parent = subject mint's event id). The swapped test asserts `ExecutorDecision::Denied{DenyReason::SlotTypeMismatch{sink,arg,expected,found}}` via a `match` arm (never hand-constructed), with `sink=="email.send"`, `arg=="to"`, `expected==["recipient","email_address"]`, `found==Some("subject")` — matches `sink_sensitivity.rs::expected_role` exactly (lines 147-162). Hand-mirrors `plan_node_evaluated` event append parented on the recipient mint's event id, then asserts `find_event_by_type(...).parent_id == Some(recipient_event_id)` and `verify_chain(...) == true`. The control test reuses the same two minted values routed correctly and asserts `Allowed`. Ran locally: `cargo test -p brokerd --test s9_acceptance slot_type_binding` → **2 passed, 0 failed**. |
| 2 | Existing tests relying on permissive `UserTrusted`-in-any-slot behavior are identified via a regression audit and confirmed not silently bypassed by a role-less fixture (T2-07) | VERIFIED | `.planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md` independently re-runs both search commands (22 Linux-gated files, 0 with direct-mint sites; 42 raw `.mint(`/`ValueRecord {` hits across 9 files), classifies every hit, cross-references all 31 role-checked-slot sites against `expected_role`, records **31/31 CORRECT, 0 NEEDS-FIX** explicitly, and closes with a "Reconciliation" section stating exit 0 / 46 test-result-ok blocks / 269 passed, 0 failed, and a final **"T2-07 verdict: PASS."** |
| 3 | `scripts/mailpit-verify.sh` is independently re-run green (0 failures) after the change lands — not assumed from a prior pass (T2-08) | VERIFIED (as recorded) | `.planning/phases/25-regression-live-proof/25-03-SUMMARY.md` records: bare recipe (no `MAILPIT_VERIFY_CMD` override), `RESULT=0` captured on the line immediately after the invocation (before any pipe), verbatim sentinel `Mailpit-backed Linux verification suite PASSED.`, 0 `test result: FAILED`, 46 `test result: ok` blocks, and the held-out test name `slot_type_binding_swapped_subject_recipient_denies ... ok` present in the Linux log (line 650), plus the control test also green (line 649). This is a recorded live run I cannot re-execute (no Docker/Colima in this verification environment); the record is internally consistent with the plan's exact assertion contract and shows no signs of a piped-exit-code shortcut. |
| 4 | A human confirms the v1.5 DONE milestone-close — not assumed from a prior/scoped pass (T2-08, PLAN 03 must-have truth; the phase goal's own "(the v1.5 DONE gate)" clause) | VERIFIED | `.planning/phases/25-regression-live-proof/25-03-SUMMARY.md` line 83, "Human sign-off — OBTAINED": the full Task-1 evidence (all four assertions PASS) was presented to the human at the blocking `autonomous:false` checkpoint, who gave explicit **"approved"** on **2026-07-12**. Durably recorded. (Recorded after the initial verification run — a recording lag, not a missing approval; see re_verification notes.) |

**Score:** 4/4 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/brokerd/tests/s9_acceptance.rs::slot_type_binding_swapped_subject_recipient_denies` | held-out swapped-deny proof | VERIFIED | Exists, substantive, passes (`cargo test` run confirmed) |
| `crates/brokerd/tests/s9_acceptance.rs::slot_type_binding_correctly_routed_allows` | isolation control | VERIFIED | Exists, substantive, passes |
| `.planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md` | per-site catalog + verdict | VERIFIED | Exists, complete catalog, explicit NEEDS-FIX=0, Reconciliation section, final PASS verdict |
| `.planning/phases/25-regression-live-proof/25-03-SUMMARY.md` | live Linux gate evidence + sign-off | VERIFIED | Records all required evidence fields AND the now-obtained human milestone-close sign-off |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `mint_from_intent` (genuine origin_role) | `submit_plan_node` (Step 1c) | direct call in test body | WIRED | Both new tests call the real production fns; no `DenyReason` hand-construction found in either test |
| Hand-mirrored `append_event(plan_node_evaluated)` | `find_event_by_type` + `verify_chain` | in-test assertions | WIRED | Confirmed lines ~648-672 of s9_acceptance.rs |
| Non-cfg-gated `s9_acceptance.rs` | `scripts/mailpit-verify.sh`'s `cargo test --workspace` | Linux container run | WIRED (per recorded log) | 25-03-SUMMARY.md quotes the held-out test's name appearing in the Linux log at line 650 |
| 25-03-PLAN.md Task 2 (blocking human-verify checkpoint) | phase-complete mark in ROADMAP.md | required gate before completion | WIRED | Human "approved" recorded in 25-03-SUMMARY.md; the ROADMAP "Complete" mark follows a genuine sign-off, not a self-approval |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| T2-06 | 25-01-PLAN.md | Held-out swapped-handle deny test, genuine chain, audit event, verify_chain true | SATISFIED | Test code verified above (ran green). REQUIREMENTS.md traceability row still reads "Pending" pending `phase.complete 25` sync — a known write-path lag, not an engineering gap. |
| T2-07 | 25-02-PLAN.md | Regression audit, no silent bypass | SATISFIED | REQUIREMENTS.md traceability table already shows "Complete" (25-02 wrote it) |
| T2-08 | 25-03-PLAN.md | Independent Linux re-run + human sign-off | SATISFIED | Live-run evidence recorded + human sign-off obtained. REQUIREMENTS.md row still "Pending" pending `phase.complete 25` sync. |

No orphaned requirements — T2-06/T2-07/T2-08 are the full requirement set for this phase and all three are satisfied. REQUIREMENTS.md's traceability rows for T2-06/T2-08 will be reconciled to "Complete" by `gsd-tools query phase.complete 25` (gated on this passing verdict); the verifier does not edit REQUIREMENTS.md directly.

### Anti-Patterns Found

None. No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/placeholder markers in `s9_acceptance.rs` or `25-REGRESSION-AUDIT.md`.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| T2-06 held-out tests pass on Mac | `cargo test -p brokerd --test s9_acceptance slot_type_binding` | `2 passed, 0 failed` | PASS |
| No `DenyReason` hand-construction | manual read of test bodies | `match` arm only, no literal construction | PASS |
| `expected_role` table matches test assertions | `crates/executor/src/sink_sensitivity.rs:147-162` vs. test assertions | Exact match (`to`→`[recipient,email_address]`) | PASS |
| Real-Linux run of held-out test | (cannot re-run Docker/Colima in this environment) | N/A | SKIPPED — relying on recorded 25-03-SUMMARY.md evidence, internally consistent |

### Gaps Summary

No remaining gaps. All three engineering criteria were independently confirmed against real code
(the T2-06 held-out test genuinely drives the production `mint_from_intent → submit_plan_node`
path with a passing local run; the T2-07 regression audit is a real independent re-sweep with an
explicit 0-NEEDS-FIX verdict; the T2-08 Linux run's recorded evidence satisfies its own
assertion contract). The sole prior blocker — a missing durable record of the blocking human
milestone-close sign-off — is closed: 25-03-SUMMARY.md now records an explicit human "approved"
(2026-07-12) against the presented four-assertion evidence. The approval was genuinely obtained
at the checkpoint and written to the repo after the initial verification run (a recording lag).
The REQUIREMENTS.md traceability rows for T2-06/T2-08 remain "Pending" pending the downstream
`phase.complete 25` sync that this passing verdict unblocks — a tracking-sync step, not an
engineering gap. Phase goal achieved; the v1.5 DONE gate is met.

---

_Verified: 2026-07-12T13:45:00Z (re-verification after gap closure)_
_Verifier: Claude (gsd-verifier)_
