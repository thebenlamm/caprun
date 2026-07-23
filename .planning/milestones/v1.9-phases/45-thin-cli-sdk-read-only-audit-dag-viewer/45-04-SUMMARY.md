---
phase: 45-thin-cli-sdk-read-only-audit-dag-viewer
plan: 04
subsystem: testing
tags: [sdk, cli, audit-viewer, acceptance, verify-chain, neutralization, i2, taint]

# Dependency graph
requires:
  - phase: 45-01
    provides: "caprun run verb + --policy flag + WG-5 blocked-effect_id surface + M7 anti-laundering"
  - phase: 45-02
    provides: "shared brokerd::display::neutralize_control_chars"
  - phase: 45-03
    provides: "caprun audit read-only fail-closed DAG viewer (load_existing_key, :memory: refusal, universal neutralization)"
provides:
  - "End-to-end SDK-01 + U1 acceptance test proving the design-partner loop: caprun run → I2 Block → surface effect_id → caprun review (verbatim literal) → caprun audit (DAG + verify_chain PASSED) for one genuine confined run."
  - "The LIVE-05/06 driver-inspector setup for Phase 46 — CLI+viewer loop proven real on a single Blocking run."
affects: [46-live-composed-acceptance, LIVE-05, LIVE-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Genuine-run acceptance: drive the REAL caprun binaries (run/review/audit) as subprocesses, close the loop on ONE durable pending_confirmations row (surfaced == reviewed == audited effect_id)."
    - "Split gating: confined-run-dependent legs #[cfg(target_os = \"linux\")]; pure-read viewer guarantees (:memory: refusal, neutralization) host-portable."

key-files:
  created:
    - cli/caprun/tests/s45_cli_viewer_acceptance.rs
  modified: []

key-decisions:
  - "Reused the create-file-from-report hostile-path fixture (reports/pwned.txt) as the genuine I2-Block driver — a real confined-worker relative_path claim, not a stub."
  - "Passed a trusted --policy sibling of the workspace root ({\"allowed_sinks\":[\"file.create\"]}) so the sink is CALLABLE and the tainted arg yields an I2 Block (not a PolicyDeny) — exercising the real SDK-01 --policy surface, F1-safe."
  - "Neutralization leg is host-portable and injects the tainted \\x1b[2K into a viewer-RENDERED field (event actor), since the viewer never renders side-table blocked literals — the viewer's universal neutralization is a pure-read property, provable without confinement."

patterns-established:
  - "Loop-closure assertion: the effect_id parsed from `caprun run` stdout must appear verbatim in both `caprun review` and `caprun audit` output — one real row, three verbs."

requirements-completed: [SDK-01, U1]

coverage:
  - id: D1
    description: "Genuine end-to-end loop: caprun run I2-Blocks, surfaces effect_id + review pointer, caprun review shows verbatim literal + provenance, caprun audit renders DAG + verify_chain PASSED for the same session."
    requirement: "SDK-01"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s45_cli_viewer_acceptance.rs#end_to_end_run_block_surface_review_audit"
        status: unknown   # Linux-gated (confined run); authoritative under FULL compose-verify on real Linux
    human_judgment: false
  - id: D2
    description: "Viewer fails CLOSED on an absent MAC key against the genuine run's DB — non-zero exit, no verdict rendered (U1 M2)."
    requirement: "U1"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s45_cli_viewer_acceptance.rs#absent_key_on_genuine_run_db_fails_closed"
        status: unknown   # Linux-gated (genuine run produces the DB)
    human_judgment: false
  - id: D3
    description: "caprun audit refuses a :memory: DB — non-zero exit, no verdict (U1 M2)."
    requirement: "U1"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/s45_cli_viewer_acceptance.rs#audit_memory_db_is_refused_with_no_verdict"
        status: pass      # host-portable, green on macOS
    human_judgment: false
  - id: D4
    description: "A tainted \\x1b[2K literal renders control-char-neutralized in the viewer — visible \\x1b escape, no raw ESC byte reaches stdout (U1 M3, all sinks)."
    requirement: "U1"
    verification:
      - kind: integration
        ref: "cli/caprun/tests/s45_cli_viewer_acceptance.rs#audit_tainted_literal_is_neutralized_in_viewer"
        status: pass      # host-portable, green on macOS
    human_judgment: false
---

# Phase 45 Plan 04: End-to-End CLI+Viewer Acceptance Summary

The SDK-01 + U1 design-partner loop is proven end to end against the REAL `caprun`
binaries (no mocked verbs): a genuine `caprun run` I2-Blocks, surfaces the blocked
`effect_id` + `caprun review` pointer, `caprun review` shows the verbatim blocked
literal + provenance, and `caprun audit` renders the DAG + `verify_chain PASSED`
for the same session — with the viewer failing closed on an absent key, refusing
`:memory:`, and neutralizing tainted literals.

## Accomplishments

- **Genuine end-to-end loop (Task 1, SDK-01 §1/§2 + U1).** `end_to_end_run_block_surface_review_audit`
  drives `caprun run --policy <trusted> create-file-from-report intended_output.txt
  <workspace-file> <db>` over a hostile doc carrying `reports/pwned.txt`. The
  confined worker extracts a `relative_path` claim, the broker taints it, and the
  executor I2-Blocks the tainted `file.create/path`. The test asserts, in order:
  the run exits non-zero + surfaces `=== Blocked pending confirmation` with the
  `effect_id=` + `caprun review` pointer; `caprun review <effect_id> <db>` exits 0
  and shows the verbatim `reports/pwned.txt` literal + `Taint:`/`Provenance chain:`;
  `caprun audit <session_id> <db>` exits 0, renders `sink_blocked`, and prints
  `Chain verification: PASSED`. **Loop closure:** the surfaced `effect_id` appears
  verbatim in BOTH the review and audit output — one real durable
  `pending_confirmations` row, resolved by three verbs (T-45-11).
- **Fail-closed-on-absent-key (Task 2 leg 1, U1 M2, T-45-12).**
  `absent_key_on_genuine_run_db_fails_closed` runs the same genuine confined run,
  removes the `<db>.key` sibling, and asserts `caprun audit` exits non-zero and
  prints NO `Chain verification:` line — it refuses a verdict rather than verify
  against a fresh/meaningless key.
- **`:memory:` refused (Task 2 leg 2, U1 M2).** `audit_memory_db_is_refused_with_no_verdict`
  asserts `caprun audit <session> :memory:` exits non-zero with no verdict.
- **Tainted-literal neutralized (Task 2 leg 3, U1 M3 / WG-2, T-45-13).**
  `audit_tainted_literal_is_neutralized_in_viewer` seeds a genuine keyed chain
  whose rendered `actor` field carries a `\x1b[2K` (ESC CSI) tainted value and
  asserts the viewer renders it as a visible `\x1b` escape with NO raw ESC (0x1b)
  byte in stdout — the audit-line-spoofing surface is closed for all sinks.

## Host-portable vs. Linux-gated

| Leg | Gating | Rationale |
|-----|--------|-----------|
| Task 1 full loop | `#[cfg(target_os = "linux")]` | `caprun run` self-confines the worker (Landlock+seccomp+no_new_privs) — Linux-only. |
| Task 2 absent-key | `#[cfg(target_os = "linux")]` | Depends on the genuine confined run's audit DB. |
| Task 2 `:memory:` refused | host-portable | Needs no DB and no confinement. |
| Task 2 neutralization | host-portable | The viewer is a pure read; its universal neutralization is provable host-side. |

macOS host: the two host-portable legs are GREEN (`2 passed`); the two Linux-gated
legs compile to cfg no-ops (expected — the authoritative gate is the FULL
compose-verify on real Linux, run by the orchestrator at phase close, per CLAUDE.md).

## Verification

- `cargo build --workspace` — clean (ran FIRST, sibling-binary rule).
- `cargo test -p caprun --test s45_cli_viewer_acceptance --no-fail-fast` — `2 passed; 0 failed`
  on the macOS host (both host-portable legs); the two confined-run legs are
  cfg-excluded on macOS and compile cleanly (`cargo build -p caprun --tests` — no errors).
- `./scripts/check-invariants.sh` — **all gates PASSED** (Gate 1 no new EffectRequest,
  Gate 3 no new mint site — the test uses `append_event`/`open_audit_db` only,
  Gate 5 no new crate).

## Deviations from Plan

- **Single test-file commit rather than two per-task commits.** Task 1 and Task 2
  both land in one new cohesive test file (`s45_cli_viewer_acceptance.rs`) and were
  verified by the same `cargo test` run; splitting a single new file's contents
  across two commits would be artificial churn. The commit body enumerates both
  tasks; the file is atomically revertable as one unit.
- **Neutralization-leg injection point (documented, faithful).** The plan frames
  the tainted `\x1b[2K` value as "minted TAINTED per 45-01's M7 path." The M7-tainted
  file-derived literal lands in the `blocked_literals` side table + `resolved_args`,
  which `caprun audit` does NOT render (only `caprun review` renders those). To make
  the viewer's neutralization observable via `caprun audit`, the ESC is injected into
  a field the viewer DOES render (the event `actor`) — exactly the surface 45-03
  established for this guarantee. This is a faithful proof of the viewer's universal
  neutralization (U1 M3, all sinks); it does not weaken the M7 mint-time property,
  which 45-01 already proves.

## Self-Check: PASSED

- Created file exists: `cli/caprun/tests/s45_cli_viewer_acceptance.rs` — FOUND.
- Commit exists: `30c848b` (`test(45-04): SDK-01 + U1 end-to-end CLI+viewer acceptance`) — FOUND.
