# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

caprun is an **Intent Runtime** on stock Linux: a user-space execution layer where agents have no ambient authority, every external effect is authorized against a Session, and confinement is kernel-enforced. The project, repo, and v0 binary are all named `caprun`. It is **not** a kernel fork, agent framework, desktop-automation platform, memory product, or marketplace.

**Source of truth:** `planning-docs/PLAN.md` ("caprun v0 — Definitive Plan"). On any conflict between docs, code comments, or this file, **PLAN.md wins.** Background detail lives in `archive/` (excluded from GSD ingest); the canonical security spec is `archive/AGENT-RUNTIME-HANDOVER.md`.

## Build & test

Single Cargo workspace at the repo root (`resolver = "3"`, edition 2021). Crates in `crates/*`, binary in `cli/caprun`.

```bash
cargo build --workspace
cargo test --workspace --no-fail-fast      # --no-fail-fast: else it stops at first failing test binary
cargo test -p brokerd audit_dag            # single crate / single test target
./scripts/check-invariants.sh              # architectural-invariant gate (grep-based, runs before any code)
```

### Linux-only security tests (critical)

All v0 security claims are **Linux-only** (Landlock + seccomp-bpf + no_new_privs + rlimits). The dev machine is a Mac; macOS paths are `#[cfg(not(target_os = "linux"))]` no-op stubs, and all enforcement / negative-assertion / e2e tests are `#[cfg(target_os = "linux")]`. **`cargo test` on macOS shows these as "0 passed" — that is expected, not a gap.** Do not "fix" it by removing the cfg gates.

To actually run them from the Mac (Colima installed):

```bash
colima start
docker run --rm \
  --security-opt seccomp=unconfined \      # required: default profile blocks landlock()/seccomp() syscalls
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \            # keep Linux artifacts out of host target/
  rust:1 \
  cargo test --workspace --no-fail-fast
```

No `--privileged` — the confinement stack is fully unprivileged. Landlock needs kernel ≥5.13.

**Phases needing a local capture SMTP (Mailpit) — e.g. Phase 13, 17:** use
`scripts/mailpit-verify.sh` instead of the bare `docker run rust:1` recipe
above. It starts an `axllent/mailpit` sidecar on a user-defined Docker
network, runs the same unprivileged `rust:1` verification container on that
network (`CAPRUN_SMTP_HOST=mailpit`/`CAPRUN_SMTP_PORT=1025`), additionally
installs `libssl-dev`/`pkg-config` before `cargo test` (required once
`lettre`'s default `native-tls` feature is a build dependency), and tears
down the sidecar afterward. Run: `bash scripts/mailpit-verify.sh`.

## Architecture

Layers, by security role (see PLAN.md "Layer roles"):

| Crate | Role |
|-------|------|
| `runtime-core` | Pure types: `Intent`, `Session`, `Effect`, `Artifact`, `Event`, `ValueNode`, `PlanNode`, `ExecutorDecision`. **No I/O, no async, no network** — enforced by `check-invariants.sh` Gate 2. |
| `sandbox` | The security **boundary** — namespaces, Landlock, seccomp, default-deny net, rlimits. `confine-probe` bin. |
| `brokerd` | Reference monitor / control plane (NOT the boundary) — Session lifecycle, SQLite audit DAG (SHA-256 hash chain), UDS IPC, policy. |
| `adapter-fs` | The only path to fs effects — broker opens workspace files, passes the fd to the worker via `SCM_RIGHTS`. |
| `executor` | (after DESIGN doc) Deterministic, non-LLM I2 enforcement — **the security differentiator**. |
| `cli/caprun` | Orchestrator: starts broker, creates Session, spawns `caprun-worker`. Worker **self-confines after connecting** to the broker (Landlock deny-all + seccomp deny-execve can't precede exec). |

### Security invariants (I0/I1/I2)

- **I1 (instruction injection):** no LLM context holds untrusted content *and* authority for irreversible effects. Default = dynamic taint (reading raw untrusted bytes → draft-only). Tier 3+ = hard planner/worker split.
- **I2 (value injection):** no attacker-tainted value occupies a sensitive sink argument without literal-value human confirmation. **Hardcoded in the Rust TCB (executor) — never a swappable policy file.** Policy may gate *which* sinks are callable; it can never disable I2.
- **I0:** a Session seeded from external/untrusted content starts draft-only and cannot auto-authorize Tier 3+ effects.

### Effect path is locked

The broker effect path takes **plan nodes** from day one: `submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> })`. Each `ValueNode` carries literal + provenance + taint. **Never** introduce a raw `EffectRequest { effect, args: Map }` that goes straight to a sink — that bakes in a bypass with nowhere for the executor to stand. `check-invariants.sh` Gate 1 fails the build if the token `EffectRequest` appears under `crates/` (annotate intentional mentions with `planner-discipline-allow`).

## Hard constraints when working here

1. **v0 DONE = the §9 value-injection acceptance test passing, nothing less.** §9 requires a kernel-confined worker whose only egress is broker-mediated plan nodes, with a **genuine** taint chain (raw read `Event` → `ValueNode` → sensitive sink arg → deterministic block) verified as an unbroken edge in the audit DAG. **Substrate working ≠ v0 done.** Taint stapled on at the sink instead of propagated through the DAG = the demo fails and proves nothing.
2. **Two design-gate docs block executor code:** `planning-docs/DESIGN-taint-model.md` and `planning-docs/DESIGN-plan-executor.md` must exist before any code in the `executor` crate.
3. **Out of scope until §9 holds** (do not build): Git/GitHub adapters, Cedar policy engine, cross-host delegation / Biscuit crypto, gVisor/Firecracker, an LLM planner (a hardcoded/stub planner is sufficient), web UI, marketplace, long-term memory.
4. **Terminology is locked** (public API + docs): `Intent`, `Session`, `Planner`, `Worker`, `Broker`, `Adapter`, `Effect`, `Artifact`, `Event`. `ExecutionContext` is an internal struct backing a Session only — never in the public API. The project, repo, and v0 binary are all named `caprun` (formerly "AgentOS").
5. **TCB is Rust.** Python is allowed for non-TCB experiments only.

## Planning workflow

This build is managed with GSD. State lives in `.planning/` — `PROJECT.md` (intent), `ROADMAP.md` (phases), `STATE.md` (current phase/progress), `REQUIREMENTS.md`. Active phase artifacts are under `.planning/phases/`. The `.claude/` and `.codex/` dirs are regenerable scaffolding (gitignored), not part of the product.
