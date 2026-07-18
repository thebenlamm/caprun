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
- ✅ **v1.8 — Git/GitHub Adapters (Effect Breadth II)** — Phases 35-38,40 (shipped 2026-07-18; git.push/Phase 39 deferred to v1.9)
- 🚧 **v1.9 — Authorized Egress + Policy & Audit Surface** — Phases 41-46 (in progress; roadmapped 2026-07-18)

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

<details>
<summary>✅ v1.8 — Git/GitHub Adapters (Effect Breadth II) (Phases 35-38, 40) — SHIPPED 2026-07-18</summary>

Full detail archived in [`milestones/v1.8-ROADMAP.md`](milestones/v1.8-ROADMAP.md).

**Milestone goal:** Add the external-effect sinks that make a coding agent's work durable and shareable — `git.commit`, `git.push`, `github.pr`, and read-only `http.request` egress — each routed through caprun's locked plan-node → taint → executor(I2) → audit-DAG path, proving the Safe Coding Agent anchor end-to-end.

- [x] **Phase 35: DESIGN Gate + Fresh Adversarial Code-Trace** (2/2 plans) — `DESIGN-git-github-http-sinks.md` closes all 11 design-gate-blocking pitfalls, cleared a fresh non-self adversarial code-trace (2 rounds, APPROVE) before any TCB code — completed 2026-07-18
- [x] **Phase 36: `git.commit` Sink** (2/2 plans) — broker-spawned confined-child `git commit` (MutateReversible, reuses v1.7 `caprun-exec-launcher` + `mint_from_exec`), git config/hooks neutralized — completed 2026-07-18
- [x] **Phase 37: `http.request` GET Egress** (4/3 plans) — new `mint_from_http` inbound-taint mechanism (Observe, non-stapled, rooted on `http_response_received`), session I1 demotion on response, SSRF resolve-and-pin defense — completed 2026-07-18
- [x] **Phase 38: `github.pr` Sink** (6/5 plans) — broker-held bearer token (never in worker/planner/ValueNode/audit-literal), human auth-grant, tainted title/body Block, duplicate-PR CAS — completed 2026-07-18
- ⛔ **Phase 39: `git.push` Sink — DEFERRED TO v1.9** — Phase-35 design gate proved (BLOCKER-1) seccomp cannot pin a `connect()` destination for a confined child; the sound fully-unprivileged, destination-pinned alternative is a new trust posture needing its own design-gate. GIT-02/GIT-03 → v1.9. See `planning-docs/DECISION-git-push-deferral-v1.8.md`.
- [x] **Phase 40: CLI Compose, Sidecar `env_clear()` & Composed Live Proof (v1.8 DONE)** (4/4 plans) — sidecar `env_clear()` hermetic via `webpki-roots`, composed exec→fs→git.commit→github.pr(mock)+http-GET live proof on real Linux, 3 adversarial legs Blocked, full-workspace regression — completed 2026-07-18

**v1.8 DONE gate cleared:** 13/13 active requirements satisfied (GIT-02/03 gate-authorized deferral to v1.9, not a gap); 13/13 cross-phase integration wired; full-workspace regression 498 passed/0 failed/60 binaries on real Linux, no v1.0–v1.7 regression; every TCB change cleared a fresh non-self adversarial code-trace (DESIGN gate caught BLOCKER-1 + 3 MAJOR; Phase 37 caught a MAJOR aws-lc-rs-in-workspace + a git.commit Landlock/exit-code defect). Honest scope: proves edit→commit→open-PR (mock GitHub) + authorized HTTP fetch — the real `git.push` step is deferred to v1.9. No invariant weakened (I0/I1/I2 intact; no raw `EffectRequest`).

</details>

### 🚧 v1.9 — Authorized Egress + Policy & Audit Surface (Phases 41-46) — IN PROGRESS

**Milestone goal:** Complete the authorized-write-egress story so the Safe Coding Agent's full loop (edit → test → commit → **push** → open PR) is real, and add the first usability/trust-surface layer (a minimal per-session policy + a CLI/audit-DAG viewer) toward a design-partner-runnable slice — without weakening I0/I1/I2 or adding any raw `EffectRequest` path.

**Structure (both reviewers' agreed sequencing):** design-gate-first (Phase 41, standing precedent) → the policy foundation before the sinks it gates (Phase 42) → the two write-egress sinks, split by blast radius so a `git.push` deferral leaves http-write untouched (Phase 43 http-write, Phase 44 git.push) → the CLI/SDK + audit-DAG viewer trust surface, on the acceptance critical path (Phase 45) → composed live proof (Phase 46). `git.push` is gate-deferrable: research assesses a fully-unprivileged broker-performed smart-HTTP egress FEASIBLE and the roadmap plans it to SHIP, but if the Phase-41 gate cannot pin a sound fully-unprivileged destination-pinning mechanism, GIT-02/03 defer a 3rd time (disclosed + sign-off-gated, never silent) and the other three tracks still ship.

- [x] **Phase 41: v1.9 DESIGN Gate + Fresh Adversarial Code-Trace** — one DESIGN doc pins git.push egress + http-write egress + the policy-vs-I2 boundary (incl. POLICY-03 binding/provenance); clears a fresh non-self orchestrator-owned adversarial code-trace before any TCB code (completed 2026-07-18)
- [x] **Phase 42: Policy Layer — Binding, Enforcement & the I2 Boundary** — a minimal per-session policy narrows which sinks/args are callable, is bound from a trusted source outside the worker's reach, and can never override I2 (completed 2026-07-18)
- [x] **Phase 43: `http.request` WRITE (POST/PUT) Egress** (4/4 plans) — a DISTINCT `http.request.write` sink classed CommitIrreversible (the MAJOR-1 I0-escape fix), taint-governed body/url under I2, exact {POST,PUT} method-enum gate, a distinct fail-closed `WRITE_HOST_ALLOWLIST` reusing the shipped SSRF resolve-and-pin, broker-env-only optional credential, opaque non-minting two-phase audit, Allowed-dispatch + single-shot confirm-release, proven differentially (taint the sole variable); live mock-endpoint delivery composes in Phase 46 (LIVE-05/06) — completed 2026-07-18 (compose-verify 584/0 on real Linux; fresh Fable-5 adversarial trace APPROVE, 0 defects)
- [x] **Phase 44: `git.push` — Broker-Performed Destination-Pinned Egress** (5/5 plans) — SHIPPED (did NOT defer a 3rd time). A fully-unprivileged, broker-performed smart-HTTP push (info/refs GET + git-receive-pack POST over the shipped reqwest-ring resolve-and-pin, IP frozen across both requests, redirect refused); the pack-gen child stays net-denied under the unchanged exec_child_filter (WG-2 binary-safe `run_launcher_capture_bytes` + `git pack-objects`); remote/refspec from TRUSTED intent; --force/--force-with-lease/:delete/+refspec hard-denied by construction; broker-env-only credential (Basic x-access-token) scrubbed from value-store/audit/logs; opaque non-minting audit; ALWAYS confirm-gated (no auto-dispatch arm — clean Allowed → synthetic BlockedPendingConfirmation with a MAC'd frozen-new-oid pending row) + WG-7 anti-TOCTOU freeze + WG-8 taint-provenance renderer + P33/P34 precheck-before-burn; HYG-01 zero-new-crate re-asserted — completed 2026-07-18 (compose-verify 668/0 on real Linux incl. leg_c real delivery to the mock git-receive-pack + leg_d force/delete refused + leg_e redirect refused; fresh Fable-5 adversarial trace APPROVE, 0 security defects across 8 surfaces)
- [x] **Phase 45: Thin CLI/SDK + Read-Only Audit-DAG Viewer** (4/4 plans) — SDK-01: a `caprun run <intent> <workspace> [--policy <path>]` verb binding the trusted policy at session creation (POLICY-03 enforcement point) + surfacing the blocked effect_id + `caprun review` pointer on an I2 Block, with the M7 anti-laundering fix (file-derived `--seed-from-file` content minted TAINTED via the broker-side `mint_from_read` site, operator literals stay trusted, provenance threaded through ProvideIntent — no new mint site). U1: a read-only `caprun audit <session>` viewer rendering events/decisions + verify_chain, using a load-ONLY fail-closed key (refuses absent key + `:memory:`, F1 containment, opens read-only), neutralizing every displayed literal via the shared `brokerd::display::neutralize_control_chars` (hardened this phase to also escape the Trojan-Source BiDi/zero-width spoof class per the adversarial trace). Existing confirm/deny/grant/review verbs unchanged — completed 2026-07-18 (compose-verify 691/0 on real Linux incl. the genuine end-to-end run→Block→review→audit loop; fresh Fable-5 adversarial trace APPROVE, M7 + viewer fail-closed both sound)
- [ ] **Phase 46: Composed Live Proof (v1.9 DONE)** — the full authorized-write loop, driven & inspected via the new CLI+viewer on real Linux, every adversarial/negative leg independently attributable

### Phase 41: v1.9 DESIGN Gate + Fresh Adversarial Code-Trace

**Goal**: A single reviewed DESIGN doc pins all three v1.9 TCB mechanisms — git.push egress, http-write egress, and the policy-vs-I2 boundary — and clears a fresh non-self adversarial code-trace before ANY `crates/{executor,brokerd,sandbox,runtime-core}` TCB code (unbroken precedent: v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26, v1.7 P31, v1.8 P35).
**Depends on**: Nothing new (first v1.9 phase; builds on the v1.8-shipped substrate)
**Requirements**: DESIGN-17, DESIGN-18
**Success Criteria** (what must be TRUE):

  1. A DESIGN doc exists pinning (a) the fully-unprivileged, broker-mediated, destination-pinned `git.push` egress (child net-denied; the pin in a broker/application layer that SEES the destination, NEVER seccomp — the research-recommended broker-performed git smart-HTTP transfer reusing the shipped reqwest+rustls(ring)+webpki-roots+SSRF resolve-and-pin stack), (b) the `http.request` WRITE (POST/PUT) egress, (c) the policy-vs-I2 boundary — exactly what policy can/cannot do AND where policy comes from / how it binds (POLICY-03) — carrying forward v1.8 §2 / §2.5 credential-scrub / §2.7 payload-at-confirm / §9 confirm-release.
  2. The doc formalizes the `git.push` safety-valve: if no fully-unprivileged destination-pinning mechanism proves sound, `git.push` defers (the other three tracks proceed) — a disclosed decision, never a silent drop.
  3. The DESIGN doc clears a fresh, non-self, **orchestrator-owned** adversarial code-trace (NOT a gsd-executor), with every BLOCKER/MAJOR resolved before the gate clears.
  4. No v1.9 TCB code exists until the gate clears; and the trace re-runs if the git.push trust-posture or transport-dependency choice changes mid-implementation ("the riskiest surface in the project" must not bypass its one gate).

**Plans**: 1 plan

- [x] 41-01-PLAN.md — Author planning-docs/DESIGN-v1.9-egress-policy.md (docs-only): pin git.push egress + http-write egress + policy-vs-I2 boundary incl. POLICY-03 binding; carry forward v1.8 §2/§2.5/§2.7/§9; declare the orchestrator-owned adversarial-trace gate (DESIGN-17/18)

### Phase 42: Policy Layer — Binding, Enforcement & the I2 Boundary

**Goal**: A minimal declarative per-session policy narrows WHICH sinks/args are callable, is bound by the broker from a trusted source provably outside the confined worker's reach, is immutable for the session, and can NEVER disable or override I2 — the #1 adversarial-trace risk, made structural. Lands before the sinks it gates.
**Depends on**: Phase 41 (design gate)
**Requirements**: POLICY-01, POLICY-02, POLICY-03
**Success Criteria** (what must be TRUE):

  1. A hardcoded-schema per-session policy (NOT Cedar) specifies which sinks are callable + coarse arg constraints (allowlisted hosts/paths/repos); a sink or arg the policy does not permit is refused with a **distinct, machine-checkable policy-deny outcome** separate from an I2 Block.
  2. The broker binds the policy at session creation from a trusted source, canonicalizing and **refusing** any policy that resolves at-or-beneath the workspace root (F1 containment reused verbatim from `key.rs`); the policy is immutable for the session's life and its identity/hash is recorded as a genuine audit-DAG event.
  3. A confined worker that writes/rewrites a policy file mid-session does NOT change the enforced allowlist (negative live leg).
  4. I2 executes **unconditionally on every policy-permitted call** and can never be short-circuited by any policy outcome (policy is a pre-I2 narrowing gate, never a post-I2 override) — proven by a live leg where a permissive policy does NOT weaken an I2 taint Block on an existing sink; the I2 decision stays HARDCODED in the Rust TCB executor.

**Plans**: 4 plans (waves: 01+02 → 03 → 04)

- [x] 42-01-PLAN.md — SessionPolicy hardcoded-schema type (runtime-core) + distinct DenyReason::PolicyDeny [POLICY-01]
- [x] 42-02-PLAN.md — Extract shared F1 containment helper (adapter-fs) + rewire key.rs + anti-drift gate [POLICY-03 foundation]
- [x] 42-03-PLAN.md — Executor deny-only pre-I2 policy gate + threading + POLICY-01/POLICY-02 enforcement-order proof [POLICY-01, POLICY-02]
- [x] 42-04-PLAN.md — POLICY-03 binding at session creation: trusted source, F1 refusal, immutability, hash-chained audit event [POLICY-03]

### Phase 43: `http.request` WRITE (POST/PUT) Egress

**Goal**: caprun can POST/PUT to an allowlisted host with the request BODY taint-governed and content-sensitive under I2 — the simpler write-egress sink, extending the shipped `http.request` GET path, split from git.push so a push deferral leaves it untouched.
**Depends on**: Phase 41 (design gate), Phase 42 (policy gates the write sink)
**Requirements**: HTTP-W-01
**Success Criteria** (what must be TRUE):

  1. `http.request` supports POST/PUT to a host on a **WRITE allowlist distinct from the read/GET allowlist** (a host being GET-readable does not imply it is POST/PUT-writable).
  2. A tainted request body deterministically Blocks under I2 (content-sensitive, exactly like an email/PR body); the `url` is routing-sensitive; the shipped SSRF resolve-and-pin (loopback/RFC1918/link-local/metadata/userinfo@/redirect denied) + webpki-roots egress is reused.
  3. Any write credential lives in broker-local env only (never a ValueNode/plan-arg/audit-literal/worker/planner); the captured response is scrubbed of credential material (or not minted) before it reaches the value store or audit chain.
  4. Acceptance is **differential** on real Linux: the tainted-body-Blocks leg and the clean-body-Allowed leg are identical in host/url/method/policy (taint is the sole variable), and the clean leg's body is confirmed to have actually delivered to the mock endpoint (mock records receipt) — not merely "not blocked," so a block-everything I2 regression cannot pass.

**Plans**: 4 plans (waves: 01 → 02 → 03 → 04)

- [ ] 43-01-PLAN.md — Executor TCB: register `http.request.write` (distinct id) as CommitIrreversible, schema {url,body,method}, taint/sensitivity tables, method-enum {POST,PUT} gate, production-policy allowlist [HTTP-W-01]
- [ ] 43-02-PLAN.md — Broker WRITE egress: distinct `WRITE_HOST_ALLOWLIST` + generic SSRF-pinned `invoke_http_write` + `http_write` orchestration module (broker-env credential, opaque non-minting two-phase audit, shared `prepare_http_write` precheck) [HTTP-W-01]
- [ ] 43-03-PLAN.md — Wiring: server.rs Allowed-decision dispatch arm + confirmation.rs confirm-release (guard + P33/P34 precheck + dispatch) [HTTP-W-01]
- [ ] 43-04-PLAN.md — HTTP-W-01 differential acceptance test: I0 draft-deny + tainted-body Block vs clean-body Allow (identical url/method/policy) + method-enum Deny; clean leg reaches egress (full live mock delivery → Phase 46) [HTTP-W-01]

### Phase 44: `git.push` — Broker-Performed Destination-Pinned Egress

**Goal**: caprun can push to a TRUSTED-intent remote via a fully-unprivileged, broker-mediated, destination-pinned egress with the push child kept net-denied — completing the edit→test→commit→push→open-PR loop — or, if the gate proved no sound mechanism, defer disclosed. Carries the supply-chain absence gate + two hygiene items (the transport-dep choice lands here).
**Depends on**: Phase 41 (design gate), Phase 42 (policy), Phase 43 (write-egress path proven)
**Requirements**: GIT-02, GIT-03, HYG-01
**Success Criteria** (what must be TRUE):

  1. `git.push` performs a broker-mediated smart-HTTP transfer with the destination pinned in the application layer (reqwest resolve-and-pin, IP frozen across the two-request exchange); the push child stays net-denied (no seccomp relaxation); remote/refspec come from TRUSTED intent, never the untrusted repo's `.git/config`; `--force`/`--force-with-lease`/ref-deletion/`+`-force-refspec are hard-denied by construction (unreachable even via confirm).
  2. The push credential lives in broker-local env only (never a ValueNode/plan-arg/audit-literal/child/planner), is never followed across a `receive-pack` redirect, and captured child/transport output is scrubbed of credential/URL material before value-store/audit (§2.5).
  3. A tainted push `remote`/`refspec` deterministically Blocks under I2, releasable only by single-shot human confirm whose terminal audit event precedes the terminal state (P33/P34 `prepare_git_push` precheck); at confirm the human sees the pushed payload (commit range/branch + a summary flagging any pushed file whose content derives from untrusted taint) and the pushed pack is generated from that confirmed range **at-or-after confirm** (no payload-vs-destination TOCTOU).
  4. The workspace-scoped supply-chain **absence assertion** re-runs after the transport-dep choice (`cargo tree --workspace -i` = absent for aws-lc-rs/openssl-sys; ring-only + webpki-roots), enumerating any new transport deps; plus the `compose-verify.sh` feature-OFF guard and the workspace-wide `check-invariants` Gate 4b grep (HYG-01).
  5. **Safety-valve:** if the Phase-41 gate proved no sound fully-unprivileged destination-pinning mechanism exists, GIT-02/03 defer a 3rd time — a disclosed, sign-off-gated deferral (the git.push leg auto-descopes from LIVE-05/06), never shipping arbitrary child egress and never a silent drop.

**Plans**: 5 plans (4 waves)
- [ ] 44-01-PLAN.md — Executor-TCB registration: git.push sink schema {remote,refspec}, CommitIrreversible + routing-sensitive tables, PRODUCTION_SINKS, effect-ontology WG-4 reconcile, llm-planner WG-5 (GIT-02/03)
- [ ] 44-02-PLAN.md — Protocol substrate: pkt-line encode/decode + advertisement/report-status parsers, validate_git_refspec + structural force/delete denial, WG-1 single-frozen-IP primitive (GIT-02)
- [ ] 44-03-PLAN.md — Pack-gen + transport: WG-2 binary confined-spawn variant + net-denied pack-objects child, broker-env credential + distinct host-allowlist + opaque scrubbed audit, frozen-IP two-request transfer driver (GIT-02)
- [ ] 44-04-PLAN.md — Dispatch + confirm-release: always-confirm-gate (no auto-dispatch), new-oid freeze thread (WG-7), Step-4.8d prepare_git_push precheck, commit-range/taint-provenance confirm renderer (WG-8) (GIT-02/03)
- [ ] 44-05-PLAN.md — Differential acceptance + git-receive-pack mock (WG-9) + HYG-01 supply-chain re-run/compose feature-OFF guard/Gate 4b broadening (GIT-02/03, HYG-01)

### Phase 45: Thin CLI/SDK + Read-Only Audit-DAG Viewer

**Goal**: An operator can define an intent, point it at a workspace with a trusted policy, run it end-to-end, and INSPECT the proof — the design-partner-runnable trust surface. On the acceptance critical path (LIVE-05 requires the composed proof be driven AND inspected via this CLI + viewer), not trailing tooling. No web UI.
**Depends on**: Phase 42 (binds the trusted policy — the POLICY-03 enforcement point), Phase 43 & Phase 44 (something to drive and inspect)
**Requirements**: SDK-01, U1
**Success Criteria** (what must be TRUE):

  1. A thin CLI/SDK defines an intent, points at a workspace, and runs it end-to-end against the broker (extends, does not replace, the existing `caprun confirm`/`deny`/`grant`/`review` verbs); the run entrypoint takes the trusted policy path and binds it at session creation.
  2. When a sink Blocks under I2 the entrypoint surfaces the blocked `effect_id` (+ the `caprun review` pointer) so the operator can reach confirm/deny/grant; SDK-constructed values carry trusted provenance ONLY for genuinely operator-typed literals — any file-/stream-/env-sourced content the SDK ingests is minted TAINTED (draft-only per I0/I1), not laundered.
  3. A read-only audit-DAG viewer renders a session's events/decisions and surfaces `verify_chain`, reusing the exact `load_or_create_key` MAC-key custody + F1 containment refusal — failing closed (refusing to render a `verify_chain` verdict) if the key is absent, never loading a fresh/`:memory:` key, out of the confined worker's reach.
  4. All tainted literal bytes (e.g. a tainted commit message or POST body) are control-char-neutralized/escaped before display — the terminal viewer never interprets attacker-tainted content as ANSI/formatting (audit-line-spoofing surface closed).

**Plans**: 4 plans (waves: 01+02 → 03 → 04)

- [ ] 45-01-PLAN.md — SDK-01 `caprun run` verb + `--policy` flag (WG-6/POLICY-03) + post-Block effect_id surfacing (WG-5/Matt #2) + M7 anti-laundering disjointness (WG-1) [SDK-01]
- [ ] 45-02-PLAN.md — extract `neutralize_control_chars` into a shared `brokerd::display` pub fn + anti-drift test (WG-2, U1 M3 foundation) [U1]
- [ ] 45-03-PLAN.md — `caprun audit <session> <db>` read-only viewer: load-only fail-closed `load_existing_key` (WG-4/U1 M2), open-by-path DAG walk + verify_chain, universal literal neutralization (WG-2/U1 M3) [U1]
- [ ] 45-04-PLAN.md — end-to-end acceptance: `caprun run` → I2 Block → surfaced effect_id → `caprun review` → `caprun audit` renders + verify_chain; fail-closed-on-absent-key + :memory:-refused + tainted-literal-neutralized negatives (LIVE-05 driver-inspector setup) [SDK-01, U1]

### Phase 46: Composed Live Proof (v1.9 DONE)

**Goal**: The full authorized-write loop runs and is inspected on real Linux, with every adversarial/negative leg independently attributable — the v1.9 DONE gate. Mirrors v1.2 P11, v1.3 P17, v1.4 P22, v1.5 P25, v1.6 P30, v1.7 P34, v1.8 P40.
**Depends on**: Phases 42, 43, 44, 45 (all sinks + policy + the CLI/viewer driver-inspector)
**Requirements**: LIVE-05, LIVE-06
**Success Criteria** (what must be TRUE):

  1. A composed workflow — `process.exec` (test) → filesystem edit → `git.commit` → `git.push` → `github.pr` PLUS an `http.request` POST leg — runs on real Linux (mock git remote + mock endpoint), **DRIVEN and INSPECTED via the new CLI + audit-DAG viewer**, with every step gated/tainted/audit-DAG-chained and `verify_chain` true across the run.
  2. Five **independently attributable** negative legs each deterministically Block/refuse: (1) a tainted push remote/refspec (I2 Blocks); (2) a tainted POST body (I2 Blocks); (3) a policy-deny leg (an off-allowlist sink refused via the distinct policy-deny outcome) — where the I2-Block legs run a sink+arg the policy explicitly PERMITS (distinct machine-checkable terminal-event tags asserted separately); (4) a destination-pin negative (a push/POST redirected/off-pin refused at the broker/application layer); (5) a credential-absence assertion (after a real push, no credential or remote-URL material in the value store or audit chain).
  3. Full-workspace regression green on real Linux, no v1.0–v1.8 regression.
  4. If GIT-02 deferred, the `git.push` leg auto-descopes AND the deferral is recorded as a disclosed milestone gap requiring explicit user sign-off — never an orchestrator-autonomous silent drop.

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
| 37. `http.request` GET Egress | v1.8 | 4/3 | Complete    | 2026-07-18 |
| 38. `github.pr` Sink | v1.8 | 6/5 | Complete    | 2026-07-18 |
| 39. `git.push` Sink | v1.8 | — | ⛔ Deferred → v1.9 | 2026-07-18 |
| 40. CLI Compose, Sidecar env_clear() & Composed Live Proof (v1.8 DONE) | v1.8 | 4/4 | Complete    | 2026-07-18 |
| 41. v1.9 DESIGN Gate + Fresh Adversarial Code-Trace | v1.9 | 1/1 | Complete    | 2026-07-18 |
| 42. Policy Layer — Binding, Enforcement & the I2 Boundary | v1.9 | 4/4 | Complete    | 2026-07-18 |
| 43. `http.request` WRITE (POST/PUT) Egress | v1.9 | 0/? | Not started | - |
| 44. `git.push` — Broker-Performed Destination-Pinned Egress | v1.9 | 0/? | Not started | - |
| 45. Thin CLI/SDK + Read-Only Audit-DAG Viewer | v1.9 | 0/4 | Not started | - |
| 46. Composed Live Proof (v1.9 DONE) | v1.9 | 0/? | Not started | - |
