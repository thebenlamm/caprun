# Milestones

## v1.9 Authorized Egress + Policy & Audit Surface (Shipped: 2026-07-19)

**Phases completed:** 6 phases, 22 plans, 22 tasks

**Key accomplishments:**

- Task 1 — `SessionPolicy` (`crates/runtime-core/src/policy.rs`, `lib.rs`)
- Deny-only pre-I2 policy gate wired into the executor TCB (policy_gate.rs, Step 0.25) with the breaking `policy: &SessionPolicy` signature threaded through every workspace caller; POLICY-02 proven by construction and by an enforcement-order test — a permissive policy provably cannot weaken an I2 taint Block.
- The broker binds the session policy at session creation from a trusted source outside the confined worker's reach — refusing any at-or-beneath-workspace path via the SAME shared containment helper as MAC-key custody, capturing it immutably by value, and recording its SHA-256 identity as a genuine hash-chained policy_bound audit-DAG event.
- Task 1 — KNOWN_SINKS row + GitPush ontology reconcile (WG-4)
- Task 1 — pkt-line + advertisement + report-status parsers
- Task 1 — WG-2 binary launcher + confined pack generation
- A differential git.push acceptance test where taint is the sole Block variable — proven on real Linux by delivering a clean confirmed push to a pinned mock git-receive-pack (git_push_succeeded, credential/URL absent) while a tainted remote/refspec Blocks on the named arg and force/delete are refused by construction — plus a git-receive-pack mock (WG-9) and completed HYG-01 hygiene gates.
- `caprun run` verb with a `--policy` flag over the single bind_policy call, a post-I2-Block review/confirm/deny operator pointer, and the M7 disjointness guarantee — file-derived intent literals minted TAINTED via mint_from_read instead of laundered through the trusted intent mint.
- Extracted the private `neutralize_control_chars` into a single cross-crate `brokerd::display::neutralize_control_chars` pub fn, rewired confirmation.rs's git.push confirm-prompt call sites to it with byte-identical behavior, and added an anti-drift test binding both callers to the ONE implementation (WG-2 / U1 M3).
- A Linux-gated composed live proof that drives the full authorized-write chain (process.exec → filesystem edit → git.commit → git.push confirm-release → github.pr → http.request.write POST) through the REAL broker arms over ONE shared persisted audit.db, inspects every session via a genuine `caprun audit` subprocess, and includes a genuine `caprun run` I2-Block leg — half of the v1.9 DONE gate (LIVE-05).
- Five independently-attributable negative legs — two I2 Blocks under a policy that PERMITS the sink, a distinct policy-deny (`code()=="policy_deny"`, no `sink_blocked`), a destination-pin redirect refusal, and credential-absence across value store / audit chain / broker log — proven in ONE composed run over a shared persisted `audit.db` on real Linux.
- Assembled `46-MILESTONE-RECORD.md` — the framing-honest v1.9 DONE-gate evidence record: hybrid-composition disclosure (composed-in-crate through the real broker arms vs `caprun audit`-inspected vs one genuine `caprun run` Block leg), the 5 independently-attributable negative legs with correctly-attributed credential-absence clauses, the ratified decision-level policy-deny reading (W3), the git.push SHIPPED / safety-valve-NOT-triggered disposition, and the 10MB pack-cap non-blocking deferral — with the full-workspace no-regression run honestly delegated to the orchestrator and human sign-off left as an AWAITING placeholder.

---

## v1.8 Git/GitHub Adapters (Effect Breadth II) (Shipped: 2026-07-18)

**Phases completed:** 5 phases, 16 plans, 23 tasks

**Key accomplishments:**

- A 658-line, §0-§12 design contract (DESIGN-15) pinning the dispatch pattern, effect-class, I2-sensitive args, taint flow, and confinement for git.commit (MutateReversible), git.push (net-allowed confined child, FORK 1), read-only http.request GET (Observe + the new mint_from_http/HttpRaw mechanism), and github.pr (session-scoped auth-grant + duplicate-PR CAS) — closing all 11 design-gate-blocking pitfalls with a named mechanism each, ready for the fresh non-self adversarial code-trace.
- DESIGN-16 — the v1.8 DESIGN doc clears a fresh, non-self,
- TaintLabel::HttpRaw (compile-forced untrusted) plus the hardcoded executor rows that make http.request a callable, Observe-classified GET sink whose single `url` arg is routing- AND content-sensitive with an unconstrained role gate — no network, mint, or dispatch code (those are Plans 02/03).
- Task 1 — brokerd net deps (broker-only).
- Task 1 — `mint_from_http` + Gate-3 extension (quarantine.rs, check-invariants.sh).
- Added a `KNOWN_SINKS` row for `github.pr` with `allowed == required == {owner,repo,base,head,title,body}` (exact-match, all six required — mirrors `file.write`, not `process.exec`'s optional-arg asymmetry). Any extra arg (`draft`, `headers`, `method`, …) is `Denied(UnknownArg)` at Step 0; a repeat is `DuplicateArg`; any of the six absent is `MissingArg`. Doc comment states the schema gate enforces only the arg NAME set, not taint.
- Session-scoped github.pr auth-grant (session_grants + has_github_grant gate) and a content-derived duplicate-PR CAS (created_prs + combined_digest-keyed reserve_created_pr), plus the distinct `caprun grant` human CLI verb — the two independent gates and the replay defense Plans 38-04/38-05 consume.
- Task 1 — the Allowed `github.pr` arm
- 1. [Rule 3 - Blocking] Shared env lock across test modules
- A never-default cargo feature that adds EXACTLY one checked-in test CA + one test host (`github-mock.caprun.test`) to the broker egress under gate — with the release build provably webpki-roots-only + `[api.github.com]`-only, and the SSRF resolve-and-pin path untouched.
- A stdlib-only HTTPS mock GitHub endpoint plus a sibling `compose-verify.sh` that stands up Mailpit + the mock on ONE public-range docker network, add-hosts only the mock (api.github.com untouched), and runs the unprivileged rust:1 suite with the `mock-egress-ca` feature and a leading workspace build — true exit code captured before any pipe.
- The v1.8 DONE gate passes on real Linux: a composed exec -> file.write -> git.commit (real confined) -> github.pr (mock 201) -> http GET (real api.github.com) workflow, three deterministic adversarial Blocks/Denies, and the ENV-01 hermetic-env_clear live-HTTPS proof — full workspace 498 passed / 0 failed, TRUE_RC=0.

---

## v1.7 Effect Breadth I (Shipped: 2026-07-18)

**Phases completed:** 4 phases, 17 plans, 38 tasks

**Key accomplishments:**

- Authored `planning-docs/DESIGN-effect-breadth-exec.md` (§0-§10, ~730 lines): pins the broker-spawned `pre_exec`-confined-child model for `process.exec` (Option A default, Option B launcher fallback), the sole `mint_from_exec` taint-mint site rooted at a new `process_exited` Event, the `O_WRONLY|O_TRUNC` fs write/edit sink, and a complete fail-closed defaults table — with `process.exec`'s own command/args pinned routing- AND content-sensitive under the unmodified I2 collect-then-Block loop.
- A fresh non-self Fable-5 adversarial code-trace found 1 BLOCKER + 3 MAJOR in the effect-breadth DESIGN doc; all resolved by design-tightening amendments, gate CLEARED, authorizing Phases 32-34.
- `process.exec` is now a fail-closed, CommitIrreversible, I2-governed sink whose `command`/`args` classify BOTH routing- and content-sensitive (so tainted values Block), added entirely via table entries in `sink_schema.rs`/`sink_sensitivity.rs` with zero changes to `submit_plan_node` enforcement logic, plus a new compiler-enforced `TaintLabel::ExecRaw` untrusted variant in runtime-core.
- New `exec_child_ruleset()` (Landlock narrow-allow: system-path Execute + workspace-only ReadWrite) and `exec_child_filter()` (seccomp net-deny without execve-deny), added beside the unchanged worker constructors for the upcoming `caprun-exec-launcher` (32-03) to self-apply post-fork.
- New `sinks::process_exec::invoke_process_exec` async function that spawns `caprun-exec-launcher` (never the worker) via `tokio::process::Command`, captures its combined stdout+stderr concurrently under a 30s wall-clock timeout and a 10 MiB shared byte cap, and records a two-phase `process_exited`/`process_spawn_failed` durable audit event chained onto the causal head — returning `(event_id, hash, combined_output)` for 32-05's `mint_from_exec` to root its taint chain on, without minting anything itself (Gate 3).
- `mint_from_exec` mints captured `process.exec` output as a genuinely-rooted untrusted `ValueNode` — provenance_chain anchored on `invoke_process_exec`'s already-appended `process_exited` event id, never a stapled/fresh root — and the minted handle is now wired back to the worker via a new required `BrokerResponse::PlanNodeDecision.output_value_id` field, closing the producer→consumer path EXEC-03's later Block depends on.
- Wrote and ran, ON REAL LINUX (Colima+Docker, `rust:1`, `seccomp=unconfined`), the phase's per-requirement acceptance tests for all four EXEC-01..04 success criteria — and in the process found and fixed FOUR genuine bugs (two blocking compile/runtime errors in prior plans' code, two missing Landlock rights) that a Mac-only build could never have caught.
- Added `WorkspaceRoot::write_within` (O_WRONLY|O_TRUNC via openat2, no O_CREAT) as the existing-file-only sibling to `create_exclusive_within`, with a fresh 5-test negative/edge set proving fail-closed ENOENT-on-missing-target and kernel-level absolute/traversal/symlink rejection, verified green on real Linux via Colima.
- Registered `file.write` in the executor's I2 schema/sensitivity/role tables as a table-entries-only extension, with `contents` deliberately admitting a wider role set than `file.create` to support the Phase 32 chained exec-output flow.
- Per-session `RequestFd` call limiter (`MAX_REQUEST_FD_PER_SESSION = 256`) added to `crates/brokerd/src/server.rs`, bound-checked at the top of the existing single-file RequestFd arm, failing closed with an `Error` while keeping the connection alive past the bound.
- New `invoke_file_write` broker sink module (two-phase durable audit) wired as a fourth Allowed-dispatch arm in `evaluate_plan_node_and_record`, overwriting existing WorkspaceRoot files via `write_within` with no mint and no new EffectRequest.
- s9_file_write_block.rs proves a genuine (non-stapled) taint-Block on file.write's path/contents slots in-process, cross-platform; the full FS-01/02/03 change set compiles and passes on real Linux with zero cfg-linux blind spots.
- Added `invoke_process_exec_from_resolved` — the confirm-time release twin of `invoke_process_exec` — reusing the same confined-launcher spawn discipline and chaining its two-phase audit onto the passed `confirm_granted` head, with Linux unit coverage proving both the success and byte-cap spawn-failure paths.
- `caprun confirm` now releases a Blocked `process.exec` at parity with file.create/file.write/email.send — async `confirm()`, a synchronized Step-4.75 guard + Step-7 dispatch arm, a sanctioned inline-annotated `mint_from_exec` call site, and a real cross-process Linux acceptance test proving exactly-once release with an unbroken `verify_chain`-true audit chain.
- Authored a shared-audit.db, four-leg composed live-proof test (`live_acceptance_v1_7_composed.rs`) proving the tainted-exec I2 Block, a clean exec Allow, an in-WorkspaceRoot `file.write`, and the EXEC-05 confirm-release path all in one run — LIVE-01 passed true-exit-0 on real Linux, and LIVE-02's full-workspace regression is green (390/0) with no regression to v1.0-v1.6.

---

## v1.6 Security Hardening (Shipped: 2026-07-17)

**Phases completed:** 5 phases, 14 plans, 27 tasks

**Key accomplishments:**

- DESIGN-security-hardening.md pins the mechanism + fail-closed default for all five v1.6 TCB-local residuals (demote-at-RequestFd, keyed-MAC audit chain, Allowed-path replay CAS, compile-out forced-Active mint, file.create contents slot), three cross-cutting rulings, and an explicit fold-not-accept ruling on the newly-surfaced Planner-connection session_status staleness (X-04) — every § anchored to a re-verified file:line.
- 26-02 (wave 2, depends_on 26-01)
- RequestFd now demotes a session to Draft the instant an untrusted fd is granted (fstat inode-identity compare, no path-string compare), and session_status became one shared, monotonic Arc<Mutex<SessionStatus>> re-read at the top of every dispatch_request call — closing the X-04 stale-Planner-snapshot gap in the same PR.
- Compile-excluded the CreateSession-IPC forced-Active mint arm behind a `test-fixtures` Cargo feature (executor-crate precedent), added a real behavioral D-10 negative gate proving a featureless build denies the mint even with the legacy env flag set, and recorded build-artifact SC3 evidence of its absence from a default release build.
- 7 live-test fixtures relocated to confirm.rs's F1-safe subdirectory layout (behaviorally a no-op — 274/274 tests still pass) plus hmac 0.12.1 + getrandom 0.4 wired into crates/brokerd, with zero crypto logic written — pure groundwork for HARDEN-02's F1 refusal and keyed-MAC audit chain landing in Plans 02-05.
- `load_or_create_key(audit_path, workspace_root)` — a single getrandom-backed, 0600, read-existing-first cross-process MAC-key custody helper with a canonical-path F1 containment refusal, unit-tested in isolation (3/3 green) and NOT yet wired into any runtime `open_audit_db` call site.
- Converts the audit hash chain from unkeyed SHA-256 self-consistency to a keyed, domain-separated, length-framed HMAC-SHA256 MAC, threading the broker key through all 19 production `append_event` sites and both `verify_chain` callers, with the key sourced cross-process via Plan 02's `load_or_create_key`.
- Adds a MAC'd `chain_anchor(session_id, head_event_id, head_hash, event_count)` table, upserted atomically inside `append_event`, and extends `verify_chain` to cross-check it — turning tail-truncation (raw-SQL DELETE of the last N events) from a previously-invisible attack into a detected one, and failing closed on legacy pre-Phase-28 databases with no anchor row.
- Folds `pending_confirmations` into the same broker-key HMAC-SHA256 MAC scheme as the events chain (whole-row, domain-separated), and gives `deny()` the SAME fail-closed integrity gates `confirm()` already had — closing the flip-back/delete gap on the one table that survives a confirm/deny process restart, per DESIGN's X-02 uniform ruling.
- Content-derived `plan_node_idempotency_key` (SHA256 of sink + sorted arg-name/value_id pairs) and a new `sent_plan_nodes` CAS table with idempotent migration, both landed in `audit.rs` in isolation ahead of the dispatch-site wiring in plan 29-02.
- Wired the HARDEN-03 replay CAS into server.rs's Allowed email.send dispatch (CAS INSERT OR IGNORE + attempt-marker append committed in one transaction before any SMTP socket opens) and proved at-most-once-per-plan-node live on real Linux via a new Mailpit-backed double-submit integration test.
- `file.create`'s `contents` arg is now content-sensitive and role-checked to `Some(&["path"])` in the executor TCB — closing the last unconstrained sink-arg slot from v1.5's slot-type-binding work, with zero regression to the live flow.
- Standalone false-assurance-guarded bash wrapper that forces the self-skipping harden04 D-10 negative test to actually execute, plus an independent audit confirming zero weakened/ignored hardening tests across Phases 27-29.

---

## v1.5 Slot-Type Binding Enforcement (Shipped: 2026-07-12)

**Phases completed:** 3 phases, 8 plans, 10 tasks

**Key accomplishments:**

- `planning-docs/DESIGN-slot-type-binding.md` authored (440 lines, §0–§10 + Acceptance
- `planning-docs/DESIGN-slot-type-binding.md` cleared a fresh, independent,
- Added the exhaustive `DenyReason::SlotTypeMismatch` variant with owned-type fields (never `&'static`) and updated both exhaustive matches (`code()`, `Display`) with no wildcard arm — a purely additive, self-contained change to `crates/runtime-core/src/executor_decision.rs`.
- Hardcoded `expected_role()` table + fail-closed Step 1c role check wired into `submit_plan_node` — a misrouted `UserTrusted` value now hard-Denies with `SlotTypeMismatch` before it can reach a sink, closing the v1.4 T2 residual; I0/I2 precedence unchanged.
- Added two `#[test] fn`s to `s9_acceptance.rs` that drive the real broker path (`mint_from_intent` -> `submit_plan_node`) to prove Phase 24's Step 1c slot-type binding catches a swapped subject/recipient handle pair, with a durable `plan_node_evaluated` audit-DAG event and `verify_chain` true, plus a correctly-routed Allowed control proving the deny is Step-1c-attributable.
- Independently re-ran both T2-07 search commands from scratch (not citing prior counts), cross-referenced all 31 role-checked-slot direct-mint sites against `sink_sensitivity.rs`'s `expected_role` table, found 0 bypasses, discovered one new Mac-buildable direct-mint file not in the prior session's target list, and confirmed the full Mac workspace green (46 binaries, 269 passed, 0 failed).

---

## v1.4 Trust-Boundary Integrity & the Adversarial Planner (Shipped: 2026-07-11)

**Phases completed:** 5 phases (18-22), 15 plans, 32 tasks

**Key accomplishments:**

- Authored `planning-docs/DESIGN-session-trust-coherence.md`, cleared a 2-round fresh adversarial review that caught and fixed a genuine BLOCKER before any TCB code was written — round 1's original fix design (release the occupancy latch on disconnect, permit reconnect) would have left the exact cross-connection bypass reachable via a sequential close-then-reconnect sequence; remediated to a ONE-WAY, session-lifetime latch, confirmed sound by an independent round-2 reviewer with no memory of round 1.
- Shipped the one-way occupancy latch in `run_broker_server`'s accept loop (`crates/brokerd/src/server.rs`) — rejects any 2nd connection to an already-active session, closing the confirmed live cross-connection `ProvideIntent` bypass that let a worker mint an attacker-controlled `UserTrusted` literal and route it to `email.send` as `Allowed`.
- Restructured `two_connection_intent_bypass.rs` into 3 independent fresh-broker regression variants (guard-a intra-connection control, overlapping-connection repro, sequential-reconnect repro) — all green on real Linux, full workspace suite 253 passed / 0 failed / 37 binaries (v1.3's 250/0/36 baseline plus the 3 newly-un-ignored tests), no regression.
- Introduced a real `Planner` trait (`cli/caprun/src/planner.rs`) — the existing deterministic intent→PlanNode logic (previously a bare fn) now implements it as `DeterministicPlanner`, unchanged behavior, all existing tests pass.
- Extended the broker with a `ConnectionRole` capability model — a session may now admit exactly one additional, capability-restricted planner-role connection via a `DeclarePlannerRole` handshake, fail-closed default-deny on all 4 mint verbs (`ProvideIntent`/`ReportClaims`/`ReportDerivedClaim`/`CreateSession`) plus `RequestFd`/`ReportRead`, receiving only a reduced `PlanNodeDecisionReduced{blocked}` signal on `SubmitPlanNode` (no anchors/literal_sha256/literal) — all without weakening Phase 19's one-way worker-slot latch (empty diff on its regression test).
- Built a genuine OpenAI-backed `LlmPlanner` (`gpt-4o-mini` default, `CAPRUN_PLANNER_MODEL`-configurable) implementing the `Planner` trait exactly like `DeterministicPlanner` — in-process, synchronous, worker submits via its own connection. The actual LLM HTTP call runs in a separate `caprun-planner` sidecar process, spawned by unconfined `caprun` main specifically because the confined worker itself cannot `execve` or open `AF_INET` sockets per seccomp — a structural requirement, not a style choice.
- Proved live on real Linux the milestone's HARD GATE: a hostile document's embedded injection reaches the LLM planner via a genuinely taint-tracked `task_instruction` channel (mint_from_read-rooted, structurally incapable of becoming a sink-arg value); offered both a trusted and a tainted recipient handle, the model complies with the injection and routes the tainted one to `to`; the executor Blocks it deterministically via I2 (`verify_chain` true, Mailpit==0 for the attacker); a separate trusted-intent control in the SAME composed run Allows and delivers exactly once.
- Found, during Phase 22 execution, a genuine architectural conflict between the originally-planned 3-leg proof design and a locked v1.2 invariant (Draft sessions unconditionally deny `CommitIrreversible` sinks) — resolved by redefining the control leg's expected outcome to `Denied` (proven via diagnostic-log evidence that the model still chose the trusted handle) rather than weakening any TCB code, surfacing a stronger defense-in-depth finding than originally anticipated (two independent layers, I0 and I2, both correctly firing depending on the model's actual choice).
- Replaced the theater-grade context-dump grep with a deterministic, non-network unit test (GATE-04) that feeds the real prompt-construction function a sentinel-tagged tainted record and asserts the sentinel bytes never appear in the constructed prompt.
- Documented T2 (slot-type binding) as v1.4's accepted residual risk in PROJECT.md, deferred to v1.5, without designing or implementing any enforcement.
- Independently re-verified the entire milestone end-to-end as a closing gate — re-running the full default `scripts/mailpit-verify.sh` recipe from scratch caught and fixed a real Cargo build-artifact-placement bug (a bare `cargo test --workspace` doesn't reliably place a bin-only sibling crate's binary copy, intermittently breaking the LLM live tests) before declaring the milestone done: real exit 0, 46 test groups, 0 failures.

---

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
