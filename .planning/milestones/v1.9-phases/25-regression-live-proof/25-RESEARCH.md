# Phase 25: Regression & Live Proof - Research

**Researched:** 2026-07-11
**Domain:** Rust security TCB — audit-DAG acceptance testing, regression-fixture auditing, live Linux verification
**Confidence:** HIGH

## Summary

Phase 25 does not add any new mechanism — Phase 24 already shipped the enforcement (Step 1c,
`expected_role()`, `DenyReason::SlotTypeMismatch`, all `[VERIFIED: crates/executor/src/lib.rs]`
per `24-VERIFICATION.md`). Phase 25's job is entirely **proof**: a held-out acceptance test that
exercises the FULL broker path (not just the in-process executor unit tests Phase 24 already
wrote), a regression audit of pre-existing fixtures, and an independently-run Linux live-suite
green bar. All three requirements (T2-06/07/08) are testing/verification work only — zero
production code is expected to change, though `DESIGN-slot-type-binding.md` §3's stale `body` row
should be doc-synced as a non-blocking cleanup (flagged by `24-03-SUMMARY.md` and
`24-VERIFICATION.md`, not yet done).

Three concrete findings from direct code inspection, load-bearing for planning:

1. **A `Denied` decision is recorded in the audit DAG as a bare `plan_node_evaluated` event with
   no anchors** — `evaluate_plan_node_and_record`'s match (`crates/brokerd/src/server.rs:650-683`)
   has exactly two arms: `BlockedPendingConfirmation` (gets a `sink_blocked` event with anchors)
   and a catch-all `_` (gets a generic `plan_node_evaluated` event, `vec![]` anchors) that fires
   for BOTH `Allowed` and `Denied`. The `SlotTypeMismatch` reason itself is **not persisted in the
   audit DAG event payload** — only the fact that *a* decision was evaluated and chained. T2-06's
   "corresponding audit-DAG event recorded" claim must be read as: a `plan_node_evaluated` event
   is durably appended with the correct causal parent, and `verify_chain` still returns true across
   it — mirroring the existing `clean_path_intent_value_evaluates_to_allowed` test in
   `crates/brokerd/tests/s9_acceptance.rs:281-404`, but asserting `Denied` instead of `Allowed`.

2. **No existing test exercises a genuine subject↔recipient SWAP through the full broker path.**
   Phase 24's `role_mismatch_denies` test (`crates/executor/tests/executor_decision.rs:690-721`)
   proves a `body`-tagged value routed to `to` denies, and `role_none_at_role_checked_slot_denies`
   proves a `None`-role value denies — but both use `ValueStore::mint()` directly (bypassing the
   broker's `mint_from_intent`/audit-DAG/`verify_chain` machinery entirely) and neither swaps two
   otherwise-valid `UserTrusted` handles into each other's slots. T2-06's swapped subject↔recipient
   scenario is a genuinely new test, not a re-assertion of an existing one.

3. **The regression-audit search space (T2-07) is narrower than it first appears.** Every
   `#[cfg(target_os = "linux")]`-gated test file in the workspace was greped for direct
   `.mint(`/`ValueRecord {` construction; **none of the 7 Linux-only e2e test files
   (`s9_live_block.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`,
   `live_acceptance_tainted_session.rs`, `e2e.rs`, `llm_planner_live_accept.rs`, `confirm.rs`)
   construct a `ValueRecord`/`.mint()` fixture directly** — they all drive the real `caprun` binary
   as a subprocess, so their `UserTrusted` values are minted by production `server.rs` code, which
   Phase 24 Plan 01 already fixed to select `primary_role` correctly inside the intent-variant
   match. The audit's real blind spot is therefore **Mac-buildable, in-process Rust tests that call
   `.mint(`/`ValueRecord {` directly** — a space Phase 24 already sic-swept (~40 sites,
   `24-01-SUMMARY.md`) but which Phase 25 must independently re-confirm, not merely trust.

**Primary recommendation:** Write ONE new held-out test in `crates/brokerd/tests/s9_acceptance.rs`
(or a sibling file) that drives `mint_from_intent` twice (once per swapped field) through the real
broker path exactly like `clean_path_intent_value_evaluates_to_allowed`, asserts
`ExecutorDecision::Denied { reason: DenyReason::SlotTypeMismatch { .. } }`, appends/finds the
`plan_node_evaluated` event, and asserts `verify_chain` still true. Independently re-grep the
entire workspace for `.mint(`/`ValueRecord {` sites with `UserTrusted` taint routed into a
role-checked slot with a `None` or mismatched role (T2-07). Run the DEFAULT (unscoped)
`bash scripts/mailpit-verify.sh` with no `MAILPIT_VERIFY_CMD` override for T2-08's milestone-close
gate.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Held-out acceptance test (T2-06) | TCB test layer (`crates/brokerd/tests/`) | Linux live e2e (`cli/caprun/tests/`) | The role-check logic itself is pure Rust, Mac-buildable (no kernel confinement needed to prove it); DESIGN §9 additionally requires a Linux-live confirmation via the real confined worker |
| Regression audit (T2-07) | Database/Storage tier analogue: in-memory `ValueStore` fixture construction across `crates/executor`, `crates/brokerd` test suites | — | Fixtures are test-only state, not runtime persistence — but they are the "stored state" this audit inventories |
| Live re-run (T2-08) | CI/Verification tier (`scripts/mailpit-verify.sh`, Docker/Colima) | — | External environment dependency, not application code |

## Package Legitimacy Audit

Not applicable — Phase 25 adds zero new dependencies. All crates used (`sha2`, `hex`, `uuid`,
`chrono`, `rusqlite`, `serde`) are pre-existing workspace dependencies exercised identically to
`s9_acceptance.rs`'s existing imports. No `Cargo.toml` change is anticipated.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| T2-06 | Held-out acceptance test: swapped subject↔recipient (both UserTrusted) produces the new deny, with a corresponding audit-DAG event and `verify_chain` true | See "T2-06: Held-Out Acceptance Test" below — exact test shape, mint-site calls, DAG assertions, held-out discipline |
| T2-07 | Regression audit: existing tests relying on permissive `UserTrusted`-in-any-slot behavior identified and updated | See "T2-07: Regression Audit" below — search methodology, confirmed-narrow blind spot, what to grep and where |
| T2-08 | `scripts/mailpit-verify.sh` independently re-run green (0 failures), not assumed | See "T2-08: Live Re-Run Discipline" below — exact command, exit-code capture, environment prerequisite |
</phase_requirements>

## Standard Stack

No new libraries. Reuses the exact test-time stack already present in `crates/brokerd/tests/s9_acceptance.rs`:

### Core (existing, reused)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rusqlite` | workspace-pinned | in-memory audit DB (`open_audit_db(":memory:")`) | already the project's sole SQLite binding — `crates/brokerd/src/audit.rs` |
| `sha2` + `hex` | workspace-pinned | `sha256_hex` helper for `literal_sha256` assertions | mirrors `s9_acceptance.rs:29-35` verbatim |
| `uuid` | workspace-pinned | `session_id`, `effect_id` minting | standard across every existing acceptance test |
| `chrono` | workspace-pinned | `Utc::now()` for manually-appended DAG events (if the test appends its own `plan_node_evaluated` event, mirroring `s9_acceptance.rs:358-367`) | only needed if NOT calling `evaluate_plan_node_and_record` directly |

**Installation:** none — no `Cargo.toml` change.

## Architecture Patterns

### System Architecture Diagram

```
   [Test code: T2-06]
        │
        │ 1. mint_from_intent(conn, store, session_id, "boss@company.com", .., Some("subject"))
        │    (SUBJECT-role literal routed as if it were the recipient)
        │ 2. mint_from_intent(conn, store, session_id, "Re: quarterly report", .., Some("recipient"))
        │    (RECIPIENT-role literal routed as if it were the subject)
        ▼
   [ValueStore]  ── both handles are opaque ValueIds; test never re-reads taint/role after mint
        │
        │ 3. Build PlanNode{ sink: "email.send", args: [ {name:"to", value_id: subject_tagged_id},
        │                                                {name:"subject", value_id: recipient_tagged_id} ] }
        ▼
   [executor::submit_plan_node]  (crates/executor/src/lib.rs)
        │  Step 0  schema gate            — passes (well-formed args)
        │  Step 1  resolve handle         — passes (both resolve)
        │  Step 1a/1b structural guards   — pass (non-empty taint/provenance; UserTrusted taint set)
        │  Step 1c  ROLE CHECK (NEW, Phase 24) — "to" expects ["recipient","email_address"], found "subject" → MISMATCH
        ▼
   ExecutorDecision::Denied { reason: DenyReason::SlotTypeMismatch { sink, arg: "to", expected, found: Some("subject") } }
        │
        │ (returned to broker's evaluate_plan_node_and_record, server.rs:550-560)
        ▼
   [Audit DAG]  Event::new(.., event_type: "plan_node_evaluated", anchors: vec![])  — catch-all `_` arm, server.rs:671-682
        │  causal parent = last_event_id (the 2nd intent_received event's hash)
        ▼
   [verify_chain(&conn, session_id)]  — must still return true (hash chain unbroken across the Denied evaluation)
```

Neither I0 (class-deny) nor I2 (untrusted-Block) can fire on this scenario: both values are
minted `UserTrusted` (never tainted), so `is_untrusted()` is false for both and the Steps 2/3
collect-then-Block loop never adds either arg to `blocked`. This is what proves Step 1c
specifically — the deny is unreachable by any pre-Phase-24 code path.

### Recommended Project Structure

No new files required. Two options, in order of preference:

1. **Preferred:** add the held-out test as a new `#[test] fn` inside the EXISTING
   `crates/brokerd/tests/s9_acceptance.rs` — it already imports everything needed
   (`mint_from_intent`, `open_audit_db`, `verify_chain`, `find_event_by_type`, `append_event`) and
   is the canonical home for "§9-style, held-out, full-broker-path" acceptance tests (it already
   hosts `clean_path_intent_value_evaluates_to_allowed`, the closest existing analogue).
2. Alternative: a new file `crates/brokerd/tests/t2_slot_swap_acceptance.rs` if the plan wants a
   visually separate held-out artifact (matches the project's per-milestone acceptance-test-file
   convention, e.g. `durable_anchor.rs`, `extract_provenance_threading.rs`).

For the Linux-live confirmation (DESIGN §9's explicit instruction), add a
`#[cfg(target_os = "linux")]`-gated test to `cli/caprun/tests/s9_live_block.rs` (the existing home
for "block" e2e scenarios) rather than creating a new live test file — it already has the
CLI-driving harness (spawn `caprun`, capture exit code/stdout, inspect the on-disk audit DB
afterward).

### Pattern 1: Full-broker-path held-out test (the T2-06 shape)

**What:** Mint two `UserTrusted` values via `mint_from_intent` with swapped roles, submit a
`PlanNode` routing them into each other's slots, assert `Denied { SlotTypeMismatch }`, then assert
the audit DAG recorded a `plan_node_evaluated` event with `verify_chain` still true.

**When to use:** Any acceptance test that must prove a GENUINE, non-stapled deterministic
decision — mirrors the project's `#1 tripwire` discipline (PLAN.md §9: "If [taint/role] is
stapled on at the sink instead of propagated through the DAG, the demo fails — it proves
nothing"). Applied here to origin_role instead of taint: the test must never construct
`DenyReason::SlotTypeMismatch` directly or call `ValueStore::mint()` bypassing `mint_from_intent`
— it must go through the SAME production call path a real worker/broker exchange uses.

**Example:**
```rust
// Source: mirrors crates/brokerd/tests/s9_acceptance.rs:281-404
// (clean_path_intent_value_evaluates_to_allowed), inverted to prove Denied.
#[test]
fn slot_type_binding_swapped_subject_recipient_denies() {
    let conn = open_audit_db(":memory:").expect("open_audit_db");
    let mut store = ValueStore::default();
    let session_id = Uuid::new_v4();

    // Mint the SUBJECT-role literal but tag it "recipient" via the same
    // mint_from_intent call site server.rs uses for the "to" arm — HELD-OUT
    // means this test calls mint_from_intent directly with a DELIBERATELY
    // swapped role param, standing in for a hypothetical planner bug that
    // routes the wrong ValueId into the wrong PlanArg. This is the exact
    // shape DESIGN §0 names as the T2 gap.
    let (subject_event_id, subject_hash, subject_value_id) = mint_from_intent(
        &conn, &mut store, session_id,
        "Re: quarterly report".to_string(),
        None, None,
        Some("subject".to_string()),   // <- correct role for THIS literal
    ).expect("mint_from_intent (subject) failed");

    let (recipient_event_id, _recipient_hash, recipient_value_id) = mint_from_intent(
        &conn, &mut store, session_id,
        "boss@company.com".to_string(),
        Some(subject_event_id), Some(subject_hash),  // causal chain, not a fabricated parent
        Some("recipient".to_string()), // <- correct role for THIS literal
    ).expect("mint_from_intent (recipient) failed");

    // SWAP: route the subject-tagged handle into "to", and the
    // recipient-tagged handle into "subject" — both otherwise valid,
    // both UserTrusted, neither tainted. Neither I0 nor I2 can fire.
    let plan_node = PlanNode {
        sink: SinkId("email.send".into()),
        args: vec![
            PlanArg { name: "to".into(), value_id: subject_value_id },
            PlanArg { name: "subject".into(), value_id: recipient_value_id },
        ],
    };

    let decision = executor::submit_plan_node(
        session_id, Uuid::new_v4(), &plan_node, &store, &SessionStatus::Active,
    );

    let (sink, arg, expected, found) = match decision {
        ExecutorDecision::Denied { reason: DenyReason::SlotTypeMismatch { sink, arg, expected, found } } =>
            (sink, arg, expected, found),
        other => panic!("expected Denied(SlotTypeMismatch), got {:?}", other),
    };
    assert_eq!(sink, "email.send");
    assert_eq!(arg, "to");
    assert_eq!(expected, vec!["recipient".to_string(), "email_address".to_string()]);
    assert_eq!(found, Some("subject".to_string()));

    // Durably record it exactly as the broker's evaluate_plan_node_and_record
    // does for any non-Blocked decision (server.rs:671-682) — a
    // plan_node_evaluated event, causally parented on the last DAG head.
    let eval_event = runtime_core::Event::new(
        Uuid::new_v4(), Some(recipient_event_id), session_id,
        "executor".into(), "plan_node_evaluated".into(), chrono::Utc::now(), vec![],
    );
    append_event(&conn, &eval_event, Some(&_recipient_hash)).expect("append plan_node_evaluated");

    let eval_evt = find_event_by_type(&conn, &session_id.to_string(), "plan_node_evaluated")
        .expect("find_event_by_type").expect("plan_node_evaluated event must exist");
    assert_eq!(eval_evt.parent_id, Some(recipient_event_id));

    assert!(
        verify_chain(&conn, &session_id.to_string()),
        "verify_chain must return true — the audit DAG hash chain must be unbroken across the Denied evaluation"
    );
}
```

Note: the example above hand-appends the `plan_node_evaluated` event (mirroring how
`clean_path_intent_value_evaluates_to_allowed` does it) because the test drives
`executor::submit_plan_node` directly, not `evaluate_plan_node_and_record` (which is `async` and
lives in `brokerd::server`, a private-ish module — check whether it is `pub(crate)` or exported
before choosing between calling it directly vs. hand-mirroring its recording logic; if it is
reachable, PREFER calling it directly since that removes any risk of the test's hand-rolled
event-append drifting from production behavior).

### Anti-Patterns to Avoid
- **Constructing `DenyReason::SlotTypeMismatch` directly in the test** — this staples the
  assertion instead of proving the production code path produces it. The held-out discipline
  requires driving `mint_from_intent` + `submit_plan_node`, never hand-building the decision.
- **Calling `ValueStore::mint()` directly with a swapped role** instead of `mint_from_intent` —
  this bypasses the broker's audit-DAG recording entirely (no `intent_received` event), which
  defeats T2-06's explicit "corresponding audit-DAG event recorded" requirement. All 4 of Phase
  24's existing Step-1c tests use `store.mint()` directly and are explicitly NOT audit-DAG tests —
  they are unit tests of the executor decision alone. T2-06 needs the broker-level test, which is
  new.
- **Skipping the Linux-live confirmation** — DESIGN §9 explicitly names this: "the held-out
  swapped-recipient↔subject acceptance test must live behind `#[cfg(target_os = "linux")]` and run
  via `scripts/mailpit-verify.sh`, not bare `cargo test` on the Mac dev machine." A Mac-only
  in-process test satisfies the audit-DAG/`verify_chain` proof but does NOT satisfy the "real
  Linux, real confined worker" proof DESIGN §9 asks for. Plan for BOTH layers.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Audit DAG hash-chain verification | A custom re-hash/compare loop in the test | `brokerd::audit::verify_chain(&conn, &session_id_str)` | Already exists, already the sole gate every other acceptance test uses; a custom check would diverge from production semantics |
| Event lookup by type | Raw SQL query in the test | `brokerd::audit::find_event_by_type(&conn, &session_id_str, "plan_node_evaluated")` | Same function every existing acceptance test uses (`s9_acceptance.rs`, `s9_live_block.rs`) |
| SHA-256 digest for assertions | `sha2`/`hex` boilerplate inline | Copy `sha256_hex` helper verbatim from `s9_acceptance.rs:31-35` | Already the project's canonical test helper for this exact computation |

**Key insight:** every primitive T2-06 needs already exists and is exercised by `s9_acceptance.rs`
and `s9_live_block.rs` — this phase is composition of existing, load-bearing helpers into ONE new
scenario, not new infrastructure.

## Runtime State Inventory

Not applicable — Phase 25 is a test/verification-only phase (no rename/refactor/migration). No
production schema, config, or runtime state changes.

## Common Pitfalls

### Pitfall 1: Asserting on the wrong audit-DAG event type
**What goes wrong:** Expecting a `sink_blocked` event (with an `anchors` payload carrying the
`SlotTypeMismatch` reason) instead of the generic `plan_node_evaluated` event that
`evaluate_plan_node_and_record`'s catch-all `_` arm actually produces for a `Denied` decision.
**Why it happens:** `Denied` "feels" like a block, so it's tempting to assume it gets the richer
`sink_blocked` event shape that `BlockedPendingConfirmation` gets.
**How to avoid:** Assert on `plan_node_evaluated` with `anchors: vec![]`, per
`server.rs:650-683`'s literal match arms. If the plan wants the `SlotTypeMismatch` reason itself
durably recorded (richer than today's behavior), that would be a NEW production-code change beyond
Phase 25's stated scope — flag as an Open Question below rather than assuming it exists.

### Pitfall 2: Re-testing what Phase 24 already tested (redundant, not held-out)
**What goes wrong:** Writing a T2-06 test that is functionally identical to
`role_mismatch_denies`/`role_none_at_role_checked_slot_denies` (in-process `store.mint()`,
body→to swap) — this would pass trivially and prove nothing new.
**Why it happens:** Those tests are visible, nearby, and already pass — easy to copy-paste and
declare done.
**How to avoid:** The held-out test MUST (a) go through `mint_from_intent`, not `store.mint()`
directly, (b) exercise a genuine subject↔recipient swap (not body→to), (c) assert the audit-DAG
event + `verify_chain`, none of which the existing 4 Step-1c tests do.

### Pitfall 3: Trusting Phase 24's "~40 fixtures updated" claim without independent re-grep (T2-07)
**What goes wrong:** Marking T2-07 complete because Phase 24's SUMMARY says fixtures were updated
"per the role-assignment discipline."
**Why it happens:** Phase 24's own summary is self-reported (not independently audited) —
`24-01-SUMMARY.md`'s own "Next Phase Readiness" section explicitly asks Phase 25 to confirm this
independently: "Phase 25's regression audit should confirm this holds but should not need to
re-derive roles from scratch."
**How to avoid:** Re-run the searches this research already ran (see "T2-07: Regression Audit"
below) as an independent verification step, not a rubber stamp. Specifically re-confirm the
`grep -c '\.mint(' crates/executor/tests/executor_decision.rs` count (22, of which the newest 4
tests belong to Phase 24 Plan 03) and spot-check a sample of the other 18 for role correctness.

### Pitfall 4: Verification-exit-code-through-a-pipe (recurring project incident)
**What goes wrong:** Piping `mailpit-verify.sh`'s output through `tee`/`grep`/`tail` and checking
`$?`, which reflects the LAST command in the pipe, not the script's actual exit code — this project
has a documented prior incident (`~/.claude/memory/mempalace: "Verification exit code through a
pipe"`) of nearly shipping a false PASS this way at Phase 15.
**Why it happens:** Wanting to filter/summarize a long verification log.
**How to avoid:** Capture `$?` immediately after the bare `bash scripts/mailpit-verify.sh`
invocation, before any pipe. Assert on the script's own terminal line
`"Mailpit-backed Linux verification suite PASSED."` (only printed after a non-error `docker run`
exit under `set -euo pipefail`) AND on named test counts, never on exit-0-through-a-pipe alone.

## Code Examples

### T2-06: Held-Out Acceptance Test — full mechanics

See "Pattern 1" above for the primary code shape. Two additional things the plan should pin
before writing tasks:

1. **`mint_from_intent`'s exact signature** (confirmed live,
   `crates/brokerd/src/quarantine.rs:435-442` per DESIGN §9, threaded with the new role param per
   Phase 24 Plan 01):
   ```rust
   // Source: crates/brokerd/tests/s9_acceptance.rs:297-306 (verbatim existing call site)
   pub fn mint_from_intent(
       conn: &Connection,
       store: &mut ValueStore,
       session_id: Uuid,
       literal: String,
       parent_id: Option<Uuid>,
       parent_hash: Option<String>,
       origin_role: Option<String>,   // <- the Phase-24-added 7th param
   ) -> anyhow::Result<(Uuid /* event_id */, String /* hash */, ValueId)>
   ```
2. **Causal chaining across two `mint_from_intent` calls in the same test.** The 2nd call's
   `parent_id`/`parent_hash` must be the 1st call's returned `(event_id, hash)` — NOT `None, None`
   — otherwise the two `intent_received` events are siblings rather than a genuine chain, and
   `verify_chain` would still pass (it tolerates forked DAGs) but the test would not demonstrate a
   REALISTIC single-session two-field-mint sequence (mirrors how `server.rs`'s real
   `SendEmailSummary` dispatch chains its 3 sequential `mint_from_intent` calls,
   per `24-VERIFICATION.md`'s reference to `server.rs:1300-1313`/`:1337`).

### T2-07: Regression Audit — search methodology, already run once this session

The following greps were run directly against the live workspace during this research session
(not hypothetical) and should be RE-RUN by the plan's executor as its own independent T2-07 step
(not merely cited from here):

```bash
# 1. Every Linux-only-gated test file in the workspace:
find crates cli -name '*.rs' -not -path '*/target/*' \
  | xargs grep -l 'cfg(target_os = "linux")'
# 21 files found this session (2026-07-11).

# 2. Of those, which construct a ValueRecord/.mint() fixture directly (the actual blind spot):
for f in $(find crates cli -name '*.rs' -not -path '*/target/*' \
    | xargs grep -l 'cfg(target_os = "linux")'); do
  grep -q '\.mint(\|ValueRecord {' "$f" && echo "$f"
done
# 0 files found this session — every Linux-only test is an e2e CLI-driver, none mints directly.

# 3. Mac-buildable direct-mint sites (the real regression-audit search space):
grep -rn '\.mint(\|ValueRecord {' crates/ --include='*.rs' | grep -v '/target/'
# Cross-reference each hit's routed PlanArg name against sink_sensitivity.rs's expected_role
# table; confirm every UserTrusted mint into a role-checked slot ("to"/"cc"/"bcc"/"subject"/"path")
# carries a role in that slot's expected list, and every mint into an unconstrained slot
# ("contents", or a non-role-checked arg) is unaffected either way.
```

**Confirmed this session:** `crates/executor/tests/executor_decision.rs` has 22 `.mint(` call
sites (16 pre-existing + Phase 24 fixed, 6 new/Phase-24-Plan-03-added for Step 1c's own tests).
`crates/brokerd/src/quarantine.rs`'s own test module, `s9_acceptance.rs`, `durable_anchor.rs`, and
`file_create.rs` were the other files Phase 24 Plan 01 touched (`24-01-SUMMARY.md`'s file list).
T2-07 should independently spot-check a sample from each of these files (not just trust the
summary), and explicitly document "0 found" if the re-grep confirms no gap — per this project's
own Runtime-State-Inventory discipline ("leaving it blank is not acceptable").

### T2-08: Live Re-Run Discipline

```bash
# The DEFAULT, UNSCOPED recipe — no MAILPIT_VERIFY_CMD override. Per the project's
# milestone-close discipline (scoped runs have masked real bugs before — see
# scripts/mailpit-verify.sh's own module doc comment on the v1.4 caprun-planner
# build-artifact-placement bug, only caught by a full unscoped re-run).
bash scripts/mailpit-verify.sh
RESULT=$?   # capture immediately, before any pipe/tee/grep post-processing
echo "mailpit-verify.sh exit code: $RESULT"
```

Colima + Docker were CONFIRMED running and reachable during this research session
(`colima status` reported "colima is running using macOS Virtualization.Framework"; `docker info`
succeeded). This is a human/environment prerequisite the plan should still name explicitly (per
the phase description's own note) since it can be stopped between research and execution.

The default `MAILPIT_VERIFY_CMD` is `cargo build --workspace && cargo test --workspace
--no-fail-fast` (script's own default, `scripts/mailpit-verify.sh:91`) — this is what "full
workspace regression" means for T2-08, and matches the phase's Success Criterion 3 verbatim
("independently re-run green (0 failures)").

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `UserTrusted` value accepted into any slot regardless of semantic origin | `UserTrusted` value must carry a role matching its slot's expected-role list (Step 1c) | Phase 24 (2026-07-12) | Any pre-existing test asserting `Allowed` for a role-mismatched `UserTrusted` value at a role-checked slot now `Denied`s — Phase 24 already fixed the ~40 known sites; T2-07 independently re-confirms no site was missed |
| DESIGN §3's `body` row pinned as `Some(["body"])` | Corrected to `Some(["body", "doc_fragment"])` (Phase 24 Plan 03 deviation) | 2026-07-12, during Phase 24 execution | The DESIGN doc's §3 table text still reads the STALE `Some(["body"])` — a documentation-only gap (code is correct, doc is not) flagged by both `24-03-SUMMARY.md` and `24-VERIFICATION.md` as unresolved. Phase 25 should fold in a doc-sync line item (non-blocking, not a T2-06/07/08 requirement, but cheap to fix while the context is loaded) |

**Deprecated/outdated:** The `role_mismatch_denies`/`role_none_at_role_checked_slot_denies` unit
tests are NOT deprecated — they remain the correct, fast, in-process coverage for the mechanism
itself. T2-06 supplements them with the missing full-broker-path + audit-DAG layer; it does not
replace them.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `evaluate_plan_node_and_record` (or an equivalently-visible helper) is reachable/callable from `crates/brokerd/tests/` for a test that wants the EXACT production recording behavior rather than a hand-mirrored `Event::new(..)` append | Pattern 1 code example | If not reachable (e.g. it is a private `fn` in `server.rs` with no `pub(crate)`/test-exposed wrapper), the held-out test must hand-mirror the append (as the existing `clean_path_intent_value_evaluates_to_allowed` test already does) — functionally equivalent but a small extra verification burden to keep the mirror faithful. Plan should verify visibility before locking task text. |
| A2 | The Linux-live confirmation (DESIGN §9's `#[cfg(target_os = "linux")]` instruction) can be satisfied by extending `cli/caprun/tests/s9_live_block.rs` with a new scenario, without requiring a new CLI subcommand/planner-scripting surface | Pattern: Recommended Project Structure | If the existing e2e harness cannot express "route recipient-tagged literal into subject slot" via its current CLI-arg/scripted-plan surface (all existing e2e tests use `caprun`'s built-in scripted plan construction, not raw PlanNode JSON), the plan may need a small, scoped CLI test-hook addition. This is plausible but not confirmed — the plan should re-check `cli/caprun/src/main.rs`'s plan-construction path before committing to "no production code changes." |
| A3 | No production code changes will be needed beyond an optional DESIGN §3 doc-sync (the `body` row) | Summary | If the T2-06 test's Linux-live layer requires a new scripted-plan CLI surface (per A2), that would be a small, scoped, test-support-only production change — not a security-relevant TCB change, but still code, and should be flagged to the user rather than silently added. |

## Open Questions

1. **Does the audit DAG need to durably record the `SlotTypeMismatch` reason itself, or is a bare
   `plan_node_evaluated` event sufficient to satisfy T2-06's "corresponding audit-DAG event
   recorded" language?**
   - What we know: today's code (`server.rs:671-682`) records only a generic
     `plan_node_evaluated` event with `anchors: vec![]` for ANY non-`BlockedPendingConfirmation`
     decision (`Allowed` and `Denied` alike) — the specific deny reason lives only in the
     in-memory `ExecutorDecision` returned over IPC, not in the SQLite-persisted event.
   - What's unclear: whether the phase's success criterion ("a corresponding audit-DAG event
     recorded") is satisfied by proving THIS event exists and chains correctly (a proof of
     "the broker durably logged that an evaluation happened, non-repudiably"), or whether it
     implies a richer record (e.g. a new `DenyReason`-carrying variant analogous to
     `sink_blocked`).
   - Recommendation: read T2-06's wording literally — "a corresponding audit-DAG event recorded"
     — and satisfy it with the existing `plan_node_evaluated` event + `verify_chain` proof (no new
     production code). If the plan-review or discuss-phase step surfaces a stronger reading (e.g.
     the user wants a dedicated audit trail entry naming WHICH slot mismatched, for operator
     forensics), that is a legitimate scope question to raise explicitly rather than assume either
     way — it would be new production code, which the phase's stated goal ("Regression & Live
     Proof") does not obviously license.

2. **Is `evaluate_plan_node_and_record` visible enough to call from `crates/brokerd/tests/`?**
   - What we know: it is an `async fn` in `crates/brokerd/src/server.rs` (not `pub` per the
     `fn evaluate_plan_node_and_record(` signature read this session — no `pub` keyword visible at
     `server.rs:550`).
   - What's unclear: whether `crates/brokerd/tests/` integration tests can reach a crate-private
     `fn` (they generally cannot — integration tests only see the crate's public API) — meaning
     the held-out test likely CANNOT call it directly and must hand-mirror the DAG-recording logic,
     exactly as `clean_path_intent_value_evaluates_to_allowed` already does.
   - Recommendation: default to the hand-mirrored `Event::new(..)` + `append_event(..)` pattern
     (Pattern 1's code example already does this) — it is the SAME pattern the closest existing
     analogue test already uses, so it is low-risk and precedented, not a new technique.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Colima | T2-08 (Linux verification container host) | Yes (confirmed running) | macOS Virtualization.Framework | none needed |
| Docker | T2-08 (`rust:1` + `axllent/mailpit` containers) | Yes (confirmed via `docker info`) | Docker Engine — Community, client 29.6.1 | none needed |
| `scripts/mailpit-verify.sh` | T2-08 | Yes (present, executable, unmodified by this phase) | — | — |

**Missing dependencies with no fallback:** none — environment fully available as of this research
session (2026-07-11). Re-verify at execution time; Colima/Docker can be stopped between sessions.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `cargo test`, workspace-wide (no external test framework) |
| Config file | none — `Cargo.toml` per-crate `[dev-dependencies]`; `scripts/check-invariants.sh` is a supplementary grep-based architectural gate, not a test framework |
| Quick run command | `cargo test -p brokerd --test s9_acceptance` (Mac-buildable, seconds) |
| Full suite command | `bash scripts/mailpit-verify.sh` (Linux-live, ~1-3 min incl. container startup) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| T2-06 | Swapped subject↔recipient UserTrusted plan node denies with audit-DAG event + verify_chain | integration (Mac) | `cargo test -p brokerd --test s9_acceptance slot_type_binding_swapped_subject_recipient_denies` | ❌ Wave 0 (new test to write) |
| T2-06 | Same scenario proven on real Linux (kernel-confined worker, DESIGN §9) | live/e2e (Linux-only) | `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test s9_live_block <new_test_name>' bash scripts/mailpit-verify.sh` (scoped run for iteration; unscoped default for the final T2-08 gate) | ❌ Wave 0 (new test to write) |
| T2-07 | No existing test silently bypasses the new role check via an unassigned/mismatched role | other (grep-based regression audit + spot-check re-run) | `grep -rn '\.mint(\|ValueRecord {' crates/ --include='*.rs'` cross-referenced against `sink_sensitivity.rs`'s `expected_role` table, plus `cargo test --workspace --no-fail-fast` (0 unexpected failures) | ✅ existing files, audit is a manual/scripted cross-reference, not a new test file |
| T2-08 | Full workspace regression green on real Linux, independently re-run, not assumed | integration (Linux-only, full suite) | `bash scripts/mailpit-verify.sh` (bare, no `MAILPIT_VERIFY_CMD` override) | ✅ script exists, always re-run fresh (never cached) |

### Sampling Rate
- **Per task commit:** `cargo test -p brokerd --test s9_acceptance` (or the specific new test name) — Mac-buildable, fast feedback for T2-06's logic.
- **Per wave merge:** `cargo test --workspace --no-fail-fast` (Mac) to catch any T2-07 regression before the expensive Linux run.
- **Phase gate:** `bash scripts/mailpit-verify.sh` (bare, default `MAILPIT_VERIFY_CMD`) — full green required before `/gsd-verify-work`, per T2-08's own wording ("not assumed from a prior pass").

### Wave 0 Gaps
- [ ] The new T2-06 held-out test (`crates/brokerd/tests/s9_acceptance.rs`, new `#[test] fn`) — does not exist yet.
- [ ] The new T2-06 Linux-live confirmation test (`cli/caprun/tests/s9_live_block.rs`, new `#[cfg(target_os = "linux")] #[test] fn`) — does not exist yet; contingent on resolving Open Question 2 / Assumption A2 (whether the existing e2e harness's scripted-plan surface can express the swap without new production code).
- [ ] No new framework/config install needed — `cargo test` and `scripts/mailpit-verify.sh` are both already fully wired.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Not in scope — no auth surface touched |
| V3 Session Management | No | `SessionStatus`/`SessionId` untouched by this phase (Step 1c is per-arg, session-status-independent per DESIGN §6 Landmines note) |
| V4 Access Control | Yes (indirectly) | The role-check IS an access-control mechanism (which literal may occupy which sink-arg slot) — Phase 25 proves it, does not build it. Standard control: the hardcoded `expected_role()` table in `crates/executor/src/sink_sensitivity.rs`, never a config file (mirrors `sink_sensitivity.rs`'s existing CONTENT-01/02 discipline) |
| V5 Input Validation | Yes | `validate_schema` (Step 0) + Step 1c together form the input-validation layer for `PlanNode` args; Phase 25 adds no new validation, only proof |
| V6 Cryptography | No | Not touched — `sha256_hex`/audit-DAG hashing is pre-existing, unchanged this phase |

### Known Threat Patterns for {stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Confused-deputy / slot misrouting (a planner routes a legitimately-trusted value into the wrong semantic slot — the v1.4 T2 residual this whole milestone closes) | Spoofing / Tampering | Step 1c's hardcoded `expected_role()` table + fail-closed `None`/mismatch → `Denied` (Phase 24, PROVEN by Phase 25) |
| Test-suite laundering (a regression test silently accepts a role-mismatched fixture because it was never updated, masking a real bypass) | Repudiation (of the security proof itself) | T2-07's independent regression audit — this is exactly what T2-07 defends against |
| False-green live verification (asserting exit-0 through a pipe, or trusting a prior/cached Linux run) | Repudiation | T2-08's explicit "not assumed from a prior pass" discipline; capture `$?` before any pipe (Pitfall 4 above) |

## Sources

### Primary (HIGH confidence)
- `planning-docs/DESIGN-slot-type-binding.md` (locked design, §3/§4/§6/§7/§9, incl. the Phase-24 body/doc_fragment amendment) — read in full this session
- `.planning/REQUIREMENTS.md` (T2-06/07/08 exact wording) — read in full this session
- `.planning/phases/24-slot-type-binding-enforcement/24-01-SUMMARY.md`, `24-02-SUMMARY.md`, `24-03-SUMMARY.md`, `24-VERIFICATION.md` — read in full this session
- `crates/brokerd/tests/s9_acceptance.rs` (the canonical held-out-test template) — read in full this session
- `crates/brokerd/src/server.rs:533-830` (`evaluate_plan_node_and_record`, the audit-DAG recording logic) — read directly this session
- `crates/executor/tests/executor_decision.rs:32-50,690-822` (existing Step 1c tests, `email_send_with_to` helper) — read directly this session
- `scripts/mailpit-verify.sh` — read in full this session
- `planning-docs/PLAN.md:150-220` (§9 acceptance-test discipline, held-out/genuine-taint-chain non-negotiable) — read directly this session
- Direct `Bash` verification: `colima status`, `docker info` (both confirmed available this session); workspace-wide greps for `cfg(target_os = "linux")` + `.mint(`/`ValueRecord {` (T2-07 blind-spot analysis, 21 Linux-gated files found, 0 with direct mint construction)

### Secondary (MEDIUM confidence)
- None — this phase required no external web research; all grounding is direct project-code and project-doc inspection.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies, 100% reuse of existing, already-verified test helpers
- Architecture: HIGH — every claim traced to a specific file:line read this session, not inferred
- Pitfalls: HIGH — Pitfall 4 (exit-code-through-pipe) is a documented prior project incident, not speculation; Pitfalls 1-3 derived directly from reading `server.rs`'s actual match arms and Phase 24's actual test file

**Research date:** 2026-07-11
**Valid until:** 14 days (fast-moving — this is the final phase of an active milestone; re-verify file:line citations if Phase 25 execution begins more than a few days after this research, per the project's own stated grounding-decay convention in `DESIGN-slot-type-binding.md`'s header)

## RESEARCH COMPLETE
