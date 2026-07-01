# Milestone v1.2 Seed — I1/I0 Enforcement + Confirmation Loop

**Status:** Proposal (input to /gsd-new-milestone). Written 2026-07-01 from a post-v1.1 codebase assessment.
**Precedence:** PLAN.md wins on any conflict. This doc scopes the *next* milestone; it locks nothing.

---

## Where the project stands (assessment summary)

v1.1 shipped the core I2 claim live: a kernel-confined worker whose only egress is
broker-mediated plan nodes, with a genuine (non-stapled) taint chain deterministically
blocking value-injection at routing-sensitive sink args — from the real CLI, 29/29
Linux tests green.

**Verified strengths (keep; do not rework):**

- Executor TCB is small, pure, fail-closed at every step (schema gate → resolve →
  empty-taint → empty-provenance → sensitivity). `crates/executor/src/lib.rs` (142 lines).
- Anti-stapling is build-enforced (`check-invariants.sh` negative greps; broker is sole
  minter; planner signature accepts only opaque `ValueId` handles — trust-laundering is
  type-impossible).
- Worker self-confinement ordering (connect → confine → IPC) and the lossy-extraction
  guarantee (raw hostile bytes never cross IPC; broker taints all claims regardless of
  extractor variant).
- Redactable-digest audit chain; causal DAG vs value-lineage kept as distinct graphs.

**Known gaps (candidate scope, ranked):**

1. **I1/I0 are untouched.** "Reading raw untrusted bytes → session demoted to draft-only"
   is stated in PLAN.md but not mechanically enforced anywhere. This is the existential
   bet: can the opaque-handle architecture host an LLM-shaped planner without breaking
   the invariants? Better to know before the sink catalog grows.
2. **No confirmation loop.** `BlockedPendingConfirmation` exits 1. I2's escape hatch —
   literal-value human confirmation — is unbuilt. This is what turns "exit 1" into a
   usable runtime, and where confirmation-fatigue risk lives.
3. **Content-sensitive args don't block** (executor step 3, deferred by design).
   Exfiltration via email body is the obvious attack once routing args block.
4. **Positioning debt:** README should position vs Google CaMeL (2025) — kernel
   enforcement + audit DAG are the differentiators. Small, non-blocking.

**Explicitly NOT recommended for v1.2:** more sinks (linear engineering, proves nothing
new), real LLM planner (stub/LLM-shaped is sufficient per PLAN.md scope lock), Git/GitHub
adapters, Cedar, cross-host delegation.

---

## Proposed milestone: v1.2 — "Tainted Session, Human Gate"

**Core value statement:** A session that touches untrusted content is mechanically
demoted to draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked
sink arg can be released only by literal-value human confirmation — all deterministic,
all in the audit DAG.

### Candidate phase shape (for roadmapper to refine)

1. **Session taint state (I1 dynamic default).** Broker tracks per-session trust state.
   The `mint_from_read` path (raw untrusted read Event) flips the session to draft-only.
   Draft-only sessions: `CommitIrreversible`-class plan nodes are Denied (new
   `DenyReason` variant), `MutateReversible`/`Observe` still allowed. Recorded as an
   audit event with the causal edge to the read event.
2. **I0 creation rule.** A Session whose intent/seed derives from external content
   starts draft-only and cannot auto-authorize Tier 3+. Needs a seed-provenance field
   at session creation (`caprun` CLI decides trusted-arg vs file-derived seed).
3. **Confirmation loop.** `BlockedPendingConfirmation` surfaces the verbatim literal +
   provenance to the human (CLI prompt in v0), records confirm/deny as an audit event
   anchored to the `SinkBlockedAnchor.effect_id`, and on confirm releases exactly that
   (sink, arg, literal-digest) triple — not a session-wide waiver. Deny is durable.
4. **Acceptance test (§9-style, live from CLI):** hostile workspace file → worker reads
   it → session demoted (I1) → tainted routing arg Blocked (I2, existing) → human denies
   → nothing sent; separately, human confirms → effect proceeds exactly once; audit DAG
   shows the unbroken chain read → demotion → block → human decision.

### Hard constraints carried forward

- Terminology lock, effect-path lock (plan nodes only), TCB-is-Rust, design-gate docs
  before new executor behavior (a DESIGN doc for session-trust-state / confirmation
  semantics should gate phase 1 and 3).
- I2 remains hardcoded; the confirmation release path must live in the TCB, not policy.
- Mint invariants (non-empty taint + provenance) unchanged; LocalWorkspace still minted
  as ExternalUntrusted per the standing decision.

### Open questions for /gsd-new-milestone discussion

- Does draft-only deny at the executor (new decision branch) or at the broker before the
  executor is consulted? (Recommendation: executor — keep all deny logic in one TCB
  function with one DenyReason taxonomy.)
- Confirmation UX surface in v0: interactive TTY prompt vs a `caprun confirm <effect_id>`
  second command. (Recommendation: second command — testable, non-interactive-friendly.)
- Does a confirm create a standing policy entry (exact-match) or is it single-shot?
  (Recommendation: single-shot in v1.2; standing policy is scope creep.)
