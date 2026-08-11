# Phase 52: Minimal Linux Packaging - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-11
**Phase:** 52-minimal-linux-packaging
**Areas discussed:** Installation experience, Destination and upgrades, Binary scope and verification, Configuration guidance

---

## Prior Todo Matches

| Option | Description | Selected |
|--------|-------------|----------|
| Fold none | Keep Phase 52 focused on PKG-01 | ✓ |
| Review and pick | Inspect the three loosely matched notes individually | |
| Fold all | Expand the phase to include all matched notes | |

**User's choice:** Fold none.
**Notes:** All three matcher results were generic keyword matches and remain deferred.

---

## Installation Experience

| Option | Description | Selected |
|--------|-------------|----------|
| Script plus manual path | Thin installer backed by equivalent documented commands | ✓ |
| Manual commands only | Documentation without a convenience script | |
| Script only | Installer is the sole supported walkthrough | |

**User's choice:** Agent discretion (`yolo`).
**Notes:** Selected the transparent script-plus-documentation approach because PKG-01 explicitly permits a thin script and design partners should still be able to audit or reproduce its actions manually.

---

## Destination and Upgrades

| Option | Description | Selected |
|--------|-------------|----------|
| User-local default | `${HOME}/.local/bin`, explicit override, no implicit sudo | ✓ |
| System-wide default | `/usr/local/bin`, normally requiring elevation | |
| Repo-local only | Leave release binaries under `target/release` | |

**User's choice:** Agent discretion (`yolo`).
**Notes:** Re-running the installer is the upgrade path; all three binaries must remain a coherent sibling set.

---

## Binary Scope and Verification

| Option | Description | Selected |
|--------|-------------|----------|
| Three required binaries | Install and verify caprun, worker, and exec launcher | ✓ |
| Include planner sidecar | Also install optional LLM planner functionality | |
| Main binary only | Install caprun without required siblings | |

**User's choice:** Agent discretion (`yolo`).
**Notes:** The three-binary selection follows PKG-01 exactly. Verification is non-destructive layout checking and must not overclaim the full Linux proof.

---

## Configuration Guidance

| Option | Description | Selected |
|--------|-------------|----------|
| Tiered operator checklist | Separate normal inputs, sink-specific secrets, and internal/test variables | ✓ |
| Exhaustive env dump | Present every discovered variable without usage classification | |
| Minimal token note | Mention only GitHub credentials and omit other applicable setup | |

**User's choice:** Agent discretion (`yolo`).
**Notes:** Examples use placeholders, recommend least-scope credentials, prefer `--policy`, and never tell operators to set worker protocol or proof-only switches.

## the agent's Discretion

- The user explicitly delegated all four selected gray areas with `yolo`.
- Exact script flags, document structure, and the smallest reliable smoke check remain planner discretion within the locked decisions.

## Deferred Ideas

- Broader package formats, hosted artifacts, auto-updates, and macOS support remain future PACK-02 work.
