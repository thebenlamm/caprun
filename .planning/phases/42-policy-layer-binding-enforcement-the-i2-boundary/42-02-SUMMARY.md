---
phase: 42-policy-layer-binding-enforcement-the-i2-boundary
plan: 02
subsystem: containment
tags: [containment, F1, policy-binding, anti-drift, adapter-fs, POLICY-03, MAJOR-2]
requires:
  - "adapter-fs crate (shared dep of cli/caprun + brokerd)"
  - "cli/caprun/src/key.rs::load_or_create_key (inline F1 predicate, pre-extraction)"
provides:
  - "adapter_fs::containment::refuse_if_beneath_workspace(path, workspace_root) — the ONE shared at-or-beneath-workspace-root refusal predicate"
  - "check-invariants.sh Gate 6 — containment-predicate anti-drift (marker-uniqueness + delegation)"
affects:
  - "cli/caprun/src/key.rs (now delegates F1 to the shared helper)"
  - "crates/brokerd/src/policy.rs (Plan 04 will delegate to the same helper)"
tech-stack:
  added: []
  patterns:
    - "Single-source-of-truth security predicate lifted to a shared dependency crate + a grep-based anti-drift gate stamped with a distinctive marker."
key-files:
  created:
    - crates/adapter-fs/src/containment.rs
  modified:
    - crates/adapter-fs/src/lib.rs
    - cli/caprun/src/key.rs
    - scripts/check-invariants.sh
decisions:
  - "Host crate = adapter-fs, not runtime-core (Gate 2 bans std::fs there) and not a new crate (adapter-fs already owns WorkspaceRoot containment and is already a dep of BOTH cli/caprun and brokerd)."
  - "Anti-drift realized as marker-uniqueness (exact-count of a distinctive `containment-predicate` tag) + per-call-site delegation, NOT a bare starts_with token count that would false-positive on unrelated path comparisons."
  - "Predicate logic preserved byte-for-byte from the F1 original — MAJOR-2 confirmed the only gap was factoring, not the (component-wise-correct) logic."
metrics:
  duration: "~7 min"
  completed: 2026-07-18
  tasks: 3
  files: 4
status: complete
---

# Phase 42 Plan 02: Extract Shared Containment Helper (MAJOR-2 fix) Summary

Extracted the inline F1 at-or-beneath-workspace-root refusal predicate out of `cli/caprun/src/key.rs` into ONE shared, unit-tested `adapter_fs::containment::refuse_if_beneath_workspace`, rewired MAC-key custody to delegate to it, and added a marker-based anti-drift gate — closing gate-record MAJOR-2 so the Plan-04 broker policy binder (POLICY-03) can reach the exact same containment logic instead of re-inlining a copy that could drift.

## What Was Built

- **`crates/adapter-fs/src/containment.rs`** (new) — `pub fn refuse_if_beneath_workspace(path, workspace_root) -> anyhow::Result<()>` plus the private `canonicalize_existing_or_parent`. Semantics preserved EXACTLY from the F1 original: `std::fs::canonicalize(workspace_root)` fail-closed (requires-root-exists), candidate resolved via parent-then-rejoin (file need not exist), component-wise `Path::starts_with` refusal. A distinctive `// containment-predicate` marker is stamped on the canonical refusal line. 6 host-portable unit tests: equal-to-root refuse, descendant refuse, **sibling-prefix ACCEPT** (`/ws-foo` vs root `/ws`), unresolvable-root refuse, unresolvable-parent refuse, nonexistent-candidate-with-existing-parent accept.
- **`cli/caprun/src/key.rs`** — `load_or_create_key` now loops over the audit-DB + `.key` candidates and calls the shared helper (refusal still runs FIRST, before any key is generated/read/returned). The inline canonicalize+`starts_with` block AND the local `canonicalize_existing_or_parent` fn are deleted. The `:memory:` carve-out and idempotent read-first custody are untouched.
- **`scripts/check-invariants.sh`** — new **Gate 6**: (6a) the `containment-predicate` marker must appear EXACTLY once across `crates/` + `cli/`, in `crates/adapter-fs/src/containment.rs`; (6b) each known call site (`key.rs`; `brokerd/src/policy.rs` once Plan 04 lands, skipped gracefully until then) must call `refuse_if_beneath_workspace` and carry no canonicalize+prefix-compare block of its own.

## Tasks & Commits

| Task | Name | Type | Commit |
| ---- | ---- | ---- | ------ |
| 1 | Extract shared containment helper into adapter-fs | feat | `efa9ea6` |
| 2 | Rewire key.rs to delegate (delete inline F1) | refactor | `ad45401` |
| 3 | Anti-drift gate in check-invariants.sh | test | `680b465` |

## Verification

- `cargo test -p adapter-fs` — PASS (containment lib unittests 8/0, incl. sibling-prefix accept + both unresolvable-path refusals).
- `cargo test -p caprun` — PASS (all targets green; the 3 surviving key.rs custody tests `f1_refusal_when_audit_under_workspace_root` / `key_file_reused_across_calls` / `memory_audit_path_ephemeral` still pass after the rewire).
- `cargo build --workspace` — compiles clean.
- `./scripts/check-invariants.sh` — exit 0, all 6 gates PASS including new Gate 6.
- **Adversarial negative tests on Gate 6 (per [[false-assurance-regression-test]]):** clean tree exits 0; a duplicate `containment-predicate` marker (verbatim re-inline) drives 6a → exit 1; a divergently-spelled marker-less canonicalize+`starts_with` re-inline in key.rs drives 6b → exit 1. Tree restored after each. The gate is proven to actually catch drift, not a false-PASS.

## Deviations from Plan

None — plan executed exactly as written.

## TDD Gate Compliance

Task 1 was marked `tdd="true"`. Because it is an extraction of already-proven F1 logic (MAJOR-2 confirmed the logic was correct; only the factoring was wrong), the comprehensive test suite was authored alongside the moved implementation and committed together as the `feat(42-02)` commit rather than as a separate RED commit. The tests exercise every `<behavior>` clause (equal / descendant / sibling-prefix accept / unresolvable-root / unresolvable-parent / nonexistent-with-existing-parent) and all pass on the host. No behavior was invented that lacked a test.

## Notes / Follow-ups

- **Linux compile-check ([[cfg-linux-test-blindness]]):** not run in the Colima/Docker container for this plan. Rationale: the changes are host-portable (`canonicalize` works on macOS) and introduce **no cross-crate signature changes** — only an additive `pub fn` (cannot break a `cfg(linux)` caller) and the removal of a `key.rs`-private helper that no Linux-gated code referenced. `load_or_create_key`'s signature is unchanged. Workspace build + full host test run are green. If a belt-and-suspenders Linux target compile-check is desired, `cargo build --tests -p adapter-fs -p caprun` in the standard container suffices.
- **Plan 04 handoff:** when `crates/brokerd/src/policy.rs` binds the session policy (POLICY-03), it MUST call `adapter_fs::containment::refuse_if_beneath_workspace` — Gate 6b will then automatically enforce delegation on that file (it is already listed as a call site and skipped only until the file exists).

## Self-Check: PASSED

- `crates/adapter-fs/src/containment.rs` — FOUND
- Commits `efa9ea6`, `ad45401`, `680b465` — FOUND in git log
- Gate 6 present in `scripts/check-invariants.sh`; full script exits 0
