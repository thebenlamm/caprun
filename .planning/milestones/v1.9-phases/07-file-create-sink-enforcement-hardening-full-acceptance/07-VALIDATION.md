---
phase: 7
slug: file-create-sink-enforcement-hardening-full-acceptance
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-30
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust workspace, `resolver = "3"`) |
| **Config file** | `Cargo.toml` (workspace root) |
| **Quick run command** | `cargo test -p <crate> --no-fail-fast` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` |
| **Estimated runtime** | ~60–120 seconds (macOS, cross-platform tests only) |

**Linux-gated tests (critical):** all `file.create`/`openat2`/enforcement/e2e §9 tests are `#[cfg(target_os = "linux")]` and show "0 passed" on macOS — that is expected, not a gap. Run them via Colima+Docker per the CLAUDE.md recipe: `docker run --rm --security-opt seccomp=unconfined -v "$PWD":/work -w /work -e CARGO_TARGET_DIR=/tmp/lt rust:1 cargo test --workspace --no-fail-fast`. Kernel ≥5.13. The ACC-07 durable-anchor + mint-invariant tests are **cross-platform** (no Linux gate).

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <crate> --no-fail-fast` for the touched crate + `./scripts/check-invariants.sh`
- **After every plan wave:** Run `cargo test --workspace --no-fail-fast` (macOS) — and the Linux Docker suite after any `openat2`/enforcement/e2e wave
- **Before `/gsd-verify-work`:** Full workspace suite green on macOS AND the Linux-gated §9 suite green in the container
- **Max feedback latency:** ~120 seconds (macOS); Linux container adds a build cold-start

---

## Per-Task Verification Map

*Populated by the planner/executor. Every Success Criterion and requirement must map to a concrete test layer. See `07-RESEARCH.md` → `## Validation Architecture` for the requirement → test-layer map that seeds this table.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 7-01-01 | 01 | 1 | HARD-02/mint | T-04-03 | `ValueStore::mint` rejects empty taint/provenance | unit | `cargo test -p executor value_store` | ❌ W0 | ⬜ pending |

**Non-inferable / backstop checks (call out explicitly):**
- **ACC-07 after-exit anti-stapling sentinel** — file-backed DB, drop+reopen, `verify_chain` passes FIRST then anchor is trusted; `file_read` Event `id == anchor.read_event_id` with untrusted taint; event-order-only assertion is INSUFFICIENT. Test: `crates/brokerd/tests/durable_anchor.rs` (new).
- **Tamper-evidence** — `UPDATE` the real `payload` column to change the literal → `verify_chain` returns **false** (mutate the DB, not memory).
- **Golden serde byte-fixture** — existing events (`anchor=None`) round-trip byte-identical.
- **Negative:** `append_event` of a `sink_blocked` with `anchor=None` → `Err`; forged `ValueId` → `Denied(DanglingHandle)`; cross-session handle access denied; unknown sink/arg → fail closed.

---

## Wave 0 Requirements

- [ ] `crates/brokerd/tests/durable_anchor.rs` — ACC-07 after-exit + tamper-evidence stubs
- [ ] Golden byte-fixture test for `Event` serde (anchor=None) — proves no DB migration
- [ ] Extend `crates/brokerd/tests/e2e.rs` Linux harness for `file.create` hostile-block + clean-allow

*Existing infrastructure (cargo test, `s9_acceptance.rs`, `e2e.rs`, `check-invariants.sh`) covers the rest.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live §9 file.create block on a real Linux caprun run | ACC-03/04/05 | Requires kernel Landlock+seccomp; runs only in the Colima/Docker Linux container, not macOS CI | Run the CLAUDE.md Docker recipe; assert `sink_blocked` in audit DB, no file written, non-zero exit; then trusted-intent run creates the file |

*All other phase behaviors have automated (in-process or Linux-gated) verification.*

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s (macOS)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
