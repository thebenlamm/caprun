# Phase 26: Security Hardening Design Gate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-12
**Phase:** 26-security-hardening-design-gate
**Areas discussed:** verify_chain threat ceiling (HARDEN-02), file.create contents fix depth (HARDEN-05)

---

## Framing (pre-question)

The five residuals were split by the orchestrator (validated by a fresh Fable-5
code-tracing reviewer) into: three with a clearly-correct fail-closed answer (locked as
recommendations — HARDEN-01/03/04) and two genuine scope/cost forks put to the owner.
The Fable reviewer additionally surfaced three cross-cutting rulings (label continuity,
shared-store restart authority, TOCTOU atomic ordering) that no single residual names but
the design doc must resolve — those were locked as recommendations, not asked.

## HARDEN-02 — Authenticated audit chain: threat ceiling

| Option | Description | Selected |
|--------|-------------|----------|
| In-host DB-writer | Keyed MAC (key outside worker's Landlock scope) + anchored/monotonic head vs truncation/rollback; defer host/admin external notarization as a named residual. Keeps Phase 28 bounded. | ✓ |
| Also host/admin compromise | Add external out-of-store anchor/notarization; larger Phase 28, custody infra + external dependency. | |

**User's choice:** In-host DB-writer
**Notes:** Matches the v1.6 charter ("close the TCB-local residuals"). Host/root compromise
that can read the broker key is explicitly deferred and must be recorded as a named
Accepted Residual Risk. Doc must not overclaim tamper-evidence beyond this model.

## HARDEN-05 — file.create `contents` fix depth

| Option | Description | Selected |
|--------|-------------|----------|
| Input-role + name laundering residual | Content-sensitive I2 treatment for `contents` (mirror `body`); name the write→re-read laundering round-trip as a tracked cross-cutting residual closed by label continuity. Right-sized. | ✓ (recommended; owner did not object) |
| Full output-provenance labeling now | Broker stamps every created file with the writing session's taint so a re-read can't launder it; bigger, adapter-spanning. | |

**User's choice:** Recommended depth taken (owner interrupted the modal after answering
HARDEN-02 to avoid extra question stops — per standing minimize-question-stops preference).
**Notes:** Input-role treatment is the v1.6-scoped fix; the deeper laundering loop is
closed by the X-01 label-continuity cross-cutting ruling and tracked as a residual. Full
output-provenance labeling deferred if the input-role fix proves insufficient.

---

## Claude's Discretion

- Exact idempotency-key derivation (HARDEN-03), precise `contents` expected-role membership
  (HARDEN-05), and the no-feature negative-gate shape (HARDEN-04) — pinned by
  researcher/planner against live code; directions are locked in CONTEXT.md.

## Deferred Ideas

- Effects-budget / per-session send rate-limit (defense-in-depth beyond CAS).
- External out-of-store notarization for the audit chain (host/root-compromise defense).
- Full output-file provenance labeling model for `file.create` `contents`.
- v1.7 — Breadth adapters (Git/GitHub/test/patch-PR/snapshot).
