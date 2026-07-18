# DESIGN — Effect Breadth II: `git.commit`, `git.push`, `github.pr`, read-only `http.request` GET

**Milestone:** v1.8 — Git/GitHub Adapters (Effect Breadth II)
**Phase:** 35 (Design Gate) — blocks all `crates/executor` / `crates/brokerd` /
`crates/sandbox` / `crates/runtime-core` code for this milestone
**Status:** Draft → pending fresh (non-self, orchestrator-owned) adversarial
code-trace (DESIGN-16, Plan 35-02; recorded in
`planning-docs/DESIGN-GATE-RECORD-v1.8.md`)
**Author date:** 2026-07-18
**Grounding:** `.planning/phases/35-design-gate-fresh-adversarial-code-trace/35-CONTEXT.md`
(the AUTHORITATIVE decisions this doc transcribes + elaborates) and
`.planning/research/{SUMMARY,ARCHITECTURE,PITFALLS,STACK}.md`. Every `file:line`
below traces to a direct code read this session; re-verify if Phases 36-40 begin
many commits later, per this project's own convention.
**Requirements:** DESIGN-15 (this doc) → enables GIT-01 (Phase 36),
HTTP-01..03 (Phase 37), GITHUB-01..04 (Phase 38), GIT-02/03 (Phase 39),
ENV-01 + LIVE-03/04 (Phase 40).

> **Design-gate discipline.** No `crates/executor` / `crates/brokerd` /
> `crates/sandbox` / `crates/runtime-core` code for any of the four v1.8 sinks
> may be written until this document clears a fresh, non-self adversarial
> code-trace with every finding resolved — mirroring v1.0 Phase 2, v1.2 Phase 8,
> v1.3 Phase 12, v1.4 Phase 18, v1.5 Phase 23, v1.6 Phase 26, v1.7 Phase 31.
> This doc pins **decisions**, not options — the AUTHORITATIVE forks are already
> decided in `35-CONTEXT.md`; Phases 36-40 are a mechanical realization of what
> is fixed here. `git.push`'s network-from-a-confined-child path is the riskiest
> new surface shipped to date, so this doc pins the model precisely enough that
> the fresh reviewer can trace every claim against real code.

---

## §0. Purpose & Scope

**What this doc pins (DESIGN-15).** The dispatch mechanism + fail-closed default
for all four new external-effect sinks, before any TCB code exists:

1. **`git.commit`** — Pattern B (broker-spawned confined child), effect-class
   `MutateReversible` (§1).
2. **`git.push`** — Pattern B extended to a **net-allowed** confined child
   (FORK 1), effect-class `CommitIrreversible` (§2).
3. **read-only `http.request` GET** — Pattern A (in-broker/broker-helper
   egress), effect-class `Observe`, plus the one genuinely NEW mechanism this
   milestone introduces: `mint_from_http` + `TaintLabel::HttpRaw` + session
   demotion (§3).
4. **`github.pr`** — Pattern A, effect-class `CommitIrreversible`, plus the
   session-scoped human auth-grant (FORK 3) and the duplicate-PR CAS (§4).
5. The rustls **crypto provider** decision (FORK 2) and the `env_clear()`
   webpki-roots TLS-cert allowlist policy (ENV-01) (§5).

It then closes all **11 design-gate-blocking pitfalls** each with a NAMED
mechanism (§6), proves the design weakens **no** invariant (§7), gives the
fail-closed defaults table (§8), mandates the P33/P34 confirm-release discipline
(§9), summarizes the new symbols + the mandated Gate 3 extension (§10), lists
open items + accepted residuals (§11), and states the acceptance predicate
(§12).

**The two shipped dispatch patterns the whole design rests on.** Nothing here
introduces a third pattern, and nothing introduces a raw effect-request-to-sink
path:

- **Pattern A — in-broker / broker-helper network egress.** Exemplar
  `crates/brokerd/src/sinks/email_smtp.rs` — the ONLY code path in the TCB that
  performs an actual SMTP call today (`email_smtp.rs:1-5`), broker-resident,
  never confined-worker-resident. Any secret/endpoint is read from broker-local
  process env ONLY (`email_smtp.rs:87-112`, D-04) — never a `ValueNode`,
  plan-node arg, audit-DAG literal, the confined worker, or the planner sidecar.
  Owns `http.request` (§3) and `github.pr` (§4).
- **Pattern B — broker-spawned confined child.** Exemplar
  `crates/brokerd/src/sinks/process_exec.rs` + `cli/caprun-exec-launcher`: the
  broker spawns the launcher (never the worker — the worker's own seccomp filter
  denies `execve` unconditionally), the launcher self-confines (rlimits →
  Landlock exec-child ruleset → seccomp exec-child filter) in its OWN address
  space THEN self-replaces via `execve` into the target (`process_exec.rs:1-8`,
  Option B, DESIGN-effect-breadth-exec.md §1.3). Owns `git.commit` (§1) and
  `git.push` via a net-allowed variant (§2, FORK 1).

**Explicitly DEFERRED to v1.9+ (locked in `35-CONTEXT.md` <specifics>, do NOT
build this milestone):** `http.request` write egress / POST, PR merge/comment,
a real LLM planner loop, a declarative policy file, `git2`/`gix`, `octocrab`.
The git ops shell out to the system `git` binary via the v1.7 exec-launcher
(zero new TCB deps); `github.pr` + `http.request` use raw `reqwest =0.13.4`
(rustls), not `octocrab`.

**Locked terminology (unchanged by this doc):** `Intent`, `Session`, `Planner`,
`Worker`, `Broker`, `Adapter`, `Effect`, `Artifact`, `Event`.
`ExecutionContext` remains internal-only. Nothing here introduces new public-API
vocabulary.

**No TCB code this phase.** This doc lives entirely under `planning-docs/`. The
git diff for Plan 35-01 touches only
`planning-docs/DESIGN-git-github-http-sinks.md`. `scripts/check-invariants.sh`
stays green (its prose under `planning-docs/` cannot trip any Gate that scans
`crates/` or `cli/`).

---

## §1. `git.commit` — Pattern B, `MutateReversible` (GIT-01)

### 1.1 Dispatch — Pattern B, exec-launcher reuse (near-verbatim)

`git.commit` is dispatched exactly like `process.exec`: the broker spawns the
v1.7 `caprun-exec-launcher` as an async, cancellable child via
`tokio::process::Command`; the launcher self-confines (rlimits → Landlock
exec-child ruleset → seccomp exec-child filter) in its own address space, THEN
`execve`s the system `git` binary (`process_exec.rs:1-8`, Option B). The
confined worker NEVER `execve`s git — its seccomp filter denies `execve`
unconditionally. This is the SAME `run_launcher` path already shipped
(`process_exec.rs:381-477`): `env_clear()` + minimal `SAFE_EXEC_PATH`
(`process_exec.rs:379,400-401`), `Stdio::piped()` capture, wall-clock timeout
(`process_exec.rs:75,453`), combined-output byte cap (`process_exec.rs:67`),
`kill_on_drop(true)` (`process_exec.rs:424`). `git.commit` adds no new spawn
machinery — it is a `process.exec` whose `command` is the resolved `git` path
and whose `args` are the commit argv.

### 1.2 Effect-class — `MutateReversible` (a deliberate, justified exception)

`git.commit` is pinned `MutateReversible` — a **deliberate, explicitly-justified
exception** to the fail-closed `unknown → CommitIrreversible` default that
`sink_effect_class` applies to every unregistered sink
(`sink_sensitivity.rs:40-58`, the `_ => EffectClass::CommitIrreversible` arm). A
local commit is **reversible** (`git reset`, `git commit --amend`, branch
deletion) and causes NO external effect — only `push`/`pr` leave the trust
boundary. This mirrors the locked 3-class `Effect` ontology
(`runtime-core/src/effect.rs:36-40`: `Observe` / `MutateReversible` /
`CommitIrreversible`; `ReversibleEffect` already lists
`ApplyPatch`/`EditWorkspaceFile`, `effect.rs:17-21`). Consequence:
`git.commit` **survives an I1-demoted (draft-only) session** — a session that
read untrusted content can still record local work, exactly as it can already
`file.write` a reversible workspace edit. The `MutateReversible` classification
is a NEW `sink_effect_class` arm (`sink_sensitivity.rs:40-58`) — the FIRST
non-`CommitIrreversible` real sink — so Phase 36 must add
`"git.commit" => EffectClass::MutateReversible` and a test asserting it (the
existing `test.observe` fixture arm at `sink_sensitivity.rs:55-56` is the only
current non-`CommitIrreversible` mapping, and it is test-only).

### 1.3 Sink args + I2-sensitivity — the commit message is the taint carrier

`git.commit`'s args (pinned shape; exact schema is a Phase 36 `KNOWN_SINKS` row):
- **`message`** — the commit message. Classified **content-sensitive** (reuse
  the CONTENT-01 discipline: `EMAIL_SEND_CONTENT_SENSITIVE` /
  `FILE_CREATE_CONTENT_SENSITIVE`, `sink_sensitivity.rs:80,87`). This is the
  taint **CARRIER** that must genuinely propagate downstream and MUST NEVER be
  re-minted clean: a tainted `message` (e.g. assembled from untrusted file
  content or exec output) Blocks under the UNMODIFIED `submit_plan_node`
  collect-then-Block loop, exactly like a tainted `email.send` `body`.
- **paths / pathspec** (if modeled) — routing-sensitive, reusing the `path`
  role vocabulary (`expected_role` `Some(&["path","relative_path"])`,
  `sink_sensitivity.rs:197`).

### 1.4 Taint flow — no new mint site (git IS an exec under Pattern B)

git output reuses the EXISTING exec-output taint label. Because `git.commit`
runs through the exec-launcher, its captured stdout/stderr is minted by the
already-shipped `mint_from_exec` (`quarantine.rs:838-853`), rooted on the
`process_exited` Event the sink module appends
(`process_exec.rs:160-176`), carrying `vec![ExternalUntrusted, ExecRaw]` and
`origin_role = Some("exec_output")`. **No new mint site, no new
`TaintLabel` variant for git** — git is an exec, and its output is exec output.
(Only `http.request`, §3, introduces a genuinely new mint site.)

### 1.5 Confinement + git config/hook neutralization (closes P2 — RCE)

The launcher hardcodes a neutralized git environment so a **planted malicious
`.git/config` or hook in the workspace repo does NOT execute**. This rides on
the launcher's existing `env_clear()` (`process_exec.rs:400`) plus these
git-specific settings, all pinned by `35-CONTEXT.md` decision 2:
- `GIT_CONFIG_NOSYSTEM=1` — ignore `/etc/gitconfig`.
- `GIT_CONFIG_GLOBAL=/dev/null` — ignore `~/.gitconfig`.
- `-c core.hooksPath=/dev/null` — no repo hooks fire (pre-commit, etc.).
- **no aliases** — a neutralized config cannot define an alias that shells out.
- `GIT_TERMINAL_PROMPT=0` — never block on an interactive credential prompt.
- `env_clear()`'d child — inherits NONE of the broker's env (no
  `OPENAI_API_KEY`, no `CAPRUN_SMTP_*`), matching `run_launcher`'s existing
  guarantee (`process_exec.rs:390-401`).

Landlock is confined to the workspace repo (the exec-child ruleset already
grants `ReadFile`+`WriteFile` on `WorkspaceRoot` only). seccomp **network-deny
is UNCHANGED** — a local commit needs no network, so `git.commit` reuses the
exec-child filter's `socket(AF_INET/AF_INET6)` deny verbatim. Assumed host
binary floor: **git ≥2.30** (a Phase 36 deployment constant, §11).

---

## §2. `git.push` — Pattern B net-allowed confined child, `CommitIrreversible` (FORK 1)

### 2.1 FORK 1 — RE-DECIDED at the gate: broker-mediated egress, child stays net-denied

> **⚠ FORK-1 CORRECTION (design-gate-driven, orchestrator, 2026-07-18 — FLAG FOR BEN).**
> `35-CONTEXT.md` FORK 1 originally decided "net-allowed confined child." The fresh
> non-self adversarial code-trace (BLOCKER-1, recorded in `DESIGN-GATE-RECORD-v1.8.md`)
> proved that decision **unsound as a seccomp relaxation**: seccomp-bpf gates syscall
> *numbers + scalar registers* only — `connect()`'s destination is a `struct sockaddr *`
> **behind a pointer seccomp cannot dereference**, and `socket()` exposes only the address
> *family*. So the only relaxation seccomp can make is all-or-nothing "stop denying
> `AF_INET`," which grants **arbitrary egress** to a child holding a live push credential —
> exactly the exfiltration primitive the taint model exists to defeat. Landlock cannot
> help either: `LANDLOCK_ACCESS_NET_CONNECT_TCP` needs ABI V4 / kernel **6.7** (above the
> project's 5.13 floor) and filters by **port only, never destination IP**
> (`crates/sandbox/src/landlock.rs:20` pins ABI V3). **A per-destination pin is provably
> not expressible in seccomp or Landlock at the kernel floor.** FORK 1 is therefore
> re-decided below.

**FORK 1 is RE-DECIDED = broker-mediated egress with the git.push child kept FULLY
net-denied (NO seccomp relaxation).** The destination pin lives in the **broker's
application-layer resolve-and-pin egress path** — the SAME model §3.6 uses and which the
reviewer confirmed IS sound (application-mediated, not kernel-syscall-filtered) — never in
seccomp. The exec-child seccomp net-deny (`sandbox/src/seccomp.rs` `exec_child_filter`,
`socket(AF_INET/AF_INET6)` → `EPERM`) is **UNCHANGED** for `git.push`, identical to
`git.commit` (§1.5). Rationale still honored: the push credential + network leg are
broker-mediated (keep the child incapable of arbitrary egress), honoring "keep the broker
small; broker bugs = full compromise" (CLAUDE.md residual-risks + DEC-layer-roles) — the
broker mediates the *destination policy* (a tiny, auditable resolve-and-pin check) while
the child does only local git plumbing.

**Mechanism CLASS pinned here (the security control):** git.push's network leg reaches
ONLY the single pinned, trusted-intent-sourced remote endpoint, enforced by the
BROKER (application-layer resolve-and-pin, §3.6 model), never by the child directly. The
child cannot open an arbitrary `AF_INET` socket. The precise fully-unprivileged realization
is a Phase-39 decision among: (a) a broker-side egress proxy the child must route through,
with the child placed in a per-push network namespace whose only exit is that proxy and an
nftables/route egress filter pinning the resolved remote IP:port (netfilter DOES see the
destination; seccomp does not); or (b) the broker performs the network transfer itself
(resolve-and-pin, Pattern A) and the child does only local pack generation. **HARD
CONSTRAINT on Phase 39:** the destination pin MUST be enforced by a broker/netfilter layer
that can actually see the destination — it may NEVER be claimed of seccomp — and **if no
fully-unprivileged destination-pinning mechanism proves feasible, `git.push` is DEFERRED to
a later milestone rather than shipped with arbitrary child egress.** This is the riskiest
surface in the project to date; the gate now pins a SOUND control locus (BLOCKER-1 resolved).

`git.push` reuses `git.commit`'s Pattern-B *local* dispatch (§1.1) — the child is
net-denied exactly like `git.commit`; only the broker-mediated egress layer differs.
Effect-class = pinned `CommitIrreversible` (`sink_sensitivity.rs`, matching the existing
`IrreversibleEffect::GitPush { remote, branch }` already in the locked ontology,
`effect.rs:27`).

### 2.2 Destination pinning (closes P3 — swapped-remote push)

The push **remote URL + branch are captured from the TRUSTED intent at session
creation** and passed EXPLICITLY to the child — NEVER resolved from the
untrusted repo's `.git/config` (which a prompt-injected worker could have
rewritten). This mirrors `email_smtp.rs`'s D-04 endpoint sourcing: the SMTP
host/port/from are read from trusted broker-local env, NEVER from any
block-time-writable field (`email_smtp.rs:43-56,87-112`), because "sourcing it
from writable state would let a tamperer redirect a confirmed send to an
uncovered destination." Same principle: `remote` + `refspec` are the routing
identity, sourced from trusted intent, and are **I2-gated sink args** (§8).

### 2.3 Broker-mediated egress, child net-denied (closes P12 — net-deny widening)

The confined **WORKER never gains net** — unchanged and non-negotiable — AND the
**`git.push` child ALSO stays fully net-denied** (BLOCKER-1 correction, §2.1): there is
NO seccomp relaxation, because seccomp provably cannot pin a destination (it filters
syscall families/scalars, not the `connect()` sockaddr behind a pointer), so a relaxation
would grant arbitrary egress to a credential-bearing child. The exec-child filter's
`socket(AF_INET/AF_INET6)` → `EPERM` deny is reused **verbatim** from `git.commit`
(`sandbox/src/seccomp.rs` `exec_child_filter`); Landlock stays workspace-confined.

The single-pinned-destination guarantee is enforced by the **broker's application-layer
resolve-and-pin egress layer** (§2.1, §3.6 model) — a locus that can actually see the
destination IP:port — not by the child. The exact fully-unprivileged realization
(broker egress proxy + per-push netns/nftables egress filter, or broker-performed
transfer) is Phase 39 (§11), under the HARD CONSTRAINT in §2.1: the pin lives in a
broker/netfilter layer, never seccomp; git.push is deferred rather than shipped with
arbitrary child egress if no unprivileged mechanism proves feasible. Contrast §1.5:
`git.commit` needs no egress at all; `git.push`'s egress is broker-mediated to the one
pinned remote.

### 2.4 `--force` / destructive-refspec hard-denial (closes P4)

Hard-denied **regardless of confirmation**: no `--force` / `--force-with-lease`,
no `:refspec` deletion (push-to-delete), no `+`-prefixed force-refspec. These
are refused by construction at the sink's arg-validation, never reachable even
via a human confirm (a human confirms a *specific* push, not a license to
rewrite history). `remote` + `refspec` are I2-gated sink args, so a tainted
value in either also Blocks (§8).

### 2.5 Credential injection (closes P5 — credential leak/flow)

The push credential lives in **broker-local env ONLY** (same custody model as
`email_smtp.rs`'s D-04 secrets), and is **never** a `ValueNode`, plan-node arg,
audit-DAG literal, the confined worker, or the planner sidecar. It is injected
**short-lived** to the git.push child via a `GIT_ASKPASS`/env visible ONLY to
that child, and `env_clear()`'d otherwise — riding the existing
`run_launcher` `env_clear()` + explicit-allowlist discipline
(`process_exec.rs:390-402`), which already proves the confined child inherits
NONE of the broker's secrets (the `run_launcher_env_clear_prevents_broker_secret_leak`
test, `process_exec.rs:820-871`). The credential is the ONE explicitly-injected
non-`SAFE_EXEC_PATH` env var, scoped to the push child alone.

**Captured-output scrub discipline (closes MAJOR-3 — credential-in-output leak).**
Unlike `git.commit`, `git.push`'s child performs a network exchange whose stderr routinely
echoes endpoint/credential-adjacent material a local commit never does — proxy-auth
failures (`Received HTTP code 407 from proxy after CONNECT`), redirect/URL echoes on auth
failure, and `GIT_ASKPASS` does not guarantee the token never appears in a diagnostic
line. Pattern B's default (`process_exec.rs:381-461` capture → `mint_from_exec` mint,
`quarantine.rs:838-853`) would place that text in a taint-tracked `ValueRecord` holding
the literal. Therefore `git.push` MUST follow `github.pr`'s opaque-payload discipline
(§4.2), NOT `process.exec`'s mint-the-output default: the push child's captured
stdout/stderr is **either not minted at all** (only a broker-side `git_push_succeeded`/
`_failed` opaque event with `effect_id`, mirroring `email_smtp.rs:256-301`) **or scrubbed
of any `https://…@…` userinfo / proxy-auth / token material before minting** — Phase 39
pins which, and a regression test asserts no credential/URL substring survives into the
value store or the audit chain. `git.push` does NOT silently inherit `process.exec`'s
mint-the-output behavior.

### 2.6 Confirm-release (P33/P34 class — see §9)

A tainted push `remote`/`refspec` Blocks at the sink under I2 and is releasable
ONLY by single-shot human confirmation. The confirm-release path MUST write the
TERMINAL AUDIT EVENT **before** the terminal state, via a `prepare_git_push`
precheck — the exact discipline `process.exec` already implements with
`prepare_process_exec` at confirm()'s Step 4.8 (`confirmation.rs:847-866`),
which folds every fallible pre-spawn leg through the single terminal-event
branch so a burned one-shot confirmation can never dangle without a
`process_spawn_failed`/terminal event (`process_exec.rs:265-333`). Full mandate
in §9.

### 2.7 Pushed-payload visibility at confirm (closes MAJOR-2 — invisible-payload exfil)

I2 mediates values ROUTED INTO sink args. `git.push`'s args are `remote` + `refspec`
only — **the commit contents being pushed are not a plan-node arg**, so absent the
mechanism below, the human confirming a push would see only the destination, not what
leaves the boundary. That breaks the "human sees exactly what would leave the boundary"
property this design relies on for every other exfil sink (§4.4 shows verbatim PR
title/body; email.send shows the body). Committed-tree content can enter the workspace via
`file.write` (I2 content-sensitive on `contents`) and be staged by `git.commit` — but a
worker holding a workspace fd could write file bytes that were never themselves an
I2-gated sink arg, then commit + push them. **So the git.push confirm MUST surface the
PAYLOAD, not just the destination:** at the `git.push` confirm prompt the human is shown
(a) the pushed commit range / branch, and (b) a **provenance summary of the pushed diff
flagging any file whose content derives from untrusted taint** (via the same value-store
provenance the executor already tracks). The payload — not just `remote`/`refspec` — is
what the human authorizes. Phase 39 pins the exact rendering (commit-range + tainted-file
provenance list); the §7/§4.4 "human sees what leaves the boundary" claim is thereby made
literally true for push, not merely implied. (Accepted residual, §11: this surfaces
provenance/diff for human judgment; it does not by itself Block untainted-but-sensitive
content — that remains the human's call at confirm, consistent with the confirmation model.)

---

## §3. read-only `http.request` GET — Pattern A + `mint_from_http` (HTTP-01..03)

### 3.1 Dispatch — Pattern A, `reqwest` (not `octocrab`)

The HTTP client runs in the broker / a broker-helper (Pattern A), NOT in a
confined child and NEVER in the worker — the same in-broker-egress shape as
`email_smtp.rs`, the project's only existing network-egress sink
(`email_smtp.rs:1-5`). Stack: raw `reqwest =0.13.4` (rustls), NOT `octocrab`
(`35-CONTEXT.md` Stack). Only the GET method is in scope this milestone; POST /
write egress is deferred to v1.9+ (§0).

### 3.2 Effect-class — `Observe`, but the response demotes the session

`http.request` GET is pinned `Observe` (`sink_sensitivity.rs:18-22` EffectClass;
the FIRST real `Observe` sink — `test.observe` is today the only `Observe`
mapping and it is test-only, `sink_sensitivity.rs:55-56`). A GET is therefore
**allowed even in an I1-demoted (draft-only) session** — reading is not an
external mutation. BUT its inbound response is untrusted-on-arrival and **demotes
the session** the moment it is minted (§3.3): fetching untrusted bytes is
exactly the I1 direction (mirroring `mint_from_read`'s session demotion,
`quarantine.rs:391-401`).

### 3.3 The one genuinely NEW mechanism — `mint_from_http` (closes P8)

`mint_from_http` is a NEW mint site in the sanctioned
`crates/brokerd/src/quarantine.rs` locus, mirroring `mint_from_read`
(`quarantine.rs:301`) and `mint_from_exec` (`quarantine.rs:838`) in shape. It
MUST, in this exact order (the non-stapled-taint genesis, `quarantine.rs:316-389`):

1. **Append a real `http_response_received` audit `Event` FIRST** (a NEW event
   type, analogous to `mint_from_read`'s `file_read` at `quarantine.rs:361-369`
   and `mint_from_exec`'s `process_exited` root at `process_exec.rs:160-176`),
   obtaining its id + row hash via `append_event`.
2. **THEN mint the inbound response body** as untrusted-on-arrival via
   `ValueStore::mint`, with `provenance_chain = [that Event's id]`
   (non-stapled — `provenance_chain[0]` EQUALS the `http_response_received`
   Event id, exactly like `mint_from_read`'s anchor identity guarantee,
   `quarantine.rs:374-389` + the `mint_from_read_anchor_identity` test at
   `quarantine.rs:917-928`). Taint vector: `vec![ExternalUntrusted, HttpRaw]`
   (mirroring `mint_from_exec`'s `vec![ExternalUntrusted, ExecRaw]`,
   `quarantine.rs:848`); `origin_role = Some("http_response")`.
3. **Demote the session to draft-only (I1)** — DECIDED YES (`35-CONTEXT.md`
   mint_from_http). Same atomic in-`conn` demotion `mint_from_read` performs at
   `quarantine.rs:391-401` (`UPDATE sessions SET status='Draft'` + a
   `session_demoted` Event chained onto the mint event) — never a second lock,
   never stapled.

### 3.4 `TaintLabel::HttpRaw` — compile-forced into the exhaustive match

A NEW `TaintLabel::HttpRaw` variant is added to the enum
(`plan_node.rs:13-29`, today `UserTrusted`, `LocalWorkspace`,
`ExternalUntrusted`, `EmailRaw`, `PdfRaw`, `LlmGenerated`, `WorkerExtracted`,
`PathRaw`, `ExecRaw`), mirroring the `ExecRaw` naming/pairing convention exactly
(`plan_node.rs:28`). Adding the variant is a **compile-time-enforced** change:
`is_untrusted()`'s exhaustive `match self` with NO wildcard arm
(`plan_node.rs:45-56`, doc-commented "Adding a new `TaintLabel` variant without
updating this match is a compile error, not a silent false-allow") FORCES
Phase 37 to place `HttpRaw` in the untrusted arm — the compiler catches an
omission a runtime default could silently miss.

### 3.5 Anti-staple discipline (closes P11 + stapled-taint)

Taint is genuinely propagated through the DAG: a fetched value routed into a
sensitive sink arg (a PR body, a commit message, an email body) Blocks on a
**real DAG edge** rooted at `http_response_received`, never stapled at the
consuming sink — the same discipline the executor already enforces (it never
mints, never sets taint; it only reads through `value_store.resolve()`). An
**anti-staple test is REQUIRED** (the §9/§12 genuineness standard): assert that
`store.resolve(fetched_value_id).provenance_chain[0]` equals the
`http_response_received` Event id AND that a downstream sensitive-slot routing
of that value Blocks — mirroring `mint_from_read_anchor_identity`
(`quarantine.rs:917-928`).

### 3.6 SSRF resolve-and-pin (closes P7/P10)

The `url` arg is I2-gated (routing-sensitive, §8). The fetch model:
- **Resolve the host, PIN the destination IP**, and connect to that pinned IP
  (with SNI/Host = the original hostname) — closing DNS-rebind TOCTOU.
- **Deny** loopback (`127.0.0.0/8`, `::1`), RFC1918 (`10/8`, `172.16/12`,
  `192.168/16`), link-local (`169.254/16`, `fe80::/10`), CGNAT
  (`100.64/10`), cloud-metadata (`169.254.169.254`), ULA (`fc00::/7`), and
  IPv6-mapped equivalents (`::ffff:0:0/96`).
- **NO redirect following** by default (a 30x cannot bounce to a denied range).
- **Reject** `userinfo@` in the URL, any non-`https` scheme, and IP-encoding
  tricks (decimal/octal/hex-packed addresses).
- **Host allowlist** — the fetch target must be on an explicit allowlist
  (an operator-surfaced deployment constant, §11), never arbitrary.

---

## §4. `github.pr` — Pattern A, `CommitIrreversible` (FORK 3) + duplicate-PR CAS

### 4.1 Dispatch — Pattern A, one REST POST via `reqwest`

`github.pr` runs in the broker (Pattern A), reusing the Phase-37 `http.request`
egress infra: one REST `POST /repos/{owner}/{repo}/pulls` via `reqwest`, with a
broker-held bearer token. Effect-class = pinned `CommitIrreversible`
(`sink_sensitivity.rs:40-58`).

**API base-URL pinning (closes MAJOR-4 — github.pr POST destination SSRF gap).**
§3.6's SSRF resolve-and-pin is written for the GET path; `github.pr` is a POST and MUST
NOT be an unguarded destination. The GitHub API base (`https://api.github.com`) is a
**fixed, broker-owned trusted-config constant**, sourced from trusted broker-local env
exactly like the SMTP endpoint (`email_smtp.rs:87-112`, D-04) — **never** derived from a
resolved/tainted arg, and never from `owner`/`repo`. The POST destination rides the SAME
§3.6 resolve-and-pin + single-entry host allowlist as the GET path (host must resolve to a
public GitHub IP; loopback/RFC1918/link-local/metadata ranges denied; no redirect
following; `userinfo@`/non-`https` rejected). `owner`/`repo`/`base`/`head` being
routing-sensitive (tainted → Block, §4.4) bounds the URL *path*; the base *host* is pinned
here so a UserTrusted-but-attacker-influenced `owner`/`repo` can never redirect the POST to
a non-GitHub host. See the §8 fail-closed row for the github.pr POST destination.

### 4.2 Credential hygiene (closes P5/P8)

The bearer token is read from **broker-local env ONLY** (same D-04 custody as
`email_smtp.rs:87-112`) — never a `ValueNode`, plan-node arg, audit-DAG literal,
the confined worker, or the planner sidecar. Audit events for `github.pr` carry
OPAQUE payloads (only `effect_id` in the `actor` field + a static event-type
marker), mirroring `email_smtp.rs`'s opaque `email_send_succeeded`/`_failed`
convention (`email_smtp.rs:58-65,256-301`) — the token and raw API response text
never enter the hash chain.

### 4.3 FORK 3 — DECIDED: session-scoped capability grant, independent of per-PR confirm

**FORK 3 is DECIDED = session-scoped capability grant, separate from per-effect
confirm** (`35-CONTEXT.md` FORK 3). Rationale: a bearer token's authority far
exceeds one PR (push / merge / cross-repo read), it opens a default-deny-net
hole, and it is a broker-held secret — so **confirming a PR body ≠ authorizing
the credential**. Mechanism — TWO independent gates:

1. **Capability grant (NEW):** a distinct human action `caprun grant <session>
   ...` authorizes the broker to USE the token for that session, recorded as its
   OWN audit event, session-scoped (the grant's lifetime is the Session; it does
   not persist across sessions). Mirrors the v1.4 `ConnectionRole` capability
   precedent.
2. **Per-effect I2 confirm:** the ordinary per-PR I2 confirm of the (sink, args)
   triple (§4.4).

A PR **cannot be created on a bare confirm alone** — absent a live grant the
sink Denies (§8), independent of any per-PR confirm. The exact `caprun grant`
verb + session-scoped lifetime are FINALIZED here as the model; the precise CLI
surface is a Phase 38 detail (§11).

### 4.4 Tainted-PR-body block (the marquee P6-exfil — closes it) + token scoping

- **`title` / `body`** are content-sensitive sink args (reuse CONTENT-01,
  `is_content_sensitive`, `sink_sensitivity.rs:140`), so a value assembled from
  untrusted content (an `http_response`-tainted, `ExecRaw`-tainted, or
  `doc_fragment`-tainted string) Blocks under the UNMODIFIED collect-then-Block
  loop. This is the marquee exfiltration threat: without this, a
  prompt-injected worker could smuggle secrets out through a PR body. Taint is
  genuinely propagated; the **verbatim, provenance-annotated `title`/`body` is
  shown to the human at confirm** (the human sees exactly what would leave the
  boundary).
- **`owner`/`repo`/`base`/`head`** are routing-sensitive (they determine WHERE
  the PR lands), I2-gated.
- **Token scoping (closes P6-overscope):** MINIMAL scopes — a fine-grained PAT
  with `Pull requests: write` + `Contents: read` only — stated as an operator
  responsibility surfaced at grant time (§11).

### 4.5 Duplicate-PR CAS (closes P16 — replay)

A content-derived idempotency key — a digest over
`(owner, repo, base, head, title, body)` — is committed to a CAS table
**BEFORE** the GitHub API call, mirroring v1.6 HARDEN-03's Allowed-path replay
defense (`DESIGN-security-hardening.md` §c, `email_smtp.rs:16-20`): the CAS +
attempt append commit as one atomic unit BEFORE any socket opens; a
PRIMARY-KEY-constraint violation on replay IS the CAS, suppressing the second
call. Result: a replayed identical submission creates **at most one PR**. The
key MUST be **derived** from resolved plan-node content, never carried as a new
`PlanNode` field, and never keyed on `effect_id` (a fresh `effect_id` per
resubmit would defeat it — `DESIGN-security-hardening.md` §c's load-bearing
fact). Accepted scope caveat (D-08, inherited): this is at-most-once PER PLAN
NODE, not a per-session send budget.

### 4.6 Confirm-release (P33/P34 class — see §9)

`github.pr` is `CommitIrreversible` + confirm-releasable: the confirm-release
path writes the TERMINAL AUDIT EVENT before the terminal state via a
`prepare_github_pr` precheck (the `prepare_process_exec` pattern,
`confirmation.rs:847-866`). Full mandate in §9.

---

## §5. Crypto provider (FORK 2) + `env_clear()` TLS-cert allowlist policy (ENV-01)

### 5.1 FORK 2 — DECIDED: lean `ring`

**FORK 2 is DECIDED = lean `ring` (pure-Rust) for the new egress** (`35-CONTEXT.md`
FORK 2). Rationale: "minimize untrusted C in the TCB" — the net egress runs
inside the TCB boundary (broker / confined child), so any new C crypto is a
conscious add; `ring` is pure-Rust and well-audited. The planner sidecar already
ships `aws-lc-rs` but is a SEPARATE process, so no in-process rustls
`CryptoProvider` conflict forces a match. `aws-lc-rs` is ACCEPTABLE if
provider-consistency turns out materially cleaner at Phase 37 — this is
low-stakes either way (both well-audited); the doc picks `ring`, and the
reviewer sanity-checks rather than over-constrains.

### 5.2 `env_clear()` webpki-roots TLS policy (ENV-01, realized Phase 40)

CA roots = compiled-in **`webpki-roots 1.0.8`** (NOT `rustls-platform-verifier`)
so that `env_clear()` is **HERMETIC** — a TLS-egress process needs no
`SSL_CERT_*` env var and no readable system cert store to validate a server
cert. Consequence: the surviving env allowlist for any TLS-egress process is
ONLY `HTTPS_PROXY`/`NO_PROXY` (when behind a proxy) + minimal `PATH`/locale.
This closes the deferred v1.7 planner-sidecar `env_clear()` todo: the planner
sidecar spawn is TODAY **NOT** `env_clear()`'d (`cli/caprun/src/main.rs:314-335`
— it forwards `OPENAI_API_KEY` explicitly at `main.rs:325-327` but inherits the
rest of the broker env by inheritance), UNLIKE the worker spawn which IS
`env_clear()`'d with an explicit allowlist (`main.rs:357-358`). Phase 40 must
`env_clear()` the sidecar the same way. This MUST be validated by a **LIVE HTTPS
run** — the only place a TLS-env regression manifests; offline/mocked tests do
NOT catch a missing-cert-root failure (§11, §12).

---

## §6. Threat Model — 11 Design-Gate-Blocking Pitfalls → Named Mechanism

Each pitfall is closed by a NAMED mechanism, cross-referenced to the § that pins
it. All 11 from `35-CONTEXT.md` (the source the fresh reviewer cross-checks
against `.planning/research/PITFALLS.md`):

| # | Pitfall | Named mechanism | § |
|---|---------|-----------------|---|
| 1 | Tainted PR-body / commit-message exfil (marquee) | `title`/`body`/`message` are I2 content-sensitive sink args (CONTENT-01, `sink_sensitivity.rs:140`); taint genuinely propagated (not stapled); verbatim provenance shown at confirm | §1.3, §4.4 |
| 2 | git config/hook RCE | Hardcoded launcher neutralization: `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `-c core.hooksPath=/dev/null`, no aliases, `GIT_TERMINAL_PROMPT=0`, `env_clear()`'d child (`process_exec.rs:400`) | §1.5 |
| 3 | Swapped-remote push | `remote`+`branch` captured from trusted intent at session creation, passed explicitly to the child — NEVER resolved from the untrusted repo's `.git/config` (D-04 sourcing analog, `email_smtp.rs:43-56`) | §2.2 |
| 4 | `--force` / destructive refspec | Hard-denied regardless of confirmation (no `--force`/`--force-with-lease`, no `:refspec` deletion, no `+` force-refspec); `remote`+`refspec` I2-gated | §2.4 |
| 5 | Credential leak / flow | Token/push-cred in broker-local env ONLY (D-04, `email_smtp.rs:87-112`); never a ValueNode/plan-arg/audit-literal/worker/planner; short-lived injection to the push child only, `env_clear()`'d otherwise (`process_exec.rs:390-402`) | §2.5, §4.2 |
| 6 | Token over-scoping | Minimal fine-grained PAT scopes (`Pull requests: write` + `Contents: read`), operator responsibility surfaced at grant time | §4.4 |
| 7 | http SSRF | Resolve-and-pin the IP; deny loopback/RFC1918/link-local/CGNAT/metadata(`169.254.169.254`)/ULA/IPv6-mapped; no redirects; reject `userinfo@`/non-`https`/IP-encoding; host allowlist | §3.6 |
| 8 | http response not minted / stapled taint | `mint_from_http` at arrival rooted on a real `http_response_received` Event + session demotion (I1); anti-staple test proving the downstream Block | §3.3, §3.5 |
| 9 | net-deny widening | Confined WORKER never gains net; the git.push CHILD also stays fully net-denied (NO seccomp relaxation — seccomp cannot pin a destination, BLOCKER-1); egress is broker-mediated resolve-and-pin to the single pinned remote (§2.1/§2.3 model, §3.6-sound) | §2.1, §2.3, §3.1 |
| 10 | push/PR effect-class | Pinned per §1.2/§2.1/§4.1; `git.commit`'s `MutateReversible` exception explicitly justified | §1.2 |
| 11 | Replay / duplicate-PR CAS | Content-derived idempotency key committed to a CAS table BEFORE the GitHub API call (mirrors HARDEN-03, `DESIGN-security-hardening.md` §c) → at-most-one PR | §4.5 |

---

## §7. Invariant Preservation

Each item is checked with a one-line justification (mirroring
`DESIGN-effect-breadth-exec.md` §6 / `DESIGN-slot-type-binding.md`):

- [x] **I0 unaffected** — no new session-creation path exists in this model; a
  session seeded from external/untrusted content still starts draft-only and
  cannot auto-authorize a `CommitIrreversible` push/PR.
- [x] **I1 preserved AND extended** — `mint_from_http` demotes the session on an
  inbound HTTP response (§3.3), exactly the I1 direction, using
  `mint_from_read`'s atomic in-`conn` demotion pattern
  (`quarantine.rs:391-401`). No sink reads raw untrusted bytes into the worker.
- [x] **I2 NOT weakened or bypassed** — all four sinks are `PlanNode{sink,args}`
  from spawn and route through the UNMODIFIED `submit_plan_node`
  collect-then-Block loop. The ONLY executor changes are table rows
  (`KNOWN_SINKS`, `sink_effect_class`, `is_routing_sensitive` /
  `is_content_sensitive`, `expected_role`) — no new `ExecutorDecision` variant,
  no new enforcement step. Same discipline as v1.5/v1.7.
- [x] **No new raw effect-request-to-sink path** — the plan-node path is
  preserved (DEC-architectural-lock-plan-nodes, `plan_node.rs:1-9`); this doc
  introduces no such token anywhere, so `check-invariants.sh` Gate 1
  (`check-invariants.sh:29-36`) stays green with zero new hits.
- [x] **Sink sensitivity stays HARDCODED in the executor** — the new sinks add
  `is_routing_sensitive` / `is_content_sensitive` / `expected_role` / `sink_effect_class`
  TABLE ROWS ONLY (`sink_sensitivity.rs:40-253`), never a swappable policy file.
  Sensitivity is a security property, not a config knob (CON-i2-non-bypassable,
  `sink_sensitivity.rs:1-8`).
- [x] **Genuine, non-stapled taint** — HTTP taint is minted ONLY inside
  `mint_from_http` at the `http_response_received` Event
  (`provenance_chain[0]` == that Event id); git output via the existing
  `mint_from_exec` (`quarantine.rs:838-853`). The executor never mints, never
  sets taint (it only `value_store.resolve()`s).

---

## §8. Fail-Closed Defaults Table

| Sink arg | Sensitivity | Default posture | Fail-closed behavior |
|---|---|---|---|
| `git.commit` `message` | content-sensitive | taint carrier, never re-minted clean; `MutateReversible` survives I1 | tainted → Block (collect-then-Block); unknown/missing → Deny at Step 0 schema gate |
| `git.push` `remote` | routing-sensitive | from TRUSTED intent only, never repo `.git/config` | tainted → Block; not-from-trusted-intent → Deny |
| `git.push` `refspec` | routing-sensitive | `--force`/deletion/`+`-force hard-denied regardless of confirm | tainted → Block; force/delete shape → hard Deny |
| `http.request` `url` | routing-sensitive **+ content-sensitive** (NIT-6 defense-in-depth) | I2-gated; host allowlist; resolve-and-pin | non-allowlisted host or SSRF range → Deny; tainted (incl. secret assembled into query) → Block |
| `github.pr` POST destination (API base host) | routing (pinned) | fixed broker-owned trusted-config `https://api.github.com`, never from a resolved/tainted arg; rides §3.6 resolve-and-pin | non-GitHub/SSRF-range resolution or redirect → Deny |
| `http.request` response `ValueNode` | untrusted origin | `HttpRaw`+`ExternalUntrusted`, `origin_role="http_response"`; demotes session | unknown/unrecognized shape → fail-closed mint error (mirrors T-07-47), never default-tagged |
| `github.pr` `title`/`body` | content-sensitive (CONTENT-01) | verbatim + provenance shown at confirm | tainted → Block; unknown/missing → Deny at Step 0 |
| `github.pr` `owner`/`repo`/`base`/`head` | routing-sensitive | I2-gated | tainted → Block |
| `github.pr` credential use | capability | requires session auth-grant AND per-effect I2 confirm — BOTH gates | absent grant → Deny (a bare confirm cannot create a PR) |
| duplicate submission (git.push/github.pr) | idempotency | content-derived CAS committed before the API call | CAS hit → at-most-once (no second effect) |
| unregistered sink, or unknown/duplicate/missing arg | structural | `KNOWN_SINKS` exact-match schema, Step 0 gate | Deny at Step 0, before any resolve / sensitivity / role check runs |

---

## §9. Confirm-Release Audit-Gap Discipline (P33/P34 — MANDATORY)

`git.push` and `github.pr` are BOTH `CommitIrreversible` + confirm-releasable.
For EACH, this doc MANDATES: the confirm-release path writes the **TERMINAL
AUDIT EVENT before the terminal state** — NEVER a terminal STATE (e.g.
`Confirmed`, `confirm_granted` appended) before the terminal EVENT that
justifies it (the effect's `..._succeeded`/`..._failed`). Each MUST have a
`prepare_*` precheck — `prepare_git_push` and `prepare_github_pr` — that runs
BEFORE `confirm()` appends `confirm_granted` (Step 5) and burns the one-shot
(Step 6 CAS→Confirmed), folding every fallible pre-effect leg through the single
terminal-event branch. This is the EXACT pattern `process.exec` already ships:
`prepare_process_exec` at confirm()'s Step 4.8 (`confirmation.rs:847-866`),
called by BOTH the precheck and the sink dispatch so they cannot drift
(`process_exec.rs:346-369`), with every `?` leg on the dispatch side folded into
the branch that appends `process_spawn_failed` FIRST (`process_exec.rs:265-333`).

This is the RECURRING MAJOR audit-gap class that a passing verifier + green
gates missed TWICE — v1.7 P33 (file.write) and P34 (process.exec, the exact
"pre-spawn `?` legs burned the one-shot AFTER Step 5/6 with no terminal event"
MAJOR-1) — and that only the fresh adversarial code-trace caught. Phases 38/39
MUST implement `prepare_github_pr`/`prepare_git_push` with a regression test
asserting NO dangling `confirm_granted`-without-terminal-event (mirroring
`confirm_on_process_exec_malformed_args_does_not_burn_confirmation`,
`confirmation.rs:1965`).

**Entry-guard extension (required, from the gate citation audit).** `confirm()`'s
Step-4.75 entry guard carries an explicit per-sink allow-list of confirm-releasable sinks
(`confirmation.rs:824-845`, the `:836-845` match). Phases 38/39 MUST extend that allow-list
to admit `github.pr` and `git.push` — a new confirm-releasable sink that is NOT added there
is denied at the guard (fail-closed), so the extension is a required, not optional, step and
its omission would silently make the new sinks non-releasable.

---

## §10. New-Mechanism Symbol Summary + Gate 3 Mandate

**New symbols the implementation phases introduce** (each appears ONLY in
DESIGN-doc prose this phase, NEVER under `crates/` or `cli/`):

| Symbol | Phase | Locus |
|--------|-------|-------|
| `TaintLabel::HttpRaw` | 37 | `runtime-core/src/plan_node.rs` (compile-forced into `is_untrusted()`) |
| `mint_from_http` + `http_response_received` Event | 37 | `crates/brokerd/src/quarantine.rs` (mint) + `server.rs` (call site) |
| duplicate-PR CAS table (`created_prs` or similar) | 38 | `crates/brokerd/src/audit.rs` migration + `server.rs` |
| session auth-grant event + `caprun grant` verb | 38 | `crates/brokerd` (grant event) + `cli/caprun` (verb) |
| `prepare_git_push` / `prepare_github_pr` | 39 / 38 | `crates/brokerd/src/sinks/*` + `confirmation.rs` precheck |
| `git.commit`=`MutateReversible`, `http.request`=`Observe`, new `KNOWN_SINKS`/sensitivity rows | 36-39 | `crates/executor/src/{sink_schema,sink_sensitivity}.rs` |

**Gate 3 mandate.** `scripts/check-invariants.sh` Gate 3 TODAY restricts exactly
four call-site tokens — `mint_from_read(`, `mint_from_derivation(`,
`mint_from_exec(`, `.mint(` — to the sanctioned loci `quarantine.rs`,
`server.rs` (+ `value_store.rs` for `.mint(`) (`check-invariants.sh:134-137`). A
new `mint_from_http(` call site will **NOT** be caught by Gate 3 as written
today. This doc **MANDATES** that Phase 37 extend Gate 3 with a fifth
`check_mint_token "mint_from_http("` call restricted to the SAME sanctioned loci
(`quarantine.rs`, `server.rs`), in the SAME commit that introduces
`mint_from_http` — exactly as Phase 32 extended it for `mint_from_exec(`. Without
this extension the new mint site's call-site restriction is silently unenforced;
the fresh reviewer must confirm the extension exists before clearing.

---

## §11. Open Items & Accepted Residual Risks

**OPEN (model pinned, deployment constants deferred — NOT model gaps):**

1. **The exact fully-unprivileged git.push destination-enforcement realization.**
   §2.1/§2.3 pin the CONTROL and its locus: the destination pin is **broker-mediated**
   (application-layer resolve-and-pin, §3.6 model), the child stays fully net-denied, and
   the pin is NEVER claimed of seccomp (BLOCKER-1 correction — seccomp cannot see a
   `connect()` destination). This is the **core security control of the riskiest surface**,
   NOT a deployment constant. Phase 39 picks the realization (broker egress proxy + per-push
   netns/nftables egress filter that CAN see the destination, or broker-performed transfer)
   under the HARD CONSTRAINT that the pin live in a broker/netfilter layer, and **defers
   `git.push` entirely rather than shipping arbitrary child egress if no unprivileged
   destination-pinning mechanism proves feasible.**
2. **The exact `caprun grant` lifetime/verb surface.** §4.3 pins session-scoped
   as the model; the precise CLI surface is finalized at Phase 38.
3. **The `webpki-roots` surviving-env allowlist confirmation via a live HTTPS
   run.** §5.2 pins the policy; Phase 40's live run confirms `env_clear()` is
   hermetic (offline/mocked tests do not catch a cert-root regression).
4. **git binary floor** (≥2.30, §1.5) and the **SSRF host allowlist** contents
   (§3.6) — environment-dependent constants resolved at implementation.

**Accepted residual risks (mirroring prior DESIGN docs' convention):**

- **The `git.push` net-relaxation is the riskiest surface in the project to
  date** — explicitly the TOP item for the fresh adversarial review to
  pressure-test (§2.1). A confined child WITH network is a genuinely new trust
  posture; the mitigation (single pinned host:port, no arbitrary egress,
  Landlock still workspace-confined, short-lived credential) is pinned but must
  be traced against real code by the reviewer.
- **An Allowed `http.request` GET in a network-scoped context is inert** unless
  the host allowlist permits it — deliberately scoped, mirroring
  DESIGN-effect-breadth-exec.md §1.6's "an Allowed `curl` in a network-denied
  child is inert" residual.
- **Duplicate-PR CAS is at-most-once PER PLAN NODE, not a per-session budget**
  (D-08, inherited from HARDEN-03) — a statically-compromised worker submitting
  N distinct plan nodes gets N distinct keys.
- **Duplicate-PR CAS crash window (MAJOR-5 → accepted residual):** because the CAS key +
  attempt append commit atomically BEFORE the API call, a crash/failure in the
  CAS-commit → PR-confirmed window orphans the key: a legitimate retry under the SAME key
  hits the PRIMARY-KEY violation and is suppressed (PR **lost, not duplicated**). This is
  the intended at-most-once fail-closed tradeoff, identical to email.send's accepted
  at-most-once (HARDEN-03). Implementers MUST NOT add a "clear the key on failure" path —
  that reintroduces the duplicate-send hole the CAS exists to close.
- **git.push confirm surfaces payload provenance for human judgment, not automated
  content-Blocking (MAJOR-2 → accepted residual):** §2.7 makes the pushed diff +
  tainted-file provenance visible at confirm so the human authorizes the payload, not just
  the destination; it does not by itself Block untainted-but-sensitive committed content —
  that is the human's call at confirm, consistent with the confirmation model.

---

## §12. Acceptance Predicate — Done When

Phase 35's gate is cleared when ALL are true:

1. This doc pins, per sink, the dispatch pattern, effect-class
   (`git.commit`=`MutateReversible`, `git.push`/`github.pr`=`CommitIrreversible`,
   `http.request` GET=`Observe`), the I2-sensitive sink args, the taint flow,
   and the confinement (§1-§4). **(DESIGN-15, this plan.)**
2. This doc pins the `mint_from_http` inbound-taint + session demotion, the
   `TaintLabel::HttpRaw` variant, the git config/hook neutralization, git.push
   destination-pinning + credential injection, the SSRF resolve-and-pin model,
   the github.pr session-scoped auth-grant, the env_clear() webpki-roots TLS
   policy, and the duplicate-PR CAS (§1-§5). **(DESIGN-15, this plan.)**
3. This doc closes all 11 design-gate-blocking pitfalls with a NAMED mechanism
   each (§6); nothing disables/bypasses I2 and no new raw effect-request-to-sink
   path is introduced; sink sensitivity stays hardcoded in the executor (§7);
   the P33/P34 confirm-release discipline (`prepare_git_push`/`prepare_github_pr`)
   is mandated (§9); and the Gate 3 `mint_from_http(` extension is mandated
   (§10).
4. `scripts/check-invariants.sh` exits 0 against this doc's presence (no
   architectural-invariant regression from its prose).
5. This doc has cleared a fresh, non-self adversarial code-trace (traced against
   real code, not prose-read) with every finding resolved, recorded in
   `planning-docs/DESIGN-GATE-RECORD-v1.8.md` (Plan 35-02) — and no
   `crates/executor` / `crates/brokerd` / `crates/sandbox` / `crates/runtime-core`
   code exists yet (`git diff` touches only `planning-docs/` + `.planning/`).

---
