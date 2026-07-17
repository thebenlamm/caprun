# DESIGN — Effect Breadth I: `process.exec` + Filesystem Read/Write Breadth

**Milestone:** v1.7 — Effect Breadth I
**Phase:** 31 (Design Gate) — blocks all `crates/executor` / `crates/brokerd` /
`crates/sandbox` / `crates/runtime-core` code for this milestone
**Status:** Draft → pending fresh (non-self) adversarial review (see
`DESIGN-GATE-RECORD-v1.7.md`, produced by Plan 31-02)
**Author date:** 2026-07-17
**Grounding:** `.planning/phases/31-effect-breadth-design-gate/31-RESEARCH.md` (every
file:line below traces to a direct code read this session; re-verify if Phase 32
begins many commits later, per this project's own convention)
**Requirements:** DESIGN-13, DESIGN-14 (this doc) → enables EXEC-01..04 (Phase 32),
FS-01..03 (Phase 33), LIVE-01/02 (Phase 34)

> **Design-gate discipline.** No `crates/executor` / `crates/brokerd` /
> `crates/sandbox` / `crates/runtime-core` code for `process.exec` or the fs
> write/edit sink may be written until this document clears a fresh, non-self
> adversarial review with every finding resolved — mirroring v1.0 Phase 2,
> v1.2 Phase 8, v1.3 Phase 12, v1.4 Phase 18, v1.5 Phase 23, v1.6 Phase 26.
> This doc pins **decisions**, not options; Phase 32/33 are a mechanical
> realization of what is fixed here.

---

## §0. Purpose & Scope

**What this doc pins.** Two new effect primitives and their fail-closed
defaults, before any TCB code exists:

1. **DESIGN-13 model 1** — the `process.exec` broker-spawned confined-child
   model: who spawns the child, how it is kernel-confined, how its
   stdout/stderr are captured, and how the captured output is taint-minted
   (§1, §2).
2. **DESIGN-13 model 2** — the filesystem read/write-breadth model: reading
   multiple workspace files (FS-01) and writing/editing an existing file
   (FS-02) (§3).
3. **DESIGN-14** — the fail-closed defaults for both new sinks slotting into
   the existing I2 / slot-type-binding machinery: `process.exec`'s
   command/arg schema and (dis)allow posture, the exec-output taint label +
   `origin_role`, and the fs read/write path & slot constraints (§4, §5).

This doc **hard-blocks Phases 32-34** (CLAUDE.md: "Two design-gate docs block
executor code" — this is the effect-breadth analog of
`DESIGN-taint-model.md`/`DESIGN-plan-executor.md` for v0, and of
`DESIGN-slot-type-binding.md` for v1.5). `process.exec` under Landlock+seccomp
is the riskiest primitive shipped to date — genuinely novel (no `.pre_exec(`
call exists anywhere in this codebase today, confirmed by a full-tree grep at
RESEARCH time) — so this doc pins the model precisely enough that a fresh
reviewer can trace every claim against real code.

**What this doc does NOT pin (deferred to Phase 32/33 as deployment
constants, not model gaps — §8):**
- The exact enumerated Landlock allow-list path strings for the `rust:1`
  verification container (§0's method is pinned in §1; the literal paths are
  environment-dependent).
- Whether `RequestFd`'s current absence of a per-session read-count limiter is
  a genuine gap or an unexercised-but-fine default (§3 pins the fail-closed
  answer: add an explicit upper bound regardless).
- Confirmation of the verification container's exact kernel version floor.

**Locked terminology (unchanged by this doc):** `Intent`, `Session`,
`Planner`, `Worker`, `Broker`, `Adapter`, `Effect`, `Artifact`, `Event`.
`ExecutionContext` remains internal-only. Nothing in this doc introduces new
public-API vocabulary.

**No TCB code this phase.** This doc lives entirely under `planning-docs/`.
The git diff for Plan 31-01 touches only `planning-docs/DESIGN-effect-breadth-exec.md`.

**Explicitly out of scope (locked at milestone scoping,
`.planning/REQUIREMENTS.md` Future Requirements):** Git/GitHub adapters,
`http.request`, shell-script execution (recursive exec) as a first-class
primitive, a command allowlist/policy engine, cross-host delegation. These
remain v1.8+ decisions.

---

## §1. `process.exec` — Broker-Spawned Confined-Child Model (DESIGN-13)

### 1.1 Why the broker, not the worker, must spawn the child

The confined worker's own seccomp filter denies `execve`/`execveat`
unconditionally (`crates/sandbox/src/seccomp.rs:64-66`, both denied with an
empty-vec "always match" rule, matched action `Errno(EPERM)`). This is a
structural guarantee, not a policy toggle — the worker **cannot** run an
external command under any circumstance. Therefore `process.exec` MUST be a
**broker-spawned separate process**, mediated exactly like every other
external effect this project makes (adapter-fs's fd-pass mediation is the
generalizing precedent: the broker performs the ambient-authority action, the
confined side only ever receives an opaque, mediated result).

### 1.2 Why neither existing broker-spawn precedent fits directly

Two broker-spawned child precedents already exist in `cli/caprun/src/main.rs`,
neither of which fits `process.exec` unmodified:

- **The v1.4 `caprun-planner` sidecar** (`main.rs:311-332`) — spawned via a
  plain `std::process::Command::new(&planner_binary)...spawn()`, **fully
  unconfined**. This is safe only because the sidecar is caprun's OWN trusted
  binary. `process.exec`'s target is arbitrary, non-caprun code — running it
  unconfined would be a direct arbitrary-code-execution hole.
- **The `caprun-worker` spawn** (`main.rs:334-357`) — spawned normally, then
  **self-confines AFTER connecting** to the broker
  (`crates/sandbox/src/lib.rs:1-18`, `apply_confinement()`). Self-confinement
  works for the worker because the worker is caprun's own binary that knows
  to call `sandbox::apply_confinement()` post-handshake — `crates/sandbox/src/lib.rs:7-18`
  explicitly documents that this ordering exists BECAUSE Landlock deny-all
  and seccomp deny-execve, if applied in `pre_exec`, would block the worker's
  own binary from ever loading. `process.exec`'s target is never caprun's own
  binary and has no IPC handshake to self-confine after — there is nothing
  to teach it to call `apply_confinement()`.

The only way to kernel-confine an **arbitrary** child is to apply confinement
in the fork, **before** the child's own `execve` — via
`std::process::Command::pre_exec()` (a stdlib extension trait,
`std::os::unix::process::CommandExt`, not a new dependency). This is
genuinely new: no `.pre_exec(` call exists anywhere in this codebase today.

### 1.3 Spawn ownership — Option A (recommended) vs Option B (fallback)

- **Option A (RECOMMENDED, fail-closed default):** `brokerd`'s own dispatch
  handler (already inside a `tokio::spawn`'d per-connection task,
  `crates/brokerd/src/server.rs:271,308`) calls
  `std::process::Command::new(cmd).pre_exec(|| { ... }).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`
  inline. This is the exact `std::process::Command::spawn()` call shape
  already exercised twice, unmodified, in this codebase (`main.rs:328,356`) —
  only the `pre_exec` closure and `Stdio::piped()` capture are net-new.
- **Option B (documented fallback):** a dedicated `caprun-exec-launcher`
  helper binary, spawned unconfined (mirroring `caprun-worker`'s
  separate-binary pattern), which receives the target command over the same
  kind of env-var/UDS channel already used for the worker and performs its
  OWN post-fork self-confinement (the SAME proven ordering as
  `apply_confinement()`) before its own `execve`. This avoids requiring the
  `pre_exec` closure itself to be async-signal-safe (§9), at the cost of an
  extra binary + IPC round-trip.
- **Ruling:** pin **Option A** as the v1.7 default. The rationale is that
  `Command::spawn()` on Linux performs fork+exec as a single library call
  (not a raw manual `fork()` inside caprun's own multi-threaded async
  runtime), and this exact call shape already runs twice in production
  without incident. This doc does NOT silently assume away the
  async-signal-safety caveat that Option A's `pre_exec` closure raises — it
  is a named, accepted residual risk (§9), with Option B as the documented
  fallback if the fresh adversarial reviewer rules it a blocker.

### 1.4 Kernel confinement of the exec child

**Landlock — a NEW narrow-allow-list ruleset, NOT `deny_all_filesystem()`
reused verbatim.** `deny_all_filesystem()`
(`crates/sandbox/src/landlock.rs:16-32`) calls
`Ruleset::default().handle_access(AccessFs::from_all(ABI::V3)).create()...restrict_self()`
with **zero allow-rules added** — everything, including the `Execute` access
right, is denied. That ruleset was designed for the WORKER, which
self-confines AFTER its own binary has already loaded and is running — a
fundamentally different ordering than `pre_exec`, which must apply BEFORE the
target's own `execve`. Reusing `deny_all_filesystem()` verbatim in a
`pre_exec` closure would make the target binary itself fail to load
(`EACCES`/`ENOEXEC` on the very first `execve`). This doc pins a **distinct,
NEW ruleset constructor** — provisionally named `exec_child_ruleset()`,
living beside `deny_all_filesystem()` in `crates/sandbox/src/landlock.rs` —
with explicit allow-rules: `ReadFile`+`Execute` on standard system
binary/library paths (`/usr`, `/bin`, `/lib`, `/lib64` or the container's
equivalent), `ReadFile`+`WriteFile` on `WorkspaceRoot` only, deny everything
else (including `~/.ssh`, matching the existing M0 negative assertion
`planning-docs/PLAN.md:152`). Consistent with this project's "sink
sensitivity map is hardcoded, no runtime registry" discipline
(`crates/executor/src/sink_sensitivity.rs:1-9`) applied to the confinement
layer: the allow-list is explicitly enumerated, never a wildcard. The exact
literal path strings are an Open Item (§8), not pinned here — the METHOD
(explicit hardcoded allow-list, narrowest-that-works, resolved against the
Phase 32 verification container's actual layout) is what this doc fixes.

**seccomp — reused network-deny, no execve-deny for the child's own one-time
exec.** The worker's filter (`crates/sandbox/src/seccomp.rs:62-103`) denies
`execve`/`execveat` unconditionally and `socket(AF_INET/AF_INET6)` — correct
for the worker, wrong for the exec child, which needs exactly ONE `execve` to
run. This doc pins a NEW `exec_child_filter()` (beside
`apply_worker_filter()` in `seccomp.rs`) that reuses the identical
`socket(AF_INET/AF_INET6)` deny rule unchanged (default-deny net, §T-31-04),
but does **NOT** add an execve deny for that one legitimate exec — the filter
is installed inside `pre_exec`, BEFORE the child's own `execve` call, and
persists across that `execve` per standard Linux seccomp-BPF inheritance
semantics (an assumption, not re-verified against kernel source this session
— §9 Assumption). Whether the exec child's OWN descendants may further
`execve` (e.g. a shell script re-execing sub-commands) is a genuine open
question (RESEARCH Open Question 2); this doc pins the fail-closed default:
**deny recursion** — the child's own filter denies `execve`/`execveat` for
anything AFTER its own initial one, closing the path where an unaudited
grandchild makes network calls or spawns further processes the executor
never scored. "Run a shell script" is explicitly out of scope for v1.7 (a
v1.8+ decision alongside `git`/`http.request`, per REQUIREMENTS.md Future
Requirements).

**rlimits — reused unchanged, PLUS a NEW wall-clock timeout.**
`RLIMIT_AS`/`RLIMIT_CPU` (`crates/sandbox/src/rlimits.rs:13-27`) are reused
unmodified. `RLIMIT_CPU` bounds **CPU-seconds consumed**, not wall-clock
elapsed time (`rlimits.rs:5`, "wall-clock unlimited; CPU-time bounded") — a
child that blocks on I/O or sleeps evades it entirely. No existing mechanism
in this codebase bounds wall-clock time. This doc pins a NEW broker-side
`tokio::time::timeout(...)` wrapped around the child's `wait()`, killing via
the SAME `child.kill()` teardown path this codebase already exercises for
the planner sidecar (`main.rs:372-378`) — no new syscall surface, reuses an
already-proven kill path. This closes T-31-05 (DoS via an idle/sleeping
child).

**Captured-output byte cap.** No existing mechanism bounds captured
stdout/stderr size. This doc pins an explicit byte cap on the combined
captured output (a sane default, e.g. on the order of 10 MiB — exact value
is a Phase 32 implementation detail, not re-litigated here); exceeding it is
a **fail-closed deny/truncate**, never fail-open (never silently drop the
cap and keep reading unboundedly).

**stdout/stderr capture.** Pinned via `Stdio::piped()` on the `Command`
builder — the standard `std::process::Command` capture mechanism, not an
fd-pass (§1.6 explains why fd-pass is the wrong template here).

### 1.5 `process.exec` arg schema

Pinned shape (RESEARCH Open Decision 4):
- `command: String` — a single resolved binary path or `PATH`-relative name.
  **Never** passed through `sh -c` or any shell interpreter — this closes
  shell-metacharacter injection (`; rm -rf`, `$(...)`) **by construction**,
  not by sanitization (T-31-02).
- `args: Vec<String>` — each element is a separate `execve` argv element,
  passed directly to `execve`'s argv array, never shell-joined into a single
  string. This is the second half of the shell-injection closure: even a
  malicious `args` element cannot break out of its own argv slot.
- `cwd: Option<String>` — workspace-relative, resolved via the same
  `RESOLVE_BENEATH`/`RESOLVE_NO_SYMLINKS` discipline as every other
  workspace-scoped path in this codebase (§3).
- No environment-variable passthrough by default — mirrors the existing
  precedent that `OPENAI_API_KEY` is forwarded to the planner sidecar ONLY,
  never the worker (`main.rs:309-310,321-324`).

### 1.6 (Dis)allow posture — no command allowlist for v1.7

**Option A (no allowlist — confinement is the sole control) is pinned as the
v1.7 default**, over Option B (a hardcoded per-command allowlist mirroring
`sink_sensitivity.rs`'s discipline). This matches the milestone's stated
scope — "the two effect primitives a coding agent minimally needs"
(`.planning/REQUIREMENTS.md:10-13`) — a command allowlist would need
product-level curation deferred to a later milestone alongside `POL-01`
declarative policy (Future Requirements). This is a deliberate scoping
decision, not an oversight, with an accepted residual: an `Allowed` exec of
e.g. `curl` inside a confined, network-denied child is inert — network
egress is already denied by the reused seccomp rule (§1.4) regardless of
allowlist membership.

### 1.7 Why fd-pass is the wrong template for exec output

The existing broker-mediated pattern for handing the confined side a
resource is fd-pass (`crates/adapter-fs/src/lib.rs:40-52`, `pass_fd`, one
`ControlMessage::ScmRights` per `sendmsg`). `process.exec` output does **not**
use this pattern: the output arrives as `Stdio::piped()` bytes the broker
reads directly from the child's stdout/stderr pipes, not as a file descriptor
handed across the UDS boundary. This is called out explicitly to avoid a
design mistake of trying to fd-pass a pipe when a direct captured-bytes read
is simpler and is what `std::process::Command` + `Stdio::piped()` provides
natively.

---

## §2. Exec-Output Taint Mint (DESIGN-13 / DESIGN-14)

### 2.1 Sole mint site: a new `mint_from_exec` helper

The SOLE mint site for exec-output taint is a new `mint_from_exec` helper
living in the sanctioned `crates/brokerd/src/quarantine.rs` locus — the exact
same file that defines `mint_from_read` (`quarantine.rs:301-420`), the
template this new helper mirrors in shape. `mint_from_read` demonstrates the
non-negotiable pattern this project is built on
(`planning-docs/PLAN.md:186`: "If taint is stapled on at the sink instead of
propagated through the DAG, the demo fails — it proves nothing"):

1. Build a NEW audit `Event` FIRST — for `mint_from_exec`, a new
   `process_exited` event type (mirroring `mint_from_read`'s `file_read`
   event type at `quarantine.rs:361-369`).
2. Append that Event to the audit DAG via `append_event`, obtaining its id
   and row hash (`quarantine.rs:372`).
3. THEN mint the `ValueRecord` via `ValueStore::mint`, with
   `provenance_chain = [that Event's id]` (`quarantine.rs:382-389`).

This ordering — mint happens in the SAME function that records the
originating Event, and `provenance_chain[0]` equals that Event's id — IS the
genuine-non-stapled-taint guarantee (mirrors the
`mint_from_read_anchor_identity` test pattern, `quarantine.rs:856-880`).
**Taint MUST NOT be stapled at the consuming sink** — the same anti-stapling
discipline `mint_from_read` and `mint_from_derivation` already enforce
(T-04-03: the executor never mints, never sets taint — it only reads through
`value_store.resolve()`, `crates/executor/src/lib.rs:8-10`).

### 2.2 Taint label and origin role

A NEW `TaintLabel::ExecRaw` variant is added to the 8-variant enum
(`crates/runtime-core/src/plan_node.rs:13-24`: `UserTrusted`,
`LocalWorkspace`, `ExternalUntrusted`, `EmailRaw`, `PdfRaw`, `LlmGenerated`,
`WorkerExtracted`, `PathRaw`), mirroring the existing `PathRaw`/`EmailRaw`/
`PdfRaw` naming convention. It is always paired with `ExternalUntrusted`
(matching every existing untrusted-origin mint site's 2-label vector pattern,
e.g. `mint_from_read`'s `vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw]`
at `quarantine.rs:334`): exec output mints as
`vec![TaintLabel::ExternalUntrusted, TaintLabel::ExecRaw]`.

Adding the variant is a **compile-time-enforced** change: `is_untrusted()`'s
exhaustive `match self` with no wildcard arm
(`crates/runtime-core/src/plan_node.rs:40-50`, doc-commented "Adding a new
`TaintLabel` variant without updating this match is a compile error, not a
silent false-allow") FORCES Phase 32 to place `ExecRaw` in the untrusted arm
(alongside `ExternalUntrusted`/`EmailRaw`/`PdfRaw`/`LlmGenerated`/
`WorkerExtracted`/`PathRaw`) — the compiler catches an omission that a
runtime default could silently miss.

`origin_role = Some("exec_output".to_string())` — a new role string, keyed
by the mint site the same way every other untrusted-origin `claim_type`
becomes its `origin_role` verbatim
(`planning-docs/DESIGN-slot-type-binding.md` §2's dual-vocabulary
convention).

### 2.3 Fail-closed unknown-classification discipline

`mint_from_exec` mirrors `mint_from_read`'s fail-closed unknown-`claim_type`
discipline (T-07-47, `quarantine.rs:324,354-358`: "only recognized
[...] types get a taint set — no default-tagging of an unrecognized case").
Concretely: exec output has exactly ONE recognized shape (combined
stdout+stderr bytes from a `process_exited` child) — there is no branching
classification to get wrong, but the discipline this doc pins is that any
FUTURE variant of exec-output classification (e.g. distinguishing stdout
from stderr, or a structured-vs-raw distinction) must follow the same
`other => Err(...)` fail-closed shape `mint_from_read` uses at
`quarantine.rs:354-358` — never a default/fallback taint assignment.

### 2.4 Mandated `check-invariants.sh` Gate 3 extension

`scripts/check-invariants.sh` Gate 3 (lines 50-141) TODAY restricts exactly
three call-site tokens — `mint_from_read(`, `mint_from_derivation(`, `.mint(`
— to the sanctioned loci `crates/brokerd/src/quarantine.rs`,
`crates/brokerd/src/server.rs`, and (for `.mint(` only)
`crates/executor/src/value_store.rs` (`check-invariants.sh:133-135`). **A new
`mint_from_exec(` call site will NOT be caught by Gate 3 as written today** —
this is a concrete, actionable gap, not a general reminder. This doc
**mandates** that Phase 32 extend Gate 3 with a fourth `check_mint_token`
call for the literal `mint_from_exec(` token, restricted to the SAME
sanctioned loci (`quarantine.rs`, `server.rs`), in the SAME commit that
introduces `mint_from_exec`. Without this extension, the new mint site's
call-site restriction is silently unenforced — a fresh adversarial reviewer
must specifically confirm this extension exists before clearing the gate
(§6, §7 of the RESEARCH Gate-Record Shape).

### 2.5 Named forward residual: async-signal-safety inside `pre_exec`

`landlock::Ruleset::create()`/`restrict_self()` and
`seccompiler::apply_filter()` (§1.4's confinement primitives, now invoked
from INSIDE a `pre_exec` closure for the first time in this codebase) likely
allocate heap memory internally (both build `Vec`-backed structures) between
`fork()` and `execve()`. Rust's own `pre_exec` documentation requires
closures to call only async-signal-safe functions in that window; allocator
state inherited from a multi-threaded parent process can be inconsistent in
the child under rare scheduling. This is a widely-accepted soft violation in
the Rust sandboxing ecosystem, not exercised anywhere in THIS codebase
before (the worker's self-confinement runs long after its own fork, never
inside a `pre_exec` closure). This doc names it as an explicit, accepted
residual risk rather than silently ignoring it — resolved fully in §9, with
Option B (§1.3) as the documented fallback if the fresh adversarial reviewer
rules it a blocker.

<!-- gsd:write-continue -->
