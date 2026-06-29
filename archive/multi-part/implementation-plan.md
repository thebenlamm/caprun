# Implementation Plan

This document describes how to build the runtime.

The objective is not to build a platform.

The objective is to build the smallest useful implementation that validates the architecture.

The implementation strategy deliberately favors simplicity over completeness.

The runtime should solve one real problem extremely well before expanding.

---

# Design Goals

The implementation should satisfy the following constraints.

### Simple

One machine.

One broker.

SQLite.

Unix sockets.

No distributed consensus.

---

### Useful

Must immediately improve an existing workflow.

The first target is coding agents.

Not general desktop automation.

Not browser agents.

Not office assistants.

---

### Replaceable

Every subsystem should be replaceable.

Planner.

Policy engine.

Adapters.

LLM.

Persistence.

Sandbox backend.

---

### Observable

Every decision should be explainable.

No hidden state.

No invisible authority.

Everything belongs to a Session.

---

# Technology Stack

Language:

Rust

Reasons:

* memory safety
* Linux systems programming
* async ecosystem
* strong type system
* good FFI
* excellent Unix support

Python remains acceptable for:

* experimental workers
* adapters
* research
* temporary tooling

The runtime core should remain Rust.

---

# Initial Dependencies

Core:

```text id="wvb0i9"
tokio

serde

serde_json

uuid

chrono

thiserror

tracing

anyhow
```

Security:

```text id="gctdd7"
ed25519-dalek

rand

zeroize
```

Linux:

```text id="bajw06"
nix

rustix

landlock

seccompiler
```

Persistence:

```text id="hqk7rk"
sqlx

SQLite
```

IPC:

```text id="86pyzn"
Unix domain sockets

SCM_RIGHTS

pidfd
```

Do not add:

Kafka.

Redis.

Postgres.

Message buses.

Those can come later.

---

# Repository Layout

Suggested structure:

```text id="e5pn74"
runtime/

    crates/

        runtime-core/

        broker/

        sandbox/

        planner/

        adapters/

            fs/

            git/

            process/

            github/

            gmail/

        policy/

        audit/

        approval/

        worker/

        cli/

        common/
```

Keep the crates small.

Avoid circular dependencies.

---

# Runtime Core

runtime-core defines the shared model.

Nothing here performs I/O.

Contains:

```text id="itkjh7"
Intent

Session

Effect

Artifact

Budget

Approval

Event

WorkerRef

Identifiers

Enums

Traits
```

Everything else depends on this crate.

---

# Broker

Responsible for:

* Session lifecycle
* effect authorization
* worker creation
* adapter dispatch
* fd passing
* approval requests
* audit generation

Should contain almost no business logic.

The broker coordinates.

It should not become a "god object."

---

# Planner

Initially:

One function.

```rust id="tew8ku"
plan(
    session: &Session
) -> Vec<EffectRequest>
```

It may call an LLM.

Or may be deterministic.

The runtime should not care.

The planner is replaceable.

---

# Worker

Workers execute one bounded task.

Example trait:

```rust id="13qhl7"
trait Worker {

    async fn execute(
        &self,
        session: SessionId,
    ) -> WorkerResult;

}
```

Workers should be disposable.

Do not build long-running workers initially.

---

# Adapters

Each adapter converts semantic effects into implementation.

Filesystem adapter:

```text id="jlwmam"
ReadWorkspace

↓

open()

↓

fd
```

Git adapter:

```text id="scm4dj"
CreatePR

↓

GitHub API
```

Every adapter owns exactly one domain.

Avoid giant generic adapters.

---

# Sandbox

Sandbox creation becomes a dedicated crate.

Responsibilities:

```text id="uxv6h6"
Create namespaces

Apply Landlock

Install seccomp

Mount workspace

Create broker socket

Launch worker

Return pidfd
```

The broker requests sandboxes.

Sandbox crate creates them.

---

# IPC

Communication:

Unix domain sockets.

Messages:

```rust id="8n7b7y"
enum RuntimeMessage {

    Discover,

    RequestEffect,

    EffectResult,

    ApprovalResult,

    Shutdown,

}
```

Use structured serialization.

Avoid ad hoc protocols.

---

# File Descriptor Passing

The happy path:

Broker:

```text id="zbo6c3"
open()

↓

sendmsg()

↓

SCM_RIGHTS
```

Worker:

```text id="htxhkj"
recvmsg()

↓

fd

↓

read()
```

Broker leaves the hot path.

This should become the preferred pattern.

---

# Persistence

SQLite is enough.

Suggested tables.

Intent

```text id="0v2vt4"
intent

id

description

status

timestamps
```

Session

```text id="6wlpd5"
session

id

intent_id

status

policy

budget
```

Artifacts

```text id="j4n2g5"
artifact

id

session

worker

hash

taint

type
```

Events

```text id="lvzhso"
event

id

parent

session

actor

type

timestamp
```

Approvals

```text id="sm31jy"
approval

id

session

effect

decision

user

timestamp
```

Workers

```text id="bcjlwm"
worker

id

session

pidfd

status
```

Keep schema intentionally boring.

SQLite scales surprisingly far.

---

# Session Lifecycle

```
Create Intent

↓

Create Session

↓

Planner proposes

↓

Broker authorizes

↓

Spawn Worker

↓

Worker executes

↓

Artifacts created

↓

Planner continues

↓

Approval

↓

Commit

↓

Session complete
```

Every transition becomes an Event.

---

# CLI

The CLI is the first UI.

Examples:

```bash id="dk09h6"
runtime create-intent

runtime sessions

runtime inspect SESSION

runtime approve EFFECT

runtime deny EFFECT

runtime replay SESSION

runtime events SESSION
```

Do not build a web UI first.

The CLI will reveal what abstractions are actually needed.

---

# Discovery

The planner interacts through:

```rust id="drl3cq"
discover(
    session
)
```

Returns:

* available effects
* remaining budget
* approval requirements
* previous artifacts

Discovery should become the planner's primary API.

---

# Logging

Use structured logs.

Not println.

Every log line should contain:

```text id="f7yqpm"
session_id

intent_id

worker_id

effect

event_id
```

Never log secrets.

---

# Configuration

Single config file.

Example:

```toml id="q4l4bs"
[planner]

provider = "openai"

model = "gpt-5"

[workspace]

root = "/projects"

[sandbox]

enabled = true

[audit]

database = "./runtime.db"
```

Keep configuration human-readable.

---

# Testing Strategy

Three layers.

## Unit

Pure Rust.

Models.

Policy.

Planner.

No I/O.

---

## Integration

Broker.

Sandbox.

SQLite.

Unix sockets.

Real fd passing.

---

## End-to-End

Create Session.

Run worker.

Approve effect.

Verify audit.

These tests should become the primary correctness signal.

---

# Smallest Useful Milestone (Two Weeks)

By the end of two weeks the runtime should be able to:

1. Create an Intent.

2. Start a Session.

3. Spawn one sandboxed worker.

4. Discover available effects.

5. Authorize:

```text id="fycqyg"
ReadWorkspace
```

6. Open a file.

7. Pass the fd using SCM_RIGHTS.

8. Worker reads file directly.

9. Worker creates an artifact.

10. Planner requests one irreversible effect.

11. Runtime asks for approval.

12. Human approves.

13. Runtime records complete provenance.

14. Session completes.

No LLM sophistication is required.

The architecture—not the intelligence—is what is being validated.

---

# Things Explicitly Deferred

Do **not** build these yet.

Distributed Sessions.

Multiple brokers.

Biscuit integration.

Cross-machine delegation.

Long-term memory.

Learning planner.

GUI.

Plugin ecosystem.

Marketplace.

Scheduling.

Cloud deployment.

Natural language policy authoring.

Undo snapshots.

Billing.

Complex effect ontology.

The temptation will be to build a platform.

Resist it.

---

# Success Criteria

Version 0 is successful if it demonstrates one complete execution loop:

```text id="5zv2am"
Intent

↓

Session

↓

Planner

↓

Worker

↓

Broker

↓

Linux sandbox

↓

Approval

↓

Audit

↓

Done
```

If that loop feels coherent, understandable, and safer than today's agent frameworks, the architecture is validated.

Everything else is iterative refinement.
