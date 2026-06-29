# Intent Runtime

## An Intent-Scoped Execution Runtime for Autonomous Agents

**Status:** Architecture Proposal (v0.1)

---

# Executive Summary

This document describes the architecture for a new class of software: an **Intent Runtime**.

This is **not** a new operating system kernel.

It is **not** another agent framework.

It is **not** merely a capability broker.

Instead, it is a runtime that sits on top of an existing operating system and provides the execution model that modern autonomous agents actually need.

Its central idea is simple:

> Humans execute programs.
>
> Agents execute intents.

The runtime's job is to safely execute an intent from beginning to completion while minimizing ambient authority, maximizing auditability, preserving reversibility whenever possible, and keeping humans out of the loop except when irreversible actions require approval.

The system borrows heavily from object-capability security, Linux sandboxing, capability operating systems, workload identity, and distributed authorization, but does not attempt to recreate them. Instead it integrates them into a runtime designed specifically for LLM-based autonomous systems.

---

# Why This Exists

Today's agent systems are fundamentally backwards.

An LLM is given:

* filesystem access
* shell access
* API keys
* browser automation
* cloud credentials

…and then surrounded with prompts telling it to "be careful."

This is ambient authority.

It is exactly the model capability systems spent decades trying to eliminate.

Prompt injection demonstrates why this fails.

A malicious document does not need to exploit the operating system.

It only needs to convince the LLM to misuse authority it already possesses.

Current systems respond with:

* better prompts
* output classifiers
* tool descriptions
* safety filters

These improve reliability.

They do not fundamentally change the authority model.

---

# Original Thesis

The original proposal was:

> Build an "agent-first operating system" around a Capability Broker.

Every operation would require:

* explicit capability
* scoped authority
* delegation
* audit
* revocation

Initially, the Capability Broker was viewed as the central primitive.

During design review this thesis changed substantially.

---

# The Critical Correction

A capability broker is **not** a security boundary.

It is a policy engine.

If an agent still possesses:

* filesystem access
* networking
* shell
* secrets

then a compromised agent can simply ignore the broker.

The broker only provides security if the agent has no way around it.

Therefore:

**The broker cannot be the center of the architecture.**

The runtime must instead be built around three distinct layers:

1. Intent
2. Execution Runtime
3. Enforced Mediation

The broker becomes one subsystem inside that runtime.

---

# Revised Thesis

An agent-first operating system is:

> An intent-scoped execution runtime running on top of an existing operating system where agents possess no ambient authority, every externally visible effect is authorized against an execution context, and confinement is enforced by the operating system rather than by agent cooperation.

That statement replaces the original thesis.

It is both more precise and significantly harder to attack.
