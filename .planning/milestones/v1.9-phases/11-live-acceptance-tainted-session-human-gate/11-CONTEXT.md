# Phase 11: Live Acceptance — Tainted Session, Human Gate - Context

**Gathered:** 2026-07-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Prove, live on real Linux `caprun`, that the full v1.2 chain works end to end
for BOTH outcomes: a hostile workspace file is read by the confined worker →
the session is demoted to draft-only (I1, `mint_from_read`) → the same
extracted value routed to a sensitive sink arg is Blocked (I2) → a human
decision via `caprun confirm`/`caprun deny` either releases the effect exactly
once or durably blocks it forever — and the audit DAG shows one unbroken
causal chain (read → demotion → block → decision) for both runs.

This is the v1.2 DONE gate (ACC-01/02/03). No new mechanism is being built —
Phases 9 and 10 already implement session demotion and confirmation release
and are individually unit/integration-tested. Phase 11's job is to wire a
**live, Linux-verified, end-to-end proof** that both pieces compose correctly,
mirroring the discipline used for Phase 4 (v0 DONE §9) and Phase 7 (v1.1 full
acceptance).

</domain>

<decisions>
## Implementation Decisions

### Scenario design — carried forward from DESIGN docs (not re-litigated)
- **D-01:** The hostile workspace file serves double duty: reading it triggers
  `mint_from_read` (I1 demotion, session `Active → Draft`), AND the value
  extracted from that same read is the one routed to the sensitive sink arg
  that Blocks (I2) — this is the exact "genuine chain, not stapled" scenario
  `DESIGN-session-trust-state.md` §11 and `CON-s9-taint-genuineness` require.
  No `--seed-from-file` (I0) needed for the primary scenario — I0 has its own
  coverage from Phase 9 (`origin_seed_provenance.rs`); Phase 11's ACC-01/02
  wording ("worker reads it") is the I1 mid-run demotion path.
- **D-02:** Reuse the existing hardened `file.create` sink (Phase 7) as the
  Blocked sink, not a new sink — out of scope per REQUIREMENTS.md ("More sinks
  beyond file.create/email.send" is explicitly out of scope for v1.2).

### Test harness — auto-selected (recommended default, logged for planner)
- **D-03:** Implement as a new Linux-gated Rust integration test (e.g.
  `cli/caprun/tests/live_acceptance_tainted_session.rs`), following the exact
  pattern of `cli/caprun/tests/s9_live_block.rs` and `cli/caprun/tests/confirm.rs`:
  spawn the real compiled `caprun` binary (`CARGO_BIN_EXE_caprun`) as a
  subprocess per run, isolated temp dir + fresh `audit.db` per run, assert
  exit codes and query the SQLite audit DAG directly (no in-process shortcuts).
  Run via the project's standard Colima+Docker recipe
  (`docker run --rm --security-opt seccomp=unconfined ... cargo test -p caprun --test live_acceptance_tainted_session`).
- **D-04:** Two scenarios, one file, two test functions (or two `#[test]`s in
  the same file) — a deny run and a confirm run — each its own isolated
  temp dir/DB so they can't interfere. Reuses the deny/confirm exit-code
  contract already established in `confirm.rs` (0/2/3/4/5/6).
- **D-05:** "Human decision" for ACC-01/02 acceptance purposes is satisfied by
  the integration test programmatically invoking the real `caprun
  confirm`/`caprun deny` CLI verbs (same binary a human would type) — not a
  requirement that Ben personally type the commands during verification. This
  is consistent with this project's own `DEC-ai-review-satisfies-human-gate`
  precedent (Phase 8) and with how Phase 10's `confirm.rs` already proved the
  confirm/deny mechanism cross-process. If Ben wants to additionally run the
  CLI by hand for his own satisfaction at verification time, that's additive,
  not a planning requirement.

### Evidence artifact — auto-selected (recommended default, logged for planner)
- **D-06:** In addition to the passing integration test, produce a short
  written acceptance record for the milestone close-out (mirrors v1.1 Phase
  7's "verifier independently re-ran the Colima/Docker recipe" narrative) —
  e.g. as part of the phase's `SUMMARY.md`/`VERIFICATION.md`, not a new
  standalone doc. Captures: the two run commands, exit codes, and the queried
  audit-DAG rows proving the unbroken chain, for both deny and confirm.

### Claude's Discretion
- Exact test file name/layout, temp-dir/DB naming, and whether deny/confirm
  live in one file or two — planner/executor's call, following existing
  `s9_live_block.rs`/`confirm.rs` conventions.
- The precise SQL/assertions used to prove "one unbroken causal chain" (which
  event's `parent_id` points to which) — this is architecture, not vision;
  researcher should trace the actual wiring in `quarantine.rs` (`mint_from_read`
  demotion), `server.rs` (block-time `PendingConfirmation` insert), and
  `confirmation.rs` (`confirm`/`deny` event `parent_id`) rather than assume a
  single linear parent chain — recall the locked architectural distinction
  between the causal DAG (`Event.parent_id`) and the value-lineage
  (`anchor.provenance_chain`) graphs; ACC-03's "unbroken chain" claim must be
  proven against whichever of these the actual code produces, not asserted.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & governance
- `planning-docs/DESIGN-session-trust-state.md` §8, §9, §11 — I1/I0 rules, the
  I2-Block-takes-precedence fix, and the explicit "Two-Invariant" framing that
  a Draft session's tainted-arg Block IS the I0/I1 human-gate act
- `planning-docs/DESIGN-confirmation-release.md` — confirm/deny CLI contract,
  exit-code mapping, TCB residency
- `planning-docs/DESIGN-GATE-RECORD-v1.2.md` — design-gate provenance
  (`DEC-ai-review-satisfies-human-gate`)
- `.planning/PROJECT.md` — Locked Decisions, `CON-s9-taint-genuineness`,
  `DEC-ai-review-satisfies-human-gate`
- `.planning/REQUIREMENTS.md` — ACC-01/02/03 exact wording, Out of Scope table

### Precedent acceptance tests (pattern to mirror)
- `cli/caprun/tests/s9_live_block.rs` — live Linux-gated CLI integration test
  pattern (spawn real binary, query audit DB, `#[cfg(target_os = "linux")]`
  gating, Colima/Docker run recipe in the file's doc comment)
- `cli/caprun/tests/confirm.rs` — cross-process confirm/deny integration test
  pattern (persistent SQLite DB across separate `caprun` invocations)
- `crates/brokerd/tests/durable_anchor.rs` — audit-DAG SQL assertion pattern
  (`assert_anchored_event`, `parent_id` checks)
- `.planning/phases/07-file-create-sink-enforcement-hardening-full-acceptance/07-CONTEXT.md`
  — prior live-acceptance phase's captured decisions (ACC-05/07 anti-stapling
  discipline)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli/caprun/src/main.rs` `run_confirm_or_deny` — already dispatches `caprun
  confirm <effect_id>`/`caprun deny <effect_id>` to `brokerd::confirmation`,
  exit codes 0/2/3/4/5/6 fully mapped (`confirm`/`deny` need no new CLI code)
- `crates/brokerd/src/confirmation.rs` — `confirm`/`deny`/`render_block_display`
  already implemented and unit/integration-tested (Phase 10)
- `crates/brokerd/src/quarantine.rs::mint_from_read` — already performs the
  atomic I1 demotion (session status UPDATE + `session_demoted` event, same
  `conn`) — Phase 11 exercises this, does not modify it
- Existing hardened `file.create` sink (`crates/brokerd/src/sinks/file_create.rs`)
  — reuse as-is

### Established Patterns
- Every prior live-acceptance phase (4, 7) used a Linux-gated `#[cfg(target_os
  = "linux")]` integration test under `cli/caprun/tests/`, spawning the real
  binary via `CARGO_BIN_EXE_caprun`, with cross-platform guard tests running on
  macOS so `cargo test` isn't silently "0 passed" without explanation.
- Audit-DAG proofs query SQLite directly (not just trust exit codes) —
  `assert_anchored_event`-style helpers already exist in
  `crates/brokerd/tests/durable_anchor.rs` and can likely be reused/adapted.

### Integration Points
- New test file lives in `cli/caprun/tests/` alongside `s9_live_block.rs` and
  `confirm.rs`; registered as a `[[test]]` in `cli/caprun/Cargo.toml` if needed
  (check how `confirm.rs` was registered — Phase 10 already did this).

</code_context>

<specifics>
## Specific Ideas

No particular UI/UX or naming preferences volunteered — this is a backend/CLI
security-acceptance phase with the scenario itself already fully specified by
the DESIGN docs. Auto-mode selected the recommended default at every gray
area, grounded in this project's own established precedent (Phase 4, Phase 7
live-acceptance pattern; Phase 8's `DEC-ai-review-satisfies-human-gate`).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. (v2 items already tracked in
REQUIREMENTS.md: `CONTENT-01` content-sensitive sink args, `DOC-01` README
positioning vs CaMeL — neither came up here and neither belongs in Phase 11.)

</deferred>

---

*Phase: 11-live-acceptance-tainted-session-human-gate*
*Context gathered: 2026-07-07*
