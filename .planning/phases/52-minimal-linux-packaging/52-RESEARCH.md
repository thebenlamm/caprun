# Phase 52: Minimal Linux Packaging - Research

**Researched:** 2026-08-11
**Domain:** Shell packaging/install scripting + documentation of an existing Rust workspace's sibling-binary deployment model (no new runtime code, no new crates)
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Installation Experience**
- **D-01:** Provide both a thin `scripts/install-linux.sh` convenience path and equivalent manual release-build/copy commands in the documentation. The documentation remains sufficient when users do not want to execute a repository script.
- **D-02:** The script is a transparent source-tree installer: build the required release targets, copy the resulting sibling binaries into one destination, and report what it installed. It must not download opaque artifacts, invoke privileged package managers, or require root.
- **D-03:** Fail early with actionable messages for unsupported operating systems, missing build prerequisites, failed builds, missing expected binaries, or an unusable destination.

**Destination and Upgrades**
- **D-04:** Default to the user-local executable directory `${HOME}/.local/bin`, while accepting an explicit destination override. System-wide installation is an operator-chosen override, not the default and not performed through implicit `sudo`.
- **D-05:** Keep all required executables in exactly the same destination directory and document PATH setup when that directory is not already discoverable.
- **D-06:** Re-running the installer is the upgrade path. Replacement should be deterministic and avoid leaving a visibly mixed three-binary set if copying fails partway; the planner should choose the simplest shell-safe staging/replacement mechanism consistent with this guarantee.

**Binary Scope and Verification**
- **D-07:** The required installed set is exactly `caprun`, `caprun-worker`, and `caprun-exec-launcher`. `caprun-planner` is optional LLM-sidecar functionality and is not part of the PKG-01 minimal deterministic workflow.
- **D-08:** Explicitly warn that `cargo install --path cli/caprun` is insufficient because it does not install the separate `caprun-exec-launcher` package.
- **D-09:** The install path must perform or document a fail-fast post-install check that all three sibling paths exist and are executable. Keep this check non-destructive: it should not need live credentials, mutate a repository, or claim to re-run the full Linux security proof.
- **D-10:** Build release binaries from the workspace without adding crates, changing runtime sibling resolution, or introducing a packaging framework.

**Configuration Guidance**
- **D-11:** Organize configuration as an operator checklist with three tiers: always needed for the chosen command (workspace/audit and policy inputs), sink-specific credentials/settings, and explicitly internal/test-only variables that design partners should not set.
- **D-12:** Prefer the existing `--policy` CLI path in runnable examples and document `CAPRUN_POLICY` as its fallback. Include a minimal policy-file example or point to a canonical checked-in example if one is created during implementation.
- **D-13:** Document credentials only with placeholder exports and least-scope guidance. Never write real tokens into files, command examples, logs, or planning artifacts. Cover `CAPRUN_GITHUB_TOKEN` and `CAPRUN_GIT_PUSH_TOKEN` for the Safe Coding Agent path, and mention other broker-local sink variables only where applicable.
- **D-14:** Clearly separate operator-facing variables from worker protocol variables and proof/test switches. In particular, do not instruct users to set `BROKER_SOCK`, `SESSION_ID`, `WORKSPACE_FILE`, `CAPRUN_CODING_I2_PROOF`, or `CAPRUN_ENABLE_IPC_CREATE_SESSION` for normal operation.

### Claude's Discretion
- Exact script flag spelling, document placement, headings, and wording are left to the researcher and planner, provided the decisions above and PKG-01 remain intact.
- The planner may choose the smallest reliable post-install check supported by the current CLI rather than adding a new command solely for packaging.

### Deferred Ideas (OUT OF SCOPE)
- cargo-dist, deb, snap, hosted binary releases, checksummed artifact distribution, automatic updater behavior, and macOS support remain PACK-02 or later work.
- Reviewed todos not folded into this phase: GSD executors self-marking phase completion (tooling-process issue), v1.3 Phase 16 v2 security obligations (broader security follow-up), `gsd_run phases.clear` deleting all milestones' phase dirs (GSD tooling issue) — all unrelated to PKG-01.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-------------------|
| PKG-01 | A minimal Linux design-partner install path: documented release build that co-locates `caprun`, `caprun-worker`, and `caprun-exec-launcher` (sibling `current_exe()` layout), plus env/credential checklist (`CAPRUN_*`, policy file, GitHub grant token as applicable). Thin install script acceptable; not cargo-dist/deb/snap productization. `cargo install --path cli/caprun` alone is **not** sufficient (misses exec-launcher). | "Architecture Patterns / Pattern 1" documents the exact two sibling-resolution functions and why co-location in one flat directory is required; "Code Examples" gives the verified CLI usage shape, the verified `SessionPolicy` JSON schema, and a D-09-shaped post-install check; "Common Pitfalls" documents the precise `cargo install --path cli/caprun` insufficiency (Pitfall 1) with its root cause (`caprun-exec-launcher` is a separate Cargo package); "Security Domain" table enumerates every `CAPRUN_*` var's operator-facing vs. internal-only status per D-14 |
</phase_requirements>

## Summary

This phase is documentation-plus-a-thin-shell-script work, not TCB code. Everything the planner needs is already fully determined by code already in the repo: the exact `current_exe()`-relative sibling-resolution algorithm (two independent implementations — a single-hop lookup in `cli/caprun/src/main.rs` for `caprun-worker`, and a bounded 3-level ancestor walk in `crates/brokerd/src/sinks/process_exec.rs` for `caprun-exec-launcher`), the exact `[[bin]]` targets the workspace produces, and the exact `CAPRUN_*` environment variables the code reads. All of these were read directly from source in this session — none of this section is inferred from training knowledge.

The install script and doc updates must produce a directory containing exactly three files — `caprun`, `caprun-worker`, `caprun-exec-launcher` — because `caprun-worker`'s resolution is a **single** `.parent()` hop (no fallback search) and will hard-fail if it is not a direct sibling of `caprun`, while `caprun-exec-launcher`'s resolution additionally tolerates being up to 2 directories higher (accommodating `cargo test`'s `target/{debug,release}/deps/` layout) but is satisfied, and simplest, by being a direct sibling too. `cargo install --path cli/caprun` only installs `caprun` (the crate's sole non-worker `[[bin]]` reachable that way is `caprun-worker`, but `cargo install` only installs the package's **default-run/primary** bin unless `--bin` is repeated per target, and it never reaches into the separate `caprun-exec-launcher` package at all) — this substantiates D-08's warning precisely: `cargo install --path cli/caprun` installs `caprun` and (if `--bin caprun-worker` is also passed) `caprun-worker`, but it can never produce `caprun-exec-launcher`, which lives in a wholly separate Cargo package (`cli/caprun-exec-launcher`).

Both `README.md` and `docs/GETTING-STARTED.md`/`docs/CONFIGURATION.md` are **stale** in ways directly relevant to this phase: they describe only two binaries, an old two-positional-arg CLI usage that predates the `run`/`--policy`/`confirm`/`deny`/`review`/`grant`/`audit` verbs, and a Colima/Mac workflow CLAUDE.md itself now calls stale. `docs/CONFIGURATION.md`'s "Worker Environment Variables" table lists `SESSION_ID`, which is **not read anywhere in the current codebase** (verified by grep — it was apparently removed in an earlier phase without a doc update), and omits every `CAPRUN_*` var that actually exists. This phase's job is exactly to replace these stale sections with what the code actually does — not create a fourth competing doc.

**Primary recommendation:** Write `scripts/install-linux.sh` as a small, `set -euo pipefail` bash script mirroring the house style of `scripts/compose-verify.sh`/`scripts/docker-cache.sh` (header doc-comment with Usage/env-override table, `${VAR:-default}` overrides, explicit `echo`-based PASS/FAIL, non-zero exit on any failure) that runs `cargo build --workspace --release`, copies exactly the three required binaries from `target/release/` into `${INSTALL_DEST:-$HOME/.local/bin}`, verifies the destination afterward with `test -x` on each of the three paths, and prints a PATH hint if the destination isn't already on `$PATH`. Update `docs/GETTING-STARTED.md` and `docs/CONFIGURATION.md` in place (not new files) to reflect the real three-binary layout, real CLI verbs, and the real `CAPRUN_*` surface; touch `README.md`'s build/repo-layout sections only to the extent they actively contradict the new install path. No code in `crates/{executor,brokerd,sandbox,runtime-core}` or `cli/caprun` changes.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Release binary build (`cargo build --workspace --release`) | Build/CI tooling (shell script) | — | Pure build orchestration; no runtime component owns this |
| Sibling-binary co-location (copy 3 bins to one dir) | Build/CI tooling (shell script) | — | Deployment-layout concern; the runtime only *consumes* the resulting layout via `current_exe()`, it does not create it |
| Sibling resolution at runtime (`current_exe().parent()`) | `cli/caprun` (orchestrator) / `crates/brokerd` (broker) | — | Existing, unchanged production code (`main.rs`, `process_exec.rs`); this phase documents it, never edits it |
| Post-install layout verification (exist + executable) | Build/CI tooling (shell script) | — | Non-destructive filesystem check; explicitly NOT a broker/session/audit-DAG concern (D-09) |
| Policy file location/selection | `cli/caprun` (CLI flag) / `crates/brokerd` (`bind_policy`) | Docs (checklist) | `--policy`/`CAPRUN_POLICY` are existing CLI/broker surface; docs only describe it |
| Credential custody (`CAPRUN_GITHUB_TOKEN` etc.) | `crates/brokerd` (broker-local process env reads) | Docs (checklist) | Broker-only env reads by design (never forwarded to the confined worker); docs describe placeholder-only usage |
| Confinement preconditions (kernel ≥ 5.13) | `crates/sandbox` (Landlock negotiation) | Docs (platform note) | Runtime already negotiates down gracefully; docs surface the requirement, do not enforce it |

## Standard Stack

### Core

No new libraries. This phase adds zero Cargo dependencies and zero new crates (D-10, HYG-02 carry-forward). The only "stack" element is the shell itself.

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|---------------|
| `bash` | any POSIX-ish bash already assumed by `scripts/*.sh` | Install script interpreter | Matches every existing script in `scripts/` (`#!/usr/bin/env bash`, `set -euo pipefail`) — introducing a different interpreter (POSIX `sh`, Python) would break house-style consistency for no benefit |
| `cargo` (workspace-pinned via `rustup`) | Whatever the operator's toolchain provides; workspace has no `rust-toolchain.toml` pin `[VERIFIED: /home/ben/Workspace/caprun/docs/GETTING-STARTED.md:12]` "There is no rust-toolchain.toml or rust-version field pinning a specific version." | Builds the release binaries | Already the project's sole build tool |

### Supporting

None. No `jq`, no `curl`-based downloader, no checksumming tool is needed for a source-build-and-copy script (that machinery belongs to the explicitly deferred PACK-02 cargo-dist/deb/snap path).

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Thin bash install script | `cargo install --path cli/caprun --bin caprun --bin caprun-worker` + a second manual `cp` for the launcher | Still requires manual steps for the launcher and a manual destination move; doesn't verify the three-binary set; CONTEXT.md D-08 explicitly requires warning that plain `cargo install --path cli/caprun` is insufficient, so this is documented as a **pitfall**, not adopted as the mechanism |
| Thin bash install script | `cargo-dist` / `.deb` / snap | Explicitly out of scope — deferred to PACK-02 (D-01–D-03, ROADMAP "Deferred") |
| `${HOME}/.local/bin` default destination | `/usr/local/bin` system-wide default | Requires `sudo`/root; D-04 locks user-local default, system-wide only as an explicit override, never via implicit `sudo` |

**Installation:** No package installation. The script itself is a checked-in shell file (`scripts/install-linux.sh`), not something users install via a package manager.

**Version verification:** N/A — no external package versions to verify. `rustc`/`cargo` version is whatever `rustup` provides; the workspace pins no specific version `[VERIFIED: Cargo.toml:1-6]` (`resolver = "3"`, `edition = "2021"`, no `rust-version` field present in the root `[workspace.package]` table read this session).

## Package Legitimacy Audit

Not applicable — this phase introduces **zero** new Cargo dependencies and zero new crates (D-10; HYG-02 carry-forward re-asserted for v1.10). No `npm`/`pip`/`cargo add` package is installed by the install script or by any doc change.

**Packages removed due to [SLOP] verdict:** none (no packages evaluated — none proposed).
**Packages flagged as suspicious [SUS]:** none.

## Architecture Patterns

### System Architecture Diagram

```
 Design partner's shell
        │
        │ 1. clone repo
        ▼
 ┌─────────────────────────────┐
 │ scripts/install-linux.sh    │
 │  (this phase's deliverable) │
 └───────────┬──────────────────┘
             │ 2. cargo build --workspace --release
             ▼
   target/release/
     caprun
     caprun-worker
     caprun-exec-launcher
     caprun-planner        (optional, NOT copied — D-07)
             │ 3. copy exactly 3 required bins
             ▼
   ${INSTALL_DEST:-$HOME/.local/bin}/
     caprun
     caprun-worker
     caprun-exec-launcher
             │ 4. post-copy check: test -x on all 3 (D-09)
             ▼
        install reports success / fails early with actionable message (D-03)

 ── separately, at RUN time (existing runtime code, unmodified) ──

 operator$ caprun run --policy policy.json <intent-kind> <intent-param> <workspace-file> [audit-db]
        │
        ▼
 cli/caprun/src/main.rs
   │  current_exe().parent().join("caprun-worker")   ── SINGLE hop, no fallback
   │        (main.rs:607-611)
   ▼
 spawns caprun-worker (self-confines after connecting to broker)
   │
   │  (only if the coding recipe reaches process.exec)
   ▼
 crates/brokerd/src/sinks/process_exec.rs::resolve_launcher_path()
   │  current_exe().parent(), then walk up to 2 MORE ancestor dirs
   │  looking for `caprun-exec-launcher`               (process_exec.rs:736-756)
   ▼
 spawns caprun-exec-launcher (self-confines, then execve's the target command)
```

### Recommended Project Structure

No new source directories. The one new file is a sibling of the existing `scripts/*.sh`:

```
scripts/
├── install-linux.sh      # NEW — this phase's deliverable
├── check-invariants.sh   # existing — style/pattern reference
├── compose-verify.sh     # existing — style/pattern reference; ALSO shows how
│                          # sibling binaries get produced for verification
├── docker-cache.sh       # existing — style reference for a non-Docker,
│                          # status/argument-dispatch style script
└── mailpit-verify.sh     # existing — style reference
docs/
├── GETTING-STARTED.md    # UPDATE — replace stale 2-binary / Colima content
└── CONFIGURATION.md      # UPDATE — replace stale env-var table
README.md                 # UPDATE — build section + repo layout only
```

### Pattern 1: current_exe()-relative sibling resolution (READ-ONLY, do not modify)

**What:** Two independent, already-shipped resolution functions locate sibling binaries relative to the running process's own binary path — never via `$PATH`, never via a hardcoded absolute path.

**Where (verified this session):**

1. `caprun-worker` resolution — `cli/caprun/src/main.rs:607-611`:
```rust
// Source: cli/caprun/src/main.rs:607-611 (read this session)
let worker_binary = std::env::current_exe()
    .context("current_exe")?
    .parent()
    .ok_or_else(|| anyhow::anyhow!("caprun has no parent dir"))?
    .join("caprun-worker");
```
This is a **single** `.parent()` hop with **no fallback search**. If `caprun-worker` is not literally in the same directory as the running `caprun` binary, `Command::new(&worker_binary).spawn()` fails with an OS-level "No such file or directory" — there is no custom error message wrapping this failure at the resolution site itself (the `.context("current_exe")?` only wraps the `current_exe()` call, not the eventual spawn). **This is the single strongest reason the install script must place `caprun-worker` as a direct sibling of `caprun` — there is no ancestor-walk tolerance here, unlike the launcher.**

2. `caprun-planner` resolution (optional sidecar, only spawned when `CAPRUN_PLANNER=llm`) — `cli/caprun/src/main.rs:571-575`, same single-hop pattern. Not part of the required three-binary install set (D-07), but co-locating it costs nothing and the existing `cargo build --workspace --release` produces it anyway.

3. `caprun-exec-launcher` resolution — `crates/brokerd/src/sinks/process_exec.rs:736-756`:
```rust
// Source: crates/brokerd/src/sinks/process_exec.rs:736-756 (read this session)
pub(crate) fn resolve_launcher_path() -> Result<std::path::PathBuf> {
    let current_exe =
        std::env::current_exe().context("process.exec: could not resolve current_exe()")?;
    let mut dir = current_exe.parent().map(|p| p.to_path_buf());
    for _ in 0..3 {
        let Some(candidate_dir) = dir else { break };
        let candidate = candidate_dir.join("caprun-exec-launcher");
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = candidate_dir.parent().map(|p| p.to_path_buf());
    }
    Err(anyhow::anyhow!(
        "process.exec: could not locate sibling binary `caprun-exec-launcher` near \
         current_exe() {current_exe:?} (checked current_exe()'s parent and up to 2 \
         ancestor directories — covers both the production `caprun` binary layout and \
         a `cargo test` integration-test binary under target/{{debug,release}}/deps/; \
         run `cargo build --workspace` first if this is a fresh checkout — \
         cargo-test-workspace-missing-sibling-binary)"
    ))
}
```
This one **does** tolerate up to 2 extra ancestor hops (to cope with `cargo test`'s `target/{debug,release}/deps/<hash>` binary placement), and it fails with an explicit, actionable error string if none of the 3 checked directories contain the file. **Installing it as a direct sibling of `caprun`/`caprun-worker` satisfies this at depth 0** — the ancestor-walk tolerance is a test-harness accommodation, not something the install layout should rely on.

**When to use:** This is exactly why co-location in ONE destination directory is a *functional and security-relevant deployment invariant* (CONTEXT.md, "Established Patterns"), not a documentation nicety — `caprun-worker` resolution has zero fallback tolerance.

**Confinement note (secondary, for doc accuracy only — not this phase's concern to modify):** `caprun-exec-launcher` is the Option B (DESIGN-effect-breadth-exec.md §1.3) self-confining exec helper — spawned unconfined by the broker, applies Landlock+seccomp to itself post-fork, then `execve`s the real target command. It is never invoked directly by an operator; it reads `EXEC_COMMAND`/`EXEC_ARGS`-shaped env vars set only by the broker `[VERIFIED: cli/caprun-exec-launcher/src/main.rs:37-45]`.

### Anti-Patterns to Avoid

- **Copying only `caprun` (or `caprun` + `caprun-worker`) and stopping:** `cargo install --path cli/caprun` produces this exact broken partial install — `caprun` will run fine right up until a `process.exec` plan node fires (e.g. the Safe Coding Agent's test step), then fail with `resolve_launcher_path()`'s explicit error. D-08 requires the docs/script to warn against this specific mistake by name.
- **Reinventing or rewriting the resolution algorithm in the install script's error messages / logic:** the script's job is to satisfy the existing algorithm's constraints (same-directory placement), not to reimplement or second-guess it. Do not add a new discovery mechanism (env var override for sibling lookup, `$PATH` search, symlink farm) — that would be a runtime behavior change, explicitly out of scope (D-10).
- **Making the post-install check invoke the binaries or open a session:** D-09 requires the check to be non-destructive — no live credentials, no repo mutation, no claim of re-running the Linux security proof. `test -f "$dest/$bin" && test -x "$dest/$bin"` for each of the three names is sufficient and matches the wording of D-09 exactly ("all three sibling paths exist and are executable").
- **Defaulting to a system-wide destination or invoking `sudo` implicitly:** D-04 forbids this; `/usr/local/bin` or similar must only be reachable via an explicit destination override the operator supplies.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Detecting whether a directory is on `$PATH` | A custom parser of `$PATH` with edge-case handling (trailing slash, empty entries meaning `.`) | A simple `case ":$PATH:" in *":$dest:"*) ... esac` substring test (the standard shell idiom used across countless install scripts) | Sufficient for a hint-only, non-blocking check; a hand-rolled fully-correct PATH parser is unnecessary engineering for advisory output |
| Atomic/safe "replace in place" for re-install (D-06) | A custom lockfile/rename protocol | Stage to a temp subdirectory under the SAME destination filesystem (`mktemp -d` inside `$dest`'s parent, or `$dest.new`), copy all 3 binaries there, then `mv` each into place (or `mv` the whole staged dir over the old one) — POSIX `rename()` on the same filesystem is atomic per-file; doing all 3 renames back-to-back, in a fixed order, right after each copy succeeds, is the simplest shell-safe mechanism that avoids a visibly mixed three-binary set on a mid-copy failure (D-06 leaves the exact mechanism to planner discretion, but explicitly asks for "the simplest shell-safe staging/replacement mechanism") | Reinventing package-manager-grade transactional install is disproportionate for 3 files; a temp-dir-then-rename pattern is standard and auditable in a few lines |
| Kernel version comparison for the Landlock ≥5.13 precondition | A custom semver-parsing routine | `uname -r` string compare is already how `docs/GETTING-STARTED.md`'s "Common setup issues" section documents this (`Check your kernel: uname -r`) — the install script can mirror this as an advisory check (warn, don't hard-block, since `crates/sandbox` itself negotiates ABI down gracefully on older-but-still-≥5.13 kernels) | The runtime itself already handles the negotiation; the script only needs to warn early per D-03 ("unsupported operating systems" / actionable messages), not re-implement kernel capability detection |

**Key insight:** Every "don't hand-roll" item here is about restraint, not libraries — the phase's entire risk surface is scope creep into either (a) rebuilding what `current_exe()` resolution or Landlock negotiation already does, or (b) building toward the deferred PACK-02 productization (checksums, signing, auto-update, package-manager integration).

## Runtime State Inventory

Not applicable — this is not a rename/refactor/migration phase. Skipped per the trigger condition (Step 2.5 applies only to rename/refactor/migration phases).

## Common Pitfalls

### Pitfall 1: Documenting or shipping a two-binary install (the exact D-08 hazard)
**What goes wrong:** An operator runs `cargo install --path cli/caprun`, gets `caprun` and (if they pass `--bin caprun-worker` too) `caprun-worker` on their `$PATH` via `~/.cargo/bin`, and everything appears to work until a `process.exec` plan node (e.g. the coding recipe's test step) fires and fails with `resolve_launcher_path()`'s error.
**Why it happens:** `caprun-exec-launcher` is a **separate Cargo package** (`cli/caprun-exec-launcher/Cargo.toml`), not a second `[[bin]]` inside the `cli/caprun` package — so no single `cargo install --path cli/caprun` invocation, however many `--bin` flags are added, can ever produce it.
**How to avoid:** The script always builds and copies all three from one `cargo build --workspace --release`, and the docs explicitly call out (D-08) that `cargo install --path cli/caprun` alone is insufficient and why.
**Warning signs:** `resolve_launcher_path()`'s error string is itself the warning sign at runtime — it already names the missing binary and suggests `cargo build --workspace` `[VERIFIED: crates/brokerd/src/sinks/process_exec.rs:748-755]`.

### Pitfall 2: Trusting the "±2 ancestor directories" launcher tolerance as a substitute for co-location
**What goes wrong:** A planner or script author sees that `resolve_launcher_path()` tolerates the launcher being up to 2 directories above `current_exe()`'s parent and concludes co-location is optional for the launcher.
**Why it happens:** That tolerance exists **only** to accommodate `cargo test`'s `target/{debug,release}/deps/<hash>` binary placement (documented explicitly in the function's own doc comment, lines 705-735) — it is a test-harness accommodation, not a deployment feature. `caprun-worker`'s resolution has **zero** such tolerance (a single hop, main.rs:607-611), so any install layout that separates the launcher from `caprun`/`caprun-worker` (relying on the ancestor walk) still breaks the worker spawn.
**How to avoid:** Install all three into exactly one flat directory. Never rely on the ancestor-walk behavior as an install-layout feature.
**Warning signs:** Worker spawn failure with a bare OS "No such file or directory," not the launcher's own descriptive error — because the worker fails first and has no custom message wrapping the spawn.

### Pitfall 3: Stale documentation drift (already present in the repo — do not perpetuate it)
**What goes wrong:** `docs/CONFIGURATION.md`'s "Worker Environment Variables" table currently lists `SESSION_ID` as a worker-consumed env var. It is not read anywhere in the current codebase (verified by grep across `crates/` and `cli/` this session — zero matches). The same table omits every `CAPRUN_*` variable that actually exists (`CAPRUN_POLICY`, `CAPRUN_GITHUB_TOKEN`, `CAPRUN_GIT_PUSH_TOKEN`, `CAPRUN_HTTP_WRITE_TOKEN`, `CAPRUN_SMTP_HOST`/`_PORT`/`_FROM`, `CAPRUN_GITHUB_API_BASE`, `CAPRUN_CONFIRM`, `CAPRUN_CONFIRM_TIMEOUT_SECS`, `CAPRUN_PLANNER`, `CAPRUN_PLANNER_MODEL`).
**Why it happens:** The doc was last accurate for an earlier, simpler CLI shape (two positional args, no verbs) that predates `run`/`--policy`/`confirm`/`deny`/`review`/`grant`/`audit` and the v1.9/v1.10 sink additions.
**How to avoid:** This phase's plan should explicitly task rewriting these tables from the verified grep results in this document (see "Environment Availability" / checklist below), not incrementally patching the existing stale table.
**Warning signs:** Any new doc content that copies forward `SESSION_ID` or the old two-positional-arg usage string is propagating the same drift.

### Pitfall 4: README.md's Colima/Mac guidance is itself flagged stale by CLAUDE.md
**What goes wrong:** Copying README.md's or GETTING-STARTED.md's "Running from macOS (Colima)" section forward as if it reflects the current verification workflow.
**Why it happens:** `[VERIFIED: /home/ben/Workspace/caprun/CLAUDE.md]` — CLAUDE.md itself states: "the current dev box is native Linux... this section previously described a Mac + Colima setup, which is stale," and that all Linux security verification now goes through `scripts/mailpit-verify.sh`/`scripts/compose-verify.sh` on ephemeral EC2, never bare `docker run rust:1`.
**How to avoid:** This phase should not attempt a full rewrite of the Colima sections (out of scope — this phase is about the install path, not the verification workflow), but any NEW text this phase adds must not contradict CLAUDE.md's current guidance, and if the planner chooses to touch README.md's build section it should avoid re-asserting the Colima recipe as current.
**Warning signs:** New install-path text that tells a design partner to use Colima/Docker for a normal `cargo build --workspace --release` + copy — Docker is never required for this phase's install path.

## Code Examples

### Minimal policy JSON example (D-12)

`SessionPolicy`'s two fields, verified from its own definition:
```rust
// Source: crates/runtime-core/src/policy.rs:175-184 (read this session)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionPolicy {
    allowed_sinks: BTreeSet<String>,
    arg_constraints: BTreeMap<String, BTreeMap<String, ArgConstraint>>,
}
```
No `#[serde(rename...)]` attribute is present on the struct or its fields (verified by reading lines 170-235 of the same file), so the JSON keys are exactly `allowed_sinks` and `arg_constraints`. A minimal hand-written policy file admitting only the file/commit sinks:

```json
{
  "allowed_sinks": ["file.create", "file.write", "git.commit"],
  "arg_constraints": {}
}
```

The nine production sink id strings, verified from the same file `[VERIFIED: crates/runtime-core/src/policy.rs:34-55]`:
```
"email.send", "file.create", "file.write", "process.exec",
"git.commit", "http.request", "github.pr", "http.request.write", "git.push"
```

**Containment constraint on the policy file's own location:** `bind_policy` refuses a policy path at-or-beneath the workspace root `[CITED: crates/brokerd/src/policy.rs:89 doc comment — "bind_policy: refusing to bind a policy at-or-beneath the workspace ..."]` — the checklist/example must place the policy file OUTSIDE the workspace directory being operated on.

### CLI invocation shape (verified from source, not memory)

```
// Source: cli/caprun/src/main.rs:9,236-279 (read this session)
caprun [run] [--policy <path>] <intent-kind> <intent-param> <workspace-file> [audit-db-path]
caprun confirm <effect_id> [audit-db-path]
caprun deny <effect_id> [audit-db-path]
caprun review <effect_id> [audit-db-path]
caprun grant <session_id> [audit-db-path]
caprun audit <session_id> <audit-db-path>          # audit-db-path is REQUIRED here (no :memory: default)
```
`--policy <path>` and `CAPRUN_POLICY` feed the SAME `bind_policy` call; the flag takes precedence, env is the fallback, and if neither is set the broker's `broker_default()` allowlist (the 9 production sinks, no arg constraints) is bound `[VERIFIED: cli/caprun/src/main.rs:434-439]`.

### Post-install layout check (shell, matching D-09's exact wording)

```bash
# Illustrative shape only — not a full script. Matches D-09: existence +
# executable bit, nothing more (no invocation, no credentials, no session).
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ ! -x "${dest}/${bin}" ]; then
        echo "FAIL — ${dest}/${bin} missing or not executable" >&2
        exit 1
    fi
done
echo "PASS — all 3 required binaries present and executable in ${dest}"
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| Two-binary layout (`caprun`, `caprun-worker`) as documented in README.md/GETTING-STARTED.md/CONFIGURATION.md | Three-binary required layout (`caprun`, `caprun-worker`, `caprun-exec-launcher`) plus optional `caprun-planner` | `caprun-exec-launcher` shipped in the v1.7 process.exec work (DESIGN-effect-breadth-exec.md, Option B); docs never caught up | Any doc content copied forward from the current README/GETTING-STARTED without correction perpetuates an install path that breaks on `process.exec` |
| Bare positional CLI (`caprun <workspace-file> [audit-db-path]`) | Verb + flag CLI (`caprun [run] [--policy <path>] <intent-kind> <intent-param> <workspace-file> [audit-db-path]`, plus `confirm`/`deny`/`review`/`grant`/`audit` verbs) | v1.8/v1.9 SDK-01/U1/POLICY-03 work | Example commands in stale docs no longer match the actual binary's argv parsing |
| Colima+Docker as the only way to run Linux security tests | Native Linux dev box (no Docker) for `check-invariants.sh`/`git diff`/hashing; ephemeral EC2 for anything needing Docker (`mailpit-verify.sh`/`compose-verify.sh`) | Documented as current in CLAUDE.md, updated 2026-08-09 | This phase's install script and its own validation must NOT assume Docker is present — it isn't, on the dev box, and shouldn't be required for a source-build install anyway |

**Deprecated/outdated:**
- `docs/CONFIGURATION.md`'s worker env var table (lists non-existent `SESSION_ID`, omits every real `CAPRUN_*` var) — must be replaced, not patched.
- README.md/GETTING-STARTED.md's "two binaries" framing — must be replaced.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|----------------|
| A1 | Design partners' target machines will have `cargo`/`rustup` already available or will install it themselves (script does not attempt to install a Rust toolchain) | Standard Stack / Don't Hand-Roll | If wrong, the install script's "missing build prerequisites" fail-fast message (D-03) needs to name `cargo`/`rustc` explicitly rather than assume familiarity — low risk, easily covered by a clear precondition check |
| A2 | `${HOME}/.local/bin` is writable and typically already exists (or is safely `mkdir -p`-able) without special permissions on a "minimal Linux install" target environment | Standard Stack (Alternatives Considered) / D-04 | If wrong on some minimal/container base image, the script's fail-fast path (D-03, "unusable destination") must produce an actionable message — this is a fallback the design already requires, so risk is low |
| A3 | No project style/lint tool (e.g. `shellcheck`) is required or expected to gate this script, since `shellcheck` is not installed on this dev box and no existing script's header or CI config references it | Validation Architecture | If the user later wants `shellcheck` as a gate, it would need separate installation; using `bash -n` as the available local syntax check is a safe, honest fallback either way |

**If this table is empty:** N/A — see above; all three assumptions are low-risk operational defaults, not claims about locked repo behavior. Every claim about *existing repo behavior* in this document (sibling resolution, CLI usage, env vars, policy schema, doc staleness) is `[VERIFIED]` against source read this session, not `[ASSUMED]`.

## Open Questions

1. **Should the install script build in `--release` mode unconditionally, or offer a debug-mode option?**
   - What we know: `docs/GETTING-STARTED.md` currently documents debug-mode usage (`target/debug/caprun`); D-01/D-02 describe "the required release targets" for the script specifically.
   - What's unclear: Whether a debug-mode fallback is worth offering for faster local iteration by a design partner, vs. keeping the script single-purpose (release only, matching D-02's "build the required release targets" wording).
   - Recommendation: Default to `--release` only, per D-02's explicit wording ("build the required release targets"); the manual-commands documentation path (D-01) can optionally mention debug mode as a separate, unscripted alternative for contributors, not design partners.

2. **Exact final destination structure for the doc rewrite (single consolidated "Install" section vs. distributed across GETTING-STARTED + CONFIGURATION)?**
   - What we know: CONTEXT.md's canonical_refs name both `docs/GETTING-STARTED.md` (source-build + sibling-binary guidance) and `docs/CONFIGURATION.md` (CLI/env/audit/confinement reference) as the natural homes; agent discretion is explicitly granted for "document placement, headings, and wording."
   - What's unclear: Whether the planner should add a dedicated "Installation" H2 in GETTING-STARTED.md (replacing "Clone and build") with the checklist living in CONFIGURATION.md, or something else.
   - Recommendation: Put the install walkthrough (script + manual equivalent, D-01) in GETTING-STARTED.md replacing the "Clone and build"/"Running the substrate demo" sections' binary-count claims; put the tiered env/credential checklist (D-11) in CONFIGURATION.md replacing its stale env-var table. This mirrors each doc's existing stated purpose and requires touching, not duplicating, the existing structure.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| `cargo`/`rustc` (via `rustup`) | Building release binaries | ✓ (this dev box) | cargo 1.97.1 `[VERIFIED: cargo --version, run this session]` | Script should fail-fast with an actionable "install rustup" message if absent on a target machine (D-03) |
| Docker | NOT required by this phase's install script or its local validation | ✗ (not installed on this dev box, per CLAUDE.md) | — | Not needed — the install script is a plain `cargo build` + `cp`, no containers involved |
| `shellcheck` | Optional lint for the new script | ✗ (not installed on this dev box) | — | Use `bash -n scripts/install-linux.sh` for a local syntax check (confirmed working this session); treat `shellcheck` as a nice-to-have if available in CI/another environment, not a hard local gate |
| Linux kernel ≥ 5.13 (Landlock) | Full confinement guarantee at runtime (not the install script itself) | ✓ (kernel 6.8.0 on this dev box, `uname -r` run this session) | 6.8.0-136-generic | Script can `uname -r`-check and warn (advisory only, matches how `docs/GETTING-STARTED.md`'s existing "Common setup issues" section already phrases this) |

**Missing dependencies with no fallback:** none — every dependency this phase actually needs (`cargo`) is present on this dev box and the fallback story for a target machine lacking it is simply "fail fast with a clear message," which is already required by D-03.

**Missing dependencies with fallback:** Docker (not needed at all for this phase); `shellcheck` (use `bash -n` locally).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | None dedicated to shell scripts in this repo; the project's Rust test framework is `cargo test` (workspace-wide), unrelated to this phase's deliverable |
| Config file | none — `scripts/install-linux.sh` is a standalone bash file, same as its siblings |
| Quick run command | `bash -n scripts/install-linux.sh` (syntax check only, no execution) |
| Full suite command | A dry-run install into a temp prefix: `INSTALL_DEST="$(mktemp -d)" bash scripts/install-linux.sh` followed by the post-install existence/executable check |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| PKG-01 (criterion 1) | Release build co-locates all 3 required siblings | integration (dry-run) | `INSTALL_DEST="$(mktemp -d)" bash scripts/install-linux.sh && ls "$INSTALL_DEST"` — expect exactly `caprun caprun-worker caprun-exec-launcher` | ❌ Wave 0 (script does not exist yet — this phase creates it) |
| PKG-01 (criterion 1, negative) | `cargo install --path cli/caprun` alone does NOT produce the launcher — documented, not asserted by a test (this is a documentation claim, not a runtime behavior this phase changes) | manual/doc-review | N/A — verified via source reading this session (`cli/caprun-exec-launcher` is a separate package); no test needed since no code changes | N/A |
| PKG-01 (criterion 2) | Env/credential checklist matches the real `CAPRUN_*` surface | doc-review (grep cross-check) | `grep -roE 'CAPRUN_[A-Z_]+' crates/ cli/ --include=*.rs \| grep -v /tests/ \| sort -u` — every name in the checklist must appear in this output, and vice versa | ✅ (grep itself is the "test"; already run this session, see Standard Stack / Code Examples) |
| PKG-01 (criterion 3) | Script stays "thin" — no packaging framework, no new crate, no privileged operation | structural check | `./scripts/check-invariants.sh` (still green; this phase adds no `EffectRequest`, no runtime-core I/O) + manual review that no `sudo`/`apt`/`brew`/download-and-exec appears in the script | ✅ `check-invariants.sh` already exists and is Docker-free |
| D-09 (post-install check) | All 3 sibling paths exist and are executable after install, non-destructively | integration (dry-run) | Same dry-run command as criterion 1, plus asserting exit code 0 and that the script performed no network call, no credential read, no repo mutation (`git status --porcelain` unchanged before/after) | ❌ Wave 0 |
| D-06 (re-run/upgrade safety) | Re-running the installer replaces the set deterministically, no mixed partial set on failure | integration (fault injection) | Run the install twice into the same `INSTALL_DEST`; second run's binaries must all have fresh mtimes / match a fresh `cargo build` hash; a simulated mid-copy failure (e.g. `chmod 000` the destination between the 1st and 2nd binary copy in a controlled test) must not leave 1-of-3 or 2-of-3 stale+fresh mixed | ❌ Wave 0 — this is the highest-value test to add given D-06's explicit "avoid leaving a visibly mixed three-binary set" requirement |

### Sampling Rate

- **Per task commit:** `bash -n scripts/install-linux.sh` (instant, always runnable locally, no EC2/Docker needed)
- **Per wave merge:** Full dry-run install into a `mktemp -d` destination, on this dev box (no Docker/EC2 required — this is plain `cargo build --workspace --release` + `cp`, exactly as available locally as any other cargo build)
- **Phase gate:** `./scripts/check-invariants.sh` green (Docker-free, already required project-wide) + the dry-run install producing exactly the 3 required binaries, all executable, in the temp destination + a manual read-through confirming the doc updates match this RESEARCH.md's verified `CAPRUN_*`/CLI-usage findings

### Wave 0 Gaps

- [ ] `scripts/install-linux.sh` — does not exist yet; this phase's core deliverable
- [ ] No existing test harness exercises install scripts at all (`scripts/*.sh` other than this one are verification harnesses, not testable-in-isolation install logic) — the dry-run-into-`mktemp -d` pattern above is the gap-filling approach; it needs no new framework, just disciplined manual/CI invocation since there is no CI in this repo (`[CITED: README.md:114]` "There is no CI in this repo... no .github/workflows")
- [ ] A fault-injection check for D-06's "no mixed partial set on failure" guarantee (see table above) — this is the one test worth writing deliberately rather than leaving to manual review, since it's the requirement most likely to have a subtle bug (partial `cp` before a later `cp` fails)

*(Docker/EC2-gated Linux security tests are entirely out of scope for this phase's own validation — this phase touches no code under `crates/{executor,brokerd,sandbox,runtime-core}` or the worker submit/confirm-hold path, so `scripts/compose-verify.sh`/`scripts/mailpit-verify.sh` need not be re-run for this phase specifically, though the existing project convention of not regressing `check-invariants.sh` still applies and is Docker-free.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|----------------|---------|-------------------|
| V2 Authentication | no | This phase adds no authentication surface |
| V3 Session Management | no | No session-related code changes; `caprun`'s existing Session model is unmodified |
| V4 Access Control | no | No access-control code changes |
| V5 Input Validation | marginal — yes for the script's own destination-path handling | The install script must not blindly `eval`/`source` user-supplied destination paths, and must quote all path variables (matches D-03's "unusable destination" fail-fast requirement); no parsing of untrusted network input is involved |
| V6 Cryptography | no | No cryptographic code; the script never downloads or verifies signed artifacts (explicitly deferred to PACK-02 — D-13's "never write real tokens" is a docs/handling discipline, not a crypto implementation) |

### Known Threat Patterns for this phase's stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|-----------------------|
| Install script silently invoking `sudo`/privileged operations | Elevation of Privilege | D-04 explicitly forbids implicit `sudo`; script must never re-exec itself with elevated privileges, only accept an explicit destination override that a human chose |
| Credentials leaking into the install script's own output/logs, or into checked-in example files | Information Disclosure | D-13 — placeholder-only exports in all examples, least-scope guidance; the script itself never reads or echoes `CAPRUN_GITHUB_TOKEN`/`CAPRUN_GIT_PUSH_TOKEN`/`CAPRUN_HTTP_WRITE_TOKEN` (it doesn't need them — those are broker-runtime-only env reads, `[VERIFIED: crates/brokerd/src/sinks/github_pr.rs:74-83, git_push.rs:621-630, http_write.rs:75-82]`, entirely orthogonal to the build/copy install script) |
| Documenting worker-protocol-internal env vars (`BROKER_SOCK`, `WORKSPACE_FILE`, `INTENT`, `CAPRUN_CODING_I2_PROOF`, `CAPRUN_ENABLE_IPC_CREATE_SESSION`) as if operators should set them | Tampering (operator confusion leading to a misconfigured/bypassed session) | D-14 explicitly forbids instructing users to set these; verified this session that `caprun-worker` reads exactly `BROKER_SOCK`, `WORKSPACE_FILE`, `INTENT`, `PRIMARY_SEED_FILE_DERIVED`, `CAPRUN_CODING_I2_PROOF`, `CAPRUN_PLANNER`, `PLANNER_SOCK` — all broker-set, never operator-set `[VERIFIED: cli/caprun/src/worker.rs:99-393]` |
| A stale/incorrect env-var doc leading an operator to set an internal test-only switch like `CAPRUN_ENABLE_IPC_CREATE_SESSION` "just in case" | Elevation of Privilege (this switch gates a dangerous IPC session-creation arm) | `[VERIFIED: crates/brokerd/src/server.rs:1766-1772]` — this switch defaults to disabled and gates an internal arm; D-14 names it explicitly as something the checklist must NOT tell users to set |

## Sources

### Primary (HIGH confidence — all read directly from the repository this session)
- `cli/caprun/src/main.rs` (lines 1-60, 75-330, 530-650, 800-1050) — CLI usage/verbs, sibling resolution, env var construction for the worker/planner spawn, `--policy`/`CAPRUN_POLICY` binding
- `cli/caprun/src/worker.rs` (env::var call sites) — the complete worker-side env var surface
- `crates/brokerd/src/sinks/process_exec.rs` (lines 695-770) — `resolve_launcher_path()`, the launcher's bounded ancestor-walk resolution and its doc comment explaining the `cargo test` accommodation
- `crates/brokerd/src/sinks/github_pr.rs`, `git_push.rs`, `http_write.rs`, `email_smtp.rs` — every broker-local `CAPRUN_*` credential/config env var and its required/optional semantics
- `crates/brokerd/src/server.rs` (line 1755-1772) — `CAPRUN_ENABLE_IPC_CREATE_SESSION` internal-only gate
- `crates/brokerd/src/policy.rs` (lines 1-120) — `bind_policy` semantics, JSON parse, F1 containment refusal for the policy path itself
- `crates/runtime-core/src/policy.rs` (lines 1-235) — `SessionPolicy` struct, exact serde field names, `PRODUCTION_SINKS` list, `broker_default()`
- `cli/caprun-exec-launcher/src/main.rs` (lines 1-45) — what the launcher is, its env-var contract, why it's a separate self-confining process
- `Cargo.toml` (workspace root) + `cli/caprun/Cargo.toml` + `cli/caprun-exec-launcher/Cargo.toml` + `cli/caprun-planner/Cargo.toml` + `crates/sandbox/Cargo.toml` — the complete `[[bin]]` inventory across the workspace
- `README.md`, `docs/GETTING-STARTED.md`, `docs/CONFIGURATION.md` (full read) — current (stale) documentation baseline this phase must correct
- `scripts/check-invariants.sh`, `scripts/compose-verify.sh`, `scripts/docker-cache.sh`, `scripts/mailpit-verify.sh` (headers + relevant bodies) — house shell-scripting style/conventions
- `planning-docs/DESIGN-effect-breadth-exec.md` (grep-located excerpts) — Option B security rationale for the separate exec launcher
- Local shell commands run this session: `cargo --version` (1.97.1), `uname -r` (6.8.0-136-generic), `command -v shellcheck` (not found), `bash -n scripts/compose-verify.sh` (succeeds), `ls target/debug/` (confirms all 4 binaries already build locally)

### Secondary (MEDIUM confidence)
- None used — no WebSearch/Context7 lookups were needed; this phase's entire technical surface is internal-repo behavior, verifiable directly by reading source.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — no external libraries involved; every claim is either "no new dependency" or a version read directly from `cargo --version`/`Cargo.toml`.
- Architecture: HIGH — both sibling-resolution functions were read in full from source this session, not inferred.
- Pitfalls: HIGH — every pitfall traces to a specific line range read this session (stale `SESSION_ID`, the two resolution functions' differing tolerance, CLAUDE.md's own stale-Colima disclosure).

**Research date:** 2026-08-11
**Valid until:** Effectively pinned to the current commit's source tree — re-verify the `CAPRUN_*` grep and the two resolution functions if any brokerd/CLI code changes land before this phase is planned/executed (stable otherwise; no external ecosystem dependency to go stale).
