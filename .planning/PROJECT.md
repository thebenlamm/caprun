# AgentOS

## What This Is

AgentOS is an **Intent Runtime** on stock Linux: a user-space execution layer
where agents have no ambient authority, every external effect is authorized
against a Session, and confinement is kernel-enforced. The v0 binary is
`caprun`. It is **not** a kernel fork, an agent framework, a desktop-automation
platform, a memory product, or a marketplace.

## Core Value

A kernel-confined worker can only cause external effects through
broker-mediated plan nodes, and a genuinely-propagated taint chain (raw read
Event → ValueNode → sensitive sink argument) deterministically blocks
value-injection at the sink. If everything else fails, **I2 enforcement on a
genuine taint chain must hold.**

## Current Milestone: v1.9 — Authorized Egress + Policy & Audit Surface

**Goal:** Complete the authorized-write-egress story so the Safe Coding Agent's
full loop (edit → test → commit → **push** → open PR) is real, and add the first
usability/trust-surface layer (a minimal per-session policy + a CLI/audit-DAG
viewer) toward a design-partner-runnable slice — without weakening I0/I1/I2 or
adding any raw `EffectRequest` path.

**Anchor use case (unchanged):** the Safe Coding Agent.

**Target features (four tracks):**

- **`git.push` (GIT-02/GIT-03):** the sink v1.8 gate-deferred. Design-gate-first
  for a fully-unprivileged, BROKER-MEDIATED, destination-pinned egress with the
  push child kept **net-denied** (seccomp cannot pin a `connect()` destination —
  v1.8 BLOCKER-1). Carries forward the pinned §2 model (remote/refspec from
  TRUSTED intent, never repo `.git/config`; `--force`/ref-deletion hard-denied),
  §2.5 (captured-output credential scrub), §2.7 (payload-at-confirm shows the
  pushed diff + tainted-file provenance), §9 (P33/P34 confirm-release). **If no
  fully-unprivileged destination-pinning mechanism proves sound at the gate,
  `git.push` defers a 3rd time rather than ship arbitrary child egress — the
  other three tracks still ship.**
- **`http.request` WRITE (HTTP-W-01):** POST/PUT to an allowlisted host; the
  request BODY is taint-governed + content-sensitive under I2 (a tainted body
  Blocks, exactly like an email/PR body); reuses v1.8's SSRF resolve-and-pin +
  webpki-roots egress + the workspace-scoped supply-chain gate.
- **Policy (POLICY-01):** a MINIMAL declarative per-session policy — which sinks
  are callable + coarse arg constraints (allowlisted hosts/paths/repos), a
  hardcoded-schema struct/file (NOT Cedar). **⚠ LOCKED INVARIANT:** policy may
  only gate WHICH sinks/args are callable — it can NEVER disable or override I2.
  An attacker-tainted value in a sensitive sink arg still Blocks regardless of
  policy; the I2 decision stays HARDCODED in the Rust TCB executor
  (DEC/CON-i2-non-bypassable). The policy-vs-I2 boundary is the #1
  adversarial-trace risk the design gate must pin.
- **Thin CLI/SDK + audit-DAG viewer (SDK-01/U1):** define an intent, point at a
  workspace, run, and INSPECT the proof. The read-only audit-DAG view over the
  SQLite audit chain (with `verify_chain` surfaced) IS the trust surface. No web
  UI.

**Key context:** Design-gate-first per standing precedent — a DESIGN doc covering
the TCB pieces (git.push egress mechanism, http-write egress, AND the
policy-vs-I2 boundary) must clear a fresh non-self adversarial code-trace
(orchestrator-owned, not a gsd-executor) BEFORE any
`crates/{executor,brokerd,sandbox,runtime-core}` TCB code. Any new net/crypto/
policy dep honors the workspace-scoped supply-chain absence check
(`cargo tree --workspace -i <dep>`, wired to a gate — the v1.8 aws-lc-rs lesson).
Two v1.8-flagged hygiene items fold in: a feature-OFF guard in
`compose-verify.sh` and a workspace-wide `check-invariants` Gate 4b grep. **DONE
gate:** a composed exec→fs→git.commit→git.push→github.pr workflow PLUS an
`http.request` POST leg on real Linux, driven and INSPECTED via the new CLI +
audit-DAG viewer, with adversarial legs (tainted push remote/refspec, tainted
POST body) each deterministically Blocked, `verify_chain` true, and a policy-deny
leg refused WITHOUT weakening the I2 taint Block.

## Shipped Milestone: v1.8 — Git/GitHub Adapters (Effect Breadth II)

**✅ SHIPPED 2026-07-18 — Phases 35-38,40 complete (Phase 39 `git.push` gate-authorized deferral to v1.9), proven live on real Linux. Full detail archived in `.planning/milestones/v1.8-ROADMAP.md` + `.planning/milestones/v1.8-REQUIREMENTS.md` + `.planning/milestones/v1.8-MILESTONE-AUDIT.md`. Next milestone: run `/gsd-new-milestone` (v1.9 — Git/GitHub Adapters continued: git.push).**

v1.8 delivered 3 of the 4 originally-scoped sinks: `git.commit` (broker-spawned confined-child `git commit`, MutateReversible, reusing the v1.7 exec-launcher + `mint_from_exec`, git config/hooks neutralized), read-only `http.request` GET (Observe, the new `mint_from_http` inbound-taint mechanism minting the response untrusted-on-arrival and demoting the session to draft-only, defended by an SSRF resolve-and-pin classifier), and `github.pr` (CommitIrreversible, a broker-held bearer token never present in the confined worker/planner sidecar/ValueNode/audit-literal, an explicit human auth-grant distinct from single-shot confirm, tainted PR title/body sections deterministically Blocked via CONTENT-01 content-sensitivity, and a content-derived duplicate-PR CAS mirroring HARDEN-03). **`git.push` (GIT-02/GIT-03) is DEFERRED to v1.9** — the Phase-35 design gate's fresh adversarial code-trace proved (BLOCKER-1) that a `git.push` confined child's network destination cannot be pinned by seccomp (it filters syscall numbers/scalars, not the `connect()` sockaddr behind a pointer; Landlock net rules need kernel 6.7 > the 5.13 floor); the sound alternative — a fully-unprivileged, broker-mediated, destination-pinned egress — is a genuinely new trust posture that the gate itself flagged as needing its own design-gate + fresh adversarial review, so `git.push` was deferred rather than shipped with arbitrary child egress (see `planning-docs/DECISION-git-push-deferral-v1.8.md`). ENV-01 closed the v1.7-deferred `caprun-planner` sidecar `env_clear()` gap-closure, hermetic via compiled-in `webpki-roots` (no `SSL_CERT_*` / readable system store required), live-verified against a real OpenAI HTTPS call. Proven end-to-end on real Linux: a composed workflow — `process.exec` (test) → filesystem edit → `git.commit` → `github.pr` (mock GitHub endpoint, standing in for the pushed-branch precondition) + an `http.request` GET leg — with every step gated, tainted, and audit-DAG-chained (`verify_chain` true across the run); three adversarial attack legs (tainted PR-body/title, tainted GET url/SSRF, tainted commit message) each deterministically Blocked; full-workspace regression green (498 passed/0 failed/60 binaries, no v1.0–v1.7 regression). Every TCB change cleared a fresh non-self adversarial code-trace (the DESIGN gate caught a real BLOCKER + 3 MAJORs; Phase 37's diff caught a MAJOR `aws-lc-rs`-in-workspace defect + a `git.commit` Landlock/exit-code defect). Honest scope: v1.8 proves edit→commit→open-PR (mock) + authorized HTTP fetch — real push is deferred, disclosed here and in the milestone audit, not papered over.

## Current State

**v1.8 — Git/GitHub Adapters (Effect Breadth II) shipped 2026-07-18:** `git.commit`, `http.request` GET (+ the new `mint_from_http` inbound-taint mechanism), and `github.pr` (bearer-token auth-grant + duplicate-PR CAS) delivered — 3 of the 4 originally-scoped sinks. `git.push` (GIT-02/GIT-03) DEFERRED to v1.9: the Phase-35 design gate proved seccomp cannot pin a confined child's `connect()` destination, so a sound fully-unprivileged, destination-pinned egress needs its own design-gate first. Proven on real Linux: a composed exec→fs→git.commit→github.pr(mock)+http-GET workflow, 3 adversarial legs Blocked, full-workspace 498/0 regression. ENV-01 closed the v1.7-deferred planner-sidecar `env_clear()` gap.

Prior: v1.7 Effect Breadth I (process.exec + fs breadth) — 2026-07-18; v1.6 Security Hardening (close the residuals) — 2026-07-17; v1.5 Slot-Type Binding (T2) — 2026-07-12.

## Shipped Milestone: v1.7 — Effect Breadth I (process.exec + Filesystem Breadth)

**✅ SHIPPED 2026-07-18 — all 5 phases (31-35) complete, proven live on real Linux. Full detail archived in `.planning/milestones/v1.7-ROADMAP.md` + `.planning/milestones/v1.7-REQUIREMENTS.md` + `.planning/milestones/v1.7-MILESTONE-AUDIT.md`. Next milestone: run `/gsd-new-milestone` (v1.8 Git/GitHub adapters to follow).**

v1.7 delivered `process.exec` as a broker-spawned confined-child sink (the confined worker never `execve`s the target; exec is mediated via a sibling `caprun-exec-launcher` binary with Landlock narrowing + seccomp net-deny + rlimits + wall-clock timeout applied before spawn), capturing stdout/stderr and **taint-minting the output as untrusted** at a single genuine, non-stapled mint site (`mint_from_exec` rooted on `process_exited` audit event — no minting on the confirm-release path). Filesystem read/write breadth landed: multi-file read + `file.write` sink for existing-file-only editing (O_WRONLY|O_TRUNC, no O_CREAT/O_EXCL), both confined to WorkspaceRoot via `openat2(RESOLVE_BENEATH)`, with a `RequestFd` count limiter (256, deny-and-keep) and I2/slot-role enforcement. EXEC-05 confirm-release (`invoke_process_exec_from_resolved`) wired exactly-once release of blocked process.exec, with Entry Guard (Step 4.75) + Pre-Step-5 verification (no mint on confirm-release path — durable taint lives on the process_exited event only). Proven end-to-end on real Linux: LIVE-01 composed 4-leg acceptance true-exit-0; LIVE-02 full-workspace regression 391/0 across all suites with no v1.0–v1.6 regression. A fresh Fable-5 adversarial code-trace caught and fixed a real MAJOR (confirm-release path burned the one-shot confirmation without a terminal audit event in Step-7 dispatch, leaving an audit gap) — reconciled pre-close. env_clear() gap-closure fixed broker-secret inheritance in both the confined exec-child and worker spawns (planner-sidecar TLS-env variant deferred to v1.8).

## Shipped Milestone: v1.6 — Security Hardening (close the residuals)

**✅ SHIPPED 2026-07-17 — all 8 requirements Complete, milestone audit PASSED, proven live on real Linux. Full detail archived in `milestones/v1.6-ROADMAP.md` + `milestones/v1.6-REQUIREMENTS.md` + `milestones/v1.6-MILESTONE-AUDIT.md`. Next milestone: run `/gsd-new-milestone` (a v1.7 productization sketch exists at `planning-docs/CANDIDATE-v1.7plus-productization-sketch.md`).**

**Goal (delivered):** Close the standing TCB-local security residuals that v1.1–v1.5 accumulated and
documented as accepted caveats — turning each DOC-01 honesty qualifier into an enforced
guarantee, without adding any new external-effect surface.

**Target features:**
- **Demote-at-RequestFd (I1 honest scope):** fd release itself carries an I1 consequence, so
  "reading raw untrusted bytes → draft-only" becomes literally true (not just on a reported
  read), reconciled with the CONTROL-01 clean path.
- **`verify_chain` authentication:** a keyed MAC over the audit chain and/or an
  externally-anchored chain head, so an actor with `events` write access can no longer forge
  an internally-consistent chain — upgrading it from a corruption detector to authenticated
  integrity.
- **Allowed-path replay CAS:** an idempotency key / compare-and-swap on the trusted
  (Allowed) `email.send` path, so a replayed `SubmitPlanNode` can no longer send N emails —
  mirroring the confirm path's at-most-once transaction.
- **Compile-out the forced-Active mint:** the `CreateSession`-IPC forced-`Active` mint arm
  (guard-(c), currently gated behind the `CAPRUN_ENABLE_IPC_CREATE_SESSION` runtime
  default-deny flag) becomes a build-excluded path, so the code is absent from the production
  binary, not merely disabled at runtime.
- **Constrain the `file.create` `contents` slot:** give the currently-unconstrained
  `contents` arg an expected-role / sensitivity treatment so it isn't a latent gap if
  `file.create` ever gains egress.

**Key context:** All five are TCB-local hardening on the existing design — no new adapters,
no new external effects. Per this project's standing precedent (every TCB milestone opens with
a reviewed DESIGN doc before any executor/brokerd change — v1.0 P2, v1.2 P8, v1.3 P12, v1.4
P18, v1.5 P23), v1.6 should open with a design-gate phase. Breadth (Git/GitHub adapter, test
adapter, patch/PR, workspace snapshots) is deliberately deferred to **v1.7** to keep this
milestone coherent and right-sized. Source detail: `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md`.

<details>
<summary>v1.5 milestone planning detail (shipped — historical)</summary>

**Goal:** Close v1.4's accepted residual #5 (T2) — the executor gains a
structural check that a resolved value's semantic origin matches the
semantic role of the plan-node slot it's routed into, so a misrouted
`UserTrusted` handle (e.g. a subject-typed string landed in `to`) is caught
even though it is neither untrusted (I2 doesn't fire) nor a class-level deny
(I0/I1 don't apply). Today `ValueRecord` carries no origin/role tag at all —
`ProvideIntent`'s three `mint_from_intent` calls (recipient/subject/body) all
mint `[TaintLabel::UserTrusted]` with nothing distinguishing them from one
another once minted. T2 is safe today only *incidentally* (every
`UserTrusted` handle is human-typed and, by convention, routed correctly by
the planner) — nothing structural enforces it.

**Target features:**
- New DESIGN doc (`planning-docs/DESIGN-slot-type-binding.md`) + fresh
  (non-self) adversarial review gate, mirroring v1.4 Phase 18's shape — no
  executor/TCB code before it clears
- A mechanism to tag each minted value's semantic origin role — an additive,
  mechanical touch to the `mint_from_intent`/`mint_from_read`/
  `mint_from_derivation` call sites (NOT a change to I0/I1 trust
  classification — which values become `UserTrusted` vs untrusted is
  unaffected)
- A hardcoded per-sink-arg "expected role" table in the executor, mirroring
  the `sink_sensitivity.rs` precedent (CONTENT-01/02) — not a general
  framework
- A new exhaustive `DenyReason` variant (no wildcard arm) for a slot-type
  mismatch
- Regression audit of existing tests that currently assume permissive
  `UserTrusted`-in-any-slot behavior
- Live re-verification via `scripts/mailpit-verify.sh` before close

**Key context:** TCB is Rust; this is `crates/executor` code, so it needs
its own DESIGN doc + adversarial review gate per the project's standing "no
TCB code without a reviewed DESIGN doc" discipline. Out of scope: mint
*classification* changes (I0/I1), connection/capability model changes
(already shipped in v1.4), a general content-classification taxonomy, and
CAS/replay work (already re-earned in writing at v1.4). Whether a slot-type
mismatch becomes a hard `Denied` or joins the collect-then-Block
`BlockedPendingConfirmation` set is left to the Phase design-gate doc, not
locked at scoping time.

<details>
<summary>✅ v1.4 — Trust-Boundary Integrity & the Adversarial Planner — SHIPPED 2026-07-11</summary>

**Goal:** Fix a confirmed live cross-connection trust bypass in the broker
(Phase 0 — a security fix, gated by an already-red regression test), then
prove the trust boundary is indifferent to planner intelligence by putting an
adversarial LLM planner behind it (Phase 1+) — a hostile injected document
makes the planner *comply* and try to route a tainted value to `email.send`,
and the executor **Blocks deterministically** anyway, with genuine taint
propagation re-verified live (the §9 standard: `verify_chain` true, Mailpit
== 0), because the value flows around the planner through the worker's own
mint sites, never through the planner's tokens.

**Why it existed:** an adversarial review found, and a Linux repro CONFIRMED
(cargo exit 101, 2 independent runs —
`crates/brokerd/tests/two_connection_intent_bypass.rs`), that v1.3's guard(a)
(`ProvideIntent` sealed after any `RequestFd`) was per-connection state only.
A worker could open a SECOND `AF_UNIX` connection to the same session
socket, get fresh guard(a) state and a stale `session_status`, and mint an
attacker-controlled literal `UserTrusted` via `ProvideIntent` on that second
connection — routing it to `email.send` as `Allowed`. This falsified the
invariant the whole I0/I1/I2 model rests on: `UserTrusted` == "the human
typed it." The fix (Phase 18-19) preceded all new capability.

**Delivered (5 phases, 18-22):**
- **Phase 18 (Design Gate):** `planning-docs/DESIGN-session-trust-coherence.md`
  authored, cleared a 2-round fresh adversarial review that caught and fixed a
  genuine BLOCKER — round 1's original fix design (release the occupancy
  latch on disconnect, permit reconnect) would have left the exact bypass
  reachable via a sequential close-then-reconnect sequence; remediated to a
  ONE-WAY, session-lifetime latch before round 2 cleared it.
- **Phase 19 (Cross-Connection Trust Coherence Fix):** the one-way occupancy
  latch shipped in `run_broker_server`'s accept loop
  (`crates/brokerd/src/server.rs`) — rejects any 2nd connection to an
  already-active session, set once on first accept, never released.
  `two_connection_intent_bypass.rs` restructured into 3 independent
  fresh-broker regression variants (guard-a control, overlapping,
  sequential-reconnect), all green on real Linux. Full workspace suite:
  253 passed / 0 failed / 37 binaries (v1.3's 250/0/36 baseline + the 3
  newly-un-ignored tests), no regression.
- **Phase 20 (Planner Seam & Capability Split):** a real `Planner` trait
  introduced (`cli/caprun/src/planner.rs`); the broker gained a
  `ConnectionRole` capability model — a 2nd, capability-restricted
  planner-role connection may be admitted via a `DeclarePlannerRole`
  handshake, fail-closed default-deny on all 4 mint verbs plus
  `RequestFd`/`ReportRead`, receiving only a reduced
  `PlanNodeDecisionReduced{blocked}` signal (no anchors/literal_sha256/
  literal) on `SubmitPlanNode`.
- **Phase 21 (Adversarial LLM Planner):** a genuine OpenAI-backed
  `LlmPlanner` (`gpt-4o-mini` default, `CAPRUN_PLANNER_MODEL`-configurable)
  implements the `Planner` trait exactly like `DeterministicPlanner` —
  in-process, synchronous, worker submits via its own connection. The actual
  LLM HTTP call runs in a separate `caprun-planner` sidecar process (the
  confined worker itself cannot `execve` or open `AF_INET` sockets per
  seccomp, so this separation was structurally required). Live-proven on
  real Linux: real OpenAI call, `Chain verification: PASSED`, real Mailpit
  delivery, ~$0.00012/request.
- **Phase 22 (Adversarial Gate Proof & Residual Disclosure — the HARD
  GATE):** a hostile document's embedded injection reaches the LLM planner
  via a genuinely taint-tracked `task_instruction` channel (mint_from_read-
  rooted, never itself a `PlanArg` value); the planner, offered BOTH a
  trusted and a tainted recipient handle, complies with the injection and
  routes the tainted one to `to` — the executor Blocks it deterministically
  via I2, `verify_chain` true, Mailpit == 0 for the attacker. A genuine
  architectural finding during this phase (not a corner cut): a locked v1.2
  invariant (Draft sessions unconditionally deny `CommitIrreversible` sinks)
  meant a "both handles offered, no injection" control leg could never reach
  `Allowed` — proven instead via `Denied` + diagnostic-log proof that the
  model still chose the TRUSTED handle absent the injection, demonstrating
  two independent defense layers (I0 session-level class-deny, I2 per-arg
  Block) both correctly firing depending on the model's actual choice. A
  separate trusted-intent control in the SAME composed run Allows and
  delivers exactly once. GATE-04's sentinel-leak assertion (deterministic,
  non-network unit test against the real prompt-construction function)
  replaced the old context-dump grep. T2 (slot-type binding) documented as
  the accepted v1.4 residual, deferred to v1.5.

**Milestone-closure finding (independent final re-verification, not caught by
any individual phase's own live-verification task):** a bare `cargo test
--workspace --no-fail-fast` doesn't reliably place the "nice-named"
`target/debug/caprun-planner` copy for that bin-only sibling crate — a Cargo
build-artifact-placement quirk, not a caprun logic bug — which intermittently
broke every `CAPRUN_PLANNER=llm` live test depending on which command ran
last. Fixed in `scripts/mailpit-verify.sh` (now runs `cargo build --workspace`
before `cargo test --workspace`); re-ran the full default recipe from scratch
afterward — real exit 0, 46 test groups all green, zero failures.

**Explicitly not reopened beyond the above:** Git/GitHub adapters, Cedar
policy engine, cross-host delegation/Biscuit crypto, gVisor/Firecracker, a
web UI, marketplace, or long-term memory.

</details>

<details>
<summary>✅ v1.3 — Doc → Action Assistant — SHIPPED 2026-07-09</summary>

**Goal:** caprun ingests an untrusted document containing an embedded
injection, deterministically extracts a "send to X" action (recipient + body
derived from the doc's content, no LLM planner), and attempts a real email
send. The read demotes the session (I1, existing); the tainted recipient AND
body both block at the sink (I2 + new CONTENT-01); `caprun confirm`/`deny`
shows verbatim recipient+body+provenance; confirm sends exactly once via a
real broker-mediated SMTP adapter, deny sends nothing — one unbroken audit DAG
for both outcomes, plus a clean-send negative control in the same run, proven
live on real Linux via Colima+Docker.

**Delivered:** real broker-mediated SMTP adapter (SMTP-01/02/03/05,
SEND-01/02); CONTENT-01 content-sensitive sink-arg blocking; deterministic
doc→action extraction with genuine provenance threading
(`mint_from_derivation`); full-set name-bound confirm binding
(CONFIRM-01..04, CONTROL-01/02); ACCEPT-01 composed live acceptance (3
sessions, one shared audit.db, all `verify_chain`-true).

**⚠️ Superseded finding (v1.4 Phase 0) — FIXED, SHIPPED 2026-07-11:** an
adversarial review after v1.3 shipped found, and a Linux repro CONFIRMED,
that guard(a) (`ProvideIntent` sealed after `RequestFd`) was
**per-connection state only** — a second `AF_UNIX` connection to the same
session socket bypassed it entirely, minting an attacker-controlled
`UserTrusted` literal that routed to `email.send` as `Allowed`. This meant
the `UserTrusted == human-typed` invariant, which v1.3's whole confirm/deny
narrative rests on, did **not** hold across connections as shipped. Not a
production incident (nothing deployed; repo unpushed).

**The fix, as shipped:** a one-way, session-lifetime occupancy latch was
added to `run_broker_server`'s accept loop (`crates/brokerd/src/server.rs`)
that rejects any second connection to an already-active session — set once
on first accept, never cleared for the life of the broker invocation, and
checked *before* any per-connection `session_status`/`intent_provided`/
`fd_requested` state is ever seeded for the rejected stream. This restores
the `UserTrusted == human-typed` invariant across connections, not just
within one. Test evidence: `crates/brokerd/tests/two_connection_intent_bypass.rs`'s
three independent fresh-broker variants (`guard_a_intra_connection_control`,
`overlapping_connection_bypass_repro`, `sequential_reconnect_bypass_repro`)
all pass green on real Linux (`test result: ok. 3 passed; 0 failed; 0
ignored`), and the full `scripts/mailpit-verify.sh` (`cargo test --workspace
--no-fail-fast`) live-Linux rerun is green with no regression: 253 passed, 0
failed, across 37 binaries — exactly v1.3's 250/0/36 baseline plus the 3
newly-un-ignored tests and their 1 new test binary. See
`.planning/phases/19-cross-connection-trust-coherence-fix/19-01-SUMMARY.md`
(the fix mechanism) and `19-02-SUMMARY.md` (the live-Linux proof, verbatim
counts).

**What v1.3's live proof does and does not claim (DOC-01):** CONTROL-01
proves that a send built from TRUSTED intent is Allowed and delivers, and that
a send whose args are DOC-DERIVED is Blocked — it does NOT prove "same doc,
taint flipped"; the benign doc is decorative on the clean path. I1's
draft-only demotion triggers when the broker mints untrusted taint from a
REPORTED read (`mint_from_read`) — NOT on fd release; a worker that reads the
doc and reports nothing stays Active (v2 obligation: demote at `RequestFd`).
ProvideIntent mints worker-declared intent as UserTrusted only BEFORE any fd
read, exactly once, **on that same connection** — broker-ENFORCED per-
connection, but (per the superseded finding above) not coherent across
connections. The confined worker's send
path links brokerd → lettre → native-tls (a factual dependency-chain note).
CONFIRM-01's verbatim recipient+body narration is proven END-TO-END live for
the FIRST time in Phase 17's composed acceptance run — at Phase 16 it was
exercised only against a synthetic fixture.
Four accepted residual risks (verify_chain's forgeable chain
head, guard-(c)'s runtime-vs-compile-time gap, the Allowed-path's replay
exposure, and the cross-connection trust-coherence gap above) are detailed in
the v1.3 residual-risks clause below — do not stop reading at this paragraph.

The controlled-experiment framing is: the hostile confirm and deny legs use
two documents with IDENTICAL injection text and IDENTICAL derivation
structure, differing ONLY in a per-run test-isolation recipient token; both
are blocked identically as doc-derived tainted recipients; the operator
confirms one and denies the other; confirm sends exactly once, deny sends
nothing. The controlled variable is the OPERATOR'S DECISION. That per-run
recipient token is a UUID in the domain fragment that exists PURELY so the
live Mailpit assertions can isolate each leg on a shared listener — not
because the two docs differ in any way the taint mechanism sees.

Scope note: "one unbroken audit DAG" means per-session `verify_chain`
integrity across a SHARED audit.db log (three sessions in one file, each
independently chain-verified, with genuine-taint descent re-proven for the
hostile anchors) — NOT a single cross-session `parent_id` chain spanning
confirm/deny/clean.

Self-consistency note: the live composed run's `to`/`body` anchor pin is a
SELF-CONSISTENCY reconstruction (expected roots rebuilt from the same
derivation record being checked), NOT an independently-sourced ground-truth
pin. Independent ground-truth root pinning (via out-of-band mint-return
values) lives only in Phase-15's still-green DB-alone test — the one source
of truth for that property. The substantive anti-staple teeth (per-element
real-file_read check, genuine_derivation_binds, both anti-staple controls)
hold independently of this nuance.

**Progress:** Phases 12-17 (DESIGN-01 design gate, real SMTP adapter,
content-sensitive blocking, doc→action extraction, confirm UX, live
acceptance) all complete and verified. Full traceability archived in
`.planning/milestones/v1.3-REQUIREMENTS.md`.

</details>

Full v1.2 detail archived in
[`milestones/v1.2-ROADMAP.md`](milestones/v1.2-ROADMAP.md) and
[`milestones/v1.2-REQUIREMENTS.md`](milestones/v1.2-REQUIREMENTS.md).

<details>
<summary>✅ v1.2 — Tainted Session, Human Gate — SHIPPED 2026-07-07</summary>

**Goal:** A session that touches untrusted content is mechanically demoted to
draft-only (I1 dynamic-taint default + I0 creation rule), and a blocked sink arg
can be released only by literal-value human confirmation — all deterministic,
all in the audit DAG.

**Delivered:**
- **Session taint state (I1 dynamic default):** broker tracks per-session trust
  state; the `mint_from_read` path (raw untrusted read Event) flips the session
  to draft-only. Draft-only sessions: `CommitIrreversible`-class plan nodes are
  Denied (new `DenyReason` variant, decided **in the executor** — one TCB deny
  function, one DenyReason taxonomy); `MutateReversible`/`Observe` still
  allowed. Demotion recorded as an audit event with the causal edge to the read.
- **I0 creation rule:** a Session whose intent/seed derives from external
  content starts draft-only and cannot auto-authorize Tier 3+. Seed-provenance
  field at session creation; the `caprun` CLI decides trusted-arg vs
  file-derived seed.
- **Confirmation loop:** `BlockedPendingConfirmation` surfaces the verbatim
  literal + provenance to the human via a **second command**
  (`caprun confirm <effect_id>` — testable, non-interactive-friendly); records
  confirm/deny as an audit event anchored to `SinkBlockedAnchor.effect_id`; on
  confirm releases exactly that (sink, arg, literal-digest) triple —
  **single-shot**, not a session-wide waiver or standing policy. Deny is
  durable. The release path lives in the TCB, not policy.
- **Live acceptance (§9-style, from the CLI):** hostile workspace file → worker
  reads it → session demoted (I1) → tainted routing arg Blocked (I2, existing)
  → human denies → nothing sent; separately, human confirms → effect proceeds
  exactly once; audit DAG shows the unbroken chain read → demotion → block →
  human decision — proven live on real Linux via Colima+Docker in Phase 11.

**Design gate:** a DESIGN doc for session-trust-state / confirmation semantics
gated the phases that added executor behavior (same discipline as the v1.0
executor gate) — `planning-docs/DESIGN-session-trust-state.md` +
`planning-docs/DESIGN-confirmation-release.md`, Phase 8.

**Explicitly not in v1.2:** more sinks, real LLM planner, Git/GitHub adapters,
Cedar, cross-host delegation, content-sensitive arg blocking (deferred to v2 —
tracked as `CONTENT-01`/`DOC-01`). README-vs-CaMeL positioning remains a small
optional add-on, still not done.

**Seed:** `planning-docs/MILESTONE-v1.2-SEED.md` (2026-07-01 post-v1.1
assessment). PLAN.md wins on any conflict.

</details>

</details>

## Requirements

### Validated

Shipped in **v1.8 — Git/GitHub Adapters (Effect Breadth II)** (2026-07-18). Full traceability
archived in `.planning/milestones/v1.8-REQUIREMENTS.md`.

- ✓ DESIGN-15/16: `DESIGN-git-github-http-sinks.md` (per-sink effect-class, `mint_from_http`
  mechanism, git config/hook neutralization, SSRF resolve-and-pin, auth-grant model, duplicate-PR
  CAS, closing all 11 design-gate-blocking pitfalls) cleared a fresh non-self adversarial
  code-trace (2 rounds, APPROVE) before any TCB code — v1.8
- ✓ GIT-01: `git.commit` broker-spawned confined-child sink (MutateReversible, reuses v1.7
  exec-launcher + `mint_from_exec`), git config/hooks neutralized, Linux-verified — v1.8
- ✓ HTTP-01/02/03: `http.request` GET sink (Observe, allowlisted host only); new `mint_from_http`
  mint site (non-stapled, rooted on `http_response_received`) demotes session to draft-only;
  SSRF resolve-and-pin defense (loopback/RFC1918/link-local/metadata/userinfo@/redirects denied) — v1.8
- ✓ GITHUB-01..04: `github.pr` sink via broker-held bearer token (never in worker/planner/
  ValueNode/audit-literal), explicit human auth-grant distinct from single-shot confirm, tainted
  title/body deterministically Blocked (CONTENT-01 reuse), content-derived duplicate-PR CAS — v1.8
- ✓ ENV-01: `caprun-planner` sidecar `env_clear()`'d + minimal allowlist, hermetic under
  compiled-in `webpki-roots`, live-verified against a real OpenAI HTTPS call — v1.8
- ✓ LIVE-03/04: composed exec→fs→git.commit→github.pr(mock)+http-GET workflow proven on real
  Linux, `verify_chain` true across the run; 3 adversarial legs (tainted PR-body/title, tainted
  GET url/SSRF, tainted commit message) deterministically Blocked; full-workspace regression
  green (498/0, 60 binaries), no v1.0–v1.7 regression — v1.8
- ✓ **v1.8 DONE gate cleared:** 13/13 active requirements satisfied + wired; every TCB change
  cleared a fresh non-self adversarial code-trace (DESIGN gate caught BLOCKER-1 + 3 MAJORs;
  Phase 37 caught a MAJOR `aws-lc-rs`-in-workspace defect + a git.commit Landlock/exit-code
  defect); milestone audit PASSED. Deferred: GIT-02/GIT-03 (`git.push`) → v1.9, a gate-authorized
  deferral, not a gap.

Shipped in **v1.7 — Effect Breadth I** (2026-07-18). Full traceability archived in
`.planning/milestones/v1.7-REQUIREMENTS.md`.

- ✓ DESIGN-13/14: effect-breadth DESIGN doc (confined-child exec + fs breadth + fail-closed defaults) cleared a fresh non-self adversarial trace — v1.7
- ✓ EXEC-01..04: process.exec broker-spawned confined-child sink; captured stdout/stderr mint_from_exec-minted (non-stapled, rooted on process_exited), I2-governed, kernel-confined — v1.7
- ✓ EXEC-05: blocked process.exec human-released via caprun confirm (invoke_process_exec_from_resolved, exactly-once, verify_chain true; no confirm-release mint — taint on process_exited event) — v1.7
- ✓ FS-01..03: multi-file read + file.write (O_WRONLY|O_TRUNC, existing-file-only) under WorkspaceRoot, RequestFd count limiter, I2/slot-role governed — v1.7
- ✓ LIVE-01/02: composed 4-leg acceptance + full-workspace regression green on real Linux (391/0), no v1.0–v1.6 regression — v1.7

Shipped in **v1.6 — Security Hardening** (2026-07-17). Full traceability archived in
`.planning/milestones/v1.6-REQUIREMENTS.md`.

- ✓ DESIGN-11/12: `DESIGN-security-hardening.md` (mechanism + fail-closed default for all
  five residuals) cleared a fresh non-self adversarial review before any TCB code — v1.6
- ✓ HARDEN-01: fd release (`RequestFd`) itself demotes the session to draft-only (fstat
  inode identity), CONTROL-01 benign path stays Active — v1.6
- ✓ HARDEN-02: authenticated audit chain (keyed HMAC-SHA256 + MAC'd anchor truncation/orphan
  detection); `verify_chain` is forge-resistant, not just corruption-detecting — v1.6
- ✓ HARDEN-03: replayed Allowed `email.send` at-most-once via content-derived idempotency
  CAS committed before SMTP — v1.6
- ✓ HARDEN-04: forced-Active `CreateSession` mint compiled out of the production binary
  (cfg), proven absent by a featureless-build gate — v1.6
- ✓ HARDEN-05: `file.create` `contents` role-checked + content-sensitive under I2 — v1.6
- ✓ HARDEN-06: full workspace regression green on real Linux (331/0, 49 suites) + a proven
  test per closed residual, no regression to v1.1–v1.5 — v1.6
- ✓ **v1.6 DONE gate cleared:** all 5 residuals enforced and proven live; independent
  adversarial code-trace APPROVED; milestone audit PASSED (8/8, 5/5 seams). Accepted
  residuals (D-08 per-session send budget, HARDEN-05 e2e-tainted-contents pre-D-12) carried
  forward as named future work.

Shipped in **v1.0 — AgentOS v0** (2026-06-30). Full traceability archived in
`.planning/milestones/v1.0-REQUIREMENTS.md`.

- ✓ Substrate (M0): runtime-core, sandbox, brokerd, fs adapter, substrate demo,
  locked plan-node API — v1.0
- ✓ Design gate (M0-design): DESIGN-taint-model.md, DESIGN-plan-executor.md
  (hard gate before any executor code) — v1.0
- ✓ Security demo (M1 = v0 DONE): quarantined reader, deterministic executor,
  mediated sink stub, approval hook, §9 value-injection acceptance test — v1.0
- ✓ **v0 DONE gate cleared:** the §9 test passes on a kernel-confined worker
  with a genuine, audited taint chain (`mint_from_read` is the sole broker
  taint-mint site; stapled taint fails the test). `cargo test --workspace` = 51 green.

Shipped in **v1.1 — Usable Runtime (Live §9 from the CLI)** (2026-07-01). Full
traceability archived in `.planning/milestones/v1.1-REQUIREMENTS.md`.

- ✓ Unified `caprun` onto the `brokerd::server` dispatch (no second executor path) — v1.1
- ✓ Typed `ReportClaims` IPC from the confined worker — raw bytes never reach the planner — v1.1
- ✓ Session-scoped handles; cross-session resolution denied (HARD-03) — v1.1
- ✓ Deterministic intent → PlanNode planner (handles only) + `mint_from_intent`; clean allow-path reachable (HARD-02) — v1.1
- ✓ `file.create` sink: arg-schema fail-closed, `O_EXCL`, dirfd + `openat2 RESOLVE_BENEATH` (SINK-01..04, HARD-04) — v1.1
- ✓ Mint invariant at source (HARD-05), typed `DenyReason`, broker-minted `effect_id` (HARD-06) — v1.1
- ✓ Durable genuine-taint anchor (ACC-07) + full live §9 acceptance green on real Linux (ACC-01/03/04/05/06) — v1.1

Shipped in **v1.2 — Tainted Session, Human Gate** (2026-07-07). Full
traceability archived in `.planning/milestones/v1.2-REQUIREMENTS.md`.

- ✓ Session taint state: `mint_from_read` demotes the session to draft-only;
  draft-only denies `CommitIrreversible` plan nodes in the executor, one TCB
  deny function (TAINT-01..04) — v1.2
- ✓ I0 creation rule: externally-seeded sessions start draft-only via
  `--seed-from-file` (ORIGIN-01/02) — v1.2
- ✓ Confirmation loop: `caprun confirm`/`caprun deny <effect_id>` releases or
  durably blocks exactly one (sink, arg, literal-digest) triple, TCB-resident,
  single-shot (CONFIRM-01..04) — v1.2
- ✓ DESIGN doc (session-trust-state + confirmation semantics) gated all
  executor behavior changes before code (PROC-01) — v1.2
- ✓ **Live acceptance on real Linux (v1.2 DONE gate):** hostile read → I1
  demotion → I2 block → human deny (nothing sent) / human confirm (effect
  proceeds exactly once), one unbroken audit-DAG causal chain for both
  outcomes, proven via Colima+Docker (ACC-01/02/03). Caught and fixed a
  pre-existing stale test assertion (`s9_live_block.rs`, dating to Phase 9,
  never previously exercised on Linux) in the process.

Shipped in **v1.3 — Doc → Action Assistant** (2026-07-09). Full traceability
archived in `.planning/milestones/v1.3-REQUIREMENTS.md`.

- ✓ DESIGN-01: adversarially-reviewed DESIGN doc gates all v1.3 executor/TCB
  code (content-sensitivity, real-adapter mediation, confirm-binding) — v1.3
- ✓ SMTP-01/02/03/05, SEND-01/02: real broker-mediated SMTP adapter (lettre,
  `email_smtp.rs`) — worker never touches the network, secrets never leave
  the broker, atomic at-most-once send, kernel-denied negative-net control,
  CRLF/header-injection defense proven live — v1.3
- ✓ CONTENT-01/02: executor collect-then-Block reshape — a tainted email
  body Blocks the same way a tainted recipient does, in the SAME decision,
  never first-match-wins — v1.3
- ✓ EXTRACT-01/02/03: deterministic doc→action extraction with genuine
  provenance threading (`mint_from_derivation`) — closes the milestone's #1
  laundering risk (a transform-derived value can no longer be stapled fresh
  at the sink) — v1.3
- ✓ CONFIRM-01..04, CONTROL-01/02: full-set name-bound `combined_digest`
  confirm binding, verbatim recipient+body narration, a real live negative
  control (trusted send Allowed & delivers vs. doc-derived send Blocked) —
  v1.3
- ✓ **ACCEPT-01 (v1.3 DONE gate):** ONE shared audit.db, 3 sessions
  (confirm/deny/clean), all independently `verify_chain`-true, live on real
  Linux via Colima+Docker: hostile doc read → I1 demotion → deterministic
  extraction → tainted recipient+body Block → confirm sends exactly once →
  a SEPARATE hostile block denies, sending nothing (both Mailpit count==0
  AND audit-ledger absence) → the clean-send control delivers ungated. The
  milestone's HARD GATE (Phase 15's unbroken-edge + anti-staple proof)
  re-verified against these live anchors, not assumed from Phase 15's own
  coverage.
- ✓ **DOC-01:** PROJECT.md honestly scopes what v1.3 proves (taint
  enforcement via a deterministic extractor with genuine propagation) and
  does not prove (taint surviving a real LLM planner's regeneration) — see
  "What v1.3's live proof does and does not claim" above.

Shipped in **v1.4 — Trust-Boundary Integrity & the Adversarial Planner**
(2026-07-11). Full traceability in `.planning/REQUIREMENTS.md`.

- ✓ TRUST-01/02/03, DOC-02: one-way session-lifetime occupancy latch closes
  the cross-connection `ProvideIntent` bypass; 3 independent regression
  variants (overlapping + sequential-reconnect, the latter added after a
  fresh adversarial review caught the original release-on-disconnect design
  was unsound); live on real Linux, no regression from v1.3 — v1.4
- ✓ DESIGN-01..06: `DESIGN-session-trust-coherence.md` cleared a 2-round
  fresh adversarial review before any TCB change — v1.4
- ✓ PLANNER-01/02/04: real `Planner` trait seam; broker `ConnectionRole`
  capability model admits one capability-restricted planner-role connection
  (fail-closed default-deny on all mint verbs + raw-fd access), reduced
  `PlanNodeDecisionReduced{blocked}` signal — v1.4
- ✓ PLANNER-03: genuine OpenAI-backed `LlmPlanner`, structurally isolated in
  a separate `caprun-planner` sidecar process (the confined worker cannot
  `execve`/open `AF_INET` sockets), live-proven end-to-end on real Linux —
  v1.4
- ✓ **GATE-01/02/03/04 (v1.4 HARD GATE):** a hostile document's injection
  reaches the LLM planner via a taint-tracked `task_instruction` channel
  (never itself a sink-arg value); the planner, offered both a trusted and a
  tainted handle, complies and routes the tainted one to `to`; the executor
  Blocks deterministically via I2, `verify_chain` true, Mailpit==0 for the
  attacker; a trusted-intent control in the SAME composed run Allows and
  delivers exactly once; GATE-04's sentinel-leak assertion is a
  deterministic, non-network unit test against the real prompt-construction
  function. A genuine architectural finding (a locked v1.2 invariant made a
  planned control leg's "Allowed" outcome impossible) was resolved without
  touching any TCB code — see the v1.4 milestone summary above.
- ✓ T2-01: slot-type binding (handle-origin-to-slot mismatch, e.g. a
  `UserTrusted` handle placed in `to`) documented as the accepted v1.4
  residual — safe today only incidentally (every `UserTrusted` handle is
  human-typed) — enforcement deferred to v1.5.

Shipped in **v1.5 — Slot-Type Binding Enforcement (T2)** (2026-07-12). Full
traceability archived in `.planning/milestones/v1.5-REQUIREMENTS.md`.

- ✓ DESIGN-07..10: `DESIGN-slot-type-binding.md` pinned the additive
  `origin_role` tag (no I0/I1 change), unified with the existing `claim_type`
  taxonomy, resolved `mint_from_derivation` role propagation, and pinned the
  fail-closed default — cleared a fresh (non-self) adversarial review before
  any TCB code — v1.5
- ✓ T2-02..05: `origin_role` mint-time tag threaded through every mint site
  (`mint_from_read`/`mint_from_intent`/`mint_from_derivation`) and carried on
  `ValueRecord`; hardcoded `expected_role()` table in `sink_sensitivity.rs`;
  exhaustive `DenyReason::SlotTypeMismatch` (owned fields, no wildcard arm);
  fail-closed "Step 1c" per-arg hard-Deny in `submit_plan_node` — I0/I2
  precedence unchanged — v1.5
- ✓ **T2-06/07/08 (v1.5 DONE gate):** a deliberately swapped subject↔recipient
  handle pair (both `UserTrusted`) hard-Denies via Step 1c through the real
  broker path, with a durable `plan_node_evaluated` audit event and
  `verify_chain` true; an independent regression audit found 0 fixture
  bypasses; the full-workspace regression was independently re-run green on
  real Linux (309 passed/0 failed) via the bare `mailpit-verify.sh` recipe,
  with human milestone-close sign-off — v1.5. Sound documented deviation:
  `email.send` body expected-role is `["body","doc_fragment"]` (no `"body"`
  claim_type exists); recipient exfil slots unchanged.

### Active

**v1.9 — Authorized Egress + Policy & Audit Surface.** Completes the
authorized-write-egress story (git.push + http-write) and adds the first
trust-surface layer (policy + CLI/audit-viewer). Four tracks; git.push is
gated/deferrable (design-gate-first — if no fully-unprivileged destination-pinning
mechanism proves sound, it defers a 3rd time and the other three tracks still
ship):

- [ ] **DESIGN gate** — one DESIGN doc covering the TCB pieces (git.push egress
  mechanism, http-write egress, policy-vs-I2 boundary) clears a fresh non-self
  adversarial code-trace (orchestrator-owned) before any TCB code
- [ ] **GIT-02** — `git.push` sink: fully-unprivileged, broker-mediated,
  destination-pinned egress, child net-denied; remote/refspec from TRUSTED
  intent; `--force`/ref-deletion hard-denied; captured-output credential scrub;
  payload-at-confirm surfaces pushed diff + tainted-file provenance
- [ ] **GIT-03** — tainted push remote/refspec Block + confirm-release for
  git.push
- [ ] **HTTP-W-01** — `http.request` WRITE (POST/PUT) to an allowlisted host;
  request BODY taint-governed + content-sensitive under I2; reuses SSRF
  resolve-and-pin + webpki-roots egress
- [ ] **POLICY-01** — minimal declarative per-session policy (which sinks/args
  callable, allowlisted hosts/paths/repos); hardcoded-schema, NOT Cedar; **may
  never disable or override I2** (LOCKED — I2 stays hardcoded in the Rust TCB)
- [ ] **SDK-01/U1** — thin CLI/SDK to define an intent, run against a workspace,
  and a read-only audit-DAG viewer over the SQLite audit chain (`verify_chain`
  surfaced) — the trust surface
- [ ] **LIVE gate** — composed exec→fs→git.commit→git.push→github.pr + an
  http.request POST leg on real Linux, driven + inspected via the CLI/viewer, with
  adversarial legs (tainted push remote/refspec, tainted POST body) Blocked and a
  policy-deny leg refused WITHOUT weakening the I2 Block

The v1.8 DESIGN doc's §2 (git.push model), §2.5 (captured-output scrub), §2.7
(payload-at-confirm), §9 (confirm-release) carry forward as v1.9's starting
design. Out of scope for v1.9: the real LLM planner loop (planner stays
deterministic/stub), github.pr merge/comment breadth, Cedar, a web UI,
cross-host delegation.

### Out of Scope

Non-goals, reviewed at each milestone close (v0/v1.1/v1.2/v1.3) — still valid
as of 2026-07-07 unless noted:

- `process.exec` sink + filesystem read/write breadth — **IN SCOPE as v1.7**
  (Effect Breadth I; the first primitives of the Safe Coding Agent anchor,
  2026-07-17)
- Git / GitHub adapters (`git.commit`, `github.pr`), `http.request` authorized
  egress — **IN SCOPE as v1.8** (Effect Breadth II, shipped 2026-07-18). `git.push`
  gate-authorized-deferred to **v1.9** (Phase-35 design gate BLOCKER-1: seccomp
  cannot pin a confined child's `connect()` destination). Test adapter, patch/PR,
  workspace snapshots remain deferred beyond v1.9.
- Real LLM planner loop (multi-step tool-use on the v1.4 sidecar seam),
  declarative policy file, thin SDK/CLI, audit-DAG viewer, packaging —
  deferred to **v1.9/v1.10+** per the productization sketch (2026-07-17)
- Cedar policy engine — simple TOML/rules for sink access is fine; I2 stays in
  Rust (still true through v1.3 — the executor's `sink_effect_class` table
  remains hardcoded, not policy-driven)
- Cross-host delegation / Biscuit crypto — v3 concern
- gVisor / Firecracker — bubblewrap + seccomp + Landlock remains the boundary
  through v1.3
- LLM planner — a hard-coded / deterministic planner remained sufficient
  through v1.3 (re-affirmed at v1.3 scoping; NOT reopened alongside
  CONTENT-01/adapter — see `DOC-01`). Reopened and SHIPPED in v1.4 (see the
  v1.4 milestone summary above and Validated Requirements: PLANNER-01..04).
- T2 slot-type binding (executor enforcement that a handle's semantic origin
  matches its slot) — identified at v1.4 scoping as unenforced but safe-by-
  incidental-human-typing; documented as v1.4's accepted residual
  (T2-01); enforcement deferred to v1.5
- Live SES / real inbox send — **downgraded from a v1.3 requirement to an
  optional post-milestone config-swap** (was `SMTP-04` in the initial draft).
  MailHog/Mailpit IS a real SMTP send with a web UI showing arrival, which
  satisfies "real send" for the gate; live SES adds credentials/DNS/
  deliverability/throttling fragility and a live exception to default-deny-net
  at the exact claim being demoed, for ~zero legibility gain. (caprun-opus-77
  + advisor panel, 2026-07-07)
- General content-classification taxonomy/abstraction — `CONTENT-02` hardcodes
  sensitivity for the email sink's args only (one match arm), not a reusable
  framework
- Rich approval-policy learning, undo snapshots, broad effect taxonomy
- Web UI, marketplace, long-term memory, browser control, natural-language
  policy authoring
- Mac / WSL2 support — deferred best-effort; all security claims remain
  Linux-only through v1.3

## Context

- **Current state (v1.4 shipped 2026-07-11):** v0 done (v1.0) + Usable
  Runtime (v1.1) + Tainted Session, Human Gate (v1.2) + Doc → Action
  Assistant (v1.3) + Trust-Boundary Integrity & the Adversarial Planner
  (v1.4). 22 phases, 68 plans total across `runtime-core`, `sandbox`,
  `brokerd`, `executor`, `adapter-fs`, `crates/llm-planner`,
  `cli/caprun-planner`, and the `caprun` binary. Live on real Linux, the
  v1.4 composed HARD GATE run: a hostile document's injection reaches a
  genuine OpenAI-backed `LlmPlanner` (via a taint-tracked instruction
  channel, never a sink-arg value), the model complies and routes the
  tainted handle to `to`, the executor Blocks it deterministically
  (`verify_chain` true, Mailpit==0 for the attacker); a trusted-intent
  control in the SAME run Allows and delivers exactly once. Full default
  `scripts/mailpit-verify.sh` recipe: 46 test groups, 0 failed, real exit 0,
  independently re-run from scratch as the milestone-closure gate (which
  itself caught and fixed a Cargo build-artifact-placement bug — see the
  v1.4 milestone summary above).
- **Prior state (v1.3 shipped 2026-07-09):** v0 done (v1.0) + Usable Runtime
  (v1.1) + Tainted Session, Human Gate (v1.2) + Doc → Action Assistant
  (v1.3). 17 phases, 55 plans total. Live on real Linux, ONE composed run
  (`live_acceptance_v1_3_composed`, shared audit.db, 3 sessions): a hostile
  document's bytes are read (I1 demotion), deterministically extracted into
  a tainted recipient+body pair, Blocked (I2+CONTENT-01) with a
  genuinely-propagated (not stapled) taint chain re-proven live; a human
  `caprun confirm` sends exactly once via the real broker-mediated SMTP
  adapter; a SEPARATE hostile block is denied, sending nothing (Mailpit
  count==0 AND no send-attempt ledger entry); a clean, trusted-intent send
  is Allowed and delivers ungated in the SAME run. All 3 sessions
  independently `verify_chain`-true. `cargo test --workspace` = 250 passed /
  0 failed across 36 binaries on real Linux via `scripts/mailpit-verify.sh`.
- **Prior state (v1.2 shipped 2026-07-07):** v0 done (v1.0) + Usable Runtime
  (v1.1) + Tainted Session, Human Gate (v1.2). 11 phases, 34 plans across `runtime-core`,
  `sandbox`, `brokerd`, `executor`, `adapter-fs`, and the `caprun` binary.
  Live on real Linux: a session demoted mid-run by a hostile read (I1) has its
  tainted routing arg Blocked at `file.create` (I2), and a human `caprun
  deny`/`caprun confirm` either durably blocks the effect or releases it
  exactly once — one unbroken audit-DAG causal chain
  (`fd_granted→file_read→session_demoted→sink_blocked→confirm_{denied,granted}`)
  proven for both outcomes via Colima+Docker (ACC-01/02/03). `cargo test
  --workspace` green on macOS (Linux-gated tests correctly show as excluded,
  not "0 passed" gaps).
- **Prior state (v1.1 shipped 2026-07-01):** v0 done (v1.0) + Usable Runtime
  (v1.1). 7 phases, 30 plans across `runtime-core`, `sandbox`, `brokerd`,
  `executor`, `adapter-fs`, and the `caprun` binary. A real kernel-confined
  `caprun` run drives a live `file.create` sink: hostile input is
  deterministically blocked on a genuine, DB-durable taint chain; a trusted
  intent path is allowed. `cargo test --workspace` green on macOS; full live §9
  acceptance (ACC-03/04/05/07) green on real Linux (Colima+Docker). Security
  claims remain Linux-only.
- **v1.1 delivered (Phase 7 complete, 2026-07-01):** `file.create` is a real,
  hardened sink (schema gate, fail-closed, `O_EXCL`, dirfd + `openat2`
  `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`); mint invariant enforced at the source;
  typed `DenyReason`; durable genuine-taint anchor (ACC-07) persisted across
  process exit; live §9 hostile-block + clean-allow + unbroken causal chain green
  on real Linux `caprun`. Verifier independently re-ran the Colima/Docker recipe.
- **Next milestone:** unscoped — run `/gsd-new-milestone` (questioning →
  research → requirements → roadmap). v1.5 candidates already identified:
  T2 slot-type binding enforcement (deferred at v1.4 scoping).
- **Source of truth:** `planning-docs/PLAN.md` ("AgentOS v0 — Definitive Plan").
  On any conflict, PLAN.md wins. Background detail lives under `archive/`
  (security: `archive/AGENT-RUNTIME-HANDOVER.md`; architecture narrative:
  `archive/multi-part/*`; red-team / open risks:
  `archive/agent-execution-runtime-handover.md`).
- **Thesis:** Humans execute programs; agents execute intents. Object-capability
  scoping is natural for machines. The runtime gives agents no ambient
  authority; every external effect is authorized against a Session; confinement
  is kernel-enforced.
- **Convergence:** Plan agreed by AoS-claude, AoS-codex, AoS-grok (2026-06-29),
  `#aos-session0` convergence. Debate closed on all `(DECIDED)` items.
- **Residual risks (acknowledged, not solved in v0):** an fd cannot be
  selectively revoked after SCM_RIGHTS handoff (mitigated by disposable workers
  + mediated high-risk effects); planner/intent-creation injection (mitigated by
  the I0 draft-only rule); steganographic encoding in extract values (accepted,
  documented in the threat model); broker bugs = full compromise (mitigated by
  keeping the broker small).
- **v1.3 residual risks (Phase 16/17, DOC-01):** `verify_chain` detects
  single-store and non-recomputing multi-store tampering, but the chain head
  is NOT externally anchored — an actor with `events` table write access can
  forge it end-to-end. Accepted residual risk; v2: keyed-MAC / head-pin. The
  Allowed email.send path has NO CAS — a replayed `SubmitPlanNode` sends N
  emails; the durable per-attempt ledger makes each send auditable but does
  not prevent duplication. Accepted residual risk. Guard-(c)
  (`CAPRUN_ENABLE_IPC_CREATE_SESSION`) is a runtime default-deny flag, not a
  compile-time exclusion — the forced-Active mint code ships in the
  production binary. v2: build-exclude it.
- **v1.4 residual risks (Phase 22, T2-01):** Phase 22's live gate proved the
  trust boundary Blocks deterministically regardless of planner
  intelligence — a real, adversarial, OpenAI-backed LLM planner complying
  with an injected instruction still routes into a fail-closed executor.
  T2 (slot-type binding) is the one remaining unenforced degree of freedom
  in that boundary, disclosed here rather than left implicit: the executor
  does not check that a handle's semantic origin (its taint/trust label)
  matches the semantic role of the slot it is routed into (e.g. a
  `UserTrusted` handle placed in a `to` slot is neither sensitive-untrusted
  nor slot-checked, so I2 does not fire on that basis alone). This is safe
  today only *incidentally* — every `UserTrusted` handle is human-typed (via
  `ProvideIntent`, and coherently guarded across connections since the
  Phase 19 fix), so a misrouted handle carries the human's own string, not
  an attacker's. Enforcement is explicitly deferred to v1.5 — a new
  `DenyReason` variant plus slot/taint-matching logic is real TCB scope,
  not wiring (Locked, Ben 2026-07-10 scoping). Authoritative ruling:
  `planning-docs/DESIGN-session-trust-coherence.md` §9 residual #5 (NOT
  designed there either); tracked in `.planning/REQUIREMENTS.md`'s Out of
  Scope table and `T2-01`.
- **Post-v0 roadmap:** v1 — Git/GitHub/test adapters, patch/PR, workspace
  snapshots, rich approval. v2 — multi-worker decomposition, parallel execution.
  v3 — cross-machine Sessions, Ed25519 export, broker federation. v4 — general
  adapters (email, cloud, MCP ecosystem).

## Constraints

- **Platform**: Linux (Ubuntu) only for M0/M1 — all v0 security claims are
  Linux-only (`CON-platform-linux-only`).
- **Stack / TCB**: Rust (tokio, serde, sqlx/SQLite, nix/rustix, landlock,
  seccompiler, ed25519-dalek). Python permitted for non-TCB experiments only —
  never in the trusted computing base. I2 enforcement is a deterministic,
  non-LLM plan executor hardcoded in the Rust TCB (`CON-stack-tcb`).
- **Broker API shape**: the broker effect path takes plan nodes from day one —
  `submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> }) ->
  ExecutorDecision`. Raw `EffectRequest { effect, args: Map }` straight to sinks
  is forbidden (`CON-broker-api-shape`).
- **Effect classes (v0)**: Observe / MutateReversible / CommitIrreversible at
  the planner surface; grow the ontology from audit-DAG observations, not
  upfront speculation (`CON-effect-classes`).
- **Repo layout**: single Cargo workspace at repo root; crates at
  `crates/` (`CON-repo-layout`, `DEC-repo-layout`).
- **I2 non-bypassable**: policy files may gate which sinks are callable but
  cannot disable I2; sink sensitivity map is hardcoded in v0
  (`CON-i2-non-bypassable`).
- **§9 taint genuineness**: if taint is stapled on at the sink instead of
  propagated through the DAG, the demo proves nothing and fails
  (`CON-s9-taint-genuineness`).

## Locked Decisions

All decisions below are **locked** — synthesized from the canonical SPEC's
`(DECIDED)` sections. They cannot be auto-overridden downstream; changing one
requires explicit re-opening with the user.

<decisions>

### DEC-platform-linux-only — LOCKED
M0/M1 target Linux (Ubuntu) only. Mac/WSL2 deferred to post-v0 best-effort. All
v0 security claims are Linux-only.

### DEC-product-boundary — LOCKED
Build an Intent Runtime: a user-space execution layer on stock Linux where
agents have no ambient authority, every external effect is authorized against a
Session, and confinement is kernel-enforced. NOT a kernel fork, agent framework,
desktop-automation platform, memory product, or marketplace. v0 binary is
`caprun`. Repo root is a single Rust workspace; crates live at `crates/` (no
separate `caprunner/` subdir).

### DEC-security-invariants (I0 / I1 / I2) — LOCKED
I1 and I2 are both required for v0 DONE; I0 is the creation-time rule.
- **I1 (instruction injection):** No LLM context may simultaneously hold
  untrusted content and authority to cause irreversible/external effects.
  Default = dynamic taint (reading raw untrusted bytes taints the context →
  draft-only thereafter). High-risk (Tier 3+) = hard planner/worker split.
- **I2 (value injection):** No attacker-tainted value may occupy a sensitive
  argument of an irreversible/external sink without literal-value human
  confirmation (or exact standing policy match). Enforced by a deterministic,
  non-LLM plan executor hardcoded in the Rust TCB. Policy files may gate which
  sinks are callable; they cannot disable I2.
- **I0 (intent/session-creation injection):** A Session whose intent text or
  seed derives from external/untrusted content starts draft-only and cannot
  auto-authorize Tier 3+ effects. Human gate required on context creation from
  tainted data.

### DEC-layer-roles — LOCKED
Sandbox = security boundary (namespaces, Landlock, seccomp, default-deny net).
Broker = reference monitor / control plane, NOT the boundary. Executor = I2
enforcement, the security differentiator. Adapters = the only paths to effects
(v0: fs + one mediated sink stub).

### DEC-fd-pass-policy — LOCKED
fd-pass (SCM_RIGHTS) is for read-only workspace I/O and test output (low-risk,
short-lived, disposable workers). External, irreversible, high blast-radius
effects are mediated only. Revocation = kill the worker via pidfd; leases are
not revocation.

### DEC-terminology — LOCKED
Public API and docs use exactly: Intent, Session, Planner, Worker, Broker,
Adapter, Effect, Artifact, Event. `ExecutionContext` is an internal Rust struct
backing a Session — never in the public API. Planner proposes Effects
(`RunTests`, `ApplyPatch`, …); broker/adapters use typed resources
(`fs.path:…`) internally. Grow the effect ontology from audit-DAG observations.

### DEC-architectural-lock-plan-nodes — LOCKED
Broker effect path takes plan nodes from day one:
`submit_plan_node(session_id, PlanNode { sink, args: Vec<ValueNode> }) ->
ExecutorDecision`, where each ValueNode carries literal + provenance + taint. Do
NOT authorize raw `EffectRequest { effect, args: Map }` straight to sinks. The
Week-2 executor is a minimal stub walking this shape. The API shape is not
optional.

### DEC-canonical-docs — LOCKED
PLAN.md is the single source of truth; on any conflict, PLAN.md wins. Background
detail lives under `archive/` (Security → `archive/AGENT-RUNTIME-HANDOVER.md`;
Architecture narrative → `archive/multi-part/*`; Red-team / open risks →
`archive/agent-execution-runtime-handover.md`). Gates before executor code:
DESIGN-taint-model.md then DESIGN-plan-executor.md.

### DEC-repo-layout — LOCKED
Repo root = single Cargo workspace. Crates: `runtime-core` (Intent, Session,
Effect, Artifact, Event — no I/O), `brokerd` (session lifecycle, policy, audit
DAG, adapters), `executor` (deterministic I2 interpreter, after DESIGN doc),
`sandbox` (bubblewrap, seccomp, Landlock, cgroups), `adapters/fs`, `captoken`
(v0 minimal; broker DB is authority on single host), `cli/caprun`. Stack: Rust
(tokio, serde, sqlx/SQLite, nix/rustix, landlock, seccompiler, ed25519-dalek).
Python OK for non-TCB experiments only.

</decisions>

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Linux-only for v0 (DEC-platform-linux-only) | All security claims rest on Linux kernel primitives (namespaces, Landlock, seccomp, pidfd) | — Locked |
| Intent Runtime, not a framework/platform (DEC-product-boundary) | Keep the product boundary tight; the differentiator is kernel-enforced confinement + I2, not breadth | — Locked |
| Plan-node API from day one (DEC-architectural-lock-plan-nodes) | Raw EffectRequest→sink bakes in a path where tainted values reach sensitive args with nowhere for the executor to stand | — Locked |
| I2 in deterministic Rust TCB, never LLM (DEC-security-invariants) | LLM cannot be trusted to enforce a security invariant; enforcement must be deterministic | — Locked |
| DESIGN docs gate executor code (DEC-canonical-docs) | Writing crates/executor before the taint/executor model is reviewed risks a wrong-shape enforcer | — Locked |
| §9 with genuine taint = the only v0-DONE gate | Substrate proves mediation but not value-injection defense; stapled taint proves nothing | — Locked |
| v1.2: draft-only deny decided in the executor, not a broker pre-check | Keep all deny logic in one TCB function with one DenyReason taxonomy | — Locked (Phase 9) |
| v1.2: confirmation UX = `caprun confirm <effect_id>` second command | Testable and non-interactive-friendly vs a TTY prompt | — Locked (Phase 10) |
| v1.2: confirm is single-shot (one (sink, arg, literal-digest) triple) | Standing exact-match policy is scope creep for v1.2 | — Locked (Phase 10) |
| **DEC-ai-review-satisfies-human-gate** (2026-07-06): an AI-performed adversarial re-read (by the current best-available Claude model) may satisfy the "human reviewer" requirement in design-gate checkpoints (e.g. `08-03-PLAN.md` Task 2's `checkpoint:human-verify`), when Ben Lamm explicitly authorizes it in place of his own read | Ben's explicit call after being shown the tension directly: this reverses the checkpoint's original intent — mirrored from v1.0 Phase 2 and from this milestone's own core value (AI/agent judgment is insufficient for consequential decisions; hence I0/I1/I2 + human confirmation) — but he chose to accept an AI review (Fable 5) as equivalent to his own for Phase 8's gate, after the tradeoff was named explicitly (raised: self-review of one's own prior finding is a weaker check than independent human adversarial judgment; a fresh-session independent AI check was offered as a middle ground and declined) | **Locked, retroactive to Phase 8's round-2 gate.** Applies going forward to future design-gate checkpoints unless revisited. Does NOT retroactively bless anything already recorded as "reviewed by Ben personally" elsewhere (e.g. round 1, `planning-docs/DESIGN-REVIEW-v1.2-round1.md`, is now understood to have also been Fable-authored — accepted under this same decision, not because it was independently re-verified as human work). |
| v1.2: programmatic `caprun confirm`/`caprun deny` invocation (by an integration test or an agent) satisfies "human decision" for ACC-01/02 live-acceptance purposes — Ben typing the commands himself is additive, not required | Consistent with `DEC-ai-review-satisfies-human-gate`'s precedent; the confirm/deny CLI verbs ARE the human-interface artifact regardless of who invokes them, and Phase 10's `confirm.rs` already proved the mechanism this way | — Locked (Phase 11, discuss-phase D-05). Independently re-verified anyway: the orchestrator ran the live Colima+Docker proof itself at Phase 11 verification, closing gsd-verifier's `human_needed` gap with real evidence rather than relying solely on the executor's self-report. |
| **REOPENED v1.3** — Content-sensitive sink-arg blocking (`CONTENT-01`), deferred to v2 at v1.2 scoping, is now IN | The doc→action hero demo requires blocking a tainted email *body*, not just recipient/routing — v1.2's routing-only I2 scope can't demonstrate it | — Reopened 2026-07-07 (Ben + caprun-opus-77). Hardcoded sensitivity for the email sink's args only (`CONTENT-02`), not a general taxonomy — scope guard from the advisor panel. |
| **REOPENED v1.3** — Real broker-mediated SMTP adapter, previously a mediated sink *stub* per `DEC-layer-roles`, is now IN | The hero demo requires an actual send (confirm → email arrives) to be a genuine live-acceptance proof, not a stub invocation | — Reopened 2026-07-07 (Ben + caprun-opus-77). Confined worker never performs the SMTP call; secrets live only in the broker; gate test targets local MailHog/Mailpit — live SES is optional and NOT gated (see Out of Scope). |
| **NOT reopened v1.3** — LLM planner stays out/deterministic (`DEC-security-invariants`, `DEC-canonical-docs`) | v1.3 proves taint *enforcement* through a deterministic extractor; it explicitly does not claim taint survives a real LLM planner's regeneration ("laundering" — a real model can re-emit a tainted value as fresh model-authored tokens with no provenance). That is a v1.4+ concern. | — Confirmed 2026-07-07. `DOC-01` requires PROJECT.md/external claims to state this scope honestly — no claim that v1.3 proves taint-survives-a-real-agent. |
| Phase 17's ACCEPT-01 composes 3 sessions sharing ONE audit.db, "one unbroken DAG" = per-session `verify_chain` integrity, NOT a literal cross-session `parent_id` chain | Confirm/deny are mutually-exclusive terminal states on one blocked effect (structurally requires ≥3 sessions); a literal single-session chain across confirm/deny/clean would contradict the pinned single-session-per-process DESIGN model and buy nothing — a synthetic cross-session edge is exactly the "staple" this milestone exists to reject | — Locked (caprun-opus-77, Phase 17 round 1). Adversarial panel caught a DAG-fork bug in the anti-staple control's re-mint parenting before execution (mid-chain `sink_blocked` vs. `current_chain_head`) — fixed pre-execution, confirmed in the shipped test. |
| **v1.3 "Doc → Action Assistant" SHIPPED** (2026-07-09) | Genuine byte-descent taint that propagates and blocks, collect-then-Block, full-set name-bound confirm binding, a real live email send, a controlled negative experiment (confirm sends once / deny sends nothing on the same hostile input), a closed exfiltration path, and honest disclosure of every residual risk — all proven live on real Linux and independently re-verified at every phase gate | — Shipped. No git tag (Ben's call, no push). |
| **v1.4 Phase 0 fix shape: reject a 2nd connection to an active session** | Smaller hammer — a confined worker legitimately holds exactly one connection; rejecting a 2nd is simpler TCB surface than making all per-connection trust state (intent_provided/fd_requested/session_status) coherent across N connections | — Locked (Ben, 2026-07-10 scoping) |
| **v1.4 MAJOR-2 replay risk: re-earn "accepted" in writing, no new CAS** | Under the v1.4 adaptive-planner threat model the replay actor collapses from "external" to "the milestone's own adversary," but amplification stays bounded to trusted/human-typed recipients (untrusted still Blocks) → DoS/duplication, not new exfil. Re-justify in the DESIGN doc rather than add TCB scope this milestone | — Locked (Ben, 2026-07-10 scoping). Revisit if Phase 1's planner can ever hold a mint verb. |
| **v1.4 T2 (slot-type binding): defer to v1.5** | Keeps v1.4 to one milestone (Phase 0 fix + Phase 1 adversarial planner, T2 deferred); enforcing it now would split v1.4 into two milestones per matt-essentialist's right-sizing review | — Locked (Ben, 2026-07-10 scoping). Documented as v1.4's accepted residual: safe today only because every `UserTrusted` handle is human-typed. |
| **v1.4 Phase 22 Leg-2 outcome: `Denied`, not `Allowed`** (architectural finding, not a corner cut) | A locked v1.2 invariant (Draft sessions unconditionally deny `CommitIrreversible` sinks) meant the original "both handles offered, no injection → Allowed" control leg was structurally unreachable without weakening TCB code. Redefined to assert `Denied` + diagnostic-log proof the model still chose the trusted handle — a stronger defense-in-depth finding (two independent layers both fire correctly) than the original design anticipated | — Locked (orchestrator decision during Phase 22 execution, 2026-07-11, verified directly against `crates/executor/src/lib.rs` Step 0.5 before deciding; `crates/executor` untouched). |
| **v1.4 "Doc → Action Assistant" successor SHIPPED** (2026-07-11) | The one-way trust-coherence fix (live-verified, no regression), a real `Planner` trait + broker capability split, a genuine OpenAI-backed adversarial planner structurally isolated in its own sidecar process, and the milestone's HARD GATE (hostile-doc injection reaches the LLM, model complies, executor Blocks deterministically with genuine live-verified taint propagation, trusted control still Allows+delivers) — all proven live on real Linux, independently re-verified end-to-end by the orchestrator as the closing gate (which itself caught and fixed a real Cargo build-artifact-placement bug) | — Shipped. No git tag, not pushed (matches v1.3's precedent — Ben's standing call unless told otherwise). |
| **v1.5 `email.send` body expected-role = `["body","doc_fragment"]`, not DESIGN's `["body"]`** (Phase 24) | No `"body"` claim_type exists anywhere in the code — body content arrives as `doc_fragment` (`WorkerClaim::DocFragment`); the DESIGN's literal `["body"]` would fail-closed-Deny every real body flow (incl. the shipped CONTENT-01 hostile-body-Block path). Sound because body stays content-sensitive so I2 remains the real gate, and the exfil-critical recipient slots (to/cc/bcc) were untouched and still reject doc_fragment | — Locked (Phase 24 execution, 2026-07-11; DESIGN §3 amended in-place, commit 92b9d6f). Confirmed by both the phase verifier and the milestone integration checker. |
| **v1.5 T2-08 live gate run directly by the orchestrator, not delegated to a subagent** | T2-08's whole purpose is a non-laundered independent re-run with the true exit code captured before any pipe; a subagent relaying "it passed" reintroduces the indirection the gate exists to distrust (mirrors the v1.3 coordinator-gate precedent) | — Locked (Phase 25 execution, 2026-07-12). |
| **v1.5 "Slot-Type Binding Enforcement (T2)" SHIPPED** (2026-07-12) | v1.4's accepted residual #5 closed: a misrouted `UserTrusted` handle now hard-Denies with `SlotTypeMismatch` via a fail-closed Step 1c, proven live on real Linux with a held-out swapped-handle test (genuine audit chain), a 0-bypass regression audit, and an independent bare `mailpit-verify.sh` re-run (309 passed/0 failed) + human sign-off. The independent verifier caught a real close-time bookkeeping gap (sign-off recorded post-rollup + REQUIREMENTS lag) that was reconciled, not papered over | — Shipped. Milestone audit PASSED (11/11 reqs, 5/5 integration hops). No git push yet (Ben's call). |
| **v1.7: confirm-release path does NOT mint the released exec output** | Dead ceremony — no live ValueStore/consumer in the human-driven confirm process; durable non-stapled taint lives on the process_exited event only, a structural improvement over an audit-gap that a passing verifier + green gates missed until fresh Fable-5 adversarial review caught it | ✓ Good (34-03 reconciliation). |
| **v1.7: env_clear() the confined exec-child AND worker spawns** | Neither should inherit broker secrets (OPENAI_API_KEY/CAPRUN_SMTP_*); planner-sidecar variant (TLS-env regression risk) deferred to v1.8 | ✓ Good. |
| **v1.7: fresh non-self Fable-5 adversarial code-trace guardrail caught its 8th real defect** | The confirm-release audit-gap MAJOR (Step-7 dispatch burned the one-shot confirmation, leaving no terminal event in the DAG) that a passing verifier + green Linux gates both missed — reinforces [[fresh-context-adversarial-review]] as a standing architectural necessity | ✓ Good. |
| **v1.8: defer `git.push` (GIT-02/GIT-03) to v1.9 rather than ship arbitrary child egress** | The Phase-35 design gate's fresh adversarial code-trace (BLOCKER-1) proved seccomp cannot pin a confined child's `connect()` destination — the only seccomp "relaxation" possible is all-or-nothing `AF_INET` allow, the exact exfiltration primitive the taint model exists to defeat. The sound alternative (fully-unprivileged, broker-mediated, destination-pinned egress) is a genuinely new trust posture that itself needs a design-gate + fresh adversarial review — not something to design, review, implement, and live-prove correctly in the same pass | ✓ Good (the gate did its job — see `planning-docs/DECISION-git-push-deferral-v1.8.md`). |
| **v1.8 "Git/GitHub Adapters (Effect Breadth II)" SHIPPED** (2026-07-18) | `git.commit` + `http.request` GET (new `mint_from_http` inbound-taint mechanism) + `github.pr` (bearer-token auth-grant + duplicate-PR CAS) delivered and proven on real Linux via a composed exec→fs→git→github(+http) workflow with 3 adversarial legs Blocked and 498/0 full-workspace regression; every TCB change cleared a fresh non-self adversarial code-trace; `git.push` honestly disclosed as deferred, not papered over | — Shipped. |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-18 after **starting milestone v1.9 — Authorized Egress + Policy & Audit Surface** (`/gsd-new-milestone`). Four tracks: `git.push` (GIT-02/03, gated/deferrable — opens with its own fully-unprivileged destination-pinning design-gate), `http.request` WRITE (HTTP-W-01, taint-governed body under I2), a minimal per-session policy (POLICY-01, which-sinks-callable only — NEVER overrides I2, LOCKED), and a thin CLI/SDK + read-only audit-DAG viewer (SDK-01/U1) toward a design-partner-runnable slice. Anchor unchanged: the Safe Coding Agent (edit→test→commit→push→open-PR now real). Design-gate-first (git.push egress + http-write + policy-vs-I2 boundary must clear a fresh non-self adversarial code-trace before any TCB code); a focused researcher is investigating the git.push unprivileged-egress mechanism. Prior: 2026-07-18 after v1.8 (Git/GitHub Adapters — Effect Breadth II) milestone SHIPPED. Delivered 3 of the 4 originally-scoped sinks — `git.commit`, `http.request` GET (new `mint_from_http` inbound-taint mechanism), `github.pr` (bearer-token human auth-grant + duplicate-PR CAS) — proven live on real Linux via a composed exec→fs→git.commit→github.pr(mock)+http-GET workflow with three adversarial legs deterministically Blocked and a 498/0 full-workspace regression, no v1.0–v1.7 regression. `git.push` (GIT-02/GIT-03) is DEFERRED to v1.9: the Phase-35 design gate's fresh adversarial code-trace proved (BLOCKER-1) that seccomp cannot pin a confined child's network destination, and the sound fully-unprivileged alternative is a genuinely new trust posture needing its own design-gate — a gate-authorized deferral, not a gap, disclosed in the milestone audit and here. Every TCB change cleared a fresh non-self adversarial code-trace (the DESIGN gate caught a real BLOCKER + 3 MAJORs; Phase 37's diff caught a MAJOR `aws-lc-rs`-in-workspace defect + a git.commit Landlock/exit-code defect). ENV-01 closed the v1.7-deferred `caprun-planner` sidecar `env_clear()` gap, hermetic via compiled-in `webpki-roots`. **NEXT: `/gsd-new-milestone`** (v1.9 — git.push, opening with its own destination-pinning design-gate). Prior: 2026-07-18 after v1.7 (Effect Breadth I) SHIPPED — process.exec confined-child sink + filesystem read/write breadth + EXEC-05 confirm-release, proven on real Linux (LIVE-01 composed 4-leg + LIVE-02 391/0); env_clear gap-closure (exec-child + worker) fixed, planner-sidecar deferred to v1.8.*

Prior: 2026-07-17 after v1.6 "Security Hardening (close the residuals)"
SHIPPED — all 5 phases (26-30) complete, turning the five standing TCB-local
residuals v1.1–v1.5 documented as accepted caveats into enforced guarantees
(HARDEN-01 demote-at-RequestFd, HARDEN-02 keyed-MAC audit chain, HARDEN-03
Allowed-path replay CAS, HARDEN-04 compile-out forced-Active mint, HARDEN-05
file.create contents slot), proven live on real Linux (bare mailpit-verify.sh
331 passed/0 failed across 49 suites + a separate featureless-build gate for
HARDEN-04) with true-exit-before-pipe discipline. Phase 26's DESIGN doc cleared
a fresh non-self adversarial review before any TCB code; an independent
adversarial code-trace of the final diff APPROVED (2 stale-comment fixes folded);
milestone audit PASSED (8/8 requirements, 5/5 cross-phase seams wired). No git
push yet (Ben's call).
Prior: 2026-07-12 after v1.5 "Slot-Type Binding Enforcement (T2)"
SHIPPED — all 3 phases (23-25) complete, closing v1.4's accepted residual #5.
Phase 23's DESIGN doc cleared a fresh non-self adversarial review before any
TCB code. Phase 24 threaded an additive `origin_role` mint-time tag through
every mint site, added a hardcoded `expected_role()` table and an exhaustive
`DenyReason::SlotTypeMismatch`, and wired a fail-closed "Step 1c" per-arg
hard-Deny into `submit_plan_node` (I0/I2 precedence unchanged); a sound
documented deviation corrected the body slot's expected-role to
`["body","doc_fragment"]` since no `"body"` claim_type exists. Phase 25 proved
it: a held-out swapped subject↔recipient deny test through the real broker
path with a genuine audit chain, an independent regression audit (0 fixture
bypasses), and an independent bare `scripts/mailpit-verify.sh` re-run green on
real Linux (309 passed/0 failed) with human milestone-close sign-off. The
independent phase verifier caught a real bookkeeping gap at close (human
sign-off recorded to the repo only after the auto-rollup marked the phase
complete + a REQUIREMENTS traceability lag) — reconciled before the milestone
was allowed to close, not papered over. Milestone audit PASSED (11/11
requirements, 5/5 integration hops wired). No git push yet (Ben's call).
Prior: 2026-07-11 after v1.4 "Trust-Boundary Integrity & the
Adversarial Planner" SHIPPED — all 5 phases (18-22) complete. Phase 18's
2-round fresh adversarial review caught and fixed a genuine BLOCKER before
any TCB code was written (release-on-disconnect would have left a sequential
bypass reachable). Phase 19 shipped the one-way occupancy latch, live-verified
on real Linux with no regression. Phase 20 shipped the `Planner` trait seam
and the broker's `ConnectionRole` capability split. Phase 21 shipped a
genuine OpenAI-backed adversarial planner in an isolated sidecar process.
Phase 22 — the milestone's HARD GATE — proved live that a hostile document's
injection makes the LLM planner comply and the executor Blocks it
deterministically anyway, with a trusted control still Allowing and
delivering in the same composed run; a genuine architectural finding mid-
phase (a locked v1.2 invariant made one planned control-leg outcome
unreachable) was resolved without touching any TCB code, strengthening
rather than weakening the milestone's security narrative. The orchestrator's
own independent final re-verification (re-running the full default
`scripts/mailpit-verify.sh` recipe from scratch) caught and fixed one real
bug of its own — a Cargo build-artifact-placement quirk breaking the LLM
live tests intermittently depending on build order — before declaring the
milestone done. No git tag, not pushed (matches v1.3's precedent). v1.5
unscoped. Prior: 2026-07-10 after scoping v1.4 (`/gsd-new-milestone`). An
adversarial review of a
proposed LLM-planner milestone found, and a Linux repro CONFIRMED (cargo exit
101, 2 runs), that v1.3's guard(a) is per-connection state only — a 2nd
`AF_UNIX` connection to the same session socket bypasses it, minting an
attacker-controlled `UserTrusted` literal that routes to `email.send` as
`Allowed`. v1.3's Current Milestone entry above and its `DOC-01` section were
retroactively annotated with a "Superseded finding" disclosure; the v1.3
shipped-record itself is not rewritten. v1.4 Phase 0 (fix, blocks everything)
rejects a 2nd connection to an active session, gated by a DESIGN doc + fresh
adversarial panel; Phase 1+ puts an adversarial LLM planner behind the fixed
boundary, replacing the theater-grade context-dump grep with a deterministic
construction-site sentinel assertion; T2 slot-type binding is deferred to
v1.5. Three open decisions (fix shape, replay CAS-vs-re-earn, T2 defer-vs-
enforce) resolved with Ben, all matching the recommended defaults. Prior:
2026-07-09 after v1.3 "Doc → Action Assistant" shipped —
Phase 17 (Live Acceptance & Framing Honesty) closed the milestone: a
composed 3-session live run on real Linux proves confirm-sends-once,
deny-sends-nothing (Mailpit count AND ledger), and a clean control delivers,
all sharing one audit.db with per-session `verify_chain` integrity; the
milestone's HARD GATE (genuine, non-stapled taint descent) was re-proven
against these live anchors; all 8 DOC-01 honesty points landed in this
document. Independently re-verified twice — once by caprun-sonnet-77 (250/250
tests, exit 0 captured before any pipe) and once by caprun-opus-77 tracing
the committed test source directly, not trusting either party's SUMMARY.
Six phases (12-17), ~4 adversarial FAMP rounds each; every round found
something real, including a live email-exfiltration hole this project's own
Phase-16 mandate had opened and a Phase-17 audit-DAG fork bug. No git tag
this milestone (Ben's call) — v1.4 unscoped. Prior: 2026-07-08 after Phase 14
(content-sensitive sink-arg
blocking) completed and verified — CONTENT-01/02 confirmed, collect-then-Block
plural-anchor reshape independently gsd-verifier-checked. Prior: 2026-07-08
after Phase 13 (real broker-mediated SMTP adapter) completed and verified —
SMTP-01/02/03/05, SEND-01/02 all confirmed live on real Linux via
Colima+Docker. Before that: 2026-07-07 after starting v1.3
"Doc → Action Assistant" milestone (`/gsd-new-milestone`). Reopened
`CONTENT-01` and the real SMTP adapter (see Key Decisions); LLM planner
remains out. Prior:
v1.2's DONE gate (Phase 11):
a new Linux-gated integration test
(`cli/caprun/tests/live_acceptance_tainted_session.rs`) proves ACC-01/02/03
live on real Linux via Colima+Docker — hostile read → I1 demotion → I2 block
→ human deny (nothing sent) / human confirm (effect proceeds exactly once),
one unbroken causal chain (`verify_chain()` true, corrected `parent_id` walk)
for both outcomes. A pre-existing stale assertion in `s9_live_block.rs`
(dating to Phase 9's chain-head fix, never previously exercised on Linux) was
caught and fixed as part of this phase. VERIFICATION.md records both the
initial gsd-verifier pass (macOS, correctly scored human_needed for the
Linux-only claims) and the orchestrator's independent same-session
Colima+Docker re-run that closed the gap with real evidence. v1.0 shipped
the mechanism proof; v1.1 shipped the live runtime; **v1.2 shipped the
tainted-session human gate** — draft-only demotion (I1/I0) and single-shot
confirmation (CONFIRM-01..04) are now proven live, not just unit-tested.
Full v1.2 detail archived to `.planning/milestones/`. Next: unscoped — run
`/gsd-new-milestone`.*
