---
phase: 51-non-hybrid-live-proof-v1-10-done
reviewed: 2026-08-09T03:23:11Z
depth: deep
files_reviewed: 5
files_reviewed_list:
  - cli/caprun/tests/live_acceptance_v1_10_cli.rs
  - crates/brokerd/src/audit.rs
  - crates/brokerd/src/confirmation.rs
  - crates/brokerd/src/server.rs
  - crates/brokerd/tests/audit_chain_fork_regression.rs
findings:
  critical: 1
  warning: 0
  info: 0
  total: 1
status: issues_found
---

# Phase 51: Code Review Report

**Reviewed:** 2026-08-09T03:23:11Z
**Depth:** deep
**Files Reviewed:** 5
**Status:** issues_found

## Summary

The final Phase 51 source/test delta was reviewed across Plans 51-01 through 51-09, including the append serialization fix at `442a056`, the LIVE-08 oracle repairs at `171bdc0` and `54bca8e`, all plan/summary artifacts, the independent adversarial trace, retained LIVE evidence, and subsequent git history.

The append-at-durable-head change and the corrected anchor-first LIVE-08 oracle pass their focused regressions. The retained LIVE log hashes match `51-LIVE-EVIDENCE.md`, and `git diff cb34b91..HEAD` is empty for every reviewed source/test file, so there was no post-proof source mutation. However, the grant writer still commits the authorization row separately from its required audit event. A recoverable append failure can therefore leave an active, unaudited `github.pr` capability. This is a security/audit-integrity blocker.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: GitHub authorization can commit without its audit event

**Classification:** BLOCKER

**File:** `/home/ben/Workspace/caprun/crates/brokerd/src/audit.rs:542-564`

**Issue:** `record_github_grant` inserts `session_grants` while the connection is in autocommit mode, then calls `append_event` as a second transaction. If the append cannot start or commit (for example, SQLite contention exceeds the five-second busy timeout, disk-full, or an I/O fault), the function returns an error but the authorization row is already durable. A retry executes `INSERT OR IGNORE`, receives zero affected rows, skips `github_grant_authorized`, and returns success. `has_github_grant` checks only the row, so subsequent `github.pr` dispatch accepts a capability for which the tamper-evident audit chain has no authorization event. The Phase 51 append serialization fix makes each individual append atomic, but does not make this capability mutation atomic with its audit record.

**Fix:** Put the grant row insertion and conditional event append in one `IMMEDIATE` transaction, committing only after `append_event` succeeds. Because `append_event` detects the enclosing transaction, it will append at the locked durable head without opening a nested transaction. Add a fault/lock regression proving an append failure rolls back `session_grants`, and a retry produces exactly one grant row and exactly one `github_grant_authorized` event.

```rust
let tx = rusqlite::Transaction::new_unchecked(
    conn,
    rusqlite::TransactionBehavior::Immediate,
)?;
let inserted = tx.execute(
    "INSERT OR IGNORE INTO session_grants (session_id, grant_type, granted_at) \
     VALUES (?1, ?2, ?3)",
    rusqlite::params![session_id, "github.pr", chrono::Utc::now().to_rfc3339()],
)?;
if inserted == 1 {
    // construct event, then append_event(&tx, key, &event, ...)?
}
tx.commit()?;
```

## Recorded Non-blocking Comment Defects

These are already documented in `51-ADVERSARIAL-TRACE.md` and are not counted as new actionable findings:

- `server.rs:1044-1049` overstates atomicity for the sink-blocked event plus later literal/checkpoint writes. Ordering is mutex-protected and failures remain fail-closed, but those writes are not one database transaction.
- `audit.rs:1033-1037` still says there are 19 production append sites although the Phase 51 inventory established 45.

## Verification and Evidence Consistency

- `cargo test -p brokerd --test audit_chain_fork_regression -- --test-threads=1`: 2 passed.
- `cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca live_08_attribution_is_independent_of_exit_event_order -- --exact --test-threads=1`: 1 passed.
- Retained hashes match the evidence record: scoped `dc57e49...b98d`, full `4bcb275b...ee3e`, and prior failure `9b78bd4f...b98f`.
- The executed proof revision is `cb34b91`. Later commits through `2460f7f` are documentation-only for the reviewed source/test scope; `git diff cb34b91..HEAD` is empty for all five reviewed files.
- The earlier review's proof-selector, provenance-oracle, and environment-hermeticity warnings are resolved in the current implementation: the selector is feature-gated, subprocess commands begin with `env_clear`, and LIVE-08 attributes through the durable anchor's `read_event_id` with actor/cardinality checks.

---

_Reviewed: 2026-08-09T03:23:11Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
