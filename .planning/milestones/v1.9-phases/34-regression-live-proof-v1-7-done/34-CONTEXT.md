# Phase 34: Regression & Live Proof (v1.7 DONE) - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning
**Source:** Operator fold-in scope (locked decisions handed to the orchestrator at `/gsd-plan-phase 34 --auto`)

<domain>
## Phase Boundary

Phase 34 is the **v1.7 live-proof close**. It delivers three things, in this order:

1. **EXEC-05 (TCB slice) — MUST land FIRST.** Close the Phase-33 adversarial-trace
   open item: a `process.exec` blocked by I2 currently yields
   `BlockedPendingConfirmation` but `confirm()` has **no `process.exec` dispatch
   arm** — the P33 pre-Step-5 entry-guard makes it fail closed-recoverable, but a
   blocked `process.exec` cannot yet be human-released. Wire the release.
2. **LIVE-01 — composed acceptance on real Linux** exercising exec (blocked + clean),
   fs write/edit within `WorkspaceRoot`, and the new EXEC-05 confirm-release path.
3. **LIVE-02 — full-workspace regression** green on real Linux, no regression to
   v1.0–v1.6, asserted on counts + named tests, plus a dedicated negative test per
   new sink.

**Explicitly NOT in scope:** any new security decision, new `ExecutorDecision`
variant, `submit_plan_node` signature change, new raw `EffectRequest` path, new
Gate-3 mint site, or a new design gate. EXEC-05 is a **`from_resolved` extension**
of the Phase-31-pinned `process.exec` model — not a new security posture.
</domain>

<decisions>
## Implementation Decisions

### EXEC-05 — process.exec confirm-release (TCB, lands before the live proof)

- **D-01:** Add `invoke_process_exec_from_resolved` in
  `crates/brokerd/src/sinks/process_exec.rs` that re-invokes the exec sink from the
  frozen `PendingConfirmation.resolved_args`, **mirroring**
  `invoke_file_write_from_resolved` / `invoke_file_create_from_resolved` (read those
  two as the reference implementations before writing).
- **D-02:** The released run re-applies the **EXACT Allowed-path discipline** already
  used by `invoke_process_exec`: broker-spawned confined child (Landlock + seccomp +
  default-deny net + rlimits + wall-clock timeout + byte cap), stdout/stderr captured.
- **D-03:** Output is taint-minted via the **sanctioned** `mint_from_exec` — untrusted,
  **non-stapled**, provenance anchored at the `exec` Event — and `output_value_id` is
  populated. The `from_resolved` path REUSES `mint_from_exec`, so the Gate-3 mint-site
  allow-list needs **no new entry** (confirm it stays green).
- **D-04:** The two-phase `process_exited` / `process_spawn_failed` audit Events are
  **chained onto the `confirm_granted` head** (not a fresh root), preserving one
  unbroken audit DAG.
- **D-05:** Add the `"process.exec"` arm to `confirmation.rs` **Step-7 dispatch** so
  `caprun confirm` routes a released exec sink to `invoke_process_exec_from_resolved`.
- **D-06:** The command runs **exactly once** on release (idempotent / no double-spawn),
  matching the file.write / file.create confirm-release contract.
- **D-07:** **Preserve** the Phase-33 pre-Step-5 entry-guard: any still-un-dispatchable
  sink remains **fail-closed-recoverable** (the guard must not regress OPEN).

### EXEC-05 discipline invariants (must hold — no bypass)

- **D-08:** I2 stays **table-entries-only** — no new `ExecutorDecision`, no
  `submit_plan_node` change, no policy that can disable I2.
- **D-09:** No new raw `EffectRequest` path (`check-invariants.sh` Gate 1 stays green).
- **D-10:** `mint_from_exec` remains the sole sanctioned exec-output mint site
  (Gate-3 mint-site list unchanged and green).

### EXEC-05 acceptance test (cfg(linux))

- **D-11:** A `cfg(target_os = "linux")` test: block a `process.exec` with a tainted
  arg → `caprun confirm` releases it → the command runs **exactly once**, output is
  taint-minted, the sink Event is durably chained (`verify_chain` true); PLUS a leg
  asserting the entry-guard's fail-closed behavior for any still-un-dispatchable sink.

### LIVE-01 — composed acceptance run (real Linux)

- **D-12:** One composed acceptance run on real Linux proves end-to-end, in the same
  run: (a) an `exec` whose **tainted output** is routed to a sensitive sink arg is
  **Blocked** (I2, genuine non-stapled taint chain, `verify_chain` true); (b) a **clean
  exec/fs path is Allowed**; (c) a **fs write/edit within `WorkspaceRoot` succeeds and
  is audited**; (d) the EXEC-05 confirm-release path is exercised.
- **D-13:** Run via `scripts/mailpit-verify.sh` **or an exec-scoped equivalent**,
  capturing the **true exit code BEFORE any pipe** (never `script | tail`); assert on
  named tests + counts.

### LIVE-02 — full-workspace regression (real Linux)

- **D-14:** Full-workspace regression re-runs **green on real Linux** with **no
  regression to v1.0–v1.6**, asserted on **counts + named tests** (never exit 0 through
  a pipe), plus a **dedicated negative test per new sink** (process.exec, fs write/edit).

### MANDATORY release gates (orchestrator-owned — NOT a gsd-executor)

- **D-15:** After the EXEC-05 TCB slice lands and **before** the composed live proof:
  run the **Linux compile-check** — `cargo build --tests --workspace --keep-going` via
  `scripts/mailpit-verify.sh`, true-exit-0 captured before any pipe (guards
  cfg(linux) test-blindness — a green macOS build compiles no `#[cfg(target_os="linux")]`
  targets).
- **D-16:** Also before the composed live proof: a **fresh, non-self Fable-5
  adversarial code-trace** of the confirm-release TCB diff (standing project
  guardrail — has caught real MAJORs a passing verifier + green gates missed 7×). The
  **orchestrator owns that spawn**, not a gsd-executor. Its findings must be resolved
  (or the diff APPROVED) before the live proof is authorized.
- **D-17:** v1.7 close requires a **human DONE sign-off** before the milestone is
  marked (v1.5 / v1.6 precedent). **Not pushed** unless the operator explicitly says so.

### Claude's Discretion

- Exact plan/wave decomposition (EXEC-05 TCB in an early wave, live proofs after the
  release gates), test file names, and the precise shape of the exec-scoped verify
  harness vs. reusing `scripts/mailpit-verify.sh`.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & spec (pinned — do NOT re-decide)
- `planning-docs/DESIGN-effect-breadth-exec.md` — the Phase-31 canonical spec pinning
  the broker-spawned confined-child `process.exec` model, exec-output taint label +
  `origin_role`, and fail-closed defaults. EXEC-05 is a `from_resolved` extension of
  this model; it introduces no new security decision.
- `planning-docs/PLAN.md` — caprun definitive plan (source of truth on any conflict).

### Reference implementations to mirror (from_resolved pattern)
- `crates/brokerd/src/sinks/process_exec.rs` — existing `invoke_process_exec`
  (Allowed-path discipline to replicate) + where `invoke_process_exec_from_resolved`
  lands.
- `crates/brokerd/src/sinks/` — `invoke_file_write_from_resolved` /
  `invoke_file_create_from_resolved` (the exact mirror the new function follows).
- `crates/brokerd/src/confirmation.rs` — Step-7 dispatch (add the `"process.exec"`
  arm) + the Phase-33 pre-Step-5 entry-guard to preserve.

### Open item this phase closes
- `.planning/phases/33-filesystem-read-write-breadth/33-ADVERSARIAL-REVIEW.md`
- `.planning/phases/33-filesystem-read-write-breadth/33-05-SUMMARY.md`
  — both record the process.exec confirm-release open follow-up.

### Invariant gates (must stay green)
- `scripts/check-invariants.sh` — Gate 1 (no raw `EffectRequest`), Gate 3 (mint-site
  allow-list — EXEC-05 adds NO new entry).
- `scripts/mailpit-verify.sh` — the Linux verification harness (true-exit-before-pipe).

### Project rules
- `CLAUDE.md` — hard constraints (TCB is Rust; I2 hardcoded in executor; effect path
  is locked; terminology locked; Linux-only security tests).
</canonical_refs>

<specifics>
## Specific Ideas

- Mirror `invoke_file_write_from_resolved` almost line-for-line, swapping the sink body
  for the exec-spawn/capture/mint path from `invoke_process_exec`.
- The EXEC-05 acceptance test should reuse the existing exec-block test scaffolding
  (P32 `s9_*` exec tests) and add a confirm-release leg, exactly as P33 added
  `s9_file_write_block` + its confirm-release leg for file.write.
- The composed LIVE-01 run mirrors prior milestone composed acceptances (v1.3
  `live_acceptance_v1_3_composed`, v1.6 Phase 30) — one run, multiple legs, per-session
  `verify_chain` true.
</specifics>

<deferred>
## Deferred Ideas

- `git` / `github.pr` and `http.request` sinks — v1.8 (Effect Breadth II).
- Real multi-step LLM planner loop — v1.9 (needs an eval set + baseline first).
- Declarative policy file / Cedar, SDK / audit-DAG viewer, packaging — v1.10+.
- Full push to origin — only on explicit operator instruction (v1.3/v1.5 precedent of
  close-without-push; v1.6 chose to push — this is the operator's call at close).
</deferred>

---

*Phase: 34-regression-live-proof-v1-7-done*
*Context folded in 2026-07-18 by the orchestrator from operator-supplied locked decisions*
