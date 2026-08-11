# Phase 52: Minimal Linux Packaging - Context

**Gathered:** 2026-08-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver a minimal, documented Linux source-build and install path for design partners. The installed layout must place `caprun`, `caprun-worker`, and `caprun-exec-launcher` together and explain the operator-facing policy, environment, and credential setup needed by applicable workflows. This phase does not introduce a package format, release distribution service, new runtime behavior, or broader platform support.

</domain>

<decisions>
## Implementation Decisions

### Installation Experience
- **D-01:** Provide both a thin `scripts/install-linux.sh` convenience path and equivalent manual release-build/copy commands in the documentation. The documentation remains sufficient when users do not want to execute a repository script.
- **D-02:** The script is a transparent source-tree installer: build the required release targets, copy the resulting sibling binaries into one destination, and report what it installed. It must not download opaque artifacts, invoke privileged package managers, or require root.
- **D-03:** Fail early with actionable messages for unsupported operating systems, missing build prerequisites, failed builds, missing expected binaries, or an unusable destination.

### Destination and Upgrades
- **D-04:** Default to the user-local executable directory `${HOME}/.local/bin`, while accepting an explicit destination override. System-wide installation is an operator-chosen override, not the default and not performed through implicit `sudo`.
- **D-05:** Keep all required executables in exactly the same destination directory and document PATH setup when that directory is not already discoverable.
- **D-06:** Re-running the installer is the upgrade path. Replacement should be deterministic and avoid leaving a visibly mixed three-binary set if copying fails partway; the planner should choose the simplest shell-safe staging/replacement mechanism consistent with this guarantee.

### Binary Scope and Verification
- **D-07:** The required installed set is exactly `caprun`, `caprun-worker`, and `caprun-exec-launcher`. `caprun-planner` is optional LLM-sidecar functionality and is not part of the PKG-01 minimal deterministic workflow.
- **D-08:** Explicitly warn that `cargo install --path cli/caprun` is insufficient because it does not install the separate `caprun-exec-launcher` package.
- **D-09:** The install path must perform or document a fail-fast post-install check that all three sibling paths exist and are executable. Keep this check non-destructive: it should not need live credentials, mutate a repository, or claim to re-run the full Linux security proof.
- **D-10:** Build release binaries from the workspace without adding crates, changing runtime sibling resolution, or introducing a packaging framework.

### Configuration Guidance
- **D-11:** Organize configuration as an operator checklist with three tiers: always needed for the chosen command (workspace/audit and policy inputs), sink-specific credentials/settings, and explicitly internal/test-only variables that design partners should not set.
- **D-12:** Prefer the existing `--policy` CLI path in runnable examples and document `CAPRUN_POLICY` as its fallback. Include a minimal policy-file example or point to a canonical checked-in example if one is created during implementation.
- **D-13:** Document credentials only with placeholder exports and least-scope guidance. Never write real tokens into files, command examples, logs, or planning artifacts. Cover `CAPRUN_GITHUB_TOKEN` and `CAPRUN_GIT_PUSH_TOKEN` for the Safe Coding Agent path, and mention other broker-local sink variables only where applicable.
- **D-14:** Clearly separate operator-facing variables from worker protocol variables and proof/test switches. In particular, do not instruct users to set `BROKER_SOCK`, `SESSION_ID`, `WORKSPACE_FILE`, `CAPRUN_CODING_I2_PROOF`, or `CAPRUN_ENABLE_IPC_CREATE_SESSION` for normal operation.

### the agent's Discretion
- Exact script flag spelling, document placement, headings, and wording are left to the researcher and planner, provided the decisions above and PKG-01 remain intact.
- The planner may choose the smallest reliable post-install check supported by the current CLI rather than adding a new command solely for packaging.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Locked Scope
- `.planning/ROADMAP.md` § Phase 52 — goal, dependency, and success criteria for Minimal Linux Packaging.
- `.planning/REQUIREMENTS.md` § PKG-01 — required three-binary layout, configuration checklist, and explicit productization exclusions.
- `.planning/PROJECT.md` § v1.10 — design-partner packaging intent and milestone boundaries.

### Current User Documentation
- `docs/GETTING-STARTED.md` — current source-build instructions, Linux requirements, and existing sibling-binary guidance that must be updated rather than contradicted.
- `docs/CONFIGURATION.md` — current CLI, environment, audit, and confinement reference; currently incomplete for the shipped multi-step workflow.
- `README.md` — top-level build instructions and repository orientation.

### Binary Layout and Runtime Resolution
- `cli/caprun/Cargo.toml` — defines the `caprun` and `caprun-worker` binaries in one package.
- `cli/caprun-exec-launcher/Cargo.toml` — defines the separately packaged third required binary.
- `cli/caprun/src/main.rs` — production `current_exe()` sibling resolution for `caprun-worker`, policy selection, confirmation settings, and operator notices.
- `crates/brokerd/src/sinks/process_exec.rs` — production sibling resolution for `caprun-exec-launcher`.
- `planning-docs/DESIGN-effect-breadth-exec.md` — security rationale for the separate self-confining exec launcher.

### Existing Build/Verification Pattern
- `scripts/compose-verify.sh` — authoritative Linux verification layout and existing construction of sibling binaries; reference for consistency, not an install implementation to copy wholesale.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `scripts/` already contains fail-fast Bash verification utilities and is the established location for a thin Linux installer.
- Cargo workspace release builds already produce `caprun`, `caprun-worker`, and the separate `caprun-exec-launcher` target without a new packaging dependency.
- `docs/GETTING-STARTED.md` and `docs/CONFIGURATION.md` are the natural homes for the install walkthrough and operator checklist.

### Established Patterns
- Runtime lookup is deliberately relative to `current_exe()`; co-location is a functional and security-relevant deployment invariant, not a documentation preference.
- The project uses explicit, fail-closed shell scripts and keeps Linux security claims tied to the authoritative compose verification rather than lightweight smoke tests.
- Credentials remain broker-local and are scrubbed from confined child environments; packaging docs must preserve that distinction.

### Integration Points
- The installer should build from the workspace root and install artifacts from `target/release/` into one destination.
- Documentation must replace stale statements that only two binaries are produced or required.
- Configuration guidance must align with the actual CLI parsing and environment reads in `cli/caprun/src/main.rs` and broker sink modules.

</code_context>

<specifics>
## Specific Ideas

- Use a familiar user-local default (`${HOME}/.local/bin`) and print a concise PATH hint when needed.
- Keep the script auditable and small enough that a design partner can understand the equivalent manual commands at a glance.
- Treat installation verification as layout validation, not as a substitute for the retained LIVE evidence or `scripts/compose-verify.sh`.

</specifics>

<deferred>
## Deferred Ideas

- cargo-dist, deb, snap, hosted binary releases, checksummed artifact distribution, automatic updater behavior, and macOS support remain PACK-02 or later work.

### Reviewed Todos (not folded)
- `GSD plan executors can self-mark phase-level completion before verification runs` — tooling-process issue unrelated to PKG-01.
- `v1.3 Phase 16 v2 security obligations` — broader security follow-up outside this packaging phase.
- `gsd_run phases.clear deletes all milestones' phase dirs` — GSD tooling issue unrelated to the install deliverable.

</deferred>

---

*Phase: 52-minimal-linux-packaging*
*Context gathered: 2026-08-11*
