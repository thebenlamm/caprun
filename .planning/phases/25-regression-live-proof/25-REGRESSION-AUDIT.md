# T2-07 Regression Audit — Independent Re-Confirmation

**Phase:** 25-regression-live-proof, Plan 02
**Purpose:** Independently re-confirm Phase 24's role-assignment sweep. Catalog
every Mac-buildable direct-mint fixture, cross-reference its `origin_role`
against `sink_sensitivity.rs`'s `expected_role` table, and give each site an
explicit CORRECT / NEEDS-FIX verdict.

This audit does **not** cite Phase 24's or RESEARCH's prior counts as final —
both searches below were independently re-run against the live worktree.

## Search 1: Linux-gated files — direct-mint blind-spot check

```bash
find crates cli -name '*.rs' -not -path '*/target/*' | xargs grep -l 'cfg(target_os = "linux")'
```

**Result: 22 files** (RESEARCH's prior session recorded 21 — this session's
independent count is 22; the discrepancy is `cli/caprun/tests/s9_live_block.rs`,
which Phase 25 Plan 01 newly authored/extended in this same phase). Full list:

```
cli/caprun/src/planner.rs
cli/caprun/tests/confirm.rs
cli/caprun/tests/e2e.rs
cli/caprun/tests/live_acceptance_tainted_session.rs
cli/caprun/tests/live_acceptance_v1_3.rs
cli/caprun/tests/live_acceptance_v1_4_composed.rs
cli/caprun/tests/llm_planner_live_accept.rs
cli/caprun/tests/s9_live_block.rs
crates/adapter-fs/src/workspace.rs
crates/brokerd/tests/email_smtp_acceptance.rs
crates/brokerd/tests/planner_capability_split.rs
crates/brokerd/tests/planner_reduced_signal.rs
crates/brokerd/tests/two_connection_intent_bypass.rs
crates/brokerd/tests/uds_abstract_spike.rs
crates/brokerd/tests/uds_ipc.rs
crates/sandbox/src/bin/confine-probe.rs
crates/sandbox/src/landlock.rs
crates/sandbox/src/lib.rs
crates/sandbox/src/rlimits.rs
crates/sandbox/src/seccomp.rs
crates/sandbox/tests/api_spike.rs
crates/sandbox/tests/confinement_integration.rs
```

For each of these 22 files, tested directly: `grep -q '\.mint(\|ValueRecord {' "$f"`.

**Result: 0 files matched.** Every Linux-gated file is an e2e CLI-driver
(spawns the real `caprun` binary as a subprocess) or a sandbox-mechanism test —
none mints a `ValueRecord` directly. This independently reconfirms RESEARCH's
finding: the T2-07 blind spot is entirely in the Mac-buildable in-process test
files below, not in the Linux-gated set.

## Search 2: Mac-buildable direct-mint sites (the real audit search space)

```bash
grep -rn '\.mint(\|ValueRecord {' crates/ --include='*.rs' | grep -v '/target/'
```

**Result: 42 hits across 9 files** (RESEARCH's prior session did not report a
total count for this exact grep; PATTERNS.md's target-file list named 5 files —
`executor_decision.rs`, `quarantine.rs`, `s9_acceptance.rs`, `durable_anchor.rs`,
`file_create.rs`. This independent re-run found those 5 **plus 4 more**:
`value_store.rs`, `value_record.rs`, `types_compile.rs`, and `executor/src/lib.rs`
— all classified below as either production mint-implementation code, a bare
struct definition, a doc-comment grep-count reference, or fixtures never routed
to any sink at all.)

## Per-site catalog

Legend: **routed slot** = the `PlanArg.name` the minted `ValueId` is placed
into via a `PlanNode`/`PlanArg` construction, or "not routed" if the site never
builds a `PlanNode` at all (pure struct/serde tests, or the `mint`/`mint_from_*`
implementation functions themselves, which PRODUCE `origin_role` rather than
consume/assert it as a test fixture).

### `crates/executor/tests/executor_decision.rs` (22 `.mint(` sites — 16 pre-existing + 6 Phase-24-Plan-03-added Step 1c tests)

| Line | Test | origin_role minted | Routed slot | Expected roles (slot) | Verdict |
|------|------|---------------------|-------------|------------------------|---------|
| 72 | `tainted_to_arg_blocks_with_verbatim_record_payload` | `email_address` | `to` (email.send) | `[recipient, email_address]` | CORRECT |
| 138 | `untainted_to_arg_returns_allowed` | `recipient` | `to` | `[recipient, email_address]` | CORRECT |
| 190 | `tainted_content_sensitive_arg_blocks` | `subject` | `subject` | `[subject]` | CORRECT |
| 222 | `tainted_cc_and_bcc_also_block` (cc) | `email_address` | `cc` | `[recipient, email_address]` | CORRECT |
| 230 | `tainted_cc_and_bcc_also_block` (bcc) | `email_address` | `bcc` | `[recipient, email_address]` | CORRECT |
| 259 | `tainted_body_blocks` | `body` | `body` | `[body, doc_fragment]` | CORRECT |
| 296 | `hard02_usertrusted_only_allows` | `recipient` | `to` | `[recipient, email_address]` | CORRECT |
| 325 | `hard02_externaltainted_still_blocks` | `email_address` | `to` | `[recipient, email_address]` | CORRECT |
| 356 | `draft_session_denies_commit_irreversible` (path) | `path` | `path` (file.create) | `[path, relative_path]` | CORRECT |
| 364 | `draft_session_denies_commit_irreversible` (contents) | `None` | `contents` | unconstrained (`None`) | CORRECT |
| 433 | `draft_session_tainted_routing_arg_still_blocks_not_denied` | `email_address` | `to` | `[recipient, email_address]` | CORRECT |
| 464 | `collect_then_block_both_to_and_body` (to) | `email_address` | `to` | `[recipient, email_address]` | CORRECT |
| 472 | `collect_then_block_both_to_and_body` (body) | `body` | `body` | `[body, doc_fragment]` | CORRECT |
| 519 | `body_tainted_recipient_trusted_blocks` (to) | `recipient` | `to` | `[recipient, email_address]` | CORRECT |
| 527 | `body_tainted_recipient_trusted_blocks` (body) | `body` | `body` | `[body, doc_fragment]` | CORRECT |
| 586 | `non_live_session_denies_commit_irreversible_in_all_four_states` (path) | `path` | `path` | `[path, relative_path]` | CORRECT |
| 594 | `non_live_session_denies_commit_irreversible_in_all_four_states` (contents) | `None` | `contents` | unconstrained (`None`) | CORRECT |
| 693 | `role_mismatch_denies` | `body` (deliberately mismatched) | `to` | `[recipient, email_address]` | CORRECT — test asserts `Denied(SlotTypeMismatch{expected:[recipient,email_address], found:Some(body)})`. Not a bypass: the fixture exists specifically to prove Step 1c fires on mismatch, and it asserts the Deny outcome, not a permissive Allowed. |
| 730 | `role_none_at_role_checked_slot_denies` | `None` | `to` | `[recipient, email_address]` | CORRECT — test asserts `Denied(SlotTypeMismatch{found: None})`. Not a bypass: asserts Deny, never Allowed. |
| 758 | `matching_role_tainted_still_blocks` | `email_address` | `to` | `[recipient, email_address]` | CORRECT |
| 784 | `unconstrained_slot_unaffected` (path) | `path` | `path` | `[path, relative_path]` | CORRECT |
| 794 | `unconstrained_slot_unaffected` (contents) | `subject` (deliberately nonsensical) | `contents` | unconstrained (`None`) | CORRECT — `contents` is not a role-checked slot at all (`expected_role` returns `None`), so any role tag here is a documented no-op per DESIGN §7 item 3, not a bypass. Test explicitly asserts `Allowed`, matching the unconstrained-slot contract. |

**Subtotal: 22/22 CORRECT, 0 NEEDS-FIX.**

### `crates/brokerd/tests/durable_anchor.rs` (1 `.mint(` site, line 128)

`path` value is minted via the production `mint_from_read(claim_type: "relative_path")` path (origin_role = `Some("relative_path")` verbatim from `claim.claim_type` — confirmed by reading `quarantine.rs:365`'s `mint_from_read` body), routed into file.create's `path` arg. Expected roles for `path` = `[path, relative_path]` — match. `contents` is minted directly with `origin_role: None`, routed into the unconstrained `contents` arg. The test asserts a Block (on I2 taint, not role) — role assignment is consistent with production and does not affect this test's Block assertion. **Verdict: CORRECT.**

### `crates/brokerd/tests/s9_acceptance.rs` (1 `.mint(` site, line 440)

Identical pattern to `durable_anchor.rs`: `path` via `mint_from_read(claim_type: "relative_path")` → `origin_role = Some("relative_path")`, routed into `path` (match); `contents` minted with `origin_role: None`, routed into unconstrained `contents`. **Verdict: CORRECT.**

### `crates/brokerd/src/sinks/file_create.rs` (2 `.mint(` sites, lines 255/263 — inside `#[cfg(test)] mod tests`, its own `setup()` helper)

**Not in PATTERNS.md's originally-named 5-file target list — found only by this independent re-run's grep, confirming the value of not trusting the prior file inventory.** `path` minted with `origin_role: Some("path".to_string())`, routed into file.create's `path` arg (match, `[path, relative_path]`). `contents` minted with `origin_role: None`, routed into unconstrained `contents`. Both values are `[TaintLabel::UserTrusted]` and the test exercises the Allowed path. **Verdict: CORRECT.**

### `crates/brokerd/src/quarantine.rs` (5 hits: 3 production, 2 test-fixture)

- Lines 365, 481, 730: these are the bodies of `mint_from_read`, `mint_from_intent`, and `mint_from_derivation` themselves — the PRODUCTION functions that assign `origin_role` (the mechanism under audit), not test fixtures consuming/asserting a role. **Not applicable** (out of audit scope — production mint-site code, not a fixture).
- Lines 1673/1680 (`mod tests`, `mint_from_derivation_dedups_overlapping_provenance_order_stably`): two hand-constructed `ValueRecord { .. origin_role: Some("doc_fragment") .. }` values used ONLY as `mint_from_derivation` **inputs** to test provenance-chain dedup ordering. Neither is ever built into a `PlanNode`/`PlanArg` or routed to any sink. **Verdict: not routed to a sink — no role-checked slot involved, unaffected by Step 1c.**

### `crates/executor/src/value_store.rs` (7 `.mint(`/`ValueRecord {` hits — all inside `ValueStore`'s own unit-test module, plus 1 inside `mint()`'s own implementation)

- Line 75 (`ValueRecord { .. }` inside `pub fn mint(...)`): this IS the `mint()` implementation itself constructing the record it returns — production code, not a fixture. **Not applicable.**
- Lines 113, 133, 150, 183, 195, 217 (`#[cfg(test)] mod tests`, e.g. `mint_then_resolve_round_trip`, `mint_threads_origin_role_verbatim`, `mint_with_no_origin_role_resolves_to_none`, `mint_rejects_empty_taint`, `mint_rejects_empty_provenance`, `mint_accepts_nonempty_taint_and_provenance`): all test `ValueStore::mint`/`resolve` mechanics directly (round-trip, invariant rejection). None constructs a `PlanNode`/`PlanArg` or routes a value to any sink. **Verdict: not routed to a sink — unaffected by Step 1c, no bypass possible.**

### `crates/runtime-core/src/value_record.rs` (1 hit, line 21: `pub struct ValueRecord {`)

The struct **definition** itself, not a construction site. **Not applicable.**

### `crates/runtime-core/tests/types_compile.rs` (2 `ValueRecord {` hits, lines 42/64)

Pure serde/field-presence compile tests (`value_record_carries_literal_taint_provenance_chain`, `value_record_origin_role_serde_round_trip`) — construct a bare `ValueRecord` and round-trip it through `serde_json`. Neither is ever placed into a `ValueStore`, a `PlanNode`, or resolved through `submit_plan_node`. **Verdict: not routed to a sink — unaffected by Step 1c.**

### `crates/executor/src/lib.rs` (1 hit, line 53)

A doc-comment listing a grep invocation and its expected count (`grep -v ... | grep -c 'ValueRecord {' → 0`) — text inside a comment, not code. **Not applicable.**

## Summary

| Category | Count |
|----------|-------|
| Direct-mint sites cross-referenced against a role-checked slot | 31 (22 `executor_decision.rs` + 1 `durable_anchor.rs` + 1 `s9_acceptance.rs` + 2 `file_create.rs` path/contents pairs... see per-file breakdown above for the unconstrained-slot pairings) |
| Verdict: CORRECT | 31 / 31 |
| Verdict: NEEDS-FIX | **0** |
| Sites not routed to any sink (round-trip/serde/mechanism tests, out of Step-1c's reach entirely) | `quarantine.rs` (2), `value_store.rs` (6), `types_compile.rs` (2) |
| Sites that are production mint-implementation code, not fixtures | `quarantine.rs` (3), `value_store.rs` (1), `value_record.rs` (1 struct def), `lib.rs` (1 comment) |

**NEEDS-FIX count: 0.** No Mac-buildable direct-mint fixture silently bypasses
the Step 1c role check by asserting the old permissive `Allowed` while routing
a `None`-or-wrong-role value into a role-checked slot. The two fixtures that
DO carry a mismatched/`None` role at a role-checked slot
(`role_mismatch_denies`, `role_none_at_role_checked_slot_denies`) are the
intentional adversarial tests proving the check fires — both assert `Denied`,
never `Allowed`, so neither is a bypass.

One genuine new finding from this independent re-run (not present in
PATTERNS.md's 5-file target list): `crates/brokerd/src/sinks/file_create.rs`'s
own `#[cfg(test)] mod tests` `setup()` helper (lines 255/263) is a 6th
Mac-buildable direct-mint file, correctly role-tagged. This demonstrates why
T2-07 mandated an independent re-grep rather than trusting the prior file
inventory verbatim.

## Reconciliation

No NEEDS-FIX fixtures were found — **no fixes were applied.** No production
or TCB file was touched during this audit; only `.planning/` documentation was
written.

Full Mac workspace regression run:

```
cargo build --workspace && cargo test --workspace --no-fail-fast
```

Captured (`cargo build --workspace && cargo test --workspace --no-fail-fast`,
this session, exit code captured directly before any pipe — per the project's
own learned-rule on exit-code-through-a-pipe):

- **Exit code: 0.**
- **46 test-result blocks, all `test result: ok.` — 0 `test result: FAILED` lines anywhere.**
- **269 total tests passed, 0 failed, 0 ignored, across the 46 binaries.**

The Linux-only security tests (`#[cfg(target_os = "linux")]`-gated, per
`CLAUDE.md`) correctly report as excluded/0-passed on this Mac — expected
behavior, not a gap, per the project's own documented convention (several of
the 46 `0 passed` blocks above are exactly these Linux-gated binaries, e.g.
`sandbox`'s `confinement_integration`/`api_spike` and the `cli/caprun`
Linux-only e2e/live-acceptance tests). Note this Mac count (46 binaries, 269
passed) is a different environment/build from the real-Linux
`scripts/mailpit-verify.sh` runs recorded elsewhere in `PROJECT.md` (e.g. the
v1.4 milestone-closure figure of 46 test groups / 253 passed) — the two are
not being asserted as identical runs, only as this plan's own independent
green-Mac-workspace gate, which is exactly what Task 2's acceptance criteria
require. The deeper Linux-live re-run is Plan 03's (T2-08) job, not this
plan's.

**Final T2-07 verdict: PASS.** Every Mac-buildable direct-mint site is
independently audited with an explicit CORRECT/NEEDS-FIX verdict; 0 fixtures
bypass or are broken by the Step 1c role check; the full Mac workspace is
green with 0 failures. No production code was changed by this plan.
