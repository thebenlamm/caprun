# Roadmap: AgentOS

## Milestones

- ✅ **v1.0 MVP — AgentOS v0** — Phases 1-4 (shipped 2026-06-30)
- ✅ **v1.1 — Usable Runtime (Live §9 from the CLI)** — Phases 5-7 (shipped 2026-07-01)
- ✅ **v1.2 — Tainted Session, Human Gate** — Phases 8-11 (shipped 2026-07-07)
- ✅ **v1.3 — Doc → Action Assistant** — Phases 12-17 (shipped 2026-07-09)
- ✅ **v1.4 — Trust-Boundary Integrity & the Adversarial Planner** — Phases 18-22 (shipped 2026-07-11)
- ✅ **v1.5 — Slot-Type Binding Enforcement (T2)** — Phases 23-25 (shipped 2026-07-12)
- ✅ **v1.6 — Security Hardening (close the residuals)** — Phases 26-30 (shipped 2026-07-17)
- ✅ **v1.7 — Effect Breadth I (`process.exec` + Filesystem Breadth)** — Phases 31-34 (shipped 2026-07-18)
- 🚧 **v1.8 — Git/GitHub Adapters (Effect Breadth II)** — Phases 35-40 (in progress)

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

<details>
<summary>✅ v1.7 — Effect Breadth I (`process.exec` + Filesystem Breadth) (Phases 31-34) — SHIPPED 2026-07-18</summary>

Full detail archived in [`milestones/v1.7-ROADMAP.md`](milestones/v1.7-ROADMAP.md).

**Milestone goal:** Give caprun the two effect primitives a coding agent minimally needs — running a command in the sandbox with **captured + tainted** output (`process.exec`), and reading/editing repo files beyond single-file create (filesystem breadth) — each routed through the same plan-node → taint → executor(I2) → audit discipline. First milestone toward the **Safe Coding Agent** anchor. Design-gate-first (Phase 31), implementation split by blast radius (32 exec sink, 33 fs breadth), dedicated live-proof close (34).

- [x] **Phase 31: Effect-Breadth Design Gate** (2/2 plans) — `DESIGN-effect-breadth-exec.md` (broker-spawned confined-child exec model + fs read/write-breadth model + fail-closed defaults) cleared a fresh non-self adversarial code-trace before any TCB code (DESIGN-13/14) — completed 2026-07-17
- [x] **Phase 32: `process.exec` Sink — Broker-Spawned Confined Child** (6/6 plans) — `process.exec` as a fail-closed, I2-governed sink: `caprun-exec-launcher` self-confines (Landlock+seccomp) post-fork and execs the target; captured stdout/stderr `mint_from_exec`-minted (non-stapled, rooted on `process_exited`) and wired back via `output_value_id`; EXEC-01..04 proven on real Linux (4 genuine bugs caught only by the Linux run) — completed 2026-07-17
- [x] **Phase 33: Filesystem Read/Write Breadth** (5/5 plans) — `WorkspaceRoot::write_within` (O_WRONLY|O_TRUNC, existing-file-only) + `file.write` broker sink (two-phase audit) + per-session `RequestFd` count limiter (256, fail-closed) + `file.write` executor I2 schema/sensitivity/slot-role tables; genuine non-stapled taint→I2 Block proven live (FS-01/02/03) — completed 2026-07-18
- [x] **Phase 34: Regression & Live Proof (v1.7 DONE)** (4/4 plans) — EXEC-05 `process.exec` confirm-release (`invoke_process_exec_from_resolved` + async `confirm()` guard/dispatch/precheck); orchestrator-owned release gates (Linux compile-check D-15 + fresh Fable-5 trace D-16); `live_acceptance_v1_7_composed.rs` 4-leg composed proof (LIVE-01) + full-workspace regression (LIVE-02) green on real Linux — completed 2026-07-18

**v1.7 DONE gate cleared:** all EXEC-01..05 + FS-01..03 + LIVE-01/02 proven on real Linux via `mailpit-verify.sh` (LIVE-01 composed 4-leg true-exit-0; LIVE-02 full regression 391/0, no v1.0–v1.6 regression) with true-exit-before-pipe discipline. The Phase-34 fresh Fable-5 confirm-release trace caught a real MAJOR audit-gap (a burned one-shot confirmation with no terminal event — the P33 MAJOR-1 class) that the passing verifier + green gates missed; fixed + re-traced APPROVED, and the mint-vs-spec deviation reconciled. A post-close env_clear gap-closure (exec-child + worker broker-secret inheritance) was fixed + independently APPROVED + Linux-verified (391/0); the planner-sidecar variant is deferred to v1.8. Human DONE sign-off + push authorized by Ben (2026-07-18). All 31-34 phases verified passed; closed with 4 acknowledged-deferred debt items (3 pre-existing todos + the v1.8 sidecar follow-up).

</details>

### 🚧 v1.8 — Git/GitHub Adapters (Effect Breadth II) (In Progress)

**Milestone Goal:** Add the external-effect sinks that make a coding agent's work durable and shareable — `git.commit`, `git.push`, `github.pr`, and read-only `http.request` egress — each routed through caprun's locked plan-node → taint → executor(I2) → audit-DAG path, proving the Safe Coding Agent anchor end-to-end. Design-gate-first (Phase 35), then mechanical/wiring phases ordered so each new mechanism is proven before the sink that depends on it — Phase 36 `git.commit` (lowest risk, reuses the v1.7 exec-launcher + `mint_from_exec`), Phase 37 `http.request` GET (establishes the NEW `mint_from_http` inbound-taint mechanism), Phase 38 `github.pr` (reuses http egress + inbound mint, adds the bearer token + human auth-grant + duplicate-PR CAS), Phase 39 `git.push` (hardest — network-from-confined-child + push-credential injection, done after the credential boundary is proven by github.pr) — dedicated composed live-proof close (Phase 40).

**Standing precedent honored:** no `crates/executor` / `crates/brokerd` / `crates/sandbox` / `crates/runtime-core` TCB code before Phase 35's DESIGN doc clears a fresh non-self adversarial code-trace (v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26, v1.7 P31). The orchestrator — not a gsd-executor — owns that review spawn.

#### Phase 35: DESIGN Gate + Fresh Adversarial Code-Trace

**Goal**: A reviewed DESIGN doc pins the mechanism + fail-closed default for all four new sinks (`git.commit`, `git.push`, `github.pr`, `http.request`) and clears a fresh non-self adversarial code-trace — hard-blocking every subsequent TCB-code phase.
**Depends on**: Phase 34 (v1.7 shipped)
**Requirements**: DESIGN-15, DESIGN-16
**Success Criteria** (what must be TRUE):

  1. `planning-docs/DESIGN-git-github-http-sinks.md` exists and pins: per-sink effect-class (`git.commit`=MutateReversible, `git.push`/`github.pr`=CommitIrreversible, `http.request`=Observe); the `mint_from_http` inbound-taint mechanism + session demotion; the git config/hook neutralization surface; `git.push` destination-pinning + credential-injection mechanism; the SSRF resolve-and-pin model; the `github.pr` human auth-grant model; the `env_clear()` TLS-cert allowlist policy; duplicate-PR CAS semantics; and the new `TaintLabel` variants.
  2. The doc explicitly closes all 11 design-gate-blocking pitfalls identified in research (git config/hook RCE, swapped-remote push, tainted PR-body/commit-message exfil, SSRF, credential leak, replay, etc.) with a named mechanism per pitfall — nothing in it disables or bypasses I2, and no new raw `EffectRequest` path is introduced.
  3. A fresh, **non-self** adversarial code-trace review (orchestrator-owned, not a gsd-executor) clears the doc (all findings resolved), recorded in a gate record; no `crates/executor`/`brokerd`/`sandbox`/`runtime-core` TCB code is written before this gate clears.

**Plans**: 2 plans

- [x] 35-01-PLAN.md — Author `DESIGN-git-github-http-sinks.md` (all four sinks, 11 pitfall closures, three forks, mint_from_http/HttpRaw/CAS/auth-grant, invariant preservation) [DESIGN-15]
- [x] 35-02-PLAN.md — Orchestrator-owned fresh non-self adversarial code-trace + `DESIGN-GATE-RECORD-v1.8.md` clearance [DESIGN-16]

#### Phase 36: `git.commit` Sink

**Goal**: caprun can commit staged workspace changes via a broker-spawned confined-child `git`, with the commit message's taint genuinely propagated and git config/hooks neutralized.
**Depends on**: Phase 35
**Requirements**: GIT-01
**Success Criteria** (what must be TRUE):

  1. A `git.commit` plan-node sink runs `git commit` via the broker-spawned confined-child launcher (reusing the v1.7 `caprun-exec-launcher` + `mint_from_exec` pattern), classified **MutateReversible** so it survives an I1-demoted session.
  2. A tainted commit message routed through the sink genuinely propagates downstream (not re-minted clean) — verifiable as an unbroken audit-DAG edge.
  3. git system config and hooks are neutralized in the child (`GIT_CONFIG_NOSYSTEM`, `core.hooksPath=/dev/null`, no aliases, `env_clear()`'d) — a planted malicious hook/alias in the workspace repo does not execute.

**Plans**: 2 plans

- [x] 36-01-PLAN.md — executor TCB rows: KNOWN_SINKS git.commit schema + MutateReversible effect-class + message content-sensitivity (wave 1)
- [x] 36-02-PLAN.md — brokerd git.commit sink dispatch (Pattern B launcher reuse + git config/hook neutralization) + mint_from_exec wiring + Linux-gated spawn tests (wave 2)

#### Phase 37: `http.request` GET Egress

**Goal**: caprun can make an allowlisted, read-only outbound HTTP GET whose response is minted untrusted-on-arrival and demotes the session, defended against SSRF.
**Depends on**: Phase 35
**Requirements**: HTTP-01, HTTP-02, HTTP-03
**Success Criteria** (what must be TRUE):

  1. An `http.request` sink performs a broker-mediated GET to an allowlisted host only, classified **Observe**, with `url` as an I2-gated sink arg — a non-allowlisted host is denied.
  2. The HTTP response body is minted untrusted via a new `mint_from_http` mint site rooted on a genuine `http_response_received` audit event, and the session demotes to draft-only (I1) on that response.
  3. A fetched value later routed into a sensitive sink arg is deterministically **Blocked** on a genuinely-propagated (non-stapled) taint chain — an anti-staple test proves this, per the §9 genuineness standard.
  4. Requests to loopback/RFC1918/link-local/cloud-metadata IPs, `userinfo@` tricks, and default redirect-following are all denied (resolve-and-pin SSRF defense).

**Plans**: TBD

#### Phase 38: `github.pr` Sink

**Goal**: caprun can open a GitHub pull request via a broker-held bearer token with explicit human auth-grant, tainted title/body sections deterministically blocked, and replay-safe.
**Depends on**: Phase 37 (reuses `mint_from_http` + the http egress infra)
**Requirements**: GITHUB-01, GITHUB-02, GITHUB-03, GITHUB-04
**Success Criteria** (what must be TRUE):

  1. A `github.pr` sink creates a GitHub PR via a broker-held session bearer token that is never present in the confined worker, the planner sidecar, a ValueNode, or the audit-DAG literal, classified **CommitIrreversible**.
  2. Creating a PR requires an explicit human auth-grant for the credential, distinct from single-shot confirm — a PR cannot be created on a bare confirm alone (a token's authority exceeds one PR).
  3. A tainted PR title/body section is deterministically **Blocked** (I2, reusing CONTENT-01 content-sensitivity) — the verbatim, provenance-annotated title/body is shown to the human at confirm.
  4. A replayed `github.pr` submission creates **at most one PR** (content-derived idempotency CAS committed before the API call, mirroring HARDEN-03).

**Plans**: TBD

#### Phase 39: `git.push` Sink

**Goal**: caprun can push to a remote pinned to the session's trusted intent-origin, with a tainted remote/refspec deterministically blocked and human-releasable.
**Depends on**: Phase 38 (the credential boundary is proven incrementally by `github.pr` before `git.push`'s credential-injection path is built)
**Requirements**: GIT-02, GIT-03
**Success Criteria** (what must be TRUE):

  1. A `git.push` sink pushes to a remote+branch pinned to the session's trusted intent-origin and passed explicitly (never resolved from the untrusted repo's `.git/config`), classified **CommitIrreversible**.
  2. `--force` and ref-deletion are hard-denied regardless of confirmation.
  3. A tainted push remote/refspec is deterministically **Blocked** at the sink (I2) and releasable only by single-shot human confirmation, with the confirm-release path writing the terminal audit event **before** the terminal state (the recurring P33/P34 audit-gap discipline).

**Plans**: TBD

#### Phase 40: CLI Compose, Sidecar `env_clear()` & Composed Live Proof (v1.8 DONE)

**Goal**: The full Safe Coding Agent workflow is proven end-to-end on real Linux, the planner sidecar's `env_clear()` is hermetic under compiled-in TLS roots, and every adversarial attack leg is deterministically blocked.
**Depends on**: Phase 36, Phase 37, Phase 38, Phase 39
**Requirements**: ENV-01, LIVE-03, LIVE-04
**Success Criteria** (what must be TRUE):

  1. The `caprun-planner` sidecar spawn is `env_clear()`'d and given only the minimal env it needs; all new broker-side TLS egress uses compiled-in `webpki-roots` so `env_clear()` is hermetic (no `SSL_CERT_*` / readable system store required), validated by a **live** HTTPS run.
  2. A composed agent workflow is proven on **real Linux** — `process.exec` (test) → filesystem edit → `git.commit` → `git.push` → `github.pr`, plus an `http.request` GET leg — with every step gated, tainted, and audit-DAG-chained, and `verify_chain` true across the run.
  3. Each adversarial attack leg — (a) tainted push remote/refspec, (b) tainted PR-body section, (c) tainted GET url (SSRF/exfil) — is deterministically **Blocked** with `verify_chain` true, plus a post-`env_clear()` **live** HTTPS call succeeds.
  4. Full-workspace regression is green on real Linux with **no regression to v1.0–v1.7**.

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
| 32. `process.exec` Sink — Broker-Spawned Confined Child | v1.7 | 6/6 | Complete    | 2026-07-17 |
| 33. Filesystem Read/Write Breadth | v1.7 | 5/5 | Complete    | 2026-07-18 |
| 34. Regression & Live Proof (v1.7 DONE) | v1.7 | 4/4 | Complete    | 2026-07-18 |
| 35. DESIGN Gate + Fresh Adversarial Code-Trace | v1.8 | 2/2 | Complete    | 2026-07-18 |
| 36. `git.commit` Sink | v1.8 | 2/2 | Complete    | 2026-07-18 |
| 37. `http.request` GET Egress | v1.8 | 0/TBD | Not started | - |
| 38. `github.pr` Sink | v1.8 | 0/TBD | Not started | - |
| 39. `git.push` Sink | v1.8 | 0/TBD | Not started | - |
| 40. CLI Compose, Sidecar env_clear() & Composed Live Proof (v1.8 DONE) | v1.8 | 0/TBD | Not started | - |
