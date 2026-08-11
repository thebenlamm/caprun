# Phase 47: Multi-step Plan Stream Design Gate - Pattern Map

**Mapped:** 2026-07-23
**Files analyzed:** 12 (2 phase deliverables + 10 live-path citation targets)
**Analogs found:** 12 / 12

> **Phase nature:** Doc-only design gate. No TCB / CLI multi-step code this phase.
> Primary deliverables are markdown under `planning-docs/`. Live code files below
> are **citation analogs** the DESIGN must pin against (`file:line`), not
> modification targets.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `planning-docs/DESIGN-multi-step-plan-stream.md` | design-doc | request-response (process) | `planning-docs/DESIGN-v1.9-egress-policy.md` | exact |
| `planning-docs/DESIGN-GATE-RECORD-v1.10.md` | gate-record | request-response (process) | `planning-docs/DESIGN-GATE-RECORD-v1.9.md` (+ v1.8 for multi-round) | exact |
| *(cite)* `cli/caprun/src/planner.rs` | service / trait seam | transform | self (single-node Planner) | exact |
| *(cite)* `cli/caprun/src/worker.rs` | controller (worker loop) | request-response / sequential | self (one-shot submit) | exact |
| *(cite)* `crates/brokerd/src/proto.rs` | model / IPC | request-response | self (`SubmitPlanNode` + `PlanNodeDecision`) | exact |
| *(cite)* `crates/brokerd/src/server.rs` | controller | request-response + event-driven audit | self (multi-submit + always-confirm) | exact |
| *(cite)* `crates/runtime-core/src/plan_node.rs` | model | transform | self (PlanNode / PlanArg / ValueId) | exact |
| *(cite)* `crates/runtime-core/src/executor_decision.rs` | model | request-response | self (Allowed / Block / Denied / PolicyDeny) | exact |
| *(cite)* `crates/runtime-core/src/intent.rs` | model | transform | self (`CaprunIntent` closed enum) | exact |
| *(cite)* `crates/executor/src/sink_sensitivity.rs` | service | transform | self (`sink_effect_class`) | exact |
| *(cite)* `planning-docs/DESIGN-plan-executor.md` | design-doc (handle spine) | — | self (PLAN-03 / ValueId model) | exact |
| *(cite)* `planning-docs/DESIGN-confirmation-release.md` | design-doc (confirm) | — | self (single-shot / no re-submit) | exact |

Optional (planner may emit; not required by RESEARCH):

| File | Role | Closest Analog | Match Quality |
|------|------|----------------|---------------|
| `.planning/phases/47-…/47-VALIDATION.md` | validation contract | `.planning/milestones/v1.9-phases/31-effect-breadth-design-gate/31-VALIDATION.md` | exact |

## Pattern Assignments

### `planning-docs/DESIGN-multi-step-plan-stream.md` (design-doc, process)

**Analog:** `planning-docs/DESIGN-v1.9-egress-policy.md` (primary structure)
**Secondary shape analogs:** `planning-docs/DESIGN-git-github-http-sinks.md`, `planning-docs/DESIGN-effect-breadth-exec.md`
**Handle / confirm carry-forward:** `DESIGN-plan-executor.md`, `DESIGN-confirmation-release.md`, `DESIGN-session-trust-coherence.md`

**Header / discipline pattern** (`DESIGN-v1.9-egress-policy.md` lines 1–31):

```markdown
# DESIGN — <title pinning the mechanisms>

**Milestone:** v1.10 — Multi-step Safe Coding Agent Loop
**Phase:** 47 (Design Gate) — blocks all `crates/{executor,brokerd,sandbox,runtime-core}`
and multi-step worker submit/confirm-hold in `cli/caprun`
**Status:** Draft → pending a fresh, **non-self, orchestrator-owned** adversarial
code-trace (DESIGN-20) to be recorded in `planning-docs/DESIGN-GATE-RECORD-v1.10.md`.
This doc is authored by a `gsd-executor`; the executor does **not** run or self-perform
that trace (gsd-executors have no Agent tool).
**Author date:** …
**Grounding:** `.planning/research/{SUMMARY,ARCHITECTURE,PITFALLS}.md`, REQUIREMENTS
(DESIGN-19/20, HYG-02, STREAM/CODE/CLI/CONFIRM/LIVE), live `file:line` citations.
**Requirements:** DESIGN-19 (this doc) → enables STREAM-01/02 (48), CODE-01/02 (49),
CLI-01/02 + CONFIRM-01 (50), LIVE-07/08 (51). DESIGN-20 is the gate that clears it.

> **Design-gate discipline.** No multi-step TCB / worker submit / confirm-hold code
> until this document clears a fresh, non-self, orchestrator-owned adversarial
> code-trace — unbroken precedent (v1.0 P2 … v1.9 P41). This doc pins **decisions**,
> not options.
```

**§ outline to copy** (RESEARCH recommended + v1.9 § numbering style):

| § | Title | Copy from |
|---|-------|-----------|
| §0 | Purpose & Scope | v1.9 §0 (what pins / deferred / no TCB this phase) |
| §1 | Plan-stream shape | RESEARCH Pattern 1; cite broker multi-submit + Planner seam |
| §2 | Worker sequential submit + handle bag | RESEARCH Pattern 2; cite `output_value_id` discard |
| §3 | Mid-loop Block-and-Hold | RESEARCH Pattern 3 + `DESIGN-confirmation-release.md` single-shot |
| §4 | I1×coding-loop bounds | RESEARCH Pattern 4 + `sink_effect_class` table |
| §5 | Instruction vs value channel | RESEARCH Pattern 5 + `planner.rs` PLAN-03 docs |
| §6 | Deny/abort mid-stream | `ExecutorDecision` + worker exit-on-non-Allowed |
| §7 | Carry-forward invariants | ProvideIntent-once, P33/P34, POLICY-02, Gate 3 |
| §8 | HYG-02 / Gate discipline | v1.9 §3 crypto/HYG shape adapted to zero-crate multi-step |
| §9 | Threat model | v1.9 §6 one-row-per-pitfall → named mechanism |
| §10 | Invariant preservation checklist | v1.9 §7 (I0/I1/I2, no EffectRequest, no batch authorize) |
| §11 | Fail-closed defaults table | v1.9 §4 |
| §12 | New-symbol summary | v1.9 §8 (expect "none — no new mint sites / TaintLabels") |
| §13 | Adversarial-trace gate (DESIGN-20) | v1.9 §9 (orchestrator-owned; re-run triggers) |
| §14 | Acceptance predicate | v1.9 §10 |
| Amendments | Post-review fold placeholder | v1.9 "Round-1 Amendments" section |

**Decisions-not-options voice** (v1.9 §0 lines 37–63): each mechanism is **pinned** with a named realization and explicit rejects (batch DAG, EffectRequest, reconnect-remint, session-wide waiver, LLM multi-step).

**Citation discipline:** every load-bearing claim must cite live code `file:line` (v1.9 header: "Every `file:line` below traces to a direct code read this session").

**No TCB code reconfirmation** (v1.9 §0 lines 101–104 + §10 item 5):

```text
git status --porcelain -- crates cli   # empty
./scripts/check-invariants.sh          # exit 0
```

---

### `planning-docs/DESIGN-GATE-RECORD-v1.10.md` (gate-record, process)

**Analog:** `planning-docs/DESIGN-GATE-RECORD-v1.9.md` (primary — Round-1 clear shape)
**Secondary:** `planning-docs/DESIGN-GATE-RECORD-v1.8.md` (multi-round + standing corrections list)

**Header pattern** (v1.9 lines 1–15):

```markdown
# DESIGN GATE RECORD — v1.10 (Multi-step Plan Stream)

**Phase:** 47 — v1.10 DESIGN Gate + Fresh Adversarial Code-Trace
**DESIGN doc under review:** `planning-docs/DESIGN-multi-step-plan-stream.md`
**Requirements gated:** DESIGN-19 (pin TCB mechanisms), DESIGN-20 (clear fresh non-self
adversarial code-trace before any multi-step TCB code)
**Status:** … CLEARED / NOT CLEARED …
**Date:** …

## Gate discipline (standing precedent, unbroken v1.0 P2 → v1.9 P41)

No `crates/{executor,brokerd,sandbox,runtime-core}` or multi-step worker
submit/confirm-hold in `cli/caprun` may be written until this DESIGN clears a
**fresh, non-self, ORCHESTRATOR-owned** adversarial code-trace. The orchestrator
(not a gsd-executor) owns the review spawn and the finding-fold.
```

**Findings table pattern** (v1.9 lines 32–38):

```markdown
| # | Sev | Finding | Confirmed code fact | Resolution |
|---|-----|---------|---------------------|------------|
| … | BLOCKER/MAJOR/MINOR | claim | `file:line` fact | **Fixed §N:** … |
```

**Independence + method** (v1.8 lines 11–23): author ≠ reviewer; list files opened; method = code-trace every `file:line`, not prose skim.

**Phase-47 attack surfaces the reviewer brief must supply** (from RESEARCH):

1. Cross-node taint laundering via `output_value_id`
2. ProvideIntent reopened mid-stream
3. Draft demotion "fixed" by weakening CommitIrreversible Step 0.5
4. Batch authorize / EffectRequest / new effect path
5. Mid-loop confirm Session split / reconnect-remint / session-wide waiver
6. Instruction channel collapsed into bindable ValueId
7. Policy mid-stream rebind or I2 override
8. Hybrid composition framed as CLI multi-step DONE
9. New mint site outside Gate 3
10. P33/P34 confirm-release order under multi-confirm Session

**No-TCB-code reconfirmation** (v1.9 lines 45–46, 59):

```text
git status --porcelain -- crates cli   # empty throughout
./scripts/check-invariants.sh          # exit 0
```

**Verdict authorizing Phases 48–52** (mirror v1.9 Outcome section lines 55–59).

---

### Live path: `cli/caprun/src/planner.rs` (trait seam — DESIGN §1 / §5 must cite)

**Analog for additive multi-node API:** existing `Planner` trait (lines 56–80).

**PLAN-03 / instruction channel pattern** (module docs lines 1–41 + trait lines 61–79):

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

**Pin for DESIGN:**

- Additive multi-node surface only (`plan_next` and/or static sequence index) — **keep** one-shot `plan()` for email/file + LlmPlanner.
- Parameters remain: typed `CaprunIntent` + opaque `ValueId`s only; never `ValueRecord`, raw untrusted bytes, or taint.
- `task_instruction: Option<String>` may influence handle *selection*; it is **never** a `ValueId` and cannot bind as a sink arg (GATE-01 / PLAN-03).

---

### Live path: `cli/caprun/src/worker.rs` (one-shot gap — DESIGN §2 / §3 must pin the replacement)

**Self-confinement order** (module docs lines 1–57): connect → confine → ProvideIntent → RequestFd → plan once → SubmitPlanNode once → exit on non-Allowed.

**Submit + discard `output_value_id`** (lines 361–412) — the multi-step gap:

```361:412:cli/caprun/src/worker.rs
    let plan_node = planner.plan(
        &intent,
        intent_value_id,
        derived_recipient,
        body,
        trusted_subject_handle,
        trusted_body_handle,
        task_instruction,
    );

    // ── Submit for I2 evaluation (no session_id field — HARD-03) ─────────────
    send_framed(&std_stream, &BrokerRequest::SubmitPlanNode { plan_node })?;
    // …
    let (decision, output_value_id) = match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::PlanNodeDecision {
            decision,
            output_value_id,
        } => (decision, output_value_id),
        other => anyhow::bail!("unexpected response to SubmitPlanNode: {other:?}"),
    };
    let _ = &output_value_id;
    // …
    if !matches!(decision, ExecutorDecision::Allowed) {
        eprintln!(
            "[worker] NOT ALLOWED ({decision:?}): no effect ran — exiting 1"
        );
        std::process::exit(1);
    }
```

**Pin for DESIGN:**

- Replace one-shot with sequential loop + **handle bag** storing `output_value_id`.
- On `BlockedPendingConfirmation`: **HOLD** (do not exit/remint); human confirm via durable pending row; **do not re-submit** blocked node (`DESIGN-confirmation-release.md` "Confirm MUST NOT Re-Invoke `submit_plan_node`").
- On `Denied` / `policy_deny`: abort remaining nodes (fail-closed).
- ProvideIntent remains once, before RequestFd (ordering invariant in worker docs lines 9–12).

---

### Live path: `crates/brokerd/src/proto.rs` (IPC already multi-submit safe)

**SubmitPlanNode** (lines 125–138) — no `session_id` (HARD-03):

```136:138:crates/brokerd/src/proto.rs
    SubmitPlanNode {
        plan_node: runtime_core::PlanNode,
    },
```

**PlanNodeDecision + output_value_id** (lines 229–254):

```251:254:crates/brokerd/src/proto.rs
    PlanNodeDecision {
        decision: runtime_core::ExecutorDecision,
        output_value_id: Option<runtime_core::plan_node::ValueId>,
    },
```

**ProvideIntent** (lines 110–112) — only mint yielding UserTrusted from supplied intent:

```110:112:crates/brokerd/src/proto.rs
    ProvideIntent {
        intent: runtime_core::intent::CaprunIntent,
        primary_file_derived: bool,
    },
```

**Pin:** multi-step reuses these messages; no new effect-path verb; no second ProvideIntent.

---

### Live path: `crates/brokerd/src/server.rs` (broker multi-submit + always-confirm)

**Patterns to cite (not re-implement):**

| Mechanism | Where | DESIGN implication |
|-----------|-------|--------------------|
| Connection loop accepts N× `SubmitPlanNode` | `handle_connection` / `dispatch_request` | Stream shape = sequential submits already legal |
| `intent_provided` / `fd_requested` guards | locals ~565–566, ProvideIntent arm | ProvideIntent once; no mid-stream remint |
| `evaluate_plan_node_and_record` shared path | ~723+ | Per-node policy pre-I2 then I2 |
| git.push always-confirm rewrite | ~778–848 | Mid-loop Block **will** fire even without I2 taint |
| PendingConfirmation snapshot on Block | ~985+ | Confirm from snapshot; worker does not re-submit |

**git.push always-confirm rewrite** (conceptual excerpt at lines 778–848): clean Allowed → synthetic `BlockedPendingConfirmation` before dispatch. DESIGN §3 must treat this as a first-class mid-loop hold, not only I2 Blocks.

---

### Live path: `crates/runtime-core/src/plan_node.rs` (handle model spine)

**PlanArg / PlanNode** (lines 115–139) — planner never carries literals/taint:

```122:139:crates/runtime-core/src/plan_node.rs
pub struct PlanArg {
    pub name: String,
    pub value_id: ValueId,
}
// …
pub struct PlanNode {
    pub sink: SinkId,
    pub args: Vec<PlanArg>,
}
```

**ValueId** (lines 67–79): opaque UUID handle only.

**TaintLabel::is_untrusted** (lines 52–64): exhaustive match, no wildcard — multi-step must not add a silent-trusted label without DESIGN + Gate 3 discipline.

---

### Live path: `crates/runtime-core/src/executor_decision.rs` (stream stop taxonomy)

**Outcomes multi-step must distinguish** (enum ~265–286):

| Outcome | Multi-step semantics (DESIGN pin) |
|---------|-----------------------------------|
| `Allowed` | Dispatch; maybe `output_value_id` → handle bag |
| `BlockedPendingConfirmation` | Hold same Session; human gate; no silent continue |
| `Denied { PolicyDeny {..}}` | Distinct `policy_deny`; abort remaining |
| `Denied { DraftOnlySessionDeniesCommitIrreversible {..}}` | Step 0.5 class gate — do not weaken |
| Other `Denied` | Abort remaining |

**PolicyDeny** (lines 75–94): pre-I2 narrowing only; never I2 override (POLICY-02).

---

### Live path: `crates/runtime-core/src/intent.rs` (closed CaprunIntent)

**Closed enum** (lines 20–47) — `SendEmailSummary`, `CreateFileFromReport` today:

```20:47:crates/runtime-core/src/intent.rs
#[serde(tag = "kind")]
pub enum CaprunIntent {
    SendEmailSummary { recipient: String, subject: String, body: String },
    CreateFileFromReport { path: String },
}
```

**Pin:** coding multi-step adds a **closed enum variant** (Phase 49); no free-form tool maps. Success-path literals come from operator intent at ProvideIntent once.

---

### Live path: `crates/executor/src/sink_sensitivity.rs` (I1×coding-loop class table)

**`sink_effect_class`** (lines 40–110) — DESIGN §4 must not re-litigate without explicit fork:

| Sink | Class | Draft session |
|------|-------|---------------|
| `git.commit` | MutateReversible | Allowed (class gate) |
| `http.request` GET | Observe | Allowed |
| `file.write` / `file.create` | CommitIrreversible | Denied Step 0.5 |
| `process.exec` | CommitIrreversible | Denied |
| `git.push` / `github.pr` / `http.request.write` / `email.send` | CommitIrreversible | Denied |
| unknown | CommitIrreversible (fail-closed `_ =>`) | Denied |

Trusted-intent success path: avoid multi-file untrusted RequestFd demotion before irreversible nodes.

---

### Citation: `planning-docs/DESIGN-plan-executor.md` (handle model)

**Core spine** (lines 71–78): planner references values by opaque `ValueId`; never sees/mints/retypes literal or taint; broker-owned store carries capability metadata. Multi-step handle bag is **composition** of this spine — not a new trust model.

---

### Citation: `planning-docs/DESIGN-confirmation-release.md` (Block-and-Hold continuity)

**Single-shot process today** (lines 19–22): caprun exits on Block; `PendingConfirmation` freezes full resolved args so confirm can release without live ValueStore.

**Confirm MUST NOT re-invoke `submit_plan_node`** (~line 190 section): multi-step hold inherits this — after confirm, remaining **subsequent** nodes still go through full submit; the blocked node does not.

**Confirm ≠ session trust waiver** (~lines 354–372): confirming a Block does not change Draft/Live; does not disable I2 for later nodes.

---

### Optional: `47-VALIDATION.md` (Nyquist for design-gate)

**Analog:** `.planning/milestones/v1.9-phases/31-effect-breadth-design-gate/31-VALIDATION.md`

- Framework: none — doc-assertion + process checks
- Quick: section presence greps + `test -f` DESIGN path
- Full: DESIGN-19 grep bundle + gate CLEARED + `check-invariants.sh` + empty TCB porcelain
- Manual: non-self adversarial code-trace

## Shared Patterns

### Design-gate process (two-plan / orchestrator clear)

**Source:** RESEARCH Prior Design-Gate Pattern; plans under `.planning/milestones/v1.9-phases/{35,41}-*`

| Plan | Owner | Produces | Forbidden |
|------|-------|----------|-----------|
| **47-01** Author DESIGN | gsd-executor | `DESIGN-multi-step-plan-stream.md` only | Any `crates/` / multi-step `cli/` edit; self-running adversarial trace |
| **47-02** or orchestrator post-01 gate | Orchestrator spawns non-self reviewer | Amendments + `DESIGN-GATE-RECORD-v1.10.md` CLEARED | Reviewer = author; gsd-executor performing review itself |

Prefer stating ownership explicitly (RESEARCH A3: either P35 two-plan or P41 orchestrator-only clear is fine if DESIGN-20 independence holds).

### Architectural locks (apply to DESIGN prose + gate)

**Source:** `CLAUDE.md` + `scripts/check-invariants.sh`

- Effect path = `PlanNode { sink, args: Vec<PlanArg> }` only — **never** `EffectRequest`
- Gate 1: no `EffectRequest` under `crates/`
- Gate 3: mint sites only at sanctioned loci; multi-step default **zero** new mints
- HYG-02: zero new crates unless design-gate-justified (default zero)
- I2 hardcoded in executor; policy pre-I2 only (POLICY-02)
- Policy bind once outside worker (POLICY-03)

### Terminology (public API + docs)

Intent, Session, Planner, Worker, Broker, Adapter, Effect, Artifact, Event. Never `ExecutionContext` in public API. Project/binary = `caprun`.

### Validation for this phase

```bash
test -f planning-docs/DESIGN-multi-step-plan-stream.md
# DESIGN-19 section greps (see 47-RESEARCH.md Validation Architecture)
test -f planning-docs/DESIGN-GATE-RECORD-v1.10.md
grep -qiE 'CLEARED|APPROVE' planning-docs/DESIGN-GATE-RECORD-v1.10.md
./scripts/check-invariants.sh
test -z "$(git status --porcelain -- crates cli)"
```

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| — | — | — | All phase deliverables and citation targets have strong analogs. Multi-step **worker loop** and **CLI multi-node driver** have no shipped implementation yet — those are Phase 48–50 code; DESIGN pins composition rules by citing the one-shot gap + broker multi-submit legality. |

## Metadata

**Analog search scope:** `planning-docs/DESIGN-*.md`, `planning-docs/DESIGN-GATE-RECORD-v1.*.md`, `cli/caprun/src/{planner,worker,main}.rs`, `crates/{brokerd,runtime-core,executor}/`, `.planning/milestones/v1.9-phases/{31,35,41}-*`
**Files scanned:** ~25 primary + grep hits across brokerd server/proto
**Pattern extraction date:** 2026-07-23
**CONTEXT.md:** none (discuss-phase skipped under --auto); file list from RESEARCH.md + REQUIREMENTS DESIGN-19/20/HYG-02
