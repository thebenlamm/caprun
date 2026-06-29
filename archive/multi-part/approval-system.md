# Approval, Policy, Audit, Memory, and Provenance

This document describes what is likely the most important—and most differentiated—part of the runtime.

Most systems treat approval as an authorization check.

This runtime treats approval as a first-class interaction model.

The authorization engine is security infrastructure.

The approval system is product.

---

# The Product Is Not Security

Traditional security systems ask:

```text
Allow?

[Y/N]
```

This is acceptable for administrators.

It is disastrous for autonomous systems.

An agent runtime may execute thousands of effects every day.

If every effect requires approval, the runtime has failed.

The objective is not:

> Ask permission safely.

The objective is:

> Avoid asking permission.

The human should only appear when:

* something irreversible is about to happen,
* the runtime genuinely lacks enough authority,
* or policy intentionally requires oversight.

Success means the runtime learns enough structure that 95%+ of sessions complete without interruption.

---

# Approval Is Session State

Approvals do not belong to workers.

They do not belong to capabilities.

They belong to the Session.

```text
Intent
    ↓
Session
    ↓
Approval State
```

Every approval modifies the session.

Example:

```text
Intent:
Fix failing tests

Session:
Approved:
    - write workspace
    - run tests

Pending:
    - create PR

Denied:
    - git push main
```

This means approvals naturally become part of:

* audit
* provenance
* replay
* undo
* future planning

---

# Approval Classes

Effects should not individually decide whether they require approval.

Policy does.

Each effect falls into one of four classes.

```rust
pub enum ApprovalClass {
    Auto,
    AutoAfterCondition,
    HumanWithContext,
    Never,
}
```

Examples:

```text
Observe

Read workspace
Auto

Run tests
Auto

--------------------------------

Mutate Reversible

Apply patch
Auto

Write report
Auto

--------------------------------

Commit Irreversible

Create PR
HumanWithContext

Push branch
HumanWithContext

Deploy staging
HumanWithContext

Purchase
HumanWithContext

Delete production database
Never
```

The planner proposes.

Policy classifies.

The broker enforces.

---

# Approval Conditions

Many approvals should disappear once a condition has been satisfied.

Examples:

Instead of:

```text
Approve every PR.
```

Use:

```text
Approve PR creation
if

tests passed

AND

diff < 200 lines

AND

no production config changed

AND

confidence > 0.9
```

This dramatically reduces human involvement.

---

# Rich Approval

The runtime should never ask:

```text
Approve Git Push?
```

Instead:

```text
Session:
Fix retry bug in payment module

Planner wants:

Create Pull Request

Reason:

All failing tests now pass.

Diff:

+42
-11

Files:

payment.rs
retry.rs

Confidence:

94%

Similar approvals:

Approved 27 times previously

Blast radius:

Repository only

Choices:

Approve once

Approve all PR creation
for this session

Approve all PR creation
after tests pass

Reject

Explain
```

This is not merely security.

This is workflow.

---

# Structured Responses

Approval replies should themselves be structured.

Example:

```rust
pub enum ApprovalResponse {
    ApproveOnce,
    ApproveUntilSessionEnds,
    ApproveMatchingPolicy,
    Reject,
    RejectWithReason(String),
}
```

Eventually:

```rust
ApproveAfterTestsPass

ApproveForRepository

ApproveForRecipient

ApproveBelowDollarAmount

ApproveUntil(DateTime)
```

Humans teach policy through approvals.

---

# Policy Learning

The runtime should observe repeated approvals.

Example:

Twenty approvals:

```text
Create PR

after

tests pass

repository=myrepo
```

The runtime proposes:

> You have approved this action twenty-one consecutive times.
>
> Create a standing policy?

The runtime never silently expands authority.

It proposes.

The human installs.

---

# Policy Engine

Policy should be declarative.

Not embedded in Rust code.

Eventually something like:

```text
permit
if

effect == CreatePullRequest

AND

repository in trusted

AND

tests_passed

AND

diff_lines < 250

AND

session.owner == Ben
```

Initially:

A simple TOML or YAML rules file is sufficient.

Do not overbuild this.

The runtime should own the semantics.

The policy language can evolve later.

---

# Budgets

Sessions own budgets.

Budgets are another form of authority.

Examples:

```rust
pub struct Budget {
    pub usd_remaining: Decimal,
    pub llm_tokens_remaining: u64,
    pub api_calls_remaining: u64,
    pub wall_clock_remaining: Duration,
}
```

Effects consume budget.

Budget exhaustion becomes another reason effects may be denied.

Examples:

```text
Run GPT-5

Estimated cost:

$0.42

Remaining:

$0.81

Allowed.
```

Later:

```text
Remaining:

$0.11

Denied.

Suggest cheaper model.
```

Budgets become planning input.

---

# Session Memory

Memory is frequently discussed in agent systems.

Most designs focus on conversation history.

This runtime treats memory differently.

Memory belongs to the Session.

Example:

```rust
pub struct SessionMemory {
    pub artifacts: Vec<Artifact>,
    pub observations: Vec<Observation>,
    pub planner_notes: Vec<PlannerNote>,
    pub decisions: Vec<Decision>,
}
```

Memory is temporary.

When the session ends, memory ends.

Long-term knowledge belongs elsewhere.

---

# Runtime Memory vs Long-Term Memory

The runtime should distinguish:

Runtime memory

```text
What has happened
during this session?
```

Persistent memory

```text
What has been learned
across many sessions?
```

These are fundamentally different.

The runtime should only own the first.

---

# Audit

Traditional audit logs are append-only event streams.

That is insufficient.

The runtime needs a causal graph.

Example:

```text
Intent

↓

Planner Decision

↓

Worker

↓

Artifact

↓

Effect

↓

Approval

↓

Commit
```

Every object references the object that caused it.

---

# Event Model

Example:

```rust
pub struct Event {
    pub id: EventId,
    pub session: SessionId,
    pub parent: Option<EventId>,
    pub actor: Principal,
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
}
```

Examples:

```text
Planner proposed effect

↓

Broker approved

↓

Worker executed

↓

Artifact created

↓

Approval requested

↓

Approval granted

↓

PR created
```

This is a graph.

Not merely a timeline.

---

# Provenance

Everything produced by the runtime should answer:

```text
Why does this exist?
```

Artifacts should know:

* which worker created them
* under which session
* under which intent
* from which inputs
* after which approvals

This enables replay.

It also enables debugging.

---

# Undo

Undo belongs to Sessions.

Not effects.

A session accumulates reversible changes.

Eventually:

```text
Rollback Session
```

becomes possible.

Examples:

```text
Delete generated artifacts

Restore edited files

Revert patches

Close draft PR
```

Irreversible effects remain irreversible.

But the runtime should maximize what can be undone.

---

# Session Timeline

Every session should naturally produce:

```text
Intent

↓

Planner

↓

Worker A

↓

Artifact

↓

Planner

↓

Worker B

↓

Tests

↓

Approval

↓

Commit

↓

Done
```

This timeline becomes:

* audit
* debugging
* explanation
* replay
* documentation

A human should be able to inspect a completed session and understand exactly why every external effect occurred.

---

# Discovery and Approval Are Connected

Discovery should not simply answer:

```text
What can I do?
```

It should answer:

```text
What can I do?

What would require approval?

What is likely to succeed?

What should I do next?
```

Example:

```text
Available

✓ Run tests

✓ Apply patch

✓ Generate documentation

⚠ Create PR
(approval required)

⚠ Push branch
(approval required)

✓ Generate changelog
```

This turns discovery into a planning primitive rather than a permission list.

---

# Design Principle

The runtime should optimize for uninterrupted autonomous execution.

Approval is not evidence that the runtime is safe.

Approval is evidence that the runtime failed to derive sufficient authority automatically.

The best approval interaction is the one that never occurs.

The approval system therefore has two goals:

1. Protect irreversible external effects.
2. Learn enough structure that future sessions no longer require interruption.

That makes approval not merely a security mechanism, but an adaptive interface between human intent and autonomous execution.
