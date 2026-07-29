# Phase 49: Deterministic Multi-step Coding Planner - Research

**Researched:** 2026-07-29
**Domain:** Deterministic multi-node coding planner on shipped caprun plan-stream substrate (Rust TCB; zero new crates)
**Confidence:** HIGH

## Summary

Phase 49 lands the **deterministic multi-step coding planner** (CODE-01/02) on top of the Phase 48 stream substrate. Phase 48 already shipped: additive `Planner::plan_next` + `PlanStreamContext`, worker sequential N× `SubmitPlanNode`, opaque handle bag (`out_{step}` for any `Some(output_value_id)`), and fail-closed empty/deny/block branches. What is **missing** is a closed `CaprunIntent` coding variant, ProvideIntent multi-field trusted mint + bag seed for ~12 operator args, and a `plan_next` override that emits the fixed sink sequence **file.write → process.exec → git.commit → git.push → github.pr** using only those trusted handles on the success path.

The load-bearing constraint is **not** the planner loop (that exists) — it is the **trusted-arg mint surface**. Today's `ProvideIntent` / `IntentAccepted` path mints at most three handles (`value_id`, optional `subject_value_id`/`body_value_id`) for email/file. A coding recipe needs many distinct `UserTrusted` handles (path, contents, command, args JSON, commit message, remote, refspec, owner, repo, base, head, title, body). Phase 49 must extend ProvideIntent-once multi-mint inside the existing broker arm (Gate 3–sanctioned `server.rs` + `mint_from_intent`) and return named handles so the worker can seed the bag — **without** mid-stream ProvideIntent, without planner mint, and without multi-file untrusted RequestFd before irreversible sinks.

**Primary recommendation:** Add a closed `CaprunIntent` coding variant; extend ProvideIntent mint + `IntentAccepted` with a **named-handle map** (additive; email/file keep three-slot shape); implement `DeterministicPlanner::plan_next` coding arm as a **static step-index sequence** placing bag keys into shipped sink schemas; keep default one-shot adapter for email/file; unit-test plan emission + anti-laundering + email/file regression. CLI multi-node driver, Block-and-Hold product hold, and LIVE-07/08 are **out of phase**.

## User Constraints

> No `*-CONTEXT.md` for this phase (discuss-phase skipped under `--auto`). Locked authority is derived from REQUIREMENTS (CODE-01/02), `planning-docs/DESIGN-multi-step-plan-stream.md` (CLEARED), ROADMAP Phase 49 success criteria, Phase 48 shipped substrate, and CLAUDE.md hard constraints.

### Locked Decisions (must honor — do not re-litigate)

1. **CODE-01:** Deterministic multi-step coding planner produces a multi-node plan covering at least: filesystem edit → `process.exec` (tests) → `git.commit` → `git.push` → `github.pr`. No LLM tool-use loop. Email/file single-node planners remain green. [VERIFIED: `.planning/REQUIREMENTS.md`]
2. **CODE-02:** Success-path nodes use **trusted-intent** operator args only (paths, commands, messages, remotes/refspecs from CLI/intent at session start). Recipe must **not** require multi-file untrusted RequestFd before irreversible sinks. Mid-loop I2 proof uses deliberate tainted-handle routing (LIVE-08 later), not success-path laundering. [VERIFIED: REQUIREMENTS.md]
3. **Stream shape (DESIGN §1):** One Session, one worker connection, N sequential `SubmitPlanNode` only — not batch DAG, not `EffectRequest`. [CITED: DESIGN-multi-step-plan-stream.md §1]
4. **Static sequence sufficient (DESIGN §1.3):** Deterministic coding planner needs only a static ordered sequence / step index. Reactive observation-driven `plan_next` for LLM tool-use is **out of v1.10**. [CITED: DESIGN §1.3]
5. **Handle bag (DESIGN §2 / Phase 48):** Opaque `ValueId`s only; planner never mints / never strips taint; store any `Some(output_value_id)` under `out_{step}`. [VERIFIED: `cli/caprun/src/worker.rs` bag insert + 48-VERIFICATION]
6. **ProvideIntent exactly once** before RequestFd; mid-stream re-ProvideIntent DENIED. [VERIFIED: `crates/brokerd/src/server.rs:2370-2391`]
7. **Trusted-intent success path (DESIGN §4):** All irreversible-sink args minted once at session start from operator-typed intent. **No** weakening CommitIrreversible Draft denies (Step 0.5). [CITED: DESIGN §4]
8. **Closed CaprunIntent only (DESIGN §8.3):** Coding variant is closed enum; all success-path literals from operator intent at ProvideIntent. Exact field/variant names may be chosen in this phase if pins hold. [CITED: DESIGN §8.3]
9. **HYG-02:** Zero new crates; no `EffectRequest` under `crates/`; Gate 3 mint list unchanged or explicitly amended; `check-invariants.sh` green. [CITED: DESIGN §8 / REQUIREMENTS HYG-02]
10. **Phase boundaries:** Phase 49 = planner/recipe (+ mint/bag seed needed for trusted args). **Not** CLI multi-node driver (50), **not** Block-and-Hold product UX (50), **not** LIVE-07/08 (51), **not** packaging (52), **not** LLM multi-step (LLM-MS-01).

### Claude's Discretion (recommend in plan)

- Exact coding variant name and field names (DESIGN §8.3 / §12: naming alone is not a security decision). Recommended: `SafeCodingWorkflow` (research SUMMARY name) with explicit sink-arg fields.
- Whether multi-step lives as `DeterministicPlanner::plan_next` override vs a separate `DeterministicCodingPlanner` type (both OK if PLAN-03 holds; recommend override on `DeterministicPlanner` so default `CAPRUN_PLANNER` path picks it up without env plumbing).
- Exact `IntentAccepted` wire shape for N named handles (additive map vs parallel Option fields). Recommend **one additive named map**.
- Staging strategy for `git.commit` (already-staged only): fold `git add` into operator-typed test command, or insert an extra `process.exec` step. Recommend fold into `test_command` / `test_args` (CODE-01 minimum five sinks still holds).
- LIVE-08 expressibility mechanism: separate proof-only planner / intent flag / test-only `plan_next` branch that places `out_*` into a sensitive arg — **not** success-path default.

### Deferred Ideas (OUT OF SCOPE)

- CLI multi-node driver, exit codes, `caprun run` coding verb (Phase 50 / CLI-01/02)
- Mid-loop Block-and-Hold product path (Phase 50 / CONFIRM-01)
- Non-hybrid LIVE success + I2 Block proofs (Phase 51 / LIVE-07/08)
- Packaging (Phase 52 / PKG-01)
- LLM multi-step / ReAct (LLM-MS-01)
- github.pr merge/comment, replan-from-observation (CODE-BREADTH-01)
- Batch DAG, session-wide confirm waiver, new crates, new sinks, new mint helpers outside Gate 3

## Project Constraints (from CLAUDE.md)

Treat with the same authority as locked DESIGN decisions:

1. **Source of truth:** `planning-docs/PLAN.md` wins on doc/code conflicts.
2. **Effect path locked:** `submit_plan_node(session_id, PlanNode { sink, args: ValueIds })` only — never raw `EffectRequest`. Gate 1 fails if `EffectRequest` appears under `crates/` (annotate intentional mentions with `planner-discipline-allow`).
3. **I0 / I1 / I2:** I2 hardcoded in Rust executor; policy never disables I2; untrusted seed → draft-only; no ambient authority for workers.
4. **Terminology locked:** Intent, Session, Planner, Worker, Broker, Adapter, Effect, Artifact, Event. Project/binary = `caprun`.
5. **TCB is Rust.** Linux-only security claims; macOS stubs expected.
6. **From Phase 16+:** Linux verification that may touch SMTP uses `scripts/mailpit-verify.sh`; full composed LIVE uses `scripts/compose-verify.sh`. Never bare `docker run rust:1` alone when SMTP may fire. Never bind named Docker volumes for `CARGO_TARGET_DIR` as a manual speed hack.
7. **v0/v1 DONE lineage:** substrate working ≠ done; genuine taint through plan nodes + audit DAG. v1.10 DONE = non-hybrid CLI multi-node LIVE (Phase 51), not hybrid rebrand.
8. **Out of scope:** agent frameworks, memory, marketplace, Cedar, web UI, cross-host/Biscuit, gVisor/Firecracker, LLM multi-step until relevant gates.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CODE-01 | Deterministic multi-step coding planner (new `CaprunIntent` coding variant or equivalent) produces multi-node plan over shipped sinks: filesystem edit → `process.exec` (tests) → `git.commit` → `git.push` → `github.pr`. No LLM tool-use. Email/file single-node remain green | §§ Standard Stack, Architecture Patterns 1–3, Code Examples (static sequence), Validation Architecture |
| CODE-02 | Success-path nodes use trusted-intent operator args only; no multi-file untrusted RequestFd before irreversible sinks; recipe does not launder untrusted observations; mid-loop I2 proof routing expressible for LIVE-08 without weakening success path | §§ Architecture Pattern 2–4, Trusted-intent mint path, LIVE-08 expressibility, Common Pitfalls 1–4, Security Domain |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Coding CaprunIntent variant + serde | `runtime-core` (pure types) | CLI main (Phase 50 constructs from argv) | Closed enum; no I/O (Gate 2) |
| ProvideIntent multi-field UserTrusted mint | Broker ProvideIntent arm (`server.rs`) | quarantine `mint_from_intent` | Gate 3–sanctioned sole trusted-string mint; once before RequestFd |
| IntentAccepted named-handle return | Broker proto / wire | Worker bag seed | Worker receives opaque ValueIds only |
| Multi-node plan emission (static sequence) | Planner seam (`cli/caprun/src/planner.rs`) | Worker `plan_next` call site | PLAN-03 handles-only; no mint |
| Sequential submit + bag | Worker (Phase 48 shipped) | Broker multi-submit (already legal) | Phase 49 consumes substrate; does not re-design loop |
| Per-node policy pre-I2 + I2 | Executor TCB | Broker `evaluate_plan_node_and_record` | Unchanged per-node path |
| Sink dispatch (write/exec/commit/push/PR) | Broker sinks (shipped) | sandbox / exec-launcher | Phase 49 does not reimplement sinks |
| CLI multi-node driver + Block-and-Hold | CLI main | Confirm substrate | **Phase 50** — not this phase |
| LIVE multi-node proof | compose-verify / CLI | Hybrid harness only as unit scaffold | **Phase 51** |

## Standard Stack

### Core (reuse only — zero new crates / packages)

| Library / artifact | Version / locus | Purpose | Why Standard |
|--------------------|-----------------|---------|--------------|
| Rust edition 2021, workspace resolver 3 | root `Cargo.toml` | TCB language | Locked project stack [VERIFIED: Cargo.toml + `cargo 1.97.1`] |
| `runtime-core` | workspace | `CaprunIntent`, `PlanNode`, `PlanArg`, `ValueId`, `SinkId` | Add closed coding variant here [VERIFIED: `intent.rs:22-47`] |
| `cli/caprun` planner | `cli/caprun/src/planner.rs` | `Planner`, `PlanStreamContext`, `DeterministicPlanner`, `plan_next` | Primary recipe surface [VERIFIED: Phase 48 `plan_next` default adapter] |
| `cli/caprun` worker | `cli/caprun/src/worker.rs` | Sequential loop + bag (shipped); coding match arm + bag seed | Phase 48 substrate [VERIFIED: `worker.rs` loop] |
| `brokerd` ProvideIntent + sinks | `server.rs`, `sinks/*` | Multi-mint UserTrusted; live write/exec/git/github | Shipped effect surface [VERIFIED: sink_schema + sinks] |
| `executor` sink_schema / sink_sensitivity | workspace | Arg name sets + routing/content/role tables | Recipe must match exact arg names [VERIFIED: `sink_schema.rs:62-160`] |
| `std::collections::HashMap` | std | Bag + named IntentAccepted handles | Already used [VERIFIED: planner/worker] |
| `scripts/check-invariants.sh` | repo | Gates 1–6 | HYG-02 enforcement [VERIFIED: Gate 3 allows `server.rs` mint] |
| `scripts/mailpit-verify.sh` / `compose-verify.sh` | repo | Linux authority when needed | CLAUDE.md Phase 16+ [CITED: CLAUDE.md] |

### Supporting

| Artifact | Purpose | When to Use |
|----------|---------|-------------|
| `planning-docs/DESIGN-multi-step-plan-stream.md` | Authoritative pins | Every task decision |
| `planning-docs/DESIGN-GATE-RECORD-v1.10.md` | CLEARED + re-run triggers | If mint/stream/confirm pivots |
| Phase 48 `cli/caprun/tests/planner.rs` MultiNodeTestPlanner | Pattern for static `plan_next` | Model coding arm tests |
| Phase 48 `cli/caprun/tests/stream_substrate.rs` | Stream branch + taint-via-bag | Regression; LIVE-08 expressibility analog |
| `cli/caprun/tests/live_acceptance_v1_9_composed.rs` | Per-sink trusted mint + role tags | Field/role mint reference (hybrid only) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Static step-index `plan_next` | Observation-driven reactive `plan_next` | **Rejected for v1.10** — LLM-MS-01 deferred (DESIGN §1.3) |
| Separate `DeterministicCodingPlanner` type | Override `DeterministicPlanner::plan_next` | Separate type needs worker selection plumbing; override is simpler for default path |
| Mid-stream ProvideIntent for late args | Session-start multi-mint only | **Rejected** — ProvideIntent laundering (DESIGN §2.3) |
| Reuse one ValueId for all coding args | Distinct mint per literal | **Rejected** — Phase 15 finding #6 forbids degenerate handle reuse |
| Batch `SubmitPlanDAG` | Sequential N× SubmitPlanNode | **Rejected** — I2 bypass (DESIGN §1.4) |
| New crate / workflow engine | In-tree planner + CaprunIntent | **Rejected** — HYG-02 / product boundary |
| LLM tool-use coding loop | Deterministic sequence | **Out of scope** — LLM-MS-01 |

**Installation:** none — **zero** external packages.

**Version verification:** No new packages. Host toolchain `cargo 1.97.1` / `rustc 1.97.1` [VERIFIED: local].

## Package Legitimacy Audit

> Phase 49 installs **zero** external packages (HYG-02 continues). Package Legitimacy Gate **N/A**.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| *(none)* | — | — | — | — | — | No installs |

**Packages removed due to [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
Operator (Phase 50 CLI — OUT OF P49 product path)
  │  INTENT JSON = CaprunIntent::SafeCodingWorkflow { …literals… }
  ▼
Worker (caprun-worker) — Phase 49 touches bag seed + intent match
  1. connect → self-confine (UNCHANGED)
  2. ProvideIntent ONCE { intent, primary_file_derived:false }
       └─ Broker mints N UserTrusted via sequential mint_from_intent
       └─ IntentAccepted { value_id, subject?, body?, named_handles }
  3. [coding success path] NO multi-file untrusted RequestFd / no claim extract
       (optional: single trusted seed RequestFd that does not demote — Phase 50)
  4. bag ← seed named_handles + primary slots
  5. LOOP (Phase 48 shipped):
       plan_next(ctx) → PlanNode{sink, args: ValueIds from bag}
         step 0: file.write    {path, contents}
         step 1: process.exec  {command, args?}
         step 2: git.commit    {message}
         step 3: git.push      {remote, refspec}  → always Block (substrate exit 1)
         step 4: github.pr     {owner,repo,base,head,title,body}
       SubmitPlanNode → PlanNodeDecision
         Allowed + Some(out) → bag[out_{step}] = id  (NOT placed on success path)
         Block → stop no re-submit (Phase 50 hold)
         Deny  → abort remaining
        │
        ▼
Broker: per-node policy pre-I2 → I2 → dispatch (UNCHANGED core)
Executor I2: per-arg routing/content Block; Step 0.5 Draft×CommitIrreversible
```

### Recommended Project Structure (touch points)

```
crates/runtime-core/src/
└── intent.rs                 # ADD CaprunIntent coding variant (closed enum)

crates/brokerd/src/
├── proto.rs                  # EXTEND IntentAccepted with named handles (additive)
└── server.rs                 # EXTEND ProvideIntent match: multi mint_from_intent

cli/caprun/src/
├── planner.rs                # OVERRIDE plan_next for coding; plan_from_intent arm
├── worker.rs                 # coding match: no claim extract; seed bag from named handles
└── main.rs                   # OPTIONAL minimal unknown-kind still fail-closed;
                              # full coding CLI verb = Phase 50 (do not productize here)

cli/caprun/tests/
├── planner.rs                # EXTEND: coding multi-node emission + email/file regression
└── (optional) coding_planner_mint.rs  # ProvideIntent multi-mint integration if needed

# DO NOT for Phase 49 product scope:
# - Block-and-Hold stay-connected (Phase 50)
# - LIVE hybrid→CLI framing (Phase 51)
# - New crates / Gate 3 new mint tokens
```

### Pattern 1: Static step-index coding recipe (CODE-01)

**What:** `plan_next` matches `ctx.step_index` and returns `Some(PlanNode)` with exact shipped sink + arg names, placing **only** bag handles offered under documented keys. Returns `None` after the last step.

**When to use:** Always for the v1.10 deterministic coding path.

**Prescriptive sequence (minimum CODE-01):**

| step | sink | PlanArg names | bag keys (recommended) |
|------|------|---------------|------------------------|
| 0 | `file.write` | `path`, `contents` | `write_path`, `write_contents` |
| 1 | `process.exec` | `command` (+ optional `args`) | `test_command`, `test_args` |
| 2 | `git.commit` | `message` | `commit_message` |
| 3 | `git.push` | `remote`, `refspec` | `push_remote`, `push_refspec` |
| 4 | `github.pr` | `owner`, `repo`, `base`, `head`, `title`, `body` | `pr_owner`, `pr_repo`, `pr_base`, `pr_head`, `pr_title`, `pr_body` |

Missing required bag key → return `None` early (fail-closed empty/partial stream at worker, DESIGN §8.2) rather than inventing a literal.

**Sink schema (exact — do not invent arg names)** [VERIFIED: `crates/executor/src/sink_schema.rs`]:

- `file.write`: required `{path, contents}`
- `process.exec`: required `{command}`; optional `{args, cwd}` — `args` literal is **JSON `Vec<String>`** [VERIFIED: `process_exec.rs:127-135`]
- `git.commit`: required `{message}` only (commits **already-staged** changes) [VERIFIED: `git_commit.rs` comment + argv]
- `git.push`: required `{remote, refspec}`
- `github.pr`: required `{owner, repo, base, head, title, body}`

**Staging note:** `git.commit` does not take a pathspec and only commits staged changes. Operator-typed `test_command`/`test_args` SHOULD stage as needed (e.g. command=`sh`, args=`["-c","git add -A && cargo test"]`) so the five-node recipe remains honest without inventing a new sink.

**file.write pre-existence:** Live sink uses O_TRUNC on an **existing** file [VERIFIED: `live_acceptance_v1_9_composed.rs:538-539`]. Fixtures / Phase 50 workspace setup must pre-create the target path (or a later breadth phase can use `file.create` first — not required for CODE-01 if fixtures pre-create).

### Pattern 2: Trusted-intent multi-mint at ProvideIntent (CODE-02)

**What:** All success-path literals live on the closed `CaprunIntent` coding variant. Broker ProvideIntent arm exhaustively matches the variant and runs **sequential** `mint_from_intent` calls (same linear chain-head threading as email subject/body), returning named handles.

**Critical gap today** [VERIFIED: `intent.rs` only two variants; `IntentAccepted` only three slots at `proto.rs:219-223`; ProvideIntent match only email/file at `server.rs:2423-2434`]:

```219:223:crates/brokerd/src/proto.rs
    IntentAccepted {
        value_id: runtime_core::plan_node::ValueId,
        subject_value_id: Option<runtime_core::plan_node::ValueId>,
        body_value_id: Option<runtime_core::plan_node::ValueId>,
    },
```

*(Exact types: `ValueId` from `runtime_core::plan_node` — three fields only today.)*

**Prescriptive wire extension (additive):**

```rust
// proto.rs — additive; email/file set named_handles empty / default
IntentAccepted {
    value_id: ValueId,                      // primary (coding: write_path)
    subject_value_id: Option<ValueId>,      // email only
    body_value_id: Option<ValueId>,         // email only
    named_handles: Vec<(String, ValueId)>,  // or BTreeMap — stable serde
}
```

Use a **plain required field** (not silent `#[serde(default)]` that hides missing provenance) **or** document default empty vec with exhaustive construction sites updated — prefer explicit empty vec at every construction site (matches Phase 15 discipline on `primary_file_derived` / `output_value_id`).

**Role tags for Step 1c** [VERIFIED: `sink_sensitivity::expected_role`]:

| Minted field | Recommended `origin_role` | Why |
|--------------|---------------------------|-----|
| write `path` | `Some("path")` | file.write path expects `path`/`relative_path` |
| write `contents` | `Some("path")` | file.write contents admits `path` \| `exec_output` \| `doc_fragment` — use `path` for trusted operator contents (same as LIVE-05 clean mint) |
| test command / args | `None` or descriptive | process.exec command/args unconstrained at role gate |
| commit message | unconstrained OK | git.commit message role `None` |
| push remote/refspec | unconstrained OK | git.push roles unconstrained |
| PR six fields | unconstrained OK | github.pr roles unconstrained |

**Gate 3:** Additional `mint_from_intent(` in `server.rs` is fine — Gate 3 greps `mint_from_read` / `mint_from_derivation` / `mint_from_exec` / `mint_from_http` / `.mint(` restricted to quarantine + server (+ value_store for `.mint`). Do **not** call `.mint` from planner/worker. [VERIFIED: `scripts/check-invariants.sh` Gate 3]

**primary_file_derived for coding success path:** must be `false` (operator-typed). Never mint coding success-path args via `mint_from_read` (M7).

### Pattern 3: Keep one-shot email/file green

**What:** Default `plan_next` one-shot adapter (`step_index == 0` → `plan()`, else `None`) stays for non-coding intents. `plan_from_intent` email/file arms unchanged. Worker claim-extract match arms unchanged for email/file.

**When:** Every regression run — CODE-01 success criterion 3.

**Exhaustive match blast radius** [VERIFIED: grep CaprunIntent match sites]: adding a variant forces compile fixes in:

- `crates/brokerd/src/server.rs` ProvideIntent arm
- `cli/caprun/src/planner.rs` `plan_from_intent` + `LlmPlanner` arms
- `cli/caprun/src/worker.rs` claim-extract match
- `cli/caprun/src/main.rs` intent-kind map (can keep unknown-kind fail-closed until Phase 50)
- Any test helpers constructing intents

**LlmPlanner:** coding arm must **fail closed** (no multi-step LLM) — exit 1 / refuse coding intent rather than invent tool-use.

### Pattern 4: LIVE-08 expressibility without success-path laundering (CODE-02)

**What:** Success-path `plan_next` **never** places `out_*` bag handles into routing/content-sensitive args. LIVE-08 (Phase 51) needs a **deliberate** alternate routing that places a genuine tainted handle (e.g. `out_1` from process.exec / `mint_from_exec`) into e.g. `github.pr`/`body` or `git.push`/`refspec` under a policy-permitted sink.

**Prescriptive Phase 49 deliverable (expressibility only, not LIVE proof):**

1. Document bag key contract: `out_{step}` retains untrusted provenance.
2. Unit/test-only planner (or `#[cfg(test)]` / proof flag on intent **default off**) that at PR step places `out_1` into `body` instead of `pr_body`.
3. Assert success-path recipe **does not** reference `out_*` keys.
4. Optionally reuse Phase 48 Linux taint-via-bag pattern as the hybrid spine reference — **frame as expressibility, not LIVE-07 DONE**.

Do **not** implement full CLI LIVE-08 in Phase 49.

### Anti-Patterns to Avoid

- **Planner invents string literals for PlanArg:** breaks PLAN-03 / UserTrusted == human-typed.
- **Re-ProvideIntent after observations:** laundering valve (DESIGN §2.3).
- **Success path routes `out_*` into PR body/refspec:** launders untrusted into irreversible args.
- **Multi-file RequestFd then push:** HARDEN-01 Draft demotion kills CommitIrreversible (DESIGN §4).
- **Weaken Step 0.5 Draft×CommitIrreversible:** I0/I1 breach — rejected.
- **EffectRequest / free-form tool map:** Gate 1.
- **Batch authorize whole recipe:** rejected stream shape.
- **Claim hybrid composition as Phase 49 DONE for multi-step CLI:** honesty class (LIVE-05); Phase 49 is planner emission only.
- **Degenerate one handle reused for all coding args:** Phase 15 finding #6.
- **LLM multi-step in Deterministic planner:** out of scope.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-node submit loop | New orchestration crate / re-loop | Phase 48 worker loop | Already STREAM-01 complete |
| Intermediate output storage | Literal cache / re-mint | Opaque bag `out_{step}` | Taint must stay broker-owned |
| Trusted multi-arg mint | Mid-stream ProvideIntent / planner mint | ProvideIntent-once multi `mint_from_intent` in server.rs | Only UserTrusted string mint site |
| Sink arg validation | Custom arg maps | `sink_schema` + `sink_sensitivity` | Exact names/roles already hardcoded |
| process.exec argv packing | Ad-hoc string concat | JSON `Vec<String>` literal for `args` | Sink contract [VERIFIED: process_exec] |
| git commit plumbing | libgit2 / new crate | Shipped `git.commit` sink | Pattern B exec-launcher |
| git push / github.pr | Worker-side network | Broker-resident sinks | Kernel deny-net on worker |
| I2 Block proof | Stapled taint at sink | Genuine `mint_from_exec` → bag → PlanArg | LIVE-08 / Phase 48 taint-via-bag precedent |
| Email/file multi-step rewrite | Force all intents through coding planner | Keep one-shot `plan()` + default `plan_next` | CODE-01 no-regression |

**Key insight:** Phase 49 is a **recipe + mint-surface** problem on a finished stream substrate — not a new runtime architecture.

## Common Pitfalls

### Pitfall 1: IntentAccepted three-slot bottleneck
**What goes wrong:** Coding intent lands but only three handles return; planner reuses one ValueId for path/command/PR body.  
**Why:** Pre-Phase-49 wire shape is email-shaped.  
**How to avoid:** Additive named-handle map; one `mint_from_intent` per operator literal; unit-test distinct ValueIds per bag key.  
**Warning signs:** Same `ValueId` in multiple PlanArgs of different semantics; missing proto field.

### Pitfall 2: Success-path laundering via `out_*`
**What goes wrong:** After green tests, planner puts exec stdout into PR body as "summary".  
**Why:** Product instinct; bag makes it easy.  
**How to avoid:** Success-path static sequence uses only intent-minted keys; proof-only path is separate and default-off.  
**Warning signs:** `handles.get("out_")` in success-path coding arm.

### Pitfall 3: HARDEN-01 Draft demotion before irreversible sinks
**What goes wrong:** Worker RequestFd multi-file / claim extract demotes Session → Step 0.5 denies file.write/exec/push/PR.  
**Why:** Coding loop pressure after reads.  
**How to avoid:** Coding success path: no multi-file untrusted RequestFd; no claim-driven demotion; trusted-intent only (DESIGN §4).  
**Warning signs:** `session_demoted` events before step 0 Allowed; DraftOnlySessionDeniesCommitIrreversible.

### Pitfall 4: git.commit with nothing staged
**What goes wrong:** Recipe Allows through I2 but git commit fails closed (no staged changes).  
**Why:** Sink commits already-staged only.  
**How to avoid:** Operator-typed test command stages; fixtures stage; document in intent field docs.  
**Warning signs:** `process_spawn_failed` / non-zero git exit on commit step.

### Pitfall 5: file.write target missing
**What goes wrong:** O_TRUNC fails if path does not exist.  
**Why:** Live sink requires pre-existing file.  
**How to avoid:** Fixtures pre-create; Phase 50 workspace checklist.  
**Warning signs:** sink dispatch Err on write leg.

### Pitfall 6: Breaking email/file exhaustive matches
**What goes wrong:** New CaprunIntent variant; incomplete match arms; LlmPlanner panics.  
**Why:** Closed enum is load-bearing.  
**How to avoid:** Plan a compile-green sweep of all match sites; LlmPlanner fail-closed for coding; keep email/file arms byte-stable.  
**Warning signs:** `cargo test -p caprun --test planner` red; non_exhaustive errors.

### Pitfall 7: Role-check Deny on file.write
**What goes wrong:** Mint contents without allowed `origin_role` → structural Deny (not I2 Block).  
**Why:** Step 1c role gate.  
**How to avoid:** Mint path/contents with `Some("path")` as LIVE-05 does.  
**Warning signs:** `Denied` with role mismatch, not `BlockedPendingConfirmation`.

### Pitfall 8: Treating Phase 49 as CLI multi-node DONE
**What goes wrong:** Hybrid in-crate submit of coding nodes claimed as v1.10 product.  
**Why:** LIVE-05 honesty class.  
**How to avoid:** Phase 49 success = plan emission + mint/bag seed + unit tests; CLI driver = Phase 50; LIVE = Phase 51.  
**Warning signs:** ROADMAP Phase 50/51 checkboxes flipped early.

### Pitfall 9: DESIGN re-run trigger on mint path pivot
**What goes wrong:** Mid-stream trusted remint or new mint site outside Gate 3 without re-trace.  
**Why:** DESIGN §13.2 trusted-arg mint path pivot.  
**How to avoid:** Keep ProvideIntent-once at session start only; extra fields at same arm are OK; do not add mid-stream mint verbs. If inventing a new mint helper outside server/quarantine, re-run adversarial gate.  
**Warning signs:** New IPC mint verb; planner-side mint.

### Pitfall 10: Always-confirm git.push surprises substrate tests
**What goes wrong:** Full five-node stream never completes Allowed through step 3 under real broker — push rewrites to Block.  
**Why:** Broker always-confirm on clean git.push [VERIFIED: `server.rs:807-848`].  
**How to avoid:** Unit-test plan **emission** without requiring Allowed on push; integration expectations: Block at push is success-path normal until Phase 50 hold.  
**Warning signs:** Tests that require five Allowed decisions on live broker without confirm.

## Code Examples

Verified patterns from codebase (adapt for coding; do not invent EffectRequest).

### CaprunIntent closed variant shape (prescriptive)

```rust
// crates/runtime-core/src/intent.rs — ADD (names discretionary)
// Source: DESIGN §8.3 closed enum; sink_schema arg sets
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum CaprunIntent {
    SendEmailSummary { recipient: String, subject: String, body: String },
    CreateFileFromReport { path: String },
    /// Deterministic multi-step Safe Coding Agent recipe (Phase 49 / CODE-01).
    /// All fields are operator-typed; minted UserTrusted once at ProvideIntent.
    SafeCodingWorkflow {
        path: String,
        contents: String,
        test_command: String,
        /// JSON-encoded `Vec<String>` for process.exec `args`, or empty/"[]".
        test_args_json: String,
        commit_message: String,
        remote: String,
        refspec: String,
        owner: String,
        repo: String,
        base: String,
        head: String,
        pr_title: String,
        pr_body: String,
    },
}
```

### Static plan_next coding arm (prescriptive)

```rust
// cli/caprun/src/planner.rs — pattern after MultiNodeTestPlanner (Phase 48)
// Source: cli/caprun/tests/planner.rs MultiNodeTestPlanner + sink_schema
fn plan_coding_next(ctx: &PlanStreamContext) -> Option<PlanNode> {
    let h = |key: &str| ctx.handles.get(key).cloned();
    match ctx.step_index {
        0 => Some(PlanNode {
            sink: SinkId("file.write".into()),
            args: vec![
                PlanArg { name: "path".into(), value_id: h("write_path")? },
                PlanArg { name: "contents".into(), value_id: h("write_contents")? },
            ],
        }),
        1 => {
            let mut args = vec![PlanArg {
                name: "command".into(),
                value_id: h("test_command")?,
            }];
            if let Some(a) = h("test_args") {
                args.push(PlanArg { name: "args".into(), value_id: a });
            }
            Some(PlanNode { sink: SinkId("process.exec".into()), args })
        }
        2 => Some(PlanNode {
            sink: SinkId("git.commit".into()),
            args: vec![PlanArg {
                name: "message".into(),
                value_id: h("commit_message")?,
            }],
        }),
        3 => Some(PlanNode {
            sink: SinkId("git.push".into()),
            args: vec![
                PlanArg { name: "remote".into(), value_id: h("push_remote")? },
                PlanArg { name: "refspec".into(), value_id: h("push_refspec")? },
            ],
        }),
        4 => Some(PlanNode {
            sink: SinkId("github.pr".into()),
            args: vec![
                PlanArg { name: "owner".into(), value_id: h("pr_owner")? },
                PlanArg { name: "repo".into(), value_id: h("pr_repo")? },
                PlanArg { name: "base".into(), value_id: h("pr_base")? },
                PlanArg { name: "head".into(), value_id: h("pr_head")? },
                PlanArg { name: "title".into(), value_id: h("pr_title")? },
                PlanArg { name: "body".into(), value_id: h("pr_body")? },
            ],
        }),
        _ => None,
    }
}
```

### Success path must NOT place bag outputs (anti-launder check)

```rust
// Source: DESIGN §4.5 / CODE-02 — test assertion pattern
for step in 0..5 {
    let node = plan_coding_next(&ctx_at(step, &success_bag)).unwrap();
    for arg in &node.args {
        assert!(
            !success_bag.iter().any(|(k, v)| k.starts_with("out_") && v == &arg.value_id),
            "success-path step {step} must not place out_* handles into sink args"
        );
    }
}
```

### LIVE-08 expressibility (test-only placement)

```rust
// Source: Phase 48 taint-via-bag + DESIGN §4.5 — NOT the success path
// At github.pr step, place out_1 (exec output) into body under policy-permitted sink:
PlanArg {
    name: "body".into(),
    value_id: ctx.handles.get("out_1").cloned()?, // genuine mint_from_exec handle
}
// title/other args still trusted intent handles
```

### Existing MultiNodeTestPlanner reference

```918:960:cli/caprun/tests/planner.rs
    fn plan_next(&self, ctx: &planner::PlanStreamContext) -> Option<PlanNode> {
        match ctx.step_index {
            0 => { /* file.create with intent handle */ }
            1 => {
                // Place a bag-offered handle into a PlanArg (STREAM-02).
                let bag_handle = ctx
                    .handles
                    .get("out_0")
                    .or_else(|| ctx.handles.get("seed_handle"))
                    .cloned()?;
                // …
            }
            _ => None,
        }
    }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| One-shot worker + discard `output_value_id` | Sequential loop + opaque bag | Phase 48 (2026-07-28) | Coding recipe can pile nodes |
| Hybrid LIVE-05 multi-session composition | Single Session multi-node stream (product path incomplete) | v1.9 → v1.10 roadmap | Phase 49 recipe; Phase 50 CLI; Phase 51 LIVE |
| Email/file only CaprunIntent | + coding closed variant | Phase 49 | Multi-arg trusted mint surface |
| IntentAccepted 1–3 handles | Named multi-handle map | Phase 49 (this research) | Unblocks CODE-02 trusted args |
| LLM multi-step | Deferred | LLM-MS-01 | Deterministic first |

**Deprecated/outdated:**

- Worker one-shot plan (Phase 48 replaced)
- "process.exec only" bag comments (F-01 drift fixed Phase 48)
- Hybrid composition as multi-step DONE claim (forbidden for v1.10 LIVE)

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Variant name `SafeCodingWorkflow` is acceptable (DESIGN leaves naming open) | Pattern 1 / Code Examples | Rename only — low security risk |
| A2 | Additive `named_handles: Vec<(String,ValueId)>` is preferred IntentAccepted shape | Pattern 2 | Alternate map type OK if additive + exhaustive construction |
| A3 | Staging can be folded into operator-typed process.exec without a sixth node | Pattern 1 | May need extra process.exec step if product forbids shell `-c` |
| A4 | Phase 49 may touch worker bag seed + ProvideIntent without being "CLI multi-node" | Architecture | Over-scoping into Phase 50 if CLI verb/confirm-hold lands here |
| A5 | DESIGN §13.2 re-run not required for multi-field ProvideIntent-once at same arm | Pitfall 9 | If reviewers treat multi-field as mint-path pivot, schedule re-trace |

**If empty table were required for verification-only claims:** remaining claims are code-verified or DESIGN-cited; A1–A5 are product/shape discretion only.

## Open Questions

1. **IntentAccepted serde default vs explicit empty**
   - What we know: Phase 15 avoided silent defaults for security-relevant fields.
   - What's unclear: whether additive `named_handles` may use `#[serde(default)]` for old test fixtures.
   - Recommendation: explicit empty at all construction sites; update tests; avoid default that hides incomplete mint.

2. **Worker RequestFd for coding on host tests**
   - What we know: success path must not multi-file demote; worker currently always RequestFd.
   - What's unclear: whether Phase 49 changes worker to skip RequestFd for coding or leaves skip to Phase 50.
   - Recommendation: Phase 49 coding arm **skips claim extract**; RequestFd of a single trusted seed file is OK if it matches HARDEN-01; prefer skip RequestFd for coding if easy — document choice in plan.

3. **CLI intent-kind string in Phase 49**
   - What we know: main.rs fail-closes unknown kinds.
   - What's unclear: whether to add a non-product parse path for integration tests.
   - Recommendation: unit tests construct `CaprunIntent` in-process; full `caprun run safe-coding-workflow` verb is Phase 50.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | build + unit tests | ✓ | cargo 1.97.1 / rustc 1.97.1 | — |
| `scripts/check-invariants.sh` | HYG-02 gate | ✓ | present | — |
| `scripts/mailpit-verify.sh` | Linux SMTP-safe verify | ✓ | present | Host unit tests for pure planner |
| Docker / Colima | mailpit-verify if used | probe at exec | — | Prefer host `cargo test -p caprun --test planner` for pure CODE-01 |
| pkg-config / libssl-dev | lettre / native-tls builds | host-dependent | — | mailpit-verify installs; or userland openssl debs (Phase 48 note) |

**Missing dependencies with no fallback:** none for pure planner unit tests.

**Missing dependencies with fallback:** Docker for full Linux gate — use host unit tests for Phase 49 planner; mailpit-verify when integration touches broker SMTP paths (coding path typically does not need SMTP).

## Validation Architecture

> `workflow.nyquist_validation` absent in `.planning/config.json` → treat as **enabled**.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (workspace) |
| Config file | none — Cargo workspace defaults |
| Quick run command | `./scripts/check-invariants.sh && cargo test -p caprun --test planner -- --nocapture` |
| Full suite command | `cargo test --workspace --no-fail-fast` (Linux security legs); SMTP-touching: `bash scripts/mailpit-verify.sh` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CODE-01 | Coding `plan_next` emits 5 sinks in order with correct arg names | unit | `cargo test -p caprun --test planner coding_ — --nocapture` | ❌ Wave 0 |
| CODE-01 | Email `plan_next` step0 still matches `plan()` | unit | `cargo test -p caprun --test planner plan_next_step0_matches_plan_for_email` | ✅ |
| CODE-01 | File `plan_next` step0 still matches `plan()` | unit | `cargo test -p caprun --test planner plan_next_step0_matches_plan_for_file` | ✅ |
| CODE-01 | Default one-shot ends at step≥1 | unit | `cargo test -p caprun --test planner plan_next_step_ge1_returns_none_one_shot` | ✅ |
| CODE-02 | Success-path args only from intent bag keys (no `out_*`) | unit | new `coding_success_path_does_not_place_out_handles` | ❌ Wave 0 |
| CODE-02 | ProvideIntent multi-mint returns distinct named handles | unit/integration | new broker or worker-level mint test | ❌ Wave 0 |
| CODE-02 | LIVE-08 expressibility: proof planner places `out_*` into sensitive arg | unit | new `coding_i2_proof_places_out_handle` | ❌ Wave 0 |
| HYG-02 | Gate 1/3 green; zero new crates | script | `./scripts/check-invariants.sh` | ✅ |
| STREAM regression | stream_substrate still green | unit | `cargo test -p caprun --test stream_substrate` | ✅ |

### Sampling Rate

- **Per task commit:** `./scripts/check-invariants.sh && cargo test -p caprun --test planner`
- **Per wave merge:** above + `cargo test -p caprun --test stream_substrate` + `cargo test -p brokerd --test stream_multi_submit` (Linux)
- **Phase gate:** Full workspace green on host for non-Linux-gated; Linux legs via mailpit-verify when broker integration runs; no claim of LIVE-07/08

### Wave 0 Gaps

- [ ] `cli/caprun/tests/planner.rs` — coding multi-node emission tests (CODE-01)
- [ ] `cli/caprun/tests/planner.rs` — success-path no-`out_*` placement (CODE-02)
- [ ] `cli/caprun/tests/planner.rs` — LIVE-08 expressibility placement test (CODE-02)
- [ ] ProvideIntent multi-mint / IntentAccepted named_handles round-trip tests (`crates/brokerd/tests/proto_claims.rs` or sibling)
- [ ] Exhaustive CaprunIntent match compile sweep (not a test file — plan task)
- [ ] Framework install: none — use existing cargo test

*(Existing infrastructure covers email/file plan_next, stream substrate, Gate scripts; coding-specific tests are Wave 0.)*

### Dimensions & Confidence (Nyquist)

| Dimension | What to measure | Pass signal | Confidence |
|-----------|-----------------|-------------|------------|
| Plan shape | 5 sinks × arg names × order | Exact equality to sink_schema | HIGH |
| Trust surface | All success PlanArg ValueIds ∈ intent-minted set | No `out_*` intersection | HIGH |
| Regression | Email/file plan_next + plan() | Existing 22+ planner tests green | HIGH |
| Mint integrity | N distinct UserTrusted handles from one ProvideIntent | Distinct ValueIds; roles OK | MEDIUM until mint tests land |
| Anti-launder expressibility | Proof path can place tainted handle | Unit placement only | MEDIUM (full LIVE = P51) |
| Runtime end-to-end CLI | Five-node CLI Session | **Out of Phase 49** | N/A this phase |

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (no new auth) | Existing GitHub token / grant path unchanged |
| V3 Session Management | yes (Session continuity) | ProvideIntent-once; occupancy latch; same Session stream |
| V4 Access Control | yes | POLICY-02/03 pre-I2 narrowing; never overrides I2 |
| V5 Input Validation | yes | Closed CaprunIntent; sink_schema exact arg sets; fail-closed unknown intent |
| V6 Cryptography | no new crypto | Gate 5 ring-only unchanged |

### Known Threat Patterns for multi-step coding planner

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-node taint laundering via `out_*` | Tampering / Elevation | Success path never places `out_*`; opaque bag; planner never mints |
| Mid-stream ProvideIntent remint | Elevation | Broker once-before-RequestFd guard |
| Draft demotion "fixed" by class weaken | Elevation / I0-I1 | Trusted-intent success path; no Step 0.5 change |
| Degenerate single trusted handle for all args | Tampering | One mint per literal; Phase 15 finding #6 |
| EffectRequest / free-form tool map | Spoofing | Gate 1; PlanNode path only |
| Instruction channel as bindable ValueId | Tampering | `task_instruction` remains String, unused by deterministic coding |
| Hybrid sold as CLI multi-step DONE | Integrity of claims | Phase boundaries; LIVE Phase 51 only |
| New mint outside Gate 3 | Tampering | mint only in server ProvideIntent via mint_from_intent |

## Sources

### Primary (HIGH confidence)

- [VERIFIED: codebase] `crates/runtime-core/src/intent.rs` — CaprunIntent closed enum (email/file only today)
- [VERIFIED: codebase] `cli/caprun/src/planner.rs` — `Planner`, `PlanStreamContext`, default `plan_next`, `DeterministicPlanner`
- [VERIFIED: codebase] `cli/caprun/src/worker.rs` — sequential loop, bag seed, decision branch table
- [VERIFIED: codebase] `crates/brokerd/src/proto.rs` — ProvideIntent / IntentAccepted wire
- [VERIFIED: codebase] `crates/brokerd/src/server.rs` — ProvideIntent once guard + mint arm; git.push always-confirm
- [VERIFIED: codebase] `crates/executor/src/sink_schema.rs` + `sink_sensitivity.rs` — arg sets, roles, effect classes
- [VERIFIED: codebase] `cli/caprun/tests/planner.rs` — MultiNodeTestPlanner pattern; 22 tests baseline
- [VERIFIED: codebase] Phase 48 `48-01-SUMMARY.md`, `48-02-SUMMARY.md`, `48-VERIFICATION.md`
- [CITED: planning-docs/DESIGN-multi-step-plan-stream.md] §§1–8, §12–13 (CLEARED)
- [CITED: .planning/REQUIREMENTS.md] CODE-01/02, STREAM-01/02 complete, LIVE/CLI deferred
- [CITED: .planning/research/SUMMARY.md + ARCHITECTURE.md + PITFALLS.md] v1.10 multi-step research

### Secondary (MEDIUM confidence)

- [VERIFIED: codebase] `live_acceptance_v1_9_composed.rs` — per-sink trusted mint roles (hybrid composition reference only)
- Host toolchain versions (local probe 2026-07-29)

### Tertiary (LOW confidence)

- Exact product CLI flag names for coding fields (Phase 50)
- Whether adversarial re-trace is required for multi-field ProvideIntent-once (A5)

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — zero new packages; substrate and sinks code-verified
- Architecture: **HIGH** — DESIGN pins + Phase 48 shipped loop; mint-surface gap verified
- Pitfalls: **HIGH** — DESIGN threat model + LIVE-05/HARDEN-01 code facts
- IntentAccepted wire shape: **MEDIUM** — prescriptive recommendation, not pre-locked in DESIGN field list
- Staging/`sh -c` product choice: **MEDIUM** — operational, not security pin

**Research date:** 2026-07-29  
**Valid until:** 2026-08-28 (30 days; stable substrate; re-verify if Phase 48 worker/planner reverts)

---

*Phase 49 research complete. Planner can create PLAN.md files for CaprunIntent + ProvideIntent multi-mint + static coding `plan_next` + regression tests — without CLI multi-node product path or LIVE proof.*
