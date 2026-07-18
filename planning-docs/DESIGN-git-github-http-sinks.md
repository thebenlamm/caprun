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

### 2.1 FORK 1 — DECIDED: net-allowed confined child (Pattern B extended)

**FORK 1 is DECIDED = net-allowed confined child** (`35-CONTEXT.md` FORK 1).
Rationale, verbatim-in-substance: keep the broker small — a push credential +
network egress live inside a **short-lived, kernel-confined child**, not the
long-lived reference monitor, honoring "keep the broker small; broker bugs =
full compromise" (CLAUDE.md residual-risks + DEC-layer-roles). The in-broker
`git push` alternative is **REJECTED**: it puts an unconfined git subprocess as
a child of the broker and widens the reference monitor. **The net-relaxation is
the riskiest new surface in the project to date and is explicitly the TOP item
for the fresh adversarial review to pressure-test (§11).**

`git.push` reuses `git.commit`'s Pattern-B dispatch (§1.1) with exactly ONE
relaxation: a minimal seccomp net-allow (§2.3). Effect-class = pinned
`CommitIrreversible` (`sink_sensitivity.rs`, matching the existing
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

### 2.3 Net relaxation (closes P12 — net-deny widening)

The confined **WORKER never gains net** — this is unchanged and non-negotiable.
Only the `git.push` child gets a MINIMAL seccomp relaxation permitting ONLY the
socket syscalls needed to reach the ONE pinned remote host:port — **NO arbitrary
egress**. Landlock stays confined to the workspace repo. This is a NEW
`exec_child_filter` variant (a push-specific relaxation beside the existing
net-deny exec-child filter) — the METHOD is pinned here (narrowest relaxation to
reach the one resolved remote endpoint); the exact syscall set + the pinned
host:port resolution are a Phase 39 deployment constant (§11). Contrast §1.5:
`git.commit`'s child keeps the full net-deny; only `git.push`'s child relaxes,
and only to the single pinned destination.

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
