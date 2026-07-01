---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
type: plan-outline
plan_count: 5
---

# Phase 7 Plan Outline — file.create Sink, Enforcement Hardening & Full Acceptance

Two work streams: **Stream 1 (LOCKED SPEC — implement, do not re-derive)** = mint invariant + durable `SinkBlockedAnchor` (DESIGN §4–§8, TASK-mint). **Stream 2 (DESIGN — this phase's own)** = `file.create` sink internals (openat2/dirfd/O_EXCL), HARD-04 workspace-root capability, `RelativePath` claim, dispatch-ordering hardening.

## Outline Manifest

| Plan ID | Objective | Wave | Depends On | Requirements |
|---------|-----------|------|------------|--------------|
| 07-01 | **Mint-nonempty invariant + base `DenyReason` enum + executor empty-guard.** `ValueStore::mint` rejects empty taint/provenance (`Result<ValueId, MintInvariantError>`); executor defense-in-depth guard moves UP (right after `resolve`, before sensitivity check): empty-taint→`Denied(EmptyTaintInvariantViolation)`, empty-provenance→`Denied(MissingProvenanceAnchor)`. Introduces the typed `DenyReason` base enum (`DanglingHandle`/`EmptyTaintInvariantViolation`/`MissingProvenanceAnchor`). RED-first per TASK-mint; re-grep `\.mint(` against HEAD first (06-05 may have added a caller). Cross-platform, one atomic commit. | 1 | — | HARD-05 (fail-closed ordering foundation; establishes the base `DenyReason` enum that HARD-01 in 07-04 extends — sequenced strictly before per RESEARCH open-q #3) |
| 07-02 | **Durable `SinkBlockedAnchor` + `Event.anchor` migration + `append_event` guard + `ExecutorDecision` reshape.** Per DESIGN §4–§8: add `SinkBlockedAnchor` struct, `Event::new(...)` + `#[serde(default, skip_if_none)] anchor: Option<..>`, migrate ~13 `Event {..}` literals, broker-owned anchor-setting constructor for `sink_blocked`, golden byte-fixture test (byte-identical serialization). `append_event` REJECTS `sink_blocked` with `anchor==None`. Reshape `ExecutorDecision::BlockedPendingConfirmation` to `{ anchor }`. `effect_id` broker-minted, passed into `submit_plan_node`. Anti-stapling: anchor fields cloned verbatim from resolved `ValueRecord`, executor never sets taint. Delete `phase5_dispatch.rs:190` causal-vs-lineage conflation. REUSE `is_untrusted()`; no `TrustClass`. Cross-platform (no Linux gate). | 2 | 07-01 | ACC-01, ACC-06, ACC-07, HARD-06 (effect_id) |
| 07-03 | **HARD-04 workspace-root dirfd capability (read-side).** New `adapter_fs::workspace::WorkspaceRoot(OwnedFd)` opened once in `main()` via `nix::fcntl::open(O_DIRECTORY\|O_RDONLY)`; `read_within` uses `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)`; cfg-gated real(linux)/stub(other) mirroring `sandbox::landlock`. Replaces the unguarded `std::fs::File::open(&path)` in the `RequestFd` arm (`server.rs:248`). Thread `Arc<WorkspaceRoot>` through `run_broker_server`→`handle_connection`→`dispatch_request`. Confirm CLI-surface Option (a): derive root from `workspace-file` parent, worker sends relative path. The shared capability prerequisite for SINK-04. | 1 | — | HARD-04 |
| 07-04 | **file.create sink SINK-01..04 + `RelativePath` claim + arg-schema ordering.** New `executor::sink_schema::validate_schema` (KNOWN_SINKS, per-sink arg lists) as first statement of `submit_plan_node` — **extends the 07-01 `DenyReason` enum** (`UnknownSink`/`UnknownArg`/`DuplicateArg`/`MissingArg`), never a second error type. Add `file.create`→`["path"]` routing-sensitive (SINK-02). New `adapter-fs create_exclusive_within` (openat2 `O_CREAT\|O_EXCL\|RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS`, TOCTOU-safe single syscall). New `brokerd::sinks::file_create` wired live into `SubmitPlanNode` on `Allowed` (two-phase durable audit: `sink_executed`/`sink_execution_failed`). `WorkerClaim::RelativePath(String)` + `extract_relative_path_claims` + `mint_from_read` arm with `[ExternalUntrusted, PathRaw]` (new label; NEVER `LocalWorkspace` per CONTEXT caveat). Sink internals Linux-gated. | 3 | 07-01, 07-02, 07-03 | SINK-01, SINK-02, SINK-03, SINK-04, HARD-01, HARD-05 |
| 07-05 | **Live Linux e2e §9 full acceptance.** Restore continuously-proven live §9 via `file.create` (Phase-6 email hostile block became unreachable). Hostile block: tainted path from `mint_from_read`→`file.create` `path` arg→executor Blocks→non-success exit, NO file on disk, durable `sink_blocked` with non-None anchor. Clean allow: trusted intent→creates exactly the expected file, `sink_executed`. Causal chain `fd_granted→file_read→plan_node_evaluated→sink_blocked\|sink_executed` via `verify_chain` + depth walk. After-exit DB-alone anti-stapling sentinel (drop+reopen conn, verify_chain first, genuine-taint backstops) + tamper-evidence (UPDATE payload → verify_chain false). Reuses `run_caprun_intent_on` harness; `#[cfg(target_os="linux")]`. | 4 | 07-02, 07-04 | ACC-03, ACC-04, ACC-05, ACC-07 (live) |

## Coverage Check (all 14 IDs)

SINK-01 (07-04) · SINK-02 (07-04) · SINK-03 (07-04) · SINK-04 (07-04) · HARD-01 (07-04) · HARD-04 (07-03) · HARD-05 (07-01, 07-04) · HARD-06 (07-02) · ACC-01 (07-02) · ACC-03 (07-05) · ACC-04 (07-05) · ACC-05 (07-05) · ACC-06 (07-02) · ACC-07 (07-02 durable, 07-05 live). **All 14 covered.**

## Wave Structure

- **Wave 1 (parallel):** 07-01 (mint/DenyReason base), 07-03 (workspace-root capability) — no shared files, independent.
- **Wave 2:** 07-02 (durable anchor) — depends 07-01 (DenyReason base).
- **Wave 3:** 07-04 (file.create sink) — depends 07-01+07-02 (DenyReason enum sequencing / `executor/lib.rs` conflict avoidance) + 07-03 (capability for SINK-04).
- **Wave 4:** 07-05 (live §9 e2e) — depends 07-02 (anchor) + 07-04 (sink).

**Sequencing note (RESEARCH open-q #3):** 07-01 lands the base `DenyReason` enum; 07-02 reshapes `ExecutorDecision`/consumes it; 07-04 extends it with schema variants. These serialize (not wave-parallel) to avoid merge conflict on the shared enum + `executor/src/lib.rs`.
