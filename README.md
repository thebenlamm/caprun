<!-- generated-by: gsd-doc-writer -->
# AgentOS

An **Intent Runtime** on stock Linux: a user-space execution layer where agents have no ambient authority, every external effect is authorized against a Session, and confinement is kernel-enforced.

**v0 binary:** `caprun` — **Linux (Ubuntu) only.** All v0 security claims rest on Linux kernel primitives (Landlock, seccomp-bpf, namespaces, pidfd). Mac/WSL2 are deferred post-v0. This is **not** a kernel fork, agent framework, desktop-automation platform, memory product, or marketplace.

---

## Security model

Three invariants, all required for v0:

| Invariant | Guarantee |
|-----------|-----------|
| **I0** (session-creation injection) | A Session seeded from external/untrusted content starts draft-only and cannot auto-authorize Tier 3+ effects. |
| **I1** (instruction injection) | No LLM context simultaneously holds untrusted content and authority for irreversible/external effects. Default = dynamic taint → draft-only. Tier 3+ = hard planner/worker split. |
| **I2** (value injection) | No attacker-tainted value may occupy a sensitive sink argument without literal-value human confirmation. Hardcoded in the Rust TCB (executor) — never a swappable policy file. |

The broker effect path uses **plan nodes** from day one:

```rust
submit_plan_node(session_id, PlanNode {
    sink: SinkId,
    args: Vec<ValueNode>,   // each carries literal + provenance + taint
}) -> ExecutorDecision
```

Introducing a raw `EffectRequest { effect, args: Map }` straight to a sink is architecturally forbidden — `scripts/check-invariants.sh` Gate 1 catches this.

**v0 DONE gate:** The §9 value-injection acceptance test must pass on a kernel-confined worker whose only egress is broker-mediated plan nodes, with a genuine taint chain (raw read `Event` → `ValueNode` → sensitive sink argument → deterministic block) verified as an unbroken edge in the audit DAG. Substrate working ≠ v0 done. Taint stapled on at the sink proves nothing.

---

## Architecture

| Crate | Role |
|-------|------|
| `crates/runtime-core` | Pure types: `Intent`, `Session`, `Effect`, `Artifact`, `Event`, `ValueNode`, `PlanNode`, `ExecutorDecision`. No I/O, no async, no network (enforced by Gate 2). |
| `crates/sandbox` | Security **boundary** — namespaces, Landlock, seccomp, default-deny net, rlimits. Provides the `confine-probe` binary. |
| `crates/brokerd` | Reference monitor / control plane (NOT the boundary) — Session lifecycle, SQLite audit DAG (SHA-256 hash chain), UDS IPC, policy. |
| `crates/adapter-fs` | The only path to fs effects — broker opens workspace files, passes the fd to the worker via `SCM_RIGHTS`. |
| `crates/executor` | (after design-gate docs) Deterministic, non-LLM I2 enforcement — the security differentiator. |
| `cli/caprun` | Orchestrator: starts broker, creates Session, spawns `caprun-worker`. Worker self-confines after connecting to the broker. |

Effect classes at the planner surface (v0):

```
Observe            — read, list, summarize
MutateReversible   — write artifact, apply patch
CommitIrreversible — send, git push, deploy, purchase
```

---

## Build & test

Requires Rust stable (2021 edition, workspace resolver 3).

```bash
# Build everything
cargo build --workspace

# Run all tests (Linux-only security tests require Linux — see below)
cargo test --workspace --no-fail-fast

# Run a single crate / single test target
cargo test -p brokerd audit_dag

# Architectural invariant gate (runs before code; exits non-zero on violation)
./scripts/check-invariants.sh
```

### Linux-only security tests

All enforcement tests (`#[cfg(target_os = "linux")]`) are no-op stubs on macOS. Seeing "0 passed" for the security crates on a Mac is expected — do not remove the `cfg` gates.

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

No `--privileged` needed. Landlock requires kernel ≥ 5.13.

---

## Repository layout

```
AgentOS/
  Cargo.toml              # workspace root (resolver = "3", edition 2021)
  scripts/
    check-invariants.sh   # Gate 1 (EffectRequest absent) + Gate 2 (runtime-core purity)
  crates/
    runtime-core/         # pure types — no I/O
    sandbox/              # security boundary
    brokerd/              # reference monitor
    adapter-fs/           # fs effects via fd-pass
    executor/             # (after DESIGN docs) I2 enforcement
  cli/
    caprun/               # caprun + caprun-worker binaries, e2e tests
  planning-docs/          # PLAN.md (source of truth), DESIGN-*.md
```

**Terminology (locked):** `Intent` · `Session` · `Planner` · `Worker` · `Broker` · `Adapter` · `Effect` · `Artifact` · `Event`. `ExecutionContext` is an internal struct — never in the public API.

---

## Source of truth

`planning-docs/PLAN.md` is the single source of truth. On any conflict between docs, code comments, or this file, PLAN.md wins.

---

## License

MIT OR Apache-2.0
