---
phase: 15-deterministic-doc-action-extraction
plan: 01
subsystem: security
tags: [taint-tracking, provenance, audit-dag, rust-tcb, value-store]

requires:
  - phase: 14-content-sensitive-sink-arg-blocking
    provides: "Collect-then-Block executor loop (Vec<BlockedArg>), plural sink-arg blocking"
provides:
  - "mint_from_derivation: the provenance-threading, fail-closed derived-value mint (closes the milestone's #1 laundering BLOCKER for transform-derived values)"
  - "doc_fragment raw claim type + looks_like_doc_fragment shape predicate + extract_doc_fragments/concat_doc_fragments confined-worker-callable helpers"
  - "Event's four skip_serializing_if derivation-payload fields + Event::derivation constructor (durable DAG derivation edge, no DB migration)"
  - "check-invariants.sh Gate 3: mint-call-site restriction (DETECTION backstop)"
affects: [15-02-extract-provenance-threading-proof, 15-03, 15-04, 17-acceptance]

tech-stack:
  added: []
  patterns:
    - "Provenance-threading mint: a transform-derived ValueRecord's provenance_chain is the deduplicated, order-stable concatenation of its inputs' own read-rooted chains -- never a fresh transform-local root"
    - "Byte-descent verification: reconstruct join(input_literals, '@') and compare against the worker's claimed transformed_literal, fail-closed on mismatch -- turns metadata-descent into byte-descent without adding a parser over raw hostile bytes"
    - "Every-element (not just [0]) file_read-root guard, gating on the union taint's untrustedness"
    - "Mint-call-site grep gate as DETECTION, explicitly documented as non-load-bearing backstop distinct from the Result-returning invariant PREVENTION"

key-files:
  created: []
  modified:
    - crates/brokerd/src/quarantine.rs
    - crates/runtime-core/src/event.rs
    - scripts/check-invariants.sh

key-decisions:
  - "mint_from_derivation mints the ValueRecord BEFORE constructing the derivation Event (reverse of mint_from_read's append-then-mint order) because the event's hashed payload embeds derived_value_id == the minted value_id, which cannot be known before ValueStore::mint returns it."
  - "Event::derivation sets actor = \"confined-reader\" (mirroring mint_from_read's file_read actor), since a derivation event records the confined worker's transform action, not a broker-only action."
  - "check-invariants.sh Gate 3 targets literal call-site syntax (mint_from_read(, mint_from_derivation(, .mint() rather than bare-word token mentions, so it does not need to annotate the many pre-existing prose mentions of 'mint_from_read' in doc comments across proto.rs/worker.rs/planner.rs/event.rs/server.rs."
  - "Gate 3 exempts any file under a tests/ directory and any line at/after a file's own #[cfg(test)] marker -- required because several pre-Phase-15 test files already call mint_from_read/ValueStore::mint directly as legitimate test infrastructure (see Deviations)."

patterns-established:
  - "TransformKind tag as a plain String field (transform_kind: Option<String>) rather than a new Rust enum -- \"concat\" is the only tag Phase 15 defines; a future phase adding a non-'@' join MUST introduce its own distinct tag, never reuse \"concat\" for a different separator (see recipient-scoped-separator note below)."

requirements-completed: [EXTRACT-01, EXTRACT-03]

coverage:
  - id: D1
    description: "doc_fragment raw claim type + looks_like_doc_fragment + extract_doc_fragments + concat_doc_fragments helpers, hand-rolled and dependency-free; mint_from_read's additive doc_fragment arm fails closed on an assembled ('@'-containing) recipient"
    requirement: "EXTRACT-01"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/quarantine.rs#looks_like_doc_fragment_accepts_plain_tokens_rejects_assembled_recipient, extract_doc_fragments_finds_marker_anchored_fragments_in_order, concat_doc_fragments_joins_with_at_separator, mint_from_read_doc_fragment_valid_fragment_mints_external_untrusted, mint_from_read_doc_fragment_rejects_assembled_recipient"
        status: pass
    human_judgment: false
  - id: D2
    description: "mint_from_derivation threads inputs' provenance_chain (no re-anchor), unions taint plus unconditional WorkerExtracted, drops UserTrusted under an untrusted union, enforces the every-element file_read-root guard (index 0 and index>0), byte-verifies transformed_literal == join(input_literals,'@') (MAJOR-1), fails closed on zero inputs and on an all-UserTrusted input set, never demotes the session"
    requirement: "EXTRACT-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/quarantine.rs#mint_from_derivation_threads_provenance_and_taint, mint_from_derivation_no_re_anchor, mint_from_derivation_drops_user_trusted_when_union_untrusted, mint_from_derivation_rejects_non_file_read_root_at_index_0, mint_from_derivation_rejects_non_file_read_root_at_index_gt_0, mint_from_derivation_rejects_all_user_trusted_inputs, mint_from_derivation_concat_byte_verify_rejects_mismatch, mint_from_derivation_dedups_overlapping_provenance_order_stably, mint_from_derivation_zero_inputs_fails_closed, mint_from_derivation_does_not_demote_session"
        status: pass
    human_judgment: false
  - id: D3
    description: "Event carries the four skip_serializing_if derivation-payload fields (derived_value_id, input_value_ids, input_provenance_chains, transform_kind) + Event::derivation constructor; the golden byte-fixture proves pre-derivation events stay byte-identical (no DB migration)"
    verification:
      - kind: unit
        ref: "crates/runtime-core/src/event.rs#anchors_empty_event_serializes_byte_identical_and_round_trips, derivation_event_payload_round_trips"
        status: pass
    human_judgment: false
  - id: D4
    description: "check-invariants.sh Gate 3 (mint-call-site DETECTION backstop) restricts mint_from_read(/mint_from_derivation(/.mint( to the sanctioned loci, with a verified negative control"
    verification:
      - kind: other
        ref: "./scripts/check-invariants.sh (full run, PASS); hand-tested negative control: a scratch store.mint(...) call in a third file fails the gate, confirmed then removed"
        status: pass
    human_judgment: false

duration: ~75min
completed: 2026-07-08
status: complete
---

# Phase 15 Plan 01: Provenance-Threading Derived Mint Summary

**`mint_from_derivation` closes the milestone's #1 laundering BLOCKER: a transform-derived value's provenance_chain now threads its inputs' own read-rooted chains (never a fresh transform-local root), with five mint-time guards including a byte-verified `join(input_literals, '@')` check against the worker's claim.**

## Performance

- **Duration:** ~75 min
- **Started:** 2026-07-08T~12:25Z
- **Completed:** 2026-07-08T13:44:44Z
- **Tasks:** 3 (2 TDD, 1 auto)
- **Files modified:** 3 (`crates/brokerd/src/quarantine.rs`, `crates/runtime-core/src/event.rs`, `scripts/check-invariants.sh`)

## Accomplishments

- **`mint_from_derivation`** — the `mint_from_read` successor for transform-derived values. Threads every input's provenance chain (dedup, order-stable); unions taint plus an unconditionally-appended `WorkerExtracted`; drops `UserTrusted` when the union is untrusted (always true, given `WorkerExtracted`); enforces an EVERY-element (not just `[0]`) file_read-root guard when the union is untrusted; byte-verifies the worker's claimed `transformed_literal` against `join(input_literals, '@')` for the `"concat"` transform (MAJOR-1); fails closed on zero inputs; appends a durable `derivation` audit event; never demotes the session.
- **`doc_fragment` claim type + extraction helpers** — `looks_like_doc_fragment` (crisp shape predicate rejecting any `'@'`-containing token), `extract_doc_fragments` (hand-rolled `Reply-To:`/`Domain:` marker-anchored scanner, no regex), `concat_doc_fragments` (plain `'@'` join, no parsing). `mint_from_read` gains one additive `doc_fragment` claim_type arm, fail-closed guarded by `looks_like_doc_fragment` (finding #1a — the concat transform's own output can never re-enter as a fresh single-element chain).
- **`Event` derivation-payload fields** — `derived_value_id`, `input_value_ids`, `input_provenance_chains`, `transform_kind`, each `#[serde(default, skip_serializing_if)]` (no DB migration; golden byte-fixture still passes byte-identical). `Event::derivation` constructor (sibling of `Event::sink_blocked`).
- **`check-invariants.sh` Gate 3** — a DETECTION-only mechanical backstop restricting `mint_from_read(`/`mint_from_derivation(`/`.mint(` call sites to the sanctioned loci, explicitly documented as non-load-bearing (the load-bearing PREVENTION is `ValueStore::mint`'s Result-returning invariant + `mint_from_derivation`'s own guards).

## Task Commits

1. **Task 1 RED — failing doc_fragment tests** — `e0f0cce` (test)
2. **Task 1 GREEN — doc_fragment claim type + helpers** — `8ed7ba7` (feat)
3. **Task 2 RED — Event derivation payload + failing mint_from_derivation tests** — `cc5c667` (test)
4. **Task 2 GREEN — mint_from_derivation implementation** — `39b1cb1` (feat)
5. **Task 3 — check-invariants.sh mint-call-site gate** — `ee2324f` (chore)

_TDD tasks (1, 2) each produced a RED (compile-failure) commit followed by a GREEN (passing) commit, per the plan's `tdd="true"` requirement._

## Files Created/Modified

- `crates/brokerd/src/quarantine.rs` — `looks_like_doc_fragment`, `extract_doc_fragments`, `concat_doc_fragments`, the `doc_fragment` arm in `mint_from_read`, `mint_from_derivation`, `resolve_event_type_by_id` (private inline session-scoped lookup, mirroring `find_event_by_type`'s shape but by exact id), plus 6 new unit tests for Task 1 and 10 new unit tests for Task 2.
- `crates/runtime-core/src/event.rs` — four new `Event` fields, `Event::derivation` constructor, updated `Event::new`/`Event::sink_blocked` to set the new fields to empty defaults, one new payload round-trip test.
- `scripts/check-invariants.sh` — new Gate 3 (mint-call-site restriction), plus a `pipefail` fix discovered while testing the gate.

## Decisions Made

- **Mint-before-append ordering in `mint_from_derivation`**: since the derivation Event's hashed payload embeds `derived_value_id == the minted value_id`, and `ValueStore::mint` generates the `ValueId` internally, the value must be minted first, then the event constructed with that id, then appended. This is the reverse of `mint_from_read`'s append-then-mint order but is forced by the payload requirement, not a stylistic choice.
- **`Event::derivation`'s `actor` field is `"confined-reader"`**, mirroring `mint_from_read`'s `file_read` actor, since the derivation event records the confined worker's transform action (not specified explicitly in the plan text; a reasonable, low-risk implementation detail filling a silent gap).
- **Unknown `transform_kind` values fail closed** (`match transform_kind { "concat" => ..., other => Err(...) }`), mirroring `mint_from_read`'s existing unknown-claim_type fail-closed discipline. The plan's tests didn't explicitly require this, but it's the established codebase pattern for any typed-tag dispatch and costs nothing.
- **`resolve_event_type_by_id`** is a new private (non-exported) helper in `quarantine.rs`, not added to `audit.rs` — per the plan's own read_first note ("do NOT depend on Plan 02's `find_event_by_id` since this is Wave 1"), and consistent with the plan's `files_modified` frontmatter, which does not list `audit.rs`.

**SUMMARY note (opus round-4, non-blocking, verbatim per Task 2's `<done>` instruction):** `TransformKind::Concat`'s `'@'` separator (implemented here as the string tag `"concat"` on `Event.transform_kind` — no separate Rust enum exists yet) is RECIPIENT-SCOPED — a Phase-15 artifact of assembling an email address specifically, not a generic join-any-strings primitive. Phase 16+ MUST NOT reuse the `"concat"` tag for a non-`'@'` join (e.g. a differently-delimited field) without introducing a new, distinct transform tag with its own separator, or `mint_from_derivation`'s byte-verify guard will false-reject a legitimate derivation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Extended check-invariants.sh Gate 3 with test-directory and `#[cfg(test)]` exemptions beyond the three literally-named allowed files**

- **Found during:** Task 3
- **Issue:** The plan's Task 3 action text names exactly three allowed loci for `mint_from_read`/`mint_from_derivation`/`.mint(`: `quarantine.rs`, `server.rs`, and (`.mint(` only) `value_store.rs`. Grepping the real tree showed this would immediately break the build: five pre-Phase-15 test files call `mint_from_read` directly as legitimate integration-test infrastructure (`crates/brokerd/tests/durable_anchor.rs`, `s9_acceptance.rs`, `phase5_dispatch.rs`; `cli/caprun/tests/live_acceptance_tainted_session.rs`, `s9_live_block.rs` reference it in assertions/comments), and `crates/executor/tests/executor_decision.rs` plus `crates/brokerd/src/sinks/file_create.rs`'s own `#[cfg(test)]` module call `ValueStore::mint` (`.mint(`) directly for test fixture construction — none of these are in the plan's three-file allow-list.
- **Fix:** Extended Gate 3 with two exemptions applied uniformly to all three tokens: (a) any file whose path contains `/tests/` (a Cargo integration-test binary, compiled only for `cargo test`, never part of the shipped binary), and (b) any line at/after a file's own `#[cfg(test)]` marker (this codebase's convention places exactly one such unit-test module last in the file). The task's own action text already anticipated exactly this class of problem for the `.mint(` token specifically ("exclude `#[cfg(test)]`/test-module lines for the `.mint(` token to avoid flagging legitimate executor unit tests") — I extended the same reasoning to all three tokens for consistency, since the underlying rationale (test-only code that never ships is not "a new module bypassing the mint discipline") applies identically to `mint_from_read`/`mint_from_derivation`.
- **Why this doesn't weaken the security control:** Gate 3 is explicitly labeled DETECTION, not PREVENTION, in both the plan text and the gate's own comment — it is a mechanical backstop over the load-bearing `ValueStore::mint` invariant + `mint_from_derivation`'s own guards (built and tested in Task 2). Exempting test-only compilation units (never part of the production binary) does not create any bypass of the production mint discipline; it only prevents the DETECTION-only gate from false-flagging pre-existing, unrelated-to-Phase-15 test code.
- **Files modified:** `scripts/check-invariants.sh`
- **Verification:** `./scripts/check-invariants.sh` passes on the full tree; hand-tested the negative control (a `store.mint(...)` call added to a scratch third-party file under `crates/brokerd/src/`, confirmed to fail the gate with a specific `FAIL` line, then removed).
- **Committed in:** `ee2324f` (Task 3 commit)

**2. [Rule 3 - Blocking] Fixed a `pipefail` bug discovered while implementing Gate 3**

- **Found during:** Task 3 (testing the negative control)
- **Issue:** `grep -n '#\[cfg(test)]' "$file" | head -1 | cut -d: -f1` aborts the entire script under `set -euo pipefail` when `$file` has no `#[cfg(test)]` marker at all — `grep`'s no-match exit code (1) propagates as the pipeline's exit status even though `head`/`cut` both succeed.
- **Fix:** Added `|| true` to the pipeline.
- **Files modified:** `scripts/check-invariants.sh` (same commit as above)
- **Verification:** Re-ran the gate against the real tree and the negative-control scratch file; both produced correct PASS/FAIL output without the script aborting early.
- **Committed in:** `ee2324f`

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking issues in Task 3's check-invariants.sh gate implementation; no deviations in Tasks 1 or 2).
**Impact on plan:** Both fixes are confined to the DETECTION-only mechanical backstop (Task 3); neither touches the load-bearing security guards built in Task 2 (`mint_from_derivation`'s five mint-time guards), any DESIGN doc, or any MUST in the plan text. No scope creep — the exemptions only prevent false positives against pre-existing, unrelated test infrastructure.

## Issues Encountered

None beyond the two documented deviations above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- `mint_from_derivation` is ready for Plan 02 (the DB-alone EXTRACT-02 unbroken-edge + anti-staple proof) to consume: it appends a durable `derivation` event whose hashed payload (`derived_value_id`, `input_value_ids`, `input_provenance_chains`, `transform_kind`) is exactly what Plan 02's audit walk needs to verify genuine multi-input descent.
- `doc_fragment` + `extract_doc_fragments`/`concat_doc_fragments` are ready for Plan 03/04 to wire into the confined worker (`cli/caprun/src/worker.rs`) and the planner (`cli/caprun/src/planner.rs`'s `subject`/`body` PlanArgs) — not done in this plan, consistent with this plan's `files_modified` scope (`quarantine.rs` + `event.rs` + `check-invariants.sh` only).
- No blockers. `cargo test --workspace --no-fail-fast` (34/34 test-result blocks `ok`) and `./scripts/check-invariants.sh` (all 3 gates PASS) are both green at this plan's completion.

---
*Phase: 15-deterministic-doc-action-extraction*
*Completed: 2026-07-08*

## Self-Check: PASSED

All claimed files found on disk (`crates/brokerd/src/quarantine.rs`, `crates/runtime-core/src/event.rs`, `scripts/check-invariants.sh`, this SUMMARY.md); all 5 claimed commit hashes (`e0f0cce`, `8ed7ba7`, `cc5c667`, `39b1cb1`, `ee2324f`) verified present in `git log --oneline --all`.
