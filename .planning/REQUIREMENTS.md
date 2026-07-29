# Requirements: caprun (AgentOS) — v1.10 Multi-step Safe Coding Agent Loop

**Defined:** 2026-07-23
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2). v1.10 makes the Safe Coding Agent path (edit → test → commit → push → open PR) a **single Session, CLI-driven multi-node stream** — closing the v1.9 hybrid LIVE-05 honesty gap — without weakening I0/I1/I2, without a raw `EffectRequest` path, and without becoming an agent framework.

**Research basis:** `.planning/research/SUMMARY.md` (2026-07-23) — zero new crates; sequential plan stream on existing Planner seam; deterministic multi-step first; Block-and-Hold mid-loop confirm; non-hybrid LIVE DONE bar.

## v1 Requirements

Requirements for the v1.10 milestone. Each maps to exactly one roadmap phase (see Traceability). REQ-IDs are milestone-local; phase numbers continue from v1.9 (last phase 46 → start at 47).

### Design Gate (blocks all multi-step TCB code)

- [x] **DESIGN-19**: A single DESIGN doc (`planning-docs/DESIGN-multi-step-plan-stream.md`) pins the TCB mechanisms for multi-step orchestration: (a) **plan-stream shape** on the existing `Planner` seam (additive multi-node API — static sequence and/or `plan_next` — **not** batch DAG authorize and **not** `EffectRequest`); (b) **worker sequential submit loop** + handle bag for `output_value_id` (opaque ValueIds only; planner never mints); (c) **mid-loop Block-and-Hold confirm continuity** — same Session, same policy bind, same audit chain; no reconnect-and-remint resume; no session-wide confirm waiver; (d) **I1×coding-loop bounds** — success path is trusted-intent-driven (operator-typed args via ProvideIntent once at session start); multi-file RequestFd demotion must not be "fixed" by weakening CommitIrreversible Draft denies; (e) **instruction vs value channels** remain disjoint under multi-node (PLAN-03 handles-only); (f) **deny/abort semantics** mid-stream (recommended: abort remaining nodes, durable terminal events). Carries forward ProvideIntent-once, Gate 3 mint-site discipline, P33/P34 precheck-before-burn, POLICY-02 non-bypass of I2.

- [x] **DESIGN-20**: The DESIGN doc clears a fresh, non-self, orchestrator-owned adversarial code-trace (NOT a gsd-executor) before any multi-step TCB change in `crates/{executor,brokerd,sandbox,runtime-core}` or the worker submit/confirm-hold path in `cli/caprun`. Unbroken precedent through v1.9 P41. The trace **re-runs if stream shape, confirm-hold, or trusted-arg mint path changes mid-implementation**.

### Plan-Stream Substrate

- [x] **STREAM-01**: In one Session, on one worker connection, the runtime can evaluate and submit **N sequential plan nodes** (`SubmitPlanNode` × N). Each node is independently I2-evaluated; policy remains the pre-I2 narrowing gate; no batch-authorize shortcut. Broker multi-submit is already legal — this requirement is the **worker loop + chain-head continuity** so every decision/event lands on the same audit DAG with `verify_chain` true for the Session.

- [x] **STREAM-02**: Intermediate sink outputs exposed as `output_value_id` (e.g. `mint_from_exec`) are carried only as **opaque ValueIds** in a worker-side handle bag. They retain genuine taint/provenance. The planner may only place handles into later nodes — never literals, never re-mint via mid-stream ProvideIntent (M7 anti-laundering preserved). ProvideIntent remains **exactly once** before RequestFd for the Session.

### Deterministic Multi-step Coding Planner

- [x] **CODE-01**: A deterministic multi-step coding planner (new `CaprunIntent` coding variant or equivalent) produces a multi-node plan over **shipped** sinks for at least one concrete workflow: filesystem edit → `process.exec` (tests) → `git.commit` → `git.push` → `github.pr`. No LLM tool-use loop. Email/file single-node planners remain green (no regression).

- [x] **CODE-02**: Success-path plan nodes for the coding recipe use **trusted-intent** operator args only (paths, commands, messages, remotes/refspecs from CLI/intent at session start). The recipe does **not** require multi-file untrusted RequestFd before irreversible sinks for the happy path (avoids HARDEN-01 Draft demotion killing CommitIrreversible). Mid-loop I2 proof uses a deliberate tainted-handle routing path (see LIVE-08), not success-path laundering.

### CLI Multi-node Driver

- [x] **CLI-01**: `caprun run` (or an explicitly documented sibling verb) accepts a coding multi-step intent + workspace + trusted `--policy`, binds policy at session creation (POLICY-03), and **drives the full multi-node coding chain** end-to-end. Existing Block → `review`/`confirm`/`deny`/`grant` surfaces are preserved and pointed at from the driver.

- [x] **CLI-02**: Stream stop semantics are honest and machine-checkable: on I2 Block → stop (or Block-and-Hold per CONFIRM-01), surface `effect_id` + review pointer; on `policy_deny` → distinct outcome; on Deny → abort remaining nodes; on full success → clear success exit. Exit codes distinguish success vs blocked vs denied/aborted. Silent continue-past-Block is forbidden.

### Mid-loop Confirm Continuity

- [x] **CONFIRM-01**: When a mid-stream node returns `BlockedPendingConfirmation` (e.g. always-confirm `git.push`, or I2 Block released by confirm), the multi-node run **holds the same Session** (Block-and-Hold): worker stays connected or has a designed same-Session resume that does **not** re-open ProvideIntent, re-bind policy, or mint new trusted values. Human confirm/deny acts on the durable pending row; remaining nodes continue only after Allowed release (or abort on deny). No dual-Session "stitch the chain later" as the product path.

### Live Proof (v1.10 DONE gate)

- [ ] **LIVE-07**: On real Linux, a design partner can run the multi-step coding intent via the real CLI (`caprun run` or documented equivalent) under a bound policy: edit → test → commit → push (confirm-release) → open PR (mock GitHub allowed for CI). The entire path is **one Session**, inspected via real `caprun audit`, with `verify_chain` true. **This is not a hybrid in-crate composition** — the SUCCESS claim requires the multi-node chain to be CLI-driven (closes v1.9 LIVE-05 honesty gap). Full-workspace regression green; no v1.0–v1.9 regression.

- [ ] **LIVE-08**: In the same proof family (same or sibling composed run), a mid-loop **I2 Block** is independently attributable: a genuinely tainted handle (non-stapled provenance root on a real read/exec event) occupies a sensitive sink arg (e.g. PR body and/or push refspec) under a **policy-permitted** sink; executor Blocks; `policy_deny` is not what fired; no effect of that node; chain remains `verify_chain` true. Distinct from a policy-deny control if one is included. Framing must not claim hybrid composition as CLI multi-step.

### Packaging

- [ ] **PKG-01**: A minimal Linux design-partner install path: documented release build that co-locates `caprun`, `caprun-worker`, and `caprun-exec-launcher` (sibling `current_exe()` layout), plus env/credential checklist (`CAPRUN_*`, policy file, GitHub grant token as applicable). Thin install script acceptable; not cargo-dist/deb/snap productization. `cargo install --path cli/caprun` alone is **not** sufficient (misses exec-launcher).

### Supply-Chain & Invariant Hygiene

- [x] **HYG-02**: Multi-step work re-asserts HYG-01 / Gate discipline: zero new crates unless design-gate-justified (default: **zero**); no `EffectRequest` token under `crates/`; Gate 3 mint-site list unchanged or explicitly amended; `check-invariants.sh` green; compose-verify remains the authoritative Linux gate.

## Future Requirements

Deferred past v1.10. Tracked, not in this roadmap.

### Planner / productization

- **LLM-MS-01**: Real multi-step LLM tool-use loop on the v1.4 sidecar (retries/error handling) — only after eval baseline / domain rubrics
- **CODE-BREADTH-01**: github.pr merge/comment, richer coding recipes, replan-from-observation loops
- **PACK-02**: Broader packaging (deb/snap/cargo-dist), Mac best-effort install without security claims

### Residual / hygiene (non-blocking carry-forward)

- **PUSH-CAP-01**: Lift git.push 10MB pack-cap (streaming/chunked) — fails closed today
- **SCRUB-01**: leg-5b credential scrub-branch hardening on push error path

## Out of Scope

Explicitly excluded for v1.10. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| LLM multi-step / ReAct tool-use loop | Manual-ops-first; evals before agent loop; v1.4 single-shot seam remains |
| Agent frameworks, memory, marketplace, multi-agent, MCP host | Product boundary (PLAN.md / CLAUDE.md) — not an agent framework |
| Cedar / rich policy language | v1.9 hardcoded-schema policy sufficient; POLICY-02 keeps I2 in Rust TCB |
| Web UI | Trust surface is CLI + `caprun audit` |
| New sink families (DB, merge, comment, deploy, …) | Effect surface already covers the coding loop; multi-step first |
| Session-wide confirm waiver / YOLO mode | Defeats always-confirm and I2 human gate |
| Raw `EffectRequest` / free-form tool maps | Architectural invariant (Gate 1) |
| Cross-host / Biscuit / gVisor / Firecracker | Post-v1.x platform concerns |
| Mac / WSL2 security claims | Linux-only remains |
| git.push pack-cap lift as DONE requirement | Non-blocking residual; fails closed |

## Traceability

Which phases cover which requirements. Filled by the roadmapper.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-19 | Phase 47 | Complete |
| DESIGN-20 | Phase 47 | Complete |
| STREAM-01 | Phase 48 | Complete |
| STREAM-02 | Phase 48 | Complete |
| CODE-01 | Phase 49 | Complete |
| CODE-02 | Phase 49 | Complete |
| CLI-01 | Phase 50 | Complete |
| CLI-02 | Phase 50 | Complete |
| CONFIRM-01 | Phase 50 | Complete |
| LIVE-07 | Phase 51 | Pending |
| LIVE-08 | Phase 51 | Pending |
| PKG-01 | Phase 52 | Pending |
| HYG-02 | Phase 47 | Complete |

**Coverage:**

- v1 requirements: 13 total
- Mapped to phases: 13/13 ✓
- Unmapped: 0

---
*Requirements defined: 2026-07-23*
*Last updated: 2026-07-23 after `/gsd-new-milestone` v1.10 roadmap (phases 47-52, 13/13 mapped)*
