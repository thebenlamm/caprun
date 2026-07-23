# Phase 10: Single-Shot Confirmation Loop - Research

**Researched:** 2026-07-07
**Domain:** Rust TCB extension — durable cross-process pause/resume workflow, SQLite side-table persistence, second CLI verb dispatch (internal architecture only; no new external libraries)
**Confidence:** HIGH

## Summary

There is no CONTEXT.md for this phase (discuss-phase was skipped). Its role is filled entirely by
`planning-docs/DESIGN-confirmation-release.md`, an approved design doc (gated under
`DESIGN-GATE-RECORD-v1.2.md`, same `DEC-ai-review-satisfies-human-gate` provenance as Phase 9's
gate). That document is exhaustively prescriptive: it names the exact `PendingConfirmation` struct
shape, the exact Step 1-4 decision logic, the exact CLI output format, and the exact exit-code
table. **This phase is implementation against an already-locked spec, not open design.** The job of
this research is to map every MUST in that document onto the current shape of `crates/brokerd`,
`crates/executor`, `crates/runtime-core`, and `cli/caprun` — verified by direct reading of the live
source this session — and to surface the mechanical gaps the design doc does not resolve.

Five concrete, verified gaps drive the plan:

1. **`cli/caprun` has no subcommand dispatch framework.** `main.rs` parses positional args directly
   (`intent_kind`, `intent_param`, `workspace_path`, `audit_path`) with no `clap` or similar crate in
   the workspace dependency list. `caprun confirm <effect_id>` / `caprun deny <effect_id>` must be
   recognized as new first-argument branches in (or alongside) this same binary, dispatched BEFORE
   the existing intent-kind match, not bolted on as an interactive mode of the existing flow.
2. **`invoke_file_create` resolves args via the in-memory `ValueStore`, which will not exist in the
   confirm process.** `crates/brokerd/src/sinks/file_create.rs::invoke_file_create` calls
   `value_store.resolve(&arg.value_id)` to get the literal at call time. The design doc's
   `PendingConfirmation.resolved_args` is a frozen snapshot specifically because this store is gone
   by confirm time — so Phase 10 needs a NEW invocation path (e.g.
   `invoke_file_create_from_resolved`) that takes literals directly, not a call into the existing
   `ValueStore`-dependent function. Reusing the existing function signature unmodified is not
   possible without either faking a `ValueStore` (defeats the design doc's snapshot rationale) or
   changing its parameter type.
3. **No mechanism currently persists the workspace root path anywhere in the audit DB or session
   record.** `cli/caprun/src/main.rs` derives `WorkspaceRoot` from the CLI-supplied `workspace_path`'s
   parent directory at process-start time; `adapter_fs::workspace::WorkspaceRoot`'s Linux path holds
   only a `dirfd` (no path field) — only the non-Linux stub retains `root_path: PathBuf`. The design
   doc's CLI contract (`caprun confirm <effect_id> [audit-db-path]`) does not mention a
   workspace-path argument, yet re-invoking `file.create` at confirm time requires opening the SAME
   workspace root the original blocked plan node targeted. This is a genuine gap the design doc does
   not close — see Open Questions.
4. **The `events` table has no indexed `effect_id` column** — `effect_id` only exists inside
   `SinkBlockedAnchor`, itself nested inside the JSON `payload` column. The design doc anticipates
   this (`find_pending_confirmation` — "a new capability"); the natural fix is that the NEW
   `pending_confirmations` side table is keyed by `effect_id` as its `PRIMARY KEY`, giving an indexed
   lookup for free without touching the `events` table at all.
5. **`confirm_granted`/`confirm_denied` events need a way to carry `effect_id`** despite `Event`
   having no `effect_id` column (adding one breaks the golden byte-fixture / no-DB-migration
   invariant per `DESIGN-plan-executor.md` §5, mirrored in `DESIGN-confirmation-release.md`'s own
   framing). The existing precedent (`crates/brokerd/src/sinks/file_create.rs`'s
   `format!("sink:file.create:{effect_id}")` actor-field convention) is the pattern to reuse — the
   `effect_id` rides in `actor`, not a new column, and the causal `parent_id` link to the
   `sink_blocked` event provides the DAG-traversable anchor the design doc calls "anchored to
   `effect_id`."

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CONFIRM-01 | `caprun confirm <effect_id>` displays verbatim literal + provenance | `DESIGN-confirmation-release.md`'s exact output format (reproduced in Code Examples); `PendingConfirmation.resolved_args` supplies literal/taint/provenance_chain without re-resolving |
| CONFIRM-02 | Confirm releases exactly one `(sink, arg, literal-digest)` triple, single-shot | `PendingConfirmationState` one-way transition (`Pending`→`Confirmed`/`Denied`, never reversed); confirm never consults or writes any allowlist/policy — verified no such structure exists in `crates/executor` or `crates/brokerd` today |
| CONFIRM-03 | Deny is durable; same `effect_id` can never later be confirmed | `state` column read from persisted DB (never in-memory) on every invocation; Step 3 refuses any non-`Pending` state — this MUST be implemented as a DB read, not a cached in-process check, because confirm/deny are always separate processes |
| CONFIRM-04 | Confirm/deny audited, anchored to `SinkBlockedAnchor.effect_id`, release path in TCB | `effect_id` carried in `actor` field (existing project convention, see Gap 5); confirm/deny logic lives in `crates/brokerd` (broker-owned, same trust tier as `append_event`/`invoke_file_create`), never a policy file |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `PendingConfirmation` persistence (write at Block time, read at confirm time) | `brokerd` (audit/side-table layer) | — | Mirrors the existing `blocked_literals` side-table pattern; `brokerd` already owns the SQLite connection and all side-table read/write functions |
| `caprun confirm` / `caprun deny` CLI verbs | `cli/caprun` (orchestrator) | `brokerd` (decision logic) | The CLI's role stays "parse args, open DB, call into brokerd, print result" — exactly the role it already plays in `main.rs`'s existing flow. The actual state-transition + terminal-check + sink re-invocation logic is broker-owned (TCB), never inlined in `main.rs` |
| Terminal-state check (Pending/Confirmed/Denied) | `brokerd` | — | Must be a Rust function reading the DB directly — never a policy file, never re-derived from `submit_plan_node` |
| Sink re-invocation from frozen snapshot | `brokerd` (`sinks/file_create.rs` extension) | `adapter-fs` (unchanged — same `create_exclusive_within` syscall) | Confirm MUST NOT re-run `executor::submit_plan_node`; it invokes the sink adapter directly using already-adjudicated literals, exactly as the existing `Allowed` path does today, just sourced from `resolved_args` instead of `ValueStore::resolve` |
| Confirm/deny audit event append (`confirm_granted`/`confirm_denied`) | `brokerd` (`audit.rs` + `Event` constructors) | `runtime-core` (no new `Event` field; reuse `Event::new`) | Same append-only, hash-chained `events` table already in place; no schema change to `events` needed |
| Workspace-root resolution at confirm time | `brokerd` (persist path) or `cli/caprun` (accept as arg) | — | UNRESOLVED by the design doc — see Open Questions. Must be decided by the planner, not left implicit |

## Standard Stack

No new external crates are required. Every capability this phase needs already exists in the
workspace dependency graph verified this session:

| Library | Version (workspace-pinned) | Purpose | Why no new dependency |
|---------|------|---------|------------------------|
| `rusqlite` | `0.32` (bundled) [VERIFIED: codebase — `Cargo.toml` workspace.dependencies] | New `pending_confirmations` side table + indexed lookup | Same connection/schema pattern as `blocked_literals` |
| `uuid` | `1.23.4` (v4, serde) [VERIFIED: codebase] | `effect_id` parsing from CLI arg string → `Uuid` | `Uuid::parse_str` already used implicitly via `serde`/`FromStr` |
| `serde` / `serde_json` | `1.0.228` / `1.0.150` [VERIFIED: codebase] | `PendingConfirmation` (de)serialization for the side-table row | Same derive pattern as every other domain type in `runtime-core` |
| `anyhow` | `1.0.103` [VERIFIED: codebase] | Error propagation in the new confirm/deny functions | Matches every existing `brokerd`/`cli` function signature |
| `chrono` | `0.4.45` [VERIFIED: codebase] | `Event.timestamp` on the new `confirm_granted`/`confirm_denied` events | Unchanged from existing `Event::new` call sites |

**No `clap` (or any CLI-parsing crate) is in the workspace today** [VERIFIED: codebase — grep of
`Cargo.toml` workspace.dependencies and `cli/caprun/Cargo.toml`]. Adding one would be the "obvious"
choice for a real subcommand CLI, but it is a new dependency for a codebase whose existing CLI
parsing is 100% manual (`main.rs`'s `raw_args`/`idx` walk). Given the project's surgical-change
discipline and that only two new verbs are being added, **recommend extending the existing manual
parsing** (branch on `raw_args[0] == "confirm" | "deny"` before the current intent-kind match)
rather than introducing `clap` for two subcommands. This is Claude's discretion absent a locked
decision — flag it for the planner to confirm, not something research can lock unilaterally.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Extending `cli/caprun/src/main.rs` with a first-arg branch | A third `[[bin]]` target (e.g. `caprun-confirm`) | A separate binary avoids touching the existing intent-parsing code path at all, but duplicates the `open_audit_db`/arg-parsing boilerplate and splits "the caprun CLI" across three binaries instead of one with verbs — the design doc's CLI contract (`caprun confirm <effect_id>`) reads as one binary, multiple verbs, not multiple binaries |
| Manual arg parsing (current pattern) | `clap` derive macros | `clap` scales better if more verbs are added later, but is a new dependency for exactly two additional branches on top of already-manual parsing; inconsistent with the rest of the CLI unless the whole CLI is migrated, which is out of scope |

**Installation:** None — no `cargo add` needed for this phase.

## Package Legitimacy Audit

Not applicable — this phase introduces zero new external packages. All required functionality
(SQLite side table, UUID parsing, JSON serialization) is provided by crates already present and
pinned in the workspace `Cargo.toml` (verified above).

## Architecture Patterns

### System Architecture Diagram

```
Process 1 (original blocking run — unchanged flow, extended at one point)
──────────────────────────────────────────────────────────────────────────
caprun <intent-kind> ... ──▶ broker (SubmitPlanNode) ──▶ executor::submit_plan_node
                                                              │
                                              BlockedPendingConfirmation { anchor, literal }
                                                              │
                                                              ▼
                             Event::sink_blocked appended (existing, unchanged)
                                                              │
                                              ┌───────────────┴────────────────┐
                                              │  NEW: PendingConfirmation row   │
                                              │  written atomically alongside  │
                                              │  the sink_blocked event, in the │
                                              │  SAME transaction/lock          │
                                              └───────────────┬────────────────┘
                                                              │
                                                    process exits non-zero
                                                    (nothing survives in memory)

Process 2 (LATER, separate invocation — human-initiated)
──────────────────────────────────────────────────────────────────────────
caprun confirm <effect_id> [audit-db-path]
        │
        ▼
  reopen SAME persistent SQLite file (never :memory:)
        │
        ▼
  find_pending_confirmation(conn, effect_id)  ── indexed lookup on new table's PRIMARY KEY
        │
        ├── None ─────────────────────────────▶ "unknown effect_id", exit non-zero
        │
        ├── state != Pending ─────────────────▶ "already terminal", exit non-zero
        │
        └── state == Pending
                │
                ▼
        display literal + taint + provenance (CONFIRM-01)
                │
                ▼
        append confirm_granted Event, parent_id = sink_blocked event id
                │
                ▼
        PendingConfirmation.state: Pending → Confirmed  (persisted BEFORE sink call)
                │
                ▼
        invoke sink adapter using resolved_args (FROZEN literals — no ValueStore, no
        re-run of submit_plan_node)
                │
        ┌───────┴────────┐
        ▼                ▼
  sink succeeds    sink fails (e.g. O_EXCL conflict)
  exit 0           append sink_invocation_failed Event, exit non-zero (distinct code)

caprun deny <effect_id> [audit-db-path]  — same Steps 1-3, then:
        │
        ▼
  append confirm_denied Event, parent_id = sink_blocked event id
        │
        ▼
  PendingConfirmation.state: Pending → Denied (terminal, no retry), exit non-zero
```

### Recommended Project Structure

```
crates/brokerd/src/
├── audit.rs                    # ADD: pending_confirmations DDL, insert/find/transition fns
├── confirmation.rs             # NEW: PendingConfirmation struct, confirm/deny decision logic
│                                #      (Steps 1-4 from the design doc) — the TCB-resident module
└── sinks/
    └── file_create.rs          # ADD: invoke_file_create_from_resolved (frozen-literal variant)

crates/runtime-core/src/
└── (no new types needed — PendingConfirmation is brokerd-internal per the design doc's own framing:
    "a NEW, DISTINCT durable record" that is NOT part of the hashed Event/anchor chain)

cli/caprun/src/
└── main.rs                     # ADD: first-arg branch for "confirm" / "deny" BEFORE the existing
                                 #      intent-kind parse; new dispatch functions call into
                                 #      brokerd::confirmation
```

### Pattern 1: Atomic side-table write alongside the anchoring event

**What:** `PendingConfirmation` MUST be persisted in the same transaction/connection-lock scope as
the `sink_blocked` Event append — never as a separate, later write.
**When to use:** Any time a durable side-table row's existence is a precondition for a later,
security-relevant read (mirrors `insert_blocked_literal`'s existing call-site discipline in
`server.rs`'s `SubmitPlanNode` arm).
**Example:**
```rust
// Source: existing pattern, crates/brokerd/src/server.rs SubmitPlanNode arm (verified this session)
let new_hash = {
    let locked = conn.lock().map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
    let hash = append_event(&locked, &audit_event, Some(last_event_hash))?;
    if let Some(literal) = &blocked_literal {
        crate::audit::insert_blocked_literal(&locked, &audit_event.id.to_string(), literal)?;
    }
    // Phase 10 ADDS here, same lock scope:
    // crate::confirmation::insert_pending_confirmation(&locked, &pending_confirmation)?;
    hash
};
```

### Pattern 2: `effect_id` carried in `actor`, not a new `Event` column

**What:** Every event that needs to reference an `effect_id` (post-Block) encodes it into the
`actor` string field, following the exact convention already used by `invoke_file_create`'s
`sink_executed`/`sink_execution_failed` events (`format!("sink:file.create:{effect_id}")`).
**When to use:** `confirm_granted` and `confirm_denied` events — e.g. `actor:
format!("confirm:{effect_id}")` / `format!("deny:{effect_id}")`.
**Example:**
```rust
// Source: crates/brokerd/src/sinks/file_create.rs (verified this session)
let event = Event::new(
    Uuid::new_v4(),
    Some(parent_id),           // the sink_blocked event's id — preserves the causal chain
    session_id,
    format!("confirm:{effect_id}"),   // Phase 10's analogous convention
    "confirm_granted".into(),
    Utc::now(),
    vec![],
);
```

### Pattern 3: Frozen-snapshot sink re-invocation (never re-resolve, never re-submit)

**What:** The confirm path invokes the sink adapter directly with literals already captured in
`PendingConfirmation.resolved_args` — it does not call `executor::submit_plan_node` a second time
and does not need a live `ValueStore`.
**When to use:** `caprun confirm`'s Step 4a.4 (design doc). This requires a NEW function alongside
(not replacing) `invoke_file_create`, since the existing function's signature is hardwired to
`&ValueStore`.
**Example:**
```rust
// Illustrative shape — Phase 10 implements exactly this against ResolvedArg from the design doc.
pub fn invoke_file_create_from_resolved(
    conn: &rusqlite::Connection,
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],   // frozen literals, no ValueId resolution
    workspace_root: &WorkspaceRoot,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let path = resolved_args.iter().find(|a| a.name == "path")
        .ok_or_else(|| anyhow::anyhow!("missing path"))?;
    let contents = resolved_args.iter().find(|a| a.name == "contents")
        .ok_or_else(|| anyhow::anyhow!("missing contents"))?;
    workspace_root.create_exclusive_within(&path.literal, contents.literal.as_bytes())?;
    // ... same two-phase sink_executed / sink_execution_failed audit pattern as invoke_file_create
}
```

### Anti-Patterns to Avoid

- **Re-submitting the original `PlanNode` to `executor::submit_plan_node` on confirm:** taint is
  monotonic (never cleared), so this either re-Blocks forever (no-op confirm) or requires a
  special-cased bypass inside the TCB deny function itself — exactly the "policy file disables I2"
  failure mode the project's `CON-i2-non-bypassable` forbids. The design doc calls this out
  explicitly as the critical soundness rule.
- **Checking `PendingConfirmationState` from any in-memory/cached value:** confirm and deny are
  ALWAYS separate OS processes from the one that created the block (and potentially from each
  other, on a double-invocation). Every state check MUST re-read the persisted DB row.
- **Adding an `effect_id` column to the `events` table:** breaks the existing golden byte-fixture
  test (`crates/runtime-core/src/event.rs`'s `anchor_none_event_serializes_byte_identical_and_round_trips`)
  and requires a DB migration. Use the new side table's own `PRIMARY KEY` instead.
- **Interactive TTY prompt inside the original blocking `caprun` process:** explicitly out of scope
  per `REQUIREMENTS.md`'s Out of Scope table and the design doc's CLI contract — confirm/deny MUST
  be scriptable second commands, never a blocking-process stdin prompt.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Digest of the blocked literal for redaction-tamper-evidence | A new hashing scheme | `sha2::Sha256` (already a workspace dependency, already used for `literal_sha256` in `SinkBlockedAnchor`) | One digest algorithm for the whole project; reusing it avoids a second crypto primitive to audit |
| Effect-id indexed lookup | A manual linear scan over `events.payload` JSON | A `PRIMARY KEY effect_id` column on the new `pending_confirmations` table | SQLite gives this for free; scanning JSON payloads for a nested `effect_id` field is both slow and fragile |
| Atomicity of the two-write (event + side table) | Manual two-phase commit / retry logic | The connection's existing `Mutex`-guarded single lock scope (same pattern as `insert_blocked_literal`) | The codebase already has this exact pattern proven correct; a new locking scheme is unnecessary complexity |
| CLI argument parsing for two new verbs | `clap` (new dependency) OR a hand-rolled state machine | Extend the existing manual `raw_args`/`idx` walk in `main.rs` with one more branch | Matches existing code exactly; avoids introducing an asymmetric parsing style for just two verbs (see Standard Stack discussion) |

**Key insight:** Every mechanism this phase needs (hashing, atomic side-table writes, indexed
lookup, event chaining) already has a proven, reviewed precedent somewhere in `crates/brokerd`. The
work is disciplined reuse of those exact patterns for a new data shape (`PendingConfirmation`), not
invention of new mechanisms.

## Common Pitfalls

### Pitfall 1: Re-resolving args via `ValueStore` at confirm time
**What goes wrong:** A naive port of `invoke_file_create` tries to call `value_store.resolve()`
against a fresh, empty `ValueStore` in the confirm process — every resolve returns `None`, so the
function fails or (worse) is silently patched to accept literals as if they were `ValueId`s.
**Why it happens:** `invoke_file_create`'s current signature takes `&ValueStore` and the temptation
is to reuse it unmodified.
**How to avoid:** Write a distinct function (Pattern 3 above) that takes `&[ResolvedArg]` directly —
never a `ValueStore` — matching the design doc's explicit framing that resolved_args is "captured at
Block time," not re-derived.
**Warning signs:** Any confirm-path code that constructs a fresh `ValueStore::default()` or that
calls `store.mint(...)` during confirm — minting during confirm would violate the "never re-run
`submit_plan_node`" rule and is a sign the wrong code path was reused.

### Pitfall 2: Workspace root path unavailable at confirm time
**What goes wrong:** `caprun confirm <effect_id>` cannot open the workspace root to actually create
the file, because nothing today persists the workspace-root path anywhere the second process can
read it. `WorkspaceRoot` on Linux holds only a `dirfd`, not a path.
**Why it happens:** The original `caprun` process derives `WorkspaceRoot` from a CLI argument
(`workspace_path`'s parent) at process-start; that argument is gone once the process exits, and
`PendingConfirmation`'s design-doc schema (as written) does not include a workspace-root field.
**How to avoid:** This MUST be resolved during planning (see Open Questions) — either (a) add a
`workspace_root_path: String` field to `PendingConfirmation` at persistence time (the broker already
has `workspace_root: Arc<WorkspaceRoot>` in scope when the block occurs — it needs the underlying
path string, which the CLI currently discards after deriving it), or (b) require `caprun confirm` to
take an explicit workspace-path argument, extending the design doc's CLI contract.
**Warning signs:** A plan that has `caprun confirm` succeed against `:memory:`/toy tests but has no
answer for "what directory does `create_exclusive_within` actually target" in a real multi-process
run.

### Pitfall 3: Non-atomic state transition + sink invocation treated as a single unit
**What goes wrong:** Treating "mark Confirmed" and "invoke the sink" as one atomic operation leads
to ambiguity when the sink invocation itself fails (a SQLite transaction cannot wrap a syscall like
`openat2`).
**Why it happens:** It is tempting to want "exactly-once" semantics, but the design doc is explicit
that this is **at-most-once**: the state transitions to `Confirmed` BEFORE the sink runs, and if the
sink then fails, the effect is permanently `Confirmed` with no retry (a distinct
`sink_invocation_failed` event records this).
**How to avoid:** Persist `state = Confirmed` first (own transaction/lock scope), THEN invoke the
sink; on sink failure, append `sink_invocation_failed` and return a distinct non-zero exit code —
never attempt to roll the state back to `Pending`.
**Warning signs:** Any code path that resets `PendingConfirmationState` back to `Pending` after a
sink failure, or that wraps both the DB write and the filesystem syscall in one `rusqlite`
transaction (impossible — the syscall isn't part of the SQL transaction).

### Pitfall 4: Redaction of `blocked_literals` not propagated to `PendingConfirmation.resolved_args`
**What goes wrong:** `blocked_literals` is redactable (existing v1.1 feature: `redact_blocked_literal`
deletes the row). If `PendingConfirmation.resolved_args` holds its own independent copy of the same
literal, redacting only `blocked_literals` leaves the sensitive literal still readable via
`caprun confirm`'s display step — silently defeating redaction.
**Why it happens:** `PendingConfirmation` is a NEW side table, separate from `blocked_literals`, by
design (it needs the FULL arg set, not just the one blocked arg) — but that separateness means a
redaction operation touching one table does not automatically touch the other.
**How to avoid:** Either (a) redact both side tables in the same operation (extend
`redact_blocked_literal` or add a paired redaction function), or (b) have `caprun confirm` check
`blocked_literals` first and refuse to release (fail closed) if that entry has already been
redacted, per the design doc's explicit "MUST either... or..." framing.
**Warning signs:** A redaction test that only asserts `blocked_literals` is empty, without also
asserting `caprun confirm` refuses or that `resolved_args`' copy is also gone.

### Pitfall 5: Terminal-state check implemented as an in-process cache
**What goes wrong:** A plan that stores "already decided" state in any process-local variable
(instead of re-reading the DB row on every invocation) will pass a same-process unit test but fail
the real cross-process scenario the design doc requires (confirm-then-confirm-again as two separate
`caprun` invocations).
**Why it happens:** Rust unit tests naturally construct one connection and call functions in
sequence within the same test function, which can mask the requirement to always re-query.
**How to avoid:** Every test exercising CONFIRM-03 MUST spawn the confirm/deny logic against a
FRESH read of the persisted row (or, ideally, drive the real compiled binary twice via
`env!("CARGO_BIN_EXE_caprun")`, following the existing `cli/caprun/tests/e2e.rs` pattern) — not just
call an in-process function twice with the same `PendingConfirmation` struct held in a local
variable.
**Warning signs:** A test that constructs one `PendingConfirmation` value and passes it by mutable
reference to both the confirm and the re-confirm assertion, rather than re-fetching from `conn`.

### Pitfall 6: New CLI verbs collide with existing intent-kind dispatch
**What goes wrong:** `main.rs`'s current `match intent_kind.as_str()` only knows about
`"send-email-summary"` / `"create-file-from-report"` and calls `anyhow::bail!("unknown intent kind:
...")` for anything else — including, today, the strings `"confirm"` and `"deny"`. If the new verbs
are added AFTER the existing arg-parsing (workspace-file, audit-path positionals already consumed),
the CLI will try to parse `<effect_id>` as a workspace-file path and fail with a confusing error
instead of a clean confirm/deny dispatch.
**Why it happens:** The existing `--seed-from-file` flag precedent shows the CLI already special-
cases its first argument(s) before the general intent-kind parse — the same discipline must extend
to `confirm`/`deny`, checked even earlier (before `--seed-from-file` parsing, since confirm/deny take
a completely different argument shape: `<effect_id> [audit-db-path]`, not `<intent-kind>
<intent-param> <workspace-file> [audit-db-path]`).
**Warning signs:** A plan that adds the confirm/deny branch anywhere after the `--seed-from-file`
check or the `intent_kind` parse, rather than as the very first branch in `main()`.

## Code Examples

### Exact required CLI output format (CONFIRM-01)
```
// Source: planning-docs/DESIGN-confirmation-release.md (approved design doc, verbatim)
$ caprun confirm <effect_id>

Effect blocked pending confirmation.

Effect ID:         <effect_id>
Sink:               file.create
Arg:                path
Literal value:      "../../etc/passwd"
Taint:              [external.untrusted, path.raw]
Source:             file_read evt_a1b2c3...  (session <session_id>)
Provenance chain:   evt_a1b2c3... -> evt_d4e5f6... -> (this arg)

This value came from untrusted content read during this session. Run
`caprun confirm <effect_id>` to release this EXACT value, or
`caprun deny <effect_id>` to block it permanently.
```

### Exit-code contract (CONFIRM-02/03, exact table from the design doc)
```
// Source: planning-docs/DESIGN-confirmation-release.md
| Outcome                                                              | Exit code |
|-----------------------------------------------------------------------|-----------|
| Confirm succeeds; sink invoked                                        | 0         |
| Confirm recorded but sink invocation failed                           | non-zero, distinct |
| Deny recorded                                                         | non-zero  |
| Unknown effect_id                                                     | non-zero  |
| effect_id already terminal (Confirmed or Denied) — refused             | non-zero  |
```
Only a successful confirm-and-release returns 0 — every other path is non-zero, and the
confirm-recorded-but-sink-failed case MUST be distinguishable by exit code alone (no stdout parsing).

### Existing `blocked_literals` DDL pattern to mirror for `pending_confirmations`
```rust
// Source: crates/brokerd/src/audit.rs (verified this session) — mirror this shape exactly
CREATE TABLE IF NOT EXISTS blocked_literals (
    event_id TEXT PRIMARY KEY,
    literal  TEXT NOT NULL
) STRICT;
// Phase 10 adds an analogous table, e.g.:
// CREATE TABLE IF NOT EXISTS pending_confirmations (
//     effect_id     TEXT PRIMARY KEY,
//     session_id    TEXT NOT NULL,
//     sink          TEXT NOT NULL,
//     resolved_args TEXT NOT NULL,   -- JSON-serialized Vec<ResolvedArg>
//     state         TEXT NOT NULL    -- "pending" | "confirmed" | "denied"
// ) STRICT;
```

## State of the Art

Not applicable in the "library churn" sense — there is no external framework whose API changed. The
one relevant "old approach → current approach" shift is internal to this project's own design
history:

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| (hypothetical) in-memory callback / waiting process for confirmation | Durable pause-and-resume via a persisted `PendingConfirmation` side table, resumed by a wholly separate later process | Locked in `DESIGN-confirmation-release.md` (approved this milestone) | Confirm/deny MUST NOT assume any in-memory state survives from the blocking process — this is the single most load-bearing constraint in the whole design doc |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `caprun confirm`/`caprun deny` should be added as new first-argument branches on the existing `caprun` binary (not a new `[[bin]]` target, not `clap`) | Standard Stack / Architecture Patterns | Low — this is a structural/style choice, not a security-load-bearing one; if the planner or user prefers a separate binary or `clap`, the underlying `brokerd::confirmation` logic is unaffected either way |
| A2 | The workspace-root-path gap (Pitfall 2) should be closed by persisting `workspace_root_path` inside `PendingConfirmation` rather than adding a new CLI argument | Common Pitfalls #2 / Open Questions | Medium — if the planner instead chooses the CLI-argument route, the exact CLI contract in `DESIGN-confirmation-release.md` (`caprun confirm <effect_id> [audit-db-path]`) would need an amendment, which may require a design-doc update or at minimum an explicit STATE.md decision entry |

**None of these assumptions touch the security-invariant substance of CONFIRM-01..04** — both are
mechanism/plumbing choices the approved design doc leaves open, not reinterpretations of a MUST.

## Open Questions

1. **How does `caprun confirm <effect_id>` obtain the workspace root to actually re-invoke
   `file.create`?**
   - What we know: `DESIGN-confirmation-release.md`'s CLI contract shows only
     `caprun confirm <effect_id> [audit-db-path]` — no workspace-path argument. The broker has
     `Arc<WorkspaceRoot>` in scope at Block time (verified in `server.rs`'s `dispatch_request`), but
     `WorkspaceRoot`'s Linux variant does not retain the path string, only a `dirfd`.
   - What's unclear: whether the design doc's omission was intentional (implying the planner should
     add a `workspace_root_path` field to `PendingConfirmation`, persisted at Block time using the
     PATH the CLI originally passed in — before it's turned into a `dirfd`) or an oversight the
     planner must flag back to the design-gate process.
   - Recommendation: persist `workspace_root_path: String` in `PendingConfirmation` at Block-time —
     the CLI/broker already has this string available (`workspace_root_dir` in `main.rs`) before it
     opens the dirfd; capturing it costs nothing and keeps `caprun confirm`'s CLI surface exactly as
     the design doc specifies. Confirm this plumbing choice explicitly in the plan rather than
     discovering it mid-implementation.

2. **Should `pending_confirmations.resolved_args` be a JSON blob column, or normalized into its own
   child table (one row per `ResolvedArg`)?**
   - What we know: the design doc's Rust shape is `Vec<ResolvedArg>` per `PendingConfirmation`; the
     project's existing pattern for structured-but-nested data (e.g. `Event.payload`) is "serialize
     the whole struct to a JSON TEXT column," never a normalized join table.
   - What's unclear: nothing security-relevant — this is a pure implementation-ergonomics choice.
   - Recommendation: follow the existing `events.payload` precedent — one JSON TEXT column for the
     full `Vec<ResolvedArg>`, deserialized on read. Simpler, and consistent with how `Event` itself
     is stored.

3. **Does `email.send`'s stub sink need a confirm-time re-invocation path at all?**
   - What we know: `crates/brokerd/src/sinks/email_send.rs::invoke_email_send_stub` performs no real
     network effect today — it only appends an `email_send_stub` audit event. `file.create` is the
     only LIVE side-effecting sink in scope (per `REQUIREMENTS.md`'s Out of Scope table: "More sinks
     beyond file.create/email.send — Linear engineering, proves nothing new").
   - What's unclear: whether Phase 10's confirm path needs a generic `match plan_node.sink.0 { ... }`
     dispatch (mirroring `server.rs`'s existing `if plan_node.sink.0 == "file.create"` check) that
     handles both sinks, or whether email.send's stub can just re-append its stub event on confirm
     without any real re-invocation logic.
   - Recommendation: mirror the existing `SubmitPlanNode` arm's sink dispatch pattern exactly — a
     match/if-chain by `sink.0`, with `file.create` doing the real frozen-literal re-invocation
     (Pattern 3) and `email.send` doing the equivalent of its existing no-op stub append. This keeps
     confirm's sink-dispatch shape parallel to the Allow path's existing shape, which the plan-checker
     can verify against.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| SQLite (via `rusqlite` bundled feature) | `pending_confirmations` side table | ✓ | bundled 0.32 [VERIFIED: codebase] | — (bundled, no system dependency) |
| Linux kernel ≥5.13 (Landlock) + `openat2`/`RESOLVE_BENEATH` | Real `create_exclusive_within` re-invocation on confirm | ✗ on this dev Mac; only in Colima/Docker per `CLAUDE.md` | — | `adapter_fs::workspace::WorkspaceRoot`'s existing `#[cfg(not(target_os = "linux"))]` stub already provides a macOS-safe fallback path (plain `std::fs::File::options().create_new(true)`), so confirm-path unit tests are macOS-runnable; only the live Linux-only e2e/acceptance test (Phase 11, ACC-02) requires the Colima+Docker recipe already documented in this project's memory |

**Missing dependencies with no fallback:** none — the existing cfg-gated Linux/non-Linux split
already covers this phase's needs exactly as it does for every prior sink-invocation phase.

**Missing dependencies with fallback:** Linux-only kernel security enforcement (see above); use the
documented Colima+Docker recipe for the live acceptance run, per `CLAUDE.md`'s "Linux-only security
tests" section.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `cargo test` (no external test framework; no config file) |
| Config file | none — `Cargo.toml` `[[test]]` targets per crate (see `cli/caprun/Cargo.toml`'s existing `e2e`/`planner` targets) |
| Quick run command | `cargo test -p brokerd confirmation` (single-module, once the new module exists) |
| Full suite command | `cargo test --workspace --no-fail-fast` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|--------------------|--------------|
| CONFIRM-01 | `caprun confirm` displays verbatim literal + provenance | unit (string-format assertion) + integration (drives real binary) | `cargo test -p brokerd confirmation` / `cargo test -p caprun --test e2e` | ❌ Wave 0 — new `crates/brokerd/src/confirmation.rs` and a new `cli/caprun/tests/confirm.rs` |
| CONFIRM-02 | Confirm releases exactly one triple, no standing policy | unit — assert no allowlist/policy structure is consulted; assert a second Block on the same literal in a NEW session still Blocks | `cargo test -p brokerd confirmation` | ❌ Wave 0 |
| CONFIRM-03 | Deny durable; re-confirm on same `effect_id` refused | integration — drive real `caprun confirm` twice against the SAME persisted DB file, or `caprun deny` then `caprun confirm` | `cargo test -p caprun --test confirm` (new) | ❌ Wave 0 |
| CONFIRM-04 | Confirm/deny audited, anchored to `effect_id`, TCB-resident | unit — assert `confirm_granted`/`confirm_denied` events exist with `parent_id` == the `sink_blocked` event id, and `actor` contains `effect_id` | `cargo test -p brokerd confirmation` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p brokerd confirmation` (or the relevant crate's fast subset)
- **Per wave merge:** `cargo test --workspace --no-fail-fast`
- **Phase gate:** Full suite green before `/gsd-verify-work`; Linux-only enforcement tests (via
  Colima+Docker) deferred to Phase 11's live acceptance run per this project's existing convention
  (Phase 9's tests followed the same split)

### Wave 0 Gaps

- [ ] `crates/brokerd/src/confirmation.rs` — new module: `PendingConfirmation`, `ResolvedArg`,
  `PendingConfirmationState`, `insert_pending_confirmation`, `find_pending_confirmation`,
  `transition_state`, `confirm`, `deny` — covers CONFIRM-01..04
- [ ] `crates/brokerd/src/audit.rs` — `pending_confirmations` DDL addition (extend `SCHEMA_DDL`)
- [ ] `crates/brokerd/src/sinks/file_create.rs` — `invoke_file_create_from_resolved` — covers the
  frozen-snapshot re-invocation half of CONFIRM-02
- [ ] `cli/caprun/tests/confirm.rs` — new integration test target (mirrors `e2e.rs`'s pattern of
  driving `env!("CARGO_BIN_EXE_caprun")` as a real subprocess) — needed because CONFIRM-03's
  cross-process durability claim cannot be honestly tested within one process (Pitfall 5)
- [ ] `cli/caprun/Cargo.toml` — add the new `[[test]]` target entry for `confirm.rs`, mirroring the
  existing `e2e`/`planner`/`s9_live_block`/`origin_seed_provenance` entries

*(No framework install needed — `cargo test` is already fully configured for this workspace.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V1 Architecture, Design and Threat Modeling | yes | This phase implements an already-approved design doc (`DESIGN-confirmation-release.md`); the plan must preserve every MUST/MUST-NOT verbatim |
| V4 Access Control | yes | Confirm/deny decision logic and terminal-state enforcement live exclusively in `crates/brokerd` (TCB) — never a policy file, never re-derived from `submit_plan_node` (CONFIRM-04) |
| V5 Input Validation | yes | `caprun confirm <effect_id>`'s `effect_id` argument is untrusted CLI input — must fail closed (parse error or "unknown effect_id") on malformed UUIDs or unrecognized ids, never silently proceed |
| V6 Cryptography | yes (reuse only) | `literal_sha256` digesting already exists via `sha2::Sha256` — this phase must not introduce a second/alternate hashing scheme for the same purpose |
| V8 Data Protection / Audit | yes | `confirm_granted`/`confirm_denied` events must carry the exact `parent_id` causal edge to `sink_blocked`; the `pending_confirmations`/`blocked_literals` redaction interplay (Pitfall 4) is a data-protection requirement, not merely a nice-to-have |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Human confirms the wrong literal due to a truncated/summarized display | Repudiation (human didn't see what they authorized) | Display the byte-exact literal verbatim (no truncation/elision), per the exact CONFIRM-01 output format — already an Accepted Residual Risk in the design doc for path-specific confusables |
| Replay: an attacker re-invokes `caprun confirm` on an already-`Confirmed`/`Denied` `effect_id` hoping for a second release | Tampering / Elevation of Privilege | Terminal-state check MUST re-read the persisted DB row on every invocation (never in-memory); a second confirm attempt on a terminal state is refused (CONFIRM-03) |
| Confused-deputy: a worker or planner supplies its own `effect_id` to `caprun confirm`, forging a pending confirmation that was never actually Blocked | Spoofing | `effect_id` is broker-minted at Block time (`Uuid::new_v4()` inside `server.rs`, never client-supplied) — `find_pending_confirmation` returning `None` for any id not actually persisted at Block time is the fail-closed backstop |
| Redaction bypass via the `PendingConfirmation.resolved_args` copy surviving a `blocked_literals` redaction | Tampering / Information Disclosure | Pitfall 4's fix: redact both side tables together, or refuse to release if `blocked_literals` was redacted |
| TOCTOU: workspace state changes between Block and Confirm (e.g. target path created by another process in the interim) | Tampering | Accepted Residual Risk 2 in the design doc — `O_EXCL` + `RESOLVE_BENEATH` at confirm-invocation time still fails closed on conflict; not a new risk introduced by this phase, already covered by existing sink-adapter invariants |

## Sources

### Primary (HIGH confidence — all read directly this session)
- `planning-docs/DESIGN-confirmation-release.md` — the authoritative, APPROVED spec for this
  phase's entire mechanism (`PendingConfirmation` schema, Steps 1-4 decision logic, CLI contract,
  exit-code table, single-shot/durable-deny/TCB-residency semantics, accepted residual risks)
- `planning-docs/DESIGN-GATE-RECORD-v1.2.md` (referenced via `.planning/STATE.md`'s Blockers/Concerns
  entry — gate UNBLOCKED, same `DEC-ai-review-satisfies-human-gate` provenance noted for Phase 9)
- `crates/runtime-core/src/{executor_decision,event,plan_node,session,value_record,lib}.rs` —
  current `SinkBlockedAnchor`/`ExecutorDecision::BlockedPendingConfirmation` shape, `Event`
  constructors (no `effect_id` column, golden byte-fixture test), `PlanNode`/`SinkId`/`ValueId`,
  `Session`/`SessionStatus`, `ValueRecord`
- `crates/executor/src/{lib,value_store}.rs` — `submit_plan_node`'s decision function (confirming it
  must never be re-invoked for a confirm), `ValueStore::resolve`/`mint` (confirming the in-memory
  store is per-connection and does not survive process exit)
- `crates/brokerd/src/{audit,server,session}.rs` — current `SCHEMA_DDL` (`sessions`, `events`,
  `blocked_literals`), `append_event`'s `sink_blocked`-requires-anchor guard, `insert_blocked_literal`/
  `get_blocked_literal`/`redact_blocked_literal`, `find_event_by_type`/`query_events_by_session`
  (confirming no `effect_id`-indexed lookup exists today), `SubmitPlanNode` IPC arm (confirming
  broker-side `effect_id` minting, current sink-dispatch-by-name pattern), `run_broker_server`/
  `dispatch_request`'s `workspace_root: Arc<WorkspaceRoot>` threading
- `crates/brokerd/src/sinks/{file_create,email_send}.rs` — current `invoke_file_create` (confirming
  its `&ValueStore`-dependent signature and the `sink:file.create:{effect_id}` actor convention to
  mirror), `invoke_email_send_stub` (confirming email.send is a no-op stub, not a live effect)
- `crates/adapter-fs/src/workspace.rs` — `WorkspaceRoot::open`/`create_exclusive_within`/
  `read_within` (confirming the Linux variant retains no path field, only a `dirfd`; the non-Linux
  stub retains `root_path: PathBuf`)
- `cli/caprun/src/main.rs` — current positional-only arg parsing (`--seed-from-file` precedent,
  intent-kind match, `workspace_root_dir` derivation before dirfd-open — confirming the path string
  IS available pre-dirfd, supporting Open Question 1's recommendation)
- `Cargo.toml` (workspace root) + `cli/caprun/Cargo.toml` + `crates/brokerd/Cargo.toml` — confirming
  no `clap` (or any CLI-parsing crate) exists in the dependency graph; confirming exact pinned
  versions of `rusqlite`/`uuid`/`serde`/`chrono`/`anyhow`/`sha2`
- `cli/caprun/tests/e2e.rs` — existing pattern for driving the real compiled binary via
  `env!("CARGO_BIN_EXE_caprun")`, the template for the new cross-process CONFIRM-03 test
- `.planning/{REQUIREMENTS,STATE}.md` — locked v1.2 scope, requirement IDs CONFIRM-01..04, prior
  decisions (confirm-UX is a second command not an interactive prompt; Phase 8/9/10 dependency
  structure)
- `.planning/phases/09-session-trust-state-i1-i0/09-RESEARCH.md` — precedent for this project's
  research depth/format and its explicit "no external tool_strategy invocation needed" scoping
  rationale, reused here for the same reason (zero new external dependencies)

### Secondary (MEDIUM confidence)
- None — no external documentation or library research was needed this phase.

### Tertiary (LOW confidence)
- None used as load-bearing claims. All `[ASSUMED]`-tier claims are isolated to the Assumptions Log
  above and are explicitly non-security-load-bearing plumbing/structure choices, not reinterpretations
  of the approved design doc's substance.

## Metadata

**Confidence breakdown:**
- Standard stack: N/A — no new packages this phase, nothing to assess
- Architecture: HIGH — every gap identified traces to a specific, currently-read function/struct
  whose shape does not yet support the new requirement; the confirmation mechanism itself is already
  adversarially-reviewed and approved (not this research's own design)
- Pitfalls: HIGH — each pitfall is grounded in a concrete, verified current-code fact (exact function
  signature, exact missing DB column, exact missing path field), not speculation

**Deliberate scoping note:** This research did not invoke the `tool_strategy` external-research seam
(no `research-plan`/WebSearch/Context7 calls) because Phase 10 introduces zero new external
dependencies and the entire security-relevant design space is already resolved by the approved
`DESIGN-confirmation-release.md`. All effort went into verifying that document's claims against the
live codebase — including two gaps (workspace-root-path persistence, `ValueStore`-dependent sink
re-invocation) the design doc itself does not close — rather than researching external libraries.

**Research date:** 2026-07-07
**Valid until:** Until `crates/brokerd`, `crates/executor`, `crates/runtime-core`, `crates/adapter-fs`,
or `cli/caprun` change meaningfully from the state read this session, or until the design doc is
amended — recommend re-verifying the workspace-root-path gap (Open Question 1) and the exact
`invoke_file_create` signature (Pitfall 1) immediately before planning if any other phase work has
landed in the interim.
