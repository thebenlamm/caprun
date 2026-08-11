# Phase 48: Plan-Stream Substrate - Pattern Map

**Mapped:** 2026-07-28
**Files analyzed:** 8 (primary change surface + recommended tests + optional docs drift)
**Analogs found:** 8 / 8

> **Phase nature:** First multi-step **TCB/worker composition** phase (STREAM-01/02).
> Zero new crates, zero new mint sites, zero new IPC effect verbs. Physics already
> exist (broker multi-submit + `output_value_id`); gap is worker loop + bag +
> additive Planner surface + tracer tests. Product CLI multi-node / Block-and-Hold
> hold UX / coding recipe are Phases 49–50 — not this map.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cli/caprun/src/worker.rs` | controller (worker process) | sequential request-response | self one-shot tail (`:361-414`) + framing helpers (`:470-489`) | exact (extend) |
| `cli/caprun/src/planner.rs` | service / trait seam | transform (handles → PlanNode) | self `Planner` trait + `DeterministicPlanner` (`:61-110`) | exact (additive) |
| `cli/caprun/tests/planner.rs` | test (unit, macOS-safe) | transform | self existing planner unit tests | exact (extend) |
| `cli/caprun/tests/stream_substrate.rs` | test (integration / hybrid) | sequential + taint | `cli/caprun/tests/s9_process_exec_block.rs` + one-shot worker framing | role-match |
| `crates/brokerd/tests/stream_multi_submit.rs` | test (integration, Linux) | sequential multi-submit + audit | `crates/brokerd/tests/replay_cas.rs` (2× SubmitPlanNode) | exact (adapt) |
| `crates/brokerd/src/proto.rs` | model / IPC docs | request-response | self `PlanNodeDecision` docs (`:242-254`) | docs-only |
| `crates/brokerd/src/server.rs` | controller docs | request-response | self Allowed mint arms + comment at multi-submit arm | docs-only |
| `cli/caprun/src/main.rs` | orchestrator | — | self | **unchanged** in 48 |

## Pattern Assignments

### `cli/caprun/src/worker.rs` (controller, sequential request-response)

**Analog:** Self — one-shot submit/decision/exit at lines 361–414; framing at 470–489.
**Secondary:** RESEARCH target loop shape; DESIGN §2 / §6 branch table.
**Do not copy:** Batch submit, mid-stream ProvideIntent, re-submit on Block, bagging literals.

**Imports / types already in file** (lines 77–87):

```rust
mod planner;

use anyhow::Context;
use crate::planner::Planner;
use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind, WorkerClaim};
use brokerd::quarantine::{concat_doc_fragments, extract_doc_fragments, extract_relative_path_claims};
use runtime_core::intent::CaprunIntent;
use runtime_core::plan_node::ValueId;
use runtime_core::ExecutorDecision;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
// Phase 48: add `use std::collections::HashMap;` for handle bag
```

**One-shot gap to replace** (lines 361–414) — current pattern discards `output_value_id`:

```361:414:cli/caprun/src/worker.rs
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
    let _ = &output_value_id;  // ← STREAM-02 gap: must become bag.insert

    if !matches!(decision, ExecutorDecision::Allowed) {
        eprintln!(
            "[worker] NOT ALLOWED ({decision:?}): no effect ran — exiting 1"
        );
        std::process::exit(1);
    }
    Ok(())
```

**Framing pattern to keep** (lines 470–489) — reuse for every loop iteration:

```470:489:cli/caprun/src/worker.rs
fn send_framed(stream: &std::os::unix::net::UnixStream, msg: &impl serde::Serialize) -> anyhow::Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    (&*stream).write_all(&len)?;
    (&*stream).write_all(&body)?;
    Ok(())
}

fn recv_framed<T: serde::de::DeserializeOwned>(
    stream: &std::os::unix::net::UnixStream,
) -> anyhow::Result<T> {
    let mut len_buf = [0u8; 4];
    (&*stream).read_exact(&mut len_buf)?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    (&*stream).read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
```

**Decision branch table to implement** (from DESIGN §6 + existing non-Allowed exit):

| Outcome | Action (Phase 48) | Copy from |
|---------|-------------------|-----------|
| `Allowed` | if `Some(output_value_id)` → bag insert **any** sink; `step += 1`; continue | F-01: not process.exec-only |
| `BlockedPendingConfirmation` | **stop**; exit 1; **no re-submit** | substrate only; product hold = Phase 50 |
| `Denied { .. }` / `NotImplemented` | **abort remaining**; exit 1 | existing `!matches!(Allowed)` exit shape (`:407-411`) |
| loop ends with `submitted == 0` | fail-closed reject | DESIGN §8.2 |

**Handle bag seed** — seed from existing locals after ProvideIntent / claims (same call-site convention as `planner.plan` args today):

```rust
// Opaque ValueIds only — never literals / taint / ValueRecord
let mut bag: HashMap<String, ValueId> = HashMap::new();
bag.insert("intent".into(), intent_value_id.clone());
// derived_recipient, body, trusted_subject_handle, trusted_body_handle when present
```

**ValueId is Hash+Eq** (`crates/runtime-core/src/plan_node.rs:70-73`) — bag may key by `ValueId` or by named slots (`String` → `ValueId`); prefer named slots for planner call-site convention.

**Preserve unchanged:** connect → self-confine → ProvideIntent **once** → RequestFd → claims order (module docs lines 1–57). Multi-step starts **after** that setup.

**F-01 comment drift** at lines 376–381 ("Some only on process.exec") — docs-only fix when bagging any `Some`.

---

### `cli/caprun/src/planner.rs` (service / trait seam, transform)

**Analog:** Self — `Planner` trait + `DeterministicPlanner` + `plan_from_intent` (lines 1–213).
**Secondary:** DESIGN §1.2–1.3 additive surface; PLAN-03 module docs lines 3–41.

**Trait seam pattern** (lines 61–80) — **keep** `plan()`; **add** multi-node method with default one-shot adapter:

```61:80:cli/caprun/src/planner.rs
pub trait Planner {
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
}
```

**Recommended additive shape** (discretion; PLAN-03 semantics locked):

```rust
/// Worker-owned routing context — never broker authority.
/// Only opaque ValueIds + typed intent + step index + task_instruction String.
pub struct PlanStreamContext {
    pub intent: CaprunIntent,
    pub step_index: usize,
    pub handles: /* HashMap<String, ValueId> or structured named fields */,
    pub task_instruction: Option<String>,
}

pub trait Planner {
    fn plan(/* existing */) -> PlanNode;

    /// Additive multi-node surface. Default: one-shot via plan() at step 0, then None.
    fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode> {
        if ctx.step_index == 0 {
            Some(self.plan(
                &ctx.intent,
                /* pull named handles from ctx.handles */,
                ctx.task_instruction.clone(),
            ))
        } else {
            None
        }
    }
}
```

**PLAN-03 boundary to preserve** (module docs lines 3–41, trait docs 56–69):

- Parameters: typed `CaprunIntent` + opaque `ValueId` only
- `task_instruction: Option<String>` — **never** a `ValueId`
- No `ValueRecord`, no raw untrusted bytes as values, no taint params
- Planner places handles by **call-site convention**; never mints / never strips taint

**Call-site convention pattern** (lines 162–185) — planner only chooses which offered handle goes in which `PlanArg`:

```162:185:cli/caprun/src/planner.rs
        CaprunIntent::SendEmailSummary { .. } => {
            let to = derived_recipient.unwrap_or_else(|| intent_value_id.clone());
            let body_value_id = body.unwrap_or(trusted_body_handle);
            PlanNode {
                sink: SinkId("email.send".into()),
                args: vec![
                    PlanArg { name: "to".into(), value_id: to },
                    PlanArg { name: "subject".into(), value_id: trusted_subject_handle },
                    PlanArg { name: "body".into(), value_id: body_value_id },
                ],
            }
        }
```

**Implementors:**

| Type | Phase 48 action |
|------|-----------------|
| `DeterministicPlanner` | Inherit default one-shot `plan_next` (email/file byte-stable) |
| `LlmPlanner` | Same default; no multi-step LLM loop (LLM-MS-01 out of v1.10) |
| Test-only multi-node planner | NEW private struct / test fixture emitting static N-node sequence over existing sinks — **not** CaprunIntent coding variant (Phase 49) |

**Keep `plan_from_intent` pure** — no I/O, no async, infallible `-> PlanNode` (lines 112–119).

---

### `cli/caprun/tests/planner.rs` (test, transform)

**Analog:** Self — pure unit tests via `#[path = "../src/planner.rs"]` (lines 20–23).

**Include pattern** (lines 20–29):

```20:29:cli/caprun/tests/planner.rs
// Include the planner module directly so these integration tests can call
// `plan_from_intent` without requiring a lib target in the caprun crate.
#[path = "../src/planner.rs"]
mod planner;

use llm_planner::{PlannerResponse, ResponseArg};
use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};
```

**Assertion helper** (lines 31–37):

```31:37:cli/caprun/tests/planner.rs
fn arg<'a>(plan: &'a PlanNode, name: &str) -> &'a PlanArg {
    plan.args
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("plan must carry a `{name}` arg"))
}
```

**Extend with:**

| Test | Asserts |
|------|---------|
| Default `plan_next` step 0 | `Some(node)` matches existing `plan()` for email/file |
| Default `plan_next` step ≥1 | `None` (one-shot adapter) |
| Test multi-node planner | step 0 → node_a, step 1 → node_b with bag handle in `PlanArg`, step 2 → `None` |
| Opaque-only | bag/API types are `ValueId` only (compile-time; no literal field) |
| Regression | existing `plan_from_intent_*` tests stay green |

**Not Linux-gated** — pure planner surface (header lines 1–6).

---

### `cli/caprun/tests/stream_substrate.rs` (test, sequential + taint) — NEW

**Primary analog:** `cli/caprun/tests/s9_process_exec_block.rs` (genuine exec-output → second process.exec command → Block + `verify_chain`).
**Secondary:** RESEARCH Minimal Tracer T2/T3/T5; hybrid honesty class (substrate ≠ LIVE DONE).

**Genuine bag taint spine** (s9 lines 219–322) — copy provenance backstop + Block match:

```219:322:cli/caprun/tests/s9_process_exec_block.rs
        let output_value_id = mint_from_exec(&mut store, session_id, combined_output, exec_event_id)
            .expect("mint_from_exec must succeed");
        // …
        let plan_node2 = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![PlanArg {
                name: "command".into(),
                value_id: output_value_id,
            }],
        };
        // … submit_plan_node → BlockedPendingConfirmation …
        assert_eq!(
            anchor.provenance_chain[0], exec_event_id,
            "GENUINE-TAINT BACKSTOP: anchor.provenance_chain[0] must equal the \
             process_exited event id (non-stapled)"
        );
        // …
        assert!(
            verify_chain(&locked, &session_id.to_string(), TEST_KEY),
            "verify_chain must be true — ONE unbroken causal chain: \
             session_created -> process_exited -> sink_blocked"
        );
```

**Why `process.exec` command for T3:** s9 header notes role-unconstrained command path — email/file role checks would Deny with SlotTypeMismatch before taint (see s9 comments around lines 236–241).

**Phase 48 framing:** hybrid in-crate multi-node OK for **substrate** STREAM-02; do **not** claim CLI multi-step DONE / LIVE-07/08.

**Recommended cases:**

| ID | Behavior | Gate |
|----|----------|------|
| T2 | Bag stores any `Some(output_value_id)`; second plan arg carries bag handle | unit or hybrid |
| T3 | exec output handle → process.exec command → I2 Block + provenance root + `verify_chain` | `#[cfg(target_os = "linux")]` |
| T5 | After Denied, no further SubmitPlanNode (abort remaining) | unit with mock planner / count |
| F-01 unit | bag insert has no `if sink == process.exec` filter | unit |

**Linux verification:** `MAILPIT_VERIFY_CMD='cargo test -p caprun --test stream_substrate …' bash scripts/mailpit-verify.sh` when Docker available (CLAUDE.md Phase 16+).

---

### `crates/brokerd/tests/stream_multi_submit.rs` (test, sequential multi-submit) — NEW

**Analog:** `crates/brokerd/tests/replay_cas.rs` — same-connection 2× `SubmitPlanNode` + real `run_broker_server`.
**Adapt:** DIFFERENT plan nodes (not identical replay CAS); assert `verify_chain`; store first `output_value_id` into second node when proving bag at broker layer.

**Linux gate + mailpit header pattern** (replay_cas lines 1–24):

```1:24:crates/brokerd/tests/replay_cas.rs
//! … Linux-only: abstract-namespace UDS …
//!
//!   MAILPIT_VERIFY_CMD='cargo test -p brokerd --test replay_cas \
//!     allowed_email_send_replay_delivers_once' bash scripts/mailpit-verify.sh

#![cfg(target_os = "linux")]
```

**Framing helpers** (lines 173–188):

```173:188:crates/brokerd/tests/replay_cas.rs
async fn send_req(stream: &mut tokio::net::UnixStream, req: &BrokerRequest) {
    let body = serde_json::to_vec(req).expect("serialize request");
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await.expect("write length");
    stream.write_all(&body).await.expect("write body");
}

async fn read_resp(stream: &mut tokio::net::UnixStream) -> BrokerResponse {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.expect("read length");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_body = vec![0u8; msg_len];
    stream.read_exact(&mut resp_body).await.expect("read body");
    serde_json::from_slice(&resp_body).expect("deserialize response")
}
```

**spawn_fresh_broker pattern** (lines 197–248) — in-memory audit DB + `persist_session` + `run_broker_server` + abstract sock `\0/agentos/{name}` + `SessionPolicy::allow_all()`.

**Multi-submit sequence to copy shape** (lines 269–350) — change second node to a **different** PlanNode; optionally thread `output_value_id`:

```rust
// ProvideIntent once → IntentAccepted handles
// SubmitPlanNode(node_1) → PlanNodeDecision { Allowed, output_value_id: Some(h) }
// SubmitPlanNode(node_2 with PlanArg { value_id: h }) → decision_2
// drop stream; open audit.db; assert verify_chain(&conn, &session_id, key)
// assert ≥2 evaluations / sink events on SAME session_id
```

**ProvideIntent-once mid-stream (T4):** after first ProvideIntent + optional submit, second ProvideIntent must get `BrokerResponse::Error` with existing message shape (server arm rejects when `intent_provided || fd_requested`). No new UserTrusted mint.

**verify_chain pattern:** also used in `s9_process_exec_block.rs:319` and `durable_anchor.rs` — prefer `brokerd::audit::verify_chain` on the session after N submits.

---

### `crates/brokerd/src/proto.rs` (docs-only, optional)

**Analog:** Self `PlanNodeDecision` docs (lines 242–254).

**F-01 drift** — comments claim `output_value_id` is `Some` **only** for `process.exec` Allowed. Live mint arms also set `Some` for `git.commit` / `http.request` (DESIGN F-01). Phase 48 docs-only fix:

```text
// BEFORE (drift): Some only when sink == process.exec && Allowed
// AFTER (authority): Some when Allowed and broker minted an intermediate output
//   (process.exec / git.commit / http.request today); None otherwise
```

**Do not change wire shape** — `PlanNodeDecision { decision, output_value_id }` stays; no new IPC verb.

---

### `crates/brokerd/src/server.rs` (docs-only, optional)

**Analog:** Self multi-submit loop + Allowed mint arms + comment near SubmitPlanNode response.

**Behavior already correct:** connection loop accepts N independent `SubmitPlanNode`s; each runs `evaluate_plan_node_and_record` independently. Phase 48 **must not** add batch authorize.

**F-01:** fix "process.exec only" comment near response construction (~2257–2259 per RESEARCH) to match multi-sink mint.

**ProvideIntent-once** (authoritative reject text pattern from DESIGN citation):

```text
"ProvideIntent rejected: must arrive exactly once, before any RequestFd (fail-closed)"
```

Worker multi-step must never attempt a second ProvideIntent.

---

### `cli/caprun/src/main.rs` — unchanged in Phase 48

CLI multi-node driver + exit-code taxonomy = Phase 50 (CLI-01/02). Do not expand `caprun run` product path here.

## Shared Patterns

### Sequential multi-submit (STREAM-01)

**Source:** Broker already legal (`server.rs` connection loop); worker one-shot is the gap.
**Apply to:** `worker.rs` loop; `stream_multi_submit.rs`; `stream_substrate.rs`
**Rules:**

- One Session, one worker connection, N× `BrokerRequest::SubmitPlanNode { plan_node }` only
- No `session_id` on IPC (HARD-03)
- No batch / multi-node one-shot I2
- Each decision independent; chain head advances per event → `verify_chain` true

### Opaque handle bag (STREAM-02)

**Source:** DESIGN §2.2; `ValueId` Hash+Eq (`plan_node.rs:70-73`); discard site `worker.rs:389`
**Apply to:** worker loop; planner stream context; bag unit tests
**Rules:**

```rust
// Store ANY Some(output_value_id) — F-01
if let Some(id) = output_value_id {
    bag.insert(format!("out_{step}"), id);
}
```

- Opaque `ValueId` only — never literal / taint / `ValueRecord`
- Post-confirm outputs out-of-bag (F-02) — never re-submit blocked node to "recover" output
- Planner places offered handles only (PLAN-03)

### PLAN-03 / instruction channel

**Source:** `planner.rs` module docs + `task_instruction: Option<String>`
**Apply to:** additive `plan_next` / `PlanStreamContext`
**Rules:** `task_instruction` never becomes `ValueId`; no mid-stream ProvideIntent remint

### Fail-closed decision branching

**Source:** `worker.rs:407-411` + DESIGN §6
**Apply to:** sequential loop body

```rust
match decision {
    ExecutorDecision::Allowed => { /* bag + continue */ }
    ExecutorDecision::BlockedPendingConfirmation { .. } => {
        // Phase 48 substrate: stop, no re-submit (Phase 50 product hold)
        std::process::exit(1);
    }
    ExecutorDecision::Denied { .. } | ExecutorDecision::NotImplemented => {
        // abort remaining
        std::process::exit(1);
    }
}
```

Empty stream (`submitted == 0`) → reject / N/A, not exit 0 (DESIGN §8.2).

### IPC framing (4-byte LE + JSON)

**Source:** `worker.rs:470-489` (sync std); `replay_cas.rs:173-188` (async tokio)
**Apply to:** all multi-submit tests and worker loop iterations

### Linux / verification authority

**Source:** CLAUDE.md Phase 16+; `replay_cas` / s9 headers
**Apply to:** process.exec / SMTP-touching / confinement legs

```bash
./scripts/check-invariants.sh
# host unit:
cargo test -p caprun --test planner -- --nocapture
# Linux authority when Docker available:
MAILPIT_VERIFY_CMD='cargo test -p brokerd --test stream_multi_submit -- --nocapture' \
  bash scripts/mailpit-verify.sh
```

Gates 1 + 3 must stay green — **no** `EffectRequest` under `crates/`; **no** new mint sites.

### HYG-02

**Source:** DESIGN §8; RESEARCH Standard Stack
**Apply to:** entire phase

| Rule | Pin |
|------|-----|
| New crates | zero |
| New mint sites | zero (consume existing `output_value_id` only) |
| New IPC effect verbs | zero (`SubmitPlanNode` × N) |
| CaprunIntent coding variant | Phase 49 — not required for STREAM substrate |

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| *(none for core STREAM)* | — | — | All primary files have exact or strong role-match analogs |
| Full Block-and-Hold stay-connected | product UX | hold | Intentionally **no** product analog this phase — Phase 50; substrate stop-without-re-submit only |
| CaprunIntent coding recipe planner | service | multi-node static | Phase 49; use test-only multi-node planner in 48 |

## Metadata

**Analog search scope:** `cli/caprun/src/{worker,planner}.rs`, `cli/caprun/tests/{planner,s9_process_exec_block}.rs`, `crates/brokerd/tests/{replay_cas,durable_anchor}.rs`, `crates/brokerd/src/proto.rs`, `crates/runtime-core/src/{plan_node,executor_decision}.rs`, prior `47-PATTERNS.md` (doc-only phase)
**Files scanned:** ~15 primary + DESIGN/RESEARCH authority
**Pattern extraction date:** 2026-07-28
**Authority:** `planning-docs/DESIGN-multi-step-plan-stream.md` (CLEARED) + STREAM-01/02; no CONTEXT.md (discuss skipped)
