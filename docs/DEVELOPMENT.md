<!-- generated-by: gsd-doc-writer -->
# Development Guide

AgentOS is a single Rust Cargo workspace. This guide covers the contributor
workflow: local setup, build commands, crate layout, and the hard architectural
rules that every contributor must understand before writing a line of code.

See also: [README](../README.md) · [Architecture](ARCHITECTURE.md) · [Configuration](CONFIGURATION.md)

---

## Prerequisites

- **Rust** (stable, edition 2021). Install via `rustup`.
- **Linux (Ubuntu)** for all security-relevant work. The dev machine may be a
  Mac, but all v0 security claims — Landlock, seccomp-bpf, `no_new_privs`,
  rlimits — are Linux-kernel features. macOS code paths are
  `#[cfg(not(target_os = "linux"))]` stubs. See [Running Linux-only tests](#running-linux-only-tests).
- **Colima + Docker** (Mac only) to run the Linux security tests locally.
- **SQLite** is bundled via the `rusqlite` `bundled` feature — no system
  install required.

---

## Workspace Layout

The repo root is the Cargo workspace (`resolver = "3"`, edition 2021).

```
AgentOS/
  Cargo.toml              # workspace root
  scripts/
    check-invariants.sh   # architectural gate — run before any code
  planning-docs/
    PLAN.md               # single source of truth (wins on all conflicts)
    DESIGN-taint-model.md # hard gate: must exist before crates/executor
    DESIGN-plan-executor.md
    DESIGN-GATE-RECORD.md
  crates/
    runtime-core/         # pure domain types — no I/O, no async, no network
    sandbox/              # security boundary: namespaces, Landlock, seccomp, rlimits
    brokerd/              # reference monitor: Session lifecycle, SQLite audit DAG, UDS IPC
    adapter-fs/           # only path to fs effects: SCM_RIGHTS fd-pass
    executor/             # deterministic I2 enforcement (gated — see below)
  cli/
    caprun/               # orchestrator binary
  .planning/              # GSD planning state (PROJECT.md, ROADMAP.md, STATE.md, phases/)
  docs/                   # project documentation
```

### Crate roles

| Crate | Security role | Key constraint |
|-------|---------------|----------------|
| `runtime-core` | Pure domain types: `Intent`, `Session`, `Effect`, `Artifact`, `Event`, `ValueNode`, `PlanNode`, `ExecutorDecision` | **No I/O, no async, no network** — enforced by Gate 2 |
| `sandbox` | The security **boundary** — namespaces, Landlock, seccomp, default-deny net | Workers self-confine after connecting to broker |
| `brokerd` | Reference monitor / control plane (NOT the boundary) — Session lifecycle, SQLite audit DAG (SHA-256 hash chain), UDS IPC | — |
| `adapter-fs` | The only path to filesystem effects: broker opens workspace files and passes fd via `SCM_RIGHTS` | — |
| `executor` | Deterministic non-LLM I2 enforcement — the security differentiator | **Gated by two DESIGN docs** — see below |
| `cli/caprun` | Orchestrator: starts broker, creates Session, spawns `caprun-worker` | — |

---

## Build Commands

```bash
# Build the full workspace
cargo build --workspace

# Run all tests (use --no-fail-fast so all crates run even if one fails)
cargo test --workspace --no-fail-fast

# Run a single crate / single test target
cargo test -p brokerd audit_dag

# Run the architectural invariant gate (run this before any code change)
./scripts/check-invariants.sh
```

The invariant gate script is grep-based and structural — it runs before any
code executes and is part of the contribution workflow.

---

## Running Linux-only Tests

Security enforcement tests (`#[cfg(target_os = "linux")]`) show as "0 passed"
on macOS — that is **expected**. Do not remove the `cfg` gates.

To run them from a Mac via Colima:

```bash
colima start
docker run --rm \
  --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 \
  cargo test --workspace --no-fail-fast
```

`--security-opt seccomp=unconfined` is required because the default Docker
seccomp profile blocks `landlock()` and `seccomp()` syscalls. Do not use
`--privileged` — the confinement stack is fully unprivileged. Landlock requires
kernel >= 5.13.

---

## Architectural Invariant Gate

**Run `./scripts/check-invariants.sh` before committing any code.** The gate
exits non-zero on any violation and must pass before work is considered
complete.

### Gate 1 — No raw effect-to-sink type under `crates/`

The token `EffectRequest` must not appear anywhere under `crates/`. Its
presence means a bypass path has been introduced: a raw
`EffectRequest { effect, args: Map }` routed straight to a sink with nowhere
for the executor to stand. The broker effect path takes **plan nodes** from day
one:

```rust
submit_plan_node(session_id, PlanNode {
    sink: SinkId,
    args: Vec<ValueNode>,  // each carries literal + provenance + taint
}) -> ExecutorDecision
```

This API shape is locked (decision `DEC-architectural-lock-plan-nodes`).

**Escape hatch:** If a comment or doc string must mention `EffectRequest` for
historical or explanatory reasons, annotate that line with the inline comment
`planner-discipline-allow`. The gate grep excludes lines containing that token.

```rust
// This is why we do NOT use EffectRequest here. <!-- planner-discipline-allow: EffectRequest -->
```

### Gate 2 — `runtime-core` purity

The file tree under `crates/runtime-core/src/` must contain no occurrence of:
`std::io`, `std::fs`, `std::net`, `tokio`, or `async fn`.

`runtime-core` is pure domain types only. Adding I/O or async to it violates
the layer separation that lets the rest of the stack depend on these types
without pulling in runtime concerns.

---

## Locked Terminology

Public API, docs, comments, and commit messages use exactly these terms
(decision `DEC-terminology`):

| Term | Meaning |
|------|---------|
| `Intent` | What the agent has been asked to accomplish |
| `Session` | A bounded execution context for one Intent |
| `Planner` | The component that proposes Effects |
| `Worker` | The confined process that executes work |
| `Broker` | The reference monitor / control plane |
| `Adapter` | The only path to a class of external effects |
| `Effect` | A proposed action at the planner surface (`Observe`, `MutateReversible`, `CommitIrreversible`) |
| `Artifact` | A produced output (file, patch, result) |
| `Event` | An auditable fact appended to the audit DAG |

**`ExecutionContext`** is an internal Rust struct backing a Session. It must
never appear in the public API.

Do not introduce synonyms, abbreviations, or alternative phrasings in code or
documentation. Using off-spec terms is a constraint violation, not a style
preference.

---

## Design-Gate Docs — Executor Code Is Blocked

Two documents in `planning-docs/` must exist and be reviewed before **any**
code is written in `crates/executor`:

1. `planning-docs/DESIGN-taint-model.md` — dynamic-taint default, hard
   planner/worker split for Tier 3+, and the I0 draft-only rule for Sessions
   seeded from untrusted content.
2. `planning-docs/DESIGN-plan-executor.md` — `ValueNode`, `PlanNode`, sink
   sensitivity, taint propagation, and literal-value confirmation UX.

The gate record is `planning-docs/DESIGN-GATE-RECORD.md`. Both docs are
currently complete and reviewed (Phase 2 done). The `crates/executor` crate
now exists, but any structural change to the executor's I2 enforcement logic
must still be consistent with these design docs.

---

## GSD Planning Workflow

This build is managed with GSD. All state lives in `.planning/`:

| File | Purpose |
|------|---------|
| `PROJECT.md` | Project intent, constraints, locked decisions |
| `ROADMAP.md` | All phases with success criteria and plan lists |
| `STATE.md` | Current phase, progress, accumulated decisions |
| `REQUIREMENTS.md` | Detailed requirements with acceptance criteria |
| `phases/` | Per-plan artifacts (PLAN.md files) for the active phase |

**Source of truth:** `planning-docs/PLAN.md`. On any conflict between docs,
code comments, or `CLAUDE.md`, **PLAN.md wins.**

Current position (as of last update): Phase 4 — Value-Injection Security Demo
— executing. Phases 1, 2, and 3 are complete.

---

## Out-of-Scope Until §9 Passes

The following must not be built until the §9 value-injection acceptance test
passes end-to-end with a genuine taint chain:

- Git / GitHub adapters
- Cedar policy engine (simple TOML rules for sink access are fine; I2 stays in
  the Rust TCB)
- Cross-host delegation, Biscuit crypto
- gVisor / Firecracker
- An LLM planner (a hardcoded/stub planner is sufficient for v0)
- Rich approval-policy learning, undo snapshots, broad effect taxonomy
- Web UI, marketplace, long-term memory, browser control
- Natural-language policy authoring
- Mac / WSL2 platform support (deferred post-v0)

**v0 DONE is one thing:** §9 passes on a kernel-confined worker whose only
egress is broker-mediated plan nodes, with genuine taint propagation verified
as an unbroken edge in the audit DAG. Substrate complete does not equal v0
done. Taint stapled on at the sink instead of propagated through the DAG means
the demo fails and proves nothing.

---

## Code Style

No linter or formatter configuration file is present in the repository. Use
`cargo fmt` and `cargo clippy --workspace` as standard Rust toolchain
conventions. Contributions must compile without warnings.

---

## Security Model Reminder

Before making any change that touches the effect path:

- **I2 is non-bypassable.** Policy files may gate which sinks are callable;
  they cannot disable I2. The sink sensitivity map is hardcoded in the Rust
  TCB in `crates/executor`.
- **I1:** no LLM context simultaneously holds untrusted content and authority
  for irreversible effects.
- **I0:** a Session seeded from external/untrusted content starts draft-only
  and cannot auto-authorize Tier 3+ effects.

See `docs/ARCHITECTURE.md` for the full security model and layer roles.
