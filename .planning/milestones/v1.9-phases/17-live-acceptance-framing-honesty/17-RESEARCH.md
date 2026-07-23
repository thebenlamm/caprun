# Phase 17: Live Acceptance & Framing Honesty - Research

**Researched:** 2026-07-09
**Domain:** Rust integration-test harness composition (multi-process/multi-session audit-DB
composition) + live SMTP-capture assertions (Mailpit HTTP API) + project-scope documentation honesty
**Confidence:** HIGH (all claims below are sourced by direct reading of the current repo state —
`cli/caprun/src/main.rs`, `cli/caprun/tests/*.rs`, `crates/brokerd/src/{audit,confirmation,server}.rs`,
`planning-docs/DESIGN-session-trust-state.md`, `.planning/PROJECT.md`, and Phase 16's SUMMARY/plan/
VERIFICATION artifacts). This phase has almost no external-library unknowns — it is a composition and
documentation phase, not a new-dependency phase.

## Summary

Phase 17 has two independent halves. **ACCEPT-01** is a test-harness composition problem, not a new
feature: every primitive it needs (live email block, live confirm-driven send, live deny, live clean
control, Mailpit HTTP assertions) already exists and already passes individually
(`cli/caprun/tests/s9_live_block.rs`, `cli/caprun/tests/live_acceptance_tainted_session.rs`). What does
**not** exist is (a) a single shared `audit.db` across all four legs of the scenario in one test, (b) a
live email-specific confirm/deny pair (today's live confirm/deny tests only exercise the `file.create`
sink; the email sink's live block is confirmed-or-denied only in the *non-live* `confirmation.rs` unit
tests), and (c) a numeric Mailpit message-count assertion proving "deny sends nothing" — today's closest
analog (`assert_recipient_not_captured`) is a presence/absence check, reused here as the count primitive.

**DOC-01** is a documentation-only change: all 8 required framing points already exist, verbatim or
near-verbatim, scattered across Phase 16's SUMMARY.md/plan/VERIFICATION files and one dedicated
v2-obligations todo file. The work is consolidating them into `.planning/PROJECT.md` under one
anchor section — none of the 8 points requires new investigation, only citation and transcription.

**Primary recommendation:** Build ONE new live test function that (1) shares a single audit-db path
across sequential `caprun` invocations (a pattern the codebase already uses for block→confirm/deny
pairs, just never for 3+ sessions in one file), (2) replaces every `SELECT id FROM sessions LIMIT 1`
call-site with a session-scoped discovery mechanism (existing code silently assumes exactly one session
per DB — this assumption is the single biggest landmine in this phase), and (3) extends the existing
Mailpit test-client with a `count_messages_for_recipient` helper for the literal count==0 deny
assertion. Treat "one unbroken audit-DAG causal chain" as "one shared audit.db, every session's
`verify_chain` independently true" — a literal single-session chain across mutually-exclusive
confirm/deny/clean outcomes contradicts the pinned `DESIGN-session-trust-state.md` single-session-per-
process model and is NOT achievable without a DESIGN amendment (see Open Questions #1).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Live scenario orchestration (spawn N `caprun` invocations against one DB) | Test harness (`cli/caprun/tests/*.rs`) | — | Tests are the only tier that owns process orchestration; no production code changes needed for composition itself |
| Session/effect-ID discovery across process boundaries | Test harness | Broker/audit (`crates/brokerd/src/audit.rs`) | The DB schema already supports multi-session discovery (`session_id` column everywhere); the gap is entirely in test helper queries, not schema |
| Deny-sends-nothing assertion | Test harness (Mailpit HTTP client) | Broker (`email_send_succeeded`/`email_send_attempted` events) | Both an audit-DB-level check (event absence) AND a live Mailpit-level check (message-count==0) are needed per the coordinator's "no prose, no proxy" mandate |
| Audit-DAG unbroken-edge + anti-staple proof (EXTRACT-02, reused from Phase 15) | Broker/audit (already implemented) | Test harness (re-invocation in the live context) | Phase 15 hard-gate logic is unchanged; Phase 17 just needs to exercise it inside the newly-composed live run, not reimplement it |
| PROJECT.md framing honesty (DOC-01) | Documentation (`.planning/PROJECT.md`) | — | Pure prose; no code tier owns this |
| SMTP send mechanics (lettre/native-tls) | Broker (`crates/brokerd/src/sinks/email_smtp.rs`) | — | Unchanged from Phase 13/16 — Phase 17 does not touch the adapter |

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ACCEPT-01 | Full live acceptance, Linux-verified, ONE unbroken audit DAG: hostile read → I1 demotion → deterministic extraction → tainted recipient+body block → confirm sends exactly once → deny sends nothing; composes CONTROL-01 in the SAME run | See "Live Test Harness — Current State" and "Composition Plan" below; existing primitives in `s9_live_block.rs`/`live_acceptance_tainted_session.rs` cover 4 of 5 needed legs individually — the genuinely new work is (1) shared-DB composition, (2) a live email-sink confirm+Mailpit-capture test (does not exist today), (3) a live email-sink deny+Mailpit-count-zero test (does not exist today) |
| DOC-01 | PROJECT.md explicitly scopes what v1.3 proves/does not prove (taint enforcement via deterministic extractor; does NOT claim taint survives a real LLM planner's regeneration) | See "DOC-01 — Exact Content and Anchor Point" below; all 8 coordinator-mandated points are already recorded verbatim in Phase 16 artifacts and the v2-obligations todo — this is a consolidation, not new research |
</phase_requirements>

## Live Test Harness — Current State (read directly from source)

### How `caprun` is invoked today (one process = one session = one broker)

`cli/caprun/src/main.rs` (verified by direct read, lines 50-308):
- Each invocation: opens the audit DB at the CLI-supplied path (`open_audit_db(&audit_path)`,
  defaults to `:memory:` if omitted), mints ONE new `session_id = Uuid::new_v4()`, calls
  `create_session(...)`, persists it, spawns `run_broker_server` bound to abstract socket
  `\0/agentos/{session_id}` (unique per session — no collision risk across concurrent invocations),
  spawns exactly one `caprun-worker` child, waits for it to exit, aborts the broker task, prints the
  DAG, and exits. **There is no concept of "N sessions in one process" — one `caprun` invocation is
  hard-wired to exactly one session.** [VERIFIED: cli/caprun/src/main.rs:196-308]
- `DESIGN-session-trust-state.md` (line 480) states this explicitly as a DESIGN-level constraint:
  *"single-threaded-per-session process model (`caprun` runs one session to completion per process
  invocation, per `cli/caprun/src/main.rs`'s single-worker-per-session design) — there is no
  multi-worker-per-session concurrency in scope for this milestone."* [VERIFIED: planning-docs/DESIGN-session-trust-state.md:480-482]
- **Consequence for Phase 17:** composing 3+ distinct outcomes (hostile-confirm, hostile-deny,
  clean-control) into "one run" MUST mean multiple sequential `caprun` process invocations sharing one
  audit-db PATH — never a single process with multiple sessions. This is already the pattern used for
  block→confirm and block→deny (see below); it just has never been extended past 2 invocations into
  one file, nor combined with the email sink + Mailpit.

### `caprun confirm`/`caprun deny`/`caprun review` (main.rs:63-96, 322-380)

- Handled as the VERY FIRST branch in `main()`, before any intent parsing. Takes `<effect_id>
  [audit-db-path]` — reopens the SAME persistent DB the original blocking run used (never `:memory:`
  for this step — an in-memory DB would vanish between the two processes). `find_pending_confirmation`
  looks up the row by `effect_id` (globally unique — a UUID, not scoped by session), and the row itself
  carries `session_id` (`crates/brokerd/src/confirmation.rs:176`), so confirm/deny already resolves the
  correct session internally without needing a `SELECT id FROM sessions` scan. **This means the harness
  never needs to guess which session confirm/deny targets — only the FIRST hop (discovering `effect_id`
  from the blocking run's own output) is the fragile part.** [VERIFIED: crates/brokerd/src/confirmation.rs:176,247-276]

### The `SELECT id FROM sessions LIMIT 1` landmine (the single most important finding)

Every existing live test that needs to look up a session_id after a `caprun` run uses this exact
pattern:
```rust
let session_id: String = conn
    .query_row("SELECT id FROM sessions LIMIT 1", [], |row| row.get(0))
    .expect("one session row must exist");
```
This appears in `s9_live_block.rs` (6 call sites: lines 135, 219, 294, 439, 571, and inside
`s9_control_ab_taint_driven` at 815/871) and in `live_acceptance_tainted_session.rs` (4 call sites:
140, 171, 252, 281). **It is only correct because every existing test uses a FRESH tmp-dir/audit.db
per invocation set** — the query is unambiguous ONLY because exactly one session row ever exists in
that file. [VERIFIED: cli/caprun/tests/s9_live_block.rs, cli/caprun/tests/live_acceptance_tainted_session.rs
— direct grep + read]

**This pattern silently breaks the moment a second session is inserted into the same DB file** — which
is exactly what composing CONTROL-01's clean session alongside the hostile-block session into one
shared `audit.db` requires. `LIMIT 1` with no `ORDER BY` returns SQLite's arbitrary/implementation-
defined row order (in practice, insertion/rowid order on an unmodified table, but this is not a
documented guarantee) — a planner that "composes" by literally reusing this query as-is against a
multi-session DB will get an ambiguous/wrong session silently, not a compile error or test failure that
points at the real cause.

**Recommended fix for the planner:** since all invocations within one test are strictly sequential (no
concurrent writers — one `caprun` process fully exits, including broker-task abort and DB flush, before
the harness spawns the next), the safe replacement is `SELECT id FROM sessions ORDER BY rowid DESC LIMIT
1` executed immediately after each invocation, to capture "the session just created by the invocation I
just ran" — OR (more robust, no ordering assumption) capture the session_id from `caprun`'s own stdout,
which already prints `=== Audit DAG (session {session_id}) ===` (`main.rs:293`) before exiting 0/non-
zero. The stdout-capture approach is more churn (parsing) but has zero reliance on SQLite row-order
semantics; the `ORDER BY rowid DESC` approach is a 1-line diff per call site. Either is viable; the
`rowid DESC` approach is lower-risk given the harness's already-established sequential-invocation
discipline (Common Pitfall 1 below expands on this).

### Composition Plan — what a genuinely single-audit.db, multi-session live test looks like

1. Mint ONE tmp dir + ONE `audit_db` path (not per-invocation, as `run_caprun_intent_on` does today —
   see `s9_live_block.rs:90-95`, which mints a fresh `run_id`-suffixed tmp dir on EVERY call). The
   existing `run_caprun_block`/`run_caprun_verb` pair in `live_acceptance_tainted_session.rs` already
   demonstrates the correct shape for 2 invocations sharing one DB (`run_caprun_block(&tmp, &audit_db)`
   takes the shared path as a parameter) — Phase 17 needs to (a) extend this shape to the email sink
   (today `run_caprun_block`/`run_caprun_verb` only exist for `create-file-from-report`), and (b) run 3
   sequential invocations against the SAME path instead of 2.
2. Sequence: (i) `caprun send-email-summary <recipient> <hostile-doc-2anchor> <shared-db>` → blocks
   (reuses `HOSTILE_EMAIL_CONTENT` from `s9_live_block.rs:76-80`, produces a 2-anchor `to`+`body` block)
   → discover `effect_id` from the sink_blocked event (needs the session-scoping fix above) → `caprun
   confirm <effect_id> <shared-db>` → assert exit 0, `Released`, `email_send_succeeded` event, AND
   Mailpit `wait_for_recipient_captured` for the derived recipient (`accounts@ev1l.com`, per the
   existing HOSTILE_EMAIL_CONTENT fixture's concat-derivation). (ii) A SECOND hostile-block invocation
   (same or a differently-tagged hostile fixture — needed because confirm is a ONE-SHOT terminal
   decision per `PendingConfirmation`, so the SAME blocked effect cannot be both confirmed and denied;
   a fresh block is required for the deny leg) → discover its OWN `effect_id` → `caprun deny <effect_id>
   <shared-db>` → assert exit 2, `Denied`, no `email_send_succeeded`, AND the NEW numeric Mailpit
   count==0 assertion (see below). (iii) The CONTROL-01 clean-intent invocation (`CLEAN_PATH_CONTENT`, a
   UNIQUE per-run recipient as `s9_control_ab_taint_driven` already does) → assert exit 0,
   `plan_node_evaluated`, zero `pending_confirmations` rows for ITS session, `email_send_succeeded`, and
   Mailpit capture.
3. After all three invocations, open the ONE shared `audit_db` connection and, for EACH of the (now
   three) `session_id` rows present, assert `verify_chain(&conn, &session_id)` is true, AND re-run
   Phase 15's EXTRACT-02 unbroken-edge + anti-staple check (`crates/brokerd`'s existing per-anchor walk,
   reused, not reimplemented — see `crates/brokerd/tests/extract_provenance_threading.rs`'s
   `assert_unbroken_edge`/`genuine_derivation_binds` helpers per STATE.md's Phase 15 decisions log)
   against the hostile-block session's TWO anchors (`to` and `body`) specifically, since Phase 17's
   success criterion #3 is an explicit HARD GATE reusing this exact proof.
4. This DB-file-level composition (3 sessions, all sharing one file, all independently `verify_chain`-
   true, all queryable via one open connection) is the achievable, architecturally-sound interpretation
   of "the whole scenario is one unbroken audit-DAG causal chain" — see Open Questions #1 for why a
   LITERAL single parent_id-linked chain across all three outcomes is not achievable without
   contradicting the pinned single-session-per-process DESIGN.

### Mailpit HTTP client — existing helpers and the count==0 gap

`cli/caprun/tests/s9_live_block.rs`'s `mod mailpit_client` (lines 624-775) already implements, in
std-only Rust (deliberate small duplication from `crates/brokerd/tests/email_smtp_acceptance.rs`'s
shape — no new Cargo dependency):
- `host()` — reads `CAPRUN_SMTP_HOST` (set by `scripts/mailpit-verify.sh` to the resolved sidecar
  container IP), fixed HTTP API port 8025.
- `message_ids(host)` — GETs `/api/v1/messages?limit=250`, returns ALL message IDs (unfiltered).
- `detail_addressed_to(host, id, recipient)` — GETs `/api/v1/message/{id}`, checks the `To` array.
- `wait_for_recipient_captured(host, recipient)` — polls (50×200ms) until a message addressed to
  `recipient` appears; panics on timeout. Used for the CONFIRM/CONTROL-01 positive case.
- `assert_recipient_not_captured(host, recipient)` — single-pass negative check (no polling — correct,
  per its own doc comment, only when the caller already knows no send was ever attempted, i.e. after a
  BLOCK that was never confirmed).

**Gap:** none of these return a numeric count — they are all presence/absence checks. The coordinator's
requirement A explicitly wants "a post-deny Mailpit message-count == 0 / inbox-empty assertion... no
prose, no proxy." **Recommended new helper** (small, additive — same module, same
`message_ids`/`detail_addressed_to` primitives already present):
```rust
/// Returns the COUNT of captured messages addressed to `recipient` — the literal
/// numeric primitive `caprun deny`'s "sends nothing" claim needs (ACCEPT-01
/// point A), rather than the boolean assert_recipient_not_captured.
pub fn count_messages_for_recipient(host: &str, recipient: &str) -> usize {
    message_ids(host)
        .iter()
        .filter(|id| detail_addressed_to(host, id, recipient))
        .count()
}
```
Then the deny-leg test asserts `count_messages_for_recipient(&host, &deny_recipient) == 0` — a genuine
numeric assertion, not a proxy. Use a per-run-unique recipient (embed a UUID into the deny leg's hostile
doc content, mirroring how `s9_control_ab_taint_driven` already does this for the clean leg's
recipient) rather than reusing the fixed `"accounts@ev1l.com"` literal, so the assertion is airtight
against cross-test-binary pollution in a shared Mailpit inbox (the existing fixed-literal usage is
already argued-safe in `s9_live_block.rs`'s own comments, but a fresh literal removes any doubt and
costs nothing).

**A GLOBAL inbox-count==0 check (literally zero messages in the entire Mailpit instance) is NOT
achievable** given `mailpit-verify.sh` runs ONE shared Mailpit sidecar for the WHOLE `cargo test
--workspace` invocation (or whatever `MAILPIT_VERIFY_CMD` scopes to) — other tests/other legs of this
same composed scenario legitimately send mail into the same inbox. A recipient-scoped count==0 is the
correct, already-established idiom in this codebase (see `assert_recipient_not_captured`'s own doc
comment explaining exactly this constraint) — do not attempt a bare "GET /api/v1/messages, assert
len()==0" against the whole inbox.

## Standard Stack

No new external dependencies are introduced by this phase — it is a test-harness composition +
documentation phase. All Rust crates involved (`rusqlite`, `uuid`, `sha2`, `serde_json`, `lettre
0.11.22` [VERIFIED: crates/brokerd/Cargo.toml:23]) are already pinned from Phase 13/14/15/16 and
unchanged here.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| (none new) | — | — | Phase 17 adds test code + docs only, no new crate dependencies |

### Package Legitimacy Audit

**Not applicable — this phase installs no new packages.** No `package-legitimacy check` run was
needed; `Cargo.toml` diffs for this phase should be empty or test-only (`[dev-dependencies]` additions,
if any, would reuse crates already vetted in Phase 13/16, e.g. no new `serde_json`/`uuid` version
bumps expected).

## Architecture Patterns

### System Architecture Diagram — the composed live-acceptance run

```
                    ONE shared audit.db (tmp dir, created once per test run)
                              │
   ┌──────────────────────────┼──────────────────────────┬─────────────────────────┐
   │ Invocation 1              │ Invocation 2 (same DB)    │ Invocation 3 (same DB)   │
   │ caprun send-email-summary │ caprun confirm <eid1>     │ caprun send-email-summary│
   │   <hostile doc A>         │   <shared-db>             │   <clean intent, unique  │
   │   <shared-db>             │                           │    recipient>            │
   │        │                  │        │                  │        │                 │
   │  worker reads doc (fd)    │  find_pending_confirmation│  worker: no fragments →  │
   │  → ReportClaims           │   (by effect_id, session  │   plan_from_intent routes│
   │  → mint_from_read         │    resolved internally)   │   UserTrusted → to/body  │
   │  → session_demoted (I1)   │  → verify_chain + digest  │  → executor: Allowed     │
   │  → extractor derives      │    recompute (fail-closed)│  → email.send dispatch   │
   │    to+body (concat        │  → email.send (real       │  → real send             │
   │    transform, worker-side)│    adapter, lettre/SMTP)  │        │                 │
   │  → plan_from_intent       │        │                  │        ▼                 │
   │  → executor: collect-then-│        ▼                  │  Mailpit :1025 (SMTP)    │
   │    Block (2 anchors)      │  Mailpit :1025 (SMTP)      │  Mailpit :8025 (HTTP API)│
   │  → sink_blocked (durable) │  Mailpit :8025 (HTTP API)  │  wait_for_recipient_     │
   │  → caprun exits non-zero  │  wait_for_recipient_       │    captured(unique_recip)│
   └───────────────────────────┴  captured(hostile_recip)  ─┴─────────────────────────┘
                              │
   ┌──────────────────────────┼──────────────────────────┐
   │ Invocation 4 (same DB)    │ Invocation 5 (same DB)    │
   │ caprun send-email-summary │ caprun deny <eid2>        │
   │   <hostile doc B, unique  │   <shared-db>             │
   │    recipient variant>     │  → deny() durable         │
   │   <shared-db>             │    confirm_denied event   │
   │  → blocks (2 anchors)     │  → NO sink invocation     │
   └──────────────────────────┴──────────────────────────┴─────────────────────
                              │
                              ▼
              Harness opens shared audit.db ONCE, enumerates ALL session_id
              rows, asserts verify_chain(conn, sid) TRUE for each, re-runs
              Phase 15's per-anchor unbroken-edge + anti-staple proof against
              the two hostile-block sessions' `to`+`body` anchors, and asserts
              count_messages_for_recipient(deny_recipient) == 0 via Mailpit.
```

### Recommended Test-Harness Structure

```
cli/caprun/tests/
├── s9_live_block.rs                      # existing — CONTROL-01/CONTROL-02, per-invocation DBs (unchanged)
├── live_acceptance_tainted_session.rs    # existing — file.create block→confirm/deny pattern (unchanged; the MODEL to extend)
└── live_acceptance_v1_3.rs               # NEW (Phase 17) — the composed ACCEPT-01 scenario:
                                           #   shared-db harness fns (email sink variant of
                                           #   run_caprun_block/run_caprun_verb), the
                                           #   count_messages_for_recipient Mailpit helper,
                                           #   and the enumerate-all-sessions verify_chain loop
```

### Pattern 1: Shared-audit-db, sequential-invocation composition
**What:** Multiple `caprun` process invocations (and confirm/deny CLI calls) targeting the SAME
audit-db file path, run strictly sequentially within one `#[test]` function.
**When to use:** Whenever a single test needs to prove properties that span more than one session
(e.g., "this DB now contains a confirmed hostile send, a denied hostile send, and an allowed clean
send, and every one of them is a verifiably intact chain").
**Example (extending the existing `run_caprun_block`/`run_caprun_verb` shape from
`live_acceptance_tainted_session.rs:59-112` to the email sink):**
```rust
// Source: adapted from cli/caprun/tests/live_acceptance_tainted_session.rs (existing pattern),
// applied to the email sink instead of file.create.
fn run_caprun_email_on(
    recipient: &str,
    content: &[u8],
    audit_db: &std::path::Path, // SHARED path — caller decides, never minted per-call
    tag: &str,
) -> bool {
    let workspace_file = audit_db.parent().unwrap().join(format!("{tag}.txt"));
    std::fs::write(&workspace_file, content).expect("write workspace file");
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    std::process::Command::new(caprun_bin)
        .arg("send-email-summary").arg(recipient)
        .arg(workspace_file.to_str().unwrap())
        .arg(audit_db.to_str().unwrap())
        .output().expect("spawn caprun").status.success()
}
```

### Pattern 2: Session discovery after a shared-DB invocation (replaces `LIMIT 1`)
**What:** Discover the session_id / effect_id created by the invocation JUST run, without assuming
it is the only row in the table.
**When to use:** Any assertion querying `sessions`/`events` after a shared-DB invocation.
**Example:**
```rust
// Source: this research (no direct upstream precedent — LIMIT 1 is the anti-pattern being replaced)
fn latest_session_id(conn: &rusqlite::Connection) -> String {
    conn.query_row(
        "SELECT id FROM sessions ORDER BY rowid DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .expect("at least one session row must exist")
}
```

### Anti-Patterns to Avoid
- **`SELECT id FROM sessions LIMIT 1` against a shared multi-session DB:** silently returns an
  arbitrary/wrong session once a second session exists in the file — no compile error, no panic, just a
  wrong assertion target. Audit every call site before reusing them in the new composed test (do not
  copy-paste the existing per-invocation-DB tests' query verbatim).
- **Reusing a fixed hostile literal recipient across composed legs without per-run uniqueness where a
  numeric Mailpit count assertion is involved:** fine for presence/absence (`assert_recipient_not_
  captured`'s existing usage is safe because it's a negative claim that never depends on run-count), but
  risk-reducing to make unique for any NEW count==0 assertion.
- **Minting `:memory:` for any leg of the composed scenario:** an in-memory DB cannot be reopened by a
  follow-up process (confirm/deny, or the harness's own final `verify_chain` sweep) — always a real
  file path, as every existing live test already does correctly.
- **Treating "one causal DAG" as requiring a single `parent_id` chain across confirm+deny+clean:**
  contradicts the pinned single-session-per-process DESIGN model; do not attempt to merge these into one
  session's event chain (see Open Questions #1).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Audit-DAG unbroken-edge + anti-staple proof (EXTRACT-02) | A new "prove genuine taint propagation" checker for the live run | The existing `assert_unbroken_edge`/`genuine_derivation_binds` routines (per STATE.md's Phase 15 decisions log, implemented in `crates/brokerd/tests/extract_provenance_threading.rs`) | This is the phase's HARD GATE (#3) reusing Phase 15's already-adversarially-verified proof; reimplementing it risks silently weakening the exact check the milestone depends on |
| Mailpit HTTP client (chunked-transfer decode, JSON parse) | A new HTTP client for the live test | Extend `s9_live_block.rs`'s existing `mod mailpit_client` (chunked-decode already handles Mailpit's Go HTTP server's streaming behavior — an empirically-discovered gotcha per its own doc comment) | Duplicating a second chunked-decode implementation risks reintroducing the exact bug this module's comments say was already found and fixed |
| Session/effect-ID discovery across process boundaries | A new IPC/signaling mechanism between the harness and `caprun` | SQLite queries against the shared, persistent (never `:memory:`) audit-db file, executed strictly after each process's `.output()` call returns | The DB is already the durable source of truth every existing live test uses; introducing a second discovery channel (e.g. parsing stdout) is unnecessary complexity unless the `ORDER BY rowid DESC` approach proves insufficient in practice |

**Key insight:** Phase 17's technical risk is almost entirely in test-harness composition correctness
(the `LIMIT 1` landmine, minimum-3-sessions-required-for-3-mutually-exclusive-outcomes), not in new
production code. Resist the temptation to add new brokerd/executor code to make composition easier —
none is needed.

## Common Pitfalls

### Pitfall 1: `SELECT id FROM sessions LIMIT 1` silently targets the wrong session
**What goes wrong:** Once 2+ sessions exist in a shared audit.db, every existing test's session-lookup
query becomes ambiguous. A copy-pasted test built by extending an existing file naively will discover
this only when an assertion fails against data from the WRONG session (e.g., a hostile-block assertion
silently checking the clean-control session's events instead) — a confusing failure mode, not an
obvious one.
**Why it happens:** Every existing live test happens to use a fresh tmp-dir/audit.db per invocation set,
so the query has always been correct by construction, never by design.
**How to avoid:** Grep every new test file for `LIMIT 1` before considering it complete; replace with
`ORDER BY rowid DESC LIMIT 1` captured IMMEDIATELY after each invocation (not once, at the end, for all
three) or thread the session_id from stdout capture.
**Warning signs:** Any assertion in the new composed test that checks event TYPE presence/absence
without also checking that the `session_id` used matches the invocation just run.

### Pitfall 2: Confirm and deny are mutually exclusive terminal states on ONE `PendingConfirmation`
**What goes wrong:** Attempting to "compose" the confirm and deny legs by reusing the SAME blocked
effect for both (confirm it, then somehow also deny it) is architecturally impossible — `confirm()`/
`deny()` transition a `PendingConfirmation` row to a TERMINAL state (`Released`/`Denied`), and a second
call against the same `effect_id` returns `AlreadyTerminal` (exit code 5), not a fresh result.
**Why it happens:** The coordinator's language ("hostile recipient+body block → confirm sends exactly
once ... deny sends nothing") reads as if both outcomes apply to the same block; they cannot.
**How to avoid:** The composed scenario needs (at minimum) TWO separate hostile-block invocations —
one that gets confirmed, one that gets denied — plus the one clean-control invocation. Minimum 3
sessions, not 2.
**Warning signs:** A plan task that says "block once, then both confirm and deny it."

### Pitfall 3: Mailpit's shared inbox means presence/absence and count checks must be recipient-scoped
**What goes wrong:** A literal "assert Mailpit has zero total messages" fails the moment any OTHER leg
of the SAME composed test (or any concurrently-run test binary sharing the same sidecar) has already
sent mail — which is guaranteed here, since the composed scenario's confirm-leg and clean-control leg
both send successfully into the SAME shared inbox.
**Why it happens:** `mailpit-verify.sh` runs ONE Mailpit sidecar for the entire verification command
scope (default: `cargo test --workspace`), by design (see the script's own header comments) — not one
per test.
**How to avoid:** Every Mailpit assertion in this codebase is already recipient-scoped
(`wait_for_recipient_captured`, `assert_recipient_not_captured`) — extend that idiom
(`count_messages_for_recipient`) rather than querying the whole inbox.
**Warning signs:** Any new Mailpit helper that calls `/api/v1/messages` and asserts on `.len()` without
filtering by recipient first.

### Pitfall 4: `MAILPIT_VERIFY_CMD` scoping must include the new test file/function explicitly
**What goes wrong:** Running the new composed test without scoping `MAILPIT_VERIFY_CMD` to it
specifically means either (a) the whole workspace suite runs (slow, and other tests' sends pollute the
shared inbox the new test also queries) or (b) an unscoped run accidentally omits the new test file from
compilation/execution if the test-binary name is misremembered.
**Why it happens:** `scripts/mailpit-verify.sh`'s default is `cargo test --workspace --no-fail-fast`;
Phase 16's own precedent (`MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block
s9_control_ab_taint_driven'`) shows the correct scoping idiom.
**How to avoid:** When independently re-running the live proof (per the coordinator's standing gate),
explicitly scope: `MAILPIT_VERIFY_CMD='cargo test -p caprun --test <new_test_file_name>
<new_test_fn_name>' bash scripts/mailpit-verify.sh`.
**Warning signs:** A verification report that doesn't name the exact scoped command used, or that pipes
output through `tail`/`grep` without capturing `$?` first (see the project's own `learned-rules.md`
entry "Verification exit code through a pipe" — `script | tail` returns `tail`'s exit status, not the
test binary's).

### Pitfall 5: The final-wave executor self-marking the phase-level ROADMAP.md checkbox
**What goes wrong:** Per the user's own memory record (2-for-2 across Phases 15 and 16), the last-wave
executor's own doc-completion commit has repeatedly flipped ROADMAP.md's PHASE-level checkbox
prematurely, not just its own plan-line checkbox — before independent verification actually ran.
**Why it happens:** A recurring GSD tooling behavior, not specific to this phase's content.
**How to avoid:** Diff-check ROADMAP.md after the final wave's merge, specifically for the Phase 17
line, before recording DONE. This is a standing coordinator gate, not new to this research, but
repeating it here since Phase 17 IS the final phase of this milestone (highest-stakes recurrence).

## Code Examples

### Discovering `effect_id` from a fresh block in a shared, multi-session DB
```rust
// Source: adapted from cli/caprun/tests/live_acceptance_tainted_session.rs:138-152 (existing
// pattern), corrected for the shared-DB, multi-session case (Pitfall 1 fix applied).
fn discover_latest_blocked_effect_id(audit_db: &std::path::Path) -> uuid::Uuid {
    use brokerd::audit::{find_event_by_type, open_audit_db};
    let conn = open_audit_db(audit_db.to_str().unwrap()).expect("open audit DB");
    let session_id: String = conn
        .query_row("SELECT id FROM sessions ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0))
        .expect("a session row must exist for the invocation just run");
    let blocked = find_event_by_type(&conn, &session_id, "sink_blocked")
        .expect("query sink_blocked")
        .expect("sink_blocked event must exist for the invocation just run");
    blocked.anchors.first().expect("anchor must be present").effect_id
}
```

### Enumerating all sessions in the shared DB for the final verify_chain sweep
```rust
// Source: this research — new, extends the existing single-session verify_chain call pattern
// (e.g. s9_live_block.rs:490) to a multi-session DB.
fn all_session_ids(conn: &rusqlite::Connection) -> Vec<String> {
    let mut stmt = conn.prepare("SELECT id FROM sessions ORDER BY rowid").unwrap();
    stmt.query_map([], |row| row.get(0)).unwrap().filter_map(Result::ok).collect()
}
// then: for sid in all_session_ids(&conn) { assert!(verify_chain(&conn, &sid), "session {sid} chain broken"); }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| Per-invocation fresh tmp-dir/audit.db (`s9_live_block.rs`'s `run_caprun_intent_on`) | Shared audit-db path across sequential invocations (`live_acceptance_tainted_session.rs`'s `run_caprun_block`/`run_caprun_verb`, for 2 invocations) | Introduced Phase 11 (v1.2), reused Phase 16 | Phase 17 extends this to 3+ invocations and to the email sink — no new mechanism, more invocations sharing it |
| "same run" == same Rust `#[test]` function, different audit.db files per half (`s9_control_ab_taint_driven`) | "same run" == same audit.db FILE across all halves (this phase's requirement) | Phase 17 (per coordinator's explicit correction) | The coordinator explicitly flags the Phase 16 composition as insufficiently composed — Phase 17 must not repeat that shape |

**Deprecated/outdated:**
- Treating `s9_control_ab_taint_driven`'s two-audit.db composition as satisfying "one unbroken audit
  DAG" — the coordinator has explicitly rejected this shape for Phase 17; it remains valid as a
  CONTROL-01 regression test in its own right, just not as the ACCEPT-01 proof.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | "One unbroken audit-DAG causal chain" is satisfiable as "one shared audit.db file, every session's `verify_chain` independently true" rather than one literal single-session parent_id chain across confirm/deny/clean | Composition Plan, Open Questions #1 | If the coordinator actually requires a literal single-session chain, this phase needs a DESIGN amendment (contradicting the pinned single-session-per-process model) before any test code is written — get sign-off before planning tasks around the shared-DB-multiple-sessions interpretation |
| A2 | `ORDER BY rowid DESC LIMIT 1`, executed immediately after each sequential invocation, correctly identifies "the session just created" without needing stdout-parsing | Composition Plan, Pitfall 1 | If SQLite's rowid ordering is somehow not monotonic with insertion order in this schema (unlikely — no `INTEGER PRIMARY KEY` reordering triggers are present, per the schema read), this fix would need to fall back to stdout capture instead |
| A3 | A minimum of 3 sessions (2 hostile-block-then-decide + 1 clean) is both necessary AND sufficient to satisfy ACCEPT-01's composed scenario, per Pitfall 2's mutual-exclusivity finding | Composition Plan, Pitfall 2 | If the coordinator intends a 4th or different combination (e.g., two DIFFERENT hostile documents to avoid literal-content reuse across the confirm and deny legs), the plan should confirm this with caprun-opus-77 rather than assume the same `HOSTILE_EMAIL_CONTENT` fixture reused for both |

**None of these assumptions concern DOC-01** — all 8 of its points are sourced directly from committed
Phase 16 artifacts (see next section), not inferred.

## Open Questions

1. **Does "one unbroken audit-DAG causal chain" require a literal single-session parent_id chain, or is
   "one shared audit.db, every session's chain independently verified" sufficient?**
   - What we know: `verify_chain(conn, session_id)` is architecturally session-scoped (takes a
     `session_id` parameter, its recursive CTE filters `WHERE session_id = ?1` at every step —
     `crates/brokerd/src/audit.rs:477-500`). `DESIGN-session-trust-state.md` pins "one process
     invocation = one session" as a DESIGN-level constraint (line 480-482). Confirm and deny are
     mutually exclusive terminal states on one blocked effect (Pitfall 2), so the composed scenario
     structurally needs ≥3 sessions.
   - What's unclear: whether the coordinator's phrasing ("multiple plan-node submissions/sessions if
     needed but one causal DAG") already anticipates and accepts the multi-session-in-one-file
     interpretation, or whether "one causal DAG" is meant more literally and would require a DESIGN
     amendment (e.g., a new cross-session linking mechanism) to satisfy strictly.
   - Recommendation: the planner should surface this interpretation explicitly to caprun-opus-77 (per
     the coordinator's own standing gate: "any forced DESIGN deviation → STOP and flag the coordinator")
     BEFORE writing tasks that assume the multi-session-shared-DB shape is sufficient. This research's
     recommendation (Assumption A1) is that it IS sufficient and is the only architecturally sound
     reading, but it is a judgment call, not a verified fact, and should be confirmed rather than
     silently assumed by the plan.

2. **Should the deny leg reuse `HOSTILE_EMAIL_CONTENT` (same fixture as the confirm leg) or a distinct
   hostile-doc variant?**
   - What we know: confirm and deny cannot share one `PendingConfirmation` (Pitfall 2), so two separate
     blocking invocations are needed regardless.
   - What's unclear: whether using the byte-identical `HOSTILE_EMAIL_CONTENT` fixture twice (once
     confirmed, once denied) is acceptable, or whether the coordinator wants a visibly distinct fixture
     for the deny leg to avoid any appearance that "the same block" was both confirmed and denied.
   - Recommendation: reusing the same fixture is technically sound (two separate invocations,
     necessarily two separate `effect_id`s/sessions) and keeps the new test file's fixture surface
     minimal; flag this choice in the plan's task description so it's an explicit, reviewable decision
     rather than an implicit one.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Colima + Docker | All Linux-gated live tests (unchanged from Phase 13-16) | ✓ (per CLAUDE.md, already the project's standing verification path) | — | — |
| `axllent/mailpit` Docker image | Live SMTP capture + HTTP API assertions | ✓ (already used by `scripts/mailpit-verify.sh` since Phase 13) | — | — |
| `rust:1` Docker image | Verification container | ✓ (already the project's standing recipe) | — | — |
| `libssl-dev`/`pkg-config` (inside the `rust:1` container) | `lettre`'s `native-tls` feature build | ✓ (already installed by `scripts/mailpit-verify.sh:115`) | — | — |

**Missing dependencies with no fallback:** none — Phase 17 reuses the exact toolchain established in
Phase 13/16; no new environment requirement is introduced.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in), `#[cfg(target_os = "linux")]`-gated live assertions |
| Config file | none — plain `Cargo.toml` workspace, no test-framework config file |
| Quick run command | `cargo build --workspace` (compile check only, macOS-safe) |
| Full suite command | `bash scripts/mailpit-verify.sh` (default `MAILPIT_VERIFY_CMD='cargo test --workspace --no-fail-fast'`) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ACCEPT-01 | Composed live scenario: hostile-block→confirm→Mailpit-capture, hostile-block→deny→Mailpit-count-zero, clean-control→Mailpit-capture, all in ONE shared audit.db, all sessions `verify_chain`-true | integration (live, Linux-gated) | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test <new_test_file> <new_test_fn>' bash scripts/mailpit-verify.sh` | ❌ Wave 0 — new test file needed |
| ACCEPT-01 (hard gate #3) | Phase 15's unbroken-edge + anti-staple proof holds for the live composed run's hostile-block session(s) | integration (reuses existing helper) | same command as above, asserting via `crates/brokerd/tests/extract_provenance_threading.rs`'s existing `assert_unbroken_edge`/`genuine_derivation_binds` routines applied to the new session's anchors | ✅ helper exists — ❌ new call-site needed |
| DOC-01 | PROJECT.md carries all 8 framing points, anchored appropriately | manual (documentation review, not automatable) | none — human/adversarial review of the edited PROJECT.md | N/A — prose task |

### Sampling Rate
- **Per task commit:** `cargo build --workspace` (macOS, compile-only sanity)
- **Per wave merge:** `bash scripts/mailpit-verify.sh` scoped via `MAILPIT_VERIFY_CMD` to the new test
  file/function, then unscoped `cargo test --workspace --no-fail-fast` once the new test is stable
- **Phase gate:** Full unscoped `bash scripts/mailpit-verify.sh` run, with `$?` captured BEFORE any pipe
  (per the project's own learned-rules.md entry on this exact failure mode), before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `cli/caprun/tests/live_acceptance_v1_3.rs` (or equivalent new name) — the composed ACCEPT-01
      scenario; covers ACCEPT-01 end to end
- [ ] Email-sink variant of `run_caprun_block`/`run_caprun_verb` (currently only exist for
      `create-file-from-report` in `live_acceptance_tainted_session.rs`) — needed as a shared harness fn,
      whether added to that file or the new one
- [ ] `count_messages_for_recipient` addition to (or duplication of) `mailpit_client` — covers the
      "deny sends nothing" numeric assertion
- [ ] No new fixtures required — `HOSTILE_EMAIL_CONTENT` and `CLEAN_PATH_CONTENT` (both already in
      `s9_live_block.rs`) are reusable verbatim (const visibility permitting — currently `const` items
      are file-private; either re-declare them in the new file or make them `pub(crate)`/move to a
      shared test-fixtures module)

## Security Domain

> This project's "security" IS the product under test (kernel confinement, taint tracking, plan-node
> mediation) — conventional web-app ASVS categories (session cookies, CSRF, etc.) do not map cleanly
> onto a CLI/broker/worker architecture. The relevant "security" work for Phase 17 is DOCUMENTATION
> honesty about already-implemented mitigations (DOC-01), not new mitigation code.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | No | No human-facing auth surface in this CLI/broker architecture |
| V3 Session Management | Partially (repurposed) | caprun's own `Session`/`SessionStatus` (Draft/Active) — not ASVS web sessions; already covered by `DESIGN-session-trust-state.md`, unchanged this phase |
| V4 Access Control | Yes (repurposed) | The I2 executor's sink-arg blocking IS the access-control mechanism; unchanged this phase — Phase 17 only exercises it live |
| V5 Input Validation | Yes | The deterministic extractor's marker-anchored parsing (`Reply-To:`/`Domain:`/`Body:`) — unchanged this phase, Phase 15's concern |
| V6 Cryptography | Partially | SHA-256 audit-chain hashing (`crates/brokerd/src/audit.rs`) — unchanged; DOC-01 point 3 requires HONESTLY stating this chain is NOT externally anchored/authenticated (see below) |

### Known Threat Patterns for this stack (already mitigated, re-verified live this phase — not new work)
| Pattern | STRIDE | Standard Mitigation (already implemented) |
|---------|--------|---------------------------------------------|
| CRLF/header injection via tainted email body | Tampering | `lettre`'s typed `Address`/`Message` builder (SMTP-05, Phase 13) — never a raw `format!()`-built header |
| Value injection at sink args (tainted recipient/body) | Tampering, Elevation of Privilege | Executor's I2 collect-then-Block over `ValueNode` taint (Phase 14/15) |
| Audit-chain tamper (single-store or non-recomputing multi-store) | Tampering, Repudiation | SHA-256 hash chain + `verify_chain` — **honestly scoped as NOT externally anchored** (DOC-01 point 3) |
| Replay of an Allowed `SubmitPlanNode` (no CAS on the trusted-send path) | Repudiation | Durable per-attempt ledger (`email_send_attempted`) makes each send auditable but does NOT prevent duplication — **accepted residual risk, DOC-01 point 5** |

## DOC-01 — Exact Content and Anchor Point

### Where PROJECT.md currently stands (verified by direct read)

- Line 19-59: "Current Milestone: v1.3" section already states the core scope-limiting sentence
  ("Explicitly not reopened: the LLM planner stays out/deterministic. v1.3 proves taint *enforcement*
  through a deterministic extractor — it does not claim taint survives a real LLM planner's
  regeneration ('laundering'); that is v1.4+ (see `DOC-01`)." — lines 43-46). This is coordinator point
  set #C's HEADLINE claim (the DOC-01 requirement text itself) and is ALREADY present. **It does not
  yet reference this research's/Phase 16's 8 detailed sub-points (the CONTROL-01 vacuous-proof caveat,
  I1's honest scope, verify_chain's honest scope, guard-(c)'s runtime-vs-compile-time gap, the replay
  residual risk, ProvideIntent enforcement, CONFIRM-01's live-vs-synthetic status, or the
  brokerd→lettre→native-tls link).**
- Line 166-195: "Out of Scope" table already references `DOC-01` for the LLM-planner line (line
  178-180) and separately lists live-SES-downgrade and content-taxonomy-descope reasoning — good
  existing precedent for terse, reason-cited bullet style.
- Line 237-242: a "Residual risks (acknowledged, not solved in v0)" bullet exists but is explicitly
  v0-era (fd-revocation, planner/intent injection, steganographic encoding, broker-bugs-full-compromise)
  — **it has NOT been updated for v1.2/v1.3's residual risks** (verify_chain scope, replay risk, guard-c
  runtime-vs-build-exclusion) despite the additional_context's expectation that this section "should
  probably be the anchor point."
- Line 368-370 ("Key Decisions" table): already carries the REOPENED-v1.3/NOT-reopened-v1.3 decision
  rows, including the exact DOC-01 sentence for the LLM-planner non-reopening.

### Recommended minimal edit (for the planner to scope as tasks, not to author here)

1. **Extend the existing v0-era "Residual risks" bullet (line 237-242)** into a versioned list — keep
   the v0 items (still valid, unchanged), and ADD a new "v1.3 residual risks (Phase 16, DOC-01)" clause
   immediately after it, carrying (in this order, to mirror the coordinator's numbered list C):
   - Point 3 (verify_chain scope) — cite `crates/brokerd/src/audit.rs`'s `verify_chain` doc comment
     and Phase 16 SUMMARY (16-02-SUMMARY.md:39,150,159) verbatim language: *"detects single-store and
     non-recomputing multi-store tampering... NOT authenticated or externally-anchored... an actor with
     events write access can forge it end-to-end... Accepted Residual Risk... v2: keyed-MAC/head-pin."*
   - Point 5 (Allowed-path replay, no CAS) — cite 16-04-SUMMARY.md:210-212 verbatim: *"The Allowed
     email.send path has NO CAS... a replayed SubmitPlanNode sends N emails... ACCEPTED RESIDUAL RISK...
     durable per-attempt ledger makes each send auditable."*
   - Point 4 (guard-(c) runtime flag, not compile-time exclusion) — cite the v2-obligations todo file
     (`.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md`, item 4) verbatim:
     *"the forced-Active mint CODE remains compiled into the production binary... a runtime default-
     deny, not an absence guarantee... v2: convert this to a build-excluded path."*
2. **Add a new sentence to the "Current Milestone: v1.3" section (near line 43-46, alongside the
   existing LLM-planner non-reopening sentence)** carrying the remaining points:
   - Point 1 (CONTROL-01's honest scope: "a send built from trusted intent is Allowed and delivers; a
     send whose args are doc-derived is Blocked" — NOT "same doc, taint flipped"; the benign doc is
     decorative on the clean path) — this exact framing already appears verbatim in
     `s9_live_block.rs`'s own doc comments (lines 199-201, 790-794) and should be lifted into PROJECT.md
     rather than re-derived.
   - Point 2 (I1's honest scope: draft-only triggers on a REPORTED read via `mint_from_read`, not on
     fd release) — cite the v2-obligations todo file item 1's exact qualifier language, already
     flagged as a "DOC-01 QUALIFIER (Phase 16 SUMMARY MUST carry forward)."
   - Point 6 (ProvideIntent enforcement, not assumption) — cite 16-04-SUMMARY.md:17,141 verbatim:
     "ProvideIntent accepted EXACTLY ONCE and ONLY BEFORE any RequestFd, broker-enforced."
   - Point 7 (CONFIRM-01's live-vs-synthetic status) — Phase 17 itself is what makes this claim TRUE
     for the first time (per the coordinator's own framing: "proven end-to-end here for the first time;
     at Phase 16 it was a synthetic fixture") — this sentence should be added AFTER Phase 17's own live
     test passes, not before; the planner should sequence this edit as a LATE task in the phase, not an
     early one, so it doesn't prematurely claim something not yet proven.
   - Point 8 (confined worker → brokerd → lettre → native-tls dependency chain) — a single factual
     sentence; cite `scripts/mailpit-verify.sh`'s own header comment (lines 23-25) and
     `crates/brokerd/Cargo.toml:23` (`lettre = "0.11.22"`) [VERIFIED: crates/brokerd/Cargo.toml:23].
3. **Do not create a brand-new top-level section** — the additional_context's own instinct (anchor to
   the existing residual-risks section rather than a new one) is correct and avoids duplicating
   REQUIREMENTS.md's DOC-01 entry, which already states the headline claim; PROJECT.md's job is the
   detailed 8-point scope, not a restatement of the one-line requirement.

**All 8 points' source material is [CITED: internal repo — Phase 16 SUMMARY/plan/VERIFICATION/todo
files]** — none require external research; the planner's task is transcription + correct anchoring,
not investigation.

## Sources

### Primary (HIGH confidence — direct source-code reads this session)
- `cli/caprun/src/main.rs` (full read) — CLI invocation model, confirm/deny/review dispatch, session
  creation, broker spawn lifecycle
- `cli/caprun/tests/s9_live_block.rs` (full read) — existing live test patterns, Mailpit client module,
  the `LIMIT 1` pattern, CONTROL-01/CONTROL-02 composition shape
- `cli/caprun/tests/live_acceptance_tainted_session.rs` (full read) — the existing shared-audit-db,
  2-invocation (block→confirm/deny) pattern for `file.create`
- `crates/brokerd/src/audit.rs` (grep + targeted read) — `verify_chain`'s session-scoping, schema DDL
  (`sessions`/`events`/`pending_confirmations` tables)
- `crates/brokerd/src/confirmation.rs` (grep + targeted read) — `PendingConfirmation.session_id`,
  `find_pending_confirmation`, confirm/deny state-transition terminality
- `planning-docs/DESIGN-session-trust-state.md` (grep) — single-session-per-process DESIGN constraint
  (line 480-482)
- `.planning/PROJECT.md` (full read) — current DOC-01 coverage, residual-risks section, Key Decisions
- `.planning/phases/16-confirm-ux-literal-binding-negative-controls/{16-02,16-04}-SUMMARY.md`,
  `16-04-PLAN.md`, `16-VERIFICATION.md` (grep + targeted read) — exact sourced language for DOC-01
  points 1-2, 3, 5, 6
- `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` (full read) — the
  consolidated v2-obligations list underlying DOC-01 points 2, 3, 4
- `scripts/mailpit-verify.sh` (targeted read) — verification recipe, `MAILPIT_VERIFY_CMD` scoping
  precedent, `libssl-dev`/`pkg-config` requirement (DOC-01 point 8)
- `crates/brokerd/Cargo.toml` (grep) — `lettre = "0.11.22"` pin [VERIFIED: crates/brokerd/Cargo.toml:23]
- `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/ROADMAP.md` (full read) — requirement
  text, decisions log, phase dependency chain

### Secondary (MEDIUM confidence)
- None used — this phase required no external web research; every claim is sourced from the repo
  itself.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; existing pins verified in Cargo.toml
- Architecture (composition plan): HIGH for the mechanism (verified against actual source), MEDIUM for
  the "is multi-session-in-one-DB the correct interpretation of 'one causal DAG'" judgment call (Open
  Question #1 — recommend coordinator confirmation before planning tasks around it)
- Pitfalls: HIGH — the `LIMIT 1` landmine and confirm/deny terminality are directly verified in source,
  not inferred
- DOC-01 content: HIGH — all 8 points sourced verbatim from committed Phase 16 artifacts

**Research date:** 2026-07-09
**Valid until:** No expiry concern — this is an internal-codebase research pass, not tracking an
external fast-moving dependency; valid until the next phase's code changes (i.e., effectively for the
life of Phase 17's planning window).
