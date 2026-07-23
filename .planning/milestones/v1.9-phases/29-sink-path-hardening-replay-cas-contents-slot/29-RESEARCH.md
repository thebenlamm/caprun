# Phase 29: Sink-Path Hardening — Replay CAS & contents Slot - Research

**Researched:** 2026-07-17
**Domain:** TCB security hardening — SQLite CAS/transaction idioms (HARDEN-03), sink-arg role-constraint tables (HARDEN-05), both inside `caprun`'s Rust reference monitor (`crates/brokerd`, `crates/executor`)
**Confidence:** HIGH — every claim below is re-verified against live code at authoring time (`git grep`/`Read`), not inherited from the DESIGN doc's authoring-time line numbers, which have shifted since Phase 27/28 landed.

## Summary

This phase is a mechanical realization of `planning-docs/DESIGN-security-hardening.md` §c (HARDEN-03) and §e (HARDEN-05) — there is no CONTEXT.md; the DESIGN doc is the locked spec, mirroring Phases 27/28. Both residuals are sink-dispatch-level: HARDEN-03 makes the trusted (never-blocked) `email.send` Allowed path replay-safe via a content-derived idempotency key + PK-violation-as-CAS in a real SQL transaction, mirroring the already-shipped `confirm()` SEND-01 pattern almost verbatim but with a *different* key derivation (content-derived, not `effect_id`-keyed, because `effect_id` is freshly minted every call and provides zero replay protection). HARDEN-05 adds two small, precisely-scoped edits to `sink_sensitivity.rs` so `file.create`'s `contents` arg becomes role-checked (`Some(&["path"])`, not `None`) and content-sensitive — without breaking the one live `file.create` flow, which reuses the trusted `"path"`-role literal in the `contents` slot too.

**Primary recommendation:** For HARDEN-03, wrap the `conn.lock()` MutexGuard's connection in a real `rusqlite::Transaction` (mirroring `confirmation.rs`'s `let tx = conn.transaction()?`) inside `server.rs`'s Allowed `email.send` block, compute the idempotency key as `SHA256(sink.0 || sorted (arg_name, value_id.0-as-string) pairs)`, `INSERT` into a new `sent_plan_nodes` table before appending `email_send_attempted`, and commit before opening SMTP. For HARDEN-05, add `FILE_CREATE_CONTENT_SENSITIVE = &["contents"]` to `is_content_sensitive` and flip `expected_role`'s `"contents" => None` to `"contents" => Some(&["path"])` — nothing else.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Replay-safe Allowed `email.send` dispatch | Broker / Reference Monitor (`crates/brokerd::server`) | Audit store (`crates/brokerd::audit`, new table) | The broker is the only component that dispatches to sinks; the CAS row lives in the same SQLite audit DB it already owns. |
| Idempotency-key derivation | Broker / Reference Monitor | — | Must be computed from broker-resolved `plan_node`/`value_id` state, never from planner/worker input — mirrors `combined_digest`'s placement in `confirmation.rs`. |
| `contents` slot role constraint | Executor (I2 TCB, `crates/executor::sink_sensitivity`) | — | Slot-type/role checking is hardcoded executor logic (`CON-i2-non-bypassable`), never broker or CLI logic. |
| `contents` value production (`"path"`-role reuse) | CLI / Planner (`cli/caprun::planner`) | — | Unaffected by this phase — the planner already places `intent_value_id` (role `"path"`) into both `path` and `contents` slots; HARDEN-05 only changes what the executor accepts there. |

## User Constraints

No CONTEXT.md exists for this phase. The canonical, locked spec is `planning-docs/DESIGN-security-hardening.md` §c/§e (mirroring Phases 27 and 28, which also planned directly from this DESIGN doc). Treat every "pinned" statement in §c/§e as a locked decision, not a discretionary option. `.planning/REQUIREMENTS.md`'s Out-of-Scope section (v1.7 breadth: Git/GitHub/test/patch/snapshot adapters; per-session effects-budget/rate-limit; full output-file provenance labeling) remains out of scope for this phase.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HARDEN-03 | A replayed `SubmitPlanNode` on the trusted (Allowed) `email.send` path sends at most once via an idempotency key / CAS, matching the confirm path's at-most-once transaction discipline. | See "Code Examples" #1 (current Allowed-dispatch block + exact edit points), "Don't Hand-Roll" (reuse `mac_frame`/migration idioms, don't invent new ones), "Common Pitfalls" #1-#3 (mutable-lock, key-derivation, transaction-vs-lock confusion). |
| HARDEN-05 | The `file.create` `contents` arg carries an expected-role/sensitivity treatment under the same I2/slot-type discipline as other sensitive args, closing the currently-unconstrained-slot gap, without regressing the only live `file.create` flow. | See "Code Examples" #2 (exact two-line diff + inverted test), "Common Pitfalls" #4-#5 (wrong role list, over-widening content-sensitivity to `path`). |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rusqlite` (bundled feature) | already a workspace dep — `crates/brokerd/Cargo.toml` | SQLite access, `Connection::transaction()` | Already used for the confirm-path CAS (`confirmation.rs`); no new dependency needed. [VERIFIED: crates/brokerd/Cargo.toml + live code at `crates/brokerd/src/confirmation.rs:875`] |
| `sha2` | already a workspace dep | SHA-256 for the idempotency-key hash | Already used by `compute_event_hash`/`combined_digest`. [VERIFIED: crates/brokerd/src/audit.rs, confirmation.rs] |
| `hex` | already a workspace dep | Hex-encoding digests | Already used by `combined_digest`. [VERIFIED: confirmation.rs:98] |

No new crates are required for this phase — every primitive HARDEN-03/HARDEN-05 needs already exists in the workspace.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| PK-violation-as-CAS (`INSERT` into `sent_plan_nodes`, catch/ignore constraint error) | `INSERT OR IGNORE` + check `rows_affected() == 0` | Both are valid per the DESIGN doc ("pick one explicitly in Phase 29"); `INSERT OR IGNORE` is simpler to reason about in Rust (no error-matching on `rusqlite::Error::SqliteFailure`) — **recommended**. |
| Content-derived idempotency key | `effect_id`-keyed CAS | **Rejected by DESIGN as unsound** — `effect_id` is minted fresh every call (`server.rs:664`, `Uuid::new_v4()` at the top of `evaluate_plan_node_and_record`), so it can never detect a replay. Do not implement this. |

## Package Legitimacy Audit

Not applicable — this phase adds no new external package dependencies (§ Standard Stack above: everything needed is already a workspace dependency).

## Architecture Patterns

### System Architecture Diagram

```
SubmitPlanNode (IPC, Worker or Planner connection)
        │
        ▼
dispatch_request (server.rs:1146, pub)
        │
        ▼
evaluate_plan_node_and_record (server.rs:651, private)
        │  1. mint effect_id (Uuid::new_v4()) — NOT usable as replay key
        │  2. executor::submit_plan_node → ExecutorDecision
        │
        ├─ Decision == BlockedPendingConfirmation ──▶ existing Block/PendingConfirmation path (unchanged this phase)
        │
        └─ Decision == Allowed && sink == "email.send"   ◄── HARDEN-03 touches HERE (~server.rs:901-977)
                │
                ▼
           [NEW] compute idempotency_key = SHA256(sink.0 || sorted (arg_name, value_id) pairs)
                │
                ▼
           conn.lock() → mut MutexGuard  →  tx = locked.transaction()?
                │
                ├─ INSERT INTO sent_plan_nodes (idempotency_key PK, ...)   ── PK violation ⇒ replay ⇒ suppress send
                ├─ append_event(&tx, key, email_send_attempted_event, ...)
                └─ tx.commit()
                │
                ▼   (only on fresh insert)
           invoke_email_smtp_from_resolved  →  real SMTP send (Mailpit in test)


submit_plan_node (executor::lib, sync, pure)     ◄── HARDEN-05 touches HERE
        │
        ▼
sink_schema::validate_schema (Step 0 — arg presence/shape)
        │
        ▼
per-arg loop: is_routing_sensitive / is_content_sensitive / expected_role   (crates/executor/src/sink_sensitivity.rs)
        │                                                    │
        │  "file.create","path"     → routing-sensitive, Some(["path","relative_path"])   (unchanged)
        │  "file.create","contents" → [NEW] content-sensitive, Some(["path"])              (was: not content-sensitive, None)
        ▼
tainted + sensitive → collect-then-Block   |   role mismatch → SlotTypeMismatch Deny   |   else → Allowed
```

### Recommended Project Structure

No new files or directories — all edits land in existing files:

```
crates/brokerd/src/
├── server.rs        # HARDEN-03: Allowed email.send block gets key-derivation + CAS transaction
├── audit.rs         # HARDEN-03: new `sent_plan_nodes` table DDL + migration fn (mirror migrate_chain_anchor_schema)
└── confirmation.rs  # unchanged — cited as the template only

crates/executor/src/
└── sink_sensitivity.rs   # HARDEN-05: two edits + one inverted test

crates/brokerd/tests/
└── (new or extended file, e.g. replay_cas.rs)  # Phase 30 negative test lives here or in email_smtp_acceptance.rs style
```

### Pattern 1: SEND-01 atomic CAS-then-append-then-commit-then-effect (the mirror target)

**What:** Confirm path's `email.send` arm already does exactly the ordering HARDEN-03 needs: open a real `rusqlite::Transaction`, do the CAS write, append the durable attempt event inside the SAME transaction, commit, and only THEN open the SMTP socket.
**When to use:** Copy this ordering discipline for the Allowed-path CAS; do not invent a new ordering.
**Example (current code, `crates/brokerd/src/confirmation.rs:868-896`):**
```rust
"email.send" => {
    // SEND-01: the CAS (`pending -> confirmed`) and the durable
    // `email_send_attempted` append MUST commit in ONE atomic SQLite
    // transaction, BEFORE any SMTP connection is opened.
    let tx = conn.transaction()?;
    let affected = transition_state(&tx, key, &pc, PendingConfirmationState::Confirmed)?;
    if affected == 0 {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }
    let attempted_event = runtime_core::Event::new(/* ... */);
    let attempted_event_id = attempted_event.id;
    let attempted_hash =
        crate::audit::append_event(&tx, key, &attempted_event, Some(&granted_hash))?;
    tx.commit()?;
    // AFTER commit — the CAS + attempt are now durable together, or
    // neither is; only now does an SMTP connection ever open.
    match crate::sinks::email_smtp::invoke_email_smtp_from_resolved(/* conn, not tx — post-commit */) { ... }
}
```
Note `conn` here is `&mut rusqlite::Connection` (confirm/deny run in a fresh, single-threaded OS process — no `Arc<Mutex<>>` needed). The Allowed path's `conn` parameter is `&Arc<Mutex<rusqlite::Connection>>` — see Pitfall 1 below for the resulting mutability wrinkle.

### Pattern 2: `migrate_chain_anchor_schema` — the mirror target for the new `sent_plan_nodes` migration

**What:** A brand-new table (not a column-widening of an existing one) gets a `CREATE TABLE IF NOT EXISTS` in `SCHEMA_DDL` plus a defensive presence-check function called from `open_audit_db`, exactly like `chain_anchor` (Phase 28).
**Example (current code, `crates/brokerd/src/audit.rs:263-304`):**
```rust
fn migrate_chain_anchor_schema(conn: &rusqlite::Connection) -> Result<()> {
    let table_exists: bool = match conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chain_anchor'",
        [], |_row| Ok(()),
    ) {
        Ok(()) => true,
        Err(rusqlite::Error::QueryReturnedNoRows) => false,
        Err(e) => return Err(anyhow::Error::from(e)),
    };
    if !table_exists {
        return Err(anyhow::anyhow!(
            "chain_anchor table missing after SCHEMA_DDL ran — open_audit_db invariant violated"
        ));
    }
    Ok(())
}
```
`open_audit_db` (`audit.rs:315-322`) calls `migrate_pending_confirmations_schema` then `migrate_chain_anchor_schema` — a new `migrate_sent_plan_nodes_schema` call slots in right after these, following the identical shape. **A pre-existing `audit.db` with no `sent_plan_nodes` table is exactly analogous to a legacy pre-Phase-28 DB with no `chain_anchor` row** — no backfill is needed (a legacy replayed plan node was never protected anyway; this is forward-looking, not retroactive).

### Pattern 3: `combined_digest`'s sort-then-hash shape — the mirror target for the idempotency key (NOT a direct reuse)

**What:** `combined_digest` (`confirmation.rs:79-109`) sorts args by name, then for each pair computes a fixed-width SHA-256 of the name and of the value, feeding both into one outer hasher. The idempotency key needs the SAME sort-then-fixed-width-hash discipline, but over `(arg_name, value_id)` pairs (the resolved `ValueId` UUID, not the dereferenced literal) — a genuinely new function, since `combined_digest`'s signature is `&[(&str, &str)]` (literal strings) and the DESIGN pin explicitly requires `value_id`-scope, not literal-scope (D-08).
**Example (new code to write, following the existing shape):**
```rust
/// Domain: sorted (arg_name, value_id) pairs — NOT literal-scoped (D-08:
/// this deliberately does not catch a worker that mints a NEW value_id
/// resolving to the identical literal; that is out of v1.6 scope).
pub(crate) fn plan_node_idempotency_key(sink: &SinkId, args: &[PlanArg]) -> String {
    let mut sorted: Vec<&PlanArg> = args.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut hasher = Sha256::new();
    hasher.update(sink.0.as_bytes());
    for arg in sorted {
        hasher.update(arg.name.as_bytes());
        hasher.update(arg.value_id.0.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}
```
(Whether to adopt `combined_digest`'s fixed-width-inner-hash-per-field discipline verbatim, or the simpler direct-concatenation shown above, is a planning-level implementation choice — `PlanArg.name`/`ValueId` values do not have the same "partition-blindness" collision risk `combined_digest` was hardened against, since `ValueId` is a fixed-width UUID string already. Either is acceptable; state the choice explicitly in the plan.)

### Anti-Patterns to Avoid
- **Keying the CAS on `effect_id`:** proven unsound — `effect_id` is minted fresh (`Uuid::new_v4()`) at the top of `evaluate_plan_node_and_record` (`server.rs:664`) on every call, including every replay. A CAS keyed on it never fires.
- **Using `conn.execute()` calls without a transaction wrapper for the CAS + append:** two separate autocommit statements is NOT one atomic unit — a crash between them reopens exactly the gap the CAS exists to close (this is explicitly called out in `confirmation.rs:840-842`'s comment about why the generic Confirmed-transition is skipped for `email.send`).
- **`.unwrap_or(&[])` when adding the `contents` role list:** `sink_sensitivity.rs`'s own doc comment (`:122-124`) forbids ever constructing `Some(&[])` — always a design bug, never a runtime state.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Atomic CAS + audit append | A custom locking scheme, a new mutex, a hand-rolled "check-then-insert" pattern | `rusqlite::Connection::transaction()` (already used by SEND-01) | SQLite's own transaction semantics already give the exact atomicity guarantee needed; a hand-rolled check-then-insert has a TOCTOU race even under the broker's own `Arc<Mutex<>>` if a panic occurs mid-sequence without a rollback boundary. |
| Idempotency-key hashing | A custom fixed-width string concatenation scheme from scratch | Mirror `combined_digest`'s / `mac_frame`'s sort-then-hash shape | Two independently-invented ad hoc hash schemes in the same codebase increase audit burden with zero benefit; reusing the established shape is both faster to review and consistent with the DESIGN doc's own instruction ("mirroring confirmation.rs's combined_digest shape"). |
| Schema migration presence-check | A blind `CREATE TABLE` with no idempotency check, or a manual `try/catch "table exists"` pattern | Mirror `migrate_chain_anchor_schema`'s `sqlite_master` presence-check idiom | Already-proven-idempotent, already-tested pattern (`open_audit_db` re-runs on every process start); reinventing it risks a subtly different failure mode on a pre-existing legacy DB. |

**Key insight:** Every mechanism HARDEN-03 needs (transactional CAS, content-derived digest, idempotent schema migration) already has a proven, shipped precedent in this exact codebase from Phase 28's HARDEN-02 work and the pre-existing SEND-01/CONFIRM-03 machinery. This phase is deliberately "no new architecture" — a planning task that introduces a genuinely new pattern here should be treated as a red flag.

## Common Pitfalls

### Pitfall 1: The `conn.lock()` binding in the Allowed-path block is currently immutable — `.transaction()` needs `&mut`
**What goes wrong:** `server.rs`'s current Allowed `email.send` block (line 926-928) binds `let locked = conn.lock().map_err(...)?;` — an immutable binding. `rusqlite::Connection::transaction(&mut self)` requires a mutable reference. A naive port of the SEND-01 pattern into this block will fail to compile until the binding is changed to `let mut locked = ...`.
**Why it happens:** The existing code only ever calls `append_event(&locked, ...)` (a `&Connection` call), which doesn't need mutability — no one has needed `.transaction()` on this MutexGuard before.
**How to avoid:** Change the binding to `let mut locked = conn.lock()...?;` before calling `locked.transaction()?`. `MutexGuard<T>` implements `DerefMut`, so this compiles once the binding is mutable.
**Warning signs:** A compile error like "cannot borrow `*locked` as mutable, as it is not declared as mutable" at the `.transaction()` call site.

### Pitfall 2: Confusing `Arc<Mutex<Connection>>` serialization with real SQL transaction atomicity
**What goes wrong:** It's tempting to reason "we already hold the Mutex lock for the whole block, so two separate `conn.execute()` calls are already atomic w.r.t. other broker tasks." This is true for *concurrent-task* ordering but NOT for *crash/panic* atomicity — two separate autocommit `execute()` calls can leave the DB with the CAS row but not the attempted-event row (or vice versa) if the process dies between them.
**Why it happens:** The Mutex and the SQL transaction solve different problems (in-process serialization vs. durable atomicity) and their guarantees are easy to conflate.
**How to avoid:** Always wrap the CAS-insert + `email_send_attempted` append in a real `rusqlite::Transaction` (per Pattern 1), even though the Mutex is also held — they compose (transaction-under-mutex), they don't substitute for each other.
**Warning signs:** A CAS implementation with two bare `locked.execute(...)` calls and no `.transaction()`/`.commit()` anywhere.

### Pitfall 3: Picking an `effect_id`-scoped or session-scoped key instead of the pinned content-derived, per-plan-node key
**What goes wrong:** A key derived from `effect_id` (freshly minted every call) or session-scoped (one CAS row per session) either does nothing (former) or over-suppresses legitimate distinct sends within one session (latter — a session that legitimately sends two DIFFERENT emails would collide if the key doesn't include the resolved args).
**Why it happens:** `effect_id` "looks like" the natural row identity because it's already threaded through the Allowed-dispatch block for the audit-event `actor` field (`format!("sink:email.send:{effect_id}")`).
**How to avoid:** Key strictly on `SHA256(sink.0 || sorted (arg_name, value_id) pairs)` as pinned — `value_id`-scope, not resolved-literal-scope, not `effect_id`-scope, not session-scope.
**Warning signs:** A `sent_plan_nodes` schema whose PK is `effect_id` or `session_id` instead of a content-derived `idempotency_key`.

### Pitfall 4: Choosing a `contents` role list that omits `"path"`
**What goes wrong:** Any role list for `file.create`'s `contents` slot that does NOT include `"path"` will hard-`Deny` (`SlotTypeMismatch`) the only currently-passing `file.create` clean-allow flow (`s9_live_file_create_clean_allow`, `cli/caprun/tests/s9_live_block.rs:556`), because the planner (`cli/caprun/src/planner.rs:208`) reuses the SAME trusted `"path"`-role `intent_value_id` for both the `path` and `contents` `PlanArg`s — there is no separate `"contents"`/`"file_body"`-role mint site in the codebase today.
**Why it happens:** A reasonable-looking analogy to `email.send`'s `body` role list (`Some(&["body", "doc_fragment"])`) tempts an implementer to invent parallel role names like `"contents"` or `"file_body"` for `file.create` — but those roles don't exist anywhere in the current mint vocabulary.
**How to avoid:** The role list MUST be exactly `Some(&["path"])`. This is locked, non-negotiable, traced end-to-end in the DESIGN doc §e — verify by re-tracing `planner.rs:208` → `server.rs`'s `CaprunIntent::CreateFileFromReport` arm before implementing.
**Warning signs:** `s9_live_file_create_clean_allow` starts failing with a `SlotTypeMismatch`/Deny after the change.

### Pitfall 5: Over-widening `is_content_sensitive`'s new `file.create` arm to match unconditionally
**What goes wrong:** Copying the email pattern carelessly (`"file.create" => true` instead of `"file.create" => FILE_CREATE_CONTENT_SENSITIVE.contains(&arg_name)`) would make `path` ALSO content-sensitive — a real regression, since `path` is currently (correctly) routing-sensitive only.
**Why it happens:** `is_content_sensitive`'s `email.send` arm is a `.contains()` check but it's easy to miswrite the new arm as an unconditional `true` if not careful about matching the existing pattern shape.
**How to avoid:** `FILE_CREATE_CONTENT_SENSITIVE` must be `&["contents"]` only; the arm must be `.contains(&arg_name)`, matching the `EMAIL_SEND_CONTENT_SENSITIVE` shape exactly.
**Warning signs:** The existing tests `file_create_path_is_routing_sensitive` / `file_create_contents_not_routing_sensitive` (both currently green, `sink_sensitivity.rs:182-195`) still pass, but a NEW test asserting `path` is NOT content-sensitive would catch this if added — the plan should add one.

### Pitfall 6: Forgetting the D-08 scope caveat when writing HARDEN-03's success criteria
**What goes wrong:** Describing HARDEN-03 as "prevents replay" without the per-plan-node qualifier overclaims — a statically-compromised worker that mints a FRESH, DISTINCT `value_id` for the "same" recipient (e.g. re-resolving `mint_from_intent`/`mint_from_derivation`) gets a DIFFERENT idempotency key and sends N times. This is explicitly out of scope (D-08, filed as future effects-budget work).
**How to avoid:** State the guarantee honestly in the plan's success criteria: "at-most-once PER PLAN NODE (identical `value_id`s), not bounded sends per session."

## Code Examples

### 1. Current Allowed `email.send` dispatch block (re-verified location: `crates/brokerd/src/server.rs:901-977`)
```rust
if matches!(decision, runtime_core::ExecutorDecision::Allowed)
    && plan_node.sink.0 == "email.send"
{
    // ... resolve args into Vec<ResolvedArg> (unchanged) ...
    let locked = conn.lock().map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
    // MAJOR-4: append a durable, OPAQUE email_send_attempted event
    // BEFORE the SMTP socket ever opens ...
    let attempted_event = Event::new(/* ... */);
    let attempted_hash = append_event(&locked, key, &attempted_event, Some(last_event_hash))?;
    // ... AFTER the durable attempt append succeeded — only now does
    // an SMTP connection ever open. REPLAY RESIDUAL RISK (named, not
    // silent): this Allowed path has no CAS/PendingConfirmation —
    // a replayed SubmitPlanNode mints a fresh effect_id and would
    // send again (N submissions => N emails). [THIS COMMENT IS THE
    // RESIDUAL HARDEN-03 CLOSES — remove/update it once the CAS lands.]
    let (sink_event_id, sink_hash) = crate::sinks::email_smtp::invoke_email_smtp_from_resolved(/* ... */)?;
    *last_event_id = sink_event_id;
    *last_event_hash = sink_hash;
}
```
The edit: insert the idempotency-key computation + `let mut locked = ...` + `let tx = locked.transaction()?;` + CAS `INSERT` + `append_event(&tx, ...)` + `tx.commit()` BEFORE the `invoke_email_smtp_from_resolved` call, with an early-return/suppression path when the CAS insert signals a PK violation (replay).

### 2. Current `sink_sensitivity.rs` state (re-verified: exact current lines, matching DESIGN doc's anchors almost exactly)
```rust
// is_content_sensitive, line 102-107 (current):
pub fn is_content_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_CONTENT_SENSITIVE.contains(&arg_name),
        _ => false,   // <- file.create arm to be ADDED here
    }
}

// expected_role, line 147-162 (current):
pub fn expected_role(sink: &SinkId, arg_name: &str) -> Option<&'static [&'static str]> {
    match sink.0.as_str() {
        "email.send" => match arg_name { /* unchanged */ },
        "file.create" => match arg_name {
            "path" => Some(&["path", "relative_path"]),
            "contents" => None, // <- FLIPS to Some(&["path"])
            _ => None,
        },
        _ => None,
    }
}

// The test that INVERTS, line 312-319 (current):
#[test]
fn file_create_contents_is_unconstrained() {
    assert_eq!(
        expected_role(&file_create(), "contents"),
        None,
        "file.create `contents` stays unconstrained for v1.5 (Assumption A2)"
    );
}
// becomes (rename recommended, e.g. file_create_contents_expects_path):
#[test]
fn file_create_contents_expects_path() {
    assert_eq!(
        expected_role(&file_create(), "contents"),
        Some(&["path"][..]),
        "file.create `contents` is role-checked as of v1.6 HARDEN-05 — the \
         only live production value is the reused trusted `path`-role literal"
    );
}
```

### 3. `sink_schema.rs` file.create required args (unaffected — re-verified: `crates/executor/src/sink_schema.rs:56`)
```rust
SinkSchema {
    sink: "file.create",
    required: &["path", "contents"],
    // ...
}
```
No change needed here — HARDEN-05 does not touch the schema, only the sensitivity/role tables.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| Allowed `email.send` path: attempt-ledger only, no CAS (accepted residual since Phase 16) | Content-derived idempotency-key CAS in the same atomic transaction as the attempt append | This phase (v1.6, Phase 29) | Closes the "N submissions ⇒ N emails" replay gap named in `server.rs`'s own code comment since Phase 16. |
| `file.create` `contents`: unconstrained slot (`None`, since v1.5 T2/slot-type-binding) | Role-checked (`Some(&["path"])`) | This phase | Wires a FUTURE tainted `contents` value into the I2 collect-then-Block path; currently a no-op on the live path since `contents` only ever carries a trusted `"path"`-role value (honest, documented residual). |

**Deprecated/outdated:** The `server.rs` comment block at the Allowed `email.send` site naming the replay risk as "Accepted for v1.3... v2 obligation tracked in .planning/todos/pending" is now STALE once this phase lands — update/remove it in the same PR (mirrors the discipline Phase 27 applied to `mint_from_read`'s doc comment).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `INSERT OR IGNORE` + `rows_affected()==0` is the recommended CAS idiom over catching a `rusqlite::Error::SqliteFailure` constraint violation | Standard Stack / Alternatives Considered | Low — DESIGN doc explicitly leaves this as a Phase-29 implementation choice; either idiom satisfies the pinned mechanism. Flagged `[ASSUMED]` only insofar as it's a stylistic recommendation, not a locked decision. |
| A2 | The idempotency-key hash function does not need `combined_digest`'s fixed-width-inner-hash-per-field discipline (since `ValueId` values are already fixed-width UUID strings with no partition-blindness collision risk) | Architecture Patterns / Pattern 3 | Low — if wrong, a planner could instead mirror `combined_digest` verbatim (also acceptable); worst case is defense-in-depth left on the table, not a security hole, since arg names are also fixed by the schema (`sink_schema.rs`'s `required`/`allowed` sets), not attacker-controlled. |

**If this table is empty:** N/A — two low-risk implementation-style assumptions logged above; no claim here needs user confirmation before planning proceeds, since both are DESIGN-doc-permitted discretion, not contested facts.

## Open Questions

1. **Should the `sent_plan_nodes` CAS row additionally record `sink` and the resolved arg names for audit legibility, beyond the DESIGN-pinned minimal shape?**
   - What we know: DESIGN pins `sent_plan_nodes(idempotency_key TEXT PRIMARY KEY, effect_id TEXT NOT NULL, session_id TEXT NOT NULL, sent_at TEXT NOT NULL)` — a minimal shape.
   - What's unclear: Whether the planner should add a `sink` column purely for human-readability during incident review (not load-bearing for the CAS itself).
   - Recommendation: Keep the DESIGN-pinned minimal shape; the `sink` is always `"email.send"` for now (the only Allowed-dispatch-CAS sink), so a column would be redundant. Revisit if a second sink ever gets this treatment.

2. **Does the Phase-30 negative test (submit the SAME `SubmitPlanNode` twice, assert exactly one Mailpit delivery) belong in `crates/brokerd/tests/email_smtp_acceptance.rs` (extending the existing Mailpit-backed suite) or a new file?**
   - What we know: `email_smtp_acceptance.rs` already drives `confirm()` directly (not via CLI/IPC) against a real Mailpit sidecar, and `dispatch_request` (`server.rs:1146`) is `pub async fn` — reachable from an integration test without going through a UDS socket.
   - What's unclear: Whether the Allowed-path CAS test needs the FULL `dispatch_request`/session-setup machinery (closer to `uds_ipc.rs`'s style) or can call the lower-level `evaluate_plan_node_and_record`-adjacent path directly (it's currently private, so a public-ish helper may be needed, or the test goes through `dispatch_request`).
   - Recommendation: This is a Phase-30 (not Phase-29) planning concern per the roadmap's phase split — flag it for the Phase 30 planner rather than resolving here, since Phase 29 lands the mechanism, not the negative test.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo | All phase code | ✓ | cargo 1.92.0 | — |
| Docker + Colima (for Linux-only tests) | HARDEN-03/05 negative tests are `#[cfg(target_os = "linux")]` | Not probed this session — CLAUDE.md confirms `scripts/mailpit-verify.sh` is the standing recipe | — | Use `scripts/mailpit-verify.sh` per CLAUDE.md; do not run bare `docker run rust:1` (Phase 16+ requirement — a benign `email.send` Allowed run now performs a LIVE SMTP send). |
| Mailpit sidecar | Any test exercising the Allowed `email.send` CAS (a real, un-blocked send) | Provisioned automatically by `scripts/mailpit-verify.sh` | `axllent/mailpit` | — |

**Missing dependencies with no fallback:** None — this is a pure Rust/SQLite code change; all tooling needed already exists in the project's standing verification recipe.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in test harness) + `scripts/mailpit-verify.sh` for Linux-gated security tests |
| Config file | none — plain `Cargo.toml` per-crate test targets; `crates/brokerd/tests/*.rs` integration tests |
| Quick run command | `cargo test -p executor sink_sensitivity` (HARDEN-05, runs on macOS — no `#[cfg(linux)]` gate on unit tests in this file) |
| Full suite command | `bash scripts/mailpit-verify.sh` (Linux-only, required for HARDEN-03's SMTP-touching negative test and any `#[cfg(target_os = "linux")]` test) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HARDEN-03 | Replayed `SubmitPlanNode` on Allowed `email.send` path sends at most once | integration, Linux-only (real SMTP via Mailpit) | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test <new_or_extended_test_file>' bash scripts/mailpit-verify.sh` | ❌ Wave 0 — new test needed (see Open Question 2) |
| HARDEN-03 | Unit-level: idempotency-key derivation is order-invariant / sink+arg-scoped (mirrors `combined_digest`'s own unit tests) | unit, cross-platform | `cargo test -p brokerd plan_node_idempotency_key` (or wherever the fn lands) | ❌ Wave 0 — new unit tests needed alongside the new fn |
| HARDEN-05 | `file.create` `contents` is content-sensitive + role-checked to `Some(["path"])` | unit, cross-platform | `cargo test -p executor sink_sensitivity` | ✅ existing test file — Phase 29 inverts/adds specific assertions (see Code Examples #2) |
| HARDEN-05 | Regression: `s9_live_file_create_clean_allow` still Allows after the role-list change | integration, Linux-only | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_live_file_create_clean_allow' bash scripts/mailpit-verify.sh` | ✅ existing test — regression canary, must stay green |

### Sampling Rate
- **Per task commit:** `cargo test -p executor sink_sensitivity` (fast, macOS-runnable) for HARDEN-05 edits; `cargo build --workspace` for HARDEN-03 edits (macOS cannot run the Linux-gated integration test but should still compile-check).
- **Per wave merge:** `bash scripts/mailpit-verify.sh` (full workspace, Linux, real SMTP) — this is the ONLY way to actually exercise the HARDEN-03 CAS and the `s9_live_file_create_clean_allow` regression canary.
- **Phase gate:** Full `bash scripts/mailpit-verify.sh` green before `/gsd-verify-work`. Remember the `cfg(linux)` test-blindness gotcha: a green macOS `cargo test` run proves NOTHING about either HARDEN-03's CAS test or the HARDEN-05 regression canary, both of which are `#[cfg(target_os = "linux")]`.

### Wave 0 Gaps
- [ ] New unit tests for the idempotency-key derivation function (order-invariance, sink-scoping, distinct-args-distinct-key) — mirrors `combined_digest`'s existing unit-test suite shape in `confirmation.rs`.
- [ ] New Linux-only integration test: submit the identical `SubmitPlanNode` twice on a trusted (never-blocked) `email.send` path, assert exactly one Mailpit delivery AND exactly one `sent_plan_nodes` row AND exactly one `email_send_attempted` event. Decide test-file placement per Open Question 2 above.
- [ ] New unit test asserting `file.create`'s `path` arg is NOT content-sensitive after the HARDEN-05 change (guards against Pitfall 5's over-widening failure mode) — not explicitly named in the DESIGN doc but recommended defense-in-depth given the existing `file_create_contents_not_routing_sensitive` test's precedent shape.
- [ ] `migrate_sent_plan_nodes_schema`-analog unit test mirroring `pending_confirmations_migration_is_idempotent` / the `chain_anchor` presence-check tests (idempotent re-run on an already-migrated DB).

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | no | N/A — no user-facing auth in this phase |
| V3 Session Management | no | N/A — session lifecycle unaffected; only sink-dispatch logic changes |
| V4 Access Control | no | N/A — no new authorization boundary; existing I2/Allowed-decision gate is unchanged, only made replay-safe |
| V5 Input Validation | yes | `sink_sensitivity.rs`'s hardcoded role/sensitivity tables ARE the input-validation control for sink args (`CON-i2-non-bypassable`) — never a swappable policy file. HARDEN-05 extends this existing control, does not introduce a new validation mechanism. |
| V6 Cryptography | yes (incidental) | SHA-256 (via `sha2` crate) for the idempotency-key digest — non-secret, content-derived, purely for CAS uniqueness (not a security-sensitive MAC/signature; contrast with HARDEN-02's HMAC-SHA256 keyed chain, which IS security-critical and unrelated to this phase). Never hand-roll a hash function. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Replay of an authorized (Allowed) IPC message causing duplicate irreversible external effects | Repudiation / Denial-of-Service (resource/side-effect exhaustion) | Content-derived idempotency key + PK-constraint-as-CAS inside an atomic transaction committed before the effect fires (this phase's HARDEN-03). |
| Tainted value smuggled into an unconstrained sink-argument slot, later exfiltrated or used to plant data via a legitimate-looking sink call | Tampering / Information Disclosure | Slot-type/role-binding — `expected_role` + `is_content_sensitive`/`is_routing_sensitive` hardcoded tables in the executor TCB, extended (never bypassed) by this phase's HARDEN-05. |
| Incomplete role list on a newly-role-checked slot silently breaking the one legitimate production flow (a false-positive availability regression, not a security hole, but breaks the acceptance test suite) | (not STRIDE — availability/correctness) | Exhaustive end-to-end trace of the slot's CURRENT production value(s) before picking the role list (this research's Pitfall 4) — a locked, non-negotiable table entry per the DESIGN doc, not left to planner discretion. |

## Sources

### Primary (HIGH confidence)
- `planning-docs/DESIGN-security-hardening.md` §c, §e, §f, §g, §i — the canonical, reviewed, adversarially-cleared locked design for this phase. [CITED: planning-docs/DESIGN-security-hardening.md]
- Live source re-verification via `git grep`/`Read` at research time (this session): `crates/brokerd/src/server.rs` (lines 651-680, 893-980), `crates/brokerd/src/audit.rs` (lines 1-330), `crates/brokerd/src/confirmation.rs` (lines 55-150, 433-445, 830-918), `crates/executor/src/sink_sensitivity.rs` (full file), `cli/caprun/src/planner.rs` (grep for `contents`/`intent_value_id`), `crates/executor/src/sink_schema.rs` (line 56), `crates/runtime-core/src/plan_node.rs` (PlanNode/PlanArg/ValueId/SinkId shapes), `cli/caprun/src/key.rs` (`load_or_create_key`), `cli/caprun/tests/s9_live_block.rs` (lines 556-600, 813-900), `crates/brokerd/tests/email_smtp_acceptance.rs` (lines 300-400). [VERIFIED: live codebase at commit `8c248c1` (HEAD at research time)]
- `.planning/REQUIREMENTS.md` — HARDEN-03/HARDEN-05 requirement text and phase-mapping table. [CITED: .planning/REQUIREMENTS.md]
- `.planning/STATE.md` — accumulated decisions confirming HARDEN-03/05 traceability back to Phase 16 (MAJOR-4) and Phase 25 (contents-slot gap). [CITED: .planning/STATE.md]

### Secondary (MEDIUM confidence)
None — this phase's research required no external/web sources; it is a pure code-tracing exercise against an already-locked internal design doc.

### Tertiary (LOW confidence)
None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; every primitive already lives in the workspace and is directly re-verified in live code.
- Architecture: HIGH — every file:line anchor in this document was re-verified against live source this session (not inherited unchecked from the DESIGN doc's authoring-time anchors, several of which have shifted by ~100-150 lines since Phase 28 landed HARDEN-02 code above the Allowed-dispatch block).
- Pitfalls: HIGH — each pitfall traces to a specific, re-verified code location or an explicit DESIGN-doc ruling (D-08, the SEND-01 skip-condition comment, the `expected_role` doc-comment's `Some(&[])` prohibition).

**Research date:** 2026-07-17
**Valid until:** ~7 days (fast-moving — this is an active TCB hardening milestone with phases landing every 1-2 days; re-verify file:line anchors before planning if Phase 30 or any hotfix has landed in the interim).
