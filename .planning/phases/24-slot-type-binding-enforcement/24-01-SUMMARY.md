---
phase: 24-slot-type-binding-enforcement
plan: 01
subsystem: security
tags: [rust, tcb, taint-tracking, value-record, mint-sites, serde]

# Dependency graph
requires:
  - phase: 23-slot-type-binding-design-gate
    provides: DESIGN-slot-type-binding.md (locked mechanism, role vocabulary, ordering ruling) — the authoritative spec this plan mechanically realizes
provides:
  - "ValueRecord.origin_role: Option<String> field with #[serde(default)]"
  - "ValueStore::mint's 4th parameter, threaded into the sole production ValueRecord constructor"
  - "origin_role threading through mint_from_read (claim_type reuse), mint_from_intent (caller-supplied), mint_from_derivation (transform_kind-derived, 2-input arity-guarded)"
  - "server.rs primary_role selection inside the :1294 intent-variant match, never hardcoded at the shared mint_from_intent call"
  - "All ~40 compilation-forced test-fixture call sites updated with role-assignment-discipline-correct origin_role values"
affects: [24-02-slot-type-binding-executor, 24-03-slot-type-binding-enforcement-wiring, 25-regression-and-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Additive, orthogonal tag field on a broker-owned record (mirrors taint/provenance_chain — never folded into the trust-classification TaintLabel enum)"
    - "Role derived from a transform's own verified output shape, never inherited/unioned from derivation inputs (anti-laundering discipline)"
    - "Role selected inside the producing match arm, never hardcoded at a shared downstream call site reachable by multiple variants"

key-files:
  created: []
  modified:
    - crates/runtime-core/src/value_record.rs
    - crates/executor/src/value_store.rs
    - crates/runtime-core/tests/types_compile.rs
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/sinks/file_create.rs
    - crates/brokerd/tests/s9_acceptance.rs
    - crates/brokerd/tests/durable_anchor.rs
    - crates/executor/tests/executor_decision.rs

key-decisions:
  - "origin_role is Option<String> with #[serde(default)], never bare String — None must be representable as a state distinct from every valid role tag (Wave-2 fail-closed default keys off this bit, per DESIGN §1/F6)"
  - "mint_from_derivation's concat arm guards inputs.len() == 2 before assigning \"recipient\" — the local@domain shape only holds for the 2-input case; any other arity gets None, with I2 as the backstop (DESIGN §4/F2)"
  - "server.rs's primary_role is selected INSIDE the :1294 intent-variant match (same arm as primary_literal), never hardcoded at the shared :1317 mint_from_intent call — avoids mistagging a file.create path as \"recipient\" (DESIGN §2/F3)"
  - "Test-fixture role assignment followed the discipline of preserving each fixture's EXISTING assertion for the future Wave-2 gate: UserTrusted values routed to role-checked slots (to/path) got the matching role (recipient/path); untrusted doc-extracted-shaped values got the untrusted vocabulary (email_address); unconstrained slots (contents) got None"

requirements-completed: [T2-02]

coverage:
  - id: D1
    description: "ValueRecord gains origin_role: Option<String> with #[serde(default)], round-trips through serde, and pre-field JSON still deserializes to None"
    requirement: T2-02
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/types_compile.rs#value_record_origin_role_serde_round_trip"
        status: pass
      - kind: unit
        ref: "crates/runtime-core/tests/types_compile.rs#value_record_origin_role_defaults_to_none_for_pre_field_json"
        status: pass
    human_judgment: false
  - id: D2
    description: "ValueStore::mint threads a 4th origin_role parameter verbatim into the minted record"
    requirement: T2-02
    verification:
      - kind: unit
        ref: "crates/executor/src/value_store.rs#mint_threads_origin_role_verbatim"
        status: pass
      - kind: unit
        ref: "crates/executor/src/value_store.rs#mint_with_no_origin_role_resolves_to_none"
        status: pass
    human_judgment: false
  - id: D3
    description: "mint_from_read reuses claim.claim_type verbatim; mint_from_intent takes a caller-supplied role; mint_from_derivation derives role from transform_kind with a 2-input arity guard and never reads inputs[*].origin_role"
    requirement: T2-02
    verification:
      - kind: unit
        ref: "cargo test -p brokerd --lib quarantine (44 mint_from_* fixtures, all passing)"
        status: pass
      - kind: other
        ref: "grep -n 'inputs\\[' crates/brokerd/src/quarantine.rs — no read of inputs[*].origin_role"
        status: pass
    human_judgment: false
  - id: D4
    description: "server.rs selects primary_role inside the intent-variant match (recipient for SendEmailSummary, path for CreateFileFromReport), never hardcoded at the shared mint_from_intent call site"
    requirement: T2-02
    verification:
      - kind: other
        ref: "grep -n '\"recipient\"|\"path\"' crates/brokerd/src/server.rs (:1294 match region)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Full Mac workspace builds and tests green after all ~40 compilation-forced test-fixture call sites are updated; no test weakened, no #[ignore] added"
    requirement: T2-02
    verification:
      - kind: integration
        ref: "cargo build --workspace"
        status: pass
      - kind: integration
        ref: "cargo test --workspace --no-fail-fast"
        status: pass
      - kind: other
        ref: "./scripts/check-invariants.sh (Gates 1/2/3)"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-07-12
status: complete
---

# Phase 24 Plan 01: Origin-Role Tag Threading Summary

**Additive `origin_role: Option<String>` tag threaded through `ValueRecord`, `ValueStore::mint`, all three broker `mint_from_*` wrappers, and every `server.rs` dispatch site — no enforcement added, I0/I1 trust classification unchanged, full Mac workspace green.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 3
- **Files modified:** 9 (3 production-only: value_record.rs, value_store.rs, quarantine.rs+server.rs share; 6 test/fixture files touched for the compilation-forced mass update)

## Accomplishments
- `ValueRecord` gained a fifth, additive field (`origin_role: Option<String>`, `#[serde(default)]`) — serde round-trips it, and pre-field-existence JSON still deserializes to `None`.
- `ValueStore::mint` (the sole production `ValueRecord` constructor) threads the tag verbatim onto every minted record.
- All three broker mint sites now populate the tag per the DESIGN's per-site rule: `mint_from_read` reuses `claim.claim_type` verbatim; `mint_from_intent` takes a new caller-supplied role parameter; `mint_from_derivation` derives the role from `transform_kind` inside its own existing `"concat"` match arm, guarded on `inputs.len() == 2` before assigning `"recipient"`, and never reads `inputs[*].origin_role` (closes the anti-laundering path DESIGN §4 calls out).
- `server.rs`'s intent-variant match now also produces `primary_role` in the same arm as `primary_literal`, threaded to the shared `mint_from_intent` call — closing the F3 mistagging risk (a `file.create` path could otherwise be mistagged `"recipient"`).
- Every compilation-forced test call site (~40 across `quarantine.rs`'s own test module, `executor_decision.rs`, `s9_acceptance.rs`, `durable_anchor.rs`, `file_create.rs`, plus `types_compile.rs`/`value_store.rs`'s own fixtures) was updated with a role value chosen to preserve that fixture's existing assertion, per the plan's role-assignment discipline.
- Full Mac workspace green: `cargo build --workspace` exit 0, `cargo test --workspace --no-fail-fast` exit 0 (all binaries "ok", Linux-only security tests correctly report "0 passed" by design), `./scripts/check-invariants.sh` all 3 gates PASS.

## Task Commits

1. **Task 1: Add origin_role to ValueRecord + thread through ValueStore::mint** - `9b06fc2` (feat)
2. **Task 2: Thread origin_role through the 3 mint_from_* wrappers + 5 server.rs dispatch sites** - `071892a` (feat)
3. **Task 3: Update all compilation-forced test fixtures to keep the workspace green** - `b6d321f` (test)

**Plan metadata:** pending (this commit)

## Files Created/Modified
- `crates/runtime-core/src/value_record.rs` - added `origin_role: Option<String>` field with `#[serde(default)]`
- `crates/executor/src/value_store.rs` - `mint`'s 4th parameter, threaded into the `ValueRecord{}` literal; 2 new behavioral tests
- `crates/runtime-core/tests/types_compile.rs` - updated the direct `ValueRecord{}` literal; 2 new serde tests (round-trip with role, pre-field-JSON default)
- `crates/brokerd/src/quarantine.rs` - all 3 `mint_from_*` wrappers threaded; 8 test call sites + 2 direct `ValueRecord{}` test literals updated
- `crates/brokerd/src/server.rs` - `primary_role` selected inside the `:1294` intent-variant match, threaded to all 3 `mint_from_intent` call sites
- `crates/brokerd/src/sinks/file_create.rs` - own test module's 2 `.mint(` sites (`path`→`"path"`, `contents`→`None`)
- `crates/brokerd/tests/s9_acceptance.rs` - 2 sites (`mint_from_intent`→`"recipient"`, direct `.mint(`→`None` for `contents`)
- `crates/brokerd/tests/durable_anchor.rs` - 1 site (`contents`→`None`)
- `crates/executor/tests/executor_decision.rs` - 16 `.mint(` call sites, role assigned per arg semantics (to/recipient, to/email_address, subject, body, path, contents)

## Decisions Made
- `origin_role` stays `Option<String>` with `#[serde(default)]`, never a `TaintLabel` variant or bare `String` — matches DESIGN §1 exactly, keeps I0/I1 trust classification structurally unaffected.
- `mint_from_derivation`'s `"concat"` arm assigns `"recipient"` only when `inputs.len() == 2` (the verified `local@domain` shape); any other arity gets `None`, relying on I2's unconditional-untrusted-taint backstop for degenerate arities — matches DESIGN §4/F2 exactly.
- `server.rs` selects `primary_role` inside the intent-variant match, in the same arm that produces `primary_literal` — never hardcoded at the shared `:1317` call site reachable by both `SendEmailSummary` and `CreateFileFromReport` — matches DESIGN §2/F3 exactly.
- For the ~40 test-fixture call sites, role was assigned per the plan's discipline: preserve each fixture's existing assertion under the future Wave-2 role check (e.g., a `to`-slot `Allowed` test with `UserTrusted` taint got `Some("recipient")`; an untrusted doc-extracted-shaped `to` value got `Some("email_address")`; `contents` — an unconstrained slot per DESIGN §3 — got `None` throughout).
- Verification-command scoping: the plan's per-task `<verify>` blocks (`cargo test -p executor value_store`, `cargo test -p brokerd quarantine`) implicitly assume the whole package compiles, but Rust's per-crate compilation unit means Task 1 and Task 2's own signature changes leave OTHER test binaries in the same package broken until Task 3 lands its compilation-forced fixture updates (the plan's own objective text names this as "one atomic unit... all of them land in this plan"). Ran `--lib`-scoped verification during Tasks 1-2 to confirm the specific code under test in that task, and deferred the full `cargo build --workspace` / `cargo test --workspace --no-fail-fast` / `check-invariants.sh` gate to Task 3, where the plan's own acceptance criteria require and achieve it. No check was weakened — the full, unscoped gate ran and passed before Task 3 was committed.

## Deviations from Plan

None — plan executed exactly as written. The verification-command scoping noted above (using `--lib`/`--package`-scoped `cargo test` during Tasks 1-2, deferring the full-workspace gate to Task 3) is a technical necessity of Rust's compilation model given the plan's own explicit design (mass mechanical fixture updates land in Task 3), not a deviation from the plan's substance — every acceptance criterion in the plan was ultimately verified and passed, including the full-workspace gate.

## Issues Encountered
None. The plan's own file:line citations and pattern map (`24-PATTERNS.md`) were accurate; no drift found between the plan's assumed code shape and the live codebase.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The `origin_role` tag now rides on every minted `ValueRecord` in production code, ready for Plan 24-02/24-03 to add the hardcoded `expected_role()` lookup table and wire Step 1c's fail-closed enforcement in `submit_plan_node`.
- No enforcement exists yet — this plan is purely additive (as designed); a mismatched role currently has zero observable effect. That is expected and intentional per the plan's own success criteria.
- Test fixtures across the touched files now carry semantically-plausible roles that should NOT need re-visiting when Wave-2 enforcement lands, per the role-assignment discipline applied in Task 3 — Phase 25's regression audit should confirm this holds but should not need to re-derive roles from scratch.

---
*Phase: 24-slot-type-binding-enforcement*
*Completed: 2026-07-12*

## Self-Check: PASSED

All 9 modified production/test files confirmed present on disk; all 3 task commit hashes (`9b06fc2`, `071892a`, `b6d321f`) confirmed present in `git log --all`.
