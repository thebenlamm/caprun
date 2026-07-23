# Project Research Summary

**Project:** caprun (Intent Runtime) — v1.10 Multi-step Safe Coding Agent Loop  
**Domain:** Capability-secure multi-step plan orchestration on a shipped Rust Intent Runtime (not an agent framework)  
**Researched:** 2026-07-23  
**Confidence:** HIGH

## Executive Summary

caprun is a **security-first Intent Runtime** on stock Linux: kernel-confined workers have no ambient authority; every external effect is a broker-mediated `PlanNode` evaluated by a hardcoded Rust I2 executor; policy can only narrow sinks, never disable I2. Through v1.9 the Safe Coding Agent surface (edit → test → commit → push → open PR) is fully shipped as real sinks + policy + confirm/audit CLI — but the composed multi-node path is still a **hybrid in-crate LIVE-05**, and `caprun run` only drives single-node email/file intents. **v1.10 closes that product gap:** one Session, CLI-driven multi-node stream, genuine audit chain, non-hybrid LIVE proof.

Experts do not build this as LangChain/Temporal/ReAct. Industry coding agents use agentic tool loops with soft permissions; caprun’s structural difference is plan nodes + genuine taint + Landlock/seccomp. Research is unanimous: **zero new crates**, extend the existing `Planner` seam + worker sequential `SubmitPlanNode` loop + a deterministic coding planner over shipped sinks, with **Block-and-Hold** mid-loop confirm (same Session, no reconnect-remint). Design-gate first (`DESIGN-multi-step-plan-stream.md` + fresh adversarial code-trace), then types → planner → worker loop → CLI → confirm continuity → LIVE → packaging.

**Primary risks:** cross-node taint laundering via `output_value_id`, ProvideIntent reopened as a mid-stream trust valve, Draft demotion “fixed” by weakening CommitIrreversible rules, hybrid composition rebranded as CLI multi-step, and mid-loop confirm splitting Sessions. Mitigation is locked substrate discipline: ProvideIntent once, handle-only planner (PLAN-03), per-node I2, single-shot confirm, trusted-intent-only success path (no multi-file RequestFd demotion before irreversible sinks), and a DONE bar that requires real `caprun run` multi-node on Linux compose-verify — never hybrid as the claim.

## Key Findings

### Recommended Stack

**v1.10 needs zero new crates.** Multi-node streams, deterministic coding planner, session-loop orchestration, and design-partner packaging are pure in-tree Rust over the v1.9 substrate. Anything that looks like an agent framework, workflow engine, new IPC transport, clap, Cedar, libgit2, cargo-dist, or aws-lc-rs is out of scope and actively harmful under HYG-01 / product boundary.

**Core technologies (keep — no version bumps required):**
- **Rust edition 2021 / workspace resolver 3** — TCB language; Python non-TCB only
- **tokio + serde_json + framed UDS** — broker accept loop + existing `SubmitPlanNode` wire (already multi-call safe)
- **rusqlite + sha2/hmac** — session continuity + authenticated audit DAG (`verify_chain`)
- **landlock + seccompiler + nix** — kernel boundary; multi-node must not re-open ambient net/exec
- **reqwest (rustls-no-provider) + ring + webpki-roots** — broker-side git.push / github.pr / http.* (reuse; Gate 5 ring-only)
- **Existing `Planner` trait + `output_value_id`** — stream return + handle bag; no new packages

**Code-only delta:** `CaprunIntent` coding variant, `Planner` multi-node surface (`plan_next` / `Vec<PlanNode>`), `DeterministicCodingPlanner`, worker multi-submit + handle bag, CLI intent-kind, thin `scripts/install-linux.sh` (three co-located bins — `cargo install` alone breaks `caprun-exec-launcher` sibling layout).

Details: [STACK.md](./STACK.md)

### Expected Features

**Must have (table stakes for design-partner slice):**
- **Multi-node plan stream** — N× `SubmitPlanNode` in one Session (broker already allows this; gap is planner/worker/CLI)
- **Deterministic multi-step coding planner** — hardcoded edit→test→commit→push→PR over shipped sinks
- **CLI-driven multi-node coding session** — `caprun run` (or equivalent) drives the chain
- **One-session continuity** — one policy bind, one audit DAG, `verify_chain` true end-to-end
- **Mid-loop confirm/deny/grant** — push always-confirm; PR auth-grant; stream pauses without abandoning Session
- **Honest stop semantics** — Block/Deny do not silently continue; distinct exit outcomes
- **Non-hybrid LIVE proof** — success + mid-loop I2 Block via real CLI on Linux
- **Minimal packaging** — co-located release bins + env/credential checklist

**Should have (differentiators — prove, don’t invent):**
- I2 holds mid multi-node stream with genuine (non-stapled) taint provenance
- Policy narrows sinks; never disables I2 (`policy_deny` ≠ `sink_blocked`)
- Kernel confinement preserved; planner handles-only (PLAN-03)
- Deterministic-first positioning (LLM multi-step deferred)

**Defer (post-v1.10 / out of scope):**
- LLM multi-step / ReAct tool-use loop
- Agent frameworks, memory, marketplace, multi-agent, MCP host
- Cedar / rich policy language, web UI, new sink families
- Session-wide confirm waiver / YOLO mode
- git.push pack-cap lift, cross-host/Biscuit, gVisor/Firecracker, Mac security claims
- Raw `EffectRequest` / free-form tool maps

Details: [FEATURES.md](./FEATURES.md)

### Architecture Approach

Keep multi-step **inside the confined worker + planner seam**, not a new orchestration crate and not unconfined orchestrator-submitted effects. Recommended pattern: **sequential plan stream** (one connection, N independent I2 evaluations) + **handle bag** for `output_value_id` + **Block-and-Hold** for mid-loop `BlockedPendingConfirmation` (worker stays connected; human confirms durable snapshot; worker does not re-submit the blocked node). Success path = **trusted-intent-driven** plan nodes only (operator-typed paths/commands/messages via ProvideIntent) so HARDEN-01 multi-file RequestFd demotion does not kill CommitIrreversible sinks mid-loop. Mid-loop I2 Block path routes a genuinely tainted handle (e.g. exec output) into a sensitive arg under a policy-permitted sink.

**Major components:**
1. **DESIGN gate** — `DESIGN-multi-step-plan-stream.md` locks stream shape, confirm continuity, I1×Draft, instruction vs value channels
2. **runtime-core** — closed `CaprunIntent` coding variant (+ optional stream context types)
3. **Planner seam** — additive multi-node API; `DeterministicCodingPlanner`; keep email/file single-node green
4. **Worker** — sequential submit loop, handle bag, fail-closed on Deny/policy_deny, hold-for-confirm
5. **CLI main** — coding intent kind, mid-loop confirm UX, policy bind once at session create
6. **Broker / executor / sandbox** — mostly unchanged; no batch authorize, no I2 stream waiver
7. **LIVE + packaging** — non-hybrid compose-verify proof; install script (non-TCB)

Details: [ARCHITECTURE.md](./ARCHITECTURE.md)

### Critical Pitfalls

1. **Cross-node taint laundering** — treat `output_value_id` / exec stdout as trusted for PR body/refspec. **Avoid:** opaque ValueIds only; no mid-stream ProvideIntent; LIVE negative with genuine provenance root.
2. **ProvideIntent as multi-step trust valve** — re-open trusted mint after observations. **Avoid:** ProvideIntent exactly-once before RequestFd; all coding trusted args at session start from CLI/intent.
3. **Draft demotion “fixed” by weakening I1** — auto-allow CommitIrreversible after green tests. **Avoid:** keep Step 0.5; success path avoids demotion before irreversible nodes; confirm is not a class waiver.
4. **Hybrid sold as CLI multi-step** — LIVE-05 rebranded. **Avoid:** DONE requires real multi-node `caprun run`; hybrid unit harness only, never DONE evidence.
5. **Mid-loop confirm splits Session / standing waiver** — new session for PR tail, or confirm-all. **Avoid:** Block-and-Hold same Session; single-shot per effect_id; subsequent nodes still full I2.

Also continuous: Gate 3 mint discipline, chain-head parenting, BiDi neutralization on new surfaces, cfg-linux blindness (compose-verify is authoritative).

Details: [PITFALLS.md](./PITFALLS.md)

## Implications for Roadmap

Based on combined research, suggested phase structure (order is load-bearing; numbers for roadmapper):

### Phase 1: Design Gate — Multi-step Plan Stream
**Rationale:** Every prior TCB milestone (v1.2–v1.9) hard-blocked code until a DESIGN doc + fresh non-self adversarial code-trace cleared. Multi-step reopens I0/I1/I2 composition (demotion × irreversible tail, ProvideIntent, confirm resume, instruction vs value).
**Delivers:** `planning-docs/DESIGN-multi-step-plan-stream.md` APPROVED; adversarial trace record; locked decisions on stream shape, handle bag, Block-and-Hold, trusted-intent success path, fail-closed abort-on-deny.
**Addresses:** Design-gate prerequisite for all table-stakes features; anti-features explicit (no EffectRequest, no LLM multi-step, no session waiver).
**Avoids:** P2 ProvideIntent laundering, P3 Draft weaken, P5 planner seam collapse, P8 policy erosion, P13 agent-framework creep, P14 standing confirm.

### Phase 2: Plan-Stream Substrate (types + worker loop + handle bag)
**Rationale:** Broker already accepts N× SubmitPlanNode; gap is runtime-core types + worker sequential loop + chain/handle discipline before a coding recipe piles nodes on.
**Delivers:** Coding-ready intent types if needed for compile; worker multi-submit; `output_value_id` consumption rules; verify_chain across N nodes; no new mint sites (Gate 3).
**Addresses:** Multi-node plan stream; one-session continuity foundation.
**Avoids:** P1 cross-node taint, P9 Gate 3 drift, P11 DAG fork, P15 blind retry of irreversible sinks.
**Uses:** Existing tokio/UDS/serde IPC; std HashMap handle bag; no Cargo.toml deps.

### Phase 3: Deterministic Multi-step Coding Planner
**Rationale:** Once stream seam is locked, a pure scripted planner is low-novelty and testable without full CLI productization.
**Delivers:** `DeterministicCodingPlanner` (or equivalent) mapping coding intent → ordered handle-only PlanNodes over `file.write` → `process.exec` → `git.commit` → `git.push` → `github.pr`; email/file single-node paths remain green.
**Addresses:** Deterministic multi-step coding planner feature.
**Avoids:** P5 control/data collapse; LLM tool-use (out of milestone).
**Implements:** Architecture Pattern 1 (static stream / plan_next index).

### Phase 4: CLI Multi-node Driver
**Rationale:** DONE forbids hybrid; a real binary path must exist before LIVE can claim CLI honesty.
**Delivers:** `caprun run` coding intent-kind; policy bind once; surfaces effect_id + review/confirm on Block; framing machine-check ready.
**Addresses:** CLI-driven multi-node coding session; honest stop semantics.
**Avoids:** P4 hybrid honesty failure; P2 intent binding mistakes; P8 mid-run policy rebind; P10 BiDi on new prints.

### Phase 5: Mid-loop Confirm Continuity
**Rationale:** SUCCESS path includes always-confirm `git.push` and grant-gated `github.pr`; confirm was designed for one effect, not a stream scheduler — this is the hard integration.
**Delivers:** Block-and-Hold (or design-gate-locked equivalent); same Session resume after confirm; terminal audit events for every released effect; no re-submit of blocked node.
**Addresses:** Mid-loop confirm/deny/grant; one-session continuity under human gates.
**Avoids:** P6 audit gap, P7 session split, P14 standing waiver, P10 BiDi on resume UX.

### Phase 6: Non-hybrid LIVE Proof (v1.10 DONE gate)
**Rationale:** Composition re-proves; does not assume (v1.3 Phase 17 / v1.9 LIVE pattern). Authoritative signal is full Linux compose-verify (mailpit-verify only if SMTP involved).
**Delivers:** CLI success path full chain + mid-loop I2 Block (tainted PR body and/or push refspec) with genuine taint chain; `verify_chain` true; honest milestone record (no hybrid DONE claim); independent orchestrator re-run.
**Addresses:** Non-hybrid LIVE proof; I2 mid-stream differentiator proof.
**Avoids:** P4 framing lie, P12 cfg-linux blindness, P1/P8 negative vacuity.

### Phase 7: Minimal Linux Packaging
**Rationale:** Independent of TCB; can draft early but should not gate security claims; partners need co-located bins.
**Delivers:** `scripts/install-linux.sh` (or equivalent) + env/credentials/policy checklist; three sibling bins documented.
**Addresses:** Minimal packaging / install path.
**Avoids:** cargo-dist / Docker-as-product scope creep; broken `current_exe().parent()` installs.

### Phase Ordering Rationale

- **Design gate first** — multi-step reopens trust composition questions that must not be invented mid-code (v1.2 B1 lesson).
- **Substrate before planner** — stream/session/chain must exist before a coding script piles nodes on.
- **Deterministic planner before CLI** — pure sequence logic testable without full product verb.
- **CLI before LIVE** — DONE forbids hybrid; CLI must exist to be proven.
- **Confirm continuity before final LIVE** — SUCCESS includes git.push always-confirm.
- **LIVE last** — composition re-proves A–E; packaging is ops polish, parallelizable after gate.

### Research Flags

Phases likely needing deeper research during planning (`/gsd-plan-phase --research` or design-gate depth):
- **Phase 1 (Design gate):** resume semantics, demotion×coding loop, what may be UserTrusted at node 0 vs later, exact Block-and-Hold IPC (stdout vs side channel vs broker verb), residual plan after deny (recommend abort).
- **Phase 2 (Substrate):** ValueStore lifetime across nodes; planner reduced signal if dual-connection; chain locking under multi-event append.
- **Phase 5 (Confirm continuity):** durable stream resume while confirm is a separate process/snapshot model — **new product surface**, not just wiring.

Phases with standard patterns (skip deep research-phase once gate locked):
- **Phase 3 (Deterministic planner):** scripted templates over known sinks.
- **Phase 4 (CLI):** hand-rolled argv extension of existing `caprun run`.
- **Phase 6 (LIVE):** standing compose-verify / framing-honesty / independent re-run discipline.
- **Phase 7 (Packaging):** shell copy + docs; Cargo Book multi-package install caveat already researched.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | **HIGH** | In-repo Cargo pins + live binary layout + HYG-01 Gate 5 verified; packaging path cross-checked against Cargo Book |
| Features | **HIGH** | Table stakes derived from PROJECT.md + v1.9 hybrid gap; ecosystem norms MEDIUM but filtered by product boundary |
| Architecture | **HIGH** | Live crates + locked DESIGN docs; broker multi-SubmitPlanNode and unused `output_value_id` code-verified |
| Pitfalls | **HIGH** | Project retros + design gates + recurring defect classes (audit gap, cfg-linux, framing honesty); external agent lit secondary |

**Overall confidence:** **HIGH**

### Gaps to Address

These are intentionally left to the design gate / phase planning — not blockers for roadmap structure:

- **Exact worker↔main “blocked, waiting” signal** (stdout convention vs side UDS vs broker verb) — Phase 1 must lock.
- **Interactive in-process confirm in `caprun run` vs dual-terminal `caprun confirm` only** — UX product call inside design gate; single-shot semantics non-negotiable.
- **How many workspace files success path may RequestFd without demotion** — default: seed only / none; multi-file untrusted read + still push is a **separate** future design-gate.
- **Exec-output → later-node routing rules** — constrain recipe so irreversible args stay UserTrusted; tainted handles only for negative LIVE leg unless confirm.
- **Exact CaprunIntent field set / naming** — product naming only; closed enum structure is locked.
- **Residual plan after mid-loop deny** — research recommends abort fail-closed; confirm in design gate.
- **Whether `file.write`/`process.exec` CommitIrreversible class is re-litigated** — default **no** for v1.10 (too easy to weaken I1).

## Sources

### Primary (HIGH confidence)

- `.planning/PROJECT.md` — v1.10 goal, hybrid LIVE-05 honesty, locked I0/I1/I2, active requirements
- `planning-docs/PLAN.md` — architectural lock (plan nodes, layer roles, §9 genuine taint)
- `planning-docs/DESIGN-plan-executor.md`, `DESIGN-taint-model.md`, `DESIGN-session-trust-coherence.md`, `DESIGN-confirmation-release.md`, `DESIGN-security-hardening.md`, `DESIGN-v1.9-egress-policy.md`
- `planning-docs/CANDIDATE-v1.7plus-productization-sketch.md` — Safe Coding Agent anchor; multi-step P1; packaging D1
- `.planning/milestones/v1.9-phases/46-composed-live-proof-v1-9-done/46-MILESTONE-RECORD.md` — hybrid framing
- In-repo: `cli/caprun/src/planner.rs`, `worker.rs`, `crates/brokerd/src/server.rs` / `proto.rs`, `crates/runtime-core/src/intent.rs`, `scripts/check-invariants.sh`
- Workspace Cargo.toml pins (verified 2026-07-23)
- [Cargo Book — cargo-install](https://doc.rust-lang.org/cargo/commands/cargo-install.html) — multi-package sibling bin install mechanics

### Secondary (MEDIUM confidence)

- Anthropic tool-use / Claude Code docs — agentic client loop shape (caprun deliberately diverges)
- Aider / SWE-agent / mini-SWE-agent docs — edit/test/commit table stakes, not authority model
- CaMeL (arXiv:2503.18813) — control/data separation supporting planner handle-only discipline
- CVE-2021-42574 Trojan Source — BiDi confirm-surface class (fixed v1.9; reopens on new multi-step UX)

### Tertiary (LOW confidence)

- Exact Planner stream API shape (`plan_next` vs `Vec` return) — design-gate owns final signature
- Exact Block-and-Hold wire protocol — recommended pattern, not shipped

---
*Research completed: 2026-07-23*  
*Ready for roadmap: yes*  
*Mode: synthesis of STACK + FEATURES + ARCHITECTURE + PITFALLS for caprun v1.10*
