# Requirements: AgentOS (caprun) — v1.7

**Defined:** 2026-07-17
**Core Value:** A kernel-confined worker can only cause external effects through
broker-mediated plan nodes, and a genuinely-propagated taint chain (raw source →
ValueNode → sensitive sink arg) deterministically blocks value-injection at the
sink (I2). v1.7 extends the set of real sinks (`process.exec`, filesystem
read/write breadth) without weakening that guarantee.

**Milestone goal:** Effect Breadth I — give caprun the two effect primitives a
coding agent minimally needs (run a command with captured+tainted output; read/
edit repo files beyond single-file create), each routed through the plan-node →
taint → executor(I2) → audit path, toward the **Safe Coding Agent** anchor.

**Standing precedent:** opens with a design-gate phase; no `crates/executor` /
`crates/brokerd` / `crates/sandbox` / `crates/runtime-core` TCB code before the
DESIGN doc clears a **fresh non-self adversarial code-trace** (v1.0 P2, v1.2 P8,
v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26). The orchestrator — not a gsd-executor —
owns that review spawn.

## v1 Requirements

Requirements for the v1.7 milestone. Each maps to exactly one roadmap phase.

### Design Gate

- [x] **DESIGN-13**: A reviewed DESIGN doc (`planning-docs/DESIGN-effect-breadth-exec.md`)
  pins the broker-spawned confined-child-`exec` model (how a child is spawned
  from the broker — the confined worker cannot `execve` per seccomp deny-execve —
  how it is confined, and how stdout/stderr are captured and taint-minted) and
  the filesystem read/write-breadth model, and **clears a fresh non-self
  adversarial code-trace before any TCB code**.

- [x] **DESIGN-14**: The DESIGN doc pins the **fail-closed defaults** for the new
  sinks — `process.exec` command/arg schema and (dis)allow posture, exec-output
  taint label + `origin_role`, and fs read/write path & slot constraints —
  consistent with the existing I0/I1/I2 and v1.5 slot-type-binding discipline
  (nothing here may disable or bypass I2).

### Process Exec

- [x] **EXEC-01**: A `process.exec` plan-node sink runs a command **as a
  broker-spawned confined child process** (mediated like the v1.4 caprun-planner
  sidecar / adapter-fs fd-pass), never via the confined worker's own `execve`.

- [x] **EXEC-02**: The child's stdout/stderr are captured and **taint-minted as
  untrusted**, producing a ValueNode whose provenance chain is genuinely rooted
  at the `exec` Event (the sole exec-output taint-mint site — no stapling).

- [x] **EXEC-03**: **Exec-output taint is I2-enforced** — a tainted exec-output
  value routed to a sensitive sink arg is deterministically **Blocked** by the
  executor, verifiable as an unbroken audit-DAG edge (exec Event → ValueNode →
  sink arg → block) with `verify_chain` true.

- [x] **EXEC-04**: The exec child is itself **kernel-confined** (Landlock +
  seccomp + default-deny net + resource/time limits), the sink is **fail-closed
  on arg-schema**, and a durable audit Event records the spawn and exit.

### Filesystem Breadth

- [x] **FS-01**: The worker can **read multiple workspace files** beyond the
  single current read path, each resolved beneath `WorkspaceRoot` via
  `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`, taint-minted as untrusted like
  the existing read path.

- [x] **FS-02**: A filesystem **write/edit sink modifies an existing file**
  within `WorkspaceRoot` (beyond `file.create`'s `O_EXCL` new-file-only),
  fail-closed on path schema, kernel-confined, and durably audited.

- [x] **FS-03**: The fs write/edit sink args are governed by the executor under
  the **same I2 / slot-type-binding discipline** — a tainted path or contents in a
  sensitive slot Blocks; there is no I2 bypass and no new raw `EffectRequest` path.

### Live Proof

- [ ] **LIVE-01**: On **real Linux**, a composed acceptance run proves end-to-end:
  an `exec` whose tainted output is routed to a sensitive sink arg is **Blocked**
  (I2, genuine non-stapled taint chain, `verify_chain` true); a clean exec/fs path
  is **Allowed**; a fs write/edit within `WorkspaceRoot` succeeds and is audited —
  via `scripts/mailpit-verify.sh` or an exec-scoped equivalent, true-exit-before-pipe.

- [ ] **LIVE-02**: **Full-workspace regression** re-runs green on real Linux with
  **no regression to v1.0–v1.6**, asserted on counts + named tests (not exit 0
  through a pipe), plus a dedicated negative test per new sink.

## Future Requirements (v1.8+)

Deferred to future milestones per the productization sketch
(`planning-docs/CANDIDATE-v1.7plus-productization-sketch.md`). Tracked, not in this
roadmap.

### Effect Breadth II (v1.8)

- **GIT-01**: `git` / `github.pr` sink — commit + open a PR (the irreversible
  external effect that triggers I2 / human confirmation).

- **HTTP-01**: `http.request` sink — outbound HTTP to an allowlisted host
  (authorized hole in default-deny net, broker-mediated).

### Planner & Product (v1.9 / v1.10+)

- **PLAN-LOOP-01**: Real multi-step LLM planner loop (tool-use, retries, error
  handling) on the v1.4 sidecar seam — requires an eval set FIRST (domain ground
  truth, gradable rubrics) and a plain-prompt baseline before scaffolding.

- **POL-01**: Minimal declarative per-session policy (sink allowlist + arg
  constraints) — hardcoded struct first, file format later; Cedar deferred.

- **SDK-01**: Thin SDK/CLI + audit-DAG viewer (the trust surface).
- **PKG-01**: Single-binary packaging + documented "run on a Linux box/container"
  path.

## Out of Scope

Explicitly excluded from v1.7. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| `git` / `github.pr`, `http.request` | v1.8 (Effect Breadth II) — the irreversible external effect comes *after* the exec+fs primitives that produce the change |
| Real LLM planner loop | v1.9 — needs an eval set + baseline first; a deterministic/stub planner remains sufficient to prove the new sinks |
| Declarative policy file / Cedar | v1.10+ — a hardcoded arg-schema is enough for v1.7; I2 stays in the Rust TCB regardless |
| Thin SDK / audit-DAG viewer / packaging | v1.10+ — product surface, not a security primitive |
| Cross-host delegation / Biscuit, gVisor/Firecracker, web UI, marketplace, long-term memory | Post-v1.x per PLAN.md; bubblewrap+seccomp+Landlock remains the boundary |
| Mac / WSL2 support | All security claims remain Linux-only (kernel ≥5.13) |

## Traceability

Which phases cover which requirements. 100% coverage — every v1.7 requirement
maps to exactly one phase (`/gsd-roadmapper`, 2026-07-17).

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-13 | Phase 31 | Complete |
| DESIGN-14 | Phase 31 | Complete |
| EXEC-01 | Phase 32 | Complete |
| EXEC-02 | Phase 32 | Complete |
| EXEC-03 | Phase 32 | Complete |
| EXEC-04 | Phase 32 | Complete |
| FS-01 | Phase 33 | Complete |
| FS-02 | Phase 33 | Complete |
| FS-03 | Phase 33 | Complete |
| LIVE-01 | Phase 34 | Pending |
| LIVE-02 | Phase 34 | Pending |

**Coverage:**

- v1.7 requirements: 11 total
- Mapped to phases: 11/11 ✓ (Phase 31 design gate: 2; Phase 32 exec: 4; Phase 33 fs: 3; Phase 34 live proof: 2)
- Unmapped: 0 ✓ — no orphans, no duplicates

---
*Requirements defined: 2026-07-17*
*Last updated: 2026-07-17 — roadmap created (`/gsd-roadmapper`), traceability filled (Phases 31-34)*
