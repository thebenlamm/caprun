# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- ✅ **v1.3 — Doc → Action Assistant** — Phases 12-17 (shipped 2026-07-09)
- ✅ **v1.4 — Trust-Boundary Integrity & the Adversarial Planner** — Phases 18-22 (shipped 2026-07-11)
- ✅ **v1.5 — Slot-Type Binding Enforcement (T2)** — Phases 23-25 (shipped 2026-07-12)
- ✅ **v1.6 — Security Hardening (close the residuals)** — Phases 26-30 (shipped 2026-07-17)
- 🚧 **v1.7 — Effect Breadth I (`process.exec` + Filesystem Breadth)** — Phases 31-34 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP — AgentOS v0 (Phases 1-4) — SHIPPED 2026-06-30</summary>

Full detail archived in [`milestones/v1.0-ROADMAP.md`](milestones/v1.0-ROADMAP.md).

- [x] **Phase 1: Substrate Foundation** (2/2 plans) — Cargo workspace, domain core, locked plan-node broker API — completed 2026-06-29
- [x] **Phase 2: Security Design Gate** (3/3 plans) — taint-model + plan-executor DESIGN docs that hard-gate all executor code — completed 2026-06-29
- [x] **Phase 3: Confinement & Mediation Substrate** (5/5 plans) — kernel confinement, broker reference monitor, fd-pass fs adapter, no-LLM substrate demo (Linux-verified 29/29) — completed 2026-06-29
- [x] **Phase 4: Value-Injection Security Demo (v0 DONE)** (5/5 plans) — §9 acceptance test passes with a genuine, audited taint chain — completed 2026-06-30

**v0 DONE gate cleared:** the §9 value-injection test blocks a tainted address at a mediated sink with literal-value confirmation; `mint_from_read` is the sole broker taint-mint site; stapled taint fails the test. `cargo test --workspace` = 51 green.

</details>

<details>
<summary>✅ v1.1 — Usable Runtime (Live §9 from the CLI) (Phases 5-7) — SHIPPED 2026-07-01</summary>

Full detail archived in [`milestones/v1.1-ROADMAP.md`](milestones/v1.1-ROADMAP.md).

**Milestone goal:** Turn the proven-in-tests value-injection defense into a real `caprun` run — a deterministic scripted planner turns an intent into PlanNodes, a confined worker drives toward a real `file.create` sink, and the deterministic I2 block fires on a genuine taint chain (with a clean, broker-minted allow-path too).

- [x] **Phase 5: Runtime Spine & Live §9 Email Block** (4/4 plans) — collapsed dual dispatch, session-scoped handle model (HARD-03), live §9 block with durable blocked-path audit (ACC-02) through the email.send stub — completed 2026-06-30
- [x] **Phase 6: Deterministic Planner & Intent Input** (5/5 plans) — typed intent → PlanNode planner, `mint_from_intent` `[UserTrusted]` values, executor predicate over `is_untrusted()` (HARD-02), clean allow-path reachable — completed 2026-07-01
- [x] **Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance** (6/6 plans) — real hardened `file.create` sink (schema gate, `O_EXCL`, dirfd + `openat2 RESOLVE_BENEATH`), mint invariant + typed `DenyReason`, durable genuine-taint anchor (ACC-07), full live §9 acceptance green on real Linux — completed 2026-07-01

**v1.1 DONE gate cleared:** a real kernel-confined `caprun` `file.create` run blocks a genuine-tainted path (no file, non-zero exit, durable `sink_blocked` anchor, no effect) and allows a trusted-intent path (`sink_executed`); each run is ONE unbroken causal chain (ACC-05); the canonical ACC-07 proof is a dispatch-level, after-exit, DB-alone anti-stapling sentinel + tamper-evidence. Verified on real Linux via Colima/Docker. All 14 Phase-7 requirement IDs Complete; verifier scored 14/14.

</details>

<details>
<summary>✅ v1.2 — Tainted Session, Human Gate (Phases 8-11) — SHIPPED 2026-07-07</summary>

Full detail archived in [`milestones/v1.2-ROADMAP.md`](milestones/v1.2-ROADMAP.md).

**Milestone goal:** A session that touches untrusted content is mechanically demoted to draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked sink arg can be released only by literal-value human confirmation — all deterministic, all in the audit DAG.

- [x] **Phase 8: Session-Trust & Confirmation Design Gate** (3/3 plans) — DESIGN doc for session-trust-state (I1 demotion + I0 creation rule) and confirmation-release semantics, reviewed before any executor code — completed 2026-07-06
- [x] **Phase 9: Session Trust State (I1 + I0)** (4/4 plans) — reading untrusted content or being seeded from external content demotes/starts a session as draft-only; draft-only sessions deny CommitIrreversible plan nodes via one executor TCB function — completed 2026-07-07
- [x] **Phase 10: Single-Shot Confirmation Loop** (3/3 plans) — `caprun confirm <effect_id>` shows the human the blocked literal + provenance and releases exactly one (sink, arg, literal-digest) triple; deny is durable — completed 2026-07-07
- [x] **Phase 11: Live Acceptance — Tainted Session, Human Gate** (1/1 plans) — live run on real Linux: hostile read → session demotion → sink block → human deny (nothing sent) / human confirm (exactly once), one unbroken audit chain — completed 2026-07-07

**v1.2 DONE gate cleared:** live on real Linux via Colima+Docker, a hostile workspace-file read demotes the session (I1), the same tainted value Blocks `file.create` (I2), and a human `caprun deny`/`caprun confirm` either durably blocks the effect or releases it exactly once — one unbroken audit-DAG causal chain (`fd_granted→file_read→session_demoted→sink_blocked→confirm_{denied,granted}`) for both outcomes (ACC-01/02/03). A pre-existing stale test assertion in `s9_live_block.rs` (dating to Phase 9, never previously run on Linux) was caught and fixed in the process. All 14 v1.2 requirement IDs Complete.

</details>

<details>
<summary>✅ v1.3 — Doc → Action Assistant (Phases 12-17) — SHIPPED 2026-07-09</summary>

Full detail archived in [`milestones/v1.3-ROADMAP.md`](milestones/v1.3-ROADMAP.md).

**Milestone goal:** caprun ingests an untrusted document containing an embedded injection, deterministically extracts a "send to X" action (recipient + body derived from the doc's content, no LLM planner), and attempts a real email send. The read demotes the session (I1, existing); the tainted recipient AND body both block at the sink (I2 + new CONTENT-01); `caprun confirm`/`deny` shows verbatim recipient+body+provenance; confirm sends exactly once via a real broker-mediated SMTP adapter, deny sends nothing — one unbroken audit DAG for both outcomes, plus a clean-send negative control in the same run, proven live on real Linux via Colima+Docker.

- [x] **Phase 12: Content, Adapter & Confirm-Binding Design Gate** (3/3 plans) - A reviewed, adversarially-reviewed DESIGN doc covering content-sensitivity semantics, real-adapter mediation, and confirm-binding exists before any executor/TCB code for this milestone is written (completed 2026-07-07)
- [x] **Phase 13: Real Broker-Mediated SMTP Adapter** (4/4 plans) - caprun can send a real email through a broker-mediated adapter — worker never touches the network, secrets never leave the broker, and the send is idempotent/failure-safe (completed 2026-07-08)
- [x] **Phase 14: Content-Sensitive Sink-Arg Blocking** (2/2 plans) - The executor blocks a tainted email body the same way it already blocks a tainted recipient (completed 2026-07-08)
- [x] **Phase 15: Deterministic Doc→Action Extraction** (4/4 plans) - A confined, deterministic extractor turns a hostile document's bytes into a plan node, with a proven unbroken audit-DAG edge from read to block, including through a transformation (completed 2026-07-08, independently verified live on Linux — 8/8 must-haves)
- [x] **Phase 16: Confirm UX, Literal Binding & Negative Controls** (4/4 plans) - A human sees the verbatim, provenance-narrated recipient and body before deciding; confirm is bound to the exact resolved literals; the gate is proven taint-driven, not a blanket email block (completed 2026-07-09, independently verified — 10/10 checks, real exfiltration hole confirmed closed in source)
- [x] **Phase 17: Live Acceptance & Framing Honesty** (4/4 plans) - The full doc→action chain runs live on real Linux as one unbroken audit DAG, composing the hostile block and the clean control in the same run, with honest public framing of what was and wasn't proven (completed 2026-07-09, independently re-verified by both caprun-sonnet-77 and caprun-opus-77 — 250/250 tests passed on real Linux, HARD GATE tooth #2 confirmed genuine not stapled, DOC-01 prose read confirmed honest)

**v1.3 DONE gate cleared:** live on real Linux via Colima+Docker, ONE shared audit.db across 3 sessions (`live_acceptance_v1_3_composed`) — a hostile doc read demotes the session (I1), deterministic extraction derives a tainted recipient+body pair, the executor Blocks both (I2+CONTENT-01) with genuinely-propagated (not stapled) taint re-proven against these exact live anchors, `caprun confirm` sends exactly once via the real SMTP adapter, a SEPARATE hostile block is denied sending nothing (Mailpit count==0 AND no send-attempt ledger entry), and a clean trusted-intent send is Allowed and delivers ungated in the SAME run — all 3 sessions independently `verify_chain`-true. All 20 v1.3 requirement IDs Complete. No git tag (Ben's call).

</details>

<details>
<summary>✅ v1.4 — Trust-Boundary Integrity & the Adversarial Planner (Phases 18-22) — SHIPPED 2026-07-11</summary>

**Milestone goal:** Fix a confirmed live cross-connection trust bypass in the broker (Phase 0 — a security fix, gated by an already-red regression test), then prove the trust boundary is indifferent to planner intelligence by putting an adversarial LLM planner behind it (Phase 1+) — a hostile injected document makes the planner *comply* and try to route a tainted value to `email.send`, and the executor **Blocks deterministically** anyway, with genuine taint propagation re-verified live (the §9 standard: `verify_chain` true, Mailpit == 0), because the value flows around the planner through the worker's own mint sites, never through the planner's tokens.

- [x] **Phase 18: Trust-Boundary Coherence Design Gate** - A DESIGN doc resolving the cross-connection fix shape, the replay-risk re-earning, the three-mint-site audit, the decision-oracle question, the forward-looking per-verb capability split, and guard-(c)'s status exists and clears a fresh adversarial review, before any `server.rs` change (completed 2026-07-11)
- [x] **Phase 19: Cross-Connection Trust Coherence Fix** - The broker rejects a second connection to an already-active session, closing the cross-connection `ProvideIntent` bypass; the regression test goes green by fixing the broker, never by weakening its assertions (completed 2026-07-11)
- [x] **Phase 20: Planner Seam & Capability Split** - A designed `Planner` trait/seam exists, a planner-role connection can never hold a mint verb, and the planner is structurally kept out of the worker's raw-bytes path (completed 2026-07-11)
- [x] **Phase 21: Adversarial LLM Planner** - A minimal LLM-backed planner, running behind the new seam, emits only `PlanNode{sink, args}` — no literal field to carry (completed 2026-07-11)
- [x] **Phase 22: Adversarial Gate Proof & Residual Disclosure** - A hostile-doc-primed planner complies and is Blocked deterministically with genuine, live-verified taint propagation; T2 is documented as the accepted v1.4 residual (completed 2026-07-11)

**v1.4 DONE gate cleared:** live on real Linux, a hostile document's injection reaches a genuine OpenAI-backed `LlmPlanner` via a taint-tracked `task_instruction` channel (never itself a sink-arg value); the model complies and routes the tainted handle to `to`; the executor Blocks it deterministically (`verify_chain` true, Mailpit==0 for the attacker); a trusted-intent control in the SAME composed run Allows and delivers exactly once. Full default `scripts/mailpit-verify.sh` recipe: 46 test groups, 0 failed, real exit 0. T2 (slot-type binding) documented as the accepted residual, deferred to v1.5. All v1.4 requirement IDs Complete. No git tag, not pushed (Ben's call).

</details>

<details>
<summary>✅ v1.5 — Slot-Type Binding Enforcement (T2) (Phases 23-25) — SHIPPED 2026-07-12</summary>

Full detail archived in [`milestones/v1.5-ROADMAP.md`](milestones/v1.5-ROADMAP.md).

**Milestone goal:** Close v1.4's accepted residual #5 (T2) — the executor gains a structural check that a resolved value's semantic origin matches the semantic role of the plan-node slot it's routed into, so a misrouted `UserTrusted` handle (e.g. a subject-typed string landed in `to`) is caught even though it is neither untrusted (I2 doesn't fire) nor a class-level deny (I0/I1 don't apply).

- [x] **Phase 23: Slot-Type Binding Design Gate** (2/2 plans) — `DESIGN-slot-type-binding.md` cleared a fresh non-self adversarial review before any TCB code — completed 2026-07-12
- [x] **Phase 24: Slot-Type Binding Enforcement** (3/3 plans) — origin_role mint-time tag threaded through all mint sites, hardcoded `expected_role()` table, exhaustive `DenyReason::SlotTypeMismatch`, fail-closed Step 1c in `submit_plan_node` — completed 2026-07-12
- [x] **Phase 25: Regression & Live Proof** (3/3 plans) — held-out swapped subject↔recipient deny test (genuine audit chain), 0-NEEDS-FIX regression audit, independent bare `mailpit-verify.sh` green on real Linux (309 passed/0 failed) + human DONE sign-off — completed 2026-07-12

**v1.5 DONE gate cleared:** a deliberately swapped subject↔recipient handle pair (both `UserTrusted`) hard-Denies with `SlotTypeMismatch` via Step 1c through the real broker path, with a durable `plan_node_evaluated` audit event and `verify_chain` true — proven live on real Linux. Regression audit found 0 fixture bypasses; full-workspace regression independently re-run green. All 11 requirements (DESIGN-07..10, T2-02..08) Complete; milestone audit PASSED (11/11 reqs, 5/5 integration hops wired). No git push yet (Ben's call).

</details>

<details>
<summary>✅ v1.6 — Security Hardening (close the residuals) (Phases 26-30) — SHIPPED 2026-07-17</summary>

Full detail archived in [`milestones/v1.6-ROADMAP.md`](milestones/v1.6-ROADMAP.md).

**Milestone goal:** Close the five standing TCB-local security residuals v1.1–v1.5 accumulated and documented as accepted caveats — turning each honesty qualifier into an enforced guarantee, without adding any new external-effect surface. Design-gate-first (Phase 26), implementation grouped by blast radius (27-29), dedicated live-proof close (30).

- [x] **Phase 26: Security Hardening Design Gate** (2/2 plans) — `DESIGN-security-hardening.md` (mechanism + fail-closed default for all five residuals) cleared a fresh non-self adversarial review before any TCB code (DESIGN-11/12) — completed 2026-07-12
- [x] **Phase 27: Session & Connection Integrity Hardening** (2/2 plans) — fd release itself demotes the session to draft-only (fstat inode-identity, HARDEN-01) + forced-Active CreateSession mint compiled out of the production binary (cfg, HARDEN-04); folded in the X-04 shared-session_status fix — completed 2026-07-12
- [x] **Phase 28: Authenticated Audit Chain** (5/5 plans) — keyed HMAC-SHA256 chain + MAC'd chain_anchor truncation/orphan detection + pending_confirmations whole-row MAC + confirm/deny entry gates + F1 key custody (HARDEN-02) — completed 2026-07-13
- [x] **Phase 29: Sink-Path Hardening — Replay CAS & contents Slot** (3/3 plans) — content-derived idempotency-key CAS makes a replayed Allowed `email.send` at-most-once (HARDEN-03) + `file.create` `contents` given expected-role/content-sensitivity under I2 (HARDEN-05) — completed 2026-07-17
- [x] **Phase 30: Regression & Live Proof** (2/2 plans) — new `scripts/verify-harden04-featureless.sh` closes the criterion-4 self-skip false-assurance gap; full workspace re-run green on real Linux (331 passed/0 failed, 49 suites) + a proven test per closed residual (HARDEN-06) — completed 2026-07-17

**v1.6 DONE gate cleared:** all 5 residuals enforced and proven live on real Linux (bare `mailpit-verify.sh` 331/0 + a separate featureless-build gate for HARDEN-04) with true-exit-before-pipe discipline; an independent adversarial code-trace APPROVED the diff (2 stale-comment fixes folded); milestone audit PASSED (8/8 requirements, 5/5 cross-phase seams wired). No git push yet (Ben's call).

</details>

### 🚧 v1.7 — Effect Breadth I (`process.exec` + Filesystem Breadth) (In Progress)

**Milestone Goal:** Give caprun the two effect primitives a coding agent minimally needs — running a command in the sandbox with **captured + tainted** output (`process.exec`), and reading/editing repo files beyond single-file create (filesystem breadth) — each routed through the same plan-node → taint → executor(I2) → audit discipline. First milestone toward the **Safe Coding Agent** anchor. Design-gate-first (Phase 31 — `process.exec` under Landlock+seccomp is the riskiest primitive to date), implementation split by blast radius (32 exec sink, 33 fs breadth), dedicated live-proof close (34).

**Standing precedent honored:** no `crates/executor` / `crates/brokerd` / `crates/sandbox` / `crates/runtime-core` TCB code before Phase 31's DESIGN doc clears a fresh non-self adversarial code-trace (v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26). The orchestrator — not a gsd-executor — owns that review spawn.

#### Phase 31: Effect-Breadth Design Gate

**Goal**: A reviewed DESIGN doc pins the broker-spawned confined-child-`exec` model and the filesystem read/write-breadth model, and clears a fresh non-self adversarial code-trace — hard-blocking every subsequent TCB-code phase.
**Depends on**: Phase 30 (v1.6 shipped)
**Requirements**: DESIGN-13, DESIGN-14
**Success Criteria** (what must be TRUE):

  1. `planning-docs/DESIGN-effect-breadth-exec.md` exists and pins the broker-spawned confined-child-`exec` model (how the child is spawned from the broker — the confined worker cannot `execve` per seccomp deny-execve — how it is confined, and how stdout/stderr are captured and taint-minted) AND the filesystem read/write-breadth model.
  2. The doc pins the **fail-closed defaults** for both new sinks — `process.exec` command/arg schema + (dis)allow posture, exec-output taint label + `origin_role`, and fs read/write path & slot constraints — consistent with I0/I1/I2 and v1.5 slot-type binding; nothing in it disables or bypasses I2, and no new raw `EffectRequest` path is introduced.
  3. A fresh, **non-self** adversarial code-trace review clears the doc (all findings resolved), recorded in a gate record; no `crates/executor`/`brokerd`/`sandbox`/`runtime-core` TCB code is written before this gate clears.

**Plans**: 2 plans
**Wave 1**

- [x] 31-01-PLAN.md — Author `DESIGN-effect-breadth-exec.md`: pin the broker-spawned confined-child `process.exec` model + fs read/write-breadth model + fail-closed defaults for both new sinks (DESIGN-13/14)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 31-02-PLAN.md — Fresh non-self Fable-5 adversarial code-trace clears the doc; record clearance in `DESIGN-GATE-RECORD-v1.7.md`; reconfirm no TCB code (DESIGN-13/14)

#### Phase 32: `process.exec` Sink — Broker-Spawned Confined Child

**Goal**: caprun can run a command as a broker-spawned confined child whose captured stdout/stderr are genuinely taint-minted and deterministically I2-enforced.
**Depends on**: Phase 31
**Requirements**: EXEC-01, EXEC-02, EXEC-03, EXEC-04
**Success Criteria** (what must be TRUE):

  1. A `process.exec` plan-node sink runs a command **as a broker-spawned confined child process** (mediated like the v1.4 caprun-planner sidecar / adapter-fs fd-pass), never via the confined worker's own `execve`.
  2. The child's stdout/stderr are captured and **taint-minted as untrusted**, producing a ValueNode whose provenance chain is genuinely rooted at the `exec` Event (the sole exec-output taint-mint site — no stapling).
  3. A tainted exec-output value routed to a sensitive sink arg is deterministically **Blocked** by the executor, verifiable as an unbroken audit-DAG edge (exec Event → ValueNode → sink arg → block) with `verify_chain` true.
  4. The exec child is itself **kernel-confined** (Landlock + seccomp + default-deny net + resource/time limits), the sink is **fail-closed on arg-schema**, and a durable audit Event records the spawn and exit.

**Plans**: 6/6 plans complete

**Wave 1**

- [x] 32-01-PLAN.md — Foundational: `TaintLabel::ExecRaw` + `process.exec` sink tables (schema/sensitivity/expected_role), table-entries-only (EXEC-01..04)
- [x] 32-02-PLAN.md — Sandbox exec-child confinement primitives: `exec_child_ruleset` (narrow-allow Landlock) + `exec_child_filter` (net-deny, no execve-deny) (EXEC-04)

**Wave 2** *(blocked on Wave 1)*

- [x] 32-03-PLAN.md — `caprun-exec-launcher` binary: self-confines post-fork then execve the target (Option B) (EXEC-01/04)

**Wave 3** *(blocked on 32-01, 32-03)*

- [x] 32-04-PLAN.md — Exec sink module `invoke_process_exec`: spawn launcher, capture, wall-clock timeout, byte-cap, two-phase `process_exited`/`process_spawn_failed` audit (EXEC-01/04)

**Wave 4** *(blocked on 32-01, 32-04)*

- [x] 32-05-PLAN.md — `mint_from_exec` (non-stapled) + Gate-3 extension + `output_value_id` wire field + server.rs process.exec Allowed dispatch (EXEC-02/03)

**Wave 5** *(blocked on 32-05, 32-03)*

- [x] 32-06-PLAN.md — EXEC-03 acceptance (genuine taint→I2 Block) + exec-child confinement negative test + mandatory Linux compile-check (EXEC-02/03/04)

#### Phase 33: Filesystem Read/Write Breadth

**Goal**: The worker can read many workspace files and modify existing files, all resolved beneath `WorkspaceRoot`, taint-minted, and governed by the executor under the same I2 / slot-type-binding discipline.
**Depends on**: Phase 31
**Requirements**: FS-01, FS-02, FS-03
**Success Criteria** (what must be TRUE):

  1. The worker can **read multiple workspace files** beyond the single current read path, each resolved beneath `WorkspaceRoot` via `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`, taint-minted as untrusted like the existing read path.
  2. A filesystem **write/edit sink modifies an existing file** within `WorkspaceRoot` (beyond `file.create`'s `O_EXCL` new-file-only), fail-closed on path schema, kernel-confined, and durably audited.
  3. The fs write/edit sink args are governed by the executor under the **same I2 / slot-type-binding discipline** — a tainted path or contents in a sensitive slot Blocks; there is no I2 bypass and no new raw `EffectRequest` path.

**Plans**: TBD

#### Phase 34: Regression & Live Proof (v1.7 DONE)

**Goal**: On real Linux, the new sinks are proven end-to-end and the full workspace regresses green with no regression to v1.0–v1.6.
**Depends on**: Phase 32, Phase 33
**Requirements**: LIVE-01, LIVE-02
**Success Criteria** (what must be TRUE):

  1. A composed acceptance run on **real Linux** proves end-to-end: an `exec` whose tainted output is routed to a sensitive sink arg is **Blocked** (I2, genuine non-stapled taint chain, `verify_chain` true); a clean exec/fs path is **Allowed**; a fs write/edit within `WorkspaceRoot` succeeds and is audited — via `scripts/mailpit-verify.sh` or an exec-scoped equivalent, true-exit-before-pipe.
  2. **Full-workspace regression** re-runs green on real Linux with **no regression to v1.0–v1.6**, asserted on counts + named tests (not exit 0 through a pipe), plus a dedicated negative test per new sink.

**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Substrate Foundation | v1.0 | 2/2 | Complete | 2026-06-29 |
| 2. Security Design Gate | v1.0 | 3/3 | Complete | 2026-06-29 |
| 3. Confinement & Mediation Substrate | v1.0 | 5/5 | Complete | 2026-06-29 |
| 4. Value-Injection Security Demo (v0 DONE) | v1.0 | 5/5 | Complete | 2026-06-30 |
| 5. Runtime Spine & Live §9 Email Block | v1.1 | 4/4 | Complete | 2026-06-30 |
| 6. Deterministic Planner & Intent Input | v1.1 | 5/5 | Complete | 2026-07-01 |
| 7. file.create Sink, Enforcement Hardening & Full Acceptance | v1.1 | 6/6 | Complete | 2026-07-01 |
| 8. Session-Trust & Confirmation Design Gate | v1.2 | 3/3 | Complete | 2026-07-06 |
| 9. Session Trust State (I1 + I0) | v1.2 | 4/4 | Complete | 2026-07-07 |
| 10. Single-Shot Confirmation Loop | v1.2 | 3/3 | Complete | 2026-07-07 |
| 11. Live Acceptance — Tainted Session, Human Gate | v1.2 | 1/1 | Complete | 2026-07-07 |
| 12. Content, Adapter & Confirm-Binding Design Gate | v1.3 | 3/3 | Complete   | 2026-07-07 |
| 13. Real Broker-Mediated SMTP Adapter | v1.3 | 4/4 | Complete    | 2026-07-08 |
| 14. Content-Sensitive Sink-Arg Blocking | v1.3 | 2/2 | Complete    | 2026-07-08 |
| 15. Deterministic Doc→Action Extraction | v1.3 | 4/4 | Complete    | 2026-07-08 |
| 16. Confirm UX, Literal Binding & Negative Controls | v1.3 | 4/4 | Complete    | 2026-07-09 |
| 17. Live Acceptance & Framing Honesty | v1.3 | 4/4 | Complete | 2026-07-09 |
| 18. Trust-Boundary Coherence Design Gate | v1.4 | 2/2 | Complete    | 2026-07-11 |
| 19. Cross-Connection Trust Coherence Fix | v1.4 | 2/2 | Complete    | 2026-07-11 |
| 20. Planner Seam & Capability Split | v1.4 | 3/3 | Complete    | 2026-07-11 |
| 21. Adversarial LLM Planner | v1.4 | 4/4 | Complete    | 2026-07-11 |
| 22. Adversarial Gate Proof & Residual Disclosure | v1.4 | 3/3 | Complete    | 2026-07-11 |
| 23. Slot-Type Binding Design Gate | v1.5 | 2/2 | Complete    | 2026-07-12 |
| 24. Slot-Type Binding Enforcement | v1.5 | 3/3 | Complete    | 2026-07-12 |
| 25. Regression & Live Proof | v1.5 | 3/3 | Complete    | 2026-07-12 |
| 26. Security Hardening Design Gate | v1.6 | 2/2 | Complete    | 2026-07-12 |
| 27. Session & Connection Integrity Hardening | v1.6 | 2/2 | Complete    | 2026-07-12 |
| 28. Authenticated Audit Chain | v1.6 | 5/5 | Complete   | 2026-07-13 |
| 29. Sink-Path Hardening — Replay CAS & contents Slot | v1.6 | 3/3 | Complete    | 2026-07-17 |
| 30. Regression & Live Proof | v1.6 | 2/2 | Complete    | 2026-07-17 |
| 31. Effect-Breadth Design Gate | v1.7 | 2/2 | Complete    | 2026-07-17 |
| 32. `process.exec` Sink — Broker-Spawned Confined Child | v1.7 | 6/6 | Complete   | 2026-07-17 |
| 33. Filesystem Read/Write Breadth | v1.7 | 0/TBD | Not started | - |
| 34. Regression & Live Proof (v1.7 DONE) | v1.7 | 0/TBD | Not started | - |
