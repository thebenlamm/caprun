<!-- generated-by: gsd-doc-writer -->
# caprun

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

### Docker cache policy

**Incident (2026-07-20):** two ad hoc named volumes, `caprun-lt` and `caprun-lt-cache`, were manually bound as `CARGO_TARGET_DIR` mounts (e.g. `-v caprun-lt:/tmp/lt`) during a single long verification session to speed up repeated `rust:1` compiles, then never cleaned up. They silently grew to ~16GB combined — two near-duplicate `target/debug` trees — with zero containers referencing them by the time anyone looked. Deleted.

Neither `scripts/mailpit-verify.sh` nor `scripts/compose-verify.sh` creates a named volume today — both always use an **ephemeral** `CARGO_TARGET_DIR` inside a `--rm` container, so nothing in this repo currently causes this growth. This is scoped strictly to this repo's own Docker volumes — it does **not** touch Colima/Lima VM-level state (VM disk size, VM recreation, `colima start`/`stop` flags). The Colima VM itself is shared infrastructure used by other projects on this machine; VM-level disk management is handled separately, at the machine level.

**Policy, if you ever add a persistent build-cache volume for speed:**
- Name it `caprun-<something>` — exactly **one** such volume, never several (duplication, not size, caused the incident).
- It will show up in `scripts/docker-cache.sh status` / `check` automatically (anything matching `caprun-*`).
- Prune it with `scripts/docker-cache.sh clean` when you're done, or before it crosses the cap.

**Tooling:**
```bash
scripts/docker-cache.sh status        # show current caprun-* volumes + sizes
scripts/docker-cache.sh clean         # prune them (prompts first; --yes to skip)
```
`scripts/docker-cache.sh check` runs automatically at the top of both `mailpit-verify.sh` and `compose-verify.sh` — it's a **warn-only** gate (never blocks the run) that fires if any `caprun-*` volume set exceeds `DOCKER_CACHE_WARN_GB` (default 8GB) or if more than one such volume exists, so growth surfaces on the next verification run instead of silently reaching double digits of GB again.

**Pre-commit hook (one-time setup per clone):** git never auto-installs hooks from a tracked directory, so run this once:
```bash
git config core.hooksPath scripts/hooks
```
That activates `scripts/hooks/pre-commit`, which runs `docker-cache.sh check` on every commit — same warn-only behavior, so the check surfaces even if you never manually run a verification script. There is no CI in this repo to wire it into instead (no `.github/workflows`), and the check is inherently about local Docker/Colima state, which a CI runner would never see anyway.

---

## Repository layout

```
caprun/
  Cargo.toml              # workspace root (resolver = "3", edition 2021)
  scripts/
    check-invariants.sh   # Gate 1 (EffectRequest absent) + Gate 2 (runtime-core purity)
    docker-cache.sh       # caprun-* Docker volume retention policy (status/check/clean)
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
