---
phase: 26-security-hardening-design-gate
verified: 2026-07-12T18:00:00Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 26: Security Hardening Design Gate — Verification Report

**Phase Goal:** A DESIGN doc (`planning-docs/DESIGN-security-hardening.md`) specifies the mechanism +
fail-closed default for all five hardening residuals AND clears a fresh (non-self) adversarial review
before any `crates/executor`, `crates/brokerd`, or `crates/runtime-core` hardening code is written.

**Verified:** 2026-07-12
**Status:** passed
**Re-verification:** No — initial verification

This is a DOC + REVIEW gate. There is intentionally no source code produced by this phase; "0
passed"/absence of code under `crates/` is a **success criterion**, not a gap. All claims below were
checked against live source, not accepted from SUMMARY.md prose.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | DESIGN doc pins mechanism + fail-closed default for all 5 residuals (§a–§e), each grounded in a real file:line | ✓ VERIFIED | `planning-docs/DESIGN-security-hardening.md` §a (server.rs:996-1047 RequestFd arm — anchors confirmed against live `crates/brokerd/src/server.rs`), §b (audit.rs schema/verify_chain, HMAC mechanism + broker-enforced key-path fail-closed startup refusal), §c (server.rs:562 effect_id mint, sent_plan_nodes CAS table, PK-violation-as-signal), §d (server.rs:904-994 CreateSession gate, `test-fixtures` Cargo feature mirroring real `crates/executor/Cargo.toml:22/28` precedent — confirmed), §e (`crates/executor/src/sink_sensitivity.rs:157` `"contents" => None` confirmed live-unchanged, pinned target `Some(&["path"])`). Each §has an explicit "Fail-closed default" subsection. |
| 2 | Doc pins the 3 cross-cutting rulings (X-01/X-02/X-03) as ONE uniform rule each, and rules explicitly on X-04 | ✓ VERIFIED | §f (lines 605-730): X-01 label continuity (reuses §a's `is_trusted_labeled`, fail-closed-by-default for broker-written files), X-02 reframed to the confirm/deny process + `pending_confirmations` (MAC-re-verified, fail-closed), X-03 restates authorize-before-effect (already live at `server.rs:709-751`), X-04 explicitly RULED "fold into HARDEN-01" (not silently inherited) with anchors verified against `server.rs:149,185,202,231,1131`. |
| 3 | GATE-RECORD exists: fresh non-self reviewer, F1/F2/F3 findings each resolved, Amendments section folds them, Status line reads CLEARED | ✓ VERIFIED | `planning-docs/DESIGN-GATE-RECORD-v1.6.md` names reviewer as a distinct `Agent(model:"fable")` (Claude Fable 5), states author=Opus subagent / reviewer=separate agent, lists F1 (BLOCKER, §b key custody), F2 (MAJOR, §a trusted-label compare), F3 (MINOR, §f/X-04 monotonic write) with code evidence + resolution each. `DESIGN-security-hardening.md`'s "## Amendments (post-review)" section (line 868) folds all three as Round-1 blockquotes, cross-referenced at each affected §(§a line 120, §b line 255, §f/X-04 line 711). Doc `Status:` line (line 5) reads `CLEARED — passed a fresh (non-self) adversarial review`. |
| 4 | NO hardening code exists in `crates/executor`, `crates/brokerd`, or `crates/runtime-core`; mechanism symbols absent; anchors unchanged; invariants gate green | ✓ VERIFIED | `git status --porcelain crates/ cli/` → empty. `git diff --stat b39891a..HEAD -- crates/ cli/` → empty (0 files touched across all Phase 26 commits; only `planning-docs/` and `.planning/` changed). `grep -rn "Hmac\|chain_anchor\|sent_plan_nodes\|is_trusted_labeled" crates/ cli/` → 0 hits. `crates/executor/src/sink_sensitivity.rs:157` still reads `"contents" => None,` (unchanged). `crates/brokerd/Cargo.toml` has no `[features]` section (confirmed by direct read). `bash scripts/check-invariants.sh` → exit 0, all 3 gates PASS. |
| 5 | D-02 amendment to `DESIGN-session-trust-state.md` names RequestFd as a second broker-side I1 demotion site, marked as landing in Phase 27 | ✓ VERIFIED | `planning-docs/DESIGN-session-trust-state.md:92-108` — amendment block explicitly permits a second broker-only demotion site (`RequestFd` entry), states "The code lands in Phase 27," and notes it "does not describe current shipped behavior (as of v1.5, `mint_from_read` remains the only demotion site in code)." |

**Score:** 5/5 truths verified (0 present-behavior-unverified — this is a doc/review gate with no runtime
state transitions to exercise)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `planning-docs/DESIGN-security-hardening.md` | Mechanism + fail-closed default doc, all 5 residuals + cross-cutting rulings | ✓ VERIFIED | 894 lines, §0/§a-§e/§f/§g/§h/§i/§j/Acceptance Predicate/Amendments all present and substantive; every file:line anchor spot-checked against live source resolved correctly |
| `planning-docs/DESIGN-GATE-RECORD-v1.6.md` | Fresh non-self review record | ✓ VERIFIED | 171 lines; reviewer identity, revision history, 3 findings w/ resolutions, "verified as sound" list, no-TCB-code reconfirmation, CLEARED verdict |
| `planning-docs/DESIGN-session-trust-state.md` (D-02 amendment) | Amendment naming RequestFd as 2nd demotion site | ✓ VERIFIED | Lines 92-108, correctly scoped as a forward design decision (Phase 27), not current behavior |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| DESIGN doc §a-§e anchors | Live source (`server.rs`, `audit.rs`, `sink_sensitivity.rs`, `Cargo.toml`s) | file:line citations | ✓ WIRED | Spot-checked `server.rs:145-235` (X-04 anchors), `server.rs:990-1050` (RequestFd/§a anchor), `sink_sensitivity.rs:150-160` (§e anchor), `brokerd/Cargo.toml` (§d anchor) — all match cited line numbers and content exactly |
| GATE-RECORD findings | DESIGN doc Amendments section | round-tagged blockquotes | ✓ WIRED | F1→§b (line 255), F2→§a (line 120), F3→§f/X-04 (line 711) all present with matching content |
| DESIGN-session-trust-state.md D-02 | DESIGN-security-hardening.md §a | cross-reference | ✓ WIRED | D-02 amendment explicitly cites `DESIGN-security-hardening.md §a, cleared DESIGN-GATE-RECORD-v1.6.md` |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `planning-docs/DESIGN-security-hardening.md` | 540 | quoted mention of "placeholder reuse" | ℹ️ Info | Describes existing (pre-phase-26) code behavior being analyzed, not a debt marker in the doc itself — not a blocker |

No TBD/FIXME/XXX/HACK debt markers found in any phase-26-authored file. No empty implementations
(this is a documentation-only phase; the "code" grep patterns for stubs do not apply).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|--------------|--------|----------|
| DESIGN-11 | 26-01 | DESIGN doc pins mechanism + fail-closed default, all 5 residuals | ✓ SATISFIED | Doc content verified above (Truth #1) |
| DESIGN-12 | 26-02 | Doc clears fresh non-self adversarial review, all findings resolved | ✓ SATISFIED | GATE-RECORD + Amendments verified above (Truth #3) |

**Process note (not a goal-achievement gap):** `.planning/REQUIREMENTS.md`'s traceability table still
shows `DESIGN-12 | Phase 26 | Pending` and the checkbox unchecked, and `.planning/ROADMAP.md`'s Phase 26
/ plan checkboxes are still unchecked, and `.planning/STATE.md` still shows `status: executing` /
`stopped_at: Phase 26 context gathered`. This is bookkeeping lag, not a substantive gap — the actual
DESIGN doc, GATE-RECORD, and 26-02-SUMMARY.md (`"DESIGN-12: satisfied. Gate CLEARED"`) unambiguously
confirm completion. This project's own memory log documents this exact pattern (v1.5 Phase 25): checkbox
reconciliation happens via the phase-completion step, not required for the verifier to pass. Flagging so
the orchestrator reconciles REQUIREMENTS.md/ROADMAP.md/STATE.md when this phase is marked complete.

No orphaned requirements — REQUIREMENTS.md maps only DESIGN-11/DESIGN-12 to Phase 26, both claimed by
plans 26-01/26-02 respectively.

### Human Verification Required

None. This is a doc + review gate; the "fresh, non-self adversarial review" IS the phase's verification
mechanism (by design, mirroring v1.0 P2/v1.2 P8/v1.3 P12/v1.4 P18/v1.5 P23), and it has already run and
cleared. No runtime behavior exists in this phase to spot-check (Step 7b: SKIPPED — no runnable entry
points; this phase produces documentation only).

### Gaps Summary

None. All 5 roadmap success criteria verified against live source (not SUMMARY.md prose):

1. DESIGN doc exists, pins mechanism + fail-closed default for all 5 residuals, anchors verified accurate.
2. Cross-cutting rulings X-01/X-02/X-03 pinned, X-04 explicitly ruled (fold-into-HARDEN-01).
3. GATE-RECORD exists, names a distinct non-self reviewer (Fable-5), 3 findings each resolved, folded as
   Amendments, Status line CLEARED.
4. No hardening code in `crates/executor`/`crates/brokerd`/`crates/runtime-core` — confirmed via empty
   git diff across all phase commits, absent mechanism symbols, unchanged anchor lines, green
   `check-invariants.sh`.
5. D-02 amendment to `DESIGN-session-trust-state.md` present, correctly scoped to Phase 27.

Phases 27-29 are unblocked to begin hardening implementation. The one process note (stale
REQUIREMENTS.md/ROADMAP.md/STATE.md checkboxes) should be reconciled by the orchestrator at
phase-completion but does not affect the substance of goal achievement.

---

_Verified: 2026-07-12_
_Verifier: Claude (gsd-verifier)_
