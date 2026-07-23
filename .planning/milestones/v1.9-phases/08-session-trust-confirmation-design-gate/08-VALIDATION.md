---
phase: 8
slug: session-trust-confirmation-design-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-01
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> This phase produces DESIGN docs, not code. "Tests" here means the same grep-based
> completeness checklist + human adversarial review gate the project already used for
> Phase 2 (`planning-docs/DESIGN-GATE-RECORD.md`), not `cargo test`.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Grep-based completeness checklist (bash), then human adversarial review — mirrors `planning-docs/DESIGN-GATE-RECORD.md` round 2 (APPROVED) precedent |
| **Config file** | none — the checklist is authored per-gate-record, matching each DESIGN doc's Done-When predicate |
| **Quick run command** | `grep -c "MUST\|MUST NOT" planning-docs/DESIGN-session-trust-state.md planning-docs/DESIGN-confirmation-release.md` (adapt filenames to what the plan actually authors) |
| **Full suite command** | The 5-step "How to Verify" human review procedure — read end-to-end as an attacker, confirm every MUST/MUST-NOT, re-run `shasum -a 256` on the reviewed docs, then set Decision/Gate status in the gate-record artifact |
| **Estimated runtime** | ~1 second (grep) / hours (human review — not automatable) |

---

## Sampling Rate

- **After every task commit:** re-run the grep completeness check after each DESIGN doc section is drafted
- **After every plan wave:** full human "read as an attacker" pass per `DESIGN-GATE-RECORD.md`'s 5-step procedure
- **Before `/gsd-verify-work`:** gate-record artifact must show `Decision: APPROVED` and `Gate status: UNBLOCKED`
- **Max feedback latency:** ~1s automated (grep); human review is out-of-band and not latency-bounded

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | PROC-01 | V1 / V4 | Session-trust-state DESIGN doc defines `SessionStatus::Draft`, the I1 demotion trigger (`mint_from_read`), the I0 creation rule, and the new `DenyReason` variant | grep + human review | `grep -c "MUST\|MUST NOT" planning-docs/DESIGN-session-trust-state.md` | ❌ W0 | ⬜ pending |
| 08-01-02 | 01 | 1 | PROC-01 | V4 / V8 | Confirmation-release DESIGN doc defines the single-shot `(sink, arg, literal-digest)` release path, durable deny, and the TCB-resident (not policy-file) release decision | grep + human review | `grep -c "MUST\|MUST NOT" planning-docs/DESIGN-confirmation-release.md` | ❌ W0 | ⬜ pending |
| 08-01-03 | 01 | 1 | PROC-01 | V1 | Gate-record artifact records human review Decision/Gate status for both docs before Phase 9/10 begin | human review | manual — set `Decision: APPROVED` / `Gate status: UNBLOCKED` in gate-record file | ❌ W0 | ⬜ pending |

*Exact task IDs are illustrative — the planner assigns final IDs; this map's Req/Threat/Behavior columns are the binding contract.*

---

## Wave 0 Requirements

- [ ] `planning-docs/DESIGN-session-trust-state.md` (or paired doc) — covers TAINT-01..04, ORIGIN-01..02, authored fresh in this phase (no existing test file to extend)
- [ ] `planning-docs/DESIGN-confirmation-release.md` (or paired doc) — covers CONFIRM-01..04, authored fresh in this phase
- [ ] A gate-record artifact (new file, or a new round appended to `planning-docs/DESIGN-GATE-RECORD.md`) — authored fresh, recording the human review Decision/Gate status per PROC-01

No cargo/pytest framework gap — this phase adds no code, so no test-framework install is required.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|--------------------|
| DESIGN doc rigor and adversarial soundness (session self-declares "trusted" to bypass I0; replaying/re-confirming an already-decided `effect_id`; confirm silently re-invoking the general I2 decision function; multi-arg sink confirm executing with stale/wrong non-blocked args) | PROC-01 | Design-review correctness cannot be grepped — it requires a human reading the doc as an attacker, exactly as Phase 2's `DESIGN-GATE-RECORD.md` round 1→2→APPROVED process did | Follow the 5-step "How to Verify" procedure in `planning-docs/DESIGN-GATE-RECORD.md`: read both docs end-to-end as an attacker; confirm every MUST/MUST-NOT is unambiguous and testable; confirm the three known threat patterns above (Security Domain, RESEARCH.md) are explicitly addressed; re-run `shasum -a 256` on the reviewed docs and pin the hashes; record Decision + Gate status |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify (grep completeness check) or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (both DESIGN docs + gate-record artifact)
- [ ] No watch-mode flags
- [ ] Feedback latency < 1s (automated layer); human review latency is out of scope for this metric
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
