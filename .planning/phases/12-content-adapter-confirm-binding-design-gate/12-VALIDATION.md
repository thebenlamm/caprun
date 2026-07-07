---
phase: 12
slug: content-adapter-confirm-binding-design-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-07
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | N/A — this phase produces DESIGN documentation only, no code and no test files (per `12-RESEARCH.md`'s Validation Architecture section) |
| **Config file** | N/A |
| **Quick run command** | `grep`-based completeness checklist against the DESIGN doc(s) draft — see Per-Task Verification Map below |
| **Full suite command** | Same grep-completeness checklist, re-run against the final draft before the adversarial review pass |
| **Estimated runtime** | seconds (grep, not compilation/test execution) |

---

## Sampling Rate

- **After every task commit:** Re-run the grep-completeness checklist against the current doc draft (confirms D-01 through D-22 each have a corresponding section)
- **After every plan wave:** Full checklist re-run
- **Before `/gsd-verify-work` (this phase's gate):** `planning-docs/DESIGN-GATE-RECORD-v1.3.md` must show `Decision: APPROVED` and `Gate status: UNBLOCKED`, produced by a genuinely adversarial, fresh-context review (D-11) — not the checklist alone
- **Max feedback latency:** seconds (no build/test cycle in this phase)

---

## Per-Task Verification Map

This phase has no unit/integration tests — its "tests" are grep-based presence assertions against the DESIGN doc's own text, mirroring `planning-docs/DESIGN-GATE-RECORD-v1.2.md`'s established pattern, plus the adversarial review itself as the phase-gating check.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 12-01-0x | 01 | 1 | DESIGN-01 (D-01, D-14) | D-12(a) | DESIGN doc states collect-then-Block as a MUST, `ExecutorDecision`/`SinkBlockedAnchor` become plural | checklist | `grep -q "collect-then-Block" planning-docs/DESIGN-*.md` | ❌ W0 | ⬜ pending |
| 12-01-0x | 01 | 1 | DESIGN-01 (D-15) | — | DESIGN doc states I2-over-I1 precedence unchanged (Step 0.5 still runs only after the collect-all loop completes with no Block) | checklist | `grep -q "Step 0.5" planning-docs/DESIGN-*.md` | ❌ W0 | ⬜ pending |
| 12-01-0x | 01 | 1 | DESIGN-01 (D-19) | D-12(b) | DESIGN doc states CONFIRM-03's hash covers the FULL SET of blocked args as one combined digest, computed post-transformation | checklist | `grep -q "combined.*digest\|combined.*hash" planning-docs/DESIGN-*.md` | ❌ W0 | ⬜ pending |
| 12-01-0x | 01 | 1 | DESIGN-01 (D-07/SMTP-05) | D-12(c) | DESIGN doc specifies wire-message construction defense and forbids `dangerous_new_pre_encoded`/string-built headers | checklist | `grep -q "dangerous_new_pre_encoded" planning-docs/DESIGN-*.md` | ❌ W0 | ⬜ pending |
| 12-01-0x | 01 | 1 | DESIGN-01 (D-21) | — | DESIGN doc records that `is_content_sensitive` already exists (cites file/line) and Phase 14's work is Step 3's consequence only | checklist | `grep -q "sink_sensitivity.rs" planning-docs/DESIGN-*.md` | ❌ W0 | ⬜ pending |
| 12-02-0x | 02 | 2 | DESIGN-01 (D-11, D-12, D-13) | all three D-12 vectors | Adversarial review completed by a fresh-context reviewer (not self-review); each D-12(a/b/c) vector has a dedicated "How to Verify" section tracing to actual code/line numbers, mirroring `DESIGN-REVIEW-v1.2-round1.md`'s B1 finding | checkpoint | manual/AI-adversarial review producing `planning-docs/DESIGN-GATE-RECORD-v1.3.md` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `planning-docs/DESIGN-<topic>.md` (one or two docs, per Claude's Discretion in CONTEXT.md) — does not yet exist, Wave 1 authors it
- [ ] `planning-docs/DESIGN-GATE-RECORD-v1.3.md` — must be authored following `DESIGN-GATE-RECORD-v1.2.md`'s structure (Documents Under Review + sha256 table, checklist mapped to CONTENT-01/02, SMTP-01/02/03/05, CONFIRM-03, explicit "How to Verify" steps per D-12 vector, Decision/Gate-status fields) — Wave 2 authors it after the adversarial review

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|--------------------|
| Genuinely adversarial review of the DESIGN doc(s) | DESIGN-01 (D-11, D-12) | Cannot be grepped — requires a reasoning pass attacking the three named failure modes against actual code, arranged by caprun-opus-77 per `DEC-ai-review-satisfies-human-gate` (the executor/planner in this session must not self-review) | Executor stops at the review checkpoint, flags caprun-opus-77 via FAMP, waits for the arranged fresh-context reviewer's findings, resolves any BLOCKER/MAJOR findings, re-reviews if needed (mirroring v1.2's round-1 → round-2 pattern), then records the gate record with `Decision: APPROVED` before the phase can close |

---

## Validation Sign-Off

- [ ] All Wave 1 tasks have a grep-completeness `<automated>` verify per the map above
- [ ] Wave 2's adversarial-review task is explicitly `autonomous: false` / checkpoint-gated — it is the one task in this phase that cannot be automated
- [ ] Wave 0 covers both MISSING references (DESIGN doc(s), gate record)
- [ ] No watch-mode flags (N/A — no test runner in this phase)
- [ ] Feedback latency < 5s (grep-only)
- [ ] `nyquist_compliant: true` set in frontmatter once Wave 1 tasks' checklist commands are finalized by the planner

**Approval:** pending
