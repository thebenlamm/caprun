# DESIGN — Multi-step Plan Stream: sequential orchestration on the existing Planner seam

**Milestone:** v1.10 — Multi-step Safe Coding Agent Loop
**Phase:** 47 (Design Gate) — blocks all multi-step TCB code under
`crates/{executor,brokerd,sandbox,runtime-core}` and the worker submit /
confirm-hold path in `cli/caprun`
**Status:** Draft → pending a fresh, **non-self, orchestrator-owned** adversarial
code-trace (DESIGN-20) to be recorded in `planning-docs/DESIGN-GATE-RECORD-v1.10.md`.
This doc is authored by a `gsd-executor`; the executor does **not** run or
self-perform that trace (gsd-executors have no Agent tool — §13).
**Author date:** 2026-07-23
**Grounding:** `.planning/research/{SUMMARY,ARCHITECTURE,PITFALLS}.md` (v1.10
milestone research), `.planning/REQUIREMENTS.md` (DESIGN-19/20, HYG-02,
STREAM/CODE/CLI/CONFIRM/LIVE), `.planning/phases/47-multi-step-plan-stream-design-gate/47-RESEARCH.md`
(AUTHORITATIVE outline), and the v1.9 twin `planning-docs/DESIGN-v1.9-egress-policy.md`
(section shape + decisions-not-options voice). Every `file:line` below traces to a
direct code read this session; re-verify if Phases 48–52 begin many commits later,
per this project's own convention.
**Requirements:** DESIGN-19 (this doc) → enables STREAM-01/02 (Phase 48),
CODE-01/02 (Phase 49), CLI-01/02 + CONFIRM-01 (Phase 50), LIVE-07/08 (Phase 51).
DESIGN-20 is the gate that clears it (§13). HYG-02 is re-asserted here (§8).

> **Design-gate discipline.** No multi-step TCB / worker submit / confirm-hold
> code may be written under `crates/{executor,brokerd,sandbox,runtime-core}` or
> the multi-step path in `cli/caprun` until this document clears a fresh, non-self,
> **orchestrator-owned** adversarial code-trace with every BLOCKER/MAJOR resolved
> — the unbroken caprun precedent (v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23,
> v1.6 P26, v1.7 P31, v1.8 P35, v1.9 P41). This doc pins **decisions**, not
> options. Multi-step security failures here are almost all **composition** of
> already-correct single-node controls; the design gate exists so Phases 48–52
> cannot invent laundering valves mid-code.

---

## §0. Purpose & Scope

**What this doc pins (DESIGN-19).** The multi-step orchestration composition
contract on the **shipped single-node substrate**, before any multi-step TCB or
worker-loop code exists:

1. **Plan-stream shape** (§1) — sequential multi-node on the existing `Planner`
   seam: one Session, one worker connection, N sequential `SubmitPlanNode`
   calls; each node independently policy-pre-I2 then I2. Additive multi-node
   Planner surface; **not** batch DAG authorize; **not** a free-form tool-map
   effect path.
2. **Worker sequential submit + handle bag** (§2) — replace the one-shot worker
   with a sequential loop + worker-side bag of opaque `ValueId`s from
   `PlanNodeDecision.output_value_id`. Planner never mints. ProvideIntent
   remains **exactly once** before RequestFd.
3. **Mid-loop Block-and-Hold confirm continuity** (§3) — on
   `BlockedPendingConfirmation` (I2 Block **or** always-confirm `git.push`
   rewrite), worker holds the **same Session**; human confirms durable
   `PendingConfirmation`; worker does **not** re-submit the blocked node;
   remaining nodes continue under same Session id, same policy bind, same audit
   chain.
4. **I1×coding-loop bounds** (§4) — trusted-intent success path: coding args
   minted once at ProvideIntent from operator-typed CLI/intent; no weakening of
   CommitIrreversible Draft denies (Step 0.5) to green multi-file demotion.
5. **Instruction vs value channel disjointness** (§5) — PLAN-03 / GATE-01:
   `task_instruction` is never a bindable `ValueId`.
6. **Deny/abort mid-stream** (§6) — Deny / `policy_deny` abort remaining nodes
   fail-closed; Block holds; sequential order only.

Plus: **carry-forward invariants** (§7), **HYG-02 / Gate discipline** (§8), a
**§-per-pitfall threat model** (§9), **invariant preservation** (§10),
**fail-closed defaults** (§11), **new-symbol summary** (§12), the
**orchestrator-owned adversarial-trace gate** (§13, DESIGN-20), and the
**acceptance predicate** (§14).

**What is deferred (out of this design gate and out of v1.10 TCB):**

- LLM multi-step / ReAct tool-use loop (LLM-MS-01) — deferred past v1.10; this
  gate must not invent a free-form tool-use scheduler in the TCB.
- New sinks (merge/comment, richer recipes) — CODE-BREADTH-01.
- Session-wide confirm waiver / "workflow YOLO" — permanently rejected, not deferred.
- Agent frameworks, Temporal/LangGraph-class orchestration crates, memory,
  marketplace, Cedar, web UI, cross-host/Biscuit, gVisor/Firecracker — product
  boundary (Intent Runtime, not agent framework).

**Hard-blocks Phases 48–52.** No multi-step worker loop, coding multi-node
planner, CLI multi-node driver, confirm-hold product path, LIVE multi-step
proof, or packaging-of-unproven multi-step layout may land until DESIGN-20
records CLEARED.

**DOC-ONLY this phase.** This doc lives entirely under `planning-docs/`. Plan
47-01's git diff touches only this file (plus optional `.planning/` summary).
`scripts/check-invariants.sh` stays green (prose under `planning-docs/` trips
no Gate that scans `crates/` or `cli/`).

**Locked terminology (unchanged):** `Intent`, `Session`, `Planner`, `Worker`,
`Broker`, `Adapter`, `Effect`, `Artifact`, `Event`. `ExecutionContext` stays
internal-only. Project / repo / v0 binary = `caprun`. Nothing here introduces
new public-API vocabulary.

**Effect path locked (unchanged).** The only authorized effect path is
`submit_plan_node(session_id, PlanNode { sink, args: Vec<PlanArg> })` where each
`PlanArg` is an opaque `ValueId` (`crates/runtime-core/src/plan_node.rs:122-139`).
No raw plan-node-bypass effect path may exist under `crates/` — `check-invariants.sh`
Gate 1 fails the build if the free-form effect-request token appears under
`crates/` (annotate intentional mentions with the project's planner-discipline
allow marker). This DESIGN never introduces that path.

**Shipped substrate this composition rides (gap = composition, not physics):**

| Component | Status through v1.9 | Multi-step gap |
|-----------|---------------------|----------------|
| Broker N× `SubmitPlanNode` on one connection | Legal (`server.rs` connection loop) | Worker one-shot |
| `PlanNodeDecision.output_value_id` | Wired (`proto.rs:251-254`) | Worker discards (`worker.rs:389`) |
| Confirm durable single-shot snapshot | Shipped (`DESIGN-confirmation-release.md`) | Not mid-loop hold |
| ProvideIntent once before RequestFd | Broker-enforced (`server.rs:565-566`, `:2370-2391`) | Must not reopen |
| Per-node policy pre-I2 then I2 | Shipped (`evaluate_plan_node_and_record`) | No batch authorize |
| `caprun run` | Single-node email/file | Multi-node coding driver |
| LIVE multi-node | Hybrid in-crate (LIVE-05 honesty class) | CLI one Session (LIVE-07/08) |

---

## §1. Plan-stream shape (DESIGN-19a)

### 1.1 DECISION — sequential multi-node on the existing Planner seam

**THE stream shape for v1.10 is:**

1. **One Session** (broker-owned session id; HARD-03 — never from IPC).
2. **One worker connection** (self-confined after connect; Landlock deny-all +
   seccomp deny-execve post-connect order unchanged — `cli/caprun/src/worker.rs`
   module docs lines 1–57).
3. **N sequential `BrokerRequest::SubmitPlanNode { plan_node }` calls** on that
   connection (`crates/brokerd/src/proto.rs:136-138`).
4. **Each node independently** runs policy pre-I2 then I2 via
   `evaluate_plan_node_and_record` (`crates/brokerd/src/server.rs:740+` shared
   path; production arm at `server.rs:2221-2230`).

Broker multi-submit is **already legal**. The connection loop in
`handle_connection` (`server.rs:543+`, request loop at `:572+`) dispatches each
framed request independently and does not force single-submit-then-close. The
gap is the **worker loop + planner multi-node surface + CLI driver**, not a new
broker multi-node protocol.

### 1.2 DECISION — additive multi-node Planner surface; keep one-shot `plan()`

Today's `Planner` trait is one-shot (`cli/caprun/src/planner.rs:61-79`):

```70:79:cli/caprun/src/planner.rs
    fn plan(
        &self,
        intent: &CaprunIntent,
        intent_value_id: ValueId,
        derived_recipient: Option<ValueId>,
        body: Option<ValueId>,
        trusted_subject_handle: ValueId,
        trusted_body_handle: ValueId,
        task_instruction: Option<String>,
    ) -> PlanNode;
```

**DECISION:** Multi-node is an **additive** surface on this seam — a static
sequence index and/or a `plan_next`-style method that returns `Option<PlanNode>`
from a worker-held stream context. Parameters remain: typed `CaprunIntent` +
opaque `ValueId`s only (PLAN-03). **Keep** the existing one-shot `plan()` for
email/file + `LlmPlanner` single-node paths — no regression of shipped intents
(`CaprunIntent::SendEmailSummary` / `CreateFileFromReport` at
`crates/runtime-core/src/intent.rs:22-46`).

### 1.3 DECISION — Open Question 5 default: static index sufficient for v1.10

A deterministic coding planner (Phase 49) needs only a **static ordered sequence**
of sinks over the recipe. Reactive / observation-driven `plan_next` for an LLM
tool-use loop is **out of v1.10 trait requirements** (LLM-MS-01 deferred). An
additive `plan_next` name may appear in later phase prose as a planning-only
symbol (§12); it is not required for the deterministic coding path.

### 1.4 EXPLICITLY REJECTED stream shapes

| Rejected shape | Why |
|----------------|-----|
| **Batch DAG authorize-all** (`SubmitPlanDAG`, multi-node authorize in one I2 pass) | New I2 bypass surface; violates architectural lock that every effect is a per-node plan evaluation. **No batch** product path. |
| Free-form tool-map / raw effect-request path under `crates/` | Gate 1 + CLAUDE.md effect-path lock. Plan-node path only. |
| Orchestrator-submitted PlanNodes outside the confined worker | Weakens "kernel-confined worker is the only egress story"; product boundary. |
| New workflow / agent-framework crate | HYG-02 default zero new crates (§8). |

---

## §2. Worker sequential submit + handle bag (DESIGN-19b)

### 2.1 DECISION — replace one-shot with sequential loop

Today the worker is one-shot (`cli/caprun/src/worker.rs:361-412`):

1. `planner.plan(...)` once (`:361-369`).
2. One `SubmitPlanNode` (`:372`).
3. Receive `PlanNodeDecision { decision, output_value_id }` (`:382-388`).
4. **Discard** `output_value_id` via `let _ = &output_value_id` (`:389`).
5. Exit non-zero on any non-`Allowed` (`:407-411`).

**DECISION:** Phase 48 replaces this with a **sequential submit loop**: for each
next plan node from the additive Planner surface, submit, receive decision,
branch per §6. On `Allowed` with `output_value_id: Some(id)`, store the handle
in the worker-side bag. The broker IPC messages stay
`SubmitPlanNode` / `PlanNodeDecision` (`proto.rs:136-138`, `:251-254`) — no new
effect-path verb.

### 2.2 DECISION — handle bag stores only opaque ValueIds

**DECISION:** The worker-side **handle bag** is a map (or equivalent) of opaque
`ValueId` handles only — never literals, never taint labels, never
`ValueRecord`s. Source of handles:

- Session-start mints returned by ProvideIntent / derivation (existing).
- Intermediate `PlanNodeDecision.output_value_id` on Allowed sinks that mint
  outputs (e.g. `mint_from_exec` wiring already documented at `worker.rs:376-381`
  and `proto.rs:242-253`).

`PlanArg` remains name + `value_id` only (`plan_node.rs:122-125`). `PlanNode`
remains sink + args (`plan_node.rs:136-139`). The planner **never mints** and
**never strips taint** (PLAN-03 / `DESIGN-plan-executor.md`; module docs
`planner.rs:3-41`). Planner may only place handles **offered by call-site
convention** — the worker (call site) decides which bag handles are offered for
each step; the planner places named handles without seeing provenance.

### 2.3 DECISION — ProvideIntent exactly once; mid-stream re-ProvideIntent DENIED

Broker enforces ProvideIntent **exactly once, only before any RequestFd**, via
per-connection locals (`server.rs:561-566`) and the ProvideIntent arm
(`server.rs:2370-2391`):

```2370:2391:crates/brokerd/src/server.rs
        BrokerRequest::ProvideIntent { intent, primary_file_derived } => {
            // Phase 16 (BLOCKER-1 guard a): ProvideIntent is accepted EXACTLY
            // ONCE and ONLY BEFORE any RequestFd on this connection —
            // …
            if *intent_provided || *fd_requested {
                send_response(
                    stream,
                    &BrokerResponse::Error {
                        message: "ProvideIntent rejected: must arrive exactly once, \
                                  before any RequestFd (fail-closed)"
                            .into(),
                    },
                )
```

**DECISION (idempotency pin):** Multi-step **does not** reopen ProvideIntent.
A second ProvideIntent mid-stream is **DENIED** by the existing broker guards.
All coding trusted-intent literals for the success path are declared at session
start (operator-typed CLI → ProvideIntent once). There is **no** second
"declare trusted after observation" verb.

---

## §3. Mid-loop Block-and-Hold confirm continuity (DESIGN-19c)

### 3.1 DECISION — Block-and-Hold same Session

On `ExecutorDecision::BlockedPendingConfirmation` (`executor_decision.rs:265-282`),
the multi-step product path is **Block-and-Hold**:

1. Worker **stays connected** (or uses a design-locked same-Session hold that
   does not exit the occupancy latch / remint).
2. Human acts on the **durable** `PendingConfirmation` snapshot already produced
   by broker block machinery (`server.rs:985+` pending insert path; confirm
   substrate in `DESIGN-confirmation-release.md`).
3. Sink executes from the **snapshot** on confirm — worker **MUST NOT re-submit**
   the blocked node (`DESIGN-confirmation-release.md`: confirm MUST NOT
   re-invoke `submit_plan_node` for the blocked node).
4. Remaining nodes continue only after Allowed release under:
   - **same Session id**
   - **same policy bind** (POLICY-03 immutable for Session)
   - **same audit chain** (`verify_chain` continuity)

Confirm is **not** a session trust waiver and **not** a class waiver for
CommitIrreversible.

### 3.2 DECISION — always-confirm `git.push` is a first-class mid-loop hold

Broker rewrites clean `git.push` `Allowed` → synthetic
`BlockedPendingConfirmation` before dispatch (`server.rs:778-848`):

```807:848:crates/brokerd/src/server.rs
    if matches!(decision, runtime_core::ExecutorDecision::Allowed)
        && plan_node.sink.0 == "git.push"
    {
        // … build anchors from routing args …
        decision = runtime_core::ExecutorDecision::BlockedPendingConfirmation { anchors };
    }
```

**DECISION:** Mid-loop hold applies to **both** I2 Block and this always-confirm
rewrite. The coding success path **will** hit mid-loop Block on `git.push` even
without taint. Design and LIVE must treat always-confirm as first-class, not
only I2.

### 3.3 DECISION — Open Question 1 primary path; Open Question 2 UX

**Open Question 1 (worker↔main "blocked, waiting" signal) — PRIMARY PATH:**

CLI main holds broker lifetime + either:

- **interactive confirm** inside `caprun run`, or
- **documented dual-terminal** `caprun confirm` / `caprun deny` against the
  durable pending row,

**without** worker exit that forces ProvideIntent remint, without dual-Session
stitch, and without re-opening the occupancy latch as a resume strategy.

| Alternative | Disposition |
|-------------|-------------|
| Exit worker + reconnect / `caprun continue` remint | **REJECTED** — empty ValueStore, ProvideIntent laundering tripwire, occupancy latch one-way |
| Dual-Session "stitch the chain later" as product path | **REJECTED** — splits audit chain / policy bind |
| Session-wide confirm waiver / auto-confirm mid-loop | **REJECTED** — single-shot per effect_id; subsequent nodes full I2 |
| Side UDS / broker poll verb as sole primary | Allowed only as implementation detail **inside** same-Session constraint; not a Session-split resume |

**Open Question 2 (interactive-in-run vs dual-terminal):** both OK if same
Session + no re-submit of blocked node. Machine-checkable stop semantics
(CLI-02) apply either way: Block surfaces `effect_id` + review pointer; silent
continue-past-Block is forbidden.

---

## §4. I1×coding-loop bounds (DESIGN-19d)

### 4.1 DECISION — trusted-intent success path

**DECISION:** The coding **success path** mints all irreversible-sink args
(paths, commands, messages, remotes/refspecs, PR title/body) **once** at session
start via ProvideIntent from **operator-typed** CLI / intent. Success path does
**not** require multi-file untrusted RequestFd before CommitIrreversible nodes.

Rationale (code-verified): HARDEN-01 demotes non-seed files → Draft; Draft ×
CommitIrreversible is denied at executor Step 0.5. Reading untrusted workspace
bytes mid-loop before push/PR is a demotion hazard, not a green-test fix target.

### 4.2 Effect-class table (code-verified — do not re-litigate without explicit fork)

From `crates/executor/src/sink_sensitivity.rs` `sink_effect_class` (`:40-110`):

| Sink | Class | Draft session |
|------|-------|---------------|
| `git.commit` | `MutateReversible` (`:55`) | Allowed (class gate) |
| `http.request` (GET) | `Observe` (`:64`) | Allowed |
| `file.write` | `CommitIrreversible` (`:44`) | **Denied** Step 0.5 |
| `process.exec` | `CommitIrreversible` (`:45`) | **Denied** |
| `git.push` | `CommitIrreversible` (`:98`) | **Denied** |
| `github.pr` | `CommitIrreversible` (`:71`) | **Denied** |
| `http.request.write` | `CommitIrreversible` (`:85`) | **Denied** |
| unknown | `CommitIrreversible` (`:110` `_ =>`) | **Denied** (fail-closed) |

### 4.3 EXPLICITLY REJECTED — weaken CommitIrreversible Draft denies

**REJECTED:** "Fixing" multi-file demotion by weakening Step 0.5 /
CommitIrreversible Draft denies so push/PR auto-Allow after untrusted reads.
That is an I0/I1 breach, not a multi-step feature.

### 4.4 DECISION — Open Question 4 default

**Seed only / none** RequestFd for the irreversible success path. Multi-file
untrusted read then still push = **separate future design-gate**, not a silent
class change in this doc.

### 4.5 Mid-loop I2 proof is deliberate, not success-path laundering

LIVE-08 (Phase 51) proves mid-loop I2 Block by **deliberate tainted-handle
routing** (genuine provenance root on a real read/exec event occupying a
sensitive sink arg under a policy-permitted sink). It is **not** the success
path, and success-path framing must not depend on laundering intermediate
outputs into trusted args.

---

## §5. Instruction vs value channel disjointness (DESIGN-19e)

### 5.1 DECISION — PLAN-03 / GATE-01 under multi-node

From `cli/caprun/src/planner.rs:64-69` and module docs `:3-41`:

- `task_instruction: Option<String>` may influence **which offered handle** the
  planner places (LLM framing; DeterministicPlanner ignores it).
- It is a `String`, **NEVER** a `ValueId` — it carries no handle and **cannot
  be bound** into a sink arg.
- Values bind only as pre-minted opaque handles via `PlanArg.value_id`
  (`plan_node.rs:122-125`).

**DECISION:** Multi-node **does not** collapse the instruction channel into a
bindable value. Additive multi-node Planner methods retain the same compile-time
boundary: typed intent + opaque `ValueId`s only; never `ValueRecord`, raw
untrusted bytes as values, or taint parameters.

---

## §6. Deny/abort semantics mid-stream (DESIGN-19f)

### 6.1 DECISION — branch on existing ExecutorDecision taxonomy

From `crates/runtime-core/src/executor_decision.rs:265-286`:

| Outcome | Multi-step action |
|---------|-------------------|
| `Allowed` | Dispatch (broker-side); if `output_value_id: Some`, store in handle bag; continue to next node |
| `BlockedPendingConfirmation` | **Hold** same Session (§3) — **no silent continue** |
| `Denied { reason }` including `DenyReason::PolicyDeny` (`code()=="policy_deny"`, `:133`) | **Abort remaining** nodes fail-closed; durable terminal events already recorded for the denied node; no further SubmitPlanNode in this stream |
| DraftOnlySessionDeniesCommitIrreversible (as Denied reason class) | Stays — abort remaining; not confirm-releasable as a class waiver |

`policy_deny` remains a **Denied** outcome, never
`BlockedPendingConfirmation` (`executor_decision.rs:82-84`, `:334-339`).

### 6.2 DECISION — Open Question 3: abort remaining on deny/policy_deny

**Pin: abort remaining** fail-closed on Deny / `policy_deny`. Distinct exit-code
taxonomy (success vs blocked vs denied/aborted — CLI-02) is deferred **detail**
to Phase 50 but the **semantics** (abort remaining; no silent continue; no
partial-continue past Deny) are locked here.

### 6.3 DECISION — sequential submit order only

**Ordering pin:** sequential submit order on one worker connection. **No**
parallel dual-occupancy of the same Session stream. No concurrent multi-worker
composition as the product multi-step path.

---

<!-- gsd:design-tail-pending -->
