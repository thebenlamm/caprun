# Deferred Items — Phase 27

Out-of-scope discoveries found during plan execution, logged per the executor's
SCOPE BOUNDARY rule (not fixed here — pre-existing, unrelated to the plan's task).

## 1. `spawn caprun-planner sidecar: No such file or directory` in a fresh Linux container

**Found during:** 27-02 Task 2 verification (`MAILPIT_VERIFY_CMD='cargo test --workspace --no-fail-fast' bash scripts/mailpit-verify.sh`).

**Symptom:** `live_acceptance_v1_4_composed_three_legs` and
`llm_planner_clean_allow_delivers` fail with `Error: spawn caprun-planner
sidecar / Caused by: No such file or directory (os error 2)` when
`cargo test --workspace` is run as the FIRST cargo invocation inside a fresh
`rust:1` container.

**Not caused by this plan:** neither test touches `CreateSession`, the
`test-fixtures` feature, or any file this plan (27-02) modifies
(`crates/brokerd/src/server.rs`, `crates/brokerd/Cargo.toml`,
`crates/brokerd/tests/uds_ipc.rs`, `cli/caprun/tests/harden04_featureless_create_session.rs`).
Both tests spawn the `caprun` binary, which itself needs to locate the
sibling `caprun-planner` binary via `current_exe().parent().join(...)`; a
bare `cargo test --workspace` in a container with no prior build does not
reliably place that nice-named binary before test binaries run.

**Matches a previously-recorded gotcha:** see
`~/.claude/projects/-Users-benlamm-Workspace-AgentOS/memory/cargo-test-workspace-missing-sibling-binary.md`
— "bare `cargo test --workspace` doesn't reliably place a bin-only sibling
crate's nice-named `target/debug/<bin>` copy; run `cargo build --workspace`
first if code resolves a sibling binary via `current_exe().parent().join(...)`."

**Recommended fix (not applied here, out of scope for 27-02):** run
`cargo build --workspace` before `cargo test --workspace` in
`scripts/mailpit-verify.sh` and/or the CLAUDE.md Linux verification recipe,
or have those two tests build the sidecar explicitly before spawning it.

**Status:** deferred — not fixed by 27-02 (scope boundary: not caused by
this plan's changes; a build-order fix would live in shared verification
tooling — `scripts/mailpit-verify.sh` and/or the CLAUDE.md recipe — not in
any file this plan modifies). Not independently re-verified with a
build-first ordering in this session; flagged from the matching prior
gotcha for whoever picks this up next.
