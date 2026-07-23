---
phase: 32
slug: process-exec-sink-broker-spawned-confined-child
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-17
---

# Phase 32 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **TCB implementation phase.** Real Rust code lands in `crates/{sandbox,brokerd,executor,runtime-core}`
> + a new `cli/caprun-exec-launcher` binary. Enforcement is **Linux-only**
> (`#[cfg(target_os="linux")]`); the dev machine is a Mac. **Mac `cargo test`
> compiles ZERO Linux test targets** — a Mac-green build can hide broken Linux
> sites (the project's `cfg-linux-test-blindness` lesson: 279/279 macOS-green
> once hid 8 broken sites). Therefore this phase's validation has a MANDATORY
> Linux compile-check step, and the full live acceptance is Phase 34.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) + `scripts/check-invariants.sh` (grep gates) |
| **Config file** | Cargo workspace (`Cargo.toml`, `resolver = "3"`) |
| **Quick run command** | `cargo build --workspace && cargo test --workspace --no-fail-fast` (Mac: Linux enforcement tests show 0 — expected) |
| **Full suite command (Linux, authoritative)** | `bash scripts/mailpit-verify.sh` (Colima+Docker, unprivileged, seccomp=unconfined) |
| **Linux compile-check (MANDATORY this phase)** | inside the `rust:1` container: `cargo build --tests --keep-going` — enumerates every `#[cfg(target_os="linux")]` compile error before Phase 34 |
| **Estimated runtime** | Mac unit ~1–2 min; Linux container build+test ~5–15 min |

---

## Sampling Rate

- **After every task commit:** `cargo build --workspace` (Mac) — must compile; then the task's own `cargo test -p <crate> <names>`.
- **After the sink is wired:** `bash scripts/check-invariants.sh` — Gate 1 (no `EffectRequest`), Gate 3 (mint call-site restriction, now MUST include the new `mint_from_exec(` line) must PASS.
- **After the phase's code is complete (before verification):** run the Linux compile-check (`cargo build --tests --keep-going` in the container) — non-negotiable; a Mac-green build is not sufficient evidence.
- **Max feedback latency:** Mac ~2 min; Linux container ~15 min.

---

## Per-Task Verification Map

| Task | Req | Secure Behavior | Test Type | Automated Command | Status |
|------|-----|-----------------|-----------|-------------------|--------|
| exec sink table entries | EXEC-01/04 | `process.exec` in KNOWN_SINKS (command/args/cwd allowed+required); command/args routing+content-sensitive, expected_role=None; CommitIrreversible | unit | `cargo test -p executor sink_schema sink_sensitivity` | ✅ |
| TaintLabel::ExecRaw | EXEC-02 | new variant; `is_untrusted()` + every non-wildcard TaintLabel match updated (compiler-enforced) | unit | `cargo test -p runtime-core` | ✅ |
| mint_from_exec (server.rs locus) | EXEC-02 | mints combined stdout/stderr ValueNode rooted at a new `process_exited` Event; provenance_chain[0] == that Event id (non-stapled); fail-closed unknown-classification | unit | `cargo test -p brokerd mint_from_exec` (mirror `mint_from_read_anchor_identity`) | ✅ |
| Gate 3 extension | EXEC-02 | `check_mint_token "mint_from_exec("` added, same commit | gate | `bash scripts/check-invariants.sh` exits 0 | ✅ |
| caprun-exec-launcher | EXEC-01/04 | new bin: receives target, self-confines post-fork (exec_child_ruleset + exec_child_filter + rlimits), then execve; built by `cargo build --workspace` (sibling-binary gotcha) | integration (linux) | `#[cfg(target_os="linux")]` launcher confinement test (`crates/sandbox/tests/exec_child_confinement.rs`, 4/4 pass in the mandatory Linux container) | ✅ |
| broker spawn + capture + timeout | EXEC-01/04 | broker spawns launcher (never worker execve); captures stdout/stderr; wall-clock `tokio::time::timeout` + child.kill; output byte cap | integration (linux) | `#[cfg(target_os="linux")]` exec spawn test (`crates/brokerd/tests/process_exec_spawn.rs`, 3/3 pass in the mandatory Linux container) | ✅ |
| two-phase durable audit | EXEC-04 | `process_exited`/`process_spawn_failed` Event pair chained on parent_id/parent_hash (mirror file_create.rs) | integration | `cargo test -p brokerd` audit test (same `process_exec_spawn.rs` run, all three assert `verify_chain`) | ✅ |
| EXEC-03 acceptance (genuine taint → I2 Block) | EXEC-03 | exec Event → ValueNode → sensitive sink arg → deterministic Block; unbroken audit-DAG edge; `verify_chain` true (NON-stapled) | e2e (linux) | `#[cfg(target_os="linux")]` s9_process_exec_block.rs::s9_process_exec_genuine_taint_block, 4/4 pass in the mandatory Linux container | ✅ |
| negative test per sink | EXEC-03/04 | clean exec Allowed; confinement escape denied; tainted-command Blocks | e2e (linux) | `s9_process_exec_block.rs` (clean-allow control + tainted-command negative) + `exec_child_confinement.rs` (fs-escape denied, net-deny persists across execve, benign write + legitimate execve succeed) — 8/8 Linux tests pass | ✅ |

*Status: ⬜ pending · ✅ green · ❌ red*

---

## Wave 0 Requirements

- [x] `cli/caprun-exec-launcher/` new crate scaffolding (Cargo.toml + main.rs) added to the workspace members.
- [x] `tokio` `process` feature enabled where the broker spawns/awaits the child (research gap #3).
- [x] `output_value_id: Option<ValueId>` wire field added to the broker decision response (research gap #2) so the worker can route exec-output into a later sink arg (required for EXEC-03).

*Existing test infrastructure (cargo + check-invariants.sh + mailpit-verify.sh) covers the rest.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Exec child is genuinely kernel-confined | EXEC-04 | Requires observing a real Landlock/seccomp denial on Linux, not just a Mac stub | Run the `#[cfg(target_os="linux")]` confinement test in the container; confirm a write outside WorkspaceRoot and an outbound socket both fail |
| Taint chain is genuine, not stapled | EXEC-02/03 | Auditing that provenance_chain[0] is the exec Event (not stapled at the sink) is a design-correctness judgment | Inspect the audit DAG: exec Event → ValueNode → sink arg → Block edge is unbroken; `verify_chain` true |

---

## Validation Sign-Off

- [x] All EXEC-01..04 requirement IDs covered by at least one test (see map)
- [x] `bash scripts/check-invariants.sh` exits 0, INCLUDING the new Gate-3 `mint_from_exec(` line
- [x] Mac `cargo build --workspace` + `cargo test --workspace` green (Linux tests show 0 — expected)
- [x] **Linux compile-check ran**: `cargo build --tests --keep-going` in the `rust:1` container enumerated zero errors (cfg-linux-blindness guard) — via `MAILPIT_VERIFY_CMD='cargo build --workspace && cargo build --tests --keep-going' bash scripts/mailpit-verify.sh`, real exit 0 captured before any pipe
- [x] `nyquist_compliant: true` set in frontmatter
- [x] Full live acceptance (EXEC-03 non-stapled Block, clean Allow) deferred to Phase 34 LIVE-01, but the tests exist, compile, AND RUN GREEN on Linux now (8/8 new Linux tests + 3/3 pre-existing process_exec_spawn.rs, all via `scripts/mailpit-verify.sh`)

**Approval:** 32-06 executor — Linux container verification complete 2026-07-17 (Colima+Docker, rust:1, seccomp=unconfined, via `scripts/mailpit-verify.sh`). Two genuine pre-existing bugs found and fixed in the process (see 32-06-SUMMARY.md Deviations): a landlock 0.4.5 API-mismatch compile error in `exec_child_ruleset`, an `EXEC_CWD=""` chdir("") bug in `run_launcher`, and two Landlock ruleset gaps (missing `ReadDir`/`MakeReg`) discovered running a real `/usr/bin/python3` target through the launcher. Full composed live acceptance (LIVE-01) remains Phase 34.
