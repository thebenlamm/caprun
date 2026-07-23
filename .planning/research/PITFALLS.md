# Pitfalls Research

**Domain:** Multi-step plan orchestration / Safe Coding Agent loop on a capability-secure Intent Runtime (caprun I0/I1/I2)
**Researched:** 2026-07-23
**Confidence:** HIGH (project retros + design gates + live code seams; external agent literature secondary)

**Scope note:** These are pitfalls of *adding multi-node plan streams and a multi-step coding loop onto a proven single-node security substrate* (v1.0–v1.9). Greenfield Intent Runtime mistakes already closed by v1.9 are listed only when multi-step reopens them.

**Substrate baseline (do not re-break):**
- Plan-node API only (`submit_plan_node`; no `EffectRequest`)
- Broker-owned `ValueRecord` / opaque `ValueId` handles; planner never mints taint
- I2 hardcoded in Rust TCB; policy is pre-I2 narrowing only
- Sole mint-site discipline (Gate 3): `mint_from_read` / `mint_from_derivation` / `mint_from_exec` / `mint_from_http` / `.mint(`
- ProvideIntent exactly-once, before any RequestFd, session-lifetime occupancy latch
- Confirm is single-shot on a durable resolved snapshot (separate process)
- v1.9 honesty: `caprun run` is single-node; LIVE-05 multi-node chain was hybrid in-crate composition

---

## Critical Pitfalls

### Pitfall 1: Intermediate-result taint laundering across plan nodes

**What goes wrong:**
Node N produces an observation (`process.exec` stdout, `http.request` body, file read, derivation). Node N+1 routes that value into a sensitive sink arg (`github.pr` title/body, `git.push` refspec, `http.request.write` body, `email.send` to/body) as if it were `UserTrusted` — either by re-minting via ProvideIntent, inventing a "sanitize then trust" mint, or stapling clean taint at the later sink.

**Why it happens:**
Single-node demos never need cross-node handle reuse. Multi-step coding loops *require* routing `output_value_id` (already wired but unused beyond binding in `worker.rs`) into subsequent `PlanArg`s. The instinct is "the test passed, so the output is trusted" or "the planner rewrote the string so it's clean." CaMeL-class defenses fail the moment the planner can retype literals or clear taint.

**How to avoid:**
- Intermediate outputs may only enter later nodes as opaque `ValueId`s minted by existing untrusted-origin sites (`mint_from_exec`, `mint_from_http`, `mint_from_read`, `mint_from_derivation` with provenance union).
- Never open ProvideIntent mid-stream to "re-declare" an observation as intent.
- No new mint site without Gate 3 extension + design gate. No "planner-authored literal" path.
- I2 sensitivity tables must treat exec/http/read-derived labels as blocking on content/routing-sensitive slots (the reopening tripwire from `DESIGN-session-trust-coherence.md` §5).

**Warning signs:**
- A new `mint_from_*` helper outside quarantine/server sanctioned loci
- ProvideIntent accepted after RequestFd or after a prior SubmitPlanNode
- Tests that assert `Allowed` on a PR body / push refspec built from exec stdout without a human confirm
- Provenance chain that does not root on the real `process_exited` / `http_response_received` / `file_read` event

**Phase to address:**
**Design gate + plan-stream substrate** (first multi-node TCB phases). Negative LIVE leg: tainted intermediate → later sensitive sink → I2 Block with genuine chain.

---

### Pitfall 2: ProvideIntent / UserTrusted as a multi-step laundering valve

**What goes wrong:**
To feed multi-step plans, someone re-opens ProvideIntent (or adds a second "declare trusted args" verb) so the worker can inject fresh `UserTrusted` literals mid-session. That reopens the v1.3/v1.4 ProvideIntent hole class: `UserTrusted == human-typed` collapses.

**Why it happens:**
ProvideIntent is the *only* mint that yields TRUSTED from a supplied string. Multi-step coding needs many string args (paths, commit messages, refspecs, PR titles). The "easy" fix is more ProvideIntent. v1.9 already caught the M7 variant (`--seed-from-file` laundered as trusted).

**How to avoid:**
- Keep ProvideIntent exactly-once, only before RequestFd, on the worker connection.
- Multi-step trusted args must be minted at session start from the operator's typed intent / CLI argv (or remain fixed policy-bound constants), not re-supplied after observations.
- File/env/seed content stays `mint_from_read` TAINTED (M7 discipline).
- Planner connection still has zero mint verbs.

**Warning signs:**
- Proto change that allows ProvideIntent after SubmitPlanNode
- "Re-intent" / "refresh intent" / "supply more trusted args" IPC
- Coding planner that embeds free-form strings into PlanNode instead of ValueIds

**Phase to address:**
**Design gate** (lock the multi-step trust model) + **CLI multi-node driver** (where argv/intent binding lands).

---

### Pitfall 3: Mid-loop I1 demotion silently breaks or is "fixed" by weakening Draft rules

**What goes wrong:**
Coding loop: edit → `process.exec` (tests) → commit → push → PR. Any untrusted read/http/exec mint demotes the session to Draft. Later `CommitIrreversible` sinks (`git.push`, `github.pr`, `http.request.write`) hard-Deny at Step 0.5 — either the product loop is dead after first observation, or someone "fixes" it by letting Draft auto-authorize Tier 3+ (I0/I1 breach).

**Why it happens:**
v1.2 designed demotion for single-effect sessions. Multi-step coding *needs* Observe/MutateReversible after demotion *and* a human gate before CommitIrreversible — but product pressure will demand auto-push after green tests.

**How to avoid:**
- Keep Draft → deny CommitIrreversible (no auto-authorize). Confirm/release remains the only path for blocked irreversible effects; class-deny is not confirmable as a standing waiver.
- Design the coding loop so irreversible nodes are either (a) pre-planned with only UserTrusted args and no demotion trigger before them, or (b) explicitly Block→confirm, not silently Allowed after demotion.
- Document which nodes may run post-demotion (Observe / MutateReversible) vs which require Active or confirm.
- Do not source `session_status` from PlanNode or worker IPC.

**Warning signs:**
- Special-case "coding session stays Active after exec mint"
- Draft + CommitIrreversible → Allowed in any test
- Planner that assumes push/PR always Allowed after green tests

**Phase to address:**
**Design gate** (session-trust × multi-node) + **mid-loop confirm continuity** phase + **LIVE negative** (demotion then irreversible attempt).

---

### Pitfall 4: Hybrid composition rebranded as CLI-driven multi-step (framing honesty)

**What goes wrong:**
v1.10's DONE claim is a design partner drives edit→test→commit→push→PR as **one Session via the CLI**, not hybrid in-crate composition. The trap: keep the v1.9 hybrid (in-crate `evaluate_plan_node_and_record_for_test` chain + one decorative `caprun run` leg) and market it as multi-step CLI — the exact overclaim v1.9 explicitly refused.

**Why it happens:**
Hybrid is faster to ship, already green at 696/0, and looks identical in the audit DB if you only check `verify_chain`. Product narrative pressure ("driven via caprun run") collides with single-node CLI reality.

**How to avoid:**
- DONE gate requires a genuine multi-node path driven by `caprun run` (or a named equivalent verb) in one Session — not a test-only composer.
- Machine-check framing: forbid claims that a single-node driver drove the whole chain; require the SUCCESS path to be CLI-orchestrated.
- In-crate composition may remain as a *unit* harness, never as the LIVE DONE evidence.
- DOC-01 lineage: state bluntly what the CLI does and does not drive.

**Warning signs:**
- LIVE test imports `evaluate_plan_node_and_record_for_test` for the "CLI" success path
- Acceptance prose says "driven via caprun run" while the multi-node chain never enters the binary's run path
- Only email/file intents still exist at DONE

**Phase to address:**
**CLI multi-node driver** + **non-hybrid LIVE proof** (last functional phase). Design gate should pin the acceptance shape early.

---

### Pitfall 5: One-shot Planner trait extended carelessly (control/data flow collapse)

**What goes wrong:**
Today `Planner::plan(...) -> PlanNode` is one-shot, handle-only. Multi-node needs a stream/iterator/loop. Bad extensions: (1) planner returns literals, (2) planner receives full decision anchors/literals as "feedback," (3) planner gets mint verbs, (4) planner sees raw tool output bytes to "decide next step."

**Why it happens:**
Agent frameworks feed tool results back into the model context. That is the default mental model and exactly what I1/I2 forbid for privileged planning over irreversible sinks.

**How to avoid:**
- Extend the seam to multi-node *without* widening the type boundary: still only opaque ValueIds + typed intent; observations enter as handles, not strings.
- Planner-role connection keeps reduced decision signal (allow/block-ish, no anchors/literals/sha256) — `DESIGN-session-trust-coherence.md` §7.
- Deterministic multi-step coding planner first (scripted node sequence); LLM multi-step tool-use stays out of v1.10.
- Worker (or broker-mediated loop) owns the stream state machine; planner proposes nodes, never effects.

**Warning signs:**
- Trait method taking `String` tool output or `ValueRecord`
- Planner sidecar logging/receiving blocked literals
- "Replan from observation text" in the privileged planner path for CommitIrreversible

**Phase to address:**
**Design gate + plan-stream substrate**; deterministic coding planner implements the locked seam only.

---

### Pitfall 6: Audit gap — terminal state before terminal event (amplified by multi-node)

**What goes wrong:**
Confirm burns the one-shot pending row (or CAS commits) before the sink's terminal audit event is appended; a failure leaves `confirm_granted` / "effect released" without `*_succeeded`/`*_failed`. Multi-step multiplies confirm-release sites (push, PR, http-write, exec) in one Session — one gap poisons the whole chain's honesty.

**Why it happens:**
Recurring class (v1.7 P33/P34, applied to v1.9 write sinks). Multi-step adds more Step-7 dispatch arms and mid-loop confirm UX pressure.

**How to avoid:**
- Standing checklist per confirm-release sink: precheck-before-burn; every failure path appends a terminal event; no mint on confirm-release path.
- Multi-node LIVE must `verify_chain` across the whole Session and assert terminal events for every released effect.
- Never treat "pending row cleared" as success.

**Warning signs:**
- Confirm returns success while audit lacks terminal event
- `?` after burning confirmation
- New sink copy-pastes confirm path without Step-4.8-style precheck

**Phase to address:**
Every phase that touches confirm-release or multi-node dispatch; **LIVE proof** re-verifies composition. Design gate restates the checklist.

---

### Pitfall 7: Mid-loop Block/confirm breaks one-Session continuity

**What goes wrong:**
Node mid-stream Blocks (I2 or always-confirm `git.push`). Worker exits non-zero. Human runs `caprun confirm` in a **separate process**. Multi-step either (a) dies and never resumes remaining nodes, (b) starts a **new Session** for the tail (splits the audit DAG / rebinds policy), or (c) auto-continues after confirm without re-checking I2 on residual args.

**Why it happens:**
Confirm was designed as durable pause-and-resume for one blocked effect, not as a stream scheduler. Coding-loop UX wants "confirm push then open PR" in one narrative Session.

**How to avoid:**
- Pin resume semantics in the design gate: same Session id, same policy_bound, same ValueStore/session trust cell (or durable equivalents), chain continues on the same audit DAG.
- Confirm remains single-shot on the resolved snapshot — not a session-wide waiver for later nodes.
- After confirm-release, subsequent nodes still go through full `submit_plan_node` (policy + I2 + slot-type).
- CLI surfaces blocked `effect_id` + how to resume the stream (not only how to confirm one effect).

**Warning signs:**
- Tail of the coding loop creates a new session_id
- Policy re-read from worker-reachable path on resume
- "Confirmed session may skip I2"

**Phase to address:**
**Mid-loop confirm / session continuity** phase; exercised in **LIVE** (success path includes at least one confirm gate, e.g. git.push).

---

### Pitfall 8: Policy-vs-I2 boundary erosion under multi-step convenience

**What goes wrong:**
A multi-step coding policy "allows the whole loop" and is mistaken for safety — or policy is mutated mid-session after a failed node — or policy is used to override an I2 Block ("user said auto-push"). Distinct `policy_deny` vs `sink_blocked` collapses into one "denied" UX.

**Why it happens:**
v1.9 POLICY-02 is structural only if every path still runs I2 after policy. Multi-step productization invites "policy grants the workflow" narratives.

**How to avoid:**
- Policy bound once at session creation from a trusted source outside the worker (F1); immutable for the Session; `policy_bound` hash-chained.
- Policy pre-I2 only; never disables I2; never grants taint clearance.
- LIVE negative legs keep distinct tags: `code()=="policy_deny"` vs `sink_blocked` under a policy that *permits* the sink.
- Multi-node does not re-bind policy per node.

**Warning signs:**
- Policy file fields like `ignore_taint` / `auto_confirm` / `trust_exec_output`
- Mid-session policy update IPC
- Tests where policy-permit alone makes tainted push Allowed

**Phase to address:**
**Design gate** (reaffirm POLICY-02) + **LIVE negatives**; CLI phase only binds policy at start.

---

### Pitfall 9: Gate 3 mint-site discipline drift under "just one more helper"

**What goes wrong:**
Multi-step needs more observation types (test output parsing, git status, PR URL). Someone adds `mint_from_parse` / `ValueStore::mint` in the planner, adapter, or CLI — stapling taint or minting trusted outside quarantine/server.

**Why it happens:**
Each new observation feels like a one-line mint. Gate 3 only greps known tokens; a renamed helper can slip if not extended.

**How to avoid:**
- New observation kinds extend an existing mint helper or add a Gate-3-tracked token with design-gate approval.
- Prefer claims + `mint_from_read`/`mint_from_derivation` over new sites.
- `check-invariants.sh` Gate 3 stays in CI; multi-step PRs must not weaken it.
- Confirm-release paths mint nothing (durable taint on the effect event only).

**Warning signs:**
- Gate 3 allowlist growth without DESIGN amendment
- Mint calls in `cli/caprun` or planner crates
- Provenance rooted on "planner_step" instead of a real effect/read event

**Phase to address:**
**Design gate + every implementation phase**; invariants gate is continuous.

---

### Pitfall 10: BiDi / display spoof on multi-step confirm and audit surfaces

**What goes wrong:**
Attacker-tainted intermediate strings (refspec, PR title, test output summarized into confirm UX) use Trojan-Source BiDi/zero-width characters to reverse or hide what the human confirms. v1.9 fixed this once for confirm/audit; multi-step adds more display surfaces and resume prompts.

**Why it happens:**
CVE-2021-42574 class; any new renderer that prints attacker-tainted literals without `neutralize_control_chars` (Cf + Cc) reopens it.

**How to avoid:**
- All human-facing decision surfaces (confirm, review, audit, mid-loop "next node" summaries) share the hardened neutralizer.
- Anti-drift tests: new display paths must call the shared helper.
- Never print raw tool output as the confirm binding without neutralization + full-set digest binding where applicable.

**Warning signs:**
- New `println!` of sink args without neutralize
- Confirm prompt built from exec stdout verbatim
- Tests only covering ASCII hostile fixtures

**Phase to address:**
**CLI / confirm continuity** phases; regression in **LIVE**.

---

### Pitfall 11: DAG-fork / chain-head parenting in multi-event streams

**What goes wrong:**
Multi-node appends many events. Code that parents onto a *named* prior event (`sink_blocked`, `file_read`) instead of the **current chain head** forks the DAG; `verify_chain` fails or, worse, a lax test only checks a subgraph.

**Why it happens:**
Recurring when single-session templates are copied into multi-event composition (v1.3 Phase 17 finding). Multi-step is that class by default.

**How to avoid:**
- Always append onto current chain head under the session lock.
- LIVE asserts full-session `verify_chain` true after the entire coding loop (and after confirm).
- Composition tests must not reuse single-node parenting patterns blindly.

**Warning signs:**
- `parent_id: Some(specific_event)` hardcoded to a mid-stream event type
- verify_chain only on a prefix of the session
- Parallel node submission without a defined chain order

**Phase to address:**
**Plan-stream substrate** + **LIVE proof**.

---

### Pitfall 12: cfg(linux) test-blindness on the multi-step LIVE gate

**What goes wrong:**
Multi-step e2e is Linux-gated (confinement + real sinks). macOS/`cargo test` looks green while the multi-node CLI path never ran. Scoped Linux runs hide defects that only full `compose-verify` / `mailpit-verify` catch (v1.9 P44 hit this twice).

**Why it happens:**
Standing project hazard; multi-step adds more `#[cfg(target_os = "linux")]` surface.

**How to avoid:**
- Authoritative DONE = full compose-verify (or mailpit-verify where SMTP is involved) on real Linux, true exit before pipe.
- Do not accept macOS "0 passed" Linux suites as coverage.
- Independent orchestrator re-run of the LIVE multi-step proof at milestone close.

**Warning signs:**
- Phase SUMMARY claims multi-step proven without compose-verify counts
- Only host-portable unit tests for the stream state machine
- Scoped `cargo test -p …` as the sole Linux signal

**Phase to address:**
**Every phase with Linux-gated code**; hard gate on **LIVE proof**.

---

### Pitfall 13: Becoming an "agent framework" (product boundary collapse)

**What goes wrong:**
Multi-step coding loop accretes retries, tool registries, memory, plugins, LLM tool-use loops, web UI — diluting the Intent Runtime boundary and expanding TCB.

**Why it happens:**
v1.10 is the first product-shaped loop; "while we're here" pressure is high.

**How to avoid:**
- Hard out-of-scope: LLM multi-step tool-use, Cedar, new sink families, pack-cap lift, cross-host/gVisor, web UI, marketplace, memory.
- Deterministic scripted multi-node planner first.
- Manual-ops-first; design-gate + fresh adversarial code-trace before multi-step TCB.

**Warning signs:**
- Generic tool-registry abstraction
- Retry/orchestration framework crates
- Scope PRs that add sinks "needed for a better loop"

**Phase to address:**
**Milestone scoping / design gate**; enforced at plan-checker time.

---

### Pitfall 14: Standing confirmation / session-wide waiver disguised as multi-step UX

**What goes wrong:**
"Confirm once to allow the rest of the coding loop" — converts single-shot (sink, arg, literal-digest) release into a standing policy, bypassing I2 on later nodes.

**Why it happens:**
Humans hate N confirms for push+PR+http. Multi-step UX will request batching.

**How to avoid:**
- Confirm stays single-shot per pending effect snapshot.
- Batch UX may *present* multiple pendings; each release is still per-effect_id with its own digest.
- No `session.confirm_all_future_irreversible`.

**Warning signs:**
- Confirm API taking session_id without effect_id
- Pending row that wildcards args
- Tests that one confirm Allows a later different plan node

**Phase to address:**
**Mid-loop confirm continuity** design + implementation.

---

### Pitfall 15: Replay / duplicate effects across multi-node adaptive loops

**What goes wrong:**
Stream retries a node on failure or re-submits the same Allowed `SubmitPlanNode` (exec, commit, send). Amplification of trusted effects (duplicate commits/PRs/pushes) or confusing audit trails. v1.6 CAS covers Allowed email.send; not all sinks.

**Why it happens:**
Multi-step loops retry by nature. Adaptive planners (future) benefit from duplication.

**How to avoid:**
- Per-sink at-most-once where CommitIrreversible (existing CAS patterns for email/PR; push always-confirm + frozen oid).
- Deterministic v1.10 planner should not blindly retry irreversible nodes.
- Audit must make duplicate attempts visible.

**Warning signs:**
- Unconditional retry around SubmitPlanNode for push/PR
- Missing idempotency on new multi-step sinks

**Phase to address:**
**Plan-stream substrate** (retry policy) + sink-specific phases if new dispatch; document residual if accepted.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| In-crate multi-node composer as LIVE DONE | Ships "loop" without CLI stream | Framing lie; product gap remains | Unit tests only — never DONE claim |
| LLM multi-step tool-use in v1.10 | "Real agent" demo | I1 feedback channel; huge TCB | Never this milestone |
| Re-ProvideIntent mid-stream for convenience | Easy trusted args | Launders UserTrusted | Never |
| Draft stays Active after exec "for coding UX" | Loop completes auto | I1 dead | Never |
| Session-wide confirm waiver | Fewer human prompts | I2 bypass | Never |
| Scoped Linux tests only | Faster CI | cfg-blindness ships holes | Never for DONE; OK for mid-phase iteration if full gate follows |
| New mint helper without Gate 3 | Quick observation type | Stapling / laundering | Never without design gate |
| Hybrid + honest DOC footnote as v1.10 DONE | Reuses v1.9 | Fails milestone goal | Never — that was v1.9's ratified interim |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `Planner` trait → multi-node | Change return to graphs carrying literals | Stream of PlanNodes still handle-only; worker/broker owns loop |
| `output_value_id` → next PlanArg | Treat as trusted string | Keep as tainted ValueId; I2 at later sink |
| `caprun confirm` mid-loop | New session for remaining nodes | Same Session, durable pending, resume stream |
| `caprun run` + policy | Re-bind policy each node / from workspace file worker can write | Bind once at session create from trusted path (F1) |
| `git.push` always-confirm | Auto-dispatch after green tests | Keep always-confirm; stream pauses; human releases |
| Planner sidecar (future LLM) | Feed tool transcripts into privileged planner | Out of v1.10; if ever: extracts/handles only, reduced decision signal |
| Audit viewer / confirm UX | Render multi-step tainted summaries raw | Shared BiDi/control neutralization |
| compose-verify / mailpit-verify | Skip because "no email in coding loop" | Full Linux recipe still authoritative for confinement + sinks |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Unbounded plan stream length | Session hangs, audit DB growth, RequestFd exhaustion | Cap nodes per session; existing RequestFd limiter (256) | Hostile/runaway planner |
| Full compose-verify per micro-commit | Slow iteration | Scoped tests mid-phase; full gate at phase/milestone close | Not a prod scale issue — process trap |
| Per-node process spawn of full broker | Multi-second node latency | One broker + one Session for the stream | Design-partner demos feel broken |
| Holding huge exec stdout in ValueStore | Memory pressure | Existing output caps; don't mint multi-MB as sink args | Large test logs |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Intermediate tool output → trusted sink arg without confirm | Value injection / exfil (I2 break) | Taint-preserving mints + I2 sensitivity |
| Mid-stream ProvideIntent | UserTrusted laundering | Once-before-RequestFd only |
| Policy override of I2 | Irreversible effect with attacker value | POLICY-02 structural order |
| Draft auto CommitIrreversible | I0/I1 break | Executor Step 0.5 unchanged |
| Planner sees Block anchors/literals | Oracle + replan laundering | Reduced decision signal |
| New mint outside Gate 3 | Stapled/fake provenance | Invariants gate + design gate |
| Confirm-all / standing waiver | I2 bypass | Single-shot effect_id only |
| BiDi in multi-step confirm text | Human confirms wrong literal | Shared neutralization |
| Second connection / mint-capable planner | Cross-connection trust bypass class | Occupancy latch + capability set |
| Hybrid LIVE as "CLI multi-step" | False assurance of product security story | Non-hybrid DONE gate |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent stop after mid-loop Block | Partner thinks run failed mysteriously | Print effect_id, review pointer, resume instructions |
| N opaque confirms without context | Rubber-stamp risk | Verbatim neutralized literals + which node in the stream |
| "Policy allows coding loop" as safety story | False confidence | Separate policy_deny vs I2 Block messaging |
| Hybrid demo sold as CLI | Design partner can't reproduce via CLI | Ship real multi-node `caprun run` |
| Auto-retry irreversible effects | Duplicate push/PR | Explicit confirm; no silent retry |

---

## "Looks Done But Isn't" Checklist

- [ ] **Multi-node stream:** Worker/CLI submits **sequence** of PlanNodes in one Session — not one PlanNode only
- [ ] **CLI driver:** SUCCESS path is driven by `caprun run` (or named multi-node verb), not only `evaluate_plan_node_and_record_for_test`
- [ ] **One Session:** Single session_id, one `policy_bound`, `verify_chain` true across the full loop
- [ ] **Genuine taint cross-node:** Intermediate `output_value_id` / reads keep provenance roots; anti-staple holds on mid-loop Block
- [ ] **Mid-loop I2 Block:** LIVE negative (tainted PR body or push refspec) via CLI, not in-crate only
- [ ] **Confirm continuity:** At least one confirm-gated node (e.g. git.push) in SUCCESS path; resume without new Session
- [ ] **ProvideIntent:** Still exactly-once; no mid-stream trusted re-mint
- [ ] **Draft rules:** Demotion still denies CommitIrreversible; no coding exception
- [ ] **Policy-vs-I2:** Distinct outcomes under multi-node policy
- [ ] **Gate 3:** No unsanctioned mint sites
- [ ] **Display:** BiDi neutralization on all new confirm/audit/resume surfaces
- [ ] **Linux gate:** Full compose-verify green; not macOS-only
- [ ] **Framing honesty:** PROJECT/README/milestone record do not claim hybrid as CLI multi-step
- [ ] **Product boundary:** No LLM multi-step tool-use / agent-framework scope creep
- [ ] **Audit gap:** Every confirm-release has terminal event; chain head parenting correct

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Intermediate taint laundering shipped | HIGH | Revoke mint path; restore Gate 3; add LIVE anti-launder leg; design-gate amendment |
| Hybrid sold as DONE | MEDIUM | Reopen LIVE phase; wire real CLI stream; fix framing; re-verify |
| Draft weakened for coding UX | HIGH | Revert executor exception; redesign loop around confirm |
| ProvideIntent mid-stream | HIGH | Re-close guard(a); session latch regression; adversarial re-trace |
| Audit gap on new stream confirm | MEDIUM | Precheck-before-burn fix pattern (P34); regression test; re-trace |
| BiDi on new surface | LOW–MEDIUM | Route through shared neutralizer; anti-drift test |
| cfg-linux blindness | MEDIUM | Full compose-verify; fix latent defects; ban scoped-only DONE |
| Policy mutability mid-session | MEDIUM | Bind-once enforcement; refuse update IPC; LIVE immutability test |

---

## Pitfall-to-Phase Mapping

Suggested v1.10 phase skeleton (numbers TBD by roadmapper; order is the load-bearing part):

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| P5 Planner seam collapse; P2 ProvideIntent; P3 Draft×loop; P8 Policy; P13 boundary; P14 confirm waiver | **Phase A — Design gate** (`DESIGN-multi-node-plan-stream.md` + fresh adversarial trace) | APPROVED gate record; no TCB code before |
| P1 cross-node taint; P9 Gate 3; P11 DAG fork; P15 retry/replay policy | **Phase B — Plan-stream substrate** (multi SubmitPlanNode / one Session, handle routing, chain head) | Unit + Linux: N nodes, verify_chain, no new mint sites |
| P1 routing for coding sinks; P5 deterministic planner only | **Phase C — Deterministic multi-step coding planner** (edit→test→commit→push→PR script) | Scripted sequence emits handle-only nodes; no LLM loop |
| P4 hybrid honesty; P2 intent binding; P8 policy bind-once; P10 display | **Phase D — CLI multi-node `caprun run`** | Real binary drives ≥2 nodes; framing machine-check |
| P7 resume; P6 audit gap; P10 BiDi; P14 single-shot | **Phase E — Mid-loop Block/confirm continuity** | Block→confirm→resume same Session; terminal events present |
| P4 non-hybrid DONE; P1/P8 negatives; P12 Linux; P11 chain | **Phase F — Non-hybrid LIVE proof** | compose-verify full workspace; CLI success + mid-loop I2 Block; honest milestone record |
| Packaging only | **Phase G — Minimal Linux packaging** (if in scope) | Install path doc; no security claim change |

**Ordering rationale:**
1. Design gate first — multi-step reopens I0/I1/I2 composition questions (demotion × irreversible tail, ProvideIntent, confirm resume) that must not be invented mid-code (v1.2 B1 lesson).
2. Substrate before planner — stream/session/chain must exist before a coding script piles nodes on.
3. Deterministic planner before CLI — pure sequence logic testable without full product verb.
4. CLI before LIVE — DONE forbids hybrid; CLI must exist to be proven.
5. Confirm continuity before final LIVE — SUCCESS path includes git.push always-confirm.
6. LIVE last — composition re-proves, does not assume (v1.3 Phase 17 pattern).

**Research flags for phases:**
- Phase A: **Needs deep design research** — resume semantics, demotion×coding loop, what may be UserTrusted at node 0 vs later.
- Phase B: **Needs careful TCB research** — ValueStore lifetime across nodes, planner reduced signal if dual-connection, chain locking.
- Phase C: Standard once seam locked — scripted planner, low novelty.
- Phase D: Integration-heavy — intent surface for coding workflow, packaging of multi-node intent.
- Phase E: **Needs research** — confirm is separate process; durable resume of a *stream* is new.
- Phase F: Standard LIVE discipline (compose-verify, independent re-run, framing honesty) — high process risk, low design novelty if A–E held.
- Phase G: Ops only.

---

## Sources

**Project (HIGH confidence — primary):**
- `.planning/PROJECT.md` — v1.10 goal; v1.9 hybrid disclosure; locked I0/I1/I2; M7 laundering; POLICY-02
- `.planning/RETROSPECTIVE.md` — cfg-linux blindness; audit-gap class; adversarial-trace defects; hybrid framing
- `.planning/milestones/v1.9-phases/46-composed-live-proof-v1-9-done/46-MILESTONE-RECORD.md` — hybrid layers bluntly stated
- `planning-docs/PLAN.md` — architectural lock plan nodes; §9 genuine taint
- `planning-docs/DESIGN-taint-model.md` — I0/I1/I2; hard planner/worker split Tier 3+
- `planning-docs/DESIGN-plan-executor.md` — handle model; anti strip/staple
- `planning-docs/DESIGN-session-trust-coherence.md` — ProvideIntent sole trusted-from-string mint; planner reduced signal; replay re-earn
- `planning-docs/DESIGN-session-trust-state.md` — Draft demotion; session_status not from PlanNode
- `planning-docs/DESIGN-confirmation-release.md` — durable pause/resume; resolved snapshot; single-shot
- `planning-docs/DESIGN-v1.9-egress-policy.md` — policy pre-I2; always-confirm push
- `cli/caprun/src/planner.rs` / `worker.rs` — one-shot Planner; unused `output_value_id` for later nodes
- `crates/brokerd/src/server.rs` / `proto.rs` — ProvideIntent once; planner capability; output_value_id
- `scripts/check-invariants.sh` — Gate 3 mint-site restriction

**External prior art (MEDIUM — supporting, not authoritative for caprun):**
- CaMeL — Debenedetti et al., "Defeating Prompt Injections by Design," arXiv:2503.18813 (control/data flow separation; capabilities on tool calls; P-LLM never retypes values)
- Project-cited FIDES (arXiv:2505.23643) information-flow framing in DESIGN-taint-model.md
- CVE-2021-42574 Trojan Source — BiDi confirm-prompt spoof class (fixed v1.9 P45; reopens on new surfaces)

**Not used as authority:** Generic "agent framework best practices" blogs; training-recall without project grounding.

---
*Pitfalls research for: caprun v1.10 multi-step Safe Coding Agent loop on I0/I1/I2 substrate*
*Researched: 2026-07-23*
