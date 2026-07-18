# Phase 34: Regression & Live Proof (v1.7 DONE) - Research

**Researched:** 2026-07-18
**Domain:** Rust broker-side confirm-release plumbing (`from_resolved` mirror) + Linux live-proof harness composition
**Confidence:** HIGH (all claims code-grounded via direct file reads of the exact modules the planner will edit)

## Summary

Phase 34 is almost entirely a **mechanical mirror + composition** task, not new
design. EXEC-05's `invoke_process_exec_from_resolved` has two already-built
twins (`invoke_file_write_from_resolved`, `invoke_file_create_from_resolved`)
in the same crate, and the `confirmation.rs` Step-7 dispatch + Step-4.75
entry-guard already generically support any sink (they were built sink-generic
in Phase 10/16/33) — EXEC-05 is "add one more match arm to two places" plus
one genuinely new plumbing wrinkle: **`invoke_process_exec` is `async` (it
spawns `caprun-exec-launcher` via `tokio::process::Command`), but
`confirmation::confirm()` and its caller `run_confirm_or_deny` are currently
synchronous.** Async has to thread through both, and the mint call site for
the confirm-release path cannot live inside `sinks/process_exec.rs` (that
module's own doc comment says it deliberately never mints) or inside
`server.rs` (that path isn't invoked at confirm time at all) — it must live in
`confirmation.rs`'s new `"process.exec"` Step-7 arm, which means Gate 3's
`mint_from_exec(` allow-list is satisfied via the **inline
`<!-- planner-discipline-allow: mint_from_exec -->` exemption**
`check-invariants.sh` already implements for exactly this case, not by adding
a new file to the two-file allow-list array (preserving CONTEXT.md D-03's "no
new entry").

LIVE-01/LIVE-02 have three directly reusable precedents already in the repo:
the Linux-gated `s9_process_exec_block.rs`/`s9_file_write_block.rs` per-sink
acceptance pattern, the cross-process `caprun confirm` subprocess harness in
`cli/caprun/tests/confirm.rs`, and the shared-audit.db multi-leg composed-run
pattern from `live_acceptance_v1_3.rs`/`live_acceptance_v1_4_composed.rs`. The
true-exit-before-pipe discipline is a copy-paste of
`scripts/verify-harden04-featureless.sh`'s `set +e; ... ; rc=$?; set -e`
block.

**Primary recommendation:** Mirror `invoke_file_write_from_resolved`'s
two-phase-audit *shape* but `invoke_process_exec`'s *async signature and
plain-`&rusqlite::Connection` locking discipline* (no `Arc<Mutex<>>` needed —
confirm-time has no concurrent broker connections); make `confirm()` async;
add `"process.exec"` to both the Step-4.75 guard and the Step-7 match; do the
mint in `confirmation.rs`, annotated inline, never in the sink module.

## User Constraints

<user_constraints>
### Locked Decisions (from 34-CONTEXT.md)

- D-01: Add `invoke_process_exec_from_resolved` in
  `crates/brokerd/src/sinks/process_exec.rs`, mirroring
  `invoke_file_write_from_resolved`/`invoke_file_create_from_resolved`.
- D-02: Re-apply the EXACT Allowed-path discipline (broker-spawned confined
  child: Landlock + seccomp + default-deny net + rlimits + wall-clock timeout
  + byte cap, stdout/stderr captured).
- D-03: Output taint-minted via the sanctioned `mint_from_exec` — untrusted,
  non-stapled, provenance anchored at the `exec` Event — `output_value_id`
  populated. Reuses `mint_from_exec`; Gate-3 mint-site allow-list needs **no
  new entry**.
- D-04: The two-phase `process_exited`/`process_spawn_failed` audit Events are
  chained onto the `confirm_granted` head (not a fresh root).
- D-05: Add `"process.exec"` arm to `confirmation.rs` Step-7 dispatch.
- D-06: The command runs exactly once on release (idempotent/no double-spawn).
- D-07: Preserve the Phase-33 pre-Step-5 entry-guard: fail-closed-recoverable.
- D-08: I2 stays table-entries-only — no new `ExecutorDecision`, no
  `submit_plan_node` change, no policy that can disable I2.
- D-09: No new raw `EffectRequest` path (Gate 1 stays green).
- D-10: `mint_from_exec` remains the sole sanctioned exec-output mint site
  (Gate-3 mint-site list unchanged and green).
- D-11: A `cfg(target_os = "linux")` test: block → `caprun confirm` releases
  → runs exactly once, output taint-minted, sink Event durably chained
  (`verify_chain` true); plus an entry-guard fail-closed leg.
- D-12: One composed acceptance run on real Linux proves, in the same run:
  (a) tainted exec-output → sensitive sink arg is Blocked (I2, non-stapled,
  `verify_chain` true); (b) a clean exec/fs path is Allowed; (c) fs write/edit
  within `WorkspaceRoot` succeeds and is audited; (d) the EXEC-05
  confirm-release path is exercised.
- D-13: Run via `scripts/mailpit-verify.sh` or an exec-scoped equivalent,
  true-exit-before-pipe, asserting named tests + counts.
- D-14: Full-workspace regression green on real Linux, no regression to
  v1.0–v1.6, counts + named tests, plus a dedicated negative test per new sink.
- D-15: Linux compile-check (`cargo build --tests --workspace --keep-going`
  via `mailpit-verify.sh`, true-exit-0) after EXEC-05 lands, before the
  composed live proof.
- D-16: Fresh non-self Fable-5 adversarial code-trace of the confirm-release
  TCB diff, orchestrator-owned, before the live proof is authorized.
- D-17: v1.7 close requires human DONE sign-off; not pushed unless requested.

### Claude's Discretion

- Exact plan/wave decomposition (EXEC-05 TCB in an early wave, live proofs
  after the release gates), test file names, and the precise shape of the
  exec-scoped verify harness vs. reusing `scripts/mailpit-verify.sh`.

### Deferred Ideas (OUT OF SCOPE)

- `git`/`github.pr` and `http.request` sinks — v1.8 (Effect Breadth II).
- Real multi-step LLM planner loop — v1.9.
- Declarative policy file/Cedar, SDK/audit-DAG viewer, packaging — v1.10+.
- Full push to origin — only on explicit operator instruction.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EXEC-05 | process.exec confirm-release TCB slice | §"The `from_resolved` mirror" + §"confirmation.rs Step-7 dispatch" below give the exact signatures, locking discipline, and the async-threading + Gate-3-mint-site findings the planner must account for. |
| LIVE-01 | Composed acceptance run on real Linux | §"The live-proof harness" gives the exact composed-run precedent (`live_acceptance_v1_3.rs`/`live_acceptance_v1_4_composed.rs`) and the cross-process `caprun confirm` harness (`confirm.rs`) to extend for the exec block+release leg. |
| LIVE-02 | Full-workspace regression, no v1.0–v1.6 regression | §"Invariant gates" + §"Validation Architecture" give the exact gate commands and true-exit-before-pipe pattern (`verify-harden04-featureless.sh`) to reuse. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `process.exec` confirm-release re-invocation | Broker (`brokerd::sinks::process_exec`) | — | Same tier as the Allowed-path sink; the broker is the sole spawner of the confined child, confirm-time or not. |
| Confirm-release dispatch (Step 4.75 guard + Step 7 match) | Broker (`brokerd::confirmation`) | — | Owns the durable state-machine + audit-chain discipline; a separate later OS process (`caprun confirm`) always re-enters through this module. |
| Exec-output taint mint | Broker (`brokerd::quarantine::mint_from_exec`) | — | Sole sanctioned taint-mint site (Gate 3); called from `confirmation.rs` at confirm time exactly as it is called from `server.rs` at Allow time. |
| Composed live-proof orchestration | CLI / test harness (`cli/caprun/tests/*`, `scripts/mailpit-verify.sh`) | Broker (exercised in-process) | The proof drives real broker+sink code paths but the harness itself is test/ops tooling, not TCB. |
| Full-workspace regression gate | CI/ops tooling (`scripts/*.sh`) | — | Orchestrator-owned, non-TCB verification surface. |

## Standard Stack

No new external dependencies. `tokio` (`time`, `process`, `io-util` features)
is already a `brokerd` dependency (`crates/brokerd/Cargo.toml:29`) and already
used by `sinks/process_exec.rs`'s `invoke_process_exec`. No package legitimacy
audit is required — this phase adds zero new crates.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Making `confirm()` async + threading `.await` through `run_confirm_or_deny`/`main.rs` | Wrapping the exec spawn in `tokio::runtime::Handle::current().block_on(...)` inside a still-sync `confirm()` | Blocking on the current multi-threaded Tokio runtime from within an already-running task risks starving the runtime and is generally discouraged; since `main.rs` is already `#[tokio::main] async fn main()` and calls `run_confirm_or_deny` directly (not via `spawn_blocking`), simply making the call chain async top-to-bottom is simpler, matches existing project style (`server.rs`'s `evaluate_plan_node_and_record` is already async and `.await`s `invoke_process_exec`), and avoids a runtime-nesting footgun. **Recommended: make it async.** |
| Minting inside `confirmation.rs`'s new Step-7 arm with an inline Gate-3 exemption comment | Adding `crates/brokerd/src/confirmation.rs` as a third allow-list file in `check-invariants.sh`'s `check_mint_token "mint_from_exec(" ...` call | Either technically keeps Gate 3 green. The inline-annotation form more literally satisfies D-03's "the Gate-3 mint-site allow-list needs no new entry" (the two-file array in `check-invariants.sh` itself stays byte-identical); a file-list edit would still pass but changes the gate's source. **Recommended: inline annotation**, since D-03 phrases the constraint at the file-list level specifically ("confirm this stays green" without editing it). |

## Package Legitimacy Audit

Not applicable — this phase introduces zero new external packages (npm/PyPI/crates or otherwise). No `package-legitimacy check` run was needed.

## Architecture Patterns

### System Architecture Diagram

```
caprun run  (original process)              caprun confirm <effect_id>  (LATER, separate process)
     │                                                  │
     ▼                                                  ▼
submit_plan_node (I2)                        find_pending_confirmation (indexed lookup)
     │  BlockedPendingConfirmation                      │
     ▼                                                  ▼
server.rs: persist sink_blocked +            confirmation::confirm()
  PendingConfirmation (resolved_args frozen)   Step 1.5  verify_pending_confirmation_mac
     │                                          Step 2    terminal-state check
     │  (process exits; in-memory                Step 3    redaction gate
     │   ValueStore is gone)                      Step 4    render_block_display (verbatim)
     │                                          Step 4.5a verify_chain (keyed HMAC)
     │                                          Step 4.5b recompute-and-compare combined_digest
     │                                          Step 4.75 ENTRY GUARD — add "process.exec" here
     │                                          Step 5    append confirm_granted (chain head)
     │                                          Step 6    transition_state -> Confirmed
     │                                          Step 7    DISPATCH — add "process.exec" arm here
     │                                                    │
     │                                                    ▼
     │                                     sinks::process_exec::invoke_process_exec_from_resolved
     │                                       (async: spawn caprun-exec-launcher, confined child,
     │                                        capture stdout+stderr, wall-clock timeout + byte cap,
     │                                        append process_exited/process_spawn_failed on
     │                                        confirm_granted head)
     │                                                    │
     │                                                    ▼
     │                                     confirmation.rs: mint_from_exec(combined_output,
     │                                       sink_event_id)  <-- NEW call site, inline Gate-3
     │                                       exemption, NOT inside sinks/process_exec.rs
     │                                                    │
     │                                                    ▼
     │                                     ConfirmOutcome::Released / ConfirmedButSinkFailed
```

### Recommended Project Structure

No new files/directories required for EXEC-05 itself — all edits land in
existing files:

```
crates/brokerd/src/
├── sinks/process_exec.rs   # ADD invoke_process_exec_from_resolved (mirrors
│                           #   invoke_file_write_from_resolved's structure,
│                           #   invoke_process_exec's async/locking discipline)
├── confirmation.rs         # ADD "process.exec" to Step 4.75 guard match +
│                           #   Step 7 dispatch match; ADD the mint_from_exec
│                           #   call site (inline Gate-3 exemption comment)
└── (server.rs, quarantine.rs — UNCHANGED; the Allow-path dispatch and the
     Gate-3 sanctioned-loci list both already exist and need no edits)

cli/caprun/src/main.rs       # confirm()'s caller chain: run_confirm_or_deny
                              # becomes async (threaded from #[tokio::main])

cli/caprun/tests/            # NEW test file (or extend s9_process_exec_block.rs)
                              # for D-11's cfg(linux) confirm-release acceptance

scripts/                     # optionally: an exec-scoped verify wrapper, OR
                              # reuse mailpit-verify.sh with MAILPIT_VERIFY_CMD
                              # scoped to the new test target (Claude's Discretion)
```

### Pattern 1: The `from_resolved` Mirror (async variant)

**What:** A `ValueStore`-free sibling of the Allow-path sink function, called
only from `confirmation::confirm()`'s Step 7, operating exclusively on the
frozen `PendingConfirmation.resolved_args` snapshot — never re-resolving,
never calling `executor::submit_plan_node`.

**When to use:** Any sink whose Allowed-path invocation needs a confirm-time
release twin. `file.create`/`file.write` already established the *sync*
variant of this pattern (`conn: &rusqlite::Connection`, no `Arc<Mutex<>>`,
no `.await`). `process.exec` needs the *async* variant because
`invoke_process_exec` itself is `async fn` (it spawns via
`tokio::process::Command` and awaits the launcher).

**Example — the exact reference implementation to mirror (structure), from
`crates/brokerd/src/sinks/file_write.rs:149-237`:**
```rust
// Source: crates/brokerd/src/sinks/file_write.rs (verbatim structure to mirror)
pub fn invoke_file_write_from_resolved(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let path = resolved_literal(resolved_args, "path")?;
    let contents = resolved_literal(resolved_args, "contents")?;
    match workspace_root.write_within(path, contents.as_bytes()) {
        Ok(()) => { /* append sink_executed, chained on parent_id/parent_hash */ }
        Err(e) => { /* append sink_invocation_failed FIRST, then propagate (no retry) */ }
    }
}
```

**And the exact async/locking discipline to carry over instead, from
`crates/brokerd/src/sinks/process_exec.rs:110-200`** (note: `conn` here is
`&Arc<Mutex<rusqlite::Connection>>` for the Allow-path because the broker
serves concurrent connections; the confirm-release path has no such
concurrency, so `invoke_process_exec_from_resolved` should take a plain
`conn: &rusqlite::Connection` — mirroring `file_write.rs`'s parameter type —
while still being `async fn` and still calling the SAME private
`run_launcher(...)` helper `invoke_process_exec` already uses, which itself
takes no `conn` parameter at all and is therefore directly reusable
unmodified):

```rust
// Source: crates/brokerd/src/sinks/process_exec.rs:143-151 (the reusable,
// conn-free async helper — call this from invoke_process_exec_from_resolved too)
let spawn_result = run_launcher(
    &launcher_path, &command, &args_json, cwd.as_deref(), workspace_root, &args,
).await;
match spawn_result {
    Ok((_exit_status, combined_output)) => {
        // append `process_exited` chained onto `parent_id`/`parent_hash`
        // (== granted_event_id/granted_hash from confirm()'s Step 5, per D-04)
    }
    Err(e) => {
        // append `process_spawn_failed` chained onto `parent_id`/`parent_hash`, propagate
    }
}
```

**Concrete signature recommendation for `invoke_process_exec_from_resolved`:**
```rust
pub async fn invoke_process_exec_from_resolved(
    conn: &rusqlite::Connection,          // plain, no Arc<Mutex<>> — confirm-time, no concurrency
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],         // "command" (required), "args" (optional JSON), "cwd" (optional)
    workspace_root: &WorkspaceRoot,        // reopened at confirm time from PendingConfirmation.workspace_root_path
    parent_id: Uuid,                       // == granted_event_id (confirm_granted, per D-04)
    parent_hash: &str,                     // == granted_hash
) -> Result<(Uuid, String, String)>        // (process_exited event_id, hash, combined_output) — mirrors invoke_process_exec's return shape so the caller can mint
```
The literal-lookup helper mirrors `file_write.rs`'s `resolved_literal`; `args`
must be decoded from its JSON-`Vec<String>` literal exactly as
`invoke_process_exec`'s `resolve_arg_optional` + `serde_json::from_str` does
(process_exec.rs:131-135), just reading from `resolved_args` instead of a
`ValueStore`.

### Pattern 2: Step 4.75 Entry Guard + Step 7 Dispatch (both must change together)

**What:** `confirmation.rs`'s `confirm()` has a fail-closed allow-list
(`"file.create" | "email.send" | "file.write" => {}`, else `Err(...)`, row
stays `Pending`) immediately BEFORE `confirm_granted` is appended
(confirmation.rs:825-834), and an exhaustive `match pc.sink.0.as_str()` at
Step 7 (confirmation.rs:877-975) that actually dispatches. The guard's own
doc comment states explicitly: *"This list MUST stay in sync with the sink
match arms in Step 7 below."* EXEC-05 must edit BOTH in the same commit.

**When to use:** Adding confirm-release support for any sink.

**Example (exact code to extend), `crates/brokerd/src/confirmation.rs:825-834`:**
```rust
// Source: crates/brokerd/src/confirmation.rs:825-834 — ADD "process.exec" here
match pc.sink.0.as_str() {
    "file.create" | "email.send" | "file.write" | "process.exec" => {}
    other => {
        return Err(anyhow::anyhow!(
            "confirm: sink `{other}` has no confirm-release dispatch wired \
             — refusing before confirm_granted/state transition (fail-closed, \
             row remains Pending)"
        ));
    }
}
```

**And the Step 7 dispatch arm (new, mirrors the `"file.write"` arm at
confirmation.rs:900-914, but async + mint):**
```rust
// Source: pattern from crates/brokerd/src/confirmation.rs:900-914, extended
"process.exec" => {
    match crate::sinks::process_exec::invoke_process_exec_from_resolved(
        conn, key, pc.session_id, pc.effect_id, &pc.resolved_args,
        workspace_root, granted_event_id, &granted_hash,
    ).await {
        Ok((sink_event_id, _hash, combined_output)) => {
            // Gate 3: mint_from_exec( — this call site is intentionally here,
            // never in sinks/process_exec.rs (that module never mints) and
            // never in server.rs (not on this call path). Annotate inline
            // per check-invariants.sh's documented exemption mechanism:
            // <!-- planner-discipline-allow: mint_from_exec -->
            let mut throwaway_store = executor::value_store::ValueStore::default();
            match crate::quarantine::mint_from_exec(
                &mut throwaway_store, pc.session_id, combined_output, sink_event_id,
            ) {
                Ok(_output_value_id) => Ok(ConfirmOutcome::Released),
                Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
            }
        }
        Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
    }
}
```
The `ValueStore` here is deliberately throwaway/local: there is no live
worker connection at confirm time to hand `output_value_id` to over the wire
(the Allow-path's `output_value_id` is already discarded by the worker today
— `cli/caprun/src/worker.rs:377`, `let _ = &output_value_id;`). What matters
for D-03/EXEC-05 is that `mint_from_exec` runs (producing a genuinely-rooted,
non-stapled `ValueId` whose `provenance_chain == [sink_event_id]`), not that
the id is durably persisted anywhere beyond the audit Event chain itself.

**Critical coupling — confirm() must become async:** `invoke_process_exec`
(and therefore `invoke_process_exec_from_resolved`) is `async fn` because it
spawns a child process and awaits its exit. `confirmation::confirm()` is
currently `pub fn confirm(...)` (sync), called from
`cli/caprun/src/main.rs::run_confirm_or_deny` (also sync), called inline
(no `.await`) from inside `#[tokio::main] async fn main()`
(`cli/caprun/src/main.rs:62-63,99`). The planner must:
1. Change `confirm()` to `pub async fn confirm(...)`.
2. Change `run_confirm_or_deny` to `async fn` and `.await` the `confirm(...)`/
   `deny(...)` calls (deny stays sync internally but the fn wrapping it can
   still be async — or split the match arms; Claude's Discretion).
3. `.await` the call to `run_confirm_or_deny` at its one call site in `main`
   (already inside the async runtime — no new `tokio::main`/`block_on`
   needed).
This is pure plumbing, not a security-posture change — no `ExecutorDecision`,
`submit_plan_node`, or `EffectRequest` token is touched.

### Anti-Patterns to Avoid

- **Minting inside `sinks/process_exec.rs`'s `invoke_process_exec_from_resolved`:**
  That module's own doc comment (process_exec.rs:29-36) states it deliberately
  never mints, precisely so Gate 3's allow-list stays satisfied for the
  Allow-path function. Adding a `mint_from_exec(` call there requires either an
  inline exemption on THAT file too or breaks Gate 3 — keep the mint call
  exclusively in `confirmation.rs`'s Step 7 arm as shown above.
- **Using `tokio::runtime::Runtime::new().block_on(...)` inside a still-sync
  `confirm()`:** panics with "Cannot start a runtime from within a runtime"
  since `main` is already inside a Tokio runtime. Thread `async`/`.await`
  through instead.
- **Forgetting to update the Step 4.75 guard when adding the Step 7 arm (or
  vice versa):** the guard's own doc comment calls this out; a drift leaves
  either a live security regression (Step 7 dispatches something the guard
  should have refused) or an "unreachable" internal-invariant error (Step 7
  has no arm for something the guard let through) — verify both edits land in
  the same diff.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Confirm-time frozen-arg lookup | A new resolved-arg accessor | The existing `resolved_literal` helper pattern (private fn, duplicated per sink module — `file_write.rs:141-147`, `file_create.rs:132-138`) | Already proven, trivial, and each sink module already keeps its own private copy — no shared abstraction to build. |
| Combined-digest recompute-and-compare | A new integrity check for process.exec | `confirmation::combined_digest` (already generic over `&[(&str, &str)]` — confirmation.rs:79-109) | Sink-agnostic already; process.exec's resolved_args flow through the SAME Step 4.5b check with zero changes needed. |
| True-exit-before-pipe capture | A bespoke exit-code capture pattern | `scripts/verify-harden04-featureless.sh`'s `set +e; ... > log 2>&1; rc=$?; set -e` block (lines 107-112) | Already the project's proven pattern for exactly this failure mode (a piped `tail`/`grep` silently returning the pipe's own exit code, not the command's). |

**Key insight:** Nothing in this phase's TCB slice needs a new abstraction —
the entire job is extending two already-generic dispatch points (Step 4.75,
Step 7) and mirroring an already-twice-proven pattern (`_from_resolved`) for
a third sink. The only genuinely new wrinkle is the sync/async seam, which is
plumbing, not design.

## Common Pitfalls

### Pitfall 1: Guard/match drift (already a documented failure mode)
**What goes wrong:** `"process.exec"` added to the Step 4.75 guard's allow-list
but the Step 7 match still routes it to the exhaustive `other` catch-all,
which returns `Err(anyhow!("confirm: unreachable — sink `{other}`..."))`
(confirmation.rs:971-974) — the row was already transitioned to `Confirmed`
by Step 6 by the time this fires, so a real dispatch would be silently lost
even though the state machine looks "done."
**Why it happens:** The two lists (guard match, dispatch match) are
maintained by hand in two different places in the same function.
**How to avoid:** Edit both in one commit; the D-11 acceptance test's
"entry-guard fail-closed leg" should specifically assert this does NOT
regress for any FUTURE un-dispatchable sink (not process.exec itself, since
process.exec will now be dispatchable) — i.e. re-run the existing guard test
with a still-unwired sink name to prove the guard mechanism itself still
fires.
**Warning signs:** `caprun confirm` on a process.exec block exits non-zero
with an "unreachable" message after already logging `confirm_granted`.

### Pitfall 2: `Arc<Mutex<Connection>>` vs plain `&Connection` signature mismatch
**What goes wrong:** Copying `invoke_process_exec`'s signature verbatim
(`conn: &Arc<Mutex<rusqlite::Connection>>`) into
`invoke_process_exec_from_resolved` won't compile against `confirm()`'s
`conn: &mut rusqlite::Connection` without an unnecessary wrap-in-Arc-Mutex at
every call site.
**Why it happens:** `invoke_process_exec`'s Arc<Mutex<>> exists because the
broker serves many concurrent connections sharing one connection; the
confirm-release path is a single-shot CLI process with no such concurrency.
**How to avoid:** Give `invoke_process_exec_from_resolved` the plain
`&rusqlite::Connection` signature (matching `file_write.rs`'s
`_from_resolved` sibling), calling the shared `run_launcher(...)` helper
(which never touches `conn` at all) for the async spawn, then doing the
`append_event` calls directly against the un-locked reference — exactly
`file_write.rs`'s `_from_resolved` locking discipline, `process_exec.rs`'s
async body.
**Warning signs:** Compile errors trying to pass `&mut rusqlite::Connection`
where `&Arc<Mutex<rusqlite::Connection>>` is expected.

### Pitfall 3: Treating the D-11 acceptance test as cross-platform
**What goes wrong:** Writing the confirm-release acceptance test as a plain
(non-`#[cfg(target_os = "linux")]`) test, matching `s9_file_write_block.rs`'s
convention (which IS cross-platform because `file.write` has no confined
child process).
**Why it happens:** `s9_file_write_block.rs`'s header explicitly explains why
it is NOT Linux-gated (no spawn, no launcher, no confinement to prove) —
easy to over-generalize that reasoning to process.exec.
**How to avoid:** `process.exec`'s confirm-release test MUST be
`#[cfg(target_os = "linux")]`-gated like `s9_process_exec_block.rs`
(cli/caprun/tests/s9_process_exec_block.rs:47), because
`invoke_process_exec_from_resolved` genuinely spawns `caprun-exec-launcher`,
whose confinement primitives are Linux-only no-op stubs on macOS
(process_exec.rs's own header comment says a Mac run "would prove nothing
about the confined spawn path").
**Warning signs:** A "green on macOS" test that never actually ran on Linux
(cfg-linux-test-blindness — the project's own standing gotcha).

### Pitfall 4: `process.exec`/`command`'s slot is role-UNCONSTRAINED, not slot-type-mismatched
**What goes wrong:** Assuming the exec-block test needs a role-mismatch
setup like `email.send`/`body` or `file.create`/`path`.
**Why it happens:** `s9_process_exec_block.rs`'s own doc comment
(lines 114-124) explicitly documents that `process.exec`/`command`'s
`expected_role` is `None` (deliberately unconstrained — no legitimate exec
command has an `origin_role`-producing mint site), so the tainted
exec-output → command routing Blocks purely on **taint** (I2), never on
`SlotTypeMismatch`. Do not add a role check that doesn't exist in the
executor tables (out of scope per CONTEXT.md D-08).
**How to avoid:** Reuse the exact `s9_process_exec_block.rs` block-setup
(first exec produces tainted output via `mint_from_exec`, second
`process.exec` plan node routes that handle into its own `command` arg) —
the block is already proven to fire correctly; EXEC-05 only adds the release
leg after it.
**Warning signs:** A test that asserts `DenyReason::SlotTypeMismatch` for
process.exec's command arg (wrong — it's a plain taint Block).

### Pitfall 5: Mint failure after a successful spawn is a real, if rare, edge case
**What goes wrong:** `mint_from_exec` can in principle fail (its
`ValueStore::mint` invariant guards against empty taint/provenance — never
actually empty here since the taint vec is hardcoded, but the function still
returns a `Result`). If it fails AFTER `invoke_process_exec_from_resolved`
already durably appended `process_exited` (the effect genuinely happened —
the command ran), what `ConfirmOutcome` is correct?
**Why it happens:** Two fallible operations (spawn+capture, then mint) are
chained in one match arm.
**How to avoid:** Map both failure points to `ConfirmedButSinkFailed`
(consistent with the existing arms' single-`Err`-branch shape) — the
process genuinely ran and is durably audited either way; the confirm-time
CLI exit code contract doesn't distinguish "spawn failed" from "spawn
succeeded but mint bookkeeping failed," which is acceptable since both are
sink-adapter-internal outcomes distinct from `Released`.
**Warning signs:** A code path that silently drops the mint error via `let _
= mint_from_exec(...)` instead of surfacing it through the match.

## Code Examples

### Confirm-release cross-process test harness pattern (mirror for EXEC-05)
```rust
// Source: cli/caprun/tests/confirm.rs:64-174 (seed_pending_file_create_block)
// — mirror this shape for a seed_pending_process_exec_block(), noting the
// exec case needs a REAL process_exited event (from an actual
// invoke_process_exec call against a real launcher binary), so this harness
// must run inside a #[cfg(target_os = "linux")] module, unlike confirm.rs's
// existing (non-gated) file.create/email.send seeders.
fn seed_pending_file_create_block(
    db_path: &Path, key: &[u8], path: &str, contents: &str, workspace_root: &Path,
) -> (Uuid, Uuid, Uuid) {
    // ... open_audit_db, append session_created root, construct SinkBlockedAnchor
    // + ResolvedArg set + combined_digest, append sink_blocked, insert_blocked_literal,
    // construct + insert_pending_confirmation ...
}

// Then, mirroring confirm.rs:244-293's `confirm_releases_once_and_second_confirm_is_already_terminal`:
let (code, stdout) = run_caprun_verb("confirm", effect_id, &db_path); // spawns REAL `caprun confirm` binary
assert_eq!(code, 0);
// assert the command actually ran exactly once (e.g. a marker file/output side effect)
let (code2, _) = run_caprun_verb("confirm", effect_id, &db_path); // second confirm, same effect_id
assert_eq!(code2, 5, "AlreadyTerminal — no double-spawn");
```

### True-exit-before-pipe pattern (reuse for LIVE-01/LIVE-02 gates)
```bash
# Source: scripts/verify-harden04-featureless.sh:107-112
set +e
MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test <new_exec05_test>' \
  bash scripts/mailpit-verify.sh > "${LOG_FILE}" 2>&1
rc=$?
set -e
echo "delegated run exit code = ${rc}"
tail -n 60 "${LOG_FILE}" || true
# then assert on named test output / counts extracted from LOG_FILE, never on `$?` alone through a pipe
```

### Composed multi-leg shared-audit.db pattern (reuse for LIVE-01)
```rust
// Source: cli/caprun/tests/live_acceptance_v1_4_composed.rs:1-60 (doc comment) —
// one shared audit.db across N legs (Blocked exec, Allowed exec/fs, fs write/edit,
// EXEC-05 confirm-release), each leg its own session_id, EACH session's
// verify_chain independently asserted true — never reopen a fresh :memory: DB
// per leg.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `process.exec` Blocked-by-I2 had no confirm-release path | Human release via `caprun confirm` → `invoke_process_exec_from_resolved` | This phase (EXEC-05) | Closes the Phase-33 adversarial-review open follow-up; `process.exec` reaches parity with `file.create`/`file.write`/`email.send` for confirm-release. |
| `confirmation::confirm()` was synchronous | Becomes `async fn`, threaded through `run_confirm_or_deny`/`main` | This phase | Pure plumbing; no behavior change for the three already-wired sinks (file.create/email.send/file.write), which remain synchronous internally but now run inside an `async fn`'s body — no `.await` needed on their own calls, only on the new `process.exec` arm. |

**Deprecated/outdated:** None — no prior mechanism is being replaced, only
extended.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `invoke_process_exec_from_resolved` should take a plain `&rusqlite::Connection` (not `Arc<Mutex<>>`) since confirm-time has no concurrent connections | Pattern 1 / Pitfall 2 | If the planner instead threads an `Arc<Mutex<>>` through `confirm()`'s signature for consistency with `evaluate_plan_node_and_record`, that's a valid alternative — low risk either way, purely a signature-ergonomics call, not a security decision. |
| A2 | The `mint_from_exec` call site for confirm-release should live in `confirmation.rs` (inline-annotated), not be added to `check-invariants.sh`'s two-file allow-list array | Pattern 2 / Standard Stack "Alternatives Considered" | If the planner instead edits the allow-list array, Gate 3 still passes — CONTEXT.md D-03's exact wording ("no new entry") is satisfied either loosely (gate stays green) or strictly (array is byte-identical); worth a 1-line confirmation with the operator if the planner picks the array-edit path, since D-03 phrases it as a locked constraint. |
| A3 | Mint failure after a successful spawn should map to `ConfirmedButSinkFailed` | Pitfall 5 | Low risk — this is an edge case with no realistic trigger given `mint_from_exec`'s hardcoded non-empty taint/provenance args; any reasonable mapping (including a new dedicated `ConfirmOutcome` variant) satisfies D-06/D-07's actual security requirements. |

## Open Questions

1. **Exact test file for D-11 (new file vs. extending `s9_process_exec_block.rs`)**
   - What we know: `s9_process_exec_block.rs` already has the exact
     `#[cfg(target_os = "linux")] mod linux` scaffolding, `TEST_KEY`, and
     `mint_trusted`/`fresh_workspace` helpers the confirm-release leg needs.
   - What's unclear: whether the planner should add a new `#[tokio::test]` fn
     inside that same file's `mod linux` block, or create a new
     `s9_process_exec_confirm_release.rs` test file (mirroring how
     `s9_file_write_block.rs` is its own file rather than folded into an
     existing one).
   - Recommendation: extend `s9_process_exec_block.rs`'s existing `mod linux`
     block — it already has every fixture helper needed (seed_root_event,
     mint_trusted, fresh_workspace), avoiding duplication. Explicitly
     Claude's Discretion per CONTEXT.md.

2. **Whether LIVE-01 needs a dedicated exec-scoped verify script or can reuse `mailpit-verify.sh` via `MAILPIT_VERIFY_CMD`**
   - What we know: `mailpit-verify.sh` already supports scoping via
     `MAILPIT_VERIFY_CMD` (used by `verify-harden04-featureless.sh` and
     documented in the script's own header); process.exec's live proof needs
     no Mailpit SMTP capture of its own (LIVE-01's (a)/(b)/(c)/(d) legs are
     exec/fs/confirm-release, not email) but LIVE-02's full regression DOES
     still need Mailpit up (for the pre-existing email.send suite it must not
     regress).
   - What's unclear: whether a genuinely exec-scoped harness (no Mailpit
     sidecar at all) is worth the extra script vs. just running the
     exec/fs-scoped test target through the existing `mailpit-verify.sh`
     (Mailpit simply sits idle for that scoped run).
   - Recommendation: reuse `mailpit-verify.sh` with `MAILPIT_VERIFY_CMD`
     scoped to the new composed test target for LIVE-01, and the default
     (unscoped, full `cargo build --workspace && cargo test --workspace
     --no-fail-fast`) invocation for LIVE-02 — no new script needed. Explicitly
     Claude's Discretion per CONTEXT.md.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker (Colima) | Linux verification gates (D-15, LIVE-01, LIVE-02) | ✓ (per CLAUDE.md: "Colima installed") | — | none needed |
| `rust:1` Docker image | `mailpit-verify.sh` verification container | ✓ (pulled on demand by `docker run`) | latest `rust:1` tag | none needed |
| `axllent/mailpit` | LIVE-02 full-workspace regression (existing email.send suite) | ✓ (pulled on demand) | latest | none needed |
| Kernel ≥5.13 (Landlock) | All Linux-only confinement tests, incl. the new EXEC-05 acceptance test | Assumed ✓ inside the `rust:1` container per existing project precedent (Phases 32-33 already ran this successfully) | — | none — a missing kernel feature blocks execution entirely (documented project constraint, not new to this phase) |

No missing dependencies with no fallback beyond what prior phases (32, 33)
already depend on and have proven working.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in test harness), `#[tokio::test]` for async cases |
| Config file | none — Cargo workspace defaults |
| Quick run command | `cargo test -p brokerd invoke_process_exec_from_resolved` (once written) / `cargo test -p caprun --test s9_process_exec_block` |
| Full suite command | `cargo build --workspace && cargo test --workspace --no-fail-fast` via `scripts/mailpit-verify.sh` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXEC-05 | `invoke_process_exec_from_resolved` success + failure (unit) | unit | `cargo test -p brokerd --lib -- process_exec` | ❌ Wave 0 — add `#[cfg(test)] mod tests` to `sinks/process_exec.rs` mirroring `file_write.rs`'s `_from_resolved` unit tests |
| EXEC-05 | Full confirm-release: block → `caprun confirm` → runs once, taint-minted, `verify_chain` true | integration (cfg(linux)) | `cargo test -p caprun --test s9_process_exec_block` (extended) | ❌ Wave 0 — extend `cli/caprun/tests/s9_process_exec_block.rs`'s `mod linux` |
| EXEC-05 | Entry-guard fail-closed leg (a still-un-dispatchable sink) | unit/integration | existing pattern in `confirmation.rs`'s own `#[cfg(test)] mod tests` (extend) | ❌ Wave 0 — add a case asserting the Step 4.75 guard still refuses an unknown sink name after process.exec is added |
| LIVE-01 | Composed acceptance: exec Block + clean Allow + fs write/edit + confirm-release, one shared audit.db | integration (cfg(linux)) | `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_7_composed' bash scripts/mailpit-verify.sh` | ❌ Wave 0 — new composed test file, mirroring `live_acceptance_v1_4_composed.rs`'s multi-leg-shared-db structure |
| LIVE-02 | Full-workspace regression, counts + named tests, no regression v1.0–v1.6 | integration (full suite) | `bash scripts/mailpit-verify.sh` (default, unscoped) | ✓ — script exists; the regression itself just needs a clean full run captured with true-exit-before-pipe |
| LIVE-02 | Dedicated negative test per new sink (process.exec, fs write/edit) | integration (cfg(linux) for exec; cross-platform for fs) | `cargo test -p caprun --test s9_process_exec_block` / `cargo test -p caprun --test s9_file_write_block` | ✓ — both already exist from Phase 32/33; confirm they still pass, and add the process.exec confirm-release negative leg (entry-guard) from EXEC-05 above |

### Sampling Rate
- **Per task commit:** the relevant `cargo test -p brokerd`/`cargo test -p caprun --test <name>` scoped run (macOS shows 0 Linux-gated tests run — expected).
- **Per wave merge:** `cargo build --tests --workspace --keep-going` in the `rust:1` Colima container (D-15).
- **Phase gate:** full `bash scripts/mailpit-verify.sh` (default, unscoped) green, true-exit-0 before any pipe, BEFORE the composed live proof (D-15's ordering requirement) and again as the final LIVE-02 regression.

### Wave 0 Gaps
- [ ] `crates/brokerd/src/sinks/process_exec.rs` — add `invoke_process_exec_from_resolved` + its own `#[cfg(test)] mod tests` (mirrors `file_write.rs`'s unit-test shape).
- [ ] `crates/brokerd/src/confirmation.rs` — extend Step 4.75 guard, Step 7 dispatch, async signature; extend its own `#[cfg(test)] mod tests` if a unit-level guard/dispatch test is added.
- [ ] `cli/caprun/src/main.rs` — `run_confirm_or_deny` becomes async, `.await`ed at its call site.
- [ ] `cli/caprun/tests/s9_process_exec_block.rs` — extend `mod linux` with the confirm-release acceptance leg (D-11).
- [ ] A new composed live-proof test file (name TBD, Claude's Discretion) for LIVE-01, mirroring `live_acceptance_v1_4_composed.rs`.

## Security Domain

`security_enforcement` is absent from `.planning/config.json` → treat as
enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No auth surface changes this phase. |
| V3 Session Management | no | Session-trust-state (I0/I1) is untouched — EXEC-05 is confirm-release plumbing only. |
| V4 Access Control | yes | I2 (executor table-entries-only Block) is the access-control primitive under test; this phase must NOT weaken it (D-08/D-09/D-10). |
| V5 Input Validation | yes | `resolved_literal`/arg-schema validation already exists (Phase 32/33 tables); confirm-release reuses the SAME frozen, already-validated `resolved_args` — no new validation surface. |
| V6 Cryptography | yes (indirect) | The `pending_confirmations` whole-row HMAC-SHA256 (v1.6 HARDEN-02) and the keyed audit-chain MAC are already in place and unmodified by this phase — never hand-roll a new MAC scheme here. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Confirm-release double-spawn (re-running `caprun confirm` twice executes the command twice) | Repudiation / Tampering | `transition_state`'s SQL `WHERE state = 'pending'` guard (CONFIRM-03) — already generic, applies to process.exec with zero changes. |
| Guard/dispatch drift silently burning a one-shot confirmation with no effect (the exact Phase-33 MAJOR-1 bug this phase fixes for process.exec) | Denial of Service (self-inflicted) / audit-DAG gap | Step 4.75 entry-guard, kept in sync with Step 7 (Pitfall 1 above). |
| Confirm-release path bypassing I2 by re-invoking `executor::submit_plan_node` | Elevation of Privilege | `confirm()`'s doc comment (confirmation.rs:713-722) already states it NEVER calls `submit_plan_node` — `invoke_process_exec_from_resolved` must uphold the same invariant (CON-i2-non-bypassable, T-10-05). |
| A stapled (fabricated) taint anchor for the confirm-release mint | Tampering | `mint_from_exec`'s `provenance_chain == [sink_event_id]` where `sink_event_id` is the REAL `process_exited` event id `invoke_process_exec_from_resolved` just durably appended — never a fresh/fabricated root (mirrors the Allow-path's own anti-stapling discipline). |

## Sources

### Primary (HIGH confidence — direct code reads this session)
- `crates/brokerd/src/sinks/process_exec.rs` — `invoke_process_exec`, `run_launcher`, `resolve_launcher_path`, Gate-3 doc comment.
- `crates/brokerd/src/sinks/file_write.rs` — `invoke_file_write`, `invoke_file_write_from_resolved`, unit tests.
- `crates/brokerd/src/sinks/file_create.rs` — `invoke_file_create`, `invoke_file_create_from_resolved`, unit tests.
- `crates/brokerd/src/confirmation.rs` — full module (combined_digest, PendingConfirmation, confirm(), deny(), Step 4.75 guard, Step 7 dispatch).
- `crates/brokerd/src/server.rs` — `evaluate_plan_node_and_record` (Allow-path process.exec dispatch + `mint_from_exec` call site), `PendingConfirmation` construction at Block time.
- `crates/brokerd/src/quarantine.rs` — `mint_from_exec` definition + its own unit test.
- `scripts/check-invariants.sh` — Gate 1, Gate 3 mint-site allow-list + inline-annotation exemption mechanism.
- `scripts/mailpit-verify.sh` — Linux verification harness, `MAILPIT_VERIFY_CMD` scoping.
- `scripts/verify-harden04-featureless.sh` — true-exit-before-pipe pattern (`set +e; ...; rc=$?; set -e`).
- `cli/caprun/tests/confirm.rs` — cross-process `caprun confirm`/`deny` subprocess harness, `seed_pending_*_block` pattern.
- `cli/caprun/tests/s9_process_exec_block.rs` — Linux-gated exec-block acceptance test, role-unconstrained `command` arg finding.
- `cli/caprun/tests/s9_file_write_block.rs` — cross-platform fs-write-block acceptance test (contrast for Pitfall 3).
- `cli/caprun/src/main.rs` — `run_confirm_or_deny`, `#[tokio::main]` structure, call-site of `run_confirm_or_deny`.
- `.planning/phases/34-regression-live-proof-v1-7-done/34-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md` — locked scope/decisions.
- `CLAUDE.md` — TCB/effect-path/terminology constraints, Linux verification recipe.

### Secondary (MEDIUM confidence)
- `cli/caprun/tests/live_acceptance_v1_4_composed.rs` (doc comment only, not full body) — composed multi-leg shared-audit.db pattern.

### Tertiary (LOW confidence)
- None — every claim in this document traces to a direct file read this session.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; existing `tokio` features already cover the async spawn need.
- Architecture: HIGH — every pattern cited is an exact, currently-compiling reference implementation in the repo.
- Pitfalls: HIGH — each pitfall is derived from an explicit doc comment, an existing test's own header rationale, or a structural signature mismatch discovered by direct comparison of the two call sites.

**Research date:** 2026-07-18
**Valid until:** Should remain valid through Phase 34's execution (this is a terminal phase of v1.7; no further phases depend on this research surviving beyond milestone close).
