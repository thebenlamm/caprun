---
phase: 05-runtime-spine-live-9-email-block
plan: "02"
subsystem: brokerd/server
tags: [reference-monitor, ipc-dispatch, genuine-taint, fail-closed, hard-03, acc-02, asm-04, security-core]
status: complete

dependency_graph:
  requires:
    - brokerd::proto::WorkerClaim (Plan 01)
    - brokerd::proto::BrokerRequest::ReportClaims (Plan 01)
    - brokerd::proto::BrokerResponse::ClaimsReceived (Plan 01)
  provides:
    - brokerd::server::run_broker_server (new 5-arg signature, no shared ValueStore)
    - brokerd::server::dispatch_request (pub, per-request, testable)
    - per-connection ValueStore isolation (HARD-03)
    - sink_blocked durable fail-closed audit path (ACC-02)
  affects:
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/proto.rs (SubmitPlanNode loses session_id)
    - crates/brokerd/src/lib.rs (invariant annotation)
    - crates/brokerd/Cargo.toml (adapter-fs dep)
    - crates/brokerd/tests/uds_ipc.rs (call sites updated)
    - crates/brokerd/tests/phase5_dispatch.rs (new)

tech_stack:
  added:
    - adapter-fs (path dep on brokerd — pass_fd now lives on the broker path)
  patterns:
    - per-connection state threading (last_event_id / last_event_hash by &mut)
    - extracted async dispatch_request for socket-free unit testing (UnixStream::pair)
    - fail-closed audit append via ? propagation (no best-effort if-let swallow)
    - genuine-taint mint via the sole quarantine::mint_from_read site (anti-stapling)

key_files:
  created:
    - crates/brokerd/tests/phase5_dispatch.rs
  modified:
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/lib.rs
    - crates/brokerd/Cargo.toml
    - crates/brokerd/tests/uds_ipc.rs

decisions:
  - "Tasks 1 and 2 committed together (2eab79b): removing SubmitPlanNode.session_id from proto.rs breaks the old server.rs dispatch immediately, so the proto field removal and the server rewrite are inseparable for a green build. Splitting them would require an intentionally-broken intermediate commit."
  - "Executed INLINE on main (not via an isolated worktree subagent): two consecutive gsd-executor subagent attempts died on API streaming connection drops (~17-23 min in, zero commits). The user approved switching to inline execution on the stable main connection."
  - "ReportRead downgraded to a deprecated arm returning an Error directing callers to ReportClaims — the variant stays in proto for wire compatibility but is no longer a live broker path."
  - "Added adapter-fs as a brokerd dependency so the RequestFd arm can call pass_fd directly (it previously lived only in cli/caprun). pass_fd/recv_fd are plain Unix (nix), not Linux-gated, so brokerd still compiles cross-platform."
  - "Annotated the pre-existing EffectRequest doc-comment mention in lib.rs with (planner-discipline-allow) on the same line — check-invariants Gate 1 is line-based. This closes the out-of-scope debt flagged by Plan 01; Task 2's verify requires Gate 1 green."

metrics:
  completed_date: "2026-06-30"
  tasks_completed: 3
  files_created: 1
  files_modified: 5
---

# Phase 05 Plan 02: Unified Session-Scoped Fail-Closed Broker Dispatch

The security-critical core of Phase 5. The broker's dual dispatch is collapsed into one stateful `brokerd::server` path — the live, session-scoped, fail-closed reference monitor. `RequestFd`, `ReportClaims` (genuine-taint mint), and a durable `sink_blocked` `SubmitPlanNode` handler all route through one `dispatch_request`.

## What Was Built

### `run_broker_server` — new signature, no shared store
Dropped the `value_store: Arc<Mutex<ValueStore>>` parameter; added `session_id_uuid: Uuid`, `initial_last_event_id: Uuid`, `initial_last_event_hash: String`. The accept loop spawns a per-connection `handle_connection`, cloning the initial chain hash per connection.

### `handle_connection` — per-connection state (HARD-03)
Each connection owns a fresh `ValueStore::default()` and threads `last_event_id` / `last_event_hash` (by `&mut`) across every message. Keeps the 4-byte LE framing, the 64 KiB `MAX_MSG_SIZE` guard, and the generic deserialize-error response. Delegates each message to `dispatch_request`.

### `dispatch_request` (pub, extracted) — the unified dispatch
- **CreateSession** — unchanged behavior (own fresh session, `parent_id: None`, does not thread the connection chain) so `uds_ipc` tests still pass.
- **RequestFd** — opens the file (ambient fs), appends a causal `fd_granted` event (`parent_id: Some(prior)`), passes the fd via `adapter_fs::pass_fd` inside `tokio::task::spawn_blocking`, then advances the chain.
- **ReportClaims** — exhaustively matches `WorkerClaim::EmailAddress`, builds a `quarantine::Claim`, and calls `mint_from_read` (the SOLE mint site — no direct `ValueStore::mint`). Advances the chain to the returned read event and pushes the `ValueId`; replies `ClaimsReceived`.
- **SubmitPlanNode** — evaluates via `executor::submit_plan_node(session_id, …)` using the **connection** session_id. Chooses `sink_blocked` (on `BlockedPendingConfirmation`) vs `plan_node_evaluated`, appends with `Some(parent)` and `?` propagation (fail-closed) BEFORE the decision is sent; advances the chain only on a durable append.
- **ReportRead** — deprecated arm → `Error` directing callers to `ReportClaims`.

### proto.rs — `SubmitPlanNode` loses `session_id` (HARD-03)
Now `SubmitPlanNode { plan_node }`. Doc comment states the broker uses the connection-established identity and never trusts a message-supplied one (T-05-03).

### Tests — `crates/brokerd/tests/phase5_dispatch.rs` (new, cross-platform)
Six tests, runnable on macOS (no abstract sockets, no confinement; `UnixStream::pair` for the live arms):
1. `mint_anchors_provenance_to_file_read_event` (ASM-04)
2. `handle_from_other_connection_store_is_denied` (HARD-03)
3. `submit_plan_node_has_no_session_id_field` (HARD-03 — serde proof)
4. `block_appends_durable_causal_sink_blocked` (ACC-02 — via real `dispatch_request`)
5. `append_failure_is_fail_closed` (ACC-02 — forced append failure → `Err`, chain head unchanged)
6. `report_claims_variant_constructs` (wire-type smoke)

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build -p brokerd --tests` | green |
| `cargo test -p brokerd --test phase5_dispatch` | 6/6 passed |
| `cargo test -p brokerd --test uds_ipc` | 0 tests (Linux-gated; expected on macOS) |
| `cargo test -p brokerd --test s9_acceptance` | 1/1 passed (non-negotiable regression) |
| `cargo test --workspace --no-fail-fast` | all green (26 result groups) |
| `./scripts/check-invariants.sh` | Gate 1 + Gate 2 PASS |
| `grep -c "value_store: Arc" server.rs` | 0 |
| `SubmitPlanNode {…session_id}` in proto.rs | 0 |
| `grep -c "mint_from_read" server.rs` | 4 |
| direct `.mint` on live path (non-comment) | 0 |
| `grep -c "sink_blocked" server.rs` | 2 |
| `grep -c "not wired until Plan 05" server.rs` | 0 |
| `parent_id: None` in server.rs | 1 (CreateSession root only) |

## Deviations from Plan

1. **Tasks 1 & 2 committed together** (`2eab79b`) — proto field removal + server rewrite are inseparable for a green build (see decisions). Task 3 committed separately (`5c25c53`).
2. **Inline execution on `main`** instead of a worktree subagent — two consecutive executor subagents died on API streaming connection drops with zero commits; the user approved the inline fallback. Commits are atomic and hook-verified; STATE/ROADMAP are updated by the orchestrator.
3. **adapter-fs added to brokerd Cargo.toml** (not in the plan's `files_modified`) — required to call `pass_fd` from the broker `RequestFd` arm. Cross-platform (nix), so the workspace still builds on macOS.
4. **lib.rs invariant annotation** (not in `files_modified`) — closed the pre-existing Gate 1 `EffectRequest` debt flagged by Plan 01, which Task 2's verify gate requires.

## Threat Surface Scan
- **T-05-03 (spoofing session identity):** mitigated — `session_id` removed from `SubmitPlanNode`; evaluation uses the connection identity. Serde test proves the field's absence.
- **T-05-04 (cross-session handle resolution):** mitigated — per-connection `ValueStore`; cross-store handle → `Denied` (test 2).
- **T-05-05 (taint stapling):** mitigated — `mint_from_read` is the only mint; anti-stapling grep + provenance-anchor test.
- **T-05-06 (audit evasion via best-effort append):** mitigated — `?`-propagated append; decision sent only after durable write; fail-closed test asserts `Err`.

## Notes for Plan 03
- `cli/caprun/src/main.rs` still contains `handle_worker_connection` + its local `send_response`. Plan 03 deletes both and points `caprun` at `brokerd::server::run_broker_server` (new 5-arg signature), and upgrades the worker to the `ReportClaims` → `SubmitPlanNode` protocol.
- The Linux-gated `e2e.rs` substrate tests still encode the old `ReportRead` byte-count actor protocol and the 3-event benign chain — Plan 03 updates them.

## Self-Check: PASSED
- `crates/brokerd/src/server.rs` — unified `dispatch_request`, `mint_from_read`, `sink_blocked`, no shared `value_store` param.
- `crates/brokerd/tests/phase5_dispatch.rs` — exists, 6 tests, all pass.
- Commits `2eab79b`, `5c25c53` present in git log.
