---
phase: 28-authenticated-audit-chain
plan: 03
subsystem: security
tags: [rust, hmac, hmac-sha256, audit-chain, mac, tamper-evidence, rusqlite]

# Dependency graph
requires:
  - phase: 28-authenticated-audit-chain
    provides: "28-01: F1-safe live-test fixture layout + hmac/getrandom deps in crates/brokerd"
  - phase: 28-authenticated-audit-chain
    provides: "28-02: load_or_create_key(audit_path, workspace_root) — cross-process MAC-key custody + F1 fail-closed refusal, unit-tested in isolation"
provides:
  - "compute_event_hash/verify_event_hash keyed HMAC-SHA256 (audit.rs), replacing the unkeyed SHA-256 chain"
  - "mac_frame: a shared, domain-separated + length-framed MAC-input builder for reuse by Plan 04 (chain anchor) and Plan 05 (pending_confirmations)"
  - "append_event/verify_chain both keyed; key threaded through all 19 production append_event call sites + both verify_chain callers"
  - "main.rs run-path wiring: load_or_create_key (F1-checked) called before the broker spawns, key fed into the audit handle and end-of-run verify_chain"
  - "minimal key-load also wired into run_confirm_or_deny (both verbs) — necessary consequence of confirm()/deny()'s new key parameter, not full Plan 05 scope"
affects: [28-04-authenticated-audit-chain, 28-05-authenticated-audit-chain]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "mac_frame(mac, domain, fields): domain tag first, then each field length-prefixed (8-byte LE) before its bytes — closes both cross-record-type MAC replay (via domain separation) and field-boundary ambiguity (via length framing). Plans 04/05 MUST reuse this helper with their own domain tags (b\"caprun.audit.anchor.v1\" / b\"caprun.audit.pending-confirmation.v1\"), never a bare per-field mac.update() concatenation."
    - "key threaded as a plain `&[u8]` sibling parameter through every function in the call chain that appends/verifies (never bundled into the connection type) — the accepted Step-C fallback since `conn`'s locked guard is consumed as a bare `&rusqlite::Connection` by many non-audit call sites (session/pending_confirmations/blocked_literals helpers) that would otherwise need restructuring too."
    - "server.rs threads the key as `Arc<[u8; 32]>`, cloned per accepted connection exactly like the `conn` Arc."

key-files:
  created: []
  modified:
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/confirmation.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/sinks/file_create.rs
    - crates/brokerd/src/sinks/email_smtp.rs
    - cli/caprun/src/main.rs
    - crates/brokerd/tests/audit_dag.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/harden01_session_integrity.rs
    - crates/brokerd/tests/phase5_dispatch.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - crates/brokerd/tests/extract_provenance_threading.rs
    - crates/brokerd/tests/email_smtp_acceptance.rs
    - cli/caprun/tests/confirm.rs
    - cli/caprun/tests/e2e.rs
    - cli/caprun/tests/llm_planner_live_accept.rs
    - cli/caprun/tests/live_acceptance_v1_3.rs
    - cli/caprun/tests/live_acceptance_tainted_session.rs
    - cli/caprun/tests/s9_live_block.rs
    - cli/caprun/tests/live_acceptance_v1_4_composed.rs

key-decisions:
  - "mac_frame domain-separation + length-framing (security_emphases #1, additive hardening beyond the plan's literal per-field text): compute_event_hash/verify_event_hash build their MAC input via a SHARED mac_frame(mac, domain, fields) helper — domain tag b\"caprun.audit.event.v1\" mixed in first, then each field 8-byte-LE-length-prefixed before its bytes. This closes both the (\"ab\",\"c\")/(\"a\",\"bc\") field-boundary collision AND cross-record-type MAC replay if the broker key is ever reused across record types without a domain tag. Plans 04/05 MUST reuse this exact helper with their own domain tags — noted in the module doc comment and the DigestMismatch doc comment so the next plans find it."
  - "Key threading strategy: sibling `&[u8]`/`Arc<[u8; 32]>` parameter, not a bundled KeyedAudit connection type (plan's must_haves truth preferred bundling; Step C explicitly accepts this fallback). Bundling would ALSO have required a `key: &[u8]` parameter on every inner function (mint_from_read/mint_from_intent/mint_from_derivation/confirm/deny/invoke_file_create/invoke_email_smtp_from_resolved) since those receive an already-locked bare `&rusqlite::Connection`, not the wrapper — bundling's only benefit (avoiding a `run_broker_server` signature change) came at the cost of restructuring every `conn.lock()` call site's downstream usage. The sibling-parameter approach touches more function signatures but leaves every existing `&locked`/`&conn` usage untouched, which was less invasive in this codebase's shape."
  - "run_confirm_or_deny (main.rs) gained a minimal key-load for BOTH confirm and deny verbs — genuinely necessary, not scope creep: cli/caprun/tests/confirm.rs's 4 cross-process tests run unconditionally on macOS (not Linux-gated) and assert real exit codes from a real `caprun confirm`/`deny` subprocess against a chain seeded by the SAME test process. Once confirm()/deny() require a `key: &[u8]` parameter (mandated by this plan's Task 1 for the confirmation.rs:550/656/717/776/599 sites), run_confirm_or_deny needed SOME key value to compile — and only the REAL load_or_create_key value (fetched via pc.workspace_root_path, exactly mirroring what Plan 05 Task 2 formalizes) keeps confirm()'s keyed verify_chain gate from spuriously failing on every legitimate cross-process confirm. A placeholder/wrong key would have regressed a currently-passing test suite (violates this plan's own \"no new failures\" verification bar). This pre-completes ONLY the key-load half of Plan 05 Task 2 — the pending_confirmations whole-row MAC column/fold, the MAC-verify-before-terminal-state gate, and deny()'s brand-new verify_chain call are untouched and remain Plan 05's scope."
  - "cli/caprun/tests/confirm.rs cannot import cli/caprun/src/key.rs's load_or_create_key (cli/caprun is bin-only, no lib target reachable from external integration tests) — added a test-local seed_test_key() that duplicates ONLY the idempotent read-existing-.key-file-first behavior (no F1 check needed; the test fully controls its own tmpdir layout, which is always F1-safe — audit.db and workspace/ are siblings, never nested). The test writes the key BEFORE seeding events with it, so when the caprun confirm/deny subprocess later calls the real load_or_create_key, it reads back the SAME persisted bytes."
  - "Linux-gated live-acceptance test files (e2e.rs, s9_live_block.rs, live_acceptance_v1_3/v1_4_composed/tainted_session.rs, llm_planner_live_accept.rs) call verify_chain against a DB a spawned caprun subprocess wrote — fixed these to read back `<audit_db_path>.key` from disk (the subprocess's own load_or_create_key already wrote it) rather than guessing a key. None of these run on macOS (cfg-stripped), so this is verified by successful compilation only in this environment; the phase's Linux gate (bash scripts/mailpit-verify.sh) is the actual runtime proof, deferred to Plan 05/30 per this phase's structure."
  - "audit.rs's two Task 2 tests were authored in the same edit pass as Task 1's signature changes, then git-split into two commits (Task-1-only version built/verified independently before committing, then the additive 178-line Task 2 block restored and committed separately) to honor the plan's per-task atomic-commit protocol."

requirements-completed: [HARDEN-02]

coverage:
  - id: D1
    description: "compute_event_hash is a keyed HMAC-SHA256 (domain-separated + length-framed via mac_frame); two different keys over identical fields produce different digests"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::verify_chain_is_key_dependent"
        status: pass
    human_judgment: false
  - id: D2
    description: "A self-consistent forgery built WITHOUT the broker key (unkeyed SHA-256 recompute of every descendant row) is rejected by the keyed verify_chain — Success Criterion 1"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::self_consistent_forgery_without_key_is_rejected"
        status: pass
    human_judgment: false
  - id: D3
    description: "verify_chain called with the WRONG key on an otherwise-untampered chain returns false — Success Criterion 2; constant-time compare via Mac::verify_slice, never ==/!= on the hex digest"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/audit.rs#audit::tests::verify_chain_is_key_dependent"
        status: pass
    human_judgment: false
  - id: D4
    description: "The broker key reaches all 19 production append_event call sites + both verify_chain callers + main.rs run-path key load; cargo build --workspace compiles clean with zero missed sites"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "cargo build --workspace (exit 0); ./scripts/check-invariants.sh (exit 0, 4/4 PASS)"
        status: pass
    human_judgment: false
  - id: D5
    description: "cargo test --workspace --no-fail-fast on macOS shows no new failures (277 baseline + 2 new tests = 279 passed / 0 failed), including cli/caprun/tests/confirm.rs's 4 cross-process confirm/deny tests, which exercise the keyed chain across real OS processes"
    requirement: "HARDEN-02"
    verification:
      - kind: unit
        ref: "cargo test --workspace --no-fail-fast (279 passed / 0 failed, exit 0)"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/confirm.rs (4/4 passed: confirm_releases_once_and_second_confirm_is_already_terminal, deny_is_durable_and_confirm_after_deny_is_already_terminal, confirm_and_deny_on_unknown_effect_id_exit_4, confirm_email_send_adapter_failure_exits_7)"
        status: pass
    human_judgment: false

duration: 31min
completed: 2026-07-12
status: complete
---

# Phase 28 Plan 03: Keyed HMAC-SHA256 Audit Chain Summary

**Converts the audit hash chain from unkeyed SHA-256 self-consistency to a keyed, domain-separated, length-framed HMAC-SHA256 MAC, threading the broker key through all 19 production `append_event` sites and both `verify_chain` callers, with the key sourced cross-process via Plan 02's `load_or_create_key`.**

## Performance

- **Duration:** 31 min
- **Started:** 2026-07-12T20:40:31-04:00
- **Completed:** 2026-07-12T21:11:08-04:00
- **Tasks:** 2
- **Files modified:** 22 (0 created)

## Accomplishments
- `audit.rs`: added `type HmacSha256 = Hmac<Sha256>`, a shared `mac_frame(mac, domain, fields)` domain-separated + length-framed MAC-input builder (reserved for Plans 04/05's own domain tags), `EVENT_MAC_DOMAIN = b"caprun.audit.event.v1"`, keyed `compute_event_hash(key, ...)` (key FIRST param), and a new `verify_event_hash(key, expected_hex, ...)` using `Mac::verify_slice` (constant-time, fail-closed on hex-decode error).
- `append_event`/`verify_chain` both take `key: &[u8]`; `verify_chain`'s per-row compare now calls `verify_event_hash` instead of a plain `!=` hex compare.
- Threaded `key` through all 19 enumerated production `append_event` sites (`server.rs` x5, `quarantine.rs` x4, `confirmation.rs` x4, `sinks/file_create.rs` x4, `sinks/email_smtp.rs` x2) plus `main.rs`'s own session_created append and both `verify_chain` callers (`confirmation.rs::confirm`, `main.rs`'s end-of-run check).
- `quarantine.rs`'s three mint functions (`mint_from_read`, `mint_from_intent`, `mint_from_derivation`) and `confirmation.rs`'s `confirm`/`deny` and `sinks/file_create.rs`'s two `invoke_file_create*` and `sinks/email_smtp.rs`'s `invoke_email_smtp_from_resolved` all gained a `key: &[u8]` parameter.
- `server.rs` threads the key as `Arc<[u8; 32]>` from `run_broker_server` down through `classify_second_connection`/`handle_connection` (cloned per connection, mirroring `conn`), then as `&[u8]` through `evaluate_plan_node_and_record`/`dispatch_request`/`create_session_arm`.
- `main.rs`'s run path calls `key::load_or_create_key(&audit_path, workspace_root_dir)` (F1-checked, fail-closed) after `workspace_root_dir` is derived and before the broker task spawns; converts to a fixed `[u8; 32]` wrapped in `Arc`; feeds it into `run_broker_server`, the `session_created` append, and the end-of-run `verify_chain`.
- `run_confirm_or_deny` also loads the F1-checked key for both `confirm` and `deny` verbs (via `pc.workspace_root_path`, fetched before dispatch) — the minimal wiring needed for `confirm()`/`deny()`'s new `key` parameter to actually verify correctly cross-process (see Decisions).
- Updated ~19 more test files (both `#[cfg(test)]` modules inside `brokerd` and external `tests/` integration files) to thread a fixed, non-secret `TEST_KEY` constant through every `append_event`/`verify_chain`/mint-function/`dispatch_request`/`confirm`/`deny` call, keeping the existing 277-test baseline green.
- `cli/caprun/tests/confirm.rs` (4 cross-process tests, unconditionally run on macOS) gained a test-local `seed_test_key()` custody helper so the test's own seeding writes match what the spawned `caprun confirm`/`deny` subprocess reads back — all 4 tests pass.
- Fixed 6 Linux-gated live-acceptance test files (compile-verified only, not run, on macOS) to read back the persisted `<audit_db>.key` for their `verify_chain` assertions against a subprocess-produced chain.
- Refreshed the stale unkeyed-chain doc comments (`ConfirmOutcome::DigestMismatch`, `confirm()`'s Step 4.5a comment, `audit.rs`'s module doc) to describe the keyed scheme and honestly scope what remains open for Plans 04 (chain-anchor/tail-truncation) and 05 (`pending_confirmations` MAC fold).
- Task 2: added `self_consistent_forgery_without_key_is_rejected` and `verify_chain_is_key_dependent` to `audit.rs`, proving Success Criteria 1 and 2.

## Task Commits

Each task was committed atomically:

1. **Task 1: Keyed compute_event_hash + constant-time verify helper; thread key through all 19 append_event sites, both verify_chain callers, and main.rs runtime wiring** - `f16a0a9` (feat)
2. **Task 2: Tests — self-consistent forgery without the key is rejected; key-dependence** - `394869e` (test)

## Files Created/Modified
- `crates/brokerd/src/audit.rs` - keyed `compute_event_hash`/`verify_event_hash`, `mac_frame`, `HmacSha256`, keyed `append_event`/`verify_chain`, 2 new forgery/key-dependence tests
- `crates/brokerd/src/quarantine.rs` - `key: &[u8]` threaded through `mint_from_read`/`mint_from_intent`/`mint_from_derivation`
- `crates/brokerd/src/confirmation.rs` - `key: &[u8]` threaded through `confirm`/`deny`/`append_confirm_digest_mismatch_event`; refreshed stale MAJOR-6/Step-4.5a doc comments
- `crates/brokerd/src/server.rs` - `Arc<[u8; 32]>` threaded from `run_broker_server` down through the connection-handling call chain
- `crates/brokerd/src/sinks/file_create.rs` - `key: &[u8]` threaded through `invoke_file_create`/`invoke_file_create_from_resolved`
- `crates/brokerd/src/sinks/email_smtp.rs` - `key: &[u8]` threaded through `invoke_email_smtp_from_resolved`/`record_send_failed`
- `cli/caprun/src/main.rs` - run-path `load_or_create_key` wiring (Step D); `run_confirm_or_deny` key-load for both verbs
- 15 test files (`crates/brokerd/tests/*`, `cli/caprun/tests/*`) - threaded a fixed `TEST_KEY` or subprocess key-file read-back through every call site whose signature changed

## Decisions Made
See `key-decisions` in frontmatter (mac_frame domain-separation, sibling-parameter key threading over bundling, minimal `run_confirm_or_deny` key-load, `confirm.rs` test-local key custody, Linux-gated file fixes, the audit.rs commit split).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Minimal key-load wiring into `run_confirm_or_deny` for both verbs**
- **Found during:** Task 1, after threading `key: &[u8]` through `confirm()`/`deny()` per the plan's enumerated `confirmation.rs:550/656/717/776/599` sites
- **Issue:** `main.rs`'s `run_confirm_or_deny` calls `confirm()`/`deny()`, which now require a `key: &[u8]` argument. `cli/caprun/tests/confirm.rs`'s 4 cross-process tests run unconditionally on macOS (not Linux-gated) and assert real exit codes from a `caprun confirm`/`deny` subprocess. Any key value other than the SAME one `load_or_create_key` would derive for that `audit_path`/`workspace_root` would make `confirm()`'s keyed `verify_chain` gate spuriously fail (DigestMismatch) on every legitimate cross-process confirm — a real regression of a currently-passing suite, violating this plan's own "no new test failures" verification bar.
- **Fix:** Restructured both the `confirm` and `deny` arms of `run_confirm_or_deny` to fetch the `PendingConfirmation` first (deny previously skipped this), then call `key::load_or_create_key(audit_path, Path::new(&pc.workspace_root_path))` before dispatching. This is the key-load HALF of Plan 05 Task 2's described wiring — it does not add the `pending_confirmations` MAC column, the MAC-verify-before-terminal-state gate, or `deny()`'s new `verify_chain` call, all of which remain Plan 05's scope (documented in `main.rs`'s `mod key;` doc comment and in this SUMMARY so Plan 05 can pick up cleanly).
- **Files modified:** `cli/caprun/src/main.rs`
- **Verification:** `cli/caprun/tests/confirm.rs` — 4/4 passed (`cargo test -p caprun --test confirm`).
- **Committed in:** `f16a0a9` (Task 1 commit)

**2. [Rule 3 - Blocking] Threaded the key through ~19 additional test files beyond the plan's enumerated production sites**
- **Found during:** Task 1, after the production signature changes made `cargo test --workspace --no-run` fail to compile
- **Issue:** The plan's 19+2 enumerated sites cover only production `append_event`/`verify_chain` callers. `brokerd::audit`/`brokerd::quarantine`/`brokerd::confirmation`/`brokerd::server::dispatch_request` are `pub`, so numerous existing `#[cfg(test)]` modules and `tests/*.rs` integration files also call these functions directly and needed the same signature update to keep compiling.
- **Fix:** Added a fixed, non-secret `TEST_KEY`/`seed_test_key()`/read-back-`<path>.key` helper to each affected file (15 test files touched in Task 1's commit) and threaded it through every affected call site.
- **Files modified:** see `key-files.modified` (test files).
- **Verification:** `cargo test --workspace --no-run` compiles with 0 errors; `cargo test --workspace --no-fail-fast` — 279 passed / 0 failed (baseline 277 + this plan's 2 new tests).
- **Committed in:** `f16a0a9` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking correctness/compile fixes necessitated by the signature change this plan's own Task 1 mandates)
**Impact on plan:** No scope creep into Plan 04/05's DISTINCT deliverables (chain-anchor MAC; `pending_confirmations` MAC fold + MAC-verify gate + deny()'s new verify_chain call). Both deviations were required for the plan's own stated verification bar ("no new test failures") to hold, and are documented for Plan 05 to build on cleanly.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Plan 04 (chain-anchor MAC, tail-truncation detection) can build directly on `mac_frame` with its own domain tag (`b"caprun.audit.anchor.v1"`, reserved in `audit.rs`'s doc comments) and the now-keyed `verify_chain`.
- Plan 05 (`pending_confirmations` MAC fold + `deny()`'s new integrity gate + full `run_confirm_or_deny` wiring) has a clean starting point: `confirm()`/`deny()` already accept `key: &[u8]`, and `run_confirm_or_deny` already loads the correct key for both verbs via `pc.workspace_root_path` — Plan 05 only needs to ADD the pending_confirmations MAC column/fold, move the MAC-verify to before the terminal-state branch, and give `deny()` its own `verify_chain` call (it currently has none).
- `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (279/279), and `./scripts/check-invariants.sh` (4/4 gates) all green at hand-off. No blockers.
- Linux gate (`bash scripts/mailpit-verify.sh`) not run in this environment (macOS dev box, per CLAUDE.md) — the 6 Linux-gated live-acceptance files were fixed and compile-verified via cfg-stripping only; their actual execution is deferred to this phase's Linux verification step.

---
*Phase: 28-authenticated-audit-chain*
*Completed: 2026-07-12*

## Self-Check: PASSED

`.planning/phases/28-authenticated-audit-chain/28-03-SUMMARY.md` confirmed present on disk. Both task commits confirmed present in `git log --oneline --all`: `f16a0a9` (Task 1, feat), `394869e` (Task 2, test).
