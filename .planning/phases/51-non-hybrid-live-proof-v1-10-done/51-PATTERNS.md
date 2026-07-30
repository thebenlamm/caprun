# Phase 51: Non-hybrid LIVE Proof (v1.10 DONE) - Pattern Map

**Mapped:** 2026-07-29
**Files analyzed:** 4 (1 NEW + 3 MODIFY)
**Analogs found:** 4 / 4

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cli/caprun/tests/live_acceptance_v1_10_cli.rs` | test | request-response + event-driven (CLI e2e, external confirm sidecar) | `cli/caprun/tests/live_acceptance_v1_9_composed.rs` + `cli/caprun/tests/coding_cli.rs` | exact (layout/helpers) + role-match (coding argv) |
| `cli/caprun/src/planner.rs` | service (planner trait impl) | transform (plan_next → PlanNode) | `cli/caprun/tests/planner.rs` `CodingI2ProofPlanner` (lines 1313–1377) | exact (promote test-only type) |
| `cli/caprun/src/worker.rs` | controller (worker main loop) | request-response (env → planner selection) | same file planner selection (lines 359–389) | exact (extend match) |
| `cli/caprun/src/main.rs` | controller (orchestrator spawn) | request-response (env_clear allowlist) | same file `worker_cmd` + `worker_planner_env` (lines 600–642, 541–591) | exact (mirror CAPRUN_PLANNER forward) |

**Regression-only references (do not claim LIVE-07/08 DONE):**

| File | Role for Phase 51 |
|------|-------------------|
| `cli/caprun/tests/stream_substrate.rs` | Genuine taint assert template (`taint_via_bag_*`) |
| `crates/brokerd/tests/s46_negative_legs_composed.rs` | I2 vs `policy_deny` distinct-tag pattern |
| `cli/caprun/tests/coding_cli.rs` | Host argv / F1 layout / intent fixture (not multi-node SUCCESS) |
| `cli/caprun/tests/live_acceptance_v1_9_composed.rs` | Hybrid SUCCESS template — **invert framing** for v1.10 |

## Pattern Assignments

### `cli/caprun/tests/live_acceptance_v1_10_cli.rs` (test, CLI e2e)

**Analog (primary):** `cli/caprun/tests/live_acceptance_v1_9_composed.rs`  
**Analog (coding fixtures):** `cli/caprun/tests/coding_cli.rs`  
**Analog (genuine taint asserts):** `cli/caprun/tests/stream_substrate.rs`  
**Analog (I2 vs policy_deny tags):** `crates/brokerd/tests/s46_negative_legs_composed.rs`

#### Framing honesty (INVERT v1.9 hybrid language)

**Source:** `live_acceptance_v1_9_composed.rs` lines 1–45 — **anti-pattern for DONE claim**.  
Phase 51 module docs must **invert** this: multi-node SUCCESS is driven by real `caprun run safe-coding-workflow` one Session; hybrid `evaluate_plan_node_and_record_for_test` is **not** the claim.

```rust
// LIVE-07 framing pins (machine-checkable — reference from success test body)
const LIVE_07_DRIVER: &str =
    "caprun run safe-coding-workflow (CLI multi-node, one Session)";
const LIVE_07_NOT: &str =
    "hybrid in-crate evaluate_plan_node_and_record_for_test composition";
```

**Module doc recipe (from RESEARCH Pattern 3 + v1.9 invert):**

1. State SUCCESS is real CLI multi-node, one Session.
2. Explicitly name hybrid v1.9 composition as **not** this claim.
3. Document compose-verify recipe with `CARGO_BIN_EXE` sibling build.

**cfg + host guard pattern** (v1.9 lines 85–83, 59–68; coding_cli / s9 guards):

```rust
// Host-safe always-on guard (macOS / no Docker): binary resolves.
#[test]
fn live_acceptance_v1_10_cli_guard_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(!caprun_bin.is_empty());
}

// LIVE body only on Linux + mock-egress-ca (compose-verify).
#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]
mod linux { /* LIVE-07 + LIVE-08 */ }
```

**compose-verify recipe** (copy structure from v1.9 lines 77–79):

```bash
COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun \
  --test live_acceptance_v1_10_cli --features brokerd/mock-egress-ca' \
  bash scripts/compose-verify.sh
```

#### F1 layout + coding fixtures

**Source:** `coding_cli.rs` lines 19–100 (`CodingLayout`, `CODING_INTENT_JSON`, `MINIMAL_POLICY_JSON`).

```19:52:cli/caprun/tests/coding_cli.rs
const CODING_INTENT_JSON: &str = r#"{
  "kind": "SafeCodingWorkflow",
  "path": "src/hello.txt",
  "contents": "hello from caprun\n",
  "test_command": "sh",
  "test_args_json": "[\"-c\", \"git add -A && true\"]",
  "commit_message": "caprun: safe coding demo",
  "remote": "origin",
  "refspec": "HEAD:refs/heads/caprun-demo",
  "owner": "acme",
  "repo": "demo",
  "base": "main",
  "head": "caprun-demo",
  "pr_title": "caprun safe coding demo",
  "pr_body": "Opened by multi-node stream"
}"#;

const MINIMAL_POLICY_JSON: &str = r#"{
  "allowed_sinks": [
    "email.send",
    "file.create",
    "file.write",
    "process.exec",
    "git.commit",
    "git.push",
    "github.pr",
    "http.request",
    "http.request.write"
  ],
  "arg_constraints": {}
}"#;
```

**LIVE-07 adaptations (from RESEARCH):**

| Field | coding_cli host | LIVE-07 compose |
|-------|-----------------|-----------------|
| `remote` | `"origin"` | `"https://github-mock.caprun.test/accept/repo.git"` |
| `refspec` | demo branch | e.g. `HEAD:refs/heads/caprun-live-07` |
| tokens | none | `CAPRUN_GIT_PUSH_TOKEN`, `CAPRUN_GITHUB_TOKEN` |
| confirm | none | `CAPRUN_CONFIRM=external`, short `CAPRUN_CONFIRM_TIMEOUT_SECS` |

**F1-safe structure** (coding_cli `CodingLayout::new` + v1.9 seed_test_key):

```
tmp/
├── audit.db + audit.db.key   # siblings of workspace (never under WorkspaceRoot)
├── policy.json
├── coding-intent.json
└── workspace/
    ├── workspace.txt
    ├── src/hello.txt         # pre-create for file.write O_TRUNC
    └── .git/                 # git init + identity (see setup_git_push_repo)
```

**Git fixture pattern** — `live_acceptance_v1_9_composed.rs` lines 240–286:

```240:286:cli/caprun/tests/live_acceptance_v1_9_composed.rs
    fn git_in(dir: &Path, args: &[&str]) -> (bool, String) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("failed to spawn system git");
        // ...
    }

    fn setup_git_push_repo(tmp: &Path, tag: &str) -> (PathBuf, Arc<WorkspaceRoot>) {
        // git init; add; commit; branch -M main
        // ...
    }
```

For LIVE coding: workspace **is** the git repo; `test_command` should stage (`git add -A`) so `git.commit` has content. Pre-create write target path.

#### CLI spawn + audit subprocess

**Source:** v1.9 `run_caprun` / `assert_audit_passed` (lines 342–388); coding_cli `caprun_bin` + argv (lines 102–111, 273–280).

```342:388:cli/caprun/tests/live_acceptance_v1_9_composed.rs
    fn run_caprun(args: &[&str]) -> (i32, Vec<u8>, String) {
        let caprun_bin = env!("CARGO_BIN_EXE_caprun");
        let output = Command::new(caprun_bin)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("spawn caprun {args:?}: {e}"));
        // ...
    }

    fn assert_audit_passed(session_id: &str, db: &str) -> String {
        let (code, stdout_bytes, stderr) = run_caprun(&["audit", session_id, db]);
        // assert code == 0 && stdout.contains("Chain verification: PASSED")
        // ...
    }
```

**LIVE-07 driver argv (locked Phase 50):**

```text
caprun run --policy <policy.json> \
  safe-coding-workflow <coding-intent.json> <workspace-file> <audit.db>
```

**Spawn with env + piped stdio** (sidecar needs stdout lines):

```rust
let mut child = Command::new(env!("CARGO_BIN_EXE_caprun"))
    .args([
        "run",
        "--policy", layout.policy.to_str().unwrap(),
        "safe-coding-workflow",
        layout.intent.to_str().unwrap(),
        layout.workspace_file.to_str().unwrap(),
        layout.audit_db.to_str().unwrap(),
    ])
    .env("CAPRUN_CONFIRM", "external")
    .env("CAPRUN_CONFIRM_TIMEOUT_SECS", "60")
    .env("CAPRUN_GIT_PUSH_TOKEN", "test-push-token-not-for-audit")
    .env("CAPRUN_GITHUB_TOKEN", "ghp_test_not_for_audit")
    // CAPRUN_GITHUB_API_BASE set by compose-verify
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn caprun run safe-coding-workflow");
```

**Parse `session_id=`** — main prints at lines 448–449:

```448:449:cli/caprun/src/main.rs
        println!("session_id={session_id}");
        println!("grant: caprun grant {session_id} {audit_path}");
```

**Parse `effect_id=`** — v1.9 `extract_surfaced_effect_id` (lines 357–370):

```357:370:cli/caprun/tests/live_acceptance_v1_9_composed.rs
    fn extract_surfaced_effect_id(stdout: &str) -> String {
        for line in stdout.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("effect_id=") {
                return rest
                    .split_whitespace()
                    .next()
                    .expect("effect_id= line must carry a value")
                    .to_string();
            }
        }
        panic!("no `effect_id=` surface line in caprun run stdout:\n{stdout}");
    }
```

#### External confirm + grant sidecar

**Source product path:** `main.rs` `confirm_mode_is_external` + `resolve_external_hold` (lines 823–828, 1015–1064).

```823:828:cli/caprun/src/main.rs
fn confirm_mode_is_external() -> bool {
    if std::env::var("CAPRUN_CONFIRM").as_deref() == Ok("external") {
        return true;
    }
    !std::io::stdin().is_terminal()
}
```

```1025:1037:cli/caprun/src/main.rs
    let timeout_secs = std::env::var("CAPRUN_CONFIRM_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(EXTERNAL_CONFIRM_DEFAULT_TIMEOUT_SECS);
    // ...
    // Use: caprun confirm {effect_id} {audit_path}
```

**Sidecar algorithm (RESEARCH Pattern 2 — new helper in test file):**

1. Background thread reads child stdout lines.
2. On `session_id=<uuid>` → once: `Command::new(caprun).args(["grant", &sid, audit_db])`.
3. On `effect_id=…` with `sink=git.push` (or parse BLOCKED surface) → `caprun confirm <effect_id> <audit.db>` **only for push** on LIVE-07.
4. LIVE-08: do **not** confirm I2-blocked `github.pr` (no effect of that node).
5. Join sidecar; assert child exit: LIVE-07 → `0`; LIVE-08 → `3` (blocked) or `2` (deny), never `0` as SUCCESS.

**Do not** dual-Session stitch; grant is session-scoped concurrent with early nodes.

#### LIVE-07 success asserts

| Assert | Pattern source |
|--------|----------------|
| Exit 0 | CLI-02 / Phase 50 stream terminal |
| Exactly one coding `session_id` | Framing SC2; parse stdout once |
| `caprun audit` → `Chain verification: PASSED` | v1.9 `assert_audit_passed` |
| Terminal events `git_push_succeeded`, `github_pr_succeeded` | v1.9 `count_events` / `find_event_by_type` |
| Argv contains `safe-coding-workflow` | Framing machine-check |
| No `evaluate_plan_node_and_record_for_test` in this test | Anti-hybrid |

#### LIVE-08 I2 asserts (sibling Session)

**Genuine taint template** — `stream_substrate.rs` lines 915–1001 (in-process ValueStore; CLI LIVE prefers DAG-visible edges):

```915:1001:cli/caprun/tests/stream_substrate.rs
        // Anti-stapling: process_exited is durably in the DAG before mint.
        let dag_event =
            find_event_by_type(&locked, &session_id.to_string(), "process_exited")
                .expect("query process_exited")
                .expect("process_exited must exist");
        // ...
        assert_eq!(
            minted.provenance_chain,
            vec![exec_event_id],
            "mint_from_exec provenance_chain must be exactly [process_exited]"
        );
        // ...
        assert_eq!(
            anchor.provenance_chain[0], exec_event_id,
            "GENUINE-TAINT BACKSTOP: provenance_chain[0] must equal process_exited id"
        );
```

**CLI LIVE-08 (DAG-level, no in-process ValueStore):**

1. Same CLI harness + `.env("CAPRUN_CODING_I2_PROOF", "1")`.
2. Confirm only mid-loop **git.push** (always-confirm); leave **github.pr** I2 Block unconfirmed.
3. Assert `process_exited` exists in Session (real exec mint root).
4. Assert `sink_blocked` present; **zero** `github_pr_succeeded` / completed PR write.
5. Policy **permits** `github.pr` (`MINIMAL_POLICY_JSON` includes it) so Block ≠ `policy_deny`.
6. Distinct-tag pattern from s46 module doc (lines 9–29): I2 → `sink_blocked`; policy_deny → `code()=="policy_deny"` + **no** `sink_blocked`.
7. `verify_chain` true / `caprun audit` PASSED.
8. Exit blocked/deny (3 or 2), not 0.

---

### `cli/caprun/src/planner.rs` (service, transform)

**Analog:** `cli/caprun/tests/planner.rs` lines 1313–1377 (`CodingI2ProofPlanner`) + product `DeterministicPlanner` / `plan_coding_next` (src lines 206–331).

**Promote test-only type into product module (default-off selection only):**

```1313:1377:cli/caprun/tests/planner.rs
/// Test-only proof planner for LIVE-08 expressibility (CODE-02).
///
/// Mirrors the five-node success recipe but at `github.pr` places bag `out_1`
/// into PlanArg `body`. **Not** product code — never selected by the worker.
struct CodingI2ProofPlanner;

impl planner::Planner for CodingI2ProofPlanner {
    fn plan(/* ... */) -> PlanNode {
        unreachable!("CodingI2ProofPlanner uses plan_next only")
    }

    fn plan_next(&self, ctx: &planner::PlanStreamContext) -> Option<PlanNode> {
        match ctx.step_index {
            0..=3 => planner::Planner::plan_next(&planner::DeterministicPlanner, ctx),
            4 => {
                let h = |key: &str| ctx.handles.get(key).cloned();
                Some(PlanNode {
                    sink: SinkId("github.pr".into()),
                    args: vec![
                        // owner/repo/base/head/title from pr_* intent keys
                        PlanArg {
                            name: "body".into(),
                            value_id: h("out_1")?,
                        },
                        // ...
                    ],
                })
            }
            _ => None,
        }
    }
}
```

**Update module doc** (src `planner.rs` lines 90–103): after promote, document env-gated product selection (`CAPRUN_CODING_I2_PROOF=1`), still **not** success-path, still **not** LIVE DONE by itself.

**Do not change** `plan_coding_next` success path (lines 244–331) — never place `out_*` (CODE-02). Keep anti-launder unit test in `tests/planner.rs` green.

**Unit regression to keep:** `coding_i2_proof_places_out_handle` (tests lines 1230–1311) — retarget import to product `CodingI2ProofPlanner` once moved.

---

### `cli/caprun/src/worker.rs` (controller, request-response)

**Analog:** same file planner selection (lines 359–389).

**Current pattern:**

```382:389:cli/caprun/src/worker.rs
    let planner: Box<dyn Planner> = match std::env::var("CAPRUN_PLANNER").as_deref() {
        Ok("llm") => {
            let planner_sock = std::env::var("PLANNER_SOCK")
                .context("PLANNER_SOCK required when CAPRUN_PLANNER=llm")?;
            Box::new(crate::planner::LlmPlanner::new(planner_sock))
        }
        _ => Box::new(crate::planner::DeterministicPlanner),
    };
```

**Phase 51 extension (RESEARCH Pattern 4 sketch):**

```rust
let planner: Box<dyn Planner> = match (
    std::env::var("CAPRUN_PLANNER").as_deref(),
    std::env::var("CAPRUN_CODING_I2_PROOF").as_deref(),
    &intent,
) {
    (Ok("llm"), _, CaprunIntent::SafeCodingWorkflow { .. }) => {
        anyhow::bail!("SafeCodingWorkflow unsupported on LlmPlanner");
    }
    (Ok("llm"), _, _) => {
        let planner_sock = std::env::var("PLANNER_SOCK")
            .context("PLANNER_SOCK required when CAPRUN_PLANNER=llm")?;
        Box::new(crate::planner::LlmPlanner::new(planner_sock))
    }
    (_, Ok("1"), CaprunIntent::SafeCodingWorkflow { .. }) => {
        Box::new(crate::planner::CodingI2ProofPlanner)
    }
    _ => Box::new(crate::planner::DeterministicPlanner),
};
```

**Bag bag already stores `out_{step}`** on Allowed (worker docs lines 42–44) — no bag change for LIVE-08; proof planner only **places** `out_1` into `body`.

**Hold path** (SafeCodingWorkflow Block-and-Hold ~472–513) — **no change** expected; LIVE-08 holds on I2 Block at PR the same way push holds on always-confirm.

---

### `cli/caprun/src/main.rs` (controller, env allowlist)

**Analog:** `worker_cmd` env_clear allowlist (lines 600–642) + LLM planner env forward (lines 541–591).

**Security pattern — only explicit non-secret keys after `env_clear()`:**

```615:642:cli/caprun/src/main.rs
    worker_cmd
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin")
        .env("BROKER_SOCK", format!("/agentos/{session_id}"))
        .env("WORKSPACE_FILE", workspace_rel)
        .env(
            "INTENT",
            serde_json::to_string(&intent).context("serialise intent")?,
        )
        .env(
            "PRIMARY_SEED_FILE_DERIVED",
            if primary_file_derived { "1" } else { "0" },
        )
        .envs(worker_planner_env);
```

**Phase 51 addition (mirror `CAPRUN_PLANNER=llm` forward):**

```rust
// After building worker_cmd base allowlist (or fold into worker_planner_env):
if std::env::var("CAPRUN_CODING_I2_PROOF").as_deref() == Ok("1") {
    worker_cmd.env("CAPRUN_CODING_I2_PROOF", "1");
}
// NEVER forward CAPRUN_GIT_PUSH_TOKEN / CAPRUN_GITHUB_TOKEN / OPENAI_API_KEY to worker
```

Tokens stay on **parent/broker** process only (opaque audit — v1.9 `assert_no_bearer_token_anywhere`).

**Coding stream orchestration** (`orchestrate_coding_stream` lines 835–913) — **no change** for LIVE-07 SUCCESS; external confirm already productized.

---

## Shared Patterns

### Framing honesty (LIVE-07 SC2)

**Source:** Invert `live_acceptance_v1_9_composed.rs` §FRAMING HONESTY; ROADMAP Phase 51 SC2.  
**Apply to:** `live_acceptance_v1_10_cli.rs` module docs + asserts.

- SUCCESS driver must spawn `CARGO_BIN_EXE_caprun` with argv `safe-coding-workflow`.
- Forbid hybrid `evaluate_plan_node_and_record_for_test` as DONE driver.
- One Session for multi-node chain (no dual-Session stitch).
- Keep v1.9 composed tests as **regression only**.

### env_clear allowlist (ENV-01)

**Source:** `main.rs` lines 600–642.  
**Apply to:** any new worker env key (`CAPRUN_CODING_I2_PROOF` only; non-secret).

### External mid-loop confirm (CONFIRM-01)

**Source:** `main.rs` `CAPRUN_CONFIRM=external` + `resolve_external_hold`.  
**Apply to:** LIVE-07/08 harness sidecars — concurrent `caprun confirm` / `caprun grant`.

### Genuine taint (non-stapled)

**Source:** `stream_substrate.rs` `taint_via_bag_exec_output_blocks_with_genuine_provenance`; broker `mint_from_exec`.  
**Apply to:** LIVE-08 asserts — `process_exited` in Session before Block; no test-local Untrusted mint.

### I2 vs policy_deny distinct tags

**Source:** `s46_negative_legs_composed.rs` module doc + LEG 1/3.  
**Apply to:** LIVE-08 — permitting policy for `github.pr`; assert `sink_blocked`; not `policy_deny`.

### F1 workspace layout

**Source:** `coding_cli.rs` `CodingLayout`; v1.9 seed_test_key / sibling policy.  
**Apply to:** LIVE fixtures — audit.db + key + policy as siblings of workspace root.

### Success-path anti-launder (CODE-02)

**Source:** `plan_coding_next` never places `out_*`; `coding_i2_proof_places_out_handle` success-path assert.  
**Apply to:** product `DeterministicPlanner` unchanged; proof planner only under env gate.

### Linux + mock-egress-ca gate

**Source:** CLAUDE.md; compose-verify; v1.9 cfg.  
**Apply to:** LIVE bodies `#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]`; host guard always on.

### Zero new crates / Gate 1

**Source:** HYG-02; `check-invariants.sh`.  
**Apply to:** Phase 51 — no new crates, no `EffectRequest`, no new mint sites.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| *(none for core deliverables)* | — | — | Concurrent grant+confirm sidecar is **new composition** of existing `caprun grant` / `caprun confirm` CLIs — copy spawn patterns from v1.9 `run_caprun` + main external hold docs; no prior single helper with both. |

**Partial novelty:** `drive_external_confirm_and_grant` helper has no identical prior function — implement by composing:

- stdout line parse (`session_id=`, `effect_id=`) from main + v1.9 extractors  
- subprocess `caprun grant` / `caprun confirm` like `run_caprun`  
- product external poll semantics from `resolve_external_hold`

## Metadata

**Analog search scope:** `cli/caprun/src/{main,worker,planner,stream_hold}.rs`, `cli/caprun/tests/{live_acceptance_v1_9_composed,coding_cli,stream_substrate,planner}.rs`, `crates/brokerd/tests/s46_negative_legs_composed.rs`, `scripts/compose-verify.sh` (cited)  
**Files scanned:** ~12 primary analogs  
**Pattern extraction date:** 2026-07-29  

**Planner must-not (from RESEARCH):**

- Do not claim LIVE-07 via hybrid multi-leg composition  
- Do not place `out_*` on DeterministicPlanner success path  
- Do not auto-grant / auto-confirm / session-wide waiver  
- Do not dual-Session stitch push→PR  
- Do not add crates / EffectRequest / new mint sites  
- Do not implement PKG-01 (Phase 52)
