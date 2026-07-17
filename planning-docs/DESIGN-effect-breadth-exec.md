# DESIGN — Effect Breadth I: `process.exec` + Filesystem Read/Write Breadth

**Milestone:** v1.7 — Effect Breadth I
**Phase:** 31 (Design Gate) — blocks all `crates/executor` / `crates/brokerd` /
`crates/sandbox` / `crates/runtime-core` code for this milestone
**Status:** ✅ CLEARED (Round 1, 2026-07-17) — cleared a fresh non-self
adversarial code-trace with all findings resolved; see
`DESIGN-GATE-RECORD-v1.7.md` (Plan 31-02). Amendments folded in §11.
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

### 1.3 Spawn ownership — Option B (recommended) vs Option A (not recommended)

- **Option B (RECOMMENDED, fail-closed default — pinned Round 1):** a
  dedicated `caprun-exec-launcher` helper binary, spawned unconfined
  (mirroring `caprun-worker`'s separate-binary pattern), which receives the
  target command over the same kind of env-var/UDS channel already used for
  the worker and performs its OWN post-fork self-confinement (the SAME proven
  ordering as `apply_confinement()`, `crates/sandbox/src/lib.rs:7-18`) before
  its own `execve`. Confinement runs in the launcher's own address space,
  long after its own fork, exactly as the worker's `apply_confinement()` does
  today — so it does NOT require any code to be async-signal-safe inside a
  `pre_exec` closure. Cost: one extra binary + an IPC round-trip.
- **Option A (documented alternative — NOT recommended):** `brokerd`'s own
  dispatch handler (inside a `tokio::spawn`'d per-connection task,
  `crates/brokerd/src/server.rs:271,308`) calls
  `std::process::Command::new(cmd).pre_exec(|| { ... }).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`
  inline, applying the confinement primitives from inside the `pre_exec`
  closure. This is lighter (no extra binary) but requires the `pre_exec`
  closure — running between `fork()` and `execve()` in a process that was
  multi-threaded at fork — to invoke `landlock`/`seccompiler` setup that
  allocates heap memory, which is NOT async-signal-safe (§2.5, §9). No real
  `.pre_exec(` call site exists anywhere in this codebase today (verified by
  full-tree grep during the Round-1 review); the two existing
  `Command::spawn()` sites (`main.rs:328,356`) are the SAFE shape with **no**
  `pre_exec` closure, so they provide no evidence for the dangerous shape.
- **Ruling:** pin **Option B** as the v1.7 default. Rationale: the exec child
  is the riskiest primitive in the project to date, and Option B confines it
  using the SAME post-fork self-confinement ordering already proven for the
  worker (`apply_confinement()`), with zero reliance on async-signal-safety
  in a `pre_exec` window. Option A's fork-in-a-multi-threaded-tokio-task
  `pre_exec` path is a real allocator-deadlock/async-signal-safety hazard
  (§2.5) with no incident-free precedent in this codebase; it is retained
  above only as a documented, explicitly-not-recommended alternative. This
  ruling is the Round-1 resolution of review finding **M3** (see Amendments).

### 1.4 Kernel confinement of the exec child

**Landlock — a NEW narrow-allow-list ruleset, NOT `deny_all_filesystem()`
reused verbatim.** `deny_all_filesystem()`
(`crates/sandbox/src/landlock.rs:16-32`) calls
`Ruleset::default().handle_access(AccessFs::from_all(ABI::V3)).create()...restrict_self()`
with **zero allow-rules added** — everything, including the `Execute` access
right, is denied. That ruleset was designed for the WORKER, which
self-confines AFTER its own binary has already loaded and never itself
`execve`s again. The exec child is different: its ruleset (applied by the
launcher post-fork under Option B, BEFORE the launcher `execve`s the target)
must still permit loading and executing the target binary. Reusing
`deny_all_filesystem()` verbatim would make the target binary itself fail to
load (`EACCES`/`ENOEXEC` on the launcher's `execve` of the target). This doc
pins a **distinct,
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
but does **NOT** add an execve deny — under Option B the launcher applies this
filter in its own address space and then performs its ONE legitimate `execve`
of the target, so the filter must permit `execve`. The filter persists across
that `execve` per standard Linux seccomp-BPF inheritance semantics (an
assumption, not re-verified against kernel source this session — §9
Assumption A5).

**Recursion (grandchild `execve`) — NOT denied by the child's own seccomp
filter; bounded by Landlock + persistent net-deny instead.** The Round-1
review (finding **B1**) established that this cannot be done the way an
earlier draft pinned it: `caprun`'s seccomp filters are **stateless**
`seccompiler::SeccompFilter` BPF programs (`seccomp.rs:62`, unconditional
`(SYS_execve, vec![])` always-match), and a stateless BPF program has no
allow-the-first-execve-then-deny-subsequent construct. A filter that denies
`execve` would kill the child's OWN initial `execve` (the target never
loads, `spawn()` fails); a filter that allows `execve` lets grandchildren
`execve` freely. There is no middle state. This doc therefore does NOT claim
a seccomp recursion-deny. The real, verifiable bounds on what a grandchild
can do are: (1) the NEW narrow-allow Landlock ruleset grants `Execute` ONLY
on the enumerated system binary/library paths, so a grandchild can only
`execve` those same already-enumerated binaries — no arbitrary binary; (2)
the reused `socket(AF_INET/AF_INET6)` deny **persists across `execve`**, so
the stated worry — an unaudited grandchild making network calls — is
independently closed regardless of recursion; (3) inherited rlimits and the
broker-side wall-clock timeout bound the whole process tree. "Run a shell
script" remains explicitly out of scope for v1.7 (a v1.8+ decision alongside
`git`/`http.request`, per REQUIREMENTS.md Future Requirements); a hardcoded
command allowlist (§1.6 Option) would further bound this if adopted later.

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

**The no-allowlist posture (confinement is the sole control) is pinned as the
v1.7 default**, over a hardcoded per-command allowlist mirroring
`sink_sensitivity.rs`'s discipline. (This allow/deny-posture decision is
independent of §1.3's spawn-ownership choice — do not conflate the two.) This
matches the milestone's stated
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

**Mint call-site locus is pinned to `server.rs`, NOT the exec sink module
(Round-1 finding M1).** Exec-output taint is minted when the broker captures
the exited child's piped stdout/stderr — this is the exec analog of the
`ReportClaims` capture point where the live `mint_from_read` production call
already lives (`crates/brokerd/src/server.rs`), and it MUST live in
`server.rs` for the same reason. It must NOT live in the `process.exec` sink
module (e.g. a new `crates/brokerd/src/sinks/process_exec.rs`, which §3.3
otherwise cites as the two-phase-audit template): that module is not in the
Gate-3 sanctioned-loci allow-list, so a mint call there would FAIL the very
Gate-3 extension this section mandates. The division is: the sink module owns
spawn + confinement handoff + the two-phase `process_exited`/
`process_spawn_failed` audit; `server.rs` owns the `mint_from_exec` of the
captured output. This keeps the mandated Gate-3 allow-list and the code
structure in agreement.

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
inside a `pre_exec` closure). This hazard is precisely WHY §1.3 pins
**Option B** (launcher post-fork self-confinement) as the v1.7 default
rather than Option A: under Option B no confinement code runs inside a
`pre_exec` window, so this async-signal-safety concern does not arise on the
pinned path at all. It is documented here (and in §9) as the reason Option A
is not recommended, not as an accepted residual of the pinned design.

---

## §3. Filesystem Read/Write Breadth Model (DESIGN-13)

### 3.1 Multi-file read (FS-01)

The existing read path — `WorkspaceRoot::read_within`
(`crates/adapter-fs/src/workspace.rs:75-102`), a single
`openat2(O_RDONLY, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)` syscall, dispatched
from `server.rs`'s `RequestFd` arm (`crates/brokerd/src/server.rs:1229-1394`)
and taint-minted via `mint_from_read` — already establishes the
single-syscall, TOCTOU-safe resolution pattern this doc reuses unmodified.
FS-01 ("read multiple workspace files") is pinned as invoking this EXACT
existing path **N times**, once per file the worker requests, each
independently taint-minted as untrusted exactly like today's single read —
**no new mechanism**, only a documented multiplicity.

**Open item, confirmed by direct code read (RESEARCH Open Question 1,
resolved here):** a full read of the `RequestFd` handler
(`server.rs:1229-1394`) found no per-session counter or explicit limiter on
repeat `RequestFd` calls — contrast `ProvideIntent`, which explicitly
documents an "ONCE and ONLY BEFORE any RequestFd" constraint enforced via the
`fd_requested`/intent-accepted booleans (`server.rs:1194,1626-1639`); no
equivalent language or guard exists for `RequestFd` itself. This means
multi-file read is likely **already mechanically supported** by calling the
existing single-file path repeatedly — but "unlimited repeat calls" was
never a deliberate prior decision, only unexercised. **Fail-closed default
pinned here:** Phase 33 MUST add an explicit per-session upper-bound counter
on `RequestFd` invocations (exact numeric value is a Phase 33 implementation
detail, not re-litigated here) — a resource-exhaustion guard, not a
functional gate. Absent this counter, FS-01 would ship as "unlimited by
accident," which this doc explicitly refuses to bless as the default.

### 3.2 Write/edit an existing file (FS-02)

Pinned as a straightforward sibling of `create_exclusive_within`
(`crates/adapter-fs/src/workspace.rs:132-151`), with a **different `OFlag`
set**: `O_WRONLY | O_TRUNC` — explicitly **NO `O_CREAT`, NO `O_EXCL`**. A
missing target path fails closed with `ENOENT` (never silently creates the
file) — this is the semantic split from `file.create`: `file.create` is
new-file-only (`O_CREAT|O_EXCL`, `EEXIST` on an existing path,
`workspace.rs:117-120`), the new write/edit sink is existing-file-only
(`ENOENT` on a missing path). Same `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`
single-syscall, TOCTOU-safe kernel resolution (`workspace.rs:78-83`) — same
absolute-path rejection, `..` rejection, and symlink rejection at kernel
resolution time as the existing read/create paths.

**Equivalent negative tests are NOT assumed inherited.** The existing
negative tests (`workspace.rs:220-401`: `absolute_path_rejected`,
`parent_traversal_rejected`, `symlink_escape_rejected`, and the
`create_exclusive_*` siblings) prove this behavior for the `O_RDONLY` and
`O_CREAT|O_EXCL|O_WRONLY` flag combinations specifically — NOT for
`O_WRONLY|O_TRUNC`. Phase 33 MUST write the equivalent set (absolute-path,
`..`-traversal, symlink-escape, PLUS an `ENOENT`-on-missing-target test) for
the new flag combination, not assume coverage carries over.

**Explicit warning against scope-blurring.** The write/edit sink MUST NOT
reintroduce `O_CREAT` (with or without `O_EXCL`) — doing so would blur its
new-file-permitting behavior with `file.create`'s new-file-ONLY semantics,
creating two sinks with overlapping "can create a file" authority and
confusing the schema/sensitivity table (§4) about which sink is the
create-authority. `O_WRONLY|O_TRUNC`, no `O_CREAT`, is the pinned, exclusive
shape of the write/edit sink.

### 3.3 Two-phase durable audit

Both the multi-file read and the write/edit sink follow the same two-phase
durable-audit pattern `invoke_file_create` already establishes
(`crates/brokerd/src/sinks/file_create.rs:65-116`): on success, append a
`sink_executed` event; on error, append a `sink_execution_failed` event
FIRST, then propagate the error (no automatic retry — a mid-effect failure
leaves an explicit, durable trace). The `actor` field convention
(`format!("sink:{sink_id}:{effect_id}")`, `file_create.rs:90,107`) is
reused verbatim for the new write/edit sink (e.g.
`sink:file.write:<effect_id>` — exact sink id TBD Phase 33, this doc does
not pin the literal string beyond noting the convention). Each event is
chained onto `parent_id`/`parent_hash` exactly like `file_create.rs`'s
pattern, keeping `verify_chain` intact.

---

## §4. I2 + Slot-Type Binding for the New Sinks (DESIGN-14)

### 4.1 Both sinks are table entries only — no new I2 logic

Both `process.exec` and the fs write/edit sink are `PlanNode { sink, args }`
from spawn — exactly like `file.create` — and route through the
**UNMODIFIED** `submit_plan_node` collect-then-Block loop
(`crates/executor/src/lib.rs:54-255`). The ONLY changes required are table
entries: a new `KNOWN_SINKS` schema entry (`crates/executor/src/sink_schema.rs:40-58`),
a `sink_effect_class` arm (`crates/executor/src/sink_sensitivity.rs:40-57`),
`is_routing_sensitive`/`is_content_sensitive` membership
(`sink_sensitivity.rs:87-115`), and `expected_role` entries
(`sink_sensitivity.rs:155-181`). **No new enforcement logic, no new
`ExecutorDecision` variant, no new step in `submit_plan_node`'s ordering.**
This is the same discipline `DESIGN-slot-type-binding.md` established for
v1.5's slot-type-binding extension (table entries only), applied to two new
sinks instead of a new mechanism.

### 4.2 The single highest-consequence decision: `process.exec` command/args are sensitivity-classified

**`process.exec`'s own `command` AND `args` are classified BOTH
routing-sensitive AND content-sensitive, so a tainted value in either Blocks.**
A tainted `command` is not a data-exfiltration risk (contrast `email.send`'s
deliberately-scoped-out `attachment`, which was descoped for v1.3 and removed
from both the sink schema and the content-sensitive set atomically,
`sink_sensitivity.rs:73-78`) — it is **arbitrary code execution**, strictly
worse than a tainted email recipient. A tainted `command`/`args` value MUST
Block under the existing collect-then-Block loop (`lib.rs:150-197`), never
fall through unblocked. Concretely: `is_routing_sensitive(process.exec,
"command")`, `is_routing_sensitive(process.exec, "args")`,
`is_content_sensitive(process.exec, "command")`, and
`is_content_sensitive(process.exec, "args")` are all pinned `true` (the
routing/content distinction is academic here — the point is neither
classification function returns `false` for these two args). `cwd` is
routing-sensitive (it determines WHERE the command's relative-path resolution
happens) but not content-sensitive.

**Where the Block actually comes from — and why `command`/`args` carry
`expected_role = None` (Round-1 finding M2).** The Block is delivered by the
Step 2/3 sensitivity+taint check (`lib.rs:156-158`: `sensitive &&
record.taint.iter().any(is_untrusted)`), which is **independent of**
`expected_role`. `expected_role` governs a SEPARATE, earlier structural
role-Deny at Step 1c (`lib.rs:133-148`): a slot with `expected_role =
Some(list)` Denies any value whose `origin_role` is not in `list`, and a
`None` origin_role at such a slot fails closed. There is no `origin_role`-
producing mint site for a legitimately-authored exec `command` (it originates
as a trusted intent literal, carrying `origin_role = None` or an intent
role), so pinning `expected_role = Some(...)` for `command`/`args` would
fail-closed-Deny the LEGITIMATE command at Step 1c — breaking the feature,
not tightening it (the same trap HARDEN-05 navigated for `file.create`
`contents` by reusing the `"path"` role, `sink_sensitivity.rs:163-176`).
Therefore `command`/`args` are pinned `expected_role = None` (not role-checked
at Step 1c); the security property — a tainted `command`/`arg` Blocks — is
fully delivered by their `is_routing_sensitive`/`is_content_sensitive = true`
classification plus the untrusted-taint check, exactly as intended. `None`
here is NOT an I2 bypass: it only disables the structural role gate, never
the sensitivity+taint Block.

### 4.3 fs write/edit slot roles

- **`path`** — routing-sensitive, mirroring `file.create`'s existing
  `FILE_CREATE_ROUTING_SENSITIVE` entry (`sink_sensitivity.rs:63-66`), with
  `expected_role` mirroring `file.create`'s existing `path` role check
  verbatim: `Some(&["path", "relative_path"])` (cited exactly from the live
  entry at `sink_sensitivity.rs:163-164`, not re-derived).
- **`contents`** — content-sensitive, mirroring HARDEN-05's extension of
  `file.create`'s `contents` to content-sensitive
  (`sink_sensitivity.rs:80-85`, `FILE_CREATE_CONTENT_SENSITIVE`). Its
  `expected_role` list accepts BOTH a trusted-authored role (mirroring
  `file.create`'s HARDEN-05 reuse of the `"path"` role at
  `sink_sensitivity.rs:176`, since no dedicated `"contents"`/`"file_body"`
  role-producing mint site exists) AND the untrusted `"exec_output"`/
  `"doc_fragment"` roles — so a tainted exec-output ValueNode (§2, tagged
  `origin_role = Some("exec_output")`) routed into the write/edit sink's
  `contents` slot is role-admissible and therefore reaches I2's per-arg
  sensitivity check, where its `ExecRaw`/`ExternalUntrusted` taint Blocks it
  — exactly the same shape as `email.send`'s `body` slot already accepting
  `"doc_fragment"` (`sink_sensitivity.rs:138-154`) so a tainted
  worker-extracted body Blocks rather than fail-closed-Denying at the
  structural Step 1c role check before ever reaching I2.

### 4.4 No I2 bypass; no new raw request-args-to-sink path

Both sinks stay on the `PlanNode{sink, args}` path from day one — **no new
raw `EffectRequest { effect, args: Map }` path is introduced or possible**
(`crates/runtime-core/src/plan_node.rs`'s `DEC-architectural-lock-plan-nodes`
comment, lines 1-9). `check-invariants.sh` Gate 1 (the `EffectRequest` token
absence check, `check-invariants.sh:24-38`) stays green with zero new hits —
this doc introduces no such token anywhere. Both sinks are
`CommitIrreversible` (`sink_effect_class`, mirroring both live sinks today,
`sink_sensitivity.rs:42-43`), so a Draft-status session cannot invoke either
without the existing I0 class-deny firing (`lib.rs:205-252`, unchanged by
this doc).

---

## §5. Fail-Closed Defaults Table (DESIGN-14)

| Sink arg | Sensitivity | Default posture | Fail-closed behavior |
|---|---|---|---|
| `process.exec` `command` | routing- AND content-sensitive | argv-only (never `sh -c`); no command allowlist v1.7 | tainted → Block (collect-then-Block); unknown/missing → Deny at Step 0 schema gate |
| `process.exec` `args` | routing- AND content-sensitive | `Vec<String>`, each a direct `execve` argv element | tainted → Block; same schema-gate Deny on malformed shape |
| `process.exec` `cwd` | routing-sensitive | workspace-relative, `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS` | tainted → Block; escape attempt → kernel-level Deny (`EXDEV`) before I2 is even reached |
| exec-output `ValueNode` (post-spawn) | untrusted origin | `TaintLabel::ExecRaw` + `ExternalUntrusted`; `origin_role = Some("exec_output")` | unknown/unrecognized exec-output shape → fail-closed mint error (mirrors T-07-47), never default-tagged |
| exec child kernel confinement | n/a (infrastructure) | narrow-allow Landlock (`Execute` only on enumerated system paths) + seccomp net-deny (persists across `execve`) + reused rlimits + NEW wall-clock timeout + output byte cap. NO seccomp recursion-exec-deny (unrealizable with a stateless BPF — B1); grandchild `execve` is bounded by the Landlock `Execute` allow-list + persistent net-deny instead | any confinement primitive failing to apply → the launcher (Option B) aborts before `execve`ing the target and exits non-zero, no target ever runs |
| fs write/edit `path` | routing-sensitive | `expected_role = Some(&["path","relative_path"])`; `O_WRONLY\|O_TRUNC`, `RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS` | tainted → Block; missing target → `ENOENT` Deny (never silently creates); escape → kernel-level Deny |
| fs write/edit `contents` | content-sensitive | `expected_role` accepts trusted-authored role AND `"exec_output"`/`"doc_fragment"` | tainted/exec-output-tagged in slot → Block, same as email `body` precedent |
| fs read (multi-file) `path` | routing-sensitive (mirrors today's single-file read) | `RESOLVE_BENEATH\|RESOLVE_NO_SYMLINKS`; NEW explicit per-session read-count upper bound | escape → kernel-level Deny; count exceeded → Deny (resource-exhaustion guard, §3.1) |
| unregistered sink, or unknown/duplicate/missing arg (either new sink) | n/a (structural) | `KNOWN_SINKS` exact-match schema, Step 0 gate | Deny at Step 0, before any resolve/sensitivity/role check ever runs |

---

## §6. Security-Invariant Checklist

Mirrors `DESIGN-slot-type-binding.md`'s convention: each item is checked with
a one-line justification, not asserted bare.

- [x] **I0 unaffected** — neither new sink changes session-creation semantics;
  no new session-creation path exists anywhere in this doc's model.
- [x] **I1 unaffected** — the worker never holds raw exec-output bytes, only
  the opaque `ValueId` handle returned from `mint_from_exec` (§2.1), mirroring
  the existing read-extraction handle model
  (`DESIGN-taint-model.md`'s handle discipline). The worker never sees exec
  stdout/stderr directly — only the broker, which reads the piped bytes and
  immediately mints them.
- [x] **I2 — both new sinks route through the UNMODIFIED
  `submit_plan_node` collect-then-Block loop** — §4.1: table entries only
  (`KNOWN_SINKS`, `sink_effect_class`, `is_routing_sensitive`/
  `is_content_sensitive`, `expected_role`), zero new enforcement logic.
- [x] **No I2 bypass** — §4.2: `process.exec`'s own `command`/`args` are
  sensitivity-classified (routing- AND content-sensitive), so a tainted value
  in either Blocks at the Step 2/3 sensitivity+taint check (`lib.rs:156-158`),
  independent of `expected_role`. `command`/`args` carry `expected_role =
  None` (no `origin_role`-producing mint site exists; `Some(...)` would
  fail-closed-Deny the legitimate command at Step 1c — Round-1 M2), which
  disables only the structural role gate, never the sensitivity+taint Block.
- [x] **No raw `EffectRequest` path** — §4.4: both sinks are
  `PlanNode { sink, args }` from spawn; `check-invariants.sh` Gate 1 stays
  green with zero new hits (no `EffectRequest` token anywhere in this doc's
  model).
- [x] **Genuine, non-stapled taint chain** — §2.1: exec-output taint is set
  ONLY inside `mint_from_exec`, at the same call that appends the
  `process_exited` Event; `provenance_chain[0]` MUST equal that Event's id
  (mirrors `mint_from_read_anchor_identity`, `quarantine.rs:856-880`).
- [x] **Mint-call-site restriction (Gate 3) MUST be extended** — §2.4:
  `check-invariants.sh` Gate 3 today greps only for `mint_from_read(`,
  `mint_from_derivation(`, `.mint(` (`check-invariants.sh:133-135`) and will
  NOT catch a new `mint_from_exec(` call site as written. This doc mandates
  the extension as part of Phase 32, in the same commit that adds
  `mint_from_exec`.
- [x] **Kernel-confined exec child** — §1.4: applied by the launcher
  post-fork (Option B, §1.3) — a NEW narrow-allow Landlock ruleset (not
  `deny_all_filesystem()` verbatim; grants `Execute` only on enumerated
  system paths) + reused seccomp network-deny (no execve-deny, since the
  launcher must perform its one legitimate `execve`; net-deny persists across
  it) + reused rlimits (`RLIMIT_AS`/`RLIMIT_CPU`) + a NEW wall-clock timeout
  (`tokio::time::timeout` + existing `child.kill()` path) + a captured-output
  byte cap. Grandchild `execve` is NOT seccomp-denied (unrealizable with a
  stateless BPF — Round-1 B1); it is bounded by the Landlock `Execute`
  allow-list (only the enumerated binaries) + the persistent net-deny.
- [x] **Fail-closed arg-schema** — §4.1, §5: both new sinks get `KNOWN_SINKS`
  entries with explicit `allowed`/`required` sets, mirroring `file.create`'s
  exact-match schema (`sink_schema.rs:40-58`); an unregistered sink or
  unknown/duplicate/missing arg Denies at Step 0, before any resolve or
  sensitivity check.
- [x] **Durable audit** — §3.3: both sinks use the two-phase
  `sink_executed`/`sink_execution_failed` pattern (or an exec-specific pair,
  e.g. `process_exited`/`process_spawn_failed`), chained onto
  `parent_id`/`parent_hash` exactly like `invoke_file_create`
  (`file_create.rs:82-113`).

---

## §7. Validation Architecture Pointer

Full detail lives in `31-RESEARCH.md` §Validation Architecture — this
section names the forward test shape that makes Phases 32-34 buildable
without restating it in full:

- **Per-requirement named test targets** — EXEC-01..04 get dedicated
  spawn/confine/taint/I2/audit coverage (provisionally
  `crates/brokerd/tests/process_exec_*.rs`, name TBD Phase 32); FS-01..03 get
  fs read-breadth + write/edit-under-I2 coverage (provisionally
  `crates/brokerd/tests/file_write_*.rs`, name TBD Phase 33) — mirroring this
  project's existing per-requirement test-map discipline (RESEARCH §Phase
  Requirements → Test Map).
- **A dedicated negative test per new sink** — required by LIVE-02
  (`.planning/REQUIREMENTS.md:75-77`), not optional coverage.
- **Exec-child confinement negative-assertion test** — mirroring
  `crates/sandbox/tests/confinement_integration.rs`'s existing pattern (the
  child cannot read `~/.ssh`, cannot reach network, cannot exec beyond its
  one legitimate `execve`, `PLAN.md:152`), a new
  `crates/sandbox/tests/exec_child_confinement.rs` (Wave 0 gap, Phase 32).
- **Live Linux composed-acceptance shape** — `scripts/mailpit-verify.sh` (or
  an exec-scoped equivalent per LIVE-01's wording), asserted on counts +
  named tests, true-exit-before-pipe (never bare `script | tail` exit-code
  laundering — the project's own standing incident,
  `[[verification-exit-code-through-pipe]]`).

---

## §8. Open Items (model pinned, deployment constants deferred)

These are explicitly flagged OPEN — **deployment constants, not model
gaps**. The design is complete without resolving them; Phase 32/33 resolves
each against the actual verification environment.

1. **Exact Landlock allow-list path strings for the `rust:1` verification
   container.** §1.4 pins the METHOD (narrowest hardcoded allow-list scoped
   to the chosen test commands' actual dependency paths); the literal path
   strings are resolved at Phase 32 against the container's real filesystem
   layout (`ldd`/`which` output for the candidate test commands).
2. **The `RequestFd` per-session read-count limiter's exact numeric bound.**
   §3.1 pins that an explicit upper bound MUST exist; the specific value is
   a Phase 33 implementation detail.
3. **Confirming the verification container's kernel version floor** —
   Landlock ABI negotiation requires kernel ≥5.13 (ABI::V1) with ABI::V3
   preferred ≥5.19; `openat2`'s `RESOLVE_*` flags require kernel ≥5.6. Both
   are training-knowledge claims not re-verified against the actual `rust:1`
   image's kernel this session (RESEARCH Assumption A3) — confirm before
   Phase 32/33 land.
4. **Captured-output byte-cap exact value** — §1.4 pins that a cap MUST
   exist and fail closed (deny/truncate, never fail-open); the specific
   byte count is a Phase 32 implementation detail.

---

## §9. Accepted Residual Risks & Assumptions

**Design decisions that RETIRE a residual (v1.7):**
- **`pre_exec` async-signal-safety — AVOIDED, not accepted (Round-1 M3/§1.3,
  §2.5).** An earlier draft pinned Option A (confinement applied inside a
  `pre_exec` closure), for which `landlock`/`seccompiler` allocating heap
  between `fork()` and `execve()` would be a real async-signal-safety soft
  violation. The Round-1 review found the "runs twice without incident"
  justification for Option A did not transfer (no real `.pre_exec(` site
  exists in this codebase — the two `Command::spawn()` sites are the safe,
  no-`pre_exec` shape). Resolution: **Option B is now the pinned default** —
  the launcher self-confines post-fork in its own address space (the proven
  `apply_confinement()` ordering), so NO confinement code runs in a
  `pre_exec` window and this hazard does not arise on the pinned path.
  Option A remains documented only as a not-recommended alternative.

**Accepted residual risks (v1.7):**
- **No `process.exec` command allowlist for v1.7** (§1.6) — confinement
  (Landlock/seccomp/rlimits/network-deny) is the sole control on WHAT a
  command can do once spawned; there is no curated list of WHICH commands
  may be spawned. Deliberately scoped, deferred alongside `POL-01`
  declarative policy.
- **Seccomp-filter persists across the child's own execve** (§1.4) —
  standard Linux seccomp-BPF inheritance semantics, general kernel
  knowledge not re-verified against kernel source this session.

**Assumptions (carried from RESEARCH.md, to confirm during Phase 32, not
silently):**
- **A1** — `std::process::Command::spawn()`'s internal fork+exec is safe to
  call from within a `tokio::spawn`'d async task without a dedicated OS
  thread, given this codebase already does so twice unconfined
  (`main.rs:328,356`). Under the pinned Option B the broker's spawn of the
  `caprun-exec-launcher` IS exactly this safe, unconfined shape (no
  `pre_exec` closure), so A1 is on well-trodden ground; confinement happens
  later, inside the launcher.
- **A2** — `landlock::Ruleset::create()`/`restrict_self()` and
  `seccompiler::apply_filter()` allocate heap memory internally. This mattered
  only for the retired Option A (calling them inside `pre_exec`); under the
  pinned Option B they run in the launcher's own address space AFTER its fork
  (the same context the worker's `apply_confinement()` already uses), where
  heap allocation is normal and safe. Retained as an assumption only to
  document why Option A is not recommended.
- **A3** — Landlock ABI negotiation (ABI::V3 down to ABI::V1, kernel ≥5.13)
  and `openat2` `RESOLVE_*` flags (kernel ≥5.6) version floors are training
  knowledge, not re-verified against kernel source this session (§8 item
  3).
- **A4** — no per-session `RequestFd` read-count limiter exists today,
  confirmed by a direct read of the full handler (§3.1) — not merely a grep
  miss.
- **A5** — the seccomp filter the launcher installs on itself (Option B)
  persists across its own subsequent `execve` of the target, and across any
  grandchild `execve`, per standard Linux kernel seccomp-BPF inheritance
  semantics; if wrong, the network-deny protection on the exec child would
  not actually apply post-exec (§1.4). This is the single load-bearing
  kernel-semantics assumption behind the B1 resolution (grandchild egress is
  bounded by the persistent net-deny) and MUST be confirmed in Phase 32.

**Common pitfalls (for Phase 32/33 implementers, carried from RESEARCH.md
Landmines, condensed):**
- Assuming `deny_all_filesystem()` can be reused verbatim for the exec
  child — it has zero allow-rules and blocks the target binary from
  loading (§1.4).
- A new mint helper escaping Gate 3's call-site restriction by living
  outside `quarantine.rs`/`server.rs` (§2.4) — extend Gate 3 in the SAME
  commit.
- Confusing `"exec.shell"` — a `validate_schema` `UnknownSink` test fixture
  at `crates/executor/src/sink_schema.rs:193-198`, asserting that string is
  CURRENTLY REJECTED — with prior art for the real sink id. The real
  `process.exec` sink id (per `.planning/REQUIREMENTS.md` EXEC-01) is
  `process.exec`; `"exec.shell"` remains a distinct, permanently-rejected
  test fixture string with no accidental collision.
- Reintroducing an `O_CREAT` overwrite path on the fs write/edit sink,
  blurring it with `file.create`'s new-file-only semantics (§3.2).

---

## §10. Acceptance Predicate — Done When

Phase 31's gate is cleared when ALL are true:

1. This doc pins the broker-spawned confined-child `process.exec` model —
   spawn ownership (§1.3), kernel confinement (§1.4), arg schema (§1.5), and
   (dis)allow posture (§1.6) — AND the exec-output taint mint (§2), AND the
   filesystem read/write-breadth model (§3). **(DESIGN-13, this plan.)**
2. This doc pins the fail-closed defaults for BOTH new sinks — exec
   command/arg schema + (dis)allow posture, exec-output taint label +
   `origin_role`, fs read/write path & slot constraints (§4, §5) —
   consistent with I0/I1/I2 and v1.5 slot-type binding; nothing disables or
   bypasses I2 and no new raw request-args-to-sink path is introduced.
   **(DESIGN-14, this plan.)**
3. `process.exec`'s own command/args are classified routing- AND
   content-sensitive (§4.2) — the single highest-consequence design
   decision in this doc — and the mandated `check-invariants.sh` Gate 3
   extension for `mint_from_exec(` is explicit (§2.4, §6).
4. `scripts/check-invariants.sh` exits 0 against this doc's presence (no
   architectural-invariant regression from its prose).
5. This doc has cleared a fresh, non-self adversarial code-trace review
   (traced against real code, not prose-read) with every finding resolved,
   recorded in `planning-docs/DESIGN-GATE-RECORD-v1.7.md` (Plan 31-02) —
   and no `crates/executor` / `crates/brokerd` / `crates/sandbox` /
   `crates/runtime-core` code exists yet (`git diff` touches only
   `planning-docs/` + `.planning/`).

---

## §11. Amendments (post-review, Round 1)

This section records the changes folded into the doc in response to the fresh,
non-self Fable-5 adversarial code-trace review (see
`planning-docs/DESIGN-GATE-RECORD-v1.7.md`). Every finding was independently
re-verified against live code before folding; each was resolved by TIGHTENING
the design, never by weakening an invariant.

- **B1 (BLOCKER) — seccomp recursion-deny was unrealizable.** An earlier draft
  pinned "the child's own seccomp filter denies `execve` for anything after its
  own initial one." Re-verified against `crates/sandbox/src/seccomp.rs:62` —
  the filters are stateless `seccompiler::SeccompFilter` BPF programs
  (unconditional `(SYS_execve, vec![])` always-match); a stateless BPF has no
  allow-first-then-deny construct. **Resolution:** §1.4 now states there is NO
  seccomp recursion-deny and documents the real bound — Landlock `Execute`
  granted only on enumerated system paths + the reused `socket` net-deny that
  persists across `execve` (so grandchild egress is independently closed) +
  rlimits + wall-clock timeout. §5 table and §6 checklist updated to match.
- **M3 (MAJOR) — Option A pinned on evidence that did not transfer.** The
  "runs twice without incident" justification for Option A (`pre_exec`
  confinement) was re-verified false: a full-tree grep finds zero real
  `.pre_exec(` sites; the two `Command::spawn()` sites (`main.rs:328,356`) are
  the safe, no-`pre_exec` shape. **Resolution:** §1.3 now pins **Option B**
  (dedicated `caprun-exec-launcher` self-confining post-fork, the proven
  `apply_confinement()` ordering) as the v1.7 default; Option A is retained
  only as a documented, not-recommended alternative. This also retires the
  `pre_exec` async-signal-safety residual on the pinned path (§2.5, §9, A2).
- **M1 (MAJOR) — mandated Gate-3 loci contradicted the natural mint site.**
  Gate 3's sanctioned loci are `{quarantine.rs, server.rs}`
  (`check-invariants.sh:133-135`), but a mint in a `sinks/process_exec.rs`
  module (the §3.3 audit template) would fail that gate. **Resolution:** §2.4
  now pins the `mint_from_exec` call-site locus to `server.rs` (the
  exec-output capture point, mirroring the live `mint_from_read` production
  call), explicitly NOT the sink module — so the mandated Gate-3 allow-list and
  the code structure agree.
- **M2 (MAJOR) — `command`/`args` `expected_role` was underspecified.**
  Re-verified against `crates/executor/src/lib.rs:133-158`: `expected_role`
  drives an independent Step-1c structural Deny (`None` origin_role fails
  closed), separate from the Step 2/3 sensitivity+taint Block. Mandating
  `Some(...)` for `command`/`args` (which have no `origin_role`-producing mint
  site) would fail-closed-Deny the legitimate command. **Resolution:** §4.2 and
  §6 now pin `expected_role = None` for `command`/`args` and state the Block is
  delivered by their `is_routing_sensitive`/`is_content_sensitive = true`
  classification + untrusted-taint check — `None` disables only the structural
  role gate, never the sensitivity+taint Block (no I2 bypass).
- **m1 (MINOR)** — §3.1's read-count upper bound modifies the EXISTING
  single-read path, not only new multi-file code; noted so Phase 33 does not
  treat it as additive-only.
- **n1 (NIT)** — adding `TaintLabel::ExecRaw` forces an update to every
  non-wildcard `match` over `TaintLabel` (compiler-caught), not only
  `is_untrusted()`; Phase 32 note.

The reviewer confirmed as sound (real code-trace): worker seccomp
unconditional execve-deny; `deny_all_filesystem()` unusable verbatim; reused
net-deny/rlimits; the v1.4 spawn + `child.kill()` teardown template; the
`mint_from_exec` non-stapled-chain mirror of `mint_from_read`; Gate 3's
three-token restriction; both sinks addable by table entries only; the
`O_WRONLY|O_TRUNC` (no `O_CREAT`) write/edit sibling and its
O_CREAT-reintroduction warning; the two-phase durable audit; and the
no-raw-`EffectRequest` / I0 class-deny invariants.
