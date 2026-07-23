---
phase: 45-thin-cli-sdk-read-only-audit-dag-viewer
plan: 03
subsystem: cli/caprun
tags: [audit-viewer, read-only, fail-closed, neutralization, U1]
requires: ["45-01", "45-02"]
provides:
  - "caprun audit <session_id> <audit-db-path> — read-only audit-DAG viewer verb"
  - "key::load_existing_key — load-ONLY fail-closed MAC-key custody"
affects: [cli/caprun]
tech-stack:
  added: []
  patterns:
    - "read-only open-by-path (rusqlite SQLITE_OPEN_READ_ONLY) — no migrations, no writes"
    - "load-only key custody sibling of load_or_create_key (fail-closed on absent / :memory:)"
    - "universal control-char neutralization of every displayed literal via shared brokerd::display fn"
key-files:
  created:
    - cli/caprun/tests/audit_viewer.rs
  modified:
    - cli/caprun/src/key.rs
    - cli/caprun/src/main.rs
decisions:
  - "FAILED verify_chain verdict → non-zero exit (9); absent key / :memory: / bad-arg → exit 1 (fail-closed)"
  - "viewer reuses the print_audit_dag CTE walk in a NEW neutralizing fn rather than editing the run-path print_audit_dag (surgical; run path un-neutralized by design)"
  - "F1 workspace root for the key check is a throwaway <path>.audit-ws sibling (mirrors load_grant_key)"
metrics:
  duration: ~35m
  completed: 2026-07-18
status: complete
---

# Phase 45 Plan 03: Read-Only Audit-DAG Viewer Summary

A read-only `caprun audit <session_id> <audit-db-path>` verb that opens the audit SQLite DB by path read-only, renders the session's events/decisions + a `verify_chain` verdict with every literal control-char-neutralized (U1 M3), and loads the MAC key via a load-ONLY `load_existing_key` that fails closed on an absent key and refuses `:memory:` (U1 M2) — adding no new TCB (pure read reusing `verify_chain`, `query_events_by_session`, the 45-02 neutralizer, and the F1 containment refusal).

## Tasks Completed

1. **`load_existing_key` (WG-4 / U1 M2)** — `cli/caprun/src/key.rs`. The load-ONLY sibling of `load_or_create_key`: refuses `:memory:` (hard `Err`), runs the SAME F1 `refuse_if_beneath_workspace` check on both the audit path and its `.key` sibling, reads back the existing key if present, and hard-errors (writing nothing) if absent. 4 new unit tests. Commit `e4ee506`.
2. **`caprun audit` viewer verb (WG-2 / WG-3 / U1 M2+M3)** — `cli/caprun/src/main.rs`. Dispatch branch alongside confirm/deny/review/grant (fail-closed UUID parse, REQUIRED db-path, no `:memory:` default). `run_audit_viewer` opens the DB by path with `SQLITE_OPEN_READ_ONLY` (never `open_audit_db`), loads the key via `load_existing_key` (fail-closed), renders header + neutralized DAG walk (`render_audit_dag_readonly`) + pending-decision lines + a `verify_chain` verdict. Mints nothing, appends nothing, opens no workspace root, invokes no sink. Commit `30e6394`.
3. **Integration tests** — `cli/caprun/tests/audit_viewer.rs`. Four host-portable legs driving the real binary against a genuine keyed chain seeded via brokerd `append_event`. Commit `92e138e`.

## Commit SHAs

- `e4ee506` feat(45-03): load-only fail-closed load_existing_key for audit viewer
- `30e6394` feat(45-03): caprun audit read-only DAG viewer verb
- `92e138e` test(45-03): audit viewer integration tests

## Test Pass Counts

- `cargo test -p caprun` — **52 passed, 0 failed** (incl. 7 `key::tests` and the 4 `audit_viewer` legs).
- `cargo test -p brokerd` — **329 passed, 0 failed** (unchanged; no brokerd source touched).
- `cargo build --workspace` — clean, no warnings.
- macOS host: `#[cfg(target_os = "linux")]` tests compile to no-ops (EXPECTED). All four viewer legs are host-portable (no confined worker needed — the fixture chain is seeded directly via brokerd's public API), so no Linux gating was required for this plan.

## Security-Requirement Confirmation (U1)

- **Fails CLOSED on absent key (U1 M2, T-45-07):** `load_existing_key` hard-errors when `<db>.key` is absent, writing nothing; `run_audit_viewer` loads the key BEFORE printing anything, so no `Chain verification:` verdict is ever rendered against a fresh/meaningless key. Verified by unit test `load_existing_key_absent_key_errors_and_writes_nothing` + integration leg `absent_key_fails_closed_with_no_verdict` (non-zero exit, no verdict in stdout).
- **Refuses `:memory:` (U1 M2):** refused both at the top of `run_audit_viewer` (before any open) and inside `load_existing_key`. Verified by `load_existing_key_refuses_memory` + `memory_db_is_refused_with_no_verdict`.
- **Opens READ-ONLY (WG-3, T-45-09):** `rusqlite::Connection::open_with_flags(path, SQLITE_OPEN_READ_ONLY)` — never `open_audit_db` (RW + migrations). No mint, no `append_event`, no workspace root, no sink — models the `review` read-only posture. Verified by the render leg (a strictly read-only open of the seeded WAL DB succeeds after the seeding connection checkpoints on close).
- **Neutralizes ALL literals (U1 M3, WG-2):** every displayed field (session id, event_type, actor, hash, parent hash, pending effect_id/sink, audit-path) routes through the shared `brokerd::display::neutralize_control_chars` unconditionally (not the git.push-only guard). Verified by `tainted_actor_literal_is_neutralized` — a `\x1b[2K` actor renders as visible `\x1b`, with NO raw ESC (0x1b) byte in stdout.
- **F1 containment (T-45-10):** the key load runs the SAME `refuse_if_beneath_workspace` check key custody uses (an audit DB at/beneath the workspace root is refused — out of the confined worker's reach). Verified by `load_existing_key_f1_refusal_when_audit_under_workspace_root`.

## check-invariants Result

`bash scripts/check-invariants.sh` — **All invariant gates PASSED.** Gate 1 (no raw EffectRequest in crates/), Gate 2 (runtime-core purity), Gate 3 (mint-call-site restriction — the viewer mints nothing, byte-identical loci), Gate 4/4b (feature gating), Gate 5 (aws-lc-rs absent / no new crate), Gate 6 (containment anti-drift) all pass.

## Deviations from Plan

None of substance. Two minor, plan-sanctioned choices:
- The viewer's neutralizing DAG walk is a NEW `render_audit_dag_readonly` fn rather than a modification of the run-path `print_audit_dag` (the plan permits "reusing `print_audit_dag`'s recursive-CTE ordering but parameterized on this read-only connection"). This keeps the `caprun run` path un-neutralized-by-design and the viewer path fully neutralized — surgical, no existing behavior changed.
- Exit code `9` chosen for a FAILED verdict (fail-closed non-zero), `1` for load/open/arg errors. The plan left the FAILED exit code unspecified; the render leg asserts exit 0 on PASSED and the fail-closed legs assert non-zero.

## Self-Check: PASSED

- `cli/caprun/src/key.rs` — FOUND (load_existing_key + 4 unit tests)
- `cli/caprun/src/main.rs` — FOUND (audit dispatch + run_audit_viewer + load_viewer_key + render_audit_dag_readonly)
- `cli/caprun/tests/audit_viewer.rs` — FOUND (4 integration legs)
- Commits `e4ee506`, `30e6394`, `92e138e` — all present in `git log`.
