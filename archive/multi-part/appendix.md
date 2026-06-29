# Appendix

This appendix captures the intellectual lineage of the project, defines its terminology, explains what is genuinely novel, records rejected alternatives, and identifies future research directions.

The goal is to preserve the architectural reasoning behind the runtime so future contributors understand *why* particular decisions were made.

---

# Intellectual Lineage

This project is not attempting to invent security, capabilities, or operating systems.

Almost every major building block already exists.

The contribution is the integration of these ideas into a runtime designed specifically for autonomous agents.

## Object Capability Systems

Primary influences:

* KeyKOS
* E
* CapROS
* Coyotos
* seL4 (conceptually)

Key idea:

Authority should be explicit, transferable, attenuable, and never ambient.

Borrowed:

* least authority
* delegation
* attenuation
* object-capability thinking

Not borrowed:

* microkernel architecture
* capability-based hardware assumptions

---

## Unix Capability Systems

Influences:

* Capsicum
* Landlock

Borrowed:

* process confinement
* explicit authority
* capability mode

---

## Linux Security

Primary primitives:

* user namespaces
* mount namespaces
* network namespaces
* seccomp
* Landlock
* cgroups
* pidfd
* SCM_RIGHTS

These form the enforcement substrate.

The runtime intentionally builds on these instead of replacing them.

---

## Distributed Authorization

Influences:

* Biscuit
* Macaroons
* OAuth (negative example)
* SPIFFE/SPIRE

Borrowed:

* attenuation
* delegation
* workload identity

Not borrowed:

* authorization as the primary architectural abstraction

Tokens exist primarily to move authority between runtimes.

They are not the runtime's internal execution model.

---

## Agent Systems

Influences:

* AutoGen
* CrewAI
* LangGraph
* OpenAI Responses
* MCP

Borrowed:

* multi-agent orchestration
* tool abstraction
* planner/worker decomposition

Not borrowed:

* ambient tool authority
* prompt-centric security
* conversation as the primary execution model

---

# What Is Actually Novel?

Many ideas in this project already exist individually.

The novelty is their composition.

The strongest claims of novelty are:

## 1. Intent-Centered Execution

Modern operating systems execute programs.

This runtime executes intents.

Intent becomes the root object.

Everything derives from it.

---

## 2. Session as the Unit of Authority

Authority belongs to a Session.

Not:

* process
* worker
* thread
* capability token

This changes:

* approval
* audit
* rollback
* delegation
* provenance

---

## 3. Effects as the Planning Surface

Traditional systems expose:

* files
* sockets
* processes

This runtime exposes:

* RunTests
* ApplyPatch
* CreatePullRequest
* SendEmail

The planner reasons about effects.

Adapters translate effects into operating-system operations.

---

## 4. Approval as a Product Surface

Most systems treat approval as authorization.

This runtime treats approval as a user experience.

Approval becomes adaptive.

Policy evolves from repeated interaction.

---

## 5. Provenance as a First-Class Runtime Object

Logs answer:

"What happened?"

Provenance answers:

"Why?"

This distinction is fundamental.

---

## 6. Privilege-Separated Cognition

This is arguably the most important security innovation.

The runtime explicitly separates:

* reading hostile information
* deciding
* performing irreversible effects

Prompt injection becomes a runtime architecture problem rather than merely an LLM prompting problem.

---

# Terminology

These terms have precise meanings.

They should not be used interchangeably.

---

## Intent

Why work exists.

---

## Session

The live execution state for one Intent.

Owns:

* authority
* approval
* budget
* provenance
* workers
* memory

---

## Worker

A bounded execution unit.

May be:

* LLM
* deterministic process
* parser
* compiler
* script

Workers are disposable.

---

## Planner

Chooses which effects should happen next.

Does not perform effects.

---

## Broker

Reference monitor.

Authorizes effects.

Creates kernel capabilities.

Coordinates adapters.

---

## Adapter

Maps semantic effects onto operating-system or service operations.

---

## Effect

A semantic action requested by the planner.

Examples:

* RunTests
* CreateArtifact
* CreatePullRequest

Not:

* open()
* connect()
* write()

---

## Artifact

Structured output produced during a Session.

Carries provenance.

---

## Provenance

The causal explanation for why something exists.

---

## Approval

A human decision that modifies Session authority.

---

# Rejected Architectures

These were considered and intentionally rejected.

---

## Broker as the Entire Runtime

Original idea.

Problem:

The broker became responsible for everything.

Result:

No clear separation of concerns.

Rejected.

---

## Pure Token-Based Authority

Everything represented by signed capabilities.

Problem:

Expensive.

Awkward.

Duplicates broker state.

Ignores native kernel capabilities.

Rejected.

---

## Proxy Every Operation

Every filesystem read.

Every network packet.

Every write.

Problem:

High overhead.

Poor scalability.

Linux already provides kernel capabilities.

Rejected.

---

## Fork Linux

Idea:

Create a true agent operating system.

Problem:

Enormous engineering cost.

Very little practical benefit.

Rejected.

---

## Capability-Only Security

Idea:

Prompt injection is solved by capability scoping.

Problem:

False.

Prompt injection abuses legitimate authority.

Requires privilege separation.

Rejected.

---

## Giant Effect Ontology

Idea:

Define hundreds of Effects before implementation.

Problem:

Premature abstraction.

Rejected.

---

# Design Heuristics

When making architectural decisions, prefer:

Explicit over implicit.

Small authority over broad authority.

Kernel enforcement over user-space convention.

Effects over resources.

Sessions over processes.

Explanation over cleverness.

Composition over inheritance.

Determinism over hidden magic.

Simple adapters over universal adapters.

---

# Future Research

Several topics deserve dedicated investigation.

* Formal Session semantics.
* Information-flow control.
* Cross-runtime Session migration.
* Deterministic replay.
* Adaptive approval policy.
* Multi-planner architectures.
* Verification of planner correctness.
* Capability-aware model fine-tuning.
* Intent decomposition algorithms.
* Economic models for autonomous execution.

These are intentionally outside the scope of Version 0.

---

# One-Sentence Summary

This project proposes an **Intent Runtime**: a user-space execution runtime that treats **Intent** as the root abstraction, **Session** as the unit of authority, **Effects** as the planning surface, **the Broker** as the reference monitor, and **the operating system** as the enforcement substrate, enabling autonomous agents to execute work with explicit authority, structural confinement, rich provenance, and minimal human interruption.

---

# A Note to Future Contributors

It will be tempting to add features.

Resist that temptation.

The value of this project is not that it combines every interesting idea from operating systems, security, distributed systems, and AI.

Its value is that it has a *small number of coherent ideas* that reinforce one another:

* Intent over commands.
* Sessions over processes.
* Effects over resources.
* Kernel-enforced least authority over ambient authority.
* Provenance over logging.
* Approval as an adaptive interface rather than a permission dialog.

If a proposed feature strengthens those ideas, it probably belongs.

If it weakens them, even if it is useful in isolation, it probably does not.

Conceptual integrity is the project's greatest asset.
