---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 04a
subsystem: security
tags: [executor, i2, taint, file.create, openat2, arg-schema, seccomp, landlock, nix]

# Dependency graph
requires:
  - phase: 07-01
    provides: DenyReason typed taxonomy + mint non-empty-taint invariant
  - phase: 07-02
    provides: SinkBlockedAnchor reshape + effect_id param in submit_plan_node
  - phase: 07-03
    provides: WorkspaceRoot dirfd capability (read_within) + openat2 RESOLVE_* pattern
provides:
  - "TaintLabel::PathRaw (untrusted workspace-read path label)"
  - "file.create path routing-sensitivity (is_routing_sensitive arm)"
  - "sink arg-schema gate (validate_schema) as Step 0 of submit_plan_node"
  - "DenyReason extended with UnknownSink/UnknownArg/DuplicateArg/MissingArg"
  - "WorkspaceRoot::create_exclusive_within (O_EXCL exclusive-create write capability)"
affects: [07-04b]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SinkSchema registry with separate allowed/required arg sets (fail-closed unknown sink/arg)"
    - "cfg-gated Linux(openat2)/macOS(stub) capability pair mirroring 07-03 read_within"

key-files:
  created:
    - crates/executor/src/sink_schema.rs
  modified:
    - crates/runtime-core/src/plan_node.rs
    - crates/runtime-core/src/executor_decision.rs
    - crates/runtime-core/tests/intent_taint.rs
    - crates/executor/src/sink_sensitivity.rs
    - crates/executor/src/lib.rs
    - crates/adapter-fs/src/workspace.rs
    - crates/brokerd/src/lib.rs

key-decisions:
  - "SinkSchema splits allowed (rejects unknown/duplicate) from required (rejects missing) — exact-match would have broken pre-existing single-arg email.send tests and the §9 acceptance test"
  - "email.send required=[] to preserve pre-07-04a semantics; 'attachment' added to allowed to match the actual live shape (EMAIL_SEND_CONTENT_SENSITIVE) which the plan's explicit list omitted"
  - "validate_schema is Step 0 of submit_plan_node — runs before resolve/taint/sensitivity so unknown sink/arg/duplicate/missing fails closed first (HARD-01/HARD-05)"
  - "New DenyReason variants carry the offending sink/arg name (String) for audit/CLI; single enum, no second error type"

patterns-established:
  - "Arg-schema gate: hardcoded KNOWN_SINKS registry in the Rust TCB, no config file (mirrors sink_sensitivity)"
  - "Exclusive-create capability: single openat2(O_CREAT|O_EXCL, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS) syscall, TOCTOU-safe"

requirements-completed: [SINK-01, SINK-02, SINK-03, SINK-04, HARD-01, HARD-05]

coverage:
  - id: D1
    description: "TaintLabel::PathRaw exists and is_untrusted() returns true (exhaustive match, no wildcard)"
    requirement: "SINK-02"
    verification:
      - kind: unit
        ref: "crates/runtime-core/tests/intent_taint.rs#is_untrusted_path_raw_returns_true"
        status: pass
    human_judgment: false
  - id: D2
    description: "file.create path is routing-sensitive; contents is not"
    requirement: "SINK-02"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_sensitivity.rs#file_create_path_is_routing_sensitive, file_create_contents_not_routing_sensitive"
        status: pass
    human_judgment: false
  - id: D3
    description: "validate_schema rejects unknown sink/arg, duplicate, missing arg BEFORE resolve/sensitivity; file.create accepts exactly {path,contents}; email.send subset preserved"
    requirement: "SINK-01"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#tests (7 cases) + crates/brokerd/src/lib.rs#submit_plan_node_unknown_sink_denied"
        status: pass
    human_judgment: false
  - id: D4
    description: "DenyReason extended with UnknownSink/UnknownArg/DuplicateArg/MissingArg (single enum, code()/Display updated)"
    requirement: "HARD-01"
    verification:
      - kind: unit
        ref: "crates/executor/src/sink_schema.rs#tests; full workspace build"
        status: pass
    human_judgment: false
  - id: D5
    description: "WorkspaceRoot::create_exclusive_within: O_EXCL exclusive-create under dirfd, rejects existing(EEXIST)/absolute/../symlink; Linux-only, macOS stub builds"
    requirement: "SINK-03"
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/workspace.rs#create_exclusive_* (5 Linux-gated tests) — run under Colima/Docker per CLAUDE.md"
        status: unknown
    human_judgment: true
    rationale: "Enforcement tests are #[cfg(target_os=linux)] and show 0-passed on the macOS dev machine (expected per CLAUDE.md). Actual pass requires the Colima/Docker Linux recipe; not run in this macOS execution."

# Metrics
duration: 15min
completed: 2026-07-01
status: complete
---

# Phase 07 Plan 04a: file.create Enforcement Mechanisms Summary

**Deterministic decision-side machinery for `file.create`: PathRaw taint label, path routing-sensitivity, a fail-closed arg-schema gate (validate_schema) as Step 0 of submit_plan_node, and the O_EXCL exclusive-create WorkspaceRoot capability — no live sink wired (that is 07-04b).**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-07-01T09:33Z (approx)
- **Completed:** 2026-07-01T09:48Z
- **Tasks:** 3 (+1 in-scope fix)
- **Files modified:** 8 (1 created)

## Accomplishments
- `TaintLabel::PathRaw` added as an untrusted label via the exhaustive `is_untrusted()` match (no wildcard) — a tainted workspace-read path now blocks on routing-sensitive args.
- `file.create` registered as routing-sensitive on `path` (not `contents`), mirroring the `email.send` sensitivity arm.
- `validate_schema` arg-schema gate runs FIRST in `submit_plan_node`: unknown sink/arg, duplicate, or missing arg → `Denied` before any resolve/taint/sensitivity work (HARD-01/HARD-05). Extends the single `DenyReason` taxonomy — no second error type.
- `WorkspaceRoot::create_exclusive_within`: single `openat2(O_CREAT|O_EXCL|O_WRONLY, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)` syscall — never overwrites (EEXIST), rejects absolute/`..`/symlink-escape at kernel resolution (TOCTOU-safe). macOS stub via `create_new(true)`.

## Task Commits

Each task was committed atomically:

1. **Task 1: PathRaw taint label + file.create routing sensitivity** - `1d5a672` (feat)
2. **Task 2: sink arg-schema registry + validate_schema (HARD-01)** - `b12d86e` (feat)
3. **Task 3: create_exclusive_within (SINK-03/SINK-04 write-side)** - `21a6367` (feat)
4. **Task 2 fix: split schema allowed/required + broker test** - `c08c9b1` (fix)

## Files Created/Modified
- `crates/executor/src/sink_schema.rs` - **created.** KNOWN_SINKS registry (SinkSchema{allowed, required}) + validate_schema; 8 unit tests.
- `crates/runtime-core/src/plan_node.rs` - `TaintLabel::PathRaw` + exhaustive `is_untrusted()` arm.
- `crates/runtime-core/src/executor_decision.rs` - `DenyReason` extended with UnknownSink/UnknownArg/DuplicateArg/MissingArg (String payload); `code()`/`Display` updated.
- `crates/runtime-core/tests/intent_taint.rs` - PathRaw truth-table test (8 variants).
- `crates/executor/src/sink_sensitivity.rs` - `FILE_CREATE_ROUTING_SENSITIVE` + `file.create` arm; routing tests.
- `crates/executor/src/lib.rs` - `pub mod sink_schema;` + Step 0 `validate_schema` call in `submit_plan_node`.
- `crates/adapter-fs/src/workspace.rs` - cfg-gated `create_exclusive_within` + 5 Linux-gated tests.
- `crates/brokerd/src/lib.rs` - delegation test retargeted to a known sink; added unknown-sink-denied test.

## Decisions Made
- **allowed vs required split:** the plan's Task 2 described "missing required arg" and "accepts exactly {path,contents}". An exact-match (required == allowed) implementation broke pre-existing single-arg `email.send` tests and the §9 acceptance test, which build minimal nodes. Split into `allowed` (unknown/duplicate rejection, all sinks) and `required` (missing rejection). email.send `required=[]` preserves its pre-07-04a contract; file.create `required=[path,contents]`.
- **DenyReason variants carry names:** `UnknownSink(String)` etc. give the audit/CLI the offending name; `code()` still returns stable static codes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Arg-schema exact-match broke pre-existing email.send + §9 tests**
- **Found during:** Task 2 (full-workspace test after wiring validate_schema)
- **Issue:** Registering the schema as a single exact-match set (all declared args required) caused `submit_plan_node` to `Denied(MissingArg)` on the pre-existing single-arg `email.send` nodes used by `executor_decision.rs`, `phase5_dispatch.rs`, and the critical `s9_acceptance.rs` (v0-DONE) tests — 12 failures.
- **Fix:** Split `SinkSchema` into `allowed` + `required`. email.send `required=[]` (07-04a must not change email.send semantics); file.create `required=[path,contents]`.
- **Files modified:** crates/executor/src/sink_schema.rs
- **Verification:** `cargo test --workspace --no-fail-fast` — 0 failures.
- **Committed in:** `c08c9b1`

**2. [Rule 1 - Bug] email.send `attachment` arg missing from plan's declared shape**
- **Found during:** Task 2 fix (residual failure `tainted_body_and_attachment_allow_in_v0`)
- **Issue:** The plan listed email.send as `[to,cc,bcc,subject,body]`, but `EMAIL_SEND_CONTENT_SENSITIVE` includes `attachment` and an existing test routes a tainted `attachment`. That arg was rejected as `UnknownArg`. The plan directive was "match the current live shape" — the live shape includes attachment.
- **Fix:** Added `attachment` to email.send `allowed`.
- **Files modified:** crates/executor/src/sink_schema.rs
- **Verification:** full workspace test green.
- **Committed in:** `c08c9b1`

**3. [Rule 1 - Bug] brokerd delegation test asserted Allowed for an unknown sink**
- **Found during:** Task 2 (`submit_plan_node_empty_args_returns_allowed` used `SinkId("test.sink")`)
- **Issue:** The plan's new "unknown sinks fail closed" requirement (HARD-01/HARD-05) makes an unknown sink `Denied(UnknownSink)`, invalidating the test's old `Allowed` assertion.
- **Fix:** Retargeted the delegation smoke test to `email.send` (registered, no required args → Allowed) and added `submit_plan_node_unknown_sink_denied` asserting the new fail-closed behavior.
- **Files modified:** crates/brokerd/src/lib.rs
- **Verification:** brokerd unit tests pass.
- **Committed in:** `c08c9b1`

---

**Total deviations:** 3 auto-fixed (all Rule 1 — correctness/security of the new gate vs. pre-existing contracts).
**Impact on plan:** All fixes were necessary to make the plan's arg-schema gate coexist with pre-existing sink behavior without breaking the §9 acceptance test. No scope creep — no live sink wired, no IPC touched (07-04b scope untouched).

## Issues Encountered
- The plan's email.send arg list was an incomplete restatement of the live shape (omitted `attachment`) and its "exactly {path,contents}" / "missing required arg" language implied an exact-match model that conflicts with email.send's existing minimal-node tests. Resolved via the allowed/required split above.

## Verification Status
- `cargo build --workspace` — green.
- `cargo test --workspace --no-fail-fast` — green on macOS: 0 failures. `create_exclusive_within` Linux-gated tests show 0-passed on macOS (expected per CLAUDE.md; run under Colima/Docker `seccomp=unconfined` to exercise).
- `./scripts/check-invariants.sh` — both gates PASS (no `EffectRequest`; runtime-core pure).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Enforcement mechanisms are in place for 07-04b to wire `file.create` into the live IPC/sink path: schema gate, `path` routing-sensitivity, `PathRaw` label, and the exclusive-create capability.
- **07-04b note:** the exclusive-create + resolution guarantees are Linux-only; run the Colima/Docker recipe to confirm the 5 `create_exclusive_within` negative-assertion tests before declaring the live sink complete.

---
*Phase: 07-file-create-sink-enforcement-hardening-full-acceptance*
*Completed: 2026-07-01*
