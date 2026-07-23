# Phase 13: Real Broker-Mediated SMTP Adapter - Research

**Researched:** 2026-07-07
**Domain:** Broker-mediated SMTP send (lettre), kernel network confinement, SQLite transactional idempotency
**Confidence:** HIGH (code-grounded on all six requested areas; two open items flagged below)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-03 (adapter location, worker-never-sends):** The confined worker MUST NEVER perform the SMTP call. The new adapter lives at `crates/brokerd/src/sinks/email_smtp.rs` (broker-resident), the ONLY code path that performs the SMTP call.
- **D-03 refinement (REVERSAL of the design gate's own round-1 mandate):** The confirmed send runs in the confirm-path process (`confirmation.rs::confirm()`, same locus as `file.create`'s `invoke_file_create_from_resolved`), NOT a persistent "broker daemon." The broker is ephemeral/session-scoped — do NOT design or build a daemon.
- **D-04 (secrets custody, restated):** SMTP secrets/credentials MUST NEVER reach the confined worker or any tainted/plan-node/confined context — never in worker env/args, never as a `ValueNode`, never in any plan-node payload. For v1.3 Mailpit (`localhost:1025`, unauthenticated) there is no secret, so custody is trivially satisfied. Real secret custody for live-SES is explicitly deferred, out of scope, MUST NOT be designed/built now.
- **Endpoint sourcing (round-3 tightening):** The SMTP endpoint (host:port) MUST come from trusted local broker config or a hardcoded default — NEVER from the audit DB, a plan node, a `ValueNode`, or `PendingConfirmation` (any block-time-writable field).
- **At-most-once via the EXISTING CAS (SEND-01) — do NOT invent a new idempotency token:** `confirmation.rs::transition_state`'s `UPDATE ... WHERE state='pending'` CAS is the sole authorization gate.
- **One atomic transaction (SEND-01):** The state-transition CAS and the durable `email_send_attempted` Event append MUST commit in a SINGLE atomic SQLite transaction, BEFORE any SMTP connection is opened. Order: (1) atomic {CAS + append `email_send_attempted`}, abort if CAS affects zero rows; (2) perform the send from the frozen snapshot; (3) on success append `email_send_succeeded`, on error append `email_send_failed`.
- **Opaque payloads on ALL THREE send events (round-3 tightening):** `email_send_attempted`/`email_send_succeeded`/`email_send_failed` hashed payloads carry ONLY `effect_id`/opaque metadata — NEVER a resolved literal or raw SMTP response text. Raw detail goes to `logger.error()`/the redactable side table ONLY.
- **No auto-retry (SEND-02):** A confirmed-but-unsent state MUST NOT be auto-retried — recovery is explicit and human-visible.
- **Never swallow (SEND-02):** The v1.2 `Err(_) => Ok(ConfirmedButSinkFailed)` swallow-shape is explicitly REJECTED for `email.send`. Must `logger.error()` with raw context AND append the durable opaque-payload `email_send_failed` event, returning a distinct non-zero result (mirrors `sink_invocation_failed` exit-code discipline).
- **Kernel-enforced negative net (SMTP-01, D-05):** Point at the EXISTING `apply_worker_filter()`/`confine-probe`/`negative_net` mechanism — do NOT design a new confinement primitive.
- **Local capture target (SMTP-03, D-06):** Mailpit (`axllent/mailpit`), not MailHog. Live SES is out of gate scope.
- **Wire-message construction (SMTP-05, D-07/D-22):** Typed builder only (`Message::builder()`'s `.to()/.cc()/.bcc()/.subject()/.body()`), pinned `lettre >= 0.11.22`. Forbidden: `HeaderValue::dangerous_new_pre_encoded`, `format!()`-built headers, the `boring-tls` Cargo feature. Any `lettre` construction `Err` on a confirmed literal MUST become a fail-closed AUDITED abort (never `.unwrap()`/panic, never silent drop, never a raw/`format!`-built fallback). A CRLF fixture verified via Mailpit's HTTP API is a HARD requirement regardless of the library's by-construction defense.
- **Known pre-existing items — do NOT fix in this phase:** `file.create`'s `Err(_) => Ok(ConfirmedButSinkFailed)` swallow-shape stays as-is (grandfathered). `confirm_granted` appended before the CAS (pre-existing race, non-security, not this phase's job).
- **Explicitly OUT of this phase's scope:** content-sensitive body/subject blocking and the collect-then-Block plural reshape (Phase 14); the deterministic doc→action extractor (Phase 15); combined-digest confirm-binding, verbatim block-narration UX, CONTROL-01/02 (Phase 16); live SES/real inbox send (post-milestone, ungated).

### Claude's Discretion

- Whether `crates/brokerd/src/sinks/email_send.rs` (the old stub) is deleted outright or retained — CONTEXT.md does not mandate either; see Open Question 1.
- Exact shape of the `confine-probe` negative-net test extension (reuse `probe_net()` verbatim vs. add a new host:port-aware op) — see Pitfall 5 / Assumption A4.
- Exact Docker/Colima recipe mechanics for the Mailpit sidecar (network mode, port mapping) — CONTEXT.md specifies WHAT (Mailpit sidecar, SMTP :1025, HTTP :8025) but not the precise `docker run`/network wiring.
- New `ConfirmOutcome` variant name and CLI exit code number for the email-send-failure path (CONTEXT.md mandates it be distinct from `ConfirmedButSinkFailed` and from `denied`/`unknown effect_id`, but does not name it).

### Deferred Ideas (OUT OF SCOPE)

- Live SES / real inbox send (`SMTP-04`) — post-milestone, ungated, config-swap only. Do not scaffold credentials plumbing, a daemon, or a secret-custody model for it now.
- Persistent broker daemon + control-socket architecture — explicitly future work IF the SES path is ever taken on; MUST NOT be built in v1.3.
- Attachment support — descoped for v1.3 entirely (Phase 14 removes it from schema/sensitivity sets); this phase's adapter has no attachment code path at all, by design.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-------------------|
| SMTP-01 | Broker-mediated adapter sends email only after executor-authorize + human-confirm; confined worker's direct SMTP connection attempt FAILS under kernel-enforced default-deny net (Linux negative assertion) | Architectural Responsibility Map; Pitfall 5 (exact `confine-probe`/`probe_net` extension mechanics); Security Domain "Confined worker attempting direct network egress" row |
| SMTP-02 | SMTP secrets/credentials live only in the broker process — asserted absent from worker env/args AND any plan-node payload | Environment Availability recipe (`CAPRUN_SMTP_HOST`/`PORT` read directly by the broker/confirm process, never persisted); Validation Architecture SMTP-02 row (grep-based structural assertion) |
| SMTP-03 | Acceptance-gate test targets a local capture SMTP (Mailpit) — Linux-verifiable, repeatable, no live infra dependency | Environment Availability (Mailpit sidecar recipe); Validation Architecture SMTP-03 row |
| SMTP-05 | Adapter constructs the wire message so tainted literals cannot alter envelope/headers (CRLF/header injection); tested with a CRLF fixture verified via Mailpit's HTTP API | Code Examples / Pattern 1-2; Pitfall 2 (exact fallible-call points in `lettre`'s API); Pitfall 4 (Mailpit API field caveat); Validation Architecture SMTP-05 row |
| SEND-01 | Confirm-triggered send is idempotent — re-issued confirm/broker restart/duplicate submission cannot double-fire; audit DAG records exactly ONE send | Summary's primary finding + Pattern 2 (atomic CAS+attempt transaction); Pitfall 1 (`&mut Connection` signature migration); Validation Architecture SEND-01 row |
| SEND-02 | Adapter failure after confirm surfaces the error (never swallowed), records it in the DAG, no silent retry can double-send | Code Examples (Pattern 1's `Err` arm); Anti-Patterns ("raw SMTP error text" rule); Validation Architecture SEND-02 row |
</phase_requirements>

## Summary

This phase replaces `invoke_email_send_stub` with a real SMTP send through `lettre`, dispatched
from the SAME confirm-path process that already invokes `file.create`'s live sink
(`crates/brokerd/src/confirmation.rs::confirm()`, `"email.send"` arm at lines 417-431). The design
is already locked (Phase 12 DESIGN-01 gate, APPROVED) — this research grounds it in the actual
current code so the planner writes tasks against reality, not the design doc's prose alone.

The single most consequential code-level finding: **`rusqlite::Connection::transaction()` requires
`&mut self`**, but `confirm()`'s current signature takes `conn: &rusqlite::Connection` (shared
reference) — used by 6 test call sites and 1 `main.rs` call site. Achieving the design's MUST
("CAS + `email_send_attempted` append in ONE atomic SQLite transaction") means `confirm()`'s
signature must become `&mut rusqlite::Connection`, and — because the *existing* generic Step 6
(`transition_state`) already fires unconditionally, for every sink, BEFORE the sink dispatch match —
the email.send arm cannot reuse that generic call as-is; it must own its own CAS inside its own
transaction, executed as a special-cased branch (file.create's flow is untouched/grandfathered).
This is not a hypothetical risk: a planner following the design doc's prose literally (call the
existing `transition_state`, then separately append `email_send_attempted` in the new
`email_smtp.rs`) would produce TWO separate autocommit statements, not one atomic transaction,
silently failing the SEND-01 MUST while `cargo test` still passes (nothing currently asserts
transaction atomicity structurally). See "Code Path to Modify" below for the exact restructuring.

Second key finding: `lettre 0.11.22` (confirmed current via `crates.io` API and local `cargo search`)
matches the design doc's pin exactly. Its typed builder setters (`.to()/.cc()/.bcc()/.subject()`)
return `Self` (infallible, chainable) — the actual fallible points are `Address::new`/`.parse()`
(building each `Mailbox`, returns `Result<Address, AddressError>`) and the terminal `.body()` call
(returns `Result<Message, Error>`). This refines the design doc's "any builder setter Err" language:
in practice there is no `Result` from `.to()/.cc()/.bcc()/.subject()` themselves — the planner should
build every `Address`/`Mailbox` via `.parse()` first (propagating `Err` there, fail-closed) and treat
`.body()` as the second and only other fallible call.

Third: `SmtpTransport::builder_dangerous(host)` gives the no-TLS/no-auth local transport the design
doc requires; `.port(port)` sets a non-default port (1025 for Mailpit). `lettre`'s default features
include `native-tls`, which is a NEW build dependency for this workspace (grep of `Cargo.lock` found
zero existing `openssl` references) — flagged as a Linux-container build-environment pitfall, not a
reason to deviate from the design's locked "default features only" decision.

**Primary recommendation:** special-case `email.send`'s CAS+attempt inside its own
`conn.transaction()` block (changing `confirm()`'s signature to `&mut Connection`, a small,
enumerable, 7-call-site change), keep `file.create`'s existing flow byte-for-byte unchanged, add a
new `ConfirmOutcome::EmailSendFailed` variant (not `ConfirmedButSinkFailed`) mapped to a new CLI exit
code (7), and reuse `confine-probe`'s existing `probe_net`/`negative_net` pattern for the kernel
negative-net assertion (extending it with a new `smtp`/`connect`-style op rather than inventing a new
mechanism).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| SMTP wire send | Broker (reference monitor, confirm-path process) | — | D-03/D-04: confined worker never holds send capability; broker/confirm process is the trusted, non-tainted locus (`crates/brokerd/src/sinks/email_smtp.rs`, new) |
| Kernel network denial | Sandbox (kernel-enforced boundary) | — | seccomp-bpf `EPERM` on `socket(AF_INET/AF_INET6)` already exists (`crates/sandbox/src/seccomp.rs::apply_worker_filter`); this phase only adds a test that exercises it against the Mailpit target, no new primitive |
| At-most-once send gate | Broker (SQLite CAS + transaction) | — | `confirmation.rs::transition_state`'s `UPDATE ... WHERE state='pending'` CAS, reused; email.send needs an additional atomic wrapper around CAS+attempted-append |
| Audit ledger (attempted/succeeded/failed) | Broker (`audit.rs::append_event`) | — | Opaque-payload discipline already established by `file.create`'s `sink_executed`/`sink_invocation_failed` pattern |
| CLI exit-code discipline | CLI (`cli/caprun/src/main.rs`) | Broker (`ConfirmOutcome`) | Mirrors existing `ConfirmOutcome` → exit-code map (lines 328-339); needs one new arm |
| CRLF/header-injection defense | Library boundary (`lettre` typed builder) | Broker (adapter call-site discipline) | D-07: by-construction defense in `lettre`; broker's job is to never bypass the typed builder with `format!`/`dangerous_new_pre_encoded` |
| Local capture SMTP target | External test infra (Mailpit Docker sidecar) | Broker (client) | Not part of the TCB; reachable only from the broker/confirm process, never the confined worker |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `lettre` | `0.11.22` [VERIFIED: crates.io registry API + `cargo search` — 2026-05-14 publish, matches design doc's `>= 0.11.22` pin exactly] | Typed SMTP message construction + synchronous SMTP transport | Long-established (first published 2015), ~330K weekly downloads, GitHub repo present, `package-legitimacy check` verdict `OK` — no red flags |

**Installation** (`crates/brokerd/Cargo.toml`):
```toml
[dependencies]
lettre = "0.11.22"   # default features only — do NOT add features = [...] enabling boring-tls
```
Default features (`builder`, `hostname`, `native-tls`, `pool`, `smtp-transport`) [CITED: docs.rs/crate/lettre/0.11.22/features] are all that's needed; `builder_dangerous` bypasses TLS at the transport-construction level regardless of the `native-tls` feature being compiled in.

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none new) | — | `rusqlite::Connection::transaction()` is already available via the `rusqlite` workspace dep (`0.32`, `bundled` feature) — no new crate needed for the atomic-transaction requirement | Confirmed via docs.rs: `pub fn transaction(&mut self) -> Result<Transaction<'_>>` [CITED: docs.rs/rusqlite/0.32.1] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `lettre` typed builder | Hand-rolled `format!()` SMTP DATA construction | Forbidden by the design doc (D-07) — reintroduces exactly the CRLF-injection surface this phase exists to close |
| `conn.transaction()` (native rusqlite API) | Raw `conn.execute("BEGIN IMMEDIATE", [])` / `COMMIT` SQL via the existing `&Connection` signature | Avoids the 7-call-site signature change to `&mut Connection`, but loses the compiler-enforced "only one writer at a time" guarantee and Rust's auto-rollback-on-drop safety net. NOT recommended — see Pitfall 1 below; the native API is one line of ceremony (`let mut conn = ...`) cheaper in total than manually re-deriving the transaction-safety rusqlite already gives you for free. |

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `lettre` | crates.io | ~11 years (published 2015-10-21) [VERIFIED: `package-legitimacy check` seam + `cargo search`] | 330,289/week | github.com/lettre/lettre | OK | Approved — no `checkpoint:human-verify` needed |

**Packages removed due to `[SLOP]` verdict:** none.
**Packages flagged as suspicious `[SUS]`:** none.

No other new external packages are introduced by this phase (the `conn.transaction()` API is part of the already-vendored `rusqlite` dependency).

## Architecture Patterns

### System Architecture Diagram

```
Confined Worker (no send capability, ever)
   │  seccomp-bpf denies socket(AF_INET/AF_INET6) → EPERM (existing, apply_worker_filter)
   │  [negative-net test target: this phase adds a Mailpit-host:port connect() attempt here]
   X  (attempt to reach Mailpit directly — MUST fail kernel-side)

Human operator
   │  `caprun confirm <effect_id> [audit-db-path]`
   ▼
cli/caprun/src/main.rs::run_confirm_or_deny
   │  opens SAME persisted audit DB (`open_audit_db`)
   ▼
brokerd::confirmation::confirm(conn: &mut Connection, effect_id, workspace_root)
   │  Steps 1-5 unchanged: lookup → terminal-state check → redaction gate → display → confirm_granted append
   │
   ├─ sink == "file.create" ─────────────────────────────────────────────┐
   │     Step 6 (existing, unchanged): transition_state(conn, Confirmed) │
   │     Step 7: invoke_file_create_from_resolved(...)                  │
   │     (grandfathered swallow-shape: Err(_) => ConfirmedButSinkFailed)│
   └─────────────────────────────────────────────────────────────────────┘
   │
   └─ sink == "email.send" ────────────────────────────────────────────────────┐
         NEW special-cased branch (bypasses the generic Step 6 call):          │
         1. let tx = conn.transaction()?;                                     │
         2. affected = transition_state(&tx, Confirmed)  [reuses existing fn] │
         3. if affected == 0 { drop(tx); return AlreadyTerminal }              │
         4. append_event(&tx, email_send_attempted{effect_id}, granted_hash)  │
         5. tx.commit()?;   ← CAS + attempted are now ONE atomic unit          │
         6. (OUTSIDE the tx) email_smtp::send_via_lettre(resolved_args, ...)   │
              │                                                                │
              ├─ Ok  → append_event(conn, email_send_succeeded{effect_id})    │
              │         return ConfirmOutcome::Released                       │
              └─ Err → append_event(conn, email_send_failed{effect_id, opaque})│
                        return ConfirmOutcome::EmailSendFailed  (NEW variant)  │
   └────────────────────────────────────────────────────────────────────────────┘
                     │
                     ▼
         email_smtp.rs (NEW, crates/brokerd/src/sinks/email_smtp.rs)
           - builds Address/Mailbox per recipient via .parse() (fail-closed on Err)
           - Message::builder().to(...).cc(...).bcc(...).subject(...).body(...)
           - SmtpTransport::builder_dangerous(HOST).port(PORT).build()
           - .send(&message) → Result<Response, lettre::transport::smtp::Error>
                     │
                     ▼
              Mailpit (axllent/mailpit Docker sidecar)
                SMTP :1025 (captures message)  |  HTTP API :8025 (test asserts envelope)
```

### Recommended Project Structure
```
crates/brokerd/src/sinks/
├── email_send.rs     # UNCHANGED file, but no longer confirm()'s dispatch target for email.send
├── email_smtp.rs      # NEW — the real adapter; only code path performing the SMTP call
└── file_create.rs     # UNCHANGED — sibling pattern this phase's adapter borrows shape from
```

### Pattern 1: Frozen-snapshot re-invocation (mirror `invoke_file_create_from_resolved`)
**What:** A `ValueStore`-free function taking `&[ResolvedArg]` directly, never re-resolving or re-deciding.
**When to use:** Any confirm-time sink invocation — this is the ONLY sanctioned pattern (T-10-05, `CON-i2-non-bypassable` — never call `executor::submit_plan_node` from confirm/deny).
**Example** (structural shape, following `crates/brokerd/src/sinks/file_create.rs:175-221`):
```rust
// crates/brokerd/src/sinks/email_smtp.rs — illustrative shape, not literal code to paste.
pub fn invoke_email_smtp_from_resolved(
    conn: &rusqlite::Connection,   // used for the succeeded/failed append AFTER the tx commits
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg], // to/cc/bcc/subject/body literals, frozen at Block time
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    let message = build_message(resolved_args)?;   // Address::parse() + Message::builder(), fail-closed
    let transport = SmtpTransport::builder_dangerous(smtp_host())
        .port(smtp_port())
        .build();
    match transport.send(&message) {
        Ok(_response) => { /* append email_send_succeeded, opaque payload only */ }
        Err(e) => {
            logger_error_with_raw_context(&e);              // raw SMTP response text HERE only
            /* append email_send_failed, opaque payload only */
            return Err(anyhow::Error::new(e).context("email.send SMTP transport failed"));
        }
    }
}
```

### Pattern 2: Atomic CAS + attempt-append (NEW pattern for this codebase — no prior transaction usage exists)
**What:** Wrap `transition_state` and `append_event` in a single `rusqlite::Transaction`.
**When to use:** Only for `email.send`'s Step 6 — `file.create`'s existing Step 6 stays a standalone autocommit call (grandfathered, out of scope to change).
**Example:**
```rust
// Source: rusqlite 0.32.1 docs (docs.rs) — Connection::transaction(&mut self) -> Result<Transaction<'_>>
let tx = conn.transaction()?;                       // conn must be &mut Connection here
let affected = transition_state(&tx, effect_id, PendingConfirmationState::Confirmed)?;
if affected == 0 {
    // tx drops here without .commit() → automatic ROLLBACK (rusqlite default Drop behavior)
    return Ok(ConfirmOutcome::AlreadyTerminal);
}
let attempted_hash = append_event(&tx, &attempted_event, Some(&granted_hash))?;
tx.commit()?;   // CAS + attempted-append now durable together, or neither is
```
`transition_state`/`append_event` both take `&rusqlite::Connection`; passing `&tx` (a `&Transaction<'_>`) works via Rust's `&mut T -> &T`-style deref coercion (`Transaction: Deref<Target = Connection>`) — no signature changes needed to those two helper functions themselves.

### Anti-Patterns to Avoid
- **Reusing the generic Step 6 `transition_state(conn, ...)` call unmodified for email.send:** produces two separate autocommit statements (CAS, then attempted-append) instead of one atomic transaction — silently fails SEND-01's MUST with no test currently asserting the difference. Must special-case the dispatch (see Pattern 2).
- **Building the outgoing message from a `format!()`-assembled string, ever, even for logging/debugging:** defeats the entire D-07 defense; forbidden explicitly by the design doc.
- **Writing the raw SMTP error/response text into any of the three send Events' payloads:** SMTP rejections routinely echo the recipient address — this would staple a confirmed literal into the immutable hash chain (round-3 tightening in the design doc). Raw detail goes to `logger.error()`/the redactable side table ONLY.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SMTP wire-message / header encoding | A `format!()`-based MIME/DATA constructor | `lettre::Message::builder()` typed setters | CRLF/header-injection defense is a call-boundary guarantee of the library, not something a hand-rolled string builder can safely replicate (RUSTSEC-2021-0069 exists precisely because a prior lettre version got this wrong) |
| Atomic multi-statement DB writes | A manual `BEGIN`/`COMMIT` string pair issued via `conn.execute()` | `rusqlite::Connection::transaction()` | The native API gives auto-rollback-on-drop (crash safety) and a compiler-enforced exclusive-borrow — a raw SQL string pair has neither |
| Kernel network denial | A userspace check ("does the worker call an SMTP crate?") | The EXISTING seccomp filter (`apply_worker_filter`) + `confine-probe`/`negative_net` test pattern | Already kernel-enforced and already tested; a code-inspection-only assertion is explicitly rejected by the design doc (D-05) |
| Idempotency token | A new UUID/nonce per confirm attempt | The EXISTING `transition_state` CAS (`WHERE state='pending'`) | Explicitly mandated by the design doc — inventing a new token duplicates a mechanism that already provides exactly-one-winner semantics |

**Key insight:** every piece of new infrastructure this phase seems to need (idempotency, CRLF defense, network denial) already exists in the codebase or in `lettre`'s typed API — this phase's actual engineering content is *wiring* those together correctly (the transaction-atomicity restructuring above), not building anything new from scratch.

## Common Pitfalls

### Pitfall 1: `confirm()`'s `&Connection` → `&mut Connection` signature change is a real, bounded, but easy-to-miss migration
**What goes wrong:** A planner adds `conn.transaction()` inside `confirm()` without noticing the function's current parameter type is `conn: &rusqlite::Connection` — a compile error surfaces immediately, but the FIX (widen to `&mut Connection`) cascades to every call site.
**Why it happens:** `Connection::transaction(&mut self)` requires exclusive access; the current code was written before any transaction was needed.
**How to avoid:** Change `confirm()`'s signature to `conn: &mut rusqlite::Connection`. Exactly 7 call sites need `let conn = ...` → `let mut conn = ...` + `confirm(&conn, ...)` → `confirm(&mut conn, ...)`:
- `cli/caprun/src/main.rs:319`
- `crates/brokerd/src/confirmation.rs:717, 750, 753, 794, 819, 841` (test module)
`deny()` does NOT need this change (no transaction, no email dispatch) — leave its signature as `&rusqlite::Connection`.
**Warning signs:** `cargo build` errors of the shape "cannot borrow `*conn` as mutable, as it is behind a `&` reference" pointing at the new `.transaction()` call.

### Pitfall 2: Treating `.to()/.cc()/.bcc()/.subject()` as fallible when they are not
**What goes wrong:** Code written expecting `Result` from every builder setter (as the design doc's prose "any builder setter Err" loosely implies) either over-handles non-existent errors or, worse, skips validating a CRLF-bearing recipient because the actual fallible call (`Address::new`/`.parse()`) was never reached.
**Why it happens:** `lettre 0.11.22`'s `MessageBuilder::to/cc/bcc/subject` all return `Self` (infallible chaining) [CITED: docs.rs/lettre/0.11.22]; the `Result` surfaces only at `Address::new`/`FromStr::from_str` (building the `Mailbox`) and at the terminal `.body()` call.
**How to avoid:** Parse every recipient literal to `Address`/`Mailbox` FIRST (propagating `Err` fail-closed, per D-07), THEN feed the already-valid `Mailbox` values into `.to()/.cc()/.bcc()`, THEN call `.subject()` (infallible), THEN `.body()` (the second and last fallible call).
**Warning signs:** A test asserting the CRLF fixture is "caught" via a builder-setter Err that never actually returns one at that call.

### Pitfall 3: `lettre`'s default `native-tls` feature is a NEW build dependency for this workspace
**What goes wrong:** `cargo test --workspace` inside the `rust:1` Docker container fails to compile `openssl-sys` (native-tls's transitive dep) because `libssl-dev`/`pkg-config` aren't installed in that base image.
**Why it happens:** Grep of `Cargo.lock` confirms zero prior `openssl` references anywhere in this workspace — this phase is the first to pull in a TLS backend, even though `builder_dangerous` never uses it at runtime.
**How to avoid:** The design doc's "default features only" is a locked decision (do not strip `native-tls`) — instead, extend the Linux verification recipe (CLAUDE.md's `docker run rust:1 cargo test ...`) with `apt-get update && apt-get install -y libssl-dev pkg-config` before the `cargo test` step, OR use a Docker image/layer that already has these (verify empirically before locking the exact recipe change into the plan).
**Warning signs:** A linker/compile error referencing `openssl-sys` or `pkg-config` failures, only reproducible inside the Linux container (macOS `cargo check` for type-checking may still pass if native-tls isn't exercised, but a full `cargo build`/`cargo test` will hit it on any platform once `lettre` is added — verify locally on macOS too, since native-tls also needs OpenSSL there, typically via Homebrew).

### Pitfall 4: Mailpit's REST API's `To`/`Cc`/`Bcc` fields are header-derived, not raw SMTP-envelope `RCPT TO`
**What goes wrong:** A test written expecting a dedicated "envelope"/`RcptTo` field in Mailpit's `GET /api/v1/message/{ID}` response won't find one — the design doc's phrase "the captured message's actual To/Cc/Bcc envelope" is loosely worded relative to Mailpit's actual schema.
**Why it happens:** [CITED: mailpit swagger schema, WebFetch-summarized — MEDIUM confidence, not independently re-verified against a live Mailpit instance this session] Mailpit's `Message.To/Cc/Bcc` fields are parsed FROM the message's MIME headers, not from the raw SMTP `RCPT TO` commands the server received.
**How to avoid:** This is actually the RIGHT field to assert on for the CRLF fixture's purpose — the attack being tested is "does a CRLF-then-`Bcc:` sequence in the body get parsed as a real header by the receiving MTA," and Mailpit's header-parsed `To/Cc/Bcc` is exactly what would show a smuggled recipient if the injection succeeded. The planner should still empirically confirm the exact field names/shape against a running Mailpit instance during implementation (`curl http://localhost:8025/api/v1/message/<id> | jq`) rather than trusting this research's schema description as final.
**Warning signs:** Test asserting on a field name (`RcptTo`, `Envelope`) that doesn't exist in the actual JSON response.

### Pitfall 5: The confined-worker negative-net test target needs a NEW `confine-probe` op, not literal reuse of `probe_net`
**What goes wrong:** `probe_net()` (`crates/sandbox/src/bin/confine-probe.rs:132-156`) only calls `socket(AF_INET, SOCK_STREAM, 0)` — it never calls `connect()` to any host:port, so it cannot, as written, "attempt an actual connect() to the Mailpit host:port" as the CONTEXT.md's phrasing requests.
**Why it happens:** The existing test proves the syscall-level denial (sufficient for seccomp, since `socket()` fails before `connect()` could ever be attempted) but doesn't take a host:port argument.
**How to avoid:** Since `socket(AF_INET, ...)` is denied at `EPERM` BEFORE any `connect()` could occur, the functional assertion is identical whether or not a host:port is threaded through. Two options for the planner: (a) reuse `probe_net()` verbatim and cite it as already covering the Mailpit case (destination-independent — the design doc's own text supports this: "denies the underlying `socket()` syscall regardless of destination port"), or (b) add a new `confine-probe smtp <host> <port>` op that calls `socket()` then, only if that unexpectedly succeeds, attempts `connect()` to the given host:port — for defense-in-depth against a hypothetical future loosening of the seccomp filter to allow `socket()` but not `connect()`. Recommendation: (b) is slightly more faithful to "attempts an actual connect()" as literally requested, and is a small addition (~15 lines mirroring `probe_net`'s shape) — but (a) is also a legitimate, cheaper reading of the same requirement. Flag this choice explicitly for the planner/discuss step rather than silently picking one.
**Warning signs:** A test title claiming to test "connect() to Mailpit" that never actually references Mailpit's host:port anywhere in its assertion.

### Pitfall 6: `email_send.rs`'s existing stub and its unit test remain in the tree but become dead code for the confirm path
**What goes wrong:** `invoke_email_send_stub` (`crates/brokerd/src/sinks/email_send.rs:38-55`) is currently `confirm()`'s dispatch target for `"email.send"` (lines 417-431 of `confirmation.rs`). Once the new `email_smtp.rs` adapter takes over that dispatch, the stub function becomes unreferenced by `confirm()` but its own unit test (`invoke_email_send_stub_records_email_send_stub_event`) still compiles and passes, silently masking the fact that the stub is no longer exercised by the real flow.
**Why it happens:** Nothing forces removal of dead code; `cargo test` doesn't flag "this function's only caller was just deleted."
**How to avoid:** The planner should explicitly decide: delete `email_send.rs` entirely (cleanest, since CONTEXT.md's phase boundary says the new module "replac[es] `invoke_email_send_stub`"), or keep it only if some other test/doc still needs the pure "no-op stub" behavior — either way, make the decision explicit in a task rather than leaving both paths compiling silently.
**Warning signs:** `cargo build` produces a `dead_code` / `unused` warning after the confirm-path rewire (or doesn't, if the module is still referenced only by its own test module, which won't trigger the lint).

## Code Examples

### Confirm-path dispatch, current state (to be modified)
```rust
// Source: crates/brokerd/src/confirmation.rs:400-436 (current)
match pc.sink.0.as_str() {
    "file.create" => match crate::sinks::file_create::invoke_file_create_from_resolved(
        conn, pc.session_id, pc.effect_id, &pc.resolved_args, workspace_root,
        granted_event_id, &granted_hash,
    ) {
        Ok(_) => Ok(ConfirmOutcome::Released),
        Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),   // grandfathered swallow-shape
    },
    "email.send" => {
        // stub — replace this whole arm's body, see Pattern 2 above
        let plan_node = runtime_core::PlanNode { sink: pc.sink.clone(), args: vec![] };
        crate::sinks::email_send::invoke_email_send_stub(conn, pc.session_id, &plan_node, Some(&granted_hash))?;
        Ok(ConfirmOutcome::Released)
    }
    other => Err(anyhow::anyhow!("confirm: unreachable sink `{other}` — not a registered v1.2 sink")),
}
```
Note Step 6 (`transition_state`) currently runs ONCE, generically, at line 395 — BEFORE this match. The email.send arm above must stop relying on that call and perform its own CAS inside its own transaction (Pattern 2); file.create's arm keeps using the pre-computed `affected`/Step-6 result exactly as today.

### `ResolvedArg` shape the new adapter consumes
```rust
// Source: crates/brokerd/src/confirmation.rs:37-49 (existing, unmodified by this phase)
pub struct ResolvedArg {
    pub name: String,                                  // "to" | "cc" | "bcc" | "subject" | "body"
    pub value_id: runtime_core::plan_node::ValueId,
    pub literal: String,                                // the frozen literal — feed to Address::parse()/body()
    pub taint: Vec<runtime_core::plan_node::TaintLabel>,
    pub provenance_chain: Vec<uuid::Uuid>,
}
```
`PendingConfirmation.resolved_args: Vec<ResolvedArg>` is the FULL arg set (not just the blocked one) — `email_smtp.rs` should look up `to`/`cc`/`bcc`/`subject`/`body` by `.name` (mirroring `file_create.rs`'s `resolved_literal` helper, lines 131-137), tolerating absent optional args (`cc`/`bcc` are schema-optional today per `crates/executor/src/sink_schema.rs`).

### `append_event`'s Defect-B guard does not block the new event types
```rust
// Source: crates/brokerd/src/audit.rs:201-208 (existing, unmodified)
if event.event_type == "sink_blocked" && event.anchor.is_none() {
    return Err(anyhow::anyhow!("sink_blocked event requires an anchor (Defect B guard)"));
}
```
`email_send_attempted`/`_succeeded`/`_failed` are NOT `"sink_blocked"` events, so this guard is a no-op for them — `Event::new(...)` (which always sets `anchor: None`) is the correct constructor, exactly as `file_create.rs`'s `sink_executed`/`sink_invocation_failed` events already use it (`crates/brokerd/src/sinks/file_create.rs:190-196, 207-214`).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| MailHog as the local capture SMTP standard | Mailpit (`axllent/mailpit`) | MailHog unmaintained since ~2020 [CITED: DESIGN doc D-06, corroborated by general community knowledge] | Design doc already locks Mailpit — no action needed, just confirms the choice is current, not stale |
| `lettre` `0.10.x` boring-tls feature | Avoid `boring-tls` entirely; use default (`native-tls`) or `rustls` | RUSTSEC-2026-0141 published 2026-05-14 [CITED: rustsec.org/advisories/RUSTSEC-2026-0141] | Directly informs the Cargo.toml feature choice — confirms the design doc's forbid-list is current, not based on a stale advisory |

**Deprecated/outdated:** none directly relevant beyond the above two, already reflected in the locked design.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Mailpit's `To`/`Cc`/`Bcc` API fields are header-derived (not a separate SMTP-envelope `RcptTo` field) | Pitfall 4 | If wrong, the CRLF fixture's assertion needs a different field name — low risk, easily discovered empirically the first time the planner runs `curl` against a live Mailpit instance; does not change the test's overall design |
| A2 | A Docker user-defined network + container-name DNS (e.g. `mailpit` as hostname) is the cleanest way to make the `rust:1` test container reach the Mailpit sidecar under Colima | Environment Availability / Mailpit sidecar recipe below | If Colima's networking model behaves differently than plain Docker Desktop, the recipe may need `--network host` or explicit port-forward flags instead — should be smoke-tested once before locking into the plan's verification steps |
| A3 | `libssl-dev`/`pkg-config` are not preinstalled in the `rust:1` Docker image, requiring an explicit `apt-get install` step once `lettre`'s default `native-tls` feature is added | Pitfall 3 | If the image already has these (some `rust:*` tags bundle more build tooling than others), the extra install step is harmless but adds ~10-20s to the verification loop; if missing and NOT added, the Linux verification build fails outright |
| A4 | Extending `confine-probe` with a new host:port-aware op (vs. reusing `probe_net()` verbatim) is the more faithful implementation of "attempts an actual connect() to the Mailpit host:port" | Pitfall 5 | Low risk either way — both readings satisfy the underlying security claim (kernel-enforced `EPERM` at `socket()`); this is a documentation/naming-fidelity question, not a security gap, and is explicitly flagged for planner/discuss-phase judgment rather than presented as settled |

**If this table is empty:** N/A — see above; all four assumptions are low-blast-radius and easily falsified during implementation.

## Open Questions

1. **Should `crates/brokerd/src/sinks/email_send.rs` (the old stub) be deleted or kept?**
   - What we know: CONTEXT.md's phase boundary describes the new adapter as "replacing `invoke_email_send_stub`... as the `"email.send"` dispatch target" — implying the stub is superseded, not necessarily deleted.
   - What's unclear: whether any other code (tests, docs, a future phase) still references the stub directly.
   - Recommendation: grep for `invoke_email_send_stub` call sites beyond `confirmation.rs` and `email_send.rs`'s own test module before deciding; if none, delete the file as a cleanup task (see Pitfall 6).

2. **Exact Mailpit HTTP API response schema for the CRLF fixture assertion.**
   - What we know: `GET /api/v1/message/{ID}` returns a `Message` object with `To`/`Cc`/`Bcc` array-of-`{Name, Address}` fields (per swagger schema, WebFetch-summarized, not independently re-verified against a running instance this session).
   - What's unclear: exact casing/nesting of field names in the live JSON (schema summaries can drift from an actual running version).
   - Recommendation: the planner's first CRLF-fixture task should include an exploratory step — start Mailpit, send one known-good test message via `email_smtp.rs`, and `curl`/`jq` the response to confirm field names BEFORE writing the assertion, rather than trusting this document's schema description as final.

3. **Whether the Linux verification Docker recipe (CLAUDE.md) needs a persistent update or a one-off addition for this phase only.**
   - What we know: the recipe currently runs a single `rust:1` container with no sidecar; this phase needs Mailpit reachable from that container.
   - What's unclear: whether later phases (14-17) will also need Mailpit reachable, making a persistent recipe update (e.g., a `docker-compose.yml` under a new `dev/` or `scripts/` location) worth the investment now vs. deferring to whichever phase first needs it repeatedly.
   - Recommendation: given Phase 17's live acceptance run will also need Mailpit, introduce a reusable `scripts/`-level helper (or documented `docker network create` + two `docker run` commands, per the Environment Availability section below) now rather than one-off CLI incantations, but keep it minimal — a shell script, not a full compose file, matches this project's existing "no docker-compose file in the repo" convention.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker (via Colima) | Mailpit sidecar + Linux test container | ✓ (per CLAUDE.md's existing recipe; not independently re-probed on this research pass — dev machine is the same Mac the recipe already targets) | — | — |
| `axllent/mailpit` Docker image | SMTP-03 acceptance target | Not yet pulled (new dependency this phase introduces) | `latest` (design doc does not pin a tag) | None — SMTP-03 requires a local capture SMTP; Mailpit is itself the design-mandated choice, no fallback |
| `libssl-dev` / `pkg-config` inside `rust:1` | Compiling `lettre`'s default `native-tls` feature | Unknown — not pre-verified this session (Pitfall 3) | — | `apt-get install -y libssl-dev pkg-config` as a recipe addition if missing |

**Recommended Mailpit sidecar recipe** (extends CLAUDE.md's existing Colima+Docker verification flow — ASSUMED/A2, smoke-test once before locking into the plan):
```bash
docker network create caprun-test-net 2>/dev/null || true
docker run -d --rm --name mailpit --network caprun-test-net \
  -p 8025:8025 -p 1025:1025 axllent/mailpit
docker run --rm \
  --security-opt seccomp=unconfined \
  --network caprun-test-net \
  -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt \
  -e CAPRUN_SMTP_HOST=mailpit -e CAPRUN_SMTP_PORT=1025 \
  rust:1 \
  bash -c "apt-get update && apt-get install -y libssl-dev pkg-config && cargo test --workspace --no-fail-fast"
docker stop mailpit
```
The `CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` env vars are read directly by the broker/confirm process (never stored in the DB/plan-node — satisfies the design doc's "trusted local broker config" endpoint-sourcing rule), defaulting to `127.0.0.1:1025` when unset (for tests run outside the Docker network, e.g. a developer with a locally-running Mailpit).

**Missing dependencies with no fallback:** none blocking — Mailpit itself has no fallback, but it is a new, freely-available Docker image (no infra cost).
**Missing dependencies with fallback:** `libssl-dev`/`pkg-config` — one `apt-get install` line if the base image lacks them.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust's built-in test harness), workspace-wide via `cargo test --workspace --no-fail-fast` |
| Config file | none — no `.cargo/config.toml` test config; Linux-only gating is via `#[cfg(target_os = "linux")]` per-test, established convention (`crates/sandbox/tests/confinement_integration.rs`) |
| Quick run command | `cargo test -p brokerd` (fast, in-memory SQLite, no Docker needed for the transaction/CAS/event-shape unit tests) |
| Full suite command | Colima+Docker recipe above (`cargo test --workspace --no-fail-fast` inside `rust:1`, WITH the Mailpit sidecar for the negative-net and CRLF-fixture tests) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SMTP-01 | Confined worker `connect()`/`socket()` to Mailpit host:port fails under kernel confinement | integration (Linux-only) | `cargo test -p sandbox negative_net` (existing) or a new `confine-probe smtp` op — see Pitfall 5 | ✅ existing (`probe_net`) / ❌ new op if planner chooses option (b), Wave 0 |
| SMTP-02 | SMTP host/port/auth never appear in worker env/args/plan-node payload | unit + grep-based structural assertion | `cargo test -p brokerd` + a `check-invariants.sh`-style grep for `CAPRUN_SMTP_` tokens absent from worker-spawn code paths | ❌ new grep assertion, Wave 0 |
| SMTP-03 | Confirmed effect results in a real email captured by Mailpit | integration (Linux+Docker, requires sidecar) | new `crates/brokerd/tests/email_smtp_acceptance.rs` | ❌ new file, Wave 0 |
| SMTP-05 | CRLF-in-body fixture cannot smuggle a recipient | integration (Linux+Docker, requires sidecar + Mailpit HTTP API assertion) | same new test file as SMTP-03, or a dedicated `crlf_injection.rs` | ❌ new file, Wave 0 |
| SEND-01 | Re-issued confirm / duplicate submission cannot double-fire | unit (in-memory SQLite, no Docker needed — the CAS logic is DB-only) | `cargo test -p brokerd` — mirrors existing `confirm_twice_returns_already_terminal_and_creates_no_new_file` pattern (`confirmation.rs:739-764`) applied to email.send | ❌ new test, Wave 0 |
| SEND-02 | Adapter failure after confirm surfaces non-swallowed, DAG-recorded, non-retryable | unit (mock/force a `send()` failure — e.g. point at a closed port) | `cargo test -p brokerd` | ❌ new test, Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p brokerd` (fast, no Docker)
- **Per wave merge:** the full Colima+Docker recipe including the Mailpit sidecar
- **Phase gate:** full suite green (including SMTP-01/03/05's Linux-only tests) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `crates/brokerd/src/sinks/email_smtp.rs` — the adapter itself (no tests can exist against it until this exists)
- [ ] `crates/brokerd/tests/email_smtp_acceptance.rs` (or equivalent) — covers SMTP-03/SMTP-05
- [ ] A `confine-probe` decision (Pitfall 5) — either confirm `probe_net` already covers SMTP-01 or add a new op
- [ ] Mailpit sidecar wiring in the Colima+Docker recipe (Environment Availability section) — needed before any Linux-only test in this phase can run for real
- [ ] `libssl-dev`/`pkg-config` availability check inside `rust:1` (Pitfall 3) — one-time verification

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | no | Mailpit target is unauthenticated by design (D-04) — no auth surface in this phase |
| V3 Session Management | no | Unrelated to this phase's scope |
| V4 Access Control | yes | The executor's I2 Block/confirm gate is the access-control mechanism; unchanged by this phase (Phase 14's job), but this phase's adapter must not create a bypass path around it (`CON-i2-non-bypassable`) |
| V5 Input Validation | yes | `lettre`'s typed builder + `Address::new`/`.parse()` allow-list grammar (rejects CR/LF at parse time) — never hand-rolled string validation |
| V6 Cryptography | no | No TLS in scope for the Mailpit gate (D-04: no secret to custody); SHA-256 audit-chain hashing is pre-existing, unchanged |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| SMTP header/CRLF injection (recipient smuggling via a tainted body/subject) | Tampering | `lettre`'s typed `Message::builder()` — `Address::new`/`HeaderValueEncoder::allowed_char` reject CR/LF at construction time; body is written after the RFC 5322 header/body separator, structurally inert |
| Confirmed-send replay / double-fire | Tampering, Repudiation (partial) | The existing `transition_state` CAS (`WHERE state='pending'`) — reused, not reinvented, wrapped in one atomic transaction with the attempt-append for email.send |
| Confined worker attempting direct network egress | Elevation of Privilege | seccomp-bpf `EPERM` on `socket(AF_INET/AF_INET6)` — pre-existing, kernel-enforced, this phase adds a test exercising it against the real Mailpit target |
| Raw literal/SMTP-response leakage into the immutable audit chain | Information Disclosure | Opaque-payload discipline on all three send Events (`effect_id` only); raw detail routed to `logger.error()`/the redactable `blocked_literals`-style side table only |

## Sources

### Primary (HIGH confidence)
- `crates.io` registry API (`https://crates.io/api/v1/crates/lettre`) — confirmed `lettre` latest version `0.11.22`, published 2026-05-14
- Local `cargo search lettre` — cross-verified the same version against the actual Cargo registry index available to this machine
- `gsd-tools query package-legitimacy check --ecosystem crates lettre` — verdict `OK`, signals: 2015 first-publish, 330K/week downloads, GitHub repo present
- Direct reads of `crates/brokerd/src/confirmation.rs`, `crates/brokerd/src/sinks/email_send.rs`, `crates/brokerd/src/sinks/file_create.rs`, `crates/brokerd/src/audit.rs`, `crates/brokerd/Cargo.toml`, `crates/sandbox/src/seccomp.rs`, `crates/sandbox/src/bin/confine-probe.rs`, `crates/sandbox/tests/confinement_integration.rs`, `cli/caprun/src/main.rs`, `crates/executor/src/sink_sensitivity.rs`
- `planning-docs/DESIGN-content-adapter-mediation.md` and `.planning/phases/13-real-broker-mediated-smtp-adapter/13-CONTEXT.md` — the locked, adversarially-reviewed design contract for this phase

### Secondary (MEDIUM confidence)
- rustsec.org advisory `RUSTSEC-2026-0141` (WebFetch) — corroborates the design doc's `boring-tls` prohibition is current, not stale
- docs.rs pages for `lettre::message::MessageBuilder`, `lettre::address::Address`, `lettre::transport::smtp::SmtpTransport`/`SmtpTransportBuilder`, `rusqlite::Connection` (WebFetch-summarized — API shapes cross-checked against training-data recollection of `lettre`'s public API and found consistent)

### Tertiary (LOW confidence)
- Mailpit HTTP API v1 schema (`To`/`Cc`/`Bcc` field shape) — WebFetch-summarized from a swagger/OpenAPI spec, NOT independently re-verified against a live running Mailpit instance this session (see Assumption A1, Open Question 2 — planner should verify empirically before finalizing the CRLF-fixture assertion)
- Docker networking recipe for the Mailpit sidecar under Colima (Assumption A2) — general Docker/Colima knowledge, not verified against this project's actual Colima setup this session

## Metadata

**Confidence breakdown:**
- Standard stack (lettre version/features): HIGH — cross-verified via crates.io API, local `cargo search`, and `package-legitimacy check` seam
- Architecture / code-path-to-modify: HIGH — every claim traces to a direct file read of the current codebase this session, with exact line numbers
- Pitfalls: HIGH for Pitfalls 1-3, 5-6 (code-grounded); MEDIUM for Pitfall 4 (Mailpit schema, WebFetch-summarized, not live-verified)
- Testing/Validation Architecture: HIGH — existing test conventions directly observed (`s9_acceptance.rs`, `phase5_dispatch.rs`, `confinement_integration.rs`)

**Research date:** 2026-07-07
**Valid until:** 2026-08-06 (30 days — stable domain; re-verify `lettre` version and the Mailpit schema empirically if this research is reused after that window, since `lettre` releases roughly monthly)
