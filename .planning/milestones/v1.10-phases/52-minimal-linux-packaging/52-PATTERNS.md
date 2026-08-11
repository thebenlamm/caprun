# Phase 52: Minimal Linux Packaging - Pattern Map

**Mapped:** 2026-08-11
**Files analyzed:** 4 (1 new script, 3 doc updates)
**Analogs found:** 4 / 4

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|--------------------|------|-----------|-----------------|----------------|
| `scripts/install-linux.sh` | utility (build/install script) | file-I/O (build → copy → verify) | `scripts/docker-cache.sh` (status/check/clean CLI-dispatch shape) + `scripts/mailpit-verify.sh`/`scripts/compose-verify.sh` (house style: header, `set -euo pipefail`, env-override defaulting, trap/cleanup, fail-fast echo) | role-match (no prior *installer* exists; closest are the other `scripts/*.sh` utilities) |
| `docs/GETTING-STARTED.md` (update) | doc | transform (stale → corrected content) | itself (existing "Clone and build" / "Running the substrate demo" / "Common setup issues" sections) | exact (in-place edit) |
| `docs/CONFIGURATION.md` (update) | doc | transform | itself (existing "caprun CLI Arguments" / "Worker Environment Variables" tables) | exact (in-place edit) |
| `README.md` (update) | doc | transform | itself (existing "Build & test" / "Repository layout" sections) | exact (in-place edit, narrow scope only) |

## Pattern Assignments

### `scripts/install-linux.sh` (utility, file-I/O)

**Analogs:** `scripts/mailpit-verify.sh`, `scripts/compose-verify.sh`, `scripts/docker-cache.sh`, `scripts/check-invariants.sh`

**Shebang + header doc-comment pattern** (`scripts/mailpit-verify.sh:1-73`, `scripts/check-invariants.sh:1-16`):
```bash
#!/usr/bin/env bash
# <script-name>.sh — <one-line purpose> (Phase <N>, <req-id>)
#
# <prose paragraphs: what it does, numbered list of steps, why>
#
# Usage:
#   bash scripts/<script-name>.sh
# Run from the workspace root (same directory as Cargo.toml).
#
# Env overrides (rarely needed):
#   VAR_NAME — purpose (default: <value>)
```
Every existing script in `scripts/` opens this way; `install-linux.sh` should follow the same shape (Usage line, "Run from the workspace root" note, an "Env overrides" table in comments).

**`set -euo pipefail` + immediate fail-fast idiom** (`scripts/mailpit-verify.sh:74`, `scripts/compose-verify.sh:62`, `scripts/check-invariants.sh:17`):
```bash
set -euo pipefail
```
Universal across all four existing scripts — first non-comment line.

**`${VAR:-default}` env-override defaulting idiom** (`scripts/mailpit-verify.sh:81-82,96`, `scripts/compose-verify.sh:69-71,87`, `scripts/docker-cache.sh:54`):
```bash
MAILPIT_NET="${MAILPIT_NET:-caprun-mailpit-net}"
MAILPIT_NAME="${MAILPIT_NAME:-caprun-mailpit}"
MAILPIT_VERIFY_CMD="${MAILPIT_VERIFY_CMD:-cargo build --workspace && cargo test --workspace --no-fail-fast}"
```
Directly applicable: `INSTALL_DEST="${INSTALL_DEST:-$HOME/.local/bin}"` mirrors this exactly (matches D-04's default + override requirement).

**Fail-fast, actionable-message error pattern** (`scripts/compose-verify.sh:137-140`, `scripts/mailpit-verify.sh:141-144`):
```bash
if [ -z "${MAILPIT_IP}" ]; then
    echo "FAIL — could not resolve ${MAILPIT_NAME}'s container IP" >&2
    exit 1
fi
```
And the multi-line actionable-error style used in production Rust but worth mirroring in shell (`crates/brokerd/src/sinks/process_exec.rs:210-218`, referenced in RESEARCH.md) — always name the missing thing and suggest the fix. Apply this exact `echo "FAIL — <what> <why>" >&2; exit 1` shape to D-03's "unsupported OS / missing prerequisites / failed build / missing binary / unusable destination" cases.

**PASS/status echo convention** (`scripts/compose-verify.sh:212`, `scripts/mailpit-verify.sh:191`, `scripts/check-invariants.sh:358-363`):
```bash
echo "Composed Linux verification suite PASSED (Mailpit + mock GitHub)."
```
```bash
if [ "$overall" -eq "$PASS" ]; then
    echo "All invariant gates PASSED."
else
    echo "One or more invariant gates FAILED — see output above."
    exit 1
fi
```
Use plain `echo` (no color codes, no external logging lib) for both progress ("Building release binaries...") and final PASS/FAIL summary lines. This matches D-02's "report what it installed."

**Trap/cleanup pattern** (`scripts/mailpit-verify.sh:98-102`, `scripts/compose-verify.sh:111-116`):
```bash
cleanup() {
    echo "Cleaning up Mailpit sidecar (${MAILPIT_NAME}) ..."
    docker stop "${MAILPIT_NAME}" >/dev/null 2>&1 || true
}
trap cleanup EXIT
```
`install-linux.sh` doesn't manage a Docker sidecar, but the same `trap ... EXIT` idiom is the house pattern for D-06's "no visibly mixed three-binary set on partial failure" — stage to a temp dir, and use a trap to clean up the staging dir on any exit path (success or failure), e.g.:
```bash
STAGE_DIR="$(mktemp -d "${dest%/}.XXXXXX" 2>/dev/null || mktemp -d)"
cleanup() { rm -rf "${STAGE_DIR}"; }
trap cleanup EXIT
```

**CLI dispatch / argument-command style (secondary reference for flag parsing)** (`scripts/docker-cache.sh:43-51,105-157`):
```bash
# Usage:
#   scripts/docker-cache.sh check           # warn-only; silent if clean/under cap
#   scripts/docker-cache.sh status          # always print current caprun-* volumes
#   scripts/docker-cache.sh clean           # prune caprun-* volumes (prompts first)
#   scripts/docker-cache.sh clean --yes     # prune without prompting (agent/CI use)
...
CMD="${1:-status}"
...
case "${CMD}" in
    check) ... ;;
    status) ... ;;
    clean) ... ;;
    *)
        echo "Usage: scripts/docker-cache.sh {check|status|clean [--yes]}" >&2
        exit 1
        ;;
esac
```
If `install-linux.sh` accepts a destination override as a positional arg or flag rather than only via `INSTALL_DEST` env var, this `case`/usage-fallback pattern is the house idiom to copy.

**Post-install / gate check style** (RESEARCH.md's own "Post-install layout check" code example, matching D-09 verbatim and the same `for`/`if [ ! -x ]` shape already used in `check-invariants.sh`'s gates):
```bash
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ ! -x "${dest}/${bin}" ]; then
        echo "FAIL — ${dest}/${bin} missing or not executable" >&2
        exit 1
    fi
done
echo "PASS — all 3 required binaries present and executable in ${dest}"
```

**Docker-cache-guard-style "call a sibling script at the top" idiom** (`scripts/mailpit-verify.sh:76-79`, `scripts/compose-verify.sh:64-67`):
```bash
"$(dirname "${BASH_SOURCE[0]}")/docker-cache.sh" check
```
Not applicable content-wise (no Docker in this script), but the `$(dirname "${BASH_SOURCE[0]}")` idiom is the house way to resolve the script's own directory if `install-linux.sh` needs to locate the repo root or other sibling scripts.

**What NOT to copy:** none of the three analog scripts do a `cargo build` + `cp` + verify sequence — they all orchestrate Docker sidecars for test verification, which is out of scope here (this script has zero Docker/network dependency). Copy only the shell house-style (header, `set -euo pipefail`, `${VAR:-default}`, echo conventions, trap/cleanup, fail-fast messages), not their Docker-specific bodies.

---

### `docs/GETTING-STARTED.md` (doc, transform)

**Analog:** itself — existing structure to preserve and update in place.

**Section headings to update** (`docs/GETTING-STARTED.md:32-70` "Clone and build" / "Running the substrate demo (Linux)"):
```markdown
## Clone and build

```bash
git clone https://github.com/thebenlamm/caprun.git
cd caprun
cargo build --workspace
```

This builds all workspace crates and produces two binaries in `target/debug/`:

- `target/debug/caprun` — the orchestrator
- `target/debug/caprun-worker` — the self-confining worker (must stay in the same directory as `caprun`)
```
This "two binaries" framing (lines 40-43) is the stale content Pitfall 3/State-of-the-Art flags — replace with the three-required-binary set (`caprun`, `caprun-worker`, `caprun-exec-launcher`) plus the optional `caprun-planner`, and add the new `scripts/install-linux.sh` walkthrough alongside the manual `cargo build --workspace --release` + copy commands (D-01 requires both to be documented).

**"Common setup issues" table style to extend** (`docs/GETTING-STARTED.md:156-168`):
```markdown
**`caprun-worker` not found at startup**
`caprun` locates `caprun-worker` relative to its own binary path. Run `caprun` via its full `target/debug/caprun` path after `cargo build --workspace`. Do not copy just the `caprun` binary without `caprun-worker`.
```
Same bold-question / one-paragraph-answer style should get a new entry for the `caprun-exec-launcher` failure mode (D-08's `cargo install --path cli/caprun` pitfall) — matches this file's existing Q&A convention exactly.

**CLI usage code-fence style** (`docs/GETTING-STARTED.md:59-61`):
````markdown
```
caprun <workspace-file> [audit-db-path]
```
````
This bare-usage-line convention (no shell prompt, just the invocation shape in a plain fenced block) is stale per RESEARCH.md's verified CLI shape (`caprun [run] [--policy <path>] <intent-kind> <intent-param> <workspace-file> [audit-db-path]`) — same fence style, corrected content.

**What to leave alone:** the "Running from macOS (Colima)" section (`docs/GETTING-STARTED.md:103-135`) is already flagged stale by CLAUDE.md itself; per RESEARCH.md Pitfall 4, do not rewrite it in this phase — only avoid contradicting it with new install-path text.

---

### `docs/CONFIGURATION.md` (doc, transform)

**Analog:** itself — existing "Worker Environment Variables" table structure, replaced with corrected content.

**Table style to replace wholesale** (`docs/CONFIGURATION.md:33-43`):
```markdown
## Worker Environment Variables

`caprun` spawns `caprun-worker` and injects these environment variables. They are internal to the caprun orchestration protocol — they are not consumed by end users directly.

| Variable | Set by | Description |
|----------|--------|-------------|
| `BROKER_SOCK` | caprun | Abstract UDS socket path without the leading NUL byte (e.g., `/agentos/<session-id>`). The worker prepends `\0` before connecting. |
| `SESSION_ID` | caprun | UUID of the current broker session. |
| `WORKSPACE_FILE` | caprun | Path to the workspace file forwarded from the caprun CLI argument. |
```
This is the exact stale table RESEARCH.md Pitfall 3 identifies (`SESSION_ID` unread in current code; missing every real `CAPRUN_*` var). Same 3-column `| Variable | Set by | Description |` table shape should be kept, but content replaced with the verified worker-protocol-internal set (`BROKER_SOCK`, `WORKSPACE_FILE`, `INTENT`, `PRIMARY_SEED_FILE_DERIVED`, `CAPRUN_CODING_I2_PROOF`, `CAPRUN_PLANNER`, `PLANNER_SOCK`) marked explicitly as NOT operator-facing (D-14), plus a new tiered table (D-11) for the operator checklist: always-needed (`--policy`/`CAPRUN_POLICY`, workspace/audit-db args), sink-specific credentials (`CAPRUN_GITHUB_TOKEN`, `CAPRUN_GIT_PUSH_TOKEN`, `CAPRUN_HTTP_WRITE_TOKEN`, `CAPRUN_SMTP_HOST`/`_PORT`/`_FROM`, `CAPRUN_GITHUB_API_BASE`), and internal/test-only (explicitly listed as "do not set": `CAPRUN_ENABLE_IPC_CREATE_SESSION`, `SESSION_ID`).

**CLI arguments table style** (`docs/CONFIGURATION.md:8-27`):
```markdown
## caprun CLI Arguments

```
caprun <workspace-file> [audit-db-path]
```

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `<workspace-file>` | Yes | — | Path to the workspace file the confined worker will read. ... |
| `[audit-db-path]` | No | `:memory:` | SQLite audit database path. ... |

**Example — ephemeral audit DB (default):**
```bash
./target/debug/caprun ./my-workspace.txt
```
```
Same table + "Example —" bold-label + fenced-command style should be reused for the corrected verb+flag CLI usage (`run`/`--policy`/`confirm`/`deny`/`review`/`grant`/`audit`) and for the D-12 minimal policy-file example.

**Placeholder-only credential guidance to introduce (no existing analog table, but matches this doc's existing prose-then-table pattern, e.g. `docs/CONFIGURATION.md:63-65` "Hardcoded Confinement Parameters" intro sentence style):**
```markdown
The following confinement values are hardcoded in the Rust TCB (`crates/sandbox/`). They are **not** configurable at runtime. Changing them requires editing the source and rebuilding.
```
Use the same "state the constraint plainly in one sentence, then a table" structure for the credential checklist's D-13 "placeholder exports only, never real tokens" guidance, e.g.:
```bash
export CAPRUN_GITHUB_TOKEN=ghp_xxx_least_scope_placeholder
export CAPRUN_GIT_PUSH_TOKEN=ghp_xxx_least_scope_placeholder
```

---

### `README.md` (doc, transform — narrow scope only)

**Analog:** itself — "Build & test" section only; do not touch "Security model"/"Architecture" sections (out of scope).

**Build & test section style to update minimally** (`README.md:56-72`):
```markdown
## Build & test

Requires Rust stable (2021 edition, workspace resolver 3).

```bash
# Build everything
cargo build --workspace

# Run all tests (Linux-only security tests require Linux — see below)
cargo test --workspace --no-fail-fast

# Run a single crate / single test target
cargo test -p brokerd audit_dag

# Architectural invariant gate (runs before code; exits non-zero on violation)
./scripts/check-invariants.sh
```
```
Comment-above-command style (`# Build everything`, `# Run all tests ...`) is this file's house convention for fenced bash blocks — reuse it for a possible one-line pointer to `scripts/install-linux.sh` / `docs/GETTING-STARTED.md`'s new Installation section. Per CONTEXT.md/RESEARCH.md scope, only touch README.md's build section + repository-layout section "to the extent they actively contradict the new install path" — do not re-litigate the Colima guidance here (RESEARCH.md Pitfall 4).

**Repository layout fenced-tree style** (`README.md:120-135`):
```markdown
## Repository layout

```
caprun/
  Cargo.toml              # workspace root (resolver = "3", edition 2021)
  scripts/
    check-invariants.sh   # Gate 1 (EffectRequest absent) + Gate 2 (runtime-core purity)
    docker-cache.sh       # caprun-* Docker volume retention policy (status/check/clean)
  ...
```
```
If the new `scripts/install-linux.sh` is added to this tree listing, follow the exact same `path   # one-line trailing comment` alignment style already used for `check-invariants.sh`/`docker-cache.sh`.

## Shared Patterns

### Shell script house style (all scripts/*.sh)
**Source:** `scripts/mailpit-verify.sh:1-102`, `scripts/compose-verify.sh:1-116`, `scripts/check-invariants.sh:1-21`, `scripts/docker-cache.sh:1-65`
**Apply to:** `scripts/install-linux.sh`
- `#!/usr/bin/env bash` shebang
- Header doc-comment: one-line purpose + phase/req-id tag, prose explanation, numbered steps, "Usage:" line, "Run from the workspace root" note, "Env overrides" comment-table
- `set -euo pipefail` as the first executable line
- `${VAR:-default}` for every overridable value
- `echo "... " >&2; exit 1` for every fail-fast condition, always naming the specific missing/broken thing
- Plain `echo` (no ANSI color, no external logging) for progress and final PASS/FAIL lines
- `trap cleanup EXIT` for any staging/temp-resource cleanup

### Doc heading/table conventions (docs/*.md, README.md)
**Source:** `docs/GETTING-STARTED.md` (H2 sections, bold-Q&A "Common setup issues" pattern), `docs/CONFIGURATION.md` (`| Col | Col | Col |` markdown tables throughout, "Example — <label>:" bold-lead-in before fenced commands)
**Apply to:** `docs/GETTING-STARTED.md`, `docs/CONFIGURATION.md`, `README.md`
- `<!-- generated-by: gsd-doc-writer -->` marker at file top (both `docs/*.md` files and `README.md` currently carry this — preserve it)
- `---` horizontal rules between H2 sections
- 3-4 column markdown tables with a header row for any enumerable set (CLI args, env vars, platform requirements)
- Fenced bash blocks with `# comment` lines above each command, not inline comments after `$`

## No Analog Found

None — every file in scope has at least a role-match analog in `scripts/` or is an in-place edit of itself.

## Metadata

**Analog search scope:** `scripts/` (all 4 existing `.sh` files), `docs/GETTING-STARTED.md`, `docs/CONFIGURATION.md`, `README.md`, `cli/caprun/src/main.rs` (sibling resolution + CLI parsing, read-only reference, not a pattern source for the shell script itself), `crates/brokerd/src/sinks/process_exec.rs` (`resolve_launcher_path`, read-only reference)
**Files scanned:** 8 (4 scripts, 3 docs, 1 Rust source file cross-checked for env var accuracy)
**Pattern extraction date:** 2026-08-11
