# Phase 51 Adversarial Trace Record

## Reviewer identity and independence

| Field | Value |
|---|---|
| Reviewer kind | fresh-context, read-only adversarial reviewer |
| Reviewer identifier | `/root/review_51_07_fix` |
| Model/runtime | Codex, GPT-5 runtime |
| Effort | inherited reviewer effort |
| Independence | Reviewer was not the author of the design note, fix commit, or implementation; made no file edits; and assumed no prior claim correct. Author is not equal to reviewer. |
| Orchestrator role | Spawned the independent reviewer and independently re-verified the review against the live code before authorizing this fold. |

## Files the reviewer opened

- The exact landed diff `a6b8911f5bcb3344b90416c5fcf57294b827cc0f..442a056e7fffb1d3bd32e6f93e1228908bde31f2` and its three changed sources: `crates/brokerd/src/audit.rs`, `crates/brokerd/src/server.rs`, and `crates/brokerd/src/confirmation.rs`.
- `crates/brokerd/src/quarantine.rs` and `crates/brokerd/src/policy.rs`.
- Terminal appenders under `crates/brokerd/src/sinks/`: `email_smtp.rs`, `file_create.rs`, `file_write.rs`, `git_commit.rs`, `git_push.rs`, `github_pr.rs`, `http_write.rs`, and `process_exec.rs`.
- `cli/caprun/src/main.rs`, including the short-lived grant, confirm, deny, and external-hold-poll connection paths.
- `crates/brokerd/tests/audit_chain_fork_regression.rs`.
- `planning-docs/DESIGN-audit-append-concurrency.md`, including Sections 1–10 and Appendix A.
- `.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-BLOCKING-DEFECTS.md` and `.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-ADVERSARIAL-TRACE-BRIEF.md`.

## Mandatory checks and outcomes

| # | Outcome | Code-trace evidence |
|---:|---|---|
| 1 | **PASS** | `append_event` discards caller parent authority and selects `current_chain_head` at `audit.rs:984-1005`; the row uses that durable parent/hash at `audit.rs:1016-1030`. The reviewer traced the connection handler, `record_github_grant`, confirm-granted path, and terminal sinks and found no production bypass. |
| 2 | **PASS** | Every `open_audit_db` connection receives the explicit timeout at `audit.rs:724-730`. The autocommit append opens an immediate transaction at `audit.rs:1065-1071`; already-transactional callers join the held transaction. Mutable enclosing transactions are immediate at `confirmation.rs:1163`, `server.rs:1184`, and `server.rs:1557`. The head read occurs inside `append_at_head` under that boundary. |
| 3 | **PASS** | Direct site check: `server.rs:941` states that the causal chain head is not `read_event_id`; `server.rs:957` passes `last_event_id` as the blocked event's causal parent; `server.rs:973` passes the same causal head for the non-blocked event. Provenance remains in the anchors passed at `server.rs:950-967`; none of the three sites derives, assigns, or reuses `read_event_id` as `parent_id`. The causal parent and provenance read-event graph are not conflated. |
| 4 | **PASS** | Mechanical comparison found `verify_chain`, `current_chain_head`, `compute_event_hash`, `verify_event_hash`, `compute_anchor_mac`, and `verify_anchor_mac` unchanged. Event count is read back and the anchor MAC computed/upserted under the append boundary at `audit.rs:1042-1063`, preserving HARDEN-02 tail-truncation detection. |
| 5 | **PASS** | Reviewer verified all **45** Appendix A production sites: **2 session roots, 40 chain continuations, and 3 in-transaction sites**, with no unlisted production bypass. Raw search yielded 93 occurrences before excluding the function definition and test-only calls. `record_github_grant` continues through the choke point at `audit.rs:564`; the dual-connection same-seed case is safe because caller state cannot select the persisted parent. |
| 6 | **PASS** | The fix did not make the sink-blocked event/literal/checkpoint group less atomic. All failures remain fail-closed under the broker-owned mutex at `server.rs:1020-1054`. The reviewer did identify the pre-existing limitation recorded as MINOR-01: the comments overstate database transaction atomicity because `append_event` commits before later literal/checkpoint statements. |
| 7 | **PASS** | Rollback during append rolls back the event/anchor transaction; poisoned mutex acquisition and exhausted SQLite contention propagate errors rather than authorizing an effect. No reviewed failure path fails open or continues from an uncommitted durable head. |
| 8 | **PASS** | `git diff --name-only a6b8911f5bcb3344b90416c5fcf57294b827cc0f..442a056e7fffb1d3bd32e6f93e1228908bde31f2` contains exactly `audit.rs`, `server.rs`, and `confirmation.rs`. It does not touch `crates/executor/`, policy, mint sites, Cargo manifests/lockfile, or the `caprun run` output surface. The committed audit-chain regression passes 2/2. |

The orchestrator independently re-verified the exact three-file range, source equivalence, cited comments, timeout/transaction sites, the direct `server.rs:941`, `:957`, and `:973` separation statements, and the 45-site filtered Appendix A result. Its source-equivalence check exited 0 and its independent audit regression rerun passed 2/2.

## Findings and resolutions

| ID | Severity | Claim and code evidence | Orchestrator re-verification | Resolution |
|---|---|---|---|---|
| MINOR-01 | MINOR | Pre-existing fail-closed limitation: `server.rs:1044-1049` says the sink-blocked event, literal writes, and checkpoint “succeed or fail together,” but `append_event` completes its own transaction before the later statements at `server.rs:1033-1054`. The mutex preserves ordering and failures remain closed, but the comment overstates database atomicity. | Independently reproduced against the live code; the cited ordering and comment are present. | Non-blocking, pre-existing, and outside this plan's no-production-edit scope. Record for a scoped follow-up; do not alter source during Plan 51-08. |
| NIT-01 | NIT | Stale comment at `audit.rs:1033-1037` says there are 19 production `append_event` sites, while the filtered Appendix A inventory and fresh trace establish 45. | Independently reproduced the stale comment and the 45-site filtered inventory; raw search has 93 occurrences before documented definition/test exclusions. | Non-blocking documentation-comment follow-up. Appendix A remains the authoritative current inventory; do not alter production source during Plan 51-08. |

No BLOCKER or MAJOR finding was reported. No finding was silently discarded or restated more favourably than the independent trace.

## Verdict

| Severity | Count | Unresolved |
|---|---:|---:|
| BLOCKER | 0 | 0 |
| MAJOR | 0 | 0 |
| MINOR | 1 | 1 non-blocking follow-up |
| NIT | 1 | 1 non-blocking follow-up |

**BLOCKER count: 0.**  
**Unresolved MAJOR count: 0.**

**Verdict: PASS / gate-clearable.** All eight mandatory obligations pass. The audit append-at-head concurrency gate may be cleared, while MINOR-01 and NIT-01 remain truthfully recorded as non-blocking follow-ups. This verdict does not satisfy LIVE-07 or LIVE-08 and does not constitute real-Linux proof.
