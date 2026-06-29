# Roadmap, Open Questions, and Design Principles

This document describes how the runtime should evolve after version 0, what problems are intentionally deferred, what research questions remain open, and the architectural principles that should guide every future decision.

The purpose of this document is not to predict the future.

It is to prevent the project from becoming a collection of unrelated features.

Every new capability should reinforce the architecture rather than dilute it.

---

# Guiding Philosophy

This runtime is opinionated.

It is not trying to be the most flexible agent framework.

It is trying to be the safest and most comprehensible runtime for autonomous work.

Whenever two designs are possible, prefer the one that:

* reduces ambient authority,
* shortens authority lifetime,
* makes provenance clearer,
* improves explainability,
* simplifies the mental model.

Do not optimize for feature count.

Optimize for conceptual integrity.

---

# What Success Looks Like

The success metric is **not**:

* number of adapters
* number of supported LLMs
* number of integrations

Instead:

A user should be able to ask:

> "Why did the agent do this?"

…and receive a complete answer.

Likewise:

> "What authority did it have?"

> "Who approved this?"

> "Can I undo it?"

> "What information influenced this decision?"

These questions should always be answerable.

If they are not, the runtime has failed.

---

# Roadmap

## Version 0

Goal:

Validate the runtime architecture.

Features:

* Intent
* Session
* Broker
* Linux sandbox
* One worker
* One planner
* Filesystem adapter
* CLI approval
* Audit log
* Provenance graph

No networking.

No distributed execution.

No long-term memory.

---

## Version 1

Goal:

Useful coding assistant runtime.

New capabilities:

* Git adapter
* GitHub adapter
* Test adapter
* Patch generation
* PR creation
* Workspace snapshots
* Rich approval

Still single machine.

Still SQLite.

Still one broker.

---

## Version 2

Goal:

Multiple workers.

Planner becomes capable of decomposition.

Example:

```text id="yrm6cg"
Planner

↓

Spawn:

Test Worker

Patch Worker

Documentation Worker

Security Review Worker
```

Each receives a narrower Session view.

Parallel execution begins.

---

## Version 3

Goal:

Cross-machine Sessions.

Laptop.

↓

EC2.

↓

Remote workers.

Session authority can now leave one runtime.

This is where signed capability tokens become necessary.

Likely additions:

* Ed25519 capability export
* Biscuit
* SPIFFE-like workload identity
* remote broker federation

This is deliberately postponed until the local runtime is mature.

---

## Version 4

Goal:

General-purpose execution runtime.

Adapters:

* Email
* Calendar
* Cloud
* Kubernetes
* AWS
* Browser
* Finance
* MCP ecosystem

The runtime should not distinguish between local and remote effects.

Everything is still an Effect.

---

# What Not To Build

There are many attractive ideas.

Most should be rejected.

---

## Do Not Build a New Kernel

Linux already provides:

* namespaces
* seccomp
* Landlock
* cgroups
* pidfd
* SCM_RIGHTS

These solve 90% of the problem.

Stay in user space.

---

## Do Not Build Another Workflow Engine

Temporal.

Airflow.

Dagster.

These solve orchestration.

The runtime solves authority.

Keep those concerns separate.

---

## Do Not Build Another Agent Framework

LangGraph.

CrewAI.

AutoGen.

OpenAI Responses.

These solve orchestration and prompting.

The runtime should remain usable underneath them.

---

## Do Not Build Long-Term Memory

Memory systems are entire products.

The runtime should own:

Session memory.

Artifacts.

Provenance.

Audit.

Nothing more.

Persistent knowledge belongs elsewhere.

---

## Do Not Build a Giant Effect Taxonomy

This is one of the highest risks.

It is tempting to create hundreds of Effects.

Resist this.

Grow the ontology only when repeated Sessions require it.

Design from observation.

Not speculation.

---

# Open Research Questions

Several questions remain unanswered.

These are research problems rather than implementation tasks.

---

## How Should Sessions Decompose?

Suppose the planner creates:

```text id="2cw3j2"
Fix bug

↓

Worker A

Worker B

Worker C
```

Do all workers share one Session?

Or should each receive:

```text id="d4ltgm"
Child Session
```

with its own:

* budget
* approval
* memory
* provenance

This resembles process trees.

The correct answer is not yet obvious.

---

## How Should Information Flow Be Modeled?

Current design uses taint labels.

Eventually the runtime may need explicit information-flow control.

Questions:

Can trusted summaries inherit authority?

Should taint be immutable?

How do multiple taints compose?

Can one worker sanitize another worker's output?

This area deserves careful research.

---

## Can Approval Become Predictive?

Eventually:

The runtime should understand:

> "Ben always approves PR creation after tests pass."

Should the runtime proactively ask:

> "Would you like to create a standing policy?"

Or should approval remain entirely explicit?

The balance between automation and control remains open.

---

## Should Effects Become Declarative?

Current model:

Planner requests:

```text id="jdc2dc"
CreatePullRequest
```

Alternative:

Planner describes:

```text id="8xjlwm"
Desired outcome
```

Broker chooses implementation.

This resembles SQL.

The runtime could evolve in that direction.

---

## Should Sessions Be Replayable?

Ideally:

A completed Session could be replayed.

Questions:

Should replay preserve timestamps?

Approvals?

Randomness?

LLM outputs?

How much determinism is practical?

---

## Should Planner Be Trusted?

Current architecture assumes:

Planner is privileged.

Alternative:

Planner itself becomes decomposed.

Meta-planner.

↓

Planner.

↓

Workers.

Is this worthwhile?

Unclear.

---

# Design Principles

Every pull request should be evaluated against these principles.

---

## Principle 1

Authority should be explicit.

Never ambient.

---

## Principle 2

Authority should be temporary.

Prefer borrowing.

Not ownership.

---

## Principle 3

Authority belongs to Sessions.

Not workers.

---

## Principle 4

Effects matter more than resources.

---

## Principle 5

Provenance matters more than logging.

Logs answer:

"What happened?"

Provenance answers:

"Why?"

---

## Principle 6

The runtime should prefer kernel primitives over user-space simulation.

Prefer:

fd

pidfd

SCM_RIGHTS

Namespaces

Over:

Custom protocols.

Repeated verification.

Long-lived proxies.

---

## Principle 7

The runtime should optimize for explainability.

Every decision should be reconstructable.

---

## Principle 8

Approval is product.

Not security plumbing.

---

## Principle 9

Workers are disposable.

Sessions persist.

Intents outlive Sessions.

---

## Principle 10

Every irreversible effect should have a human-readable justification.

If the runtime cannot explain why it is performing an action, it should not perform it.

---

# The Architectural Shift

The original project began as a capability broker.

It then became a capability runtime.

It has now become something more precise.

The architecture can be summarized as:

```text id="wfbtba"
Intent

↓

Session

↓

Planner

↓

Workers

↓

Broker

↓

Sandbox

↓

Operating System
```

Each layer exists because the layer below cannot solve its problem.

The operating system enforces confinement.

The broker authorizes effects.

Workers perform computation.

The planner decides strategy.

The Session owns authority and state.

The Intent explains purpose.

That hierarchy is the architecture.

Everything else is an implementation detail.

---

# Final Thesis

This project is not an operating system in the traditional sense.

It does not replace Linux.

It replaces the execution model that today's agent systems inherit from human-operated software.

Humans execute programs.

Agents execute intents.

An **Intent Runtime** provides the missing execution substrate between autonomous reasoning and a conventional operating system.

Its purpose is to ensure that autonomous work is:

* intentionally scoped,
* explicitly authorized,
* structurally confined,
* fully explainable,
* and safely composable.

That is the architectural vision this project exists to realize.
