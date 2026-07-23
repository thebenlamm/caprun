---
phase: 13-real-broker-mediated-smtp-adapter
verified: 2026-07-07T21:15:00Z
status: passed
score: 5/5 must-haves verified (ROADMAP success criteria); 6/6 requirement IDs accounted for
behavior_unverified: 0
overrides_applied: 0
---

# Phase 13: Real Broker-Mediated SMTP Adapter Verification Report

**Phase Goal:** caprun can actually send an email through a broker-mediated adapter — the confined worker never touches the network, SMTP secrets never leave the broker process, and the confirm-triggered send is idempotent and failure-safe.
**Verified:** 2026-07-07T21:15:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Method

This verification did NOT rely on SUMMARY.md self-reports. All claims were re-derived from source, and the Linux-only claims (which cannot run on this macOS dev machine per `cargo test`) were **independently re-executed** on real Linux via Colima+Docker (`bash scripts/mailpit-verify.sh`), rather than trusting the executor's prior reported run. Fresh output is quoted below.

## Goal Achievement — ROADMAP Success Criteria

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A confirmed effect results in a real email arriving at a local capture SMTP (Mailpit), sent by the broker process — acceptance-gate test target | ✓ VERIFIED | Independently re-ran `bash scripts/mailpit-verify.sh` on real Linux (Colima). `test smtp_03_confirmed_send_captured_by_mailpit ... ok`. Test drives the send through `confirmation::confirm()` (the real `caprun confirm` entry point), never a bypass — confirmed by reading `crates/brokerd/tests/email_smtp_acceptance.rs::seed_and_confirm_email_send`, which calls `confirm(conn, ...)` directly. |
| 2 | A confined worker's direct SMTP connect FAILS under kernel-enforced default-deny net; SMTP credentials absent from worker env/args/plan-node payload | ✓ VERIFIED | Fresh Linux run: `[confine-probe] smtp: attempting connect() to 172.18.0.2:1025` → `[confine-probe] smtp: correctly blocked (errno=1, addr=172.18.0.2:1025)` → `test negative_net_smtp_mailpit ... ok`. errno=1 is EPERM (seccomp `apply_worker_filter` denying `socket(AF_INET,...)` before `connect()` can proceed — code-read confirms `probe_smtp` calls `TcpStream::connect` post-`apply_confinement()`, matching the module doc's claim that denial fires at `socket()`). `bash scripts/check-smtp-secrets-absent.sh` exits 0 (re-ran locally) — the caprun-worker spawn block in `cli/caprun/src/main.rs` contains no `CAPRUN_SMTP_` token; `smtp_host()/smtp_port()/smtp_from()` in `email_smtp.rs` read only trusted broker-process env, never `PendingConfirmation`/plan-node/ValueNode. |
| 3 | A CRLF/header-injection fixture cannot alter envelope/recipients — verified via Mailpit's actual captured envelope | ✓ VERIFIED | Fresh Linux run: `test smtp_05_crlf_body_cannot_smuggle_recipient ... ok`. Test reads `crates/brokerd/tests/email_smtp_acceptance.rs` asserts against Mailpit's HTTP DETAIL endpoint (`GET /api/v1/message/{ID}`) — `to == ["victim@example.com"]`, `cc.is_empty()`, `bcc.is_empty()` and specifically does NOT contain `attacker@evil.com` — not merely that `confirm()` returned `Released`. |
| 4 | A re-issued confirm / restart-mid-send / duplicate submission cannot double-fire — audit DAG records exactly ONE send | ✓ VERIFIED | Code-read of `confirmation.rs::confirm()`'s `"email.send"` arm: `let tx = conn.transaction()?;` wraps `transition_state(&tx, ...)` (the CAS) + `append_event(&tx, &attempted_event, ...)`, then `tx.commit()?` — one atomic SQLite transaction, confirmed by direct read (not assumed). Unit test `confirm_email_send_twice_records_exactly_one_attempted_event` (macOS `cargo test -p brokerd`, re-run locally: passes) asserts exactly one `email_send_attempted` and one `email_send_succeeded` event after two confirms on the same effect_id. |
| 5 | Adapter failure after confirm surfaces the error (never swallowed), is recorded in the DAG, cannot silently retry into a double-send | ✓ VERIFIED | Code-read: `email_smtp.rs::record_send_failed` routes raw error text to `eprintln!` (never the hash chain), appends an opaque `email_send_failed` event, returns `Err` — never `.unwrap()`/panic/silent drop. `confirmation.rs`'s `email.send` arm maps that `Err` to `ConfirmOutcome::EmailSendFailed` (distinct from `ConfirmedButSinkFailed`), and `cli/caprun/src/main.rs:339-341` maps it to exit code 7, distinct from 0/2/3/4/5/6. Unit test `confirm_email_send_adapter_failure_yields_email_send_failed` (re-run locally: passes) proves a second confirm after the failure returns `AlreadyTerminal` with no second `email_send_attempted`. |

**Score:** 5/5 truths verified, 0 present-but-behavior-unverified.

## Deep-Dive Verification of Adversarial Claims

| # | Claim | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `email.send` CAS + `email_send_attempted` append commit in ONE atomic SQLite transaction (not two autocommit statements) | ✓ CONFIRMED | `confirmation.rs:442-459`: `conn.transaction()` → `transition_state(&tx, ...)` → `append_event(&tx, ...)` → `tx.commit()`. Generic Step 6 (`confirmation.rs:411`) is explicitly skipped for `email.send` via `if pc.sink.0.as_str() != "email.send"` guard, avoiding a double-CAS. |
| 2 | `email_send_attempted`/`_succeeded`/`_failed` hashed payloads carry ONLY effect_id/opaque metadata | ✓ CONFIRMED | `audit.rs::append_event` hashes `serde_json::to_string(event)` (the whole `Event` struct). All three events set only `actor = "sink:email.send:{effect_id}"` (or `"sink:email.send:{effect_id}"` for the attempted event) and `event_type`; no `ResolvedArg.literal` or raw SMTP response text is ever placed in any `Event` field — grep-confirmed no literal-carrying variable flows into the `Event::new(...)` calls in `email_smtp.rs` or the `email.send` arm of `confirmation.rs`. Raw error text goes only to `eprintln!` (`record_send_failed`). |
| 3 | `file.create`'s dispatch arm + `ConfirmedButSinkFailed` swallow-shape are byte-for-byte unchanged | ✓ CONFIRMED | `confirmation.rs:420-434`, the `"file.create"` arm, is unmodified from its pre-Phase-13 shape (`Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed)` intact); the new `"email.send"` arm is added as a sibling match arm, not a rewrite of the existing one. |
| 4 | `confine-probe smtp` negative-net test genuinely exercises kernel-enforced denial | ✓ CONFIRMED (independently re-run) | `#[cfg(target_os = "linux")]`-gated in `confinement_integration.rs`; spawns the real `confine-probe` binary as a child process, which self-confines via `sandbox::apply_confinement()` then calls `std::net::TcpStream::connect()`. Fresh Linux run: exit 0, `errno=1` (EPERM) — this is a real kernel syscall denial, not a code-inspection claim; probe correctly distinguishes this from ECONNREFUSED (which would indicate `socket()` succeeded and only the downstream `connect()` failed — see `probe_smtp`'s explicit branch comment). |
| 5 | Old `crates/brokerd/src/sinks/email_send.rs` stub actually deleted, no dangling references | ✓ CONFIRMED | File absent from disk (`ls` returns "No such file or directory"). `grep -rn "invoke_email_send_stub\|mod email_send" crates/ cli/` returns zero matches. `sinks.rs` declares only `pub mod email_smtp;` / `pub mod file_create;`. Git history shows explicit deletion commit `a7f46f8 chore(13-02): delete the dead email_send.rs stub`. |
| 6 | No new `EffectRequest`-style bypass; `PlanNode{sink, args: Vec<ValueNode>}` API untouched | ✓ CONFIRMED | `./scripts/check-invariants.sh` Gate 1 PASS (re-run locally, exit 0) — only match is the allow-listed doc-comment mention in `lib.rs:32`. `PlanNode` struct in `runtime-core/src/plan_node.rs` unchanged by this phase (not in any `files_modified` list of the 4 plans). |

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/brokerd/src/sinks/email_smtp.rs` | Only SMTP-calling code path, typed builder only | ✓ VERIFIED | Exists, substantive (274 lines), wired via `confirmation.rs` `"email.send"` arm. `Address::parse` used before any builder call; no `format!()`-built headers; no `dangerous_new_pre_encoded` token. |
| `lettre 0.11.22` dependency, default features | Pinned, no boring-tls | ✓ VERIFIED | `Cargo.lock` resolves `lettre 0.11.22`; workspace build succeeds; Linux build log shows `native-tls` (default feature), not `boring-tls`. |
| `scripts/check-email-smtp-construction.sh` | Grep gate, CRLF-defense | ✓ VERIFIED | Executable, exit 0 (re-run locally). Confirms `Message::builder` present, forbidden token absent. |
| `confirm()` widened to `&mut rusqlite::Connection` | 7 call sites updated | ✓ VERIFIED | `confirmation.rs:352`; `deny()` unchanged at `&rusqlite::Connection` (confirmed by reading signature). |
| `ConfirmOutcome::EmailSendFailed` + exit code 7 | Distinct from swallow-shape | ✓ VERIFIED | Enum variant at `confirmation.rs:245`; CLI arm at `main.rs:339-341`. |
| `confine-probe smtp <host> <port>` op | Real connect() under confinement | ✓ VERIFIED + Linux-run confirmed | `confine-probe.rs:188-220`; independently re-run on Linux, exit 0 (kernel-denied). |
| `scripts/check-smtp-secrets-absent.sh` | CAPRUN_SMTP_ absent from worker-spawn | ✓ VERIFIED | Executable, exit 0 (re-run locally). |
| `crates/brokerd/tests/email_smtp_acceptance.rs` | SMTP-03 + SMTP-05 via Mailpit HTTP API | ✓ VERIFIED + Linux-run confirmed | Both tests independently re-run on real Linux, pass. |
| `scripts/mailpit-verify.sh` | Reusable Mailpit sidecar + Linux recipe | ✓ VERIFIED | Executed end-to-end by this verifier; full workspace test suite (all crates) passed on real Linux including both new acceptance tests and the negative-net test. |

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `confirm(email.send)` | `email_smtp::invoke_email_smtp_from_resolved` | direct call after tx.commit() | ✓ WIRED | `confirmation.rs:463-478` |
| `build_message` | recipient literal | `Address` parse before builder | ✓ WIRED | fail-closed CRLF boundary confirmed by unit test `build_message_rejects_crlf_in_to_fail_closed` (passes) |
| CLI `confirm` verb | `ConfirmOutcome::EmailSendFailed` | exit code mapping | ✓ WIRED | `main.rs:339-341` |
| `caprun-worker` spawn | `CAPRUN_SMTP_*` env | absence, verified by grep gate | ✓ CONFIRMED ABSENT | `check-smtp-secrets-absent.sh` exit 0 |

## Requirements Coverage

| Requirement | Source Plan | Status | Evidence |
|---|---|---|---|
| SMTP-01 | 13-03 | ✓ SATISFIED | Kernel-enforced negative-net test independently re-run and passing on real Linux |
| SMTP-02 | 13-03 | ✓ SATISFIED | Grep gate + code-read confirm secrets never reach worker/plan-node |
| SMTP-03 | 13-04 | ✓ SATISFIED | Mailpit acceptance test independently re-run and passing on real Linux |
| SMTP-05 | 13-01, 13-04 | ✓ SATISFIED | CRLF fixture (structural unit test + live Mailpit envelope assertion) independently re-run and passing |
| SEND-01 | 13-02 | ✓ SATISFIED | Atomic-transaction code read + unit test proving exactly-one-attempt |
| SEND-02 | 13-02 | ✓ SATISFIED | Non-swallowed failure path, exit code 7, no-auto-retry unit test |

All 6 phase-declared requirement IDs are present in `.planning/REQUIREMENTS.md`. No orphaned requirements were found mapped to Phase 13 beyond these 6.

**Documentation-hygiene note (non-blocking, ℹ️ INFO):** `.planning/REQUIREMENTS.md`'s checkbox state for SMTP-01/SMTP-02 is still `[ ]` (unchecked) and its tracking table (line ~82-83) still lists them "Pending," even though this verification confirms both are functionally satisfied and SMTP-03/05/SEND-01/SEND-02 in the same table ARE marked complete. This looks like the requirements-doc refresh step was only partially run after Phase 13's completion (SMTP-03/05/SEND-01/02 got updated, SMTP-01/02 did not). Recommend running the docs-update step to flip SMTP-01/SMTP-02 to `[x]`/Complete — this is a documentation staleness gap, not a functional gap; it does not block phase-goal achievement since the underlying code and Linux-verified tests are genuinely in place.

## Anti-Patterns Found

None. Scanned all phase-modified files (`email_smtp.rs`, `confirmation.rs`, `confine-probe.rs`, `email_smtp_acceptance.rs`, `confinement_integration.rs`, both new grep-gate scripts, `mailpit-verify.sh`, `main.rs`) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER|placeholder|coming soon|not yet implemented` — zero matches.

## Behavioral Spot-Checks / Full Verification Run

Ran the following commands directly (not relying on SUMMARY.md's reported results):

- `./scripts/check-invariants.sh` → PASS (both gates)
- `bash scripts/check-email-smtp-construction.sh` → PASS
- `bash scripts/check-smtp-secrets-absent.sh` → PASS
- `cargo build --workspace` → succeeds
- `cargo test --workspace --no-fail-fast` (macOS) → all non-Linux-gated tests pass; Linux-gated tests (email_smtp_acceptance.rs, confinement_integration.rs) show 0 passed as expected/documented
- `bash scripts/mailpit-verify.sh` (real Linux via Colima+Docker, independently re-executed by this verifier, NOT reused from executor's prior report) → full workspace test suite passes, including:
  - `negative_net_smtp_mailpit ... ok` (`errno=1`, kernel-enforced EPERM at `connect()` to the real Mailpit container IP)
  - `smtp_03_confirmed_send_captured_by_mailpit ... ok`
  - `smtp_05_crlf_body_cannot_smuggle_recipient ... ok`
  - All other 200+ workspace tests pass, 0 failures

## Human Verification Required

None. All must-haves were verifiable programmatically, including the Linux-only claims, which were independently re-executed on real hardware/kernel (via Colima+Docker) rather than accepted from the executor's self-report.

## Gaps Summary

No gaps. All 5 ROADMAP success criteria are genuinely achieved and independently re-verified, including on real Linux. One documentation-hygiene note (REQUIREMENTS.md checkbox staleness for SMTP-01/SMTP-02) is flagged as non-blocking — recommend a docs-update pass before milestone close, but it does not affect phase-goal achievement.

---

*Verified: 2026-07-07T21:15:00Z*
*Verifier: Claude (gsd-verifier)*
