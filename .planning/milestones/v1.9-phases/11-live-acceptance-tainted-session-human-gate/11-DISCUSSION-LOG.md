# Phase 11: Live Acceptance — Tainted Session, Human Gate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-07
**Phase:** 11-live-acceptance-tainted-session-human-gate
**Areas discussed:** Test harness format, Human-decision provenance for acceptance, Evidence artifact for milestone close-out

**Mode:** `--auto` — all gray areas auto-selected (recommended default), no interactive questions asked. Single pass per config `workflow.max_discuss_passes`.

---

## Test harness format

| Option | Description | Selected |
|--------|-------------|----------|
| New Linux-gated Rust integration test mirroring `s9_live_block.rs`/`confirm.rs` | Spawn real `caprun` binary, isolated temp dir + audit.db per run, assert exit codes + query SQLite directly | ✓ (recommended default — matches Phase 4/7 precedent) |
| Manual runbook/shell script only (no automated test) | Human runs commands by hand, transcript captured manually | |

**Auto-selected:** Rust integration test, same pattern as prior live-acceptance phases.
**Notes:** This project has used the automated-Linux-integration-test pattern for every prior live-acceptance milestone gate (Phase 4 §9, Phase 7 full acceptance). No signal to deviate.

---

## Human-decision provenance for acceptance

| Option | Description | Selected |
|--------|-------------|----------|
| Integration test programmatically invokes real `caprun confirm`/`caprun deny` | Satisfies ACC-01/02 "human decision" via the same CLI a human would type | ✓ (recommended default) |
| Require Ben to personally type the confirm/deny commands during verification | Mirrors literal "human" wording most strictly | |

**Auto-selected:** Programmatic invocation via integration test satisfies acceptance; Ben's own hands-on run is additive, not required.
**Notes:** Grounded in this project's own `DEC-ai-review-satisfies-human-gate` precedent (Phase 8) and Phase 10's `confirm.rs`, which already cross-process-tests the confirm/deny mechanism this way.

---

## Evidence artifact for milestone close-out

| Option | Description | Selected |
|--------|-------------|----------|
| Passing test only, no separate write-up | Minimal — rely on `cargo test` output | |
| Passing test + short acceptance record folded into SUMMARY.md/VERIFICATION.md | Captures commands, exit codes, audit-DAG rows for both runs | ✓ (recommended default) |
| Standalone new LIVE-ACCEPTANCE.md doc | Extra artifact, likely unnecessary for a phase-level gate | |

**Auto-selected:** Fold the evidence record into the phase's existing SUMMARY.md/VERIFICATION.md, matching how Phase 7's Colima/Docker re-run was documented.

---

## Claude's Discretion

- Exact test file name/layout, temp-dir/DB naming conventions, one file vs two for deny/confirm.
- The precise `parent_id`/audit-DAG SQL assertions proving "one unbroken causal chain" — flagged as a research question (trace actual wiring in `quarantine.rs`, `server.rs`, `confirmation.rs`) rather than assumed.

## Deferred Ideas

None — discussion stayed within phase scope. `CONTENT-01` and `DOC-01` (v2 requirements) were not raised and remain out of scope for Phase 11.
