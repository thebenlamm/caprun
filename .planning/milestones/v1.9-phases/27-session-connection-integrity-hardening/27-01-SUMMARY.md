---
phase: 27-session-connection-integrity-hardening
plan: 01
subsystem: security
tags: [rust, tokio, unix-domain-socket, session-trust-state, audit-dag, fstat, arc-mutex]

# Dependency graph
requires:
  - phase: 26-security-hardening-design-gate
    provides: DESIGN-security-hardening.md §a/§f (HARDEN-01 mechanism + X-04 ruling), DESIGN-GATE-RECORD-v1.6.md (F2/F3 amendments)
provides:
  - "RequestFd fd-grant-time I1 demotion via a broker-derived fstat (st_dev, st_ino) inode-identity compare against the CLI-designated <workspace-file>"
  - "A single shared, monotonic Arc<Mutex<SessionStatus>> cell replacing the per-connection owned SessionStatus snapshot (X-04/F3 fold), re-read at the top of every dispatch_request call"
  - "A genuine fd_granted -> session_demoted causal audit edge for the new demotion site"
  - "Corrected quarantine.rs doc comments + DESIGN-session-trust-state.md §2/§5 amendments naming both I1 trust-flip sites"
  - "New integration tests proving fd-grant demotion, cross-connection Draft visibility, and the SC2 clean-path regression"
affects: [27-02-featureless-createsession, 28-authenticated-audit-chain, 30-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "std::os::unix::fs::MetadataExt (st_dev/st_ino) for TOCTOU-safe fd-identity comparison on an already-open std::fs::File — no new crate"
    - "Arc<Mutex<T>> cross-task-shared, monotonic trust-state cell (mirrors the existing planner_slot_occupied: Arc<AtomicBool> pattern)"
    - "Re-read shared mutable state under lock at the TOP of every function call that decides on it, never trust a value cached at setup time"

key-files:
  created:
    - crates/brokerd/tests/harden01_session_integrity.rs
  modified:
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/quarantine.rs
    - cli/caprun/src/main.rs
    - planning-docs/DESIGN-session-trust-state.md
    - crates/brokerd/tests/durable_anchor.rs
    - crates/brokerd/tests/extract_provenance_threading.rs
    - crates/brokerd/tests/phase5_dispatch.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/brokerd/tests/uds_ipc.rs
    - crates/brokerd/tests/planner_capability_split.rs
    - crates/brokerd/tests/two_connection_intent_bypass.rs
    - crates/brokerd/tests/planner_reduced_signal.rs

key-decisions:
  - "session_status shared cell constructed ONCE inside run_broker_server (mirrors planner_slot_occupied), never passed in pre-wrapped from main.rs"
  - "dispatch_request re-reads the shared cell into an owned local snapshot at the top of every call; every write locks the shared cell AND updates the local snapshot for same-call consistency"
  - "Trusted-path identity is the CLI's raw <workspace-file> arg (ws_path.to_path_buf()), fstat'd fresh per RequestFd call against the freshly-opened granted File — no canonical-path caching, no TOCTOU window"
  - "Test B (X-04/F3) proves cross-connection shared-cell visibility via two independent dispatch_request calls (separate ValueStore/chain-state, shared Arc handle) rather than a literal ConnectionRole::Planner wire connection, because that role's ValueStore is structurally always empty under HARD-03 with today's shipped sink registry (no live production code path ever uses DeclarePlannerRole) — confirmed against planner_reduced_signal.rs's own precedent before writing the test"

requirements-completed: [HARDEN-01]

coverage:
  - id: D1
    description: "RequestFd on an untrusted (non-workspace-file) path demotes the session to Draft at fd-grant time, with no ReportClaims ever sent, via a broker-derived fstat inode-identity compare"
    requirement: "HARDEN-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/harden01_session_integrity.rs#fd_grant_on_untrusted_path_demotes_without_report_claims"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/server.rs#server::tests::provide_intent_after_request_fd_is_rejected (regression: RequestFd on the SAME trusted file stays unaffected by the new mechanism)"
        status: pass
    human_judgment: false
  - id: D2
    description: "session_status becomes one shared, monotonic Arc<Mutex<SessionStatus>>, re-read at the top of every dispatch_request call — a Worker-connection demotion is visible to any other holder of the shared handle, never a stale per-connection Active snapshot"
    requirement: "HARDEN-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/harden01_session_integrity.rs#second_dispatch_call_after_demotion_observes_draft_not_stale_active"
        status: pass
    human_judgment: false
  - id: D3
    description: "The trusted, inode-matched clean RequestFd path stays Active (no demotion) — the CONTROL-01 benign send path is not regressed"
    requirement: "HARDEN-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/harden01_session_integrity.rs#fd_grant_on_trusted_path_stays_active"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/s9_live_block.rs#s9_control_ab_taint_driven (Linux, via scripts/mailpit-verify.sh)"
        status: pass
    human_judgment: false
  - id: D4
    description: "The new fd-grant demotion appends a session_demoted Event whose parent_id equals the fd_granted Event's id — a genuine causal edge, reusing the existing session_demoted event_type literal"
    requirement: "HARDEN-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/harden01_session_integrity.rs#fd_grant_on_untrusted_path_demotes_without_report_claims (parent_id assertion)"
        status: pass
    human_judgment: false
  - id: D5
    description: "quarantine.rs's stale 'SOLE I1 trust-flip site' doc comment corrected, and DESIGN-session-trust-state.md §2/§5 amended to name both trust-flip sites and the second causal-edge case, same PR as the code"
    requirement: "HARDEN-01"
    verification:
      - kind: other
        ref: "grep -n \"SOLE I1 trust-flip site\" crates/brokerd/src/quarantine.rs (exit 1, no match)"
        status: pass
    human_judgment: false

# Metrics
duration: ~65min
completed: 2026-07-12
status: complete
---

# Phase 27 Plan 01: RequestFd Fd-Grant Demotion + Shared Session-Status Cell Summary

**RequestFd now demotes a session to Draft the instant an untrusted fd is granted (fstat inode-identity compare, no path-string compare), and session_status became one shared, monotonic Arc<Mutex<SessionStatus>> re-read at the top of every dispatch_request call — closing the X-04 stale-Planner-snapshot gap in the same PR.**

## Performance

- **Duration:** ~65 min
- **Started:** 2026-07-12T18:35:49Z (per STATE.md's recorded session start)
- **Completed:** 2026-07-12T19:06:14Z
- **Tasks:** 4/4 completed
- **Files modified:** 12 (1 created, 11 modified)

## Accomplishments

- **HARDEN-01 mechanism (Task 2):** the `RequestFd` arm now computes `(st_dev, st_ino)` on the already-open granted `std::fs::File` and compares it against the CLI-designated `<workspace-file>` (fstat'd fresh each call) — a fail-closed default demotes on ANY mismatch or metadata error, corrected F2 ordering (open → append `fd_granted` → fstat compare → demote-if-untrusted → `pass_fd`).
- **X-04/F3 fold (Task 1):** `session_status` converted from a per-connection owned `SessionStatus` snapshot to a single `Arc<Mutex<SessionStatus>>` constructed once in `run_broker_server` (mirroring `planner_slot_occupied`), threaded through `handle_connection`/`classify_second_connection`/`dispatch_request`, re-read under lock at the top of every `dispatch_request` call and at the Planner branch's Step 0.5 check.
- **Genuine causal edge:** the new demotion writes through the existing `update_session_status` helper (no new Gate-3 mint call site) and appends a `session_demoted` Event parented on the just-appended `fd_granted` Event's id — reusing the exact `"session_demoted"` event_type literal `mint_from_read` already uses.
- **Doc correction (Task 3):** `quarantine.rs`'s stale "SOLE I1 trust-flip site" claim corrected to name both sites; `DESIGN-session-trust-state.md` §2's D-02 forward-note converted from "forthcoming" to a realized description, and §5 gained the second `fd_granted -> session_demoted` causal-edge case.
- **New tests (Task 4):** `crates/brokerd/tests/harden01_session_integrity.rs` — three tests driving the REAL production `RequestFd`/`dispatch_request` code, none Linux-gated (all pass on both macOS and Linux).
- **All 12 pre-existing `dispatch_request` call sites** (1 production + 11 test) updated for the new signature; **all 7 `run_broker_server` call sites** (1 production + 6 test, the latter discovered as a Task-1 gap — see Deviations) updated for the new `trusted_workspace_path` parameter.
- Full Linux verification via `scripts/mailpit-verify.sh` (Colima+Docker): `cargo build --workspace && cargo test --workspace --no-fail-fast` green, including `s9_control_ab_taint_driven` (CONTROL-01's live clean-send A/B, proving SC2 holds against real confinement) and all real Landlock/seccomp negative tests.

## Task Commits

1. **Task 1: Thread trusted-path param + shared Arc<Mutex<SessionStatus>>** - `40addc0` (feat)
2. **Task 2: RequestFd fstat identity compare + demote-before-pass_fd** - `ad402df` (feat)
3. **Task 3: Correct stale doc comments + amend DESIGN-session-trust-state.md §2/§5** - `100a6dd` (docs)
4. **Task 4: New harden01_session_integrity.rs tests** (+ Task-1 gap fix) - `bf38a76` (test)

_No separate plan-metadata commit — this SUMMARY.md is the final artifact commit._

## Files Created/Modified

- `crates/brokerd/tests/harden01_session_integrity.rs` - NEW: 3 tests (fd-grant demotion negative, cross-connection Draft visibility, SC2 clean-path regression)
- `crates/brokerd/src/server.rs` - `run_broker_server`/`handle_connection`/`classify_second_connection`/`dispatch_request`/`evaluate_plan_node_and_record` signatures; the `RequestFd` arm's fstat compare + demotion; shared-cell re-read/write plumbing
- `crates/brokerd/src/quarantine.rs` - corrected `mint_from_read`'s stale "SOLE I1 trust-flip site" doc comments (module header + Step-4 inline)
- `cli/caprun/src/main.rs` - derives `trusted_workspace_path: PathBuf` from the CLI's `<workspace-file>` arg, threads it into `run_broker_server(...)`
- `planning-docs/DESIGN-session-trust-state.md` - §2 heading/body + D-02 amendment converted to realized description; §5 gained the second causal-edge amendment
- `crates/brokerd/tests/durable_anchor.rs`, `extract_provenance_threading.rs`, `phase5_dispatch.rs`, `proto_claims.rs` - updated `dispatch_request` call sites (6 total) for the new `Arc<Mutex<SessionStatus>>` + trusted-path params
- `crates/brokerd/tests/uds_ipc.rs`, `planner_capability_split.rs`, `two_connection_intent_bypass.rs`, `planner_reduced_signal.rs` - updated `run_broker_server` call sites (6 total, Linux-gated) for the new `trusted_workspace_path` param (Deviation, see below)

## Decisions Made

- Constructed the `Arc<Mutex<SessionStatus>>` cell INSIDE `run_broker_server` from the owned `initial_session_status` param (mirroring `planner_slot_occupied`), not pre-wrapped from `main.rs` — matches the DESIGN doc's explicit precedent and F3's construct-once pin.
- `evaluate_plan_node_and_record`'s `session_status` param changed from `&mut SessionStatus` to `&SessionStatus` (read-only) — it never wrote to it; only `dispatch_request`'s own arms (ReportClaims, RequestFd) ever write the shared cell.
- Trusted-path identity is the CLI's raw `<workspace-file>` argument (`ws_path.to_path_buf()`) — the exact value `workspace_root_dir`/`workspace_rel` are already derived from — fstat'd fresh inside the `RequestFd` arm each call, never cached/canonicalized ahead of time.
- Test B (X-04/F3) drives two independent `dispatch_request` calls (separate `ValueStore`/chain-state, one shared `Arc` handle) rather than a literal `ConnectionRole::Planner` wire connection — see Deviations/rationale below for why the latter is structurally untestable with today's sink registry.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `run_broker_server`'s new 8th positional param broke 6 Linux-gated test call sites Task 1's own budget didn't enumerate**
- **Found during:** Task 4 (while checking for other `run_broker_server` callers before writing the new Linux-adjacent test harness)
- **Issue:** Task 1's plan explicitly enumerated and fixed all 12 `dispatch_request` call sites, but `run_broker_server`'s signature ALSO changed (new `trusted_workspace_path: PathBuf` param) and RESEARCH.md's Pitfall-1 enumeration only covered `dispatch_request` callers, not `run_broker_server` callers. Six additional call sites — `crates/brokerd/tests/uds_ipc.rs` (×3), `planner_capability_split.rs`, `two_connection_intent_bypass.rs`, `planner_reduced_signal.rs` — call `run_broker_server` directly and would fail to compile with the new signature. All six are `#[cfg(target_os = "linux")]`-gated, so macOS `cargo build --workspace`/`cargo test --workspace` (Task 1/2/3's own verification gate) never surfaced the break — it would only have appeared on the next Linux run (`scripts/mailpit-verify.sh`), or in a later phase.
- **Fix:** Added a placeholder trusted-path argument (`std::env::temp_dir().join("__<file>_no_trusted_path__")`) to each of the 6 call sites — none of these tests exercise `RequestFd` against a path meaningful to the new fstat compare, so an unresolvable placeholder is correct (any `metadata()` error on the trusted side fails closed to "untrusted," matching each test's pre-existing, unrelated intent).
- **Files modified:** `crates/brokerd/tests/uds_ipc.rs`, `planner_capability_split.rs`, `two_connection_intent_bypass.rs`, `planner_reduced_signal.rs`
- **Verification:** `bash scripts/mailpit-verify.sh` (Colima+Docker, `rust:1`) — full `cargo build --workspace && cargo test --workspace --no-fail-fast` green, including all 4 touched files' own test suites (`uds_ipc.rs` 3/3, `planner_capability_split.rs` 3/3, `two_connection_intent_bypass.rs` 3/3, `planner_reduced_signal.rs` 2/2) and the full CONTROL-01 live A/B (`s9_control_ab_taint_driven`).
- **Committed in:** `bf38a76` (Task 4 commit)

### Test-B design deviation from the plan's literal wording (documented, not a bug)

The plan's Task 4 text describes Test B as "accept a Planner connection and issue a request that dispatch_request's Step 0.5 I0 check governs." A literal `ConnectionRole::Planner` wire connection (via `run_broker_server` + `DeclarePlannerRole`) was investigated first; it is **structurally unable to exercise Step 0.5 with resolvable args** in this codebase's current state: `ConnectionRole::Planner.permits()` admits ONLY `SubmitPlanNode` (no mint verb), so a Planner connection's `ValueStore` is always empty (HARD-03) — any args-bearing plan node it submits would hit `Denied{DanglingHandle}` at Step 1, before Step 0.5 is ever reached, regardless of `session_status`. This is confirmed by `planner_reduced_signal.rs`'s own existing Linux-gated accept-loop test, which uses an intentionally EMPTY-args node "under HARD-03 the planner's own ValueStore is empty, so an args-bearing node's handles would not resolve." `DeclarePlannerRole`/`ConnectionRole::Planner` is also confirmed (by grep) to be referenced only inside `crates/brokerd/src/` and `crates/brokerd/tests/` — never wired into any production `cli/caprun` code path. Test B instead drives two independent `dispatch_request` calls (separate `ValueStore`/chain-state locals, mirroring genuinely distinct per-connection scopes) sharing only the ONE `Arc<Mutex<SessionStatus>>` handle, and mints an all-`UserTrusted` `file.create` plan node directly into the second call's `ValueStore` (a legitimate test-only `ValueStore::mint` call — Gate 3-exempt for `/tests/` files, same precedent `durable_anchor.rs`/`extract_provenance_threading.rs` already use) so Step 0.5 is the ONLY thing that can explain a `Denied` outcome. This proves the exact X-04/F3 property (shared-cell visibility across independent call scopes) without depending on a wire-protocol path that cannot currently carry it.

---

**Total deviations:** 1 auto-fixed (Rule 1 bug fix) + 1 documented test-design deviation (not a bug — a plan-text-vs-code-reality reconciliation, verified before writing the test rather than assumed)
**Impact on plan:** The Rule-1 fix was necessary for Linux compilation correctness and was entirely test-file scope (no product code changed beyond Tasks 1-3's own planned edits). The Test-B design deviation is a strictly MORE rigorous proof of the same security property the plan specifies, arrived at by checking the plan's assumption against the actual sink registry/capability code before writing untestable test code.

## Issues Encountered

None beyond the deviations above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- HARDEN-01 is fully landed and tested (unit + integration + live Linux CONTROL-01 regression). The X-04 fold closes a real, previously-un-exercised bypass (confirmed real by Phase 26's adversarial review, `DESIGN-GATE-RECORD-v1.6.md` §f).
- Plan 27-02 (HARDEN-04, compile-out the forced-Active `CreateSession` mint) is unblocked — it touches `crates/brokerd/Cargo.toml`'s `[features]` block and `server.rs`'s `CreateSession` arm, disjoint from this plan's `RequestFd`/session_status surface.
- `DeclarePlannerRole`/`ConnectionRole::Planner` remains a genuinely-unused forward-looking seam (confirmed again this plan) — a future phase wiring a real second-connection planner will need to solve the "how does a Planner connection's ValueStore ever get resolvable ValueIds" question this plan's Test-B rationale surfaces; flagging for `.planning/todos/pending` if not already tracked.

---
*Phase: 27-session-connection-integrity-hardening*
*Completed: 2026-07-12*

## Self-Check: PASSED

- FOUND: crates/brokerd/tests/harden01_session_integrity.rs
- FOUND: .planning/phases/27-session-connection-integrity-hardening/27-01-SUMMARY.md
- FOUND: crates/brokerd/src/server.rs
- FOUND: crates/brokerd/src/quarantine.rs
- FOUND: cli/caprun/src/main.rs
- FOUND: planning-docs/DESIGN-session-trust-state.md
- FOUND commit: 40addc0 (Task 1)
- FOUND commit: ad402df (Task 2)
- FOUND commit: 100a6dd (Task 3)
- FOUND commit: bf38a76 (Task 4)
- FOUND commit: 8c116c2 (SUMMARY.md)
