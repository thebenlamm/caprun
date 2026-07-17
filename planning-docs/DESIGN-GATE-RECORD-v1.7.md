# DESIGN GATE RECORD — v1.7 (Effect Breadth I — `process.exec` + fs read/write breadth)

**Milestone:** v1.7 — Effect Breadth I (Phase 31 design gate)
**Document under review:** `planning-docs/DESIGN-effect-breadth-exec.md`
**Gate purpose:** Authorize (or block) any `crates/executor` / `crates/brokerd` / `crates/sandbox` /
`crates/runtime-core` code for the two new effect primitives this milestone (Phases 32–34). Mirrors
`DESIGN-GATE-RECORD-v1.6.md` (and v1.2 Phase 8 / v1.3 Phase 12 / v1.4 Phase 18 / v1.5 Phase 23).
**Requirements gated:** DESIGN-13 (doc exists, pins the broker-spawned confined-child `process.exec`
model + the fs read/write-breadth model, and clears a fresh non-self adversarial code-trace),
DESIGN-14 (doc pins the fail-closed defaults for both new sinks, nothing disables/bypasses I2, no new
raw request-args-to-sink path).

## Gate status: ✅ **CLEARED** (2026-07-17, Round 1)

Phases 32–34 are authorized to begin. All Round-1 review findings (1 BLOCKER, 3 MAJOR, 1 MINOR, 1 NIT)
are resolved in the design doc as Round-1 amendments (§11); no blocker remains. **No `crates/executor` /
`crates/brokerd` / `crates/sandbox` / `crates/runtime-core` code was written during this design-gate
phase** (re-confirmed below).

---

## Reviewer identity & independence

- **Mechanism:** a FRESH, INDEPENDENT adversarial reviewer spawned by the **orchestrator** as a separate
  agent — a **Claude Fable 5** model (`claude-fable-5`), a different model family from the doc's
  authoring context. This satisfies the project's `fresh-context-adversarial-review` discipline (a
  self-read is not sufficient; the standing `Agent(model:"fable")` substitute was used, per the
  `advisor-tool-unavailable-fable-fallback` precedent). The review spawn is orchestrator-owned per the
  standing precedent — the `gsd-executor` role has no agent-spawn capability, so the orchestrator owned
  Task 1 directly.
- **Not a self-review.** The design doc was authored by a `gsd-executor` subagent (Sonnet) from plan
  31-01; the review was run by the orchestrator spawning a distinct Fable-5 agent with no authoring
  lineage; the findings were independently re-verified against live code and folded by the orchestrator.
  Author and reviewer are distinct agents / model families.
- **Code-traced, not prose-read.** The reviewer independently opened and traced **11 files**:
  `crates/sandbox/src/{lib.rs, landlock.rs, seccomp.rs, rlimits.rs}`, `cli/caprun/src/main.rs`,
  `crates/brokerd/src/{quarantine.rs, server.rs (4 regions), sinks/file_create.rs}`,
  `crates/runtime-core/src/plan_node.rs`, `crates/executor/src/{lib.rs, sink_schema.rs,
  sink_sensitivity.rs}`, `crates/adapter-fs/src/{workspace.rs, lib.rs}`, and
  `scripts/check-invariants.sh` — plus targeted greps confirming **zero real `.pre_exec(` sites** and
  **no `RequestFd` read-count limiter** exist today.
- **Findings independently re-verified by the orchestrator against live code before folding** (per the
  project's "verify each finding against actual code before fixing — AI reviewers generate false
  positives" discipline). All four load-bearing findings (B1, M3, M1, M2) were confirmed REAL against
  live code (see per-finding code evidence); **none was a false positive.**
- **Effort:** 124,554 subagent tokens, 17 tool uses.

## Revision History

| Round | Date | Reviewer | Findings | Result |
|-------|------|----------|----------|--------|
| 1 | 2026-07-17 | Fresh independent Fable-5 agent (code-tracing) | 1 BLOCKER, 3 MAJOR, 1 MINOR, 1 NIT | All resolved as Round-1 amendments (§11) → CLEARED |

---

## Findings & resolutions

### B1 — BLOCKER → §1.4, §5, §6 (RESOLVED)

**Claim.** The doc pinned a *security* default — "the child's own seccomp filter denies `execve` for
anything AFTER its own initial one" (recursion-exec-deny) — that a stateless BPF cannot deliver. A
stateless filter either denies `execve` (killing the child's own initial exec → `spawn()` fails) or
allows it (grandchildren `execve` freely); there is no allow-once-then-deny construct. The doc checked
this impossible mechanism off in §6.

**Code evidence (orchestrator re-verified).** `crates/sandbox/src/seccomp.rs:62` — filters are stateless
`seccompiler::SeccompFilter` BPF programs; execve is denied with an unconditional `(SYS_execve, vec![])`
always-match rule (empty condition vec = always match), matched action `Errno(EPERM)`. No counting state
exists. A filter installed before the child's own `execve` is active for that execve — it cannot
distinguish the first from later ones.

**Resolution (folded, §1.4).** Dropped the recursion-deny claim. §1.4 now states there is NO seccomp
recursion-deny and documents the real, verifiable bound: the narrow-allow Landlock ruleset grants
`Execute` ONLY on enumerated system paths (so a grandchild can only exec those already-enumerated
binaries), and the reused `socket(AF_INET/AF_INET6)` net-deny **persists across `execve`** — so the
stated worry (an unaudited grandchild making network calls) is independently closed regardless of
recursion. §5 confinement-table row and §6 kernel-confined checklist item updated to match; §9 A5
promoted to the single load-bearing kernel-semantics assumption behind this resolution (net-deny
persistence, to confirm in Phase 32).

### M3 — MAJOR → §1.3, §2.5, §9 (RESOLVED)

**Claim.** Option A (`pre_exec` confinement) was pinned on the justification that "this exact call shape
already runs twice in production without incident" — evidence that does not transfer, since the existing
spawns are the *safe* no-`pre_exec` shape.

**Code evidence (orchestrator re-verified).** `cli/caprun/src/main.rs:328,356` — both existing spawns are
plain `Command::spawn()` with **no** `pre_exec` closure; a full-tree grep confirms **zero** real
`.pre_exec(` sites. The confinement primitives (`landlock`/`seccompiler`) allocate between fork and
execve, and broker dispatch runs inside a multi-threaded tokio task (`server.rs:271,308`) — the classic
fork-in-multithreaded-process allocator hazard. The "incident-free" history is for the safe shape only.

**Resolution (folded, §1.3).** Flipped the pinned default to **Option B** (a dedicated
`caprun-exec-launcher` that self-confines post-fork in its own address space — the SAME proven
`apply_confinement()` ordering as the worker, `crates/sandbox/src/lib.rs:7-18`), with zero reliance on
async-signal-safety in a `pre_exec` window. Option A is retained only as a documented, not-recommended
alternative. This is the plan-sanctioned resolution (31-02 Task 2 pre-authorized folding Option B as the
pinned path). Consequently the `pre_exec` async-signal-safety residual is now **avoided, not accepted**
on the pinned path (§2.5 reframed; §9 residual retired; A1/A2 updated).

### M1 — MAJOR → §2.4 (RESOLVED)

**Claim.** The doc mandates extending Gate 3 with a `mint_from_exec(` check restricted to
`{quarantine.rs, server.rs}`, yet also tells implementers to mirror the `sinks/file_create.rs` sink
convention — which would place the exec-output mint in a `sinks/process_exec.rs` module NOT in the
Gate-3 allow-list, so the mandated gate would fail the legitimate call site.

**Code evidence (orchestrator re-verified).** `scripts/check-invariants.sh:133-135` — Gate 3's sanctioned
loci for mint tokens are `crates/brokerd/src/quarantine.rs` and `crates/brokerd/src/server.rs` (plus
`value_store.rs` for `.mint(` only). The live `mint_from_read` production call lives in `server.rs`'s
claims-report arm, not in a sink module.

**Resolution (folded, §2.4).** Pinned the `mint_from_exec` call-site locus explicitly to `server.rs` (the
exec-output capture point, mirroring `mint_from_read`'s production locus), NOT the exec sink module. The
division is stated: the sink module owns spawn + confinement handoff + the two-phase
`process_exited`/`process_spawn_failed` audit; `server.rs` owns the `mint_from_exec` of the captured
output. The mandated Gate-3 allow-list and the code structure now agree.

### M2 — MAJOR → §4.2, §6 (RESOLVED)

**Claim.** §4.2 said `process.exec` command/args are "explicitly NOT `expected_role = None`," but
`expected_role` drives an independent structural Step-1c Deny (a `None` origin_role fails closed).
Mandating `Some(...)` for command/args — which have no `origin_role`-producing mint site — would
fail-closed-Deny the LEGITIMATE command; meanwhile `None` is *sufficient* for the security property (a
tainted command still Blocks via sensitivity+taint).

**Code evidence (orchestrator re-verified).** `crates/executor/src/lib.rs:133-148` — the `expected_role`
check is a structural role-Deny (`Some(role)` must be in the expected list; `None` → `false` → Deny),
independent of the Step 2/3 sensitivity+taint Block at `lib.rs:156-158`. `sink_sensitivity.rs:163-176` —
HARDEN-05 navigated this exact trap for `file.create` `contents` by reusing the `"path"` role rather
than inventing an ungrounded one.

**Resolution (folded, §4.2, §6).** Pinned `command`/`args` at `expected_role = None` (not role-checked at
Step 1c) and clarified that the Block is delivered by their `is_routing_sensitive`/`is_content_sensitive
= true` classification + the untrusted-taint check. §6's No-I2-bypass item now states `None` disables
only the structural role gate, never the sensitivity+taint Block — so it is NOT an I2 bypass.

### m1 — MINOR → §3.1 (RESOLVED, noted)

The pinned `RequestFd` read-count upper bound modifies the EXISTING single-read path, not only new
multi-file code. §11 records this so Phase 33 does not treat it as additive-only. (`server.rs:1229-1394`
re-verified: no count/limit token; the `ProvideIntent`-once guard has no `RequestFd` analog.)

### n1 — NIT → §11 (RESOLVED, noted)

Adding `TaintLabel::ExecRaw` forces an update to every non-wildcard `match` over `TaintLabel`
(compiler-caught), not only `is_untrusted()` (`plan_node.rs:40-50`, no-wildcard match, 8 variants).
Phase 32 note recorded in §11.

---

## Verified as sound (reviewer traced real code; not deferred trust)

The reviewer confirmed, by tracing live code, that the following load-bearing claims are accurate — this
is the evidence the review was a genuine code-trace, not a prose skim:

- **Worker seccomp denies execve unconditionally** → child must be broker-spawned:
  `seccomp.rs:64-66` (`SYS_execve`/`SYS_execveat`, empty-vec always-match, `Errno(EPERM)`).
- **`deny_all_filesystem()` unusable verbatim for the exec child:** `landlock.rs:17-32` —
  `AccessFs::from_all(V3)` (includes `Execute`) with zero allow-rules; would block the target binary load
  exactly as §1.4 argues.
- **Reused seccomp net-deny / rlimits accurate:** socket `AF_INET`/`AF_INET6` deny (`seccomp.rs:68-93`);
  `RLIMIT_AS`/`RLIMIT_CPU` with "wall-clock unlimited; CPU-time bounded" (`rlimits.rs:5,13-24`) — the
  wall-clock gap §1.4 flags is real and the NEW `tokio::time::timeout` is warranted.
- **v1.4 spawn + `child.kill()` teardown the exec model mirrors:** planner sidecar spawn
  `main.rs:311-332`, worker spawn `main.rs:334-357`, `child.kill()/wait()` teardown `main.rs:372-378`.
- **`mint_from_exec` mirror is faithful (non-stapled taint):** `mint_from_read` builds the Event first
  (`quarantine.rs:360-369`), appends it (`:372`), then mints with `provenance_chain=[event_id]`
  (`:382-389`); fail-closed unknown `claim_type` (`:354-358`); anchor-identity test (`:856-880`).
- **Gate 3 greps only the three tokens:** `check-invariants.sh:133-135` (`mint_from_read(`,
  `mint_from_derivation(`, `.mint(`) — the doc's load-bearing "would NOT catch `mint_from_exec(`" claim
  is accurate (drives the mandated Gate-3 extension + M1's locus pin).
- **Both new sinks addable by table entries only:** `submit_plan_node` collect-then-Block loop
  (`lib.rs:54-255`) + `KNOWN_SINKS` (`sink_schema.rs:40-58`) + sensitivity/role tables
  (`sink_sensitivity.rs:40-181`). No new decision path; command/args Block via `lib.rs:156-158`.
- **fs write/edit is a correct sibling (attack point d — sound):** `create_exclusive_within` uses
  `O_CREAT|O_EXCL|O_WRONLY` (`workspace.rs:140`); `O_WRONLY|O_TRUNC` (no `O_CREAT/O_EXCL`,
  ENOENT-on-missing) is the correct existing-file-only counterpart, same
  `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`. Existing negative tests cover only `O_RDONLY`/`O_CREAT|O_EXCL`
  (`workspace.rs:223,242,268`) — the "negative-tests-not-inherited" mandate is real; the
  O_CREAT-reintroduction warning is precise and well-pinned.
- **Two-phase durable audit template:** `file_create.rs:84-115` (`sink_executed` on success;
  `sink_execution_failed` appended first on error; `actor = sink:{id}:{effect_id}`).
- **No raw `EffectRequest` path / I0 class-deny:** Gate 1 (`check-invariants.sh:31`); both sinks
  `CommitIrreversible` → Draft/non-live deny at `lib.rs:215-252`.
- **No `RequestFd` read-count limiter today:** `server.rs:1229-1394` (grep-empty), confirming §3.1's
  fail-closed "add an explicit upper bound" pin is genuinely new behavior.

---

## No-TCB-code reconfirmation (DESIGN-13 hard gate)

Re-verified at gate-clearance time: `git status --porcelain crates/ cli/` is empty (only `planning-docs/`
and `.planning/` changed this phase); no new mechanism symbols exist under `crates/` — `process_exited`,
`mint_from_exec`, `TaintLabel::ExecRaw`, the exec-child Landlock ruleset (`exec_child_ruleset`), the
`exec_child_filter`, and the fs write/edit sink appear ONLY in the DESIGN-doc prose, never under
`crates/` or `cli/`. `scripts/check-invariants.sh` is green (all 4 gates PASS, exit 0). The two new
primitives remain design-only until Phases 32–33 implement them.

---

## Verdict

**CLEAR-WITH-AMENDMENTS → CLEARED.** No load-bearing mechanism was fundamentally unsound (not a FAIL),
but the fresh non-self code-trace caught one genuinely unrealizable security default (B1 — a stateless
BPF cannot deliver the pinned seccomp recursion-deny) and one flawed evidentiary basis for the riskiest
design decision (M3 — Option A's `pre_exec` path pinned on non-transferable evidence). Both were resolved
by TIGHTENING the design: B1 by documenting the real Landlock+persistent-net-deny bound instead of a
false seccomp claim; M3 by flipping the pinned spawn model to the strictly-sounder Option B (launcher
post-fork self-confinement, the proven worker ordering), which also retires the `pre_exec`
async-signal-safety residual. M1 and M2 removed a latent gate/mint-locus contradiction and an
under-grounded `expected_role` requirement that would have fail-closed-Denied the legitimate command. No
invariant was weakened and no I2 discipline was relaxed. This is the sixth consecutive milestone in which
a fresh non-self adversarial code-trace caught a real issue the author's own read approved. **Phases
32–34 are authorized.**
