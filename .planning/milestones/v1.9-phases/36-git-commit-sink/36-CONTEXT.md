# Phase 36: `git.commit` Sink - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning
**Mode:** Orchestrator-authored (autonomous v1.8 run). Design is fixed by the CLEARED
gate doc `planning-docs/DESIGN-git-github-http-sinks.md` §1 — this phase is its mechanical
realization for `git.commit`. Do not re-open design decisions.

<domain>
## Phase Boundary

Implement the `git.commit` external-effect sink: the broker runs `git commit` in a workspace
repo via the v1.7 broker-spawned confined-child launcher (`caprun-exec-launcher`), captures +
taints the output via the existing `mint_from_exec`, classifies the sink `MutateReversible`,
and neutralizes git config/hooks so a planted malicious `.git/config`/hook does NOT execute.
This is the LOWEST-RISK v1.8 sink — near-verbatim reuse of the shipped `process.exec` path.

Requirements: GIT-01 (see .planning/REQUIREMENTS.md).
Authoritative design: `planning-docs/DESIGN-git-github-http-sinks.md` §1 (+ §6 pitfalls 2/10, §7, §8).
</domain>

<decisions>
## Implementation Decisions (FIXED by the cleared DESIGN gate — DESIGN §1)

1. **Dispatch = Pattern B, exec-launcher reuse (near-verbatim).** `git.commit` dispatches
   exactly like `process.exec`: broker spawns `caprun-exec-launcher`, which self-confines
   (rlimits → Landlock exec-child ruleset → seccomp exec-child filter, net-deny UNCHANGED)
   then `execve`s the resolved system `git` binary. Reuse `run_launcher` verbatim
   (`crates/brokerd/src/sinks/process_exec.rs`): `env_clear()` + `SAFE_EXEC_PATH`,
   `Stdio::piped()` capture, wall-clock timeout, combined-output byte cap, `kill_on_drop`.
   The confined WORKER never execve's git. No new spawn machinery.

2. **Effect-class = `MutateReversible`** — a deliberate, justified exception to the
   fail-closed `unknown → CommitIrreversible` default (`sink_sensitivity.rs` `sink_effect_class`).
   Add a `"git.commit" => EffectClass::MutateReversible` arm + a test asserting it. It is the
   FIRST non-CommitIrreversible REAL sink (only `test.observe` is non-CommitIrreversible today,
   test-only). Consequence: `git.commit` survives an I1-demoted (draft-only) session.

3. **Sink args + I2-sensitivity:** `message` = content-sensitive (reuse CONTENT-01 discipline,
   `is_content_sensitive` in `sink_sensitivity.rs`) — the taint CARRIER that must genuinely
   propagate downstream, NEVER re-minted clean; a tainted `message` Blocks under the UNMODIFIED
   `submit_plan_node` collect-then-Block loop, exactly like a tainted `email.send` body. If
   paths/pathspec are modeled, they are routing-sensitive reusing the `path` role vocabulary.
   Register the sink in `KNOWN_SINKS` (`sink_schema.rs`) with an exact-match arg schema (Step-0
   fail-closed gate).

4. **Taint flow = NO new mint site.** git IS an exec under Pattern B; captured stdout/stderr is
   minted by the already-shipped `mint_from_exec` (`quarantine.rs`), rooted on the
   `process_exited` Event the sink appends, carrying `[ExternalUntrusted, ExecRaw]`,
   `origin_role="exec_output"`. No new `TaintLabel` variant. (Only http.request, Phase 37,
   adds a new mint.)

5. **git config/hook neutralization (closes P2 RCE), hardcoded in the launcher invocation:**
   `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `-c core.hooksPath=/dev/null`, no
   aliases (neutralized config can't define one), `GIT_TERMINAL_PROMPT=0`, `env_clear()`'d
   child (inherits none of the broker's env — no OPENAI_API_KEY / CAPRUN_SMTP_*). Landlock
   confined to WorkspaceRoot. seccomp net-deny UNCHANGED (a local commit needs no network).
   git binary floor: ≥2.30.
</decisions>

<code_context>
## Existing Code Insights
- `crates/brokerd/src/sinks/process_exec.rs` — the Pattern B exemplar to reuse (`run_launcher`,
  the two-phase `process_exited` audit append, the env_clear+SAFE_EXEC_PATH discipline, the
  env-clear leak regression test ~820-871).
- `crates/executor/src/sink_sensitivity.rs` — add `sink_effect_class` arm (MutateReversible),
  `is_content_sensitive` row for `git.commit` `message`; `sink_schema.rs` KNOWN_SINKS row.
- `crates/brokerd/src/quarantine.rs` — `mint_from_exec` (reused as-is).
- `crates/runtime-core/src/effect.rs` — the 3-class Effect ontology (MutateReversible exists).
- `cli/caprun-exec-launcher` — the launcher binary (git-env neutralization applied to the git argv/env).
- Tests: mirror the process.exec sink tests + a planted-hook/alias negative test (RCE does not fire),
  a tainted-message-Blocks test, a MutateReversible-survives-draft test, a genuine-propagation
  (unbroken audit-DAG edge) test. Linux-gated enforcement tests under `#[cfg(target_os="linux")]`.
</code_context>

<specifics>
## Specific Ideas
- Follow the check-invariants discipline: no raw EffectRequest; sink sensitivity stays hardcoded
  table rows (never a policy file). `scripts/check-invariants.sh` must stay green.
- macOS builds/tests: enforcement tests are Linux-gated (cfg(target_os="linux")) — a green macOS
  run showing "0 passed" for those is EXPECTED, not a gap. Real verification is the Linux gate
  (Phase 40 composed proof; per-phase, run cargo build --tests on Linux to catch cfg-gated callers).
</specifics>

<deferred>
## Deferred Ideas
None — git.push (Phase 39), http (37), github.pr (38) are separate phases.
</deferred>
