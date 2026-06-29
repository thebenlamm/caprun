# Broker, Capabilities, Discovery, and Kernel Integration

> The broker is not the runtime.
>
> The broker is not the security boundary.
>
> The broker is the runtime's authorization and mediation service.

---

# The Role of the Broker

The broker exists for one purpose:

**Authorize effects on behalf of a Session.**

It does not own planning.

It does not own memory.

It does not own the user experience.

It does not execute arbitrary business logic.

It answers one question:

> "Given this session, should this effect be allowed?"

If yes, it either:

* grants a kernel capability,
* executes the effect itself,
* or mediates access to an external system.

---

# The Broker as Reference Monitor

The broker is a classic **reference monitor**.

Every externally visible effect must pass through it.

Examples:

```text
Read workspace file
↓

Broker

↓

open()

↓

fd passed to worker
```

```text
Git push

↓

Broker

↓

Git adapter

↓

Remote
```

```text
Send email

↓

Broker

↓

Gmail adapter

↓

SMTP/API
```

The worker cannot bypass this path because the sandbox removes all ambient authority.

---

# Effects, Not Resources

The planner should never think in terms of:

```text
open()

connect()

write()

execve()
```

Nor should it think in terms of:

```text
/home/ben/project/src/main.rs

tcp://api.github.com

/usr/bin/git
```

Those belong inside adapters.

Instead the planner reasons about effects:

```text
Read source file

Run tests

Apply patch

Generate report

Commit branch

Push branch

Deploy service

Send email
```

The adapter layer translates effects into operating-system operations.

This separation keeps the planner portable.

If Linux changes tomorrow—or Windows receives a different backend—the planner remains unchanged.

---

# Adapter Architecture

The broker does not contain filesystem logic.

Nor network logic.

Nor Git logic.

Everything outside authorization lives inside adapters.

```text
               Broker
                  │
      ┌───────────┼────────────┐
      │           │            │
Filesystem    Process      External APIs
 Adapter       Adapter        Adapter
      │           │            │
 Linux FS     execve()     GitHub/Gmail/etc
```

Each adapter exposes:

```rust
trait Adapter {
    fn discover(...);

    fn authorize(...);

    fn execute(...);

    fn rollback(...);
}
```

The broker owns policy.

Adapters own implementation.

---

# Kernel Objects as Native Capabilities

One of the major architectural changes from the original proposal is recognizing that Linux already provides capability-like primitives.

Instead of repeatedly authorizing every operation:

```text
Worker

↓

ReadFile

↓

Broker

↓

verify token

↓

open()

↓

read()

↓

return bytes
```

prefer:

```text
Broker

↓

open()

↓

SCM_RIGHTS

↓

Worker receives fd

↓

Worker performs direct reads
```

The broker authorizes once.

Linux enforces afterward.

This dramatically reduces overhead.

More importantly:

The capability is now a kernel object.

Not merely a signed token.

Examples:

* file descriptor
* pidfd
* eventfd
* timerfd
* signalfd

Whenever possible, the runtime should mint these native capabilities instead of proxying every byte.

---

# Happy Path vs Mediated Path

Not every effect should remain inside the broker forever.

There are two execution paths.

## Happy Path

Effects that can safely become kernel capabilities.

Examples:

```text
Read workspace file

Write temporary artifact

Read directory

Read compiler output

Read generated report
```

Flow:

```text
Broker

↓

Authorize

↓

Open file

↓

Pass fd

↓

Worker uses fd directly
```

The broker leaves the hot path.

---

## Mediated Path

Effects that cannot be represented as kernel capabilities.

Examples:

```text
Git push

Deploy

Purchase

Email

Cloud API mutation

Database migration

Payment
```

Flow:

```text
Worker

↓

Effect request

↓

Broker

↓

Policy

↓

Adapter

↓

External service
```

The broker stays involved.

---

# Capability Handles vs Tokens

One of the biggest architectural revisions concerns capabilities themselves.

The first design assumed:

```text
Everything is a signed capability token.
```

This is no longer true.

Internally:

Prefer lightweight handles.

Externally:

Use signed tokens.

---

## Internal Representation

Inside one runtime instance:

```rust
struct CapabilityHandle {
    id: u64,
}
```

The handle indexes broker state.

Advantages:

* tiny
* fast
* revocable
* no crypto
* no serialization
* easy lookup

Most authority never leaves the broker process.

It does not need signatures.

---

## External Representation

Authority sometimes crosses boundaries.

Examples:

* another machine
* another runtime
* persistence
* asynchronous workers
* EC2 delegation

These require cryptographic integrity.

For those cases export:

```text
CapabilityToken
```

Initially:

Ed25519-signed JSON.

Eventually:

Biscuit.

The runtime should treat tokens as an interchange format.

Not as its primary in-memory representation.

---

# Why Not Tokens Everywhere?

Because they become unnecessary overhead.

If the broker already owns:

* session
* worker
* sandbox
* SQLite
* audit
* revocation

then verifying signatures for every local request buys almost nothing.

The operating system already authenticated the Unix socket.

The broker already knows the worker.

The runtime already knows the session.

Local authority should be stateful.

Distributed authority should be cryptographic.

---

# Discovery

Discovery is one of the most novel parts of the runtime.

Traditional operating systems expose:

```text
filesystem

syscalls

processes

devices
```

Agent runtimes need something different.

The planner asks:

> "Given this session, what can I do next?"

That is not a permission question.

It is a planning question.

---

# Discover API

Conceptually:

```rust
pub fn discover(
    session: SessionId,
) -> DiscoverResponse
```

Returns:

```rust
pub struct DiscoverResponse {
    pub available_effects: Vec<EffectDescriptor>,
    pub suggested_next_steps: Vec<Suggestion>,
    pub requires_approval: Vec<EffectKind>,
    pub auto_grantable: Vec<EffectKind>,
}
```

Example response:

```text
Available

✓ Read workspace

✓ Run tests

✓ Generate report

✓ Apply patch

⚠ Create PR (approval required)

⚠ Push branch (approval required)

✗ Deploy production
```

Notice:

This is not exposing permissions.

It is exposing workflow.

---

# Discovery is Contextual

Discovery changes as the session evolves.

Example:

Initial session:

```text
Read workspace

Run tests

Generate patch
```

After tests pass:

```text
Generate commit

Create PR
```

After PR approved:

```text
Merge

Deploy staging
```

Discovery therefore becomes one of the planner's primary inputs.

---

# Sessions Own Authority

Authority is not attached to workers.

Authority belongs to the session.

Workers temporarily borrow it.

Example:

```text
Intent

↓

Session

↓

Planner

↓

spawn worker

↓

borrow:

Read workspace

Run tests

Return results

↓

worker exits

↓

authority disappears
```

This keeps authority short-lived.

Workers should be disposable.

Sessions persist.

---

# Borrowing Authority

Authority should resemble Rust borrowing more than OAuth.

Worker:

```text
needs

↓

requests effect

↓

broker grants temporary authority

↓

effect completes

↓

authority released
```

Long-lived capabilities should be rare.

Especially for irreversible effects.

---

# Delegation

Sessions may delegate work.

Example:

```text
Planner

↓

spawn Test Worker

↓

borrow:

RunTests

ReadWorkspace

↓

spawn Documentation Worker

↓

borrow:

ReadArtifacts

WriteReport
```

Neither worker receives the session's entire authority.

Each receives only the minimal subset required.

This is capability attenuation applied at the session level.

---

# Broker API

Conceptually:

```rust
authorize_effect(session, effect)

discover(session)

spawn_worker(session, profile)

grant_kernel_capability(...)

delegate(...)

request_approval(...)

audit(...)

rollback(...)
```

Notice what disappeared.

There is no:

```text
VerifyToken()

GrantCapability()

Attenuate()

```

Those still exist internally or for distributed scenarios.

But they are implementation details.

The broker's public interface revolves around **effects** and **sessions**, not tokens.

This is an important conceptual shift.

---

# Core Design Principle

The broker should authorize **effects**, not resources.

The runtime should manipulate **sessions**, not permissions.

Linux should enforce **kernel capabilities**, not repeated policy checks.

Cryptographic tokens should primarily exist for **cross-runtime delegation**, not for every local operation.

This division of responsibility keeps each layer simple:

* Planner reasons about work.
* Session owns authority.
* Broker authorizes effects.
* Adapters translate effects.
* Linux enforces confinement.
* Tokens leave the machine only when authority itself leaves the runtime.
