# Phase 1: Substrate Foundation — Research

**Researched:** 2026-06-29
**Domain:** Rust Cargo workspace setup + pure domain types + broker API stub design
**Confidence:** HIGH (architecture derived from locked PLAN.md; crate versions verified against registry)

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-runtime-core | `runtime-core` crate — Intent, Session, Event, Artifact, and 3-class Effect enums compile with no I/O | Section: Standard Stack, Architecture Patterns, Code Examples |
| REQ-api-stub-plan-node | Broker `submit_plan_node()` API surface, shape locked from day one; returns `NotImplemented`; no raw EffectRequest→sink path anywhere | Section: Architecture Patterns, Code Examples, Common Pitfalls |
</phase_requirements>

---

## Summary

Phase 1 delivers two things: (1) the Cargo workspace skeleton and the `runtime-core` crate of pure domain types, and (2) the locked broker API stub (`submit_plan_node`) that forces every later effect path through `PlanNode`/`ValueNode`. Nothing here touches the filesystem, network, or any async I/O — `runtime-core` must compile with zero I/O imports. The broker stub can live as a minimal function in `crates/brokerd` that returns an `ExecutorDecision::NotImplemented` enum variant; a `todo!()` panic is the wrong pattern because it aborts rather than returning a typed value.

The workspace layout is decided and locked: virtual manifest at repo root (`[workspace]` only, no `[package]`), crates at `crates/`, `resolver = "3"` for Rust 2024 edition, shared dep versions declared once in `[workspace.dependencies]`. The domain types derive from the project's canonical architecture docs (`archive/multi-part/core-arch.md`, `planning-docs/PLAN.md`) and are already specified in enough detail to implement directly — no design freedom here.

**Primary recommendation:** Stand up the virtual workspace first, then implement `runtime-core` types, then add the brokerd stub crate with `submit_plan_node`. All three steps are `cargo build` gated.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Domain types (Intent, Session, etc.) | `crates/runtime-core` | — | Pure value types; all other crates depend on this; no I/O allowed here |
| Effect classification (3-class enum) | `crates/runtime-core` | — | Planner-surface enum; belongs in shared core, not in any one crate |
| PlanNode / ValueNode type definitions | `crates/runtime-core` | — | Both brokerd and future executor need these; they must be in the shared core |
| `submit_plan_node` stub function | `crates/brokerd` | — | Broker owns the API surface; the shape is locked but implementation is a stub |
| `ExecutorDecision` enum | `crates/runtime-core` | — | Returned by submit_plan_node; both brokerd and executor need it |
| Workspace configuration | repo root `Cargo.toml` | — | Virtual manifest; all shared dep versions live here |

---

## Standard Stack

### Core (runtime-core — no I/O)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` | 1.0.228 | Serialize/deserialize domain types | Universal Rust serialization; needed for PlanNode/ValueNode wire format later |
| `uuid` | 1.23.4 | `Uuid` type for all IDs (IntentId, SessionId, etc.) | Standard UUID library; serde feature for JSON |
| `thiserror` | 2.0.18 | Derive `Error` on error enums | dtolnay standard; avoids boilerplate `impl Error` |
| `chrono` | 0.4.45 | `DateTime<Utc>` for timestamps on all domain types | Standard Rust datetime; serde feature for JSON |

[VERIFIED: npm registry equivalent — cargo search confirmed all four crates, versions as of 2026-06-29]

### Supporting (brokerd stub only, not in runtime-core)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `anyhow` | 1.0.103 | Flexible error type for application code | Use in brokerd/cli, never in runtime-core (library should use thiserror) |
| `serde_json` | 1.0.150 | JSON encoding for IPC messages | Needed when brokerd serializes PlanNode over UDS in Phase 3 |

[VERIFIED: cargo search, 2026-06-29]

### Deferred — NOT in Phase 1

| Crate | Phase |
|-------|-------|
| `tokio` | Phase 3 (brokerd core async runtime) |
| `sqlx` / SQLite | Phase 3 (audit DAG) |
| `nix` / `rustix` | Phase 3 (sandbox) |
| `landlock` / `seccompiler` | Phase 3 (sandbox) |
| `ed25519-dalek` | Phase 3+ (captoken) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `thiserror` | `anyhow` in core | `anyhow` is for apps, not libraries; thiserror lets callers match error variants |
| `chrono` | `time` crate | time is growing but chrono is more established for serde integration in 2026 |
| `uuid` v4 | ULID | ULIDs are sortable but UUID is already in the architecture docs as the ID type |

**Installation (workspace Cargo.toml):**
```toml
[workspace.dependencies]
serde      = { version = "1.0.228", features = ["derive"] }
uuid       = { version = "1.23.4",  features = ["v4", "serde"] }
thiserror  = "2.0.18"
chrono     = { version = "0.4.45",  features = ["serde"] }
anyhow     = "1.0.103"
serde_json = "1.0.150"
```

**Version verification:** All versions confirmed via `cargo search` on 2026-06-29 against crates.io registry.

---

## Package Legitimacy Audit

> All packages verified via `gsd-tools query package-legitimacy check --ecosystem crates`.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| serde | crates.io | 11 yrs | 17.2M/wk | github.com/serde-rs/serde | OK | Approved |
| uuid | crates.io | 11 yrs | 11.1M/wk | github.com/uuid-rs/uuid | OK | Approved |
| thiserror | crates.io | 6 yrs | 21.6M/wk | github.com/dtolnay/thiserror | OK | Approved |
| chrono | crates.io | 11 yrs | 10.1M/wk | github.com/chronotope/chrono | OK | Approved |
| anyhow | crates.io | 6 yrs | 12.7M/wk | github.com/dtolnay/anyhow | OK | Approved |
| tokio | crates.io | 9 yrs | 14.0M/wk | github.com/tokio-rs/tokio | OK | Approved |
| tracing | crates.io | 8 yrs | 12.0M/wk | github.com/tokio-rs/tracing | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

---

## Architecture Patterns

### System Architecture Diagram

```
User / cargo build
        │
        ▼
[Cargo workspace root Cargo.toml]  ← virtual manifest, no [package]
        │
        ├──► crates/runtime-core/  ← pure domain types (no I/O, no async)
        │         lib.rs
        │           Intent, Session, Event, Artifact
        │           Effect (3-class enum)
        │           PlanNode { sink: SinkId, args: Vec<ValueNode> }
        │           ValueNode { literal, provenance, taint }
        │           ExecutorDecision (NotImplemented variant)
        │           TaintLabel enum
        │
        └──► crates/brokerd/       ← API stub only in Phase 1
                  lib.rs
                    submit_plan_node(session_id, PlanNode) -> ExecutorDecision
                    [returns NotImplemented; no EffectRequest→sink path]
```

Data flow for Phase 1 is compile-only — no runtime behavior beyond returning the stub.

### Recommended Project Structure

```
AgentOS/                          # repo root = Cargo workspace
  Cargo.toml                      # [workspace] virtual manifest only
  Cargo.lock                      # single lock file (committed for bins)
  crates/
    runtime-core/
      Cargo.toml
      src/
        lib.rs                    # pub mod re-exports
        intent.rs                 # Intent, IntentStatus, Principal
        session.rs                # Session, SessionStatus
        effect.rs                 # Effect, ObserveEffect, ReversibleEffect, IrreversibleEffect
        artifact.rs               # Artifact, ArtifactType, ArtifactRef
        event.rs                  # Event (audit DAG node)
        plan_node.rs              # PlanNode, ValueNode, SinkId, TaintLabel, Provenance
        executor_decision.rs      # ExecutorDecision enum
    brokerd/
      Cargo.toml
      src/
        lib.rs                    # pub fn submit_plan_node(...)
  cli/
    caprun/                       # Phase 3+ — scaffold Cargo.toml only in Phase 1
      Cargo.toml
  planning-docs/                  # existing
  archive/                        # existing
```

### Pattern 1: Virtual Workspace Manifest

**What:** Root `Cargo.toml` declares workspace members with no `[package]` section.
**When to use:** Always — the PLAN.md locks this layout.

```toml
# Source: doc.rust-lang.org/cargo/reference/workspaces.html [VERIFIED]
[workspace]
members  = ["crates/*", "cli/caprun"]
resolver = "3"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
serde      = { version = "1.0.228", features = ["derive"] }
uuid       = { version = "1.23.4",  features = ["v4", "serde"] }
thiserror  = "2.0.18"
chrono     = { version = "0.4.45",  features = ["serde"] }
anyhow     = "1.0.103"
serde_json = "1.0.150"
```

### Pattern 2: Member Crate Inheriting Workspace Deps

```toml
# crates/runtime-core/Cargo.toml
# Source: doc.rust-lang.org/cargo/reference/workspaces.html [VERIFIED]
[package]
name    = "runtime-core"
version.workspace  = true
edition.workspace  = true
license.workspace  = true

[dependencies]
serde   = { workspace = true }
uuid    = { workspace = true }
thiserror.workspace = true
chrono  = { workspace = true }
```

### Pattern 3: The 3-Class Effect Enum

**What:** Effect is three variants with associated sub-enums, not a flat list.
**When to use:** Always — PLAN.md locks this shape. Grow by adding variants to sub-enums, not new top-level classes.

```rust
// Source: archive/multi-part/core-arch.md [CITED]
// Source: planning-docs/PLAN.md [CITED]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Effect {
    Observe(ObserveEffect),
    MutateReversible(ReversibleEffect),
    CommitIrreversible(IrreversibleEffect),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ObserveEffect {
    ReadWorkspaceFile { path: String },
    ListWorkspace { path: String },
    RunTests { command: String },
    SummarizeArtifact { artifact_id: uuid::Uuid },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReversibleEffect {
    WriteArtifact { name: String, content_hash: String },
    ApplyPatch { patch_hash: String },
    EditWorkspaceFile { path: String, patch_hash: String },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrreversibleEffect {
    SendEmail { draft_hash: String, to: Vec<String> },
    GitPush { remote: String, branch: String },
    DeployService { service: String, environment: String },
}
```

### Pattern 4: ValueNode with Literal + Provenance + Taint

**What:** ValueNode carries all three fields from day one so the executor can enforce I2.
**When to use:** Always — this is the architectural lock (DEC-architectural-lock-plan-nodes).

```rust
// Source: planning-docs/PLAN.md (DEC-architectural-lock-plan-nodes) [CITED]
// Source: archive/AGENT-RUNTIME-HANDOVER.md §4.5 [CITED]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueNode {
    /// The concrete literal value (string, number, bool, etc.)
    pub literal: serde_json::Value,
    /// Where this value came from
    pub provenance: Provenance,
    /// Taint labels accumulated on this value
    pub taint: Vec<TaintLabel>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    pub source_event_id: Option<uuid::Uuid>,
    pub source_artifact_id: Option<uuid::Uuid>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaintLabel {
    UserTrusted,
    LocalWorkspace,
    ExternalUntrusted,
    EmailRaw,
    PdfRaw,
    LlmGenerated,
    WorkerExtracted,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanNode {
    pub sink: SinkId,
    pub args: Vec<ValueNode>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SinkId(pub String);  // e.g. "email.send", "git.push"
```

### Pattern 5: submit_plan_node Stub

**What:** The locked broker API surface returns `ExecutorDecision::NotImplemented` immediately.
**When to use:** Phase 1 only — this stub is replaced by the real executor in Phase 4.

```rust
// Source: planning-docs/PLAN.md (DEC-architectural-lock-plan-nodes) [CITED]
use runtime_core::{PlanNode, ExecutorDecision};
use uuid::Uuid;

/// Broker API surface — shape locked from day one.
/// Returns NotImplemented in Phase 1; full I2 enforcement in Phase 4.
///
/// INVARIANT: there must be no raw EffectRequest→sink path anywhere
/// in this crate. All effects are mediated through PlanNode/ValueNode.
pub fn submit_plan_node(
    _session_id: Uuid,
    _plan: PlanNode,
) -> ExecutorDecision {
    ExecutorDecision::NotImplemented
}
```

```rust
// In runtime-core: executor_decision.rs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExecutorDecision {
    /// Plan executed and all taint checks passed.
    Allowed,
    /// Execution blocked — tainted value in sensitive sink arg; confirmation required.
    BlockedPendingConfirmation { literal_value: String, sink: String, arg_name: String },
    /// Execution denied by policy.
    Denied { reason: String },
    /// Stub: executor not yet implemented.
    NotImplemented,
}
```

### Anti-Patterns to Avoid

- **`todo!()` for the API stub:** Panics at runtime; the plan requires returning `NotImplemented` as a typed value. Use `ExecutorDecision::NotImplemented`.
- **`EffectRequest` bypass path:** Never add a `fn submit_effect_request(...)` alongside `submit_plan_node`. This is the specific invariant the PLAN.md hard-codes.
- **I/O in runtime-core:** No `use std::io`, no `tokio`, no `async fn`, no file reading. Not even `println!` in lib code. The crate must remain pure types.
- **Putting PlanNode/ValueNode in brokerd only:** These types must be in `runtime-core` so the executor crate (Phase 4) can depend on them without a circular dependency.
- **Flat workspace (no virtual manifest):** Don't put a `[package]` in the root `Cargo.toml`. The repo root is the workspace, not a crate.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UUID generation | `rand::thread_rng()` + manual bytes | `uuid::Uuid::new_v4()` | Handles RFC4122 compliance, byte ordering, serde |
| Timestamp handling | Manual Unix epoch math | `chrono::Utc::now()` with `DateTime<Utc>` | Timezone-correct, serde-ready, database-compatible |
| Error trait boilerplate | `impl std::error::Error for MyError` by hand | `#[derive(thiserror::Error)]` | 30 lines → 3 lines; consistent Display |
| JSON serialization | Manual `write!` to string | `serde` + `serde_json` | Handles escaping, nesting, schema evolution |
| Version drift across crates | Per-crate `[dependencies]` version strings | `[workspace.dependencies]` + `{ workspace = true }` | Single source of truth; no split-brain upgrades |

**Key insight:** Phase 1 is scaffolding, not logic. The value is locking the _shape_, not implementing behavior. Every hour spent on hand-rolled infrastructure is an hour not spent verifying the architectural invariant.

---

## Common Pitfalls

### Pitfall 1: I/O Leaking into runtime-core

**What goes wrong:** A developer adds `impl fmt::Display for Intent` that calls `std::io::stdout()`, or adds a `fn load_from_file()` helper. The crate compiles but violates the no-I/O constraint.
**Why it happens:** `fmt::Display` and `impl std::fmt::Debug` are fine — they don't involve I/O. The line is any import of `std::io`, `tokio`, `std::fs`, `std::net`, or any async runtime.
**How to avoid:** Add a CI check: `grep -r "std::io\|tokio\|std::fs\|std::net\|async fn" crates/runtime-core/src/` must return nothing.
**Warning signs:** Compiler accepting `use std::io::Write` in runtime-core.

### Pitfall 2: EffectRequest Bypass Path

**What goes wrong:** Someone adds a convenience function `fn execute_effect(effect: EffectRequest)` in brokerd alongside the stub, planning to "wire it up later." This bakes in the banned path.
**Why it happens:** It seems like a natural helper for testing. It's not — it's an architectural violation.
**How to avoid:** The brokerd crate in Phase 1 contains ONLY `submit_plan_node`. Nothing else effect-related.
**Warning signs:** Any `EffectRequest` type appearing anywhere in the crate tree.

### Pitfall 3: ValueNode Missing Taint Field

**What goes wrong:** ValueNode is defined as `{ literal: Value, provenance: Provenance }` without the `taint: Vec<TaintLabel>` field. Taint tracking is patched in later, requiring a breaking API change.
**Why it happens:** "We don't need taint yet, we'll add it in Phase 4." The whole point of Phase 1 is to lock this shape.
**How to avoid:** `ValueNode` MUST have `literal + provenance + taint` from the first commit. This is a success criterion.
**Warning signs:** Reviewing the ValueNode struct and not seeing `pub taint: Vec<TaintLabel>`.

### Pitfall 4: Non-Virtual Workspace Root

**What goes wrong:** Adding `[package] name = "agentos"` to the root `Cargo.toml`. Now the root is both workspace AND crate, which complicates `cargo build --workspace` and violates the repo layout decision.
**Why it happens:** `cargo init` defaults to creating a package; workspace setup is a separate step.
**How to avoid:** Root `Cargo.toml` contains ONLY `[workspace]`, `[workspace.package]`, and `[workspace.dependencies]`. Verify: `cat Cargo.toml | grep -c "\[package\]"` must return 0.
**Warning signs:** `cargo build` in root builds a binary named `agentos`.

### Pitfall 5: serde_json in runtime-core

**What goes wrong:** `ValueNode.literal` typed as `serde_json::Value` pulls `serde_json` into `runtime-core`, making it a heavier dependency than needed.
**Why it happens:** `serde_json::Value` is convenient for "any JSON value." It's fine to use it here since runtime-core has no I/O and serde_json has no I/O in its core.
**Resolution:** `serde_json` as a dependency of `runtime-core` is acceptable since it performs no I/O. The no-I/O constraint means no runtime I/O operations, not "zero transitive dependencies." Confirm this is understood.

---

## Code Examples

### Workspace Root Cargo.toml

```toml
# Source: doc.rust-lang.org/cargo/reference/workspaces.html [VERIFIED]
[workspace]
members  = ["crates/*", "cli/caprun"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
serde      = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.150"
uuid       = { version = "1.23.4",  features = ["v4", "serde"] }
thiserror  = "2.0.18"
chrono     = { version = "0.4.45",  features = ["serde"] }
anyhow     = "1.0.103"
```

### runtime-core/src/lib.rs Skeleton

```rust
// Source: planning-docs/PLAN.md + archive/multi-part/core-arch.md [CITED]
// No I/O: no use of std::io, tokio, std::fs, std::net, or async
pub mod intent;
pub mod session;
pub mod effect;
pub mod artifact;
pub mod event;
pub mod plan_node;
pub mod executor_decision;

// Re-export the primary public types
pub use intent::{Intent, IntentStatus};
pub use session::{Session, SessionStatus};
pub use effect::{Effect, ObserveEffect, ReversibleEffect, IrreversibleEffect};
pub use artifact::{Artifact, ArtifactRef};
pub use event::Event;
pub use plan_node::{PlanNode, ValueNode, SinkId, TaintLabel, Provenance};
pub use executor_decision::ExecutorDecision;
```

### Intent Struct

```rust
// Source: archive/multi-part/core-arch.md [CITED]
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Intent {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub description: String,
    pub created_by: String,   // Principal — simple string for v0
    pub status: IntentStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IntentStatus {
    Active,
    WaitingApproval,
    Completed,
    RolledBack,
    Abandoned,
    Failed,
}
```

### Session Struct

```rust
// Source: archive/multi-part/core-arch.md (ExecutionContext → Session rename) [CITED]
// Note: ExecutionContext is INTERNAL only; public API uses Session (DEC-terminology)
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SessionStatus {
    Active,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
```

### Event Struct (Audit DAG Node)

```rust
// Source: archive/AGENT-RUNTIME-HANDOVER.md §4.7 [CITED]
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,     // causal DAG link
    pub session_id: Uuid,
    pub actor: String,               // Principal
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub taint: Vec<super::plan_node::TaintLabel>,
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Cargo workspace resolver v1 | resolver = "3" (Rust 2024) | Rust 1.84 / 2024 edition | Better feature unification; required for edition 2021+ virtual workspaces |
| Per-crate `[dependencies]` version strings | `[workspace.dependencies]` + `{ workspace = true }` | Rust 1.64 (2022) | Single version source; eliminates drift across crates |
| Flat error strings | `#[derive(thiserror::Error)]` | Established pattern | Typed errors, matched by callers |

**Deprecated/outdated:**
- `edition = "2018"` in new crates: Use 2021 or 2024. Edition 2021 is the stable default as of this Rust toolchain (1.92.0).
- `resolver = "1"` or `"2"`: Resolver 3 is current for Rust 2024 edition.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `ExecutorDecision::NotImplemented` is the right return shape (vs. `Result<(), NotImplementedError>`) | Code Examples, Pattern 5 | Minor API refactor in Phase 4; the enum is more composable so this is likely right |
| A2 | `serde_json::Value` is acceptable in runtime-core for ValueNode.literal | Common Pitfalls | If policy says "no serde_json in core", use a newtype or a custom Value enum instead |
| A3 | cli/caprun stub (`Cargo.toml` scaffold only) is in scope for Phase 1 | Architecture Patterns | If caprun is Phase 3+, the workspace members glob still picks it up — no harm either way |

**Assumptions to confirm:** A1 (ExecutorDecision shape) is worth a quick verbal confirm before coding brokerd. The other two are low-stakes.

---

## Open Questions

1. **ExecutorDecision return type: enum variant vs. Result**
   - What we know: PLAN.md says "returns `NotImplemented`" — implies a value, not a panic.
   - What's unclear: Whether the signature is `-> ExecutorDecision` or `-> Result<ExecutorDecision, BrokerError>`.
   - Recommendation: Start with `-> ExecutorDecision` for the stub; wrapping in `Result` is a one-line change when brokerd grows real error paths in Phase 3.

2. **Should `cli/caprun` be a stub Cargo.toml in Phase 1?**
   - What we know: PLAN.md shows `cli/caprun` in the workspace. It's not used until Phase 3+.
   - What's unclear: Whether Phase 1 should create the directory and empty Cargo.toml or leave it for Phase 3.
   - Recommendation: Create the directory and minimal Cargo.toml now so `cargo build --workspace` never fails on a missing member.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / rustc | All crates | ✓ | 1.92.0 (2025-12-08) | — |
| Cargo | Workspace | ✓ | 1.92.0 | — |
| crates.io access | `cargo build` fetching deps | ✓ | (confirmed by cargo search) | — |

**Missing dependencies with no fallback:** none — toolchain is ready.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | none — no external test framework needed for pure types |
| Quick run command | `cargo test -p runtime-core` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-runtime-core | Intent, Session, Event, Artifact, 3-class Effect enums compile | compile (no explicit test fn needed) | `cargo build -p runtime-core` | ❌ Wave 0 |
| REQ-runtime-core | runtime-core has zero I/O imports | compile + grep assertion | `grep -r "std::io\|tokio\|std::fs\|std::net\|async fn" crates/runtime-core/src/` exits 1 | ❌ Wave 0 |
| REQ-api-stub-plan-node | `submit_plan_node` exists and returns `NotImplemented` | unit | `cargo test -p brokerd -- submit_plan_node_returns_not_implemented` | ❌ Wave 0 |
| REQ-api-stub-plan-node | No raw EffectRequest→sink path in crate tree | grep assertion | `grep -r "EffectRequest" crates/` exits 1 | ❌ Wave 0 |
| REQ-api-stub-plan-node | ValueNode has literal + provenance + taint fields | compile / struct field check | `cargo build -p runtime-core` (fields missing = compile error) | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo build --workspace && cargo test -p runtime-core`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** `cargo test --workspace` green + negative grep assertions pass before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `crates/runtime-core/src/` — entire crate (greenfield)
- [ ] `crates/brokerd/src/lib.rs` — `submit_plan_node` stub
- [ ] `Cargo.toml` — workspace virtual manifest
- [ ] `crates/runtime-core/tests/` — basic compile + field-presence tests (optional but useful)

*(No existing test infrastructure — all Wave 0)*

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — (no auth in Phase 1) |
| V3 Session Management | no | — (Session is a data type only; no lifecycle in Phase 1) |
| V4 Access Control | no | — (enforcement deferred to Phase 4) |
| V5 Input Validation | partial | ValueNode literal is `serde_json::Value` — callers validate shape; no raw string injection surface in Phase 1 |
| V6 Cryptography | no | — (ed25519-dalek deferred to Phase 3+) |

### Security-Relevant Decisions in Phase 1

The architectural invariant that Phase 1 must enforce structurally:

> **There must be no `EffectRequest`→sink path anywhere in the crate tree.** The type `EffectRequest { effect, args: Map }` must not exist as a pub API surface. All effects are mediated through `PlanNode { sink, args: Vec<ValueNode> }`.

This is not a runtime security check — it is a **type-system constraint enforced by the absence of the wrong type.** The planner verifies it at review time, not test time.

The taint fields on `ValueNode` are security-critical infrastructure: without `taint: Vec<TaintLabel>` on the type from day one, Phase 4's I2 enforcement cannot work correctly (it would need a breaking schema change).

---

## Sources

### Primary (MEDIUM confidence — authoritative project docs)
- `planning-docs/PLAN.md` — canonical architecture, locked decisions, API shape, build order
- `archive/multi-part/core-arch.md` — concrete Rust type sketches for Intent, Session, Effect, Artifact, Taint
- `archive/AGENT-RUNTIME-HANDOVER.md §4.5, §4.7` — ValueNode/PlanNode rationale, Event audit DAG schema

### Secondary (MEDIUM confidence — verified official docs)
- [doc.rust-lang.org/cargo/reference/workspaces.html](https://doc.rust-lang.org/cargo/reference/workspaces.html) — workspace manifest syntax, members, workspace.dependencies, resolver

### Tertiary (MEDIUM confidence — verified via cargo search)
- crates.io registry: serde 1.0.228, uuid 1.23.4, thiserror 2.0.18, chrono 0.4.45, anyhow 1.0.103, serde_json 1.0.150, tokio 1.52.3 — all OK via package-legitimacy check

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crate versions verified via `cargo search` and legitimacy check
- Architecture: HIGH — domain types derived verbatim from locked PLAN.md and archive docs; no design freedom
- Pitfalls: MEDIUM — derived from architectural constraints + Rust workspace experience

**Research date:** 2026-06-29
**Valid until:** 2026-09-29 (stable Rust ecosystem; crate versions advance slowly)
