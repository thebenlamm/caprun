# Context

Narrative and background notes from `planning-docs/PLAN.md` that inform planning
but are not themselves decisions, requirements, or hard constraints.

---

## Topic: Thesis / framing

- **source:** planning-docs/PLAN.md (§ What We Are Building)
- Humans execute programs; agents execute intents. Object-capability scoping is
  natural for machines. The runtime gives agents no ambient authority; every
  external effect is authorized against a Session; confinement is
  kernel-enforced.
- Plan status: Agreed by AoS-claude, AoS-codex, AoS-grok (2026-06-29),
  `#aos-session0` convergence. Debate closed on items marked DECIDED.

## Topic: Explicitly out of v0

- **source:** planning-docs/PLAN.md (§ Explicitly OUT of v0)
- Do not build until §9 holds: Git/GitHub adapters; Cedar (simple TOML/rules for
  sink access is fine, I2 stays in Rust); cross-host delegation / Biscuit crypto;
  gVisor / Firecracker; LLM planner (hard-coded/stub planner is fine); rich
  approval policy learning; undo snapshots; broad effect taxonomy; web UI,
  marketplace, long-term memory, browser control; natural-language policy
  authoring.

## Topic: Residual risks (acknowledged, not solved in v0)

- **source:** planning-docs/PLAN.md (§ Residual Risks)
- fd cannot be selectively revoked after SCM_RIGHTS handoff (mitigated:
  disposable workers, mediated high-risk).
- Planner/intent-creation injection (mitigated: I0 draft-only rule + human gate
  on Tier 3+ from tainted session seeds).
- Steganographic encoding in extract values (accepted residual risk; document in
  threat model).
- Broker bugs = full compromise (mitigate: keep broker small).

## Topic: Post-v0 roadmap

- **source:** planning-docs/PLAN.md (§ Post-v0 Roadmap)
- v1: Git, GitHub, test adapter, patch/PR, workspace snapshots, rich approval.
- v2: Multi-worker decomposition, parallel execution.
- v3: Cross-machine Sessions, Ed25519 export, broker federation.
- v4: General adapters (email, cloud, MCP ecosystem, …).

## Topic: Immediate next actions (author's note to Ben)

- **source:** planning-docs/PLAN.md (§ Next Actions for Ben)
- Review the plan; counter anything marked DECIDED if wrong. Scaffold the
  `AgentOS/` Cargo workspace + `crates/` layout. Start M0 + M0-design in
  parallel (sandbox code + DESIGN-plan-executor.md). Do not write
  `crates/executor` until DESIGN-plan-executor.md is reviewed.

## Topic: Excluded cross-references

- **source:** planning-docs/PLAN.md classification (cross_refs)
- PLAN.md references background docs under `archive/` (AGENT-RUNTIME-HANDOVER.md,
  multi-part/*, agent-execution-runtime-handover.md) and two DESIGN docs
  (DESIGN-taint-model.md, DESIGN-plan-executor.md). `archive/` is intentionally
  excluded from this ingest; the DESIGN docs are deliverables to be authored
  (REQ-design-taint-model, REQ-design-plan-executor), not yet present. No
  cross-doc graph edges were resolvable, so no cycle risk.
