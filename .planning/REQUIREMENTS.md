# Requirements: AgentOS (caprun) — v1.5

**Defined:** 2026-07-11
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended with session-level draft-only demotion (I1/I0), single-shot human confirmation, content-sensitive blocking, coherent cross-connection trust state, a boundary proven indifferent to planner intelligence, and (v1.5) a structural check that a value's semantic origin matches the semantic role of the slot it's routed into — closing the last unenforced degree of freedom named in v1.4's residual disclosure (T2).

## v1 Requirements

Phase order is non-negotiable: the Design Gate blocks all TCB code
(`crates/executor`, `crates/brokerd`'s mint sites) exactly as v1.0 Phase 2,
v1.2 Phase 8, v1.3 Phase 12, and v1.4 Phase 18 did before it.

### Design Gate (blocks all TCB code)

- [x] **DESIGN-07**: `planning-docs/DESIGN-slot-type-binding.md` authored and
      clears a fresh (non-self) adversarial review — no `crates/executor` or
      `crates/brokerd` mint-site code before it clears. Specifies: the
      origin-role tagging mechanism, the new `DenyReason` variant's shape and
      its full exhaustive-match blast radius, and the collect-vs-deny-
      immediately ordering ruling (whether a slot-type mismatch joins the
      collect-then-Block `BlockedPendingConfirmation` set or returns a hard
      `Denied`).

- [x] **DESIGN-08**: DESIGN doc unifies with the EXISTING `claim_type`
      taxonomy already present in `crates/brokerd/src/quarantine.rs`
      (`"email_address"`/`"relative_path"`/`"doc_fragment"`, currently
      consumed only at `mint_from_read` time to derive taint labels, then
      discarded) rather than inventing a parallel role taxonomy for
      untrusted-origin values. For `ProvideIntent`-minted `UserTrusted`
      values (recipient/subject/body — no existing claim_type equivalent),
      the doc defines the analogous role tags from scratch.

- [x] **DESIGN-09**: DESIGN doc explicitly resolves role propagation through
      `mint_from_derivation` (e.g. `ReportDerivedClaim`'s `Concat` transform
      over a Reply-To/Domain pair) — what role, if any, a derived/composite
      value carries. Not left implicit.

- [x] **DESIGN-10**: DESIGN doc pins the fail-closed default: a value with
      no assigned role, or a role that isn't in the expected-role table for
      the target slot, hitting a role-checked slot is a `Deny` — never a
      silent pass-through to `Allowed`.

### Slot-Type Binding (implementation — unblocked by the Design Gate)

- [x] **T2-02**: Each minted value carries a semantic origin-role tag,
      populated at the three mint call sites (`mint_from_intent`,
      `mint_from_read`, `mint_from_derivation`) via an additive, mechanical
      change to their signatures/call sites. This does NOT change I0/I1
      trust classification — which values become `UserTrusted` vs untrusted
      is unaffected; only a new field is threaded through.

- [x] **T2-03**: A hardcoded per-sink-arg "expected role" table exists in
      `crates/executor` (mirrors the `sink_sensitivity.rs` CONTENT-01/02
      precedent — hardcoded match arms scoped to the two live sinks
      `email.send`/`file.create`, not a general framework).

- [x] **T2-04**: A new `DenyReason` variant is added to the exhaustive
      taxonomy in `runtime_core::executor_decision` (no wildcard arm, per
      the project's §10 discipline). Every existing exhaustive match over
      `DenyReason` across the workspace (CLI rendering, audit
      serialization/`code()`/`Display`, existing tests) is updated for the
      new arm — not just the match inside `submit_plan_node`.

- [x] **T2-05**: `submit_plan_node` denies (or blocks, per DESIGN-07's
      ordering ruling) a plan node when a resolved value's origin role
      doesn't match its slot's expected role, evaluated per-arg in the same
      pass as the existing routing/content-sensitivity check, without
      weakening or reordering the existing I0 (Step 0.5 class-deny) / I2
      (per-arg Block) precedence.

### Regression & Live Proof (the DONE gate)

- [ ] **T2-06**: A held-out acceptance test proves the gap is closed: a plan
      node with a deliberately swapped subject↔recipient handle pair (both
      `UserTrusted`, both otherwise valid) produces the new deny, with a
      corresponding audit-DAG event recorded and `verify_chain` still true.

- [x] **T2-07**: Existing tests that currently rely on permissive
      `UserTrusted`-in-any-slot behavior are identified (regression audit)
      and updated so the new check doesn't silently break existing coverage
      or get bypassed by a fixture that never assigns a role.

- [ ] **T2-08**: Full workspace regression via `scripts/mailpit-verify.sh`
      is independently re-run green (0 failures) after the change lands —
      not assumed from a prior pass, per this project's standing milestone-
      close discipline.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Mint-time TRUST CLASSIFICATION changes (I0/I1) | T2 adds an origin-role tag; it does not change which values become `UserTrusted` vs untrusted (Ben's choice, 2026-07-11 scoping — confirmed reading of "no changes to mint sites") |
| Connection/capability model changes | Already shipped in v1.4 (Phase 20's `ConnectionRole` split); unaffected by this milestone |
| General content-classification taxonomy/framework | T2's expected-role table stays hardcoded per-sink-arg, matching the `CONTENT-01/02` precedent — not a reusable framework |
| CAS/idempotency token on the Allowed `email.send` path | Already re-earned in writing at v1.4 (`DESIGN-session-trust-coherence.md` §6); untouched by T2 |
| Role-tagging for sinks beyond `email.send`/`file.create` | v0 sink scope stays the same two live sinks; consistent with `sink_sensitivity.rs`'s documented v0 scope |
| Git/GitHub adapters, Cedar policy engine, cross-host delegation/Biscuit crypto, gVisor/Firecracker, web UI, marketplace, long-term memory | Reaffirmed non-goals through v1.4, unaffected by this milestone |

## Traceability

Which phases cover which requirements. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-07 | Phase 23 | Complete |
| DESIGN-08 | Phase 23 | Complete |
| DESIGN-09 | Phase 23 | Complete |
| DESIGN-10 | Phase 23 | Complete |
| T2-02 | Phase 24 | Complete |
| T2-03 | Phase 24 | Complete |
| T2-04 | Phase 24 | Complete |
| T2-05 | Phase 24 | Complete |
| T2-06 | Phase 25 | Pending |
| T2-07 | Phase 25 | Complete |
| T2-08 | Phase 25 | Pending |

**Coverage:**

- v1 requirements: 11 total
- Mapped to phases: 11
- Unmapped: 0 ✓

---
*Requirements defined: 2026-07-11*
*Last updated: 2026-07-11 after v1.5 roadmap created (3 phases: 23-25), 11/11 requirements mapped, 0 orphans.*
