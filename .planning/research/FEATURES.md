# Feature Research

**Domain:** Multi-step Safe Coding Agent loop on a security-first Intent Runtime (caprun v1.10)
**Researched:** 2026-07-23
**Confidence:** HIGH (caprun baseline + shipped seams); MEDIUM (ecosystem coding-agent norms from official docs)

## Scope note (read first)

This research is **v1.10-only**. It does **not** re-scope already-shipped substrate:

| Already built (v1.7–v1.9) | Status |
|---------------------------|--------|
| Effect surface: `process.exec`, fs read/write, `git.commit`, `git.push`, `github.pr`, `http.request` GET/write | Shipped |
| Per-session policy binding (never overrides I2), `caprun confirm`/`deny`/`review`/`grant` | Shipped |
| `caprun audit` read-only DAG viewer + `verify_chain` | Shipped |
| Single-node `caprun run` (email/file intents only) | Shipped |
| Hybrid LIVE-05 multi-sink proof (in-crate composition through real broker arms) | Shipped + honestly disclosed |

**v1.10 goal:** a design partner drives **edit → test → commit → push → open PR** as **one Session via the CLI** (not hybrid in-crate composition), with I2 / policy / confirm intact and a genuine audit chain end-to-end.

caprun is an **Intent Runtime**, not an agent framework. Feature expectations below are filtered through that product boundary.

---

## How multi-step coding-agent loops work (ecosystem)

### Dominant orchestration patterns

| Pattern | Shape | Who uses it | Caprun fit for v1.10 |
|---------|-------|-------------|----------------------|
| **ReAct / agentic tool loop** | while model returns tool_use → host executes → tool_result → replan | Claude tool-use API, Claude Code, SWE-agent, OpenHands-class agents | **Defer** LLM multi-step; seam exists (v1.4 sidecar) but not this milestone |
| **Plan-then-execute** | emit multi-step plan up front, then run each step | Some enterprise orchestrators; research agents | Close to v1.10 **deterministic multi-node stream** |
| **Scripted multi-node stream** | fixed or parameterized node sequence over known sinks | Integration harnesses, “workflow” runners | **★ Choose this** for v1.10 — manual-ops-first, no replan-from-taint |
| **Independent-action bash loop** | linear history; each action is a fresh subprocess | mini-SWE-agent | Informative (simplicity wins) but ambient shell authority is the opposite of caprun |

**Canonical agentic loop (industry):** model proposes structured tool call → host executes → result re-enters conversation → repeat until stop. Official Anthropic tool-use docs describe this as a `while stop_reason == "tool_use"` client loop. Coding products layer git/PR/test tools on top of that loop.

**Caprun’s structural difference:** tools are not ambient. Every external effect is a **broker-mediated `PlanNode`** (`sink` + `ValueId` args) evaluated by the **Rust TCB executor (I2)**. The planner holds handles only — never literals/taint (PLAN-03). Multi-step therefore means **a stream of `SubmitPlanNode` calls in one Session**, not a freer shell session.

### What design partners expect from “coding agent under policy”

From Aider / Claude Code / SWE-agent product surfaces (cross-checked official docs):

1. **One continuous session** for edit → test → commit → (push) → PR — not five disconnected commands with no shared audit story.
2. **Run tests/commands** and see outcomes before irreversible steps.
3. **Git commit + PR** as first-class outcomes of the loop.
4. **Human gate** on high-blast-radius actions (Claude Code permission allow/deny/ask; Aider auto-commit with `/undo`).
5. **Installable path** — single binary or documented install, credentials via env, runs on a Linux box/container.
6. **Visibility** into what happened (diffs, logs, transcript). For caprun the trust surface is the **audit DAG**, not a chat transcript.

What they do **not** need from caprun (and often already have elsewhere): free-form chat UX, IDE plugins, multi-agent swarms, long-term memory, marketplace plugins, “auto mode” that softens security.

---

## Feature Landscape

### Table Stakes (must for v1.10 design-partner slice)

Features a design partner assumes when told “drive Safe Coding Agent edit→test→commit→push→PR under policy+I2 from the CLI.” Missing any of these = product gap (exactly the hybrid LIVE-05 honesty gap).

| Feature | Why Expected | Complexity | Category | Notes / deps |
|---------|--------------|------------|----------|--------------|
| **Multi-node plan stream** | Industry agents are multi-step by default; single `PlanNode` cannot express coding workflows | **HIGH** | MUST | Extend v1.4 `Planner` seam / worker submit loop so one Session can submit **N** plan nodes. Today: `Planner::plan → PlanNode` once; worker submits once. Needs stream API shape (iterator / `plan_next` / static sequence) **without** raw `EffectRequest`. Design-gate before TCB. |
| **Deterministic multi-step coding planner** | Partner needs a **reproducible** coding path before trusting LLM multi-step; evals-first discipline | **MEDIUM** | MUST | Scripted/hardcoded sequence over **shipped** sinks: e.g. `file.write` → `process.exec` (tests) → `git.commit` → `git.push` → `github.pr`. Not an LLM tool-use loop. New `CaprunIntent` variant (or equivalent) mapping to N nodes. |
| **CLI-driven multi-node coding session** | “I run one command and the chain runs under the runtime” | **MEDIUM** | MUST | `caprun run` (or sibling verb) drives the multi-node chain end-to-end. Preserve existing `--policy`, Block → `review`/`confirm`/`deny` surfaces. Closes “only email/file intents exist.” |
| **One-session continuity** | One policy bind, one audit DAG, one `verify_chain` for the whole path | **MEDIUM** | MUST | Single Session; policy bound once at creation (POLICY-03); every node’s decisions/events on the same chain; no multi-process “stitch later” proof as the DONE claim. |
| **Mid-loop confirm/deny still works** | Push always confirm-gated; PR auth-grant; I2 Blocks mid-chain | **LOW–MEDIUM** | MUST | Existing confirm/grant machinery; CLI multi-node driver must **pause and surface** `BlockedPendingConfirmation` / grant needs without abandoning Session or laundering state. |
| **Non-hybrid LIVE proof** | v1.9 DONE was hybrid by design; partners need CLI honesty | **HIGH** | MUST | Success path + mid-loop I2 Block (tainted PR body and/or push refspec) driven through **real CLI**, real Linux, `verify_chain` true — no in-crate composition as the primary claim. |
| **Minimal packaging / install path** | Partner must install on Linux without reverse-engineering the repo | **LOW–MEDIUM** | MUST | Single binary (or documented `cargo install`/`release` artifact) + env/credential checklist (`CAPRUN_*`, GitHub token grant, policy file). Linux-only remains OK. |
| **Honest failure / stop semantics** | Multi-step must not silently continue past Block/Deny | **MEDIUM** | MUST | Define: on I2 Block → stop stream, print `effect_id` + review pointer; on policy_deny → distinct code; on Allowed → continue. Exit codes must distinguish success vs blocked vs denied (worker already learned this for single-node). |

### Differentiators (caprun’s competitive edge — keep front-and-center)

Not “more agent features.” These are why a security/platform buyer chooses an Intent Runtime over Aider/Claude Code with allowlists.

| Feature | Value Proposition | Complexity | Category | Notes |
|---------|-------------------|------------|----------|-------|
| **I2 holds mid multi-node stream** | Tainted value cannot occupy a sensitive sink arg mid-loop without literal confirm — even if “the plan” continues | Already in TCB; **prove** multi-node | MUST (proof) | Differentiator is **enforcement + genuine taint**, not planner smarts. LIVE proof must use non-stapled provenance (`provenance_chain[0]` = real read/exec event). |
| **Policy narrows sinks; never disables I2** | Policy-permitted sink can still I2-Block; `policy_deny` is a distinct decision code | Shipped; preserve | MUST (preserve) | Multi-node must not introduce a “session waiver” or planner-side bypass. |
| **Kernel confinement of the worker** | No ambient net/exec; only broker-mediated effects | Shipped | MUST (preserve) | Multi-step must not re-open ambient authority (no worker-side `execve` of git/curl). |
| **Authenticated audit DAG for the whole coding path** | One chain proves edit→test→commit→push→PR decisions | MEDIUM (composition) | MUST | Partner demos: “show me why the PR body was blocked” via `caprun audit`. |
| **Planner never sees literals/taint** | Handles-only planner (PLAN-03); executor decides | Preserve under stream API | MUST | Multi-node planner still only places `ValueId`s. No laundering via multi-step “summary” args. |
| **Deterministic first, LLM later** | Reproducible partner demos + no eval debt this milestone | Strategic | SHOULD (positioning) | Opposite of Claude Code/Aider default; correct for security productization. |

### Anti-Features (explicitly do NOT build in v1.10)

| Anti-Feature | Why Requested | Why Problematic | What to Do Instead |
|--------------|---------------|-----------------|--------------------|
| **LLM multi-step tool-use / ReAct loop** | “Real agent”; matches Claude Code | Planner regenerates args → taint laundering risk; needs evals; expands TCB/sidecar surface; project manual-ops-first | Deterministic multi-node stream now; LLM multi-step **deferred** (v1.4 single-shot adversarial seam remains) |
| **Becoming an agent framework** (memory, skills marketplace, multi-agent teams, MCP host) | Feature parity with Claude Code | Violates DEC-product-boundary; dilutes security differentiator | Stay Intent Runtime; expose sinks + plan stream + audit |
| **Raw `EffectRequest` / free-form tool map** | Faster wiring of multi-step | Bypasses executor stand-point; Gate 1 fails; kills I2 story | Only `PlanNode { sink, args: Vec<ValueNode/ValueId> }` |
| **Session-wide confirm waiver / “YOLO mode”** | Reduce friction on push/PR | Undermines single-shot confirm + I2; attacker goal | Keep per-(sink,arg,literal-digest) confirm; always-confirm `git.push` |
| **New sink families** (browser, desktop, arbitrary cloud, merge/comment breadth) | Broader demos | Each sink is full I2/design-gate work; not needed for coding loop | Reuse shipped sinks only |
| **Cedar / rich policy language** | Enterprise policy story | Premature; v1.9 hardcoded-schema policy sufficient for partner | Keep minimal policy file; Cedar later if pulled |
| **Web UI / chat transcript as trust surface** | Familiar UX | Trust surface is audit DAG; UI is productization distraction | CLI + `caprun audit` |
| **Cross-host / Biscuit / gVisor / Firecracker** | “Harder sandbox” | Out of scope; Landlock+seccomp is the v0–v1.x boundary | Document Linux≥5.13 requirement |
| **Auto-retry / self-heal from tainted exec output into irreversible sinks** | Agent “fixes itself” | Classic I1/I2 failure mode (tainted stdout → commit message / PR body) | Fail closed; human confirm or trusted re-mint only |
| **In-crate hybrid as DONE claim** | Easier test | Already shipped as v1.9 honesty; claiming it again is false assurance | Non-hybrid CLI LIVE is the v1.10 DONE bar |
| **Lifting git.push 10MB pack-cap** | Large repos | Non-blocking functional deferral; not multi-step work | Carry forward unless partner blocks |
| **Mac/WSL security claims** | Broader install base | Security claims Linux-only | Linux design-partner path only |

---

## Feature Dependencies

```
Shipped sinks (exec, fs, git.*, github.pr, http.*) ──already──> Multi-node coding planner
Shipped policy bind + confirm/deny/grant + audit ──already──> CLI multi-node driver

Multi-node plan stream (Planner seam / worker loop)
    └──requires──> Session can accept N× SubmitPlanNode (already true at broker)
    └──requires──> Stream-shaped Planner API (new; design-gate)
    └──requires──> Worker/orchestrator loop: submit → decide → (confirm?) → next

Deterministic multi-step coding planner
    └──requires──> Multi-node plan stream
    └──requires──> New CaprunIntent (or plan recipe) for coding workflow
    └──requires──> Trusted handles for irreversible args (remote/refspec, PR metadata)
    └──enhances──> LIVE non-hybrid proof

CLI-driven multi-node coding session
    └──requires──> Deterministic multi-step planner
    └──requires──> Pause/resume on BlockedPendingConfirmation + grant
    └──requires──> One Session lifecycle in `caprun run` path

One-session continuity + verify_chain
    └──requires──> CLI multi-node driver (same session_id, same audit.db, one policy_bound)

Non-hybrid LIVE proof
    └──requires──> All of the above
    └──requires──> Mid-loop adversarial taint leg (I2 Block under policy-PERMIT)

Minimal packaging
    └──independent of planner (can parallelize)
    └──enhances──> Design-partner uptake of LIVE path

LLM multi-step tool-use ──conflicts──> v1.10 manual-ops-first / evals-not-done
Raw EffectRequest ──conflicts──> I2 / Gate 1 / DEC-architectural-lock-plan-nodes
Session-wide waiver ──conflicts──> single-shot confirm + POLICY-02
```

### Dependency Notes

- **Broker already accepts multiple `SubmitPlanNode`s per Session.** The gap is **planner + worker + CLI composition**, not a new broker effect path.
- **`output_value_id` from Allowed `process.exec`** is already plumbed for a later node to consume (worker comment) — multi-step must define **whether and how** exec output handles may enter later args (default: only into non-sensitive / Observe slots unless confirmed; never silent launder into PR body).
- **`git.push` always confirm-gated** and **`github.pr` needs grant** — multi-node CLI must integrate human steps without spawning a second Session.
- **Packaging does not depend on multi-node** and can ship early for partner dry-runs of existing single-node + audit.

---

## Expected design-partner behavior (edit→test→commit→push→PR under policy+I2)

Happy path (SUCCESS):

1. Partner installs binary on Linux ≥5.13; sets policy file + broker env credentials (GitHub token **not** in worker).
2. `caprun run <coding-intent> <workspace> --policy <trusted-policy>` creates **one** Session, binds policy once (`policy_bound` in DAG).
3. Deterministic planner emits nodes in order; confined worker submits each via broker.
4. Exec/tests and fs edits run under kernel confinement; outputs mint with genuine taint where untrusted.
5. `git.commit` Allowed (MutateReversible) when args clean.
6. `git.push` hits **always-confirm** gate → partner runs `caprun review` / `caprun confirm` → exactly one push.
7. `github.pr` requires prior `caprun grant` (session-scoped) → PR opens (or mock in CI).
8. `caprun audit <session>` → `Chain verification: PASSED` across the full path.

Hostile / mid-loop Block path (must work in same CLI story):

1. Same multi-node intent, but a node routes a **genuinely tainted** value into a sensitive arg (PR body/title or push remote/refspec).
2. Executor **I2-Blocks** even though policy **permits** that sink.
3. Stream **stops** (or refuses that node without proceeding to later irreversible effects).
4. Partner sees `effect_id` + review pointer; deny → nothing external; confirm only releases the exact triple.
5. Audit shows unbroken edge: mint/read/exec event → ValueNode → `sink_blocked` (not stapled).

Non-negotiable partner-facing rules:

- Policy cannot “allow past” I2.
- No ambient network from the worker.
- Credentials never appear in ValueStore/audit literals.
- Hybrid in-crate composition is **not** how success is claimed.

---

## MVP Definition (v1.10)

### Launch With (v1.10 DONE)

- [ ] **Multi-node plan stream** on existing Planner seam — essential for any multi-step claim
- [ ] **Deterministic multi-step coding planner** — one concrete edit→test→commit→push→PR recipe over shipped sinks
- [ ] **CLI multi-node driver** — `caprun run` (or equivalent) drives that recipe in one Session
- [ ] **Confirm/grant integration mid-stream** — push + PR human gates without session abandon
- [ ] **One-session `verify_chain` continuity** — policy bound once; full DAG
- [ ] **Non-hybrid LIVE proof** — CLI success + mid-loop I2 Block on real Linux
- [ ] **Minimal packaging/install docs** — binary + env/credentials + policy example

### Add After Validation (post–v1.10, partner-pulled)

- [ ] **LLM multi-step tool-use** — only after eval set + design-gate; reuse v1.4 sidecar capability split
- [ ] **Additional coding recipes** (lint-only, fix-from-test-log with constrained taint rules)
- [ ] **Richer pause UX** (non-interactive confirm batching still single-shot semantics)
- [ ] **git.push pack-cap lift** — if large-repo partner blocks
- [ ] **Declarative plan file** (trusted ops-authored YAML of nodes) — only if hardcoded recipe is too rigid; still not Cedar

### Future Consideration (explicitly not v1.10)

- [ ] Cedar / general policy language
- [ ] Web UI, marketplace, long-term memory, multi-agent
- [ ] New sink families; github merge/comment breadth
- [ ] Cross-host Sessions / Biscuit; gVisor/Firecracker
- [ ] Mac/WSL security claims

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | Milestone |
|---------|------------|---------------------|----------|-----------|
| Multi-node plan stream (Planner/worker) | HIGH | HIGH | **P1** | v1.10 |
| Deterministic coding multi-node planner | HIGH | MEDIUM | **P1** | v1.10 |
| CLI multi-node `caprun run` coding intent | HIGH | MEDIUM | **P1** | v1.10 |
| Mid-stream confirm/grant/stop semantics | HIGH | MEDIUM | **P1** | v1.10 |
| One-session continuity + verify_chain | HIGH | LOW–MEDIUM | **P1** | v1.10 |
| Non-hybrid LIVE success + I2 Block | HIGH | HIGH | **P1** | v1.10 (DONE gate) |
| Minimal packaging / install path | HIGH | LOW–MEDIUM | **P1** | v1.10 |
| Design-gate + adversarial trace for multi-step TCB | HIGH | MEDIUM | **P1** | v1.10 open |
| Exec-output → later-node handle routing rules | MEDIUM | MEDIUM | **P2** | v1.10 if needed for recipe; else constrain recipe |
| Second coding recipe / plan file format | MEDIUM | MEDIUM | **P3** | post-partner |
| LLM multi-step tool-use loop | HIGH (later) | HIGH | **P3** | future |
| Cedar / web UI / new sinks | LOW now | HIGH | **P3** | out of scope |

**Priority key:** P1 = must for v1.10 launch; P2 = should if recipe needs it; P3 = defer.

---

## Competitor / Peer Feature Analysis

| Feature | Claude Code | Aider | SWE-agent / mini-SWE-agent | **caprun (v1.10 target)** |
|---------|-------------|-------|---------------------------|---------------------------|
| Multi-step edit/test loop | Yes (agentic tool loop) | Yes (chat + lint/test cmds) | Yes (issue→fix trajectories) | **Yes — deterministic multi-node stream** |
| Commit / PR | Yes | Auto-commit; PR via git | Often via bash/git | **Yes — mediated `git.*` / `github.pr` sinks** |
| Human permission gates | Yes (allow/deny/ask, managed policy) | Undo; less structural | Varies / research-oriented | **Yes — confirm + grant + I2 (harder than policy alone)** |
| Sandbox | OS/permission layers; not Landlock+I2 | Mostly host trust | Container/bash isolation | **Kernel confinement + default-deny net** |
| Value-injection defense | Prompt/permission soft | Minimal | Minimal | **★ I2 deterministic TCB + genuine taint DAG** |
| Audit proof | Transcript/logs | Git history | Trajectories | **★ Authenticated audit DAG `verify_chain`** |
| Policy vs enforcement | Permissions can be broad; auto mode | Soft | Soft | **Policy never disables I2 (POLICY-02)** |
| Install path | First-class multi-platform | pip/CLI | pip/research | **Minimal Linux packaging (v1.10)** |
| LLM multi-step | Default | Default | Default | **Deferred — security product chooses deterministic first** |

**Reading:** peers compete on agent intelligence and UX breadth. caprun competes on **structural inability** to exfiltrate/push unreviewed under policy+I2, with a **CLI-honest multi-step proof**. Matching Claude Code feature breadth is an anti-goal.

---

## Complexity & risk callouts (for requirements scoping)

| Area | Complexity | Risk if under-scoped |
|------|------------|----------------------|
| Planner trait shape (`PlanNode` → stream) | HIGH | Half-stream APIs that still force hybrid tests |
| Taint across node boundaries (exec out → later args) | HIGH | Silent laundering; LIVE Block proves nothing |
| CLI pause on confirm mid-stream | MEDIUM | Second Session / lost policy bind / abandoned DAG |
| Non-hybrid LIVE harness | HIGH | Re-introduces hybrid framing as “good enough” |
| Packaging | LOW–MEDIUM | Partner cannot run; security story stays internal-only |

**Research flags for later phases:**

- Phase that touches Planner/worker multi-submit: **needs design-gate + fresh adversarial code-trace** (multi-step TCB).
- Phase that routes `output_value_id` into later sinks: **needs explicit taint rules** (likely design-gate addendum).
- Packaging phase: standard patterns; low research need.
- LLM multi-step: **separate milestone**; evals first.

---

## Sources

### Caprun-internal (HIGH confidence)

- `.planning/PROJECT.md` — v1.10 milestone goal, active requirements, locked decisions (I0/I1/I2, plan-node lock, product boundary)
- `planning-docs/CANDIDATE-v1.7plus-productization-sketch.md` — Safe Coding Agent anchor; multi-step planner called out as P1; packaging D1
- `.planning/milestones/v1.9-phases/46-composed-live-proof-v1-9-done/46-MILESTONE-RECORD.md` — hybrid LIVE-05 framing honesty; multi-node planner out of v1.9 scope
- `cli/caprun/src/planner.rs` — single-node `Planner::plan → PlanNode`; PLAN-03 handles-only
- `cli/caprun/src/worker.rs` — one-shot plan + submit; `output_value_id` reserved for later routing
- `crates/runtime-core/src/intent.rs` — only `SendEmailSummary` / `CreateFileFromReport` intents today

### Ecosystem / official docs (MEDIUM confidence; multi-source cross-check)

- Anthropic tool-use overview + “How tool use works” — agentic client loop (`stop_reason == tool_use`)  
  https://platform.claude.com/docs/en/docs/agents-and-tools/tool-use/overview  
  https://platform.claude.com/docs/en/docs/agents-and-tools/tool-use/how-tool-use-works
- Claude Code overview + settings/permissions model — multi-step coding, git/PR, managed allow/deny  
  https://code.claude.com/docs/en/overview  
  https://code.claude.com/docs/en/settings
- Aider usage, git integration, lint/test loops — edit/commit/test as table stakes  
  https://aider.chat/docs/usage.html  
  https://aider.chat/docs/git.html  
  https://aider.chat/docs/usage/lint-test.html
- SWE-agent / mini-SWE-agent — multi-step issue-fix loops; mini’s linear independent-action simplicity  
  https://github.com/SWE-agent/SWE-agent  
  https://mini-swe-agent.com/latest/faq/

### Confidence summary

| Area | Level | Reason |
|------|-------|--------|
| Table stakes for v1.10 | **HIGH** | Directly derived from PROJECT.md + v1.9 hybrid gap |
| Differentiators | **HIGH** | Locked I0/I1/I2 + shipped policy/audit substrate |
| Anti-features | **HIGH** | DEC-product-boundary + milestone out-of-scope list |
| Ecosystem loop patterns | **MEDIUM** | Official docs via WebFetch; Brave search unavailable; patterns stable and multi-sourced |
| Exact Planner stream API shape | **LOW→phase research** | Not prescribed here; design-gate owns it |

---

*Feature research for: caprun v1.10 Multi-step Safe Coding Agent Loop*
*Researched: 2026-07-23*
*Mode: ecosystem features (table stakes / differentiators / anti-features) for requirements scoping*
