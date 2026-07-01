---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
verified: 2026-07-01T06:45:00Z
status: passed
score: 14/14 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance — Verification Report

**Phase Goal:** `file.create` is a real, hardened sink; all enforcement edge cases from channel review are closed; the `RelativePath` claim variant completes the `ReportClaims` enum; and the full live §9 acceptance contract — hostile block with genuine-taint proof, clean allow, and a causal audit chain durable across process exit — is green on a real Linux `caprun` run.

**Verified:** 2026-07-01
**Status:** passed
**Re-verification:** No — initial verification

## Method note (why this is stronger than a normal verification)

This phase's central claim is a **live Linux** proof, and CLAUDE.md explicitly warns that Linux-only tests show "0 passed" on this macOS dev box and that the executor SUMMARY's claim of a passing Colima/Docker run cannot be taken on faith. Rather than mark all Linux-gated truths `human_needed`, I independently re-ran the exact CLAUDE.md-documented recipe myself in this session:

```
docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test -p brokerd -p caprun --no-fail-fast
docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work \
  -e CARGO_TARGET_DIR=/tmp/lt3 rust:1 cargo test -p adapter-fs workspace -- --test-threads=1
```

Results (this session, real Linux via Colima, kernel-confined, `seccomp=unconfined`, no `--privileged`):
- `adapter-fs::workspace` — **9/9 pass** (openat2 `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`, `O_EXCL` — legit read, absolute/`..`/symlink-escape rejected for both read and create).
- `brokerd::durable_anchor` — **2/2 pass** (the canonical ACC-07 after-exit DB-alone anti-stapling sentinel + tamper-evidence).
- `brokerd::s9_acceptance` — **3/3 pass** (incl. the `file.create` in-process backstop).
- `caprun::planner` — **5/5 pass**.
- `caprun::s9_live_block` — **4/4 pass**: `s9_live_file_create_hostile_block` (ACC-03), `s9_live_file_create_clean_allow` (ACC-04), `s9_live_clean_allow_path`, `s9_live_block_guard_binary_present`.
- One unrelated flake: `caprun::e2e::dag_chain_integrity` failed under full-parallel execution (`Connection refused` on the abstract UDS socket — a resource-contention artifact of running the whole workspace test suite concurrently in a container) and **passed cleanly (2/2) when re-run with `--test-threads=1`**. This file is untouched by any Phase-7 plan (last touched in `06-05`/`05-03`), tests the older email substrate-demo path (not `file.create`), and is not part of this phase's must-haves. Flagged as INFO, not a gap.

This means every Linux-gated ACC-03/04/05/07 claim in the SUMMARYs is **independently confirmed**, not merely trusted.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `file.create` sink exists with explicit `{path, contents}` arg schema; unknown/duplicate/missing args rejected before any sensitivity/executor step (SINK-01, HARD-01) | ✓ VERIFIED | `crates/executor/src/sink_schema.rs` `KNOWN_SINKS`/`validate_schema`, called as literal Step 0 of `submit_plan_node` (`crates/executor/src/lib.rs:53-55`); 8 unit tests pass (macOS + Linux) |
| 2 | `file.create`'s `path` is routing-sensitive; `contents` is not (SINK-02) | ✓ VERIFIED | `crates/executor/src/sink_sensitivity.rs` `FILE_CREATE_ROUTING_SENSITIVE`; tests pass |
| 3 | `file.create` never overwrites (`O_EXCL`) and resolves via `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)` under the workspace dirfd — TOCTOU-safe (SINK-03/SINK-04) | ✓ VERIFIED | `crates/adapter-fs/src/workspace.rs::create_exclusive_within`; **9/9 Linux enforcement tests pass** (independently re-run this session), incl. `create_exclusive_existing_path_rejected` (EEXIST), absolute/`..`/symlink-escape rejected |
| 4 | `RequestFd` reads are capability-restricted to the workspace root (HARD-04) | ✓ VERIFIED | `crates/adapter-fs/src/workspace.rs::read_within`; `Arc<WorkspaceRoot>` threaded `run_broker_server → handle_connection → dispatch_request`; `server.rs:261-263` calls `read_within`, no `std::fs::File::open` remains on the worker-supplied path; Linux tests pass |
| 5 | Effect-path ordering: validate schema → capability/resolve → sensitivity → executor decision → durable authorization audit → sink invocation → durable result audit; audit failure fails closed; causal parent preserved (HARD-05) | ✓ VERIFIED | `crates/brokerd/src/server.rs` `SubmitPlanNode` arm: append (`?` propagates on failure) happens before sending the decision, sink invocation happens only after that append succeeds, `parent_id` always set to chain head (never `None`) |
| 6 | Each sink attempt carries an effect id; authorization is durably recorded before invocation; a failure leaves an explicit indeterminate record with no automatic retry (HARD-06) | ✓ VERIFIED | `effect_id = Uuid::new_v4()` minted broker-side and threaded; `crates/brokerd/src/sinks/file_create.rs::invoke_file_create` two-phase audit (`sink_executed`/`sink_execution_failed`), tests pass on Linux |
| 7 | `WorkerClaim::RelativePath` completes the `ReportClaims` enum; unknown claim kinds still fail closed at deserialize (ASM-03 completion) | ✓ VERIFIED | `crates/brokerd/src/proto.rs` `WorkerClaim::RelativePath(String)`; `#[serde(tag="kind",content="value")]` (no wildcard) |
| 8 | A workspace-derived path is minted `[ExternalUntrusted, PathRaw]`, **never** `LocalWorkspace`; unknown claim_type fails closed | ✓ VERIFIED | `crates/brokerd/src/quarantine.rs::mint_from_read` match on `claim.claim_type`; `mint_from_read_relative_path_taint_is_path_raw` + `mint_from_read_unknown_claim_type_errors` tests pass |
| 9 | `BlockedPendingConfirmation` is operationally defined: zero sink invocations + non-success CLI result + durable `sink_blocked` event (ACC-01) | ✓ VERIFIED | `s9_live_file_create_hostile_block` (ran on real Linux this session): non-zero exit, no file, durable `sink_blocked` with anchor, no `sink_executed` |
| 10 | Live `file.create` hostile-block: hostile input → typed `RelativePath` claim → `mint_from_read` → `file.create` blocked, no file written (ACC-03) | ✓ VERIFIED (independently re-run) | `cli/caprun/tests/s9_live_block.rs::s9_live_file_create_hostile_block` — **passed on real Linux this session** |
| 11 | Clean allow-path: broker-minted trusted intent path creates exactly the expected file under the workspace root (ACC-04) | ✓ VERIFIED (independently re-run) | `s9_live_file_create_clean_allow` — **passed on real Linux this session**; file exists with expected contents, `sink_executed` recorded |
| 12 | One unbroken causal chain per run: `fd_granted → file_read → plan_node_evaluated → sink_blocked/sink_executed`, `verify_chain` true (ACC-05) | ✓ VERIFIED (independently re-run) | Both live tests assert `verify_chain` true + parent-linked walk; the `mint_from_read`/`mint_from_intent` `parent_id`-threading fix (07-05 deviation) is present in `quarantine.rs`/`server.rs` and is what makes this true |
| 13 | Forged handles and unknown sink/arg cases denied (ACC-06) | ✓ VERIFIED | `resolve()==None → Denied(DanglingHandle)`; `validate_schema` `UnknownSink`/`UnknownArg`/`DuplicateArg`/`MissingArg`; `submit_plan_node_unknown_sink_denied` test passes |
| 14 | Genuine-taint sentinel (anti-stapling), durable across process exit — `provenance_chain[0] == read_event_id`, tamper-evident (ACC-07) | ✓ VERIFIED (independently re-run) | `crates/brokerd/tests/durable_anchor.rs` — file-backed DB, **drop + reopen**, `verify_chain` first, then anchor backstops, then tamper-evidence (`UPDATE payload` → `verify_chain=false`); **passed on both macOS (this session) and real Linux (this session)** |

**Score:** 14/14 truths verified (0 present-but-behavior-unverified).

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/executor/src/value_store.rs` | `Result`-returning `mint`, rejects empty taint/provenance | ✓ VERIFIED | Confirmed exact match to spec; 5 unit tests pass |
| `crates/runtime-core/src/executor_decision.rs` | `DenyReason` enum + `SinkBlockedAnchor` + reshaped `BlockedPendingConfirmation` | ✓ VERIFIED | All 7 `DenyReason` variants present; anchor 8 fields match DESIGN §4 exactly |
| `crates/executor/src/lib.rs` | Step 0 schema gate → resolve → empty-taint/provenance guards → sensitivity → decision | ✓ VERIFIED | Ordering matches DESIGN §3 exactly; anchor built by verbatim clone (T-04-03) |
| `crates/runtime-core/src/event.rs` | `Event.anchor: Option<SinkBlockedAnchor>`, `Event::new`, `Event::sink_blocked` | ✓ VERIFIED | Golden byte-fixture test present and passing (byte-identical, no `"anchor"` key on `None`) |
| `crates/brokerd/src/audit.rs` | `append_event` rejects `sink_blocked` with `anchor==None` | ✓ VERIFIED | Guard at top of `append_event`; exercised indirectly by `durable_anchor.rs` |
| `crates/adapter-fs/src/workspace.rs` | `WorkspaceRoot` + `read_within` + `create_exclusive_within` (cfg-gated Linux real / macOS stub) | ✓ VERIFIED | 9/9 Linux enforcement tests re-run and passing this session |
| `crates/executor/src/sink_schema.rs` | `KNOWN_SINKS` + `validate_schema` | ✓ VERIFIED | 8 unit tests pass |
| `crates/executor/src/sink_sensitivity.rs` | `FILE_CREATE_ROUTING_SENSITIVE` + `file.create` arm | ✓ VERIFIED | Tests pass |
| `crates/brokerd/src/proto.rs` | `WorkerClaim::RelativePath(String)` | ✓ VERIFIED | Activated, exhaustive match, no wildcard |
| `crates/brokerd/src/quarantine.rs` | `extract_relative_path_claims` + claim-type-driven mint | ✓ VERIFIED | 5 new tests pass; `LocalWorkspace` never used |
| `crates/brokerd/src/sinks/file_create.rs` | `invoke_file_create` — two-phase durable audit | ✓ VERIFIED | Real `create_exclusive_within` write, `sink_executed`/`sink_execution_failed`, tests pass |
| `crates/brokerd/tests/durable_anchor.rs` | After-exit DB-alone sentinel + tamper-evidence | ✓ VERIFIED | Ran directly this session on macOS AND Linux — 2/2 both |
| `cli/caprun/tests/s9_live_block.rs` | Live hostile-block + clean-allow file.create tests | ✓ VERIFIED | Ran directly this session on real Linux — 4/4 |
| `crates/runtime-core/src/intent.rs`, `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs`, `cli/caprun/src/main.rs` | Intent-kind-driven CLI routing making both §9 paths reachable | ✓ VERIFIED | `CreateFileFromReport`, provenance-based routing, `create-file-from-report` CLI kind all present and wired; 5 planner tests pass |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `mint` (value_store.rs) | executor guard ordering | Empty-taint/provenance rejected before mint; guard re-checks post-resolve, pre-sensitivity | ✓ WIRED | Code inspected line-by-line; matches DESIGN §3 exactly |
| `submit_plan_node` | `validate_schema` | Called as literal first statement | ✓ WIRED | `lib.rs:53` |
| `dispatch_request` RequestFd arm | `WorkspaceRoot::read_within` | Direct call, no `std::fs::File::open` fallback | ✓ WIRED | `server.rs:261-263` |
| `SubmitPlanNode` Allowed + `file.create` | `invoke_file_create` | Direct call after durable `plan_node_evaluated` append | ✓ WIRED | `server.rs:404-424` |
| worker intent kind | claim extractor selection | `CreateFileFromReport` → `extract_relative_path_claims` → `WorkerClaim::RelativePath` | ✓ WIRED | `worker.rs:126-132` |
| planner | routing by provenance | tainted file handle first, else trusted intent handle | ✓ WIRED | `planner.rs:70-95`; 2 planner tests directly assert both routes |
| `mint_from_read`/`mint_from_intent` | causal `parent_id` chain | Threaded from connection chain head (07-05 fix) | ✓ WIRED | `quarantine.rs` signatures take `parent_id`; `server.rs` passes `Some(*last_event_id)`; `verify_chain` true on live multi-event flow (confirmed via independent Linux run) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SINK-01 | 07-04a/04b | `file.create` sink with explicit schema | ✓ SATISFIED | `sink_schema.rs`, live sink wiring, tests |
| SINK-02 | 07-04a | `path` routing-sensitive | ✓ SATISFIED | `sink_sensitivity.rs` |
| SINK-03 | 07-04a | `O_EXCL` exclusive create, never overwrites | ✓ SATISFIED | `workspace.rs::create_exclusive_within`, Linux tests re-run |
| SINK-04 | 07-03/04a | `openat2` resolution under workspace dirfd, TOCTOU-safe | ✓ SATISFIED | Same, Linux tests re-run |
| HARD-01 | 07-04a | Unknown sinks/args fail closed before sensitivity/executor | ✓ SATISFIED | `validate_schema` Step 0 |
| HARD-04 | 07-03 | `RequestFd` capability-restricted to workspace root | ✓ SATISFIED | `WorkspaceRoot::read_within` threaded; Linux tests re-run |
| HARD-05 | 07-01/02/04a/04b | Effect-path ordering enforced, causal parent preserved | ✓ SATISFIED | `server.rs` `SubmitPlanNode` arm ordering |
| HARD-06 | 07-02/04b | Effect id + durable pre-invocation authorization + no-retry indeterminate record | ✓ SATISFIED | `effect_id` minting, two-phase audit in `file_create.rs` |
| ACC-01 | 07-05 | `BlockedPendingConfirmation` operational definition | ✓ SATISFIED | Live hostile-block test assertions |
| ACC-03 | 07-05 | Live `file.create` hostile block | ✓ SATISFIED (independently re-run on real Linux) | `s9_live_file_create_hostile_block` |
| ACC-04 | 07-05 | Live clean allow-path | ✓ SATISFIED (independently re-run on real Linux) | `s9_live_file_create_clean_allow` |
| ACC-05 | 07-05 | One unbroken causal chain per run | ✓ SATISFIED (independently re-run on real Linux) | `verify_chain` assertions in both live tests + `durable_anchor.rs` |
| ACC-06 | 07-01/04a | Forged handles / unknown sink-arg denied | ✓ SATISFIED | `DanglingHandle`, `UnknownSink`/`UnknownArg` tests |
| ACC-07 | 07-02/05 | Genuine-taint anti-stapling sentinel, durable | ✓ SATISFIED (independently re-run on macOS + real Linux) | `durable_anchor.rs` after-exit + tamper-evidence tests |

No orphaned requirements — all 14 IDs declared in Phase-7 plan frontmatters match REQUIREMENTS.md's traceability table exactly, and all are now satisfied (REQUIREMENTS.md itself still shows "Pending" checkboxes/status — that file was not updated post-completion; this is a documentation-sync gap, not a functional gap, and does not affect the phase-goal verdict).

### Anti-Patterns Found

None. Grep for `TODO|FIXME|XXX|TBD|HACK|PLACEHOLDER` across every file touched by all five Phase-7 plans returned zero matches. No stub returns, no hardcoded empty data flowing to real paths, no `console.log`-only implementations (N/A — Rust). `git status` shows Phase-7 code fully committed (only stray untracked `.md` planning docs, no code).

One out-of-scope observation: `cli/caprun/tests/e2e.rs::dag_chain_integrity` (a pre-existing, non-Phase-7 test) failed once under full-parallel Linux execution (abstract-UDS `Connection refused` — a resource-contention flake) and passed cleanly when re-run serialized. This file was last touched in `06-05`/`05-03`, is not part of any Phase-7 plan's file list, and does not exercise `file.create`/durable-anchor functionality. **INFO, not a gap.**

### Behavioral Spot-Checks / Real Execution

Rather than spot-checks, the full relevant test suites were **actually executed** (not merely inspected):
- macOS: `cargo build --workspace` (clean), `cargo test --workspace --no-fail-fast` (30/30 targets, 0 failures), `./scripts/check-invariants.sh` (both gates PASS).
- Real Linux (Colima/Docker, `seccomp=unconfined`, no `--privileged`, independently invoked this session):
  - `cargo test -p adapter-fs workspace -- --test-threads=1` → 9/9 pass.
  - `cargo test -p brokerd -p caprun --no-fail-fast` → all Phase-7-relevant suites green (`durable_anchor` 2/2, `s9_acceptance` 3/3, `s9_live_block` 4/4, `planner` 5/5); one unrelated flake noted above, confirmed non-reproducing when serialized.

### Human Verification Required

None. Every truth that would normally require a human to confirm a Linux-only claim was independently re-executed in this verification session against real Linux via the documented Colima/Docker recipe.

### Gaps Summary

No gaps. All 14 must-have truths verified with direct code inspection AND direct test execution (both macOS and real Linux). The phase goal — `file.create` as a real hardened sink, all HARD-0x enforcement edge cases closed, `RelativePath` completing `ReportClaims`, and the full live §9 acceptance contract (hostile block + genuine-taint proof + clean allow + durable causal chain) green on a real Linux `caprun` run — is achieved and independently confirmed, not merely asserted by SUMMARY.md.

One documentation-sync item for the developer to close out (not a code gap): `.planning/REQUIREMENTS.md`'s per-requirement checkboxes/traceability status still read `[ ]`/"Pending" for all 14 Phase-7 IDs; recommend updating those to `[x]`/"Complete" now that Phase 7 verification has passed.

---

_Verified: 2026-07-01_
_Verifier: Claude (gsd-verifier)_
