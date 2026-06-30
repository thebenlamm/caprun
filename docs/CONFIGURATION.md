<!-- generated-by: gsd-doc-writer -->
# Configuration

AgentOS / caprun has a small, intentionally constrained configuration surface. Confinement parameters (rlimits, Landlock ABI, seccomp rules) are hardcoded in the Rust TCB — they are not swappable via config files. The configurable surface is limited to CLI arguments, worker environment variables, and the audit database path.

---

## caprun CLI Arguments

```
caprun <workspace-file> [audit-db-path]
```

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `<workspace-file>` | Yes | — | Path to the workspace file the confined worker will read. The broker opens this file and passes the file descriptor to the worker via `SCM_RIGHTS`. |
| `[audit-db-path]` | No | `:memory:` | SQLite audit database path. Pass a filesystem path (e.g., `audit.db`) to persist the audit DAG across runs. Defaults to an in-process ephemeral database. |

**Example — ephemeral audit DB (default):**
```bash
./target/debug/caprun ./my-workspace.txt
```

**Example — persistent audit DB:**
```bash
./target/debug/caprun ./my-workspace.txt audit.db
```

Both `audit.db` and `runtime.db` are gitignored and must not be committed.

---

## Worker Environment Variables

`caprun` spawns `caprun-worker` and injects these environment variables. They are internal to the caprun orchestration protocol — they are not consumed by end users directly.

| Variable | Set by | Description |
|----------|--------|-------------|
| `BROKER_SOCK` | caprun | Abstract UDS socket path without the leading NUL byte (e.g., `/agentos/<session-id>`). The worker prepends `\0` before connecting. |
| `SESSION_ID` | caprun | UUID of the current broker session. |
| `WORKSPACE_FILE` | caprun | Path to the workspace file forwarded from the caprun CLI argument. |

These variables are set programmatically in `cli/caprun/src/main.rs` and read in `cli/caprun/src/worker.rs`. Do not set them manually unless running the worker binary in isolation for debugging.

---

## Audit Database

The audit database is a SQLite file (or `:memory:` for ephemeral runs) managed by the `brokerd` crate. The `rusqlite` dependency uses the `bundled` feature — no system SQLite installation is required.

| Setting | Value |
|---------|-------|
| Format | SQLite 3 (STRICT tables) |
| Journal mode | WAL (enabled at open time) |
| Default path | `:memory:` (in-process, no file written) |
| Persistent path | Any writable filesystem path passed as `[audit-db-path]` |
| Gitignored filenames | `audit.db`, `runtime.db` |

The database schema (`sessions` and `events` tables) is initialized on every `open_audit_db` call via `CREATE TABLE IF NOT EXISTS`. Passing `:memory:` produces an ephemeral database suitable for tests.

---

## Hardcoded Confinement Parameters

The following confinement values are hardcoded in the Rust TCB (`crates/sandbox/`). They are **not** configurable at runtime. Changing them requires editing the source and rebuilding.

### Resource Limits (`crates/sandbox/src/rlimits.rs`)

| Limit | Value | syscall |
|-------|-------|---------|
| `RLIMIT_AS` | 512 MiB virtual address space | `setrlimit(2)` |
| `RLIMIT_CPU` | 30 CPU seconds | `setrlimit(2)` |

### Landlock (`crates/sandbox/src/landlock.rs`)

| Setting | Value |
|---------|-------|
| Target ABI | `ABI::V3` (Linux ≥ 5.19); the `landlock` crate negotiates down to `ABI::V1` on older kernels |
| Minimum kernel (ABI::V1) | Linux ≥ 5.13 |
| Allow-rules | None — deny-all filesystem |
| Abstract UDS sockets | Unaffected (not in the filesystem namespace) |

### seccomp-bpf (`crates/sandbox/src/seccomp.rs`)

| Setting | Value |
|---------|-------|
| Denied syscalls | `execve`, `execveat` (unconditional); `socket(AF_INET, ...)`, `socket(AF_INET6, ...)` |
| Denial action | `EPERM` |
| Default action for all other syscalls | `Allow` |
| `PR_SET_NO_NEW_PRIVS` | Set automatically by `seccompiler::apply_filter` — no separate `prctl` call needed |

### Confinement Application Order

Confinement is applied by the worker on itself after connecting to the broker. Order is mandatory:

1. `apply_rlimits()` — set `RLIMIT_AS` + `RLIMIT_CPU`
2. `deny_all_filesystem()` — Landlock deny-all
3. `apply_worker_filter()` — seccomp-bpf + sets `NO_NEW_PRIVS`

On macOS (and other non-Linux targets) every confinement function is a no-op stub returning `Ok(())`. Confinement is a Linux-only security claim.

---

## Cargo Build Configuration

### Workspace

| Setting | Value |
|---------|-------|
| Workspace resolver | `"3"` |
| Edition | 2021 |
| License | `MIT OR Apache-2.0` |

### Platform-Gated Dependencies (`crates/sandbox/Cargo.toml`)

The sandbox crate pulls in confinement dependencies only on Linux:

```toml
# crates/sandbox/Cargo.toml — versions are inherited from the workspace root
[target.'cfg(target_os = "linux")'.dependencies]
landlock    = { workspace = true }   # 0.4.5  (pinned in root [workspace.dependencies])
seccompiler = { workspace = true }   # 0.5.0
nix         = { workspace = true }   # 0.31.3
libc        = "0.2"
```

On macOS these crates are not compiled. No user-selectable Cargo `[features]` are defined in any workspace crate.

### Build Commands

```bash
# Build all workspace crates and binaries
cargo build --workspace

# Run tests (macOS: Linux-gated tests show "0 passed" — expected)
cargo test --workspace --no-fail-fast

# Run a single crate's test target
cargo test -p brokerd audit_dag

# Architectural invariant gate (run before code changes)
./scripts/check-invariants.sh
```

### IPC Message Size Limit

The broker enforces a maximum IPC message size of **64 KiB** (`MAX_MSG_SIZE` constant in `cli/caprun/src/main.rs`). Messages exceeding this limit receive an error response and close the connection.

---

## Platform Requirements

| Requirement | Minimum | Notes |
|-------------|---------|-------|
| Operating system (enforcement) | Linux ≥ 5.13 | Landlock ABI::V1 minimum. ABI::V3 (full feature set) requires Linux ≥ 5.19. |
| Root / elevated privileges | None | The confinement stack is fully unprivileged. |
| Rust toolchain | Recent stable | No version is pinned (`Cargo.toml` uses edition 2021); any recent stable `rustup` toolchain builds the workspace. |
| Operating system (build/compile) | macOS or Linux | Cross-compiles cleanly; confinement code is cfg-gated. |

---

## Cross-OS Testing with Colima and Docker

Linux-only security tests (Landlock, seccomp, e2e confinement) must run inside a Linux container from macOS. The standard recipe from `CLAUDE.md`:

```bash
# Start Colima (one-time per session)
colima start

# Run full workspace tests in a Linux container
docker run --rm \
  --security-opt seccomp=unconfined \
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \
  rust:1 \
  cargo test --workspace --no-fail-fast
```

| Docker option / env var | Purpose |
|-------------------------|---------|
| `--security-opt seccomp=unconfined` | Required. Docker's default seccomp profile blocks `landlock()` and `seccomp()` syscalls. `unconfined` allows them. |
| `-e CARGO_TARGET_DIR=/tmp/lt` | Keeps Linux build artifacts inside the container (`/tmp/lt`), separate from the macOS host `target/` directory. |
| `--privileged` | **Not used.** The confinement stack does not require root or elevated container privileges. |
| `rust:1` | Official Rust Docker image, latest stable 1.x. Unpinned by design; pin to a specific tag (e.g. `rust:1.82`) only if/when CI needs reproducible builds. |

Abstract-namespace UDS sockets (`\0/agentos/<session-id>`) used for broker IPC are Linux-only and function correctly inside the container.
