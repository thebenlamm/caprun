---
phase: 16-confirm-ux-literal-binding-negative-controls
verified: 2026-07-09T02:36:59Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 16: Confirm UX, Literal Binding & Negative Controls Verification Report

**Phase Goal:** A human sees the verbatim, provenance-narrated recipient and body before deciding, the confirm is cryptographically bound to the exact resolved literals so send cannot drift from what was shown, and two negative controls prove the gate is taint-driven rather than a blanket email block.
**Verified:** 2026-07-09T02:36:59Z
**Status:** passed
**Re-verification:** No — initial verification

This report does NOT rely on prior SUMMARY.md self-reports or the orchestrator's own Colima/Docker test run as evidence. Every claim below was independently re-derived by reading the current source on `main` (HEAD includes `a24abfc`).

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | CONFIRM-01: `confirm`/`deny` display verbatim recipient+body+provenance for a doc-derived block | VERIFIED | `crates/brokerd/src/confirmation.rs:589` (`confirm()`) and `:764` (`deny()`) both call `render_block_display(&pc)` before any state-changing step; `render_block_display` narrates every `resolved_arg`, untruncated, with taint + source event + provenance chain (16-02 commit `b61e043`). |
| 2 | CONFIRM-03: single combined-digest over the FULL resolved_args set, name+literal bound | VERIFIED | `combined_digest()` (`confirmation.rs:64-94`) hashes `sha256(name)‖sha256(literal)` per element, byte-wise-ascending sort, duplicate-name assert. Producer (`server.rs:515-556`) builds `pairs` from **all** `plan_node.args`, not filtered by `anchors`/blocked names — `blocked_arg_names` is explicitly commented "DISPLAY-MARKING metadata ONLY ... does NOT gate the digest's domain" (`server.rs:548-553`). Verifier (`confirmation.rs:605-620`) recomputes from `pc.resolved_args` (the full frozen set) via the SAME shared primitive. |
| 3 | CONFIRM-04: block-moment narration for every blocked arg, no truncation | VERIFIED | `render_block_display` (post-16-02) narrates every resolved arg marked `[BLOCKED]`/`[trusted]`; the prior `assert!(blocked_count <= 1)` single-arg guard was removed in commit `b61e043`, proven-to-panic-first in a separate commit `1f3336b` (two-commit T-14-08 discipline, confirmed via `git show --stat` on both). |
| 4 | CONTROL-01: a fully-trusted send proceeds with NO block and NO confirm gate | VERIFIED | `s9_control_ab_taint_driven` (`cli/caprun/tests/s9_live_block.rs:797-894`) asserts, for the trusted-intent half: `plan_node_evaluated` present, `sink_blocked` absent, `pending_confirmations` COUNT = 0, `email_send_succeeded` present, and Mailpit-captured. The taint mechanism is genuine: `worker.rs` only derives a tainted recipient/body when marker fragments (`Reply-To:`/`Domain:`/`Body:`) are actually found in the doc (`worker.rs:194-263`); `CLEAN_PATH_CONTENT` has none, so `plan_from_intent` routes the CLI's `SeedProvenance::TrustedArg` literal — not a hardcoded bypass. |
| 5 | CONTROL-02: body tainted + recipient trusted still blocks | VERIFIED | `s9_control02_body_tainted_recipient_trusted_blocks` (`s9_live_block.rs:279-336`) asserts `blocked.anchors.len() == 1` and `anchors[0].arg == "body"` — explicitly guards against an accidental 2-anchor block (Pitfall 5), proving the body-content dimension fires independently of the routing/recipient dimension. |

**Score:** 5/5 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/brokerd/src/server.rs` `ProvideIntent` arm | broker-enforced once/before-RequestFd ordering | VERIFIED | `intent_provided`/`fd_requested` are `let mut` locals owned by `handle_connection`'s per-connection loop (`server.rs:153-154`), threaded `&mut` into `dispatch_request`; `fd_requested` set at RequestFd entry (`:345`, before any other work); `ProvideIntent` rejects if `*intent_provided \|\| *fd_requested` (`:904`), and only sets `intent_provided = true` after the mint(s) succeed (`:1000`). Not a worker-cooperative flag — the worker cannot set these. |
| `crates/brokerd/src/server.rs` `CreateSession` IPC arm | opt-in flag gate, exact-match `"1"`, default-deny | VERIFIED | `!matches!(std::env::var("CAPRUN_ENABLE_IPC_CREATE_SESSION").as_deref(), Ok("1"))` (`:267-270`) — exact string match, not `.is_ok()`; unset/empty/any-other-value returns `BrokerResponse::Error` and mints nothing (`:271-283`). |
| `crates/executor/src/lib.rs` non-live-state deny gate | exhaustive match, no wildcard, Deny for `WaitingApproval`/`Done`/`Failed`/`RolledBack` | VERIFIED | `match *session_status` (`:176-213`) lists all 6 `SessionStatus` variants (`Draft`, `Active`, `WaitingApproval \| Done \| Failed \| RolledBack`) explicitly — confirmed against the enum definition (`crates/runtime-core/src/session.rs:18-25`, exactly 6 variants). No `_` wildcard arm; a 7th variant would be a compile error. |
| `crates/brokerd/src/server.rs` `email.send` Allowed-dispatch | mirrors `file.create`'s resolve-only pattern (never mints) | VERIFIED | `:721-736` resolves each `plan_node.args[i]` via `value_store.resolve(...)` into `ResolvedArg` — no `mint_from_intent`/`mint_from_derivation`/`.mint()` call anywhere in this branch. Same shape as the `file.create` Allowed branch (`:683-703`). |
| MAJOR-4 durable attempt ledger | `email_send_attempted` appended, parent-chained, under lock, strictly BEFORE the SMTP call | VERIFIED | `:738-768` appends `attempted_event` (type `email_send_attempted`, `parent_id = Some(*last_event_id)`) under the same `conn.lock()`, advances `last_event_id`/`last_event_hash`, and only THEN (`:777-785`, after the `?`-propagating append) calls `invoke_email_smtp_from_resolved`. Ordering is unconditional — a failed append short-circuits via `?` before the socket ever opens. |
| `crates/brokerd/src/confirmation.rs::combined_digest` | full-set, name+literal bound, sorted, dedup-asserted | VERIFIED | See Truth #2 above; also independently exercised by 5 unit tests (`combined_digest_name_binding_rename_differs`, `_partition_binding_boundary_shift_differs`, `_transposed_literals_differs`, `_duplicate_arg_name_panics`, `_input_order_invariant`) enumerated in the file (`:810-879`). |
| `crates/brokerd/src/confirmation.rs::review` | read-only, no mutation, no submit_plan_node | VERIFIED | `review()` (`:501-508`) only calls `find_pending_confirmation` (read) and `println!` — no `transition_state`, no `append_event`, no sink invocation. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `confirm()` / `deny()` | `crate::audit::verify_chain` | chain-verify gate | WIRED, correctly ordered | `confirm()` calls `verify_chain` (`:599`) BEFORE the digest recompute-and-compare (`:605-636`) — chain-verify-first is the documented and actual order. Doc comment at `:594-598` scopes the claim honestly: "detects single-store and non-recomputing multi-store tampering... NOT authenticated/externally-anchored" — matches `DESIGN-confirm-binding.md`'s Round-6/honesty language and the v2-deferred-obligations todo item #2. No "unforgeable"/"fully tamper-evident" overclaim found anywhere in the touched files or the design doc. |
| `confirm_digest_mismatch` / `confirm_granted` / `confirm_denied` events | chain head (not `blocked_event_id`) | `parent_id` | WIRED | All three call `current_chain_head_or_bail(conn, pc.session_id)` for `head_id` immediately before constructing their `Event` (`:600/633` mismatch, `:645` granted, `:766` denied) — none use `pc.blocked_event_id` directly as parent. `append_confirm_digest_mismatch_event`'s doc comment (`:527-533`) explicitly names this as the MAJOR-7 no-fork fix, and `digest_mismatch_then_retry_does_not_fork_dag_verify_chain_stays_true` exercises it. |
| `email.send` Allowed-dispatch | `invoke_email_smtp_from_resolved` | resolve → attempt-ledger → SMTP | WIRED | Confirmed above (MAJOR-4 row) — genuine two-phase ordering under one lock, mirroring `file.create`'s pattern. |

### Anti-Patterns Found

None. Scanned `crates/brokerd/src/confirmation.rs`, `crates/brokerd/src/server.rs`, `crates/executor/src/lib.rs`, `cli/caprun/src/main.rs`, `cli/caprun/src/worker.rs` for `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` — zero matches. No stub returns, no empty handlers, no hardcoded-empty data flowing to output.

### Requirements Coverage

| Requirement | Source Plan | Status | Evidence |
|---|---|---|---|
| CONFIRM-01 | 16-02 | SATISFIED | Truth #1 |
| CONFIRM-03 | 16-01, 16-02 | SATISFIED | Truth #2 |
| CONFIRM-04 | 16-02 | SATISFIED | Truth #3 |
| CONTROL-01 | 16-04 | SATISFIED | Truth #4 |
| CONTROL-02 | 16-03 | SATISFIED | Truth #5 |

No orphaned requirements — all 5 phase-scoped requirement IDs appear in exactly one plan's `requirements:` frontmatter field, with no additional Phase-16-mapped IDs in REQUIREMENTS.md left unclaimed.

### Specific Checks (from verification brief)

| # | Check | Result |
|---|-------|--------|
| 1 | Exfiltration hole genuinely closed (ProvideIntent ordering state, CreateSession exact-match flag, executor exhaustive non-live deny) | PASS — all three sub-checks verified against source, see Artifacts table |
| 2 | email.send mirrors file.create's resolve-only pattern | PASS — verified, no mint call in the branch |
| 3 | MAJOR-4 ledger wired strictly before SMTP call | PASS — verified, unconditional ordering via `?` |
| 4 | combined_digest full-set + name-bound | PASS — verified, not filtered by blocked-arg-name list |
| 5 | verify_chain wired before digest recompute, honestly scoped | PASS — verified, correct order, no overclaim found |
| 6 | DAG no-fork: parent_id from current chain head, not fixed blocked_event_id | PASS — verified for all three event types |
| 7 | `caprun review` read-only | PASS — verified, no mutation/append/submit |
| 8 | CONTROL-01/02 discriminate on taint, not doc content | PASS — verified; taint derivation is fragment-marker-conditioned, not string-matching |
| 9 | T-14-08 two-commit sequencing | PASS — `1f3336b` (proof panic) then `b61e043` (replace), confirmed via git log/show |
| 10 | Premature phase-completion reverted in ROADMAP.md | PASS — commit `a24abfc` reverts Phase 16's checkbox to `[ ]` with "4/4 plans executed, independent verification pending"; this verification report is what should now enable the flip |

### Out-of-scope items (correctly NOT treated as gaps)

`.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` records 5 explicitly-deferred v2 obligations (demote-at-RequestFd honest-scope, verify_chain not externally anchored, Allowed-path replay has no CAS, CreateSession runtime-flag vs build-excluded path, kind-aware source label). All five are consistent with what the code actually does today (matches source inspection above) and are correctly scoped as accepted residual risk / v2 work, not silently-dropped v1.3 requirements.

### Human Verification Required

None. All must-haves were verifiable directly against source and named tests; no behavior-dependent truth required a runtime spot-check beyond what the orchestrator's own Linux test run (out of scope for this agent to re-run) already exercises, and this report independently confirmed the code paths those tests exercise are genuine rather than mocked/stubbed.

### Gaps Summary

None found. Every specific check in the verification brief was traced to file:line evidence in current source, matches the DESIGN doc's honestly-scoped claims, and is consistent with the explicitly-recorded out-of-scope v2 obligations. ROADMAP.md Phase 16 is correctly still unchecked pending this verification — recommend flipping to `[x]` now that verification has passed.

---

_Verified: 2026-07-09T02:36:59Z_
_Verifier: Claude (gsd-verifier)_
