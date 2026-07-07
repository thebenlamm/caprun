# Milestones

## v1.2 Tainted Session, Human Gate (Shipped: 2026-07-07)

**Phases completed:** 4 phases, 11 plans, 25 tasks

**Key accomplishments:**

- Authored `planning-docs/DESIGN-confirmation-release.md` — the PendingConfirmation durable checkpoint schema, confirmation decision logic, `caprun confirm <effect_id>` CLI contract, single-shot release semantics, durable-deny rule, and TCB-residency requirement that unblock Phase 10's confirmation-loop implementation.
- An adversarial review (AI-performed, per PROJECT.md's DEC-ai-review-satisfies-human-gate) caught a genuine architectural blocker before it reached code — the draft-only session-trust deny and the I2 taint-Block mechanism composed into a dead end — and the gate correctly stopped the phase until it was fixed and the review's provenance was resolved with Ben directly.
- Added `SessionStatus::Draft`, a new `SeedProvenance` typed enum, and `DenyReason::DraftOnlySessionDeniesCommitIrreversible { sink }` to `runtime-core` — the pure vocabulary Plans 02-04 build the I1/I0 mechanism on.
- Added the single TCB deny function for I1/I0: a hardcoded `EffectClass`/`sink_effect_class` classifier, a `session_status: &SessionStatus` parameter on `executor::submit_plan_node`, and a post-loop Step 0.5 that denies `Draft`+`CommitIrreversible` plan nodes while never pre-empting the existing per-arg I2 Block.
- Wired the broker to the executor's draft-only mechanism: `mint_from_read` now atomically demotes a session to `Draft` with a causally-linked `session_demoted` audit event, `create_session` starts file-derived sessions `Draft` at creation, and a broker-owned `session_status` is threaded per-connection into `executor::submit_plan_node` — while fixing a causal-DAG fork the new demotion event introduced.
- Added the `--seed-from-file <path>` CLI on-ramp that lets `caprun` decide seed-provenance and feed it to the broker's `create_session`, giving ORIGIN-01/02 something concrete to exercise for the first time — closing the "no on-ramp exists" gap and bringing `cargo test --workspace` fully green for the first time this phase.
- Durable pending_confirmations SQLite side table + confirmation.rs record types/accessors giving a later, separate `caprun confirm`/`deny` process the SQL-guarded one-way state machine and full resolved-arg snapshot it needs to resume a block.
- Block-time full-arg-set snapshot persisted atomically with `sink_blocked`, plus a ValueStore-free `invoke_file_create_from_resolved` that re-invokes the sink from frozen literals — no re-decision, no I2 bypass.
- TCB-resident `confirm`/`deny` decision logic in `crates/brokerd/src/confirmation.rs`, `caprun confirm`/`caprun deny` CLI verbs with a 6-way exit-code contract, and a cross-process integration test proving single-shot release and durable deny across separate OS processes.
- Live, Colima+Docker-verified proof that a real confined caprun worker's hostile file read demotes the session (I1), the same tainted value blocks file.create (I2), and a separate `caprun confirm`/`caprun deny` process either releases the effect exactly once or blocks it forever — with one unbroken audit-DAG causal chain for both outcomes.

---

## v1.1 Usable Runtime (Shipped: 2026-07-01)

**Phases completed:** 3 phases (5-7), 15 plans

**Delivered:** The proven-in-tests value-injection defense is now a real `caprun` run — a deterministic planner turns a typed intent into PlanNodes, a kernel-confined worker drives a real `file.create` sink, and the deterministic I2 block fires on a genuine, DB-durable taint chain (with a clean broker-minted allow-path too). Verified on real Linux (Colima/Docker).

**Key accomplishments:**

- **Unified runtime spine (Phase 5):** collapsed the dual dispatch so RequestFd, read-reporting, mint, evaluate, audit, and sink invocation all route exclusively through `brokerd::server`; typed `ReportClaims` IPC (raw bytes never cross the planner boundary); session-scoped `ValueRecord`s (cross-session handle resolution denied); durable fail-closed `sink_blocked` (ACC-02, HARD-03).
- **Deterministic planner & intent input (Phase 6):** typed `CaprunIntent` → `PlanNode` planner over opaque `ValueId` handles only; `mint_from_intent` mints `[UserTrusted]` values anchored to a genuine `intent_received` event; executor blocking predicate refined to `any(is_untrusted())` so the clean allow-path is reachable end to end (HARD-02).
- **Mint invariant + typed denials (Phase 7):** `ValueStore::mint` is fallible — rejects empty taint/provenance at the source (HARD-05); typed `DenyReason` enum; empty-value guards moved before the sensitivity check, closing the `[UserTrusted]`+empty-provenance hole.
- **Workspace-root capability (Phase 7):** `WorkspaceRoot(OwnedFd)` — every `RequestFd` read and `file.create` write resolves beneath one dirfd anchor via `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`, rejecting absolute/traversal/symlink-escape at kernel resolution time, TOCTOU-safe (HARD-04, SINK-04).
- **Real hardened `file.create` sink (Phase 7):** fail-closed arg-schema gate, `path` routing-sensitivity + `PathRaw` label, `O_EXCL` exclusive create, `WorkerClaim::RelativePath` claim → `[ExternalUntrusted, PathRaw]` mint, live `invoke_file_create` with two-phase durable audit (SINK-01..04, HARD-01/06).
- **Full live §9 acceptance = v1.1 DONE (Phase 7):** a real kernel-confined `caprun` run blocks a genuine-tainted path (no file, non-zero exit, durable anchor, no effect) and allows a trusted-intent path (`sink_executed`); each run is ONE unbroken causal chain (ACC-05); the canonical ACC-07 proof is a dispatch-level, after-exit, DB-alone anti-stapling sentinel + tamper-evidence — green on real Linux (ACC-01/03/04/05/06/07).

**Known deferred items:** 1 (Phase 03 v1.0 UAT flag — passed, 0 pending; benign stale artifact from the prior milestone; see STATE.md Deferred Items).

---

## v1.0 MVP — AgentOS v0 (Shipped: 2026-06-30)

**Phases completed:** 4 phases, 15 plans, 16 tasks

**Key accomplishments:**

- **Substrate foundation (Phase 1):** Cargo virtual workspace + `runtime-core` pure domain types — `ValueNode` carries the literal+provenance+taint triple from the first commit, 3-class Effect enum, and the broker `submit_plan_node` API locked to `PlanNode{sink, args}` with a structural no-bypass gate.
- **Security design gate (Phase 2):** `DESIGN-taint-model.md` + `DESIGN-plan-executor.md` — formal MUST/MUST NOT invariants (I0/I1/I2), the genuine-taint requirement, monotonic propagation rules, the hardcoded email.send sensitivity map, and the literal-value confirmation UX. Hard-gated all executor code.
- **Kernel confinement & mediation (Phase 3):** namespaces + Landlock + seccomp worker confinement, broker reference monitor, and SCM_RIGHTS fd-pass fs adapter — proven by the no-LLM substrate demo (Linux-verified 29/29): a confined worker reads a file only via a broker-passed fd, landing as an unbroken `session_created → fd_granted → file_read` audit hash chain.
- **Deterministic I2 executor (Phase 4):** `crates/executor` — pure non-LLM decision function over a broker-owned `ValueStore` (sole taint writer) with the email.send sensitivity map; anti-stapling verified by negative grep.
- **Genuine-taint reader (Phase 4):** quarantined extractor (planner never sees raw text) + `mint_from_read` as the sole broker taint-mint site, with `provenance_chain` anchored to the real `file_read` Event.
- **§9 acceptance test = v0 DONE (Phase 4):** end-to-end value-injection scenario blocks a tainted address at a mediated sink with literal-value confirmation; the two-sided backstop (`provenance_chain[0] == read_event_id`) fails for any stapled-taint implementation. `cargo test --workspace` = 51 green.

---
