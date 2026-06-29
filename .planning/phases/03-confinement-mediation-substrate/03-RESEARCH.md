# Phase 3: Confinement & Mediation Substrate — Research

**Researched:** 2026-06-29
**Domain:** Linux kernel confinement (Landlock, seccomp-bpf, rlimits), Unix Domain Socket IPC (tokio), SQLite audit DAG (rusqlite), SCM_RIGHTS fd-pass (nix)
**Confidence:** MEDIUM (core stack verified against official crate registries and docs; Linux kernel API facts confirmed via kernel.org docs)

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-sandbox | `crates/sandbox` — kernel-enforced confinement: CPU, memory, broker UDS, zero ambient fs/net/shell; negative assertions hold | Landlock + seccompiler + prctl(NO_NEW_PRIVS) + rlimits stack documented in §Standard Stack and §Architecture Patterns |
| REQ-brokerd-core | `crates/brokerd` core — Session create, SQLite audit-DAG append, UDS IPC serve | tokio UnixListener + rusqlite + sha2 patterns documented; audit DAG schema in §Code Examples |
| REQ-adapters-fs | `crates/adapters/fs` — filesystem adapter via fd-pass; broker opens file, passes fd via SCM_RIGHTS | nix sendmsg/recvmsg + ControlMessage::ScmRights fully documented; pitfalls in §Common Pitfalls |
| REQ-substrate-demo | End-to-end demo: `caprun` confined worker reads file via passed fd, read Event appears in audit DAG | caprun architecture, pre_exec confinement hook, and demo flow documented in §Architecture Patterns |
</phase_requirements>

---

## Summary

Phase 3 builds three new subsystems in the existing Cargo workspace and fully implements the Phase 1 stubs. The **sandbox crate** applies kernel confinement via Landlock (filesystem restriction), seccomp-bpf (syscall filtering), `prctl(PR_SET_NO_NEW_PRIVS)`, and rlimits (CPU/memory) — all unprivileged, no root, no containers. The **brokerd crate** (expanded from the Phase 1 stub) adds a tokio-async UDS IPC server, Session creation logic, and a hash-linked SQLite audit DAG. The **adapter-fs crate** implements SCM_RIGHTS fd passing via nix so the broker opens a workspace file and transfers the fd to the worker without the worker ever having ambient filesystem access. The **caprun binary** (expanded from the Phase 1 stub) wires all three together for the no-LLM substrate demo.

The critical cross-platform constraint: **all v0 security enforcement claims are Linux-only** (explicitly stated in REQUIREMENTS.md — macOS/WSL2 is deferred post-v0). On macOS (the dev machine), the sandbox crate compiles to a no-op stub gated by `#[cfg(target_os = "linux")]`. CI must run on Linux. All integration tests that test real confinement are gated `#[cfg(target_os = "linux")]`. The planner must plan a dev/CI split: macOS builds and unit tests pass locally; Linux integration tests (negative assertions) run in CI.

The IPC framing design choice with the largest downstream impact: **use abstract-namespace UDS sockets** (path starting with `\0`) rather than path-based sockets. This is the key that lets the worker maintain broker connectivity even after Landlock has fully restricted filesystem access — Landlock governs filesystem operations, not the abstract socket namespace.

**Primary recommendation:** sandbox crate = Landlock + seccompiler + nix(prctl + rlimits); broker IPC = tokio UnixListener + serde_json length-prefixed; audit DAG = rusqlite + sha2; fd-pass = nix sendmsg/recvmsg SCM_RIGHTS; do NOT use birdcage (GPL-3.0 license incompatible with MIT/Apache-2.0).

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Kernel confinement (Landlock, seccomp, rlimits) | `crates/sandbox` | `cli/caprun` (applies it) | Confinement is applied in the forked child process before exec; sandbox crate encapsulates all kernel API calls |
| UDS IPC server (accept connections, route messages) | `crates/brokerd` | — | Broker is the reference monitor; all worker IPC terminates here |
| Session lifecycle (create, update, status) | `crates/brokerd` | `crates/runtime-core` (types) | runtime-core owns the Session struct; brokerd owns persistence and lifecycle transitions |
| Audit DAG (SQLite schema, append, hash-chain) | `crates/brokerd` | — | Audit DAG is broker-owned TCB artifact; no worker or planner can write to it directly |
| fd-pass filesystem adapter (open file, sendmsg SCM_RIGHTS) | `crates/adapter-fs` | `crates/brokerd` (triggers it) | Adapter is called by the broker when a worker requests a file fd; the broker is the mediation point |
| Worker fd receive (recvmsg, read via fd) | Worker binary (future: `crates/worker`) | `cli/caprun` (demo worker) | For Phase 3 demo, caprun includes an inline worker loop or a minimal worker binary |
| Demo orchestration (spawn confined worker, wait, print DAG) | `cli/caprun` | — | caprun is the integration harness for REQ-substrate-demo |
| IPC protocol definition (message types, framing) | `crates/brokerd` | Shared across all crates that speak to broker | Protocol struct types should live in a `brokerd-proto` module or sub-crate accessible to workers |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `nix` | 0.31.3 | Raw Unix APIs: sendmsg/recvmsg (SCM_RIGHTS), prctl, setrlimit, socket primitives | The Rust community standard for `*nix` syscall bindings; 10.9M downloads/wk [VERIFIED: npm registry] |
| `tokio` | 1.52.3 | Async runtime; `tokio::net::UnixListener` for the broker IPC server | The standard Rust async runtime; 14M downloads/wk [VERIFIED: crates.io registry] |
| `rusqlite` | 0.40.1 | SQLite bindings for the audit DAG persistence layer | The standard SQLite wrapper for Rust; 1.77M downloads/wk [VERIFIED: crates.io registry] |
| `landlock` | 0.4.5 | Landlock LSM: filesystem and network access restriction (Linux ≥ 5.13) | Official Rust bindings from the Landlock project team (landlock-lsm/rust-landlock); 342K downloads/wk [VERIFIED: crates.io registry] |
| `seccompiler` | 0.5.0 | Generate and apply seccomp-bpf filters from a rule description | From rust-vmm (Firecracker ecosystem); production-proven in AWS MicroVMs; 319K downloads/wk [VERIFIED: crates.io registry] |
| `sha2` | 0.11.0 | SHA-256 hash for audit DAG hash-chaining | From RustCrypto; 14.3M downloads/wk [VERIFIED: crates.io registry] |
| `serde_json` | 1.0.150 | JSON wire format for UDS IPC messages | Already pinned in workspace.dependencies |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio-util` | 0.7.18 | `LengthDelimitedCodec` for framing IPC messages | Use if implementing the full framed codec; optional if hand-rolling 4-byte LE prefix |
| `cgroups-rs` | 0.5.0 | cgroups v2 CPU/memory limits | Use only if running as root or with systemd-delegated cgroup; prefer rlimits for Phase 3 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `seccompiler` (BPF rule DSL) | raw BPF bytecode via nix | seccompiler eliminates manual BPF assembly while staying in Rust TCB; raw BPF is error-prone |
| rlimits via `nix::sys::resource::setrlimit` | `cgroups-rs` for CPU/memory | rlimits are unprivileged and work in all CI environments; cgroups-rs requires root or systemd delegation |
| Abstract-namespace UDS | Path-based UDS | Abstract sockets bypass Landlock filesystem rules, letting a fully-restricted worker still talk to broker |
| `serde_json` wire format | `bincode` | serde_json is already in workspace deps and human-readable for debugging; bincode saves a few bytes but adds a dep |

### DO NOT USE

| Crate | Reason |
|-------|--------|
| `birdcage` | GPL-3.0 license; **incompatible** with workspace's `MIT OR Apache-2.0` |
| `extrasafe` | SUS verdict (65 downloads/week); low adoption, not suitable for TCB |

### Installation (workspace Cargo.toml additions)

```toml
# Add to [workspace.dependencies]:
nix          = { version = "0.31.3", features = ["fs", "socket", "resource", "process", "signal"] }
tokio        = { version = "1.52.3", features = ["net", "io-util", "rt-multi-thread", "macros"] }
rusqlite     = { version = "0.40.1", features = ["bundled"] }
landlock     = "0.4.5"
seccompiler  = "0.5.0"
sha2         = { version = "0.11.0", features = ["std"] }
serde_json   = "1.0.150"      # already pinned, keep as-is
tokio-util   = { version = "0.7.18", features = ["codec"] }  # optional
```

**Note on `rusqlite` `bundled` feature:** bundles the SQLite C library at compile time. This is correct for Phase 3 — it eliminates the need for a system SQLite and ensures the audit DAG uses a pinned, known-good version.

**Note on crate naming:** REQUIREMENTS.md names the fs adapter as `adapters/fs`. The workspace glob `crates/*` does NOT match `crates/adapters/fs` (two levels deep). Two options:
- **Option A (recommended for Phase 3):** name the crate `crates/adapter-fs` — matches the existing glob, no Cargo.toml change.
- **Option B:** Add `"crates/adapters/*"` to workspace members and create `crates/adapters/fs/` — matches the requirement naming, requires one Cargo.toml edit.

The planner must choose and be consistent across all plans in this phase.

---

## Package Legitimacy Audit

| Package | Registry | Age | Downloads/wk | Source Repo | Verdict | Disposition |
|---------|----------|-----|-------------|-------------|---------|-------------|
| `nix` | crates.io | 12 yrs | 10.9M | github.com/nix-rust/nix | OK | Approved |
| `rusqlite` | crates.io | 12 yrs | 1.77M | github.com/rusqlite/rusqlite | OK | Approved |
| `tokio` | crates.io | 10 yrs | 14.0M | github.com/tokio-rs/tokio | OK | Approved |
| `tokio-util` | crates.io | 8 yrs | 10.3M | github.com/tokio-rs/tokio | OK | Approved |
| `landlock` | crates.io | 5 yrs | 342K | github.com/landlock-lsm/rust-landlock | OK | Approved |
| `seccompiler` | crates.io | 5 yrs | 319K | github.com/rust-vmm/seccompiler | OK | Approved |
| `sha2` | crates.io | 10 yrs | 14.3M | github.com/RustCrypto/hashes | OK | Approved |
| `cgroups-rs` | crates.io | 6 yrs | 77K | github.com/kata-containers/cgroups-rs | OK | Approved (optional) |
| `birdcage` | crates.io | 4 yrs | 2K | github.com/phylum-dev/birdcage | OK by seam, **GPL-3.0** | **REMOVED — license incompatible** |
| `extrasafe` | crates.io | 4 yrs | 65 | github.com/boustrophedon/extrasafe | **SUS** | **REMOVED — low adoption** |

**Packages removed:** `birdcage` (GPL-3.0 license conflict), `extrasafe` (SUS verdict).
**Packages flagged [SUS]:** none remaining after removal.

---

## Architecture Patterns

### System Architecture Diagram

```
 caprun (cli/caprun)
    │
    ├─── spawn_confined_worker() ──────────────────────────────────────────┐
    │      fork → pre_exec: {                                               │
    │        prctl(NO_NEW_PRIVS)                                            │
    │        setrlimit(RLIMIT_AS, 512MB)                                    │
    │        setrlimit(RLIMIT_CPU, 30s)                                     │
    │        landlock::restrict(deny all fs except nothing; abstract UDS ok)│
    │        seccompiler::apply(deny execve, deny AF_INET socket)           │
    │      } → exec(worker_binary)                                          │
    │                                                                        │
    │      Worker Process (confined):                                        │
    │        [no fs] [no net] [no exec] [memory/cpu bounded]                │
    │        ↕ abstract UDS socket "\0/agentos/<session_id>"                │
    │                                                                        │
    └─── brokerd UDS server (tokio UnixListener)  ←──── worker connects ───┘
              │
              ├── handle CreateSession → INSERT sessions → return session_id
              │         └── append Event(session_created) to audit DAG
              │
              ├── handle RequestFd { path } 
              │         ├── broker open(path)  [broker has ambient fs]
              │         ├── append Event(fd_granted) to audit DAG
              │         └── sendmsg(SCM_RIGHTS, fd) → worker receives fd
              │
              └── handle ReportRead { bytes_read }
                        └── append Event(file_read, taint=[]) to audit DAG
                                                        ↑
                                                 proves complete mediation
    SQLite Audit DAG (brokerd-owned):
    ┌─────────────────────────────────────────────────────┐
    │  events table (hash-linked chain)                   │
    │  session_created → fd_granted → file_read           │
    │  each row: id, parent_id, hash, parent_hash, ...    │
    └─────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
crates/
├── runtime-core/        # existing — domain types (Event, Session, PlanNode, etc.)
├── brokerd/             # existing stub → EXPAND: UDS server + SQLite DAG + fd-pass orchestration
│   └── src/
│       ├── lib.rs           # submit_plan_node stub (kept from Phase 1)
│       ├── server.rs        # tokio UnixListener IPC server
│       ├── session.rs       # Session create/update logic
│       ├── audit.rs         # SQLite audit DAG: schema, append, hash-chain
│       └── proto.rs         # IPC message types (shared with adapter-fs and worker)
├── sandbox/             # NEW — kernel confinement (Linux-only)
│   └── src/
│       ├── lib.rs           # pub fn apply_confinement(config: SandboxConfig) -> Result<()>
│       ├── landlock.rs      # Landlock filesystem restriction rules
│       ├── seccomp.rs       # seccompiler BPF filter (deny execve, AF_INET socket, etc.)
│       └── rlimits.rs       # setrlimit for RLIMIT_AS, RLIMIT_CPU
└── adapter-fs/          # NEW — fd-pass filesystem adapter
    └── src/
        ├── lib.rs           # pub fn pass_fd(socket_fd: RawFd, file_fd: RawFd) -> Result<()>
        │                    # pub fn recv_fd(socket_fd: RawFd) -> Result<RawFd>
        └── protocol.rs      # RequestFd / FdGranted message types

cli/
└── caprun/              # existing stub → EXPAND: demo orchestrator
    └── src/
        ├── main.rs          # arg parsing, broker startup, worker spawn, demo loop
        └── worker.rs        # inline minimal worker (for demo — no separate binary needed)
```

### Pattern 1: Confinement Application via `pre_exec`

**What:** Apply all Linux kernel confinement primitives in the forked child, after fork but before exec, using Rust's `Command::pre_exec` hook.
**When to use:** Any time caprun spawns a worker process. The hook runs in the async-signal-safe context of the child post-fork; Landlock and seccomp are direct syscalls and are safe here.

```rust
// Source: std::os::unix::process::CommandExt + landlock + seccompiler
use std::os::unix::process::CommandExt;

let session_id = session.id.to_string();
let mut cmd = Command::new(&worker_binary);
cmd.env("BROKER_SOCK_ABSTRACT", format!("\0/agentos/{session_id}"));
cmd.env("SESSION_ID", &session_id);

unsafe {
    cmd.pre_exec(move || {
        // Step 1: no_new_privs — MUST come before seccomp when unprivileged
        nix::sys::prctl::prctl(
            nix::sys::prctl::PrctlOption::PR_SET_NO_NEW_PRIVS,
            1, 0, 0, 0,
        ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Step 2: rlimits (unprivileged)
        use nix::sys::resource::{setrlimit, Resource};
        setrlimit(Resource::RLIMIT_AS, 512 * 1024 * 1024, 512 * 1024 * 1024)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        setrlimit(Resource::RLIMIT_CPU, 30, 30)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Step 3: Landlock (deny all filesystem access — abstract UDS is unaffected)
        sandbox::landlock::deny_all_filesystem()?;

        // Step 4: seccomp-bpf (deny exec and outbound network socket creation)
        sandbox::seccomp::apply_worker_filter()?;

        Ok(())
    });
}
let child = cmd.spawn()?;
```

**Key invariant:** Abstract namespace UDS (`\0/agentos/<session_id>`) does NOT touch the filesystem — Landlock only restricts filesystem-namespace operations. After Landlock is applied, the worker can still connect to the broker's abstract socket.

### Pattern 2: Landlock Filesystem Restriction

**What:** Use the `landlock` crate to restrict ALL filesystem access for the worker. No explicit allow-rules means everything is denied.
**When to use:** Inside `sandbox::landlock::deny_all_filesystem()` called from `pre_exec`.

```rust
// Source: docs.rs/landlock/latest — verified via WebFetch 2026-06-29
use landlock::{
    Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr,
    ABI, Compatible,
};

pub fn deny_all_filesystem() -> std::io::Result<()> {
    let abi = ABI::V3; // Linux 5.19+; fall back to V1 (5.13) if not available

    let status = Ruleset::default()
        // Claim all filesystem access rights so Landlock controls them
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .create()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        // No rules added → everything denied
        .restrict_self()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;

    // Log whether fully enforced (on older kernels Landlock may partially enforce)
    eprintln!("[sandbox] Landlock status: {:?}", status.ruleset);
    Ok(())
}
```

**Kernel version note:** Landlock requires Linux ≥ 5.13 (ABI V1). The `landlock` crate's `Compatible` trait handles older kernels gracefully (partial enforcement). For production security claims, verify `status.ruleset == RulesetStatus::FullyEnforced`. [CITED: docs.rs/landlock/latest]

### Pattern 3: seccomp-bpf Worker Filter

**What:** Use `seccompiler` to build a BPF filter that denies exec (prevents spawning new processes) and outbound network socket creation (blocks AF_INET/AF_INET6).
**When to use:** Inside `sandbox::seccomp::apply_worker_filter()` called from `pre_exec`.

```rust
// Source: docs.rs/seccompiler — [ASSUMED: API surface from training knowledge + crate description]
use seccompiler::{SeccompAction, SeccompFilter, SeccompRule, BpfProgram};
use std::collections::BTreeMap;

pub fn apply_worker_filter() -> std::io::Result<()> {
    // syscall numbers vary by arch — seccompiler resolves them by name
    let rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    // Note: seccompiler API — verify exact call pattern against docs.rs/seccompiler/0.5.0
    // The filter below is the intended semantic; planner must verify exact API calls.
    
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,   // default: allow unlisted syscalls
        SeccompAction::Errno(libc::EPERM as u32),  // deny action for matching rules
        std::env::consts::ARCH.try_into()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "arch"))?,
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;
    
    // TODO in plan: add execve, execveat, socket(AF_INET), socket(AF_INET6) to deny list
    let program: BpfProgram = filter.try_into()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;
    seccompiler::apply_filter(&program)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))
}
```

**[ASSUMED]:** The seccompiler API for specifying individual syscall rules to deny (execve, socket) uses `SeccompRule` with syscall number matching. The planner MUST verify the exact API against `docs.rs/seccompiler/0.5.0` before implementation.

### Pattern 4: Broker UDS IPC Server (tokio)

**What:** tokio async server listening on an abstract-namespace Unix socket, dispatching JSON-framed messages.
**When to use:** The primary broker IPC loop in `brokerd::server`.

```rust
// Source: docs.rs/tokio/latest/tokio/net/struct.UnixListener.html — verified via WebFetch 2026-06-29
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn run_broker_server(session_id: &str) -> anyhow::Result<()> {
    // Abstract socket: prefix with '\0'
    let sock_path = format!("\0/agentos/{session_id}");
    
    // Tokio does not directly support abstract UDS; use std then convert
    let std_listener = std::os::unix::net::UnixListener::bind(&sock_path)?;
    std_listener.set_nonblocking(true)?;
    let listener = UnixListener::from_std(std_listener)?;
    
    loop {
        let (mut stream, _addr) = listener.accept().await?;
        tokio::spawn(async move {
            handle_connection(&mut stream).await
        });
    }
}

async fn handle_connection(stream: &mut tokio::net::UnixStream) -> anyhow::Result<()> {
    // Length-prefixed framing: 4-byte LE length, then JSON body
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    stream.read_exact(&mut body).await?;
    let msg: BrokerRequest = serde_json::from_slice(&body)?;
    // dispatch...
    Ok(())
}
```

**Note on abstract UDS in tokio:** `tokio::net::UnixListener` supports abstract namespace sockets via `from_std()` after binding with `std::os::unix::net::UnixListener`. The bind path starting with `\0` is the abstract namespace convention on Linux. [ASSUMED — verify that tokio accepts paths containing `\0` bytes via `from_std`; if not, use a temp-dir path-based socket instead and add a Landlock exception for that path.]

### Pattern 5: SQLite Audit DAG Schema and Hash-Chaining

**What:** A SQLite table where each event row stores its SHA-256 hash (computed over content + parent hash), forming a tamper-evident linked chain.
**When to use:** `brokerd::audit` module; appended on every broker-mediated operation.

```rust
// Source: docs.rs/rusqlite/latest — verified via WebFetch 2026-06-29
// Schema DDL — run once at broker startup
const SCHEMA_DDL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,  -- UUID
    intent_id  TEXT NOT NULL,
    status     TEXT NOT NULL,
    created_at TEXT NOT NULL      -- ISO-8601 UTC
) STRICT;

CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,  -- UUID
    parent_id   TEXT,              -- FK events.id; NULL for session root
    session_id  TEXT NOT NULL,
    event_type  TEXT NOT NULL,     -- 'session_created' | 'fd_granted' | 'file_read'
    actor       TEXT NOT NULL,
    payload     TEXT NOT NULL,     -- JSON (event-type specific)
    taint       TEXT NOT NULL,     -- JSON array: e.g. '[]' or '[\"external.untrusted\"]'
    parent_hash TEXT,              -- hex SHA-256 of parent event's `hash`; NULL for root
    hash        TEXT NOT NULL      -- hex SHA-256 of (parent_hash || id || session_id || event_type || payload || taint)
) STRICT;
";

// Hash computation: deterministic concatenation of canonical fields
use sha2::{Sha256, Digest};

fn compute_event_hash(parent_hash: Option<&str>, id: &str, session_id: &str,
                       event_type: &str, payload: &str, taint: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parent_hash.unwrap_or(""));
    hasher.update(id);
    hasher.update(session_id);
    hasher.update(event_type);
    hasher.update(payload);
    hasher.update(taint);
    hex::encode(hasher.finalize())
}
```

**Append-only enforcement:** SQLite's `STRICT` table mode + no UPDATE/DELETE grants in application code. The broker never issues UPDATE or DELETE on the events table.

### Pattern 6: SCM_RIGHTS fd-pass (broker → worker)

**What:** Broker opens a workspace file and passes the fd to the worker via sendmsg/recvmsg with SCM_RIGHTS ancillary data over the existing UDS connection.
**When to use:** `adapter-fs` crate; broker side invoked when handling a `RequestFd` message.

```rust
// Source: docs.rs/nix/latest/nix/sys/socket — verified via WebFetch 2026-06-29
use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
use std::io::IoSlice;
use std::os::unix::io::AsRawFd;

// BROKER SIDE: send the fd
pub fn pass_fd(socket_raw_fd: std::os::fd::RawFd, file_raw_fd: std::os::fd::RawFd)
    -> nix::Result<()>
{
    // iov must have at least one byte for cmsg delivery on some kernels
    let iov = [IoSlice::new(b"\x00")];
    let fds = [file_raw_fd];
    let cmsg = ControlMessage::ScmRights(&fds);
    sendmsg::<()>(socket_raw_fd, &iov, &[cmsg], MsgFlags::empty(), None)?;
    Ok(())
}

// WORKER SIDE: receive the fd
pub fn recv_fd(socket_raw_fd: std::os::fd::RawFd) -> nix::Result<std::os::fd::RawFd> {
    use nix::sys::socket::{recvmsg, ControlMessageOwned};
    use nix::cmsg_space;
    use std::io::IoSliceMut;

    let mut buf = [0u8; 1];
    let mut iov = [IoSliceMut::new(&mut buf)];
    let mut cmsgspace = cmsg_space!([std::os::fd::RawFd; 1]);

    let msg = recvmsg::<()>(socket_raw_fd, &mut iov, Some(&mut cmsgspace),
                              MsgFlags::empty())?;
    for cmsg in msg.cmsgs()? {
        if let ControlMessageOwned::ScmRights(fds) = cmsg {
            if let Some(&fd) = fds.first() {
                // Set O_CLOEXEC immediately after recv — fd is NOT CLOEXEC by default
                nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFD(
                    nix::fcntl::FdFlag::FD_CLOEXEC,
                ))?;
                return Ok(fd);
            }
        }
    }
    Err(nix::errno::Errno::ENODATA)
}
```

**Critical:** The fd-pass path uses blocking `sendmsg`/`recvmsg` (nix synchronous calls) on the raw socket fd. When integrating with the tokio async broker, this call MUST be run via `tokio::task::spawn_blocking` or via `UnixStream::try_io()` to avoid blocking the async runtime. [CITED: docs.rs/nix/latest]

### Anti-Patterns to Avoid

- **Applying confinement in the parent process (caprun):** If caprun calls `sandbox::apply()` before forking, it confines itself and can't open workspace files or manage the process. Always apply in `pre_exec` (child-only, post-fork).
- **Path-based UDS with Landlock deny-all:** A path-based socket (`/tmp/agentos.sock`) lives in the filesystem. If Landlock denies all fs access, the worker can't connect. Use abstract namespace.
- **Multiple `ControlMessage::ScmRights` in one sendmsg:** Sending multiple SCM_RIGHTS messages in a single sendmsg call causes platform-dependent behavior. Put all fds in a single ScmRights slice. [CITED: github.com/nix-rust/nix issue #464]
- **Setting taint in brokerd at broker-call time:** The Event's taint field MUST reflect what the worker read, not what the broker asserts at fd_grant time. For Phase 3 demo events, taint is `[]` (no external content yet) — that's correct since this is a clean workspace file, not hostile input. Phase 4 adds tainted reads.
- **Skipping hash verification in audit DAG:** Always compute and store the hash at append time. Verify the chain is unbroken in the demo output. A chain with gaps proves nothing.
- **tokio blocking in async context for sendmsg:** `nix::sys::socket::sendmsg` is synchronous. Calling it directly in a tokio task blocks the thread. Use `spawn_blocking`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| seccomp-bpf filter | Manual BPF bytecode construction | `seccompiler` | BPF bytecode is architecture-specific and error-prone; seccompiler generates correct bytecode from a rule description and handles arch differences |
| Landlock ruleset | Manual `syscall(SYS_landlock_*)` | `landlock` crate | Landlock API requires careful ABI negotiation with the kernel; the crate handles version compatibility, `restrict_self`, and graceful fallback |
| Audit log hash chaining | Custom HMAC/crypto | `sha2` from RustCrypto | SHA-256 hash chaining is standard for append-only audit logs; hand-rolling HMAC introduces subtle errors |
| UDS async accept loop | `epoll`/`poll` directly | `tokio::net::UnixListener` | tokio handles the non-blocking I/O event loop; manual epoll is 400+ lines to match tokio's cancel-safety |
| SQLite connection mgmt | raw `libsqlite3-sys` | `rusqlite` | rusqlite handles statement preparation, param binding, and type coercion safely; raw FFI misses null handling |

**Key insight:** The confinement stack (Landlock + seccomp + no_new_privs) has kernel-version compatibility hazards and architecture-specific BPF encoding. Using the `landlock` and `seccompiler` crates eliminates these classes of bugs entirely.

---

## Common Pitfalls

### Pitfall 1: Confinement Applied in Parent, Not Child

**What goes wrong:** `sandbox::apply_confinement()` called in caprun before `Command::spawn()`. Caprun is now sandboxed; it can't open the SQLite DB, serve UDS connections, or manage its own lifecycle.
**Why it happens:** It seems logical to set up the sandbox "around" the spawn call.
**How to avoid:** Always apply confinement inside `Command::pre_exec(|| { ... })`. The closure runs in the forked child after fork but before exec. The parent process is unaffected.
**Warning signs:** caprun crashes immediately after spawn with permission errors; or the broker's SQLite open fails.

### Pitfall 2: `no_new_privs` Not Set Before seccomp

**What goes wrong:** `seccompiler::apply_filter()` returns `EACCES` when called without root.
**Why it happens:** Unprivileged processes can only install seccomp-bpf filters after `prctl(PR_SET_NO_NEW_PRIVS, 1)`. This is a kernel requirement.
**How to avoid:** Always call `prctl(PR_SET_NO_NEW_PRIVS, 1)` FIRST in the confinement sequence, before any seccomp calls.
**Warning signs:** `apply_filter` returns `Err(EACCES)` in the child process; no sandbox is applied.

### Pitfall 3: cmsg Buffer Undersized for recvmsg

**What goes wrong:** `recvmsg` returns an `ENOBUFS` error or `msg.cmsgs()` yields nothing — the received fd is lost.
**Why it happens:** The `cmsg_buffer` passed to `recvmsg` must be at least `cmsg_space!(N)` bytes where N is the number of fds. If the buffer is too small, the kernel truncates or drops the ancillary data.
**How to avoid:** Always use the `cmsg_space!([RawFd; N])` macro to allocate the buffer. For Phase 3 (passing one fd at a time), use `cmsg_space!([RawFd; 1])`. [CITED: docs.rs/nix/latest]
**Warning signs:** `recvmsg` succeeds but `msg.cmsgs()` iterates zero elements; file read after recv gets EBADF.

### Pitfall 4: tokio Blocking Inside Async Task (sendmsg/recvmsg)

**What goes wrong:** The tokio runtime stalls; other UDS connections to the broker stop being served while the fd-pass operation waits.
**Why it happens:** `nix::sys::socket::sendmsg` and `recvmsg` are synchronous blocking calls. Running them directly in an `async fn` blocks the tokio thread pool.
**How to avoid:** Use `tokio::task::spawn_blocking(|| { pass_fd(...) }).await?` for fd-pass operations. Alternatively, use a dedicated sync thread for the fd-pass channel if the worker is single-connection.
**Warning signs:** The broker becomes unresponsive to new connections after the first fd-pass operation.

### Pitfall 5: Landlock on macOS Dev Machine

**What goes wrong:** `landlock::Ruleset::default()` panics or fails with a compilation error on macOS.
**Why it happens:** Landlock is a Linux-only LSM. The `landlock` crate itself is not macOS-compatible.
**How to avoid:** Gate the entire `crates/sandbox` confinement path behind `#[cfg(target_os = "linux")]`. On macOS, `apply_confinement()` should log a warning and return `Ok(())`. All negative-assertion integration tests must be gated the same way.
**Warning signs:** `cargo build` on macOS fails when sandbox crate is added; or tests run on macOS and silently pass because confinement is never applied.

### Pitfall 6: fd Leaked After Worker Exit

**What goes wrong:** The file descriptor passed to the worker remains open in the worker's process table after the worker exits abnormally (panics, killed by OOM).
**Why it happens:** When the worker process dies without closing the received fd, the kernel decrements the fd's reference count. When the kernel reaps the process, all its fds are closed automatically. **This is actually not a leak** — the kernel handles it.
**Actual pitfall:** The passed fd is NOT `O_CLOEXEC` by default after `recvmsg`. If the worker `exec()`s a child process (which it shouldn't — seccomp blocks this), the fd leaks into the grandchild. Always `fcntl(fd, F_SETFD, FD_CLOEXEC)` immediately after receiving the fd.

### Pitfall 7: Abstract UDS Availability in tokio

**What goes wrong:** `tokio::net::UnixListener::bind("\0/agentos/session")` fails with `invalid argument`.
**Why it happens:** tokio's `UnixListener::bind` converts the path string via `CString`, which may reject null bytes. The `from_std` workaround using `std::os::unix::net::UnixListener` directly handles null-byte abstract paths.
**How to avoid:** Bind using `std::os::unix::net::UnixListener::bind(...)` (which accepts abstract paths), set non-blocking, then wrap with `tokio::net::UnixListener::from_std()`.
**Warning signs:** `bind` returns `EINVAL` when path starts with `\0`.

---

## Code Examples

### Verified: rusqlite Open + DDL

```rust
// Source: docs.rs/rusqlite/latest — verified via WebFetch 2026-06-29
use rusqlite::Connection;

let conn = Connection::open("/var/lib/agentos/audit.db")?;
conn.execute_batch(SCHEMA_DDL)?;
// Use WAL mode for better concurrency (broker reads while writing)
conn.execute_batch("PRAGMA journal_mode=WAL;")?;
```

### Verified: nix sendmsg / recvmsg SCM_RIGHTS

See Pattern 6 above — verified from `docs.rs/nix/latest` and the nix test suite [CITED: github.com/nix-rust/nix/blob/master/test/sys/test_socket.rs].

### Verified: Landlock deny-all

See Pattern 2 above — verified from `landlock.io/rust-landlock/landlock/` [CITED: landlock.io/rust-landlock].

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Root + chroot + pivot_root for fs isolation | Landlock LSM (unprivileged) | Linux 5.13 (2021) | No root required; Landlock is the standard for user-space process sandboxing |
| Manual BPF bytecode for seccomp | seccompiler DSL (rust-vmm) | 2021 (Firecracker) | Eliminates architecture-specific BPF assembly; Firecracker-production-proven |
| cgroups v1 + v2 hybrid | cgroups v2 unified hierarchy | Linux 5.x default | v2 is now the default; v1 deprecated; but rlimits are simpler for Phase 3 |
| Path-based UDS sockets | Abstract-namespace UDS | Always available on Linux | Abstract sockets bypass filesystem namespace — critical when worker has zero fs access |

**Deprecated/outdated:**
- `sandbox_init()` on macOS (Seatbelt): Apple deprecated the sandbox C API; App Sandbox is entitlement-based; no supported path to calling it from a CLI Rust binary without entitlements.
- gVisor / Firecracker: explicitly out of scope for v0 per REQUIREMENTS.md.

---

## Runtime State Inventory

> Not applicable — this is a greenfield build phase, not a rename/refactor. No runtime state items to audit.

None — verified by scope review. Phase 3 creates new crates from scratch; no existing stored data, OS-registered state, or live service config references the artifacts being built.

---

## Open Questions

1. **Abstract UDS socket support in tokio `bind`**
   - What we know: `std::os::unix::net::UnixListener::bind` accepts abstract paths; `tokio::net::UnixListener::from_std` should wrap it.
   - What's unclear: Whether tokio's `from_std` properly forwards abstract-namespace socket operations or if there are additional gotchas.
   - Recommendation: Write a test in Wave 0 (before implementing the full broker server) that verifies `bind("\0/test_agentos") → from_std → accept()` round-trip works. Fall back to a temp-dir path-based socket with a Landlock exception if abstract fails.

2. **seccompiler 0.5.0 exact API for deny rules**
   - What we know: seccompiler provides `SeccompFilter`, `SeccompRule`, `SeccompAction`; generates BPF; used by Firecracker.
   - What's unclear: The exact call pattern to add `execve`-deny and `socket(AF_INET/6)`-deny rules using the 0.5.0 API surface.
   - Recommendation: Planner must include a task to read `docs.rs/seccompiler/0.5.0` and verify the rule API before implementing `sandbox::seccomp`.

3. **Workspace crate naming for adapter-fs**
   - What we know: REQUIREMENTS.md names it `adapters/fs`; `crates/*` glob doesn't match two-level paths.
   - What's unclear: Project's preference for Option A (`crates/adapter-fs`) vs Option B (`crates/adapters/fs` + workspace update).
   - Recommendation: Use `crates/adapter-fs` (Option A) to avoid workspace restructuring in Phase 3. Rename post-v0 if desired.

4. **rlimit CPU semantics for Phase 3**
   - What we know: `RLIMIT_CPU` measures CPU time in seconds (not wall time); a sleeping worker isn't CPU-limited.
   - What's unclear: Whether the success criterion "starts with CPU limits" requires real-time limits (requiring cgroups) or CPU-time limits (rlimits suffice).
   - Recommendation: `RLIMIT_CPU` (CPU seconds) + `RLIMIT_AS` (virtual memory) satisfy the Phase 3 success criterion. Add a note in caprun output showing the limits applied. Escalate to cgroups only if real-time limits are required.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Linux kernel ≥ 5.13 | `crates/sandbox` (Landlock) | macOS: NO, CI (Linux): YES | Darwin 25.5.0 / Linux CI | macOS: no-op stub; compile guard `#[cfg(target_os = "linux")]` |
| `cargo` / Rust toolchain | All crates | YES (inferred from Phase 1 completion) | Rust edition 2021 | — |
| SQLite (via rusqlite `bundled`) | `crates/brokerd` | YES (bundled, no system dep) | bundled | — |
| `seccomp-bpf` (Linux kernel) | `crates/sandbox` | macOS: NO, CI: YES | kernel feature | macOS: stub |
| `/sys/fs/cgroup` write access | `cgroups-rs` (optional) | Typically NO in CI without root | — | Use rlimits instead (recommended for Phase 3) |

**Missing with no fallback:** None — all blocking dependencies are satisfied by the Linux CI environment. macOS dev builds use stubs.
**Missing with fallback:** cgroups v2 write access → rlimits fallback (recommended even in Linux CI for simplicity).

**Dev/CI split (required):**
- **macOS (dev):** `cargo build --workspace` must succeed. Sandbox crate compiles with Linux-specific code behind `#[cfg(target_os = "linux")]`. `cargo test --workspace` runs all non-Linux-gated tests.
- **Linux CI (GitHub Actions ubuntu-latest):** `cargo test --workspace` runs ALL tests including `#[cfg(target_os = "linux")]` confinement integration tests and negative assertions.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner (`cargo test`) |
| Config file | `Cargo.toml` per crate (`[[test]]` sections) |
| Quick run command | `cargo test --workspace --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-sandbox | Confined worker cannot read `~/.ssh/id_rsa` | integration (Linux-only) | `cargo test -p sandbox --test confinement_integration -- negative_fs` | No — Wave 0 |
| REQ-sandbox | Confined worker cannot open TCP to 1.1.1.1:80 | integration (Linux-only) | `cargo test -p sandbox --test confinement_integration -- negative_net` | No — Wave 0 |
| REQ-sandbox | Confined worker cannot exec `/bin/ls` | integration (Linux-only) | `cargo test -p sandbox --test confinement_integration -- negative_exec` | No — Wave 0 |
| REQ-sandbox | Confinement no-ops on macOS (no panic) | unit | `cargo test -p sandbox --lib -- noop` | No — Wave 0 |
| REQ-brokerd-core | `CreateSession` → Session row in SQLite | integration | `cargo test -p brokerd --test uds_ipc -- create_session` | No — Wave 0 |
| REQ-brokerd-core | Event appended with valid hash-chain (parent_hash links) | unit | `cargo test -p brokerd --lib -- audit_hash_chain` | No — Wave 0 |
| REQ-brokerd-core | UDS server accepts connection, responds to ping | integration | `cargo test -p brokerd --test uds_ipc -- server_accept` | No — Wave 0 |
| REQ-adapters-fs | Broker passes fd; worker reads via fd, not open() | integration | `cargo test -p adapter-fs --test fd_pass -- round_trip` | No — Wave 0 |
| REQ-adapters-fs | recv_fd sets O_CLOEXEC on received fd | unit | `cargo test -p adapter-fs --lib -- fd_cloexec` | No — Wave 0 |
| REQ-substrate-demo | `caprun` end-to-end: confined worker reads file via fd, read Event in DAG | integration (Linux-only) | `cargo test -p caprun --test e2e -- substrate_demo` | No — Wave 0 |
| REQ-substrate-demo | DAG hash chain is unbroken (verify end-to-end) | integration | `cargo test -p caprun --test e2e -- dag_chain_integrity` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --workspace --lib` (unit tests only, < 5 seconds, passes on macOS)
- **Per wave merge:** `cargo test --workspace` (includes integration tests; Linux CI only for confinement tests)
- **Phase gate:** Full suite green on Linux CI before `/gsd-verify-work`

### Wave 0 Gaps

All test files are missing. Wave 0 of the plan MUST create:

- [ ] `crates/sandbox/tests/confinement_integration.rs` — negative assertions (Linux-gated)
- [ ] `crates/brokerd/tests/uds_ipc.rs` — UDS accept + CreateSession round-trip
- [ ] `crates/brokerd/tests/audit_dag.rs` — hash-chain verification (or inline in uds_ipc)
- [ ] `crates/adapter-fs/tests/fd_pass.rs` — SCM_RIGHTS round-trip
- [ ] `cli/caprun/tests/e2e.rs` — substrate demo end-to-end (Linux-gated)
- [ ] Framework install: already available (`cargo test` ships with Rust)

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | No user auth in substrate layer |
| V3 Session Management | Yes | Session IDs are UUIDs (v4); lifetime controlled by broker; no ambient session escalation |
| V4 Access Control | Yes | Landlock + seccomp enforce capability-based access; broker is the reference monitor |
| V5 Input Validation | Yes | IPC message size bounds (max 64KB per message); JSON deserialization via serde_json |
| V6 Cryptography | Yes | SHA-256 via sha2 (RustCrypto) for audit DAG hash-chain; do NOT use MD5/SHA-1 |
| V7 Error Handling | Yes | Broker must log errors but never surface internal paths/keys to worker |
| V14 Configuration | Yes | Broker socket path, DAG file path, and worker binary path must be validated before use |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Worker opens filesystem directly (bypassing broker) | Tampering | Landlock deny-all + seccomp (cannot open() without fd-pass); negative assertions in tests |
| Worker sends network traffic (exfiltrates data) | Information Disclosure | seccomp blocks `socket(AF_INET)`, `socket(AF_INET6)`; network namespace optional but stronger |
| Worker execs a child process (escapes sandbox) | Elevation of Privilege | seccomp blocks `execve`/`execveat`; `no_new_privs` prevents privilege gain on exec |
| Audit DAG tampered post-write | Tampering | Hash-chain: modifying any row breaks all downstream hashes; append-only SQLite access pattern |
| Malformed IPC message causes broker panic | DoS | Input validation: max message size guard before allocation; serde error handling must not panic |
| fd leaks to worker's exec'd children | Information Disclosure | Set `O_CLOEXEC` immediately after `recvmsg`; seccomp blocks exec anyway |
| Taint-stripping: worker reports clean read of hostile file | Forgery | Phase 3 only passes clean workspace files — taint is `[]` by design. Phase 4 adds tainted reads; broker sets taint from provenance at fd_granted time, not from worker-reported metadata |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | seccompiler 0.5.0 API uses `SeccompFilter::new(rules, default_action, match_action, arch)` constructor | Code Examples (Pattern 3) | Implementation task fails; planner must read docs.rs/seccompiler/0.5.0 before coding |
| A2 | `tokio::net::UnixListener::from_std()` correctly handles abstract-namespace UDS sockets bound with `std::os::unix::net::UnixListener` | Architecture Patterns (Pattern 4) | Broker server can't bind abstract socket; fall back to path-based UDS with Landlock path exception |
| A3 | GitHub Actions ubuntu-latest runner supports Landlock (Linux ≥ 5.13 kernel) | Environment Availability | CI negative-assertion tests won't enforce; need to pin ubuntu-22.04 or later runner image |
| A4 | `nix::sys::prctl::prctl(PrctlOption::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)` is the correct nix 0.31 call signature | Code Examples (Pattern 1) | Compilation error; planner task must verify exact nix 0.31 prctl API surface |
| A5 | rlimits (`RLIMIT_CPU`, `RLIMIT_AS`) satisfy the Phase 3 success criterion for "starts with CPU, memory" limits | Standard Stack, Open Questions | If real-time wall-clock CPU limits are required, cgroups v2 is needed, which requires root/delegation in CI |

---

## Sources

### Primary (MEDIUM confidence)

- `docs.rs/nix/latest` — sendmsg, recvmsg, ControlMessage::ScmRights function signatures and example
- `docs.rs/rusqlite/latest` — Connection::open, execute_batch, query_map patterns
- `docs.rs/tokio/latest/tokio/net/struct.UnixListener.html` — async accept, from_std
- `landlock.io/rust-landlock/landlock/` — Ruleset builder, AccessFs, restrict_self, kernel version handling
- `docs.kernel.org/userspace-api/landlock.html` — Landlock kernel documentation
- `docs.kernel.org/userspace-api/seccomp_filter.html` — seccomp-bpf kernel documentation
- `docs.kernel.org/userspace-api/no_new_privs.html` — no_new_privs kernel documentation

### Secondary (LOW confidence — web search, cross-referenced)

- Sandlock project (arxiv.org/html/2605.26298v1, github.com/multikernel/sandlock) — confirms Landlock + seccomp-bpf + seccomp user notification sufficiency for AI agent sandboxing
- `github.com/nix-rust/nix/blob/master/test/sys/test_socket.rs` — SCM_RIGHTS test showing complete send/recv pattern
- `github.com/nix-rust/nix/issues/464` — multiple ScmRights messages platform-dependence warning
- `lib.rs/crates/extrasafe` — low download signal; excluded
- crates.io registry — all package legitimacy verdicts (nix, rusqlite, tokio, landlock, seccompiler, sha2, cgroups-rs all OK; birdcage GPL-3.0; extrasafe SUS)

---

## Metadata

**Confidence breakdown:**
- Standard stack (crate choices): MEDIUM — all crates verified on crates.io registry with OK legitimacy verdicts; kernel API facts from kernel.org docs
- Architecture patterns: MEDIUM — IPC framing and confinement stack verified against official docs; abstract UDS + tokio interaction is [ASSUMED] pending wave-0 test
- Pitfalls: MEDIUM — SCM_RIGHTS pitfalls from nix test suite and nix issue tracker; Linux-only confinement confirmed by REQUIREMENTS.md
- seccompiler API surface: LOW — exact rule construction API not directly verified this session; marked [ASSUMED]; planner must verify

**Research date:** 2026-06-29
**Valid until:** 2026-07-29 (30 days — crates.io stable; kernel API stable; seccompiler API may rev)
