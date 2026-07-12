# Requirements: caprun (AgentOS)

**Defined:** 2026-07-12
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink — with session-level draft-only demotion, human confirmation, content-sensitive blocking, coherent cross-connection trust, a boundary indifferent to planner intelligence, and slot-type binding. v1.6 hardens the standing residuals that make several of those guarantees "true only incidentally" into enforced guarantees.

## v1 Requirements

Milestone **v1.6 — Security Hardening (close the residuals)**. Five TCB-local residuals, each currently documented as an accepted DOC-01 caveat, turned into an enforced guarantee. No new external-effect surface.

### Design Gate

- [x] **DESIGN-11**: A DESIGN doc (`planning-docs/DESIGN-security-hardening.md`) exists and specifies the approach + fail-closed default for all five residuals: (a) the demote-at-RequestFd model and how it reconciles with the CONTROL-01 benign clean path; (b) the `verify_chain` authentication mechanism (keyed MAC and/or externally-anchored chain head) including key/anchor custody and threat model; (c) the Allowed-path idempotency/CAS shape; (d) the `CreateSession` forced-Active-mint compile-exclusion mechanism; (e) the `file.create` `contents` expected-role / sensitivity treatment.
- [ ] **DESIGN-12**: The DESIGN doc clears a fresh (non-self) adversarial review with every finding resolved, before any `crates/executor`, `crates/brokerd`, or `crates/runtime-core` hardening code is written (standing design-gate discipline: v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23).

### Hardening

- [ ] **HARDEN-01**: fd release to the confined worker (`RequestFd`) itself demotes the session to draft-only for the I1 reason, so "reading raw untrusted bytes → draft-only" holds at release time — without breaking the CONTROL-01 benign clean path (a fragment-free doc read must still stay Active and send).
- [ ] **HARDEN-02**: The audit chain is authenticated — an actor with `events`-table write access can no longer produce a chain that `verify_chain` accepts (keyed MAC and/or externally-anchored head), and a tampered payload/head is detected rather than silently re-consistent.
- [ ] **HARDEN-03**: A replayed `SubmitPlanNode` on the trusted (Allowed) `email.send` path sends at most once via an idempotency key / compare-and-swap, matching the confirm path's at-most-once transaction.
- [ ] **HARDEN-04**: The `CreateSession`-IPC forced-`Active` mint arm is excluded from the production build at compile time (cfg), so the code is absent from the shipped binary — not merely gated behind the `CAPRUN_ENABLE_IPC_CREATE_SESSION` runtime default-deny flag.
- [ ] **HARDEN-05**: The `file.create` `contents` arg carries an expected-role / sensitivity treatment so a tainted value routed into it is handled under the same I2 / slot-type discipline as other sensitive sink args, closing the currently-unconstrained-slot gap.

### Proof

- [ ] **HARDEN-06**: After all hardening lands, the full workspace regression is independently re-run green on real Linux via the bare `scripts/mailpit-verify.sh` recipe, with new negative tests proving each closed residual (a forged/tampered chain is rejected; a replayed Allowed send delivers exactly once; the forced-Active path is absent from the built binary; fd release demotes the session; the `file.create` `contents` slot is constrained) — and no regression to v1.1–v1.5 behavior.

## v2 Requirements

Deferred to **v1.7 — Breadth** (the PLAN.md "v1" adapter bucket), out of scope for v1.6 to keep the security-hardening milestone coherent and right-sized.

### Adapters

- **GIT-01**: Git adapter (broker-mediated repo operations)
- **GH-01**: GitHub adapter (issues/PR API)
- **TEST-01**: Test adapter (run project tests as a mediated effect)
- **PATCH-01**: Patch / PR creation flow
- **SNAP-01**: Workspace snapshots

## Out of Scope

Explicitly excluded from v1.6. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Git/GitHub/test/patch-PR/snapshot adapters | Net-new external-effect surface; deferred to v1.7 to keep v1.6 a coherent, right-sized security-hardening milestone (mixing TCB hardening with greenfield adapters cuts against the right-sizing discipline that split v1.4) |
| Cedar / policy-engine for sink access | I2 stays hardcoded in the Rust TCB; simple rules suffice (standing exclusion) |
| Cross-host delegation / Biscuit crypto | v3 concern |
| gVisor / Firecracker | bubblewrap + seccomp + Landlock remains the boundary |
| Live SES / real inbox send | Mailpit is a real SMTP send for the gate; live SES adds fragility for ~zero legibility gain (standing exclusion) |
| Mac / WSL2 support | All security claims remain Linux-only |

## Traceability

Which phases cover which requirements. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-11 | Phase 26 | Complete |
| DESIGN-12 | Phase 26 | Pending |
| HARDEN-01 | Phase 27 | Pending |
| HARDEN-04 | Phase 27 | Pending |
| HARDEN-02 | Phase 28 | Pending |
| HARDEN-03 | Phase 29 | Pending |
| HARDEN-05 | Phase 29 | Pending |
| HARDEN-06 | Phase 30 | Pending |

**Coverage:**

- v1 requirements: 8 total
- Mapped to phases: 8 (roadmap created — Phases 26-30)
- Unmapped: 0

**Phase grouping rationale (`/gsd-roadmapper`, 2026-07-12):** Phase 26 is a standalone
design-gate phase (DESIGN-11/12), mirroring this project's standing precedent (v1.0 P2,
v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23) — no `crates/executor`/`crates/brokerd`/
`crates/runtime-core` hardening code may be written before it clears a fresh adversarial
review. The five HARDEN items split into 3 implementation phases by blast radius rather
than one bundled phase (so each phase's success criteria stay independently verifiable)
or five separate phases (avoiding trivial-single-requirement phases): Phase 27 groups
HARDEN-01 + HARDEN-04 (both land in `server.rs`'s session/connection-lifecycle surface —
demote-at-RequestFd and the CreateSession forced-Active compile-exclusion); Phase 28 is
HARDEN-02 alone (audit-chain keyed-MAC/anchoring is a self-contained, substantial
mechanism — key/anchor custody + threat model — distinct from the other four); Phase 29
groups HARDEN-03 + HARDEN-05 (both are sink-dispatch-level hardening — Allowed-path CAS
and the `file.create` `contents` expected-role table entry — even though the mechanisms
differ, both close a specific sink-level gap rather than a session/connection-level one).
Phase 30 (HARDEN-06) is the dedicated regression/live-proof phase, mirroring v1.2 P11,
v1.3 P17, v1.4 P22, v1.5 P25 — depends on Phases 27, 28, and 29 all landing first.

---
*Requirements defined: 2026-07-12*
*Last updated: 2026-07-12 after v1.6 roadmap creation — 8/8 requirements mapped to Phases 26-30, 0 unmapped*
