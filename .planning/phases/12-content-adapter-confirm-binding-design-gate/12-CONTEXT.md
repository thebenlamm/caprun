# Phase 12: Content, Adapter & Confirm-Binding Design Gate - Context

**Gathered:** 2026-07-07
**Status:** Ready for planning
**Source:** Synthesized from caprun-opus-77's milestone-scoping and roadmap-approval messages (FAMP thread `019f3d72-dbce-7d23-91f4-bd129978fcf4`) — not a live discuss-phase session. Ben Lamm delegated milestone judgment calls to caprun-opus-77 (coordinator), who already specified this phase's scope and review requirements in full technical detail before roadmapping. This file transcribes those decisions; it does not introduce new ones.

<domain>
## Phase Boundary

Produce a reviewed, adversarially-reviewed DESIGN doc (or paired docs, matching the existing `DESIGN-taint-model.md` + `DESIGN-plan-executor.md` and `DESIGN-session-trust-state.md` + `DESIGN-confirmation-release.md` precedent) covering three things — content-sensitivity semantics (CONTENT-01/02), real-adapter mediation (SMTP-01/02/03/05), and confirm-binding to resolved literals (CONFIRM-03) — before any executor/TCB code for this milestone is written. This phase produces DOCUMENTATION ONLY. No code in `crates/executor`, `crates/brokerd` (adapter), or elsewhere implementing CONTENT-01, SMTP-05, or CONFIRM-03 may exist until this phase is marked complete. This mirrors the v1.0 Phase 2 (`DESIGN-taint-model.md`/`DESIGN-plan-executor.md`) and v1.2 Phase 8 (`DESIGN-session-trust-state.md`/`DESIGN-confirmation-release.md`) design-gate discipline.

</domain>

<decisions>
## Implementation Decisions

### Content-sensitivity semantics (CONTENT-01/02)
- **D-01:** The DESIGN doc defines content-sensitivity classification for the email sink's body argument as a single hardcoded match arm in the executor TCB — NOT a general content-classification taxonomy or reusable framework. Scope guard is explicit and intentional (per opus's advisor panel).
- **D-02:** The doc must resolve precedence between the existing routing/recipient I2 block and the new body-content block explicitly. This is not a hypothetical concern — v1.2's Phase 8 round-1 DESIGN doc shipped a real blocker (B1 in `planning-docs/DESIGN-REVIEW-v1.2-round1.md`) where an unstated precedence between two deny/block mechanisms made the confirm path unreachable in every live run. The new doc must show, explicitly, that a tainted recipient AND a tainted body both surface as Blocked (not one silently pre-empting or masking the other) and that both can be confirmed/denied through the existing single-shot mechanism.

### Real-adapter mediation (SMTP-01/02/03/05)
- **D-03:** Confined worker NEVER performs the SMTP call. The broker/adapter performs the SMTP call only after executor-authorize + human-confirm.
- **D-04:** SMTP secrets/credentials live only in the broker process — never in the worker's env, args, or in any plan-node payload that could reach the tainted/confined context.
- **D-05:** The doc must specify a kernel-enforced negative assertion: a confined worker's direct attempt to open an SMTP connection must FAIL under default-deny net. This is a claim about the sandbox boundary, not just code structure, and must be testable on real Linux (mirrors the project's existing default-deny-net posture).
- **D-06:** The acceptance-gate test targets a LOCAL capture SMTP (MailHog/Mailpit) — Linux-verifiable, repeatable, no live infra dependency. Live SES/real inbox is explicitly OUT of gate scope (downgraded to an optional, non-gated, post-milestone config-swap — see PROJECT.md Key Decisions and REQUIREMENTS.md's `SMTP-04` entry under v2/Out of Scope). The doc should not design for live-SES as if it were a milestone requirement.
- **D-07:** The doc must specify wire-message construction (CRLF/header-injection defense, SMTP-05): tainted literals in the body must NOT be able to alter envelope/headers. A body containing `\r\nBcc: attacker@...` must not smuggle a recipient past the human's body confirmation. The doc must show exactly where/how header injection is prevented (e.g., which layer strips/escapes CRLF, and why that layer cannot be bypassed by a tainted literal).

### Confirm-binding (CONFIRM-03)
- **D-08:** `caprun confirm` must bind to a hash of the EXACT resolved recipient+body literals — computed over the actual bytes that will be sent, not over pre-transformation input. This matters concretely because Phase 15 (deterministic extraction) includes a manipulation-variant requirement (EXTRACT-03) where the extractor transforms tainted values (e.g., concatenation, base64-decode) before they reach the sink — the doc must specify that the confirm-binding hash is computed over the POST-transformation literal, matching what the adapter actually transmits, with no drift between confirm time and send time.
- **D-09:** No truncated display of long bodies at confirm time — the human must see the verbatim literal, not a summary.
- **D-10:** Builds on the existing `PendingConfirmation` resolved_args store (from v1.2's `DESIGN-confirmation-release.md` / `crates/brokerd/src/confirmation.rs`) — extend, do not replace, that mechanism.

### Review process (non-negotiable — carried directly from opus)
- **D-11:** The review of this DESIGN doc must be GENUINELY ADVERSARIAL, not self-review. caprun-sonnet-77 (the executor/planner/implementer in this session) must NOT review its own design doc. Per `DEC-ai-review-satisfies-human-gate` in PROJECT.md, an AI-performed adversarial re-read (by a fresh-context reviewer) may satisfy the human-review checkpoint — but only when arranged by caprun-opus-77, who must be flagged at the checkpoint. The plan for this phase must include an explicit checkpoint task (not a silent auto-pass) where the executor stops and reports to caprun-opus-77 before treating the DESIGN doc as approved.
- **D-12:** The review must actively attempt to attack, at minimum, these three specific failure modes (named explicitly by opus, informed by the real v1.2 Phase 8 precedent):
  (a) Can CONTENT-01's body-block and the existing routing-block compose into an unconfirmable dead end — the v1.2 I1/I2-precedence failure mode (B1 above), reincarnated for the body arg?
  (b) Can CONFIRM-03's literal-binding hash be computed over the wrong bytes — pre-transformation input instead of the post-EXTRACT-03-transformation bytes actually sent?
  (c) Does SMTP-05's message construction have any path where a tainted literal reaches a header?
- **D-13:** Follow the existing `DESIGN-GATE-RECORD-*.md` convention (see `planning-docs/DESIGN-GATE-RECORD-v1.2.md` and `planning-docs/DESIGN-GATE-RECORD.md`) — a gate record naming the reviewer, review round, sha256 hashes of the doc(s) under review, findings by severity (blocker/major/minor), root cause if any blocker is found, fixes applied, and an explicit APPROVED/NEEDS REVISION decision. Round 1 finding a blocker (as v1.2's did) is a normal, expected outcome, not a process failure — fix and re-review, same as v1.2.

### Claude's Discretion
- Whether to write one combined DESIGN doc or split into two (e.g., `DESIGN-content-adapter-mediation.md` + `DESIGN-confirm-binding.md`), following whichever grouping the v1.0/v1.2 precedent suggests is clearest. Naming should follow the existing `DESIGN-<topic>.md` convention in `planning-docs/`.
- Exact section structure within the DESIGN doc(s), as long as it addresses D-01 through D-10 explicitly and is reviewable against them.
- Mechanics of how the fresh-context adversarial reviewer is invoked (e.g., a specific subagent or model) — that is opus's call to arrange per D-11, not something to pre-decide in the plan.

</decisions>

<specifics>
## Specific Ideas

- The whole point of this design gate, per opus, is that it "earned its cost" in v1.2 by catching a real precedence bug before any code — the plan should treat that as the standard to meet, not a formality to complete quickly.
- Opus's exact framing: "don't self-review your own design doc" — this is a hard process constraint, not a suggestion.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design-gate precedent (structure and rigor to match)
- `planning-docs/DESIGN-taint-model.md` — v1.0's original I0/I1/I2 design doc
- `planning-docs/DESIGN-plan-executor.md` — v1.0's executor design doc
- `planning-docs/DESIGN-session-trust-state.md` — v1.2's I1 demotion / I0 creation-rule design doc (revised post-B1)
- `planning-docs/DESIGN-confirmation-release.md` — v1.2's confirmation-release design doc (the `PendingConfirmation` mechanism this phase's CONFIRM-03 work extends)
- `planning-docs/DESIGN-REVIEW-v1.2-round1.md` — the round-1 adversarial review that found the B1 precedence blocker this phase's D-02/D-12(a) must not repeat
- `planning-docs/DESIGN-GATE-RECORD-v1.2.md` and `planning-docs/DESIGN-GATE-RECORD.md` — the gate-record format/convention this phase's own gate record should follow

### Milestone scope and locked decisions
- `.planning/PROJECT.md` — Current Milestone section, Key Decisions table (the two reopened locks: CONTENT-01 and the real SMTP adapter; the NOT-reopened LLM-planner lock; the SMTP-04 downgrade rationale)
- `.planning/REQUIREMENTS.md` — full v1.3 REQ-ID list this DESIGN doc must give the later phases (13-17) enough to build against
- `.planning/ROADMAP.md` — Phase 12 success criteria (including the two HARD GATE criteria added after opus's roadmap sign-off: the adversarial-review requirement here, and the genuine-taint backstop hard gate that Phase 15/17 depend on)
- `CLAUDE.md` — hard constraints: TCB is Rust; I2 (and now CONTENT-01) hardcoded in the executor, never a policy file; plan-node API only; locked terminology

### Existing mechanism this phase extends (do not rebuild)
- `crates/brokerd/src/confirmation.rs` — the existing `PendingConfirmation` resolved_args store that CONFIRM-03 extends
- `crates/executor` — the existing I2 sensitivity map / deny function that CONTENT-01/02 extends with one new match arm

</canonical_refs>

<deferred>
## Deferred Ideas

- Live SES / real inbox send — explicitly deferred to an optional post-milestone config-swap (not this phase's, or this milestone's, concern beyond noting the gate uses local capture SMTP instead). See PROJECT.md Out of Scope.
- General content-classification taxonomy/abstraction — explicitly rejected in favor of the single hardcoded match arm (D-01). Do not design for future extensibility here.
- Actual implementation of CONTENT-01, the SMTP adapter, or CONFIRM-03 — that is Phases 13-16's job. This phase produces the design doc only.

</deferred>

---

*Phase: 12-content-adapter-confirm-binding-design-gate*
*Context gathered: 2026-07-07*
