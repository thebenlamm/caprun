# Core Architecture

The runtime is organized around the following hierarchy:

```text
Intent
  ↓
ExecutionContext
  ↓
Planner
  ↓
Workers
  ↓
Broker
  ↓
Sandbox
  ↓
OS / External Services
```

Each layer has a distinct job.

---

# Intent

An **Intent** is the root object.

It represents what the user or parent agent is trying to accomplish.

Examples:

```text
Fix the failing tests in the payment module.

Summarize this hostile PDF and create a report.

Draft a reply to Golda about the BAA issue.

Research GoDaddy DNS limitations and propose the safest migration path.

Analyze this repository and open a pull request if the tests pass.
```

The intent owns the “why.”

Everything else is derived from it.

Conceptual Rust model:

```rust
pub struct Intent {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub description: String,
    pub created_by: Principal,
    pub status: IntentStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum IntentStatus {
    Active,
    WaitingApproval,
    Completed,
    RolledBack,
    Abandoned,
    Failed,
}
```

Important rule:

```text
Every effect belongs to exactly one Intent.
```

This is the grouping key for:

* audit
* approval
* budget
* memory
* undo
* delegation
* revocation
* provenance

---

# ExecutionContext

An **ExecutionContext** is the live runtime container for an intent.

It is not a Linux container, though it may own Linux sandboxes.

It is the runtime object that tracks:

* what workers exist
* what effects are currently allowed
* what approvals have been granted
* what budget remains
* what artifacts have been created
* what raw inputs were consumed
* what outputs were produced
* what can be undone
* what has already happened

Conceptual model:

```rust
pub struct ExecutionContext {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub status: ContextStatus,
    pub allowed_effects: Vec<EffectGrant>,
    pub approval_policy_id: PolicyId,
    pub budget: BudgetTracker,
    pub memory: ContextMemory,
    pub undo_log: Vec<UndoEntry>,
    pub workers: Vec<WorkerRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum ContextStatus {
    Active,
    WaitingApproval,
    Done,
    Failed,
    RolledBack,
}
```

The context owns the operational state.

The intent owns the purpose.

This distinction matters.

Multiple contexts may eventually exist under one intent:

```text
Intent: Fix failing tests

Context A: investigation
Context B: implementation
Context C: verification
Context D: PR creation
```

For v0, use one context per intent.

---

# Planner

The **Planner** decides what to try next.

It may be:

* hard-coded
* rule-based
* LLM-based
* multi-agent
* human-driven

For v0, do not start with an elaborate planner.

Use a hard-coded planner or a single simple LLM call.

The planner should not directly access the filesystem, network, secrets, shell, or external services.

It proposes **Effects**.

Example:

```rust
pub struct EffectRequest {
    pub context_id: Uuid,
    pub requested_by: Principal,
    pub effect: Effect,
    pub justification: String,
    pub evidence_refs: Vec<ArtifactRef>,
}
```

The planner asks:

```text
Given this intent and current context, what effects are available?
```

This is the role of discovery.

---

# Effects

An **Effect** is the semantic unit of action.

The planner should not think in terms of syscalls, pathnames, sockets, or raw API credentials.

It should think in terms of effects.

For v0, use only three top-level effect classes:

```rust
pub enum Effect {
    Observe(ObserveEffect),
    MutateReversible(ReversibleEffect),
    CommitIrreversible(IrreversibleEffect),
}
```

This avoids premature ontology explosion.

The important distinction is not filesystem vs process vs network.

The important distinction is:

```text
Can this hurt the outside world?

Can this be undone?
```

Examples:

```rust
pub enum ObserveEffect {
    ReadWorkspaceFile { path: RelativePathBuf },
    ListWorkspace { path: RelativePathBuf },
    RunTests { command: TestCommand },
    SummarizeArtifact { artifact_id: Uuid },
}

pub enum ReversibleEffect {
    WriteArtifact { name: String, content_ref: ArtifactRef },
    ApplyPatch { patch_ref: ArtifactRef },
    EditWorkspaceFile { path: RelativePathBuf, patch_ref: ArtifactRef },
}

pub enum IrreversibleEffect {
    SendEmail { draft_ref: ArtifactRef, to: Vec<String> },
    GitPush { remote: String, branch: String },
    CreatePullRequest { title: String, body_ref: ArtifactRef },
    DeployService { service: String, environment: String },
    Purchase { vendor: String, amount_usd: Decimal },
}
```

Do not expose Linux resource details to the planner unless absolutely necessary.

The adapter layer can translate high-level effects into low-level operations.

---

# Broker

The **Broker** is not the whole system.

It is the authorization and mediation subsystem.

Its jobs:

* authorize effects against context
* enforce budget and approval state
* mint kernel capabilities where possible
* proxy or execute irreversible effects
* create audit entries
* manage worker sandbox lifecycle
* maintain revocation state
* call approval hooks

The broker receives:

```rust
pub struct BrokerEffectRequest {
    pub context_id: Uuid,
    pub worker_id: Option<Uuid>,
    pub effect: Effect,
    pub justification: String,
}
```

It returns:

```rust
pub enum BrokerDecision {
    Allowed(EffectPermit),
    Denied(DenyReason),
    RequiresApproval(ApprovalRequest),
}
```

The broker is a reference monitor.

It must sit on the only path to side effects.

That means the sandbox must prevent the worker from bypassing it.

---

# Sandbox

The sandbox enforces the claim:

```text
Workers have no ambient authority.
```

On Linux v1, use:

* user namespace
* mount namespace
* network namespace
* pid namespace where practical
* Landlock for filesystem restrictions
* seccomp-bpf for syscall filtering
* cgroups for CPU/memory/process limits
* Unix domain socket for broker IPC
* SCM_RIGHTS for passing file descriptors

Worker process rule:

```text
The worker starts with almost nothing:
- no network
- no arbitrary filesystem
- no secrets
- no shell authority
- one broker socket
- any explicit file descriptors granted by broker
```

This is where the security boundary lives.

Not in the token.

Not in the prompt.

Not in the client library.

---

# Workers

A **Worker** performs a bounded piece of execution.

Workers may be:

* LLM processes
* deterministic tools
* Python scripts
* Rust binaries
* test runners
* parsers
* summarizers

Each worker belongs to exactly one ExecutionContext.

```rust
pub struct WorkerRef {
    pub id: Uuid,
    pub context_id: Uuid,
    pub kind: WorkerKind,
    pub pid: Option<Pid>,
    pub pidfd: Option<RawFd>,
    pub status: WorkerStatus,
    pub sandbox_profile: SandboxProfileId,
}

pub enum WorkerKind {
    Planner,
    QuarantinedReader,
    ArtifactWriter,
    TestRunner,
    Executor,
}
```

Important invariant:

```text
No worker should both ingest raw hostile content and hold irreversible authority.
```

That is the prompt-injection defense line.

---

# Privilege-Separated Cognition

Prompt injection is not solved by scoped tokens alone.

If an agent legitimately holds a powerful capability, malicious content can cause it to misuse that capability inside scope.

Therefore the runtime must separate:

```text
reading hostile content
```

from

```text
deciding and committing irreversible effects
```

Architecture:

```text
Privileged Planner
  - sees user intent
  - sees schemas
  - sees trusted summaries
  - may request irreversible effects
  - does not see raw hostile content

Quarantined Worker
  - sees raw hostile content
  - extracts typed facts
  - cannot send, delete, deploy, spend, or write externally

Executor
  - executes approved effects
  - may be deterministic where possible
```

Central invariant:

```text
No LLM context may simultaneously contain:
1. raw untrusted instructions/content, and
2. authority to perform irreversible external effects.
```

This is more important than the token design.

---

# Provenance and Taint

Every artifact must carry provenance.

Example:

```rust
pub struct Artifact {
    pub id: Uuid,
    pub context_id: Uuid,
    pub created_by: Principal,
    pub artifact_type: ArtifactType,
    pub taint: Vec<TaintLabel>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}
```

Example taint labels:

```rust
pub enum TaintLabel {
    UserTrusted,
    LocalWorkspace,
    ExternalWeb,
    EmailRaw,
    PdfRaw,
    LlmGenerated,
    WorkerExtracted,
}
```

The planner may consume trusted summaries or typed extracts.

It should not consume raw hostile data if it will later request irreversible effects.

Worker output should be schema-validated and non-instructional.

Example:

```json
{
  "source_artifact": "artifact_123",
  "taint": ["EmailRaw", "WorkerExtracted"],
  "claims": [
    {
      "type": "deadline",
      "value": "2026-07-03",
      "evidence_hash": "sha256:...",
      "confidence": 0.82
    }
  ],
  "stripped": {
    "instructions_to_agent": true,
    "tool_requests": true
  }
}
```

For v0, taint can be simple labels.

Do not build full information-flow control yet.

But design the data model so it can grow into that.
