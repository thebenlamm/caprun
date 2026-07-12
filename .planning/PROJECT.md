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

## Current State

**Shipped v1.5 — Slot-Type Binding Enforcement (T2) on 2026-07-12.** The executor
now structurally enforces that a resolved value's semantic origin role matches the
semantic role of the plan-node slot it is routed into: a misrouted `UserTrusted`
handle (e.g. a subject-typed string landed in `to`) hard-Denies with
`SlotTypeMismatch` via Step 1c, even though it is neither untrusted (I2 doesn't fire)
nor a class-level deny (I0/I1 don't apply). Proven live on real Linux with a genuine
audit chain (v1.4's accepted residual #5 is closed). Milestone audit PASSED (11/11
requirements, 5/5 integration hops wired).

**Next milestone:** TBD — run `/gsd-new-milestone` to scope v1.6.

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

Unscoped — v1.5 is the most recently shipped milestone. Run
`/gsd-new-milestone` to scope v1.6. (Full v1.5 detail: the "Current State"
section above and Validated Requirements above.)

### Out of Scope

Non-goals, reviewed at each milestone close (v0/v1.1/v1.2/v1.3) — still valid
as of 2026-07-07 unless noted:

- Git / GitHub adapters — post-v1.2, no milestone has needed them yet
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
*Last updated: 2026-07-12 after v1.5 "Slot-Type Binding Enforcement (T2)"
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
