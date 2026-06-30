<!-- generated-by: gsd-doc-writer -->
# Getting Started

This guide takes you from a fresh clone to a running `caprun` substrate demo — the no-LLM complete-mediation proof that generates a verified audit DAG.

---

## Prerequisites

### Rust toolchain

AgentOS is a single Cargo workspace (`resolver = "3"`, edition 2021). Any recent Rust stable release works. There is no `rust-toolchain.toml` or `rust-version` field pinning a specific version.

```bash
# Install via rustup if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

### Linux requirement for the security stack

All v0 security claims rest on Linux kernel primitives (Landlock, seccomp-bpf, abstract-namespace UDS sockets). The end-to-end `caprun` demo **only runs on Linux**.

| Platform | Build | Full demo | Notes |
|----------|-------|-----------|-------|
| Linux ≥ 5.13 | Yes | Yes | Landlock ABI::V1 minimum. ABI::V3 (full feature set) requires Linux ≥ 5.19. No root or elevated privileges required. |
| Linux < 5.13 | Yes | Partial | Landlock unavailable; sandbox crate negotiates down but confinement is incomplete. |
| macOS | Yes | No | Compiles cleanly; abstract-namespace UDS and confinement primitives are Linux-only stubs. See [Running from macOS](#running-from-macos-colima) below. |

---

## Clone and build

```bash
git clone https://github.com/agentos/agentos.git   # <!-- VERIFY: repository remote URL -->
cd AgentOS
cargo build --workspace
```

This builds all workspace crates and produces two binaries in `target/debug/`:

- `target/debug/caprun` — the orchestrator
- `target/debug/caprun-worker` — the self-confining worker (must stay in the same directory as `caprun`)

Run the architectural invariant gate before making code changes:

```bash
./scripts/check-invariants.sh
```

This is a grep-based gate that fails if the `EffectRequest` token appears under `crates/` (Gate 1) or if `runtime-core` imports I/O or async primitives (Gate 2). See [ARCHITECTURE.md](ARCHITECTURE.md) for details on the invariant model.

---

## Running the substrate demo (Linux)

`caprun` takes a workspace file and an optional audit database path:

```
caprun <workspace-file> [audit-db-path]
```

Create a workspace file and run the demo:

```bash
echo "hello from workspace" > /tmp/workspace.txt
./target/debug/caprun /tmp/workspace.txt /tmp/audit.db
```

`caprun-worker` must be present alongside the `caprun` binary. After `cargo build --workspace`, both live in `target/debug/` — run `caprun` from the repo root using the `./target/debug/` path so the binary locates its sibling automatically.

### What happens

1. `caprun` opens the audit DB and creates a Session.
2. It binds an abstract-namespace Unix domain socket at `\0/agentos/<session-id>`.
3. It spawns `caprun-worker` as a normal process (no pre-exec confinement).
4. The worker connects to the socket, then immediately calls `sandbox::apply_confinement()` on itself: rlimits → Landlock deny-all filesystem → seccomp deny-execve + deny-socket(AF_INET/6).
5. The worker requests the workspace file by path; the broker opens it with ambient authority and passes the file descriptor via `SCM_RIGHTS`.
6. The worker reads through the received fd and reports the byte count back to the broker.
7. The broker appends each step as an `Event` to the SQLite audit DAG with an unbroken SHA-256 hash chain.
8. `caprun` prints the audit DAG and verifies the chain.

### Expected output

```
=== Audit DAG (session <uuid>) ===
[0] session_created (actor=broker)
    hash=<8 hex chars> parent=(root)
  [1] fd_granted (actor=broker)
      hash=<8 hex chars> parent=<8 hex chars>
    [2] file_read (actor=worker:<N>)
        hash=<8 hex chars> parent=<8 hex chars>

Chain verification: PASSED
```

`<N>` is the byte count of your workspace file. `Chain verification: PASSED` confirms the SHA-256 hash chain is mathematically unbroken: `session_created` → `fd_granted` → `file_read`.

The three-event sequence is the mediation proof: the worker never called `open()` on the path directly — the file descriptor was brokered, and every step is in the audit DAG.

---

## Running from macOS (Colima)

The Linux-only security tests and the `caprun` demo run inside a Docker container via Colima. `--security-opt seccomp=unconfined` is required because Docker's default seccomp profile blocks the `landlock()` and `seccomp()` syscalls used by the sandbox crate.

```bash
# Start Colima (once per session)
colima start

# Build and run the full test suite inside Linux
docker run --rm \
  --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 \
  cargo test --workspace --no-fail-fast
```

`-e CARGO_TARGET_DIR=/tmp/lt` keeps Linux build artifacts inside the container (`/tmp/lt`), separate from the macOS host `target/` directory.

To run the substrate demo interactively inside the container:

```bash
docker run --rm \
  --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 \
  bash -c 'cargo build --workspace && \
           echo "hello from workspace" > /work/workspace.txt && \
           /tmp/lt/debug/caprun /work/workspace.txt /tmp/audit.db'
```

No `--privileged` flag is needed. The confinement stack is fully unprivileged.

---

## Running tests

```bash
# All workspace tests (macOS: Linux-gated tests show "0 passed" — expected, not a gap)
cargo test --workspace --no-fail-fast

# Single crate or single test target
cargo test -p brokerd audit_dag

# caprun e2e tests (Linux only — skipped silently on macOS)
cargo test -p caprun
```

On macOS, seeing "0 passed" for `crates/sandbox` and `cli/caprun` is expected. The `#[cfg(target_os = "linux")]` gates are intentional. Do not remove them.

---

## Common setup issues

**`caprun-worker` not found at startup**
`caprun` locates `caprun-worker` relative to its own binary path. Run `caprun` via its full `target/debug/caprun` path after `cargo build --workspace`. Do not copy just the `caprun` binary without `caprun-worker`.

**`landlock` syscall blocked in Docker**
The default Docker seccomp profile blocks `landlock()`. Pass `--security-opt seccomp=unconfined` to the `docker run` command. Do not use `--privileged` — it is not needed.

**Kernel too old for Landlock**
Landlock requires Linux ≥ 5.13 (ABI::V1). On older kernels the sandbox crate may return errors at confinement application time. Check your kernel: `uname -r`.

**`Chain verification: FAILED` in output**
This indicates audit DAG corruption — the SHA-256 hash chain has a gap or mismatch. This should not occur in normal operation. File a bug with the full `caprun` stdout/stderr output.

---

## Next steps

- **Architecture and security model:** [ARCHITECTURE.md](ARCHITECTURE.md) — invariants I0/I1/I2, the locked effect path, SCM_RIGHTS fd-pass, taint model, and the §9 v0 DONE gate.
- **Configuration reference:** [CONFIGURATION.md](CONFIGURATION.md) — CLI arguments, worker environment variables, audit DB options, hardcoded confinement parameters, and the Colima/Docker recipe in detail.
- **Source of truth:** `planning-docs/PLAN.md` wins on any conflict between docs, code comments, or this file.
