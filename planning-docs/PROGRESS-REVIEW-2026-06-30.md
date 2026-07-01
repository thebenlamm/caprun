Repo status as of this inspection

# caprun — External Architecture Review (code-grounded)

**Inspection date:** 2026-06-30 · **Inspector:** Claude (read-only; ground truth = code)
**Repo:** `/Users/benlamm/Workspace/AgentOS` · **Branch:** `main` · **HEAD:** `668477b`
**Host:** macOS (Darwin 25.5.0). All Linux-only enforcement/e2e tests are `#[cfg(target_os = "linux")]` and were **NOT executed** in this inspection (no Colima/Docker spun up). Where this matters it is marked explicitly.

Status vocabulary used strictly: **designed** (doc/comment only) · **stubbed** (compiles, no real effect) · **implemented+untested-here** (real code, but its proving test is Linux-gated and not run) · **implemented+tested** (real code, green test ran on this host).

---

## Executive summary

The **value-injection (I2) core is real, well-factored, and genuinely tested** at the function/in-process level. The handle model is implemented in the type system, not just the docs: a planner holds an opaque `ValueId` and *structurally cannot* author or strip taint. The §9 acceptance test asserts a genuine unbroken provenance edge (`provenance_chain[0] == read_event_id` **and** that id exists as a `file_read` row in the audit DAG) — it is **not** a taint-stapling false positive. The design gate held: no `crates/executor` code was committed until ~12h after `DESIGN-GATE-RECORD.md` recorded APPROVED.

What is **not** done, and what the milestone tracking already says is not done (v1.1 Phase 5 is 2/4 plans, Phases 6–7 unstarted):

1. **The live, end-to-end §9 block from a kernel-confined `caprun` run does not yet exist as a passing test.** Genuine taint is proven by *in-process* tests calling production functions directly. The Linux e2e currently runs only the **benign** path (`session_created → fd_granted`, 2 events). The hostile `file_read → sink_blocked` live path is deferred to plan 05-04, which is unwritten.
2. **The durable audit record of a block is a bare typed marker.** The persisted `sink_blocked` event carries `taint: vec![]` and no `ValueId`/literal/provenance anchor (`server.rs:333-341`). The genuine-taint proof lives only in the **in-memory** decision payload. Phase 7's ACC-07 explicitly requires the durable evidence to link `effect_id + sink + arg + ValueId + provenance anchor`; that is a real gap, not yet closed.
3. **Confinement is implemented but unverified in this inspection.** Landlock/seccomp/rlimits/SCM_RIGHTS are real Linux code; on macOS they are no-op stubs. The negative-assertion proof ("agent cannot reach net / `~/.ssh`") is `crates/sandbox/tests/confinement_integration.rs`, which is Linux-gated and shows `0 passed` here. I did not re-run it.
4. **No effect is ever actually executed.** The `email.send` sink stub (`invoke_email_send_stub`) is **never called** from the live broker dispatch — the allow-path appends `plan_node_evaluated` and returns. So the clean allow-path is currently a no-op decision, not a delivered effect.
5. **Human confirmation and I0 are designed, not implemented.** `build_confirmation_prompt` is a pure builder invoked **only** by the §9 test, never on the live path. `BlockedPendingConfirmation` is terminal (worker exits 1) — there is no confirm/resume/standing-policy machinery at all. The `Session` struct has **no** tainted-seed / draft-only field; I0 is absent from code.

**Bottom line:** the hard, novel part (deterministic non-LLM I2 with a non-forgeable handle model and a genuine taint anchor) is built and honestly tested. The remaining work is runtime assembly (wiring the proven functions into a confined live run and making the *durable* record carry the proof) plus I0 — which is exactly what the v1.1 roadmap claims. No architectural drift toward a token/authz framework was observed.

---

## Build & test evidence (ran on this host)

```
$ cargo build --workspace        → Finished (exit 0)
$ cargo test --workspace --no-fail-fast
  TOTALS: passed=60 failed=0 ignored=0
```

Linux-gated binaries report `0 passed` on macOS by design (cfg-gated out), specifically:
`cli/caprun/tests/e2e.rs` (substrate_demo, dag_chain_integrity) and `crates/sandbox/tests/confinement_integration.rs`. **Their real pass/fail status is undetermined in this inspection.**

Security-critical greps (non-test source under `crates/ cli/`):
- `todo!()` → **0** (only a comment in `executor_decision.rs:5`). `unimplemented!()` → **0**.
- `"SubmitPlanNode not wired"` placeholder → **0** (05-02 success criterion met; the placeholder is gone).
- `.unwrap()/.expect()` in non-test source, by file: `quarantine.rs` 15 (all in `#[cfg(test)]`), `audit.rs` 10 (test mod), `adapter-fs/src/lib.rs` 6 (test mod), `sandbox/seccomp.rs` 4 (error mapping, not panics), `sinks/email_send.rs` 4 (test mod), `cli/main.rs` 2 (`conn.lock().unwrap()` on the broker SQLite mutex — see Risk 5), `value_store.rs` 1 (test). The only `unwrap()` on the **live** path is mutex-poison `conn.lock().unwrap()` in `cli/caprun/src/main.rs:66,122`; the server arms instead map poison to an error (`server.rs:266,303,344`).

---

## 1. Repository overview

Single Cargo workspace (`Cargo.toml`, `resolver=3`, edition 2021). ~5,042 lines of Rust across `crates/*` + `cli/caprun`.

| Crate | Role | Key files |
|---|---|---|
| `runtime-core` | Pure domain types, no I/O | `plan_node.rs`, `value_record.rs`, `executor_decision.rs`, `session.rs`, `intent.rs`, `event.rs`, `effect.rs` |
| `executor` | Deterministic non-LLM I2 enforcement (the differentiator) | `lib.rs` (`submit_plan_node`), `value_store.rs`, `sink_sensitivity.rs` |
| `brokerd` | Reference monitor / control plane | `server.rs` (dispatch), `audit.rs` (DAG), `quarantine.rs` (mint), `approval.rs`, `session.rs`, `sinks/email_send.rs` |
| `sandbox` | Kernel confinement boundary | `landlock.rs`, `seccomp.rs`, `rlimits.rs`, `lib.rs`, `bin/confine-probe.rs` |
| `adapter-fs` | SCM_RIGHTS fd-pass — only path to fs effects | `lib.rs` (`pass_fd`/`recv_fd`) |
| `cli/caprun` | Orchestrator (`main.rs`) + self-confining `worker.rs` | `tests/e2e.rs` (Linux) |

**git:** branch `main`, clean working tree for **source** (only untracked `.planning/*` docs from a concurrent GSD phase). `git shortlog -sn` returned empty in this environment (committer aggregation unavailable); recent authored commits are by the GSD executor. Recent log shows v1.0 shipped (`80dae54` v0 DONE), then v1.1 phase 05 in progress through `668477b` (05-03 committed during this inspection).

**Implemented vs planned (high level):** v1.0 substrate + in-process §9 = implemented+tested. v1.1 = live wiring; 05-01/05-02/05-03 committed, 05-04 (live hostile block) + Phases 6–7 not built (`ROADMAP.md:48-95`).

## 2. Current architecture (as represented in code)

- **Intent** — `runtime-core/src/intent.rs:23` `Intent{ id, parent_id, description, created_by, status, ... }`. Created as a bare UUID in `cli/main.rs:50`; not yet a typed/structured intent input (that's Phase 6).
- **Session / ExecutionContext** — `runtime-core/src/session.rs:25` `Session{ id, intent_id, status, created_at, updated_at }`. `ExecutionContext` is referenced only in doc comments (terminology lock honored — it is not a public type). `SessionStatus` = `Active|WaitingApproval|Done|Failed|RolledBack`. **No taint/seed field** (see I0, §6).
- **PlanNode / PlanArg** — `runtime-core/src/plan_node.rs:92` `PlanNode{ sink: SinkId, args: Vec<PlanArg> }`; `PlanArg{ name, value_id }` (`:78`). **PlanArg carries no literal and no taint** — by construction.
- **ValueNode (legacy)** — `plan_node.rs:62` still defines `ValueNode{ literal, provenance, taint }` but the comment (`:56-60`) states it is "no longer routed into `PlanNode.args`." Retained for serde compatibility. It is the *old* planner-authored-value type; it is dead on the effect path. (Minor risk: it still exists and compiles — see Risk 4.)
- **ValueRecord** — `runtime-core/src/value_record.rs:21` `{ id, literal, taint, provenance_chain }`. Broker-owned; the authoritative resolution of a `ValueId`.
- **taint** — `plan_node.rs:13` `TaintLabel{ UserTrusted, LocalWorkspace, ExternalUntrusted, EmailRaw, PdfRaw, LlmGenerated, WorkerExtracted }`.
- **executor** — `executor/src/lib.rs:39` `submit_plan_node(_session_id, plan_node, value_store) -> ExecutorDecision`. Pure function over a broker-owned `ValueStore`.
- **broker** — `brokerd/src/server.rs:192` `dispatch_request(...)`: arms `CreateSession`, `RequestFd`, `ReportClaims`, `SubmitPlanNode`, `ReportRead`(deprecated→error).
- **audit/provenance** — `brokerd/src/audit.rs`: SQLite STRICT schema, SHA-256 hash chain (`compute_event_hash:69`, `append_event:100`, `verify_chain:211`). Provenance anchor lives on the `ValueRecord.provenance_chain`.
- **workers** — `cli/caprun/src/worker.rs`: self-confining; connects, confines, `RequestFd`, reads via passed fd, extracts claims locally, submits plan node, exits 1 on block.
- **approval** — `brokerd/src/approval.rs:57` `build_confirmation_prompt(...)` pure builder (not wired live).

**Canonical docs vs code:** `DESIGN-plan-executor.md` and `DESIGN-taint-model.md` are accurately reflected in `executor/*`, `quarantine.rs`, and `value_record.rs` — code matches doc intent. **Doc/code gaps to flag:** (a) I0 (draft-only seed) appears in `PLAN.md`/`intent.rs:4` comment but has **no** code representation; (b) "BlockedPendingConfirmation" implies a confirm/resume path that does not exist (it is terminal-deny today); (c) the durable `sink_blocked` record does not yet carry the provenance anchor the §9/Phase-7 narrative describes.

## 3. Security-critical path

**Untrusted read → taint origin → propagation.** Worker reads the workspace file **only** via the broker-passed fd (`worker.rs:69-82`; `open()` would be denied by Landlock on Linux). It extracts a typed `Claim` *locally* and sends only the address over IPC (`worker.rs:88-94`) — the raw sentence is discarded (lossy guarantee; `extract_email_claims`, `quarantine.rs:48`). Taint originates at exactly one site: `mint_from_read` (`quarantine.rs:124`), which in a single call (a) appends a `file_read` Event with `taint=[ExternalUntrusted, EmailRaw]` to the DAG and (b) mints the `ValueRecord` with `provenance_chain=[event_id]` (`:136-156`). This is the documented sole taint-mint site.

**HANDLE vs LITERAL — the decisive question.** The code uses **broker-owned opaque handles**, not planner-supplied `(literal, taint)`. `PlanArg{ name, value_id }` has no taint/literal field (`plan_node.rs:78`). The executor reads taint/literal/provenance **only** by dereferencing the handle in the broker-owned store (`lib.rs:47` `value_store.resolve`). **Therefore an injected planner that emits `taint:[]` cannot launder a tainted value — there is no taint field on the planner side to set, and it cannot fabricate a clean record because it does not call `mint`.** This is the strong design, and it is enforced by the type system, not convention.

**Can the planner forge OR suppress taint?**
- *Suppress* → structurally impossible (no taint field on `PlanArg`).
- *Forge a clean handle* → it can put any `ValueId` in a `PlanArg`, but an id not minted in **this connection's** store resolves to `None → Denied` (`lib.rs:47-57`; tested `phase5_dispatch.rs:89 handle_from_other_connection_store_is_denied`). Cross-session reuse is denied because each connection owns a fresh `ValueStore::default()` (`server.rs:128`, HARD-03).

**Block decision function.** `executor::submit_plan_node` (`lib.rs:39`): for each arg, resolve→`None`⇒`Denied`; if `is_routing_sensitive(sink,arg)` (`sink_sensitivity.rs:27`; `email.send` routing args = `to/cc/bcc`) **and** `record.taint` non-empty ⇒ `BlockedPendingConfirmation` with the payload copied **verbatim from the record** (`lib.rs:65-71`). The executor never mints and never constructs a `ValueRecord` (anti-stapling invariant T-04-03, asserted by negative grep documented at `lib.rs:36-38`). Content-sensitive args (`subject/body/attachment`) do **not** block in v0 (deferred to Tier-4 review) — a deliberate, documented scope cut (`lib.rs:74`, `sink_sensitivity.rs:19`).

**Human / literal-value confirmation.** Designed only. `build_confirmation_prompt` surfaces the byte-exact recipient (`approval.rs:57`) and is exercised **only** by `s9_acceptance.rs:170`. It is **not** called from `server.rs`. Live behavior on a block: worker prints and `exit(1)` (`worker.rs:128-131`) → caprun returns non-zero (`main.rs:133`). There is no confirm/resume, no standing policy, no endorsement code anywhere.

**Provenance — real derivation chain or just a stored reference?** For the I2 decision it is a **genuine anchored chain**: the blocked decision's `provenance_chain[0]` is the id of a `file_read` row that actually exists in the audit DAG (proven in-process; §5). **However**, the chain is currently length-1 (a single read→value edge), and the *durable* `sink_blocked` row does not embed the `ValueId`/anchor — so "unbroken edge **in the persisted DAG** from read-Event through the plan node to the blocked arg" is **only partially** realized: the read→block causal parentage is persisted (`server.rs:335`), but the value-level anchor that makes it *genuine* is not persisted into the block record. That is the Phase-7 ACC-07 gap.

## 4. Enforcement status

- **Sandboxing exists as real Linux code, unverified here.** `sandbox::apply_confinement` (`lib.rs:56`) applies, in order: `rlimits` (RLIMIT_AS 512MiB / RLIMIT_CPU 30s), Landlock deny-all (`landlock.rs:17`, ABI::V3, no allow-rules = everything denied), seccomp filter (`seccomp.rs:55`: deny `execve`/`execveat`, deny `socket(AF_INET/AF_INET6)`; `seccompiler::apply_filter` sets `NO_NEW_PRIVS` internally). On macOS all are `Ok(())` no-ops (`lib.rs:65`).
- **Self-confinement model** is correctly reasoned: worker confines itself *after* connecting (`worker.rs:62-63`), because Landlock-deny-all + seccomp-deny-execve cannot precede the exec that loads the worker (`lib.rs` module doc).
- **SCM_RIGHTS fd-pass: implemented+tested cross-platform.** `adapter-fs/src/lib.rs` `pass_fd`/`recv_fd`; sets `FD_CLOEXEC` post-recv (`:99`), returns `ENODATA` rather than a bogus fd on missing cmsg (`:110`). Round-trip test `pass_recv_fd_roundtrip` runs on macOS (not cfg-gated) → green.
- **pidfd:** not present (grep: no pidfd usage). Process lifetime is `child.wait()` (`main.rs:110`).
- **Effects mediated, but never executed.** The agent has no ambient effect path — the only egress is broker-mediated plan nodes, and reads are fd-only. But the *positive* effect (sending email) is a stub that is **not wired**: `invoke_email_send_stub` (`sinks/email_send.rs:38`) is **not referenced** in `server.rs` or the CLI (grep confirms). The allow-path returns `plan_node_evaluated` without dispatching the sink (`server.rs:329-355`).
- **"Cannot reach network / cannot read `~/.ssh`" test:** lives in `crates/sandbox/tests/confinement_integration.rs` (Linux-gated). **Not run in this inspection** — its status is per prior record (memory notes Phase 3 = 29/29 on Linux), not re-verified here. Treat the no-egress claim as *implemented, evidence-pending-on-Linux*.

## 5. Tests & acceptance criteria

**DESIGN GATE — held.** `DESIGN-GATE-RECORD.md` recorded APPROVED/UNBLOCKED in commit `21768d7` (2026-06-29 15:28:04). The **first** commit touching `crates/executor` is `4efe67d` (2026-06-30 03:40:44, a RED test). Executor code postdates the gate approval by ~12 hours. **No `crates/executor/**` was committed before the gate recorded APPROVED.** Both gate docs (`DESIGN-taint-model.md`, `DESIGN-plan-executor.md`) exist.

**§9 VALUE-INJECTION DEMO — exists, runs, and is genuine (not a false positive).** `crates/brokerd/tests/s9_acceptance.rs::s9_acceptance` ran green on this host. It drives **production** code (no re-implemented taint logic). The assertions that establish an **unbroken genuine-taint edge**:
- `s9_acceptance.rs:88-90` mints via `mint_from_read` (the test is forbidden from calling `store.mint` or setting taint — enforced by negative grep, `:82-85`).
- `:152-161` **held-out backstop**: `provenance_chain[0] == read_event_id` (the id returned by `mint_from_read`). A stapled-at-the-sink implementation has no such matching id → fails here.
- `:192-214` queries the DAG: a `file_read` event exists, carries `[ExternalUntrusted, EmailRaw]`, and **`file_read_event.id == provenance_chain[0]`** — i.e. the anchor is a *real persisted DAG event*, not an in-memory-only UUID.
- `:217` `verify_chain` confirms hash-chain integrity.

This is a real genuine-taint test. **Caveat (important):** `s9_acceptance` does **not** go through `dispatch_request` and does **not** append a `sink_blocked` event — it calls `mint_from_read` + `executor::submit_plan_node` directly, then `verify_chain` over a DAG containing essentially the `file_read` event. The durable-block causal parentage is proven by a *different* test (`phase5_dispatch.rs:147 block_appends_durable_causal_sink_blocked`: `sink_blocked.parent_id == read_event_id`, append-fail ⇒ `Err` fail-closed at `:208`). **No single currently-passing test proves the full `fd_granted → file_read → sink_blocked` chain on a real kernel-confined `caprun` run** — that is plan 05-04 (unwritten). The macOS-green `e2e.rs` runs only the benign 2-event chain (`e2e.rs:182-189` asserts exactly `session_created, fd_granted`).

**Other tests (ran green on this host, 60 total):** `executor/tests/executor_decision.rs` (`tainted_to_arg_blocks_with_verbatim_record_payload`, `untainted_to_arg_returns_allowed`, `unknown_handle_returns_denied`, `tainted_cc_and_bcc_also_block`); `quarantine.rs` mint/extract tests; `value_store.rs` mint/resolve + unknown-id-None; `approval.rs` literal-fidelity; `audit_dag.rs`; `proto_claims.rs`; `phase5_dispatch.rs` (5 dispatch/HARD-03/ACC-02 tests); `adapter-fs` fd round-trip; `sinks/email_send.rs` stub-records-event.

**What the green tests do NOT prove:** (1) kernel confinement actually denies net/fs/exec (Linux-gated, not run); (2) a live hostile `caprun` invocation blocks and exits non-zero (05-04 unwritten); (3) the durable block record carries the genuine-taint anchor (it does not — see Risk 1); (4) any effect is actually executed on the allow-path (sink unwired); (5) any human-confirmation/resume path (absent); (6) I0 seed behavior (absent).

## 6. Implementation risks (ranked by damage to the §9 claim)

**R1 — Durable block record does not carry the genuine-taint proof.** The persisted `sink_blocked` event is a bare marker: `taint: vec![]`, no `ValueId`/literal/anchor (`server.rs:333-341`). The genuineness lives only in the in-memory `ExecutorDecision`. If the §9 story is "the audit DB *proves* a genuine taint chain survived process exit," that is **not true today** — the DB proves causal *ordering*, while the value-level anchor is ephemeral. This is precisely Phase 7's ACC-07 ("durable evidence links effect_id + sink + arg + ValueId + provenance anchor … an event-order-only assertion is insufficient"). **Highest risk because it is the difference between "ordered events" and "provable genuine taint" — the entire point.**

**R2 — "Live §9" is not yet demonstrated end-to-end.** Genuine taint is proven for the *functions*; the kernel-confined CLI run that fires the block is unbuilt (05-04). Until then, "live §9" is an integration claim resting on the union of in-process tests + Linux substrate tests, not a single hostile run. Risk: the wiring (worker → ReportClaims → mint → SubmitPlanNode → block → exit) has a real chance of a latent ordering/IPC bug that no current test exercises hostilely on Linux.

**R3 — Confinement unverified in this inspection; macOS no-ops can mask regressions.** Because every enforcement primitive is `Ok(())` on macOS and the dev box is a Mac, a confinement regression (e.g., Landlock ABI negotiation, seccomp arch mismatch) would pass `cargo test` locally (60 green) and only surface on Linux. The no-egress claim is *evidence-pending*. Mitigation already in repo: the Colima/Docker recipe — but it must actually be run as a gate, not assumed.

**R4 — Legacy `ValueNode` still exists on the type surface.** `plan_node.rs:62` `ValueNode{ literal, taint }` is the *planner-authored-value* shape the handle model exists to kill. It is documented as off-path, but its continued existence is an attractive nuisance: any future code that routes a `ValueNode` into an effect path reintroduces taint-stapling/suppression. Recommend deleting it once the serde consumer is gone, or feature-gating it out of the effect crates.

**R5 — Live-path `unwrap()` on the broker SQLite mutex.** `cli/caprun/src/main.rs:66,122` `conn.lock().unwrap()` panics on a poisoned mutex; the server arms handle poison gracefully (`server.rs:266` etc.). A panic in the orchestrator after a block-but-before-print could obscure audit output. Low security impact (fail-closed-ish: a panic is not an allow), but inconsistent with the rest of the fail-closed discipline.

**Drift / seam checks:**
- **No token/authz-framework drift observed.** There is no Cedar, no Biscuit, no policy engine, no capability tokens. Enforcement is a hardcoded Rust decision function over broker-owned state — exactly the intent-scoped runtime, not an authz framework. ✅
- **Prompt-injection (I1):** mitigated at the extraction boundary — the worker discards the raw sentence and emits only typed claims (`worker.rs:88`, lossy). No LLM is in the path at all (deterministic scanner), so I1 is trivially satisfied for v0; the hard planner/worker split is moot until an LLM planner exists.
- **Value-injection / stapling / suppression:** structurally closed at the type level (handle model) and tested in-process. Residual exposure is **durability** (R1), not laundering.
- **Confused deputy:** the broker is the deputy that opens files with ambient authority (`server.rs:249`). Today it opens **any path the worker names** (`RequestFd{ path }`) with no workspace-root capability restriction — that's Phase 7 HARD-04 (`openat2`/dirfd, traversal/symlink rejection), currently **absent**. A confined worker could request `RequestFd{ path: "/etc/shadow" }` and the broker would open it and pass the fd. Confinement stops the worker from `open()`-ing it directly, but the broker will do it on request. **This is a live confused-deputy gap** until HARD-04 lands.
- **Endorsement / standing-policy seam:** *not contradictory in code because neither side exists.* "No auto-confirm" is honored (block ⇒ deny+exit). "Broker proposes standing policy" is unimplemented. When standing policy is built, this seam needs the reconciliation the docs promise — today there is nothing to contradict.
- **I0 — who sets the tainted-seed tag?** **Nobody.** `Session` (`runtime-core/src/session.rs:25`) has no seed/draft field; `create_session` always sets `status: Active` (`brokerd/src/session.rs:18-27`). I0 exists only as a doc comment (`intent.rs:4`). So the answer to "trusted session-creation path vs self-declared agent" is: **I0 is unimplemented** — there is no draft-only seed state to set or trust yet. Flag for Phase 6/7 design before any externally-seeded session is allowed to authorize effects.

## 7. Next recommended build step

**1-week milestone — Close the durable-proof gap and land the live hostile block (finish Phase 5 honestly).** Make the `sink_blocked` event carry the value-level anchor (`ValueId`, `arg`, `sink`, `provenance_chain[0]`) so the proof survives process exit (pre-empts ACC-07's hardest half), then write plan 05-04: a Linux e2e that runs `caprun` on hostile content and asserts (a) non-zero exit, (b) a durable `sink_blocked` whose persisted record's anchor equals the `file_read` id, (c) no effect executed. Run it under Colima/Docker as a gate. This converts "in-process genuine taint" into "live, durable, genuine taint."

**2-week milestone — Workspace-root capability for `RequestFd` (HARD-04) + the clean allow-path (Phase 6 core).** Implement the single workspace-root capability model so the broker rejects absolute paths / traversal / symlink escapes at open time (closes the confused-deputy gap), and add `mint_from_intent` + a `UserTrusted` value so a non-tainted plan node reaches `Allowed` **and actually dispatches the sink stub** — proving the allow-path is reachable end-to-end, not just a no-op decision.

**Do NOT build yet:** an LLM planner; I0 seed-state machinery beyond a minimal trusted-creation-path design doc (don't let an agent self-declare its seed tag); standing-policy/endorsement; `file.create` hardening *before* the capability model exists; Cedar/Biscuit/any policy engine; the real SMTP path. Also resist deleting the macOS no-op stubs "to simplify" — they are load-bearing for cross-platform dev.

---

## Appendix A — commands run (reproducible)

```bash
cargo build --workspace
cargo test --workspace --no-fail-fast            # 60 passed; 0 failed; 0 ignored (macOS; Linux tests cfg-gated → 0)
git branch --show-current; git status --porcelain; git log --oneline -30
grep -rn 'todo!()\|unimplemented!()' crates cli
grep -rn 'not wired\|SubmitPlanNode not' crates cli           # → none
# unwrap/expect per non-test file:
for f in $(find crates cli -name '*.rs' -not -path '*/tests/*'); do c=$(grep -cE '\.unwrap\(\)|\.expect\(' "$f"); [ "$c" -gt 0 ] && echo "$c $f"; done | sort -rn
# DESIGN GATE timing:
git log --reverse --format='%h %ci %s' -- crates/executor | head
git log --format='%h %ci %s' -- planning-docs/DESIGN-GATE-RECORD.md
# wiring checks:
grep -rn 'invoke_email_send_stub\|build_confirmation_prompt' crates cli
grep -rniE 'draft.?only|tainted.?seed|seed.?taint' crates/runtime-core/src crates/brokerd/src   # → none
```

Linux enforcement/e2e tests were **not** executed. To verify the no-egress and live-block claims, run (per `CLAUDE.md`):
```bash
colima start
docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast
```

## Appendix B — read these next (to verify findings)

1. `crates/executor/src/lib.rs` — the I2 decision function; confirm it only `resolve()`s and never mints.
2. `crates/brokerd/src/quarantine.rs` — `mint_from_read` (sole taint-mint site; the genuineness anchor).
3. `crates/brokerd/tests/s9_acceptance.rs` — the §9 backstop assertions (`:152-214`).
4. `crates/brokerd/src/server.rs` — `dispatch_request`; note the `sink_blocked` event has no value anchor (`:333-341`) and the sink is never invoked.
5. `crates/runtime-core/src/plan_node.rs` — handle model (`PlanArg` has no taint) + legacy `ValueNode` (`:56-69`).
6. `crates/brokerd/tests/phase5_dispatch.rs` — durable causal block + fail-closed + cross-session denial.
7. `cli/caprun/tests/e2e.rs` — confirm the live test only covers the benign 2-event chain today.
8. `crates/sandbox/src/{landlock.rs,seccomp.rs}` + `tests/confinement_integration.rs` — the (Linux-gated, unrun-here) enforcement evidence.
