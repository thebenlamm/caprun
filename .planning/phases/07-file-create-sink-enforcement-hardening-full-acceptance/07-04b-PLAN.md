---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 04b
type: execute
wave: 4
depends_on: [07-04a]
files_modified:
  - crates/brokerd/src/proto.rs
  - crates/brokerd/src/quarantine.rs
  - crates/brokerd/src/sinks/file_create.rs
  - crates/brokerd/src/sinks/mod.rs
  - crates/brokerd/src/server.rs
  - cli/caprun/src/worker.rs
autonomous: true
requirements: [SINK-01, HARD-05]

must_haves:
  truths:
    - "WorkerClaim::RelativePath completes the enum; the broker mints a path claim via mint_from_read with taint [ExternalUntrusted, PathRaw] (NEVER LocalWorkspace); unknown claim variants continue to fail closed at deserialize."
    - "On an Allowed file.create decision the broker invokes invoke_file_create, which creates the file via 07-04a's create_exclusive_within under the 07-03 WorkspaceRoot and records sink_executed (carrying effect_id); an error records sink_execution_failed with NO automatic retry."
    - "The caprun CLI can drive a file.create plan node for BOTH a tainted workspace-derived path (→ block) and a trusted intent path (→ allow); the CaprunIntent/ProvideIntent match stays exhaustive."
    - "Effect-path ordering is realized end-to-end: validate schema (04a) → path resolution under capability → sensitivity → executor decision → (on Allowed) sink invocation."
  artifacts:
    - "crates/brokerd/src/proto.rs — WorkerClaim::RelativePath(String)"
    - "crates/brokerd/src/quarantine.rs — extract_relative_path_claims + relative_path taint arm in mint_from_read"
    - "crates/brokerd/src/sinks/file_create.rs — invoke_file_create (two-phase durable audit)"
    - "cli/caprun/src/worker.rs — intent-kind-driven claim extractor selection + file.create plan construction"
  key_links:
    - "worker selects the extractor by intent kind (email → extract_email_claims → EmailAddress; file-create → extract_relative_path_claims → RelativePath), so a hostile workspace file yields a tainted RelativePath routed to file.create/path → Block, while a trusted intent path (mint_from_intent, UserTrusted) → Allowed → invoke_file_create."
  prohibitions:
    - "Workspace-derived path values are tagged ExternalUntrusted (+PathRaw), NEVER LocalWorkspace (UNREVIEWED-by-threat-lane per CONTEXT.md)."
    - "Never resolve/canonicalize the path literal at ReportClaims/mint time (RESEARCH Pitfall 4) — resolution happens only inside the single openat2 syscall at sink-execution time (04a)."
    - "Do not add unrelated CLI surface — the intent-kind wiring exists only to make both §9 paths reachable for 07-05."
---

<objective>
Wire `file.create` into the live IPC + effect path so it is a real, reachable sink. This is the second half of the split-out 07-04 (plan-checker scope blocker): 07-04a built the decision-side mechanisms (schema gate, `path` sensitivity, `PathRaw`, `create_exclusive_within`); this plan adds the `WorkerClaim::RelativePath` variant + broker extraction/mint tagging, the live `invoke_file_create` sink invocation with two-phase durable audit, and the minimal CLI/worker intent-routing that makes BOTH the hostile-block and clean-allow paths reachable for 07-05's live proof.

Cross-platform for the claim/mint/sink-registry logic; the actual file creation runs through 07-04a's `create_exclusive_within` (Linux-gated real, macOS stub).
</objective>

<execution_context>
@/Users/benlamm/Workspace/AgentOS/.claude/gsd-core/workflows/execute-plan.md
@/Users/benlamm/Workspace/AgentOS/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md

@.planning/phases/07-file-create-sink-enforcement-hardening-full-acceptance/07-CONTEXT.md
@.planning/phases/07-file-create-sink-enforcement-hardening-full-acceptance/07-RESEARCH.md
@.planning/phases/07-file-create-sink-enforcement-hardening-full-acceptance/07-PATTERNS.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: WorkerClaim::RelativePath + broker extraction/mint with PathRaw taint</name>
  <files>crates/brokerd/src/proto.rs, crates/brokerd/src/quarantine.rs, crates/brokerd/src/server.rs</files>
  <read_first>
    - crates/brokerd/src/proto.rs — `WorkerClaim` (`:21-25`) with the `// RelativePath(String), // Phase 7` placeholder to activate.
    - crates/brokerd/src/quarantine.rs — `Claim` (`:35-41`), `extract_email_claims` (`:53`), `mint_from_read` (`:129-162`, taint hardcoded `[ExternalUntrusted, EmailRaw]`). Generalize taint selection by `claim.claim_type` (keep `mint_from_read` the SOLE taint-mint site).
    - crates/brokerd/src/server.rs — the `ReportClaims` arm (`:290-318`) with the exhaustive `match claim { WorkerClaim::EmailAddress(addr) => ... }`.
  </read_first>
  <action>
Activate `WorkerClaim::RelativePath(String)` in `proto.rs`. Add `extract_relative_path_claims(raw: &str) -> Vec<Claim>` in `quarantine.rs` producing `Claim { claim_type: "relative_path", value }` (deterministic; no LLM/regex-crate; the lossy guarantee holds — only the path string crosses IPC). In `mint_from_read`, derive taint from `claim.claim_type`: `"email_address" → [ExternalUntrusted, EmailRaw]`, `"relative_path" → [ExternalUntrusted, PathRaw]` (NEVER `LocalWorkspace`); an unknown claim_type must error rather than default-tag. Add a `WorkerClaim::RelativePath(p)` arm to the `server.rs` `ReportClaims` match that builds `Claim { claim_type: "relative_path", value: p }` and mints via `mint_from_read` exactly like the email arm; keep the match exhaustive (unknown future variants fail closed at deserialize).
  </action>
  <verify>
    <automated>cargo build --workspace && cargo test -p brokerd --no-fail-fast</automated>
  </verify>
  <done>`WorkerClaim::RelativePath(String)` is live; `extract_relative_path_claims` yields `relative_path` claims; `mint_from_read` tags a `relative_path` claim `[ExternalUntrusted, PathRaw]` (never `LocalWorkspace`) and errors on an unknown claim_type; the `ReportClaims` match handles both variants and stays exhaustive.</done>
</task>

<task type="auto">
  <name>Task 2: file_create sink invocation + Allowed-path wiring (two-phase audit)</name>
  <files>crates/brokerd/src/sinks/file_create.rs, crates/brokerd/src/sinks/mod.rs, crates/brokerd/src/server.rs</files>
  <read_first>
    - crates/brokerd/src/sinks/email_send.rs — `invoke_email_send_stub` (`:38-60`): the shape for a sink invocation that records an audit Event and returns its hash. Mirror it, but `file_create` performs a REAL side effect via 07-04a's `create_exclusive_within`.
    - crates/brokerd/src/server.rs — the `SubmitPlanNode` arm (`:323-357`); 07-02 handles the `sink_blocked` branch and mints `effect_id`. This task adds the `Allowed`-branch invocation for `file.create` and threads the `Arc<WorkspaceRoot>` (07-03) already present in `dispatch_request`.
    - **Confirm the sinks module shape first:** `grep -n "mod email_send\|pub mod sinks\|mod sinks" crates/brokerd/src/*.rs` — determine whether `sinks` is a single file or a directory module before adding `sinks/mod.rs` + `sinks/file_create.rs`.
  </read_first>
  <action>
Create `crates/brokerd/src/sinks/file_create.rs` with `invoke_file_create(conn, session_id, effect_id, plan_node, workspace_root, parent_hash) -> Result<String>`: extract the `path` and `contents` args from the plan node, call `workspace_root.create_exclusive_within(path, contents.as_bytes())`; on success append a `sink_executed` Event (carrying `effect_id`) and return the hash; on error append a `sink_execution_failed` Event (two-phase durable audit — an effect-path error leaves an explicit indeterminate record, NO automatic retry) and propagate the error. Register the module (create `sinks/mod.rs` mirroring how `email_send` is declared today). In `server.rs`'s `SubmitPlanNode` `Allowed` branch, when `plan_node.sink.0 == "file.create"`, call `invoke_file_create(...)` (threading the `Arc<WorkspaceRoot>` and `effect_id` already in scope). Non-file.create Allowed decisions keep today's behavior.
  </action>
  <verify>
    <automated>cargo build --workspace && cargo test -p brokerd --no-fail-fast && ./scripts/check-invariants.sh</automated>
  </verify>
  <done>`invoke_file_create` creates the file via `create_exclusive_within` and records `sink_executed` (with `effect_id`) on success / `sink_execution_failed` on error (no auto-retry); the broker invokes it on an `Allowed` `file.create` decision; `check-invariants.sh` green (no `EffectRequest`).</done>
</task>

<task type="auto">
  <name>Task 3: CLI intent-kind + plan routing that reaches file.create (both paths)</name>
  <files>cli/caprun/src/worker.rs, crates/brokerd/src/server.rs</files>
  <read_first>
    - **Re-grep the current intent enum FIRST:** `grep -rn "enum CaprunIntent\|CaprunIntent::" crates/ cli/` and `grep -rn "plan_from_intent\|SendEmailSummary" cli/ crates/` — confirm the exact `CaprunIntent` shape and the current plan-construction site before deciding what to add (do not assume it is email-only).
    - cli/caprun/src/worker.rs — the intent/claims/plan flow (`:95-130`): ProvideIntent → RequestFd → read → `extract_email_claims` → map to `WorkerClaim::EmailAddress` (`:120-124`) → SubmitPlanNode. This mapping is currently HARD-CODED to email; it must become intent-kind-driven.
    - crates/brokerd/src/server.rs — `ProvideIntent` arm (`:359-382`, `CaprunIntent` match at `:363-365`) — a new intent kind is needed so a trusted-intent path routes to `file.create/path`.
  </read_first>
  <action>
Make the worker's claim extractor selection **explicitly intent-kind-driven** (fixes the underspecified dispatch rule): when the intent kind is the email summary, use `extract_email_claims` → `WorkerClaim::EmailAddress`; when it is the new file-create kind, use `extract_relative_path_claims` → `WorkerClaim::RelativePath`. Add a `CaprunIntent` variant for file creation (e.g. `CreateFileFromReport { path }` or similar — match the naming convention confirmed by the re-grep) and extend the exhaustive `ProvideIntent` match in `server.rs` accordingly (a new variant must be a compile-time-forced match arm, no silent default). Build the `file.create` plan node in the worker's plan step with `path` (the routed value's `ValueId`) + `contents` args. The two reachable paths: (a) hostile — the workspace file content yields a tainted `RelativePath` claim routed to `file.create/path` → Block; (b) clean — a broker-minted trusted intent path (`mint_from_intent`, `[UserTrusted]`) supplies `file.create/path` → Allow → file created. Keep the change minimal — this exists solely to make 07-05's live proof reachable; add no unrelated CLI surface.
  </action>
  <verify>
    <automated>cargo build --workspace && cargo test --workspace --no-fail-fast</automated>
  </verify>
  <done>The caprun CLI drives a `file.create` plan node for both a tainted workspace-derived path (→ block) and a trusted intent path (→ allow); the worker selects the extractor by intent kind (email vs file-create); the `CaprunIntent`/`ProvideIntent` match stays exhaustive (new variant is a forced match arm); `cargo build --workspace` green.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| tainted path value → file.create sink | A workspace-derived (untrusted) path routed to `file.create/path` must Block, never write. |
| worker claim emission → broker mint | The worker chooses which claim variant to emit; the broker independently taints it (`mint_from_read`) — the worker cannot launder trust. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-07-44 | Spoofing (trust laundering) | `mint_from_read` path arm | high | mitigate | Workspace path claims minted `[ExternalUntrusted, PathRaw]`, never `LocalWorkspace` (UNREVIEWED-by-threat-lane); routing-sensitive `path` → Block. |
| T-07-45 | Availability (crash mid-effect) | `invoke_file_create` | medium | mitigate | Two-phase audit: `sink_execution_failed` explicit indeterminate record, NO automatic retry (ACC-01/HARD-06). |
| T-07-47 | EoP (unknown claim laundering) | `mint_from_read` claim_type | medium | mitigate | An unknown `claim_type` errors rather than default-tagging; only the two known claim types get taint sets. |
</threat_model>

<verification>
- `cargo build --workspace && cargo test --workspace --no-fail-fast` — green on macOS (claim/mint/registry cross-platform; sink file I/O Linux-gated).
- `./scripts/check-invariants.sh` green (no `EffectRequest`; effect path stays plan-node-only).
- A `[ExternalUntrusted,PathRaw]` `file.create/path` arg → `BlockedPendingConfirmation`; a `[UserTrusted]` path → `Allowed` → `invoke_file_create` → `sink_executed`.
- The `ReportClaims` and `ProvideIntent` matches remain exhaustive (adding a variant is a compile error, not a silent default).
</verification>

<success_criteria>
`file.create` is live end-to-end: the `RelativePath` claim completes `ReportClaims` and mints `[ExternalUntrusted, PathRaw]`; an `Allowed` `file.create` invokes `create_exclusive_within` with two-phase durable audit (`sink_executed`/`sink_execution_failed`, no retry); and the CLI reaches both the hostile-block and clean-allow paths via intent-kind-driven extractor selection. The full effect-path ordering (validate → resolve → sensitivity → executor → invoke) is realized (HARD-05), making the sink real (SINK-01) and reachable for 07-05's live §9 proof.
</success_criteria>

<output>
Create `.planning/phases/07-file-create-sink-enforcement-hardening-full-acceptance/07-04b-SUMMARY.md` when done.
</output>
