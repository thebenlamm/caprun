# Phase 27: Session & Connection Integrity Hardening - Research

**Researched:** 2026-07-12
**Domain:** Rust TCB hardening — broker session-lifecycle integrity (I1 demotion timing, compile-time feature exclusion, cross-connection trust-state coherence)
**Confidence:** HIGH (all file:line anchors re-verified against live source this session; zero drift found from the DESIGN doc's authoring-time anchors)

## Summary

Phase 27 has no external-library research surface — it is pure Rust-TCB surgery inside `crates/brokerd/src/server.rs`, gated by a design doc (`planning-docs/DESIGN-security-hardening.md` §a/§d/§f) that already cleared a fresh adversarial review (Round 1: F1/F2/F3 folded, CLEARED). This RESEARCH.md's job is narrow and concrete: **re-verify every anchor the DESIGN doc cites against live source, and surface what has changed or been under-specified since Phase-26 authoring time.**

Result: **zero anchor drift.** Every file:line citation in the DESIGN doc (`server.rs:996-1047` RequestFd arm, `server.rs:904-994` CreateSession arm, `server.rs:149/202/231` session_status seeding, `main.rs:149/183/187/238/301` workspace_root/workspace_rel derivation, `workspace.rs:90` read_within, `quarantine.rs:376` stale doc comment, `executor/Cargo.toml:22/28/36` feature precedent) is byte-accurate against the current tree. This is expected — Phase 26 landed immediately before Phase 27 with no intervening commits to `crates/`.

Two things this research surfaces that the DESIGN doc's blast-radius notes did **not** spell out at the same level of concreteness, and that materially change Phase 27's task list:

1. **`dispatch_request` is `pub` and called directly (not just through `handle_connection`) by 4 external integration-test files** (`durable_anchor.rs`, `extract_provenance_threading.rs`, `phase5_dispatch.rs` ×2, `proto_claims.rs` ×2) plus 5 in-`server.rs` unit tests. Any signature change to `dispatch_request` (new trusted-path param for HARDEN-01, `Arc<Mutex<SessionStatus>>` for the X-04 fold) breaks all of these call sites and they **must** be updated in the same PR, not discovered as `cargo test` fallout.
2. **The `fstat (st_dev, st_ino)` identity compare F2 requires needs no new crate dependency.** `crates/brokerd/Cargo.toml` does not depend on `nix` today (it reaches fd-passing only indirectly via `adapter-fs::pass_fd`). Rust's own `std::os::unix::fs::MetadataExt` (`file.metadata()?.dev()` / `.ino()`) gives the inode identity compare with **zero new dependencies** — prefer this over adding `nix` to `brokerd/Cargo.toml`, which would be an unnecessary blast-radius increase Gate reviewers would flag.

**Primary recommendation:** Implement HARDEN-01's demotion and identity-compare entirely inside the existing `RequestFd` arm (`server.rs:996-1047`) using `std::fs::File::metadata()` (no new crate); implement HARDEN-04 by copying `crates/executor/Cargo.toml`'s `[features] test-fixtures = []` block verbatim into `crates/brokerd/Cargo.toml`; fold X-04's `Arc<Mutex<SessionStatus>>` conversion into the SAME PR since it touches the identical `dispatch_request`/`handle_connection`/`run_broker_server` signatures HARDEN-01 already needs to change — do not sequence these as three independent signature-touching passes over the same functions.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HARDEN-01 | fd release to the confined worker (`RequestFd`) itself demotes the session to draft-only for the I1 reason, without breaking the CONTROL-01 benign clean path | Anchor table below pins the exact `RequestFd` arm insertion point (`server.rs:1001-1011`, between `*fd_requested = true` and `pass_fd`), the fstat-based `is_trusted_labeled` mechanism (no new crate needed — `std::os::unix::fs::MetadataExt`), the causal-edge parenting rule, and the 4 external test files whose `dispatch_request` call sites must be updated for the new param. |
| HARDEN-04 | `CreateSession`-IPC forced-`Active` mint arm excluded from the production build at compile time (cfg), not merely runtime-gated | `executor/Cargo.toml`'s `test-fixtures` feature (verbatim precedent, re-confirmed at `:22/:28/:36`) + `sink_schema.rs`'s dual-`#[cfg]` sibling-arm pattern (re-confirmed at `:75/:93/:98`) are the exact templates to copy. The 3 `uds_ipc.rs` tests that exercise the arm are enumerated by name below, with the empirical verification step D-10/A4 demands. |

(X-04's `Arc<Mutex<SessionStatus>>` fix is folded into HARDEN-01's scope per the DESIGN doc §f ruling — no separate requirement ID was minted; see ROADMAP.md Phase 27 success criteria, which do not enumerate it as a distinct criterion but the DESIGN doc requires it as part of HARDEN-01's "session/connection-lifecycle surface" scope.)
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **TCB is Rust; I2/I1/I0 are hardcoded, never a swappable policy file.** HARDEN-01's `is_trusted_labeled` and the `Arc<Mutex<SessionStatus>>` fix must be plain Rust control flow — no config-file-driven trust list, no runtime-toggle for the trusted-path set beyond the CLI-supplied `workspace_rel` itself.
- **Effect path is locked** — `PlanNode { sink, args: Vec<ValueNode> }` only; never introduce a raw `EffectRequest { effect, args: Map }`. Phase 27 touches no `PlanNode`/`ExecutorDecision` shape at all (confirmed: HARDEN-01/04 are entirely in `server.rs`'s connection/session plumbing and `Cargo.toml`, not the executor's decision surface) — this constraint is a non-issue for this phase but `check-invariants.sh` Gate 1 (no `EffectRequest` token) and Gate 3 (mint-call-site restriction to `quarantine.rs`/`server.rs`/`value_store.rs`) still run and must stay green; §a's demotion reuses `update_session_status` (an existing, already-whitelisted call — see `session.rs:89`), so **Gate 3 needs no new exemption.**
- **Linux-only security tests show "0 passed" on macOS by design** (cfg-gated). All Phase 27 enforcement tests (the new fd-grant demotion test, the featureless-build negative test) must be `#[cfg(target_os = "linux")]`-gated and run via `bash scripts/mailpit-verify.sh`, never the bare `docker run rust:1` recipe (project has been on the Mailpit recipe since Phase 16; Phase 27 doesn't touch `email.send` but the standing rule is "ALL Linux verification" — HARDEN-04's featureless-release-build check is a **separate, additional** command, not a substitute for the Mailpit-wrapped `cargo test`).
- **Surgical changes only** — every changed line traces to HARDEN-01/HARDEN-04's DESIGN-doc mechanism. Do not touch `email.send` CAS (HARDEN-03, Phase 29), `verify_chain` (HARDEN-02, Phase 28), or `file.create` `contents` (HARDEN-05, Phase 29) in this phase.
- **check-invariants.sh runs before any code** — re-run after every Phase 27 task; the script is unchanged by this phase (no new gate needed — DESIGN doc §i confirms neither Gate 1 nor Gate 3 needs updating for HARDEN-01/04).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| fd-grant-time I1 demotion (HARDEN-01) | Broker (reference monitor / control plane) | Sandbox (the fd itself is kernel-mediated via `openat2`) | The demotion decision and its atomic DB write are broker-owned state; the fd being demoted-around is opened through the kernel-enforced `RESOLVE_BENEATH` path, but the trust decision is pure broker logic — never delegated to the sandbox or the worker. |
| Trusted-path identity compare (`is_trusted_labeled`) | Broker | — | Broker-derived `fstat` compare against the CLI-designated `workspace_rel`; must never be inferred from a worker-supplied string (anti-self-declaration, D-01/D-03). |
| `CreateSession` compile-time exclusion (HARDEN-04) | Broker (build/compile boundary) | — | A Cargo feature gate is a build-tier concern, not a runtime broker decision — the whole point is removing the arm from the compiled artifact, which no runtime tier can enforce. |
| Cross-connection `session_status` visibility (X-04 fold) | Broker | — | Shared in-process state (`Arc<Mutex<SessionStatus>>`) across tasks within one `run_broker_server` invocation — purely a broker-internal concurrency-correctness fix, no other tier involved. |
| CLI-side `workspace_rel` derivation | CLI orchestrator (`cli/caprun/src/main.rs`) | Broker (consumer) | The CLI already owns this determination (parses `argv`, derives `workspace_root`/`workspace_rel`); Phase 27 adds a NEW forwarding hop of an already-computed value into the broker, not a new CLI-side computation. |
| Confined worker | Sandbox / Worker | — | Out of scope for Phase 27 changes — the worker's `RequestFd`/`ReportClaims` wire behavior is unchanged; only the broker's server-side reaction to `RequestFd` changes. |

## Standard Stack

No new external crates. This phase adds:

| Change | Location | Mechanism | Why no new dependency |
|--------|----------|-----------|------------------------|
| `[features] test-fixtures = []` + self dev-dependency | `crates/brokerd/Cargo.toml` | Verbatim copy of `crates/executor/Cargo.toml`'s existing shape (`:22/:28/:36`) | Cargo-native feature mechanism; zero new crates. |
| Inode identity compare | `crates/brokerd/src/server.rs` (RequestFd arm) | `std::os::unix::fs::MetadataExt` (`.dev()`/`.ino()` on `std::fs::Metadata`) applied to the already-open `std::fs::File` (`file.metadata()`) | **[VERIFIED: codebase]** `crates/brokerd/Cargo.toml` has no `nix` dependency today; the fd is already a `std::fs::File` (from `workspace_root.read_within`), and `std::fs::File::metadata()` is stable std, no crate needed. Avoid adding `nix` to `brokerd` purely for this — it would be a new, avoidable dependency-graph change a Gate reviewer would flag as unnecessary blast radius. |

**Version verification:** N/A — no package-registry lookup applies; this phase's "stack" is 100% Rust std library + existing workspace crates already in the dependency graph (`runtime-core`, `adapter-fs`, `rusqlite`, `tokio`, `uuid`, `chrono` — all pre-existing, versions unchanged).

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `std::os::unix::fs::MetadataExt::dev()/ino()` | `nix::sys::stat::fstat` (already a workspace-pinned dep at `nix = "0.31.3"`, used by `adapter-fs`/`sandbox`) | `nix::fstat` returns a `libc::stat` with the same `st_dev`/`st_ino` fields but requires adding `nix` to `crates/brokerd/Cargo.toml` — a new dependency edge for no functional gain over std. Use std unless a future phase needs other `nix::sys::stat` fields. |
| Extending `WorkspaceRoot::read_within`'s return type to also surface the resolved path (the DESIGN doc's "MAY" option) | Doing the fstat compare entirely in `server.rs` against the already-obtained `std::fs::File` | The DESIGN doc marks the inode compare as **preferred** over extending `read_within`; `server.rs` already holds the opened `File` (`file` local, `server.rs:1008-1011`) and a second `File` opened once at startup for `workspace_rel` — no `adapter-fs` API change needed at all, keeping the blast radius inside `server.rs`. |

## Package Legitimacy Audit

**Not applicable — no external packages are introduced by this phase.** Phase 27 adds a Cargo `[features]` block (a manifest-only construct, not a dependency) and uses `std::os::unix::fs::MetadataExt` (part of the Rust standard library, ships with the toolchain, not a registry package). No `npm view`/`pip index`/`cargo search` verification is meaningful here. If a future task in this phase's plan proposes adding `nix` to `crates/brokerd/Cargo.toml` for the fstat compare, flag it as an **avoidable** dependency-graph change per the Alternatives table above, not a legitimacy concern (the `nix` crate itself is already a vetted, pinned workspace dependency used elsewhere — `nix = "0.31.3"` in the root `Cargo.toml`'s `[workspace.dependencies]`).

## Re-Verified Anchor Table (the load-bearing research output)

Every anchor below was re-read from live source this session (not trusted from the DESIGN doc's authoring-time citation). **Result: no drift on any anchor** — Phase 26 landed immediately before this research with zero intervening commits to `crates/`/`cli/`.

| DESIGN doc citation | Current live-source location | Status | Note for planner |
|---|---|---|---|
| `RequestFd` arm, `server.rs:996-1047` | `server.rs:996` (`BrokerRequest::RequestFd { path } =>`) through `:1047` | **CONFIRMED, unchanged** | `*fd_requested = true` is at `:1001`. `read_within` call + `file_fd` capture at `:1008-1011`. `fd_granted` Event append at `:1014-1030`. `pass_fd` (spawn_blocking) at `:1032-1040`. Insert the identity-compare + demotion **between `:1011` (fd obtained) and `:1032` (pass_fd)** — this is the corrected F2 ordering (open → fstat compare → demote-if-untrusted → pass_fd). |
| `session_status` never read/mutated in RequestFd arm | Confirmed by direct read of `:996-1047` | **CONFIRMED** | No `session_status` token appears anywhere in the current RequestFd arm body. |
| `mint_from_read`'s "SOLE I1 trust-flip site" doc comment | `crates/brokerd/src/quarantine.rs:376` (comment), demotion write at `:380` (`update_session_status(conn, session_id, &SessionStatus::Draft)`) | **CONFIRMED — MUST fix in Phase 27's PR** | Exact text: "This makes `mint_from_read` the SOLE I1 trust-flip site". Also note the `session_demoted` Event append at `:381-392` uses literal string `"session_demoted"` — Phase 27's new fd-grant demotion event MUST reuse this exact `event_type` string, not mint a new one, so `verify_chain`/audit tooling that filters on `"session_demoted"` keeps working. |
| `update_session_status` helper | `crates/brokerd/src/session.rs:89` | **CONFIRMED** | Already the exact function the fd-grant demotion should call — signature: `update_session_status(conn: &Connection, session_id: Uuid, status: &SessionStatus)`. Already whitelisted for `check-invariants.sh` Gate 3 purposes (it is not a `mint_from_read`/`.mint(` call, so Gate 3 doesn't apply to it at all). |
| `CreateSession` arm, `server.rs:904-994` | `server.rs:904` through `:994` | **CONFIRMED, unchanged** | Env-gate check at `:923-939` (exact-string `"1"` match, confirmed). Mint body (`create_session`, `persist_session`, `append_event`) at `:941-993`. |
| `crates/brokerd/Cargo.toml` has no `[features]` | Confirmed — file has `[package]`, `[dependencies]`, `[dev-dependencies]` only, no `[features]` block | **CONFIRMED** | `sha2`/`hex` already present in `[dependencies]` (for a **later** phase's HMAC — irrelevant to Phase 27 but note them as already there). `nix` is **NOT** present (see Standard Stack section above). |
| `crates/executor/Cargo.toml`'s `test-fixtures` precedent | `:22` (`[features]` header), `:28` (`test-fixtures = []`), `:36` (self dev-dep `executor = { path = ".", features = ["test-fixtures"] }`) | **CONFIRMED, exact line numbers match** | Copy this 3-line shape verbatim into `crates/brokerd/Cargo.toml`, substituting `brokerd` for `executor`. |
| `sink_schema.rs` dual-`#[cfg]` sibling-arm precedent | `:75` (`#[cfg(any(test, feature = "test-fixtures"))]`), `:93` (same, on `fn test_schema_for`), `:98` (`#[cfg(not(any(test, feature = "test-fixtures")))]` sibling) | **CONFIRMED, exact line numbers match** | This is the "same behavior on the negative path, only physical presence changes" template — copy the `#[cfg(...)] / #[cfg(not(...))]` sibling-function pattern for the `CreateSession` arm split. |
| 3 `uds_ipc.rs` tests exercising the forced-Active arm under `CREATE_SESSION_ENV_LOCK` | `server_accept` (`:63`), `create_session_round_trip` (`:124`), `create_session_over_ipc_denied_by_default_when_flag_unset` (`:214`) | **CONFIRMED — exactly 3, names verified** | All 3 currently rely on `CAPRUN_ENABLE_IPC_CREATE_SESSION` env var (set/unset). Under HARDEN-04, `server_accept` and `create_session_round_trip` need the `test-fixtures` feature to reach the arm at all (the env-gate itself may be removed/replaced); `create_session_over_ipc_denied_by_default_when_flag_unset` should be re-purposed or joined by the NEW D-10 featureless-build negative test — decide explicitly in the plan which of these 3 keep the env-var check vs. move to feature-gating (do not silently drop coverage — DESIGN doc's own risk callout). |
| `planner_capability_split.rs` — only references the `CreateSession` variant | Confirmed: `crates/brokerd/tests/planner_capability_split.rs:68-69,333-334` reference `BrokerRequest::CreateSession {...}` to test `ConnectionRole::Planner::permits()` denial — never actually exercises the forced-Active mint arm | **CONFIRMED, unaffected either way** | No change needed here for HARDEN-04. |
| `cli/caprun`'s dependency on `brokerd` — no explicit features | `cli/caprun/Cargo.toml:9` (`brokerd = { path = "../../crates/brokerd" }`, no `features = [...]`) | **CONFIRMED** | Re-confirm after adding `[features]` to `brokerd/Cargo.toml` that `cli/caprun` still builds without `test-fixtures` unless explicitly opted in — this dependency line needs NO change for Phase 27 to work correctly (default features = none). |
| `audit_path` free-form CLI arg | `cli/caprun/src/main.rs:149` (`let audit_path = args.next().unwrap_or_else(...)`) | **CONFIRMED** | Not Phase 27's concern (F1/HARDEN-02 is Phase 28) — noted only so the planner doesn't accidentally pull F1's fix forward. |
| `workspace_root_dir`/`workspace_rel` derivation | `main.rs:182-189` (`ws_path.parent()` at `:183`, `ws_path.file_name()` → `workspace_rel` at `:187`) | **CONFIRMED, exact line numbers match** | `workspace_rel` is an `&OsStr` (from `file_name()`), currently forwarded ONLY via `.env("WORKSPACE_FILE", workspace_rel)` at `:301`. Phase 27 needs a SECOND forwarding: into the `run_broker_server(...)` call at `:238-247` (currently 7 positional args ending in `ws_root_for_broker`) — add an 8th param (an owned `PathBuf`/`OsString`/`String`, not a borrowed `&OsStr`, since it must cross into a `tokio::spawn`ed `'static` future exactly like `ws_root_for_broker` does). |
| `run_broker_server` call site | `main.rs:237-248` | **CONFIRMED, exact lines match** | 7 args today: `&session_id.to_string()`, `conn_clone`, `session_id`, `session_created_id`, `session_created_hash`, `initial_session_status`, `ws_root_for_broker`. Add the new trusted-path param here. |
| `run_broker_server` signature | `server.rs:143-151` | **CONFIRMED, exact lines match** | `initial_session_status: SessionStatus` at `:149` (owned, not `Arc`). This is the X-04 fold's central target: convert to `Arc<Mutex<SessionStatus>>`, constructed ONCE inside `run_broker_server` (or passed in already-wrapped from `main.rs` — DESIGN doc says "seeded `Active` once at `run_broker_server` entry", so construct the `Arc<Mutex<_>>` INSIDE `run_broker_server` from the owned `initial_session_status` param, mirroring how `planner_slot_occupied: Arc<AtomicBool>` is constructed inside the function at `:185`, not passed in pre-wrapped). |
| Per-connection `session_status` clone-seeding | `server.rs:202` (Worker connection) and `:231` (every subsequent/Planner connection) — both `let initial_status = initial_session_status.clone();` | **CONFIRMED, exact lines match** | Under the X-04 fix, these two lines change from `.clone()`-ing an owned `SessionStatus` to `.clone()`-ing the `Arc` handle (cheap pointer clone, not a value clone) — passed into `handle_connection`/`classify_second_connection` as the shared cell, never re-seeded as a fresh owned value. |
| `handle_connection` signature | `server.rs:393-401` | **CONFIRMED, exact lines match** | `initial_session_status: SessionStatus` param (owned) at `:399`; internally becomes `let mut session_status = initial_session_status;` at `:408` (grep-confirmed). Must change to accept the shared `Arc<Mutex<SessionStatus>>` and NOT create a fresh per-connection owned local. |
| `classify_second_connection` signature | `server.rs:280-289` | **CONFIRMED, exact lines match** | `session_status: SessionStatus` param (owned) at `:286`. Same conversion as `handle_connection`. |
| `dispatch_request` signature | `server.rs:890-902` | **CONFIRMED, exact lines match** | `session_status: &mut SessionStatus` at `:899`. Convert to a shared-cell reference (e.g. `session_status: &Arc<Mutex<SessionStatus>>`), **re-read (locked) at the top of the function** per the DESIGN doc's explicit "re-read at the top of every `dispatch_request`" pin — do not just thread the Arc through unread. |
| **NEW FINDING — `dispatch_request` external callers** | `crates/brokerd/tests/durable_anchor.rs:156`, `extract_provenance_threading.rs:259`, `phase5_dispatch.rs:199` AND `:288`, `proto_claims.rs:129` AND `:363` — 6 direct call sites across 4 files, all outside `server.rs` | **NOT enumerated at this granularity in the DESIGN doc's blast-radius note** | `dispatch_request` is `pub` and these integration tests call it directly (bypassing `handle_connection`), each currently passing `&mut session_status` (an owned local) as one of 11 positional args. **Every one of these 6 call sites must be updated** for both the new trusted-path param (HARDEN-01) and the `Arc<Mutex<SessionStatus>>` conversion (X-04) — budget this explicitly as its own task, don't discover it via compile errors mid-plan. |
| `RequestFd`'s in-`server.rs` unit-test callers | `server.rs:1521, 1558, 1578, 1629, 1658` (5 in-module `#[cfg(test)]` call sites) | **CONFIRMED** | Same signature-change impact as the external files above, but inside `server.rs`'s own test module — lower risk (same file, same PR, compiler catches immediately) but still explicit work. |
| `read_within`, `crates/adapter-fs/src/workspace.rs:90` | `:90` (`pub fn read_within(&self, rel_path: &str) -> std::io::Result<std::fs::File>`) | **CONFIRMED, exact line matches; F2's premise holds** | Returns `File` only, discards the resolved path; `root_path()` at `:71` returns the root, not a per-file resolved target — confirms F2's finding that there is no "canonical form already computed" to reuse. No change to `workspace.rs` is required for Phase 27 (the DESIGN doc's F2 resolution makes the inode-fstat compare live entirely in `server.rs` against the `File` it already has). |
| `Cargo.toml` root workspace `nix` dependency | Root `Cargo.toml:17` — `nix = { version = "0.31.3", features = ["fs","socket","resource","process","signal","uio"] }` | **CONFIRMED** | Pinned centrally; `adapter-fs`/`sandbox` depend on it via `nix = { workspace = true }`. `brokerd` does not list `nix` at all today (see Standard Stack section). |
| `SessionStatus` enum — `Clone`/`PartialEq`/serde derives | `crates/runtime-core/src/session.rs:17` — `#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]` | **CONFIRMED** | Cheap to wrap in `Arc<Mutex<_>>`; no derive changes needed. Existing monotonic `Active -> Draft` transition rule (`DESIGN-session-trust-state.md` §1) is unaffected by the wrapping — only the SHARING mechanism changes, not the state machine. |
| `planner_slot_occupied: Arc<AtomicBool>` — the X-04 fix's cited precedent | `server.rs:185` (construction), `:288`/`:302` (usage in `classify_second_connection`) | **CONFIRMED, exact lines match** | This is the SAME sharing pattern to mirror for `session_status`, except `SessionStatus` is a multi-variant enum (not a bool), so it needs `Arc<Mutex<SessionStatus>>` rather than `Arc<AtomicBool>`/`Arc<AtomicU8>`. |
| `s9_control_ab` / CONTROL-01 regression test location | `cli/caprun/tests/live_acceptance_v1_3.rs`, `cli/caprun/tests/s9_live_block.rs` (both reference CONTROL-01/`s9_control_ab`-style flows) | **CONFIRMED present** | These are Linux-only, Mailpit-gated tests — Phase 27 does not need to touch them directly (Phase 30 owns the new negative test), but Phase 27's PR must not regress them; run via `scripts/mailpit-verify.sh` before calling Phase 27 done if time allows, or explicitly hand off the live-Linux gate to Phase 30 per the roadmap's phase boundary. |

## Architecture Patterns

### System Architecture Diagram

```
CLI (cli/caprun/src/main.rs)
  │
  │ 1. derives workspace_root_dir + workspace_rel from <workspace-file> arg (:182-189)
  │ 2. create_session() → initial_session_status (Active or Draft per I0)
  │ 3. spawns run_broker_server(..., initial_session_status, ws_root, [NEW] workspace_rel)
  │
  ▼
run_broker_server (server.rs:143)                    ┌─ [NEW] Arc<Mutex<SessionStatus>>
  │  constructs shared session_status cell ───────────┤    constructed ONCE here, seeded
  │  (mirrors planner_slot_occupied: Arc<AtomicBool>)  │    from initial_session_status
  │                                                     └─ monotonic Active→Draft only
  │
  ├─ accept loop: Worker connection (1st) ──► handle_connection(..., Arc<Mutex<SessionStatus>>, ConnectionRole::Worker)
  │                                                │
  │                                                ▼
  │                                          dispatch_request per message
  │                                                │
  │                                                ├─ RequestFd { path } arm:
  │                                                │    *fd_requested = true
  │                                                │    file = workspace_root.read_within(path)  [existing]
  │                                                │    [NEW] fstat(file) vs fstat(workspace_rel)
  │                                                │    [NEW] if !same-inode: lock session_status,
  │                                                │           write Draft, append session_demoted
  │                                                │           Event (parent = fd_granted id)
  │                                                │    pass_fd(file) to worker  [existing, AFTER demotion]
  │                                                │
  │                                                └─ CreateSession arm:
  │                                                     [CHANGED] #[cfg(any(test,feature="test-fixtures"))]
  │                                                       body = forced-Active mint (unchanged logic)
  │                                                     [NEW] #[cfg(not(any(test,feature="test-fixtures")))]
  │                                                       sibling = same Error response, no mint
  │
  └─ accept loop: subsequent connections ──► classify_second_connection(..., Arc<Mutex<SessionStatus>>)
                                                   │  DeclarePlannerRole → handle_connection(..., SAME Arc, Planner)
                                                   │
                                                   ▼
                                             dispatch_request re-reads (locks) the SAME
                                             shared session_status at the top of every call —
                                             sees a Worker-connection demotion IMMEDIATELY,
                                             closing the X-04 staleness gap.
```

### Recommended Task Sequencing (informative — planner's call on final task boundaries)

Given the anchor findings above, sequence to minimize repeated signature churn on the same functions:

1. **One signature-change pass** covering `run_broker_server` / `handle_connection` / `classify_second_connection` / `dispatch_request` that (a) threads the new trusted-path param (HARDEN-01) AND (b) converts `session_status` to `Arc<Mutex<SessionStatus>>` (X-04) — in the SAME task, since both changes touch the identical 4 function signatures. Update all 6 external `dispatch_request` call sites + 5 in-module unit-test call sites in this same task.
2. **RequestFd arm mechanism** — implement the fstat identity compare + demotion-before-pass_fd, using the shared cell threaded in step 1.
3. **CreateSession compile-exclusion** — `[features] test-fixtures` in `Cargo.toml`, dual-`#[cfg]` arm split, verify (empirically, per A4) which of the 3 `uds_ipc.rs` tests still compile/pass and whether `test-fixtures` needs adding to their build.
4. **Doc-comment + design-doc amendment** — fix `quarantine.rs:376`'s stale "SOLE I1 trust-flip site" comment and amend `DESIGN-session-trust-state.md` §2/§5 in the same PR (DESIGN doc explicitly requires same-PR, not a follow-up).
5. **New tests** — fd-grant demotion on an untrusted path with no `ReportClaims` (negative), a Planner-connection-after-Worker-demotion test observing `Draft` (F3's required test), and the D-10 featureless-build behavioral negative test for `CreateSession`.

### Anti-Patterns to Avoid

- **Path-string compare for trust labeling.** F2 already rejected this explicitly — a naive `rel_path == workspace_rel.to_str()` string compare is exactly the "ad-hoc string compare" the DESIGN doc forbids (permissive normalization risk: `./report.txt` vs `report.txt`). Use the fstat inode compare only.
- **Wrapping the Arc<Mutex<>> construction in `main.rs` instead of `run_broker_server`.** The DESIGN doc pins "seeded Active once at `run_broker_server` entry" — constructing it in `main.rs` and passing an already-wrapped `Arc<Mutex<SessionStatus>>` into `run_broker_server` would also satisfy "construct once" but diverges from the explicit precedent (`planner_slot_occupied` is constructed inside `run_broker_server`, not passed in). Prefer matching the precedent unless there's a reason not to — keeps the diff minimal and consistent.
- **Any per-connection code path that writes `Active` into the shared cell.** F3's whole point: connection-setup (today's `.clone()` seeding at `:202`/`:231`) must become a READ of the shared cell, never a write. A refactor that "helpfully" re-initializes the cell per connection re-opens the exact bug being fixed.
- **Adding `nix` to `brokerd/Cargo.toml` for the fstat compare** when `std::os::unix::fs::MetadataExt` already does the job with the `std::fs::File` already in scope.
- **Symbol-inspection (`nm`/`strings`/`objdump`) as the primary D-10 gate.** The DESIGN doc explicitly demotes this to optional defense-in-depth (bit-rots across Rust/LLVM versions); the primary gate is the featureless-build BEHAVIORAL negative test.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Inode identity compare | A custom path-canonicalization + string-compare routine | `std::fs::File::metadata()` + `std::os::unix::fs::MetadataExt::dev()/ino()` | Std library, TOCTOU-safe when applied to an already-open fd (no re-resolution race), zero new dependencies. |
| Compile-time test-only code exclusion | A custom `build.rs` script or env-var-driven `#[cfg]` | Cargo `[features]` + `#[cfg(any(test, feature = "..."))]` / `#[cfg(not(...))]` sibling arms | Already a proven, shipped pattern in this exact codebase (`executor/Cargo.toml` + `sink_schema.rs`) — copy, don't reinvent. |
| Cross-task shared mutable state | A custom lock-free cell, channel-based state machine, or global static | `Arc<Mutex<SessionStatus>>` | Already the exact pattern `planner_slot_occupied: Arc<AtomicBool>` establishes for the same class of problem (cross-task-visible, one-way-transitioning connection state) — a `Mutex` is appropriate here (not `AtomicBool`) because `SessionStatus` is a multi-variant enum, not a single bit. |

**Key insight:** Every mechanism this phase needs already has a shipped precedent in this exact codebase (from a sibling crate or an adjacent field in the same struct). The research task was verification of those precedents' current line numbers, not discovery of new patterns.

## Common Pitfalls

### Pitfall 1: Missing one of the 6+5 `dispatch_request` call sites
**What goes wrong:** A signature change to `dispatch_request` compiles cleanly in `server.rs` but breaks 4 external test files (6 call sites) that the developer didn't think to check because they're not in `crates/brokerd/src/`.
**Why it happens:** `dispatch_request` is `pub`, and Rust's compiler WILL catch every one of these at `cargo test --workspace` time — but only if the developer runs `cargo build --workspace` (or `cargo test --workspace`) rather than a scoped `cargo build -p brokerd`, which would only catch the in-crate unit-test callers, not the external integration-test binaries.
**How to avoid:** Run `cargo test --workspace --no-fail-fast` (per CLAUDE.md's own build instructions) after the signature-change task, not a scoped `-p brokerd` build.
**Warning signs:** Green `cargo build -p brokerd` but red `cargo test --workspace`.

### Pitfall 2: Re-seeding `Active` on the Planner connection path (F3's exact bug)
**What goes wrong:** `classify_second_connection`'s current code clones an "initial status" per-connection (`:231` today); a naive Arc-conversion might keep a similar-looking `.clone()` line that actually re-writes the shared cell to `Active` at connection-setup time, silently reopening X-04.
**Why it happens:** The existing code's shape (`let initial_status = initial_session_status.clone();`) looks like it should just become `let status_handle = session_status_arc.clone();` (an `Arc` clone, cheap pointer copy) — but if the developer instead does something like `*status_handle.lock().unwrap() = initial_session_status.clone()` at connection setup "to be safe," that's a write, and it clobbers a prior demotion.
**How to avoid:** The Arc clone must be READ-ONLY at connection-setup time — no `.lock()...= ` write anywhere in `handle_connection`/`classify_second_connection`'s setup path. Only `dispatch_request`'s existing `ReportClaims` demotion write (and the new RequestFd demotion write) may ever write to the cell, and both only ever write `Draft`.
**Warning signs:** A new test — "Planner connection accepted after Worker demotion observes Draft" — passing for the wrong reason (because it happens to run fast enough that the demotion write races ahead of a bad re-seed) rather than because the re-seed doesn't exist. Write the test to accept the connection in a controlled way that forces the ordering, per the DESIGN doc's own note.

### Pitfall 3: Silently losing `uds_ipc.rs` coverage after the Cargo feature split
**What goes wrong:** Gating the `CreateSession` forced-Active body on `#[cfg(any(test, feature = "test-fixtures"))]` without propagating `test-fixtures` into `uds_ipc.rs`'s effective build means those 3 tests silently hit the NEW fail-closed `#[cfg(not(...))]` sibling arm instead of the mint arm — `create_session_over_ipc_denied_by_default_when_flag_unset` would "pass" for the wrong reason (arm physically absent, not flag-gated), and `server_accept`/`create_session_round_trip` (which expect the mint to succeed) would start failing or silently assert on the wrong response shape.
**Why it happens:** `crates/brokerd`'s `[dev-dependencies]` graph is richer than `executor`'s (the DESIGN doc's own A4 risk callout) — Cargo's self-feature-unification proven for `executor` reaching its own `tests/*` must be independently re-verified for `brokerd`, not assumed by inspection.
**How to avoid:** Actually run `cargo test -p brokerd` after the Cargo.toml change and inspect which of the 3 named tests pass/fail/change behavior — per the DESIGN doc: "MUST be verified empirically in Phase 27 (actually run the tests), not assumed by inspection."
**Warning signs:** All 3 tests "green" without the developer having actually diffed their assertions against pre-change behavior — a false green from hitting the compiled-out sibling arm instead of the mint arm looks identical to a true green if the tests only assert "got an Error response" rather than a specific error message.

### Pitfall 4: Causal-edge parenting mistake for the new fd-grant demotion Event
**What goes wrong:** The new `session_demoted` Event (appended at fd-grant time) is parented on the wrong id — e.g., `None` (a fresh root) or the session's very first event — breaking `verify_chain`'s single-linear-chain walk (forking the DAG).
**Why it happens:** Today's ONLY `session_demoted` event is parented on `file_read.id` (`mint_from_read`'s existing behavior). The NEW fd-grant-time demotion has no `file_read` Event yet (that only exists after `ReportClaims`), so a developer copying the old parenting logic verbatim would try to parent on a `file_read` id that doesn't exist yet.
**How to avoid:** Parent on `fd_granted`'s own id (`fd_event_id`, `server.rs:1014`) if the demotion is appended AFTER `fd_granted`, or on the current chain head (`*last_event_id` before `fd_granted` is appended) if appended BEFORE — per the DESIGN doc's pinned causal-edge rule. Pick one ordering and be consistent; the corrected F2 ordering is: open fd → fstat compare → demote-if-untrusted → THEN append `fd_granted` and `pass_fd`, OR demote immediately after appending `fd_granted` (parenting on `fd_event_id`) — the DESIGN doc names `fd_granted`'s id as the primary target, so append `fd_granted` first, then the demotion Event parented on it, then `pass_fd`.
**Warning signs:** A `verify_chain` test that starts failing with a "chain fork" or "multiple children of one parent" style error only under the new RequestFd-demotion path, not under the existing `ReportClaims` path.

## Code Examples

### Inode identity compare (no new crate — std only)

```rust
// Source: Rust std library (stable), std::os::unix::fs::MetadataExt
// Pattern derived from DESIGN-security-hardening.md §a's corrected F2 pin.
use std::os::unix::fs::MetadataExt;

fn is_same_file(a: &std::fs::File, b: &std::fs::File) -> std::io::Result<bool> {
    let ma = a.metadata()?;
    let mb = b.metadata()?;
    Ok(ma.dev() == mb.dev() && ma.ino() == mb.ino())
}
```

### Cargo feature precedent to copy verbatim (from `crates/executor/Cargo.toml:22,28,36`)

```toml
# Source: crates/executor/Cargo.toml (live, shipped precedent) — re-verified this session
[features]
test-fixtures = []

[dev-dependencies]
brokerd      = { path = ".", features = ["test-fixtures"] }
# ...(existing brokerd dev-dependencies unchanged)
```

### Dual-`#[cfg]` sibling-arm split (from `crates/executor/src/sink_schema.rs:75,93,98`)

```rust
// Source: crates/executor/src/sink_schema.rs (live, shipped precedent) — re-verified this session
#[cfg(any(test, feature = "test-fixtures"))]
fn create_session_arm(/* ... */) {
    // existing forced-Active mint body, UNCHANGED
}

#[cfg(not(any(test, feature = "test-fixtures")))]
fn create_session_arm(/* ... */) {
    // return the SAME Error the runtime flag returns today — identical wire behavior
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| I1 demotion only at `ReportClaims` (worker-optional) | I1 demotion at BOTH `ReportClaims` (`mint_from_read`) AND `RequestFd` (broker-side, unconditional) | Phase 27 (this phase) | A silent/injected worker that skips `ReportClaims` still gets demoted the moment it reads an untrusted path. |
| `CreateSession` IPC gated by `CAPRUN_ENABLE_IPC_CREATE_SESSION` runtime env var | Gated by `test-fixtures` Cargo feature — physically absent from release builds | Phase 27 (this phase) | Removes a whole class of "forgot to unset the env var in prod" risk; the arm cannot exist in a compiled release binary regardless of environment. |
| `session_status` seeded once per connection from a static initial value | `Arc<Mutex<SessionStatus>>` shared across the whole `run_broker_server` invocation, re-read per `dispatch_request` | Phase 27 (this phase) | Closes the X-04 staleness bug where a Planner connection accepted after a Worker demotion still saw a stale `Active` status. |

**Deprecated/outdated:**
- `CAPRUN_ENABLE_IPC_CREATE_SESSION` as the sole gate for the forced-Active mint: superseded by compile-time exclusion, though the env-var check inside the `#[cfg(any(test, feature = "test-fixtures"))]` arm may be RETAINED as defense-in-depth within the test-only build (decide explicitly in the plan — the DESIGN doc does not require removing the env check, only adding the compile-time gate around it).
- `mint_from_read`'s "SOLE I1 trust-flip site" doc comment: becomes false the moment this phase lands; must be corrected in the SAME PR (not a follow-up task).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Using `std::os::unix::fs::MetadataExt` rather than `nix::sys::stat::fstat` is the preferred implementation choice (no new crate dependency) | Standard Stack / Code Examples | Low — this is a std-library API choice with no behavioral difference from `nix::fstat`; if the plan author prefers `nix` for consistency with `adapter-fs`'s style, that's a valid alternative, not a correctness issue. Tag as `[ASSUMED]` only insofar as it is this researcher's engineering judgment, not an explicit DESIGN-doc mandate (the DESIGN doc says "fstat" generically without naming the Rust API). |
| A2 | The `Arc<Mutex<SessionStatus>>` should be constructed inside `run_broker_server` (mirroring `planner_slot_occupied`) rather than passed in pre-wrapped from `main.rs` | Anti-Patterns / Architecture Patterns | Low — either construction site satisfies "construct once" functionally; choosing `run_broker_server` is this researcher's judgment call to match the nearest precedent, not an explicit DESIGN-doc line. If the plan author wraps it in `main.rs` instead, the security property (construct-once, monotonic) still holds — flag as a discretionary implementation detail, not a locked decision. |
| A3 | The new fd-grant `session_demoted` Event should be appended AFTER `fd_granted` (parented on `fd_event_id`) rather than before it (parented on the prior chain head) | Common Pitfalls / Pitfall 4 | Medium — the DESIGN doc names BOTH options ("parent on `fd_granted`'s id... or, if appended before `fd_granted`, on the current chain head") as valid, leaving the exact ordering to Phase 27's plan. Getting this wrong doesn't break security (both are causally valid) but could complicate a later audit-DAG-shape assumption in Phase 28/30 if not documented precisely in the plan. |

## Open Questions

1. **Should the `CAPRUN_ENABLE_IPC_CREATE_SESSION` runtime env-var check be retained INSIDE the `#[cfg(any(test, feature = "test-fixtures"))]` arm, or fully replaced by the feature gate?**
   - What we know: The DESIGN doc's primary requirement is compile-time absence from release builds; it does not explicitly forbid keeping the runtime check as belt-and-suspenders inside the test-only arm.
   - What's unclear: Whether keeping both checks adds meaningful defense-in-depth or just extra surface for the D-10 empirical-verification step to reason about.
   - Recommendation: Retain the env-var check inside the feature-gated arm (cheap, no downside, preserves the existing 3 `uds_ipc.rs` tests' exact assertions with minimal changes) — but this is the plan author's call.

2. **Exact task boundary between Phase 27's own tests and Phase 30's regression/live-proof tests.**
   - What we know: ROADMAP.md's Phase 27 success criteria (4 items) require unit/behavioral evidence within this phase (e.g., "grep/build evidence shows it absent from a default release build"); Phase 30 owns the FULL `mailpit-verify.sh` regression + the 6 named negative tests across all of v1.6.
   - What's unclear: Whether Phase 27's plan should itself run a live-Linux `mailpit-verify.sh` pass, or defer all Linux verification to Phase 30 and rely on macOS `cargo build --workspace`/`cargo test --workspace` (which no-ops the `#[cfg(target_os = "linux")]` tests) for Phase 27's own gate.
   - Recommendation: Phase 27's plan should at minimum run `cargo build --workspace` (macOS, catches signature-mismatch compile errors across all 6+5 `dispatch_request` call sites) and `cargo test --workspace --no-fail-fast` (macOS, catches non-Linux-gated logic bugs), leaving the Linux-only fd-grant-demotion and featureless-release-build proof to be either (a) run once via `scripts/mailpit-verify.sh` at the end of Phase 27 if time/budget allows, or (b) explicitly deferred to Phase 30 per the ROADMAP's phase-dependency structure — state the choice explicitly in the plan rather than leaving it implicit.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo toolchain | All Phase 27 work | check locally before executing | — | N/A (hard requirement) |
| Colima + Docker (`rust:1` image) | Linux-only enforcement tests (`#[cfg(target_os = "linux")]`) via `scripts/mailpit-verify.sh` | Per CLAUDE.md, installed on this dev machine | — | macOS `cargo test --workspace` runs everything else; Linux-gated tests show "0 passed" (expected, not a gap) until run via the script |
| `scripts/mailpit-verify.sh` | Live SMTP-adjacent verification (CONTROL-01 regression check) | Present in repo (confirmed, `scripts/` dir) | — | none needed — script exists |
| `nix` crate (workspace-pinned) | NOT required for Phase 27 if `std::os::unix::fs::MetadataExt` is used | Already present as workspace dep (0.31.3), just not in `brokerd`'s own `Cargo.toml` | 0.31.3 | Add to `brokerd/Cargo.toml` only if the plan author prefers `nix::fstat` over std — optional, not blocking |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:** none — this phase requires nothing beyond what's already available in the repo/toolchain.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (workspace-wide) + Cargo integration-test binaries under each crate's `tests/` dir |
| Config file | none (Cargo.toml `[dependencies]`/`[dev-dependencies]`/`[features]` — no separate test-runner config) |
| Quick run command | `cargo build --workspace` (catches signature-mismatch compile errors fast) then `cargo test -p brokerd --no-fail-fast` (scoped) |
| Full suite command | `cargo test --workspace --no-fail-fast` (macOS — Linux-gated tests no-op); `bash scripts/mailpit-verify.sh` (Linux, full live proof) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HARDEN-01 | `RequestFd` on an untrusted path (no subsequent `ReportClaims`) demotes session to `Draft` | unit/integration (Linux-gated) | `cargo test -p brokerd --no-fail-fast -- request_fd` (new test name TBD by plan) | ❌ Wave 0 — new test to write in Phase 27 |
| HARDEN-01 | Fragment-free clean read on the `workspace_rel` (trusted) path stays `Active`; CONTROL-01 still sends | integration (Linux-gated, Mailpit) | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test live_acceptance_v1_3' bash scripts/mailpit-verify.sh` | ✅ existing (`live_acceptance_v1_3.rs`/`s9_live_block.rs`) — regression check, not new |
| HARDEN-01 (X-04 fold) | Planner connection accepted after Worker demotion observes `Draft`, not stale `Active` | unit/integration | `cargo test -p brokerd --no-fail-fast -- planner_sees_demotion` (new test name TBD) | ❌ Wave 0 — new test (F3-required) |
| HARDEN-04 | Featureless build: `CreateSession` always returns fail-closed `Error`, no opt-in | behavioral negative test, built WITHOUT `test-fixtures` | `cargo test -p brokerd --no-default-features --no-fail-fast -- create_session` (exact invocation TBD — verify `--no-default-features` actually excludes `test-fixtures` given it's not a default feature to begin with; may need a separate non-`cfg(test)` build variant) | ❌ Wave 0 — new test (D-10 primary gate) |
| HARDEN-04 | Existing 3 `uds_ipc.rs` tests still exercise/assert correctly under the new feature gate | regression | `cargo test -p brokerd --features test-fixtures --no-fail-fast -- uds_ipc` | ✅ existing, behavior must be re-verified empirically (A4) |

### Sampling Rate
- **Per task commit:** `cargo build --workspace` (fast compile-error catch across all call sites) + scoped `cargo test -p brokerd`
- **Per wave merge:** `cargo test --workspace --no-fail-fast`
- **Phase gate:** Full suite green on macOS at minimum; Linux `mailpit-verify.sh` pass strongly recommended before marking Phase 27 done (Open Question 2) even though Phase 30 owns the formal milestone-wide regression.

### Wave 0 Gaps
- [ ] New test: fd-grant-time demotion on an untrusted (non-`workspace_rel`) path with no `ReportClaims` — covers HARDEN-01's primary behavior.
- [ ] New test: Planner connection accepted after a Worker-connection demotion observes `Draft` — covers the X-04/F3 fold.
- [ ] New test: featureless-build behavioral negative gate for `CreateSession` (D-10) — covers HARDEN-04's primary proof.
- [ ] Verify (don't assume) whether the existing 3 `uds_ipc.rs` tests need `test-fixtures` added to their effective build to keep passing for the right reason (A4) — may require adding `features = ["test-fixtures"]` to how the test binary is invoked, or confirming Cargo's default dev-dependency unification already covers it (as it does for `executor`).

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A — no user-facing auth surface in this phase |
| V3 Session Management | yes | This phase directly hardens `caprun`'s own session-trust-state model (`SessionStatus` monotonic transitions) — not a web-session analog, but the same ASVS principle (session state must be tamper-resistant and consistently visible) applies. Broker-resolved, never self-declared (existing discipline, extended not weakened). |
| V4 Access Control | yes | `ConnectionRole::Worker`/`Planner` capability restriction (existing, Phase 20) interacts with the X-04 fix — the shared `session_status` cell must remain readable by both roles but writable only from the broker's own demotion logic, never from either connection's own request handling. |
| V5 Input Validation | yes | The `RequestFd { path }` string is worker-controlled input; HARDEN-01's fail-closed default (demote unless positively identified as the trusted path) is itself an input-validation-adjacent control — validate the TRUST of the input's target, not just its syntactic shape (already handled by `RESOLVE_BENEATH`/`RESOLVE_NO_SYMLINKS`). |
| V6 Cryptography | no | Not this phase (HARDEN-02's HMAC work is Phase 28) |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Silent/injected worker skips a self-reported trust-demotion signal | Spoofing / Tampering | Move the demotion trigger to a broker-observable event (fd release) that does not depend on any worker-asserted message — exactly HARDEN-01's mechanism. |
| Stale per-connection copy of shared security state | Tampering (of the security decision's own input, not the audit log) | Shared, monotonically-transitioning state (`Arc<Mutex<SessionStatus>>`), re-read at the point of decision, never cached across the decision boundary — exactly the X-04 fix. |
| Compile-time-vs-runtime gate confusion (a runtime env-var default-deny that still ships the bypass code in the binary) | Elevation of Privilege (if the env var is ever accidentally set in prod, or read from an inherited/untrusted process environment) | Physical absence via Cargo feature/`#[cfg]` exclusion — the code cannot run if it does not exist in the compiled artifact, regardless of environment state. |
| Path-string-based trust labeling (aliasing: `./x` vs `x`, symlinks, case-folding) | Tampering / Spoofing | Kernel-level identity compare (inode `(st_dev, st_ino)`) rather than any string-normalization scheme — immune to path-aliasing tricks by construction. |

## Sources

### Primary (HIGH confidence — direct codebase read, this session)
- `crates/brokerd/src/server.rs` — `RequestFd`/`CreateSession` arms, `run_broker_server`/`handle_connection`/`classify_second_connection`/`dispatch_request` signatures, all `session_status`/`fd_requested`/`planner_slot_occupied` threading, all `dispatch_request` in-module unit-test call sites.
- `crates/brokerd/src/quarantine.rs` — `mint_from_read`, the stale doc comment, `session_demoted` event-type string, `update_session_status` call.
- `crates/brokerd/src/session.rs` — `update_session_status` function signature/location.
- `crates/adapter-fs/src/workspace.rs` — `WorkspaceRoot::read_within`/`root_path`/`create_exclusive_within`, confirms F2's "no canonical path surfaced" premise.
- `crates/brokerd/Cargo.toml`, `crates/executor/Cargo.toml`, root `Cargo.toml` — dependency/feature graph verification (no `nix` in `brokerd`, `test-fixtures` precedent in `executor`, workspace-pinned `nix = "0.31.3"`).
- `crates/executor/src/sink_schema.rs` — dual-`#[cfg]` sibling-arm precedent (`test_schema_for`).
- `crates/brokerd/tests/uds_ipc.rs`, `durable_anchor.rs`, `extract_provenance_threading.rs`, `phase5_dispatch.rs`, `proto_claims.rs`, `planner_capability_split.rs` — external `dispatch_request` call sites and `CreateSession`-arm test coverage.
- `cli/caprun/src/main.rs`, `cli/caprun/Cargo.toml` — `workspace_rel`/`workspace_root_dir` derivation, `run_broker_server` call site, `WORKSPACE_FILE` env forwarding, confirmed no `features` on the `brokerd` dependency.
- `crates/runtime-core/src/session.rs` — `SessionStatus` enum derives.
- `scripts/check-invariants.sh` — Gate 1/Gate 3 mechanics, confirmed no new exemption needed.
- `planning-docs/DESIGN-security-hardening.md`, `planning-docs/DESIGN-GATE-RECORD-v1.6.md`, `planning-docs/DESIGN-session-trust-state.md` — the authoritative design contract for this phase (Phase 26, cleared gate).
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md` — phase requirement text, success criteria, project history/decisions.
- `./CLAUDE.md` — project-specific hard constraints.

### Secondary (MEDIUM confidence)
- None — this phase required no external documentation lookup; all research was direct codebase verification against an already-approved design doc. No web search providers were configured for this session (`brave_search`/`firecrawl`/`exa_search` all `false` in the init context) and none were needed given the phase's fully-internal scope.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new external packages; std-library API choice is well-established Rust knowledge, cross-checked against the actual dependency graph (confirmed `nix` absent from `brokerd`).
- Architecture: HIGH — every anchor re-verified against live source this session; zero drift from DESIGN-doc-authoring-time citations.
- Pitfalls: HIGH — the 4 pitfalls listed are grounded in concrete, re-verified code facts (the 6 external `dispatch_request` call sites, the exact `planner_slot_occupied` precedent, the exact `uds_ipc.rs` test names, the exact `session_demoted` parenting rule) rather than generic domain knowledge.

**Research date:** 2026-07-12
**Valid until:** Effectively immediate — this research is anchor-verification against the CURRENT tree state. If any commits land in `crates/brokerd/`, `crates/adapter-fs/`, `crates/executor/Cargo.toml`, or `cli/caprun/src/main.rs` before Phase 27 executes, re-run the grep verification commands in this doc's Anchor Table before trusting the line numbers (mirrors the DESIGN doc's own re-verification discipline).
