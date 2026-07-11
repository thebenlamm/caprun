# Requirements: AgentOS (caprun) — v1.4

**Defined:** 2026-07-10
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — extended with session-level draft-only demotion (I1/I0), single-shot human confirmation, content-sensitive blocking, and (v1.4) coherent cross-connection trust state, so that the boundary holds regardless of who or what sits in the planner seat.

## v1 Requirements

Phase 0 blocks everything else — non-negotiable ordering. It is a security
fix gated by an already-red regression test
(`crates/brokerd/tests/two_connection_intent_bypass.rs`), never green by
weakening assertions.

### Trust Coherence (Phase 0 — the fix)

- [ ] **TRUST-01**: Broker rejects a second connection to an already-active
      session, closing the cross-connection `ProvideIntent` bypass (the
      smaller hammer — a confined worker only ever needs one connection)

- [ ] **TRUST-02**: `two_connection_intent_bypass_repro`'s `#[ignore]` is
      removed and the test is green — fixed by the broker's behavior, never
      by weakening the test's safe-outcome assertions

- [ ] **TRUST-03**: Existing v1.3 live acceptance
      (`scripts/mailpit-verify.sh`) stays green — independently re-run, not
      assumed from a prior pass

### Design Gate (Phase 0 — blocks all TCB code)

- [x] **DESIGN-01**: `planning-docs/DESIGN-session-trust-coherence.md`
      authored and clears a fresh adversarial panel (no self-review, per
      `DEC-ai-review-satisfies-human-gate`) before any `server.rs` change

- [x] **DESIGN-02**: DESIGN doc rules on MAJOR-2 (replay) — re-earns
      "accepted" in writing against the adaptive-planner threat model
      (bounded to trusted/human-typed recipients = DoS/duplication, not new
      exfil); no new CAS this milestone

- [x] **DESIGN-03**: DESIGN doc audits all three mint sites (`mint_from_read`,
      `mint_from_intent`, `mint_from_derivation`) and states the correct,
      narrower claim: only `ProvideIntent` yields a TRUSTED handle from a
      supplied string

- [x] **DESIGN-04**: DESIGN doc documents the decision oracle (MEDIUM-1) —
      `Allowed` vs `BlockedPendingConfirmation{anchors}` plus
      `literal_sha256` leak per-handle taint state and enable offline
      literal-guessing; rules whether Phase 1's planner connection sees full
      decisions or a reduced signal

- [x] **DESIGN-05**: DESIGN doc specifies the per-verb capability split (a
      connection may hold NO mint verb — `ProvideIntent`, `ReportClaims`,
      `ReportDerivedClaim`) forward-looking for Phase 1's planner connection

- [x] **DESIGN-06**: DESIGN doc re-confirms guard-(c)
      (`CAPRUN_ENABLE_IPC_CREATE_SESSION`) is not widened by the Phase-0 fix
      and re-states whether it should finally be compile-excluded

### Documentation Honesty (Phase 0)

- [ ] **DOC-02**: PROJECT.md correction recording that v1.3's guard(a) was
      cross-connection-bypassable and that v1.4 Phase 0 fixes it (scoping-
      time draft already landed in PROJECT.md's v1.3 `<details>` block;
      Phase 0 finalizes it against the shipped fix)

### Planner Seam & Capability Split (Phase 1+ — unblocked by Phase 0)

- [ ] **PLANNER-01**: Design and introduce the planner seam — there is no
      `Planner` trait today (`planner.rs`'s `plan_from_intent` is a bare fn);
      the seam must be designed, not dropped into

- [ ] **PLANNER-02**: The planner's connection holds NO mint verb
      (`ProvideIntent`, `ReportClaims`, `ReportDerivedClaim` unavailable) —
      applies Phase 0's per-verb capability split design

- [ ] **PLANNER-03**: A minimal LLM planner emits only
      `PlanNode{sink, args: Vec<PlanArg>}` — no literal field to carry;
      cheapest model that reliably follows a tool schema, no model-quality
      claim made

- [ ] **PLANNER-04**: The planner is NOT co-located in-process with the
      worker's raw-bytes fd (would breach "typed extracts only" and
      reintroduce token-stream laundering); it sees typed extracts + handle
      IDs only, no caps, no net beyond its own inference endpoint

### Adversarial Gate Proof (Phase 1+ — the HARD GATE)

- [ ] **GATE-01**: An LLM planner, handed a doc whose injection instructs it
      to email `attacker@evil.com`, complies — emits a syntactically valid
      `PlanNode` routing the tainted handle to `to`

- [ ] **GATE-02**: The executor Blocks deterministically, `verify_chain` is
      true, and Mailpit == 0 — genuine propagation, per the §9 standard

- [ ] **GATE-03**: A trusted-intent control on the same sink Allows and
      delivers exactly once, in the same run

- [ ] **GATE-04**: A deterministic construction-site sentinel assertion
      replaces the context-dump grep — feed the planner-prompt constructor a
      tainted record with a sentinel literal (sentinel each fragment), assert
      the sentinel bytes never appear in the constructed prompt (unit-level,
      not probabilistic)

### Residual Risk Documentation (Phase 1+)

- [ ] **T2-01**: T2 (slot-type binding — no `DenyReason` exists for a
      handle's semantic origin mismatching its slot) is documented as the
      accepted v1.4 residual, safe today only by incidental human-typing of
      every `UserTrusted` handle; enforcement deferred to v1.5

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| T2 slot-type binding enforcement | Deferred to v1.5 (Ben's choice, 2026-07-10) — keeps v1.4 to one milestone; new `DenyReason` + TCB logic is real scope, not wiring |
| CAS/idempotency token on the Allowed email.send path | Replay risk re-earned in writing against the new adaptive-planner threat model instead (Ben's choice, 2026-07-10) — amplification stays bounded to trusted/human-typed recipients, not new exfil |
| Shared coherent multi-connection trust state | Rejected in favor of the smaller hammer — reject a 2nd connection outright (Ben's choice, 2026-07-10); a confined worker only ever needs one connection |
| Guard-(c) compile-time exclusion | Re-confirmed, not re-scoped — DESIGN-06 only re-states whether it should happen, doesn't commit to doing it in v1.4 |
| Git/GitHub adapters, Cedar policy engine, cross-host delegation/Biscuit crypto, gVisor/Firecracker, web UI, marketplace, long-term memory | Reaffirmed non-goals through v1.3, unaffected by this milestone |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| TRUST-01 | Phase 19 | Pending |
| TRUST-02 | Phase 19 | Pending |
| TRUST-03 | Phase 19 | Pending |
| DESIGN-01 | Phase 18 | Complete |
| DESIGN-02 | Phase 18 | Complete |
| DESIGN-03 | Phase 18 | Complete |
| DESIGN-04 | Phase 18 | Complete |
| DESIGN-05 | Phase 18 | Complete |
| DESIGN-06 | Phase 18 | Complete |
| DOC-02 | Phase 19 | Pending |
| PLANNER-01 | Phase 20 | Pending |
| PLANNER-02 | Phase 20 | Pending |
| PLANNER-03 | Phase 21 | Pending |
| PLANNER-04 | Phase 20 | Pending |
| GATE-01 | Phase 22 | Pending |
| GATE-02 | Phase 22 | Pending |
| GATE-03 | Phase 22 | Pending |
| GATE-04 | Phase 22 | Pending |
| T2-01 | Phase 22 | Pending |

**Coverage:**

- v1 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0 ✓

**Phase structure rationale:** Phase 0 (fix) splits into two phases mirroring
this project's established design-gate/implementation precedent (v1.0 Phase
2, v1.2 Phase 8, v1.3 Phase 12 — each a standalone reviewed DESIGN doc before
any TCB code): Phase 18 is the design gate (DESIGN-01..06), Phase 19 is the
fix + regression-green + doc-honesty (TRUST-01..03, DOC-02). Phase 1+
(planner) follows the "seam design → implementation → adversarial proof"
shape named at scoping: Phase 20 designs and introduces the `Planner`
seam/capability split (PLANNER-01/02/04), Phase 21 builds the adversarial LLM
planner itself on that seam (PLANNER-03), and Phase 22 is the live HARD GATE
proof plus the T2 residual disclosure (GATE-01..04, T2-01) — mirroring the
project's separate live-acceptance phases (v1.2 Phase 11, v1.3 Phase 17).

---
*Requirements defined: 2026-07-10*
*Last updated: 2026-07-10 after roadmap creation (`/gsd-roadmapper`) — 5 phases (18-22), 19/19 requirements mapped, 0 orphans.*
