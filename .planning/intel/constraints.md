# Constraints

Technical contracts and non-functional constraints extracted from the canonical
SPEC `planning-docs/PLAN.md`. These bind implementation regardless of milestone.

---

## CON-broker-api-shape

- **source:** planning-docs/PLAN.md (§ Architectural Lock)
- **type:** api-contract
- **content:**
  ```rust
  submit_plan_node(session_id, PlanNode {
      sink: SinkId,           // e.g. email.send
      args: Vec<ValueNode>,   // each carries literal + provenance + taint
  }) -> ExecutorDecision
  ```
  The broker effect path takes plan nodes from day one. Raw
  `EffectRequest { effect, args: Map }` straight to sinks is forbidden — it bakes
  in a path where tainted values reach sensitive arguments with nowhere for the
  executor to stand. The API shape is not optional.

## CON-effect-classes

- **source:** planning-docs/PLAN.md (§ Terminology)
- **type:** schema
- **content:** Effect classes at the planner surface (v0):
  ```text
  Observe          — read, list, summarize
  MutateReversible — write artifact, apply patch
  CommitIrreversible — send, git push, deploy, purchase
  ```
  Grow ontology from audit DAG observations, not upfront speculation.

## CON-repo-layout

- **source:** planning-docs/PLAN.md (§ Repository Layout)
- **type:** schema
- **content:** Repo root = single Cargo workspace. Crate tree:
  ```text
  crates/runtime-core   # Intent, Session, Effect, Artifact, Event — no I/O
  crates/brokerd        # session lifecycle, policy, audit DAG, adapters
  crates/executor       # deterministic I2 interpreter (after DESIGN doc)
  crates/sandbox        # bubblewrap, seccomp, Landlock, cgroups
  crates/adapters/fs
  crates/captoken       # v0 minimal; broker DB is authority on single host
  cli/caprun
  ```

## CON-stack-tcb

- **source:** planning-docs/PLAN.md (§ Repository Layout — Stack)
- **type:** nfr
- **content:** Rust (tokio, serde, sqlx/SQLite, nix/rustix, landlock,
  seccompiler, ed25519-dalek). Python is permitted for non-TCB experiments only —
  never in the trusted computing base. I2 enforcement is a deterministic,
  non-LLM plan executor hardcoded in the Rust TCB.

## CON-platform-linux-only

- **source:** planning-docs/PLAN.md (§ What We Are Building)
- **type:** nfr
- **content:** M0/M1 target Linux (Ubuntu) only. All v0 security claims are
  Linux-only. Mac/WSL2 deferred to post-v0 best-effort.

## CON-i2-non-bypassable

- **source:** planning-docs/PLAN.md (§ Security Model — I2)
- **type:** protocol
- **content:** No attacker-tainted value may occupy a sensitive argument of an
  irreversible/external sink without literal-value human confirmation (or exact
  standing policy match). Policy files may gate which sinks are callable but
  cannot disable I2. In v0 the sink sensitivity map is hardcoded — no
  policy/schema system yet.

## CON-i1-taint-default

- **source:** planning-docs/PLAN.md (§ Security Model — I1)
- **type:** protocol
- **content:** No LLM context may simultaneously hold untrusted content and
  authority to cause irreversible/external effects. Default = dynamic taint
  (reading raw untrusted bytes taints the context → draft-only thereafter).
  Tier 3+ = hard planner/worker split: privileged planner sees typed extracts
  only; quarantined worker holds no dangerous caps.

## CON-i0-session-creation

- **source:** planning-docs/PLAN.md (§ Security Model — I0)
- **type:** protocol
- **content:** A Session whose intent text or seed derives from
  external/untrusted content starts draft-only and cannot auto-authorize Tier 3+
  effects. Human gate required on context creation from tainted data.

## CON-fd-pass-revocation

- **source:** planning-docs/PLAN.md (§ fd-pass policy)
- **type:** protocol
- **content:** fd-pass (SCM_RIGHTS) only for read-only workspace I/O and test
  output (low-risk, short-lived, disposable workers). External / irreversible /
  high blast-radius effects are mediated only. Revocation = kill the worker via
  pidfd. Leases are not revocation. (Residual: an fd cannot be selectively
  revoked after SCM_RIGHTS handoff — mitigated by disposable workers.)

## CON-s9-taint-genuineness

- **source:** planning-docs/PLAN.md (§ v0 Acceptance Test §9)
- **type:** protocol
- **content:** v0 DONE requires §9 passing on a kernel-confined worker whose only
  egress is broker-mediated plan nodes, with genuine taint propagation verified
  in the audit DAG. Non-negotiable: if taint is stapled on at the sink instead of
  propagated through the DAG, the demo proves nothing and fails.
