# Phase 51: Non-hybrid LIVE Proof (v1.10 DONE) - Research

**Researched:** 2026-07-29
**Domain:** Composition + acceptance proof (CLI multi-node coding stream on real Linux; framing honesty; genuine mid-loop I2 Block) — little/no TCB change; closes v1.9 LIVE-05 hybrid honesty gap
**Confidence:** HIGH

## Summary

Phase 51 is the **v1.10 DONE gate**. Phases 47–50 already shipped the design gate, plan-stream substrate, deterministic coding planner, CLI multi-node driver, and Block-and-Hold confirm continuity. What remains is **composition under the real CLI on real Linux**: (LIVE-07) one Session driven by genuine `caprun run safe-coding-workflow …` through edit → test → commit → push (confirm-release) → open PR (mock GitHub), inspected by genuine `caprun audit`, with `verify_chain` true; (LIVE-08) a sibling/same-family mid-loop I2 Block where a genuinely tainted bag handle occupies a policy-permitted sensitive sink arg, with no effect of that node and distinct from `policy_deny`. Full-workspace compose-verify regression + `check-invariants.sh` must stay green.

The load-bearing honesty delta vs v1.9 LIVE-05: the multi-node SUCCESS chain must **not** be hybrid in-crate composition (`evaluate_plan_node_and_record_for_test` per leg). v1.9 deliberately framed that hybrid and used one genuine `caprun run` only for a single-node Block leg. Phase 50 productized the multi-node path; Phase 51 must **drive the whole coding chain through that product path** and machine-check the framing so hybrid cannot be re-sold as DONE.

**Primary recommendation:** Add a Linux+`mock-egress-ca` LIVE proof module that spawns the real `caprun` binary for `safe-coding-workflow` under compose-verify (CAPRUN_CONFIRM=external + concurrent confirm/grant sidecar; mock push/PR hosts; F1-safe layout; one Session). Promote the Phase-49 test-only `CodingI2ProofPlanner` to a **default-off, allowlisted env-selected** proof planner (`CAPRUN_CODING_I2_PROOF=1` forwarded through the worker `env_clear` allowlist) so LIVE-08 is also CLI-driven without laundering the success-path recipe. Frame both claims bluntly in module docs + assertions; run authoritative gate via `scripts/compose-verify.sh`.

<user_constraints>
## User Constraints

> No `*-CONTEXT.md` for this phase (discuss-phase skipped / not present). Locked authority is derived from REQUIREMENTS (LIVE-07/08), ROADMAP Phase 51 success criteria, DESIGN-multi-step-plan-stream.md (CLEARED), Phase 50 shipped product path, Phase 49 coding planner + LIVE-08 expressibility, Phase 48 stream substrate, v1.9 Phase 46 hybrid honesty record, and CLAUDE.md hard constraints.

### Locked Decisions (must honor — do not re-litigate)

1. **LIVE-07:** On real Linux, design partner runs multi-step coding intent via real CLI (`caprun run` or documented equivalent) under bound policy: edit → test → commit → push (confirm-release) → open PR (mock GitHub allowed for CI). **One Session**, inspected via real `caprun audit`, `verify_chain` true. **Not hybrid in-crate composition** — SUCCESS requires multi-node chain CLI-driven (closes v1.9 LIVE-05 honesty gap). Full-workspace regression green; no v1.0–v1.9 regression. [VERIFIED: `.planning/REQUIREMENTS.md`; `.planning/ROADMAP.md` Phase 51]
2. **LIVE-08:** Same proof family (same or sibling composed run): mid-loop I2 Block independently attributable — genuinely tainted handle (non-stapled provenance root on real read/exec event) occupies sensitive sink arg (e.g. PR body and/or push refspec) under **policy-permitted** sink; executor Blocks; `policy_deny` is not what fired; no effect of that node; `verify_chain` true. Framing must not claim hybrid composition as CLI multi-step. [VERIFIED: REQUIREMENTS.md]
3. **Framing honesty machine-checked against hybrid overclaim** (success criteria #2). [VERIFIED: ROADMAP Phase 51 SC2; CITED: DESIGN §0 / §14 LIVE DONE pin]
4. **Authoritative Linux gate:** `scripts/compose-verify.sh` (Mailpit + mock GitHub, `brokerd/mock-egress-ca`); from Phase 16+ never bare `docker run rust:1` when SMTP may fire; never ad-hoc named Docker volume for `CARGO_TARGET_DIR`. [CITED: CLAUDE.md]
5. **Effect path locked:** plan nodes only; no `EffectRequest`; Gate 1. I2 hardcoded in executor TCB; policy never disables I2 (POLICY-02). [CITED: CLAUDE.md / DESIGN]
6. **HYG-02 / zero new crates** default; Gate 3 mint list unchanged; `check-invariants.sh` green. [CITED: DESIGN §8 / REQUIREMENTS HYG-02]
7. **Success path trusted-intent only** (CODE-02): DeterministicPlanner never places `out_*` into sink args. LIVE-08 uses deliberate alternate routing, not success-path laundering. [VERIFIED: Phase 49; `cli/caprun/src/planner.rs` `plan_coding_next`]
8. **Block-and-Hold same Session** for always-confirm `git.push` (CONFIRM-01 shipped Phase 50). No dual-Session stitch; no session-wide confirm waiver; no re-submit of blocked node; ProvideIntent once. [VERIFIED: Phase 50 VERIFICATION]
9. **Phase boundaries:** Phase 51 = LIVE-07/08 non-hybrid proof + framing + regression. **Not** packaging (PKG-01 → Phase 52), **not** LLM multi-step, **not** new sinks/crates, **not** pack-cap lift.

### Claude's Discretion (recommend in plan)

- Exact test file layout (one module vs success + negative siblings). **Recommend:** `cli/caprun/tests/live_acceptance_v1_10_cli.rs` (LIVE-07 + LIVE-08 + framing + host guard) mirroring v1.9 composed structure but CLI-driven.
- How LIVE-08 selects the proof planner without success-path laundering. **Recommend:** promote `CodingI2ProofPlanner` into `cli/caprun/src/planner.rs` as product-selectable **only** when `CAPRUN_CODING_I2_PROOF=1` (default off); main forwards that single non-secret env into the worker allowlist (mirror `CAPRUN_PLANNER=llm` pattern). Reject re-using hybrid `evaluate_plan_node_and_record_for_test` as the LIVE-08 DONE claim.
- Whether LIVE-08 is same Session as LIVE-07 or sibling. **Recommend sibling Session/run** in same test file (sequential, shared compose env) so success path stays pure trusted-intent and I2 path is independently attributable.
- Confirm automation for non-TTY CI. **Recommend:** `CAPRUN_CONFIRM=external` + concurrent `caprun confirm` / `caprun grant` sidecar thread (parent non-TTY auto-selects external poll; default 300s timeout — tighten via `CAPRUN_CONFIRM_TIMEOUT_SECS` in tests).
- Whether a thin in-crate hybrid control remains for regression of sinks. **Recommend:** keep v1.9 hybrid tests as **regression only**; never cite them as LIVE-07 SUCCESS.

### Deferred Ideas (OUT OF SCOPE)

- Phase 52 packaging / install script (PKG-01)
- LLM multi-step / ReAct (LLM-MS-01)
- New crates, EffectRequest path, session-wide confirm waiver, dual-Session stitch
- github.pr merge/comment, replan-from-observation (CODE-BREADTH-01)
- git.push 10MB pack-cap lift (PUSH-CAP-01), leg-5b scrub hardening (SCRUB-01)
- Cedar, web UI, Mac security claims, cross-host / gVisor / Firecracker
</user_constraints>

## Project Constraints (from CLAUDE.md)

Treat with the same authority as locked DESIGN decisions:

1. **Source of truth:** `planning-docs/PLAN.md` wins on doc/code conflicts.
2. **Effect path locked:** `submit_plan_node(session_id, PlanNode { sink, args: ValueIds })` only — never raw `EffectRequest`. Gate 1 fails if `EffectRequest` appears under `crates/`.
3. **I0 / I1 / I2:** I2 hardcoded in Rust executor; policy never disables I2; untrusted seed → draft-only; no ambient authority for workers.
4. **Terminology locked:** Intent, Session, Planner, Worker, Broker, Adapter, Effect, Artifact, Event. Project/binary = `caprun`.
5. **TCB is Rust.** Linux-only security claims; macOS stubs expected (`0 passed` on macOS for cfg-gated tests is expected, not a gap).
6. **From Phase 16+:** Linux verification that may touch SMTP uses `scripts/mailpit-verify.sh`; full composed LIVE uses `scripts/compose-verify.sh`. Never bare `docker run rust:1` alone when SMTP may fire. Never bind named Docker volumes for `CARGO_TARGET_DIR` as a manual speed hack.
7. **v0/v1 DONE lineage:** substrate working ≠ done. v1.10 DONE = non-hybrid CLI multi-node LIVE (this phase). Hybrid rebrand of LIVE-05 is a framing failure.
8. **Out of scope:** agent frameworks, memory, marketplace, Cedar, web UI, cross-host/Biscuit, gVisor/Firecracker, LLM multi-step until relevant gates.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LIVE-07 | Real Linux, real CLI multi-node coding chain one Session; `caprun audit` + `verify_chain` true; not hybrid; full-workspace regression green | §§ Architecture Patterns 1–3, EXISTS→NEW table, Code Examples (CLI driver harness), Validation Architecture, Common Pitfalls 1/4/8 |
| LIVE-08 | Same proof family mid-loop I2 Block: genuine tainted handle → sensitive arg under policy-permitted sink; not policy_deny; no effect; `verify_chain` true; framing honest | §§ Pattern 4 (proof planner), Pattern 5 (genuine taint), Security Domain, Validation Architecture, Pitfalls 2/5/6 |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Drive multi-node coding SUCCESS | CLI product path (`caprun run safe-coding-workflow`) | Worker stream + DeterministicPlanner | LIVE-07 forbids hybrid as SUCCESS claim |
| Mid-loop always-confirm `git.push` | CLI main `orchestrate_coding_stream` + `brokerd::confirmation` | Worker Block-and-Hold | Phase 50 shipped; LIVE consumes it |
| github.pr grant gate | Operator / test sidecar `caprun grant` | Broker `has_github_grant` | Printed pointer at session start; never auto-grant |
| Per-node I2 + always-confirm rewrite | Broker + executor TCB | — | Unchanged; proof only composes |
| Genuine taint mint | Broker `mint_from_exec` on real process_exited | Worker bag `out_{step}` | Non-stapled provenance root |
| LIVE-08 tainted placement | Env-gated proof planner (recommended) | Bag handle only | Success path must not place `out_*` |
| Inspect / verify_chain | Real `caprun audit` subprocess | In-process `verify_chain` cross-check | Viewer is trust surface |
| Mock push / PR / TLS | compose-verify sidecars + `mock-egress-ca` | — | Real TLS under non-default feature |
| Framing honesty | Test module docs + machine asserts | Milestone record (verify-work) | DOC-01 lineage |
| Full regression | `scripts/compose-verify.sh` | `check-invariants.sh` | Authoritative Linux gate |

## What Already Exists vs What Must Be Written

### EXISTS — reuse (do not rebuild)

| Artifact | Path / anchor | Role for Phase 51 |
|----------|---------------|-------------------|
| Coding CLI argv | `cli/caprun/src/main.rs` `"safe-coding-workflow"` arm (~333–356) | LIVE-07 driver |
| Mid-loop hold orchestration | `main.rs` `orchestrate_coding_stream` (~835–946), `resolve_mid_loop_hold`, CAPRUN_CONFIRM external poll | Confirm automation |
| Stream protocol | `cli/caprun/src/stream_hold.rs` | BLOCKED / STREAM_DONE / exit map 0/2/3/1 |
| Worker Block-and-Hold | `cli/caprun/src/worker.rs` SafeCodingWorkflow hold (~472–513) | Same Session; no re-submit |
| Success recipe | `cli/caprun/src/planner.rs` `plan_coding_next` steps 0–4 | file.write → process.exec → git.commit → git.push → github.pr |
| LIVE-08 unit expressibility | `cli/caprun/tests/planner.rs` `CodingI2ProofPlanner` + `coding_i2_proof_places_out_handle` | Placement pattern for proof planner (not LIVE DONE) |
| Hybrid bag taint substrate | `cli/caprun/tests/stream_substrate.rs` `taint_via_bag_exec_output_blocks_with_genuine_provenance` | Provenance assert pattern; **not** LIVE-07 claim |
| v1.9 hybrid SUCCESS template | `cli/caprun/tests/live_acceptance_v1_9_composed.rs` | Layout/F1/key/mock constants/audit subprocess helpers — **not** the DONE claim |
| v1.9 framing honesty record | `.planning/milestones/v1.9-phases/46-…/46-MILESTONE-RECORD.md` §1 | Anti-pattern to invert for v1.10 |
| LIVE-06 negative legs | `crates/brokerd/tests/s46_negative_legs_composed.rs` | Distinct-tag patterns (I2 vs policy_deny) |
| Mock GitHub / receive-pack | `scripts/mock-github/server.py` (`/accept/*`, `/repos/*/pulls` 201) | compose-verify sidecar |
| compose-verify harness | `scripts/compose-verify.sh` | Authoritative gate; sets `CAPRUN_GITHUB_API_BASE`, Mailpit, mock IP, `mock-egress-ca` |
| Policy bind | `brokerd::policy::bind_policy` + coding_cli `MINIMAL_POLICY_JSON` | POLICY-03 |
| Grant / confirm CLI | `caprun grant`, `caprun confirm` / `deny` / `review` / `audit` | Sidecar human actions |
| Content sensitivity | `GITHUB_PR_CONTENT_SENSITIVE = ["title","body"]`; `GIT_PUSH_ROUTING_SENSITIVE = ["remote","refspec"]` | LIVE-08 sink arg choice |
| Phase 50 product proofs | `coding_cli`, `stream_hold`, `stream_substrate` hold tests | Regression; do not re-claim LIVE |

### NEW — Phase 51 must write

| Deliverable | Why new |
|-------------|---------|
| Linux LIVE-07 CLI e2e test (real binary multi-node chain, one Session) | No test today drives full coding chain via `caprun run` |
| Concurrent external confirm + grant sidecar harness | Non-TTY CI cannot interactive-confirm; grant required before successful PR |
| LIVE-08 CLI-driven I2 proof path | Proof planner not product-selectable today (test-only in `planner.rs` tests); worker only selects Deterministic/Llm |
| Framing honesty machine-checks | Forbid hybrid DONE language; assert CLI spawn + single session_id |
| Workspace git fixture for push/PR under mock hosts | coding_cli host fixtures stop before confined multi-node SUCCESS |
| compose-verify scoped recipe docs in test module | Match v1.9 COMPOSE_VERIFY_CMD pattern for this binary |
| Optional: milestone framing notes in phase docs | Honesty disclosure for verify-work / close |

### Explicitly NOT new product surface (unless LIVE-08 env path)

- New crates / new sinks / new mint sites
- Reconnect-remint / dual-Session stitch
- Session-wide confirm waiver / auto-confirm / auto-grant
- Packaging (Phase 52)
- Changing success-path `plan_coding_next` to place `out_*`

## Standard Stack

### Core (reuse only — zero new crates / packages)

| Library / artifact | Version / locus | Purpose | Why Standard |
|--------------------|-----------------|---------|--------------|
| Rust edition 2021, workspace resolver 3 | root `Cargo.toml` | TCB language | Locked project stack [VERIFIED: Cargo.toml + cargo 1.97.1 / rustc 1.97.1] |
| `cli/caprun` product binary | `cli/caprun/src/{main,worker,planner,stream_hold}.rs` | Multi-node driver + hold | Phase 50 shipped path LIVE drives [VERIFIED: 50-VERIFICATION] |
| `brokerd` + `executor` + `sandbox` | workspace crates | Policy, I2, sinks, confinement | Unchanged composition targets |
| `runtime-core` | `CaprunIntent::SafeCodingWorkflow` | Closed intent enum | Phase 49 [VERIFIED: intent.rs] |
| `scripts/compose-verify.sh` | repo | Authoritative Linux LIVE | CLAUDE.md Phase 16+ / v1.8+ [VERIFIED: scripts] |
| `scripts/check-invariants.sh` | repo | Gates 1–6 | HYG-02 [VERIFIED: scripts] |
| `scripts/mock-github/` | repo | PR 201 + git-receive-pack accept | mock-egress-ca [VERIFIED: server.py] |
| rusqlite + sha2/hmac audit DAG | workspace | `verify_chain` | Shipped |

### Supporting

| Artifact | Purpose | When to Use |
|----------|---------|-------------|
| `planning-docs/DESIGN-multi-step-plan-stream.md` | LIVE DONE pin (CLI one Session; hybrid not DONE) | Framing + scope |
| Phase 50 `50-RESEARCH.md` / `50-PATTERNS.md` | CLI argv, hold, exit codes, grant pointer | Driver harness design |
| Phase 49 `CodingI2ProofPlanner` | out_1 → github.pr body placement | LIVE-08 proof planner lift |
| Phase 48 `taint_via_bag_*` | Genuine provenance asserts | LIVE-08 assertion template |
| v1.9 `live_acceptance_v1_9_composed.rs` | F1 layout, seed key, mock constants, audit spawn | Copy helpers, invert framing |
| `s46_negative_legs_composed.rs` | I2 vs policy_deny distinct tags | LIVE-08 distinctness |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| CLI multi-node SUCCESS | Hybrid `evaluate_plan_node_and_record_for_test` chain | **REJECTED as LIVE-07 DONE** — that is LIVE-05 honesty class |
| Env-gated proof planner for LIVE-08 | Hybrid bag placement only (stream_substrate style) | Weaker vs PROJECT "via real CLI"; only acceptable as unit regression, not LIVE-08 DONE |
| Env-gated proof planner | New CaprunIntent variant / CLI kind | Larger surface; env default-off is thinner and mirrors CAPRUN_PLANNER |
| Interactive mid-loop confirm in CI | CAPRUN_CONFIRM=external + concurrent confirm | Interactive needs TTY; CI is non-TTY → external is mandatory for automated LIVE |
| Same Session for LIVE-07+08 | Sibling sessions | Sibling keeps success path pure; same Session would force out_* placement mid-success |
| Auto-grant inside `caprun run` | Explicit `caprun grant` sidecar | **REJECTED** — grant is distinct human capability (GITHUB-02) |
| New crates / clap / workflow engine | In-tree Rust tests + existing CLI | HYG-02 / product boundary |

**Installation:** none — **zero** external packages.

**Version verification:** No new packages. Host toolchain `cargo 1.97.1` / `rustc 1.97.1` [VERIFIED: local].

## Package Legitimacy Audit

> Phase 51 installs **zero** external packages (HYG-02 continues). Package Legitimacy Gate **N/A**.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| *(none)* | — | — | — | — | — | No installs |

**Packages removed due to [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
LIVE-07 SUCCESS (one Session, real CLI)
──────────────────────────────────────
Test harness (Linux + mock-egress-ca)
  │  spawn: caprun run --policy P safe-coding-workflow intent.json workspace audit.db
  │  env: CAPRUN_CONFIRM=external, CAPRUN_GIT_PUSH_TOKEN, CAPRUN_GITHUB_TOKEN,
  │       CAPRUN_GITHUB_API_BASE=https://github-mock.caprun.test
  │  concurrent sidecar:
  │     read stdout → session_id=… → caprun grant <sid> audit.db
  │     on pending git.push → caprun confirm <effect_id> audit.db
  ▼
Main (product) — Phase 50 path, unchanged for SUCCESS
  bind_policy once → create_session → print session_id + grant pointer
  spawn in-process broker (KEEP ALIVE across hold)
  spawn confined worker (piped stdio)
  orchestrate_coding_stream:
    NODE_ALLOWED × {file.write, process.exec, git.commit}
    BLOCKED git.push → external poll until Confirmed → PROCEED
    NODE_ALLOWED github.pr (grant already recorded)
    STREAM_DONE → exit 0
  print DAG; verify_chain
  ▼
Worker (confined)
  ProvideIntent ONCE → bag seed named_handles (no RequestFd)
  plan_next DeterministicPlanner: 5 nodes, intent keys only
  bag out_{step} on Allowed+mint (out_1 = process.exec) — unused on success path
  hold on push Block; PROCEED advances step without re-submit
  ▼
Broker / executor
  policy pre-I2 → I2 → always-confirm rewrite for git.push
  confirm() dispatches sink from durable snapshot
  github.pr → mock POST /repos/*/pulls 201
  git.push → mock /accept/.../git-receive-pack
  ▼
Inspection
  spawn: caprun audit <session_id> audit.db
  assert "Chain verification: PASSED"
  assert single session_id for whole chain
  assert terminal events: git_push_succeeded, github_pr_succeeded (etc.)

LIVE-08 mid-loop I2 (sibling run, same proof family)
──────────────────────────────────────────────────
Same CLI harness + CAPRUN_CODING_I2_PROOF=1 (default-off)
  worker selects CodingI2ProofPlanner
  steps 0..3 same as success (incl. push confirm PROCEED)
  step 4: github.pr body = bag out_1 (genuine mint_from_exec)
  policy PERMITS github.pr → I2 Block (not policy_deny)
  DO NOT confirm the I2-blocked PR → no github_pr_succeeded
  assert sink_blocked + provenance_chain[0] == process_exited
  assert verify_chain true; exit blocked/deny (3 or 2), never 0 as SUCCESS
```

### Recommended Project Structure (touch points)

```
cli/caprun/src/
├── main.rs          # OPTIONAL: forward CAPRUN_CODING_I2_PROOF=1 into worker allowlist
├── worker.rs        # OPTIONAL: select CodingI2ProofPlanner when env set (default Deterministic)
├── planner.rs       # OPTIONAL: move CodingI2ProofPlanner from tests into product module
│                    #           (default-off; never selected without env)
└── stream_hold.rs   # NO change expected

cli/caprun/tests/
├── live_acceptance_v1_10_cli.rs   # NEW: LIVE-07 + LIVE-08 + framing + host guard
├── coding_cli.rs                  # REGRESSION (argv only; no LIVE claim)
├── stream_substrate.rs            # REGRESSION (hybrid bag taint substrate only)
├── planner.rs                     # REGRESSION (expressibility unit)
├── live_acceptance_v1_9_composed.rs  # REGRESSION hybrid; keep framing
└── s45 / e2e / confirm / grant    # REGRESSION

scripts/
├── compose-verify.sh              # REUSE as authoritative gate
└── check-invariants.sh            # REUSE every wave

# DO NOT for Phase 51:
# - New crates / EffectRequest / new mint sites
# - Dual-Session stitch / reconnect-remint
# - Success-path out_* placement
# - Packaging scripts (Phase 52)
# - Claiming LIVE-07 via evaluate_plan_node_and_record_for_test composition
```

### Pattern 1: LIVE-07 CLI one-Session SUCCESS (prescriptive)

**What:** Spawn the real compiled `caprun` binary with the Phase 50 coding argv; hold mid-loop at always-confirm `git.push`; grant before PR; inspect via real `caprun audit`.

**Argv (locked by Phase 50):**

```text
caprun run --policy <sibling-policy.json> \
  safe-coding-workflow <coding-intent.json> <workspace-file> <audit.db>
```

**Intent JSON fixture (adapt coding_cli constants for mock):**

```json
{
  "kind": "SafeCodingWorkflow",
  "path": "src/hello.txt",
  "contents": "hello from caprun live-07\n",
  "test_command": "sh",
  "test_args_json": "[\"-c\", \"git add -A && true\"]",
  "commit_message": "caprun: live-07 safe coding",
  "remote": "https://github-mock.caprun.test/accept/repo.git",
  "refspec": "HEAD:refs/heads/caprun-live-07",
  "owner": "acme",
  "repo": "demo",
  "base": "main",
  "head": "caprun-live-07",
  "pr_title": "caprun live-07",
  "pr_body": "Opened by CLI multi-node stream"
}
```

**F1-safe layout (copy v1.9 / coding_cli):**

```
tmp/
├── audit.db + audit.db.key     # siblings of workspace, never nested under WorkspaceRoot
├── policy.json
├── coding-intent.json
└── workspace/                  # WorkspaceRoot
    ├── workspace.txt           # argv workspace-file (parent = root)
    ├── src/hello.txt           # pre-create for file.write O_TRUNC
    └── .git/                   # git init + identity; staged via test_command
```

**Env for compose-verify run:**

| Env | Purpose |
|-----|---------|
| `CAPRUN_CONFIRM=external` | Force dual-terminal poll (also auto when non-TTY) |
| `CAPRUN_CONFIRM_TIMEOUT_SECS` | Shorten for tests (e.g. 60) |
| `CAPRUN_GIT_PUSH_TOKEN` | Broker push credential (scrubbed; never in audit) |
| `CAPRUN_GITHUB_TOKEN` | Broker PR bearer (scrubbed) |
| `CAPRUN_GITHUB_API_BASE` | Set by compose-verify to `https://github-mock.caprun.test` |

**Sidecar algorithm (prescriptive):**

1. `Command::new(CARGO_BIN_EXE_caprun).args([...]).env(...).stdout(piped).stderr(piped).spawn()`
2. Background thread reads lines:
   - `session_id=<uuid>` → `caprun grant <uuid> <audit.db>` (once)
   - `effect_id=… sink=git.push` (or list_pending) → `caprun confirm <effect_id> <audit.db>`
   - Do **not** auto-confirm non-push I2 Blocks on LIVE-07 (there should be none)
3. Wait child exit **0**
4. Parse stdout for `Chain verification: PASSED` from main **and** spawn `caprun audit <session> <db>` asserting the same
5. Assert **exactly one** coding session_id owns the multi-node events (no dual-Session stitch)
6. Assert durable terminals: at least `git_push_succeeded`, `github_pr_succeeded` (and earlier sink events as applicable)

**cfg gate:**

```rust
#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]
// body
#[test]
fn live_acceptance_v1_10_cli_guard_present() { assert!(cfg!(test)); }
```

**Authoritative command:**

```bash
COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun \
  --test live_acceptance_v1_10_cli --features brokerd/mock-egress-ca' \
  bash scripts/compose-verify.sh
```

Full phase gate:

```bash
./scripts/check-invariants.sh
COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test --workspace --no-fail-fast \
  --features brokerd/mock-egress-ca' \
  bash scripts/compose-verify.sh
```

### Pattern 2: External confirm + grant without dual-Session

**What:** Keep one Session by never exiting the worker across push→PR; human/sidecar acts on durable pending rows while worker holds.

**Why:** CONFIRM-01 + DESIGN §3; dual-Session stitch is the v1.9-era product gap this milestone closes.

**Rules:**

- PROCEED only after `ConfirmOutcome::Released` or durable `Confirmed` (already enforced in main)
- Grant is **session-scoped** and may be recorded after `session_id` print / before PR node — concurrent with early nodes is fine
- Never re-open ProvideIntent; never re-bind policy; never remint trusted values mid-stream

### Pattern 3: Framing honesty machine-check (LIVE-07 SC2)

**What:** Make hybrid overclaim fail the test suite, not a prose footnote.

**Prescriptive checks (all required):**

1. **Module doc** bluntly states: multi-node SUCCESS is driven by real `caprun run safe-coding-workflow` one Session; hybrid v1.9 composition is **not** this claim (invert 46-MILESTONE-RECORD §1).
2. **Assert driver binary:** test spawns `env!("CARGO_BIN_EXE_caprun")` with argv containing `safe-coding-workflow` (not only in-crate evaluate).
3. **Assert one session:** all multi-node coding events share one `session_id` extracted from CLI stdout / audit.
4. **Negative framing string guard (optional but recommended):** a unit/const test that the LIVE-07 module doc / success test name does **not** contain the LIVE-05 hybrid one-liner as a success claim (or positively asserts a `NON_HYBRID_CLI_MULTI_NODE` constant is referenced by the success test).
5. **COVERAGE / phase summary language:** "CLI multi-node SUCCESS" only after LIVE-07 green; never rebrand `stream_substrate` / v1.9 composed as LIVE-07.

**Anti-pattern:** Calling `evaluate_plan_node_and_record_for_test` for the five coding sinks and labeling that LIVE-07.

### Pattern 4: LIVE-08 genuine mid-loop I2 Block (prescriptive)

**Gap today:** `CodingI2ProofPlanner` lives only in `cli/caprun/tests/planner.rs` and is **never selected by the worker**. Worker planner selection is `CAPRUN_PLANNER=llm` → `LlmPlanner`, else `DeterministicPlanner` (`worker.rs` ~382–388). Worker `env_clear` allowlist does **not** forward arbitrary parent env (`main.rs` ~615–642). Therefore LIVE-08 **cannot** be CLI-driven without a thin product-side selection path.

**Recommended product delta (minimal, default-off):**

1. Move/implement `CodingI2ProofPlanner` in `cli/caprun/src/planner.rs` (same logic as test: steps 0..=3 → Deterministic; step 4 body = `out_1`).
2. Worker: if `CAPRUN_CODING_I2_PROOF=1` **and** intent is `SafeCodingWorkflow`, use proof planner; else Deterministic (llm path unchanged / still fail-closed for coding).
3. Main: if parent has `CAPRUN_CODING_I2_PROOF=1`, forward that single key into worker env allowlist (non-secret; never forward tokens).
4. Document: env is LIVE/proof-only; success path documentation and default unset; success-path unit tests still assert Deterministic never places `out_*`.

**Why body at github.pr (not push refspec by default):**

- Phase 49 already locked expressibility on `github.pr`/`body` [VERIFIED: `coding_i2_proof_places_out_handle`]
- `body` is CONTENT_SENSITIVE → collect-then-Block I2 path [VERIFIED: `sink_sensitivity.rs` `GITHUB_PR_CONTENT_SENSITIVE`]
- Push always-confirm Block would confound "I2 Block vs always-confirm" attribution if the tainted arg is on push; body placement keeps push clean confirm then I2 at PR (true mid-loop)

**Genuine taint chain (must assert unbroken):**

1. Step 1 `process.exec` Allowed → real confined launcher → durable `process_exited`
2. Broker `mint_from_exec` → `output_value_id` with `provenance_chain[0] == process_exited.id`, taint includes ExecRaw/ExternalUntrusted
3. Worker bags under `out_1`
4. Step 4 proof planner places `out_1` into `body`
5. Executor returns `BlockedPendingConfirmation` with anchor on `body` (or content-sensitive path)
6. Durable `sink_blocked` event; **zero** `github_pr_succeeded` / `github_pr_failed` from a completed write (no effect of that node)
7. Policy for the session **permits** `github.pr` (use broker_default or coding minimal policy) so `reason.code() != "policy_deny"`
8. `verify_chain` true

**Do not confirm the I2-blocked PR** in the LIVE-08 success-of-block proof (confirm would intentionally allow the human-gated effect). Leave blocked (exit 3) or explicit deny (exit 2); assert no PR effect either way.

**Optional distinct control (not required by LIVE-08 but good hygiene):** sibling policy_deny leg on an omitted sink — reuse s46 distinct-tag pattern. Only if time; not a substitute for LIVE-08.

### Pattern 5: Genuine vs stapled taint (copy asserts)

From `stream_substrate.rs` Linux taint-via-bag and v1.9 composed exec leg:

```rust
// Source: cli/caprun/tests/stream_substrate.rs taint_via_bag_*; live_acceptance_v1_9_composed exec leg
let exited = find_event_by_type(&conn, &session_id.to_string(), "process_exited")
    .expect("query").expect("process_exited must exist");
let minted = store.resolve(&output_vid).expect("resolve");
assert_eq!(
    minted.provenance_chain[0], exited.id,
    "GENUINE-TAINT (non-stapled): provenance_chain[0] == real process_exited id"
);
assert!(minted.taint.iter().any(|t| t.is_untrusted()));
```

For CLI LIVE-08, prefer audit-DAG edge assertions after the run (events + decision anchors) rather than in-process ValueStore, because the ValueStore lives in the broker process of the CLI run. Use:

- `sink_blocked` event present
- Anchor names `body` (or chosen sensitive arg)
- `read_event_id` / provenance fields on anchor match the session's `process_exited` when exposed in audit
- No stapled UUID invented in the test

If audit payload does not expose full provenance_chain for the blocked value, assert the decision path that only genuine bag routing can produce (Block after real exec event in same Session) + non-stapling invariant already held by mint_from_exec production code, and cite stream_substrate as the unit substrate. Prefer strongest available DAG edge.

### Anti-Patterns to Avoid

- **Hybrid sold as LIVE-07:** in-crate multi-submit labeled CLI multi-step DONE
- **Success-path out_* laundering:** changing `plan_coding_next` to place bag outputs into PR/push args
- **Stapled taint:** minting Untrusted at the sink without real process_exited root
- **policy_deny vacuity:** omitting github.pr from policy so "Block" is really PolicyDeny
- **Confirming the I2 Block then claiming "no effect"**
- **Dual-Session stitch:** push Session A + PR Session B as product path
- **Auto-grant / auto-confirm** inside run
- **cfg-linux blindness:** green macOS host tests ≠ LIVE proof
- **Bare docker run** / named volume `CARGO_TARGET_DIR` cache hack
- **EffectRequest** or new free-form tool map
- **Packaging scope creep** (Phase 52)

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-node orchestration | New workflow crate / batch authorize | Phase 50 `caprun run` + worker stream | Product path already exists |
| Confirm mid-loop | Reconnect-remint / new Wait IPC | CAPRUN_CONFIRM=external + durable confirm | DESIGN §3; occupancy latch |
| Mock GitHub / push | Hand-rolled TLS server in test | compose-verify mock-github sidecar | SSRF public-range + mock-egress-ca already wired |
| I2 enforcement | Policy rule / test double | Real executor + genuine taint | I2 is TCB hardcoded |
| Audit integrity | Custom hash check | `verify_chain` + `caprun audit` | Authenticated DAG already shipped |
| Framing disclosure | Prose-only README | Module doc + asserts on CLI spawn/session | v1.3 DOC-01 / v1.9 §1 discipline |
| Taint placement expressibility | New mint site | Bag `out_1` + proof planner | Gate 3; Phase 49 expressibility |

**Key insight:** Phase 51 is composition + honesty, not invention. The failure mode is re-proving substrate under a hybrid harness and calling it CLI multi-step DONE.

## Runtime State Inventory

> Not a rename/refactor phase — omit full five-category inventory.  
> **Note for planner:** LIVE tests create ephemeral tmp dirs + audit DBs only; no persistent service rename. Mock/Mailpit containers are compose-verify lifecycle (trap teardown). No OS-registered state.

## Common Pitfalls

### Pitfall 1: Hybrid rebrand (LIVE-05 honesty class)
**What goes wrong:** Composed in-crate chain + one `caprun audit` sold as LIVE-07.  
**Why:** Fast path; v1.9 precedent exists.  
**How to avoid:** Require `CARGO_BIN_EXE_caprun` argv `safe-coding-workflow`; forbid evaluate_for_test as SUCCESS driver.  
**Warning signs:** Test never spawns `caprun-worker`; multiple session_ids for coding chain.

### Pitfall 2: LIVE-08 without product-selectable proof path
**What goes wrong:** Unit expressibility claimed as LIVE-08; or hybrid bag test rebranded.  
**Why:** CodingI2ProofPlanner is test-only today.  
**How to avoid:** Env-gated planner selection + real CLI spawn for LIVE-08.  
**Warning signs:** Only `coding_i2_proof_places_out_handle` cited as LIVE-08 evidence.

### Pitfall 3: Always-confirm push treated as failure
**What goes wrong:** Expect five Allowed decisions; fail when push Blocks.  
**Why:** git.push always rewrites Allowed → BlockedPendingConfirmation.  
**How to avoid:** SUCCESS = nodes Allow + push Block + confirm release + PR Allow; exit 0 after STREAM_DONE.  
**Warning signs:** Test asserts `ExecutorDecision::Allowed` for git.push.

### Pitfall 4: Missing grant / missing confirm sidecar
**What goes wrong:** Hang until CAPRUN_CONFIRM timeout (exit 3) or PR denied event without success.  
**Why:** Non-TTY → external mode; grant is not automatic.  
**How to avoid:** Concurrent grant after session_id; confirm only push pending on LIVE-07.  
**Warning signs:** Flaky 300s timeouts; `github_pr_denied` / empty mock pulls.

### Pitfall 5: policy_deny vacuity on LIVE-08
**What goes wrong:** Policy omits github.pr → Denied{PolicyDeny} misread as I2 Block.  
**Why:** Both stop the stream.  
**How to avoid:** Use policy that permits github.pr; assert `sink_blocked` and `code != policy_deny` / no PolicyDeny decision.  
**Warning signs:** Only `plan_node_evaluated` without `sink_blocked`.

### Pitfall 6: Stapled or mid-stream reminted taint
**What goes wrong:** Test mints Untrusted locally and submits as if bag-routed.  
**Why:** Faster than real exec.  
**How to avoid:** Real process.exec in same Session; provenance root on process_exited.  
**Warning signs:** No process_exited event before Block; provenance UUID only in test.

### Pitfall 7: file.write / git fixture gaps
**What goes wrong:** Early stream fail (O_TRUNC missing file; git.commit nothing staged; push empty).  
**Why:** coding_cli host fixtures incomplete for full Linux chain.  
**How to avoid:** Pre-create write target; git init + identity; `git add -A` in test_command; small repo under pack-cap.  
**Warning signs:** Failures at step 0/2 before push hold.

### Pitfall 8: cfg-linux / feature blindness
**What goes wrong:** Host green, compose-verify red (or empty Linux tests).  
**Why:** macOS stubs; mock-egress-ca off by default.  
**How to avoid:** Host guard test + Linux body; always run compose-verify with feature.  
**Warning signs:** "0 passed" treated as regression on macOS.

### Pitfall 9: Confirm flakiness / race
**What goes wrong:** Sidecar confirms wrong effect or before pending row durable.  
**Why:** Poll races; multiple pending possible if mis-routed.  
**How to avoid:** Poll `list_pending` / parse BLOCKED line; confirm exact effect_id; sequential single run.  
**Warning signs:** Intermittent exit 3; ConfirmedButSinkFailed.

### Pitfall 10: Docker networking / mock host
**What goes wrong:** SSRF classifier rejects mock; TLS fails without mock-egress-ca.  
**Why:** Mock on PUBLIC 203.0.113.0/24; feature OFF untrusts cert.  
**How to avoid:** Only compose-verify recipe; never hardcode 127.0.0.1 as push remote.  
**Warning signs:** ssrf_check denials; certificate errors.

## Code Examples

### LIVE-07 driver sketch (test harness)

```rust
// Source: synthesis of coding_cli.rs + live_acceptance_v1_9_composed.rs + main CAPRUN_CONFIRM=external
// Frame: REAL binary multi-node — NOT evaluate_plan_node_and_record_for_test composition.

#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]
#[tokio::test]
async fn live_07_cli_multi_node_one_session_verify_chain() {
    let layout = LiveCodingLayout::new("live07"); // F1 siblings; git repo; intent; policy
    let caprun = env!("CARGO_BIN_EXE_caprun");

    let mut child = std::process::Command::new(caprun)
        .args([
            "run",
            "--policy", layout.policy.to_str().unwrap(),
            "safe-coding-workflow",
            layout.intent.to_str().unwrap(),
            layout.workspace_file.to_str().unwrap(),
            layout.audit_db.to_str().unwrap(),
        ])
        .env("CAPRUN_CONFIRM", "external")
        .env("CAPRUN_CONFIRM_TIMEOUT_SECS", "60")
        .env("CAPRUN_GIT_PUSH_TOKEN", "test-push-token-not-for-audit")
        .env("CAPRUN_GITHUB_TOKEN", "ghp_test_not_for_audit")
        // CAPRUN_GITHUB_API_BASE set by compose-verify; set here if running scoped
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn caprun run safe-coding-workflow");

    // Sidecar: grant on session_id=; confirm on git.push BLOCKED effect_id=
    let audit = layout.audit_db.clone();
    let sidecar = std::thread::spawn(move || {
        drive_external_confirm_and_grant(caprun, &audit, /*stdout reader*/ …)
    });

    let status = child.wait().expect("wait");
    sidecar.join().unwrap();
    assert_eq!(status.code(), Some(0), "LIVE-07 must exit 0 on full SUCCESS");

    // Real audit viewer
    let audit_out = std::process::Command::new(caprun)
        .args(["audit", &session_id, layout.audit_db.to_str().unwrap()])
        .output()
        .expect("caprun audit");
    let text = String::from_utf8_lossy(&audit_out.stdout);
    assert!(text.contains("Chain verification: PASSED"));
    // assert single session; git_push_succeeded; github_pr_succeeded
}
```

### LIVE-08 proof planner selection sketch

```rust
// Source: worker.rs planner selection + planner.rs CodingI2ProofPlanner (tests) — promote default-off
// cli/caprun/src/worker.rs (illustrative)
let planner: Box<dyn Planner> = match (
    std::env::var("CAPRUN_PLANNER").as_deref(),
    std::env::var("CAPRUN_CODING_I2_PROOF").as_deref(),
    &intent,
) {
    (Ok("llm"), _, CaprunIntent::SafeCodingWorkflow { .. }) => {
        anyhow::bail!("SafeCodingWorkflow unsupported on LlmPlanner");
    }
    (Ok("llm"), _, _) => Box::new(LlmPlanner::new(planner_sock?)),
    (_, Ok("1"), CaprunIntent::SafeCodingWorkflow { .. }) => {
        Box::new(CodingI2ProofPlanner) // steps 0..3 success; step 4 body=out_1
    }
    _ => Box::new(DeterministicPlanner),
};
```

```rust
// main.rs worker env allowlist addition (illustrative)
if std::env::var("CAPRUN_CODING_I2_PROOF").as_deref() == Ok("1") {
    worker_cmd.env("CAPRUN_CODING_I2_PROOF", "1");
}
```

### Framing constant

```rust
/// Machine-checkable framing pin (LIVE-07 SC2). Tests reference this symbol so
/// hybrid composition cannot silently become the DONE claim.
const LIVE_07_DRIVER: &str = "caprun run safe-coding-workflow (CLI multi-node, one Session)";
const LIVE_07_NOT: &str = "hybrid in-crate evaluate_plan_node_and_record_for_test composition";
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hybrid multi-sink SUCCESS + one CLI Block leg (LIVE-05) | Full multi-node SUCCESS via CLI one Session (LIVE-07) | v1.10 Phase 51 | Closes honesty gap |
| `caprun run` email/file single-node only | `safe-coding-workflow` multi-node driver | Phase 50 | Product path exists |
| Worker exit-1 on Block | Block-and-Hold PROCEED/ABORT | Phase 50 | Push mid-loop viable |
| Test-only CodingI2ProofPlanner | Env-gated product selection (recommended) | Phase 51 | LIVE-08 CLI-drivable |
| Unit bag taint (stream_substrate) | CLI mid-loop I2 with genuine exec mint | Phase 51 | LIVE-08 DONE bar |

**Deprecated/outdated as DONE claims:**

- "Driven via `caprun run`" meaning only one confined Block leg while SUCCESS is hybrid
- Hybrid `live_acceptance_v1_9_composed` as multi-step CLI proof
- Phase 49 `coding_i2_proof_places_out_handle` as LIVE-08 SUCCESS

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Env-gated `CAPRUN_CODING_I2_PROOF=1` is the preferred LIVE-08 product delta (vs hybrid sibling only) | Pattern 4 | If user rejects any product delta, LIVE-08 must be redesigned as hybrid-with-honest-framing (weaker vs PROJECT.md) |
| A2 | Sibling Session for LIVE-08 (not same Session as LIVE-07) is acceptable under "same proof family" | Discretion | If user requires same Session, proof planner must run inside success Session without laundering success path |
| A3 | github.pr body is preferred LIVE-08 sensitive arg (vs push refspec) | Pattern 4 | Refspec also works (routing-sensitive) but confounds always-confirm attribution |
| A4 | Concurrent grant+confirm sidecar is reliable under compose-verify | Pattern 2 | Flakes → need tighter pending poll or in-process test helper |
| A5 | Mock `/accept/repo.git` + pulls 201 remain sufficient for coding SUCCESS | EXISTS table | Mock route drift breaks LIVE-07 only on compose-verify |
| A6 | No new DESIGN-20 re-trace required if only default-off planner selection + tests (no stream/confirm/mint pivot) | DESIGN §13 | If confirm-hold or mint path changes, orchestrator must re-run adversarial trace |

**If wrong on A1:** planner must checkpoint with user before coding proof-planner selection.

## Open Questions (RESOLVED)

> Locked at plan-time under `/gsd-plan-phase 51 --auto` (no CONTEXT.md; RESEARCH discretion recommendations accepted as planning defaults). Plans 51-01/51-02 bake these in.

1. **Does LIVE-08 require any product code change?**
   - What we know: worker cannot select CodingI2ProofPlanner today; env allowlist strips parent env.
   - **RESOLVED (A1):** Yes — promote env-gated default-off product delta: `CAPRUN_CODING_I2_PROOF=1` selects `CodingI2ProofPlanner` (main allowlist forward + worker select). Document as non-operator proof feature. Hybrid multi-leg is **not** the LIVE-08 DONE claim.

2. **How much provenance can `caprun audit` surface for LIVE-08 anchors?**
   - What we know: sink_blocked + decision anchors exist; full ValueStore is broker-internal.
   - **RESOLVED:** Assert strongest DAG-visible fields + `process_exited` event presence in the same Session before Block; cite production `mint_from_exec` path. Do not require full ValueStore dump in audit CLI.

3. **Should LIVE-07 assert mock receipt ledgers (push/PR) like v1.9?**
   - What we know: mock server records receipts; v1.9 sometimes asserts via broker events only.
   - **RESOLVED:** Prefer durable audit terminals (`git_push_succeeded`, `github_pr_succeeded`); mock receipts optional strengthening only — not required for LIVE-07 Complete.

4. **Docker/Colima availability on this research host**
   - What we know: docker/colima **unavailable** on the research host at research time.
   - **RESOLVED:** Implement tests + host guards on any host; LIVE Complete only after `scripts/compose-verify.sh` green on a Docker-capable machine. Host-only green ≠ LIVE-07/08 Complete. If compose `rust:1` lacks `git`, install in the COMPOSE_VERIFY_CMD recipe (Wave 0).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo | Build + tests | ✓ | cargo 1.97.1 / rustc 1.97.1 | — |
| `scripts/check-invariants.sh` | HYG gates | ✓ | present | — |
| `scripts/compose-verify.sh` | LIVE-07/08 authority | ✓ script | present | **Cannot substitute host-only** for LIVE claims |
| `scripts/mailpit-verify.sh` | Workspace regression (SMTP legs) | ✓ script | present | compose-verify already includes Mailpit |
| Docker / Colima | compose-verify execution | ✗ on research host | — | Implement + host-guard here; run LIVE on Docker host |
| mock-github assets | Push/PR TLS | ✓ | `scripts/mock-github/` | — |
| `brokerd/mock-egress-ca` feature | Trust mock cert | ✓ in workspace | non-default | Must pass `--features brokerd/mock-egress-ca` |
| git binary in test container | git.commit / push fixtures | ✓ in rust:1 typical | — | compose image has git or install in recipe if missing — **verify in Wave 0** |
| pkg-config / libssl | lettre native-tls in full suite | via compose/mailpit scripts | — | scripts install inside container |

**Missing dependencies with no fallback for LIVE claims:** Docker/Colima on the machine that will **green** LIVE-07/08 (research host lacks them — execution environment must provide).

**Missing dependencies with fallback:** host unit/argv/framing tests without Docker; not sufficient for LIVE DONE.

## Validation Architecture

> `workflow.nyquist_validation` absent in `.planning/config.json` → treat as **enabled**.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (workspace), bin crate `caprun`, lib tests in `brokerd`/`executor` |
| Config file | workspace `Cargo.toml` (no jest/pytest) |
| Quick run command | `./scripts/check-invariants.sh && cargo test -p caprun --test coding_cli --test stream_hold --test stream_substrate --test planner -- --test-threads=1` |
| Full suite command (Linux authority) | `./scripts/check-invariants.sh && COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test --workspace --no-fail-fast --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh` |
| Scoped LIVE command | `COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_10_cli --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LIVE-07 | Real CLI multi-node coding SUCCESS one Session | e2e Linux+mock-egress-ca | `cargo test -p caprun --test live_acceptance_v1_10_cli live_07 --features brokerd/mock-egress-ca` | ❌ Wave 0 |
| LIVE-07 | `caprun audit` Chain verification PASSED | e2e | same test asserts audit subprocess | ❌ Wave 0 |
| LIVE-07 | Framing: CLI-driven not hybrid | unit/e2e | framing asserts + module doc pin | ❌ Wave 0 |
| LIVE-07 | Host guard binary present | unit host | `live_acceptance_v1_10_cli_guard_present` | ❌ Wave 0 |
| LIVE-08 | Mid-loop I2 Block via genuine bag taint under permitted sink | e2e Linux+mock-egress-ca | `… live_08 …` | ❌ Wave 0 |
| LIVE-08 | policy_deny is not what fired; no effect of blocked node | e2e | assert sink_blocked + no github_pr_succeeded | ❌ Wave 0 |
| LIVE-08 | verify_chain true after Block | e2e | in-process +/or audit | ❌ Wave 0 |
| LIVE-07/08 | No v1.0–v1.9 regression | full workspace | compose-verify default full suite | ✅ harness / ❌ until green run |
| HYG | check-invariants Gates 1–6 | script | `./scripts/check-invariants.sh` | ✅ |
| Regression | Phase 50 coding argv / hold / planner | unit | `cargo test -p caprun --test coding_cli --test stream_hold --test stream_substrate --test planner` | ✅ |
| Regression | v1.9 hybrid still green (not DONE claim) | e2e Linux | `… --test live_acceptance_v1_9_composed …` | ✅ |

### Sampling Rate

- **Per task commit:** `./scripts/check-invariants.sh` + host-safe caprun tests (coding_cli/stream_hold/planner)
- **Per wave merge:** host-safe suite + (when Docker available) scoped `live_acceptance_v1_10_cli` via compose-verify
- **Phase gate:** full compose-verify workspace green + invariants + framing honesty review

### Wave 0 Gaps

- [ ] `cli/caprun/tests/live_acceptance_v1_10_cli.rs` — LIVE-07 SUCCESS + LIVE-08 I2 + framing + host guard
- [ ] Shared helpers: F1 layout, git repo fixture, intent/policy fixtures, external confirm/grant sidecar
- [ ] Product delta (A1 RESOLVED): `CodingI2ProofPlanner` in `planner.rs` + worker selection + main env forward
- [ ] `51-VALIDATION.md` / COVERAGE honesty: LIVE claim only after compose-verify green
- [ ] Confirm `git` available inside compose-verify rust:1 container (install step if missing)
- [ ] Framework install: none — cargo test already present

*(If Docker unavailable during implementation: complete Wave 0 code + host guards; do not mark LIVE-07/08 Complete until compose-verify green.)*

## Security Domain

> `security_enforcement` not disabled in config → included.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | partial | github.pr grant + broker-held tokens (`CAPRUN_GITHUB_TOKEN`); not end-user auth product |
| V3 Session Management | yes | One Session continuity; occupancy latch; no dual-Session stitch |
| V4 Access Control | yes | POLICY-03 bind once; grant gate; kernel confinement Landlock/seccomp |
| V5 Input Validation | yes | Closed CaprunIntent JSON; fail-closed argv; BiDi neutralization on audit display |
| V6 Cryptography | yes | Audit MAC `verify_chain`; no hand-rolled crypto; ring-only egress stack |

### Known Threat Patterns for this LIVE phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Hybrid overclaim / false DONE | Spoofing (claims) | Framing asserts; CLI spawn required for LIVE-07 |
| Cross-node taint laundering via out_* | Tampering / Elevation | Success path never places out_*; proof path deliberate + I2 Blocks |
| policy_deny vacuity | Spoofing (attribution) | Permitting policy on I2 leg; distinct sink_blocked tag |
| Stapled taint | Tampering | mint_from_exec rooted on process_exited; DAG edge asserts |
| Confirm TOCTOU / double-dispatch | Tampering | Durable pending snapshot; no re-submit; single-shot confirm |
| Credential leak into audit | Info disclosure | Tokens broker-env only; absence asserts optional |
| Session split after push | Elevation | Block-and-Hold same Session |
| EffectRequest free-form path | Elevation | Gate 1 check-invariants |
| SSRF to mock/internal | Elevation | resolve-and-pin + public-range mock only under mock-egress-ca |

## What to Plan — Wave / Plan Split (prescriptive)

### Recommended: 2 plans (mirror Phases 49–50)

**Wave 1 — 51-01 Tracer (LIVE-07 + harness)**  
Goal: non-hybrid SUCCESS path green under compose-verify (or maximally complete with host guard if Docker blocked mid-impl).

Tasks should cover:

1. `live_acceptance_v1_10_cli.rs` scaffold: host guard, module framing docs, F1+git fixtures, policy+intent for mock hosts  
2. External confirm+grant sidecar helper  
3. LIVE-07 test: real `caprun run safe-coding-workflow` → exit 0 → one Session → `caprun audit` PASSED → terminal events  
4. Framing asserts (CLI driver, not hybrid)  
5. Invariants green; host regressions green  

**Wave 2 — 51-02 Expansion (LIVE-08 + regression gate)**  
Goal: mid-loop I2 Block attributable + full workspace compose-verify.

Tasks should cover:

1. Product delta: `CodingI2ProofPlanner` + `CAPRUN_CODING_I2_PROOF=1` worker selection + main allowlist forward (default off)  
2. LIVE-08 test: sibling CLI run with proof env; genuine taint; policy permits; no effect; verify_chain; not policy_deny  
3. Anti-launder regression still green (success path never out_*)  
4. Full compose-verify workspace + check-invariants  
5. COVERAGE / phase notes: honest LIVE claims; no packaging  

### Alternative: 3 plans if Docker iteration is slow

- 51-01 harness + framing host tests  
- 51-02 LIVE-07 only on compose-verify  
- 51-03 LIVE-08 + full regression  

Prefer 2 plans if Docker is available to the executor.

### Planner must-not list (copy into plans)

- Do not claim LIVE-07 via `evaluate_plan_node_and_record_for_test` multi-leg composition  
- Do not place `out_*` on DeterministicPlanner success path  
- Do not auto-grant / auto-confirm / session-wide waiver  
- Do not dual-Session stitch push→PR  
- Do not add crates / EffectRequest / new mint sites  
- Do not implement PKG-01 packaging  
- Do not re-run DESIGN-20 unless stream/confirm/mint pivots (A6)  
- Do not mark ROADMAP Phase 51 complete from executor (orchestrator-owned)

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` — LIVE-07/08 text  
- `.planning/ROADMAP.md` — Phase 51 success criteria  
- `.planning/STATE.md` — Phase 50 complete; Phase 51 next; locked decisions  
- `.planning/PROJECT.md` — v1.10 non-hybrid DONE intent  
- `planning-docs/DESIGN-multi-step-plan-stream.md` — LIVE DONE pin; hybrid not DONE  
- `.planning/research/SUMMARY.md` — v1.10 research basis  
- `.planning/phases/50-*/50-RESEARCH.md`, `50-VERIFICATION.md`, `50-02-SUMMARY.md` — product path  
- `.planning/phases/49-*/` — CodingI2ProofPlanner expressibility  
- `.planning/milestones/v1.9-phases/46-*/46-RESEARCH.md`, `46-MILESTONE-RECORD.md` — hybrid honesty to close  
- `cli/caprun/src/{main,worker,planner,stream_hold}.rs` — code anchors  
- `cli/caprun/tests/{coding_cli,stream_substrate,planner,live_acceptance_v1_9_composed}.rs`  
- `crates/brokerd/tests/s46_negative_legs_composed.rs`  
- `scripts/{compose-verify,mailpit-verify,check-invariants}.sh`, `scripts/mock-github/server.py`  
- `CLAUDE.md` — Linux verification + effect path + DONE lineage  
- Local toolchain: cargo 1.97.1 / rustc 1.97.1  

### Secondary (MEDIUM confidence)

- DESIGN §13 re-trace triggers applied to default-off planner selection (A6 interpretation)

### Tertiary (LOW confidence)

- git package presence inside default `rust:1` compose image (must verify Wave 0)

## Metadata

**Confidence breakdown:**

- Standard stack: **HIGH** — zero new packages; all in-tree, verified at HEAD  
- Architecture: **HIGH** — product path + hybrid gap + proof-planner gap verified in code  
- Pitfalls: **HIGH** — drawn from v1.9 LIVE, Phase 50 research, and code allowlist/env constraints  
- LIVE-08 product delta: **HIGH** after A1 locked-for-planning (env-gated planner; hybrid not DONE)

**Research date:** 2026-07-29  
**Valid until:** ~30 days (stable substrate; LIVE composition patterns)

---

## RESEARCH COMPLETE

**Phase:** 51 - Non-hybrid LIVE Proof (v1.10 DONE)  
**Confidence:** HIGH  

### Key Findings

1. **Phase 50 product path is ready** — `caprun run safe-coding-workflow` + Block-and-Hold + exit taxonomy 0/2/3/1; LIVE-07 is composition, not invention.
2. **LIVE-07 honesty bar** — SUCCESS must spawn real multi-node CLI one Session; v1.9 hybrid `live_acceptance_v1_9_composed` is regression-only, not DONE evidence.
3. **LIVE-08 gap** — `CodingI2ProofPlanner` is test-only; worker cannot select it until a default-off env path is added (recommended `CAPRUN_CODING_I2_PROOF=1` + allowlist forward).
4. **Automation shape** — `CAPRUN_CONFIRM=external` + concurrent `caprun grant`/`confirm` sidecar; mock push/PR via compose-verify + `mock-egress-ca`.
5. **Authoritative gate** — full-workspace `scripts/compose-verify.sh`; research host lacks Docker — LIVE green requires Docker-capable execution.

### File Created

`.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-RESEARCH.md`

### Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | Zero new deps; verified workspace tools |
| Architecture | HIGH | Code anchors for CLI, hold, proof gap, mocks |
| Pitfalls | HIGH | Lineage from LIVE-05 hybrid + Phase 50 hold pitfalls |

### Open Questions (RESOLVED)

- **A1 RESOLVED:** env-gated `CAPRUN_CODING_I2_PROOF=1` proof planner (not hybrid-framed LIVE-08 DONE)
- **Provenance RESOLVED:** strongest DAG-visible fields + `process_exited` + production `mint_from_exec`
- **Mock receipts RESOLVED:** optional; durable audit terminals sufficient
- **Docker / git RESOLVED:** host implement + guard anywhere; LIVE Complete only after compose-verify; install `git` in recipe if missing

### Ready for Planning

Research complete. Plans created: 51-01 LIVE-07 harness + framing; 51-02 LIVE-08 + full regression.
