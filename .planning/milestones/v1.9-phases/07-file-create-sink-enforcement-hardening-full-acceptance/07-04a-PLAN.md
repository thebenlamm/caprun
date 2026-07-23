---
phase: 07-file-create-sink-enforcement-hardening-full-acceptance
plan: 04a
type: execute
wave: 3
depends_on: [07-01, 07-02, 07-03]
files_modified:
  - crates/runtime-core/src/plan_node.rs
  - crates/executor/src/sink_sensitivity.rs
  - crates/executor/src/sink_schema.rs
  - crates/executor/src/lib.rs
  - crates/runtime-core/src/executor_decision.rs
  - crates/adapter-fs/src/workspace.rs
autonomous: true
requirements: [SINK-01, SINK-02, SINK-03, SINK-04, HARD-01, HARD-05]

must_haves:
  truths:
    - "file.create has an explicit arg schema {path, contents}; missing, duplicate, or unknown args are rejected (Denied) BEFORE any sensitivity or executor step; unknown sinks also fail closed."
    - "file.create's path arg is routing-sensitive: a tainted path value → BlockedPendingConfirmation; contents is NOT routing-sensitive."
    - "WorkspaceRoot::create_exclusive_within creates via openat2(O_CREAT|O_EXCL|RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS) — never overwrites (O_EXCL), rejects absolute/traversal/symlink-escape at kernel resolution (TOCTOU-safe single syscall)."
    - "TaintLabel::PathRaw exists and is_untrusted() returns true for it (exhaustive match, no wildcard)."
  artifacts:
    - "crates/runtime-core/src/plan_node.rs — TaintLabel::PathRaw"
    - "crates/executor/src/sink_schema.rs — KNOWN_SINKS + validate_schema; DenyReason variants UnknownSink/UnknownArg/DuplicateArg/MissingArg"
    - "crates/executor/src/sink_sensitivity.rs — FILE_CREATE_ROUTING_SENSITIVE + file.create arm"
    - "crates/adapter-fs/src/workspace.rs — create_exclusive_within (write-side of the shared capability)"
  key_links:
    - "validate_schema is the FIRST statement of submit_plan_node and extends 07-01's DenyReason (no second error type); file.create's path is routing-sensitive; create_exclusive_within is the write-side of 07-03's WorkspaceRoot dirfd capability. These are the enforcement MECHANISMS; 07-04b wires them live."
  prohibitions:
    - "Never overwrite an existing file — O_EXCL is mandatory; a create on an existing path fails (EEXIST)."
    - "Do not add a second denial error type — extend DenyReason; unknown sink/arg fail closed through it."
    - "Do not add a wildcard arm to is_untrusted() (adding PathRaw must be an explicit match arm)."
---

<objective>
Build the enforcement MECHANISMS for `file.create` — the executor arg-schema gate, `path` routing-sensitivity, the `PathRaw` taint label, and the kernel-level exclusive-create capability — without yet wiring the live sink invocation or the IPC claim (those are 07-04b). This is the security-critical foundation, split out from the sink's live wiring to keep each plan's blast radius small (plan-checker 07-04 scope blocker). Today no sink is invoked and `sinks::email_send::invoke_email_send_stub` is dead code; this plan adds the deterministic decision-side machinery so 07-04b can make `file.create` real.

`create_exclusive_within` internals are `#[cfg(target_os="linux")]` (macOS no-op stub); the schema/label/sensitivity logic is cross-platform.
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
  <name>Task 1: PathRaw taint label + file.create routing sensitivity</name>
  <files>crates/runtime-core/src/plan_node.rs, crates/executor/src/sink_sensitivity.rs</files>
  <read_first>
    - crates/runtime-core/src/plan_node.rs — `TaintLabel` enum (`:13-21`) and the EXHAUSTIVE `is_untrusted()` match (`:37-46`, NO wildcard — adding a variant without updating it is a compile error, by design).
    - crates/executor/src/sink_sensitivity.rs — `EMAIL_SEND_ROUTING_SENSITIVE` (`:14`) and the `is_routing_sensitive` match (`:27-32`); mirror the `email.send` arm for `file.create`.
  </read_first>
  <action>
Add `PathRaw` to `TaintLabel` and add it to the `is_untrusted()` match's `=> true` group (it is untrusted — a workspace-read path). Add `pub const FILE_CREATE_ROUTING_SENSITIVE: &[&str] = &["path"];` to `sink_sensitivity.rs` and a `"file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),` arm to `is_routing_sensitive`. `contents` is NOT routing-sensitive. Update the exhaustive-match tests if `runtime-core/tests/intent_taint.rs` enumerates every label.
  </action>
  <verify>
    <automated>cargo build -p runtime-core && cargo test -p executor sink_sensitivity --no-fail-fast</automated>
  </verify>
  <done>`TaintLabel::PathRaw` exists and `is_untrusted()` returns true for it; `is_routing_sensitive(SinkId("file.create"), "path")` is true and `"contents"` is false; the exhaustive match compiles (no wildcard added).</done>
</task>

<task type="auto">
  <name>Task 2: sink arg-schema registry + validate_schema (HARD-01) extending DenyReason</name>
  <files>crates/executor/src/sink_schema.rs, crates/executor/src/lib.rs, crates/runtime-core/src/executor_decision.rs</files>
  <read_first>
    - crates/executor/src/lib.rs — `submit_plan_node` (`:41-82`); 07-01 added the empty guards and 07-02 added the `effect_id` param + anchor build. `validate_schema` must run as the FIRST statement, before the per-arg loop.
    - crates/runtime-core/src/executor_decision.rs — the `DenyReason` enum from 07-01; EXTEND it (do not add a second error type).
    - 07-RESEARCH.md Q5 (arg-schema ordering) + the `email.send` schema shape.
  </read_first>
  <action>
Create `crates/executor/src/sink_schema.rs` with a `KNOWN_SINKS` registry mapping each known `SinkId` string to its allowed arg-name set: `email.send → [to, cc, bcc, subject, body]` (match the current live shape), `file.create → [path, contents]`. Add `pub fn validate_schema(plan_node: &PlanNode) -> Result<(), DenyReason>`: reject an unknown sink (`UnknownSink`), an arg not in the sink's set (`UnknownArg`), a duplicate arg name (`DuplicateArg`), and a missing required arg (`MissingArg`). Extend `DenyReason` with `UnknownSink`, `UnknownArg`, `DuplicateArg`, `MissingArg` (keep it the single enum). Call `validate_schema(plan_node)` at the TOP of `submit_plan_node`; on `Err(reason)` return `ExecutorDecision::Denied { reason }` before the resolve/guard/sensitivity loop. Register `pub mod sink_schema;` in `executor/src/lib.rs`.
  </action>
  <verify>
    <automated>cargo build -p executor && cargo test -p executor sink_schema --no-fail-fast</automated>
  </verify>
  <done>`validate_schema` rejects unknown sink → `Denied(UnknownSink)`, unknown arg → `UnknownArg`, duplicate arg → `DuplicateArg`, missing arg → `MissingArg`, all BEFORE resolve/sensitivity; `file.create` accepts exactly `{path, contents}`; `DenyReason` carries the new variants (one enum).</done>
</task>

<task type="auto">
  <name>Task 3: create_exclusive_within (SINK-03/SINK-04 write-side capability)</name>
  <files>crates/adapter-fs/src/workspace.rs</files>
  <read_first>
    - crates/adapter-fs/src/workspace.rs — the 07-03 `WorkspaceRoot` + `read_within` (cfg-gated real/stub). Mirror EXACTLY for the write method (same cfg pattern, same error mapping).
    - 07-RESEARCH.md Q1 (OpenHow builder; `O_CREAT|O_EXCL` with `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`) + Pitfall 2 (RESOLVE_BENEATH alone does NOT block symlinks).
  </read_first>
  <action>
Add `WorkspaceRoot::create_exclusive_within` as a cfg-gated pair:
- `#[cfg(target_os="linux")] pub fn create_exclusive_within(&self, rel_path: &str, contents: &[u8]) -> std::io::Result<()>`: build `OpenHow::new().flags(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY).mode(0o600).resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS)`, call `nix::fcntl::openat2(&self.dirfd, rel_path, how)`, map errors to `std::io::Error` (an existing file surfaces as `EEXIST`; a RESOLVE_* violation as `EXDEV`), then write all `contents` to the returned fd and fsync/close. Single syscall does resolution + exclusive create atomically — no validate-then-create window (SINK-04).
- `#[cfg(not(target_os="linux"))] pub fn create_exclusive_within(...)`: stub with no security claim (`std::fs::OpenOptions::new().write(true).create_new(true).open(self.root_path.join(rel_path))` then write) so the crate builds on macOS.
Add Linux-gated tests: create under root → file exists with expected bytes; create on an EXISTING path → `Err` (EEXIST, O_EXCL); absolute path → `Err`; `..` traversal → `Err`; symlink-escape → `Err`.
  </action>
  <verify>
    <automated>cargo build -p adapter-fs && cargo test -p adapter-fs workspace --no-fail-fast</automated>
  </verify>
  <done>`create_exclusive_within` exists (cfg-gated); on Linux it exclusively creates under the dirfd (never overwrites — existing path → `Err`), rejects absolute/traversal/symlink paths, and writes contents; macOS builds via the stub.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| worker plan node → executor schema gate | Arg names/counts are worker-controlled; unknown sink/arg, duplicates, missing args must fail closed before any effect. |
| file.create path resolution → filesystem | The path is resolved only inside a single `openat2` under the dirfd — never canonicalized/opened ambiently. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-07-41 | Tampering / EoP | `create_exclusive_within` | high | mitigate | `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)` rejects absolute/`..`/symlink-escape at kernel resolution; single syscall (TOCTOU-safe). |
| T-07-42 | Tampering (overwrite) | `file.create` | high | mitigate | `O_EXCL` — a create on an existing path fails (`EEXIST`); never clobbers existing data. |
| T-07-43 | EoP (unvalidated args) | `submit_plan_node` | high | mitigate | `validate_schema` runs first; unknown sink/arg, duplicate, missing → `Denied` before resolve/sensitivity/effect (HARD-01/HARD-05). |
| T-07-46 | Tampering (supply chain) | cargo deps | low | accept | No new packages — `nix` `openat2`/`OpenHow`/`ResolveFlag` already workspace-pinned (RESEARCH Package Legitimacy Audit). |
</threat_model>

<verification>
- `cargo build --workspace && cargo test --workspace --no-fail-fast` — green on macOS (schema/label/sensitivity cross-platform; `create_exclusive_within` Linux-gated shows 0-passed, expected).
- Linux (Colima/Docker recipe, `seccomp=unconfined`, no `--privileged`): `cargo test -p adapter-fs workspace` — exclusive-create + reject absolute/`..`/symlink; `cargo test -p executor sink_schema` — unknown/duplicate/missing arg denials.
- `is_routing_sensitive("file.create","path")` true; `"contents"` false.
- `./scripts/check-invariants.sh` green (no `EffectRequest`).
</verification>

<success_criteria>
The `file.create` enforcement mechanisms exist: schema-validated first (unknown/duplicate/missing/unknown-sink fail closed — HARD-01/HARD-05 ordering foundation), `path` routing-sensitive (SINK-02), the `file.create` schema registered (SINK-01), and exclusive `O_EXCL` create resolved via `openat2` under the shared `WorkspaceRoot` dirfd (SINK-03/SINK-04, TOCTOU-safe). 07-04b wires these into the live IPC/sink path.
</success_criteria>

<output>
Create `.planning/phases/07-file-create-sink-enforcement-hardening-full-acceptance/07-04a-SUMMARY.md` when done.
</output>
