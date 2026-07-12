# Phase 25: Regression & Live Proof - Pattern Map

**Mapped:** 2026-07-11
**Files analyzed:** 3 (1 new test fn in an existing file, 1 new test fn in an existing Linux-gated file, 0 new production files — plus a regression-audit target set and a doc-sync line item)
**Analogs found:** 3 / 3

Phase 25 adds zero new files. All "file classification" rows below are **new `#[test] fn`s inside existing test files**, or read-only audit targets. No production code files are created or modified (confirmed by RESEARCH.md — zero new mechanism, proof-only phase).

## File Classification

| New/Modified Location | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/brokerd/tests/s9_acceptance.rs` (new `#[test] fn slot_type_binding_swapped_subject_recipient_denies`) | test (held-out acceptance) | request-response (mint → submit_plan_node → audit-DAG append → verify) | `clean_path_intent_value_evaluates_to_allowed` in the same file, lines 281-404 | exact — same file, same helper set, same shape, inverted assertion |
| `cli/caprun/tests/s9_live_block.rs` (new `#[cfg(target_os = "linux")] #[test] fn`) | test (Linux-live e2e) | event-driven (CLI subprocess drives real confined worker; asserts exit code + on-disk audit DB) | existing "block" scenarios already in that file (CLI-driving harness) | role-match — same file, same harness pattern; contingent on Assumption A2 (scripted-plan surface expressiveness) |
| T2-07 regression-audit targets (no new file — read/verify existing fixtures) | test (fixture audit, no code change expected) | batch (grep + spot-check cross-reference) | `crates/executor/tests/executor_decision.rs` (22 `.mint(` sites) + `crates/brokerd/src/quarantine.rs` test module + `s9_acceptance.rs` + `durable_anchor.rs` + `file_create.rs` | exact — these ARE the audit's search space, not analogs to imitate |
| `scripts/mailpit-verify.sh` (T2-08 — invoked, not modified) | config/CI script | batch (containerized full-workspace test run) | itself (existing, unmodified) | exact — reused verbatim |
| `planning-docs/DESIGN-slot-type-binding.md` §3 body row (doc-sync only, non-blocking) | doc | — | N/A | — |

## Pattern Assignments

### `crates/brokerd/tests/s9_acceptance.rs` — new fn `slot_type_binding_swapped_subject_recipient_denies` (test, held-out acceptance)

**Analog:** `clean_path_intent_value_evaluates_to_allowed`, same file, lines 281-404 (verified live 2026-07-11; RESEARCH.md's own line citation 281-404 confirmed exact by direct read).

**Imports pattern** (file top, lines 20-27 — already present, no new imports needed):
```rust
use brokerd::approval::build_confirmation_prompt;
use brokerd::audit::{find_event_by_type, open_audit_db, verify_chain};
use brokerd::quarantine::{extract_email_claims, mint_from_intent, mint_from_read};
use executor::value_store::ValueStore;
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId, TaintLabel};
use runtime_core::{ExecutorDecision, SessionStatus};
use sha2::{Digest, Sha256};
use uuid::Uuid;
```
The new test additionally needs `use brokerd::audit::append_event;` and `use runtime_core::Event;` — already used locally inside `clean_path_intent_value_evaluates_to_allowed` (lines 355-356), not at file scope; same local-`use` idiom should be copied, not hoisted to file top (keep the diff surgical).

**Core pattern — genuine-mint setup** (lines 296-306, verbatim source of the ONLY sanctioned mint call shape):
```rust
let recipient = "boss@company.com";
let (intent_event_id, intent_hash, intent_value_id) = mint_from_intent(
    &conn, &mut store, session_id,
    recipient.to_string(),
    None, None,
    Some("recipient".to_string()),
).expect("mint_from_intent failed");
```
For the new test, call `mint_from_intent` TWICE with genuine causal chaining (2nd call's `parent_id`/`parent_hash` = 1st call's returned `(event_id, hash)` — see RESEARCH.md Code Examples §2), swap the roles so each literal is tagged with the OTHER slot's role, then route them into each other's `PlanArg` slots (RESEARCH.md Pattern 1 has the full worked example — reuse verbatim).

**Decision-assertion pattern** (lines 336-348, inverted for `Denied`):
```rust
let decision = executor::submit_plan_node(
    session_id, Uuid::new_v4(), &plan_node, &store, &SessionStatus::Active,
);
assert!(
    matches!(decision, ExecutorDecision::Allowed),
    "UserTrusted-only provenance must evaluate to Allowed (HARD-02), got {:?}", decision
);
```
New test replaces this with a `match` on `ExecutorDecision::Denied { reason: DenyReason::SlotTypeMismatch { sink, arg, expected, found } }` — see Phase 24's `role_mismatch_denies` (`crates/executor/tests/executor_decision.rs:689-717`, confirmed by direct read) for the exact match-arm shape to borrow, NOT for the mint-path (that test bypasses the DAG via `store.mint()` directly and is explicitly insufficient for T2-06 — see "Contrast" below).

**Audit-DAG append pattern — MANDATORY hand-mirror, not a production call** (lines 350-367, verbatim):
```rust
use brokerd::audit::append_event;
use runtime_core::Event;

let eval_event = Event::new(
    uuid::Uuid::new_v4(),
    Some(intent_event_id),   // causal parent = last mint's event id, never a fabricated UUID
    session_id,
    "executor".into(),
    "plan_node_evaluated".into(),
    chrono::Utc::now(),
    vec![],
);
append_event(&conn, &eval_event, Some(&intent_hash)).expect("append plan_node_evaluated");
```
**Confirmed live** (`crates/brokerd/src/server.rs:670-682`): `evaluate_plan_node_and_record` is `async fn` at line 550, no `pub` keyword — unreachable from `crates/brokerd/tests/` (integration tests only see the crate's public API). The hand-mirror above is therefore not a shortcut but the ONLY option; it must be kept byte-for-byte faithful to the production `_` catch-all arm shown below.

**Production arm this mirrors** (`crates/brokerd/src/server.rs:670-682`, confirmed by direct read this session):
```rust
_ => (
    Event::new(
        Uuid::new_v4(),
        Some(*last_event_id), // causal parent preserved — not None
        session_id,
        "executor".into(),
        "plan_node_evaluated".into(),
        Utc::now(),
        vec![],
    ),
    Vec::new(),
),
```
This is the catch-all that fires for BOTH `Allowed` and `Denied` (only `BlockedPendingConfirmation` gets the richer `sink_blocked` event at lines 649-666). `DenyReason::SlotTypeMismatch` is NOT persisted in the event payload — only the fact of evaluation. Assert on `plan_node_evaluated` + `anchors: vec![]`, never `sink_blocked`.

**Verify-chain pattern** (lines 371-403):
```rust
let eval_evt = find_event_by_type(&conn, &session_id.to_string(), "plan_node_evaluated")
    .expect("find_event_by_type")
    .expect("plan_node_evaluated event must be present in the audit DAG");
assert_eq!(eval_evt.parent_id, Some(intent_event_id), "...causally parented...");
assert!(verify_chain(&conn, &session_id.to_string()), "verify_chain must return true...");
```

**Contrast — insufficient prior coverage (do NOT copy the mint-path from these):** `role_mismatch_denies` and `role_none_at_role_checked_slot_denies`, `crates/executor/tests/executor_decision.rs:689-749` (confirmed by direct read). Both call `store.mint(...)` directly — no `intent_received` event, no audit-DAG participation, no `verify_chain` call, body→to swap only (not a genuine subject↔recipient swap). Useful ONLY for the `Denied`/`DenyReason::SlotTypeMismatch` match-arm shape; the mint path is the exact anti-pattern T2-06 must avoid (RESEARCH.md Anti-Patterns section).

---

### `cli/caprun/tests/s9_live_block.rs` — new `#[cfg(target_os = "linux")]` test (Linux-live e2e)

**Analog:** existing "block" scenarios in the same file (CLI-driving harness: spawn `caprun`, capture exit code/stdout, inspect on-disk audit DB afterward — per RESEARCH.md Recommended Project Structure). No new file — extend this one.

**Open dependency (Assumption A2, RESEARCH.md):** verify `cli/caprun/src/main.rs`'s scripted-plan construction path can express "route recipient-tagged literal into subject slot" via its existing CLI-arg surface before locking task text. If it cannot, a small scoped CLI test-hook addition may be needed (test-support-only, not a TCB change) — flag to the user rather than silently adding.

---

### T2-07 Regression Audit (no code pattern — a search-and-verify procedure)

**Target file set (Mac-buildable, direct `.mint(`/`ValueRecord {` construction — the actual blind spot per RESEARCH.md):**
- `crates/executor/tests/executor_decision.rs` — 22 `.mint(` sites (16 pre-existing + 6 Phase-24-added Step-1c tests)
- `crates/brokerd/src/quarantine.rs` — own test module
- `crates/brokerd/tests/s9_acceptance.rs`
- `crates/brokerd/tests/durable_anchor.rs`
- `crates/brokerd/tests/file_create.rs`

**Confirmed 0 direct-mint sites** in the 21 `#[cfg(target_os = "linux")]`-gated Linux-only test files — all drive the real `caprun` binary as a subprocess (production-minted values only). This narrows T2-07 entirely to the 5 files above.

**Re-run commands** (from RESEARCH.md, execute independently, do not trust the prior session's counts):
```bash
find crates cli -name '*.rs' -not -path '*/target/*' | xargs grep -l 'cfg(target_os = "linux")'
grep -rn '\.mint(\|ValueRecord {' crates/ --include='*.rs' | grep -v '/target/'
```
Cross-reference each hit's routed `PlanArg` name against `sink_sensitivity.rs`'s `expected_role` table.

---

### T2-08 Live Re-Run (`scripts/mailpit-verify.sh`, invoked verbatim, not modified)

**Invocation contract** (confirmed live, `scripts/mailpit-verify.sh:1-40,187`):
```bash
bash scripts/mailpit-verify.sh
RESULT=$?   # capture immediately, before any pipe/tee/grep post-processing
echo "mailpit-verify.sh exit code: $RESULT"
```
- Default `MAILPIT_VERIFY_CMD` (line ~34): `cargo build --workspace && cargo test --workspace --no-fail-fast` — do NOT override for the T2-08 phase-gate run (unscoped only masks bugs; scoped runs are for iteration only).
- Success sentinel (line 187): the literal terminal line `"Mailpit-backed Linux verification suite PASSED."`, only printed after a non-error `docker run` exit under `set -euo pipefail`.
- **Never pipe this command through `tee`/`grep`/`tail` when checking `$?`** — documented prior project incident (`~/.claude/memory/verification-exit-code-through-pipe.md`), assert on named counts + the sentinel line, never on exit-0-through-a-pipe alone.

## Shared Patterns

### Audit-DAG hash-chain verification
**Source:** `brokerd::audit::verify_chain(&conn, &session_id_str)`
**Apply to:** T2-06's held-out test — never hand-roll a re-hash/compare loop.

### Event lookup by type
**Source:** `brokerd::audit::find_event_by_type(&conn, &session_id_str, "plan_node_evaluated")`
**Apply to:** T2-06's held-out test, same helper `clean_path_intent_value_evaluates_to_allowed` and `s9_live_block.rs` already use.

### Genuine-mint discipline (never staple)
**Source:** `mint_from_intent` (`crates/brokerd/src/quarantine.rs`), signature confirmed at `crates/brokerd/tests/s9_acceptance.rs:297-306`:
```rust
pub fn mint_from_intent(
    conn: &Connection, store: &mut ValueStore, session_id: Uuid,
    literal: String, parent_id: Option<Uuid>, parent_hash: Option<String>,
    origin_role: Option<String>,
) -> anyhow::Result<(Uuid, String, ValueId)>
```
**Apply to:** T2-06 only. Never `ValueStore::mint()` directly, never construct `DenyReason::SlotTypeMismatch` by hand.

### Verification exit-code-through-pipe discipline
**Source:** project incident memory + `scripts/mailpit-verify.sh:187`
**Apply to:** T2-08 exclusively — capture `$?` before any pipe, assert on the named PASSED sentinel.

## No Analog Found

None — all three requirements (T2-06, T2-07, T2-08) have exact or role-match analogs already in the codebase; this phase is composition, not new-mechanism design (per RESEARCH.md's own framing).

## Metadata

**Analog search scope:** `crates/brokerd/tests/`, `crates/executor/tests/`, `crates/brokerd/src/server.rs`, `crates/brokerd/src/quarantine.rs`, `cli/caprun/tests/`, `scripts/mailpit-verify.sh`
**Files scanned:** 6 directly read/grepped this session (in addition to RESEARCH.md's own prior-session reads, independently re-confirmed)
**Pattern extraction date:** 2026-07-11

## PATTERN MAPPING COMPLETE
