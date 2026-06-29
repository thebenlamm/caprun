# Archive — Source Planning Docs (superseded by PLAN.md)

These are the **source** documents from the design/convergence phase. They are
**background reference only**. The single source of truth is
[`planning-docs/PLAN.md`](../planning-docs/PLAN.md).

**On any conflict, PLAN.md wins.** Do not treat these as active plans — they
contain pre-convergence framing and two competing doc lineages (CapRunner vs.
multi-part Intent Runtime) that PLAN.md has already reconciled.

Retained as canonical *deep references* for their domains:

| Doc | Use it for |
|-----|-----------|
| `AGENT-RUNTIME-HANDOVER.md` | Security detail: I1/I2, trust tiers, §9 acceptance test, executor spec |
| `multi-part/` | Architecture narrative, schemas, threat model, approval/provenance design |
| `agent-execution-runtime-handover.md` | Red-team findings, open risks, fd-revocation reality |

> **Note for GSD ingest:** ingest `planning-docs/PLAN.md` as canonical. Exclude
> or down-weight `archive/` so resolved conflicts aren't re-surfaced.
