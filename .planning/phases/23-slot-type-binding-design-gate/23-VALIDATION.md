---
phase: 23
slug: slot-type-binding-design-gate
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-11
---

# Phase 23 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

**Design-gate phase — no executable code is produced.** The sole deliverable is
the Markdown design document `planning-docs/DESIGN-slot-type-binding.md`. Standard
test-mapping (Nyquist Dimension 8) does not apply to a prose artifact; the
equivalent verification gate is a **fresh, non-self adversarial review** with every
raised finding resolved (ROADMAP.md success criterion 5). This mirrors the v1.0
Phase 2 / v1.2 Phase 8 / v1.3 Phase 12 / v1.4 Phase 18 design-gate discipline.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) — **not exercised this phase** (no code written) |
| **Config file** | none — design-doc phase |
| **Quick run command** | `./scripts/check-invariants.sh` (proves NO executor/broker mint-site code was added) |
| **Full suite command** | `cargo build --workspace` (proves the tree still builds; no new tests) |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** `./scripts/check-invariants.sh` (the gate that must stay green — no TCB code before the doc clears review)
- **After the review-resolution task:** confirm every adversarial-review finding is resolved in the doc
- **Before phase verify:** design doc exists, covers DESIGN-07..10, and the review is cleared with no open findings
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 23-01 (author doc) | 01 | 1 | DESIGN-07/08/09/10 | — | Design doc specifies role-tag mechanism, DenyReason blast radius, ordering ruling, claim_type unification, derivation propagation, fail-closed default | review | `test -f planning-docs/DESIGN-slot-type-binding.md` + section grep | ✅ | ⬜ pending |
| 23-02 (adversarial review) | 02 | 2 | DESIGN-07 | — | Fresh non-self reviewer traces the design against real code; every finding resolved; no `crates/executor`/`crates/brokerd` mint-site code exists | review | `./scripts/check-invariants.sh` (0 violations) + review-record shows all findings resolved | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase needs — no code, no new test framework. `check-invariants.sh` already exists and is the phase's guard rail (it must show 0 executor/broker mint-site additions).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Design doc clears a fresh non-self adversarial review | DESIGN-07 | A design-doc's soundness is a reasoning judgment traced against real code, not an automatable assertion | Spawn a fresh (non-self) reviewer with the doc + code access; require it to attack the role-laundering, fail-closed, I0/I2-precedence, and blast-radius-completeness angles; resolve every finding in-doc before marking the gate cleared |
| Doc content covers all four DESIGN reqs | DESIGN-07..10 | Prose coverage, not a test | Grep the doc for the required rulings; confirm each of DESIGN-07(a/b/c), 08, 09, 10 has a resolved section |

---

## Validation Sign-Off

- [ ] Design doc `planning-docs/DESIGN-slot-type-binding.md` exists
- [ ] Doc resolves DESIGN-07 (role-tag mechanism + DenyReason shape/blast-radius + ordering ruling)
- [ ] Doc resolves DESIGN-08 (unifies with existing `claim_type` taxonomy)
- [ ] Doc resolves DESIGN-09 (derivation role propagation, not left implicit)
- [ ] Doc resolves DESIGN-10 (fail-closed default pinned)
- [ ] Fresh non-self adversarial review cleared with every finding resolved
- [ ] `./scripts/check-invariants.sh` green — NO `crates/executor`/`crates/brokerd` mint-site code added this phase
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
