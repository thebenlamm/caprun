# DESIGN Review — v1.2 pair, Round 1

**Reviewed:** `DESIGN-session-trust-state.md` (446 lines), `DESIGN-confirmation-release.md` (370 lines)
**Reviewer basis:** every load-bearing claim checked against code on `main` @ `2c90278`+ (session.rs, plan_node.rs, quarantine.rs, server.rs, sink_schema.rs, executor_decision.rs, s9_live_block.rs, REQUIREMENTS.md).
**Verdict: CHANGES REQUIRED — 1 blocker, 3 major, 3 minor.** The docs are individually strong; the blocker is a composition bug between the two mechanisms that makes Phase 11's acceptance requirements unsatisfiable as specified.

---

## B1 (BLOCKER) — Step 0.5 placement makes ACC-01/ACC-02 unsatisfiable and breaks the v1.1 §9 live test

**The chain, each link verified in code:**

1. In the live hostile flow, the broker mints tainted values via `mint_from_read` when handling
   `ReportClaims` (`crates/brokerd/src/server.rs:328`). Per TAINT-01 / doc 1 §2, that mint demotes
   the session to `Draft` — *before* the worker submits its plan node.
2. Doc 1 §8 places the draft-only deny at **Step 0.5, before the per-arg loop**: `Draft` +
   `CommitIrreversible` → `Denied` immediately.
3. Both live sinks (`email.send`, `file.create` — the only entries in `KNOWN_SINKS`,
   `crates/executor/src/sink_schema.rs:40`) are classed `CommitIrreversible` by doc 1 §6.

**Therefore:** every hostile live run returns `Denied` at Step 0.5 and **never reaches the Step 2
I2 taint Block**. Consequences:

- **ACC-01 is unsatisfiable** — it requires "session demoted (I1) → tainted routing arg **Blocked
  (I2)** → human denies" in one run. Under this design the Block never fires on a demoted session.
- **ACC-02 is unsatisfiable** — no `BlockedPendingConfirmation` → no `sink_blocked` event → no
  `PendingConfirmation` row → `caprun confirm` has nothing to release. The entire
  confirmation-release mechanism (doc 2) is unreachable in any live flow.
- **The v1.1 §9 test breaks** — `cli/caprun/tests/s9_live_block.rs:243-251` asserts a durable
  `sink_blocked` event with `Some(anchor)`, `anchor.sink == file.create`, `anchor.arg == path`.
  Post-Phase-9 that run yields `Denied` with no anchor. This directly contradicts doc 1 §9's
  non-regression claim ("protects the v1.1 §9 acceptance test unchanged ... Step 0.5's predicate
  requires Draft, so it never fires for an Active session") — the claim is vacuously true at the
  unit level and false at the e2e level, because after TAINT-01 **no live session with a tainted
  value is ever Active**.

**Root cause (shared with the seed doc — this reviewer's own):** MILESTONE-v1.2-SEED.md proposed
both "draft-only sessions deny CommitIrreversible" and an acceptance flow "demoted → Blocked →
confirm" without noticing they contradict under deny-before-block ordering. The requirements
inherited both halves; the DESIGN doc resolved the ambiguity in the direction that kills the
confirm path.

**Required fix (direction, not wording):** the per-arg I2 loop must take precedence over the
draft-only class deny. Concretely:

- Move the draft-only check **after** the per-arg loop (or equivalently: run the loop; if any arg
  Blocks, return `BlockedPendingConfirmation`; only then apply Draft+CommitIrreversible → Denied).
  Both outcomes are non-allow, so no fail-open is introduced by the reorder — ordering only decides
  *which* non-allow outcome wins, and ACC-01/02 require Block to win when a tainted arg is present.
- Resulting semantics are coherent and each mechanism keeps a reachable live path:
  - tainted routing-sensitive arg (any session state) → **Block** → human confirm/deny (I2 + release valve);
  - `Draft` session, `CommitIrreversible` sink, **no** blocking arg → **Denied** (I1/I0: clean-looking
    values, but instructions may be injected — this is exactly the case confirm must NOT release).
- Update doc 1 §8 + §11.3, and TAINT-02's reading: "denies `CommitIrreversible` plan nodes *that do
  not already Block on I2*". If GSD treats requirement text as locked, TAINT-02 needs that one-line
  amendment — flagging explicitly rather than silently reinterpreting.
- Update doc 2 "Two Independent Mechanisms": add that a confirm on a Draft session's Block **is**
  the human gate I0/I1 demand ("cannot **auto**-authorize Tier 3+" — a literal-value human confirm
  is not auto-authorization). State it explicitly so the interaction is designed, not incidental.

---

## M1 (MAJOR) — `PendingConfirmation.plan_node: PlanNode` cannot do its job

Doc 2's schema declares `plan_node: PlanNode`, but `PlanNode` is `{ sink, args: Vec<PlanArg { name,
value_id }> }` (`crates/runtime-core/src/plan_node.rs:108-125`) — **opaque handles only, no
literals**. Doc 2's own premise ("The Problem Being Solved") is that the in-memory `ValueStore` is
gone at confirm time, so those `value_id`s are unresolvable. The field-purpose table says the field
carries "every `PlanArg` together with its dereferenced `ValueRecord` (literal, taint,
provenance_chain)" — the declared type cannot carry that. As written, Step 4a.4 (invoke sink with
frozen args) is unimplementable.

**Fix:** define the persisted record as a resolved snapshot type, e.g.
`resolved_args: Vec<ResolvedArg { name, value_id, literal, taint, provenance_chain }>` + `sink`,
captured at Block time inside the same transaction. Keep `PlanNode` itself locked and untouched.

## M2 (MAJOR) — the deny input mechanism is unspecified and the mock output contradicts the non-interactive lock

Steps 4a/4b branch on "if the human selects confirm/deny," but the CLI contract defines only
`caprun confirm <effect_id> [audit-db-path]` — no deny subcommand, no flag — while the mock output
shows an interactive-looking `[ Confirm ]  [ Deny ]` chooser, and the same section locks "never an
interactive TTY prompt ... MUST be scriptable." Phase 10 cannot implement deny from this spec.

**Fix:** specify the invocation for each verb — recommend `caprun confirm <effect_id>` (confirm) and
`caprun deny <effect_id>` (deny), or a `--deny` flag; drop the `[ Confirm ] [ Deny ]` chooser from
the mock output (show the info block + the two commands to run instead).

## M3 (MAJOR) — `Confirmed`-before-invoke leaves an undefined "confirmed but never executed" terminal state

Step 4a order: (2) append `confirm_granted` + (3) state → `Confirmed` (atomic), then (4) invoke the
sink. If invocation fails — doc 2's own Residual Risk 2 names a live case, `O_EXCL` conflict — or
the process dies between (3) and (4), the state is terminally `Confirmed`, the effect never ran, and
Step 3 refuses any retry. At-most-once is arguably the *right* choice for irreversible sinks, but
the doc neither states it nor covers it in the exit-code table (which has no row for
"confirm recorded; sink invocation failed" — a scripted caller would read that non-zero exit as
deny/unknown-id).

**Fix:** state the at-most-once choice explicitly; add an exit-code row and a
`sink_invocation_failed` audit Event (anchored to the same `effect_id`) for the failure leg; state
that a crash between transition and invocation is an accepted at-most-once loss, visible in the DAG
as `confirm_granted` with no subsequent invocation event.

---

## m1 (minor) — Step 0.5's equality check is a de-facto wildcard over `SessionStatus`

`*session_status == SessionStatus::Draft` silently passes `WaitingApproval`/`Done`/`Failed`/
`RolledBack` through to the allow-capable path — the same shape doc 1 §10 forbids for
`EffectClass`/`DenyReason`. Either require an exhaustive `match` (`Active` → proceed, `Draft` →
class check, all others → `Denied`, fail-closed) or document why a non-`Active`/`Draft` session can
never reach `submit_plan_node`.

## m2 (minor) — TAINT-03 has no live sink to exercise

Both registered sinks are `CommitIrreversible`, so "Draft still allows Observe/MutateReversible" is
only testable against a fake registry or a test-only sink in `KNOWN_SINKS`. Name the chosen vehicle
in doc 1 so Phase 9's verifier doesn't stall on an untestable requirement.

## m3 (minor) — redaction interplay with the new side table

`blocked_literals` is redactable by design (v1.1). The `PendingConfirmation` row will hold the same
blocked literal *plus every other arg's literal*. One sentence needed: redaction of a blocked
literal MUST also cover the pending row (or a post-redaction `caprun confirm` MUST refuse).

---

## What's right (keep verbatim)

- Monotonic one-way `Active → Draft`; sole-flip-site anti-self-declaration (§2) — exactly the right
  extension of the T-04-03 discipline to session state.
- Broker-resolved `session_status` parameter, never IPC/PlanNode-carried (§4) — closes the
  self-declaration hole correctly.
- **"Confirm MUST NOT re-invoke `submit_plan_node`"** — the soundest section in either doc; the
  two-failure-modes argument is exactly why release must be a distinct endorsement path.
- `PendingConfirmation` as a sibling side-table record (blocked_literals precedent), not an anchor
  extension — preserves the golden-byte contract.
- Fail-closed unknown `effect_id`, terminal-state refusal, single-shot semantics, and the
  exit-code philosophy (0 only on release).
- Unknown-sink → `CommitIrreversible` fail-closed default with the dead-branch justification (§6).

## Suggested resolution order (blast radius)

1. **B1** — decide the ordering semantics + TAINT-02 amendment (gates everything; touches security semantics and both docs).
2. **M1** — resolved-snapshot schema (data integrity of the checkpoint).
3. **M2, M3** — CLI verb spec + at-most-once contract (Phase 10 implementability).
4. **m1–m3** — one edit each, author's discretion, before GATE-RECORD approval.

Gate recommendation: record **REVISE** in `DESIGN-GATE-RECORD-v1.2.md`; re-review of the amended
sections only (not full docs) is sufficient for round 2.
