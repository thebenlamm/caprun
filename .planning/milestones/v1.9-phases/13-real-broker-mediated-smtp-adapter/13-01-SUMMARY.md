---
phase: 13-real-broker-mediated-smtp-adapter
plan: 01
subsystem: infra
tags: [lettre, smtp, email, crlf-injection, rusqlite, audit-dag]

# Dependency graph
requires:
  - phase: 12-design-01-gate
    provides: "Approved DESIGN-content-adapter-mediation.md (D-03/D-04/D-07/D-22 locked decisions)"
provides:
  - "crates/brokerd/src/sinks/email_smtp.rs — the ONLY code path in the TCB that performs an SMTP call"
  - "invoke_email_smtp_from_resolved(conn, session_id, effect_id, resolved_args, parent_id, parent_hash) -> Result<(Uuid, String)>"
  - "build_message(resolved_args) -> Result<lettre::Message> — fail-closed CRLF/header-injection defense"
  - "smtp_host()/smtp_port()/smtp_from() trusted-local-config readers (CAPRUN_SMTP_HOST/PORT/FROM env, sensible defaults)"
  - "scripts/check-email-smtp-construction.sh — structural grep gate proving no raw pre-encoded-header constructor is used"
affects: ["13-02-wire-confirm-dispatch", "13-04-crlf-fixture-acceptance-test"]

# Tech tracking
tech-stack:
  added: ["lettre 0.11.22 (default features only)"]
  patterns:
    - "Frozen-snapshot re-invocation (mirrors invoke_file_create_from_resolved): a ValueStore-free fn taking &[ResolvedArg] directly"
    - "Fail-closed parse-before-build: every recipient literal parsed via lettre::Address FIRST, only valid Mailbox values reach Message::builder()"
    - "Shared record_send_failed() helper: build_message errors and transport errors both route through one opaque-payload audited-abort path"

key-files:
  created:
    - crates/brokerd/src/sinks/email_smtp.rs
    - scripts/check-email-smtp-construction.sh
  modified:
    - crates/brokerd/Cargo.toml
    - crates/brokerd/src/sinks.rs

key-decisions:
  - "Added smtp_from() reading CAPRUN_SMTP_FROM (default caprun@localhost), sourced identically to smtp_host()/smtp_port() as trusted local config — not in the original plan text, required because lettre's MessageBuilder::build() returns Err(MissingFrom) without a From header and the email.send sink schema has no `from` arg (Rule 3 auto-fix, see Deviations)."
  - "Recipient parsing uses lettre::Address (not lettre::message::Mailbox's own FromStr) per the plan's explicit instruction — Address is the exact mechanism the design doc cites for the CRLF-rejecting allow-list grammar; Mailbox::from(Address) wraps the already-valid value for the builder."
  - "record_send_failed() is a shared helper for both build_message's fail-closed abort and a real transport error — both are audited-abort paths per D-07's refinement (any lettre construction Err on a confirmed literal MUST become a fail-closed AUDITED abort, same as a transport error)."

patterns-established:
  - "Pattern: TDD RED/GREEN per sub-behavior within a single task — build_message and invoke_email_smtp_from_resolved each got their own failing-test-first commit before implementation, rather than one commit per task file."

requirements-completed: [SMTP-05]

coverage:
  - id: D1
    description: "build_message constructs the outgoing message exclusively through lettre's typed Message::builder() setters, parsing every recipient via Address FIRST — a CRLF-bearing recipient fails closed at parse time, never reaching the wire"
    requirement: "SMTP-05"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/email_smtp.rs#tests::build_message_rejects_crlf_in_to_fail_closed"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/sinks/email_smtp.rs#tests::build_message_ok_for_clean_single_recipient"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/sinks/email_smtp.rs#tests::build_message_tolerates_absent_cc_bcc"
        status: pass
    human_judgment: false
  - id: D2
    description: "invoke_email_smtp_from_resolved never swallows a transport failure — it logs raw error context, appends an opaque-payload email_send_failed event, and returns a distinct Err"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/email_smtp.rs#tests::invoke_email_smtp_from_resolved_transport_failure_records_email_send_failed"
        status: pass
    human_judgment: false
  - id: D3
    description: "Structural CRLF-defense grep gate proves email_smtp.rs never uses lettre's raw pre-encoded-header constructor and does use the typed Message::builder"
    verification:
      - kind: other
        ref: "bash scripts/check-email-smtp-construction.sh"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-07-07
status: complete
---

# Phase 13 Plan 01: Real Broker-Mediated SMTP Adapter Summary

**Broker-resident `email_smtp.rs` adapter sending via lettre 0.11.22's typed `Message::builder()`, with fail-closed Address-parse CRLF rejection and opaque-payload send-lifecycle events — not yet wired into `confirm()` (Plan 02's job).**

## Performance

- **Duration:** 55 min
- **Started:** 2026-07-07T (session start)
- **Completed:** 2026-07-07
- **Tasks:** 3
- **Files modified:** 4 (2 created, 2 modified)

## Accomplishments

- Added `lettre = "0.11.22"` (default features only, no `boring-tls`) to `crates/brokerd/Cargo.toml`; confirmed resolved in `Cargo.lock`.
- Built `crates/brokerd/src/sinks/email_smtp.rs`: `build_message()` parses every recipient literal (`to`/`cc`/`bcc`) via `lettre::Address` FIRST — a CR/LF byte anywhere in the literal fails closed at parse time, before any `Message::builder()` setter runs. `cc`/`bcc` are schema-optional and simply omitted when absent.
- Implemented `invoke_email_smtp_from_resolved()`: builds the message, sends via `SmtpTransport::builder_dangerous(smtp_host()).port(smtp_port())` (no TLS, no auth — the Mailpit-shaped local target), appends an opaque-payload `email_send_succeeded` event on success, and on ANY failure (build_message's fail-closed abort or a real transport error) routes raw error text to `eprintln!("[brokerd] ...")` (never the hash chain) and appends an opaque-payload `email_send_failed` event before propagating a distinct, non-swallowed `Err`.
- Added `scripts/check-email-smtp-construction.sh`, a grep gate mirroring `check-invariants.sh`'s style: fails if the raw pre-encoded-header constructor token appears on any non-comment line of `email_smtp.rs`, and fails if `Message::builder` is absent. Verified empirically against a real forbidden-token insertion (fails) and comment-only mention (passes).

## Task Commits

Each task was committed atomically (TDD RED/GREEN pairs per sub-behavior):

1. **Task 1: Add lettre dependency and email_smtp module skeleton with build_message**
   - `3e3c42c` (test) — failing test for `build_message` CRLF rejection against a naive stub
   - `c6b56dc` (feat) — real `build_message` implementation, all 3 unit tests pass
2. **Task 2: Implement invoke_email_smtp_from_resolved (send + opaque success/failure events)**
   - `80aa562` (test) — failing test for transport-failure handling against an always-Ok stub
   - `66090ca` (feat) — real `invoke_email_smtp_from_resolved` + `record_send_failed` implementation, test passes
3. **Task 3: Structural CRLF-defense grep gate**
   - `486da33` (chore) — `scripts/check-email-smtp-construction.sh`, verified against a real forbidden-token insertion

_TDD tasks had two commits each (test → feat); Task 3 had no `<behavior>` block and was a single structural chore commit._

## Files Created/Modified

- `crates/brokerd/src/sinks/email_smtp.rs` — new adapter module: `build_message`, `parse_recipient`, `invoke_email_smtp_from_resolved`, `record_send_failed`, `smtp_host`/`smtp_port`/`smtp_from` config readers, and their unit tests.
- `crates/brokerd/Cargo.toml` — added `lettre = "0.11.22"` (default features only).
- `crates/brokerd/src/sinks.rs` — added `pub mod email_smtp;`.
- `scripts/check-email-smtp-construction.sh` — new structural grep gate (executable).

## Decisions Made

- **Added `smtp_from()`** reading `CAPRUN_SMTP_FROM` (default `caprun@localhost`), sourced the same trusted-local-config way as `smtp_host()`/`smtp_port()` — required by a real gap in the plan (see Deviations below).
- **Recipient parsing goes through `lettre::Address`**, not `lettre::message::Mailbox`'s own `FromStr` — this is the exact mechanism `planning-docs/DESIGN-content-adapter-mediation.md` cites for the CRLF-rejecting allow-list grammar (`Address::new`'s `is_atext`/`is_qtext_char`/`is_wsp` grammar excludes bytes 10/13 in any branch), per the plan's explicit instruction. `Mailbox::from(address)` wraps the already-valid value for the builder.
- **`record_send_failed()` is shared** between `build_message`'s fail-closed abort and a real SMTP transport error — both are audited-abort paths per D-07's refinement ("Any `lettre` construction `Err` ... on a literal the human already confirmed MUST become a fail-closed, AUDITED abort" — the same discipline as a genuine send failure).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `smtp_from()` trusted-config reader — lettre requires a `From` header the sink schema doesn't provide**
- **Found during:** Task 1 (writing `build_message`'s GREEN implementation)
- **Issue:** `lettre::message::MessageBuilder::build()` (invoked internally by `.body()`) returns `Err(EmailError::MissingFrom)` if no `.from(...)` call was made. The `email.send` sink schema (`crates/executor/src/sink_schema.rs`, read-only in this phase) only allows `to`/`cc`/`bcc`/`subject`/`body`/`attachment` — there is no `from` arg anywhere in the plan, CONTEXT.md, or RESEARCH.md. Without a From address, `build_message` would fail closed on EVERY call, not just CRLF-bearing ones — a correctness-blocking gap, not a security one.
- **Fix:** Added `smtp_from()`, reading `CAPRUN_SMTP_FROM` env (default `caprun@localhost`), sourced identically to `smtp_host()`/`smtp_port()` — trusted local broker config, per D-04's endpoint-sourcing rule, NEVER a resolved literal from `resolved_args`. Documented the rationale in the module doc comment so Plan 02/04 authors don't rediscover this gap.
- **Files modified:** `crates/brokerd/src/sinks/email_smtp.rs`
- **Verification:** `build_message_ok_for_clean_single_recipient` and `build_message_tolerates_absent_cc_bcc` both build a real `lettre::Message` successfully (would otherwise fail with `MissingFrom` on every call).
- **Committed in:** `c6b56dc` (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for `build_message` to produce a valid RFC 5322 message at all — without it, the adapter could never succeed, only ever fail closed. No architectural change (no schema/executor edit), no scope creep — the From address is broker-owned trusted config exactly like the already-planned `smtp_host()`/`smtp_port()`.

## Issues Encountered

None beyond the deviation above. Verified lettre 0.11.22's actual API against the downloaded crate source (`~/.cargo/registry/src/.../lettre-0.11.22`) before writing code — confirmed `Address::from_str`, `Mailbox::from(Address)`, `MessageBuilder::to/cc/bcc/subject` (all infallible `Self`), `.body()` (fallible, requires `Body::new(String)` via `IntoBody`), and `SmtpTransport::builder_dangerous(host).port(port).build()` match the RESEARCH.md's documented API shapes exactly.

## User Setup Required

None — no external service configuration required. This plan does not require a running Mailpit instance; both new unit tests run fully in-process (in-memory SQLite, a self-closed ephemeral TCP port for the failure test). The full Mailpit-backed CRLF fixture and negative-net Linux tests are Plan 03/04's job.

## Next Phase Readiness

- `email_smtp.rs` is ready for Plan 02 to wire `invoke_email_smtp_from_resolved` into `confirmation.rs::confirm()`'s special-cased atomic CAS + `email_send_attempted` transaction — this plan deliberately does NOT touch `confirmation.rs` or `email_send.rs` (Plan 02's scope).
- `cargo build --workspace` and `cargo test --workspace --no-fail-fast` both pass on macOS (Linux-only sandbox tests show their expected "0 passed" stub behavior, per CLAUDE.md). `scripts/check-invariants.sh` and the new `scripts/check-email-smtp-construction.sh` both pass.
- No blockers. Plan 02 should be aware of the new `smtp_from()` reader when it decides whether/how to surface `CAPRUN_SMTP_FROM` in the Colima+Docker verification recipe (alongside the existing `CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` env vars) for Plan 04's live Mailpit acceptance test.

---
*Phase: 13-real-broker-mediated-smtp-adapter*
*Completed: 2026-07-07*

## Self-Check: PASSED

- FOUND: crates/brokerd/src/sinks/email_smtp.rs
- FOUND: scripts/check-email-smtp-construction.sh
- FOUND: commit 3e3c42c
- FOUND: commit c6b56dc
- FOUND: commit 80aa562
- FOUND: commit 66090ca
- FOUND: commit 486da33
