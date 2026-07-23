# Phase 7: file.create Sink, Enforcement Hardening & Full Acceptance - Research

**Researched:** 2026-06-30
**Domain:** Linux kernel-mediated path resolution (openat2/dirfd), broker-side sink dispatch hardening, durable audit-DAG anchoring (spec, not derived here)
**Confidence:** HIGH (grounded in real source + verified nix 0.31.3 API + man7.org openat2(2))

## Two-Stream Intro

This phase has two work streams. **Do not conflate them.**

- **Stream 1 — LOCKED SPEC (implement, do not re-derive).** The ACC-07 durable `SinkBlockedAnchor`, the mint-nonempty invariant, and the two-graph provenance model are board-ratified in `planning-docs/DESIGN-durable-anchor-and-label-partition.md` (REV.2), `planning-docs/TASK-mint-nonempty-invariant.md`, `planning-docs/PHASE-7-HANDOFF.md`. This RESEARCH.md gives the planner pointers + concrete integration facts only — no alternative designs.
- **Stream 2 — NEEDS DESIGN (this document's real job).** `file.create` sink internals (SINK-01..04), the HARD-04/SINK-04 shared workspace-root capability model, the `RelativePath` claim variant, and HARD-01/05/06 dispatch-ordering hardening. Answered below as Q1–Q7, grounded in the actual `crates/*` source read during this research session.

---

## Stream 1 (locked spec) — pointers only

**Implement `planning-docs/DESIGN-durable-anchor-and-label-partition.md` §4–§8 verbatim. Do not propose an alternative anchor shape, do not introduce a `TrustClass` enum (explicitly killed by board review — `is_untrusted()` is the sole partition source of truth), do not re-derive the two-graph model.**

Concrete integration facts for the planner (current code the anchor work touches):

| Fact | Location |
|---|---|
| `ValueStore::mint` still returns bare `ValueId` (not `Result`) — the mint-invariant task has **not landed yet** | `crates/executor/src/value_store.rs:42-57` (verified: no `MintInvariantError`/`DenyReason` anywhere in `crates/` as of this research) |
| `mint_from_read` / `mint_from_intent` are the only two call sites of `store.mint(...)` outside tests | `crates/brokerd/src/quarantine.rs:161`, `:233` |
| `ExecutorDecision::BlockedPendingConfirmation` is still flat (`literal_value, sink, arg_name, taint, provenance_chain`) — the reshape to `{ anchor: SinkBlockedAnchor }` breaks this struct's 5 fields into 1 | `crates/runtime-core/src/executor_decision.rs:18-26` |
| `sink_blocked` append site — currently sets `taint: vec![]` on the event and never carries `value_id`/`literal`/`provenance_chain` | `crates/brokerd/src/server.rs:323-357` (the `SubmitPlanNode` arm; `event_type` is chosen at line 331) |
| `Event` struct has no `anchor` field yet — ~13 `Event { ... }` struct literals across `quarantine.rs`, `server.rs`, `main.rs`, `audit.rs` tests, `s9_acceptance.rs`, `phase5_dispatch.rs` will need migrating to `Event::new(...)` per REV.2 §5 | grep count: 7 in non-test `crates/` sources (`quarantine.rs`×2, `server.rs`×3, `main.rs`×1) + ~6 more in test files |
| The assertion to **delete** (`sink_blocked.parent_id == read_event_id`) | `crates/brokerd/tests/phase5_dispatch.rs:190-192` (exact text: `assert_eq!(blocked.parent_id, Some(read_event_id), "sink_blocked must be causally parented onto the prior (file_read) event");`) |
| `append_event` currently has no guard rejecting `anchor == None` for `sink_blocked` — REV.2 rule 7's TCB gate must be added here | `crates/brokerd/src/audit.rs:100-132` |
| `TaintLabel::is_untrusted()` — already shipped, reuse verbatim, do not touch | `crates/runtime-core/src/plan_node.rs:37-46` |
| `submit_plan_node`'s executor predicate — reuse `is_untrusted()`, but the **defense-in-depth empty-guard from REV.2 §3 does not exist yet** (no empty-taint/empty-provenance check runs today) | `crates/executor/src/lib.rs:41-82` |

**Land `TASK-mint-nonempty-invariant.md` FIRST** as Phase 7's opening plan — it is small, additive, and touches the exact files (`value_store.rs`, `quarantine.rs`) that the Stream-2 work (mint sites for `RelativePath`) will also touch. Executing it first avoids a second migration pass.

---

## Stream 2 (design) — findings

### Q1 — `openat2` in Rust, unprivileged, Linux ≥ 5.6/5.13

**Binding: `nix::fcntl::openat2` — already available, zero new dependencies.**

`nix = "0.31.3"` is already pinned in the workspace root `Cargo.toml:9` with the `fs` feature already enabled (`features = ["fs", "socket", "resource", "process", "signal", "uio"]`). `nix::fcntl::openat2`, `OpenHow`, and `ResolveFlag` all live behind `#[cfg(target_os = "linux")]` inside the nix crate itself `[VERIFIED: docs.rs/nix 0.31.3 source (raw.githubusercontent.com/nix-rust/nix/v0.31.3/src/fcntl.rs) — confirmed `#[cfg(target_os = "linux")]` on `openat2`, `OpenHow`, and `ResolveFlag`]`. This means the sink code can mirror the existing `sandbox::landlock::deny_all_filesystem` pattern exactly: a real `#[cfg(target_os = "linux")]` implementation using `nix::fcntl::openat2`, and a `#[cfg(not(target_os = "linux"))]` fallback for macOS dev-machine compilation (no security claim there — matches CLAUDE.md "All v0/v1.1 security claims are Linux-only").

**Signature** `[VERIFIED: docs.rs/nix 0.31.3]`:
```rust
pub fn openat2<P: ?Sized + NixPath, Fd: AsFd>(
    dirfd: Fd,
    path: &P,
    how: OpenHow,
) -> nix::Result<OwnedFd>
```

**Minimal call shape** (write-side, `O_CREAT|O_EXCL`, TOCTOU-safe):
```rust
// crates/adapter-fs/src/workspace.rs (new) — #[cfg(target_os = "linux")]
use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
use std::os::fd::{AsFd, OwnedFd};

pub fn create_exclusive(
    workspace_dirfd: impl AsFd,
    relative_path: &str,
) -> nix::Result<OwnedFd> {
    let how = OpenHow::new()
        .flags(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY)
        .mode(nix::sys::stat::Mode::from_bits_truncate(0o600))
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
    openat2(workspace_dirfd, relative_path, how)
}
```

**Flag set — closes traversal + symlink escape** `[VERIFIED: man7.org openat2(2)]`:
- `RESOLVE_BENEATH` — rejects any resolution component (including `..` traversal and absolute paths/absolute symlink targets) that would leave the directory tree rooted at `dirfd`. Returns `EXDEV` on violation, **not** `ENOENT`/`EACCES` — the planner's error-handling code and tests must match on `EXDEV` specifically.
- `RESOLVE_NO_SYMLINKS` — **required in addition to** `RESOLVE_BENEATH`. `RESOLVE_BENEATH` alone only rejects *escaping* symlinks; it does not block symlink traversal that stays inside the tree (e.g., an in-workspace symlink pointing to a sensitive file the workspace owner also has read access to, or a TOCTOU symlink-swap attack). `RESOLVE_NO_SYMLINKS` disallows **all** symlink resolution during the call, which is the correct posture for both read (HARD-04) and create (SINK-04) — the confined worker/attacker-controlled path string must never be allowed to follow a symlink at all.
- `RESOLVE_NO_MAGICLINKS` — optional extra hardening (blocks `/proc`-style magic links); not strictly required since the worker never has an ambient fd into `/proc`, but cheap to add for defense-in-depth. `RESOLVE_NO_SYMLINKS` already implies it per the man page.

**Kernel version:** `openat2()` itself was added in **Linux 5.6** `[VERIFIED: man7.org openat2(2)]`. This is a *lower* floor than the project's existing Landlock requirement (kernel ≥5.13 for ABI::V1, ≥5.19 for ABI::V3, per `crates/sandbox/src/landlock.rs:11` and CLAUDE.md). Since the project's Linux CI/dev target already assumes a Landlock-capable kernel, no new minimum-kernel constraint is introduced.

**Cfg gate:** mirror `crates/sandbox/src/landlock.rs` exactly — real impl `#[cfg(target_os = "linux")]`, no-op/fallback `#[cfg(not(target_os = "linux"))]`. Suggested home: a new module in `adapter-fs` (see Q2), since `adapter-fs` is already documented as "the only path to fs effects" (CLAUDE.md architecture table) and already owns the SCM_RIGHTS fd-passing that `file.create` extends.

**Alternatives considered:** `cap-std`/`cap-std::fs::Dir` (capability-safe directory handles wrapping `openat2` internally) and raw `libc::syscall(SYS_openat2, ...)` were both surfaced by initial WebSearch but are **not recommended** — `nix::fcntl::openat2` is already a locked, zero-new-dependency, workspace-pinned binding with the exact `RESOLVE_*` flags needed. Adding `cap-std` would introduce a second capability abstraction layered on top of the same syscall for no functional gain; raw `libc::syscall` would forgo `nix`'s safe `OwnedFd`/`AsFd` wrapper and error-code mapping.

### Q2 — Shared workspace-root capability model

**Recommendation: one `adapter-fs::WorkspaceRoot(OwnedFd)`, owned by the broker, opened once at `caprun main()` startup, threaded through `run_broker_server` → `handle_connection` → `dispatch_request` exactly like `conn: Arc<Mutex<Connection>>` is today.**

Current state (the gap HARD-04 must close): `RequestFd`'s broker-side handler does **not** restrict the path at all — `crates/brokerd/src/server.rs:248-251`:
```rust
BrokerRequest::RequestFd { path } => {
    let file = std::fs::File::open(&path)
        .with_context(|| format!("broker: open {path}"))?;
```
`path` here is the **full absolute path** echoed verbatim from the worker's `WORKSPACE_FILE` env var (`cli/caprun/src/worker.rs:99`: `send_framed(&std_stream, &BrokerRequest::RequestFd { path: workspace_file })`). The worker fully controls the string sent in this IPC message — today nothing stops it (or a future compromised/injected worker) from requesting an arbitrary broker-openable path. This is the concrete vulnerability HARD-04 exists to close.

**Design:**
1. Add `adapter_fs::workspace::WorkspaceRoot` — a thin `#[cfg(target_os="linux")]`-real / `#[cfg(not(linux))]`-stub wrapper around an `OwnedFd` obtained via `nix::fcntl::open(root_path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())` (plain `open`, not `openat2` — this one call establishes the anchor itself and does not need `RESOLVE_*`, since the broker has ambient fs access to the workspace root by design).
2. Expose two functions on it, both taking a **workspace-relative** path string (never absolute, never containing `..` after syntactic pre-check — see Q5):
   - `read_within(&self, rel_path: &str) -> io::Result<File>` — `openat2(self.dirfd, rel_path, OpenHow::new().flags(O_RDONLY).resolve(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS))`. **This is what HARD-04 replaces `std::fs::File::open(&path)` with.**
   - `create_exclusive_within(&self, rel_path: &str) -> io::Result<File>` — the Q1 shape above. **This is SINK-04.**
3. Own the `WorkspaceRoot` in `main.rs` alongside `conn: Arc<Mutex<Connection>>`, pass an `Arc<WorkspaceRoot>` (or plain `Arc<OwnedFd>` — a dirfd is an immutable, read-only capability with no session-scoping need, unlike `ValueStore`; it's safe to share without a `Mutex` since `openat2` is itself thread-safe/reentrant on a shared fd) into `run_broker_server`, then into `handle_connection`, then into `dispatch_request` as an added parameter next to `value_store`.
4. **CLI surface implication (flag for the planner — genuine open decision, not board-locked):** the existing CLI positional arg is a single **file** path (`caprun <intent-kind> <intent-param> <workspace-file> [audit-db-path]`, `cli/caprun/src/main.rs:9`). A workspace-*root* directory is a different concept. Two options:
   - **(a) Recommended — derive root from `workspace-file`'s parent directory**, and change `RequestFd`/worker code to send the file's **basename** (or a path relative to that parent) rather than the full absolute path. Zero new CLI surface; matches "one bounded new capability surface" scope discipline (REQUIREMENTS.md milestone goal). The existing e2e tests (`cli/caprun/tests/e2e.rs`, `s9_live_block.rs`) already create a fresh temp dir per run (`tmp.join("workspace.txt")`) — that temp dir naturally becomes the workspace root with no test-harness change beyond how the relative path is computed.
   - **(b)** Add an explicit `--workspace-root <dir>` CLI arg. More explicit, but larger CLI-surface change and touches every existing e2e test's `Command::new(caprun_bin).arg(...)` call sites.
   - Recommend (a); flag as an open question the planner should confirm before task-breakdown, since it changes what string `RequestFd.path` and `file.create`'s `path` `PlanArg` carry (workspace-relative, not absolute).

**Composes with Landlock:** the worker's own `Landlock deny-all` (`crates/sandbox/src/landlock.rs`) is unaffected either way — the worker never opens anything itself; it only receives fds via SCM_RIGHTS (existing pattern, `crates/adapter-fs/src/lib.rs:pass_fd`/`recv_fd`) or, for `file.create`, never touches the created file at all (the **broker** performs the creation entirely server-side after the executor Allows — see Q5/Q6). The dirfd capability is a broker-side-only concept; it does not need to cross the IPC boundary or be passed to the confined worker.

**Nothing to reuse for the dirfd itself** — `adapter-fs::pass_fd`/`recv_fd` (SCM_RIGHTS) is the fd-*passing* mechanism (broker → worker, for reads the worker itself performs), which is a different concern from the dirfd-*anchoring* mechanism (broker-internal, for `openat2` resolution). `file.create` never passes an fd to the worker at all — the broker resolves, creates, and writes the file itself, entirely inside `dispatch_request`, since the worker only ever supplies opaque `ValueId` handles (never literals) per the locked handle model (`crates/runtime-core/src/plan_node.rs:97-107`, `PlanArg` doc comment).

### Q3 — TOCTOU-safety: why dirfd + `openat2(RESOLVE_BENEATH)` removes the race

A conventional "validate-then-write" implementation does two separate syscalls with a window in between:
```rust
// UNSAFE — the pattern SINK-04 forbids
let canonical = std::fs::canonicalize(&path)?;      // (1) resolve/validate
if !canonical.starts_with(&workspace_root) { deny(); }
std::fs::write(&path, contents)?;                     // (2) act — SEPARATE resolution!
```
Between (1) and (2), an attacker with any write access to an intermediate path component (or a symlink race) can swap a directory/symlink so that step (2)'s **independent** path resolution lands outside the validated tree — classic TOCTOU (CWE-367). `realpath`/`canonicalize` only proves what the path *would have* resolved to at check-time; it says nothing about resolution at write-time.

`openat2(dirfd, path, RESOLVE_BENEATH|...)` collapses validation and action into **one syscall**: the kernel performs path-component resolution and the `O_CREAT|O_EXCL` open atomically, and `RESOLVE_BENEATH` is enforced *during that same resolution walk*, not as a separate stat/check beforehand `[VERIFIED: man7.org openat2(2) — RESOLVE_BENEATH is documented as a resolution-time constraint the kernel enforces component-by-component during the same open() call]`. There is no second resolution step for an attacker to race against, because there is no second call. **No separate `stat`/`access`/`canonicalize` step is needed or should be added** — adding one would only reintroduce the exact race the single-syscall design eliminates (and would be redundant, since the real check happens inside `openat2` regardless).

### Q4 — `RelativePath` claim variant

**Mirror `WorkerClaim::EmailAddress` exactly.** The enum already has the extension point commented in place: `crates/brokerd/src/proto.rs:25`: `// RelativePath(String),  // Phase 7`.

1. **Wire type:** `WorkerClaim::RelativePath(String)` added to the `#[serde(tag="kind", content="value")]` enum (`proto.rs:19-26`). Unknown `kind` tags already fail closed at deserialize today — `serde_json::from_slice::<BrokerRequest>(&body)` returns `Err`, and `handle_connection` responds `BrokerResponse::Error{message:"invalid request"}` and **breaks the connection loop** (`crates/brokerd/src/server.rs:157-170`). This mechanism needs no change; it already covers "unknown variants fail closed."
2. **Extraction (worker-local, mirrors `extract_email_claims`):** a new deterministic hand-rolled scanner in `quarantine.rs`, e.g. `extract_relative_path_claims(raw: &str) -> Vec<Claim>` returning `Claim{claim_type:"relative_path", value:<path-shaped token>}`. Same lossy guarantee (raw sentence discarded, only the token crosses the boundary) as `extract_email_claims` (`quarantine.rs:53-99`).
3. **Broker dispatch arm:** `ReportClaims`'s `match claim { WorkerClaim::EmailAddress(addr) => {...} }` (`server.rs:296-317`) is a Rust-exhaustive match with the comment "Exhaustive enum: any future variant fails closed at deserialize" — meaning **the crate will not compile** once `RelativePath` is added to the enum without a matching arm here. This is Rust's own fail-closed enforcement, stronger than the serde-deserialize-error path (which only covers truly-unknown tags never added to the enum at all).
4. **Mint site:** the new arm calls `mint_from_read` (the sole taint-mint site, unchanged — `quarantine.rs:129-164`) with a taint vector for the path claim. **Recommend adding `TaintLabel::PathRaw`** to `crates/runtime-core/src/plan_node.rs:13-21`, mirroring the existing `EmailRaw`/`PdfRaw` per-content-type labels exactly. This forces a one-line addition to `is_untrusted()`'s exhaustive match (`plan_node.rs:37-46`) — which is precisely the compile-time safety net Pitfall 5's doc comment describes ("Adding a new TaintLabel variant without updating this match is a compile error, not a silent false-allow"). Mint as `[TaintLabel::ExternalUntrusted, TaintLabel::PathRaw]`, symmetric to `mint_from_read`'s existing `[ExternalUntrusted, EmailRaw]` (`quarantine.rs:141`).
5. **⚑ Apply the CONTEXT.md workspace-trust caveat here:** any value minted from workspace *file content* — including a path string extracted from hostile content — is `ExternalUntrusted`, **never** `LocalWorkspace` (unreviewed by the threat lane; see CONTEXT.md constraints).
6. **Resolution under the capability:** the literal path string in the minted `ValueRecord` is **not** resolved/opened at claim-mint time — it is only a string with taint attached (matching how `mint_from_read`'s email claim is just a string with taint, not a validated recipient). Actual resolution against the `WorkspaceRoot` dirfd happens later, at `file.create` sink-invocation time (Q5/Q6), after the executor has made its Allow/Block decision. This keeps `mint_from_read` symmetric between claim types and avoids doing filesystem I/O inside the taint-mint path.

### Q5 — Arg-schema validation ordering (HARD-01/05)

**Current gap (verified, not previously documented anywhere in the codebase):** `executor::submit_plan_node` (`crates/executor/src/lib.rs:41-82`) has **no schema validation step at all**. It iterates `plan_node.args` and, for each, resolves the handle and checks `is_routing_sensitive`. `sink_sensitivity::is_routing_sensitive` (`crates/executor/src/sink_sensitivity.rs:27-33`) has a catch-all `_ => false` for any sink name not `"email.send"` — meaning **an entirely unknown sink, or a `file.create` call with a missing/duplicate/unrecognized arg, currently evaluates to `Allowed`** (the loop simply finds nothing routing-sensitive to block on). This is exactly the hole HARD-01 requires closing.

**Recommended design — a new validation step that runs FIRST, before the existing resolve/sensitivity loop:**
```rust
// executor/src/sink_schema.rs (new, sibling to sink_sensitivity.rs)
pub const KNOWN_SINKS: &[&str] = &["email.send", "file.create"];
pub const EMAIL_SEND_ARGS: &[&str] = &["to", "cc", "bcc", "subject", "body", "attachment"]; // existing content+routing args, made explicit
pub const FILE_CREATE_ARGS: &[&str] = &["path", "contents"];

pub fn validate_schema(sink: &SinkId, args: &[PlanArg]) -> Result<(), DenyReason> {
    let required = match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ARGS,
        "file.create" => FILE_CREATE_ARGS,
        _ => return Err(DenyReason::UnknownSink),
    };
    let mut seen = std::collections::HashSet::new();
    for arg in args {
        if !required.contains(&arg.name.as_str()) { return Err(DenyReason::UnknownArg(arg.name.clone())); }
        if !seen.insert(&arg.name) { return Err(DenyReason::DuplicateArg(arg.name.clone())); }
    }
    // (missing-arg check: required.iter().all(|r| args.iter().any(|a| &a.name==r)))
    Ok(())
}
```
Called as literally the first statement inside `submit_plan_node`, before the existing `for arg in &plan_node.args` resolve loop.

**⚑ Load-bearing integration point with Stream 1:** the ACC-07 spec (REV.2 §3/§6) mandates a **single typed `DenyReason` enum** (`DanglingHandle`/`EmptyTaintInvariantViolation`/`MissingProvenanceAnchor`), created by the mint-invariant work. **Phase 7 must extend that same enum** with `UnknownSink`/`MissingArg(String)`/`DuplicateArg(String)`/`UnknownArg(String)` variants — do **not** create a second, parallel error type for schema violations. This is the one place Stream 1 and Stream 2 code share a type; sequence the plan so the mint-invariant task lands the base `DenyReason` enum first (per Stream 1 guidance above), then HARD-01 schema validation extends it.

**HARD-05 full ordering, mapped onto real dispatch code:**

| HARD-05 stage | Where it lives (recommended) | Status today |
|---|---|---|
| 1. validate schema | new `sink_schema::validate_schema`, first line of `submit_plan_node` | **missing** — build this |
| 2. capability check | existing resolve-handle loop (`value_store.resolve`, Dangling→Denied) doubles as this for handle validity; actual path-capability enforcement (`openat2 RESOLVE_BENEATH`) happens physically at stage 5, since resolution+open are one atomic kernel call (Q3) | resolve-loop exists; path-capability enforcement is net-new |
| 3. executor decision | existing routing-sensitivity/taint check (`lib.rs:61-75`) | exists, unchanged |
| 4. durable authorization audit | existing `sink_blocked`/`plan_node_evaluated` append in `dispatch_request` (`server.rs:330-353`) — happens **before** `send_response` | exists, but currently sent to the worker before any invocation (because no invocation exists yet — see Q6) |
| 5. sink invocation | **net-new** — `sinks::file_create::invoke_file_create(...)` called only on `Allowed`, only for `file.create`, inside `dispatch_request` before `send_response` | **does not exist**; even `sinks::email_send::invoke_email_send_stub` is dead code today — grep confirms it is called only from its own unit test, never from `server.rs` |
| 6. durable result audit | **net-new** — append a `sink_executed` (success) or an explicit failure/indeterminate event after invocation returns | does not exist; `sink_executed` appears nowhere in the codebase today (grep-verified) |

**Audit-failure fail-closed / causal parent preserved:** already the pattern used everywhere in `dispatch_request` (e.g. `fd_granted`, `SubmitPlanNode` arms) — every append uses `?` propagation on `append_event`, and every append passes `Some(*last_event_id)`/`Some(last_event_hash)` rather than `None` (`server.rs:255-264`, `:334-341`). Mirror this exactly for the new sink-invocation audit steps; never construct an event with `parent_id: None` inside `dispatch_request`.

### Q6 — HARD-06/ACC-01/ACC-06: effect_id, crash-indeterminate, forged-handle/cross-session denial

- **`effect_id` per attempt:** already speced by Stream 1 (REV.2 rule 2) — broker-minted, passed into `submit_plan_node(session_id, effect_id, plan_node, store)`. The **same** `effect_id` feeds both the durable anchor (Stream 1) and, for Phase 7, the sink-invocation audit records (stages 4–6 above) — do not mint a second id for the invocation side. `dispatch_request`'s `SubmitPlanNode` arm (`server.rs:323`) is where this new parameter is minted (`Uuid::new_v4()`) and threaded through.
- **Crash → indeterminate, no auto-retry:** satisfied by the two-phase append design in Q5's table — stage 4 (authorization) is durable *before* stage 5 (invocation) runs; if the process crashes between stage 4 and stage 6, the DB shows an authorized-but-no-result record for that `effect_id`. **HARD-06's "no automatic retry" is satisfied by absence, not by a check** — do not build retry logic. A crashed process simply exits; the next `caprun` invocation starts a fresh session with a fresh `effect_id`. Recommend one new event_type, e.g. `sink_invocation_indeterminate` is **not** needed as a distinct type to *write* — indeterminacy is a *read-time* inference (authorization event present, result event absent for the same `effect_id`), which the ACC-07-style after-exit DB query can assert directly.
- **Forged `ValueId` / cross-session handle denial:** already proven for the general case — `ValueStore::resolve` returns `None` for unknown ids (`value_store.rs:64-66`, tested at `:99-106`), and `submit_plan_node`'s Step 1 already treats `None` as `Denied` (`executor/src/lib.rs:49-59`). Cross-session isolation is HARD-03 (already shipped, Phase 5) — each `handle_connection` owns a fresh `ValueStore::default()` (`server.rs:129`), so a `ValueId` minted in one connection is structurally absent from another connection's store. **No new code needed for ACC-06's forged/cross-session case** — it's a regression test on existing Phase-5/6 machinery, not new Phase-7 logic. The one net-new piece is extending the *reason* returned from `Denied` to the typed `DenyReason::DanglingHandle` (Stream 1) rather than today's free-text `format!("unresolvable handle for arg '{}': ValueId not in store", arg.name)` (`lib.rs:53-57`).

### Q7 — Live e2e §9 harness extension (ACC-03/04/05)

**Existing harness pattern to extend** (do not re-invent): `cli/caprun/tests/s9_live_block.rs` already contains `run_caprun_intent_on(intent_kind, intent_param, content, tag)` — a helper that writes a workspace file, spawns the real `caprun` binary via `env!("CARGO_BIN_EXE_caprun")`, and returns `(exit_success, audit_db_path)` for DB assertions (`s9_live_block.rs:54-80`). This is the exact shape Q7 needs to reuse for the hostile block and clean allow paths through `file.create`.

**Smallest extension, concretely:**
1. **New `CaprunIntent` variant** for the clean allow-path (ACC-04) — e.g. `CaprunIntent::CreateWorkspaceFile { path: String }` (or similar), added to `crates/runtime-core/src/intent.rs:22-29` alongside `SendEmailSummary`. `plan_from_intent` (`cli/caprun/src/planner.rs:49-68`) gets a new match arm routing the `mint_from_intent`-derived `UserTrusted` `ValueId` into `file.create`'s `path` arg (and a `contents` arg — likely a second `mint_from_intent` call, or a hardcoded/scripted literal, since v0's planner is deterministic and non-LLM).
2. **Hostile content** for ACC-03: workspace file content containing a path-shaped token the new `extract_relative_path_claims` (Q4) recognizes — mirrors the existing `"...send the project summary to accounts@ev1l.com."` pattern used for the email hostile-block proof (`quarantine.rs` tests, `s9_acceptance.rs:58-59`). The worker's local extraction (`cli/caprun/src/worker.rs:118-124`, currently only `extract_email_claims`) needs a parallel call to the new path extractor, producing `WorkerClaim::RelativePath(...)` claims alongside `EmailAddress` ones.
3. **Two new live e2e tests**, same file (`s9_live_block.rs`) or a new `s9_file_create_live.rs`:
   - `s9_live_file_create_block` — hostile path claim → `mint_from_read` (ExternalUntrusted+PathRaw) → routed into `file.create`'s `path` arg by the planner → executor Blocks → assert: `caprun` exits non-success (`!output.status.success()`), **no file exists on disk** at the resolved workspace-relative location, and `find_event_by_type(conn, session_id, "sink_blocked")` returns `Some` with a non-`None` `anchor` (once Stream 1 lands).
   - `s9_live_file_create_allow` — trusted intent path → `mint_from_intent` → executor Allows → assert: `caprun` exits 0, the expected file **exists on disk with the expected contents** under the workspace root, and `find_event_by_type(conn, session_id, "sink_executed")` returns `Some` (no `sink_blocked`).
4. **Causal chain assertion** (ACC-05): extend the existing `dag_chain_integrity`-style walk (`cli/caprun/tests/e2e.rs:101-229`) pattern — for the `file.create` runs the expected causal sequence is `session_created → intent_received → fd_granted → file_read → plan_node_evaluated → sink_blocked|sink_executed`, verified via `verify_chain` (`brokerd::audit::verify_chain`, unchanged) plus a depth-ordered walk asserting exact `event_type` sequence and `parent_hash` linkage, mirroring `e2e.rs:159-225`.
5. **Both new live tests must be `#[cfg(target_os = "linux")]`-gated**, per the existing pattern in `s9_live_block.rs` (`CLEAN_PATH_CONTENT` and `s9_live_clean_allow_path` are both gated; only the cross-platform `s9_live_block_guard_binary_present` compiles unconditionally). Run via the Colima/Docker recipe in CLAUDE.md.

---

## Package Legitimacy Audit

**No new external crates are introduced by this phase.** `nix = "0.31.3"` (already workspace-pinned, `crates/adapter-fs/Cargo.toml`, `crates/sandbox/Cargo.toml`, root `Cargo.toml:9`) already provides `openat2`/`OpenHow`/`ResolveFlag` under its existing `fs` feature, which is already enabled workspace-wide. No `Cargo.toml` dependency additions are required for SINK-01..04 or HARD-04.

| Package | Registry | Status | Verdict | Disposition |
|---|---|---|---|---|
| `nix` 0.31.3 | crates.io (npm-equivalent: cargo) | Already locked in `Cargo.lock` (checksum `cf20d2fde8ff...`), workspace-pinned, `fs` feature already enabled | OK | No change — reuse existing dependency |

**Packages removed due to SLOP verdict:** none.
**Packages flagged as suspicious [SUS]:** none.
**Alternatives explicitly rejected** (see Q1): `cap-std` (redundant capability layer over the same syscall), raw `libc::syscall(SYS_openat2)` (forgoes `nix`'s safe wrapper for no benefit).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| `file.create` sink schema validation (SINK-01/HARD-01) | API/Backend (executor crate) | — | Pure decision logic, no I/O; mirrors existing `sink_sensitivity` |
| Workspace-root dirfd capability (HARD-04/SINK-04) | API/Backend (adapter-fs, broker-owned) | — | Broker has sole ambient fs access per CLAUDE.md architecture table; worker never touches fs directly |
| `openat2`-based path resolution + O_EXCL create | Database/Storage boundary (kernel-mediated) | API/Backend (adapter-fs) | The kernel is the actual enforcer of the capability; adapter-fs is the thin Rust wrapper |
| `RelativePath` claim extraction | Browser/Client-equivalent (confined worker) | — | Worker-local, lossy extraction, mirrors `extract_email_claims`; raw bytes never cross the IPC boundary |
| Taint/provenance assignment for path claims | API/Backend (broker `mint_from_read`) | — | Sole taint-mint site invariant (T-04-03); worker cannot mint |
| Sink invocation + durable result audit | API/Backend (`brokerd::dispatch_request` + new `sinks::file_create`) | Database/Storage (SQLite audit DAG) | Effect execution and its audit trail are both broker-owned, synchronous, in-TCB |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---|---|---|---|
| `nix` | 0.31.3 (already pinned) | `openat2`/`OpenHow`/`ResolveFlag` bindings | Already the project's syscall-binding crate (used for `sendmsg`/`recvmsg`/rlimits); adding the `openat2` call is zero new surface `[VERIFIED: Cargo.lock, docs.rs/nix 0.31.3]` |

### Supporting
No new supporting libraries required. `runtime-core`, `executor`, `brokerd`, `adapter-fs` are all internal workspace crates already in the dependency graph.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|---|---|---|
| `nix::fcntl::openat2` | `cap-std::fs::Dir` | Adds a second capability-safety abstraction on top of the same kernel primitive; no functional gain for a single-process broker that already owns the dirfd exclusively |
| `nix::fcntl::openat2` | raw `libc::syscall(SYS_openat2, ...)` | Loses `nix`'s `OwnedFd`/error-code safety; no reason to bypass the already-pinned safe wrapper |

**Installation:** none — `nix`'s `fs` feature is already enabled workspace-wide (`Cargo.toml:9`).

**Version verification:** `nix = "0.31.3"` confirmed present and locked in `Cargo.lock` (line 403-406, checksum `cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d`) `[VERIFIED: Cargo.lock]`. `openat2`/`OpenHow`/`ResolveFlag` availability and signatures confirmed against docs.rs for exactly this pinned version `[VERIFIED: docs.rs/nix/0.31.3]`.

## Architecture Patterns

### System Architecture Diagram

```
                         caprun main() (orchestrator)
                                 |
              opens workspace-root dirfd (nix::fcntl::open, O_DIRECTORY)
                                 |
                                 v
                    brokerd::server::run_broker_server
                    (owns: SQLite conn, per-connection ValueStore,
                     shared WorkspaceRoot dirfd)
                                 |
                    accepts UDS connection from caprun-worker
                                 |
                                 v
                       dispatch_request (per message)
        +------------------------+-------------------------+
        |                        |                          |
   ProvideIntent            RequestFd                  ReportClaims
   mint_from_intent    openat2(dirfd, RESOLVE_       WorkerClaim::EmailAddress
   -> UserTrusted        BENEATH|NO_SYMLINKS)         WorkerClaim::RelativePath(new)
   ValueId                -> pass fd via SCM_RIGHTS   -> mint_from_read
        |                  (existing adapter-fs)      -> [ExternalUntrusted,+label]
        |                        |                          |
        |                        v                          |
        |                 confined worker reads              |
        |                 via received fd, extracts           |
        |                 claims LOCALLY (lossy)               |
        |                        |                          |
        +------------------------+--------------------------+
                                 |
                                 v
                     SubmitPlanNode { plan_node }
                                 |
                    1. validate_schema(sink,args)  <- NEW (HARD-01)
                       unknown sink/arg -> Denied
                                 |
                    2. resolve each PlanArg.value_id
                       (dangling -> Denied, empty taint/
                        provenance -> Denied, Stream 1)
                                 |
                    3. routing-sensitivity + is_untrusted()
                       -> Allowed | BlockedPendingConfirmation{anchor}
                                 |
                    4. durable authorization audit append
                       (sink_blocked | plan_node_evaluated,
                        anchor required for sink_blocked, Stream 1)
                                 |
                    5. IF Allowed AND sink=="file.create":     <- NEW (HARD-05/06)
                       sinks::file_create::invoke_file_create(
                         workspace_root_dirfd, resolved literals)
                       -> openat2(O_CREAT|O_EXCL|RESOLVE_BENEATH
                                  |RESOLVE_NO_SYMLINKS)
                                 |
                    6. durable result audit append
                       (sink_executed | explicit failure event)  <- NEW
                                 |
                                 v
                       send_response(PlanNodeDecision)
                                 |
                                 v
                     worker exits 0 (Allowed) or 1 (Blocked)
```

### Recommended Project Structure
```
crates/
├── adapter-fs/
│   └── src/
│       ├── lib.rs           # existing pass_fd/recv_fd (unchanged)
│       └── workspace.rs     # NEW: WorkspaceRoot(OwnedFd), read_within, create_exclusive_within
├── executor/
│   └── src/
│       ├── lib.rs           # add validate_schema call as Step 0; add empty-taint/provenance guard (Stream 1)
│       ├── sink_sensitivity.rs  # extend is_routing_sensitive with "file.create" -> ["path"]
│       └── sink_schema.rs   # NEW: KNOWN_SINKS, per-sink arg lists, validate_schema -> Result<(),DenyReason>
├── brokerd/
│   └── src/
│       ├── server.rs        # dispatch_request: RequestFd uses WorkspaceRoot; SubmitPlanNode invokes sink on Allowed
│       ├── quarantine.rs    # add extract_relative_path_claims; RelativePath arm in mint dispatch (server.rs ReportClaims)
│       ├── proto.rs         # WorkerClaim::RelativePath(String) — uncomment the existing stub
│       └── sinks.rs / sinks/
│           ├── email_send.rs   # existing stub (still not called live — no change required for Phase 7 scope)
│           └── file_create.rs  # NEW: real invocation, openat2-based, appends sink_executed
└── runtime-core/
    └── src/
        └── plan_node.rs      # add TaintLabel::PathRaw variant (forces is_untrusted() match update)
```

### Pattern 1: Broker-side self-cfg-gated syscall wrapper (mirror `sandbox::landlock`)
**What:** every Linux-specific syscall wrapper in this codebase pairs a real `#[cfg(target_os="linux")]` implementation with a `#[cfg(not(target_os="linux"))]` fallback that returns `Ok(())`/a functional-but-unhardened equivalent, never a compile error, on macOS.
**When to use:** for `WorkspaceRoot::read_within`/`create_exclusive_within` and any other `openat2`-based function.
**Example:**
```rust
// Source: mirrors crates/sandbox/src/landlock.rs:16-38 (existing pattern in this codebase)
#[cfg(target_os = "linux")]
pub fn read_within(&self, rel_path: &str) -> std::io::Result<std::fs::File> {
    use nix::fcntl::{openat2, OFlag, OpenHow, ResolveFlag};
    let how = OpenHow::new().flags(OFlag::O_RDONLY)
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
    let fd = openat2(&self.dirfd, rel_path, how)
        .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
    Ok(std::fs::File::from(fd))
}

#[cfg(not(target_os = "linux"))]
pub fn read_within(&self, rel_path: &str) -> std::io::Result<std::fs::File> {
    // No security claim on non-Linux — plain join+open, matches CLAUDE.md scope.
    std::fs::File::open(self.root_path.join(rel_path))
}
```

### Pattern 2: Two-phase durable audit around a side effect (HARD-06)
**What:** append an "authorized" event BEFORE performing the side effect; append a "result" event AFTER. Never perform the side effect without a prior durable authorization record.
**When to use:** any new sink invocation (`file_create`).
**Example:**
```rust
// Extends the existing pattern already used for fd_granted / sink_blocked in server.rs:255-357
// 4. durable authorization audit (existing plan_node_evaluated append, unchanged)
// 5. invocation — only reached if the above append succeeded
let create_result = sinks::file_create::invoke_file_create(&workspace_root, &plan_node, value_store, effect_id);
// 6. durable result audit — always append, success or failure, before responding to worker
let result_event_type = match &create_result { Ok(_) => "sink_executed", Err(_) => "sink_execution_failed" };
```

### Anti-Patterns to Avoid
- **`std::fs::canonicalize` + separate `std::fs::write`:** reintroduces the exact TOCTOU race `openat2(RESOLVE_BENEATH)` exists to eliminate (Q3). Never split resolution and action into two syscalls for a security-boundary path.
- **A second `TrustClass`/partition type alongside `is_untrusted()`:** explicitly killed by board review (REV.2 §2) — reuse `is_untrusted()` everywhere, including for any new `PathRaw` label.
- **A second, parallel error/deny type for schema violations:** extend the Stream-1 `DenyReason` enum; do not invent `SchemaError`/`ValidationError` alongside it.
- **Minting workspace-content-derived path values as `LocalWorkspace`:** CONTEXT.md constraint — tag `ExternalUntrusted` until a threat specialist rules on workspace-content trust (unreviewed, not cleared).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Path traversal / symlink-escape prevention | A custom path-string sanitizer (reject `..`, reject leading `/`, etc. as the *sole* defense) | `openat2(RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS)` as the real boundary | String-level sanitizers are a well-documented hallucination-prone vector (Unicode normalization tricks, encoded slashes, symlink races) — the kernel resolution-time check is authoritative. A syntactic pre-check (reject absolute/`..` in the literal) is fine as an early fail-fast *in addition to*, never *instead of*, the openat2 call. |
| Atomic file creation | Manual `if !exists(path) { write(path) }` | `O_CREAT\|O_EXCL` (POSIX-standard atomic exclusive creation, exposed via `nix::fcntl::OFlag`) | `O_EXCL` is kernel-atomic; any userspace exists-check-then-write has a race window |
| Cross-session value isolation | A manual per-request session token check | The existing per-connection `ValueStore::default()` (HARD-03, already shipped) | Already proven — reuse, don't rebuild |

**Key insight:** every "don't hand-roll" item in this phase reduces to the same principle already embedded in the codebase's architecture: push the security-critical decision into a single kernel syscall or a single typed Rust match, never a multi-step userspace sequence with a gap an attacker can race.

## Common Pitfalls

### Pitfall 1: Forgetting `sinks::email_send` is currently dead code
**What goes wrong:** a planner or implementer assumes sink invocation is already wired because `invoke_email_send_stub` exists and looks production-ready.
**Why it happens:** the function is fully implemented, documented, and unit-tested — but grep-verified as called **only from its own test module**, never from `crates/brokerd/src/server.rs`'s `dispatch_request`.
**How to avoid:** treat "wire actual sink invocation into `dispatch_request`'s `SubmitPlanNode` arm on `Allowed`" as net-new Phase-7 scope for `file.create`, not an existing pattern to merely extend.
**Warning signs:** a plan that says "mirror `email_send`'s dispatch wiring" without first confirming that wiring exists (it doesn't).

### Pitfall 2: Conflating `RESOLVE_BENEATH` with full symlink protection
**What goes wrong:** implementing `RESOLVE_BENEATH` alone and believing symlink escapes are closed.
**Why it happens:** the flag name and its "escape from root" documentation sound complete.
**How to avoid:** always pair `RESOLVE_BENEATH` with `RESOLVE_NO_SYMLINKS` for this threat model (SINK-04/HARD-04 both require symlink escapes rejected, and the worker/attacker never has a legitimate reason to create or follow a symlink under the workspace root).
**Warning signs:** a test that creates an in-workspace symlink pointing outside the tree and expects it to be followed-then-blocked rather than rejected-at-resolution.

### Pitfall 3: Building a retry mechanism for HARD-06
**What goes wrong:** interpreting "crash after invocation leaves an explicit indeterminate record" as a signal to build automatic recovery/retry logic.
**Why it happens:** "indeterminate" sounds like a state that should be resolved.
**How to avoid:** HARD-06 explicitly says "no automatic retry" — the correct implementation is the *absence* of retry code, satisfied naturally by the two-phase audit design (Q6). The indeterminate state is surfaced for a human/operator to inspect via the audit DB, not auto-resolved.

### Pitfall 4: Resolving the path claim's literal at `ReportClaims`/mint time
**What goes wrong:** calling `openat2` inside the `RelativePath` mint arm (mirroring how `RequestFd` opens a file) instead of only at `file.create` sink-invocation time.
**Why it happens:** `RequestFd`'s existing handler does open a file immediately, so it's tempting to mirror that shape for path claims.
**How to avoid:** `mint_from_read` for `RelativePath` should mint a **string** value with taint attached — no filesystem I/O — exactly like `mint_from_read` for `EmailAddress` never validates the address is deliverable. Resolution against the workspace-root capability happens once, at sink-invocation time, after the executor decision (Q4 point 6, Q5/Q6).

## Code Examples

### `OpenHow` builder (verified shape)
```rust
// Source: docs.rs/nix/0.31.3/nix/fcntl (verified 2026-06-30 against pinned workspace version)
use nix::fcntl::{OFlag, OpenHow, ResolveFlag};
let how = OpenHow::new()
    .flags(OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY)
    .mode(nix::sys::stat::Mode::from_bits_truncate(0o600))
    .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
```

### Existing mirror-this-pattern: `mint_from_read` (sole taint-mint site)
```rust
// Source: crates/brokerd/src/quarantine.rs:129-164 (unchanged pattern — RelativePath mint reuses this exact function)
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, ValueId)> { /* ... appends file_read event, mints record in one call ... */ }
```

### Existing mirror-this-pattern: cfg-gated real/stub split
```rust
// Source: crates/sandbox/src/landlock.rs:16-38 (verified real code, exact pattern to replicate for openat2 wrappers)
#[cfg(target_os = "linux")]
pub fn deny_all_filesystem() -> std::io::Result<()> { /* real landlock ruleset */ }

#[cfg(not(target_os = "linux"))]
pub fn deny_all_filesystem() -> std::io::Result<()> { Ok(()) }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| `stat`/`realpath` + `open` for "safe" path handling | `openat2(RESOLVE_BENEATH)` single-syscall resolution | Linux 5.6 (2020) | Eliminates an entire TOCTOU class userspace path-safety code has struggled with for decades |
| `AT_FDCWD`/absolute-path opens for sandboxed file access | dirfd-anchored `openat`/`openat2` | Standard practice since `openat()` (Linux 2.6.16, 2006); `openat2` adds resolution-flag enforcement | This codebase already uses dirfd-style thinking for Landlock (deny-then-allowlist); `openat2` extends the same anchoring model to explicit application code |

**Deprecated/outdated:** none specific to this phase — `openat2` is itself the modern replacement for ad-hoc path-safety checks; nothing in this design uses an outdated pattern.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | Recommendation to add `TaintLabel::PathRaw` (vs. reusing `WorkerExtracted`) is Claude's design judgment, not board-ratified | Q4 | Low — either choice satisfies `is_untrusted()`; if the planner picks reuse instead, no functional change, just a smaller diff |
| A2 | Recommendation to derive workspace-root from the existing `workspace-file` CLI arg's parent directory (Option a) rather than adding an explicit `--workspace-root` flag | Q2 | Medium — if the planner picks Option (b) instead, CLI parsing and e2e test call-sites change shape; either is compatible with the architecture, but the plan's task list differs |
| A3 | Recommendation that `file.create`'s `contents` arg is sourced via a second `mint_from_intent` call (clean path) or a scripted literal — the exact mechanism for populating `contents` on the clean allow-path is not specified by ROADMAP/REQUIREMENTS | Q7 | Low — affects only the shape of the new `CaprunIntent` variant, not the security properties |

**None of these assumptions affect Stream 1** (the locked ACC-07 spec) — they are all scoped to Stream 2 design choices explicitly delegated to "Claude's Discretion" in CONTEXT.md.

## Open questions / risks for the planner

1. **CLI surface for workspace-root (A2 above).** Confirm Option (a) — derive from `workspace-file`'s parent directory, worker sends a relative path — before task breakdown, since it determines whether `cli/caprun/src/main.rs`'s argument parsing changes and how many existing e2e test call-sites need updating.
2. **`file.create`'s `contents` arg source on the clean path (A3 above).** REQUIREMENTS.md ACC-04 says "creates exactly the expected file" but does not specify whether `contents` comes from a second trusted mint, a hardcoded planner literal, or reads from the same workspace file. Recommend: a second `mint_from_intent` call for symmetry with `path`, keeping the planner fully deterministic and free of literals (PLAN-03 discipline).
3. **Sequencing risk:** the mint-invariant task (Stream 1 prerequisite) and the `DenyReason` enum it introduces are a hard dependency for Q5's schema-validation work. The plan must sequence "mint invariant + `DenyReason` base enum" strictly before "HARD-01 schema validation extends `DenyReason`" — a wave-parallel plan that lands both simultaneously risks a merge conflict on the same enum.
4. **`sink_sensitivity.rs` naming may need generalizing.** Today's `EMAIL_SEND_ROUTING_SENSITIVE`/`EMAIL_SEND_CONTENT_SENSITIVE` constants are sink-specific; adding `file.create`'s `path` as routing-sensitive (SINK-02) fits the existing `match sink.0.as_str()` pattern in `is_routing_sensitive` without restructuring, but the planner should decide whether to also generalize the constant-naming convention now or defer (cosmetic, non-blocking).
5. **`RelativePath` extraction heuristic is undefined.** Unlike `looks_like_email` (a well-defined structural shape), "looks like a workspace-relative path" has no obvious deterministic signature (e.g., is `notes.txt` a path claim? Is `../secret`?). Recommend keeping the extractor conservative (e.g., require a `/` or a recognized extension, and explicitly INCLUDE `..`-containing tokens as valid claims — the taint/executor/openat2 layers are what reject them, not the extractor; the extractor's job is only to find candidate strings, matching the `extract_email_claims` philosophy of "lossy but not filtering for validity").

## Validation Architecture

### Test Framework
| Property | Value |
|---|---|
| Framework | Rust built-in `cargo test` (workspace), `#[test]` + `#[cfg(target_os="linux")]` gating (existing convention, no new framework) |
| Config file | none — no `pytest.ini`/`jest.config`; convention lives in per-crate `tests/` dirs and inline `#[cfg(test)] mod tests` |
| Quick run command | `cargo test -p executor -p brokerd -p runtime-core --lib` (cross-platform unit tests only, <30s) |
| Full suite command | `cargo test --workspace --no-fail-fast` (macOS: Linux-gated tests report 0 run, expected); Linux-gated full run via the Colima/Docker recipe in CLAUDE.md |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| SINK-01 | schema validation rejects missing/duplicate/unknown args | unit | `cargo test -p executor sink_schema` | ❌ Wave 0 — new `sink_schema.rs` + tests |
| SINK-02 | `file.create.path` is routing-sensitive | unit | `cargo test -p executor sink_sensitivity` | ✅ existing file, add test cases |
| SINK-03 | `O_EXCL` never overwrites | unit (Linux-gated) | `cargo test -p adapter-fs workspace -- --ignored` or `#[cfg(target_os="linux")]` | ❌ Wave 0 — new `workspace.rs` + tests |
| SINK-04 | `openat2` rejects absolute/traversal/symlink paths | unit (Linux-gated), negative-assertion | `cargo test -p adapter-fs workspace` (Linux only) | ❌ Wave 0 |
| HARD-01 | unknown sink/arg fail closed before sensitivity/executor | unit | `cargo test -p executor` | ❌ Wave 0 |
| HARD-04 | `RequestFd` capability-restricted to workspace root | unit (Linux-gated) + live e2e | `cargo test -p adapter-fs`, `cargo test -p caprun --test e2e` (Linux) | ❌ Wave 0 for unit; ✅ e2e.rs exists, extend |
| HARD-05 | dispatch ordering enforced, audit failure fails closed | in-process dispatch | `cargo test -p brokerd --test phase5_dispatch` (extend) | ✅ existing file, extend |
| HARD-06 | effect_id + crash-indeterminate + no auto-retry | in-process dispatch, after-exit DB-alone | new `crates/brokerd/tests/hard06_crash_indeterminate.rs` | ❌ Wave 0 |
| ACC-01 | BlockedPendingConfirmation operational definition (0 invocations + non-success exit + durable event) | live e2e (Linux-gated) | extend `s9_live_block.rs` or new `s9_file_create_live.rs` | ❌ Wave 0 (new hostile test) |
| ACC-03 | live file.create block, no file written | live e2e (Linux-gated) | new test, per Q7 | ❌ Wave 0 |
| ACC-04 | clean allow-path creates expected file | live e2e (Linux-gated) | new test, per Q7 | ❌ Wave 0 |
| ACC-05 | causal chain `fd_granted→file_read→plan_node_evaluated→sink_blocked/executed` | live e2e (Linux-gated), DB walk | extend `e2e.rs`-style chain walk | ❌ Wave 0 |
| ACC-06 | forged handle / cross-session denial | in-process unit (regression on existing HARD-03) | `cargo test -p executor -p brokerd` | ✅ mostly covered by existing HARD-03 tests, add file.create-specific case |
| ACC-07 | genuine-taint sentinel, after-exit DB-alone, tamper-evidence | dispatch-level after-exit DB-alone (Stream 1, cross-platform) | new `crates/brokerd/tests/durable_anchor.rs` per REV.2 §7 | ❌ Wave 0 — spec exists (REV.2), test does not |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched-crate> --lib` (quick run)
- **Per wave merge:** `cargo test --workspace --no-fail-fast` (macOS pass) + Linux-gated full run via Colima/Docker for any wave touching `sandbox`/`adapter-fs`/e2e tests
- **Phase gate:** full Linux-gated suite green (Colima/Docker) before `/gsd-verify-work` — this phase's acceptance criteria are explicitly Linux-only per CONTEXT.md

### Wave 0 Gaps
- [ ] `crates/adapter-fs/src/workspace.rs` — `WorkspaceRoot`, `read_within`, `create_exclusive_within` + Linux-gated unit tests (SINK-03/04, HARD-04)
- [ ] `crates/executor/src/sink_schema.rs` — `validate_schema` + `DenyReason` extension + unit tests (SINK-01, HARD-01)
- [ ] `crates/brokerd/src/sinks/file_create.rs` — real invocation + `sink_executed` audit event
- [ ] `crates/brokerd/tests/hard06_crash_indeterminate.rs` — two-phase audit indeterminate-state assertion
- [ ] `crates/brokerd/tests/durable_anchor.rs` — ACC-07 after-exit DB-alone + tamper-evidence tests (Stream 1 spec, REV.2 §7)
- [ ] `cli/caprun/tests/s9_file_create_live.rs` (or extend `s9_live_block.rs`) — ACC-01/03/04/05 live proofs
- [ ] `crates/runtime-core` tests — `TaintLabel::PathRaw` added to the existing exhaustive-match unit test table (mirrors `intent_taint.rs`)
- [ ] Framework install: none — all frameworks already present

*(Stream 1's own Wave 0 gaps — golden byte-fixture test, mint-invariant unit tests — are specified in `TASK-mint-nonempty-invariant.md` and `DESIGN-durable-anchor-and-label-partition.md` §7; not duplicated here.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---|---|---|
| V2 Authentication | no | no user-auth surface in caprun v0 |
| V3 Session Management | yes (existing) | session-scoped `ValueStore` (HARD-03, shipped Phase 5) — unchanged this phase |
| V4 Access Control | yes | workspace-root dirfd capability (HARD-04/SINK-04) is the access-control primitive this phase adds |
| V5 Input Validation | yes | arg-schema validation (HARD-01/SINK-01), `RelativePath` claim extraction validated at broker before mint |
| V6 Cryptography | yes (existing) | SHA-256 audit-chain hashing (`brokerd::audit`) — unchanged this phase, reused by Stream 1 |
| V12 File and Resources | yes | this phase's core: `openat2` path-traversal/symlink-escape prevention, `O_EXCL` exclusive creation |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---|---|---|
| Path traversal (`../../etc/passwd`) | Tampering / Elevation of Privilege | `openat2(RESOLVE_BENEATH)` — kernel-enforced, not string filtering |
| Symlink race / TOCTOU | Tampering | `openat2(RESOLVE_NO_SYMLINKS)` — single-syscall resolution+action, no separate stat/check step |
| Overwrite of existing file (destructive) | Tampering | `O_EXCL` — atomic exclusive creation; explicitly out of scope: `file.write`/overwrite is v2 (SINK-F3) |
| Value-injection at a routing-sensitive sink arg (I2) | Tampering / Spoofing | executor's `is_untrusted()` predicate over `PlanArg`→`ValueRecord` resolution (existing, unchanged) |
| Forged/dangling `ValueId` handle | Spoofing | `ValueStore::resolve` → `None` → `Denied` (existing) |
| Cross-session handle reuse | Spoofing / Elevation of Privilege | per-connection `ValueStore` isolation (HARD-03, existing) |
| Unknown sink/arg silently allowed | Elevation of Privilege | new schema-validation gate (HARD-01, this phase) — currently the primary open gap this phase closes |
| Crash mid-invocation masking effect state | Repudiation | two-phase durable audit (authorization-before, result-after) — HARD-06 |
| Audit-log tampering | Tampering | SHA-256 hash chain + `verify_chain` (existing, reused unchanged by ACC-07's tamper-evidence test) |

## Sources

### Primary (HIGH confidence)
- `crates/brokerd/src/{quarantine,server,proto,audit,approval,lib,sinks}.rs`, `crates/brokerd/src/sinks/email_send.rs`, `crates/executor/src/{lib,value_store,sink_sensitivity}.rs`, `crates/runtime-core/src/{plan_node,event,executor_decision,value_record,intent,lib}.rs`, `crates/adapter-fs/src/{lib,protocol}.rs`, `crates/sandbox/src/{lib,landlock}.rs`, `cli/caprun/src/{main,worker,planner}.rs`, `cli/caprun/tests/{e2e,s9_live_block}.rs`, `crates/brokerd/tests/{s9_acceptance,phase5_dispatch}.rs`, `Cargo.toml`/`Cargo.lock` (all read directly in this session, 2026-06-30)
- `planning-docs/DESIGN-durable-anchor-and-label-partition.md` (REV.2), `planning-docs/TASK-mint-nonempty-invariant.md`, `planning-docs/PHASE-7-HANDOFF.md`, `.planning/phases/07-.../07-CONTEXT.md`, `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md` — all read directly
- `docs.rs/nix/0.31.3/nix/fcntl` (`openat2`, `OpenHow`, `ResolveFlag`) — fetched and cross-checked against the raw nix v0.31.3 source on GitHub, confirming `#[cfg(target_os="linux")]` gating
- `man7.org/linux/man-pages/man2/openat2.2.html` — kernel-version (5.6), `RESOLVE_BENEATH`/`RESOLVE_NO_SYMLINKS` semantics, `EXDEV` error code

### Secondary (MEDIUM confidence)
- none used beyond the primary sources above — this phase's grounding is entirely in read source code and official docs, no unverified WebSearch claims were incorporated into recommendations

### Tertiary (LOW confidence)
- Initial WebSearch surfacing `cap-std` as a candidate crate — considered and explicitly rejected in favor of the already-pinned `nix` binding (see Alternatives Considered); not used in any recommendation

## Metadata

**Confidence breakdown:**
- Standard stack (nix/openat2 API): HIGH — verified against docs.rs for the exact pinned version, cross-checked against nix's own source repo and man7.org
- Architecture (dispatch ordering, capability model, sink wiring): HIGH for what-exists-today (all read directly from source with file:line citations); MEDIUM for the specific design recommendations (Q2/Q4/Q5's `PathRaw`/CLI-surface/`DenyReason`-extension choices) since these are Claude's-discretion design calls, not board-ratified — flagged explicitly in Assumptions Log
- Pitfalls: HIGH — all five pitfalls are grounded in specific verified facts (dead-code `email_send`, `RESOLVE_BENEATH` semantics, HARD-06 wording, mint-time-vs-invocation-time distinction)

**Research date:** 2026-06-30
**Valid until:** 30 days (stable domain — Linux syscall APIs and the project's own locked architecture change slowly; re-verify if `nix` crate version bumps in `Cargo.toml`)
