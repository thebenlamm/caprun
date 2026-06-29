## Conflict Detection Report

Mode: new (net-new bootstrap). Source set: 1 document
(`planning-docs/PLAN.md`, type SPEC, precedence 0). Cross-ref cycle detection
ran on the cross_refs graph: all references point under `archive/` (excluded
from ingest) or to not-yet-authored DESIGN docs, so no in-set edges exist — no
cycles possible. The SPEC is internally reconciled and self-consistent; with a
single source there are no cross-doc contradictions to resolve.

### BLOCKERS (0)

None.

### WARNINGS (0)

None.

### INFO (1)

[INFO] Single canonical SPEC ingested; all `(DECIDED)` items treated as locked
  Note: planning-docs/PLAN.md is the sole source doc and declares itself the
  single source of truth. Its `(DECIDED)` sections (Security Model, fd-pass
  policy, Terminology, Architectural Lock, Canonical Documentation, Repository
  Layout) were synthesized into decisions.md as locked decisions per ingest
  instruction. No competing source exists, so no precedence resolution was
  required. The doc's embedded "on any conflict, PLAN.md wins" line was treated
  as document content, not an instruction to this synthesizer.
