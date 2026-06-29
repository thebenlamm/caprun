# Phase 3: Confinement & Mediation Substrate — Pattern Map

**Mapped:** 2026-06-29
**Files analyzed:** 14 new/modified files across 4 crates
**Analogs found:** 10 / 14 (4 have no codebase analog — documented in §No Analog Found)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/sandbox/Cargo.toml` | config | — | `crates/brokerd/Cargo.toml` | role-match |
| `crates/sandbox/src/lib.rs` | utility | request-response | `crates/brokerd/src/lib.rs` | role-match |
| `crates/sandbox/src/landlock.rs` | utility | request-response | `crates/brokerd/src/lib.rs` | partial |
| `crates/sandbox/src/seccomp.rs` | utility | request-response | `crates/brokerd/src/lib.rs` | partial |
| `crates/sandbox/src/rlimits.rs` | utility | request-response | `crates/brokerd/src/lib.rs` | partial |
| `crates/brokerd/src/server.rs` | service | event-driven | `crates/brokerd/src/lib.rs` | partial |
| `crates/brokerd/src/session.rs` | service | CRUD | `crates/runtime-core/src/session.rs` | role-match |
| `crates/brokerd/src/audit.rs` | service | CRUD | `crates/runtime-core/src/event.rs` | partial |
| `crates/brokerd/src/proto.rs` | model | request-response | `crates/runtime-core/src/plan_node.rs` | role-match |
| `crates/adapter-fs/Cargo.toml` | config | — | `crates/brokerd/Cargo.toml` | role-match |
| `crates/adapter-fs/src/lib.rs` | utility | file-I/O | none | — |
| `crates/adapter-fs/src/protocol.rs` | model | request-response | `crates/runtime-core/src/plan_node.rs` | role-match |
| `cli/caprun/src/main.rs` | controller | event-driven | `crates/brokerd/src/lib.rs` (stub) | partial |
| `cli/caprun/src/worker.rs` | utility | request-response | none | — |

---

## Pattern Assignments

### `crates/sandbox/Cargo.toml` (config)

**Analog:** `crates/brokerd/Cargo.toml` (lines 1–11)

**Cargo.toml pattern:**
```toml
[package]
name    = "sandbox"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
anyhow       = { workspace = true }
# Linux-only confinement deps — add to [workspace.dependencies] first:
# landlock    = "0.4.5"
# seccompiler = "0.5.0"
# nix         = { version = "0.31.3", features = ["resource", "process", "signal"] }
[target.'cfg(target_os = "linux")'.dependencies]
landlock     = { workspace = true }
seccompiler  = { workspace = true }
nix          = { workspace = true }
```

**Key deviation from analog:** sandbox uses `[target.'cfg(target_os = "linux")'.dependencies]` for confinement crates — the existing crates don't show this pattern yet, so it is new. All workspace-level metadata fields (`version.workspace`, `edition.workspace`, `license.workspace`) copy directly from `crates/brokerd/Cargo.toml`.

---

### `crates/sandbox/src/lib.rs` (utility, request-response)

**Analog:** `crates/brokerd/src/lib.rs` (lines 1–22)

**Module-level doc comment pattern** (lines 1–6):
```rust
/// sandbox — kernel confinement primitives
///
/// All confinement is Linux-only. On non-Linux targets every public function
/// is a no-op stub that logs a warning and returns Ok(()).
/// Apply via `apply_confinement()` inside `Command::pre_exec` ONLY — never
/// in the parent process.
```

**Public API surface pattern** (follows brokerd's single-function stub style):
```rust
pub mod landlock;
pub mod seccomp;
pub mod rlimits;

pub use landlock::deny_all_filesystem;
pub use seccomp::apply_worker_filter;
pub use rlimits::apply_rlimits;
```

**cfg-gate pattern** (no existing analog — new for this crate):
```rust
#[cfg(not(target_os = "linux"))]
pub fn apply_confinement() -> std::io::Result<()> {
    eprintln!("[sandbox] WARNING: confinement is a no-op on non-Linux");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn apply_confinement() -> std::io::Result<()> {
    rlimits::apply_rlimits()?;
    landlock::deny_all_filesystem()?;
    seccomp::apply_worker_filter()?;
    Ok(())
}
```

**Test pattern** (copy from `crates/brokerd/src/lib.rs` lines 24–40):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_on_macos_does_not_panic() {
        // This test runs on macOS; apply_confinement must return Ok(())
        #[cfg(not(target_os = "linux"))]
        assert!(apply_confinement().is_ok());
    }
}
```

---

### `crates/sandbox/src/landlock.rs` (utility, request-response)

**Analog:** `crates/brokerd/src/lib.rs` — error-mapping pattern only; no direct landlock analog in codebase.

**Error mapping convention** (copy from brokerd's anyhow usage):
```rust
// brokerd uses anyhow::Result; sandbox uses std::io::Result for pre_exec compat
// Map external errors with:
.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;
```

**Full implementation pattern** — from RESEARCH.md Pattern 2 (no codebase analog):
```rust
#[cfg(target_os = "linux")]
pub fn deny_all_filesystem() -> std::io::Result<()> {
    use landlock::{Access, AccessFs, ABI, Ruleset, RulesetAttr, RulesetCreatedAttr};
    let abi = ABI::V3;
    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .create()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
        .restrict_self()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;
    eprintln!("[sandbox] Landlock status: {:?}", status.ruleset);
    Ok(())
}
```

---

### `crates/sandbox/src/seccomp.rs` (utility, request-response)

**Analog:** same error-mapping pattern as landlock.rs above.

**[ASSUMED API — verify against docs.rs/seccompiler/0.5.0 before implementing]**
```rust
#[cfg(target_os = "linux")]
pub fn apply_worker_filter() -> std::io::Result<()> {
    use seccompiler::{BpfProgram, SeccompAction, SeccompFilter};
    use std::collections::BTreeMap;
    // Planner MUST read docs.rs/seccompiler/0.5.0 to verify SeccompFilter::new signature
    // and the mechanism for specifying execve + socket(AF_INET/6) deny rules.
    // See RESEARCH.md §Assumptions Log A1 — this is the highest-risk API surface.
    todo!("verify seccompiler 0.5.0 API before implementing")
}
```

---

### `crates/sandbox/src/rlimits.rs` (utility, request-response)

**Analog:** same error-mapping convention.

```rust
#[cfg(target_os = "linux")]
pub fn apply_rlimits() -> std::io::Result<()> {
    use nix::sys::resource::{setrlimit, Resource};
    setrlimit(Resource::RLIMIT_AS, 512 * 1024 * 1024, 512 * 1024 * 1024)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    setrlimit(Resource::RLIMIT_CPU, 30, 30)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
}
// See RESEARCH.md §Assumptions Log A4 — verify nix 0.31 prctl call signature too.
```

---

### `crates/brokerd/src/proto.rs` (model, request-response)

**Analog:** `crates/runtime-core/src/plan_node.rs` (lines 1–54)

**Struct definition pattern** (copy derive stack and doc comment style):
```rust
// From runtime-core/src/plan_node.rs lines 9–18:
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaintLabel { ... }

// Copy this pattern for all IPC message types:
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerRequest {
    CreateSession { intent_id: uuid::Uuid },
    RequestFd { path: String },
    ReportRead { bytes_read: u64 },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrokerResponse {
    SessionCreated { session_id: uuid::Uuid },
    FdGranted,           // fd delivered via SCM_RIGHTS out-of-band
    Ack,
    Error { message: String },
}
```

**Wire format:** JSON via `serde_json::to_vec` / `serde_json::from_slice` — already a workspace dep.
**Framing:** 4-byte LE length prefix before JSON body (see RESEARCH.md Pattern 4).

---

### `crates/brokerd/src/session.rs` (service, CRUD)

**Analog:** `crates/runtime-core/src/session.rs` (lines 1–32)

**Session struct reuse** — brokerd's session module uses `runtime_core::Session` directly rather than defining a new struct:
```rust
// From runtime-core/src/session.rs lines 24–31 — use this type directly:
use runtime_core::{Session, SessionStatus};
use uuid::Uuid;
use chrono::Utc;

pub fn create_session(intent_id: Uuid) -> Session {
    let now = Utc::now();
    Session {
        id: Uuid::new_v4(),
        intent_id,
        status: SessionStatus::Active,
        created_at: now,
        updated_at: now,
    }
}
```

**Persistence:** `rusqlite::Connection` — no existing analog in codebase, use RESEARCH.md Code Examples §rusqlite Open + DDL.

---

### `crates/brokerd/src/audit.rs` (service, CRUD)

**Analog:** `crates/runtime-core/src/event.rs` (lines 1–29) — Event struct to reuse; persistence is new.

**Event struct reuse** (lines 16–29):
```rust
// runtime-core/src/event.rs — Event owns the domain type; audit.rs persists it.
// Import and serialize via serde_json:
use runtime_core::{Event, TaintLabel};

// INSERT pattern (no existing analog — follow RESEARCH.md Pattern 5):
fn append_event(conn: &rusqlite::Connection, event: &Event, parent_hash: Option<&str>)
    -> anyhow::Result<()>
{
    let payload = serde_json::to_string(&event)?;
    let taint   = serde_json::to_string(&event.taint)?;
    let hash    = compute_event_hash(parent_hash, &event.id.to_string(),
                                     &event.session_id.to_string(),
                                     &event.event_type, &payload, &taint);
    conn.execute(
        "INSERT INTO events (id, parent_id, session_id, event_type, actor,
                             payload, taint, parent_hash, hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            event.id.to_string(), /* parent_id */ ..., hash
        ],
    )?;
    Ok(())
}
```

---

### `crates/brokerd/src/server.rs` (service, event-driven)

**Analog:** `crates/brokerd/src/lib.rs` — function signature convention only; async UDS is new.

**Tokio async pattern** (no existing analog in codebase — from RESEARCH.md Pattern 4):
```rust
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn run_broker_server(session_id: &str) -> anyhow::Result<()> {
    let sock_path = format!("\0/agentos/{session_id}");
    // MUST use std bind → from_std for abstract namespace (RESEARCH.md Pitfall 7)
    let std_listener = std::os::unix::net::UnixListener::bind(&sock_path)?;
    std_listener.set_nonblocking(true)?;
    let listener = UnixListener::from_std(std_listener)?;
    loop {
        let (mut stream, _addr) = listener.accept().await?;
        tokio::spawn(async move { handle_connection(&mut stream).await });
    }
}
```

**Error handling convention** (copy from brokerd/src/lib.rs anyhow style):
```rust
// All async fns return anyhow::Result<T>
// Log errors with eprintln! for Phase 3; structured logging added later
```

---

### `crates/adapter-fs/Cargo.toml` (config)

**Analog:** `crates/brokerd/Cargo.toml` (lines 1–11)

```toml
[package]
name    = "adapter-fs"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
anyhow       = { workspace = true }
nix          = { workspace = true }   # SCM_RIGHTS sendmsg/recvmsg
runtime-core = { path = "../runtime-core" }
```

---

### `crates/adapter-fs/src/protocol.rs` (model, request-response)

**Analog:** `crates/runtime-core/src/plan_node.rs` derive stack (lines 9–18)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RequestFd {
    pub path: String,
    pub session_id: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FdGranted {
    pub path: String,   // echoed back for audit log correlation
}
```

---

### `cli/caprun/Cargo.toml` (config)

**Analog:** `cli/caprun/Cargo.toml` (current — lines 1–9, only `[[bin]]` + workspace metadata)

**Additions needed:**
```toml
[dependencies]
sandbox      = { path = "../../crates/sandbox" }
brokerd      = { path = "../../crates/brokerd" }
adapter-fs   = { path = "../../crates/adapter-fs" }
runtime-core = { path = "../../crates/runtime-core" }
tokio        = { workspace = true }
anyhow       = { workspace = true }
uuid         = { workspace = true }

[[test]]
name = "e2e"
path = "tests/e2e.rs"
```

---

### `cli/caprun/src/main.rs` (controller, event-driven)

**Analog:** `cli/caprun/src/main.rs` (current — empty stub, line 1 only)

**pre_exec spawn pattern** (no existing analog — from RESEARCH.md Pattern 1):
```rust
use std::os::unix::process::CommandExt;

let session_id = session.id.to_string();
let mut cmd = std::process::Command::new(&worker_binary_path);
cmd.env("BROKER_SOCK", format!("\0/agentos/{session_id}"));
cmd.env("SESSION_ID", &session_id);

unsafe {
    cmd.pre_exec(move || {
        // Order is mandatory — see RESEARCH.md Pitfall 2
        // 1. no_new_privs FIRST
        // 2. rlimits
        // 3. Landlock
        // 4. seccomp
        sandbox::apply_confinement()
    });
}
let child = cmd.spawn()?;
```

**Doc comment convention** — copy from `crates/brokerd/src/lib.rs` lines 1–6 style: top-of-file `///` block explaining the module's role and cross-phase invariants.

---

## Shared Patterns

### Workspace metadata inheritance
**Source:** `crates/brokerd/Cargo.toml` lines 2–5
**Apply to:** Every new `Cargo.toml` in this phase
```toml
version.workspace = true
edition.workspace = true
license.workspace = true
```

### Workspace `[dependencies]` additions required
**Source:** Root `Cargo.toml` `[workspace.dependencies]` (current state — lines verified above)
**Apply to:** Root `Cargo.toml` before any new crate can use `{ workspace = true }`
```toml
# Add these — they are currently ABSENT from workspace deps:
nix          = { version = "0.31.3", features = ["fs", "socket", "resource", "process", "signal"] }
tokio        = { version = "1.52.3", features = ["net", "io-util", "rt-multi-thread", "macros"] }
rusqlite     = { version = "0.40.1", features = ["bundled"] }
landlock     = "0.4.5"
seccompiler  = "0.5.0"
sha2         = { version = "0.11.0", features = ["std"] }
tokio-util   = { version = "0.7.18", features = ["codec"] }  # optional
# serde_json already present at "1.0.150" — keep as-is
```

### Workspace members addition
**Source:** Root `Cargo.toml` `[workspace]` members line
**Current value:** `members = ["crates/*", "cli/caprun"]`
**Needed addition:** `"crates/adapter-fs"` is matched by `crates/*` glob — no change needed.
If planner chooses Option B (`crates/adapters/fs`), add `"crates/adapters/*"` to members.

### Serde derive pattern
**Source:** `crates/runtime-core/src/plan_node.rs` lines 9, 23, 33, 43, 50
**Apply to:** All IPC message types in `proto.rs`, `adapter-fs/src/protocol.rs`
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
```

### anyhow error handling
**Source:** `crates/brokerd/Cargo.toml` + `crates/brokerd/src/lib.rs`
**Apply to:** All async service functions in `brokerd/src/server.rs`, `brokerd/src/audit.rs`, `brokerd/src/session.rs`
```rust
// Function signatures:
pub async fn foo() -> anyhow::Result<()>
pub fn bar() -> anyhow::Result<T>
// Never return () or unwrap in non-test code
```

### std::io::Result for pre_exec compatibility
**Source:** RESEARCH.md Pattern 1 — `pre_exec` closure must return `std::io::Result<()>`
**Apply to:** `sandbox/src/lib.rs`, `sandbox/src/landlock.rs`, `sandbox/src/seccomp.rs`, `sandbox/src/rlimits.rs`
```rust
// All sandbox public fns must return std::io::Result<()>, not anyhow::Result
// because pre_exec requires std::io::Error
.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
```

### cfg Linux-gate pattern
**Apply to:** All sandbox confinement code; all integration tests asserting confinement behavior
```rust
#[cfg(target_os = "linux")]
// — implementation

#[cfg(not(target_os = "linux"))]
// — no-op stub returning Ok(())
```

### Test structure pattern
**Source:** `crates/brokerd/src/lib.rs` lines 24–40 + `crates/runtime-core/tests/task2_types.rs` lines 1–5
**Apply to:** All unit test modules and integration test files

Inline unit tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // imports from runtime_core as needed
    #[test]
    fn descriptive_test_name() { ... }
}
```

Integration test files (e.g. `tests/uds_ipc.rs`):
```rust
// Top-of-file import pattern from runtime-core/tests/task2_types.rs lines 1–5:
use brokerd::{...};  // crate under test
// Linux-gated tests:
#[cfg(target_os = "linux")]
#[test]
fn negative_fs_access() { ... }
```

---

## No Analog Found

Files with no close match in the existing codebase — planner must use RESEARCH.md patterns:

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/adapter-fs/src/lib.rs` | utility | file-I/O | No fd-passing code exists; SCM_RIGHTS is entirely new — use RESEARCH.md Pattern 6 verbatim |
| `cli/caprun/src/worker.rs` | utility | request-response | No worker-loop analog exists; inline demo worker is greenfield — use RESEARCH.md §Architecture Patterns |
| `crates/sandbox/src/seccomp.rs` | utility | request-response | No seccomp usage exists; RESEARCH.md Pattern 3 is ASSUMED — planner must verify seccompiler 0.5.0 API before coding |
| `crates/brokerd/tests/uds_ipc.rs` (and all new test files) | test | event-driven | No async integration test exists; closest structural analog is `runtime-core/tests/task2_types.rs` for file layout only |

---

## Metadata

**Analog search scope:** `crates/brokerd/`, `crates/runtime-core/`, `cli/caprun/`
**Files scanned:** 13 existing files (full read)
**Pattern extraction date:** 2026-06-29
**Critical assumption:** seccompiler 0.5.0 API (RESEARCH.md §Assumptions Log A1) — planner must add a wave-0 task to verify before implementation.
