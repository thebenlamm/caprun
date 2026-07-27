# DESIGN — Multi-step Plan Stream: sequential orchestration on the existing Planner seam

**Milestone:** v1.10 — Multi-step Safe Coding Agent Loop
**Phase:** 47 (Design Gate) — blocks all multi-step TCB code under
`crates/{executor,brokerd,sandbox,runtime-core}` and the worker submit /
confirm-hold path in `cli/caprun`
**Status:** ✅ **CLEARED** (Round-1 amendments) after a fresh, **non-self,
orchestrator-owned** adversarial code-trace (DESIGN-20). Record:
`planning-docs/DESIGN-GATE-RECORD-v1.10.md`. This doc was authored by a
`gsd-executor`; the executor did **not** run or self-perform that trace
(gsd-executors have no Agent tool — §13).
**Author date:** 2026-07-23
**Clear date:** 2026-07-27
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
- Intermediate `PlanNodeDecision.output_value_id` on **any** Allowed sink that
  mints an output into the live session `ValueStore` and returns `Some(id)`.
  Live broker mint arms today (all untrusted provenance; not a trust upgrade):
  - `process.exec` → `mint_from_exec` (`server.rs:1274-1299`)
  - `git.commit` → `mint_from_exec` (`server.rs:1308-1332`)
  - `http.request` → `mint_from_http` (`server.rs:1343-1401`)
  **DECISION (Round-1 F-01):** Phase 48 bag logic stores **any**
  `Some(output_value_id)` regardless of sink id. Stale comments that say
  "process.exec only" (`worker.rs:376-381`, `proto.rs:242-253`,
  `server.rs:2257-2259`) are **documentation drift, not authority** — do not
  under-bag `git.commit` / `http.request` outputs or invent trust from those
  comments. A docs-only comment fix may land with Phase 48; behavior is
  already multi-sink.

**DECISION (Round-1 F-02):** Post-confirm intermediate outputs are **out of
bag**. `confirm()` intentionally does **not** mint released sink output into a
live worker `ValueStore` and **never** re-invokes `submit_plan_node`
(`confirmation.rs:819-833`, `:1204-1217`). Success-path coding that needs
intermediate outputs must rely on **Allowed** (trusted-arg) mints while the
worker connection holds the `ValueStore`. "Fixing" missing post-confirm outputs
by re-submitting the blocked node, reminting UserTrusted, or inventing a
session-wide output channel is **forbidden** without a fresh design gate
(re-opens surface (5) / ProvideIntent remint).

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

## §7. Carry-forward invariants

These are **locked in writing** for the entire multi-step milestone — not
re-opened by stream convenience:

### 7.1 ProvideIntent exactly once before RequestFd

Restated from §2.3 with full carry-forward force: ProvideIntent is accepted
**exactly once**, **only before** any RequestFd, broker-enforced
(`server.rs:561-566`, `:2370-2391`). Multi-step streams **must not** introduce
a mid-stream re-intent IPC, a second trusted-declare verb, or a worker path
that remints `UserTrusted` from observations.

### 7.2 P33/P34 precheck-before-burn + terminal-event-before-terminal-state

Carried from `DESIGN-confirmation-release.md` / v1.8–v1.9 confirm-release
discipline (P33/P34):

1. **Precheck before burn** — confirm-release paths validate release preconditions
   before irreversible external work.
2. **Terminal audit event before terminal state** — every confirm-releasable
   multi-node release appends the durable terminal event **before** marking
   terminal confirmation state.

**Multi-confirm Session must not amplify the audit-gap class.** A Session that
releases multiple blocked effects (e.g. always-confirm `git.push` then a later
confirm-releasable node) still obeys precheck-before-burn and
terminal-event-before-state **per release**. Multi-node LIVE asserts events for
every released effect.

### 7.3 POLICY-02 / POLICY-03

- **POLICY-02:** Policy is a **pre-I2 narrowing gate** only (which sinks/args are
  callable). It **never** overrides or disables I2. I2 stays **hardcoded** in
  the Rust TCB executor, **unconditional** on every policy-permitted call.
  `policy_deny` is a distinct Denied outcome (`executor_decision.rs:82-94`,
  `:133`), never a Block waiver.
- **POLICY-03:** Policy is bound **once** at session creation from a trusted
  source **outside** the confined worker, and is **immutable** for the Session.
  Multi-step must not rebind policy mid-stream from workspace content.

### 7.4 Gate 3 mint-site discipline (adjacency)

Multi-step **default introduces ZERO new mint sites**. Gate 3 in
`scripts/check-invariants.sh` (`:50-141`) restricts `mint_from_read(`,
`mint_from_derivation(`, `mint_from_exec(`, `mint_from_http(`, and `.mint()` to
sanctioned loci (`crates/brokerd/src/quarantine.rs`,
`crates/brokerd/src/server.rs`).

**Adjacency pin:** no silent merge of mint loci. If a later phase needs a new
mint site, the DESIGN (or an explicit amendment) must **name the site and amend
Gate 3** — never land a new call site first and retrofit the allowlist.

---

## §8. HYG-02 / Gate discipline

### 8.1 DECISION — default ZERO new crates

**HYG-02:** Multi-step work re-asserts Gate hygiene:

| Rule | Pin |
|------|-----|
| New crates | Default **zero** for the entire multi-step milestone unless a later phase design-justifies an exception (**none expected** for plan-stream substrate) |
| Free-form effect-request token under `crates/` | **Forbidden** — Gate 1 body at `scripts/check-invariants.sh:29-36` (header comments precede it; Round-1 F-03 citation fix) |
| Gate 3 mint-site list | **Unchanged** or **explicitly amended** (§7.4) — default unchanged |
| `check-invariants.sh` | Remains the **architectural** gate (Gates 1–6) |
| compose-verify | Remains the **authoritative Linux** gate; `mailpit-verify.sh` when SMTP may fire (CLAUDE.md Phase 16+) |

### 8.2 Empty multi-node stream

**DECISION:** An empty multi-node stream (zero plan nodes after intent) is
**rejected or N/A** as fail-closed — not a success exit pretending a coding
chain ran. Implementation detail (CLI reject vs planner empty-sequence error)
is Phase 48/50; the default is **not** silent success.

### 8.3 CaprunIntent coding variant (Phase 49) — closed enum only

`CaprunIntent` is a closed enum (`crates/runtime-core/src/intent.rs:22-46`).
Phase 49 may add a coding variant.

**DECISION (Open Question 6 default):**

- **Closed enum only** — no free-form string intent kind.
- **All success-path literals** for irreversible sinks come from operator intent
  at ProvideIntent (trusted-intent success path, §4).
- Exact field names / variant name may defer to Phase 49 **if** this pin holds;
  naming alone is not a security decision.

### 8.4 No package installs this design gate

Phase 47 installs **zero** external packages. Package Legitimacy N/A.

---

## §9. Threat model

One row per RESEARCH pitfall → named structural mechanism (STRIDE-aligned).

| # | Threat | STRIDE | Structural mechanism that closes it | Pin § |
|---|--------|--------|-------------------------------------|-------|
| 1 | Cross-node taint laundering via `output_value_id` (intermediate exec/http/read treated as trusted for PR body / push refspec) | Tampering / Elevation | Opaque ValueIds only; planner never mints / never strips taint; no mid-stream ProvideIntent; LIVE-08 genuine provenance | §2, §4.5, §7.1 |
| 2 | ProvideIntent remint mid-stream as trust valve after observations | Elevation | ProvideIntent exactly once before RequestFd; broker rejects second / post-RequestFd ProvideIntent | §2.3, §7.1 |
| 3 | Draft demotion "fixed" by weakening CommitIrreversible Step 0.5 | Elevation / I0-I1 | Trusted-intent success path; effect-class table locked; explicit reject of class weaken | §4 |
| 4 | Batch authorize / free-form tool-map / raw effect-request path | Tampering / Spoofing | Sequential N× SubmitPlanNode only; no batch product path; Gate 1 | §1.4, §8.1 |
| 5 | Mid-loop confirm Session split / reconnect-remint / session-wide waiver | Tampering | Block-and-Hold same Session, same policy, same audit chain; no re-submit blocked node; dual-Session rejected | §3 |
| 6 | Instruction channel collapsed into bindable ValueId | Tampering | `task_instruction` is String never ValueId; PLAN-03 handles-only under multi-node | §5 |
| 7 | Policy mid-stream rebind or I2 override ("workflow allow") | Elevation | POLICY-02 unconditional I2; POLICY-03 bind once immutable; policy_deny ≠ Block | §7.3 |
| 8 | Hybrid in-crate composition framed as CLI multi-step DONE | Integrity of claims | LIVE DONE requires real multi-node `caprun run` one Session (LIVE-07/08); hybrid only unit harness | §0, §14 |
| 9 | New mint site outside Gate 3 / silent allowlist merge | Tampering | Default zero new mints; Gate 3 unchanged or explicit amend | §7.4, §8.1, §12 |
| 10 | P33/P34 audit gap amplified by multi-confirm Session | Repudiation | Precheck-before-burn + terminal-event-before-state per release | §7.2 |
| 11 | Design-gate process failure (self-review / early TCB code) | Elevation (latent) | DESIGN-20 orchestrator-owned non-self trace; empty crates/cli porcelain this phase | §13, §14 |

---

## §10. Invariant preservation checklist

This design **weakens NONE** of I0/I1/I2 and adds **no** raw plan-node-bypass
effect path:

| Invariant | Preserved? | How multi-step upholds it |
|-----------|------------|---------------------------|
| **I0** — external/untrusted seed starts draft-only; cannot auto-authorize Tier 3+ | YES | Draft × CommitIrreversible Step 0.5 stays; trusted-intent success path avoids demotion-before-irreversible; no class weaken (§4) |
| **I1** — no context holds untrusted content *and* authority for irreversible effects | YES | Instruction channel non-bindable (§5); intermediate outputs retain taint in broker ValueStore; no mid-stream UserTrusted remint (§2.3) |
| **I2** — no attacker-tainted value in sensitive sink arg without literal-value human confirmation; **hardcoded in Rust TCB** | YES | Per-node I2 via existing `submit_plan_node` / `evaluate_plan_node_and_record`; policy pre-I2 only (POLICY-02); no stream-wide I2 waiver; confirm is single-shot per effect, not class waiver (§3, §7.3) |
| **No raw plan-node-bypass effect path** | YES | PlanNode path only; Gate 1; no batch authorize (§1.4, §8.1) |
| **No batch authorize** | YES | Sequential N× SubmitPlanNode; each node independent (§1, §6.3) |
| **Handle model intact** | YES | Opaque ValueIds; planner never mints (§2, §5) |
| **Kernel confinement unchanged** | YES | Worker self-confine after connect order unchanged; multi-step does not re-open ambient net/exec (§0, §1.1) |
| **ProvideIntent-once** | YES | Broker guards remain; multi-step must not reopen (§2.3, §7.1) |
| **Policy immutable for Session** | YES | POLICY-03; no mid-stream rebind (§7.3) |

---

## §11. Fail-closed defaults table

| Mechanism / edge | Fail-closed default |
|------------------|---------------------|
| Empty / missing multi-node stream | Reject or N/A — not success (§8.2) |
| Deny / `policy_deny` mid-stream | **Abort remaining** nodes; durable terminal events for denied node (§6) |
| `BlockedPendingConfirmation` | **Hold** same Session — not silent continue; not auto-confirm (§3) |
| Absent handle for a required PlanArg | Cannot bind arg — schema / resolve fail-closed (existing sink schema + ValueStore resolve) |
| Mid-stream ProvideIntent | **Deny** — mint nothing, no chain-head advance (`server.rs:2370-2391`) |
| Unknown sink | Existing schema / `sink_effect_class` `_ => CommitIrreversible` fail-closed (`sink_sensitivity.rs:110`) |
| Dual-Session resume after Block | **Rejected** as product path (§3.3) |
| Reconnect-remint after worker exit | **Rejected** (§3.3) |
| Session-wide confirm waiver | **Rejected** (§3.3) |
| Parallel dual-occupancy same Session stream | **Rejected** — sequential only (§6.3) |
| New mint outside Gate 3 | **Rejected** unless explicit DESIGN amend (§7.4) |
| Batch DAG authorize | **Rejected** (§1.4) |

---

## §12. New-symbol summary

**Default: none.** Multi-step substrate introduces:

| Category | Expectation this phase / substrate default |
|----------|--------------------------------------------|
| New `TaintLabel` variants | **None** |
| New `mint_from_*` helpers | **None** (A4: no new mint site required for multi-step substrate) |
| New workspace crates | **None** (HYG-02) |
| New IPC effect-path verbs | **None** — reuse `SubmitPlanNode` / `PlanNodeDecision` / existing confirm path |
| New public-API terminology | **None** — locked Intent/Session/Planner/Worker/Broker/… vocabulary |

**Planning-only names** (may appear in DESIGN prose and later phase plans; **not**
landed as TCB symbols this phase):

| Planning-only name | Role | Phase that may materialize |
|--------------------|------|----------------------------|
| `plan_next` / static sequence index | Additive multi-node Planner surface | 48–49 |
| handle bag (worker-side map type name TBD) | Opaque `ValueId` storage | 48 |
| CaprunIntent coding variant name / fields | Closed-enum coding recipe | 49 |

These names are **not** commitments to public API shape beyond the decisions in
§1–§6; implementors may rename as long as PLAN-03 / handle-only / sequential
semantics hold.

---

## §13. Adversarial-trace gate (DESIGN-20)

### 13.1 DECISION — orchestrator-owned, non-self, fresh code-trace

A fresh, **NON-SELF**, **ORCHESTRATOR-OWNED** adversarial code-trace must clear
this doc with every BLOCKER/MAJOR resolved **before ANY multi-step TCB change**
in:

- `crates/{executor,brokerd,sandbox,runtime-core}`, or
- the worker submit / confirm-hold path in `cli/caprun`.

**This plan (47-01) does not run the trace.** A `gsd-executor` must not
self-review: gsd-executors have no Agent tool, and self-read fails fresh-context
discipline. Plan 47-02 / the orchestrator owns spawn, fold, and the gate record.

**Unbroken precedent:** v1.0 P2 → v1.2 P8 → v1.3 P12 → v1.4 P18 → v1.5 P23 →
v1.6 P26 → v1.7 P31 → v1.8 P35 → v1.9 P41 → **v1.10 P47**.

### 13.2 Re-run triggers

The adversarial code-trace **re-runs** if any of the following pivots mid-
implementation (Phases 48–52):

1. **Stream shape** — e.g. batch authorize, new multi-node IPC verb, non-sequential
   product path, or abandonment of additive Planner seam.
2. **Confirm-hold** — e.g. dual-Session stitch, reconnect-remint resume,
   session-wide confirm waiver, or re-submit of blocked node as product path.
3. **Trusted-arg mint path** — e.g. mid-stream ProvideIntent, new mint outside
   Gate 3, or success-path reliance on untrusted intermediate outputs without
   I2.

### 13.3 Gate record

Outcome is recorded in `planning-docs/DESIGN-GATE-RECORD-v1.10.md` (or
orchestrator-equivalent path following v1.8/v1.9 naming), with:

- Reviewer identity & independence (author ≠ reviewer)
- Files opened (code-trace, not prose skim)
- Findings table (severity / claim / re-verified code fact / resolution → DESIGN §)
- No-TCB-code reconfirmation until CLEARED
- Verdict authorizing Phases 48–52

Attack surfaces the reviewer brief must pressure-test: §9 rows 1–11 (and
RESEARCH "What the adversarial reviewer must pressure-test" table).

### 13.4 Proven value of the discipline

v1.9's gate caught BLOCKER-level I0 escape (`http.request` WRITE would inherit
Observe) that plan-checker + green docs-only invariants both missed
(`[VERIFIED: planning-docs/DESIGN-GATE-RECORD-v1.9.md]`). Multi-step composition
is the same class of risk: correct single-node controls composed unsoundly.

---

## §14. Acceptance predicate — Done when

Phase 47's design gate is cleared when **ALL** are true:

1. This doc pins, as **DECISIONS** (not options), every DESIGN-19 mechanism:
   (a) sequential plan-stream on existing Planner seam — not batch DAG, not
   free-form effect path (§1); (b) worker sequential submit + opaque ValueId
   handle bag, planner never mints, ProvideIntent exactly once (§2); (c)
   Block-and-Hold same Session for I2 Block and always-confirm `git.push`, no
   re-submit blocked node, dual-Session/reconnect-remint/session-wide waiver
   rejected (§3); (d) trusted-intent success path + effect-class table + no
   CommitIrreversible Draft weaken (§4); (e) instruction vs value channel
   disjointness PLAN-03 (§5); (f) Deny/policy_deny abort remaining, Block holds,
   sequential order (§6).
2. Carry-forwards locked: ProvideIntent-once, P33/P34, POLICY-02/03, Gate 3
   adjacency (§7). HYG-02 re-asserted: zero new crates default, Gate 1/3,
   check-invariants + compose-verify authority, closed CaprunIntent coding
   constraint (§8).
3. Threat model maps listed multi-step pitfalls to named mechanisms (§9);
   invariant checklist shows I0/I1/I2 unweakened and no plan-node bypass / no
   batch authorize (§10); fail-closed defaults table present (§11); new-symbol
   summary defaults to none / no new mint sites (§12).
4. This doc declares the fresh adversarial code-trace **ORCHESTRATOR-owned
   (NOT a gsd-executor)** with **re-run triggers** on stream shape / confirm-hold
   / trusted-arg mint path pivots (§13, DESIGN-20).
5. DESIGN-20 clear is recorded **CLEARED** in
   `planning-docs/DESIGN-GATE-RECORD-v1.10.md` (Plan 47-02 / orchestrator) —
   **not** claimed by Plan 47-01 alone.
6. Until DESIGN-20 CLEARED: **no multi-step TCB code** under
   `crates/{executor,brokerd,sandbox,runtime-core}` or worker submit/confirm-hold
   in `cli/caprun`. For this authoring plan:
   `git status --porcelain -- crates cli` is empty and
   `scripts/check-invariants.sh` exits 0.

**LIVE DONE (Phase 51, not this phase):** requires real multi-node `caprun run`
one Session (LIVE-07) + mid-loop I2 Block with genuine taint (LIVE-08). Hybrid
in-crate composition is **not** the DONE claim.

---

## Amendments (post-review)

Round-1 fold (2026-07-27) after orchestrator-owned non-self adversarial
code-trace (DESIGN-20). **0 BLOCKER, 0 MAJOR.** All accepted MINOR/NIT
findings resolved by **tightening** pins — no invariant weakened, no TCB code.

| # | Sev | Claim | Re-verify | Resolution |
|---|-----|-------|-----------|------------|
| F-01 | MINOR | `output_value_id` is "process.exec only" | **CONFIRMED** — Allowed-path mints also set `Some` for `git.commit` (`server.rs:1308-1332`) and `http.request` (`server.rs:1343-1401`); stale comments at `worker.rs:376-381`, `proto.rs:242-253`, `server.rs:2257-2259` | **§2.2 tightened:** bag stores **any** `Some(output_value_id)`; comments are drift not authority |
| F-02 | MINOR | Confirm-released intermediate mint does not enter worker bag | **CONFIRMED** — `confirmation.rs:819-833`, `:1204-1217` explicitly no mint / no `ValueStore` / no re-submit | **§2.2 tightened:** post-confirm outputs out-of-bag; re-submit/remint forbidden without new design gate |
| F-03 | NIT | Gate 1 cited as `:7-31` | **CONFIRMED** — body is `check-invariants.sh:29-36` | **§8.1 citation fixed** |

Full independence proof, files-opened list, and Verified-as-sound ledger:
`planning-docs/DESIGN-GATE-RECORD-v1.10.md`.
