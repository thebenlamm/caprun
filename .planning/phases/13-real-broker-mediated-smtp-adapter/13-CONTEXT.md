# Phase 13: Real Broker-Mediated SMTP Adapter — Context

**Gathered:** 2026-07-07
**Status:** Ready for planning
**Source:** Synthesized from the APPROVED DESIGN-01 gate (Phase 12: DESIGN-GATE-RECORD-v1.3.md, Decision APPROVED / Gate UNBLOCKED) — no live discuss-phase was run, mirroring the same synthesis pattern used for Phase 12's own CONTEXT.md. Every decision below is already locked by the adversarially-reviewed design docs; this phase does not re-litigate them.

<domain>
## Phase Boundary

Build the real broker-mediated SMTP adapter and wire it into the existing single-shot confirm path. Concretely:

- A new adapter module `crates/brokerd/src/sinks/email_smtp.rs` that performs an actual SMTP send via `lettre` against a local capture SMTP (Mailpit), replacing `invoke_email_send_stub` in `crates/brokerd/src/sinks/email_send.rs` as the `"email.send"` dispatch target inside `crates/brokerd/src/confirmation.rs::confirm()`.
- At-most-once send semantics reusing the EXISTING `transition_state` CAS (`crates/brokerd/src/confirmation.rs`) — no new idempotency token, no daemon, no cross-process handoff.
- A durable, opaque-payload attempt/success/failure event ledger (`email_send_attempted` / `email_send_succeeded` / `email_send_failed`) appended to the audit DAG.
- A Linux-only kernel-enforced negative-net assertion proving the confined worker cannot open an SMTP socket.
- A CRLF/header-injection fixture proving the wire-message construction cannot smuggle recipients via a tainted body.

**Explicitly OUT of this phase's scope** (owned by later phases — do not implement here):
- Content-sensitive body/subject blocking, the collect-then-Block plural `SinkBlockedAnchor`/`ExecutorDecision` reshape, and descoping `attachment` from the sink schema/`EMAIL_SEND_CONTENT_SENSITIVE` — all **Phase 14** (CONTENT-01/CONTENT-02).
- The deterministic doc→action extractor and its provenance-threading contract — **Phase 15**.
- The combined-digest confirm-binding, verbatim block-narration UX, and the CONTROL-01/CONTROL-02 negative controls — **Phase 16**.
- Live SES / real inbox send — post-milestone, ungated (`SMTP-04`, downgraded, out of scope).

Today (pre-Phase-14), the only sink arg that actually Blocks on taint is routing (`to`/`cc`/`bcc`) — `is_content_sensitive` already classifies `subject`/`body`/`attachment` as content-sensitive (`crates/executor/src/sink_sensitivity.rs:93-98`) but the executor's current Step 3 treats that classification as a no-op, not a Block. Phase 13's own tests will naturally exercise a **tainted-recipient** Blocked/confirmed send end-to-end; body-blocking arrives with Phase 14.

</domain>

<decisions>
## Implementation Decisions

### Adapter location and worker-never-sends (SMTP-01/02, D-03/D-04)
- **D-03 (MUST):** The confined worker MUST NEVER perform the SMTP call. The new adapter lives at `crates/brokerd/src/sinks/email_smtp.rs` (broker-resident), the ONLY code path that performs the SMTP call.
- **D-03 refinement (MUST — this is a REVERSAL of the design gate's own round-1 mandate, verified unbuildable in round 2 and corrected in round 3):** The confirmed send runs in the **confirm-path process** — the SAME locus where `confirmation.rs::confirm()` invokes `file.create`'s `invoke_file_create_from_resolved` today (`confirmation.rs:403-416`, after the state-transition CAS). Do **NOT** design or build a persistent "broker daemon" — the broker is ephemeral/session-scoped (`server.rs:95-96` binds a per-session abstract socket; `cli/caprun/src/main.rs:270` aborts the broker task the instant the worker exits; there is no daemon binary; `BrokerRequest` in `proto.rs` has no confirm/perform-send control variant). `caprun confirm <effect_id>` performs the send in-process; there is no second process and no daemon handoff.
- **D-04 (MUST, restated to its real threat intent):** SMTP secrets/credentials (host, port, auth) MUST NEVER reach the confined worker or any tainted/plan-node/confined context — never in worker env/args, never as a `ValueNode`, never in any plan-node payload. For the v1.3 Mailpit gate, `localhost:1025` is unauthenticated — there is no secret, so custody is trivially satisfied. Real secret custody in a persistent process is a genuine problem ONLY for the live-SES path — explicitly deferred, out of scope, and MUST NOT be designed/built now.
- **Endpoint sourcing (MUST, round-3 tightening — closes a redirect vector):** The SMTP endpoint (host:port) MUST come from trusted local broker config or a hardcoded default — NEVER from the audit DB, a plan node, a `ValueNode`, or `PendingConfirmation` (any block-time-writable field). The combined digest (Phase 16) binds only blocked-arg literals, not the endpoint — sourcing it from writable state would let a tamperer redirect a confirmed send to an uncovered destination.

### At-most-once send + durable ledger, no swallowed errors (SEND-01/SEND-02, D-24)
- **Reuse the EXISTING CAS as the sole gate (MUST, SEND-01 — do NOT invent a new idempotency token):** `confirmation.rs::transition_state`'s `UPDATE pending_confirmations SET state=? WHERE effect_id=? AND state='pending'` CAS is the sole authorization gate for the irreversible wire action. A row already `confirmed`/`denied` matches zero rows; only the caller observing `pending → confirmed` (affected-rows = 1) may send. Round-1's cross-process/UDS-handoff idempotency-token language is dropped — there is no process boundary to make idempotent across.
- **One atomic transaction (MUST, SEND-01):** The state-transition CAS and the durable `email_send_attempted` Event append MUST commit in a SINGLE atomic SQLite transaction, BEFORE any SMTP connection is opened. Order of operations: (1) atomic {CAS `pending→confirmed` + append `email_send_attempted`}, abort if CAS affects zero rows; (2) perform the `email_smtp.rs` send from the frozen snapshot; (3) on success append `email_send_succeeded`, on error append `email_send_failed`.
- **Opaque payloads on ALL THREE send events (MUST NOT — round-3 tightening, extends beyond just the failure path):** `email_send_attempted`, `email_send_succeeded`, AND `email_send_failed` hashed payloads carry ONLY `effect_id`/opaque metadata — NEVER a resolved literal (recipient/body) or raw SMTP response text (SMTP rejections routinely echo the recipient, e.g. `550 <attacker@evil.com> rejected`). Raw error/response detail goes to `logger.error()` and/or the redactable `blocked_literals` side table ONLY — never the hash chain.
- **No auto-retry (MUST, SEND-02):** A confirmed-but-unsent state (crash/error between the step-1 transaction and a terminal step) MUST NOT be auto-retried — recovery is explicit and human-visible.
- **Never swallow (MUST NOT):** The v1.2 `Err(_) => Ok(ConfirmedButSinkFailed)` swallow-shape (currently used for `file.create` at `confirmation.rs:415`) is explicitly REJECTED for `email.send`. The error path MUST NOT `.unwrap()`/panic and MUST NOT drop the error silently — it MUST `logger.error()` with raw context AND append the durable opaque-payload `email_send_failed` event, returning a distinct non-zero result the caller can tell apart from "denied"/"unknown effect_id" (mirrors v1.2's `sink_invocation_failed` exit-code discipline).

### Kernel-enforced negative net assertion (SMTP-01, D-05)
- **MUST:** A confined worker's direct attempt to open an SMTP connection MUST FAIL under default-deny net — a kernel-enforced claim, tested on real Linux, not asserted by code inspection.
- **Point at the EXISTING mechanism (MUST NOT design a new confinement primitive):** `crates/sandbox/src/seccomp.rs::apply_worker_filter()` already denies `socket(AF_INET,...)`/`socket(AF_INET6,...)` with `EPERM`. Reuse the existing test pattern (`crates/sandbox/tests/confinement_integration.rs::negative_net`, `crates/sandbox/src/bin/confine-probe.rs::probe_net`) — Landlock does NOT restrict socket creation; only seccomp produces this `EPERM`. Do not invent a second network-denial mechanism.
- **Phase 13's negative test (MUST reuse this pattern):** add an integration test that reuses `confine-probe`'s pattern to attempt an actual `connect()` to the Mailpit host:port under confinement, asserting kernel-enforced denial — not code inspection.

### Local capture SMTP target (SMTP-03, D-06)
- **MUST:** Target Mailpit (`axllent/mailpit` Docker image) — the maintained successor to abandoned MailHog. Linux-verifiable via this project's existing Colima+Docker recipe, no live infra dependency.
- Live SES/real inbox is explicitly out of gate scope (`SMTP-04`, downgraded, ungated) — do not design the adapter, its secrets model, or its acceptance test as if live SES were a milestone requirement.

### Wire-message construction — CRLF/header-injection defense (SMTP-05, D-07/D-22)
- **Typed builder only (MUST):** Construct the outgoing message EXCLUSIVELY through `lettre`'s typed `Message::builder()` setters (`.to()`, `.cc()`, `.bcc()`, `.subject()`, `.body()`), pinned to `lettre >= 0.11.22` (fixes RUSTSEC-2021-0069 and RUSTSEC-2026-0141).
- **Forbidden (MUST NOT):** `HeaderValue::dangerous_new_pre_encoded`; building any header line via `format!()` or string concatenation; enabling the `boring-tls` Cargo feature (RUSTSEC-2026-0141 — silently disables TLS hostname verification; the local Mailpit target needs no TLS at all).
- **lettre rejection semantics (MUST, D-07 refinement):** Any `lettre` construction `Err` (`Address::new`, any builder setter) on a literal the human already confirmed MUST become a fail-closed, AUDITED abort — append a durable `email_send_failed` event with an OPAQUE payload (never the CRLF-bearing literal, never the raw `lettre` error text), route raw detail to `logger.error()`/redactable side table, return a distinct non-zero result. NEVER `.unwrap()`/panic, NEVER silent drop, NEVER fallback to a raw/`format!`-built message.
- **CRLF fixture is a HARD requirement regardless of lettre's by-construction defense (MUST, D-22):** a fixture test asserting a tainted body carrying `"...\r\nBcc: attacker@evil.com"` produces EXACTLY the intended envelope recipients at Mailpit — verified via Mailpit's HTTP API reading the captured message's actual `To`/`Cc`/`Bcc` envelope, not just that the send succeeded. "Defends by construction" must be VERIFIED, not assumed from the library's reputation.
- **Recommended negative-assertion (grep-based, mirroring `check-invariants.sh`'s style):** a test/gate asserting no `format!` call in `email_smtp.rs` builds a header line, and the token `dangerous_new_pre_encoded` never appears in that file.

### Known pre-existing items — do NOT fix in this phase, just don't regress them
- `file.create`'s `Err(_) => Ok(ConfirmedButSinkFailed)` swallow-shape stays as-is for `file.create` (grandfathered v1.2 code, out of scope). Only `email.send`'s new path must avoid this shape.
- `confirm_granted` is appended (`confirmation.rs` Step 5) BEFORE the CAS (Step 6) — a losing racer can leave a stray `confirm_granted` event. Pre-existing, already acknowledged in the code comment, non-security, not this phase's job to fix.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Design contract (pinned, adversarially reviewed — Phase 12 DESIGN-01 gate, APPROVED/UNBLOCKED)
- `planning-docs/DESIGN-content-adapter-mediation.md` (sha256 `ca6294c39b97cc85bbf2c3de369996aaaed2d1e8b0b50f37b7840c5dcba803d9`) — **primary contract for this phase**: read `## Adapter Mediation Boundary` (SMTP-01/02, D-03/D-04), `## At-Most-Once Send + Durable Attempt Ledger` (SEND-01/SEND-02, D-24), `## Kernel-Enforced Negative Net Assertion` (SMTP-01, D-05), `## Local Capture SMTP Target` (SMTP-03, D-06), and `## Wire-Message Construction` (SMTP-05, D-07/D-22) in full — these sections are this phase's spec, not background.
- `planning-docs/DESIGN-confirm-binding.md` (sha256 `fab14ec90db3a8fc5c41864fa045b1db5bf9644615c74bd33530408f35c08c17`) — background only for this phase (its content — combined-digest binding, provenance-threading, block narration — is Phase 15/16 scope). Read if touching `confirmation.rs::confirm()` to understand where this phase's changes sit relative to what Phase 16 will add on top.
- `planning-docs/DESIGN-GATE-RECORD-v1.3.md` — the full 3-round adversarial review history behind the two docs above, including the round-1→round-2 daemon-mandate reversal narrative (relevant background for why D-03's locus decision reads the way it does).

### Existing code this phase modifies or must match the pattern of
- `crates/brokerd/src/confirmation.rs` — `confirm()` (lines ~343-436) is the dispatch point; the `"email.send"` arm (currently lines 417-431, calling the stub) becomes the real adapter call. `transition_state` (~line 203) is the CAS to reuse. The `"file.create"` arm (~403-416) is the sibling pattern for a real sink invocation from a frozen snapshot — but its swallow-shape (`Err(_) => Ok(ConfirmedButSinkFailed)`) must NOT be copied for `email.send`.
- `crates/brokerd/src/sinks/email_send.rs` — the current `invoke_email_send_stub`, being replaced as confirm's dispatch target for `email.send`. Read its doc comments (T-04-05: no raw literals in payload) — the new adapter inherits this constraint.
- `crates/brokerd/src/sinks/file_create.rs` — the sibling real-sink adapter (`invoke_file_create_from_resolved`) to use as the structural analog for `email_smtp.rs`'s frozen-snapshot invocation signature.
- `crates/sandbox/src/seccomp.rs` (`apply_worker_filter`) and `crates/sandbox/tests/confinement_integration.rs` (`negative_net`) / `crates/sandbox/src/bin/confine-probe.rs` (`probe_net`) — the existing kernel-enforced net-deny mechanism and test pattern SMTP-01's negative test must reuse, not reinvent.
- `crates/executor/src/sink_schema.rs` (`email.send` schema, `allowed: &["to", "cc", "bcc", "subject", "body", "attachment"]`) and `crates/executor/src/sink_sensitivity.rs:93-98` (`is_content_sensitive`) — read-only context for this phase; do NOT edit (the `attachment` descope and any schema/sensitivity changes are Phase 14's job).

### Project-level constraints (always apply)
- `CLAUDE.md` — TCB is Rust; I2 hardcoded in the executor, never a policy file; plan-node API only (`PlanNode{sink, args: Vec<ValueNode>}`, untouched by this phase); locked terminology (Intent/Session/Planner/Worker/Broker/Adapter/Effect/Artifact/Event).
- `planning-docs/PLAN.md` — source of truth on any conflict with other docs.
- Linux-only verification recipe: Colima + `docker run --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast` (no `--privileged`).

</canonical_refs>

<specifics>
## Specific Ideas

- Mailpit runs as a Docker sidecar (`axllent/mailpit`) alongside the Linux test container — SMTP on 1025, HTTP API on 8025 for asserting captured envelope recipients (needed for the SMTP-05 CRLF fixture's verification step).
- `lettre` dependency: pin `>= 0.11.22`, default features only (do NOT enable `boring-tls`).
- The negative-net test and the CRLF fixture both require the full Linux+Colima+Docker verification path (per CLAUDE.md) — these are NOT expected to pass in a bare macOS `cargo test` run; that is expected, not a gap.

</specifics>

<deferred>
## Deferred Ideas

- Live SES / real inbox send (`SMTP-04`) — post-milestone, ungated, config-swap only. Do not scaffold credentials plumbing, a daemon, or a secret-custody model for it now (explicit future-work note in the design doc).
- Persistent broker daemon + control-socket architecture — explicitly future work IF the SES path is ever taken on; MUST NOT be built in v1.3.
- Attachment support — descoped for v1.3 entirely (Phase 14 removes it from schema/sensitivity sets); this phase's adapter has no attachment code path at all, by design.

</deferred>

---

*Phase: 13-real-broker-mediated-smtp-adapter*
*Context gathered: 2026-07-07 via DESIGN-gate synthesis (no live discuss-phase)*
