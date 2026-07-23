# Stack Research

**Domain:** Intent Runtime — multi-step Safe Coding Agent loop (v1.10) on existing Rust caprun substrate  
**Researched:** 2026-07-23  
**Confidence:** HIGH (in-repo workspace + design docs + live binary layout verified; packaging path cross-checked against Cargo Book `cargo install`)

## Recommended Stack

### Ruling (one sentence)

**v1.10 needs zero new crates.** Multi-node plan streams, a deterministic multi-step coding planner, session-loop orchestration, and design-partner packaging are all pure in-tree Rust over the shipped v1.9 substrate — extend types, the `Planner` seam, the worker submit loop, and a thin install script. Anything that looks like an agent framework, workflow engine, new IPC transport, or packaging toolchain is out of scope and actively harmful under HYG-01.

### Core Technologies (unchanged substrate — keep)

| Technology | Version (workspace pin) | Purpose | Why keep |
|------------|-------------------------|---------|----------|
| Rust edition 2021 | workspace `0.1.0`, resolver `"3"` | TCB language | Locked: TCB is Rust; Python non-TCB only |
| `tokio` | `1.52.3` (`net`, `io-util`, `rt-multi-thread`, `macros`, `time`, `process`) | async broker + process spawn | Already powers `brokerd` accept loop + exec-launcher spawn |
| `serde` / `serde_json` | `1.0.228` / `1.0.150` | framed UDS JSON IPC | Wire format for every `BrokerRequest` / `PlanNode` |
| `rusqlite` | `0.32` (`bundled`) | SQLite audit DAG | Session continuity + `verify_chain` |
| `sha2` + `hmac` + `hex` + `getrandom` | `0.10` / `0.12.1` / `0.4` / `0.4` | keyed MAC audit chain | HARDEN-02; do not replace |
| `nix` | `0.31.3` | fd / process / rlimit primitives | Sandbox + SCM_RIGHTS path |
| `landlock` + `seccompiler` | `0.4.5` / `0.5.0` | kernel confinement | Security boundary — untouched by multi-node |
| `uuid` + `chrono` + `anyhow` + `thiserror` | pinned in workspace | IDs, timestamps, errors | Existing everywhere |
| `reqwest` | `=0.13.4` (`rustls-no-provider` only) | broker-side HTTP egress | git.push / github.pr / http.* — reuse, do not re-add |
| `rustls` | `0.23` (`ring`, `std`, `tls12`; **no** aws-lc-rs) | TLS crypto provider | HYG-01 Gate 5 ring-only |
| `webpki-roots` | `1.0.8` | compiled-in CA store | `env_clear()`-hermetic TLS |
| `url` | `2.5` | SSRF host parse | Already in brokerd |
| `lettre` | `0.11.22` | SMTP (`email.send`) | Unrelated to coding loop; leave alone |
| `libc` | `0.2` | sandbox / test FD redirect | Already present; not a new dep |

### Integration points for v1.10 (no new packages)

| Capability | Where it lands | Existing seam to extend | New crate? |
|------------|----------------|-------------------------|------------|
| Multi-node plan stream type | `runtime-core` + `cli/caprun/src/planner.rs` | `PlanNode` / `PlanArg` / `ValueId` already exist | **No** — return `Vec<PlanNode>` (or a thin newtype) from the planner |
| Multi-step coding intent | `runtime-core/src/intent.rs` | `CaprunIntent` closed enum (today: email + file only) | **No** — add one variant (e.g. `SafeCodingWorkflow { … }`) with trusted literals |
| Sequential submit loop | `cli/caprun/src/worker.rs` | `BrokerRequest::SubmitPlanNode` is already per-node; session identity is connection-scoped (HARD-03) | **No** — loop submit → `PlanNodeDecision` → next node |
| Cross-step handle threading | worker (same process) | `PlanNodeDecision.output_value_id` already mints opaque `process.exec` outputs (32-05); worker currently binds and discards | **No** — keep a `HashMap`/slot table of `ValueId`s between steps |
| Deterministic multi-step planner | `cli/caprun/src/planner.rs` | `trait Planner` + `DeterministicPlanner` | **No** — new `impl Planner` (e.g. `DeterministicCodingPlanner`) mapping one coding intent → ordered nodes over shipped sinks |
| CLI driver | `cli/caprun/src/main.rs` | hand-rolled argv + `caprun run` alias + `--policy` | **No** — new intent-kind string; still no clap |
| Mid-loop Block / confirm | existing `caprun confirm` / `review` / `audit` | single-shot confirm TCB path | **No** — stop stream on non-`Allowed`; surface `effect_id`; operator uses existing verbs |
| Policy for coding sinks | `runtime-core` `SessionPolicy` + broker bind | POLICY-01/02/03 | **No** — trusted JSON allowlist at session create (already) |
| Packaging | `scripts/` + docs | sibling-binary layout via `current_exe().parent()` | **No** — thin `install.sh` + documented env; not a crate |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| *(none new)* | — | — | **Default for all v1.10 work** |
| `std::collections::HashMap` | std | step-output handle table in worker | When routing `output_value_id` into a later `PlanArg` |
| Existing `llm-planner` crate | path | wire types for LLM sidecar | **Only** if a future milestone reopens LLM multi-step; **not** v1.10 |

### Development / Packaging Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo build --release --workspace` | produce co-located sibling binaries | Required: `caprun` resolves `caprun-worker` and `caprun-exec-launcher` via `current_exe().parent()` |
| Thin `scripts/install-linux.sh` (new, shell only) | copy release bins + print env checklist | Prefer this over `cargo install` alone — see Packaging |
| `scripts/check-invariants.sh` | HYG-01 / Gate 1–6 | Re-run after any dep touch; Gate 5 must stay green |
| `scripts/compose-verify.sh` | Linux live proof | Multi-node LIVE proof rides this harness (mock egress + Mailpit) |
| `rustc` / `cargo` 1.89+ (verified on host) | build toolchain | Linux design partner: stable Rust + kernel ≥5.13 |

## Installation

### Build (developer / design partner from source)

```bash
# On Linux (kernel ≥5.13). From repo root.
cargo build --release --workspace

# Production sibling set for the Safe Coding loop (deterministic planner — no LLM sidecar):
#   target/release/caprun
#   target/release/caprun-worker
#   target/release/caprun-exec-launcher
#
# caprun-planner is ONLY required when CAPRUN_PLANNER=llm (out of v1.10 scope).
```

### Install path (recommended — multi-package workspace)

`cli/caprun` is one package with two bins (`caprun`, `caprun-worker`).  
`cli/caprun-exec-launcher` is a **separate** package.  
`cargo install --path cli/caprun` therefore does **not** place the exec launcher next to `caprun`, and the runtime will fail to spawn it.

Recommended install (no new tooling):

```bash
# PREFIX defaults to $HOME/.local
PREFIX="${PREFIX:-$HOME/.local}"
mkdir -p "$PREFIX/bin"
cp -f target/release/caprun \
      target/release/caprun-worker \
      target/release/caprun-exec-launcher \
      "$PREFIX/bin/"
# Ensure $PREFIX/bin is on PATH. Binaries MUST remain co-located (same directory).
```

Optional documented alternative (multi-invocation, still no new crates):

```bash
cargo install --path cli/caprun --locked --root "$PREFIX"
cargo install --path cli/caprun-exec-launcher --locked --root "$PREFIX"
# Partner must keep both installs sharing the same --root/bin so current_exe().parent() works.
```

### Design-partner env surface (document, do not code new config crates)

| Variable / artifact | Role |
|---------------------|------|
| `--policy <path>` / `CAPRUN_POLICY` | trusted session policy JSON (outside workspace root — F1) |
| audit DB path + sibling `.key` | keyed MAC chain (HARDEN-02) |
| `CAPRUN_GITHUB_TOKEN` | broker-env only; github.pr |
| `CAPRUN_GIT_PUSH_TOKEN` | broker-env only; git.push |
| `CAPRUN_SMTP_*` | email regression only; not required for coding loop |
| `WRITE_HOST_ALLOWLIST` / GET allowlists | already broker-side; no new config crate |

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| In-tree `Vec<PlanNode>` stream + worker loop | New IPC verb `SubmitPlanStream { nodes }` | Only if batch atomicity is required; **not** for v1.10 — sequential per-node decisions + audit edges are the product |
| Extend `trait Planner` return type | Separate `MultiStepPlanner` trait / crate | Avoid — one seam (`Planner`) already exists (PLANNER-01); keep swappable behind one trait |
| Deterministic coding planner in `cli/caprun` | New `crates/coding-planner` package | Avoid for v1.10 — planner is not TCB enforcement; living next to `DeterministicPlanner` is enough |
| Hand-rolled argv | `clap` / `argh` / `pico-args` | Never for v1.10 — CLI already has a working parser pattern; clap is a non-essential dep against HYG-01 |
| `scripts/install-linux.sh` + co-located bins | `cargo-dist` / `cargo-deb` / snap / flatpak | Later GTM only; overkill for 1–3 Linux design partners |
| Existing `caprun confirm` second command | Interactive TTY confirm mid-worker | Avoid — confirm is deliberately non-interactive-friendly and testable |
| Reuse `reqwest`/`rustls`/ring stack | New HTTP client or git2/libgit2 | Forbidden — HYG-01; git.push is already broker smart-HTTP without libgit2 |
| Worker-side sequential submit | Parent-orchestrated N× `caprun run` | Rejected by milestone goal — one Session, one audit DAG, policy bound once |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **Any new crates.io dependency** (default posture) | HYG-01 (v1.9): zero new crates unless supply-chain + Gate 5 re-proven; multi-node needs none | In-tree Rust on existing workspace deps |
| **Agent frameworks** (LangChain, AutoGen, rig, llm-chain, etc.) | Product boundary: Intent Runtime, **not** an agent framework (DEC-product-boundary) | Deterministic `Planner` impl + broker-mediated plan nodes |
| **Workflow engines** (Temporal, Cadence, DAG libs) | Wrong abstraction; session/audit already is the durable log | Worker loop + SQLite audit DAG |
| **`EffectRequest` / raw effect→sink path** | Architectural lock; Gate 1 fails the build | `SubmitPlanNode { plan_node }` only |
| **gRPC / protobuf / cap'n proto plan streams** | New wire stack; existing framed JSON UDS is sufficient | Existing 4-byte LE + JSON framing |
| **clap / structopt / argh** | Non-essential dep; argv surface is small and intentional | Hand-rolled parse in `main.rs` |
| **Cedar / OPA / Rego** | Policy language deferred; I2 must stay Rust TCB | Existing hardcoded-schema `SessionPolicy` JSON |
| **libgit2 / git2-rs / gix as push engine** | Would re-open child-egress / TCB surface v1.9 deliberately avoided | Broker-performed smart-HTTP (shipped) |
| **aws-lc-rs / native-tls / openssl for new paths** | Gate 5 / HYG-01; resolver-3 feature unification pulls C into broker TCB | `rustls-no-provider` + ring + webpki-roots only |
| **cargo-dist, cargo-deb, nfpm, Docker-as-product** | Packaging polish ahead of partner signal; Docker is a **verify harness**, not the product | Release bins + install script + env doc |
| **LLM multi-step tool-use loop libraries** | Explicitly deferred past v1.10; v1.4 single-shot sidecar remains | Deterministic multi-step coding planner first |
| **New mint sites / new sink families** | Not this milestone; sinks already cover edit→test→commit→push→PR | Wire existing sinks into one plan stream |
| **Python in TCB or planner loop** | CON-stack-tcb | Rust only |
| **Web UI / marketplace / memory products** | Out of scope forever for this product class | `caprun audit` CLI viewer |
| **gVisor / Firecracker** | Boundary remains Landlock+seccomp+namespaces | Unchanged sandbox crate |
| **Cross-host / Biscuit / federation** | v3 concern | Single-host Session |

## Stack Patterns by Variant

**If the run is the Safe Coding success path (deterministic multi-step):**
- Use a new `CaprunIntent` coding variant + `DeterministicCodingPlanner` → `Vec<PlanNode>`.
- Worker submits nodes sequentially on **one** broker connection / **one** Session.
- Thread `output_value_id` only as opaque `ValueId`s (never raw exec bytes — I1).
- Do **not** spawn `caprun-planner` (`CAPRUN_PLANNER` unset).
- Because: proves CLI-driven multi-node continuity without LLM nondeterminism.

**If a mid-loop I2 Block is the LIVE negative leg:**
- Same stack; hostile/tainted value is routed into a sensitive arg by the deterministic planner (call-site convention, not planner intelligence).
- Worker exits non-zero on non-`Allowed`; parent surfaces `effect_id` + `caprun review`/`confirm`/`deny` pointers (existing).
- Do **not** add auto-resume frameworks.
- Because: the confirm surface is already the product; a resume engine is new TCB.

**If a CommitIrreversible step is confirm-gated (e.g. `git.push` always confirm-gated):**
- Keep existing confirm path; multi-node stream **stops** at `BlockedPendingConfirmation`.
- Design-partner UX = run → review → confirm/deny; optional later “continue remaining plan” is a **separate** product decision, not a new crate.
- Because: single-shot confirm semantics are locked; standing multi-step waivers are out of scope.

**If LLM multi-step tool-use is pulled later (post-v1.10):**
- Reuse `llm-planner` + `caprun-planner` sidecar + `CAPRUN_PLANNER=llm`.
- Still no agent framework; still plan nodes only; still I2 in executor.
- Because: v1.4 already proved the adversarial single-shot boundary.

**If packaging for a partner who cannot build from source:**
- Ship a tarball of the three co-located release binaries + a one-page env/credentials doc.
- Still no cargo-dist required for v1.10.
- Because: three static-ish Linux bins + documented env is the entire install surface.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `reqwest =0.13.4` (`rustls-no-provider`) | `rustls 0.23` + ring provider + `webpki-roots 1.0.8` | Do not flip to `features = ["rustls"]` — pulls aws-lc-rs via resolver-3 |
| `brokerd` HTTP/git.push egress | same stack as above | Multi-node coding loop reuses sinks; no version bump needed |
| `cli/caprun` bins | `caprun-worker` + `caprun-exec-launcher` same release dir | `current_exe().parent()` resolution; mixed install roots break process.exec/git.commit |
| `Planner::plan` signature change | all `impl Planner` + worker call site | Coordinate trait return type (`PlanNode` → `Vec<PlanNode>`) in one change set; keep PLAN-03 (handles only) |
| `CaprunIntent` new variant | broker `ProvideIntent` mint arms + planner match | Exhaustive matches will force updates — good fail-closed |
| `SessionPolicy` allowlist | coding sinks: `file.write`, `process.exec`, `git.commit`, `git.push`, `github.pr` (+ optional `http.*`) | Policy never overrides I2 |
| Kernel | ≥5.13 Landlock | Unchanged Linux-only floor |
| `mock-egress-ca` feature | compose-verify only | Never default; Gate 4b |

## Concrete stack delta for roadmap authors

### Add (code only, zero Cargo.toml deps)

1. **`CaprunIntent` coding variant** with trusted fields needed to mint plan-arg handles (paths, test command, commit message, remote/refspec, PR title/body — exact field set is a design-gate item, not a crate choice).
2. **`Planner` multi-node return** — prefer `fn plan(...) -> Vec<PlanNode>` (or add `fn plan_stream(...)` defaulting single-node impls to `vec![plan(...)]` for email/file back-compat).
3. **`DeterministicCodingPlanner`** — hardcoded ordered sinks over the shipped effect surface; pure, infallible, handle-only.
4. **Worker multi-submit loop** — for each node: `SubmitPlanNode` → require `Allowed` to continue; stash `output_value_id` for later args; exit 1 on Block/Denied with existing messaging.
5. **CLI intent-kind** — e.g. `safe-coding-workflow` under `caprun run`, still hand-rolled argv + `--policy`.
6. **`scripts/install-linux.sh` + short partner doc** — copy three bins, print env checklist.

### Do not add (Cargo.toml / product)

- No new workspace members for “orchestration.”
- No new `[dependencies]` lines unless a design-gate proves an impossibility with std + existing crates (none identified).
- No LLM multi-step in this milestone.
- No packaging framework crates or CI release plugins as a v1.10 requirement.

## Sources

- In-repo workspace pins: `/home/ben/Workspace/caprun/Cargo.toml`, `crates/*/Cargo.toml`, `cli/*/Cargo.toml` — verified 2026-07-23 [confidence: HIGH]
- Planner seam + single-node submit: `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs` [confidence: HIGH]
- Broker IPC multi-submit readiness: `crates/brokerd/src/proto.rs` (`SubmitPlanNode`, `PlanNodeDecision.output_value_id`) [confidence: HIGH]
- HYG-01 zero-new-crate + ring-only: `planning-docs/DESIGN-v1.9-egress-policy.md` §3; `scripts/check-invariants.sh` Gate 5 [confidence: HIGH]
- Product boundary + v1.10 scope: `.planning/PROJECT.md` (Current Milestone v1.10); `planning-docs/PLAN.md`; `planning-docs/CANDIDATE-v1.7plus-productization-sketch.md` §2 D1 packaging [confidence: HIGH]
- LIVE-05 hybrid honesty (why multi-node CLI is the gap): `cli/caprun/tests/live_acceptance_v1_9_composed.rs` module docs [confidence: HIGH]
- Cargo multi-binary install semantics: [Cargo Book — cargo-install](https://doc.rust-lang.org/cargo/commands/cargo-install.html) (installs bins of **one** package; sibling packages need separate install or a copy script) [confidence: HIGH for install mechanics]

---
*Stack research for: caprun v1.10 Multi-step Safe Coding Agent Loop*  
*Researched: 2026-07-23*  
*Mode: ecosystem / stack-delta (subsequent milestone — not greenfield)*
