# Phase 51 — Blocking defects found by executing Plan 51-04

> Status: **Plan 51-04 STOPPED at Task 1.** No `51-LIVE-EVIDENCE.md` exists.
> LIVE-07 and LIVE-08 remain `Pending`. Broken windows 1, 2, 3 remain `open`.
> `ROADMAP.md` and `STATE.md` untouched (orchestrator-owned).
> Task 2 (full-workspace regression) **never ran** — its precondition is a green
> Task 1. There is therefore **no v1.0–v1.9 regression evidence** from this run.

## How the proof was run

Real-Linux host, provisioned and destroyed for this run:

| Property | Value |
|---|---|
| Instance | `i-05723d627c62ea73f`, c7i.4xlarge (16 vCPU / 30 GiB), `Project=caprun` |
| OS / kernel | Ubuntu 24.04, `6.17.0-1019-aws`, x86_64 (Landlock floor is 5.13) |
| Docker | 29.7.1, Compose v5.4.0 |
| Repo commit | `af64f59` plus the two fixes below |
| Command | the plan's scoped command, through an **unmodified** `scripts/compose-verify.sh` |
| Exit capture | `rc=${PIPESTATUS[0]}` taken **before** the `tee` — no pipe could turn a failure into a pass |

Instance, key pair and security group were deleted after the run; no residual
`Project=caprun` resources.

`OPENAI_API_KEY` **was** present and forwarded, so the v1.4/v1.8 composed legs
would have executed rather than skipped — but since Task 2 never ran, that is a
statement about configuration, not about achieved coverage.

## Retained raw logs

| Log | Result |
|---|---|
| `51-LIVE-SCOPED-FAIL-1-compile.log` | rc=101 — test module did not compile (8 errors) |
| `51-LIVE-SCOPED-FAIL-2-git-commit.log` | rc=101 — `git.commit` → `process_spawn_failed` |
| `51-LIVE-SCOPED-FAIL-3-chain-fork.log` | rc=101 — reached `git.push`; chain forked |

## Fixed and committed

**F1 — `2771948` — the Linux-gated test module never compiled.**
The module is behind `#[cfg(all(target_os = "linux", feature = "mock-egress-ca",
feature = "live-proof-fixtures"))]`, so macOS `cargo test` compiled none of it and
Plan 51-03 closed green on a target that could not build. Both test bodies bound a
`Command`, then called `.spawn()` as a statement and **discarded the returned
`Child`** — the driver process was orphaned. Plus `&Path` passed where
`open_audit_db` wants `&str`. No assertion, argv, env var or stdio setting changed.

**F2 — `1697766` — `process.exec` ran outside the workspace.**
`plan_coding_next` step 1 emits `process.exec` with `command` + optional `args`
and **no `cwd`**; `cwd` is optional, so the confined child inherited the *broker's*
cwd. The exec child's Landlock ruleset grants its write set only beneath
`workspace_root`, so the recipe's staging step `sh -c "git add -A && true"` staged
nothing. Because `process.exec` records **any** exit status as a normal
`process_exited`, that failure was invisible in the DAG and surfaced two nodes
later as `git.commit` exiting 1 with *"no changes added to commit"*. Fixed at
`configure_confined_command`, the single choke point shared by `run_launcher` and
`run_launcher_capture_bytes`, so the normal and confirm-release paths cannot drift.

Not an authority widening: the workspace is already the child's only writable
tree. An explicit `cwd` still wins and remains routing-sensitive, so a tainted
`cwd` is still blocked by I2.

> Scope note: F2 is a behaviour change in `crates/brokerd`, under a plan that
> declares "no new source symbol". It introduces no new symbol, but it is TCB
> code and is flagged here deliberately rather than buried.

## BLOCKING — not fixed

### D1 — the audit hash chain forks after any external append (severe)

**Confirmed by execution, not analysis.** A standalone probe reproduced it on
macOS in 0.01 s — no kernel feature, no Linux gate, no EC2:

1. broker appends `E1`, remembers its hash **in memory**;
2. an out-of-band process (`caprun grant` / `caprun confirm`) reads the **durable**
   head via `current_chain_head` and appends `E2` on top of it;
3. the broker resumes and appends `E3` using its now-**stale** remembered hash.

Result: `E1` has **2 children**, and `verify_chain` returns **false**.

Mechanism: `handle_connection` threads `last_event_id` / `last_event_hash` in
memory (`server.rs:547-548`) and never re-reads the durable head —
`current_chain_head` is not called anywhere in `server.rs`. The external appenders
(`audit.rs:548`, `confirmation.rs:852`) read a fresh head.

This is **deterministic, not a race**. Block-and-Hold guarantees the ordering: the
worker resumes only *after* the external confirm has committed. Corroborated by
the run itself — `confirm()` gates on `verify_chain` (`confirmation.rs:884`)
*before* appending `confirm_granted`, and both `confirm_granted` and
`git_push_succeeded` exist in the log, so the chain was still linear at confirm
time. The second child appeared afterwards, from the broker.

**The chain is linear by design — branching is not legitimate.**
`current_chain_head`'s doc comment (`audit.rs:1203-1219`) says so explicitly, as
does `confirmation.rs:777-783`. The "DAG" in `PLAN.md` is the provenance/taint
edge graph, not the event `parent_id` hash chain; the MAC'd `chain_anchor`
(single head + `event_count`) is structurally incompatible with branches.
**`verify_chain` is correct; the append path is the bug.** Loosening `verify_chain`
would delete HARDEN-02's tail-truncation detection — both reviewers independently
named this as the most attractive wrong turn.

Self-amplifying: once forked, any later `confirm`/`deny` fails its digest check and
appends *another* leaf, turning the product's one integrity alarm into a happy-path
false positive.

### D2 — no `busy_timeout` anywhere in the repo

`open_audit_db` (`audit.rs:721-731`) sets WAL only. `busy_timeout`,
`busy_handler` and `BEGIN IMMEDIATE` appear nowhere in `crates/`, `cli/` or
`scripts/`. A cross-process write collision returns `SQLITE_BUSY` immediately and
the broker `?`-propagates it (`server.rs:717`), killing the worker.

Consistent with LIVE-07's DAG — `session_created → policy_bound →
github_grant_authorized` with **zero** `intent_received`. **That SQLITE_BUSY was
the specific cause is inferred, not observed**: the harness discarded the stderr
that would prove it (see D4). Do not state it as fact in any evidence record.

**D1 and D2 must be designed together.** A bare `busy_timeout` would convert D2's
crash into a *successfully committed fork*; `BEGIN IMMEDIATE` without a
`busy_timeout` would worsen contention.

### D3 — LIVE-08 asserts a string the driver cannot print

`live_acceptance_v1_10_cli.rs:383` asserts stdout contains `"Sink: github.pr"`.
That string is produced only by `render_block_display` (`confirmation.rs:721`),
called from `review`/`confirm`/`deny` — **separate processes**. `caprun run`
prints `effect_id={id}  sink={sink}` (`main.rs:804`).

Consequence worth stating plainly: line 383 fires **before** lines 391-408, so
LIVE-08's *actual* claim — the durable `github.pr`/`body` anchor, non-empty taint,
and `read_event_id == provenance_chain[0] == process_event_id` — **has never
executed once**. This is not "one more run away". Prefer deleting the stdout
substring check rather than matching it; the durable-anchor assertions are the
real requirement and a stdout grep is its weakest form. Do **not** change
`caprun run`'s output to satisfy a test.

### D4 — the harness discards the diagnostic it needs

`live_acceptance_v1_10_cli.rs:300-304` panics on `sidecar.join()` **before**
`stderr_reader.join()`, throwing away broker/worker stderr on exactly the failure
path. Fixing this first makes the next run diagnostic instead of another guess.

## Reviewer verdicts

Two independent reviewers (different models), each given the same brief and told
not to assume agreement. **Both returned (b): stop 51-04; scope a separate
gap-closure plan for D1+D2 behind a design note and a fresh non-self adversarial
trace; fold D3/D4 in; then re-run 51-04 unchanged.**

They disagreed on one point, resolved against the more alarming claim:

> **v1.9 is NOT affected.** `stream_hold.rs` is absent at tag `v1.9`
> (added `5ee23fd`, 2026-07-29; tag is 2026-07-18), and v1.9's `main.rs` has no
> `CAPRUN_CONFIRM`/external handling and no `submit_plan_node`. There is no
> shipped v1.9 flow in which the broker appends after an external append.
> **The `v1.9` tag and its proofs stand; no advisory is warranted.**
> Verified directly against the tag, not taken on a reviewer's word.

Two *latent* instances of the same class exist in older code, neither reachable in
a shipped flow: the dual-connection same-seed head (`server.rs:307-382`, Planner
role is documented unused) and `record_github_grant`'s non-transactional
read-then-append (`audit.rs:546-561`). Latent since Phase 20; **live only in
v1.10's composition.**

## Can v1.10 be marked DONE first?

**No.** LIVE-07 acceptance requires `verify_chain` true, and LIVE-08 requires the
chain to remain valid through the I2 block. D1 makes that deterministically false
on the exact path v1.10 exists to ship. There is no harness workaround — the
mid-loop external confirm *is* the feature.

## Recommended scope for the gap-closure plan

1. **Red test first, macOS-runnable.** Turn the reproducer above into a committed
   regression test that **fails today**. It must not be `#[cfg(target_os = "linux")]`
   — cfg-linux blindness is why F1 and F2 shipped, and D1 needs no kernel feature.
   This converts the EC2 run from discovery into confirmation.
2. **Design note** for the append-at-head concurrency discipline, then the fix:
   read the head and insert under one `BEGIN IMMEDIATE` transaction, with
   `busy_timeout` set in `open_audit_db` so every connection gets it, including
   short-lived CLI ones. Note `parent_id` is serialized into the hashed payload,
   so the parent must be chosen *inside* the transaction.
3. **Fresh non-self adversarial trace** on the diff. It must specifically check
   that the causal chain `parent_id` is never conflated with provenance
   `read_event_id` (`server.rs:941`, `:957`, `:973`) — I2 and M7 ride on the
   provenance graph and must be untouched.
4. **D3 + D4** folded in.
5. Only then relaunch a proof host and re-run 51-04 **unchanged**.
