---
phase: 31
slug: effect-breadth-design-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-17
---

# Phase 31 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **Design-gate phase.** No TCB code is written this phase (CLAUDE.md hard
> constraint: design-gate docs block executor code). The deliverable is
> `planning-docs/DESIGN-effect-breadth-exec.md` + a gate record clearing a
> fresh non-self adversarial code-trace. "Validation" here is therefore
> document-completeness + gate-clearance, not a test suite. The eventual
> *code* validation (named tests, negative tests per sink, live Linux
> acceptance) is designed in RESEARCH.md §Validation Architecture and lands in
> phases 32–34 — it is specified here so the design doc can pin it, but not
> executed this phase.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | None this phase — doc-only. (Downstream: `cargo test --workspace` + `scripts/mailpit-verify.sh` on Linux) |
| **Config file** | none — no code produced |
| **Quick run command** | `grep -c '^## ' planning-docs/DESIGN-effect-breadth-exec.md` (section presence) |
| **Full suite command** | `./scripts/check-invariants.sh` (must still pass — no `EffectRequest` token, mint call-site gate intact) |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Confirm the DESIGN doc's target section for that task exists and is non-stub.
- **After the doc is complete:** Run `./scripts/check-invariants.sh` to confirm no architectural-invariant regression from any wording/example.
- **Before gate clearance:** The fresh non-self adversarial code-trace review must have all findings resolved, recorded in the gate record.
- **Max feedback latency:** ~5 seconds.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 31-01-xx | 01 | 1 | DESIGN-13 | — | DESIGN doc pins broker-spawned confined-child-exec model + fs read/write breadth (spawn, confinement, stdout/stderr capture + taint-mint) | doc-assertion | `grep -q 'broker-spawned' planning-docs/DESIGN-effect-breadth-exec.md` | ❌ W0 | ⬜ pending |
| 31-01-xx | 01 | 1 | DESIGN-14 | — | Doc pins fail-closed defaults: exec arg-schema + (dis)allow posture, exec-output taint label + origin_role, fs path/slot constraints; nothing bypasses I2; no raw EffectRequest path | doc-assertion | `./scripts/check-invariants.sh` exits 0 | ❌ W0 | ⬜ pending |
| 31-02-xx | 02 | 2 | DESIGN-13 | — | Fresh non-self adversarial code-trace clears the doc (all findings resolved), recorded in a gate record | manual-review | Gate record exists with reviewer verdict = cleared + finding resolutions | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Task IDs are placeholders — the planner assigns final IDs.*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase validation needs.* `check-invariants.sh`
already exists and gates the architectural invariants this doc must not break.
No test framework install is needed for a doc-only phase.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Design decisions are sound and I2/slot-type-binding-consistent | DESIGN-13/14 | Design correctness is a judgment call, not a source assertion | Fresh non-self reviewer (Fable-5 per `[[fresh-context-adversarial-review]]`) traces the design against the actual code (crates/{sandbox,brokerd,executor,runtime-core}); every finding resolved before gate clears |
| Fail-closed defaults genuinely fail closed | DESIGN-14 | Requires reasoning about attacker paths, not a test | Reviewer confirms: tainted exec command/args → Deny; unknown/mismatched slot role → Deny; no default that silently Allows |

---

## Validation Sign-Off

- [ ] DESIGN doc exists at `planning-docs/DESIGN-effect-breadth-exec.md` with all required sections (DESIGN-13 contents + DESIGN-14 fail-closed defaults)
- [ ] `./scripts/check-invariants.sh` passes (no `EffectRequest` token introduced; mint call-site gate discussion does not weaken the gate)
- [ ] Fresh non-self adversarial code-trace review recorded in a gate record with all findings resolved
- [ ] No TCB code written this phase (git diff touches only planning-docs/ + .planning/)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
