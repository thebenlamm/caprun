<!-- generated-by: gsd-doc-writer -->
# Configuration

caprun has a small, intentionally constrained configuration surface. Confinement parameters (rlimits, Landlock ABI, seccomp rules) are hardcoded in the Rust TCB — they are not swappable via config files. The configurable surface is limited to CLI arguments, worker environment variables, and the audit database path.

---

## caprun CLI Arguments

```
caprun [run] [--policy <path>] [--seed-from-file <path>] <intent-kind> <intent-param> <workspace-file> [audit-db-path]
caprun confirm <effect_id> [audit-db-path]
caprun deny <effect_id> [audit-db-path]
caprun review <effect_id> [audit-db-path]
caprun grant <session_id> [audit-db-path]
caprun audit <session_id> <audit-db-path>
```

The leading `run` verb is an optional, legible alias for the bare-positional intent-run form — both are accepted and behave identically. `confirm`, `deny`, `review`, `grant`, and `audit` are distinct dispatch verbs, checked before the intent-kind parse; `caprun audit` REQUIRES an explicit `<audit-db-path>` — there is no `:memory:` default for it, because an in-memory database has no persisted chain to verify.

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `<intent-kind>` | Yes (run form) | — | One of `send-email-summary`, `create-file-from-report`, `safe-coding-workflow` — see the intent-kind table below. |
| `<intent-param>` | Yes (run form), unless `--seed-from-file` is given | — | The primary literal for the chosen intent kind (recipient address, file path, or coding-intent JSON path). |
| `<workspace-file>` | Yes | — | Path to the workspace file the confined worker will read. The broker opens this file and passes the file descriptor to the worker via `SCM_RIGHTS`. |
| `[audit-db-path]` | No (Yes for `audit`) | `:memory:` | SQLite audit database path. Pass a filesystem path (e.g., `audit.db`) to persist the audit DAG across runs. The `audit` verb requires an explicit path — there is no in-memory default for it. |
| `--policy <path>` | No | broker default (see "Session policy file" below) | Trusted session policy file path. Takes precedence over `CAPRUN_POLICY` when both are set. |
| `--seed-from-file <path>` | No | — | Reads the primary intent literal from file content instead of the CLI argument. The seed file **replaces** the `<intent-param>` positional entirely — it is never consumed in addition to it. A missing or unreadable seed file is a hard error with no fallback to the positional form. |

### Intent kinds

| Intent kind | `<intent-param>` meaning |
|-------------|---------------------------|
| `send-email-summary` | Recipient email address for the workspace summary. |
| `create-file-from-report` | Path created under the workspace root. |
| `safe-coding-workflow` | Path to a JSON file deserializing to a `CaprunIntent::SafeCodingWorkflow` coding intent. |

**Example — ephemeral audit DB (default), send-email-summary:**
```bash
./target/release/caprun send-email-summary you@example.com ./my-workspace.txt
```

**Example — persistent audit DB, create-file-from-report:**
```bash
./target/release/caprun create-file-from-report report.md ./my-workspace.txt audit.db
```

**Example — with an explicit session policy located outside the workspace:**
```bash
./target/release/caprun --policy ../policy.json send-email-summary you@example.com ./my-workspace.txt audit.db
```

Both `audit.db` and `runtime.db` are gitignored and must not be committed.

### Session policy file

`--policy <path>` is the preferred, runnable way to select a session policy; `CAPRUN_POLICY` is its environment-variable fallback — both feed the SAME `bind_policy` call, and the flag takes precedence when both are set. When neither is supplied, the broker binds its own default: an explicit allowlist of the nine production sinks with no argument constraints (`SessionPolicy::broker_default()`) — never allow-everything.

A minimal policy file admitting only the file/commit sinks:

```json
{
  "allowed_sinks": ["file.create", "file.write", "git.commit"],
  "arg_constraints": {}
}
```

The nine production sink ids, any subset of which may appear in `allowed_sinks`:

```
email.send, file.create, file.write, process.exec,
git.commit, http.request, github.pr, http.request.write, git.push
```

**The policy file must live OUTSIDE the workspace directory being operated on.** The broker refuses to bind a policy file at or beneath the workspace root, because that location is reachable by the confined worker itself (the same reach the worker has to read ordinary workspace files) — binding a worker-writable policy there would let the confined worker widen its own allowlist.

A session policy can only ever *narrow* which sinks and arguments are callable. It can never relax or disable the value-injection (I2) enforcement hardcoded in the Rust trusted computing base.

---

## Operator Configuration Checklist

A design partner's configuration surface has three tiers: what every run needs regardless of sink, what a chosen sink additionally needs, and what to never set. Every `CAPRUN_*` name in this checklist is read by shipped code today — none is aspirational, and none has been silently dropped.

### Tier 1 — Always needed

Every invocation of the `run` form needs these three inputs, regardless of which sinks the intent touches:

| Input | How to set it | Default when unset |
|-------|----------------|----------------------|
| Workspace file | `<workspace-file>` positional | — (required) |
| Audit database | `[audit-db-path]` positional | `:memory:` (ephemeral; `audit` verb requires an explicit path) |
| Session policy | `--policy <path>` (preferred), or `CAPRUN_POLICY` as its fallback | The broker's own default allowlist of the nine production sinks — see "Session policy file" above |

### Tier 2 — Sink-specific credentials and settings

These variables are read by the **broker process only** — they are never forwarded into the confined worker or the confined `process.exec` child (the broker `env_clear()`s the child's environment before layering on only the non-secret vars that sink needs). Every example below uses an angle-bracket placeholder; never write a real credential into a file, command example, log line, or planning artifact. Mint each token at the least scope that lets its sink function — a fine-grained, single-repo GitHub token rather than an account-wide one, for example.

| Variable | Required when | Description |
|----------|----------------|--------------|
| `CAPRUN_GITHUB_TOKEN` | Using the `github.pr` sink, or the `caprun grant` path | Bearer token for the GitHub PR sink and the grant/confirmation flow. Fails closed (no session-widening fallback) if unset when the sink fires. |
| `CAPRUN_GIT_PUSH_TOKEN` | Using the `git.push` sink | Bearer token for the destination-pinned `git.push` sink. |
| `CAPRUN_HTTP_WRITE_TOKEN` | Using the `http.request.write` sink | Bearer token for the write-egress (`POST`/`PUT`) sink, distinct from the read-only `http.request` sink. |
| `CAPRUN_SMTP_HOST` | Using the `email.send` sink | SMTP host. Defaults to `127.0.0.1` if unset. |
| `CAPRUN_SMTP_PORT` | Using the `email.send` sink | SMTP port. Defaults to `1025` if unset. |
| `CAPRUN_SMTP_FROM` | Using the `email.send` sink | Envelope `From:` address. Defaults to `caprun@localhost` if unset. |
| `CAPRUN_GITHUB_API_BASE` | Overriding the GitHub API endpoint | Base URL used by the `github.pr` sink and the grant/confirmation path. Production default is the real GitHub API (`https://api.github.com`); override only to point at an alternate or mock API endpoint (e.g. a verification harness). |
| `CAPRUN_CONFIRM` | Forcing out-of-band confirmation | Set to `external` to force the dual-terminal confirmation poll instead of the interactive TTY prompt. |
| `CAPRUN_CONFIRM_TIMEOUT_SECS` | Tuning the external confirmation wait | How long a pending `CAPRUN_CONFIRM=external` confirmation waits before timing out (exit 3, blocked/incomplete). |

**Placeholder credential exports** (least-scope tokens only — never a real value):
```bash
export CAPRUN_GITHUB_TOKEN='<paste-your-least-scope-token>'
export CAPRUN_GIT_PUSH_TOKEN='<paste-your-least-scope-token>'
export CAPRUN_HTTP_WRITE_TOKEN='<paste-your-least-scope-token>'
```

**Optional LLM planner sidecar (outside the minimal deterministic install path):**

| Variable | Description |
|----------|--------------|
| `CAPRUN_PLANNER` | Set to `llm` to spawn the optional `caprun-planner` sidecar instead of the default deterministic planner. Unset (or any other value) keeps the deterministic path. |
| `CAPRUN_PLANNER_MODEL` | Selects the OpenAI model the sidecar uses, when `CAPRUN_PLANNER=llm`. |
| `OPENAI_API_KEY` | The sole secret the sidecar receives, forwarded only when `CAPRUN_PLANNER=llm`. Not needed for the minimal deterministic workflow this document otherwise describes (D-07). |

### Tier 3 — Internal and test-only (do not set)

An operator should never set any of these for normal operation. They are either set programmatically by the orchestrator when it spawns a subprocess, or exist purely for the test suite.

| Variable | Description |
|----------|--------------|
| `BROKER_SOCK` | Set by `caprun` when it spawns `caprun-worker`: the abstract UDS socket path the worker connects to. |
| `WORKSPACE_FILE` | Set by `caprun` when it spawns `caprun-worker`: the workspace file path forwarded from the CLI argument. |
| `INTENT` | Set by `caprun` when it spawns `caprun-worker`: the serialized typed intent the worker deserializes and acts on. |
| `PRIMARY_SEED_FILE_DERIVED` | Set by `caprun` when it spawns `caprun-worker`: whether the primary intent literal came from `--seed-from-file` (file-derived, tainted) rather than a trusted CLI argument. |
| `PLANNER_SOCK` | Set by `caprun` only when `CAPRUN_PLANNER=llm`: the socket the worker uses to reach the optional LLM planner sidecar. |
| `CAPRUN_CODING_I2_PROOF` | A test-only proof selector for the coding I2 verification suite; requires a `live-proof-fixtures` build. Not meaningful in a normal install. |
| `CAPRUN_ENABLE_IPC_CREATE_SESSION` | An internal gate, default-disabled, guarding a privileged internal session-creation arm. Setting it speculatively widens the attack surface for no operator benefit — normal `caprun run`/`confirm`/`deny`/`review`/`grant`/`audit` usage never needs it. |
| `SESSION_ID` | Legacy name from an earlier CLI shape; not read by any current code path. Retained here only so it is not reintroduced under the mistaken assumption that it still does something. |
| `CAPRUN_ENV_LEAK_SENTINEL_` (prefix) | Used only by the test suite's `env_clear()` leak-detection assertions (`crates/brokerd/src/sinks/process_exec.rs`). Never set outside that test. |

### Worker protocol variables (reference)

The Tier 3 table above already lists every variable `cli/caprun/src/worker.rs` reads (`BROKER_SOCK`, `WORKSPACE_FILE`, `INTENT`, `PRIMARY_SEED_FILE_DERIVED`, `CAPRUN_CODING_I2_PROOF`, `CAPRUN_PLANNER`, `PLANNER_SOCK`) — the broker sets all of them when it spawns the worker or the optional planner sidecar. None of them is operator-settable; they exist purely as the internal contract between `cli/caprun/src/main.rs` and `cli/caprun/src/worker.rs`.

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
