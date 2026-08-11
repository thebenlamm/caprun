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
- ✅ **v1.9 — Authorized Egress + Policy & Audit Surface** — Phases 41-46 (shipped 2026-07-18)
- 🚧 **v1.10 — Multi-step Safe Coding Agent Loop** — Phases 47-52 (in progress)

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

<details>
<summary>✅ v1.9 — Authorized Egress + Policy & Audit Surface (Phases 41-46) — SHIPPED 2026-07-18</summary>

Full detail archived in [`milestones/v1.9-ROADMAP.md`](milestones/v1.9-ROADMAP.md).

**Milestone goal:** Complete the authorized-write-egress story so the Safe Coding Agent's full loop (edit → test → commit → **push** → open PR) is real, and add the first usability/trust-surface layer (a minimal per-session policy + a CLI/audit-DAG viewer) toward a design-partner-runnable slice — without weakening I0/I1/I2 or adding any raw `EffectRequest` path.

- [x] **Phase 41: v1.9 DESIGN Gate + Fresh Adversarial Code-Trace** (1/1 plans) — one DESIGN doc pins git.push egress + http-write egress + the policy-vs-I2 boundary (incl. POLICY-03 binding/provenance); clears a fresh non-self orchestrator-owned adversarial code-trace before any TCB code (completed 2026-07-18)
- [x] **Phase 42: Policy Layer — Binding, Enforcement & the I2 Boundary** (4/4 plans) — a minimal per-session policy narrows which sinks/args are callable, is bound from a trusted source outside the worker's reach, and can never override I2 (completed 2026-07-18)
- [x] **Phase 43: `http.request` WRITE (POST/PUT) Egress** (4/4 plans) — a DISTINCT `http.request.write` sink classed CommitIrreversible (the MAJOR-1 I0-escape fix), taint-governed body/url under I2, exact {POST,PUT} method-enum gate, a distinct fail-closed `WRITE_HOST_ALLOWLIST` reusing the shipped SSRF resolve-and-pin, broker-env-only optional credential, opaque non-minting two-phase audit, Allowed-dispatch + single-shot confirm-release, proven differentially (taint the sole variable); live mock-endpoint delivery composes in Phase 46 (LIVE-05/06) — completed 2026-07-18 (compose-verify 584/0 on real Linux; fresh Fable-5 adversarial trace APPROVE, 0 defects)
- [x] **Phase 44: `git.push` — Broker-Performed Destination-Pinned Egress** (5/5 plans) — SHIPPED (did NOT defer a 3rd time). A fully-unprivileged, broker-performed smart-HTTP push (info/refs GET + git-receive-pack POST over the shipped reqwest-ring resolve-and-pin, IP frozen across both requests, redirect refused); the pack-gen child stays net-denied under the unchanged exec_child_filter (WG-2 binary-safe `run_launcher_capture_bytes` + `git pack-objects`); remote/refspec from TRUSTED intent; --force/--force-with-lease/:delete/+refspec hard-denied by construction; broker-env-only credential (Basic x-access-token) scrubbed from value-store/audit/logs; opaque non-minting audit; ALWAYS confirm-gated (no auto-dispatch arm — clean Allowed → synthetic BlockedPendingConfirmation with a MAC'd frozen-new-oid pending row) + WG-7 anti-TOCTOU freeze + WG-8 taint-provenance renderer + P33/P34 precheck-before-burn; HYG-01 zero-new-crate re-asserted — completed 2026-07-18 (compose-verify 668/0 on real Linux incl. leg_c real delivery to the mock git-receive-pack + leg_d force/delete refused + leg_e redirect refused; fresh Fable-5 adversarial trace APPROVE, 0 security defects across 8 surfaces)
- [x] **Phase 45: Thin CLI/SDK + Read-Only Audit-DAG Viewer** (4/4 plans) — SDK-01: a `caprun run <intent> <workspace> [--policy <path>]` verb binding the trusted policy at session creation (POLICY-03 enforcement point) + surfacing the blocked effect_id + `caprun review` pointer on an I2 Block, with the M7 anti-laundering fix (file-derived `--seed-from-file` content minted TAINTED via the broker-side `mint_from_read` site, operator literals stay trusted, provenance threaded through ProvideIntent — no new mint site). U1: a read-only `caprun audit <session>` viewer rendering events/decisions + verify_chain, using a load-ONLY fail-closed key (refuses absent key + `:memory:`, F1 containment, opens read-only), neutralizing every displayed literal via the shared `brokerd::display::neutralize_control_chars` (hardened this phase to also escape the Trojan-Source BiDi/zero-width spoof class per the adversarial trace). Existing confirm/deny/grant/review verbs unchanged — completed 2026-07-18 (compose-verify 691/0 on real Linux incl. the genuine end-to-end run→Block→review→audit loop; fresh Fable-5 adversarial trace APPROVE, M7 + viewer fail-closed both sound)
- [x] **Phase 46: Composed Live Proof (v1.9 DONE)** (4/4 plans) — the full authorized-write loop (process.exec → fs edit → git.commit → git.push[confirm-release] → github.pr → http.request.write POST) composed over ONE shared audit.db through the REAL broker arms, INSPECTED via a genuine `caprun audit` subprocess + a genuine `caprun run` Block leg, genuine non-stapled taint + verify_chain true per-session; 5 independently-attributable negative legs (tainted push refspec I2-Blocks, tainted POST body I2-Blocks, a distinct policy-deny [`code()=="policy_deny"`] while the I2 legs run a policy-PERMITTED sink, a destination-pin negative [redirect refused], a non-vacuous credential-absence [value-store/audit on the clean push + broker-log on the error path]); a new mock `POST /ingest`→201 endpoint; framing honesty machine-checked (no `caprun run`-drove-the-whole-chain overclaim). git.push safety-valve NOT triggered (SHIPPED Phase 44) — completed 2026-07-18 (independent compose-verify 696/0 on real Linux, LIVE-05 success chain + all 5 LIVE-06 legs RAN & PASSED, no v1.0–v1.8 regression; fresh Fable-5 adversarial trace APPROVE — proof genuine)

**v1.9 DONE gate cleared:** the full authorized-write loop is real and inspected on real Linux — an independent compose-verify run scored 696/0 (LIVE-05 success chain: `process.exec` → fs edit → `git.commit` → `git.push` [confirm-release] → `github.pr` → `http.request.write` POST over ONE shared audit.db through the REAL broker arms, driven + inspected via a genuine `caprun run` Block leg and a genuine `caprun audit` subprocess, `verify_chain` true per session) plus all 5 independently-attributable LIVE-06 negative legs (tainted push refspec I2-Block, tainted POST body I2-Block, a distinct `policy_deny` while the I2 legs run a policy-PERMITTED sink, a destination-pin redirect refusal, and a non-vacuous credential-absence check). **`git.push` SHIPPED — it did NOT defer a 3rd time** (broker-performed smart-HTTP transfer, IP frozen across both requests, force/delete hard-denied by construction, always confirm-gated, ZERO new crates). Policy narrows WHICH sinks/args are callable and can NEVER override I2 (I2 stays hardcoded in the Rust TCB, POLICY-02 by construction). All 13 v1.9 requirements Complete; every phase cleared a fresh non-self orchestrator-owned Fable-5 adversarial code-trace (APPROVE), which caught a real defect at every TCB phase; no v1.0–v1.8 regression. Gate progression: 42=535/0, 43=584/0, 44=668/0, 45=691/0, 46=696/0. Deferred (non-blocking, in STATE): git.push 10MB pack-cap (fails closed) + leg-5b optional scrub-branch hardening.

</details>

<details open>
<summary>🚧 v1.10 — Multi-step Safe Coding Agent Loop (Phases 47-52) — IN PROGRESS</summary>

**Milestone goal:** A design partner can drive the full Safe Coding Agent path (edit → test → commit → push → open PR) as **one Session via the CLI** — not a hybrid in-crate composition — with I2, per-session policy, and confirm/deny intact, and a genuine audit chain end-to-end. Closes the v1.9 LIVE-05 hybrid honesty gap. Deterministic multi-step first; LLM multi-step deferred. Zero new crates default. Design-gate + fresh non-self adversarial code-trace before any multi-step TCB change.

- [ ] **Phase 47: Multi-step Plan Stream Design Gate** - DESIGN doc + fresh adversarial code-trace locks stream shape, handle bag, Block-and-Hold, trusted-intent success path, and zero-new-crate hygiene before any multi-step TCB code
- [ ] **Phase 48: Plan-Stream Substrate** - Worker sequential multi-submit loop + opaque `output_value_id` handle bag on one Session with chain-head continuity
- [x] **Phase 49: Deterministic Multi-step Coding Planner** - Scripted coding planner emits the edit→test→commit→push→PR node sequence over shipped sinks with trusted-intent args only (completed 2026-07-29)
- [x] **Phase 50: CLI Multi-node Driver & Mid-loop Confirm Continuity** - `caprun run` drives the multi-node coding chain with honest stop semantics and same-Session Block-and-Hold confirm (completed 2026-07-29)
- [x] **Phase 51: Non-hybrid LIVE Proof (v1.10 DONE)** - CLI-driven success path + mid-loop I2 Block proven on real Linux with `verify_chain` true (no hybrid DONE claim) (completed 2026-08-09)
- [ ] **Phase 52: Minimal Linux Packaging** - Documented design-partner install path co-locating the three sibling binaries + env/credential checklist

## Phase Details

### Phase 47: Multi-step Plan Stream Design Gate

**Goal**: Multi-step orchestration mechanisms are pinned in a reviewed DESIGN doc and cleared by a fresh non-self adversarial code-trace before any multi-step TCB code lands
**Depends on**: Nothing (first v1.10 phase; builds on shipped v1.9 substrate)
**Requirements**: DESIGN-19, DESIGN-20, HYG-02
**Success Criteria** (what must be TRUE):

  1. `planning-docs/DESIGN-multi-step-plan-stream.md` exists and pins plan-stream shape (additive multi-node API on the existing Planner seam — not batch DAG authorize, not `EffectRequest`), worker sequential submit + handle bag (opaque ValueIds only), mid-loop Block-and-Hold confirm continuity (same Session, same policy bind, same audit chain), I1×coding-loop bounds (trusted-intent success path; no weakening CommitIrreversible Draft denies), instruction vs value channel disjointness, and mid-stream deny/abort semantics
  2. A fresh, non-self, orchestrator-owned adversarial code-trace (NOT a gsd-executor) clears the DESIGN with APPROVE before any multi-step TCB change in `crates/{executor,brokerd,sandbox,runtime-core}` or the worker submit/confirm-hold path in `cli/caprun`
  3. The DESIGN re-asserts HYG-02 / Gate discipline: zero new crates unless design-gate-justified (default: **zero**); no `EffectRequest` token under `crates/`; Gate 3 mint-site list unchanged or explicitly amended; `check-invariants.sh` remains the architectural gate; compose-verify remains the authoritative Linux gate
  4. Carry-forward invariants are locked in writing: ProvideIntent-once, P33/P34 precheck-before-burn, POLICY-02 non-bypass of I2; the adversarial trace re-runs if stream shape, confirm-hold, or trusted-arg mint path changes mid-implementation

**Plans:** 2 plans

Plans:
**Wave 1**

- [x] 47-01-PLAN.md — Author DESIGN-multi-step-plan-stream.md (DESIGN-19 + HYG-02 pins; docs only)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 47-02-PLAN.md — Fresh non-self adversarial code-trace + fold + DESIGN-GATE-RECORD-v1.10.md CLEARED (DESIGN-20)

### Phase 48: Plan-Stream Substrate

**Goal**: In one Session on one worker connection, the runtime can evaluate and submit N sequential plan nodes with genuine handle-bag continuity and an unbroken audit chain
**Depends on**: Phase 47
**Requirements**: STREAM-01, STREAM-02
**Success Criteria** (what must be TRUE):

  1. A worker can submit N sequential `SubmitPlanNode` calls in one Session on one connection; each node is independently I2-evaluated; policy remains the pre-I2 narrowing gate; no batch-authorize shortcut
  2. Every decision/event for the multi-node run lands on the same audit DAG with `verify_chain` true for the Session (chain-head continuity across nodes)
  3. Intermediate sink outputs exposed as `output_value_id` (e.g. `mint_from_exec`) are carried only as opaque ValueIds in a worker-side handle bag, retaining genuine taint/provenance
  4. The planner may only place handles into later nodes — never literals, never re-mint via mid-stream ProvideIntent; ProvideIntent remains exactly once before RequestFd for the Session (M7 anti-laundering preserved)
  5. No new mint sites are introduced (Gate 3 unchanged or explicitly amended in the DESIGN); `check-invariants.sh` green

**Plans:** 2 plans

Plans:
**Wave 1**

- [x] 48-01-PLAN.md — Tracer: plan_next + worker sequential loop + opaque bag + broker multi-submit verify_chain

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 48-02-PLAN.md — Expansion: deny-abort + Block no re-submit + Linux taint-via-bag + F-01 docs + COVERAGE

### Phase 49: Deterministic Multi-step Coding Planner

**Goal**: A deterministic multi-step coding planner produces a multi-node plan over shipped sinks for the Safe Coding Agent workflow without an LLM tool-use loop
**Depends on**: Phase 48
**Requirements**: CODE-01, CODE-02
**Success Criteria** (what must be TRUE):

  1. A deterministic multi-step coding planner (new `CaprunIntent` coding variant or equivalent) produces a multi-node plan covering at least: filesystem edit → `process.exec` (tests) → `git.commit` → `git.push` → `github.pr`
  2. Success-path plan nodes use trusted-intent operator args only (paths, commands, messages, remotes/refspecs from CLI/intent at session start) — no multi-file untrusted RequestFd before irreversible sinks on the happy path
  3. Email/file single-node planners remain green (no regression to existing intents)
  4. The recipe does not launder untrusted observations into trusted args; mid-loop I2 proof routing (tainted handle into a sensitive sink arg) is expressible for LIVE-08 without weakening success-path discipline

**Plans:** 2/2 plans complete

Plans:
**Wave 1**

- [x] 49-01-PLAN.md — Tracer: SafeCodingWorkflow + ProvideIntent multi-mint + bag seed + plan_next 5-node sequence + core unit tests (CODE-01/02)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 49-02-PLAN.md — Expansion: LIVE-08 expressibility, anti-launder, RequestFd hygiene, COVERAGE + validation Wave 0

### Phase 50: CLI Multi-node Driver & Mid-loop Confirm Continuity

**Goal**: A design partner can drive the full multi-node coding chain from the real CLI with honest stop semantics, and mid-stream Block-and-Hold keeps the same Session across confirm/deny
**Depends on**: Phase 49
**Requirements**: CLI-01, CLI-02, CONFIRM-01
**Success Criteria** (what must be TRUE):

  1. `caprun run` (or an explicitly documented sibling verb) accepts a coding multi-step intent + workspace + trusted `--policy`, binds policy at session creation (POLICY-03), and drives the full multi-node coding chain end-to-end
  2. Existing Block → `review`/`confirm`/`deny`/`grant` surfaces are preserved and pointed at from the driver; silent continue-past-Block is forbidden
  3. Stream stop semantics are honest and machine-checkable: on I2 Block → stop (or Block-and-Hold), surface `effect_id` + review pointer; on `policy_deny` → distinct outcome; on Deny → abort remaining nodes; on full success → clear success exit; exit codes distinguish success vs blocked vs denied/aborted
  4. When a mid-stream node returns `BlockedPendingConfirmation` (e.g. always-confirm `git.push`, or I2 Block released by confirm), the multi-node run holds the same Session (Block-and-Hold): worker stays connected or has a designed same-Session resume that does not re-open ProvideIntent, re-bind policy, or mint new trusted values
  5. Human confirm/deny acts on the durable pending row; remaining nodes continue only after Allowed release (or abort on deny); no dual-Session "stitch the chain later" as the product path; no session-wide confirm waiver

**Plans:** 2/2 plans complete

Plans:
**Wave 1**

- [x] 50-01-PLAN.md — Tracer: stream_hold protocol + worker Block-and-Hold PROCEED/ABORT + exit taxonomy + HoldContinue/HoldAbort tests (CLI-02, CONFIRM-01)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 50-02-PLAN.md — Coding argv driver + mid-loop confirm orchestration + grant pointers + exit codes + coding_cli/COVERAGE/validation (CLI-01, CLI-02, CONFIRM-01)

### Phase 51: Non-hybrid LIVE Proof (v1.10 DONE)

**Goal**: On real Linux, the multi-step coding path is proven end-to-end as a CLI-driven one-Session run (success + mid-loop I2 Block), closing the v1.9 hybrid honesty gap
**Depends on**: Phase 50
**Requirements**: LIVE-07, LIVE-08
**Success Criteria** (what must be TRUE):

  1. On real Linux, a design partner can run the multi-step coding intent via the real CLI (`caprun run` or documented equivalent) under a bound policy: edit → test → commit → push (confirm-release) → open PR (mock GitHub allowed for CI) — **one Session**, inspected via real `caprun audit`, with `verify_chain` true
  2. The SUCCESS claim is **not** a hybrid in-crate composition — the multi-node chain is CLI-driven (closes v1.9 LIVE-05 honesty gap); framing machine-checked against hybrid overclaim
  3. In the same proof family, a mid-loop I2 Block is independently attributable: a genuinely tainted handle (non-stapled provenance root on a real read/exec event) occupies a sensitive sink arg (e.g. PR body and/or push refspec) under a policy-permitted sink; executor Blocks; `policy_deny` is not what fired; no effect of that node; chain remains `verify_chain` true
  4. Full-workspace regression green on real Linux via the authoritative compose-verify gate; no v1.0–v1.9 regression; `check-invariants.sh` green

**Plans**: 9 plans

Plans:
**Wave 1**

- [x] 51-01-PLAN.md — Wave 1 tracer: LIVE-07 harness + framing + CLI multi-node SUCCESS under compose-verify

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 51-02-PLAN.md — Wave 2 expansion: LIVE-08 proof-planner path + full compose-verify regression

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 51-03-PLAN.md — Close review findings WR-01/02/03: proof-selector containment, hermetic subprocesses, exact durable `github.pr/body` provenance

**Waves 5–8** *(gap closure — added 2026-08-04 after 51-04 executed on real Linux and failed rc=101 three times; see `51-BLOCKING-DEFECTS.md`)*

- [x] 51-05-PLAN.md — Wave 5: host-runnable RED regressions for D1 and D1+D2, plus D3/D4 harness repair (no Docker, no cfg-linux gate)
- [x] 51-06-PLAN.md — Wave 6: `DESIGN-audit-append-concurrency.md` design gate authorising the TCB fix
- [x] 51-07-PLAN.md — Wave 7: coordinated D1+D2 append-at-head fix as ONE atomic commit
- [x] 51-08-PLAN.md — Wave 8: fresh non-self ORCHESTRATOR-owned adversarial code-trace of the fix diff

**Wave 9** *(blocked on Wave 8 — proof-oracle correction followed by the authoritative Docker proof)*

- [x] 51-09-PLAN.md — Wave 9: order-independent LIVE-08 attribution oracle bound to the durable `read_event_id`
- [x] 51-04-PLAN.md — Wave 9: execute and retain the real-Linux scoped + full-workspace compose proof; reconcile evidence and status ledgers

> **Phase 51 verified complete on 2026-08-09.** The retained real-Linux scoped run
> passed LIVE-07, LIVE-08, the order-independence regression, and the guard (4/4);
> the full composed workspace gate passed; all three evidence hashes matched; and the
> independent evidence review plus human checkpoint approved the record. The disclosed
> `OPENAI_API_KEY`-gated skips are not part of the LIVE-07/LIVE-08 acceptance claim.

### Phase 51.1: Grant/Audit Atomicity (CR-01) (INSERTED)

**Goal**: A `github.pr` capability can never become durable without its `github_grant_authorized` audit event — the grant row and its event commit or roll back together
**Depends on**: Phase 51 (CR-01 was found by Phase 51's code review and independent verification)
**Requirements**: CR-01
**Success Criteria** (what must be TRUE):

  1. `record_github_grant` (`crates/brokerd/src/audit.rs:542-564`) performs the `session_grants` insert and its conditional `append_event` inside ONE `TransactionBehavior::Immediate` transaction, committing only after the append succeeds — `append_event` appends at the locked durable head without opening a nested transaction
  2. A fault-injection regression proves an `append_event` failure rolls back `session_grants`, and that a retry then yields EXACTLY one grant row and EXACTLY one `github_grant_authorized` event (the uncovered error path named in `51-VERIFICATION.md`)
  3. `has_github_grant` cannot observe a grant whose authorization event is absent from the hash chain
  4. HARDEN-02 tail-truncation detection is intact — the verifier and MAC functions are byte-unchanged, and no added/removed line under `crates/brokerd/src/` touches `read_event_id` or `provenance_chain` (the I2/M7 provenance graph stays un-unified with the causal chain)
  5. A fresh non-self adversarial code-trace of the landed diff returns 0 BLOCKER and 0 unresolved MAJOR before the change is folded
  6. `check-invariants.sh` Gates 1–6 stay green; no `Cargo.toml`/`Cargo.lock` change, no new `mint_from_*` site, no raw effect path

**Notes**: No new design gate required — `DESIGN-GATE-RECORD-v1.10.md` already CLEARED the audit append-at-head concurrency gate, and that trace explicitly examined `record_github_grant` (obligations 1 and 5, confirming it continues through the choke point at `audit.rs:564`). Secondary non-blocking cleanups in scope: the stale 19-vs-45 append-site comment (`audit.rs:1033-1037`, AR-05) and the overstated atomicity comment (`server.rs:1044-1049`, AR-06). Fix and its regression are host-runnable (SQLite only, no sinks, no Docker); the full-workspace regression batches onto the same EC2 run as Phase 52.

**Plans:** 3/3 plans complete

Plans:
**Wave 1**

- [x] 51.1-01-PLAN.md — Tracer: RED fault-injection oracle, then the one-`IMMEDIATE`-transaction fix to `record_github_grant`, plus the AR-05/AR-06 comment cleanups (criteria 1, 2, 3)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 51.1-02-PLAN.md — Mechanical non-regression Gates A/A2/B/C against the pre-fix reference, plus the scoped Docker-free host regression and the unrun-regression disclosure (criteria 4, 6)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 51.1-03-PLAN.md — Adversarial trace brief, then the blocking fresh non-self external code-trace of the landed diff (criterion 5)

### Phase 52: Minimal Linux Packaging

**Goal**: A design partner has a documented minimal Linux install path that co-locates the three sibling binaries and lists required env/credentials
**Depends on**: Phase 51 (may draft docs earlier; ships after LIVE so install path matches proven binary layout)
**Requirements**: PKG-01
**Success Criteria** (what must be TRUE):

  1. A documented release build path co-locates `caprun`, `caprun-worker`, and `caprun-exec-launcher` (sibling `current_exe()` layout) — `cargo install --path cli/caprun` alone is documented as **not** sufficient
  2. An env/credential checklist covers `CAPRUN_*`, policy file, and GitHub grant token as applicable
  3. A thin install script (e.g. `scripts/install-linux.sh`) is acceptable; not cargo-dist/deb/snap productization

**Plans:** 3 plans

Plans:
**Wave 1**

- [ ] 52-01-PLAN.md — Tracer: `scripts/install-linux.sh` end-to-end (build → stage → co-locate three siblings → verify) plus the GETTING-STARTED install walkthrough, manual equivalent, and `cargo install` insufficiency warning (criteria 1, 3)
- [ ] 52-02-PLAN.md — Replace the stale CONFIGURATION surface: real verb/flag CLI, minimal policy example, and the three-tier operator env/credential checklist (criterion 2)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 52-03-PLAN.md — Narrow README layout/pointer update, cross-document consistency + phase regression gate, not-a-security-proof boundary statement, and the human sign-off (criteria 1, 2, 3)

</details>

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
| 43. `http.request` WRITE (POST/PUT) Egress | v1.9 | 4/4 | Complete    | 2026-07-18 |
| 44. `git.push` — Broker-Performed Destination-Pinned Egress | v1.9 | 5/5 | Complete    | 2026-07-18 |
| 45. Thin CLI/SDK + Read-Only Audit-DAG Viewer | v1.9 | 4/4 | Complete    | 2026-07-18 |
| 46. Composed Live Proof (v1.9 DONE) | v1.9 | 4/4 | Complete    | 2026-07-18 |
| 47. Multi-step Plan Stream Design Gate | v1.10 | 1/2 | In Progress|  |
| 48. Plan-Stream Substrate | v1.10 | 2/2 | In Progress|  |
| 49. Deterministic Multi-step Coding Planner | v1.10 | 2/2 | Complete    | 2026-07-29 |
| 50. CLI Multi-node Driver & Mid-loop Confirm Continuity | v1.10 | 2/2 | Complete   | 2026-07-29 |
| 51. Non-hybrid LIVE Proof (v1.10 DONE) | v1.10 | 9/9 | Complete    | 2026-08-09 |
| 52. Minimal Linux Packaging | v1.10 | 0/? | Not started | - |
