# Milestones

## v1.3 Doc → Action Assistant (Shipped: 2026-07-09)

**Phases completed:** 6 phases, 21 plans, 49 tasks

**Key accomplishments:**

- Authored `planning-docs/DESIGN-content-adapter-mediation.md` (342 lines, 52 MUST/MUST-NOT statements) mandating collect-then-Block executor hardening (fixes the B1-reincarnation risk for a tainted email body) and real broker-mediated SMTP adapter mediation with source-verified CRLF/header-injection defense — all 14 cited D-IDs resolved.
- Authored the CONFIRM-03 combined-digest DESIGN doc extending v1.2's PendingConfirmation mechanism — one SHA-256 digest over the FULL blocked-arg set (recipient+body together), post-transformation-bytes binding, no-truncation display, and every-arg block narration.
- Produce `planning-docs/DESIGN-GATE-RECORD-v1.3.md` and drive the DESIGN-01 adversarial-review gate to closure, without the authoring session self-reviewing or self-approving (D-11).
- Broker-resident `email_smtp.rs` adapter sending via lettre 0.11.22's typed `Message::builder()`, with fail-closed Address-parse CRLF rejection and opaque-payload send-lifecycle events — not yet wired into `confirm()` (Plan 02's job).
- `confirm()`'s `email.send` arm now performs a real SMTP send from the frozen `resolved_args` snapshot, with the `pending→confirmed` CAS and the durable `email_send_attempted` append committed in ONE atomic SQLite transaction before any socket opens — closing the double-fire window without a new idempotency token — plus a distinct `ConfirmOutcome::EmailSendFailed`/exit-7 path that never swallows a send failure.
- New host:port-aware `confine-probe smtp` op + Linux-only negative-net test proving a confined connect() to Mailpit is kernel-denied by the existing seccomp filter, plus a grep gate proving `CAPRUN_SMTP_` tokens never reach the caprun-worker spawn path.
- A reusable Mailpit sidecar helper (`scripts/mailpit-verify.sh`) plus a live-Mailpit acceptance test file proving both that a confirmed `email.send` effect is really captured by Mailpit (SMTP-03) and that a CR/LF-then-`Bcc:` body injection cannot smuggle a recipient into the captured envelope (SMTP-05) — verified empirically on real Linux, not assumed from lettre's reputation.
- Made `ExecutorDecision::BlockedPendingConfirmation`/`SinkBlockedAnchor` plural (`Vec<BlockedArg>`) and unified the executor's per-arg loop into one collect-then-Block pass, so a tainted `email.send` body now Blocks (CONTENT-01) alongside a tainted routing arg in the SAME decision instead of one silently pre-empting the other; `attachment` is fully descoped (D-23).
- Migrated `Event`, `brokerd`, and `cli/caprun` from the singular `anchor`/`BlockedPendingConfirmation{anchor,literal}` shape to Wave 1's plural `anchors: Vec<...>`, giving `blocked_literals` a composite `(event_id, arg)` key so every blocked literal persists (not just the first), and restored `cargo test --workspace` to fully green after Wave 1 intentionally left it red.
- `mint_from_derivation` closes the milestone's #1 laundering BLOCKER: a transform-derived value's provenance_chain now threads its inputs' own read-rooted chains (never a fresh transform-local root), with five mint-time guards including a byte-verified `join(input_literals, '@')` check against the worker's claim.
- A programmatic, DB-alone SQLite query proves genuine taint propagation through a concatenation transform for both anchors of a multi-anchor `email.send` Block, paired with two negative controls (fabricated root; same-session naive re-anchor) that reject staple attempts on the payload-bound predicate, not a vacuous existence check.
- `BrokerRequest::ReportDerivedClaim` dispatch arm resolves worker-supplied input handles and mints the derived value ONLY through `mint_from_derivation` (Plan 01), fail-closed on any unresolved handle, non-file_read-rooted union element, or concat byte-verify mismatch — proven by 5 new live-dispatch tests that call `dispatch_request` directly, not a hand-built record.
- The confined worker now extracts and concat-transforms multi-fragment recipients worker-side, reporting raw fragments and the derived recipient over the Plan-03 IPC; the planner emits `to`+`subject`+`body` routed by call-site convention, and the broker mints three genuinely distinct UserTrusted handles per email intent — closing EXTRACT-01's confined half and RESEARCH Pitfall 2.
- Adds a shared `combined_digest()` primitive (SHA-256 over `sha256(name)‖sha256(literal)` per element, byte-wise-ascending order, over the FULL resolved_args set) and wires server.rs's Block-time write to compute it once and persist it identically into the hash-chained `sink_blocked` Event and the mirrored `PendingConfirmation`, with an idempotent open-time schema migration for pre-existing DBs.
- Rewrites `render_block_display` to narrate every resolved arg (blocked AND trusted) verbatim in the digest's canonical order, adds a read-only `caprun review` pre-decision surface, and wires `confirm()`'s chain-verify + FULL-set recompute-and-compare integrity gate — parenting every confirm-phase event on the current chain head so a mismatch never forks the audit DAG.
- Live/wire-level fixture proving a tainted body with a TRUSTED recipient still blocks with exactly the `["body"]` anchor — the body (content) sensitivity dimension is independently live, not dead code redundant with the routing-sensitivity block.
- Wires the email.send Allowed-decision dispatch (CONTROL-01's production path — a trusted, never-blocked send now actually reaches the real SMTP adapter) behind three mandatory security guards that close a self-discovered reach-to-a-send exploit, with a durable pre-send attempt ledger and a live Mailpit-captured A/B proof.
- Moved `assert_unbroken_edge`, `genuine_derivation_binds`, and `union_provenance_chains` out of the private brokerd test binary into a new public `brokerd::provenance_proof` module, so Phase 17's live composed test (a different crate) can re-run the exact same HARD-GATE check instead of reimplementing it.
- Built `cli/caprun/tests/live_acceptance_v1_3.rs` — three sequential real `caprun` process invocations (hostile-block→confirm, a SEPARATE hostile-block→deny, clean-control) sharing ONE persistent `audit.db`, and proved live on real Linux (via `scripts/mailpit-verify.sh`) that the confirm leg sends exactly once, the deny leg sends nothing (both the Mailpit count AND the audit-ledger absence), the clean leg delivers ungated, and all three sessions independently `verify_chain`-true.
- Appended the milestone's HARD GATE to `live_acceptance_v1_3_composed`: both hostile sessions' `to`/`body` anchors from THIS composed live run are re-proven as an unbroken, genuinely-derived taint edge via the promoted `brokerd::provenance_proof` predicates, with both anti-staple controls (fabricated root, naive re-anchored staple chained onto the live chain head) rejected — verified live on Linux via `scripts/mailpit-verify.sh`.
- PROJECT.md now carries 7 of 8 DOC-01 honesty points (1,2,3,4,5,6,8) plus the revised nonce framing, anchored in the existing v1.3 and Residual-risks sections — point 7 is deliberately withheld pending caprun-opus-77's independent live-proof re-run.

---

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
