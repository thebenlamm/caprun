---
phase: 47
slug: multi-step-plan-stream-design-gate
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: validated
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-23
---

# Phase 47 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Design-gate Nyquist: verify **document completeness + process clearance**, not multi-step unit tests (code does not exist yet).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | None for multi-step code — **doc-assertion + process checks** |
| **Config file** | none — doc-only phase |
| **Quick run command** | `test -f planning-docs/DESIGN-multi-step-plan-stream.md && ./scripts/check-invariants.sh` |
| **Full suite command** | Same + gate-record CLEARED checks + `git status --porcelain -- crates cli` empty |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Section-presence greps for sections authored that task + empty TCB porcelain (`git status --porcelain -- crates cli`)
- **After every plan wave:** Full DESIGN-19 grep bundle + `./scripts/check-invariants.sh`
- **Before `/gsd-verify-work`:** DESIGN-20 gate record CLEARED + all DESIGN-19 pins greppable + no TCB code + invariants green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 47-01-01 | 01 | 1 | DESIGN-19 | T-47-01+ | DESIGN pins stream/handle/confirm/I1/I2/deny | doc-assertion | `test -f planning-docs/DESIGN-multi-step-plan-stream.md` + authoritative section-presence grep bundle below | ✅ | ✅ green |
| 47-01-02 | 01 | 1 | HYG-02 | T-47-08 | Zero new crates / Gate 1+3 re-asserted; no pre-clear TCB edits | automated | `bash scripts/check-invariants.sh && git diff --quiet 976b830..16c5ff7 -- crates cli` | ✅ | ✅ green |
| 47-02-01 | 02 | 2 | DESIGN-20 | T-47-10 | Gate record CLEARED by non-self orchestrator-owned trace | process assertion | `test -f planning-docs/DESIGN-GATE-RECORD-v1.10.md && grep -qiE 'CLEARED\|APPROVE' planning-docs/DESIGN-GATE-RECORD-v1.10.md` | ✅ | ✅ green |
| 47-02-02 | 02 | 2 | DESIGN-20 | T-47-10 | Reviewer independence and re-run triggers recorded | process assertion | `grep -qiE 'reviewer\|independence\|non-self\|Fable' planning-docs/DESIGN-GATE-RECORD-v1.10.md && grep -qiE 're-run\|re-runs' planning-docs/DESIGN-GATE-RECORD-v1.10.md` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### DESIGN-19 section-presence greps (authoritative bundle)

From `47-RESEARCH.md` Validation Architecture:

| Pin | Automated Command |
|-----|-------------------|
| Plan-stream shape | `grep -qiE 'plan.stream\|plan_next\|sequential' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'not batch\|batch.*reject\|no batch' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Handle bag / opaque ValueId | `grep -qiE 'handle bag\|output_value_id' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'opaque ValueId\|planner never mints\|PLAN-03' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Block-and-Hold | `grep -qiE 'Block-and-Hold\|block and hold' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'same Session\|no reconnect\|remint' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Trusted-intent / Draft | `grep -qiE 'trusted-intent\|ProvideIntent' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'CommitIrreversible\|Draft' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Instruction vs value | `grep -qiE 'task_instruction\|instruction.*value\|value channel' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Deny/abort mid-stream | `grep -qiE 'abort\|deny' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Carry-forward invariants | `grep -qiE 'ProvideIntent.*once\|exactly once' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'P33\|P34\|precheck\|terminal.event' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 'POLICY-02\|never.*override I2\|I2.*unconditional' planning-docs/DESIGN-multi-step-plan-stream.md` |
| Reject EffectRequest | `grep -qiE 'EffectRequest\|batch' planning-docs/DESIGN-multi-step-plan-stream.md` |
| DESIGN-20 re-run triggers | `grep -qiE 'orchestrator-owned\|non-self' planning-docs/DESIGN-multi-step-plan-stream.md && grep -qiE 're-run\|re-runs' planning-docs/DESIGN-multi-step-plan-stream.md` |
| HYG-02 in DESIGN | `grep -qiE 'zero new crate\|HYG-02\|Gate 3\|check-invariants' planning-docs/DESIGN-multi-step-plan-stream.md` |

---

## Wave 0 Requirements

- [x] `planning-docs/DESIGN-multi-step-plan-stream.md` — primary deliverable exists and passes the section-presence bundle
- [x] `planning-docs/DESIGN-GATE-RECORD-v1.10.md` — post-trace gate record exists and records APPROVE / CLEARED
- [x] Framework install: **none** — no multi-step test code belongs to this docs-only phase

*Existing `scripts/check-invariants.sh` covers architectural non-regression. No unit-test framework gap for multi-step implementation — out of phase scope.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | — | Durable gate-record assertions and commit-range checks cover this docs-only phase | — |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or durable process assertions
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all formerly missing references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** validated 2026-08-11

## Validation Audit 2026-08-11

| Metric | Count |
|--------|-------|
| Requirements audited | 3 |
| Covered | 3 |
| Partial | 0 |
| Missing | 0 |

Focused audit results: DESIGN-19 grep bundle passed; DESIGN-20 gate-status, independence, and re-run-trigger assertions passed; `scripts/check-invariants.sh` passed all gates; `git diff --quiet 976b830..16c5ff7 -- crates cli` confirmed no TCB changes between DESIGN completion and gate clearance.
