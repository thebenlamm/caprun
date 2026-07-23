# Phase 30: Regression & Live Proof - Research

**Researched:** 2026-07-17
**Domain:** Test-suite consolidation / live-proof verification (no new TCB feature surface)
**Confidence:** HIGH

## Summary

Phase 30 is NOT a feature-build phase — it is a proof-consolidation phase for the single
requirement HARDEN-06. Direct codebase inspection (not web research) is the correct method
here, and it produces a decisive result: **4 of 5 success criteria already have a dedicated,
passing negative/proof test, written in the residual's own origin phase (27, 28, 29).** Only
one criterion (HARDEN-04, forced-Active `CreateSession` compile-exclusion) has a genuine
proof gap — not a missing test, but a missing **formalized invocation path**: the existing
negative test self-detects Cargo's workspace-wide feature unification and SKIPS its own
assertion when run under the bare `scripts/mailpit-verify.sh` recipe's default
`cargo test --workspace`. The DESIGN doc (`planning-docs/DESIGN-security-hardening.md` §j)
already anticipated this and pins the fix: Phase 30 must add an explicit, scoped,
featureless-build verification step, distinct from the default `cargo test --workspace` run.

**Primary recommendation:** Do NOT write four new negative tests. Write ONE new script
artifact that formalizes the already-designed-but-not-yet-automated HARDEN-04 featureless
proof (a featureless `cargo build --workspace --release` + a scoped `cargo test -p caprun
--test harden04_featureless_create_session` run), run a regression-fixture audit sweep across
all three hardening phases' test files, then execute the bare `scripts/mailpit-verify.sh`
recipe unmodified on real Linux as the full-workspace regression proof, and record all 5
criteria's evidence plus human sign-off. This is a 2-plan, 2-wave phase, not a 3-4 plan
feature-build phase.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Full-workspace regression proof | CI/verification tooling (`scripts/mailpit-verify.sh`) | — | Orchestrates the existing Rust test harness against a real Linux container; no application-tier code |
| Audit-chain tamper proof (HARDEN-02) | `crates/brokerd/src/audit.rs` (TCB) | Test tier (`#[cfg(test)]` unit tests, same file) | Already-shipped unit tests directly exercise `verify_chain`/`compute_event_hash` in the TCB crate |
| Replay-CAS proof (HARDEN-03) | `crates/brokerd` integration test tier (`tests/replay_cas.rs`) | Broker dispatch (`server.rs`) | Drives the real `run_broker_server` over a live UDS + real SMTP (Mailpit) — an integration-tier proof of a TCB mechanism |
| CreateSession compile-exclusion proof (HARDEN-04) | Build/verification tooling (Cargo feature graph + `scripts/mailpit-verify.sh`) | `cli/caprun` integration test tier | The mechanism itself is a `#[cfg]` compile-time exclusion in `crates/brokerd/src/server.rs`; PROVING its absence is a build-graph concern, not a runtime application concern — this is why the existing test self-skips under ambient feature unification and needs a scoped invocation |
| fd-release demotion proof (HARDEN-01) | `crates/brokerd` integration test tier (`tests/harden01_session_integrity.rs`) | Session lifecycle (`server.rs`) | Already-shipped, drives real `dispatch_request` calls |
| Regression-fixture audit (no silently-weakened test) | Human/agent code review of test tier | — | Pure text audit; no runtime component |

## Package Legitimacy Audit

Not applicable. This phase adds no new external dependency (no new crate, no new npm/pip
package). All work is new test/script code plus documentation. `Cargo.lock`/`Cargo.toml`
are not expected to change.

## Standard Stack

No new libraries. This phase uses only what's already in the workspace:

| Tool | Version (verified) | Purpose |
|------|---------------------|---------|
| `cargo test` (Rust workspace, resolver "3", edition 2021) | Toolchain-pinned; `cargo 1.92.0` confirmed present in this dev environment `[VERIFIED: cargo --version]` | Full-suite + scoped test execution |
| `scripts/mailpit-verify.sh` | Project-internal, in-repo since Phase 13 | Bare-recipe Linux regression runner (Colima+Docker+Mailpit sidecar) |
| `axllent/mailpit` (Docker image) | Pinned by the script (no explicit version tag — pulls `latest`) `[CITED: scripts/mailpit-verify.sh]` | SMTP capture sidecar required because a benign Allowed `email.send` performs a live send (Phase 16 CONTROL-01) |
| Colima + Docker | Colima present (`colima status` confirms VM running, `arch: aarch64`), Docker `29.6.1` via colima socket `[VERIFIED: colima status / docker info]` | Real-Linux execution substrate for the dev Mac |

**Installation:** none required — no new packages this phase.

## Architecture Patterns

### System Architecture Diagram

```
                    ┌─────────────────────────────────────┐
                    │   scripts/mailpit-verify.sh (bare)   │
                    │   default MAILPIT_VERIFY_CMD =       │
                    │   "cargo build --workspace &&        │
                    │    cargo test --workspace             │
                    │    --no-fail-fast"                    │
                    └───────────────┬───────────────────────┘
                                    │ starts Mailpit sidecar,
                                    │ runs rust:1 container
                                    │ (seccomp=unconfined)
                                    ▼
        ┌───────────────────────────────────────────────────────────┐
        │                Full workspace test run (Linux)             │
        │  ┌───────────────┐ ┌───────────────┐ ┌──────────────────┐ │
        │  │ audit.rs unit │ │ harden01_      │ │ replay_cas.rs    │ │
        │  │ tests (chain  │ │ session_       │ │ (Linux-gated,    │ │
        │  │ forge/trunc.) │ │ integrity.rs   │ │ CAS replay)      │ │
        │  │ Criterion 2   │ │ Criterion 5    │ │ Criterion 3      │ │
        │  └───────────────┘ └───────────────┘ └──────────────────┘ │
        │  ┌───────────────────────────────────────────────────────┐│
        │  │ harden04_featureless_create_session.rs                ││
        │  │ SELF-SKIPS here (brokerd::TEST_FIXTURES_ACTIVE==true, ││
        │  │ ambient feature unification under --workspace)        ││
        │  │ Criterion 4 -- NOT actually proven by this path        ││
        │  └───────────────────────────────────────────────────────┘│
        └───────────────────────────────────────────────────────────┘
                                    │
                                    │  (Phase 30 must ADD this leg)
                                    ▼
        ┌───────────────────────────────────────────────────────────┐
        │      NEW: scripts/verify-harden04-featureless.sh (or       │
        │      an added step in mailpit-verify.sh)                   │
        │  1. cargo build --workspace --release   (featureless,     │
        │     no dev-dep feature unification: no test targets built) │
        │  2. scoped: MAILPIT_VERIFY_CMD='cargo test -p caprun       │
        │     --test harden04_featureless_create_session'            │
        │     bash scripts/mailpit-verify.sh                         │
        │     -> brokerd::TEST_FIXTURES_ACTIVE == false here,        │
        │        so the real D-10 assertion runs                     │
        │  3. (optional, defense-in-depth only per DESIGN §d)         │
        │     strings target/release/libbrokerd.rlib |                │
        │       grep -c HARDEN04_MINT_ARM_PRESENT_v1_6  == 0          │
        │  Criterion 4 -- proven HERE, not in the bare recipe          │
        └───────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

No new directories. New files land in existing locations:

```
scripts/
└── verify-harden04-featureless.sh   # NEW — formalizes the scoped/featureless HARDEN-04 proof
.planning/phases/30-regression-live-proof/
├── 30-REGRESSION-AUDIT.md            # NEW — mirrors Phase 25's audit doc
└── 30-0N-SUMMARY.md / PLAN.md        # standard GSD artifacts
```

### Pattern 1: Bare-recipe full regression as the default proof surface

**What:** `bash scripts/mailpit-verify.sh` with NO `MAILPIT_VERIFY_CMD` override runs
`cargo build --workspace && cargo test --workspace --no-fail-fast` inside an unprivileged
`rust:1` container on the same Docker network as a live `axllent/mailpit` sidecar.
**When to use:** This is the mandated proof surface for HARDEN-06 criterion 1 (ROADMAP wording
is explicit: "the BARE `scripts/mailpit-verify.sh` recipe").
**Example:**
```bash
# Source: scripts/mailpit-verify.sh (in-repo, read directly)
bash scripts/mailpit-verify.sh
# Captures true exit code BEFORE any pipe — never `| tail` without saving $? first
# (project's own standing "verification-exit-code-through-pipe" lesson).
```

### Pattern 2: Scoped MAILPIT_VERIFY_CMD override for a single-test proof

**What:** `scripts/mailpit-verify.sh` honors `MAILPIT_VERIFY_CMD` as an env override,
substituting the default `cargo test --workspace` invocation with anything the caller supplies,
while still providing the Mailpit sidecar + resolved IP.
**When to use:** Whenever a test needs to run WITHOUT ambient workspace-wide feature
unification — this is exactly HARDEN-04's requirement, since `crates/brokerd`'s own
`[dev-dependencies]` self-dependency on `test-fixtures` gets unified into ANY build graph that
also compiles `crates/brokerd`'s own test targets (i.e., any `--workspace` invocation).
**Example:**
```bash
# Source: scripts/mailpit-verify.sh (existing, in-repo; Phase 16/29 precedent)
MAILPIT_VERIFY_CMD='cargo test -p caprun --test harden04_featureless_create_session' \
  bash scripts/mailpit-verify.sh
```

### Pattern 3: Capture true exit code before any pipe

**What:** Never rely on the exit code of a piped command chain (`script | tail` returns
`tail`'s exit status, always 0).
**When to use:** Every Linux verification invocation in this phase — this is a documented
prior incident (`verification-exit-code-through-pipe`, nearly shipped a false PASS at Phase
15's gate).
**Example:**
```bash
bash scripts/mailpit-verify.sh > /tmp/verify.log 2>&1
rc=$?
tail -40 /tmp/verify.log
echo "exit=$rc"
grep -c "^test .* ok$" /tmp/verify.log   # assert on named-test COUNTS, never bare exit 0
```

### Anti-Patterns to Avoid

- **Trusting the bare `--workspace` run to prove HARDEN-04:** it will report the
  `harden04_featureless_create_session` test as passed (0 assertions actually exercised,
  self-skip branch taken) — a green line that proves nothing about compile-exclusion. This is
  the exact "false-assurance regression test" pattern this project has hit before (Phase 27
  D-10 residual, documented in project memory) — do not let a passing name stand in for a
  passing assertion.
- **Making binary `strings`/symbol-inspection the PRIMARY HARDEN-04 gate:** DESIGN §d
  explicitly rules this out ("optional defense-in-depth only... Do NOT make (a) the primary
  gate") because release optimization can inline/strip either way, causing silent false
  negatives across Rust/LLVM version bumps. Use it as a secondary check only, never the sole
  proof.
- **Writing brand-new negative tests for criteria 2, 3, 5:** these already exist, already
  pass, and already run under the bare recipe. Rewriting them duplicates effort and risks
  introducing a weaker assertion than what's already shipped (this project's own
  false-assurance-regression-test lesson: a passing verifier + green gate previously missed
  real gaps introduced by "helpful" rewrites).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Featureless production-binary proof | A brand-new integration test crate/harness | The existing `cli/caprun/tests/harden04_featureless_create_session.rs`, invoked SCOPED (not `--workspace`) | It already implements the DESIGN-pinned primary mechanism (a behavioral negative test keyed on `brokerd::TEST_FIXTURES_ACTIVE`) — the gap is invocation, not code |
| Full-suite Linux regression runner | A new CI script / GitHub Actions workflow | `scripts/mailpit-verify.sh` (bare, unmodified) | Already handles Mailpit sidecar lifecycle, IP resolution, seccomp flags, libssl-dev install, and command scoping — reinventing it risks losing the Rule-1 DNS-in-confined-process fix and the OPENAI_API_KEY-empty-tolerant forwarding logic |
| Chain-tamper / truncation negative proof | New tamper-simulation tests | `crates/brokerd/src/audit.rs`'s existing `#[cfg(test)]` module (5 tests: forge, key-dependence, truncation, legacy-fail-closed, orphan-injection) | Independently adversarially traced in Phase 28 and confirmed to have "real teeth" (each fails if its guard is removed) — do not duplicate |

**Key insight:** In a proof/regression phase, the highest-value action is auditing what
already exists for gaps in COVERAGE and INVOCATION, not authoring new mechanism code. The one
genuine gap here is procedural (a missing scoped-invocation script), not a missing assertion.

## Common Pitfalls

### Pitfall 1: Ambient Cargo feature unification masking a compile-exclusion proof
**What goes wrong:** Running `cargo test --workspace` (the bare recipe's default) silently
re-enables `brokerd`'s `test-fixtures` feature workspace-wide because `crates/brokerd`'s own
test targets declare a self dev-dependency on that feature — so ANY invocation that also
builds brokerd's own tests (which `--workspace` always does) unifies the feature into every
other crate in the same build graph, including `cli/caprun`.
**Why it happens:** Cargo's feature resolver unifies features across the whole dependency
graph for a single build invocation by default (well-documented Cargo behavior, not a bug).
**How to avoid:** Run the HARDEN-04 proof via a `-p caprun`-scoped invocation (excludes
`crates/brokerd`'s own test targets from the build plan) OR a genuinely featureless `--release`
build with no test targets at all.
**Warning signs:** A test name that always reports "passed" regardless of the underlying
`#[cfg]` gate's correctness; look for a self-detecting skip branch (`if
brokerd::TEST_FIXTURES_ACTIVE { ... return; }`) as here — that's this project's OWN
mitigation for the false-positive risk, already coded at
`cli/caprun/tests/harden04_featureless_create_session.rs:184-226`.

### Pitfall 2: Treating "no numeric baseline" as blocking
**What goes wrong:** There is no single frozen "current full-suite pass count" recorded after
Phase 29 (SUMMARY.md text says "all test binaries green" without an exact tally; the last
NUMERIC tally on record is v1.5 Phase 25's 309 passed/0 failed across 46 suites — stale by
three hardening phases' worth of new tests).
**Why it happens:** Test count naturally grows every phase (Phase 27 added 3+ tests, Phase 28
added ~8, Phase 29 added ~5); no phase before 30 has needed to pin an exact number.
**How to avoid:** Phase 30 ESTABLISHES the new baseline itself — this is literally its job.
Capture the actual `cargo test --workspace --no-fail-fast` pass/fail tally from the real Linux
run and record it in the phase's own SUMMARY/VERIFICATION, rather than trying to match a
stale number.
**Warning signs:** None — this is expected, not a gap.

### Pitfall 3: `s9_control_ab_taint_driven` colocation confusion
**What goes wrong:** Assuming the CONTROL-01 clean-path regression test lives in the same file
as the new HARDEN-01 demotion tests.
**Why it happens:** Both prove "the clean path is unaffected," but they're historically
separate: `fd_grant_on_trusted_path_stays_active` (new, Phase 27,
`crates/brokerd/tests/harden01_session_integrity.rs:397`) is the unit-level fd-grant-only
proof; `s9_control_ab_taint_driven` (pre-existing, `cli/caprun/tests/s9_live_block.rs:813`) is
the full live end-to-end CONTROL-01 send-completes proof.
**How to avoid:** Cite both explicitly in the criterion-5 proof — they are complementary, not
redundant.
**Warning signs:** A plan that only re-runs one and calls criterion 5 "done."

## Code Examples

### The self-skip guard that makes the HARDEN-04 test bare-recipe-safe-but-silent
```rust
// Source: cli/caprun/tests/harden04_featureless_create_session.rs:184-226 (existing, read directly)
if brokerd::TEST_FIXTURES_ACTIVE {
    // ambient unification under `cargo test --workspace` -- non-failing skip,
    // keeps the bare recipe green for a reason UNRELATED to D-10/HARDEN-04.
    eprintln!("harden04_featureless_create_session: SKIPPING the D-10 negative assertion ...");
    server_handle.abort();
    return;
}
// only reached under a genuinely featureless build graph (`cargo test -p caprun --test ...`)
assert!(matches!(resp, BrokerResponse::Error { .. }), "D-10 VIOLATION: ...");
```

### The compile-time exclusion this test proves
```rust
// Source: crates/brokerd/src/server.rs (existing; two cfg-gated siblings, read directly)
// #[cfg(any(test, feature = "test-fixtures"))] sibling: mints Active, env-gated (defense-in-depth)
// #[cfg(not(any(test, feature = "test-fixtures")))] sibling: unconditional Error, NO env::var read
```

### Bare recipe invocation (criterion 1 proof)
```bash
# Source: scripts/mailpit-verify.sh (existing, in-repo)
bash scripts/mailpit-verify.sh
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `nm`/`strings`/`objdump` symbol-grep as the HARDEN-04 proof (what Phase 27's verifier actually ran manually) | A behavioral negative test (`harden04_featureless_create_session.rs`) as the PRIMARY gate, symbol-grep demoted to optional defense-in-depth | DESIGN-security-hardening.md §d, pinned at Phase 26 (before Phase 27 was even implemented) | Phase 30 must NOT re-legitimize the manual `strings` command as the primary Phase 30 artifact — it must formalize the scoped test-invocation path instead |

**Deprecated/outdated:** none — this is the newest phase in the milestone; nothing here is
legacy.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `axllent/mailpit` Docker image resolves to a working, compatible image via `docker pull` with no explicit version pin in the script (pulls `latest`) | Standard Stack | If Mailpit's `latest` tag introduces a breaking API change, `mailpit-verify.sh`'s Task 2/3 SMTP-03/05 acceptance assertions could fail for reasons unrelated to v1.6 hardening — low risk (image has been stable across Phases 13-29), but not independently reverified this session |
| A2 | No numeric "current full-suite pass count" exists post-Phase-29; the last confirmed number (309/0, 46 suites) is stale | Common Pitfall 2 | If Phase 30 treats an old count as a target rather than establishing a fresh one, a legitimate test-count increase from Phases 27-29 could be misread as a discrepancy |

**If this table is empty:** N/A — two low-risk items above, neither blocks planning.

## Open Questions

1. **Should the new HARDEN-04 formalization live as a standalone script (`scripts/verify-harden04-featureless.sh`) or as an added optional step inside `scripts/mailpit-verify.sh` itself?**
   - What we know: DESIGN §j's blast-radius table lists `scripts/mailpit-verify.sh (featureless release build step)` as Phase 30's file, suggesting an in-place addition; but `mailpit-verify.sh`'s existing contract is "run ONE `MAILPIT_VERIFY_CMD` inside the Mailpit-networked container" — a genuinely featureless `--release` build is a DIFFERENT invocation shape (no test targets, no Mailpit sidecar needed).
   - What's unclear: Whether adding an unconditional new step to the shared script risks slowing down every OTHER phase's future use of the bare recipe (this script is reused project-wide, not Phase-30-specific).
   - Recommendation: Add a NEW, separate script (`scripts/verify-harden04-featureless.sh`) that performs the featureless build + scoped test + optional strings check as its own self-contained sequence, and call it explicitly from Phase 30's live-proof plan. This keeps `mailpit-verify.sh`'s existing contract/callers (used by every prior phase) unchanged while still satisfying DESIGN §j's intent. Planner should confirm this choice or defer to Ben.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Colima | Real-Linux verification substrate | ✓ | VM running (aarch64, macOS Virtualization.Framework) `[VERIFIED: colima status]` | — |
| Docker (via Colima socket) | Container runtime for `rust:1` + `axllent/mailpit` | ✓ | 29.6.1 `[VERIFIED: docker info]` | — |
| cargo/rustc | All build/test commands | ✓ | cargo 1.92.0 `[VERIFIED: cargo --version]` | — |
| `scripts/mailpit-verify.sh` | Bare-recipe full regression | ✓ | in-repo, unchanged since Phase 29 | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none — this dev environment already has everything Phase 30 needs.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in libtest), workspace-wide (`resolver = "3"`, edition 2021) |
| Config file | root `Cargo.toml` (workspace members); no separate test-framework config |
| Quick run command | `cargo test -p <crate> --test <target> <test_name>` (e.g. `cargo test -p brokerd --lib self_consistent_forgery_without_key_is_rejected`) |
| Full suite command | `bash scripts/mailpit-verify.sh` (bare, real Linux via Colima; on macOS: `cargo build --workspace && cargo test --workspace --no-fail-fast`, but Linux-gated tests show 0 on Mac by design per CLAUDE.md) |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|---------------------|--------------|
| HARDEN-06 (criterion 1: full regression) | No regression to v1.1-v1.5 behavior across the whole workspace | integration/e2e (whole suite) | `bash scripts/mailpit-verify.sh` | ✅ (script exists, unchanged) |
| HARDEN-06 (criterion 2: forged chain rejected) | `verify_chain` rejects a self-consistent forged chain and a truncated tail | unit | `cargo test -p brokerd --lib self_consistent_forgery_without_key_is_rejected tail_truncation_detected_via_anchor_mismatch legacy_db_without_anchor_fails_closed` | ✅ `crates/brokerd/src/audit.rs:1522,1673,1779` |
| HARDEN-06 (criterion 3: replay delivers once) | A replayed Allowed `email.send` sends exactly once | integration, Linux-gated | `MAILPIT_VERIFY_CMD='cargo test -p brokerd --test replay_cas allowed_email_send_replay_delivers_once' bash scripts/mailpit-verify.sh` | ✅ `crates/brokerd/tests/replay_cas.rs` (`#![cfg(target_os = "linux")]`) |
| HARDEN-06 (criterion 4: compile-exclusion) | Forced-Active `CreateSession` arm absent from a featureless build | integration, Linux-gated, SCOPED (not `--workspace`) | `MAILPIT_VERIFY_CMD='cargo test -p caprun --test harden04_featureless_create_session' bash scripts/mailpit-verify.sh` PLUS `cargo build --workspace --release` (featureless artifact) | ✅ test exists (`cli/caprun/tests/harden04_featureless_create_session.rs`); ❌ formalized scoped-invocation SCRIPT — Wave 0 gap |
| HARDEN-06 (criterion 5: fd-release demotion + clean path) | `RequestFd` on untrusted path demotes to Draft; trusted path + CONTROL-01 stay unaffected | integration | `cargo test -p brokerd --test harden01_session_integrity` PLUS `MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_control_ab_taint_driven' bash scripts/mailpit-verify.sh` | ✅ `crates/brokerd/tests/harden01_session_integrity.rs`, `cli/caprun/tests/s9_live_block.rs:813` |

### Sampling Rate
- **Per task commit:** targeted `cargo test -p <crate> <test_name>` for whatever the task touched.
- **Per wave merge:** `cargo build --workspace && ./scripts/check-invariants.sh` (macOS is sufficient for wiring checks; Linux-gated behavior deferred to phase gate).
- **Phase gate:** bare `bash scripts/mailpit-verify.sh` (full regression, criterion 1) PLUS the new `scripts/verify-harden04-featureless.sh` (criterion 4) — BOTH must be green before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `scripts/verify-harden04-featureless.sh` — does not exist yet; formalizes the DESIGN §j-pinned featureless-build + scoped-test proof for criterion 4. This is the ONE required new artifact this phase.
- [ ] `.planning/phases/30-regression-live-proof/30-REGRESSION-AUDIT.md` — mirrors Phase 25's `25-REGRESSION-AUDIT.md`; sweep all Phase 27/28/29 test files for `#[ignore]`, weakened assertions, or stale skip conditions introduced since their own verification passes.

*(No Wave 0 gap for criteria 1, 2, 3, 5 — all their tests already exist and already pass; no new test-framework scaffolding is needed.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | no | N/A — no auth surface touched this phase |
| V3 Session Management | yes | Re-verifies existing session-trust-state demotion (HARDEN-01) via existing tests; no new session mechanism |
| V4 Access Control | yes | Re-verifies existing slot-type-binding + I2 sink gating remains intact (regression, not new mechanism) |
| V5 Input Validation | no (indirectly, via regression) | Re-verified transitively by the full-suite run; no new validation logic |
| V6 Cryptography | yes | Re-verifies existing keyed-HMAC audit chain (HARDEN-02) via existing tests; no new crypto |

This phase is a **verification-only** phase for the ASVS categories already hardened in
Phases 27-29 — it introduces no new cryptographic, session, or access-control code of its own
(the one new script is a build/test orchestration artifact, not a TCB component).

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|------------------------|
| False-assurance regression test (a test whose skip/pass path is triggered by the very outcome it should catch) | Repudiation (a "proof" that proves nothing) | Already mitigated in the HARDEN-04 test's own design (self-detects `TEST_FIXTURES_ACTIVE` and errs on the side of a loud, explicit, non-silent skip message rather than a bare `#[ignore]`) — Phase 30's job is to ALSO run the scoped path that actually exercises the assertion, not merely trust the skip-safe green |
| Exit-code-through-a-pipe false PASS | Repudiation | Capture `$?` immediately after `bash scripts/mailpit-verify.sh`, before any `| tail`/`| grep` piping — documented project incident, nearly shipped a false PASS at Phase 15 |

## Sources

### Primary (HIGH confidence — direct codebase read this session)
- `.planning/REQUIREMENTS.md` — HARDEN-06 exact text, traceability table
- `.planning/ROADMAP.md` — Phase 30 success criteria (5), Phase 27/28/29 summaries
- `planning-docs/DESIGN-security-hardening.md` §d, §j, blast-radius table — the DESIGN-pinned HARDEN-04 primary-gate ruling and Phase 30 proof-plan note
- `.planning/phases/27-session-connection-integrity-hardening/27-VERIFICATION.md`
- `.planning/phases/28-authenticated-audit-chain/28-VERIFICATION.md`
- `.planning/phases/29-sink-path-hardening-replay-cas-contents-slot/29-VERIFICATION.md`
- `crates/brokerd/tests/harden01_session_integrity.rs` (read directly, function names + line numbers confirmed via grep)
- `crates/brokerd/src/audit.rs` (test names + line numbers confirmed via grep: 1522, 1613, 1673, 1725, 1779)
- `crates/brokerd/tests/replay_cas.rs` (confirmed `#![cfg(target_os = "linux")]` at line 24)
- `cli/caprun/tests/harden04_featureless_create_session.rs` (full file read; self-skip logic confirmed at lines 184-226)
- `crates/brokerd/src/lib.rs:30` — `TEST_FIXTURES_ACTIVE` const definition
- `cli/caprun/tests/s9_live_block.rs:813` — `s9_control_ab_taint_driven` confirmed present
- `scripts/mailpit-verify.sh` (full file read)
- `.planning/phases/25-regression-live-proof/` (prior analogous phase, 3-plan shape) — used as structural precedent, not literal reuse
- CLI probes this session: `colima status`, `docker info`, `cargo --version` (environment availability)

### Secondary (MEDIUM confidence)
- None used — this research was entirely code-grounded, no external documentation lookup was needed for a proof/regression phase.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new libraries, all tooling already in use and version-confirmed in this environment.
- Architecture: HIGH — every claim traced to an exact file:line in the current codebase this session, not inferred from summaries.
- Pitfalls: HIGH — the central pitfall (feature-unification self-skip) is directly confirmed in source code, not speculative.

**Research date:** 2026-07-17
**Valid until:** Effectively phase-scoped (this research describes the current, frozen state of Phases 27-29's test suite as of this commit) — re-verify file:line anchors if any commit lands on `crates/brokerd` or `cli/caprun` test files between this research and plan execution.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|--------------------|
| HARDEN-06 | After all hardening lands, the full workspace regression is independently re-run green on real Linux via the bare `scripts/mailpit-verify.sh` recipe, with new negative tests proving each closed residual (forged/tampered chain rejected; replayed Allowed send delivers exactly once; forced-Active path absent from the built binary; fd release demotes the session; `file.create` `contents` slot constrained) — and no regression to v1.1-v1.5 behavior. | This research maps each of the 5 embedded success criteria to EXISTING test artifacts (4 of 5 fully satisfied already) and identifies the ONE genuine gap (HARDEN-04's formalized scoped-invocation proof), giving the planner a precise per-criterion action list instead of an open-ended "write 5 negative tests" scope. |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **Linux-only security claims:** all v0/v1 security enforcement/negative-assertion/e2e tests are `#[cfg(target_os = "linux")]`; `cargo test` on macOS legitimately shows these as "0 passed" — never "fix" this by removing cfg gates.
- **Mailpit gate mandatory since Phase 16:** ALL Linux verification for this milestone goes through `scripts/mailpit-verify.sh`, never a bare `docker run rust:1` recipe — a benign Allowed run can trigger a live SMTP send (CONTROL-01), so the Mailpit sidecar must always be present.
- **No `--privileged`:** the confinement stack under test is fully unprivileged; only `--security-opt seccomp=unconfined` is required (default seccomp profile blocks the `landlock()`/`seccomp()` syscalls under test).
- **`EffectRequest` token banned:** `check-invariants.sh` Gate 1 fails the build if the literal token `EffectRequest` appears under `crates/` (not relevant to this phase's own new files, but any grep sweep this phase performs should not introduce it).
- **Surgical changes only:** every changed line must trace to HARDEN-06's actual scope (regression proof + the 5 negative-test criteria) — no drive-by refactoring of Phase 27/28/29 code.
- **NEVER downgrade errors to warnings / soften a safety mechanism:** if the regression audit finds a weakened assertion or an `#[ignore]`, it must be surfaced and fixed, not documented-around.
- **Terminology locked:** `Intent`, `Session`, `Planner`, `Worker`, `Broker`, `Adapter`, `Effect`, `Artifact`, `Event`; project/binary name is `caprun` (never "AgentOS" in code/docs).
- **TCB is Rust only:** any new script this phase adds is verification tooling (shell), not TCB code — Python is not applicable here either way (no new production logic is being written).

## Per-Criterion Verdict Table

| # | Criterion | Verdict | Existing Test (file:test) | Gap / Action |
|---|-----------|---------|------------------------------|----------------|
| 1 | Full workspace regression green via bare `scripts/mailpit-verify.sh` (no override), no regression to v1.1-v1.5 | **PARTIAL** (mechanism exists, needs a fresh run) | N/A — this is a whole-suite run, not a single test | Phase 30 must actually EXECUTE `bash scripts/mailpit-verify.sh` on real Linux and record the fresh pass/fail tally as this milestone's new baseline (last recorded number, 309/0 @ 46 suites, is stale by 3 phases) |
| 2 | Forged/tampered audit chain rejected by `verify_chain` (forge AND truncation) | **EXISTS** | `crates/brokerd/src/audit.rs:1522` `self_consistent_forgery_without_key_is_rejected`; `:1673` `tail_truncation_detected_via_anchor_mismatch`; `:1779` `legacy_db_without_anchor_fails_closed`; `:1725` `orphan_event_injection_detected_via_live_count` (bonus, F2-fix regression) | None — already runs under the bare recipe (unit tests, cross-platform, not Linux-gated). Just cite + re-run. |
| 3 | Replayed Allowed `email.send` delivers exactly once | **EXISTS** | `crates/brokerd/tests/replay_cas.rs::allowed_email_send_replay_delivers_once` (`#![cfg(target_os = "linux")]`) | None — already Linux-gated, already runs under the bare `--workspace` recipe, already independently re-verified live on real Linux in Phase 29's own verification. Just cite + re-run. |
| 4 | Forced-Active `CreateSession` path absent from the built production binary | **PARTIAL** — test exists but is SILENTLY INERT under the bare recipe | `cli/caprun/tests/harden04_featureless_create_session.rs::featureless_create_session_denied_even_with_flag_set` — self-skips (lines 184-226) when `brokerd::TEST_FIXTURES_ACTIVE` is true, which it ALWAYS is under `cargo test --workspace` | **Genuine gap.** Phase 30 must add a NEW script (recommend `scripts/verify-harden04-featureless.sh`) that runs (a) `cargo build --workspace --release` as the featureless production artifact and (b) `MAILPIT_VERIFY_CMD='cargo test -p caprun --test harden04_featureless_create_session' bash scripts/mailpit-verify.sh` (scoped, avoids `--workspace`'s feature unification) so the assertion actually executes. Optional defense-in-depth: `strings` symbol-absence check (DESIGN explicitly says do NOT make this the primary gate). |
| 5 | `RequestFd` demotes the session; CONTROL-01 clean path still succeeds | **EXISTS** | `crates/brokerd/tests/harden01_session_integrity.rs::fd_grant_on_untrusted_path_demotes_without_report_claims` (demotion) + `::fd_grant_on_trusted_path_stays_active` (clean path, unit-level) + `cli/caprun/tests/s9_live_block.rs:813` `s9_control_ab_taint_driven` (clean path, full live end-to-end) | None — all already run under the bare recipe. Just cite + re-run. |

**Bottom line: 4/5 criteria are already fully satisfied by shipped tests. Only criterion 4 has a genuine proof gap, and it is a formalized-invocation gap, not a missing-code gap.**

## Recommended Plan Count & Wave Structure

**2 plans, 2 waves (sequential — Plan 2 depends on the artifact from Plan 1):**

- **Wave 1 — 30-01-PLAN.md:** Close the one genuine gap (criterion 4) + regression-fixture audit.
  - Task A: Author `scripts/verify-harden04-featureless.sh` (featureless `--release` build + scoped `MAILPIT_VERIFY_CMD` invocation of the existing `harden04_featureless_create_session` test + optional `strings` defense-in-depth check). No changes to `scripts/mailpit-verify.sh` itself (keep its existing contract for all other callers/phases — see Open Question 1).
  - Task B: Regression-fixture audit sweep (mirrors Phase 25's `25-REGRESSION-AUDIT.md`) — grep all Phase 27/28/29 test files for `#[ignore]`, weakened assertions, or stale/misleading skip conditions introduced since their own phase verification passed. Produce `30-REGRESSION-AUDIT.md`.

- **Wave 2 — 30-02-PLAN.md** (depends on 30-01): Live proof + sign-off.
  - Run bare `bash scripts/mailpit-verify.sh` unmodified on real Linux (criterion 1), capturing `$?` before any pipe and the named pass/fail tally as the new baseline.
  - Run `bash scripts/verify-harden04-featureless.sh` (criterion 4, the new artifact from Wave 1).
  - Re-run/cite the already-passing tests for criteria 2, 3, 5 (targeted single-test re-runs, per this project's "verifier re-runs, doesn't trust SUMMARY narrative" discipline).
  - Compile the final per-criterion evidence table into the phase VERIFICATION artifact.
  - Record human DONE sign-off (write it into the artifact BEFORE the last plan flips complete — this project's own "record sign-off before last plan" lesson, to avoid the REQUIREMENTS.md reconciliation lag seen at the end of Phases 27/28/29).

This is NOT a 3-4 plan feature-build phase — the DESIGN doc, the ROADMAP success criteria, and
this research all converge on "mostly a live-proof run plus one small formalization script."
